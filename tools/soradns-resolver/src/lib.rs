#![allow(unexpected_cfgs)]
//! SoraDNS resolver prototype library.
//!
//! The resolver ingests proof bundles and resolver adverts, tracks resolver
//! state, emits change events, and exposes DNS transports (DoH and DoT) that
//! currently resolve against a static record set supplied via configuration.
pub mod bundle;
pub mod canonical;
pub mod config;
pub mod directory;
pub mod dns;
pub mod events;
pub mod limits;
pub mod rad;
pub mod state;
pub mod transparency;
use crate::{
    bundle::ProofBundleV1,
    config::{DotTlsConfig, ResolverConfig},
    events::EventEmitter,
    limits::{
        MAX_STATE_BUNDLES, MAX_STATE_RAD_ENTRIES, MAX_STATE_RETAINED_BYTES, MAX_TLS_CERT_BYTES,
        MAX_TLS_KEY_BYTES, read_bounded_file, read_bounded_private_file, replace_retained_bytes,
    },
    rad::{ResolverAttestation, rad_retained_bytes, validate_rad},
    state::{ResolverState, ResolverStateMetrics},
};
use axum::{
    Router,
    body::{Body, Bytes},
    extract::{DefaultBodyLimit, RawQuery, Request, State as AxumState},
    http::{HeaderMap, Response, StatusCode, header},
    middleware::{self, Next},
    response::sse::{Event as SseEvent, KeepAlive, Sse},
    routing::{get, post},
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use eyre::Result;
use futures::StreamExt;
pub use iroha_primitives::soradns::{
    GatewayHostBindings, GatewayHostError, canonical_gateway_suffix,
    canonical_gateway_wildcard_pattern, derive_gateway_hosts, pretty_gateway_suffix,
};
use norito::json::{self, Value};
use rustls::{
    ServerConfig,
    pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer},
};
use std::{
    collections::HashMap,
    convert::Infallible,
    fmt,
    net::SocketAddr,
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicI64, AtomicU64, Ordering},
    },
    time::Duration,
};
use time::OffsetDateTime;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    signal,
    sync::{OwnedSemaphorePermit, RwLock, Semaphore},
    task::JoinHandle,
    time::{Instant, MissedTickBehavior, interval_at, timeout},
};
use tokio_rustls::TlsAcceptor;
use tokio_stream::wrappers::{BroadcastStream, errors::BroadcastStreamRecvError};
use tracing::{error, info, warn};
const DNS_CONTENT_TYPE: &str = "application/dns-message";
const MAX_DNS_MESSAGE_BYTES_V1: usize = u16::MAX as usize;
const MAX_DOH_GET_ENCODED_BYTES_V1: usize = MAX_DNS_MESSAGE_BYTES_V1.div_ceil(3) * 4;
const MAX_DOT_CONCURRENT_SESSIONS_V1: usize = 256;
const OPERATIONS_AUTH_TOKEN_FILE_MAX_BYTES_V1: usize = 258;
const DOT_TLS_HANDSHAKE_TIMEOUT_V1: Duration = Duration::from_secs(5);
const DOT_IO_TIMEOUT_V1: Duration = Duration::from_secs(15);
const HTTP_CONNECT_TIMEOUT_V1: Duration = Duration::from_secs(5);
const HTTP_REQUEST_TIMEOUT_V1: Duration = Duration::from_secs(15);
const HTTP_REDIRECT_LIMIT_V1: usize = 5;
/// Shared resolver application state guarded by an async `RwLock`.
pub type SharedState = Arc<RwLock<ResolverState>>;
#[derive(Clone, Default)]
struct MetricsRegistry {
    last_sync_unix: Arc<AtomicI64>,
    dns_queries_total: Arc<AtomicU64>,
    dns_failures_total: Arc<AtomicU64>,
    validation_failures_total: Arc<AtomicU64>,
}
impl MetricsRegistry {
    fn new() -> Self {
        Self {
            last_sync_unix: Arc::new(AtomicI64::new(0)),
            dns_queries_total: Arc::new(AtomicU64::new(0)),
            dns_failures_total: Arc::new(AtomicU64::new(0)),
            validation_failures_total: Arc::new(AtomicU64::new(0)),
        }
    }
    fn update_last_sync(&self, unix: i64) {
        self.last_sync_unix.store(unix, Ordering::Relaxed);
    }
    fn last_sync(&self) -> i64 {
        self.last_sync_unix.load(Ordering::Relaxed)
    }
    fn inc_dns_query(&self) {
        let _ = self.dns_queries_total.fetch_add(1, Ordering::Relaxed);
    }
    fn inc_dns_failure(&self) {
        let _ = self.dns_failures_total.fetch_add(1, Ordering::Relaxed);
    }
    fn inc_validation_failure(&self) {
        let _ = self
            .validation_failures_total
            .fetch_add(1, Ordering::Relaxed);
    }
    fn dns_queries_total(&self) -> u64 {
        self.dns_queries_total.load(Ordering::Relaxed)
    }
    fn dns_failures_total(&self) -> u64 {
        self.dns_failures_total.load(Ordering::Relaxed)
    }
    fn validation_failures_total(&self) -> u64 {
        self.validation_failures_total.load(Ordering::Relaxed)
    }
}
/// Resolver daemon orchestrating configuration, state management, and transports.
#[derive(Clone)]
pub struct ResolverDaemon {
    config: ResolverConfig,
    state: SharedState,
    http_client: reqwest::Client,
    events: EventEmitter,
    tls: Option<ResolverTls>,
    event_addr: Option<SocketAddr>,
    metrics: MetricsRegistry,
    operations_authorization: Option<Arc<OperationsAuthorization>>,
}
#[derive(Clone)]
struct ResolverTls {
    rustls: Arc<ServerConfig>,
}
#[derive(Clone)]
struct AppContext {
    state: SharedState,
    metrics: MetricsRegistry,
    sync_interval: Duration,
}
impl AppContext {
    fn new(state: SharedState, metrics: MetricsRegistry, sync_interval: Duration) -> Self {
        Self {
            state,
            metrics,
            sync_interval,
        }
    }
    async fn resolve_dns(&self, message: &[u8]) -> Response<Body> {
        self.metrics.inc_dns_query();
        match dns::decode_message(message) {
            Ok(request) => match resolve_bytes(&self.state, &request).await {
                Some(bytes) => Response::builder()
                    .status(StatusCode::OK)
                    .header("content-type", DNS_CONTENT_TYPE)
                    .body(Body::from(bytes))
                    .unwrap_or_else(|err| {
                        warn!(?err, "failed to build DoH response");
                        self.metrics.inc_dns_failure();
                        build_error_response(
                            StatusCode::INTERNAL_SERVER_ERROR,
                            "doh response failure",
                        )
                    }),
                None => {
                    self.metrics.inc_dns_failure();
                    build_error_response(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "failed to encode dns response",
                    )
                }
            },
            Err(error) => {
                warn!(?error, "failed to decode DoH query");
                self.metrics.inc_dns_failure();
                build_error_response(StatusCode::BAD_REQUEST, "invalid dns message")
            }
        }
    }
    async fn metrics_snapshot(&self) -> ResolverStateMetrics {
        let now = OffsetDateTime::now_utc().unix_timestamp();
        let guard = self.state.read().await;
        guard.metrics_snapshot(now)
    }
    fn last_sync_unix(&self) -> i64 {
        self.metrics.last_sync()
    }
    fn sync_interval(&self) -> Duration {
        self.sync_interval
    }
}
struct OperationsAuthorization {
    token_hash: [u8; 32],
}
impl OperationsAuthorization {
    fn load(path: &Path) -> Result<Self> {
        let mut bytes = read_bounded_private_file(
            path,
            OPERATIONS_AUTH_TOKEN_FILE_MAX_BYTES_V1,
            "SoraDNS operational bearer token",
        )?;
        let token_hash = (|| {
            let token_bytes = if bytes.ends_with(b"\r\n") {
                &bytes[..bytes.len() - 2]
            } else if bytes.ends_with(b"\n") {
                &bytes[..bytes.len() - 1]
            } else {
                bytes.as_slice()
            };
            if !(32..=256).contains(&token_bytes.len())
                || !token_bytes.iter().all(u8::is_ascii_graphic)
            {
                eyre::bail!(
                    "SoraDNS operational bearer token must contain 32 to 256 printable non-whitespace ASCII bytes"
                );
            }
            Ok(*blake3::hash(token_bytes).as_bytes())
        })();
        bytes.fill(0);
        std::hint::black_box(bytes.as_mut_slice());
        Ok(Self {
            token_hash: token_hash?,
        })
    }
    fn matches(&self, candidate: &str) -> bool {
        constant_time_eq_32(
            &self.token_hash,
            blake3::hash(candidate.as_bytes()).as_bytes(),
        )
    }
    fn clear(&mut self) {
        self.token_hash.fill(0);
        std::hint::black_box(&mut self.token_hash);
    }
}
fn constant_time_eq_32(left: &[u8; 32], right: &[u8; 32]) -> bool {
    let mut difference = 0_u8;
    for index in 0..32 {
        difference |= left[index] ^ right[index];
    }
    difference == 0
}
impl fmt::Debug for OperationsAuthorization {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OperationsAuthorization")
            .field("token_hash", &"<redacted>")
            .finish()
    }
}
impl Drop for OperationsAuthorization {
    fn drop(&mut self) {
        self.clear();
    }
}
impl ResolverDaemon {
    /// Create a new resolver daemon. Validates configuration and initialises state.
    pub fn new(config: ResolverConfig) -> Result<Self> {
        config.validate()?;
        let http_client = reqwest::Client::builder()
            .user_agent("soradns-resolver/0.1.0")
            .connect_timeout(HTTP_CONNECT_TIMEOUT_V1)
            .timeout(HTTP_REQUEST_TIMEOUT_V1)
            .redirect(reqwest::redirect::Policy::limited(HTTP_REDIRECT_LIMIT_V1))
            .build()?;
        let tls = match config.dot_tls() {
            Some(tls) => Some(load_tls_configs(tls)?),
            None => None,
        };
        let event_addr = config.event_listen();
        let operations_authorization = config
            .operations_auth_token_path()
            .map(OperationsAuthorization::load)
            .transpose()?
            .map(Arc::new);
        let mut state = ResolverState::new(config.resolver_id.clone(), config.region.clone());
        state.update_static_zones(config.static_zones())?;
        let events =
            EventEmitter::new(config.resolver_id.clone(), config.event_log_path().cloned())?;
        Ok(Self {
            config,
            state: Arc::new(RwLock::new(state)),
            http_client,
            events,
            tls,
            event_addr,
            metrics: MetricsRegistry::new(),
            operations_authorization,
        })
    }
    /// Returns a clone of the shared state handle for background tasks.
    #[must_use]
    pub fn shared_state(&self) -> SharedState {
        Arc::clone(&self.state)
    }
    /// Performs a single synchronization pass and returns the number of tracked zones.
    pub async fn sync_once(&self) -> Result<usize> {
        let loaded = self.fetch_proof_bundles().await?;
        let adverts = self.fetch_rad_entries().await?;
        let mut state = self.state.write().await;
        let bundle_diff = state.update_bundles(loaded)?;
        self.events.emit_bundle_diff(&bundle_diff);
        let resolver_diff = state.update_resolver_adverts(adverts)?;
        self.events.emit_resolver_diff(&resolver_diff);
        let bundle_count = state.bundle_count();
        let rad_count = state.resolver_advert_count();
        let now = OffsetDateTime::now_utc().unix_timestamp();
        let expirations = state.prune_stale_entries(now)?;
        self.events.emit_expirations(&expirations);
        info!(
            bundles = bundle_count,
            rad_entries = rad_count,
            expired_bundles = expirations.expired_bundles.len(),
            expired_resolvers = expirations.expired_resolvers.len(),
            "resolver sync pass completed"
        );
        self.metrics.update_last_sync(now);
        Ok(state.zone_count())
    }
    async fn fetch_proof_bundles(&self) -> Result<HashMap<String, ProofBundleV1>> {
        let mut loaded: HashMap<String, ProofBundleV1> = HashMap::new();
        let mut retained_bytes = 0usize;
        for source in self.config.bundle_sources() {
            match source.fetch(&self.http_client).await {
                Ok(bundles) => {
                    for bundle in bundles {
                        let namehash_hex = hex::encode(bundle.namehash);
                        if let Err(error) = bundle.validate() {
                            warn!(?error, namehash = %namehash_hex, "skipping invalid proof bundle");
                            continue;
                        }
                        let entry_bytes = bundle
                            .retained_bytes()?
                            .checked_add(namehash_hex.capacity())
                            .and_then(|bytes| {
                                bytes.checked_add(
                                    std::mem::size_of::<(String, ProofBundleV1)>()
                                        .saturating_mul(2),
                                )
                            })
                            .ok_or_else(|| eyre::eyre!("proof-bundle sync accounting overflow"))?;
                        let prior_bytes = loaded
                            .get(&namehash_hex)
                            .map(|prior| {
                                prior.retained_bytes().map(|bytes| {
                                    bytes
                                        .saturating_add(namehash_hex.capacity())
                                        .saturating_add(
                                            std::mem::size_of::<(String, ProofBundleV1)>()
                                                .saturating_mul(2),
                                        )
                                })
                            })
                            .transpose()?
                            .unwrap_or(0);
                        let next_retained = replace_retained_bytes(
                            retained_bytes,
                            prior_bytes,
                            entry_bytes,
                            MAX_STATE_RETAINED_BYTES,
                            "proof-bundle sync map",
                        )?;
                        if !loaded.contains_key(&namehash_hex) {
                            if loaded.len() >= MAX_STATE_BUNDLES {
                                eyre::bail!(
                                    "proof-bundle sync map exceeds the {MAX_STATE_BUNDLES}-entry limit"
                                );
                            }
                            loaded.try_reserve(1).map_err(|error| {
                                eyre::eyre!("failed to grow proof-bundle sync map: {error}")
                            })?;
                        }
                        loaded.insert(namehash_hex, bundle);
                        retained_bytes = next_retained;
                    }
                }
                Err(error) => warn!(
                    ?error,
                    "failed to fetch proof bundles from configured source"
                ),
            }
        }
        Ok(loaded)
    }
    async fn fetch_rad_entries(&self) -> Result<HashMap<String, ResolverAttestation>> {
        let mut adverts: HashMap<String, ResolverAttestation> = HashMap::new();
        let mut retained_bytes = 0usize;
        for source in self.config.rad_sources() {
            match source.fetch(&self.http_client).await {
                Ok(entries) => {
                    for advert in entries {
                        if let Err(error) = validate_rad(&advert) {
                            warn!(
                                ?error,
                                resolver = %hex::encode(advert.resolver_id),
                                "skipping invalid RAD entry"
                            );
                            self.metrics.inc_validation_failure();
                            continue;
                        }
                        let resolver_key = hex::encode(advert.resolver_id);
                        let entry_bytes = rad_retained_bytes(&advert)?
                            .checked_add(resolver_key.capacity())
                            .and_then(|bytes| {
                                bytes.checked_add(
                                    std::mem::size_of::<(String, ResolverAttestation)>()
                                        .saturating_mul(2),
                                )
                            })
                            .ok_or_else(|| eyre::eyre!("RAD sync accounting overflow"))?;
                        let prior_bytes = adverts
                            .get(&resolver_key)
                            .map(|prior| {
                                rad_retained_bytes(prior).map(|bytes| {
                                    bytes
                                        .saturating_add(resolver_key.capacity())
                                        .saturating_add(
                                            std::mem::size_of::<(String, ResolverAttestation)>()
                                                .saturating_mul(2),
                                        )
                                })
                            })
                            .transpose()?
                            .unwrap_or(0);
                        let next_retained = replace_retained_bytes(
                            retained_bytes,
                            prior_bytes,
                            entry_bytes,
                            MAX_STATE_RETAINED_BYTES,
                            "RAD sync map",
                        )?;
                        if !adverts.contains_key(&resolver_key) {
                            if adverts.len() >= MAX_STATE_RAD_ENTRIES {
                                eyre::bail!(
                                    "RAD sync map exceeds the {MAX_STATE_RAD_ENTRIES}-entry limit"
                                );
                            }
                            adverts.try_reserve(1).map_err(|error| {
                                eyre::eyre!("failed to grow RAD sync map: {error}")
                            })?;
                        }
                        adverts.insert(resolver_key, advert);
                        retained_bytes = next_retained;
                    }
                }
                Err(error) => warn!(
                    ?error,
                    "failed to fetch resolver adverts from configured source"
                ),
            }
        }
        Ok(adverts)
    }
    /// Run the daemon, spawning DNS transports and the event stream endpoint.
    pub async fn run(&self) -> Result<()> {
        let _ = self.sync_once().await?;
        let mut tasks: Vec<JoinHandle<()>> = Vec::new();
        let sync_interval = self.config.sync_interval();
        if !sync_interval.is_zero() {
            let daemon = self.clone();
            let handle = tokio::spawn(async move {
                let mut ticker = interval_at(Instant::now() + sync_interval, sync_interval);
                ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
                loop {
                    ticker.tick().await;
                    if let Err(error) = daemon.sync_once().await {
                        warn!(?error, "failed to refresh resolver state");
                    }
                }
            });
            tasks.push(handle);
        }
        for &addr in self.config.doh_listen() {
            tasks.push(tokio::spawn(start_doh_server(
                addr,
                self.state.clone(),
                self.metrics.clone(),
                sync_interval,
            )));
        }
        for &addr in self.config.dot_listen() {
            if let Some(tls) = &self.tls {
                tasks.push(tokio::spawn(start_dot_server(
                    addr,
                    Arc::clone(&tls.rustls),
                    self.state.clone(),
                )));
            } else {
                warn!(%addr, "DoT listener requested without TLS configuration; skipping");
            }
        }
        if let Some(addr) = self.event_addr {
            let authorization = Arc::clone(
                self.operations_authorization
                    .as_ref()
                    .expect("validated operational listener has bearer authentication"),
            );
            tasks.push(tokio::spawn(start_event_server(
                addr,
                self.events.clone(),
                AppContext::new(self.state.clone(), self.metrics.clone(), sync_interval),
                authorization,
            )));
        }
        info!("resolver listeners started; waiting for shutdown signal");
        if let Err(error) = signal::ctrl_c().await {
            warn!(?error, "failed to install ctrl-c handler");
        }
        info!("shutdown signal received; terminating listeners");
        for handle in tasks {
            handle.abort();
        }
        Ok(())
    }
}
async fn start_doh_server(
    addr: SocketAddr,
    state: SharedState,
    metrics: MetricsRegistry,
    sync_interval: Duration,
) {
    match TcpListener::bind(addr).await {
        Ok(listener) => {
            info!(%addr, "DoH listener bound");
            let ctx = AppContext::new(state, metrics, sync_interval);
            let router = Router::new()
                .route("/dns-query", get(doh_get))
                .route("/dns-query", post(doh_post))
                .layer(DefaultBodyLimit::max(MAX_DNS_MESSAGE_BYTES_V1))
                .with_state(ctx);
            if let Err(error) = axum::serve(listener, router.into_make_service()).await {
                warn!(%addr, ?error, "DoH server exited with error");
            }
        }
        Err(error) => error!(%addr, ?error, "failed to bind DoH listener"),
    }
}
async fn start_dot_server(addr: SocketAddr, tls_config: Arc<ServerConfig>, state: SharedState) {
    match TcpListener::bind(addr).await {
        Ok(listener) => {
            info!(%addr, "DoT listener bound");
            let acceptor = TlsAcceptor::from(tls_config);
            let session_permits = Arc::new(Semaphore::new(MAX_DOT_CONCURRENT_SESSIONS_V1));
            loop {
                match listener.accept().await {
                    Ok((stream, _peer)) => {
                        let Some(permit) = try_dot_session_permit(&session_permits) else {
                            warn!("DoT session capacity reached; rejecting connection");
                            continue;
                        };
                        let acceptor = acceptor.clone();
                        let state = state.clone();
                        tokio::spawn(async move {
                            let _permit = permit;
                            if let Err(error) = handle_dot_stream(stream, acceptor, state).await {
                                warn!(?error, "DoT session failed");
                            }
                        });
                    }
                    Err(error) => {
                        warn!(%addr, ?error, "DoT accept failed");
                        break;
                    }
                }
            }
        }
        Err(error) => error!(%addr, ?error, "failed to bind DoT listener"),
    }
}
fn request_bearer_token(headers: &HeaderMap) -> Option<&str> {
    let mut values = headers.get_all(header::AUTHORIZATION).iter();
    let value = values.next()?;
    if values.next().is_some() {
        return None;
    }
    let value = value.to_str().ok()?;
    if !(39..=263).contains(&value.len()) {
        return None;
    }
    let mut parts = value.split_ascii_whitespace();
    let scheme = parts.next()?;
    let token = parts.next()?;
    if !scheme.eq_ignore_ascii_case("bearer")
        || !(32..=256).contains(&token.len())
        || !token.bytes().all(|byte| byte.is_ascii_graphic())
        || parts.next().is_some()
    {
        return None;
    }
    Some(token)
}
async fn authorize_operational_request(
    AxumState(authorization): AxumState<Arc<OperationsAuthorization>>,
    request: Request,
    next: Next,
) -> Response<Body> {
    if request_bearer_token(request.headers())
        .is_some_and(|candidate| authorization.matches(candidate))
    {
        let mut response = next.run(request).await;
        response.headers_mut().insert(
            header::CACHE_CONTROL,
            axum::http::HeaderValue::from_static("no-store"),
        );
        response.headers_mut().insert(
            header::PRAGMA,
            axum::http::HeaderValue::from_static("no-cache"),
        );
        return response;
    }
    Response::builder()
        .status(StatusCode::UNAUTHORIZED)
        .header(header::WWW_AUTHENTICATE, "Bearer")
        .header(header::CACHE_CONTROL, "no-store")
        .header(header::PRAGMA, "no-cache")
        .body(Body::from("authentication required"))
        .unwrap_or_else(|_| Response::new(Body::empty()))
}
async fn start_event_server(
    addr: SocketAddr,
    emitter: EventEmitter,
    app_context: AppContext,
    authorization: Arc<OperationsAuthorization>,
) {
    match TcpListener::bind(addr).await {
        Ok(listener) => {
            info!(%addr, "event stream listener bound");
            let event_router = Router::new()
                .route("/events", get(sse_handler))
                .with_state(emitter);
            let telemetry_router = Router::new()
                .route("/metrics", get(metrics_handler))
                .route("/healthz", get(health_handler))
                .with_state(app_context);
            let router =
                event_router
                    .merge(telemetry_router)
                    .layer(middleware::from_fn_with_state(
                        authorization,
                        authorize_operational_request,
                    ));
            if let Err(error) = axum::serve(listener, router.into_make_service()).await {
                warn!(%addr, ?error, "event stream server exited with error");
            }
        }
        Err(error) => error!(%addr, ?error, "failed to bind event listener"),
    }
}
async fn handle_dot_stream(
    stream: tokio::net::TcpStream,
    acceptor: TlsAcceptor,
    state: SharedState,
) -> eyre::Result<()> {
    let mut tls_stream = timeout(DOT_TLS_HANDSHAKE_TIMEOUT_V1, acceptor.accept(stream))
        .await
        .map_err(|_| eyre::eyre!("DoT TLS handshake timed out"))??;
    let mut len_bytes = [0_u8; 2];
    timeout(DOT_IO_TIMEOUT_V1, tls_stream.read_exact(&mut len_bytes))
        .await
        .map_err(|_| eyre::eyre!("DoT frame length read timed out"))??;
    let frame_len = u16::from_be_bytes(len_bytes) as usize;
    let mut payload = vec![0_u8; frame_len];
    timeout(DOT_IO_TIMEOUT_V1, tls_stream.read_exact(&mut payload))
        .await
        .map_err(|_| eyre::eyre!("DoT frame body read timed out"))??;
    if let Ok(request) = dns::decode_message(&payload) {
        if let Some(bytes) = resolve_bytes(&state, &request).await {
            let response_len = dot_response_length_prefix(bytes.len())?;
            timeout(DOT_IO_TIMEOUT_V1, async {
                tls_stream.write_all(&response_len).await?;
                tls_stream.write_all(&bytes).await
            })
            .await
            .map_err(|_| eyre::eyre!("DoT response write timed out"))??;
        }
    } else {
        warn!("failed to decode DoT request");
    }
    timeout(DOT_IO_TIMEOUT_V1, tls_stream.flush())
        .await
        .map_err(|_| eyre::eyre!("DoT response flush timed out"))??;
    Ok(())
}
fn dot_response_length_prefix(response_len: usize) -> eyre::Result<[u8; 2]> {
    let response_len = u16::try_from(response_len)
        .map_err(|_| eyre::eyre!("DoT response exceeds the 65,535-byte framing limit"))?;
    Ok(response_len.to_be_bytes())
}
fn try_dot_session_permit(semaphore: &Arc<Semaphore>) -> Option<OwnedSemaphorePermit> {
    Arc::clone(semaphore).try_acquire_owned().ok()
}
async fn doh_get(
    AxumState(ctx): AxumState<AppContext>,
    RawQuery(raw_query): RawQuery,
) -> Response<Body> {
    let encoded = match parse_doh_get_query(raw_query.as_deref()) {
        Ok(encoded) => encoded,
        Err(message) => return build_error_response(StatusCode::BAD_REQUEST, message),
    };
    match URL_SAFE_NO_PAD.decode(encoded.as_bytes()) {
        Ok(bytes) if bytes.len() <= MAX_DNS_MESSAGE_BYTES_V1 => ctx.resolve_dns(&bytes).await,
        Ok(_) => build_error_response(StatusCode::BAD_REQUEST, "dns parameter is too large"),
        Err(_) => build_error_response(StatusCode::BAD_REQUEST, "invalid base64 dns parameter"),
    }
}
fn parse_doh_get_query(raw_query: Option<&str>) -> std::result::Result<&str, &'static str> {
    let raw_query = raw_query.ok_or("missing dns parameter")?;
    if raw_query.len() > 4 + MAX_DOH_GET_ENCODED_BYTES_V1 {
        return Err("dns parameter is too large");
    }
    let encoded = raw_query
        .strip_prefix("dns=")
        .filter(|value| !value.is_empty() && !value.contains('&'))
        .ok_or("query must contain exactly one dns parameter")?;
    if encoded.len() > MAX_DOH_GET_ENCODED_BYTES_V1 {
        return Err("dns parameter is too large");
    }
    Ok(encoded)
}
async fn doh_post(
    AxumState(ctx): AxumState<AppContext>,
    headers: HeaderMap,
    body: Bytes,
) -> Response<Body> {
    let valid_content_type = headers
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(';').next())
        .is_some_and(|value| value.trim().eq_ignore_ascii_case(DNS_CONTENT_TYPE));
    if !valid_content_type {
        return build_error_response(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "content-type must be application/dns-message",
        );
    }
    ctx.resolve_dns(body.as_ref()).await
}
async fn metrics_handler(AxumState(ctx): AxumState<AppContext>) -> Response<Body> {
    let snapshot = ctx.metrics_snapshot().await;
    let last_sync_unix = ctx.last_sync_unix();
    let labels = format!(
        "resolver_id=\"{}\",region=\"{}\"",
        escape_label_value(&snapshot.resolver_id),
        escape_label_value(&snapshot.region),
    );
    let mut lines = Vec::new();
    lines.push("# HELP soradns_resolver_bundle_count Number of active proof bundles".into());
    lines.push("# TYPE soradns_resolver_bundle_count gauge".into());
    lines.push(format!(
        "soradns_resolver_bundle_count{{{labels}}} {}",
        snapshot.bundle_count
    ));
    lines.push("# HELP soradns_resolver_advert_count Number of resolver adverts tracked".into());
    lines.push("# TYPE soradns_resolver_advert_count gauge".into());
    lines.push(format!(
        "soradns_resolver_advert_count{{{labels}}} {}",
        snapshot.resolver_advert_count
    ));
    lines.push("# HELP soradns_resolver_static_zone_count Configured static zones".into());
    lines.push("# TYPE soradns_resolver_static_zone_count gauge".into());
    lines.push(format!(
        "soradns_resolver_static_zone_count{{{labels}}} {}",
        snapshot.static_zone_count
    ));
    if let Some(age) = snapshot.proof_age_max_secs {
        lines.push(
            "# HELP soradns_resolver_bundle_proof_age_max_seconds Maximum bundle proof age".into(),
        );
        lines.push("# TYPE soradns_resolver_bundle_proof_age_max_seconds gauge".into());
        lines.push(format!(
            "soradns_resolver_bundle_proof_age_max_seconds{{{labels}}} {}",
            age
        ));
    }
    if let Some(ttl) = snapshot.proof_ttl_min_secs {
        lines.push(
            "# HELP soradns_resolver_bundle_proof_ttl_min_seconds Minimum remaining bundle proof TTL"
                .into(),
        );
        lines.push("# TYPE soradns_resolver_bundle_proof_ttl_min_seconds gauge".into());
        lines.push(format!(
            "soradns_resolver_bundle_proof_ttl_min_seconds{{{labels}}} {}",
            ttl
        ));
    }
    lines.push(
        "# HELP soradns_resolver_sync_interval_seconds Configured background refresh cadence"
            .into(),
    );
    lines.push("# TYPE soradns_resolver_sync_interval_seconds gauge".into());
    lines.push(format!(
        "soradns_resolver_sync_interval_seconds{{{labels}}} {}",
        ctx.sync_interval().as_secs_f64()
    ));
    lines.push("# HELP soradns_resolver_last_sync_unix_seconds Unix timestamp of last sync".into());
    lines.push("# TYPE soradns_resolver_last_sync_unix_seconds gauge".into());
    lines.push(format!(
        "soradns_resolver_last_sync_unix_seconds{{{labels}}} {}",
        last_sync_unix
    ));
    lines.push(
        "# HELP soradns_resolver_dns_queries_total DNS queries handled by this resolver".into(),
    );
    lines.push("# TYPE soradns_resolver_dns_queries_total counter".into());
    lines.push(format!(
        "soradns_resolver_dns_queries_total{{{labels}}} {}",
        ctx.metrics.dns_queries_total()
    ));
    lines.push(
        "# HELP soradns_resolver_dns_failures_total DNS queries that failed decoding or response emission"
            .into(),
    );
    lines.push("# TYPE soradns_resolver_dns_failures_total counter".into());
    lines.push(format!(
        "soradns_resolver_dns_failures_total{{{labels}}} {}",
        ctx.metrics.dns_failures_total()
    ));
    lines.push(
        "# HELP soradns_resolver_validation_failures_total Resolver attestation validation failures"
            .into(),
    );
    lines.push("# TYPE soradns_resolver_validation_failures_total counter".into());
    lines.push(format!(
        "soradns_resolver_validation_failures_total{{{labels}}} {}",
        ctx.metrics.validation_failures_total()
    ));
    let body = lines.join("\n") + "\n";
    match Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "text/plain; version=0.0.4")
        .body(Body::from(body))
    {
        Ok(resp) => resp,
        Err(error) => {
            warn!(?error, "failed to build metrics response");
            build_error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "metrics response failure",
            )
        }
    }
}
async fn health_handler(AxumState(ctx): AxumState<AppContext>) -> Response<Body> {
    let snapshot = ctx.metrics_snapshot().await;
    let last_sync_unix = ctx.last_sync_unix();
    let mut map = json::Map::new();
    map.insert("status".into(), Value::from("ok"));
    map.insert("resolver_id".into(), Value::from(snapshot.resolver_id));
    map.insert("region".into(), Value::from(snapshot.region));
    map.insert(
        "bundle_count".into(),
        Value::from(snapshot.bundle_count as u64),
    );
    map.insert(
        "resolver_advert_count".into(),
        Value::from(snapshot.resolver_advert_count as u64),
    );
    map.insert(
        "static_zone_count".into(),
        Value::from(snapshot.static_zone_count as u64),
    );
    map.insert("last_sync_unix".into(), Value::from(last_sync_unix));
    map.insert(
        "sync_interval_secs".into(),
        Value::from(ctx.sync_interval().as_secs()),
    );
    map.insert(
        "dns_queries_total".into(),
        Value::from(ctx.metrics.dns_queries_total()),
    );
    map.insert(
        "dns_failures_total".into(),
        Value::from(ctx.metrics.dns_failures_total()),
    );
    map.insert(
        "validation_failures_total".into(),
        Value::from(ctx.metrics.validation_failures_total()),
    );
    if let Some(age) = snapshot.proof_age_max_secs {
        map.insert("bundle_proof_age_max_secs".into(), Value::from(age));
    }
    if let Some(ttl) = snapshot.proof_ttl_min_secs {
        map.insert("bundle_proof_ttl_min_secs".into(), Value::from(ttl));
    }
    match json::to_string(&Value::Object(map)) {
        Ok(body) => match Response::builder()
            .status(StatusCode::OK)
            .header("content-type", "application/json")
            .body(Body::from(body))
        {
            Ok(resp) => resp,
            Err(error) => {
                warn!(?error, "failed to build healthz response");
                build_error_response(StatusCode::INTERNAL_SERVER_ERROR, "health response failure")
            }
        },
        Err(error) => {
            warn!(?error, "failed to serialise health payload");
            build_error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to serialise health payload",
            )
        }
    }
}
async fn resolve_bytes(state: &SharedState, request: &dns::DnsMessage) -> Option<Vec<u8>> {
    let response = {
        let guard = state.read().await;
        guard.resolve_message(request)
    };
    match dns::encode_message(&response) {
        Ok(bytes) => Some(bytes),
        Err(error) => {
            warn!(?error, "failed to encode DNS response");
            None
        }
    }
}
async fn sse_handler(
    AxumState(emitter): AxumState<EventEmitter>,
) -> Sse<impl futures::Stream<Item = Result<SseEvent, Infallible>>> {
    let stream = BroadcastStream::new(emitter.subscribe()).filter_map(|event| async move {
        match event {
            Ok(payload) => match json::to_value(&payload).and_then(|value| json::to_string(&value))
            {
                Ok(data) => Some(Ok(SseEvent::default().data(data))),
                Err(error) => {
                    warn!(?error, "failed to serialise SSE payload");
                    None
                }
            },
            Err(BroadcastStreamRecvError::Lagged(count)) => {
                warn!(
                    lagged = count,
                    "SSE consumer lagged; dropping {count} events"
                );
                None
            }
        }
    });
    Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("keepalive"),
    )
}
fn load_tls_configs(config: &DotTlsConfig) -> Result<ResolverTls> {
    let certs = load_certs(&config.cert_path)?;
    let key = load_key(&config.key_path)?;
    let mut dot_config = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs.clone(), key.clone_key())?;
    dot_config.alpn_protocols = vec![b"dot".to_vec()];
    let rustls = Arc::new(dot_config);
    Ok(ResolverTls { rustls })
}
fn load_certs(path: &Path) -> Result<Vec<CertificateDer<'static>>> {
    let bytes = read_bounded_file(path, MAX_TLS_CERT_BYTES, "DoT certificate")?;
    let cert = CertificateDer::from(bytes);
    Ok(vec![cert])
}
fn load_key(path: &Path) -> Result<PrivateKeyDer<'static>> {
    let bytes = read_bounded_private_file(path, MAX_TLS_KEY_BYTES, "DoT private key")?;
    Ok(PrivateKeyDer::from(PrivatePkcs8KeyDer::from(bytes)))
}
fn build_error_response(status: StatusCode, message: &str) -> Response<Body> {
    Response::builder()
        .status(status)
        .header("content-type", "text/plain; charset=utf-8")
        .body(Body::from(message.to_string()))
        .unwrap_or_else(|_| Response::new(Body::from("unrecoverable error")))
}
fn escape_label_value(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '"' => escaped.push_str("\\\""),
            _ => escaped.push(ch),
        }
    }
    escaped
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::StaticZone;
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use hickory_proto::{
        op::{Message, MessageType, OpCode, Query, ResponseCode},
        rr::{Name, RData, Record, RecordType, rdata::A},
    };
    use norito::json::Value;
    use reqwest::{
        Client as HttpClient, StatusCode as HttpStatus,
        header::{ACCEPT, CONTENT_TYPE},
    };
    use std::{io::ErrorKind, net::Ipv4Addr, sync::Arc};
    use tokio::time::{Duration, sleep};
    const TEST_OPERATIONS_TOKEN: &str = "soradns-operations-token-00000001";
    fn test_operations_authorization() -> Arc<OperationsAuthorization> {
        Arc::new(OperationsAuthorization {
            token_hash: *blake3::hash(TEST_OPERATIONS_TOKEN.as_bytes()).as_bytes(),
        })
    }
    #[test]
    fn operations_auth_comparison_checks_every_digest_byte_and_redacts_debug() {
        let mut authorization = OperationsAuthorization {
            token_hash: [0xA5; 32],
        };
        assert!(constant_time_eq_32(&[0xA5; 32], &[0xA5; 32]));
        for index in 0..32 {
            let mut changed = [0xA5; 32];
            changed[index] ^= 1;
            assert!(!constant_time_eq_32(&authorization.token_hash, &changed));
        }
        let rendered = format!("{authorization:?}");
        assert!(rendered.contains("<redacted>"));
        assert!(!rendered.contains("165, 165"));
        authorization.clear();
        assert_eq!(authorization.token_hash, [0; 32]);
    }
    #[cfg(unix)]
    #[test]
    fn operations_auth_loader_requires_private_canonical_token_file() {
        use std::os::unix::fs::PermissionsExt as _;
        let directory = tempfile::tempdir_in(std::env::current_dir().expect("current directory"))
            .expect("operations auth directory");
        let path = directory.path().join("operations.token");
        std::fs::write(&path, format!("{TEST_OPERATIONS_TOKEN}\n"))
            .expect("write operations bearer token");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))
            .expect("protect operations bearer token");
        let authorization = OperationsAuthorization::load(&path).expect("load private token");
        assert!(authorization.matches(TEST_OPERATIONS_TOKEN));
        assert!(!authorization.matches("soradns-operations-token-00000002"));

        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644))
            .expect("make token permissions unsafe");
        assert!(OperationsAuthorization::load(&path).is_err());
    }
    #[test]
    fn transport_source_omits_raw_doq_and_client_address_logging() {
        let source = include_str!("lib.rs");
        let raw_peer_field = ["%", "peer"].concat();
        let raw_doq_server = ["start_", "doq_server"].concat();
        assert!(!source.contains(&raw_peer_field));
        assert!(!source.contains(&raw_doq_server));
    }
    #[test]
    fn doh_get_query_enforces_exact_encoded_ceiling_and_single_parameter() {
        let exact = "A".repeat(MAX_DOH_GET_ENCODED_BYTES_V1);
        let exact_query = format!("dns={exact}");
        assert_eq!(
            parse_doh_get_query(Some(&exact_query)).expect("exact encoded ceiling"),
            exact
        );
        let oversized = format!("dns={exact}A");
        assert_eq!(
            parse_doh_get_query(Some(&oversized)),
            Err("dns parameter is too large")
        );
        assert!(parse_doh_get_query(Some("dns=AA&dns=BB")).is_err());
        assert!(parse_doh_get_query(Some("other=AA")).is_err());
    }
    #[test]
    fn dot_session_permits_fail_closed_at_capacity() {
        let permits = Arc::new(Semaphore::new(1));
        let held = try_dot_session_permit(&permits).expect("first DoT permit");
        assert!(try_dot_session_permit(&permits).is_none());
        drop(held);
        assert!(try_dot_session_permit(&permits).is_some());
    }
    #[test]
    fn dot_response_length_prefix_rejects_u16_overflow() {
        assert_eq!(
            dot_response_length_prefix(u16::MAX as usize).expect("maximum DoT frame"),
            u16::MAX.to_be_bytes()
        );
        assert!(dot_response_length_prefix(u16::MAX as usize + 1).is_err());
    }
    #[cfg(unix)]
    #[test]
    fn dot_private_key_loader_rejects_unsafe_path() {
        use std::os::unix::fs::{PermissionsExt as _, symlink};
        let directory = tempfile::tempdir_in(std::env::current_dir().expect("current directory"))
            .expect("private-key directory");
        let target = directory.path().join("dot.key");
        let link = directory.path().join("dot.link");
        std::fs::write(&target, b"private key fixture").expect("write private-key fixture");
        std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o644))
            .expect("set unsafe private-key permissions");
        let error = load_key(&target).expect_err("world-readable private key must fail closed");
        assert!(error.to_string().contains("group or other"));

        std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o600))
            .expect("protect private-key fixture");
        symlink(&target, &link).expect("create private-key symlink");
        let error = load_key(&link).expect_err("private-key symlink must fail closed");
        assert!(error.to_string().contains("direct regular file"));
    }
    #[tokio::test(flavor = "multi_thread")]
    async fn doh_get_and_post_resolve_static_record() -> Result<()> {
        let addr = match std::net::TcpListener::bind("127.0.0.1:0") {
            Ok(listener) => {
                let addr = listener.local_addr()?;
                drop(listener);
                addr
            }
            Err(err) if err.kind() == ErrorKind::PermissionDenied => return Ok(()),
            Err(err) => return Err(err.into()),
        };
        let state = Arc::new(RwLock::new(ResolverState::new(
            "resolver".into(),
            "global".into(),
        )));
        {
            let mut guard = state.write().await;
            let name = Name::from_ascii("example.test.").unwrap();
            let record = Record::from_rdata(name, 60, RData::A(A::new(192, 0, 2, 1)));
            guard.update_static_zones(&[StaticZone {
                domain: "example.test".into(),
                records: vec![record],
                freeze: None,
                retained_bytes: 1024,
            }])?;
        }
        let server_state = state.clone();
        let metrics = MetricsRegistry::new();
        let doh_task = tokio::spawn(start_doh_server(
            addr,
            server_state,
            metrics,
            Duration::from_secs(30),
        ));
        sleep(Duration::from_millis(50)).await;
        let mut query = Message::new(0xCAFE, MessageType::Query, OpCode::Query);
        let name = Name::from_ascii("example.test.").unwrap();
        query.add_query(Query::query(name.clone(), RecordType::A));
        query.metadata.recursion_desired = true;
        let body = dns::encode_message(&query)?;
        let client = HttpClient::builder()
            .timeout(Duration::from_secs(2))
            .build()
            .expect("reqwest client");
        let base = format!("http://{}:{}/dns-query", addr.ip(), addr.port());
        // GET flow using base64url encoded payload.
        let encoded = URL_SAFE_NO_PAD.encode(&body);
        let get_url = format!("{base}?dns={encoded}");
        let get_response = client
            .get(&get_url)
            .header(ACCEPT, DNS_CONTENT_TYPE)
            .send()
            .await?;
        assert_eq!(get_response.status(), HttpStatus::OK);
        assert_eq!(
            get_response
                .headers()
                .get(CONTENT_TYPE)
                .and_then(|value| value.to_str().ok()),
            Some(DNS_CONTENT_TYPE)
        );
        let get_bytes = get_response.bytes().await?;
        assert_example_a_response(&get_bytes)?;
        // POST flow with binary DNS payload.
        let post_response = client
            .post(&base)
            .header(CONTENT_TYPE, DNS_CONTENT_TYPE)
            .body(body.clone())
            .send()
            .await?;
        assert_eq!(post_response.status(), HttpStatus::OK);
        assert_eq!(
            post_response
                .headers()
                .get(CONTENT_TYPE)
                .and_then(|value| value.to_str().ok()),
            Some(DNS_CONTENT_TYPE)
        );
        let post_bytes = post_response.bytes().await?;
        assert_example_a_response(&post_bytes)?;
        let wrong_content_type = client
            .post(&base)
            .header(CONTENT_TYPE, "application/octet-stream")
            .body(body)
            .send()
            .await?;
        assert_eq!(
            wrong_content_type.status(),
            HttpStatus::UNSUPPORTED_MEDIA_TYPE
        );
        let metrics_response = client
            .get(format!("http://{}:{}/metrics", addr.ip(), addr.port()))
            .send()
            .await?;
        assert_eq!(metrics_response.status(), HttpStatus::NOT_FOUND);
        doh_task.abort();
        Ok(())
    }
    #[tokio::test(flavor = "multi_thread")]
    async fn operational_metrics_endpoint_requires_auth_and_reports_counts() -> Result<()> {
        let addr = match std::net::TcpListener::bind("127.0.0.1:0") {
            Ok(listener) => {
                let addr = listener.local_addr()?;
                drop(listener);
                addr
            }
            Err(err) if err.kind() == ErrorKind::PermissionDenied => return Ok(()),
            Err(err) => return Err(err.into()),
        };
        let state = Arc::new(RwLock::new(ResolverState::new(
            "resolver".into(),
            "global".into(),
        )));
        let metrics = MetricsRegistry::new();
        metrics.update_last_sync(1234);
        let operational_task = tokio::spawn(start_event_server(
            addr,
            EventEmitter::new("resolver".to_owned(), None)?,
            AppContext::new(state, metrics.clone(), Duration::from_secs(30)),
            test_operations_authorization(),
        ));
        sleep(Duration::from_millis(50)).await;
        let client = HttpClient::builder()
            .timeout(Duration::from_secs(2))
            .build()
            .expect("reqwest client");
        let metrics_url = format!("http://{}:{}/metrics", addr.ip(), addr.port());
        let unauthorized = client.get(&metrics_url).send().await?;
        assert_eq!(unauthorized.status(), HttpStatus::UNAUTHORIZED);
        assert_eq!(
            unauthorized
                .headers()
                .get(reqwest::header::WWW_AUTHENTICATE)
                .and_then(|value| value.to_str().ok()),
            Some("Bearer")
        );
        let response = client
            .get(metrics_url)
            .bearer_auth(TEST_OPERATIONS_TOKEN)
            .send()
            .await?;
        assert_eq!(response.status(), HttpStatus::OK);
        assert_eq!(
            response
                .headers()
                .get(reqwest::header::CACHE_CONTROL)
                .and_then(|value| value.to_str().ok()),
            Some("no-store")
        );
        let body = response.text().await?;
        assert!(
            body.contains("soradns_resolver_bundle_count"),
            "metrics output missing bundle_count: {body}"
        );
        assert!(
            body.contains("resolver_id=\"resolver\""),
            "metrics output missing labels: {body}"
        );
        assert!(
            body.contains("soradns_resolver_sync_interval_seconds"),
            "metrics output missing sync interval gauge: {body}"
        );
        assert!(
            body.contains("soradns_resolver_last_sync_unix_seconds"),
            "metrics output missing last sync gauge: {body}"
        );
        assert!(
            body.contains("soradns_resolver_dns_queries_total"),
            "metrics output missing dns queries counter: {body}"
        );
        assert!(
            body.contains("soradns_resolver_validation_failures_total"),
            "metrics output missing validation failure counter: {body}"
        );
        operational_task.abort();
        Ok(())
    }
    #[tokio::test]
    async fn health_endpoint_reports_counters() -> Result<()> {
        let state = Arc::new(RwLock::new(ResolverState::new(
            "resolver".into(),
            "global".into(),
        )));
        let metrics = MetricsRegistry::new();
        metrics.update_last_sync(42);
        metrics.inc_dns_query();
        metrics.inc_dns_failure();
        metrics.inc_validation_failure();
        let ctx = AppContext::new(state, metrics, Duration::from_secs(15));
        let response = health_handler(AxumState(ctx)).await;
        assert_eq!(response.status(), StatusCode::OK);
        let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("read body");
        let value: Value = json::from_slice(&body_bytes).expect("decode json");
        let map = value.as_object().expect("object");
        assert_eq!(
            map.get("dns_queries_total").and_then(Value::as_u64),
            Some(1)
        );
        assert_eq!(
            map.get("dns_failures_total").and_then(Value::as_u64),
            Some(1)
        );
        assert_eq!(
            map.get("validation_failures_total").and_then(Value::as_u64),
            Some(1)
        );
        assert_eq!(map.get("last_sync_unix").and_then(Value::as_i64), Some(42));
        Ok(())
    }
    #[tokio::test(flavor = "multi_thread")]
    async fn operational_health_endpoint_requires_auth_and_reports_status() -> Result<()> {
        let addr = match std::net::TcpListener::bind("127.0.0.1:0") {
            Ok(listener) => {
                let addr = listener.local_addr()?;
                drop(listener);
                addr
            }
            Err(err) if err.kind() == ErrorKind::PermissionDenied => return Ok(()),
            Err(err) => return Err(err.into()),
        };
        let state = Arc::new(RwLock::new(ResolverState::new(
            "resolver".into(),
            "global".into(),
        )));
        let metrics = MetricsRegistry::new();
        metrics.update_last_sync(5678);
        let operational_task = tokio::spawn(start_event_server(
            addr,
            EventEmitter::new("resolver".to_owned(), None)?,
            AppContext::new(state, metrics, Duration::from_secs(30)),
            test_operations_authorization(),
        ));
        sleep(Duration::from_millis(50)).await;
        let client = HttpClient::builder()
            .timeout(Duration::from_secs(2))
            .build()
            .expect("reqwest client");
        let health_url = format!("http://{}:{}/healthz", addr.ip(), addr.port());
        let unauthorized = client.get(&health_url).send().await?;
        assert_eq!(unauthorized.status(), HttpStatus::UNAUTHORIZED);
        let response = client
            .get(health_url)
            .bearer_auth(TEST_OPERATIONS_TOKEN)
            .send()
            .await?;
        assert_eq!(response.status(), HttpStatus::OK);
        let body = response.text().await?;
        let value: Value = json::from_str(&body)?;
        assert_eq!(value.get("status").and_then(|v| v.as_str()), Some("ok"));
        assert_eq!(
            value.get("resolver_id").and_then(|v| v.as_str()),
            Some("resolver")
        );
        assert_eq!(
            value.get("sync_interval_secs").and_then(|v| v.as_u64()),
            Some(30)
        );
        operational_task.abort();
        Ok(())
    }
    fn assert_example_a_response(bytes: &[u8]) -> Result<()> {
        let response = dns::decode_message(bytes)?;
        assert_eq!(response.metadata.id, 0xCAFE);
        assert_eq!(response.metadata.response_code, ResponseCode::NoError);
        assert_eq!(response.answers.len(), 1);
        if let RData::A(answer) = &response.answers[0].data {
            assert_eq!(answer.0, Ipv4Addr::new(192, 0, 2, 1));
        } else {
            eyre::bail!("expected A record in DoH response");
        }
        Ok(())
    }
}
