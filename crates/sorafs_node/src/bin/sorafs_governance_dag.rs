//! Always-on SoraFS Governance DAG publisher and bounded public mirror.

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    fs::{self, File, OpenOptions},
    future::{Future, IntoFuture},
    io::{self, Read, Write},
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
    path::{Component, Path, PathBuf},
    process,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};

#[cfg(unix)]
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};

use axum::{
    Router,
    body::Body,
    extract::{Path as AxumPath, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::Response,
    routing::get,
};
use clap::Parser;
use iroha_config::{
    base::toml::TomlSource,
    parameters::actual::{SorafsGovernanceDagService, SorafsGovernanceDagServiceView},
};
use norito::{
    core::DecodeLimits,
    derive::{NoritoDeserialize, NoritoSerialize},
    json::{self, Map as JsonMap, Value as JsonValue},
};
use reqwest::{Client, Method, redirect::Policy};
use sorafs_manifest::{
    GovernanceDagBlockV1, GovernanceDagHeadV1, GovernanceLogPayloadV1,
    GovernanceSignatureAlgorithm, validate_governance_dag_head_against_chain_v1,
};
use thiserror::Error;
use tokio::{net::TcpListener, signal, sync::RwLock, time};
use url::Url;

const CONFIG_MAX_BYTES: u64 = 1024 * 1024;
const MUTABLE_STATE_MAX_BYTES: u64 = 64 * 1024 * 1024;
const RUNTIME_INDEX_MAX_BYTES: u64 = 64 * 1024 * 1024;
const SECRET_MAX_BYTES: u64 = 8 * 1024;
const CHECKPOINT_KEY_BYTES: usize = 32;
const CHECKPOINT_VERSION_V1: u8 = 1;
const PUBLISH_INTENT_VERSION_V1: u8 = 1;
const CHECKPOINT_AUTH_DOMAIN_V1: &[u8] = b"sorafs.governance_dag.checkpoint.auth.v1";
const INTENT_AUTH_DOMAIN_V1: &[u8] = b"sorafs.governance_dag.intent.auth.v1";
const RUNTIME_INDEX_SCHEMA: &str = "sorafs.governance_dag.runtime_signed_index.v1";
const MIRROR_INDEX_SCHEMA: &str = "sorafs.governance_dag.mirror.v1";
const CHECKPOINT_FILE: &str = "checkpoint.to";
const PUBLISH_INTENT_FILE: &str = "publish-intent.to";
const MIRROR_INDEX_FILE: &str = "mirror-index.json";
const SERVICE_LOCK_FILE: &str = ".service.lock";
const MAX_DNS_ADDRESSES: usize = 8;
const MAX_RESPONSE_HEADERS: usize = 64;
const MAX_RESPONSE_HEADER_BYTES: usize = 16 * 1024;
const MAX_IPFS_CID_BYTES: usize = 160;
const MAX_PUBLIC_TOKEN_BYTES: usize = 512;
const SOURCE_ENTRY_HARD_CAP: usize = 131_072;
const SOURCE_TOTAL_BYTES_HARD_CAP: u64 = 1024 * 1024 * 1024;
// Norito temporarily copies nested length-delimited fields while decoding.
// The governed block/head schemas stay below this amplification, while the
// finite multiplier still rejects archives that attempt allocation bombs.
const CANONICAL_DECODE_ALLOCATION_MULTIPLIER: usize = 16;
const SUPPORTED_RUNTIME_PAYLOAD_KINDS: &[&str] = &[
    "appeal_finance_report",
    "appeal_finance_settlement_receipt",
    "appeal_finance_weekly_rollup",
    "deal_settlement",
    "gc_audit",
    "moderation_ballot_event",
    "orderbook_settlement_receipt",
    "proof_token_issuance",
    "reconciliation",
    "repair_audit",
    "repair_slash",
    "reputation_snapshot",
    "transparency_ledger_publication",
];

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

#[cfg(unix)]
unsafe extern "C" {
    fn geteuid() -> std::os::raw::c_uint;
}

#[derive(Debug, Parser)]
#[command(
    author,
    version,
    about = "Always-on SoraFS Governance DAG publisher and mirror"
)]
struct Args {
    /// Iroha TOML containing `[sorafs.storage]` Governance DAG service fields.
    #[arg(long, value_name = "PATH")]
    config: PathBuf,
    /// Reconcile exactly once without starting the query listener.
    #[arg(long)]
    once: bool,
}

#[derive(Debug, Error)]
enum ServiceError {
    #[error("configuration rejected: {0}")]
    Config(String),
    #[error("filesystem safety check failed: {0}")]
    Filesystem(String),
    #[error("source snapshot rejected: {0}")]
    Source(String),
    #[error("durable state rejected: {0}")]
    State(String),
    #[error("network publication failed: {0}")]
    Network(String),
    #[error("public head conflict: {0}")]
    Conflict(String),
    #[error("service listener failed: {0}")]
    Listener(String),
}

struct SecretBytes(Vec<u8>);

impl SecretBytes {
    fn as_str(&self) -> Result<&str, ServiceError> {
        std::str::from_utf8(&self.0)
            .map_err(|_| ServiceError::Config("bearer token is not valid UTF-8".to_owned()))
    }
}

impl Drop for SecretBytes {
    fn drop(&mut self) {
        self.0.fill(0);
    }
}

impl fmt::Debug for SecretBytes {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("SecretBytes([REDACTED])")
    }
}

struct CheckpointKey([u8; 32]);

impl CheckpointKey {
    fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl Drop for CheckpointKey {
    fn drop(&mut self) {
        self.0.fill(0);
    }
}

impl fmt::Debug for CheckpointKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("CheckpointKey([REDACTED])")
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
struct CheckpointBodyV1 {
    version: u8,
    generation: u64,
    head_block_cid: Vec<u8>,
    block_count: u64,
    head_bytes_blake3: [u8; 32],
    head_ipfs_cid: String,
    public_head_token: String,
    source_index_blake3: [u8; 32],
    mirror_blake3: [u8; 32],
    published_at_unix: u64,
    mirror_blocks: Vec<PublishedBlockV1>,
}

#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, PartialEq, Eq)]
struct AuthenticatedCheckpointV1 {
    body: CheckpointBodyV1,
    authentication_tag: [u8; 32],
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
    target_source_index_blake3: [u8; 32],
    previous_public_head_blake3: Option<[u8; 32]>,
    created_at_unix: u64,
    blocks: Vec<IntentBlockV1>,
    head_ipfs_cid: Option<String>,
}

#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, PartialEq, Eq)]
struct AuthenticatedPublishIntentV1 {
    body: PublishIntentBodyV1,
    authentication_tag: [u8; 32],
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
    head: GovernanceDagHeadV1,
    head_bytes: Vec<u8>,
    blocks: Vec<SourceBlock>,
}

#[derive(Debug)]
struct RuntimeConfig {
    source_dir: PathBuf,
    state_dir: PathBuf,
    listen_addr: SocketAddr,
    poll_interval: Duration,
    max_response_bytes: u64,
    max_request_bytes: u64,
    mirror_max_entries: usize,
    mirror_max_bytes: u64,
    max_head_age_secs: u64,
    max_future_skew_secs: u64,
    allow_head_bootstrap: bool,
    expected_public_key: [u8; 32],
}

#[derive(Debug)]
struct PinnedEndpoint {
    url: Url,
    client: Client,
    bearer_token: Option<SecretBytes>,
}

#[derive(Debug)]
enum HeadMode {
    SignedHttp(PinnedEndpoint),
    Ipns { name: String, key_name: String },
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

struct Service {
    config: RuntimeConfig,
    checkpoint_key: CheckpointKey,
    checkpoint: Option<CheckpointBodyV1>,
    intent: Option<PublishIntentBodyV1>,
    ipfs: PinnedEndpoint,
    head_mode: HeadMode,
    api: ApiState,
    _state_lock: File,
}

#[tokio::main]
async fn main() {
    if let Err(err) = run().await {
        eprintln!("sorafs governance DAG service failed: {err}");
        process::exit(1);
    }
}

async fn run() -> Result<(), ServiceError> {
    let args = Args::parse();
    let view = load_service_config(&args.config)?;
    let mut service = Service::from_view(view).await?;
    if args.once {
        service.reconcile_once().await?;
        return Ok(());
    }

    let listener = TcpListener::bind(service.config.listen_addr)
        .await
        .map_err(|err| ServiceError::Listener(err.to_string()))?;
    let router = service_router(service.api.clone());
    let api = service.api.clone();
    api.0.write().await.live = true;
    let server = axum::serve(listener, router.into_make_service())
        .with_graceful_shutdown(shutdown_signal())
        .into_future();
    tokio::pin!(server);

    let mut interval = time::interval(service.config.poll_interval);
    interval.set_missed_tick_behavior(time::MissedTickBehavior::Skip);
    loop {
        tokio::select! {
            result = &mut server => {
                return result.map_err(|err| ServiceError::Listener(err.to_string()));
            }
            _ = interval.tick() => {
                if let Err(err) = service.reconcile_once().await {
                    let mut state = service.api.0.write().await;
                    state.ready = false;
                    state.last_error = Some(err.to_string());
                    state.metrics.publish_failure_total = state.metrics.publish_failure_total.saturating_add(1);
                    state.metrics.validation_failure_total = state.metrics.validation_failure_total.saturating_add(1);
                    if matches!(&service.head_mode, HeadMode::Ipns { .. }) {
                        state.metrics.ipns_update_failure_total = state.metrics.ipns_update_failure_total.saturating_add(1);
                    }
                    eprintln!("governance DAG reconciliation failed; readiness withdrawn: {err}");
                }
            }
        }
    }
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

fn load_service_config(path: &Path) -> Result<SorafsGovernanceDagServiceView, ServiceError> {
    let bytes = read_regular_file(path, CONFIG_MAX_BYTES, false)?;
    let text = std::str::from_utf8(&bytes)
        .map_err(|_| ServiceError::Config("configuration file is not UTF-8".to_owned()))?;
    let table = text
        .parse()
        .map_err(|err| ServiceError::Config(format!("configuration TOML is invalid: {err}")))?;
    SorafsGovernanceDagServiceView::from_toml_source(TomlSource::new(path.to_owned(), table))
        .map_err(|err| ServiceError::Config(err.to_string()))
}

impl Service {
    async fn from_view(view: SorafsGovernanceDagServiceView) -> Result<Self, ServiceError> {
        let service = view.service;
        if !service.enabled {
            return Err(ServiceError::Config(
                "sorafs.storage.governance_dag_service.enabled must be true".to_owned(),
            ));
        }
        let source_dir = secure_existing_directory(
            &view
                .source_dir
                .ok_or_else(|| ServiceError::Config("governance_dag_dir is missing".to_owned()))?,
            false,
        )?;
        let state_dir = service
            .state_dir
            .clone()
            .unwrap_or_else(|| source_dir.join("governance-dag-service"));
        let state_dir = secure_state_directory(&state_dir)?;
        let state_lock = acquire_service_lock(&state_dir)?;
        let listen_addr = service
            .listen_addr
            .parse::<SocketAddr>()
            .map_err(|_| ServiceError::Config("listen_addr is not a socket address".to_owned()))?;
        if !listen_addr.ip().is_loopback() {
            return Err(ServiceError::Config(
                "the Governance DAG status listener must bind a loopback address".to_owned(),
            ));
        }
        let expected_public_key = decode_fixed_hex::<32>(
            service.publisher_public_key_hex.as_deref().ok_or_else(|| {
                ServiceError::Config("publisher public key is missing".to_owned())
            })?,
            "publisher public key",
        )?;
        let checkpoint_key_path = service
            .checkpoint_key_path
            .as_deref()
            .ok_or_else(|| ServiceError::Config("checkpoint key path is missing".to_owned()))?;
        let mut checkpoint_key_bytes =
            read_regular_file(checkpoint_key_path, CHECKPOINT_KEY_BYTES as u64, true)?;
        if checkpoint_key_bytes.len() != CHECKPOINT_KEY_BYTES
            || checkpoint_key_bytes.iter().all(|byte| *byte == 0)
        {
            checkpoint_key_bytes.fill(0);
            return Err(ServiceError::Config(
                "checkpoint key must contain exactly 32 non-zero raw bytes".to_owned(),
            ));
        }
        let mut checkpoint_key = [0_u8; CHECKPOINT_KEY_BYTES];
        checkpoint_key.copy_from_slice(&checkpoint_key_bytes);
        checkpoint_key_bytes.fill(0);

        let runtime_config = RuntimeConfig {
            source_dir,
            state_dir,
            listen_addr,
            poll_interval: service.poll_interval,
            max_response_bytes: service.max_response_bytes.0,
            max_request_bytes: service.max_request_bytes.0,
            mirror_max_entries: service.mirror_max_entries,
            mirror_max_bytes: service.mirror_max_bytes.0,
            max_head_age_secs: service.max_head_age_secs,
            max_future_skew_secs: service.max_future_skew_secs,
            allow_head_bootstrap: service.allow_head_bootstrap,
            expected_public_key,
        };

        let ipfs_token = load_optional_secret(service.ipfs_bearer_token_path.as_deref())?;
        let ipfs_url = service
            .ipfs_api_url
            .as_deref()
            .ok_or_else(|| ServiceError::Config("IPFS API URL is missing".to_owned()))?;
        let ipfs = build_pinned_endpoint(ipfs_url, ipfs_token, &service, true).await?;
        let head_mode =
            match service.head_mode.as_str() {
                "signed_http" => {
                    let token = load_optional_secret(service.head_bearer_token_path.as_deref())?;
                    let url = service.signed_head_url.as_deref().ok_or_else(|| {
                        ServiceError::Config("signed head URL is missing".to_owned())
                    })?;
                    HeadMode::SignedHttp(build_pinned_endpoint(url, token, &service, false).await?)
                }
                "ipns" => HeadMode::Ipns {
                    name: validate_public_token(
                        service.ipns_name.as_deref().ok_or_else(|| {
                            ServiceError::Config("IPNS name is missing".to_owned())
                        })?,
                        "IPNS name",
                    )?,
                    key_name: validate_public_token(
                        service.ipns_key_name.as_deref().ok_or_else(|| {
                            ServiceError::Config("IPNS key name is missing".to_owned())
                        })?,
                        "IPNS key name",
                    )?,
                },
                _ => {
                    return Err(ServiceError::Config(
                        "head_mode must be signed_http or ipns".to_owned(),
                    ));
                }
            };
        let checkpoint_key = CheckpointKey(checkpoint_key);
        let checkpoint = load_checkpoint(&runtime_config.state_dir, checkpoint_key.as_bytes())?;
        let intent = load_publish_intent(&runtime_config.state_dir, checkpoint_key.as_bytes())?;
        let api = ApiState(Arc::new(RwLock::new(ApiSnapshot::default())));
        Ok(Self {
            config: runtime_config,
            checkpoint_key,
            checkpoint,
            intent,
            ipfs,
            head_mode,
            api,
            _state_lock: state_lock,
        })
    }
}

fn load_optional_secret(path: Option<&Path>) -> Result<Option<SecretBytes>, ServiceError> {
    let Some(path) = path else {
        return Ok(None);
    };
    let mut bytes = read_regular_file(path, SECRET_MAX_BYTES, true)?;
    if bytes.is_empty()
        || bytes
            .iter()
            .any(|byte| byte.is_ascii_control() || byte.is_ascii_whitespace())
    {
        bytes.fill(0);
        return Err(ServiceError::Config(
            "bearer-token file must contain one non-empty value without whitespace or controls"
                .to_owned(),
        ));
    }
    Ok(Some(SecretBytes(bytes)))
}

fn secure_existing_directory(path: &Path, secret: bool) -> Result<PathBuf, ServiceError> {
    let metadata = fs::symlink_metadata(path).map_err(|err| {
        ServiceError::Filesystem(format!("cannot inspect `{}`: {err}", path.display()))
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(ServiceError::Filesystem(format!(
            "`{}` must be a real directory",
            path.display()
        )));
    }
    #[cfg(unix)]
    if secret
        && (metadata.uid() != unsafe { geteuid() } || metadata.permissions().mode() & 0o077 != 0)
    {
        return Err(ServiceError::Filesystem(format!(
            "state directory `{}` must be owned by the service user and mode 0700 or stricter",
            path.display()
        )));
    }
    path.canonicalize().map_err(|err| {
        ServiceError::Filesystem(format!("cannot canonicalize `{}`: {err}", path.display()))
    })
}

fn secure_state_directory(path: &Path) -> Result<PathBuf, ServiceError> {
    if !path.exists() {
        fs::create_dir_all(path).map_err(|err| {
            ServiceError::Filesystem(format!(
                "cannot create state directory `{}`: {err}",
                path.display()
            ))
        })?;
        #[cfg(unix)]
        fs::set_permissions(path, fs::Permissions::from_mode(0o700)).map_err(|err| {
            ServiceError::Filesystem(format!(
                "cannot secure state directory `{}`: {err}",
                path.display()
            ))
        })?;
    }
    secure_existing_directory(path, true)
}

fn read_regular_file(path: &Path, max_bytes: u64, secret: bool) -> Result<Vec<u8>, ServiceError> {
    let before = fs::symlink_metadata(path).map_err(|err| {
        ServiceError::Filesystem(format!("cannot inspect `{}`: {err}", path.display()))
    })?;
    validate_regular_metadata(path, &before, max_bytes, secret)?;
    let mut options = OpenOptions::new();
    options.read(true);
    set_no_follow_flag(&mut options);
    let mut file = options.open(path).map_err(|err| {
        ServiceError::Filesystem(format!("cannot open `{}`: {err}", path.display()))
    })?;
    let opened = file.metadata().map_err(|err| {
        ServiceError::Filesystem(format!("cannot inspect open `{}`: {err}", path.display()))
    })?;
    validate_regular_metadata(path, &opened, max_bytes, secret)?;
    if !same_file(&before, &opened) {
        return Err(ServiceError::Filesystem(format!(
            "`{}` changed while being opened",
            path.display()
        )));
    }
    let capacity = usize::try_from(opened.len()).map_err(|_| {
        ServiceError::Filesystem(format!("`{}` exceeds host size limits", path.display()))
    })?;
    let mut bytes = Vec::with_capacity(capacity);
    Read::by_ref(&mut file)
        .take(max_bytes.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|err| {
            ServiceError::Filesystem(format!("cannot read `{}`: {err}", path.display()))
        })?;
    if bytes.len() as u64 > max_bytes || bytes.len() as u64 != opened.len() {
        return Err(ServiceError::Filesystem(format!(
            "`{}` grew, shrank, or exceeds its {} byte limit",
            path.display(),
            max_bytes
        )));
    }
    let after = fs::symlink_metadata(path).map_err(|err| {
        ServiceError::Filesystem(format!("cannot re-inspect `{}`: {err}", path.display()))
    })?;
    validate_regular_metadata(path, &after, max_bytes, secret)?;
    if !same_file(&opened, &after) || after.len() != opened.len() {
        return Err(ServiceError::Filesystem(format!(
            "`{}` changed while being read",
            path.display()
        )));
    }
    Ok(bytes)
}

fn validate_regular_metadata(
    path: &Path,
    metadata: &fs::Metadata,
    max_bytes: u64,
    secret: bool,
) -> Result<(), ServiceError> {
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(ServiceError::Filesystem(format!(
            "`{}` must be a regular file",
            path.display()
        )));
    }
    if metadata.len() > max_bytes {
        return Err(ServiceError::Filesystem(format!(
            "`{}` exceeds its {} byte limit",
            path.display(),
            max_bytes
        )));
    }
    #[cfg(unix)]
    {
        if metadata.nlink() != 1 {
            return Err(ServiceError::Filesystem(format!(
                "`{}` must have exactly one hard link",
                path.display()
            )));
        }
        if secret
            && (metadata.uid() != unsafe { geteuid() }
                || metadata.permissions().mode() & 0o077 != 0)
        {
            return Err(ServiceError::Filesystem(format!(
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

fn acquire_service_lock(state_dir: &Path) -> Result<File, ServiceError> {
    let path = state_dir.join(SERVICE_LOCK_FILE);
    reject_unsafe_output(&path)?;
    let mut options = OpenOptions::new();
    options.read(true).write(true).create(true);
    #[cfg(unix)]
    options.mode(0o600);
    set_no_follow_flag(&mut options);
    let file = options
        .open(&path)
        .map_err(|err| ServiceError::Filesystem(format!("cannot open service lock: {err}")))?;
    validate_regular_metadata(
        &path,
        &file.metadata().map_err(|err| {
            ServiceError::Filesystem(format!("cannot inspect service lock: {err}"))
        })?,
        4096,
        true,
    )?;
    match file.try_lock() {
        Ok(()) => Ok(file),
        Err(fs::TryLockError::WouldBlock) => Err(ServiceError::Filesystem(
            "another Governance DAG service owns the configured state directory".to_owned(),
        )),
        Err(fs::TryLockError::Error(err)) => Err(ServiceError::Filesystem(format!(
            "cannot acquire Governance DAG service lock: {err}"
        ))),
    }
}

fn write_atomic_secret(path: &Path, bytes: &[u8]) -> Result<(), ServiceError> {
    let parent = path.parent().ok_or_else(|| {
        ServiceError::Filesystem("durable output has no parent directory".to_owned())
    })?;
    reject_unsafe_output(path)?;
    let counter = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let temp = parent.join(format!(
        ".{}.tmp-{}-{counter}",
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("governance-state"),
        process::id()
    ));
    let result = (|| {
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        options.mode(0o600);
        set_no_follow_flag(&mut options);
        let mut file = options.open(&temp).map_err(|err| {
            ServiceError::Filesystem(format!("cannot create durable temp file: {err}"))
        })?;
        file.write_all(bytes).map_err(|err| {
            ServiceError::Filesystem(format!("cannot write durable temp file: {err}"))
        })?;
        file.sync_all().map_err(|err| {
            ServiceError::Filesystem(format!("cannot sync durable temp file: {err}"))
        })?;
        drop(file);
        reject_unsafe_output(path)?;
        fs::rename(&temp, path).map_err(|err| {
            ServiceError::Filesystem(format!("cannot install durable state: {err}"))
        })?;
        sync_directory(parent)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}

fn remove_durable_file(path: &Path) -> Result<(), ServiceError> {
    if !path.exists() {
        return Ok(());
    }
    let metadata = fs::symlink_metadata(path)
        .map_err(|err| ServiceError::Filesystem(format!("cannot inspect durable file: {err}")))?;
    validate_regular_metadata(path, &metadata, MUTABLE_STATE_MAX_BYTES, true)?;
    fs::remove_file(path)
        .map_err(|err| ServiceError::Filesystem(format!("cannot remove durable intent: {err}")))?;
    let parent = path.parent().ok_or_else(|| {
        ServiceError::Filesystem("durable intent path has no parent directory".to_owned())
    })?;
    sync_directory(parent)
}

fn reject_unsafe_output(path: &Path) -> Result<(), ServiceError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            validate_regular_metadata(path, &metadata, MUTABLE_STATE_MAX_BYTES, true)?;
        }
        Err(err) if err.kind() == io::ErrorKind::NotFound => {}
        Err(err) => {
            return Err(ServiceError::Filesystem(format!(
                "cannot inspect output `{}`: {err}",
                path.display()
            )));
        }
    }
    if let Some(parent) = path.parent() {
        let metadata = fs::symlink_metadata(parent).map_err(|err| {
            ServiceError::Filesystem(format!(
                "cannot inspect output directory `{}`: {err}",
                parent.display()
            ))
        })?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(ServiceError::Filesystem(
                "durable output parent must be a real directory".to_owned(),
            ));
        }
    }
    Ok(())
}

fn sync_directory(path: &Path) -> Result<(), ServiceError> {
    #[cfg(unix)]
    File::open(path)
        .and_then(|file| file.sync_all())
        .map_err(|err| ServiceError::Filesystem(format!("cannot sync state directory: {err}")))?;
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}

#[cfg(unix)]
fn set_no_follow_flag(options: &mut OpenOptions) {
    options.custom_flags(platform_no_follow_flag());
}

#[cfg(not(unix))]
fn set_no_follow_flag(_options: &mut OpenOptions) {}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn platform_no_follow_flag() -> i32 {
    0o400000
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
fn platform_no_follow_flag() -> i32 {
    0
}

fn decode_fixed_hex<const N: usize>(value: &str, label: &str) -> Result<[u8; N], ServiceError> {
    if value.len() != N * 2
        || !value
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
    {
        return Err(ServiceError::Config(format!(
            "{label} must be canonical lowercase {}-byte hex",
            N
        )));
    }
    let bytes =
        hex::decode(value).map_err(|_| ServiceError::Config(format!("{label} is invalid hex")))?;
    let mut out = [0_u8; N];
    out.copy_from_slice(&bytes);
    Ok(out)
}

fn validate_public_token(value: &str, label: &str) -> Result<String, ServiceError> {
    if value.is_empty()
        || value.len() > MAX_PUBLIC_TOKEN_BYTES
        || value.trim() != value
        || value.chars().any(char::is_control)
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':'))
    {
        return Err(ServiceError::Config(format!("{label} is not canonical")));
    }
    Ok(value.to_owned())
}

fn current_unix_timestamp_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}

fn blake3_array(bytes: &[u8]) -> [u8; 32] {
    *blake3::hash(bytes).as_bytes()
}

fn auth_tag<T: for<'de> norito::NoritoDeserialize<'de> + norito::NoritoSerialize>(
    key: &[u8; 32],
    domain: &[u8],
    body: &T,
) -> Result<[u8; 32], ServiceError> {
    let encoded = norito::to_bytes(body)
        .map_err(|err| ServiceError::State(format!("cannot encode durable state: {err}")))?;
    let mut hasher = blake3::Hasher::new_keyed(key);
    hasher.update(domain);
    hasher.update(&encoded);
    Ok(*hasher.finalize().as_bytes())
}

fn tags_equal(left: &[u8; 32], right: &[u8; 32]) -> bool {
    left.iter()
        .zip(right)
        .fold(0_u8, |difference, (left, right)| {
            difference | (left ^ right)
        })
        == 0
}

fn durable_decode_limits(max_bytes: u64) -> DecodeLimits {
    let max = usize::try_from(max_bytes).unwrap_or(usize::MAX);
    DecodeLimits::new(150_000, max, 1_000_000, max.saturating_mul(2), 128)
}

fn load_checkpoint(
    state_dir: &Path,
    key: &[u8; 32],
) -> Result<Option<CheckpointBodyV1>, ServiceError> {
    let path = state_dir.join(CHECKPOINT_FILE);
    if !path.exists() {
        return Ok(None);
    }
    let bytes = read_regular_file(&path, MUTABLE_STATE_MAX_BYTES, true)?;
    let envelope: AuthenticatedCheckpointV1 = norito::decode_from_bytes_with_limits(
        &bytes,
        durable_decode_limits(MUTABLE_STATE_MAX_BYTES),
    )
    .map_err(|err| ServiceError::State(format!("checkpoint decode failed: {err}")))?;
    if norito::to_bytes(&envelope).map_err(|err| ServiceError::State(err.to_string()))? != bytes {
        return Err(ServiceError::State(
            "checkpoint encoding is not canonical".to_owned(),
        ));
    }
    if envelope.body.version != CHECKPOINT_VERSION_V1 {
        return Err(ServiceError::State(
            "checkpoint version is unsupported".to_owned(),
        ));
    }
    let expected = auth_tag(key, CHECKPOINT_AUTH_DOMAIN_V1, &envelope.body)?;
    if !tags_equal(&expected, &envelope.authentication_tag) {
        return Err(ServiceError::State(
            "checkpoint authentication failed".to_owned(),
        ));
    }
    validate_checkpoint_body(&envelope.body)?;
    Ok(Some(envelope.body))
}

fn save_checkpoint(
    state_dir: &Path,
    key: &[u8; 32],
    body: &CheckpointBodyV1,
) -> Result<(), ServiceError> {
    validate_checkpoint_body(body)?;
    let envelope = AuthenticatedCheckpointV1 {
        body: body.clone(),
        authentication_tag: auth_tag(key, CHECKPOINT_AUTH_DOMAIN_V1, body)?,
    };
    let bytes = norito::to_bytes(&envelope)
        .map_err(|err| ServiceError::State(format!("checkpoint encode failed: {err}")))?;
    write_atomic_secret(&state_dir.join(CHECKPOINT_FILE), &bytes)
}

fn validate_checkpoint_body(body: &CheckpointBodyV1) -> Result<(), ServiceError> {
    if body.version != CHECKPOINT_VERSION_V1
        || body.generation == 0
        || body.block_count == 0
        || body.head_block_cid.len() != 32
        || !is_canonical_cid_v1(&body.head_ipfs_cid)
        || body.public_head_token.is_empty()
        || body.public_head_token.len() > MAX_PUBLIC_TOKEN_BYTES
        || body.mirror_blocks.is_empty()
    {
        return Err(ServiceError::State(
            "checkpoint fields violate first-release bounds".to_owned(),
        ));
    }
    let mut previous = None;
    let mut seen = BTreeSet::new();
    for block in &body.mirror_blocks {
        validate_published_block(block)?;
        if previous.is_some_and(|value| block.sequence != value + 1)
            || !seen.insert(block.governance_block_cid.clone())
        {
            return Err(ServiceError::State(
                "checkpoint mirror block order is invalid".to_owned(),
            ));
        }
        previous = Some(block.sequence);
    }
    if body
        .mirror_blocks
        .last()
        .is_none_or(|block| block.governance_block_cid != body.head_block_cid)
    {
        return Err(ServiceError::State(
            "checkpoint mirror does not end at the public head".to_owned(),
        ));
    }
    Ok(())
}

fn validate_published_block(block: &PublishedBlockV1) -> Result<(), ServiceError> {
    if block.governance_block_cid.len() != 32
        || block.governance_node_cid.len() != 32
        || block.payload_kind.is_empty()
        || block.encoded_len == 0
        || !is_canonical_cid_v1(&block.ipfs_cid)
    {
        return Err(ServiceError::State(
            "published block fields violate first-release bounds".to_owned(),
        ));
    }
    Ok(())
}

fn load_publish_intent(
    state_dir: &Path,
    key: &[u8; 32],
) -> Result<Option<PublishIntentBodyV1>, ServiceError> {
    let path = state_dir.join(PUBLISH_INTENT_FILE);
    if !path.exists() {
        return Ok(None);
    }
    let bytes = read_regular_file(&path, MUTABLE_STATE_MAX_BYTES, true)?;
    let envelope: AuthenticatedPublishIntentV1 = norito::decode_from_bytes_with_limits(
        &bytes,
        durable_decode_limits(MUTABLE_STATE_MAX_BYTES),
    )
    .map_err(|err| ServiceError::State(format!("publish intent decode failed: {err}")))?;
    if norito::to_bytes(&envelope).map_err(|err| ServiceError::State(err.to_string()))? != bytes {
        return Err(ServiceError::State(
            "publish intent encoding is not canonical".to_owned(),
        ));
    }
    if envelope.body.version != PUBLISH_INTENT_VERSION_V1 {
        return Err(ServiceError::State(
            "publish intent version is unsupported".to_owned(),
        ));
    }
    let expected = auth_tag(key, INTENT_AUTH_DOMAIN_V1, &envelope.body)?;
    if !tags_equal(&expected, &envelope.authentication_tag) {
        return Err(ServiceError::State(
            "publish intent authentication failed".to_owned(),
        ));
    }
    validate_publish_intent(&envelope.body)?;
    Ok(Some(envelope.body))
}

fn save_publish_intent(
    state_dir: &Path,
    key: &[u8; 32],
    body: &PublishIntentBodyV1,
) -> Result<(), ServiceError> {
    validate_publish_intent(body)?;
    let envelope = AuthenticatedPublishIntentV1 {
        body: body.clone(),
        authentication_tag: auth_tag(key, INTENT_AUTH_DOMAIN_V1, body)?,
    };
    let bytes = norito::to_bytes(&envelope)
        .map_err(|err| ServiceError::State(format!("publish intent encode failed: {err}")))?;
    write_atomic_secret(&state_dir.join(PUBLISH_INTENT_FILE), &bytes)
}

fn validate_publish_intent(body: &PublishIntentBodyV1) -> Result<(), ServiceError> {
    if body.version != PUBLISH_INTENT_VERSION_V1
        || body.generation == 0
        || body.target_block_count == 0
        || body.target_head_block_cid.len() != 32
        || body.target_head_bytes.is_empty()
        || body.target_head_bytes.len() as u64 > MUTABLE_STATE_MAX_BYTES
        || body.blocks.is_empty()
    {
        return Err(ServiceError::State(
            "publish intent fields violate first-release bounds".to_owned(),
        ));
    }
    let mut previous = None;
    let mut seen = BTreeSet::new();
    for block in &body.blocks {
        if block.governance_block_cid.len() != 32
            || block.governance_node_cid.len() != 32
            || block.payload_kind.is_empty()
            || block.encoded_len == 0
            || block
                .ipfs_cid
                .as_ref()
                .is_some_and(|cid| !is_canonical_cid_v1(cid))
        {
            return Err(ServiceError::State(
                "publish intent block fields are invalid".to_owned(),
            ));
        }
        if previous.is_some_and(|value| block.sequence != value + 1)
            || !seen.insert(block.governance_block_cid.clone())
        {
            return Err(ServiceError::State(
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
        return Err(ServiceError::State(
            "publish intent head CID is not canonical CIDv1 base32".to_owned(),
        ));
    }
    Ok(())
}

fn resolve_index_path(root: &Path, raw: &str) -> Result<PathBuf, ServiceError> {
    if raw.is_empty() || raw.contains('\\') {
        return Err(ServiceError::Source(
            "runtime index path is empty or contains a backslash".to_owned(),
        ));
    }
    let relative = Path::new(raw);
    if relative.is_absolute() {
        return Err(ServiceError::Source(
            "runtime index path must be relative".to_owned(),
        ));
    }
    let mut path = root.to_owned();
    for component in relative.components() {
        match component {
            Component::Normal(value) => path.push(value),
            _ => {
                return Err(ServiceError::Source(
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

fn read_verified_sidecar_file(path: &Path, max_bytes: u64) -> Result<Vec<u8>, ServiceError> {
    let bytes = read_regular_file(path, max_bytes, false)?;
    let sidecar = read_regular_file(&digest_sidecar_path(path), 65, false)?;
    let expected = format!("{}\n", hex::encode(blake3_array(&bytes)));
    if sidecar != expected.as_bytes() {
        return Err(ServiceError::Source(format!(
            "digest sidecar does not match `{}`",
            path.display()
        )));
    }
    Ok(bytes)
}

fn decode_canonical<T>(bytes: &[u8], label: &str) -> Result<T, ServiceError>
where
    for<'de> T: norito::NoritoDeserialize<'de>,
    T: norito::NoritoSerialize,
{
    let max = bytes.len().max(1);
    let value = norito::decode_from_bytes_with_limits(
        bytes,
        DecodeLimits::new(
            65_536,
            max,
            1_000_000,
            max.saturating_mul(CANONICAL_DECODE_ALLOCATION_MULTIPLIER),
            128,
        ),
    )
    .map_err(|err| ServiceError::Source(format!("{label} decode failed: {err}")))?;
    let canonical = norito::to_bytes(&value)
        .map_err(|err| ServiceError::Source(format!("{label} encode failed: {err}")))?;
    if canonical != bytes {
        return Err(ServiceError::Source(format!(
            "{label} is not canonical Norito"
        )));
    }
    Ok(value)
}

fn required_json_string(map: &JsonMap, field: &str) -> Result<String, ServiceError> {
    map.get(field)
        .and_then(JsonValue::as_str)
        .map(str::to_owned)
        .ok_or_else(|| ServiceError::Source(format!("runtime index is missing `{field}`")))
}

fn required_json_u64(map: &JsonMap, field: &str) -> Result<u64, ServiceError> {
    map.get(field)
        .and_then(JsonValue::as_u64)
        .ok_or_else(|| ServiceError::Source(format!("runtime index is missing `{field}`")))
}

fn optional_json_string(map: &JsonMap, field: &str) -> Result<Option<String>, ServiceError> {
    match map.get(field) {
        None | Some(JsonValue::Null) => Ok(None),
        Some(value) => value
            .as_str()
            .map(|value| Some(value.to_owned()))
            .ok_or_else(|| {
                ServiceError::Source(format!("runtime index `{field}` is not a string"))
            }),
    }
}

fn canonical_hex_vec(
    value: &str,
    expected_bytes: usize,
    label: &str,
) -> Result<Vec<u8>, ServiceError> {
    if value.len() != expected_bytes * 2
        || !value
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
    {
        return Err(ServiceError::Source(format!(
            "{label} must be canonical lowercase {expected_bytes}-byte hex"
        )));
    }
    hex::decode(value).map_err(|_| ServiceError::Source(format!("{label} is invalid hex")))
}

fn payload_kind(payload: &GovernanceLogPayloadV1) -> String {
    match payload {
        GovernanceLogPayloadV1::ProviderAdvert(_) => "provider_advert".to_owned(),
        GovernanceLogPayloadV1::ReplicationOrder(_) => "replication_order".to_owned(),
        GovernanceLogPayloadV1::PorChallenge(_) => "por_challenge".to_owned(),
        GovernanceLogPayloadV1::PorProof(_) => "por_proof".to_owned(),
        GovernanceLogPayloadV1::PdpArchive(_) => "pdp_archive".to_owned(),
        GovernanceLogPayloadV1::AuditVerdict(_) => "audit_verdict".to_owned(),
        GovernanceLogPayloadV1::DealSettlement(_) => "deal_settlement".to_owned(),
        GovernanceLogPayloadV1::SignedReputationSnapshot(_) => "reputation_snapshot".to_owned(),
        GovernanceLogPayloadV1::ModerationBallotEvent(_) => "moderation_ballot_event".to_owned(),
        GovernanceLogPayloadV1::AppealFinanceReport(_) => "appeal_finance_report".to_owned(),
        GovernanceLogPayloadV1::AppealFinanceWeeklyRollup(_) => {
            "appeal_finance_weekly_rollup".to_owned()
        }
        GovernanceLogPayloadV1::AppealFinanceSettlementReceipt(_) => {
            "appeal_finance_settlement_receipt".to_owned()
        }
        GovernanceLogPayloadV1::OrderbookSettlementReceipt(_) => {
            "orderbook_settlement_receipt".to_owned()
        }
        GovernanceLogPayloadV1::ExternalPayload(payload) => payload.payload_kind.clone(),
    }
}

fn validate_expected_signer(
    block: &GovernanceDagBlockV1,
    expected_public_key: &[u8; 32],
    expected_peer_id: &[u8],
) -> Result<(), ServiceError> {
    if block.block_signature.algorithm != GovernanceSignatureAlgorithm::Ed25519
        || block.node.publisher_signature.algorithm != GovernanceSignatureAlgorithm::Ed25519
        || block.block_signature.public_key.as_slice() != expected_public_key
        || block.node.publisher_signature.public_key.as_slice() != expected_public_key
    {
        return Err(ServiceError::Source(
            "runtime DAG block or node is signed by an unexpected key".to_owned(),
        ));
    }
    if block.publisher_peer_id != expected_peer_id
        || block.node.publisher_peer_id != expected_peer_id
    {
        return Err(ServiceError::Source(
            "runtime DAG block or node uses an unexpected publisher peer id".to_owned(),
        ));
    }
    Ok(())
}

fn load_source_snapshot(config: &RuntimeConfig) -> Result<SourceSnapshot, ServiceError> {
    let index_path = config.source_dir.join("runtime-dag-index.json");
    let index_bytes = read_verified_sidecar_file(&index_path, RUNTIME_INDEX_MAX_BYTES)?;
    let index_blake3 = blake3_array(&index_bytes);
    let index: JsonValue = json::from_slice(&index_bytes)
        .map_err(|err| ServiceError::Source(format!("runtime index JSON is invalid: {err}")))?;
    let map = index
        .as_object()
        .ok_or_else(|| ServiceError::Source("runtime index root is not an object".to_owned()))?;
    if map.get("schema").and_then(JsonValue::as_str) != Some(RUNTIME_INDEX_SCHEMA) {
        return Err(ServiceError::Source(
            "runtime index schema is unsupported".to_owned(),
        ));
    }
    let key_hex = required_json_string(map, "publisher_public_key_hex")?;
    if decode_fixed_hex::<32>(&key_hex, "runtime index publisher key")?
        != config.expected_public_key
    {
        return Err(ServiceError::Source(
            "runtime index publisher key does not match configuration".to_owned(),
        ));
    }
    let peer_hex = required_json_string(map, "publisher_peer_id_hex")?;
    if peer_hex.is_empty() || peer_hex.len() > 1024 || peer_hex.len() % 2 != 0 {
        return Err(ServiceError::Source(
            "runtime index publisher peer id is invalid".to_owned(),
        ));
    }
    let peer_id = canonical_hex_vec(&peer_hex, peer_hex.len() / 2, "publisher peer id")?;
    let block_values = map
        .get("blocks")
        .and_then(JsonValue::as_array)
        .ok_or_else(|| ServiceError::Source("runtime index blocks are missing".to_owned()))?;
    if block_values.is_empty() || block_values.len() > SOURCE_ENTRY_HARD_CAP {
        return Err(ServiceError::Source(format!(
            "runtime index block count must be within 1..={SOURCE_ENTRY_HARD_CAP}"
        )));
    }
    let advertised_count = required_json_u64(map, "block_count")?;
    if advertised_count != block_values.len() as u64 {
        return Err(ServiceError::Source(
            "runtime index block_count does not match its blocks array".to_owned(),
        ));
    }

    let now = current_unix_timestamp_seconds();
    let latest_allowed = now.saturating_add(config.max_future_skew_secs);
    let mut blocks = Vec::with_capacity(block_values.len());
    let mut decoded_blocks = Vec::with_capacity(block_values.len());
    let mut expected_by_digest = JsonMap::new();
    let mut expected_by_kind = BTreeMap::<String, Vec<JsonValue>>::new();
    let mut total_bytes = 0_u64;
    let mut previous_node_cid: Option<Vec<u8>> = None;
    for (position, value) in block_values.iter().enumerate() {
        let entry = value.as_object().ok_or_else(|| {
            ServiceError::Source(format!("runtime index block {position} is not an object"))
        })?;
        if required_json_u64(entry, "position")? != position as u64
            || required_json_u64(entry, "sequence")? != position as u64
        {
            return Err(ServiceError::Source(format!(
                "runtime index block {position} position or sequence is invalid"
            )));
        }
        let block_path = required_json_string(entry, "block_path")?;
        let path = resolve_index_path(&config.source_dir, &block_path)?;
        let bytes = read_verified_sidecar_file(&path, config.max_request_bytes)?;
        if required_json_u64(entry, "encoded_len")? != bytes.len() as u64 {
            return Err(ServiceError::Source(format!(
                "runtime index block {position} encoded_len is invalid"
            )));
        }
        total_bytes = total_bytes
            .checked_add(bytes.len() as u64)
            .ok_or_else(|| ServiceError::Source("source byte count overflow".to_owned()))?;
        if total_bytes > SOURCE_TOTAL_BYTES_HARD_CAP {
            return Err(ServiceError::Source(format!(
                "runtime DAG exceeds the {SOURCE_TOTAL_BYTES_HARD_CAP} byte hard cap"
            )));
        }
        let block: GovernanceDagBlockV1 = decode_canonical(&bytes, "governance DAG block")?;
        block
            .validate()
            .map_err(|err| ServiceError::Source(format!("block {position} is invalid: {err}")))?;
        validate_expected_signer(&block, &config.expected_public_key, &peer_id)?;
        if block.sequence != position as u64 || block.timestamp > latest_allowed {
            return Err(ServiceError::Source(format!(
                "block {position} sequence or timestamp is invalid"
            )));
        }
        if block.node.prev_cid != previous_node_cid {
            return Err(ServiceError::Source(format!(
                "block {position} node parent link is invalid"
            )));
        }
        previous_node_cid = Some(block.node.node_cid.clone());
        let block_cid_hex = required_json_string(entry, "block_cid_hex")?;
        let node_cid_hex = required_json_string(entry, "node_cid_hex")?;
        if canonical_hex_vec(&block_cid_hex, 32, "block CID")? != block.block_cid
            || canonical_hex_vec(&node_cid_hex, 32, "node CID")? != block.node.node_cid
        {
            return Err(ServiceError::Source(format!(
                "runtime index block {position} CID does not match canonical bytes"
            )));
        }
        let expected_block_path = format!(
            "runtime-dag/blocks/{:020}_{}.to",
            block.sequence, block_cid_hex
        );
        if block_path != expected_block_path {
            return Err(ServiceError::Source(format!(
                "runtime index block {position} path does not bind its sequence and CID"
            )));
        }
        let expected_prev_block = optional_json_string(entry, "prev_block_cid_hex")?
            .map(|value| canonical_hex_vec(&value, 32, "previous block CID"))
            .transpose()?;
        let expected_prev_node = optional_json_string(entry, "prev_node_cid_hex")?
            .map(|value| canonical_hex_vec(&value, 32, "previous node CID"))
            .transpose()?;
        if expected_prev_block != block.prev_block_cid || expected_prev_node != block.node.prev_cid
        {
            return Err(ServiceError::Source(format!(
                "runtime index block {position} parent metadata is invalid"
            )));
        }
        let kind = payload_kind(&block.node.payload);
        if SUPPORTED_RUNTIME_PAYLOAD_KINDS
            .binary_search(&kind.as_str())
            .is_err()
        {
            return Err(ServiceError::Source(format!(
                "runtime index block {position} uses unsupported payload kind `{kind}`"
            )));
        }
        if required_json_string(entry, "payload_kind")? != kind {
            return Err(ServiceError::Source(format!(
                "runtime index block {position} payload kind is invalid"
            )));
        }
        let digest = blake3_array(&bytes);
        if required_json_string(entry, "encoded_blake3")? != hex::encode(digest) {
            return Err(ServiceError::Source(format!(
                "runtime index block {position} digest is invalid"
            )));
        }
        expected_by_digest.insert(
            hex::encode(digest),
            JsonValue::Array(vec![JsonValue::from(position as u64)]),
        );
        expected_by_kind
            .entry(kind.clone())
            .or_default()
            .push(JsonValue::from(position as u64));
        decoded_blocks.push(block.clone());
        blocks.push(SourceBlock {
            block,
            bytes,
            encoded_blake3: digest,
            payload_kind: kind,
        });
    }

    let expected_by_kind = expected_by_kind
        .into_iter()
        .map(|(kind, positions)| (kind, JsonValue::Array(positions)))
        .collect::<JsonMap>();
    if map.get("by_encoded_blake3") != Some(&JsonValue::Object(expected_by_digest))
        || map.get("by_payload_kind") != Some(&JsonValue::Object(expected_by_kind))
    {
        return Err(ServiceError::Source(
            "runtime index lookup maps are non-canonical or inconsistent".to_owned(),
        ));
    }

    let head_path_label = required_json_string(map, "head_path")?;
    if head_path_label != "runtime-dag/head.to" {
        return Err(ServiceError::Source(
            "runtime index head_path is not canonical".to_owned(),
        ));
    }
    let head_path = resolve_index_path(&config.source_dir, &head_path_label)?;
    let head_bytes = read_verified_sidecar_file(&head_path, config.max_request_bytes)?;
    let head: GovernanceDagHeadV1 = decode_canonical(&head_bytes, "governance DAG head")?;
    validate_governance_dag_head_against_chain_v1(&head, &decoded_blocks)
        .map_err(|err| ServiceError::Source(format!("signed head chain is invalid: {err}")))?;
    if head.head_signature.algorithm != GovernanceSignatureAlgorithm::Ed25519
        || head.head_signature.public_key.as_slice() != config.expected_public_key
        || head.publisher_peer_id != peer_id
    {
        return Err(ServiceError::Source(
            "signed head uses an unexpected key or peer id".to_owned(),
        ));
    }
    if head.generated_at > latest_allowed
        || blocks
            .last()
            .is_some_and(|block| head.generated_at < block.block.timestamp)
        || now.saturating_sub(head.generated_at) > config.max_head_age_secs
    {
        return Err(ServiceError::Source(
            "signed head is stale, future-dated, or predates its tip".to_owned(),
        ));
    }
    if required_json_string(map, "head_block_cid_hex")? != hex::encode(&head.head_block_cid)
        || required_json_u64(map, "head_generated_at")? != head.generated_at
    {
        return Err(ServiceError::Source(
            "runtime index head metadata does not match signed head bytes".to_owned(),
        ));
    }
    let stable_index = read_verified_sidecar_file(&index_path, RUNTIME_INDEX_MAX_BYTES)?;
    if stable_index != index_bytes {
        return Err(ServiceError::Source(
            "runtime index changed while the source snapshot was being read".to_owned(),
        ));
    }
    Ok(SourceSnapshot {
        index_blake3,
        head,
        head_bytes,
        blocks,
    })
}

async fn build_pinned_endpoint(
    raw: &str,
    bearer_token: Option<SecretBytes>,
    config: &SorafsGovernanceDagService,
    ipfs_base: bool,
) -> Result<PinnedEndpoint, ServiceError> {
    if raw.is_empty()
        || raw.trim() != raw
        || raw.contains('\\')
        || raw.chars().any(char::is_control)
    {
        return Err(ServiceError::Config(
            "endpoint URL contains non-canonical text".to_owned(),
        ));
    }
    let mut url = Url::parse(raw)
        .map_err(|_| ServiceError::Config("endpoint URL is not absolute".to_owned()))?;
    if !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(ServiceError::Config(
            "endpoint URL must not contain credentials, query, or fragment".to_owned(),
        ));
    }
    match url.scheme() {
        "https" => {}
        "http" if config.allow_insecure_http => {}
        "http" => {
            return Err(ServiceError::Config(
                "plain HTTP endpoint requires allow_insecure_http".to_owned(),
            ));
        }
        _ => {
            return Err(ServiceError::Config(
                "endpoint URL scheme must be http or https".to_owned(),
            ));
        }
    }
    let host = url
        .host_str()
        .ok_or_else(|| ServiceError::Config("endpoint URL has no host".to_owned()))?
        .to_owned();
    let port = url
        .port_or_known_default()
        .ok_or_else(|| ServiceError::Config("endpoint URL has no usable port".to_owned()))?;
    if ipfs_base {
        let path = url.path().trim_end_matches('/');
        let normalized_path = if path.is_empty() {
            "/".to_owned()
        } else {
            format!("{path}/")
        };
        url.set_path(&normalized_path);
    }

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
    let client = builder
        .build()
        .map_err(|_| ServiceError::Config("cannot construct hardened HTTP client".to_owned()))?;
    Ok(PinnedEndpoint {
        url,
        client,
        bearer_token,
    })
}

async fn resolve_endpoint_addresses<F>(
    resolution: F,
    timeout: Duration,
    allow_private: bool,
) -> Result<Vec<SocketAddr>, ServiceError>
where
    F: Future<Output = io::Result<Vec<SocketAddr>>>,
{
    let mut addresses = time::timeout(timeout, resolution)
        .await
        .map_err(|_| ServiceError::Config("endpoint DNS resolution timed out".to_owned()))?
        .map_err(|_| ServiceError::Config("endpoint DNS resolution failed".to_owned()))?;
    addresses.sort_unstable();
    addresses.dedup();
    if addresses.is_empty() || addresses.len() > MAX_DNS_ADDRESSES {
        return Err(ServiceError::Config(format!(
            "endpoint DNS must resolve to 1..={MAX_DNS_ADDRESSES} addresses"
        )));
    }
    if !allow_private
        && addresses
            .iter()
            .any(|address| !is_publicly_routable(address.ip()))
    {
        return Err(ServiceError::Config(
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
    fn request(&self, method: Method, url: Url) -> Result<reqwest::RequestBuilder, ServiceError> {
        let mut request = self
            .client
            .request(method, url)
            .header(header::ACCEPT_ENCODING.as_str(), "identity");
        if let Some(token) = &self.bearer_token {
            request = request.bearer_auth(token.as_str()?);
        }
        Ok(request)
    }

    fn ipfs_url(&self, operation: &str, query: &[(&str, &str)]) -> Result<Url, ServiceError> {
        let mut url = self.url.join(operation).map_err(|_| {
            ServiceError::Network("cannot construct configured IPFS URL".to_owned())
        })?;
        {
            let mut pairs = url.query_pairs_mut();
            for (key, value) in query {
                pairs.append_pair(key, value);
            }
        }
        Ok(url)
    }
}

async fn read_bounded_response(
    mut response: reqwest::Response,
    max_bytes: u64,
) -> Result<Vec<u8>, ServiceError> {
    let headers = response.headers();
    if headers.len() > MAX_RESPONSE_HEADERS {
        return Err(ServiceError::Network(
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
        .ok_or_else(|| ServiceError::Network("remote header size overflow".to_owned()))?;
    if header_bytes > MAX_RESPONSE_HEADER_BYTES {
        return Err(ServiceError::Network(
            "remote response headers exceed the configured safety limit".to_owned(),
        ));
    }
    if let Some(encoding) = headers.get(header::CONTENT_ENCODING)
        && encoding.as_bytes() != b"identity"
    {
        return Err(ServiceError::Network(
            "compressed remote responses are forbidden".to_owned(),
        ));
    }
    let advertised_len = response.content_length();
    if advertised_len.is_some_and(|length| length > max_bytes) {
        return Err(ServiceError::Network(
            "remote response exceeds the configured body limit".to_owned(),
        ));
    }
    let capacity = usize::try_from(advertised_len.unwrap_or(0).min(max_bytes)).unwrap_or(0);
    let mut body = Vec::with_capacity(capacity);
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| ServiceError::Network("remote response body failed".to_owned()))?
    {
        let next_len = body
            .len()
            .checked_add(chunk.len())
            .ok_or_else(|| ServiceError::Network("remote response size overflow".to_owned()))?;
        if next_len as u64 > max_bytes {
            return Err(ServiceError::Network(
                "chunked remote response exceeds the configured body limit".to_owned(),
            ));
        }
        body.extend_from_slice(&chunk);
    }
    if advertised_len.is_some_and(|length| length != body.len() as u64) {
        return Err(ServiceError::Network(
            "remote response Content-Length does not match the body".to_owned(),
        ));
    }
    Ok(body)
}

fn validate_ipfs_cid(value: &str) -> Result<String, ServiceError> {
    if !is_canonical_cid_v1(value) {
        return Err(ServiceError::Network(
            "IPFS API returned a non-canonical CIDv1 base32 value".to_owned(),
        ));
    }
    Ok(value.to_owned())
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
    digest_offset
        .checked_add(digest_len)
        .is_some_and(|end| end == bytes.len())
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
) -> Result<String, ServiceError> {
    if bytes.is_empty() || bytes.len() as u64 > max_request_bytes {
        return Err(ServiceError::Network(
            "local IPFS object violates the configured request bound".to_owned(),
        ));
    }
    let url = endpoint.ipfs_url(
        "api/v0/add",
        &[
            ("pin", "false"),
            ("cid-version", "1"),
            ("raw-leaves", "true"),
            ("wrap-with-directory", "false"),
            ("quieter", "true"),
        ],
    )?;
    let part = reqwest::multipart::Part::bytes(bytes.to_vec())
        .file_name(name.to_owned())
        .mime_str("application/vnd.ipld.raw")
        .map_err(|_| ServiceError::Network("cannot construct IPFS multipart body".to_owned()))?;
    let response = endpoint
        .request(Method::POST, url)?
        .multipart(reqwest::multipart::Form::new().part("file", part))
        .send()
        .await
        .map_err(|_| ServiceError::Network("IPFS add request failed".to_owned()))?;
    if !response.status().is_success() {
        let status = response.status();
        let _ = read_bounded_response(response, max_response_bytes).await;
        return Err(ServiceError::Network(format!(
            "IPFS add returned HTTP {status}"
        )));
    }
    let body = read_bounded_response(response, max_response_bytes).await?;
    let value: JsonValue = json::from_slice(&body)
        .map_err(|_| ServiceError::Network("IPFS add returned malformed JSON".to_owned()))?;
    let cid = value
        .get("Hash")
        .and_then(JsonValue::as_str)
        .ok_or_else(|| ServiceError::Network("IPFS add response has no Hash".to_owned()))?;
    let cid = validate_ipfs_cid(cid)?;
    ipfs_pin(endpoint, &cid, max_response_bytes).await?;
    ipfs_verify_pin(endpoint, &cid, max_response_bytes).await?;
    let readback = ipfs_cat(endpoint, &cid, bytes.len() as u64, max_request_bytes).await?;
    if readback != bytes {
        return Err(ServiceError::Network(
            "IPFS readback bytes do not match the published object".to_owned(),
        ));
    }
    Ok(cid)
}

async fn ipfs_pin(
    endpoint: &PinnedEndpoint,
    cid: &str,
    max_response_bytes: u64,
) -> Result<(), ServiceError> {
    let url = endpoint.ipfs_url("api/v0/pin/add", &[("arg", cid), ("recursive", "true")])?;
    let response = endpoint
        .request(Method::POST, url)?
        .send()
        .await
        .map_err(|_| ServiceError::Network("IPFS pin request failed".to_owned()))?;
    if !response.status().is_success() {
        let status = response.status();
        let _ = read_bounded_response(response, max_response_bytes).await;
        return Err(ServiceError::Network(format!(
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
) -> Result<(), ServiceError> {
    let url = endpoint.ipfs_url("api/v0/pin/ls", &[("arg", cid), ("type", "recursive")])?;
    let response = endpoint
        .request(Method::POST, url)?
        .send()
        .await
        .map_err(|_| ServiceError::Network("IPFS pin verification failed".to_owned()))?;
    if !response.status().is_success() {
        let status = response.status();
        let _ = read_bounded_response(response, max_response_bytes).await;
        return Err(ServiceError::Network(format!(
            "IPFS pin verification returned HTTP {status}"
        )));
    }
    let body = read_bounded_response(response, max_response_bytes).await?;
    let value: JsonValue = json::from_slice(&body)
        .map_err(|_| ServiceError::Network("IPFS pin verification JSON is invalid".to_owned()))?;
    if value
        .get("Keys")
        .and_then(JsonValue::as_object)
        .is_none_or(|keys| !keys.contains_key(cid))
    {
        return Err(ServiceError::Network(
            "IPFS object is not recursively pinned".to_owned(),
        ));
    }
    Ok(())
}

async fn ipfs_cat(
    endpoint: &PinnedEndpoint,
    cid: &str,
    expected_max: u64,
    configured_max: u64,
) -> Result<Vec<u8>, ServiceError> {
    let url = endpoint.ipfs_url("api/v0/cat", &[("arg", cid)])?;
    let response = endpoint
        .request(Method::POST, url)?
        .send()
        .await
        .map_err(|_| ServiceError::Network("IPFS cat request failed".to_owned()))?;
    if !response.status().is_success() {
        let status = response.status();
        let _ = read_bounded_response(response, configured_max).await;
        return Err(ServiceError::Network(format!(
            "IPFS cat returned HTTP {status}"
        )));
    }
    read_bounded_response(response, expected_max.min(configured_max)).await
}

fn validate_remote_head(
    bytes: &[u8],
    source: &SourceSnapshot,
    config: &RuntimeConfig,
) -> Result<GovernanceDagHeadV1, ServiceError> {
    let head: GovernanceDagHeadV1 = decode_canonical(bytes, "public Governance DAG head")?;
    head.validate()
        .map_err(|err| ServiceError::Conflict(format!("public head is invalid: {err}")))?;
    if head.head_signature.algorithm != GovernanceSignatureAlgorithm::Ed25519
        || head.head_signature.public_key.as_slice() != config.expected_public_key
        || head.block_count == 0
        || head.block_count > source.blocks.len() as u64
    {
        return Err(ServiceError::Conflict(
            "public head key or block count is incompatible with the source chain".to_owned(),
        ));
    }
    let position = usize::try_from(head.block_count - 1)
        .map_err(|_| ServiceError::Conflict("public head count exceeds host limits".to_owned()))?;
    let block = &source.blocks[position].block;
    if block.block_cid != head.head_block_cid
        || block.publisher_peer_id != head.publisher_peer_id
        || head.generated_at
            > current_unix_timestamp_seconds().saturating_add(config.max_future_skew_secs)
    {
        return Err(ServiceError::Conflict(
            "public head is not a verified prefix of the local chain".to_owned(),
        ));
    }
    Ok(head)
}

async fn fetch_signed_http_head(
    endpoint: &PinnedEndpoint,
    max_response_bytes: u64,
) -> Result<PublicHead, ServiceError> {
    let response = endpoint
        .request(Method::GET, endpoint.url.clone())?
        .send()
        .await
        .map_err(|_| ServiceError::Network("signed-head GET failed".to_owned()))?;
    if response.status() == StatusCode::NOT_FOUND {
        let _ = read_bounded_response(response, max_response_bytes).await;
        return Ok(PublicHead::Missing);
    }
    if !response.status().is_success() {
        let status = response.status();
        let _ = read_bounded_response(response, max_response_bytes).await;
        return Err(ServiceError::Network(format!(
            "signed-head GET returned HTTP {status}"
        )));
    }
    let etag = response
        .headers()
        .get(header::ETAG)
        .and_then(|value| value.to_str().ok())
        .filter(|value| value.starts_with('"') && value.ends_with('"'))
        .filter(|value| value.len() <= MAX_PUBLIC_TOKEN_BYTES)
        .ok_or_else(|| ServiceError::Network("signed-head GET has no canonical ETag".to_owned()))?
        .to_owned();
    let bytes = read_bounded_response(response, max_response_bytes).await?;
    Ok(PublicHead::Present { bytes, token: etag })
}

async fn put_signed_http_head(
    endpoint: &PinnedEndpoint,
    bytes: &[u8],
    current: &PublicHead,
    allow_bootstrap: bool,
    max_response_bytes: u64,
) -> Result<PublicHead, ServiceError> {
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
            return Err(ServiceError::Conflict(
                "public signed head is missing and bootstrap is disabled".to_owned(),
            ));
        }
    }
    let response = request
        .send()
        .await
        .map_err(|_| ServiceError::Network("signed-head PUT failed".to_owned()))?;
    if matches!(
        response.status(),
        StatusCode::CONFLICT | StatusCode::PRECONDITION_FAILED
    ) {
        let _ = read_bounded_response(response, max_response_bytes).await;
        return Err(ServiceError::Conflict(
            "signed-head conditional update lost a concurrent-writer race".to_owned(),
        ));
    }
    if !response.status().is_success() {
        let status = response.status();
        let _ = read_bounded_response(response, max_response_bytes).await;
        return Err(ServiceError::Network(format!(
            "signed-head PUT returned HTTP {status}"
        )));
    }
    let _ = read_bounded_response(response, max_response_bytes).await?;
    let readback = fetch_signed_http_head(endpoint, max_response_bytes).await?;
    if !matches!(&readback, PublicHead::Present { bytes: observed, .. } if observed == bytes) {
        return Err(ServiceError::Conflict(
            "signed-head readback does not match the conditional update".to_owned(),
        ));
    }
    Ok(readback)
}

async fn resolve_ipns_head(
    ipfs: &PinnedEndpoint,
    name: &str,
    max_response_bytes: u64,
) -> Result<PublicHead, ServiceError> {
    let url = ipfs.ipfs_url(
        "api/v0/name/resolve",
        &[("arg", name), ("recursive", "true"), ("nocache", "true")],
    )?;
    let response = ipfs
        .request(Method::POST, url)?
        .send()
        .await
        .map_err(|_| ServiceError::Network("IPNS resolve failed".to_owned()))?;
    if !response.status().is_success() {
        let _ = read_bounded_response(response, max_response_bytes).await;
        return Ok(PublicHead::Missing);
    }
    let body = read_bounded_response(response, max_response_bytes).await?;
    let value: JsonValue = json::from_slice(&body)
        .map_err(|_| ServiceError::Network("IPNS resolve JSON is invalid".to_owned()))?;
    let path = value
        .get("Path")
        .and_then(JsonValue::as_str)
        .and_then(|value| value.strip_prefix("/ipfs/"))
        .ok_or_else(|| ServiceError::Network("IPNS resolve path is invalid".to_owned()))?;
    let cid = validate_ipfs_cid(path)?;
    let bytes = ipfs_cat(ipfs, &cid, max_response_bytes, max_response_bytes).await?;
    Ok(PublicHead::Present { bytes, token: cid })
}

async fn publish_ipns_head(
    ipfs: &PinnedEndpoint,
    name: &str,
    key_name: &str,
    head_cid: &str,
    bytes: &[u8],
    initial: &PublicHead,
    allow_bootstrap: bool,
    max_response_bytes: u64,
) -> Result<PublicHead, ServiceError> {
    let before = resolve_ipns_head(ipfs, name, max_response_bytes).await?;
    if public_head_identity(&before) != public_head_identity(initial) {
        return Err(ServiceError::Conflict(
            "IPNS name moved before publication".to_owned(),
        ));
    }
    if matches!(before, PublicHead::Missing) && !allow_bootstrap {
        return Err(ServiceError::Conflict(
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
    let response = ipfs
        .request(Method::POST, url)?
        .send()
        .await
        .map_err(|_| ServiceError::Network("IPNS publish failed".to_owned()))?;
    if !response.status().is_success() {
        let status = response.status();
        let _ = read_bounded_response(response, max_response_bytes).await;
        return Err(ServiceError::Network(format!(
            "IPNS publish returned HTTP {status}"
        )));
    }
    let _ = read_bounded_response(response, max_response_bytes).await?;
    let after = resolve_ipns_head(ipfs, name, max_response_bytes).await?;
    if !matches!(&after, PublicHead::Present { bytes: observed, token } if observed == bytes && token == head_cid)
    {
        return Err(ServiceError::Conflict(
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
    async fn fetch_public_head(&self) -> Result<PublicHead, ServiceError> {
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
    ) -> Result<PublicHead, ServiceError> {
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
                    name,
                    key_name,
                    head_cid,
                    bytes,
                    current,
                    self.config.allow_head_bootstrap,
                    self.config.max_response_bytes,
                )
                .await
            }
        }
    }

    async fn reconcile_once(&mut self) -> Result<(), ServiceError> {
        self.checkpoint = load_checkpoint(&self.config.state_dir, self.checkpoint_key.as_bytes())?;
        self.intent = load_publish_intent(&self.config.state_dir, self.checkpoint_key.as_bytes())?;
        let source = load_source_snapshot(&self.config)?;
        validate_checkpoint_against_source(self.checkpoint.as_ref(), &source)?;
        if let Some(intent) = &self.intent {
            validate_intent_against_source(
                intent,
                self.checkpoint.as_ref(),
                &source,
                &self.config,
            )?;
        }

        if let Some(checkpoint) = &self.checkpoint
            && checkpoint.head_block_cid == source.head.head_block_cid
            && self.intent.is_none()
        {
            self.verify_steady_state(&source, checkpoint).await?;
            self.publish_api_snapshot(&source, checkpoint, false)
                .await?;
            return Ok(());
        }

        if self.intent.is_none() {
            let current = self.fetch_public_head().await?;
            if let PublicHead::Present { bytes, .. } = &current {
                validate_remote_head(bytes, &source, &self.config)?;
            } else if !self.config.allow_head_bootstrap {
                return Err(ServiceError::Conflict(
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
            let start = self
                .checkpoint
                .as_ref()
                .map_or(0, |checkpoint| checkpoint.block_count as usize);
            let generation = self
                .checkpoint
                .as_ref()
                .map_or(1, |checkpoint| checkpoint.generation.saturating_add(1));
            let blocks = source.blocks[start..]
                .iter()
                .map(|block| IntentBlockV1 {
                    sequence: block.block.sequence,
                    governance_block_cid: block.block.block_cid.clone(),
                    governance_node_cid: block.block.node.node_cid.clone(),
                    payload_kind: block.payload_kind.clone(),
                    timestamp: block.block.timestamp,
                    encoded_blake3: block.encoded_blake3,
                    encoded_len: block.bytes.len() as u64,
                    ipfs_cid: None,
                })
                .collect::<Vec<_>>();
            if blocks.is_empty() {
                return Err(ServiceError::State(
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
                target_source_index_blake3: source.index_blake3,
                previous_public_head_blake3,
                created_at_unix: current_unix_timestamp_seconds(),
                blocks,
                head_ipfs_cid: None,
            };
            save_publish_intent(
                &self.config.state_dir,
                self.checkpoint_key.as_bytes(),
                &intent,
            )?;
            self.intent = Some(intent);
        }

        let mut intent = self.intent.take().ok_or_else(|| {
            ServiceError::State("durable publish intent disappeared before execution".to_owned())
        })?;
        if let Some(checkpoint) = &self.checkpoint
            && checkpoint.generation == intent.generation
            && checkpoint.head_block_cid == intent.target_head_block_cid
        {
            let current = self.fetch_public_head().await?;
            require_public_matches_checkpoint(&current, checkpoint)?;
            verify_or_recover_mirror_file(&self.config.state_dir, checkpoint, &source)?;
            remove_durable_file(&self.config.state_dir.join(PUBLISH_INTENT_FILE))?;
            self.intent = None;
            self.publish_api_snapshot(&source, checkpoint, false)
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
                ServiceError::State("intent sequence exceeds host limits".to_owned())
            })?;
            let source_block = source.blocks.get(sequence).ok_or_else(|| {
                ServiceError::State("intent block no longer exists in the source".to_owned())
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
            save_publish_intent(
                &self.config.state_dir,
                self.checkpoint_key.as_bytes(),
                &intent,
            )?;
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
            save_publish_intent(
                &self.config.state_dir,
                self.checkpoint_key.as_bytes(),
                &intent,
            )?;
        }
        let head_ipfs_cid = intent.head_ipfs_cid.clone().ok_or_else(|| {
            ServiceError::State("head IPFS CID is missing after verified publication".to_owned())
        })?;

        let current = self.fetch_public_head().await?;
        if let PublicHead::Present { bytes, .. } = &current {
            validate_remote_head(bytes, &source, &self.config)?;
        }
        let current_digest = public_head_digest(&current);
        let target_already_installed = current_digest == Some(intent.target_head_blake3);
        if !target_already_installed && current_digest != intent.previous_public_head_blake3 {
            self.intent = Some(intent);
            return Err(ServiceError::Conflict(
                "public head moved away from the durable publish intent".to_owned(),
            ));
        }
        let installed = if target_already_installed {
            current
        } else {
            self.install_public_head(&intent.target_head_bytes, &head_ipfs_cid, &current)
                .await?
        };
        let public_token = match &installed {
            PublicHead::Present { bytes, token }
                if blake3_array(bytes) == intent.target_head_blake3 =>
            {
                token.clone()
            }
            _ => {
                self.intent = Some(intent);
                return Err(ServiceError::Conflict(
                    "public head installation did not converge".to_owned(),
                ));
            }
        };

        let published_blocks = merge_published_blocks(
            self.checkpoint.as_ref(),
            &intent,
            &source,
            self.config.mirror_max_entries,
            self.config.mirror_max_bytes,
        )?;
        let published_at = current_unix_timestamp_seconds();
        let mirror = mirror_index_value(
            &source,
            &published_blocks,
            intent.generation,
            &head_ipfs_cid,
            &public_token,
            published_at,
        )?;
        let mirror_bytes = json::to_json_pretty(&mirror)
            .map_err(|err| ServiceError::State(format!("mirror JSON encode failed: {err}")))?
            .into_bytes();
        write_atomic_secret(
            &self.config.state_dir.join(MIRROR_INDEX_FILE),
            &mirror_bytes,
        )?;
        let checkpoint = CheckpointBodyV1 {
            version: CHECKPOINT_VERSION_V1,
            generation: intent.generation,
            head_block_cid: intent.target_head_block_cid.clone(),
            block_count: intent.target_block_count,
            head_bytes_blake3: intent.target_head_blake3,
            head_ipfs_cid,
            public_head_token: public_token,
            source_index_blake3: intent.target_source_index_blake3,
            mirror_blake3: blake3_array(&mirror_bytes),
            published_at_unix: published_at,
            mirror_blocks: published_blocks,
        };
        save_checkpoint(
            &self.config.state_dir,
            self.checkpoint_key.as_bytes(),
            &checkpoint,
        )?;
        remove_durable_file(&self.config.state_dir.join(PUBLISH_INTENT_FILE))?;
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

    async fn verify_steady_state(
        &self,
        source: &SourceSnapshot,
        checkpoint: &CheckpointBodyV1,
    ) -> Result<(), ServiceError> {
        let public = self.fetch_public_head().await?;
        require_public_matches_checkpoint(&public, checkpoint)?;
        if let PublicHead::Present { bytes, .. } = &public {
            validate_remote_head(bytes, source, &self.config)?;
        }
        ipfs_verify_pin(
            &self.ipfs,
            &checkpoint.head_ipfs_cid,
            self.config.max_response_bytes,
        )
        .await?;
        let public_bytes = match public {
            PublicHead::Present { bytes, .. } => bytes,
            PublicHead::Missing => {
                return Err(ServiceError::Conflict(
                    "public head disappeared while verifying the checkpoint".to_owned(),
                ));
            }
        };
        let readback = ipfs_cat(
            &self.ipfs,
            &checkpoint.head_ipfs_cid,
            public_bytes.len() as u64,
            self.config.max_response_bytes,
        )
        .await?;
        if readback != public_bytes {
            return Err(ServiceError::State(
                "checkpoint head IPFS readback drifted".to_owned(),
            ));
        }
        verify_or_recover_mirror_file(&self.config.state_dir, checkpoint, source)
    }

    async fn publish_api_snapshot(
        &self,
        source: &SourceSnapshot,
        checkpoint: &CheckpointBodyV1,
        just_published: bool,
    ) -> Result<(), ServiceError> {
        let bytes = read_regular_file(
            &self.config.state_dir.join(MIRROR_INDEX_FILE),
            MUTABLE_STATE_MAX_BYTES,
            true,
        )?;
        let mirror: JsonValue = json::from_slice(&bytes)
            .map_err(|err| ServiceError::State(format!("mirror JSON decode failed: {err}")))?;
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
        Ok(())
    }
}

fn validate_checkpoint_against_source(
    checkpoint: Option<&CheckpointBodyV1>,
    source: &SourceSnapshot,
) -> Result<(), ServiceError> {
    let Some(checkpoint) = checkpoint else {
        return Ok(());
    };
    if checkpoint.block_count > source.blocks.len() as u64 {
        return Err(ServiceError::Conflict(
            "source chain rolled back behind the authenticated checkpoint".to_owned(),
        ));
    }
    let position = usize::try_from(checkpoint.block_count - 1)
        .map_err(|_| ServiceError::State("checkpoint count exceeds host limits".to_owned()))?;
    if source.blocks[position].block.block_cid != checkpoint.head_block_cid {
        return Err(ServiceError::Conflict(
            "source chain forked from the authenticated checkpoint".to_owned(),
        ));
    }
    for published in &checkpoint.mirror_blocks {
        let position = usize::try_from(published.sequence)
            .map_err(|_| ServiceError::State("mirror sequence exceeds host limits".to_owned()))?;
        let source_block = source.blocks.get(position).ok_or_else(|| {
            ServiceError::Conflict("checkpoint mirror points outside the source chain".to_owned())
        })?;
        if source_block.block.block_cid != published.governance_block_cid
            || source_block.block.node.node_cid != published.governance_node_cid
            || source_block.payload_kind != published.payload_kind
            || source_block.encoded_blake3 != published.encoded_blake3
            || source_block.bytes.len() as u64 != published.encoded_len
        {
            return Err(ServiceError::Conflict(
                "checkpoint mirror no longer matches the verified source chain".to_owned(),
            ));
        }
    }
    Ok(())
}

fn validate_intent_against_source(
    intent: &PublishIntentBodyV1,
    checkpoint: Option<&CheckpointBodyV1>,
    source: &SourceSnapshot,
    config: &RuntimeConfig,
) -> Result<(), ServiceError> {
    validate_publish_intent(intent)?;
    if intent.target_block_count > source.blocks.len() as u64 {
        return Err(ServiceError::Conflict(
            "source rolled back behind the durable publish intent".to_owned(),
        ));
    }
    let target_position = usize::try_from(intent.target_block_count - 1)
        .map_err(|_| ServiceError::State("intent count exceeds host limits".to_owned()))?;
    if source.blocks[target_position].block.block_cid != intent.target_head_block_cid
        || blake3_array(&intent.target_head_bytes) != intent.target_head_blake3
    {
        return Err(ServiceError::Conflict(
            "source forked from the durable publish intent".to_owned(),
        ));
    }
    let target_head = validate_remote_head(&intent.target_head_bytes, source, config)?;
    if target_head.block_count != intent.target_block_count
        || target_head.head_block_cid != intent.target_head_block_cid
    {
        return Err(ServiceError::State(
            "durable intent head metadata is inconsistent".to_owned(),
        ));
    }
    let expected_generation = checkpoint.map_or(1, |checkpoint| {
        if checkpoint.head_block_cid == intent.target_head_block_cid {
            checkpoint.generation
        } else {
            checkpoint.generation.saturating_add(1)
        }
    });
    if intent.generation != expected_generation {
        return Err(ServiceError::State(
            "publish intent generation is not monotonic".to_owned(),
        ));
    }
    for block in &intent.blocks {
        let position = usize::try_from(block.sequence)
            .map_err(|_| ServiceError::State("intent sequence exceeds host limits".to_owned()))?;
        let source_block = source.blocks.get(position).ok_or_else(|| {
            ServiceError::Conflict("intent block is absent from the source".to_owned())
        })?;
        if source_block.block.block_cid != block.governance_block_cid
            || source_block.block.node.node_cid != block.governance_node_cid
            || source_block.payload_kind != block.payload_kind
            || source_block.encoded_blake3 != block.encoded_blake3
            || source_block.bytes.len() as u64 != block.encoded_len
        {
            return Err(ServiceError::Conflict(
                "durable intent block no longer matches source bytes".to_owned(),
            ));
        }
    }
    Ok(())
}

fn require_public_matches_checkpoint(
    public: &PublicHead,
    checkpoint: &CheckpointBodyV1,
) -> Result<(), ServiceError> {
    match public {
        PublicHead::Present { bytes, .. }
            if blake3_array(bytes) == checkpoint.head_bytes_blake3 =>
        {
            Ok(())
        }
        PublicHead::Missing => Err(ServiceError::Conflict(
            "public head disappeared after an authenticated checkpoint".to_owned(),
        )),
        PublicHead::Present { .. } => Err(ServiceError::Conflict(
            "public head diverges from the authenticated checkpoint".to_owned(),
        )),
    }
}

fn merge_published_blocks(
    checkpoint: Option<&CheckpointBodyV1>,
    intent: &PublishIntentBodyV1,
    source: &SourceSnapshot,
    max_entries: usize,
    max_bytes: u64,
) -> Result<Vec<PublishedBlockV1>, ServiceError> {
    let mut by_sequence = BTreeMap::<u64, PublishedBlockV1>::new();
    if let Some(checkpoint) = checkpoint {
        for block in &checkpoint.mirror_blocks {
            by_sequence.insert(block.sequence, block.clone());
        }
    }
    for block in &intent.blocks {
        let ipfs_cid = block.ipfs_cid.clone().ok_or_else(|| {
            ServiceError::State("intent block was not pinned before checkpointing".to_owned())
        })?;
        by_sequence.insert(
            block.sequence,
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
        );
    }
    if max_entries == 0 || max_bytes == 0 {
        return Err(ServiceError::State(
            "mirror retention bounds must be non-zero".to_owned(),
        ));
    }
    let mut retained_sequences = Vec::new();
    let mut retained_bytes = 0_u64;
    for source_block in source.blocks.iter().rev() {
        if retained_sequences.len() == max_entries {
            break;
        }
        let encoded_len = source_block.bytes.len() as u64;
        let next = retained_bytes
            .checked_add(encoded_len)
            .ok_or_else(|| ServiceError::State("mirror byte count overflow".to_owned()))?;
        if next > max_bytes {
            if retained_sequences.is_empty() {
                return Err(ServiceError::State(
                    "the head block alone exceeds mirror_max_bytes".to_owned(),
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
                ServiceError::State(
                    "retained source suffix has no authenticated IPFS mapping".to_owned(),
                )
            })
        })
        .collect()
}

fn mirror_index_value(
    source: &SourceSnapshot,
    blocks: &[PublishedBlockV1],
    generation: u64,
    head_ipfs_cid: &str,
    public_token: &str,
    published_at: u64,
) -> Result<JsonValue, ServiceError> {
    if blocks.is_empty() {
        return Err(ServiceError::State(
            "mirror index cannot be empty".to_owned(),
        ));
    }
    let mut block_values = Vec::with_capacity(blocks.len());
    let mut by_block_cid = JsonMap::new();
    let mut by_node_cid = JsonMap::new();
    let mut by_digest = JsonMap::new();
    let mut by_kind_positions = BTreeMap::<String, Vec<JsonValue>>::new();
    for (position, block) in blocks.iter().enumerate() {
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
        block_values.push(JsonValue::Object(value));
    }
    let by_kind = by_kind_positions
        .into_iter()
        .map(|(kind, positions)| (kind, JsonValue::Array(positions)))
        .collect::<JsonMap>();
    let mut head = JsonMap::new();
    head.insert(
        "head_block_cid_hex".into(),
        JsonValue::from(hex::encode(&source.head.head_block_cid)),
    );
    head.insert(
        "block_count".into(),
        JsonValue::from(source.head.block_count),
    );
    head.insert(
        "generated_at".into(),
        JsonValue::from(source.head.generated_at),
    );
    head.insert("ipfs_cid".into(), JsonValue::from(head_ipfs_cid));
    head.insert("public_token".into(), JsonValue::from(public_token));
    head.insert(
        "blake3".into(),
        JsonValue::from(hex::encode(blake3_array(&source.head_bytes))),
    );
    let mut root = JsonMap::new();
    root.insert("schema".into(), JsonValue::from(MIRROR_INDEX_SCHEMA));
    root.insert("generation".into(), JsonValue::from(generation));
    root.insert("generated_at".into(), JsonValue::from(published_at));
    root.insert("head".into(), JsonValue::Object(head));
    root.insert(
        "block_count".into(),
        JsonValue::from(source.head.block_count),
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

fn verify_mirror_file(state_dir: &Path, checkpoint: &CheckpointBodyV1) -> Result<(), ServiceError> {
    let bytes = read_regular_file(
        &state_dir.join(MIRROR_INDEX_FILE),
        MUTABLE_STATE_MAX_BYTES,
        true,
    )?;
    if blake3_array(&bytes) != checkpoint.mirror_blake3 {
        return Err(ServiceError::State(
            "mirror index digest does not match the authenticated checkpoint".to_owned(),
        ));
    }
    let value: JsonValue = json::from_slice(&bytes)
        .map_err(|err| ServiceError::State(format!("mirror index JSON is invalid: {err}")))?;
    let expected_head_cid = hex::encode(&checkpoint.head_block_cid);
    if value.get("schema").and_then(JsonValue::as_str) != Some(MIRROR_INDEX_SCHEMA)
        || value.get("generation").and_then(JsonValue::as_u64) != Some(checkpoint.generation)
        || value
            .get("head")
            .and_then(|head| head.get("head_block_cid_hex"))
            .and_then(JsonValue::as_str)
            != Some(expected_head_cid.as_str())
    {
        return Err(ServiceError::State(
            "mirror index metadata is inconsistent with the checkpoint".to_owned(),
        ));
    }
    Ok(())
}

fn verify_or_recover_mirror_file(
    state_dir: &Path,
    checkpoint: &CheckpointBodyV1,
    source: &SourceSnapshot,
) -> Result<(), ServiceError> {
    let path = state_dir.join(MIRROR_INDEX_FILE);
    match fs::symlink_metadata(&path) {
        Ok(_) => return verify_mirror_file(state_dir, checkpoint),
        Err(err) if err.kind() == io::ErrorKind::NotFound => {}
        Err(err) => {
            return Err(ServiceError::Filesystem(format!(
                "cannot inspect mirror index during recovery: {err}"
            )));
        }
    }
    if source.head.head_block_cid != checkpoint.head_block_cid
        || source.head.block_count != checkpoint.block_count
        || blake3_array(&source.head_bytes) != checkpoint.head_bytes_blake3
    {
        return Err(ServiceError::State(
            "missing mirror cannot be rebuilt from a source at a different head".to_owned(),
        ));
    }
    let mirror = mirror_index_value(
        source,
        &checkpoint.mirror_blocks,
        checkpoint.generation,
        &checkpoint.head_ipfs_cid,
        &checkpoint.public_head_token,
        checkpoint.published_at_unix,
    )?;
    let bytes = json::to_json_pretty(&mirror)
        .map_err(|err| ServiceError::State(format!("mirror recovery encode failed: {err}")))?
        .into_bytes();
    if blake3_array(&bytes) != checkpoint.mirror_blake3 {
        return Err(ServiceError::State(
            "deterministic mirror recovery does not match the checkpoint digest".to_owned(),
        ));
    }
    write_atomic_secret(&path, &bytes)?;
    verify_mirror_file(state_dir, checkpoint)
}

fn service_router(state: ApiState) -> Router {
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

async fn health_handler(State(state): State<ApiState>) -> Response {
    let snapshot = state.0.read().await;
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

async fn readiness_handler(State(state): State<ApiState>) -> Response {
    let snapshot = state.0.read().await;
    let mut value = JsonMap::new();
    value.insert(
        "schema".into(),
        JsonValue::from("sorafs.governance_dag.readiness.v1"),
    );
    value.insert("ready".into(), JsonValue::from(snapshot.ready));
    value.insert(
        "error".into(),
        snapshot
            .last_error
            .as_ref()
            .map_or(JsonValue::Null, |error| JsonValue::from(error.clone())),
    );
    json_response(
        if snapshot.ready {
            StatusCode::OK
        } else {
            StatusCode::SERVICE_UNAVAILABLE
        },
        JsonValue::Object(value),
        &HeaderMap::new(),
    )
}

async fn metrics_handler(State(state): State<ApiState>) -> Response {
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

async fn dashboard_handler(State(state): State<ApiState>, headers: HeaderMap) -> Response {
    let snapshot = state.0.read().await;
    let Some(mirror) = &snapshot.mirror else {
        return json_error(StatusCode::SERVICE_UNAVAILABLE, "mirror is not ready");
    };
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

async fn head_handler(State(state): State<ApiState>, headers: HeaderMap) -> Response {
    let snapshot = state.0.read().await;
    let Some(head) = snapshot
        .mirror
        .as_ref()
        .and_then(|mirror| mirror.get("head"))
        .cloned()
    else {
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
    State(state): State<ApiState>,
    headers: HeaderMap,
    AxumPath(cid): AxumPath<String>,
) -> Response {
    lookup_handler(state, headers, cid, "block_cid_hex", "block").await
}

async fn node_handler(
    State(state): State<ApiState>,
    headers: HeaderMap,
    AxumPath(cid): AxumPath<String>,
) -> Response {
    lookup_handler(state, headers, cid, "node_cid_hex", "node").await
}

async fn lookup_handler(
    state: ApiState,
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
    let snapshot = state.0.read().await;
    let block = snapshot
        .mirror
        .as_ref()
        .and_then(|mirror| mirror.get("blocks"))
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
    State(state): State<ApiState>,
    headers: HeaderMap,
    AxumPath(digest): AxumPath<String>,
) -> Response {
    if !is_canonical_digest_hex(&digest) {
        return json_error(
            StatusCode::BAD_REQUEST,
            "encoded digest must be lowercase 32-byte hex",
        );
    }
    let snapshot = state.0.read().await;
    let blocks = snapshot
        .mirror
        .as_ref()
        .and_then(|mirror| mirror.get("blocks"))
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

async fn checkpoint_handler(State(state): State<ApiState>, headers: HeaderMap) -> Response {
    let snapshot = state.0.read().await;
    let Some(checkpoint) = &snapshot.checkpoint else {
        return json_error(StatusCode::SERVICE_UNAVAILABLE, "checkpoint is not ready");
    };
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
    use std::{
        collections::{HashMap, VecDeque},
        process::{Child, Command, Stdio},
        sync::Arc,
    };

    use axum::{
        body::Bytes,
        extract::{RawQuery, State},
        http::{HeaderName, Request},
        response::Redirect,
        routing::post,
    };
    use iroha_crypto::{Algorithm, KeyPair, PrivateKey, Signature as IrohaSignature};
    use sorafs_manifest::{
        GOVERNANCE_DAG_BLOCK_VERSION_V1, GOVERNANCE_DAG_HEAD_VERSION_V1, GOVERNANCE_LOG_VERSION_V1,
        GovernanceLogNodeV1, GovernanceLogSignatureV1,
        deal::{
            DEAL_LEDGER_VERSION_V1, DEAL_SETTLEMENT_VERSION_V1, DealLedgerSnapshotV1,
            DealSettlementStatusV1, DealSettlementV1,
        },
        governance_dag_block_cid_v1,
    };
    use tempfile::TempDir;
    use tokio::{sync::Mutex, task::JoinHandle};
    use tower::ServiceExt as _;

    use super::*;

    const TEST_CID_PAYLOAD: &str = "bafkreibdt5m62vphg7dxcr6pkwwqygydbnwx5z2iu5bgsuxzxbjnlkjv4u";
    const TEST_CID_BLOCK: &str = "bafkreicjnlfibzgy6kp3r2gnqfwdv62i2pyqhfylhixocyambdfgomtn5y";
    const TEST_CID_HEAD: &str = "bafkreie7fzwthi3rp3ucmnj2ibf2iymndlxlnb4226jwxtuo2x2gqfesju";
    const TEST_CID_OLD: &str = "bafkreiglubvvonx26z7fjmd3kypk5fbzlz3uyul2pwiquvbwtyjghth32q";
    const TEST_CID_NEW: &str = "bafkreiarkb5a4l26nhk57jakmkq3263o4v7gxtmfyz6jxbbrwnx76ioeg4";
    const TEST_CID_ATTACKER: &str = "bafkreihgjoryus4vrrzlydkccfilursggzbcjbpnol5locdmo2i44qaizq";
    const KUBO_INTEGRATION_ENV: &str = "SORAFS_RUN_KUBO_INTEGRATION";
    const KUBO_BIN_ENV: &str = "SORAFS_KUBO_BIN";
    const KUBO_IPNS_KEY_ALIAS: &str = "sorafs-gdag-integration";

    struct KuboHarness {
        _root: TempDir,
        repo: PathBuf,
        binary: PathBuf,
        api_url: String,
        daemon_log: PathBuf,
        child: Option<Child>,
    }

    impl KuboHarness {
        async fn start() -> Self {
            assert_eq!(
                std::env::var(KUBO_INTEGRATION_ENV).as_deref(),
                Ok("1"),
                "set {KUBO_INTEGRATION_ENV}=1 to run the isolated Kubo integration lane"
            );
            let binary = std::env::var_os(KUBO_BIN_ENV)
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("ipfs"));
            let root = secure_temp_dir();
            let repo = root.path().join("ipfs-repo");
            fs::create_dir(&repo).expect("create isolated Kubo repository");
            #[cfg(unix)]
            fs::set_permissions(&repo, fs::Permissions::from_mode(0o700))
                .expect("secure isolated Kubo repository");

            Self::run_command(
                &binary,
                &repo,
                &[
                    "init",
                    "--empty-repo",
                    "--profile=test,autoconf-off,announce-off",
                ],
            );
            Self::run_command(
                &binary,
                &repo,
                &["config", "Addresses.API", "/ip4/127.0.0.1/tcp/0"],
            );
            Self::run_command(
                &binary,
                &repo,
                &["config", "Addresses.Gateway", "/ip4/127.0.0.1/tcp/0"],
            );
            Self::run_command(
                &binary,
                &repo,
                &[
                    "config",
                    "--json",
                    "Addresses.Swarm",
                    r#"["/ip4/127.0.0.1/tcp/0"]"#,
                ],
            );
            Self::run_command(
                &binary,
                &repo,
                &["config", "--bool", "Discovery.MDNS.Enabled", "false"],
            );
            Self::assert_network_isolation(&binary, &repo);

            let daemon_log = root.path().join("kubo-daemon.log");
            let stdout = File::create(&daemon_log).expect("create Kubo daemon log");
            let stderr = stdout.try_clone().expect("clone Kubo daemon log handle");
            let child = Command::new(&binary)
                .arg("daemon")
                .env("IPFS_PATH", &repo)
                .env("IPFS_TELEMETRY", "off")
                .stdin(Stdio::null())
                .stdout(Stdio::from(stdout))
                .stderr(Stdio::from(stderr))
                .spawn()
                .unwrap_or_else(|err| panic!("start isolated Kubo daemon: {err}"));
            let mut harness = Self {
                _root: root,
                repo,
                binary,
                api_url: String::new(),
                daemon_log,
                child: Some(child),
            };
            harness.api_url = harness.wait_for_api().await;
            harness.wait_until_ready().await;
            harness
        }

        fn run_command(binary: &Path, repo: &Path, args: &[&str]) -> Vec<u8> {
            let output = Command::new(binary)
                .args(args)
                .env("IPFS_PATH", repo)
                .env("IPFS_TELEMETRY", "off")
                .stdin(Stdio::null())
                .output()
                .unwrap_or_else(|err| panic!("run isolated Kubo command `{args:?}`: {err}"));
            assert!(
                output.status.success(),
                "isolated Kubo command `{args:?}` failed with {}\nstdout:\n{}\nstderr:\n{}",
                output.status,
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr),
            );
            output.stdout
        }

        fn assert_network_isolation(binary: &Path, repo: &Path) {
            let bytes = Self::run_command(binary, repo, &["config", "show"]);
            let config: JsonValue =
                json::from_slice(&bytes).expect("isolated Kubo config must be JSON");
            let null_or_empty = |value: Option<&JsonValue>| {
                value.is_none_or(|value| {
                    value.is_null() || value.as_array().is_some_and(Vec::is_empty)
                })
            };
            assert_eq!(
                config
                    .get("AutoConf")
                    .and_then(|value| value.get("Enabled"))
                    .and_then(JsonValue::as_bool),
                Some(false),
                "isolated Kubo must disable remote AutoConf"
            );
            assert!(
                null_or_empty(config.get("Bootstrap")),
                "isolated Kubo must have no bootstrap peers"
            );
            assert!(null_or_empty(
                config.get("DNS").and_then(|value| value.get("Resolvers"))
            ));
            assert!(null_or_empty(
                config
                    .get("Ipns")
                    .and_then(|value| value.get("DelegatedPublishers"))
            ));
            assert!(null_or_empty(
                config
                    .get("Routing")
                    .and_then(|value| value.get("DelegatedRouters"))
            ));
            assert_eq!(
                config
                    .get("Provide")
                    .and_then(|value| value.get("Enabled"))
                    .and_then(JsonValue::as_bool),
                Some(false),
                "isolated Kubo must disable content announcements"
            );
            let addresses = config
                .get("Addresses")
                .expect("isolated Kubo config has Addresses");
            for field in ["API", "Gateway"] {
                assert_eq!(
                    addresses.get(field).and_then(JsonValue::as_str),
                    Some("/ip4/127.0.0.1/tcp/0"),
                    "isolated Kubo {field} listener must be loopback-only"
                );
            }
            assert_eq!(
                addresses
                    .get("Swarm")
                    .and_then(JsonValue::as_array)
                    .and_then(|values| values.first())
                    .and_then(JsonValue::as_str),
                Some("/ip4/127.0.0.1/tcp/0"),
                "isolated Kubo swarm listener must be loopback-only"
            );
            assert_eq!(
                addresses
                    .get("Swarm")
                    .and_then(JsonValue::as_array)
                    .map(Vec::len),
                Some(1),
                "isolated Kubo must expose only one loopback swarm listener"
            );
        }

        async fn wait_for_api(&mut self) -> String {
            let api_path = self.repo.join("api");
            let deadline = time::Instant::now() + Duration::from_secs(20);
            loop {
                if let Ok(raw) = fs::read_to_string(&api_path) {
                    let raw = raw.trim();
                    let components = raw.split('/').collect::<Vec<_>>();
                    if components.len() == 5
                        && components[1] == "ip4"
                        && components[2] == "127.0.0.1"
                        && components[3] == "tcp"
                        && components[4].parse::<u16>().is_ok_and(|port| port != 0)
                    {
                        return format!("http://127.0.0.1:{}/", components[4]);
                    }
                    panic!("Kubo published a non-loopback or malformed API address: {raw}");
                }
                if let Some(status) = self
                    .child
                    .as_mut()
                    .expect("Kubo child exists while starting")
                    .try_wait()
                    .expect("inspect Kubo daemon status")
                {
                    panic!(
                        "isolated Kubo daemon exited early with {status}\n{}",
                        self.log_text()
                    );
                }
                assert!(
                    time::Instant::now() < deadline,
                    "timed out waiting for isolated Kubo API\n{}",
                    self.log_text()
                );
                time::sleep(Duration::from_millis(25)).await;
            }
        }

        async fn wait_until_ready(&self) {
            let endpoint = self.endpoint();
            let url = endpoint
                .ipfs_url("api/v0/version", &[])
                .expect("construct Kubo version URL");
            let deadline = time::Instant::now() + Duration::from_secs(20);
            loop {
                if let Ok(response) = endpoint
                    .request(Method::POST, url.clone())
                    .expect("construct Kubo readiness request")
                    .send()
                    .await
                    && response.status().is_success()
                {
                    let body = read_bounded_response(response, 64 * 1024)
                        .await
                        .expect("read Kubo version response");
                    let value: JsonValue =
                        json::from_slice(&body).expect("Kubo version response must be JSON");
                    let version = value
                        .get("Version")
                        .and_then(JsonValue::as_str)
                        .expect("Kubo version response has Version");
                    eprintln!("isolated Kubo {version} ready at {}", self.api_url);
                    return;
                }
                assert!(
                    time::Instant::now() < deadline,
                    "timed out waiting for isolated Kubo readiness\n{}",
                    self.log_text()
                );
                time::sleep(Duration::from_millis(25)).await;
            }
        }

        fn endpoint(&self) -> PinnedEndpoint {
            PinnedEndpoint {
                url: Url::parse(&self.api_url).expect("parse isolated Kubo API URL"),
                client: Client::builder()
                    .no_proxy()
                    .redirect(Policy::none())
                    .connect_timeout(Duration::from_secs(5))
                    .timeout(Duration::from_secs(20))
                    .build()
                    .expect("construct isolated Kubo HTTP client"),
                bearer_token: None,
            }
        }

        fn log_text(&self) -> String {
            fs::read_to_string(&self.daemon_log)
                .unwrap_or_else(|err| format!("cannot read Kubo daemon log: {err}"))
        }

        fn stop_child(&mut self) {
            let Some(mut child) = self.child.take() else {
                return;
            };
            let _ = Command::new(&self.binary)
                .arg("shutdown")
                .env("IPFS_PATH", &self.repo)
                .env("IPFS_TELEMETRY", "off")
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
            let deadline = std::time::Instant::now() + Duration::from_secs(10);
            loop {
                match child.try_wait() {
                    Ok(Some(_)) => return,
                    Ok(None) if std::time::Instant::now() < deadline => {
                        std::thread::sleep(Duration::from_millis(25));
                    }
                    Ok(None) | Err(_) => {
                        // This fallback can only target the exact child spawned above.
                        let _ = child.kill();
                        let _ = child.wait();
                        return;
                    }
                }
            }
        }

        fn shutdown(mut self) {
            self.stop_child();
        }
    }

    impl Drop for KuboHarness {
        fn drop(&mut self) {
            self.stop_child();
        }
    }

    struct TestSigner {
        private_key: PrivateKey,
        public_key: [u8; 32],
    }

    impl TestSigner {
        fn new(seed: u8) -> Self {
            let private_key = PrivateKey::from_bytes(Algorithm::Ed25519, &[seed; 32])
                .expect("test Ed25519 seed is valid");
            let keypair = KeyPair::from_private_key(private_key.clone())
                .expect("derive test Ed25519 keypair");
            let (algorithm, bytes) = keypair
                .public_key()
                .try_to_bytes()
                .expect("encode test public key");
            assert_eq!(algorithm, Algorithm::Ed25519);
            let mut public_key = [0_u8; 32];
            public_key.copy_from_slice(bytes);
            Self {
                private_key,
                public_key,
            }
        }

        fn sign(&self, payload: &[u8]) -> GovernanceLogSignatureV1 {
            let signature = IrohaSignature::try_new(&self.private_key, payload)
                .expect("sign test governance payload");
            GovernanceLogSignatureV1 {
                algorithm: GovernanceSignatureAlgorithm::Ed25519,
                public_key: self.public_key.to_vec(),
                signature: signature.payload().to_vec(),
            }
        }
    }

    fn empty_signature() -> GovernanceLogSignatureV1 {
        GovernanceLogSignatureV1 {
            algorithm: GovernanceSignatureAlgorithm::Ed25519,
            public_key: Vec::new(),
            signature: Vec::new(),
        }
    }

    fn settlement(sequence: u64, timestamp: u64) -> DealSettlementV1 {
        let mut deal_id = [0x11; 32];
        deal_id[..8].copy_from_slice(&sequence.saturating_add(1).to_le_bytes());
        let settled_at = timestamp.saturating_sub(1);
        let mut ledger = DealLedgerSnapshotV1 {
            version: DEAL_LEDGER_VERSION_V1,
            snapshot_id: [0; 32],
            sequence: 1,
            previous_snapshot_id: None,
            deal_id,
            terms_digest: [0x44; 32],
            provider_id: [0x22; 32],
            client_id: [0x33; 32],
            deal_start_epoch: settled_at.saturating_sub(2),
            deal_end_epoch: settled_at.saturating_sub(1),
            settlement_window_epochs: 2,
            window_start_epoch: settled_at.saturating_sub(2),
            window_end_epoch: settled_at,
            provider_accrual_nano: 10,
            client_liability_nano: 10,
            micropayment_credit_generated_nano: 0,
            micropayment_credit_applied_nano: 0,
            micropayment_credit_carry_nano: 0,
            client_debit_nano: 10,
            outstanding_liability_nano: 0,
            bond_total_nano: 20,
            bond_locked_nano: 0,
            bond_slashed_nano: 0,
            bond_released_nano: 20,
            window_expected_charge_nano: 10,
            window_micropayment_generated_nano: 0,
            window_micropayment_applied_nano: 0,
            window_client_debit_nano: 10,
            window_bond_slashed_nano: 0,
            window_bond_released_nano: 20,
            captured_at: settled_at,
        };
        ledger.snapshot_id = ledger.derive_snapshot_id().expect("ledger id");
        let mut settlement = DealSettlementV1 {
            version: DEAL_SETTLEMENT_VERSION_V1,
            settlement_id: [0; 32],
            deal_id,
            ledger,
            status: DealSettlementStatusV1::Completed,
            settled_at,
            audit_notes: None,
        };
        settlement.settlement_id = settlement.derive_settlement_id().expect("settlement id");
        settlement
    }

    fn signed_source(count: usize, seed: u8, first_timestamp: u64) -> SourceSnapshot {
        let signer = TestSigner::new(seed);
        let peer_id = b"12D3KooWGovernanceServiceTest".to_vec();
        let mut previous_node_cid = None;
        let mut previous_block_cid = None;
        let mut source_blocks = Vec::new();
        let mut decoded_blocks = Vec::new();
        for sequence in 0..count as u64 {
            let timestamp = first_timestamp.saturating_add(sequence);
            let mut node = GovernanceLogNodeV1 {
                version: GOVERNANCE_LOG_VERSION_V1,
                node_cid: Vec::new(),
                prev_cid: previous_node_cid.clone(),
                timestamp,
                publisher_peer_id: peer_id.clone(),
                payload: GovernanceLogPayloadV1::DealSettlement(settlement(sequence, timestamp)),
                publisher_signature: empty_signature(),
            };
            node.node_cid = node.recompute_node_cid().expect("derive test node CID");
            node.publisher_signature = signer.sign(
                &node
                    .signature_payload_bytes()
                    .expect("encode test node signing payload"),
            );
            let block_cid = governance_dag_block_cid_v1(
                previous_block_cid.as_deref(),
                sequence,
                timestamp,
                &peer_id,
                &node,
            )
            .expect("derive test block CID");
            let mut block = GovernanceDagBlockV1 {
                version: GOVERNANCE_DAG_BLOCK_VERSION_V1,
                block_cid,
                prev_block_cid: previous_block_cid.clone(),
                sequence,
                timestamp,
                publisher_peer_id: peer_id.clone(),
                node,
                block_signature: empty_signature(),
            };
            block.block_signature = signer.sign(
                &block
                    .signature_payload_bytes()
                    .expect("encode test block signing payload"),
            );
            block.validate().expect("test block is valid");
            let bytes = norito::to_bytes(&block).expect("encode test block");
            previous_node_cid = Some(block.node.node_cid.clone());
            previous_block_cid = Some(block.block_cid.clone());
            decoded_blocks.push(block.clone());
            source_blocks.push(SourceBlock {
                encoded_blake3: blake3_array(&bytes),
                payload_kind: "deal_settlement".to_owned(),
                block,
                bytes,
            });
        }
        let last = source_blocks.last().expect("test source is non-empty");
        let mut head = GovernanceDagHeadV1 {
            version: GOVERNANCE_DAG_HEAD_VERSION_V1,
            head_block_cid: last.block.block_cid.clone(),
            block_count: count as u64,
            generated_at: last.block.timestamp,
            publisher_peer_id: peer_id,
            checkpoint_cid: None,
            head_signature: empty_signature(),
        };
        head.head_signature = signer.sign(
            &head
                .signature_payload_bytes()
                .expect("encode test head signing payload"),
        );
        validate_governance_dag_head_against_chain_v1(&head, &decoded_blocks)
            .expect("test source chain is valid");
        let head_bytes = norito::to_bytes(&head).expect("encode test head");
        SourceSnapshot {
            index_blake3: [0x44; 32],
            head,
            head_bytes,
            blocks: source_blocks,
        }
    }

    fn test_runtime_config(source: &SourceSnapshot, root: &Path) -> RuntimeConfig {
        let mut expected_public_key = [0_u8; 32];
        expected_public_key.copy_from_slice(&source.head.head_signature.public_key);
        RuntimeConfig {
            source_dir: root.join("source"),
            state_dir: root.join("state"),
            listen_addr: "127.0.0.1:0".parse().expect("test address"),
            poll_interval: Duration::from_millis(10),
            max_response_bytes: 1024 * 1024,
            max_request_bytes: 1024 * 1024,
            mirror_max_entries: 1024,
            mirror_max_bytes: 1024 * 1024,
            max_head_age_secs: 3600,
            max_future_skew_secs: 60,
            allow_head_bootstrap: true,
            expected_public_key,
        }
    }

    fn checkpoint_from_source(source: &SourceSnapshot) -> CheckpointBodyV1 {
        let mirror_blocks = source
            .blocks
            .iter()
            .map(|block| PublishedBlockV1 {
                sequence: block.block.sequence,
                governance_block_cid: block.block.block_cid.clone(),
                governance_node_cid: block.block.node.node_cid.clone(),
                payload_kind: block.payload_kind.clone(),
                timestamp: block.block.timestamp,
                encoded_blake3: block.encoded_blake3,
                encoded_len: block.bytes.len() as u64,
                ipfs_cid: TEST_CID_BLOCK.to_owned(),
            })
            .collect();
        CheckpointBodyV1 {
            version: CHECKPOINT_VERSION_V1,
            generation: 1,
            head_block_cid: source.head.head_block_cid.clone(),
            block_count: source.head.block_count,
            head_bytes_blake3: blake3_array(&source.head_bytes),
            head_ipfs_cid: TEST_CID_HEAD.to_owned(),
            public_head_token: "public-token".to_owned(),
            source_index_blake3: source.index_blake3,
            mirror_blake3: [0x55; 32],
            published_at_unix: source.head.generated_at,
            mirror_blocks,
        }
    }

    fn intent_from_source(source: &SourceSnapshot) -> PublishIntentBodyV1 {
        PublishIntentBodyV1 {
            version: PUBLISH_INTENT_VERSION_V1,
            generation: 1,
            target_head_block_cid: source.head.head_block_cid.clone(),
            target_block_count: source.head.block_count,
            target_head_bytes: source.head_bytes.clone(),
            target_head_blake3: blake3_array(&source.head_bytes),
            target_source_index_blake3: source.index_blake3,
            previous_public_head_blake3: None,
            created_at_unix: source.head.generated_at,
            blocks: source
                .blocks
                .iter()
                .map(|block| IntentBlockV1 {
                    sequence: block.block.sequence,
                    governance_block_cid: block.block.block_cid.clone(),
                    governance_node_cid: block.block.node.node_cid.clone(),
                    payload_kind: block.payload_kind.clone(),
                    timestamp: block.block.timestamp,
                    encoded_blake3: block.encoded_blake3,
                    encoded_len: block.bytes.len() as u64,
                    ipfs_cid: Some(TEST_CID_BLOCK.to_owned()),
                })
                .collect(),
            head_ipfs_cid: Some(TEST_CID_HEAD.to_owned()),
        }
    }

    fn secure_temp_dir() -> TempDir {
        let dir = tempfile::tempdir().expect("create test directory");
        #[cfg(unix)]
        fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o700))
            .expect("secure test directory");
        dir
    }

    fn write_test_sidecar_file(path: &Path, bytes: &[u8]) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create source sidecar parent");
        }
        fs::write(path, bytes).expect("write source sidecar payload");
        fs::write(
            digest_sidecar_path(path),
            format!("{}\n", hex::encode(blake3_array(bytes))),
        )
        .expect("write source sidecar digest");
    }

    fn materialize_source_snapshot(root: &Path, source: &mut SourceSnapshot) {
        fs::create_dir_all(root).expect("create Governance DAG source root");
        let mut entries = Vec::with_capacity(source.blocks.len());
        let mut by_digest = JsonMap::new();
        let mut by_kind = BTreeMap::<String, Vec<JsonValue>>::new();
        for (position, block) in source.blocks.iter().enumerate() {
            let block_cid_hex = hex::encode(&block.block.block_cid);
            let block_path_label = format!(
                "runtime-dag/blocks/{:020}_{block_cid_hex}.to",
                block.block.sequence
            );
            write_test_sidecar_file(&root.join(&block_path_label), &block.bytes);

            let digest_hex = hex::encode(block.encoded_blake3);
            let mut entry = JsonMap::new();
            entry.insert("position".into(), JsonValue::from(position as u64));
            entry.insert("sequence".into(), JsonValue::from(block.block.sequence));
            entry.insert("block_path".into(), JsonValue::from(block_path_label));
            entry.insert(
                "encoded_len".into(),
                JsonValue::from(block.bytes.len() as u64),
            );
            entry.insert("block_cid_hex".into(), JsonValue::from(block_cid_hex));
            entry.insert(
                "node_cid_hex".into(),
                JsonValue::from(hex::encode(&block.block.node.node_cid)),
            );
            entry.insert(
                "prev_block_cid_hex".into(),
                block
                    .block
                    .prev_block_cid
                    .as_ref()
                    .map(hex::encode)
                    .map(JsonValue::from)
                    .unwrap_or(JsonValue::Null),
            );
            entry.insert(
                "prev_node_cid_hex".into(),
                block
                    .block
                    .node
                    .prev_cid
                    .as_ref()
                    .map(hex::encode)
                    .map(JsonValue::from)
                    .unwrap_or(JsonValue::Null),
            );
            entry.insert(
                "payload_kind".into(),
                JsonValue::from(block.payload_kind.clone()),
            );
            entry.insert("encoded_blake3".into(), JsonValue::from(digest_hex.clone()));
            entries.push(JsonValue::Object(entry));
            by_digest.insert(
                digest_hex,
                JsonValue::Array(vec![JsonValue::from(position as u64)]),
            );
            by_kind
                .entry(block.payload_kind.clone())
                .or_default()
                .push(JsonValue::from(position as u64));
        }
        write_test_sidecar_file(&root.join("runtime-dag/head.to"), &source.head_bytes);

        let mut index = JsonMap::new();
        index.insert("schema".into(), JsonValue::from(RUNTIME_INDEX_SCHEMA));
        index.insert(
            "publisher_public_key_hex".into(),
            JsonValue::from(hex::encode(&source.head.head_signature.public_key)),
        );
        index.insert(
            "publisher_peer_id_hex".into(),
            JsonValue::from(hex::encode(&source.head.publisher_peer_id)),
        );
        index.insert(
            "head_block_cid_hex".into(),
            JsonValue::from(hex::encode(&source.head.head_block_cid)),
        );
        index.insert(
            "head_generated_at".into(),
            JsonValue::from(source.head.generated_at),
        );
        index.insert("head_path".into(), JsonValue::from("runtime-dag/head.to"));
        index.insert(
            "block_count".into(),
            JsonValue::from(source.head.block_count),
        );
        index.insert("by_encoded_blake3".into(), JsonValue::Object(by_digest));
        index.insert(
            "by_payload_kind".into(),
            JsonValue::Object(
                by_kind
                    .into_iter()
                    .map(|(kind, positions)| (kind, JsonValue::Array(positions)))
                    .collect(),
            ),
        );
        index.insert("blocks".into(), JsonValue::Array(entries));
        let index_bytes = json::to_json_pretty(&JsonValue::Object(index))
            .expect("encode Governance DAG runtime index")
            .into_bytes();
        source.index_blake3 = blake3_array(&index_bytes);
        write_test_sidecar_file(&root.join("runtime-dag-index.json"), &index_bytes);
    }

    async fn kubo_key_generate(endpoint: &PinnedEndpoint, alias: &str) -> String {
        let url = endpoint
            .ipfs_url(
                "api/v0/key/gen",
                &[("arg", alias), ("type", "ed25519"), ("ipns-base", "base36")],
            )
            .expect("construct Kubo key generation URL");
        let response = endpoint
            .request(Method::POST, url)
            .expect("construct Kubo key generation request")
            .send()
            .await
            .expect("send Kubo key generation request");
        assert!(response.status().is_success(), "Kubo key generation failed");
        let body = read_bounded_response(response, 64 * 1024)
            .await
            .expect("read Kubo key generation response");
        let value: JsonValue = json::from_slice(&body).expect("Kubo key response must be JSON");
        let name = value
            .get("Name")
            .and_then(JsonValue::as_str)
            .expect("Kubo key response has Name");
        assert_eq!(name, alias);
        validate_public_token(
            value
                .get("Id")
                .and_then(JsonValue::as_str)
                .expect("Kubo key response has Id"),
            "Kubo IPNS key id",
        )
        .expect("Kubo returns a canonical IPNS key id")
    }

    async fn kubo_unpin(endpoint: &PinnedEndpoint, cid: &str) {
        let url = endpoint
            .ipfs_url("api/v0/pin/rm", &[("arg", cid), ("recursive", "true")])
            .expect("construct Kubo unpin URL");
        let response = endpoint
            .request(Method::POST, url)
            .expect("construct Kubo unpin request")
            .send()
            .await
            .expect("send Kubo unpin request");
        assert!(response.status().is_success(), "Kubo unpin failed");
        let _ = read_bounded_response(response, 64 * 1024)
            .await
            .expect("read Kubo unpin response");
    }

    async fn assert_kubo_has_no_swarm_peers(endpoint: &PinnedEndpoint) {
        let url = endpoint
            .ipfs_url("api/v0/swarm/peers", &[])
            .expect("construct Kubo swarm peers URL");
        let response = endpoint
            .request(Method::POST, url)
            .expect("construct Kubo swarm peers request")
            .send()
            .await
            .expect("send Kubo swarm peers request");
        assert!(response.status().is_success());
        let body = read_bounded_response(response, 64 * 1024)
            .await
            .expect("read Kubo swarm peers response");
        let value: JsonValue = json::from_slice(&body).expect("Kubo swarm response must be JSON");
        assert!(
            value
                .get("Peers")
                .is_none_or(|peers| peers.is_null() || peers.as_array().is_some_and(Vec::is_empty)),
            "isolated Kubo must have no swarm peers: {value:?}"
        );
    }

    fn real_kubo_service_view(
        source: &SourceSnapshot,
        source_dir: &Path,
        state_dir: &Path,
        checkpoint_key_path: &Path,
        api_url: &str,
        ipns_name: &str,
    ) -> SorafsGovernanceDagServiceView {
        let paths = [source_dir, state_dir, checkpoint_key_path];
        assert!(paths.iter().all(|path| {
            let path = path.to_string_lossy();
            !path.contains(['"', '\\', '\n', '\r'])
        }));
        let config = format!(
            r#"[sorafs.storage]
governance_dag_dir = "{}"

[sorafs.storage.governance_dag_service]
enabled = true
state_dir = "{}"
ipfs_api_url = "{}"
head_mode = "ipns"
ipns_name = "{}"
ipns_key_name = "{}"
checkpoint_key_path = "{}"
publisher_public_key_hex = "{}"
poll_interval_secs = 1
connect_timeout_ms = 5000
request_timeout_ms = 20000
dns_timeout_ms = 5000
max_head_age_secs = 3600
max_future_skew_secs = 60
allow_insecure_http = true
allow_private_ipfs_endpoint = true
allow_head_bootstrap = true
listen_addr = "127.0.0.1:0"
"#,
            source_dir.display(),
            state_dir.display(),
            api_url,
            ipns_name,
            KUBO_IPNS_KEY_ALIAS,
            checkpoint_key_path.display(),
            hex::encode(&source.head.head_signature.public_key),
        );
        let config_path = state_dir
            .parent()
            .expect("integration state directory has parent")
            .join("governance-dag-service.toml");
        fs::write(&config_path, config).expect("write standalone G-DAG service config");
        load_service_config(&config_path).expect("parse standalone G-DAG service config")
    }

    async fn spawn_router(router: Router, path: &str) -> (PinnedEndpoint, JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind mock service");
        let address = listener.local_addr().expect("mock listener address");
        let handle = tokio::spawn(async move {
            let _ = axum::serve(listener, router.into_make_service()).await;
        });
        let url = Url::parse(&format!("http://{address}{path}")).expect("mock URL");
        let client = Client::builder()
            .no_proxy()
            .redirect(Policy::none())
            .build()
            .expect("mock HTTP client");
        (
            PinnedEndpoint {
                url,
                client,
                bearer_token: None,
            },
            handle,
        )
    }

    fn test_response(status: StatusCode, body: impl Into<Body>) -> Response {
        let mut response = Response::new(body.into());
        *response.status_mut() = status;
        response
    }

    #[derive(Clone)]
    struct MockIpfsState {
        add_body: Arc<Vec<u8>>,
        cat_body: Arc<Vec<u8>>,
        pin_present: bool,
    }

    async fn mock_ipfs_add(State(state): State<MockIpfsState>) -> Response {
        test_response(StatusCode::OK, state.add_body.as_ref().clone())
    }

    async fn mock_ipfs_pin_add() -> Response {
        test_response(StatusCode::OK, "{}")
    }

    async fn mock_ipfs_pin_ls(State(state): State<MockIpfsState>) -> Response {
        let body = if state.pin_present {
            format!(r#"{{"Keys":{{"{TEST_CID_PAYLOAD}":{{}}}}}}"#)
        } else {
            r#"{"Keys":{}}"#.to_owned()
        };
        test_response(StatusCode::OK, body)
    }

    async fn mock_ipfs_cat(State(state): State<MockIpfsState>) -> Response {
        test_response(StatusCode::OK, state.cat_body.as_ref().clone())
    }

    fn mock_ipfs_router(state: MockIpfsState) -> Router {
        Router::new()
            .route("/api/v0/add", post(mock_ipfs_add))
            .route("/api/v0/pin/add", post(mock_ipfs_pin_add))
            .route("/api/v0/pin/ls", post(mock_ipfs_pin_ls))
            .route("/api/v0/cat", post(mock_ipfs_cat))
            .with_state(state)
    }

    #[derive(Default)]
    struct SignedHeadInner {
        bytes: Option<Vec<u8>>,
        etag: String,
        put_status: Option<StatusCode>,
        readback_override: Option<Vec<u8>>,
        put_count: u64,
    }

    #[derive(Clone)]
    struct SignedHeadState(Arc<Mutex<SignedHeadInner>>);

    async fn mock_signed_head_get(State(state): State<SignedHeadState>) -> Response {
        let state = state.0.lock().await;
        let Some(bytes) = &state.bytes else {
            return test_response(StatusCode::NOT_FOUND, Body::empty());
        };
        let mut response = test_response(StatusCode::OK, bytes.clone());
        response.headers_mut().insert(
            header::ETAG,
            HeaderValue::from_str(&state.etag).expect("mock ETag"),
        );
        response
    }

    async fn mock_signed_head_put(
        State(state): State<SignedHeadState>,
        _headers: HeaderMap,
        body: Bytes,
    ) -> Response {
        let mut state = state.0.lock().await;
        state.put_count = state.put_count.saturating_add(1);
        if let Some(status) = state.put_status {
            return test_response(status, Body::empty());
        }
        state.bytes = Some(
            state
                .readback_override
                .clone()
                .unwrap_or_else(|| body.to_vec()),
        );
        state.etag = "\"v2\"".to_owned();
        test_response(StatusCode::NO_CONTENT, Body::empty())
    }

    async fn spawn_signed_head(
        inner: SignedHeadInner,
    ) -> (PinnedEndpoint, SignedHeadState, JoinHandle<()>) {
        let state = SignedHeadState(Arc::new(Mutex::new(inner)));
        let router = Router::new()
            .route("/head", get(mock_signed_head_get).put(mock_signed_head_put))
            .with_state(state.clone());
        let (endpoint, handle) = spawn_router(router, "/head").await;
        (endpoint, state, handle)
    }

    #[derive(Clone)]
    struct IpnsMockState {
        resolutions: Arc<Mutex<VecDeque<String>>>,
        bodies: Arc<HashMap<String, Vec<u8>>>,
        publish_count: Arc<AtomicU64>,
    }

    fn raw_query_arg(raw: Option<&str>) -> Option<&str> {
        raw?.split('&').find_map(|pair| {
            let (key, value) = pair.split_once('=')?;
            (key == "arg").then_some(value)
        })
    }

    async fn mock_ipns_resolve(
        State(state): State<IpnsMockState>,
        RawQuery(_raw): RawQuery,
    ) -> Response {
        let cid = state.resolutions.lock().await.pop_front();
        match cid {
            Some(cid) => test_response(StatusCode::OK, format!(r#"{{"Path":"/ipfs/{cid}"}}"#)),
            None => test_response(StatusCode::NOT_FOUND, "{}"),
        }
    }

    async fn mock_ipns_publish(State(state): State<IpnsMockState>) -> Response {
        state.publish_count.fetch_add(1, Ordering::SeqCst);
        test_response(StatusCode::OK, "{}")
    }

    async fn mock_ipns_cat(
        State(state): State<IpnsMockState>,
        RawQuery(raw): RawQuery,
    ) -> Response {
        let Some(cid) = raw_query_arg(raw.as_deref()) else {
            return test_response(StatusCode::BAD_REQUEST, Body::empty());
        };
        match state.bodies.get(cid) {
            Some(bytes) => test_response(StatusCode::OK, bytes.clone()),
            None => test_response(StatusCode::NOT_FOUND, Body::empty()),
        }
    }

    fn mock_ipns_router(state: IpnsMockState) -> Router {
        Router::new()
            .route("/api/v0/name/resolve", post(mock_ipns_resolve))
            .route("/api/v0/name/publish", post(mock_ipns_publish))
            .route("/api/v0/cat", post(mock_ipns_cat))
            .with_state(state)
    }

    async fn response_header_bomb() -> Response {
        let mut response = test_response(StatusCode::OK, "ok");
        for index in 0..=MAX_RESPONSE_HEADERS {
            let name = HeaderName::from_bytes(format!("x-test-{index}").as_bytes())
                .expect("mock header name");
            response
                .headers_mut()
                .insert(name, HeaderValue::from_static("value"));
        }
        response
    }

    async fn response_body_bomb() -> Response {
        test_response(StatusCode::OK, vec![0_u8; 17])
    }

    async fn response_gzip() -> Response {
        let mut response = test_response(StatusCode::OK, "abc");
        response
            .headers_mut()
            .insert(header::CONTENT_ENCODING, HeaderValue::from_static("gzip"));
        response
    }

    #[test]
    fn canonical_decode_rejects_trailing_and_compressed_bytes() {
        let source = signed_source(1, 0x31, 1_800_000_000);
        let block = &source.blocks[0];
        let decoded_block: GovernanceDagBlockV1 =
            decode_canonical(&block.bytes, "governance DAG block")
                .expect("a valid signed governance block fits the bounded decoder budget");
        assert_eq!(decoded_block, block.block);
        let checkpoint = checkpoint_from_source(&source);
        let canonical = norito::to_bytes(&checkpoint).expect("encode checkpoint body");
        let decoded: CheckpointBodyV1 =
            decode_canonical(&canonical, "checkpoint").expect("canonical bytes accepted");
        assert_eq!(decoded, checkpoint);

        let mut trailing = canonical.clone();
        trailing.push(0);
        assert!(decode_canonical::<CheckpointBodyV1>(&trailing, "checkpoint").is_err());

        let compressed =
            norito::to_compressed_bytes(&checkpoint, Some(norito::CompressionConfig::default()))
                .expect("compress checkpoint body");
        assert_ne!(compressed, canonical);
        assert!(decode_canonical::<CheckpointBodyV1>(&compressed, "checkpoint").is_err());
    }

    #[test]
    fn bounded_norito_decode_rejects_sequence_allocation_bomb() {
        let encoded = norito::to_bytes(&vec![7_u64; 64]).expect("encode bounded vector");
        let limits = DecodeLimits::new(4, encoded.len(), 8, encoded.len() * 2, 16);
        assert!(norito::decode_from_bytes_with_limits::<Vec<u64>>(&encoded, limits).is_err());
    }

    #[test]
    fn expected_signer_rejects_wrong_key_and_peer() {
        let source = signed_source(1, 0x32, 1_800_000_000);
        let block = &source.blocks[0].block;
        let attacker = TestSigner::new(0x33);
        assert!(
            validate_expected_signer(block, &attacker.public_key, &block.publisher_peer_id,)
                .is_err()
        );
        let mut expected_key = [0_u8; 32];
        expected_key.copy_from_slice(&block.block_signature.public_key);
        assert!(validate_expected_signer(block, &expected_key, b"wrong-peer").is_err());
    }

    #[test]
    fn checkpoint_rejects_rollback_and_fork() {
        let original = signed_source(3, 0x34, 1_800_000_000);
        let checkpoint = checkpoint_from_source(&original);
        let rolled_back = signed_source(2, 0x34, 1_800_000_000);
        assert!(validate_checkpoint_against_source(Some(&checkpoint), &rolled_back).is_err());

        let fork = signed_source(3, 0x34, 1_800_000_100);
        assert!(validate_checkpoint_against_source(Some(&checkpoint), &fork).is_err());
    }

    #[test]
    fn manifest_chain_rejects_sequence_gap_and_timestamp_regression() {
        let signer = TestSigner::new(0x35);
        let source = signed_source(2, 0x35, 1_800_000_000);
        let mut sequence_blocks = source
            .blocks
            .iter()
            .map(|block| block.block.clone())
            .collect::<Vec<_>>();
        sequence_blocks[1].sequence = 7;
        sequence_blocks[1].block_cid = sequence_blocks[1]
            .recompute_block_cid()
            .expect("recompute sequence-gap CID");
        sequence_blocks[1].block_signature = signer.sign(
            &sequence_blocks[1]
                .signature_payload_bytes()
                .expect("encode sequence-gap block"),
        );
        let mut sequence_head = source.head.clone();
        sequence_head.head_block_cid = sequence_blocks[1].block_cid.clone();
        sequence_head.head_signature = signer.sign(
            &sequence_head
                .signature_payload_bytes()
                .expect("encode sequence-gap head"),
        );
        assert!(
            validate_governance_dag_head_against_chain_v1(&sequence_head, &sequence_blocks)
                .is_err()
        );

        let mut time_blocks = source
            .blocks
            .iter()
            .map(|block| block.block.clone())
            .collect::<Vec<_>>();
        time_blocks[1].timestamp = time_blocks[0].timestamp.saturating_sub(1);
        time_blocks[1].block_cid = time_blocks[1]
            .recompute_block_cid()
            .expect("recompute regressed CID");
        time_blocks[1].block_signature = signer.sign(
            &time_blocks[1]
                .signature_payload_bytes()
                .expect("encode regressed block"),
        );
        let mut time_head = source.head.clone();
        time_head.head_block_cid = time_blocks[1].block_cid.clone();
        time_head.head_signature = signer.sign(
            &time_head
                .signature_payload_bytes()
                .expect("encode regressed head"),
        );
        assert!(validate_governance_dag_head_against_chain_v1(&time_head, &time_blocks).is_err());
    }

    #[test]
    fn bounded_file_read_rejects_oversize() {
        let dir = secure_temp_dir();
        let path = dir.path().join("oversize.bin");
        fs::write(&path, [0_u8; 9]).expect("write oversized file");
        assert!(read_regular_file(&path, 8, false).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn bounded_file_read_rejects_symlink_hardlink_and_permissive_secret() {
        use std::os::unix::fs::symlink;

        let dir = secure_temp_dir();
        let target = dir.path().join("target.bin");
        fs::write(&target, [0x11; 32]).expect("write target");
        fs::set_permissions(&target, fs::Permissions::from_mode(0o600)).expect("secure target");

        let symlink_path = dir.path().join("symlink.bin");
        symlink(&target, &symlink_path).expect("create symlink");
        assert!(read_regular_file(&symlink_path, 32, true).is_err());

        let hardlink_path = dir.path().join("hardlink.bin");
        fs::hard_link(&target, &hardlink_path).expect("create hard link");
        assert!(read_regular_file(&target, 32, true).is_err());
        fs::remove_file(&hardlink_path).expect("remove hard link");

        fs::set_permissions(&target, fs::Permissions::from_mode(0o644))
            .expect("make secret permissive");
        assert!(read_regular_file(&target, 32, true).is_err());
    }

    #[test]
    fn authenticated_checkpoint_rejects_tamper_and_wrong_key() {
        let dir = secure_temp_dir();
        let source = signed_source(1, 0x36, 1_800_000_000);
        let checkpoint = checkpoint_from_source(&source);
        let key = [0x41; 32];
        save_checkpoint(dir.path(), &key, &checkpoint).expect("save checkpoint");
        assert_eq!(
            load_checkpoint(dir.path(), &key).expect("load checkpoint"),
            Some(checkpoint.clone())
        );
        assert!(load_checkpoint(dir.path(), &[0x42; 32]).is_err());

        let path = dir.path().join(CHECKPOINT_FILE);
        let mut bytes = fs::read(&path).expect("read checkpoint envelope");
        let last = bytes.last_mut().expect("checkpoint is non-empty");
        *last ^= 0x80;
        fs::write(&path, bytes).expect("tamper checkpoint");
        assert!(load_checkpoint(dir.path(), &key).is_err());
    }

    #[test]
    fn authenticated_intent_rejects_tamper() {
        let dir = secure_temp_dir();
        let source = signed_source(1, 0x37, 1_800_000_000);
        let intent = intent_from_source(&source);
        let key = [0x51; 32];
        save_publish_intent(dir.path(), &key, &intent).expect("save intent");
        assert_eq!(
            load_publish_intent(dir.path(), &key).expect("load intent"),
            Some(intent)
        );
        let path = dir.path().join(PUBLISH_INTENT_FILE);
        let mut bytes = fs::read(&path).expect("read intent envelope");
        bytes.truncate(bytes.len().saturating_sub(1));
        fs::write(&path, bytes).expect("truncate intent");
        assert!(load_publish_intent(dir.path(), &key).is_err());
    }

    #[test]
    fn mirror_retention_honours_entry_and_byte_caps() {
        let source = signed_source(3, 0x38, 1_800_000_000);
        let intent = intent_from_source(&source);
        let latest = source.blocks[2].bytes.len() as u64;
        let previous = source.blocks[1].bytes.len() as u64;
        let exact_two = latest + previous;
        let retained = merge_published_blocks(None, &intent, &source, 2, exact_two)
            .expect("retain exact two-block suffix");
        assert_eq!(retained.len(), 2);
        assert_eq!(retained[0].sequence, 1);
        assert_eq!(retained[1].sequence, 2);

        let one = merge_published_blocks(None, &intent, &source, 1, exact_two)
            .expect("entry cap retains one block");
        assert_eq!(one.len(), 1);
        assert_eq!(one[0].sequence, 2);

        let byte_limited = merge_published_blocks(None, &intent, &source, 3, exact_two - 1)
            .expect("byte cap retains the newest fitting suffix");
        assert_eq!(byte_limited.len(), 1);
        assert!(merge_published_blocks(None, &intent, &source, 3, latest - 1).is_err());
    }

    #[test]
    fn canonical_lookup_ids_reject_uppercase_short_and_non_hex() {
        assert!(is_canonical_digest_hex(&"ab".repeat(32)));
        assert!(!is_canonical_digest_hex(&"AB".repeat(32)));
        assert!(!is_canonical_digest_hex("ab"));
        assert!(!is_canonical_digest_hex(&"gg".repeat(32)));
    }

    #[test]
    fn json_response_etag_supports_exact_not_modified() {
        let value = JsonValue::from("stable");
        let first = json_response(StatusCode::OK, value.clone(), &HeaderMap::new());
        assert_eq!(first.status(), StatusCode::OK);
        let etag = first
            .headers()
            .get(header::ETAG)
            .expect("response has ETag")
            .clone();
        let mut request_headers = HeaderMap::new();
        request_headers.insert(header::IF_NONE_MATCH, etag.clone());
        let second = json_response(StatusCode::OK, value, &request_headers);
        assert_eq!(second.status(), StatusCode::NOT_MODIFIED);
        assert_eq!(second.headers().get(header::ETAG), Some(&etag));
    }

    #[tokio::test]
    async fn routes_reject_noncanonical_identifiers_before_lookup() {
        let state = ApiState(Arc::new(RwLock::new(ApiSnapshot {
            live: true,
            ready: true,
            ..ApiSnapshot::default()
        })));
        let app = service_router(state);
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/v1/sorafs/governance/dag/blocks/ABCD")
                    .body(Body::empty())
                    .expect("build request"),
            )
            .await
            .expect("route response");
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/v1/sorafs/governance/dag/digests/gggg")
                    .body(Body::empty())
                    .expect("build request"),
            )
            .await
            .expect("route response");
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn private_ipfs_permission_does_not_authorize_private_head_endpoint() {
        let config = SorafsGovernanceDagService {
            allow_insecure_http: true,
            allow_private_ipfs_endpoint: true,
            allow_private_head_endpoint: false,
            ..SorafsGovernanceDagService::default()
        };
        let ipfs = build_pinned_endpoint("http://127.0.0.1:5001", None, &config, true).await;
        assert!(ipfs.is_ok());
        let head = build_pinned_endpoint("http://127.0.0.1:9099/head", None, &config, false).await;
        assert!(head.is_err());
    }

    #[tokio::test]
    async fn dns_policy_rejects_mixed_mapped_overcap_and_timeout_answers() {
        let public = "8.8.8.8:443".parse().expect("public address");
        let private = "127.0.0.1:443".parse().expect("private address");
        assert!(
            resolve_endpoint_addresses(
                std::future::ready(Ok(vec![public, private])),
                Duration::from_secs(1),
                false,
            )
            .await
            .is_err()
        );

        let mapped = SocketAddr::new(
            IpAddr::V6("::ffff:127.0.0.1".parse().expect("mapped IPv6")),
            443,
        );
        assert!(
            resolve_endpoint_addresses(
                std::future::ready(Ok(vec![mapped])),
                Duration::from_secs(1),
                false,
            )
            .await
            .is_err()
        );

        let over_cap = (1..=(MAX_DNS_ADDRESSES + 1))
            .map(|last| SocketAddr::new(IpAddr::V4(Ipv4Addr::new(8, 8, 4, last as u8)), 443))
            .collect::<Vec<_>>();
        assert!(
            resolve_endpoint_addresses(
                std::future::ready(Ok(over_cap)),
                Duration::from_secs(1),
                false,
            )
            .await
            .is_err()
        );

        let delayed = async {
            time::sleep(Duration::from_millis(50)).await;
            Ok(vec![public])
        };
        assert!(
            resolve_endpoint_addresses(delayed, Duration::from_millis(1), false)
                .await
                .is_err()
        );

        let calls = Arc::new(AtomicU64::new(0));
        let calls_for_resolution = calls.clone();
        let resolved = resolve_endpoint_addresses(
            async move {
                calls_for_resolution.fetch_add(1, Ordering::SeqCst);
                Ok(vec![public, public])
            },
            Duration::from_secs(1),
            false,
        )
        .await
        .expect("one pinned public DNS snapshot");
        assert_eq!(resolved, vec![public]);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn ipfs_urls_cids_and_secret_debug_output_are_canonical() {
        let endpoint = PinnedEndpoint {
            url: Url::parse("http://127.0.0.1:5001/").expect("test URL"),
            client: Client::builder().no_proxy().build().expect("test client"),
            bearer_token: Some(SecretBytes(b"never-log-this-token".to_vec())),
        };
        let url = endpoint
            .ipfs_url(
                "api/v0/cat",
                &[("arg", TEST_CID_PAYLOAD), ("progress", "false")],
            )
            .expect("canonical IPFS URL");
        let pairs = url.query_pairs().collect::<Vec<_>>();
        assert_eq!(pairs.len(), 2, "query fields must not be duplicated");
        assert_eq!(pairs[0], ("arg".into(), TEST_CID_PAYLOAD.into()));
        assert_eq!(pairs[1], ("progress".into(), "false".into()));

        for cid in [
            TEST_CID_PAYLOAD,
            TEST_CID_BLOCK,
            TEST_CID_HEAD,
            TEST_CID_OLD,
            TEST_CID_NEW,
            TEST_CID_ATTACKER,
        ] {
            assert!(is_canonical_cid_v1(cid), "valid CID rejected: {cid}");
            assert_eq!(
                validate_ipfs_cid(cid).expect("canonical CID must validate"),
                cid
            );
        }
        let uppercase = TEST_CID_PAYLOAD.to_ascii_uppercase();
        let padded = format!("{TEST_CID_PAYLOAD}=");
        let truncated = &TEST_CID_PAYLOAD[..TEST_CID_PAYLOAD.len() - 1];
        for cid in [
            "",
            "QmYwAPJzv5CZsnAzt8auVZRnGi2j4XQJKiTyrZq4XgNLwN",
            "bafytestcid",
            uppercase.as_str(),
            padded.as_str(),
            truncated,
        ] {
            assert!(!is_canonical_cid_v1(cid), "invalid CID accepted: {cid}");
            assert!(validate_ipfs_cid(cid).is_err());
        }

        let rendered = format!("{endpoint:?}");
        assert!(rendered.contains("[REDACTED]"));
        assert!(!rendered.contains("never-log-this-token"));
        let checkpoint_key = CheckpointKey([0x5a; 32]);
        assert_eq!(format!("{checkpoint_key:?}"), "CheckpointKey([REDACTED])");
    }

    #[tokio::test]
    async fn hardened_http_refuses_redirect_header_body_and_encoding_attacks() {
        let redirect_router = Router::new()
            .route(
                "/redirect",
                get(|| async { Redirect::temporary("/target") }),
            )
            .route("/target", get(|| async { "followed" }));
        let (redirect, redirect_task) = spawn_router(redirect_router, "/redirect").await;
        let response = redirect
            .request(Method::GET, redirect.url.clone())
            .expect("build redirect request")
            .send()
            .await
            .expect("receive redirect response");
        assert!(response.status().is_redirection());
        redirect_task.abort();

        let router = Router::new()
            .route("/headers", get(response_header_bomb))
            .route("/body", get(response_body_bomb))
            .route("/gzip", get(response_gzip));
        let (endpoint, task) = spawn_router(router, "/headers").await;
        let response = endpoint
            .request(Method::GET, endpoint.url.clone())
            .expect("build header request")
            .send()
            .await
            .expect("receive header response");
        assert!(read_bounded_response(response, 1024).await.is_err());

        let mut body_url = endpoint.url.clone();
        body_url.set_path("/body");
        let response = endpoint
            .request(Method::GET, body_url)
            .expect("build body request")
            .send()
            .await
            .expect("receive body response");
        assert!(read_bounded_response(response, 16).await.is_err());

        let mut gzip_url = endpoint.url.clone();
        gzip_url.set_path("/gzip");
        let response = endpoint
            .request(Method::GET, gzip_url)
            .expect("build gzip request")
            .send()
            .await
            .expect("receive gzip response");
        assert!(read_bounded_response(response, 16).await.is_err());
        task.abort();
    }

    #[tokio::test]
    async fn ipfs_publication_rejects_malformed_cid_missing_pin_and_wrong_readback() {
        let cases = [
            MockIpfsState {
                add_body: Arc::new(b"not-json".to_vec()),
                cat_body: Arc::new(b"payload".to_vec()),
                pin_present: true,
            },
            MockIpfsState {
                add_body: Arc::new(br#"{"Hash":"bad/cid"}"#.to_vec()),
                cat_body: Arc::new(b"payload".to_vec()),
                pin_present: true,
            },
            MockIpfsState {
                add_body: Arc::new(format!(r#"{{"Hash":"{TEST_CID_PAYLOAD}"}}"#).into_bytes()),
                cat_body: Arc::new(b"payload".to_vec()),
                pin_present: false,
            },
            MockIpfsState {
                add_body: Arc::new(format!(r#"{{"Hash":"{TEST_CID_PAYLOAD}"}}"#).into_bytes()),
                cat_body: Arc::new(b"different".to_vec()),
                pin_present: true,
            },
        ];
        for state in cases {
            let (endpoint, task) = spawn_router(mock_ipfs_router(state), "/").await;
            let result = ipfs_add_verified(&endpoint, "block.to", b"payload", 1024, 1024).await;
            assert!(result.is_err());
            task.abort();
        }

        let valid = MockIpfsState {
            add_body: Arc::new(format!(r#"{{"Hash":"{TEST_CID_PAYLOAD}"}}"#).into_bytes()),
            cat_body: Arc::new(b"payload".to_vec()),
            pin_present: true,
        };
        let (endpoint, task) = spawn_router(mock_ipfs_router(valid), "/").await;
        assert_eq!(
            ipfs_add_verified(&endpoint, "block.to", b"payload", 1024, 1024)
                .await
                .expect("valid mock IPFS publication"),
            TEST_CID_PAYLOAD
        );
        task.abort();
    }

    #[tokio::test]
    async fn signed_head_cas_rejects_conflict_bootstrap_and_readback_drift() {
        for status in [StatusCode::CONFLICT, StatusCode::PRECONDITION_FAILED] {
            let (endpoint, _state, task) = spawn_signed_head(SignedHeadInner {
                bytes: Some(b"old".to_vec()),
                etag: "\"v1\"".to_owned(),
                put_status: Some(status),
                ..SignedHeadInner::default()
            })
            .await;
            let current = PublicHead::Present {
                bytes: b"old".to_vec(),
                token: "\"v1\"".to_owned(),
            };
            assert!(
                put_signed_http_head(&endpoint, b"new", &current, false, 1024)
                    .await
                    .is_err()
            );
            task.abort();
        }

        let (endpoint, state, task) = spawn_signed_head(SignedHeadInner::default()).await;
        assert!(
            put_signed_http_head(&endpoint, b"new", &PublicHead::Missing, false, 1024)
                .await
                .is_err()
        );
        assert_eq!(state.0.lock().await.put_count, 0);
        task.abort();

        let (endpoint, _state, task) = spawn_signed_head(SignedHeadInner {
            bytes: Some(b"old".to_vec()),
            etag: "\"v1\"".to_owned(),
            readback_override: Some(b"attacker".to_vec()),
            ..SignedHeadInner::default()
        })
        .await;
        let current = PublicHead::Present {
            bytes: b"old".to_vec(),
            token: "\"v1\"".to_owned(),
        };
        assert!(
            put_signed_http_head(&endpoint, b"new", &current, false, 1024)
                .await
                .is_err()
        );
        task.abort();
    }

    #[tokio::test]
    async fn ipns_publication_rejects_pre_post_movement_and_readback_drift() {
        let initial = PublicHead::Present {
            bytes: b"old".to_vec(),
            token: TEST_CID_OLD.to_owned(),
        };
        let cases = [
            (
                VecDeque::from([TEST_CID_ATTACKER.to_owned()]),
                HashMap::from([(TEST_CID_ATTACKER.to_owned(), b"attacker".to_vec())]),
            ),
            (
                VecDeque::from([TEST_CID_OLD.to_owned(), TEST_CID_ATTACKER.to_owned()]),
                HashMap::from([
                    (TEST_CID_OLD.to_owned(), b"old".to_vec()),
                    (TEST_CID_ATTACKER.to_owned(), b"attacker".to_vec()),
                ]),
            ),
            (
                VecDeque::from([TEST_CID_OLD.to_owned(), TEST_CID_NEW.to_owned()]),
                HashMap::from([
                    (TEST_CID_OLD.to_owned(), b"old".to_vec()),
                    (TEST_CID_NEW.to_owned(), b"wrong".to_vec()),
                ]),
            ),
        ];
        for (resolutions, bodies) in cases {
            let state = IpnsMockState {
                resolutions: Arc::new(Mutex::new(resolutions)),
                bodies: Arc::new(bodies),
                publish_count: Arc::new(AtomicU64::new(0)),
            };
            let (endpoint, task) = spawn_router(mock_ipns_router(state), "/").await;
            assert!(
                publish_ipns_head(
                    &endpoint,
                    "test-name",
                    "test-key",
                    TEST_CID_NEW,
                    b"new",
                    &initial,
                    false,
                    1024,
                )
                .await
                .is_err()
            );
            task.abort();
        }
    }

    #[test]
    fn mirror_file_rejects_truncation_metadata_drift_and_recovers_when_missing() {
        let dir = secure_temp_dir();
        let source = signed_source(2, 0x3a, 1_800_000_000);
        let mut checkpoint = checkpoint_from_source(&source);
        let mirror = mirror_index_value(
            &source,
            &checkpoint.mirror_blocks,
            checkpoint.generation,
            &checkpoint.head_ipfs_cid,
            &checkpoint.public_head_token,
            checkpoint.published_at_unix,
        )
        .expect("build test mirror");
        let canonical = json::to_json_pretty(&mirror)
            .expect("encode test mirror")
            .into_bytes();
        checkpoint.mirror_blake3 = blake3_array(&canonical);
        let path = dir.path().join(MIRROR_INDEX_FILE);
        write_atomic_secret(&path, &canonical).expect("write test mirror");
        verify_mirror_file(dir.path(), &checkpoint).expect("valid mirror accepted");

        fs::remove_file(&path).expect("remove mirror for recovery");
        verify_or_recover_mirror_file(dir.path(), &checkpoint, &source)
            .expect("missing mirror rebuilt deterministically");
        assert_eq!(fs::read(&path).expect("read rebuilt mirror"), canonical);

        fs::write(&path, &canonical[..canonical.len() / 2]).expect("truncate mirror");
        assert!(verify_mirror_file(dir.path(), &checkpoint).is_err());

        for field in ["schema", "generation", "head"] {
            let mut value = mirror.clone();
            match field {
                "schema" => {
                    value
                        .as_object_mut()
                        .expect("mirror object")
                        .insert("schema".into(), JsonValue::from("wrong.schema"));
                }
                "generation" => {
                    value
                        .as_object_mut()
                        .expect("mirror object")
                        .insert("generation".into(), JsonValue::from(99_u64));
                }
                "head" => {
                    value
                        .get_mut("head")
                        .and_then(JsonValue::as_object_mut)
                        .expect("head object")
                        .insert(
                            "head_block_cid_hex".into(),
                            JsonValue::from("00".repeat(32)),
                        );
                }
                _ => unreachable!("closed test field set"),
            }
            let bytes = json::to_json_pretty(&value)
                .expect("encode drifted mirror")
                .into_bytes();
            let mut matching_digest_checkpoint = checkpoint.clone();
            matching_digest_checkpoint.mirror_blake3 = blake3_array(&bytes);
            fs::write(&path, bytes).expect("write drifted mirror");
            assert!(verify_mirror_file(dir.path(), &matching_digest_checkpoint).is_err());
        }
    }

    #[test]
    fn durable_restart_state_preserves_every_publish_phase() {
        let dir = secure_temp_dir();
        let source = signed_source(2, 0x3b, 1_800_000_000);
        let key = [0x61; 32];
        let mut intent = intent_from_source(&source);
        for block in &mut intent.blocks {
            block.ipfs_cid = None;
        }
        intent.head_ipfs_cid = None;
        save_publish_intent(dir.path(), &key, &intent).expect("persist prepared intent");
        assert_eq!(
            load_publish_intent(dir.path(), &key)
                .expect("reload prepared intent")
                .expect("prepared intent exists")
                .blocks
                .iter()
                .filter(|block| block.ipfs_cid.is_some())
                .count(),
            0
        );

        intent.blocks[0].ipfs_cid = Some(TEST_CID_BLOCK.to_owned());
        save_publish_intent(dir.path(), &key, &intent).expect("persist partial pins");
        assert_eq!(
            load_publish_intent(dir.path(), &key)
                .expect("reload partial pins")
                .expect("partial intent exists")
                .blocks[0]
                .ipfs_cid
                .as_deref(),
            Some(TEST_CID_BLOCK)
        );

        intent.blocks[1].ipfs_cid = Some(TEST_CID_PAYLOAD.to_owned());
        intent.head_ipfs_cid = Some(TEST_CID_HEAD.to_owned());
        save_publish_intent(dir.path(), &key, &intent).expect("persist head pin");
        let loaded = load_publish_intent(dir.path(), &key)
            .expect("reload head pin")
            .expect("head intent exists");
        assert_eq!(loaded.head_ipfs_cid.as_deref(), Some(TEST_CID_HEAD));

        let target = PublicHead::Present {
            bytes: intent.target_head_bytes.clone(),
            token: "\"target\"".to_owned(),
        };
        assert_eq!(
            public_head_digest(&target),
            Some(intent.target_head_blake3),
            "restart recognizes a public head already at the durable target"
        );

        let checkpoint = checkpoint_from_source(&source);
        save_checkpoint(dir.path(), &key, &checkpoint).expect("persist checkpoint before cleanup");
        assert!(
            load_checkpoint(dir.path(), &key)
                .expect("reload checkpoint")
                .is_some()
        );
        assert!(
            load_publish_intent(dir.path(), &key)
                .expect("reload stale completed intent")
                .is_some()
        );
        remove_durable_file(&dir.path().join(PUBLISH_INTENT_FILE))
            .expect("restart removes completed intent");
        assert!(
            load_publish_intent(dir.path(), &key)
                .expect("intent remains absent")
                .is_none()
        );
    }

    #[tokio::test]
    async fn metrics_expose_exact_values_and_payload_kind_counts() {
        let mut block = JsonMap::new();
        block.insert("payload_kind".into(), JsonValue::from("deal_settlement"));
        let mut mirror = JsonMap::new();
        mirror.insert(
            "blocks".into(),
            JsonValue::Array(vec![
                JsonValue::Object(block.clone()),
                JsonValue::Object(block),
            ]),
        );
        let state = ApiState(Arc::new(RwLock::new(ApiSnapshot {
            mirror: Some(JsonValue::Object(mirror)),
            metrics: ServiceMetrics {
                publish_success_total: 2,
                publish_failure_total: 3,
                published_bytes_total: 5,
                last_publish_timestamp_seconds: 7,
                backlog: 11,
                head_age_seconds: 13,
                ipfs_pin_lag_seconds: 17,
                ipns_update_success_total: 19,
                ipns_update_failure_total: 23,
                last_ipns_update_timestamp_seconds: 29,
                validation_failure_total: 31,
                mirror_drift: 37,
            },
            ..ApiSnapshot::default()
        })));
        let response = metrics_handler(State(state)).await;
        let body = axum::body::to_bytes(response.into_body(), 64 * 1024)
            .await
            .expect("read metrics body");
        let body = std::str::from_utf8(&body).expect("metrics are UTF-8");
        for expected in [
            "result=\"success\"} 2",
            "result=\"failure\"} 3",
            "published_bytes_total{sink=\"ipfs\"} 5",
            "last_ipns_update_timestamp_seconds 29",
            "validation_failure_total 31",
            "mirror_drift 37",
            "blocks{payload_kind=\"deal_settlement\"} 2",
        ] {
            assert!(body.contains(expected), "missing metric row: {expected}");
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    #[ignore = "requires SORAFS_RUN_KUBO_INTEGRATION=1 and a local Kubo binary"]
    async fn real_kubo_publication_ipns_restart_and_tamper_lane() {
        let kubo = KuboHarness::start().await;
        let endpoint = kubo.endpoint();
        assert_kubo_has_no_swarm_peers(&endpoint).await;
        let ipns_name = kubo_key_generate(&endpoint, KUBO_IPNS_KEY_ALIAS).await;

        let direct_payload = b"sorafs-governance-dag-real-kubo-integration-v1";
        let direct_cid = ipfs_add_verified(
            &endpoint,
            "direct-integration-object.to",
            direct_payload,
            1024 * 1024,
            1024 * 1024,
        )
        .await
        .expect("real Kubo add/pin/ls/cat roundtrip");
        assert!(is_canonical_cid_v1(&direct_cid));
        assert_eq!(
            ipfs_cat(
                &endpoint,
                &direct_cid,
                direct_payload.len() as u64,
                1024 * 1024
            )
            .await
            .expect("cat direct Kubo object"),
            direct_payload
        );
        assert!(
            ipfs_cat(
                &endpoint,
                &direct_cid,
                direct_payload.len() as u64 - 1,
                1024 * 1024,
            )
            .await
            .is_err(),
            "bounded cat must reject a real response larger than expected"
        );
        kubo_unpin(&endpoint, &direct_cid).await;
        assert!(
            ipfs_verify_pin(&endpoint, &direct_cid, 1024 * 1024)
                .await
                .is_err(),
            "real Kubo pin/ls must expose a removed recursive pin"
        );
        ipfs_pin(&endpoint, &direct_cid, 1024 * 1024)
            .await
            .expect("restore direct object pin");
        assert!(
            ipfs_cat(&endpoint, TEST_CID_ATTACKER, 1024, 1024)
                .await
                .is_err(),
            "unknown content-addressed bytes must fail closed"
        );

        let work = secure_temp_dir();
        let source_dir = work.path().join("source");
        let state_dir = work.path().join("state");
        let checkpoint_key_path = work.path().join("checkpoint.key");
        fs::write(&checkpoint_key_path, [0x8d_u8; CHECKPOINT_KEY_BYTES])
            .expect("write integration checkpoint key");
        #[cfg(unix)]
        fs::set_permissions(&checkpoint_key_path, fs::Permissions::from_mode(0o600))
            .expect("secure integration checkpoint key");

        let first_timestamp = current_unix_timestamp_seconds().saturating_sub(5);
        let mut source = signed_source(3, 0x72, first_timestamp);
        materialize_source_snapshot(&source_dir, &mut source);
        let view = real_kubo_service_view(
            &source,
            &source_dir,
            &state_dir,
            &checkpoint_key_path,
            &kubo.api_url,
            &ipns_name,
        );

        let mut service = Service::from_view(view.clone())
            .await
            .expect("initialize G-DAG service against real Kubo");
        service
            .reconcile_once()
            .await
            .expect("publish verified source through real Kubo and IPNS");
        let checkpoint = service
            .checkpoint
            .clone()
            .expect("first reconciliation persists checkpoint");
        assert_eq!(checkpoint.block_count, source.blocks.len() as u64);
        assert_eq!(checkpoint.mirror_blocks.len(), source.blocks.len());
        assert!(!state_dir.join(PUBLISH_INTENT_FILE).exists());
        assert!(state_dir.join(CHECKPOINT_FILE).is_file());
        assert!(state_dir.join(MIRROR_INDEX_FILE).is_file());
        for (published, block) in checkpoint.mirror_blocks.iter().zip(&source.blocks) {
            ipfs_verify_pin(&service.ipfs, &published.ipfs_cid, 1024 * 1024)
                .await
                .expect("real Kubo retains recursive block pin");
            assert_eq!(
                ipfs_cat(
                    &service.ipfs,
                    &published.ipfs_cid,
                    block.bytes.len() as u64,
                    1024 * 1024,
                )
                .await
                .expect("read real Kubo block"),
                block.bytes
            );
        }
        let public = resolve_ipns_head(&service.ipfs, &ipns_name, 1024 * 1024)
            .await
            .expect("resolve published IPNS head");
        assert!(matches!(
            &public,
            PublicHead::Present { bytes, token }
                if bytes == &source.head_bytes && token == &checkpoint.head_ipfs_cid
        ));

        fs::remove_file(state_dir.join(MIRROR_INDEX_FILE))
            .expect("remove mirror to exercise deterministic recovery");
        service
            .reconcile_once()
            .await
            .expect("steady-state reconciliation rebuilds missing mirror");
        assert!(state_dir.join(MIRROR_INDEX_FILE).is_file());

        kubo_unpin(&service.ipfs, &checkpoint.head_ipfs_cid).await;
        let missing_pin = service
            .reconcile_once()
            .await
            .expect_err("steady state must reject a missing real Kubo head pin");
        assert!(matches!(missing_pin, ServiceError::Network(_)));
        ipfs_pin(&service.ipfs, &checkpoint.head_ipfs_cid, 1024 * 1024)
            .await
            .expect("restore real Kubo head pin");
        service
            .reconcile_once()
            .await
            .expect("steady state recovers after head repin");

        let checkpoint_path = state_dir.join(CHECKPOINT_FILE);
        let checkpoint_bytes = fs::read(&checkpoint_path).expect("read authenticated checkpoint");
        let mut tampered_checkpoint = checkpoint_bytes.clone();
        let tamper_position = tampered_checkpoint.len() / 2;
        tampered_checkpoint[tamper_position] ^= 0x80;
        fs::write(&checkpoint_path, tampered_checkpoint).expect("tamper checkpoint bytes");
        let checkpoint_error = service
            .reconcile_once()
            .await
            .expect_err("authenticated checkpoint tamper must fail closed");
        assert!(matches!(checkpoint_error, ServiceError::State(_)));
        fs::write(&checkpoint_path, checkpoint_bytes).expect("restore authenticated checkpoint");
        service
            .reconcile_once()
            .await
            .expect("restored authenticated checkpoint reconciles");

        drop(service);
        let mut restarted = Service::from_view(view)
            .await
            .expect("restart G-DAG service from durable state");
        restarted
            .reconcile_once()
            .await
            .expect("restart verifies checkpoint, IPNS head, pins, and readback");
        assert_eq!(
            restarted
                .checkpoint
                .as_ref()
                .expect("restart loaded checkpoint")
                .generation,
            checkpoint.generation
        );
        assert!(restarted.api.0.read().await.ready);

        let attacker_bytes = b"concurrent-authorized-but-unexpected-ipns-head";
        let attacker_cid = ipfs_add_verified(
            &restarted.ipfs,
            "attacker-head.to",
            attacker_bytes,
            1024 * 1024,
            1024 * 1024,
        )
        .await
        .expect("publish adversarial head bytes to real Kubo");
        let current = resolve_ipns_head(&restarted.ipfs, &ipns_name, 1024 * 1024)
            .await
            .expect("read current IPNS head before adversarial movement");
        publish_ipns_head(
            &restarted.ipfs,
            &ipns_name,
            KUBO_IPNS_KEY_ALIAS,
            &attacker_cid,
            attacker_bytes,
            &current,
            false,
            1024 * 1024,
        )
        .await
        .expect("move test IPNS name with its isolated key");
        let moved = restarted
            .reconcile_once()
            .await
            .expect_err("checkpoint reconciliation must reject unexpected IPNS movement");
        assert!(matches!(moved, ServiceError::Conflict(_)));

        let attacker = resolve_ipns_head(&restarted.ipfs, &ipns_name, 1024 * 1024)
            .await
            .expect("resolve adversarial IPNS value");
        publish_ipns_head(
            &restarted.ipfs,
            &ipns_name,
            KUBO_IPNS_KEY_ALIAS,
            &checkpoint.head_ipfs_cid,
            &source.head_bytes,
            &attacker,
            false,
            1024 * 1024,
        )
        .await
        .expect("restore checkpointed IPNS value");
        restarted
            .reconcile_once()
            .await
            .expect("restored IPNS head returns service to steady state");

        eprintln!(
            "real Kubo G-DAG lane passed: direct_cid={direct_cid} head_cid={} ipns_name={ipns_name}",
            checkpoint.head_ipfs_cid
        );
        drop(restarted);
        kubo.shutdown();
    }

    #[test]
    fn remote_head_rejects_future_timestamp() {
        let now = current_unix_timestamp_seconds();
        let signer = TestSigner::new(0x39);
        let mut source = signed_source(1, 0x39, now);
        source.head.generated_at = now + 120;
        source.head.head_signature = signer.sign(
            &source
                .head
                .signature_payload_bytes()
                .expect("encode future head"),
        );
        source.head_bytes = norito::to_bytes(&source.head).expect("encode future head");
        let dir = secure_temp_dir();
        let config = test_runtime_config(&source, dir.path());
        assert!(validate_remote_head(&source.head_bytes, &source, &config).is_err());
    }
}
