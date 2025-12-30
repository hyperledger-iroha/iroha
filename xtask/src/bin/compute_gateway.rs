//! Minimal compute gateway harness to exercise SSC compute tasks.
//!
//! This standalone binary validates `ComputeCall` envelopes against a manifest,
//! enforces replay and rate limits, and executes deterministic built-in
//! entrypoints (e.g., `echo`) to produce `ComputeReceipt` outputs. It is
//! intentionally lightweight so CI can replay requests and lock determinism
//! without running the full node.

#![allow(clippy::too_many_lines)]

use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    env, fs,
    net::SocketAddr,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};

#[path = "../compute_harness.rs"]
mod compute_harness;

use axum::{
    Router,
    body::Bytes,
    extract::State,
    http::{HeaderValue, Request, StatusCode, header},
    response::{IntoResponse, Response},
    routing::post,
};
use base64::Engine as _;
use iroha_config::parameters::{actual::ComputeEconomics, defaults::compute as compute_defaults};
use iroha_crypto::Hash;
use iroha_data_model::{
    compute::{
        ComputeAuthz, ComputeCall, ComputeCallSummary, ComputeCodec, ComputeGovernanceError,
        ComputeManifest, ComputeManifestError, ComputeRequest, ComputeRoute, ComputeRouteId,
        ComputeValidationError, enforce_sponsor_policy,
    },
    name::Name,
};
use norito::{
    derive::{JsonDeserialize, JsonSerialize},
    json,
};
use tokio::{
    net::TcpListener,
    sync::{Mutex, OwnedSemaphorePermit, Semaphore},
    task::JoinSet,
};
use tower::ServiceExt;

#[cfg(not(test))]
fn touch_compute_helpers() {
    type BuildCallFn = fn(
        &ComputeManifest,
        &ComputeRoute,
        &[u8],
        iroha_data_model::compute::ComputeCodec,
        iroha_data_model::compute::ComputeAuthz,
    ) -> Result<
        iroha_data_model::compute::ComputeCall,
        iroha_data_model::compute::ComputeValidationError,
    >;
    let _ = compute_harness::payload_with_len as fn(usize) -> Vec<u8>;
    let _ = compute_harness::build_call_for_route as BuildCallFn;
}

#[derive(Clone, JsonDeserialize, JsonSerialize)]
struct GatewayRequest {
    namespace: Name,
    codec: ComputeCodec,
    ttl_slots: std::num::NonZeroU64,
    gas_limit: std::num::NonZeroU64,
    max_response_bytes: std::num::NonZeroU64,
    determinism: iroha_data_model::compute::ComputeDeterminism,
    execution_class: iroha_data_model::compute::ComputeExecutionClass,
    #[norito(default)]
    declared_input_bytes: Option<std::num::NonZeroU64>,
    #[norito(default)]
    declared_input_chunks: Option<std::num::NonZeroU32>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    sponsor_budget_cu: Option<std::num::NonZeroU64>,
    price_family: Name,
    resource_profile: Name,
    auth: ComputeAuthz,
    #[norito(default)]
    headers: BTreeMap<String, String>,
    /// Base64-encoded payload bytes.
    payload_b64: String,
}

#[derive(JsonSerialize, JsonDeserialize)]
struct GatewayResponse {
    receipt: iroha_data_model::compute::ComputeReceipt,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    response_b64: Option<String>,
}

#[derive(Clone, JsonSerialize)]
struct BenchPercentiles {
    p50_ms: f64,
    p90_ms: f64,
    p99_ms: f64,
    max_ms: f64,
}

#[derive(Clone, JsonSerialize)]
struct BenchSloTargets {
    max_p50_ms: f64,
    max_p99_ms: f64,
    min_rps: f64,
    max_response_bytes: u64,
}

#[derive(Clone, JsonSerialize)]
struct BenchSloReport {
    passed: bool,
    #[norito(default)]
    #[norito(skip_serializing_if = "Vec::is_empty")]
    violations: Vec<String>,
}

#[derive(Clone, JsonSerialize)]
struct BenchSummary {
    namespace: Name,
    route: ComputeRouteId,
    iterations: usize,
    concurrency: usize,
    throughput_rps: f64,
    duration_ms: f64,
    successes: usize,
    failures: usize,
    percentiles_ms: BenchPercentiles,
    response_bytes_max: u64,
    response_bytes_avg: f64,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    slo: Option<BenchSloReport>,
}

#[derive(Clone)]
struct RouteCtx {
    manifest: Arc<ComputeManifest>,
    route_idx: usize,
}

impl RouteCtx {
    fn route(&self) -> &ComputeRoute {
        self.manifest
            .routes
            .get(self.route_idx)
            .expect("route index should be valid")
    }
}

#[derive(Clone)]
struct RouteQueue {
    semaphore: Arc<Semaphore>,
    queued: Arc<AtomicUsize>,
    capacity: usize,
}

impl RouteQueue {
    fn new(max_inflight: usize, capacity: usize) -> Self {
        Self {
            semaphore: Arc::new(Semaphore::new(max_inflight.max(1))),
            queued: Arc::new(AtomicUsize::new(0)),
            capacity: capacity.max(1),
        }
    }

    async fn acquire(self: &Arc<Self>) -> Result<QueueGuard, GatewayError> {
        let prev = self.queued.fetch_add(1, Ordering::SeqCst);
        if prev >= self.capacity {
            self.queued.fetch_sub(1, Ordering::SeqCst);
            return Err(GatewayError::QueueFull);
        }
        let permit = self
            .semaphore
            .clone()
            .acquire_owned()
            .await
            .map_err(|_| GatewayError::Internal("queue semaphore closed".to_string()))?;
        Ok(QueueGuard {
            queue: Arc::clone(self),
            _permit: permit,
        })
    }
}

struct QueueGuard {
    queue: Arc<RouteQueue>,
    _permit: OwnedSemaphorePermit,
}

impl Drop for QueueGuard {
    fn drop(&mut self) {
        self.queue.queued.fetch_sub(1, Ordering::SeqCst);
    }
}

#[derive(Clone)]
struct TokenBucketLimiter {
    rate_per_sec: Option<f64>,
    burst: f64,
    buckets: Arc<Mutex<HashMap<String, TokenBucket>>>,
}

#[derive(Clone, Copy)]
struct TokenBucket {
    tokens: f64,
    last: Instant,
}

#[derive(Clone, Copy)]
struct GatewayLimits {
    rate_per_sec: Option<u32>,
    burst: Option<u32>,
    replay_ttl: Duration,
    queue_capacity: usize,
    max_inflight: usize,
}

impl TokenBucketLimiter {
    fn new(rate_per_sec: Option<u32>, burst: Option<u32>) -> Self {
        let rate = rate_per_sec.and_then(|r| if r == 0 { None } else { Some(r as f64) });
        let burst = burst.unwrap_or(rate_per_sec.unwrap_or(0)).max(1) as f64;
        Self {
            rate_per_sec: rate,
            burst,
            buckets: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    async fn allow(&self, key: &str) -> bool {
        let Some(rate) = self.rate_per_sec else {
            return true;
        };
        let mut guard = self.buckets.lock().await;
        let now = Instant::now();
        let entry = guard.entry(key.to_string()).or_insert(TokenBucket {
            tokens: self.burst,
            last: now,
        });
        let elapsed = now.saturating_duration_since(entry.last).as_secs_f64();
        if elapsed > 0.0 {
            entry.tokens = (entry.tokens + elapsed * rate).min(self.burst);
            entry.last = now;
        }
        if entry.tokens >= 1.0 {
            entry.tokens -= 1.0;
            true
        } else {
            false
        }
    }
}

#[derive(Clone)]
struct ComputeGateway {
    routes: BTreeMap<ComputeRouteId, RouteCtx>,
    namespaces: BTreeSet<Name>,
    replay_ttl: Duration,
    replay: Arc<Mutex<HashMap<Hash, Instant>>>,
    limiter: TokenBucketLimiter,
    queues: Arc<Mutex<BTreeMap<ComputeRouteId, Arc<RouteQueue>>>>,
    price_families: BTreeMap<Name, iroha_data_model::compute::ComputePriceWeights>,
    default_price_family: Name,
    economics: ComputeEconomics,
    queue_capacity: usize,
    max_inflight: usize,
}

impl ComputeGateway {
    fn new(
        manifest: ComputeManifest,
        limits: GatewayLimits,
        price_families: BTreeMap<Name, iroha_data_model::compute::ComputePriceWeights>,
        default_price_family: Name,
        economics: ComputeEconomics,
    ) -> Result<Self, GatewayError> {
        manifest.validate().map_err(GatewayError::Manifest)?;
        let manifest = Arc::new(manifest);
        let mut routes = BTreeMap::new();
        for (idx, route) in manifest.routes.iter().enumerate() {
            routes.insert(
                route.id.clone(),
                RouteCtx {
                    manifest: Arc::clone(&manifest),
                    route_idx: idx,
                },
            );
        }

        Ok(Self {
            routes,
            namespaces: [manifest.namespace.clone()].into_iter().collect(),
            replay_ttl: limits.replay_ttl,
            replay: Arc::new(Mutex::new(HashMap::new())),
            limiter: TokenBucketLimiter::new(limits.rate_per_sec, limits.burst),
            queues: Arc::new(Mutex::new(BTreeMap::new())),
            price_families,
            default_price_family,
            economics,
            queue_capacity: limits.queue_capacity,
            max_inflight: limits.max_inflight,
        })
    }

    async fn router(self: Arc<Self>) -> Router {
        Router::new()
            .route("/v1/compute/{service}/{method}", post(Self::handle_compute))
            .with_state(self)
    }

    async fn handle_compute(
        State(gateway): State<Arc<Self>>,
        axum::extract::Path((service, method)): axum::extract::Path<(String, String)>,
        headers: axum::http::HeaderMap,
        body: Bytes,
    ) -> impl IntoResponse {
        let content_type_ok = headers
            .get(header::CONTENT_TYPE)
            .map(|h| h == HeaderValue::from_static("application/json"))
            .unwrap_or(true);
        if !content_type_ok {
            return StatusCode::UNSUPPORTED_MEDIA_TYPE.into_response();
        }
        let req: GatewayRequest = match json::from_slice(&body) {
            Ok(req) => req,
            Err(err) => {
                eprintln!("failed to decode compute gateway request: {err:?}");
                return StatusCode::BAD_REQUEST.into_response();
            }
        };
        let route = match route_id_from_parts(service, method) {
            Ok(id) => id,
            Err(err) => return err.into_response(),
        };
        match gateway.process(route, req).await {
            Ok(resp) => {
                let payload = match json::to_vec(&resp) {
                    Ok(bytes) => bytes,
                    Err(err) => {
                        eprintln!("failed to encode compute response: {err:?}");
                        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
                    }
                };
                (
                    StatusCode::OK,
                    [(header::CONTENT_TYPE, "application/json")],
                    payload,
                )
                    .into_response()
            }
            Err(err) => {
                eprintln!("compute gateway error: {err}");
                err.into_response()
            }
        }
    }

    async fn process(
        &self,
        route_id: ComputeRouteId,
        req: GatewayRequest,
    ) -> Result<GatewayResponse, GatewayError> {
        let ctx = self
            .routes
            .get(&route_id)
            .ok_or_else(|| GatewayError::NotFound(route_id.clone()))?;

        if !self.namespaces.contains(&req.namespace) {
            return Err(GatewayError::Validation(
                ComputeValidationError::NamespaceMismatch {
                    expected: ctx.manifest.namespace.clone(),
                    provided: req.namespace.clone(),
                },
            ));
        }

        let payload = decode_payload(&req.payload_b64)?;
        let request = ComputeRequest {
            headers: req.headers.clone(),
            payload_hash: Hash::new(&payload),
        };
        let call = ComputeCall {
            namespace: req.namespace.clone(),
            route: route_id.clone(),
            codec: req.codec,
            ttl_slots: req.ttl_slots,
            gas_limit: req.gas_limit,
            max_response_bytes: req.max_response_bytes,
            determinism: req.determinism,
            execution_class: req.execution_class,
            declared_input_bytes: req.declared_input_bytes,
            declared_input_chunks: req.declared_input_chunks,
            sponsor_budget_cu: req.sponsor_budget_cu,
            price_family: req.price_family.clone(),
            resource_profile: req.resource_profile.clone(),
            auth: req.auth,
            request,
        };

        ctx.manifest
            .validate_call(&call)
            .map_err(GatewayError::Validation)?;
        if payload.len() as u64 > ctx.route().max_request_bytes.get() {
            return Err(GatewayError::PayloadTooLarge(
                payload.len(),
                ctx.route().max_request_bytes.get(),
            ));
        }
        if let Some(budget) = call.sponsor_budget_cu {
            enforce_sponsor_policy(&self.economics.sponsor_policy, budget.get(), 0)
                .map_err(GatewayError::Governance)?;
        }

        // Basic rate limiting keyed by auth identity.
        let rate_key = match &call.auth {
            ComputeAuthz::Public => "public".to_string(),
            ComputeAuthz::Authenticated(auth) => format!("uaid:{}", auth.uaid),
        };
        if !self.limiter.allow(&rate_key).await {
            return Err(GatewayError::RateLimited);
        }

        // Replay guard keyed by request hash.
        self.prune_replay().await;
        let request_hash: Hash = call.request_hash().into();
        {
            let mut replay = self.replay.lock().await;
            if replay.contains_key(&request_hash) {
                return Err(GatewayError::Replay);
            }
            replay.insert(request_hash, Instant::now());
        }

        let queue = self.queue_for(&route_id).await;
        let _guard = queue.acquire().await?;

        let (outcome, response_bytes) =
            compute_harness::execute_entrypoint(&ctx.route().entrypoint, &payload, &call)
                .map_err(|err| GatewayError::Internal(err.to_string()))?;
        let mut metering = compute_harness::meter(ctx.route(), &call, &payload, &response_bytes);

        compute_harness::charge_units(
            &self.price_families,
            &self.default_price_family,
            &self.economics.price_amplifiers,
            self.economics.max_cu_per_call,
            self.economics.max_amplification_ratio,
            &call,
            &mut metering,
        );
        if let Some(budget) = call.sponsor_budget_cu {
            self.economics
                .validate_sponsor_allocation(budget.get())
                .map_err(GatewayError::Governance)?;
            if metering.charged_units > budget.get() {
                return Err(GatewayError::SponsorBudgetExceeded {
                    charged: metering.charged_units,
                    budget: budget.get(),
                });
            }
        }

        let receipt = iroha_data_model::compute::ComputeReceipt {
            call: ComputeCallSummary::from(&call),
            metering,
            outcome,
        };

        Ok(GatewayResponse {
            receipt,
            response_b64: response_bytes
                .map(|bytes| base64::engine::general_purpose::STANDARD.encode(bytes)),
        })
    }

    async fn queue_for(&self, route_id: &ComputeRouteId) -> Arc<RouteQueue> {
        let mut guard = self.queues.lock().await;
        guard
            .entry(route_id.clone())
            .or_insert_with(|| Arc::new(RouteQueue::new(self.max_inflight, self.queue_capacity)))
            .clone()
    }

    async fn prune_replay(&self) {
        let ttl = self.replay_ttl;
        let mut replay = self.replay.lock().await;
        let now = Instant::now();
        replay.retain(|_, seen| now.saturating_duration_since(*seen) < ttl);
    }
}

#[derive(Debug)]
enum GatewayError {
    Manifest(ComputeManifestError),
    Validation(ComputeValidationError),
    PayloadTooLarge(usize, u64),
    Replay,
    RateLimited,
    Governance(ComputeGovernanceError),
    SponsorBudgetExceeded { charged: u64, budget: u64 },
    QueueFull,
    NotFound(ComputeRouteId),
    Internal(String),
}

impl IntoResponse for GatewayError {
    fn into_response(self) -> Response {
        match self {
            GatewayError::Manifest(err) => {
                (StatusCode::BAD_REQUEST, err.to_string()).into_response()
            }
            GatewayError::Validation(err) => {
                (StatusCode::BAD_REQUEST, err.to_string()).into_response()
            }
            GatewayError::PayloadTooLarge(size, max) => (
                StatusCode::PAYLOAD_TOO_LARGE,
                format!("payload {size} exceeds cap {max}"),
            )
                .into_response(),
            GatewayError::Replay => (
                StatusCode::CONFLICT,
                "duplicate compute request (replay detected)",
            )
                .into_response(),
            GatewayError::RateLimited => {
                (StatusCode::TOO_MANY_REQUESTS, "compute rate limit exceeded").into_response()
            }
            GatewayError::Governance(err) => (
                StatusCode::BAD_REQUEST,
                format!("governance rejection: {err}"),
            )
                .into_response(),
            GatewayError::SponsorBudgetExceeded { charged, budget } => (
                StatusCode::PAYMENT_REQUIRED,
                format!("sponsor budget exceeded: charged {charged} cu, budget {budget} cu"),
            )
                .into_response(),
            GatewayError::QueueFull => {
                (StatusCode::SERVICE_UNAVAILABLE, "compute queue saturated").into_response()
            }
            GatewayError::NotFound(route) => (
                StatusCode::NOT_FOUND,
                format!("route {route:?} not found in manifest"),
            )
                .into_response(),
            GatewayError::Internal(msg) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("internal error: {msg}"),
            )
                .into_response(),
        }
    }
}

impl std::fmt::Display for GatewayError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for GatewayError {}

fn decode_payload(b64: &str) -> Result<Vec<u8>, GatewayError> {
    base64::engine::general_purpose::STANDARD
        .decode(b64.as_bytes())
        .map_err(|err| GatewayError::Internal(format!("payload base64 decode failed: {err}")))
}

fn build_economics(
    price_families: &BTreeMap<Name, iroha_data_model::compute::ComputePriceWeights>,
) -> ComputeEconomics {
    ComputeEconomics {
        max_cu_per_call: compute_defaults::max_cu_per_call(),
        max_amplification_ratio: compute_defaults::max_amplification_ratio(),
        fee_split: compute_defaults::fee_split(),
        sponsor_policy: compute_defaults::sponsor_policy(),
        price_bounds: compute_defaults::price_bounds(),
        price_risk_classes: compute_defaults::price_risk_classes(),
        price_amplifiers: compute_defaults::price_amplifiers(),
        price_family_baseline: price_families.clone(),
    }
}

const DEFAULT_MANIFEST_PATH: &str = "fixtures/compute/manifest_compute_payments.json";
const DEFAULT_PAYLOAD_PATH: &str = "fixtures/compute/payload_compute_payments.json";
const DEFAULT_BENCH_OUT_DIR: &str = "artifacts/compute_gateway";

fn load_manifest(path: &str) -> ComputeManifest {
    fs::read_to_string(path)
        .ok()
        .and_then(|contents| json::from_str(&contents).ok())
        .unwrap_or_else(compute_harness::default_manifest)
}

fn load_payload(path: Option<&str>) -> Vec<u8> {
    if let Some(candidate) = path
        && let Ok(contents) = fs::read_to_string(candidate)
    {
        let trimmed = contents.trim();
        if !trimmed.is_empty() {
            return trimmed.as_bytes().to_vec();
        }
    }
    if let Ok(contents) = fs::read_to_string(DEFAULT_PAYLOAD_PATH) {
        let trimmed = contents.trim();
        if !trimmed.is_empty() {
            return trimmed.as_bytes().to_vec();
        }
    }
    br#"{"pair":"XOR/USD"}"#.to_vec()
}

fn default_bench_slo(route: &ComputeRoute) -> BenchSloTargets {
    BenchSloTargets {
        max_p50_ms: compute_defaults::target_p50_latency_ms().get() as f64,
        max_p99_ms: compute_defaults::target_p99_latency_ms().get() as f64,
        min_rps: (compute_defaults::max_requests_per_second().get() as f64 * 0.25).max(1.0),
        max_response_bytes: route.max_response_bytes.get(),
    }
}

fn compute_percentiles(samples: &[f64]) -> BenchPercentiles {
    if samples.is_empty() {
        return BenchPercentiles {
            p50_ms: 0.0,
            p90_ms: 0.0,
            p99_ms: 0.0,
            max_ms: 0.0,
        };
    }
    let mut sorted = samples.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let percentile = |pct: f64| -> f64 {
        if sorted.len() == 1 {
            sorted[0]
        } else {
            let idx = ((sorted.len() as f64 - 1.0) * pct)
                .round()
                .clamp(0.0, (sorted.len() - 1) as f64) as usize;
            sorted[idx]
        }
    };
    BenchPercentiles {
        p50_ms: percentile(0.50),
        p90_ms: percentile(0.90),
        p99_ms: percentile(0.99),
        max_ms: *sorted.last().unwrap_or(&0.0),
    }
}

fn evaluate_slo(
    targets: &BenchSloTargets,
    percentiles: &BenchPercentiles,
    throughput_rps: f64,
    response_bytes_max: u64,
    successes: usize,
) -> BenchSloReport {
    let mut violations = Vec::new();
    if successes == 0 {
        violations.push("no successful responses recorded".to_string());
    }
    if percentiles.p50_ms > targets.max_p50_ms {
        violations.push(format!(
            "p50 latency {:.2}ms exceeds {:.2}ms budget",
            percentiles.p50_ms, targets.max_p50_ms
        ));
    }
    if percentiles.p99_ms > targets.max_p99_ms {
        violations.push(format!(
            "p99 latency {:.2}ms exceeds {:.2}ms budget",
            percentiles.p99_ms, targets.max_p99_ms
        ));
    }
    if throughput_rps < targets.min_rps {
        violations.push(format!(
            "throughput {:.2}rps below {:.2}rps target",
            throughput_rps, targets.min_rps
        ));
    }
    if response_bytes_max > targets.max_response_bytes {
        violations.push(format!(
            "response bytes {} exceeds cap {}",
            response_bytes_max, targets.max_response_bytes
        ));
    }

    BenchSloReport {
        passed: violations.is_empty(),
        violations,
    }
}

struct BenchSample {
    latency_ms: f64,
    response_bytes: usize,
}

async fn bench_gateway(
    gateway: Arc<ComputeGateway>,
    namespace: Name,
    route: &ComputeRoute,
    payload: Vec<u8>,
    iterations: usize,
    concurrency: usize,
    slo: Option<BenchSloTargets>,
) -> Result<BenchSummary, String> {
    let router = gateway.router().await;
    let iterations = iterations.max(1);
    let concurrency = concurrency.max(1);
    let uri = format!("/v1/compute/{}/{}", route.id.service, route.id.method);

    let payload_b64 = base64::engine::general_purpose::STANDARD.encode(&payload);
    let mut join_set = JoinSet::new();
    let semaphore = Arc::new(Semaphore::new(concurrency));

    let request_template = GatewayRequest {
        namespace,
        codec: route
            .codecs
            .first()
            .copied()
            .unwrap_or(ComputeCodec::NoritoJson),
        ttl_slots: route.ttl_slots,
        gas_limit: route.gas_budget,
        max_response_bytes: route.max_response_bytes,
        determinism: route.determinism,
        execution_class: route.execution_class,
        declared_input_bytes: std::num::NonZeroU64::new(payload.len() as u64),
        declared_input_chunks: std::num::NonZeroU32::new(1),
        sponsor_budget_cu: None,
        price_family: route.price_family.clone(),
        resource_profile: route.resource_profile.clone(),
        auth: ComputeAuthz::Public,
        headers: BTreeMap::new(),
        payload_b64: String::new(),
    };

    let started = Instant::now();
    for seq in 0..iterations {
        let permit = semaphore
            .clone()
            .acquire_owned()
            .await
            .map_err(|err| format!("failed to acquire bench slot: {err}"))?;
        let mut req = request_template.clone();
        req.headers
            .insert("x-bench-seq".to_string(), format!("{seq}"));
        req.payload_b64 = payload_b64.clone();
        let request_bytes = json::to_vec(&req).map_err(|err| err.to_string())?;
        let router = router.clone();
        let uri = uri.clone();
        join_set.spawn(async move {
            let _permit = permit;
            let started_request = Instant::now();
            let request = Request::builder()
                .method("POST")
                .uri(&uri)
                .header("content-type", "application/json")
                .body(axum::body::Body::from(request_bytes))
                .map_err(|err| err.to_string())?;
            let response = router
                .oneshot(request)
                .await
                .map_err(|err| err.to_string())?;
            let status = response.status();
            let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
                .await
                .map_err(|err| err.to_string())?;
            let latency_ms = started_request.elapsed().as_secs_f64() * 1_000.0;
            if status != StatusCode::OK {
                return Err(format!(
                    "bench request failed with status {status}: {}",
                    String::from_utf8_lossy(&body_bytes)
                ));
            }
            let parsed: GatewayResponse =
                json::from_slice(&body_bytes).map_err(|err| err.to_string())?;
            let response_bytes = parsed
                .response_b64
                .as_ref()
                .and_then(|b64| {
                    base64::engine::general_purpose::STANDARD
                        .decode(b64.as_bytes())
                        .ok()
                })
                .map(|bytes| bytes.len())
                .unwrap_or(0);
            Ok(BenchSample {
                latency_ms,
                response_bytes,
            })
        });
    }

    let mut successes = 0usize;
    let mut failures = 0usize;
    let mut latencies = Vec::new();
    let mut response_total: usize = 0;
    let mut response_max: u64 = 0;

    while let Some(result) = join_set.join_next().await {
        match result {
            Ok(Ok(sample)) => {
                successes += 1;
                latencies.push(sample.latency_ms);
                response_total = response_total.saturating_add(sample.response_bytes);
                response_max = response_max.max(sample.response_bytes as u64);
            }
            Ok(Err(_)) | Err(_) => {
                failures = failures.saturating_add(1);
            }
        }
    }

    let elapsed_ms = started.elapsed().as_secs_f64() * 1_000.0;
    let throughput_rps = if elapsed_ms > 0.0 {
        successes as f64 / (elapsed_ms / 1_000.0)
    } else {
        successes as f64
    };
    let percentiles = compute_percentiles(&latencies);
    let slo_targets = slo.unwrap_or_else(|| default_bench_slo(route));
    let slo = evaluate_slo(
        &slo_targets,
        &percentiles,
        throughput_rps,
        response_max,
        successes,
    );

    Ok(BenchSummary {
        namespace: request_template.namespace,
        route: route.id.clone(),
        iterations,
        concurrency,
        throughput_rps,
        duration_ms: elapsed_ms,
        successes,
        failures: failures.saturating_add(iterations.saturating_sub(successes + failures)),
        percentiles_ms: percentiles,
        response_bytes_max: response_max,
        response_bytes_avg: if successes > 0 {
            response_total as f64 / successes as f64
        } else {
            0.0
        },
        slo: Some(slo),
    })
}

fn write_bench_outputs(out_dir: &PathBuf, summary: &BenchSummary) -> Result<(), String> {
    fs::create_dir_all(out_dir)
        .map_err(|err| format!("failed to create bench output dir {out_dir:?}: {err}"))?;
    let json_path = out_dir.join("bench_summary.json");
    let json_bytes = json::to_vec_pretty(summary).map_err(|err| err.to_string())?;
    fs::write(&json_path, json_bytes)
        .map_err(|err| format!("failed to write {json_path:?}: {err}"))?;

    let slo_status = summary
        .slo
        .as_ref()
        .map(|slo| {
            if slo.passed {
                "passed".to_string()
            } else {
                format!("violations: {}", slo.violations.join("; "))
            }
        })
        .unwrap_or_else(|| "not evaluated".to_string());
    let md_path = out_dir.join("bench_summary.md");
    let md = format!(
        "# Compute gateway bench\n\
- route: {service}/{method}\n\
- iterations: {iterations}\n\
- concurrency: {concurrency}\n\
- throughput_rps: {throughput:.2}\n\
- latency_ms: p50={p50:.2}, p90={p90:.2}, p99={p99:.2}, max={max_lat:.2}\n\
- response bytes: max={resp_max}, avg={resp_avg:.1}\n\
- SLO: {slo_status}\n",
        service = summary.route.service,
        method = summary.route.method,
        iterations = summary.iterations,
        concurrency = summary.concurrency,
        throughput = summary.throughput_rps,
        p50 = summary.percentiles_ms.p50_ms,
        p90 = summary.percentiles_ms.p90_ms,
        p99 = summary.percentiles_ms.p99_ms,
        max_lat = summary.percentiles_ms.max_ms,
        resp_max = summary.response_bytes_max,
        resp_avg = summary.response_bytes_avg,
        slo_status = slo_status,
    );
    fs::write(&md_path, md).map_err(|err| format!("failed to write {md_path:?}: {err}"))?;
    Ok(())
}

fn route_id_from_parts(service: String, method: String) -> Result<ComputeRouteId, GatewayError> {
    let service_name: Name = service
        .parse()
        .map_err(|_| GatewayError::Internal("invalid service name".to_string()))?;
    let method_name: Name = method
        .parse()
        .map_err(|_| GatewayError::Internal("invalid method name".to_string()))?;
    Ok(ComputeRouteId::new(service_name, method_name))
}

#[cfg(test)]
fn default_manifest() -> ComputeManifest {
    compute_harness::default_manifest()
}

enum Mode {
    Serve {
        manifest_path: String,
        addr: SocketAddr,
    },
    Bench {
        manifest_path: String,
        iterations: usize,
        concurrency: usize,
        output_dir: PathBuf,
        payload_path: Option<String>,
    },
}

fn parse_mode(mut args: impl Iterator<Item = String>) -> Result<Mode, String> {
    let first = args.next();
    match first.as_deref() {
        Some("bench") | Some("--bench") => {
            let manifest_path = args
                .next()
                .unwrap_or_else(|| DEFAULT_MANIFEST_PATH.to_string());
            let iterations = args
                .next()
                .and_then(|v| v.parse::<usize>().ok())
                .unwrap_or(128);
            let concurrency = args
                .next()
                .and_then(|v| v.parse::<usize>().ok())
                .unwrap_or(16);
            let output_dir = args
                .next()
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from(DEFAULT_BENCH_OUT_DIR));
            Ok(Mode::Bench {
                manifest_path,
                iterations,
                concurrency,
                output_dir,
                payload_path: None,
            })
        }
        Some(value) => {
            let (manifest_path, addr_value) = if value.contains(':') && !value.contains('/') {
                (DEFAULT_MANIFEST_PATH.to_string(), value.to_string())
            } else {
                (
                    value.to_string(),
                    args.next().unwrap_or_else(|| "127.0.0.1:8088".to_string()),
                )
            };
            let addr: SocketAddr = addr_value
                .parse()
                .map_err(|err| format!("invalid socket address `{addr_value}`: {err}"))?;
            Ok(Mode::Serve {
                manifest_path,
                addr,
            })
        }
        None => Ok(Mode::Serve {
            manifest_path: DEFAULT_MANIFEST_PATH.to_string(),
            addr: "127.0.0.1:8088"
                .parse()
                .expect("default socket address must parse"),
        }),
    }
}

async fn run_server(manifest_path: String, addr: SocketAddr) -> Result<(), String> {
    let manifest = load_manifest(&manifest_path);
    let slo = compute_harness::slo_targets();
    let price_families = compute_defaults::price_families();
    let economics = build_economics(&price_families);
    let gateway = Arc::new(
        ComputeGateway::new(
            manifest,
            GatewayLimits {
                rate_per_sec: Some(slo.max_requests_per_second.get()),
                burst: Some(slo.max_requests_per_second.get()),
                replay_ttl: Duration::from_secs(300),
                queue_capacity: slo.queue_depth_per_route.get(),
                max_inflight: slo.max_inflight_per_route.get(),
            },
            price_families,
            compute_defaults::default_price_family(),
            economics,
        )
        .map_err(|err| err.to_string())?,
    );
    let router = gateway.clone().router().await;

    println!("compute gateway listening on http://{addr}");
    let listener = TcpListener::bind(addr)
        .await
        .map_err(|err| format!("failed to bind {addr}: {err}"))?;
    axum::serve(listener, router)
        .await
        .map_err(|err| format!("server error: {err}"))
}

async fn run_bench_mode(
    manifest_path: String,
    payload_path: Option<String>,
    iterations: usize,
    concurrency: usize,
    output_dir: PathBuf,
) -> Result<(), String> {
    let manifest = load_manifest(&manifest_path);
    let Some(route) = manifest.routes.first() else {
        return Err("bench requires at least one route in the manifest".to_string());
    };
    let namespace = manifest.namespace.clone();
    let payload = load_payload(payload_path.as_deref());
    let manifest_for_gateway = load_manifest(&manifest_path);
    let price_families = compute_defaults::price_families();
    let economics = build_economics(&price_families);
    let gateway = Arc::new(
        ComputeGateway::new(
            manifest_for_gateway,
            GatewayLimits {
                rate_per_sec: None,
                burst: None,
                replay_ttl: Duration::from_secs(300),
                queue_capacity: compute_defaults::queue_depth_per_route().get(),
                max_inflight: compute_defaults::max_inflight_per_route().get(),
            },
            price_families,
            compute_defaults::default_price_family(),
            economics,
        )
        .map_err(|err| err.to_string())?,
    );
    let summary = bench_gateway(
        gateway,
        namespace,
        route,
        payload,
        iterations,
        concurrency,
        None,
    )
    .await?;
    write_bench_outputs(&output_dir, &summary)?;
    println!(
        "bench complete: {successes}/{iterations} ok, {:.2} rps (p99 {:.2} ms)",
        summary.throughput_rps,
        summary.percentiles_ms.p99_ms,
        successes = summary.successes,
        iterations = summary.iterations
    );
    Ok(())
}

#[tokio::main]
async fn main() {
    #[cfg(not(test))]
    touch_compute_helpers();
    let mode = parse_mode(env::args().skip(1)).unwrap_or_else(|err| {
        eprintln!("{err}");
        std::process::exit(1);
    });
    let result = match mode {
        Mode::Serve {
            manifest_path,
            addr,
        } => run_server(manifest_path, addr).await,
        Mode::Bench {
            manifest_path,
            iterations,
            concurrency,
            output_dir,
            payload_path,
        } => {
            run_bench_mode(
                manifest_path,
                payload_path,
                iterations,
                concurrency,
                output_dir,
            )
            .await
        }
    };
    if let Err(err) = result {
        eprintln!("{err}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use axum::{
        body,
        http::{Request, StatusCode},
    };
    use iroha_data_model::compute::ComputeOutcomeKind;

    use super::*;

    fn gateway_for_tests() -> Arc<ComputeGateway> {
        let manifest: ComputeManifest = json::from_str(include_str!(
            "../../../fixtures/compute/manifest_compute_payments.json"
        ))
        .unwrap_or_else(|_| default_manifest());
        let price_families = compute_defaults::price_families();
        let economics = build_economics(&price_families);
        Arc::new(
            ComputeGateway::new(
                manifest,
                GatewayLimits {
                    rate_per_sec: Some(2),
                    burst: Some(2),
                    replay_ttl: Duration::from_secs(120),
                    queue_capacity: compute_defaults::queue_depth_per_route().get(),
                    max_inflight: compute_defaults::max_inflight_per_route().get(),
                },
                price_families,
                compute_defaults::default_price_family(),
                economics,
            )
            .expect("gateway"),
        )
    }

    async fn call(gateway: Arc<ComputeGateway>, payload: &[u8]) -> (StatusCode, Vec<u8>) {
        let router = gateway.router().await;
        let body = GatewayRequest {
            namespace: "compute".parse().unwrap(),
            codec: ComputeCodec::NoritoJson,
            ttl_slots: std::num::NonZeroU64::new(4).unwrap(),
            gas_limit: std::num::NonZeroU64::new(1_000_000).unwrap(),
            max_response_bytes: std::num::NonZeroU64::new(1024).unwrap(),
            determinism: iroha_data_model::compute::ComputeDeterminism::Strict,
            execution_class: iroha_data_model::compute::ComputeExecutionClass::Cpu,
            declared_input_bytes: std::num::NonZeroU64::new(payload.len() as u64),
            declared_input_chunks: std::num::NonZeroU32::new(1),
            sponsor_budget_cu: None,
            price_family: compute_defaults::default_price_family(),
            resource_profile: compute_defaults::default_resource_profile(),
            auth: ComputeAuthz::Public,
            headers: BTreeMap::new(),
            payload_b64: base64::engine::general_purpose::STANDARD.encode(payload),
        };
        let request = Request::builder()
            .method("POST")
            .uri("/v1/compute/payments/quote")
            .header("content-type", "application/json")
            .body(axum::body::Body::from(json::to_vec(&body).expect("json")))
            .unwrap();
        let response = router.oneshot(request).await.unwrap();
        let status = response.status();
        let body_bytes = body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec();
        (status, body_bytes)
    }

    #[tokio::test]
    async fn happy_path_echo() {
        let gw = gateway_for_tests();
        let (status, body) = call(gw, br#"{"pair":"XOR/USD"}"#).await;
        assert_eq!(status, StatusCode::OK);
        let resp: GatewayResponse = json::from_slice(&body).expect("response");
        assert!(matches!(
            resp.receipt.outcome.kind,
            ComputeOutcomeKind::Success
        ));
        assert_eq!(
            resp.response_b64,
            Some(base64::engine::general_purpose::STANDARD.encode(br#"{"pair":"XOR/USD"}"#))
        );
        assert!(resp.receipt.metering.charged_units > 0);
    }

    #[tokio::test]
    async fn rejects_replay() {
        let gw = gateway_for_tests();
        let payload = br#"{}"#;
        let (status_ok, _) = call(gw.clone(), payload).await;
        assert_eq!(status_ok, StatusCode::OK);
        let (status_dup, _) = call(gw.clone(), payload).await;
        assert_eq!(status_dup, StatusCode::CONFLICT);
    }

    #[tokio::test]
    async fn rate_limits_by_auth() {
        let gw = gateway_for_tests();
        let router = gw.router().await;
        let build_request = |nonce: u8| {
            let payload = [b'p', b'i', b'n', b'g', nonce];
            let body = GatewayRequest {
                namespace: "compute".parse().unwrap(),
                codec: ComputeCodec::NoritoJson,
                ttl_slots: std::num::NonZeroU64::new(4).unwrap(),
                gas_limit: std::num::NonZeroU64::new(1_000_000).unwrap(),
                max_response_bytes: std::num::NonZeroU64::new(1024).unwrap(),
                determinism: iroha_data_model::compute::ComputeDeterminism::Strict,
                execution_class: iroha_data_model::compute::ComputeExecutionClass::Cpu,
                declared_input_bytes: std::num::NonZeroU64::new(payload.len() as u64),
                declared_input_chunks: std::num::NonZeroU32::new(1),
                sponsor_budget_cu: None,
                price_family: compute_defaults::default_price_family(),
                resource_profile: compute_defaults::default_resource_profile(),
                auth: ComputeAuthz::Public,
                headers: BTreeMap::new(),
                payload_b64: base64::engine::general_purpose::STANDARD.encode(payload),
            };
            let body_bytes = json::to_vec(&body).expect("json");
            Request::builder()
                .method("POST")
                .uri("/v1/compute/payments/quote")
                .header("content-type", "application/json")
                .body(axum::body::Body::from(body_bytes.clone()))
                .unwrap()
        };

        let resp1 = router.clone().oneshot(build_request(1)).await.unwrap();
        assert_eq!(resp1.status(), StatusCode::OK);
        // Second request consumes remaining burst; third should be limited.
        let _resp2 = router.clone().oneshot(build_request(2)).await.unwrap();
        let resp3 = router.oneshot(build_request(3)).await.unwrap();
        assert_eq!(resp3.status(), StatusCode::TOO_MANY_REQUESTS);
    }

    #[tokio::test]
    async fn sponsor_budget_is_enforced() {
        let gw = gateway_for_tests();
        let router = gw.router().await;
        let payload = br#"{"pair":"XOR/USD"}"#;
        let body = GatewayRequest {
            namespace: "compute".parse().unwrap(),
            codec: ComputeCodec::NoritoJson,
            ttl_slots: std::num::NonZeroU64::new(8).unwrap(),
            gas_limit: std::num::NonZeroU64::new(2_000_000).unwrap(),
            max_response_bytes: std::num::NonZeroU64::new(4_096).unwrap(),
            determinism: iroha_data_model::compute::ComputeDeterminism::Strict,
            execution_class: iroha_data_model::compute::ComputeExecutionClass::Cpu,
            declared_input_bytes: std::num::NonZeroU64::new(payload.len() as u64),
            declared_input_chunks: std::num::NonZeroU32::new(1),
            sponsor_budget_cu: std::num::NonZeroU64::new(1),
            price_family: compute_defaults::default_price_family(),
            resource_profile: compute_defaults::default_resource_profile(),
            auth: ComputeAuthz::Public,
            headers: BTreeMap::new(),
            payload_b64: base64::engine::general_purpose::STANDARD.encode(payload),
        };
        let request = Request::builder()
            .method("POST")
            .uri("/v1/compute/payments/quote")
            .header("content-type", "application/json")
            .body(axum::body::Body::from(json::to_vec(&body).expect("json")))
            .unwrap();

        let resp = router.oneshot(request).await.unwrap();
        assert_eq!(resp.status(), StatusCode::PAYMENT_REQUIRED);
    }

    #[tokio::test]
    async fn bench_summary_runs() {
        let manifest = default_manifest();
        let manifest_for_gateway = default_manifest();
        let price_families = compute_defaults::price_families();
        let economics = build_economics(&price_families);
        let gateway = Arc::new(
            ComputeGateway::new(
                manifest_for_gateway,
                GatewayLimits {
                    rate_per_sec: None,
                    burst: None,
                    replay_ttl: Duration::from_secs(30),
                    queue_capacity: compute_defaults::queue_depth_per_route().get(),
                    max_inflight: compute_defaults::max_inflight_per_route().get(),
                },
                price_families,
                compute_defaults::default_price_family(),
                economics,
            )
            .expect("gateway"),
        );
        let payload = br#"{"pair":"XOR/USD"}"#.to_vec();
        let summary = bench_gateway(
            gateway,
            manifest.namespace.clone(),
            &manifest.routes[0],
            payload,
            6,
            2,
            Some(BenchSloTargets {
                max_p50_ms: 5_000.0,
                max_p99_ms: 5_000.0,
                min_rps: 0.5,
                max_response_bytes: manifest.routes[0].max_response_bytes.get(),
            }),
        )
        .await
        .expect("bench");
        assert_eq!(summary.iterations, 6);
        assert_eq!(summary.successes, 6);
        assert!(summary.slo.as_ref().unwrap().passed);
    }
}
