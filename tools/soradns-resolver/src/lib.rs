#![allow(unexpected_cfgs)]

//! SoraDNS resolver prototype library.
//!
//! The resolver ingests proof bundles and resolver adverts, tracks resolver
//! state, emits change events, and exposes DNS transports (DoH, DoT, DoQ) that
//! currently resolve against a static record set supplied via configuration.

pub mod bundle;
pub mod canonical;
pub mod config;
pub mod directory;
pub mod dns;
pub mod events;
pub mod rad;
pub mod state;
pub mod transparency;

use std::{
    collections::HashMap,
    convert::Infallible,
    net::SocketAddr,
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicI64, AtomicU64, Ordering},
    },
    time::Duration,
};

use axum::{
    Router,
    body::{Body, Bytes},
    extract::{Query, State as AxumState},
    http::{Response, StatusCode},
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
use time::OffsetDateTime;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, UdpSocket},
    signal,
    sync::RwLock,
    task::JoinHandle,
    time::{Instant, MissedTickBehavior, interval_at},
};
use tokio_rustls::TlsAcceptor;
use tokio_stream::wrappers::{BroadcastStream, errors::BroadcastStreamRecvError};
use tracing::{error, info, warn};

use crate::{
    bundle::ProofBundleV1,
    config::{DotTlsConfig, ResolverConfig},
    events::EventEmitter,
    rad::{ResolverAttestation, validate_rad},
    state::{ResolverState, ResolverStateMetrics},
};

const DNS_CONTENT_TYPE: &str = "application/dns-message";

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

impl ResolverDaemon {
    /// Create a new resolver daemon. Validates configuration and initialises state.
    pub fn new(config: ResolverConfig) -> Result<Self> {
        config.validate()?;

        let http_client = reqwest::Client::builder()
            .user_agent("soradns-resolver/0.1.0")
            .build()?;

        let tls = match config.dot_tls() {
            Some(tls) => Some(load_tls_configs(tls)?),
            None => None,
        };

        let event_addr = config.event_listen();

        let mut state = ResolverState::new(config.resolver_id.clone(), config.region.clone());
        state.update_static_zones(config.static_zones());

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
        })
    }

    /// Returns a clone of the shared state handle for background tasks.
    #[must_use]
    pub fn shared_state(&self) -> SharedState {
        Arc::clone(&self.state)
    }

    /// Performs a single synchronization pass and returns the number of tracked zones.
    pub async fn sync_once(&self) -> Result<usize> {
        let mut state = self.state.write().await;
        let bundle_count = self.load_proof_bundles(&mut state).await?;
        let rad_count = self.refresh_rad_entries(&mut state).await?;
        let now = OffsetDateTime::now_utc().unix_timestamp();
        let expirations = state.prune_stale_entries(now);
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

    async fn load_proof_bundles(&self, state: &mut ResolverState) -> Result<usize> {
        let mut loaded: HashMap<String, ProofBundleV1> = HashMap::new();
        for source in self.config.bundle_sources() {
            match source.fetch(&self.http_client).await {
                Ok(bundles) => {
                    for bundle in bundles {
                        let namehash_hex = hex::encode(bundle.namehash);
                        if let Err(error) = bundle.validate() {
                            warn!(?error, namehash = %namehash_hex, "skipping invalid proof bundle");
                            continue;
                        }
                        loaded.insert(namehash_hex, bundle);
                    }
                }
                Err(error) => warn!(
                    ?error,
                    "failed to fetch proof bundles from configured source"
                ),
            }
        }
        let diff = state.update_bundles(loaded);
        self.events.emit_bundle_diff(&diff);
        Ok(state.bundle_count())
    }

    async fn refresh_rad_entries(&self, state: &mut ResolverState) -> Result<usize> {
        let mut adverts: HashMap<String, ResolverAttestation> = HashMap::new();
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
                        adverts.insert(resolver_key, advert);
                    }
                }
                Err(error) => warn!(
                    ?error,
                    "failed to fetch resolver adverts from configured source"
                ),
            }
        }
        let diff = state.update_resolver_adverts(adverts);
        self.events.emit_resolver_diff(&diff);
        Ok(state.resolver_advert_count())
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

        for &addr in self.config.doq_listen() {
            tasks.push(tokio::spawn(start_doq_server(addr, self.state.clone())));
        }

        if let Some(addr) = self.event_addr {
            tasks.push(tokio::spawn(start_event_server(addr, self.events.clone())));
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
                .route("/metrics", get(metrics_handler))
                .route("/healthz", get(health_handler))
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
            loop {
                match listener.accept().await {
                    Ok((stream, peer)) => {
                        let acceptor = acceptor.clone();
                        let state = state.clone();
                        tokio::spawn(async move {
                            if let Err(error) = handle_dot_stream(stream, acceptor, state).await {
                                warn!(%peer, ?error, "DoT session failed");
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

/// Temporary UDP DoQ listener used until QUIC support returns.
async fn start_doq_server(addr: SocketAddr, state: SharedState) {
    match UdpSocket::bind(addr).await {
        Ok(socket) => {
            info!(%addr, "DoQ listener bound (UDP stub)");
            let mut buf = vec![0_u8; 4096];
            loop {
                match socket.recv_from(&mut buf).await {
                    Ok((len, peer)) => {
                        let payload = &buf[..len];
                        match dns::decode_message(payload) {
                            Ok(request) => {
                                if let Some(response) = resolve_bytes(&state, &request).await
                                    && let Err(error) = socket.send_to(&response, peer).await
                                {
                                    warn!(%peer, ?error, "failed to send DoQ response");
                                }
                            }
                            Err(error) => {
                                warn!(?error, %peer, "failed to decode DoQ datagram");
                            }
                        }
                    }
                    Err(error) => {
                        warn!(%addr, ?error, "DoQ listener receive error");
                        break;
                    }
                }
            }
        }
        Err(error) => error!(%addr, ?error, "failed to bind DoQ listener"),
    }
}

async fn start_event_server(addr: SocketAddr, emitter: EventEmitter) {
    match TcpListener::bind(addr).await {
        Ok(listener) => {
            info!(%addr, "event stream listener bound");
            let router = Router::new()
                .route("/events", get(sse_handler))
                .with_state(emitter);
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
    let mut tls_stream = acceptor.accept(stream).await?;
    let mut len_bytes = [0_u8; 2];
    tls_stream.read_exact(&mut len_bytes).await?;
    let frame_len = u16::from_be_bytes(len_bytes) as usize;
    let mut payload = vec![0_u8; frame_len];
    tls_stream.read_exact(&mut payload).await?;

    if let Ok(request) = dns::decode_message(&payload) {
        if let Some(bytes) = resolve_bytes(&state, &request).await {
            tls_stream
                .write_all(&(bytes.len() as u16).to_be_bytes())
                .await?;
            tls_stream.write_all(&bytes).await?;
        }
    } else {
        warn!("failed to decode DoT request");
    }

    tls_stream.flush().await?;
    Ok(())
}

async fn doh_get(
    AxumState(ctx): AxumState<AppContext>,
    Query(params): Query<HashMap<String, String>>,
) -> Response<Body> {
    let Some(encoded) = params.get("dns") else {
        return build_error_response(StatusCode::BAD_REQUEST, "missing dns parameter");
    };
    match URL_SAFE_NO_PAD.decode(encoded.as_bytes()) {
        Ok(bytes) => ctx.resolve_dns(&bytes).await,
        Err(_) => build_error_response(StatusCode::BAD_REQUEST, "invalid base64 dns parameter"),
    }
}

async fn doh_post(AxumState(ctx): AxumState<AppContext>, body: Bytes) -> Response<Body> {
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
    let bytes = std::fs::read(path)?;
    let cert = CertificateDer::from(bytes);
    Ok(vec![cert])
}

fn load_key(path: &Path) -> Result<PrivateKeyDer<'static>> {
    let bytes = std::fs::read(path)?;
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
    use std::{io::ErrorKind, sync::Arc};

    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use hickory_proto::{
        op::{Message, Query, ResponseCode},
        rr::{Name, RData, Record, RecordType, rdata::A},
    };
    use norito::json::Value;
    use reqwest::{
        Client as HttpClient, StatusCode as HttpStatus,
        header::{ACCEPT, CONTENT_TYPE},
    };
    use tokio::{
        net::UdpSocket,
        time::{Duration, sleep},
    };

    use super::*;
    use crate::config::StaticZone;

    #[tokio::test(flavor = "multi_thread")]
    async fn doq_roundtrip_resolves_static_record() -> Result<()> {
        let addr = match std::net::UdpSocket::bind("127.0.0.1:0") {
            Ok(socket) => socket.local_addr()?,
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
            }]);
        }

        let server_state = state.clone();
        let doq_task = tokio::spawn(start_doq_server(addr, server_state));

        sleep(Duration::from_millis(50)).await;

        let client = match UdpSocket::bind("127.0.0.1:0").await {
            Ok(socket) => socket,
            Err(err) if err.kind() == ErrorKind::PermissionDenied => return Ok(()),
            Err(err) => return Err(err.into()),
        };
        client.connect(addr).await?;

        let mut query = Message::new();
        let name = Name::from_ascii("example.test.").unwrap();
        query.add_query(Query::query(name.clone(), RecordType::A));
        query.set_id(0xCAFE);
        query.set_recursion_desired(true);
        let body = dns::encode_message(&query)?;

        client.send(&body).await?;

        let mut buf = vec![0_u8; 4096];
        let len = client.recv(&mut buf).await?;

        let response = dns::decode_message(&buf[..len])?;
        assert_eq!(response.id(), 0xCAFE);
        assert_eq!(response.response_code(), ResponseCode::NoError);
        assert_eq!(response.answers().len(), 1);

        if let RData::A(answer) = response.answers()[0].data() {
            assert_eq!(*answer, A::new(192, 0, 2, 1));
        } else {
            panic!("expected A record in response");
        }

        doq_task.abort();
        Ok(())
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
            }]);
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

        let mut query = Message::new();
        let name = Name::from_ascii("example.test.").unwrap();
        query.add_query(Query::query(name.clone(), RecordType::A));
        query.set_id(0xCAFE);
        query.set_recursion_desired(true);
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

        doh_task.abort();
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn doh_metrics_endpoint_reports_counts() -> Result<()> {
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

        let server_state = state.clone();
        let doh_task = tokio::spawn(start_doh_server(
            addr,
            server_state,
            metrics.clone(),
            Duration::from_secs(30),
        ));
        sleep(Duration::from_millis(50)).await;

        let client = HttpClient::builder()
            .timeout(Duration::from_secs(2))
            .build()
            .expect("reqwest client");
        let metrics_url = format!("http://{}:{}/metrics", addr.ip(), addr.port());
        let response = client.get(metrics_url).send().await?;
        assert_eq!(response.status(), HttpStatus::OK);
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

        doh_task.abort();
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
    async fn doh_health_endpoint_reports_status() -> Result<()> {
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

        let server_state = state.clone();
        let doh_task = tokio::spawn(start_doh_server(
            addr,
            server_state,
            metrics,
            Duration::from_secs(30),
        ));
        sleep(Duration::from_millis(50)).await;

        let client = HttpClient::builder()
            .timeout(Duration::from_secs(2))
            .build()
            .expect("reqwest client");
        let health_url = format!("http://{}:{}/healthz", addr.ip(), addr.port());
        let response = client.get(health_url).send().await?;
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

        doh_task.abort();
        Ok(())
    }

    fn assert_example_a_response(bytes: &[u8]) -> Result<()> {
        let response = dns::decode_message(bytes)?;
        assert_eq!(response.id(), 0xCAFE);
        assert_eq!(response.response_code(), ResponseCode::NoError);
        assert_eq!(response.answers().len(), 1);
        if let RData::A(answer) = response.answers()[0].data() {
            assert_eq!(*answer, A::new(192, 0, 2, 1));
        } else {
            eyre::bail!("expected A record in DoH response");
        }
        Ok(())
    }
}
