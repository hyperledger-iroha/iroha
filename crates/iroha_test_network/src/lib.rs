//! Puppeteer for `irohad`, to create test networks
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]

mod config;
pub mod fslock_ports;

use core::{fmt, future::Future, time::Duration};
use std::{
    borrow::Cow,
    collections::{HashMap, HashSet, hash_map::DefaultHasher},
    ffi::OsString,
    fs,
    hash::{Hash as StdHash, Hasher},
    io::{ErrorKind, Read, Seek, SeekFrom, Write},
    iter,
    net::TcpListener,
    num::NonZero,
    ops::Deref,
    path::{Component, Path, PathBuf},
    process::{ExitStatus, Stdio},
    sync::{
        Arc, Mutex as StdMutex, OnceLock,
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
    },
    time::{Instant, SystemTime, UNIX_EPOCH},
};

use color_eyre::eyre::{Context, Report, Result, eyre};
pub use config::chain_id;
use fslock::LockFile;
use fslock_ports::AllocatedPort;
use futures::{prelude::*, stream::FuturesUnordered};
use iroha::{client::Client, data_model::prelude::*};
use iroha_config::{
    base::{
        env::MockEnv,
        read::ConfigReader,
        ParameterOrigin,
        toml::{TomlSource, WriteExt as _, Writer as TomlWriter},
    },
    parameters::actual::{ConsensusMode, SumeragiNposTimeoutOverrides},
};
use iroha_core::sumeragi::consensus::{
    NPOS_TAG, PERMISSIONED_TAG, PROTO_VERSION, compute_consensus_fingerprint_from_params,
};
use iroha_core::sumeragi::network_topology::redundant_send_r_from_len;
use iroha_crypto::{Algorithm, Hash as CryptoHash, ExposedPrivateKey, KeyPair, PrivateKey, PublicKey};
#[cfg(test)]
use iroha_data_model::da::commitment::DaProofPolicyBundle;
use iroha_data_model::{
    ChainId,
    account::AccountId,
    block::consensus::{ConsensusGenesisParams, NposGenesisParams},
    isi::{
        InstructionBox, SetParameter, set_instruction_registry,
        staking::{ActivatePublicLaneValidator, RegisterPublicLaneValidator},
    },
    parameter::{
        CustomParameter, SmartContractParameter, SumeragiParameter,
        system::{SumeragiNposParameters, consensus_metadata},
    },
    transaction::Executable,
};
use iroha_genesis::{GenesisBlock, GenesisTopologyEntry};
use iroha_primitives::{
    addr::{SocketAddr, socket_addr},
    unique_vec::UniqueVec,
};
use iroha_telemetry::metrics::Status;
use iroha_test_samples::{
    ALICE_ID, ALICE_KEYPAIR, PEER_KEYPAIR, REAL_GENESIS_ACCOUNT_KEYPAIR,
    SAMPLE_GENESIS_ACCOUNT_KEYPAIR,
};
use iroha_version::codec::EncodeVersioned;
use nonzero_ext::nonzero;
use norito::json::{self, Value as JsonValue};
// no external dependency needed: versioned encoding is a single leading byte (1)
use rand::prelude::IteratorRandom;
use tokio::{
    fs::File,
    io::{AsyncBufReadExt, AsyncRead, AsyncWriteExt, BufReader},
    process::Child,
    runtime::{self, Runtime},
    sync::{Mutex, Notify, broadcast, oneshot, watch},
    task::{JoinSet, spawn_blocking},
    time::timeout,
};
use toml::{Table, Value, map::Entry};
use tracing::{Instrument, debug, error, info, info_span, warn};

use crate::config::ensure_genesis_results;
pub use crate::config::genesis as genesis_factory;
const DEFAULT_BLOCK_SYNC: Duration = Duration::from_millis(150);
// Fast localnet pipeline time for test networks; callers can opt into Sumeragi defaults.
const LOCALNET_PIPELINE_TIME: Duration = Duration::from_secs(1);
// Sumeragi defaults, used only when the builder is explicitly told to keep them.
const DEFAULT_BLOCK_TIME: Duration = Duration::from_secs(2);
const DEFAULT_COMMIT_TIME: Duration = Duration::from_secs(4);
const DEFAULT_PIPELINE_TIME: Duration = Duration::from_secs(6);
// Allow generous shutdowns in multi-peer tests; peers may need to flush logs and close streams.
const PEER_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(30);
const LOG_FLUSH_TIMEOUT: Duration = Duration::from_secs(5);
const STORAGE_LISTING_LIMIT: usize = 8;
const SNAPSHOT_MESSAGE_SNIPPET_MAX_CHARS: usize = 512;
const PEER_STARTUP_TIMEOUT_PER_PEER_SECS: u64 = 60;

const NON_OPTIMIZED_IVM_FUEL: NonZero<u64> = nonzero!(1_000_000_000u64);
/// Minimum consensus pipeline time accepted by `with_pipeline_time` (milliseconds).
const MIN_PIPELINE_TIME_MS: u64 = 2;
/// Interval at which we emit watchdog logs while waiting for block 1.
const GENESIS_BLOCK_LOG_INTERVAL: Duration = Duration::from_secs(10);
const POST_GENESIS_LIVENESS_WINDOW: Duration = Duration::from_secs(5);
const DEFAULT_RBC_STORE_MAX_SESSIONS: i64 = 256;
const DEFAULT_RBC_STORE_SOFT_SESSIONS: i64 = 192;
const DEFAULT_RBC_STORE_MAX_BYTES: i64 = 64 * 1024 * 1024;
const DEFAULT_RBC_STORE_SOFT_BYTES: i64 = 48 * 1024 * 1024;
const LOCALNET_RBC_CHUNK_MAX_BYTES: i64 = 256 * 1024;
const DA_ENABLED_ENV: &str = "SUMERAGI_DA_ENABLED";
const SERIALIZE_NETWORKS_ENV: &str = "IROHA_TEST_SERIALIZE_NETWORKS";
const NETWORK_PARALLELISM_ENV: &str = "IROHA_TEST_NETWORK_PARALLELISM";
const NETWORK_PERMIT_DIR_ENV: &str = "IROHA_TEST_NETWORK_PERMIT_DIR";
const NETWORK_PERMIT_POLL_INTERVAL: Duration = Duration::from_millis(10);
const NETWORK_PERMIT_LOG_INTERVAL: Duration = Duration::from_secs(60);
const NETWORK_PERMIT_STALE_TTL: Duration = Duration::from_secs(60 * 60 * 12);
// Keep test-network parallelism conservative; DA/RBC-heavy suites are resource intensive.
const DEFAULT_NETWORK_PARALLELISM_PEERS: usize = 64;
const TEST_CONCURRENCY_OVERSUBSCRIPTION: usize = 2;
const TEST_CONCURRENCY_MIN_THREADS: usize = 4;
const PERMISSIONED_BLS_DOMAIN: &str = "bls-iroha2:permissioned-sumeragi:v1";
const NPOS_BLS_DOMAIN: &str = "bls-iroha2:npos-sumeragi:v1";
const PIPELINE_SIDECARS_DATA_FILE: &str = "sidecars.norito";
const PIPELINE_SIDECARS_INDEX_FILE: &str = "sidecars.index";
const PIPELINE_INDEX_ENTRY_SIZE: usize = core::mem::size_of::<u64>() * 2;
const PIPELINE_INDEX_ENTRY_SIZE_U64: u64 = PIPELINE_INDEX_ENTRY_SIZE as u64;
/// Grace period before we start emitting warning-level status poll failures during startup.
/// This keeps integration test output quieter while peers are still binding sockets.
const STARTUP_STATUS_WARN_GRACE: Duration = Duration::from_secs(5);
/// Minimum spacing between repeated warning logs for startup status failures after the grace.
const STARTUP_STATUS_WARN_INTERVAL: Duration = Duration::from_secs(5);

type GenesisBuilderFn = Box<
    dyn Fn(UniqueVec<PeerId>, Vec<GenesisTopologyEntry>) -> GenesisBlock + Send + Sync + 'static,
>;

fn read_env_duration(var: &str, default: Duration) -> Duration {
    if let Ok(val) = std::env::var(var) {
        // Accept seconds or ms suffix (e.g., "45" or "4500ms")
        let trimmed = val.trim();
        if let Some(ms) = trimmed.strip_suffix("ms")
            && let Ok(n) = ms.parse::<u64>()
        {
            return Duration::from_millis(n);
        }
        if let Ok(n) = trimmed.parse::<u64>() {
            return Duration::from_secs(n);
        }
    }
    default
}

fn unix_timestamp_ms_now() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_millis()
}

/// Tracks whether startup warning messages should be emitted or downgraded.
#[derive(Clone)]
struct StartupWarnGate {
    started_at: Instant,
    grace: Duration,
    interval: Duration,
    last_warn: Arc<StdMutex<Option<Instant>>>,
}

impl StartupWarnGate {
    fn new(grace: Duration) -> Self {
        Self::with_interval(grace, STARTUP_STATUS_WARN_INTERVAL)
    }

    fn with_interval(grace: Duration, interval: Duration) -> Self {
        Self {
            started_at: Instant::now(),
            grace,
            interval,
            last_warn: Arc::new(StdMutex::new(None)),
        }
    }

    fn should_warn(&self) -> bool {
        if self.started_at.elapsed() < self.grace {
            return false;
        }

        let now = Instant::now();
        let mut last_warn = self
            .last_warn
            .lock()
            .expect("warn gate should not be poisoned");
        if let Some(last) = *last_warn
            && now.duration_since(last) < self.interval
        {
            return false;
        }
        *last_warn = Some(now);
        true
    }
}

fn log_status_warning(gate: &StartupWarnGate, warn_log: impl FnOnce(), debug_log: impl FnOnce()) {
    if gate.should_warn() {
        warn_log();
    } else {
        debug_log();
    }
}

fn status_error_is_connection_refused(err: &Report) -> bool {
    err.chain().any(|cause| {
        cause
            .downcast_ref::<std::io::Error>()
            .is_some_and(|io_err| io_err.kind() == ErrorKind::ConnectionRefused)
    })
}

/// Try binding to all provided addresses to detect missing socket permissions early.
fn preflight_bind_addresses(
    addresses: impl IntoIterator<Item = SocketAddr>,
) -> std::io::Result<()> {
    for addr in addresses {
        let listener = TcpListener::bind(addr)?;
        drop(listener);
    }
    Ok(())
}

fn sync_timeout_env() -> Duration {
    // Default 60s; override with IROHA_TEST_SYNC_TIMEOUT_SECS or *_MS
    let secs = read_env_duration("IROHA_TEST_SYNC_TIMEOUT_SECS", Duration::from_secs(0));
    if secs != Duration::from_secs(0) {
        return secs;
    }
    // Keep override available for slower hosts; default to 180s to tolerate heavier fixtures.
    read_env_duration("IROHA_TEST_SYNC_TIMEOUT_MS", Duration::from_secs(180))
}

fn peer_start_timeout_env() -> Duration {
    // Default to the sync timeout; override with IROHA_TEST_PEER_START_TIMEOUT_SECS or *_MS.
    let secs = read_env_duration("IROHA_TEST_PEER_START_TIMEOUT_SECS", Duration::from_secs(0));
    if secs != Duration::from_secs(0) {
        return secs;
    }
    // Keep generous but finite default to tolerate heavier genesis without hanging forever.
    read_env_duration("IROHA_TEST_PEER_START_TIMEOUT_MS", sync_timeout_env())
}

const CLIENT_STATUS_TIMEOUT_DEFAULT: Duration = Duration::from_secs(600);
const CLIENT_TTL_DEFAULT: Duration = Duration::from_secs(1200);
const CLIENT_TTL_MIN_SLACK: Duration = Duration::from_secs(120);

fn client_status_timeout_env() -> Duration {
    // Default 600s; override with IROHA_TEST_CLIENT_STATUS_TIMEOUT_SECS or *_MS
    let secs = read_env_duration(
        "IROHA_TEST_CLIENT_STATUS_TIMEOUT_SECS",
        Duration::from_secs(0),
    );
    if secs != Duration::from_secs(0) {
        return secs;
    }
    // Keep bounded to avoid long hangs when Torii is unreachable.
    read_env_duration(
        "IROHA_TEST_CLIENT_STATUS_TIMEOUT_MS",
        CLIENT_STATUS_TIMEOUT_DEFAULT,
    )
}

fn client_request_timeout_env() -> Duration {
    // Default 30s; override with IROHA_TEST_CLIENT_REQUEST_TIMEOUT_SECS or *_MS.
    let secs = read_env_duration(
        "IROHA_TEST_CLIENT_REQUEST_TIMEOUT_SECS",
        Duration::from_secs(0),
    );
    if secs != Duration::from_secs(0) {
        return secs;
    }
    read_env_duration(
        "IROHA_TEST_CLIENT_REQUEST_TIMEOUT_MS",
        Duration::from_secs(30),
    )
}

fn client_ttl_env(status_timeout: Duration) -> Duration {
    let secs = read_env_duration("IROHA_TEST_CLIENT_TTL_SECS", Duration::ZERO);
    let ttl = if secs != Duration::ZERO {
        secs
    } else {
        read_env_duration("IROHA_TEST_CLIENT_TTL_MS", CLIENT_TTL_DEFAULT)
    };
    let min_ttl = status_timeout + CLIENT_TTL_MIN_SLACK;
    if ttl <= min_ttl {
        // Ensure TTL meaningfully exceeds the status timeout so slow consensus does not expire txs.
        min_ttl
    } else {
        ttl
    }
}

fn post_genesis_liveness_window_env() -> Duration {
    read_env_duration(
        "IROHA_TEST_POST_GENESIS_LIVENESS_MS",
        POST_GENESIS_LIVENESS_WINDOW,
    )
}

fn hex_lower(bytes: &[u8]) -> String {
    const LUT: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &byte in bytes {
        out.push(LUT[(byte >> 4) as usize] as char);
        out.push(LUT[(byte & 0x0f) as usize] as char);
    }
    out
}

const TEMPDIR_PREFIX: &str = "irohad_test_network_";
const TEMPDIR_IN_ENV: &str = "TEST_NETWORK_TMP_DIR";
const TEMPDIR_MAX_AGE: Duration = Duration::from_secs(24 * 60 * 60);
const TEMPDIR_MAX_KEEP: usize = 256;
const KEEP_TEMPDIR_ENV: &str = "IROHA_TEST_NETWORK_KEEP_DIRS";

const PROGRAM_IROHAD_ENV: &str = "TEST_NETWORK_BIN_IROHAD";
const PROGRAM_IROHA_ENV: &str = "TEST_NETWORK_BIN_IROHA";

/// Utility to get the root of the repository
pub fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../")
        .canonicalize()
        .unwrap()
}

fn default_rans_tables_path() -> PathBuf {
    repo_root().join("codec/rans/tables/rans_seed0.toml")
}

fn tempdir_in() -> Option<impl AsRef<Path>> {
    static ENV: OnceLock<Option<PathBuf>> = OnceLock::new();

    ENV.get_or_init(|| std::env::var(TEMPDIR_IN_ENV).map(PathBuf::from).ok())
        .as_ref()
}

fn prune_stale_tempdirs() {
    let base = tempdir_in()
        .map(|p| p.as_ref().to_path_buf())
        .unwrap_or_else(std::env::temp_dir);
    let Ok(read) = fs::read_dir(&base) else {
        return;
    };
    let mut entries: Vec<(PathBuf, std::time::SystemTime)> = Vec::new();
    for entry in read.flatten() {
        let path = entry.path();
        if let Some(name) = path.file_name().and_then(|s| s.to_str()) {
            if !name.starts_with(TEMPDIR_PREFIX) {
                continue;
            }
        } else {
            continue;
        }
        if let Ok(meta) = entry.metadata()
            && let Ok(modified) = meta.modified()
        {
            entries.push((path, modified));
        }
    }
    // Newest first
    entries.sort_by_key(|(_, m)| std::cmp::Reverse(*m));
    let now = std::time::SystemTime::now();
    let mut kept = 0;
    for (path, modified) in entries {
        if kept >= TEMPDIR_MAX_KEEP {
            let _ = fs::remove_dir_all(&path);
            continue;
        }
        if let Ok(age) = now.duration_since(modified)
            && age > TEMPDIR_MAX_AGE
        {
            let _ = fs::remove_dir_all(&path);
            continue;
        }
        kept += 1;
    }
}

fn init_logger_once() {
    use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

    static ONCE: OnceLock<()> = OnceLock::new();

    ONCE.get_or_init(|| {
        let _ = tracing_subscriber::registry()
            .with(env_filter_from_env_or_default())
            .with(
                tracing_subscriber::fmt::layer().with_timer(tracing_subscriber::fmt::time::time()),
            )
            .try_init();
    });
}

/// Build the `EnvFilter` used for test network logs.
///
/// Honors `RUST_LOG` if it is set; otherwise falls back to a calmer `warn` level
/// so that integration tests do not overwhelm the output buffer with informational
/// or debug messages unless explicitly requested by the developer.
fn env_filter_from_env_or_default() -> tracing_subscriber::EnvFilter {
    tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn"))
}

trait CommandEnv {
    fn env_remove(&mut self, key: &str);
}

impl CommandEnv for tokio::process::Command {
    fn env_remove(&mut self, key: &str) {
        tokio::process::Command::env_remove(self, key);
    }
}

fn config_env_override_keys() -> &'static [&'static str] {
    static KEYS: OnceLock<Vec<&'static str>> = OnceLock::new();
    KEYS.get_or_init(|| {
        let source = include_str!("../../iroha_config/src/parameters/user.rs");
        let mut keys = Vec::new();
        let mut offset = 0;
        const MARKER: &str = "env = \"";

        while let Some(pos) = source[offset..].find(MARKER) {
            let start = offset + pos + MARKER.len();
            let Some(end_rel) = source[start..].find('"') else {
                break;
            };
            let end = start + end_rel;
            let key = &source[start..end];
            if !key.is_empty()
                && key
                    .bytes()
                    .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_')
            {
                keys.push(key);
            }
            offset = end + 1;
        }

        keys.sort_unstable();
        keys.dedup();
        keys
    })
}

fn strip_config_env_overrides(cmd: &mut impl CommandEnv) {
    // Prevent developer env overrides from shadowing test network configs.
    for key in config_env_override_keys() {
        cmd.env_remove(key);
    }
}

fn generate_and_keep_temp_dir() -> PathBuf {
    prune_stale_tempdirs();
    let mut builder = tempfile::Builder::new();
    builder.prefix(TEMPDIR_PREFIX).disable_cleanup(true);
    match tempdir_in() {
        Some(create_within) => builder.tempdir_in(create_within),
        None => builder.tempdir(),
    }
    .expect("tempdir creation should work")
    .path()
    .to_path_buf()
}

/// Environment of a specific test network.
///
/// Configures things such as the temporary directory with all artifacts or the binaries to use.
///
/// Shared across [`Network`] and [`NetworkPeer`].
#[derive(Debug)]
pub struct Environment {
    /// Working directory
    dir: PathBuf,
}

// tests module lives at the end of file

/// Programs to work with.
#[derive(Copy, Clone, Debug)]
pub enum Program {
    /// Iroha Daemon CLI
    Irohad,
    /// Iroha Client CLI
    Iroha,
}

#[derive(Debug)]
struct ProgramSpec {
    name: &'static str,
    env: &'static str,
    pkg: &'static str,
    build_args: Vec<OsString>,
}

impl Program {
    fn spec(&self) -> ProgramSpec {
        match self {
            Self::Irohad => ProgramSpec {
                name: "iroha3d",
                env: PROGRAM_IROHAD_ENV,
                pkg: "irohad",
                build_args: ["--bin", "iroha3d"]
                    .into_iter()
                    .map(OsString::from)
                    .collect(),
            },
            Self::Iroha => ProgramSpec {
                name: "iroha",
                env: PROGRAM_IROHA_ENV,
                pkg: "iroha_cli",
                build_args: Vec::new(),
            },
        }
    }
}

// Cache resolved binary paths to avoid redundant rebuilds/resolution per peer
static IROHAD_BIN: OnceLock<PathBuf> = OnceLock::new();
static IROHA_BIN: OnceLock<PathBuf> = OnceLock::new();

const BUILD_CACHE_DIR: &str = ".iroha_test_network";
const BUILD_STAMP_VERSION: u32 = 3;
const IROHA_TEST_TARGET_DIR_ENV: &str = "IROHA_TEST_TARGET_DIR";
const IROHA_TEST_BUILD_PROFILE_ENV: &str = "IROHA_TEST_BUILD_PROFILE";
const IROHA_TEST_TARGET_SUBDIR: &str = "iroha-test-network";

#[derive(Debug, Clone)]
struct BuildStamp {
    fingerprint: u64,
    profile: String,
    binary: PathBuf,
}

fn resolve_target_dir_path(repo: &Path, raw: &str) -> PathBuf {
    let candidate = PathBuf::from(raw);
    if candidate.is_absolute() {
        candidate
    } else {
        repo.join(candidate)
    }
}

/// Resolve the target directory for test-network builds and artifact lookup.
fn resolve_target_dir(repo: &Path) -> PathBuf {
    if let Ok(path) = std::env::var(IROHA_TEST_TARGET_DIR_ENV) {
        return resolve_target_dir_path(repo, &path);
    }
    if let Ok(path) = std::env::var("CARGO_TARGET_DIR") {
        return resolve_target_dir_path(repo, &path).join(IROHA_TEST_TARGET_SUBDIR);
    }
    repo.join("target").join(IROHA_TEST_TARGET_SUBDIR)
}

fn default_build_profile() -> String {
    if let Ok(profile) = std::env::var(IROHA_TEST_BUILD_PROFILE_ENV) {
        return profile;
    }
    std::env::var("PROFILE").unwrap_or_else(|_| "release".to_string())
}

fn build_cache_dir(target_dir: &Path) -> PathBuf {
    target_dir.join(BUILD_CACHE_DIR)
}

fn stamp_path(cache_dir: &Path, pkg: &str, profile: &str) -> PathBuf {
    cache_dir.join(format!("{pkg}-{profile}.json"))
}

fn lock_path(cache_dir: &Path, pkg: &str, profile: &str) -> PathBuf {
    cache_dir.join(format!("{pkg}-{profile}.lock"))
}

fn global_build_lock_path(cache_dir: &Path) -> PathBuf {
    cache_dir.join("cargo-build.lock")
}

fn is_rustc_metadata_mismatch(output: &str) -> bool {
    // Re-entrant builds occasionally trip over stale/corrupted `target` artifacts (e.g. after
    // toolchain upgrades or interrupted builds). Cleaning the target dir and retrying once is a
    // pragmatic recovery strategy.
    //
    // E0460: compiled by a different rustc / incompatible metadata.
    // E0463: dependencies are "missing" (often because their artifacts vanished or are corrupted).
    output.contains("E0460")
        || output.contains("rustc --explain E0460")
        || output.contains("E0463")
        || output.contains("rustc --explain E0463")
        || output.contains("can't find crate for `")
}

fn clean_target_dir_preserving_build_cache(target_dir: &Path) -> color_eyre::Result<()> {
    if !target_dir.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(target_dir).wrap_err_with(|| {
        eyre!(
            "Failed to list target dir for cleanup: {}",
            target_dir.display()
        )
    })? {
        let entry = entry?;
        if entry.file_name().as_os_str() == std::ffi::OsStr::new(BUILD_CACHE_DIR) {
            continue;
        }
        let path = entry.path();
        let ty = entry.file_type()?;
        if ty.is_dir() {
            fs::remove_dir_all(&path).wrap_err_with(|| {
                eyre!(
                    "Failed to remove target dir entry during cleanup: {}",
                    path.display()
                )
            })?;
        } else {
            fs::remove_file(&path).wrap_err_with(|| {
                eyre!(
                    "Failed to remove target file entry during cleanup: {}",
                    path.display()
                )
            })?;
        }
    }
    Ok(())
}

#[derive(Debug, Default)]
struct IgnoreList {
    dirs: HashSet<PathBuf>,
    files: HashSet<PathBuf>,
    globs: Vec<IgnorePattern>,
}

#[derive(Debug)]
struct IgnorePattern {
    pattern: String,
    dir_only: bool,
    match_basename: bool,
}

fn read_build_stamp(path: &Path) -> color_eyre::Result<Option<BuildStamp>> {
    if !path.exists() {
        return Ok(None);
    }
    let contents =
        fs::read_to_string(path).wrap_err_with(|| eyre!("Failed to read stamp file {path:?}"))?;
    let value = json::from_str(&contents)
        .wrap_err_with(|| eyre!("Failed to parse stamp file at {path:?}"))?;
    let JsonValue::Object(map) = value else {
        return Ok(None);
    };
    let version = map
        .get("version")
        .and_then(JsonValue::as_u64)
        .map(|v| v as u32)
        .unwrap_or(0);
    if version != BUILD_STAMP_VERSION {
        return Ok(None);
    }
    let fingerprint = match map.get("fingerprint").and_then(JsonValue::as_u64) {
        Some(val) => val,
        None => return Ok(None),
    };
    let profile = match map.get("profile").and_then(JsonValue::as_str) {
        Some(val) => val.to_owned(),
        None => return Ok(None),
    };
    let binary = match map.get("binary").and_then(JsonValue::as_str) {
        Some(val) => PathBuf::from(val),
        None => return Ok(None),
    };
    Ok(Some(BuildStamp {
        fingerprint,
        profile,
        binary,
    }))
}

fn write_build_stamp(path: &Path, stamp: &BuildStamp) -> color_eyre::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .wrap_err_with(|| eyre!("Failed to create stamp directory {parent:?}"))?;
    }
    let mut object = json::Map::new();
    object.insert(
        "version".to_string(),
        json::to_value(&BUILD_STAMP_VERSION).wrap_err("encode stamp version")?,
    );
    object.insert(
        "fingerprint".to_string(),
        json::to_value(&stamp.fingerprint).wrap_err("encode stamp fingerprint")?,
    );
    object.insert(
        "profile".to_string(),
        JsonValue::String(stamp.profile.clone()),
    );
    object.insert(
        "binary".to_string(),
        JsonValue::String(stamp.binary.to_string_lossy().to_string()),
    );
    let value = JsonValue::Object(object);
    let rendered = json::to_string(&value).wrap_err("Failed to render stamp JSON")?;
    fs::write(path, rendered).wrap_err_with(|| eyre!("Failed to write stamp file {path:?}"))?;
    Ok(())
}

fn load_ignore_list(root: &Path) -> IgnoreList {
    let mut list = IgnoreList::default();
    let gitignore = root.join(".gitignore");
    let Ok(contents) = fs::read_to_string(gitignore) else {
        return list;
    };
    for raw_line in contents.lines() {
        let trimmed = raw_line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with('!') {
            continue;
        }
        let is_dir = trimmed.ends_with('/');
        let mut path_str = trimmed
            .trim_start_matches("./")
            .trim_end_matches('/')
            .trim();
        if let Some(stripped) = path_str.strip_prefix('/') {
            path_str = stripped;
        }
        if path_str.is_empty() {
            continue;
        }
        if path_str.contains('*') || path_str.contains('?') {
            list.globs.push(IgnorePattern {
                pattern: path_str.to_string(),
                dir_only: is_dir,
                match_basename: !path_str.contains('/'),
            });
            continue;
        }
        let path = PathBuf::from(path_str);
        if is_dir {
            list.dirs.insert(path);
        } else {
            list.files.insert(path);
        }
    }
    list
}

fn should_ignore_path(rel: &Path, ignore: &IgnoreList, is_dir: bool) -> bool {
    if rel.as_os_str().is_empty() {
        return false;
    }
    if is_dir {
        for ignored in &ignore.dirs {
            if rel == ignored || rel.starts_with(ignored) {
                return true;
            }
        }
    }
    if !is_dir && ignore.files.contains(rel) {
        return true;
    }
    for ignored in &ignore.dirs {
        if rel.starts_with(ignored) {
            return true;
        }
    }
    if ignore.globs.is_empty() {
        return false;
    }
    let rel_str = normalize_rel_path(rel);
    let name = rel
        .file_name()
        .map(|s| s.to_string_lossy())
        .unwrap_or_else(|| Cow::Borrowed(""));
    for pattern in &ignore.globs {
        if pattern.dir_only && !is_dir {
            continue;
        }
        let text = if pattern.match_basename {
            name.as_ref()
        } else {
            rel_str.as_str()
        };
        if glob_match(&pattern.pattern, text) {
            return true;
        }
    }
    false
}

fn normalize_rel_path(rel: &Path) -> String {
    let mut out = String::new();
    for component in rel.components() {
        let Component::Normal(raw) = component else {
            continue;
        };
        if !out.is_empty() {
            out.push('/');
        }
        out.push_str(&raw.to_string_lossy());
    }
    out
}

fn glob_match(pattern: &str, text: &str) -> bool {
    let pattern = pattern.as_bytes();
    let text = text.as_bytes();
    let mut pi = 0;
    let mut ti = 0;
    let mut star = None;
    let mut star_match = 0;
    while ti < text.len() {
        if pi < pattern.len() && (pattern[pi] == b'?' || pattern[pi] == text[ti]) {
            pi += 1;
            ti += 1;
        } else if pi < pattern.len() && pattern[pi] == b'*' {
            star = Some(pi);
            pi += 1;
            star_match = ti;
        } else if let Some(star_pos) = star {
            star_match += 1;
            ti = star_match;
            pi = star_pos + 1;
        } else {
            return false;
        }
    }
    while pi < pattern.len() && pattern[pi] == b'*' {
        pi += 1;
    }
    pi == pattern.len()
}

fn add_target_dir_to_ignore(root: &Path, ignore: &mut IgnoreList) {
    fn push_dir(root: &Path, ignore: &mut IgnoreList, path: &Path) {
        if let Ok(relative) = path.strip_prefix(root)
            && !relative.as_os_str().is_empty()
        {
            ignore.dirs.insert(relative.to_path_buf());
        }
    }

    push_dir(root, ignore, &root.join("target"));

    if let Ok(path) = std::env::var("CARGO_TARGET_DIR") {
        let base = resolve_target_dir_path(root, &path);
        push_dir(root, ignore, &base);
        push_dir(root, ignore, &base.join(IROHA_TEST_TARGET_SUBDIR));
    }

    if let Ok(path) = std::env::var(IROHA_TEST_TARGET_DIR_ENV) {
        let custom = resolve_target_dir_path(root, &path);
        push_dir(root, ignore, &custom);
    }

    let target_dir = resolve_target_dir(root);
    push_dir(root, ignore, &target_dir);
}

fn workspace_members(root: &Path) -> color_eyre::Result<Vec<PathBuf>> {
    let manifest = root.join("Cargo.toml");
    let contents = fs::read_to_string(&manifest)
        .wrap_err_with(|| eyre!("Failed to read workspace manifest at {manifest:?}"))?;
    let parsed: toml::Value = toml::from_str(&contents)
        .wrap_err_with(|| eyre!("Failed to parse workspace manifest at {manifest:?}"))?;
    let Some(workspace) = parsed.get("workspace") else {
        return Ok(vec![]);
    };
    let Some(members) = workspace.get("members").and_then(|value| value.as_array()) else {
        return Ok(vec![]);
    };
    let mut out = Vec::new();
    for member in members {
        let Some(pattern) = member.as_str() else {
            continue;
        };
        out.extend(expand_workspace_member(root, pattern)?);
    }
    // Deduplicate while preserving order
    let mut seen = HashSet::new();
    out.retain(|path| seen.insert(path.clone()));
    Ok(out)
}

fn expand_workspace_member(root: &Path, pattern: &str) -> color_eyre::Result<Vec<PathBuf>> {
    if pattern.contains('*') {
        expand_workspace_pattern(root, pattern)
    } else {
        Ok(vec![root.join(pattern)])
    }
}

fn expand_workspace_pattern(root: &Path, pattern: &str) -> color_eyre::Result<Vec<PathBuf>> {
    let segments: Vec<&str> = pattern.split('/').collect();
    let mut results = Vec::new();

    fn recurse(
        root: &Path,
        current: &Path,
        segments: &[&str],
        index: usize,
        results: &mut Vec<PathBuf>,
    ) -> color_eyre::Result<()> {
        if index == segments.len() {
            if current.exists() {
                results.push(current.to_path_buf());
            }
            return Ok(());
        }
        let segment = segments[index];
        if segment.is_empty() {
            return recurse(root, current, segments, index + 1, results);
        }
        if segment == "*" {
            if !current.exists() {
                return Ok(());
            }
            for entry in fs::read_dir(current)
                .wrap_err_with(|| eyre!("Failed to expand workspace glob at {current:?}"))?
            {
                let entry = entry?;
                if entry.file_type()?.is_dir() {
                    recurse(root, &entry.path(), segments, index + 1, results)?;
                }
            }
            return Ok(());
        }
        let next = if current == root {
            root.join(segment)
        } else {
            current.join(segment)
        };
        recurse(root, &next, segments, index + 1, results)
    }

    recurse(root, root, &segments, 0, &mut results)?;
    Ok(results)
}

fn hash_file_entry(root: &Path, path: &Path, metadata: &fs::Metadata, hasher: &mut DefaultHasher) {
    let rel = path.strip_prefix(root).unwrap_or(path);
    rel.to_string_lossy().hash(hasher);
    metadata.len().hash(hasher);
    if let Ok(modified) = metadata.modified() {
        match modified.duration_since(UNIX_EPOCH) {
            Ok(duration) => {
                duration.as_secs().hash(hasher);
                duration.subsec_nanos().hash(hasher);
            }
            Err(_) => {
                // Ignore files with pre-epoch timestamps (unlikely on supported filesystems)
            }
        }
    }
}

fn hash_file_if_exists(
    workspace_root: &Path,
    path: &Path,
    hasher: &mut DefaultHasher,
) -> color_eyre::Result<()> {
    match fs::metadata(path) {
        Ok(metadata) if metadata.is_file() => {
            hash_file_entry(workspace_root, path, &metadata, hasher);
        }
        Ok(_) => {}
        Err(err) => {
            if err.kind() != std::io::ErrorKind::NotFound {
                return Err(err).wrap_err_with(|| eyre!("Failed to read metadata for {path:?}"));
            }
        }
    }
    Ok(())
}

fn hash_member_dir(
    workspace_root: &Path,
    member_dir: &Path,
    ignore: &IgnoreList,
    hasher: &mut DefaultHasher,
) -> color_eyre::Result<()> {
    if !member_dir.exists() {
        return Ok(());
    }

    let mut stack = vec![member_dir.to_path_buf()];
    let mut visited = HashSet::new();

    while let Some(dir) = stack.pop() {
        if !dir.exists() {
            continue;
        }
        let Ok(relative_dir) = dir.strip_prefix(workspace_root) else {
            continue;
        };
        if !visited.insert(relative_dir.to_path_buf()) {
            continue;
        }
        if should_ignore_path(relative_dir, ignore, true) || should_skip_dir(&dir) {
            continue;
        }

        let entries = match fs::read_dir(&dir) {
            Ok(iter) => iter,
            Err(err) => {
                if dir == member_dir {
                    return Err(err)
                        .wrap_err_with(|| eyre!("Failed to read workspace member at {dir:?}"));
                }
                continue;
            }
        };

        for entry in entries {
            let entry = match entry {
                Ok(e) => e,
                Err(_) => continue,
            };
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            if file_type.is_symlink() {
                continue;
            }
            let path = entry.path();
            let Ok(relative_path) = path.strip_prefix(workspace_root) else {
                continue;
            };
            if file_type.is_dir() {
                if should_ignore_path(relative_path, ignore, true) || should_skip_dir(&path) {
                    continue;
                }
                stack.push(path);
            } else if file_type.is_file() {
                if should_ignore_path(relative_path, ignore, false) || should_skip_file(&path) {
                    continue;
                }
                let Ok(metadata) = entry.metadata() else {
                    continue;
                };
                hash_file_entry(workspace_root, &path, &metadata, hasher);
            }
        }
    }

    Ok(())
}

fn should_skip_dir(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
        return false;
    };
    matches!(
        name,
        "target"
            | ".git"
            | ".hg"
            | ".svn"
            | ".idea"
            | ".vscode"
            | ".cargo"
            | "node_modules"
            | ".venv"
            | "venv"
            | ".pytest_cache"
            | ".mypy_cache"
            | ".ruff_cache"
            | "__pycache__"
            | "coverage"
            | "dist"
            | "tmp"
    )
}

fn should_skip_file(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
        return false;
    };
    matches!(name, ".DS_Store" | "Thumbs.db")
}

fn workspace_fingerprint(root: &Path) -> color_eyre::Result<u64> {
    let mut hasher = DefaultHasher::new();
    let mut ignore = load_ignore_list(root);
    add_target_dir_to_ignore(root, &mut ignore);
    let members = workspace_members(root)?;

    hash_file_if_exists(root, &root.join("Cargo.toml"), &mut hasher)?;
    hash_file_if_exists(root, &root.join("Cargo.lock"), &mut hasher)?;
    hash_file_if_exists(root, &root.join("rust-toolchain.toml"), &mut hasher)?;
    hash_file_if_exists(root, &root.join("rust-toolchain"), &mut hasher)?;

    if members.is_empty() {
        hash_member_dir(root, root, &ignore, &mut hasher)?;
    } else {
        for member in members {
            hash_member_dir(root, &member, &ignore, &mut hasher)?;
        }
    }

    Ok(hasher.finish())
}

fn fingerprint_with_build_args(base: u64, build_args: &[OsString]) -> u64 {
    let mut hasher = DefaultHasher::new();
    base.hash(&mut hasher);
    for arg in build_args {
        arg.hash(&mut hasher);
    }
    hasher.finish()
}

fn build_env_overrides() -> [(&'static str, &'static str); 2] {
    // Streaming runtime requires bundled rANS tables; compile test binaries with bundles enabled.
    // Developers may work with unsynced Norito bindings locally; skip the workspace-level
    // bindings check when building test binaries to avoid unrelated integration test failures.
    [
        ("ENABLE_RANS_BUNDLES", "1"),
        ("NORITO_SKIP_BINDINGS_SYNC", "1"),
    ]
}

#[allow(clippy::too_many_arguments)] // Helper aggregates build context parameters.
fn ensure_binary_fresh(
    repo: &Path,
    pkg: &str,
    name: &str,
    target_dir: &Path,
    profile: &str,
    binary_path: &Path,
    allow_build: bool,
    build_args: &[OsString],
) -> color_eyre::Result<()> {
    let cache_dir = build_cache_dir(target_dir);
    fs::create_dir_all(&cache_dir)
        .wrap_err_with(|| eyre!("Failed to prepare build cache directory {cache_dir:?}"))?;
    let stamp_path = stamp_path(&cache_dir, pkg, profile);
    let lock_path = lock_path(&cache_dir, pkg, profile);
    let mut lock = LockFile::open(&lock_path)
        .wrap_err_with(|| eyre!("Failed to open build lock at {lock_path:?}"))?;
    lock.lock()
        .wrap_err_with(|| eyre!("Failed to acquire build lock for {pkg}"))?;

    let mut fingerprint = workspace_fingerprint(repo)?;
    fingerprint = fingerprint_with_build_args(fingerprint, build_args);
    let stamp = read_build_stamp(&stamp_path)?;

    let mut needs_build = !binary_path.exists();
    if !needs_build {
        match &stamp {
            Some(prev) if prev.fingerprint == fingerprint && prev.profile == profile => {
                // Binary is present and fingerprint matches; reuse existing build.
            }
            _ => needs_build = true,
        }
    }

    if needs_build && !allow_build {
        return Err(eyre!(
            "cannot build `{name}` (pkg `{pkg}`) while another Cargo invocation is running; \
             build it ahead of time with `cargo build -p {pkg}` or rerun with \
             IROHA_TEST_SKIP_BUILD=1 to reuse an existing binary, \
             or set IROHA_TEST_ALLOW_REENTRANT_BUILD=1 to force a rebuild; target_dir={}",
            target_dir.display()
        ));
    }

    if needs_build {
        tracing::info!(%name, %pkg, %profile, "building `{name}` for tests");
        let build_lock_path = global_build_lock_path(&cache_dir);
        let mut build_lock = LockFile::open(&build_lock_path)
            .wrap_err_with(|| eyre!("Failed to open build lock at {build_lock_path:?}"))?;
        build_lock
            .lock()
            .wrap_err_with(|| eyre!("Failed to acquire global build lock for {pkg}"))?;
        let cargo_program =
            std::env::var("TEST_NETWORK_CARGO").unwrap_or_else(|_| "cargo".to_owned());
        let mut attempt = 0_u8;
        loop {
            attempt = attempt.saturating_add(1);

            let mut command = std::process::Command::new(&cargo_program);
            command.arg("build").arg("-p").arg(pkg);
            match profile {
                "debug" => {}
                "release" => {
                    command.arg("--release");
                }
                other => {
                    command.arg("--profile").arg(other);
                }
            }
            for arg in build_args {
                command.arg(arg);
            }
            command.env("CARGO_TARGET_DIR", target_dir);
            for (key, value) in build_env_overrides() {
                command.env(key, value);
            }
            let output = command
                .current_dir(repo)
                .output()
                .wrap_err("failed to invoke cargo to build binary")?;
            if output.status.success() {
                break;
            }

            let code = output.status.code();
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            let combined = format!("{stdout}\n{stderr}");
            if attempt == 1 && is_rustc_metadata_mismatch(&combined) {
                warn!(
                    %name,
                    %pkg,
                    %profile,
                    target_dir = %target_dir.display(),
                    "detected stale/corrupted build artifacts; cleaning target dir and retrying build"
                );
                clean_target_dir_preserving_build_cache(target_dir)?;
                continue;
            }

            tracing::warn!(?code, build_stdout = %stdout, build_stderr = %stderr, "`cargo build` returned non-zero status");
            let err = eyre!(
                "failed to build `{name}` (pkg `{pkg}`), cargo status: {code:?}\n--- stdout ---\n{stdout}\n--- stderr ---\n{stderr}"
            );
            let _ = build_lock.unlock();
            return Err(err);
        }
        // Refresh fingerprint after the successful build to capture generated files.
        fingerprint = workspace_fingerprint(repo)?;
        fingerprint = fingerprint_with_build_args(fingerprint, build_args);

        build_lock
            .unlock()
            .wrap_err_with(|| eyre!("Failed to release global build lock for {pkg}"))?;
    }

    if binary_path.exists() {
        let stamp = BuildStamp {
            fingerprint,
            profile: profile.to_owned(),
            binary: binary_path.to_path_buf(),
        };
        write_build_stamp(&stamp_path, &stamp)?;
    }

    lock.unlock()
        .wrap_err_with(|| eyre!("Failed to release build lock for {pkg}"))?;
    Ok(())
}

fn allow_reentrant_build(running_under_cargo: bool) -> bool {
    if !running_under_cargo {
        return true;
    }
    // Reentrant builds use a namespaced target dir (`.../iroha-test-network`) so they don't
    // contend with the outer Cargo invocation's build lock. Keep an opt-out for debugging.
    bool_env_override("IROHA_TEST_ALLOW_REENTRANT_BUILD").unwrap_or(true)
}

impl Program {
    /// Resolve program path.
    ///
    /// Tries, in order:
    /// - Explicit env override (`TEST_NETWORK_BIN_*`).
    /// - `CARGO_BIN_EXE_*` if Cargo provided a direct path to the built binary
    /// - Common target locations (debug/release) under the repo root (defaulting to
    ///   `target/iroha-test-network`, or under `IROHA_TEST_TARGET_DIR` / `CARGO_TARGET_DIR` when set)
    /// - Rebuilds with `cargo build -p <pkg>` when the cached fingerprint disagrees with the current
    ///   workspace state (skipped when `IROHA_TEST_SKIP_BUILD=1`).
    ///
    /// # Errors
    /// If the path is not found (and build did not help).
    fn resolve_internal(&self, skip_build_override: Option<bool>) -> color_eyre::Result<PathBuf> {
        fn bin_name(raw: &str) -> String {
            if cfg!(windows) {
                format!("{raw}.exe")
            } else {
                raw.to_owned()
            }
        }

        fn try_candidates<'a>(
            candidates: impl IntoIterator<Item = Cow<'a, Path>>,
        ) -> Option<PathBuf> {
            for candidate in candidates {
                if let Ok(resolved) = candidate.as_ref().canonicalize() {
                    return Some(resolved);
                }
            }
            None
        }

        let ProgramSpec {
            name,
            env,
            pkg,
            build_args,
        } = self.spec();

        // 1) Explicit override
        if let Ok(path) = std::env::var(env) {
            let raw = PathBuf::from(&path);
            let candidate = if raw.is_absolute() {
                raw
            } else {
                repo_root().join(raw)
            };
            return candidate
                .canonicalize()
                .wrap_err_with(|| eyre!("Used path from {env}: {path}"))
                .wrap_err_with(|| {
                    eyre!("Could not resolve path of `{name}` program. Have you built it?")
                });
        }

        // Fast path via cache (only when no override is present)
        match self {
            Program::Irohad => {
                if let Some(p) = IROHAD_BIN.get() {
                    return Ok(p.clone());
                }
            }
            Program::Iroha => {
                if let Some(p) = IROHA_BIN.get() {
                    return Ok(p.clone());
                }
            }
        }

        let repo = repo_root();
        let bin = bin_name(name);

        // 2) Prefer paths Cargo already built (`CARGO_BIN_EXE_*`) but still allow rebuilds
        let cargo_bin_env = format!("CARGO_BIN_EXE_{name}");
        let cargo_bin_candidate = std::env::var(&cargo_bin_env)
            .ok()
            .and_then(|p| PathBuf::from(p).canonicalize().ok());

        // 3) Prepare candidate locations under the current target directory
        let profile = default_build_profile();
        let target_dir = resolve_target_dir(&repo);
        let primary_binary = target_dir.join(format!("{profile}/{bin}"));

        let mut candidates: Vec<PathBuf> = Vec::new();
        let mut push_candidate = |path: PathBuf| {
            if !candidates.contains(&path) {
                candidates.push(path);
            }
        };
        if let Some(path) = cargo_bin_candidate {
            push_candidate(path);
        }
        push_candidate(primary_binary.clone());
        push_candidate(target_dir.join(format!("debug/{bin}")));
        push_candidate(target_dir.join(format!("release/{bin}")));

        let default_target = repo.join("target");
        push_candidate(default_target.join(format!("{profile}/{bin}")));
        push_candidate(default_target.join(format!("debug/{bin}")));
        push_candidate(default_target.join(format!("release/{bin}")));

        let prebuild_candidate =
            try_candidates(candidates.iter().map(|p| Cow::Borrowed(p.as_path())));

        // 4) Decide whether to (re)build.
        //    We default to building to avoid using stale binaries across source changes.
        //    Set IROHA_TEST_SKIP_BUILD=1 to skip attempting a build.
        let skip_build = skip_build_override.unwrap_or_else(|| {
            std::env::var("IROHA_TEST_SKIP_BUILD")
                .ok()
                .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
                .unwrap_or(false)
        });
        let running_under_cargo = std::env::var_os("CARGO").is_some();
        let allow_reentrant = allow_reentrant_build(running_under_cargo);
        if running_under_cargo && !skip_build && !allow_reentrant {
            if let Some(found) = prebuild_candidate.clone() {
                warn!(
                    %name,
                    %pkg,
                    ?found,
                    "reentrant build disabled under Cargo; using existing binary"
                );
                match self {
                    Program::Irohad => {
                        let _ = IROHAD_BIN.set(found.clone());
                    }
                    Program::Iroha => {
                        let _ = IROHA_BIN.set(found.clone());
                    }
                }
                return Ok(found);
            }
        }
        if !skip_build {
            ensure_binary_fresh(
                &repo,
                pkg,
                name,
                &target_dir,
                &profile,
                &primary_binary,
                allow_reentrant,
                &build_args,
            )?;
        }

        // 5) Return the best candidate after the (optional) build
        if let Some(found) = try_candidates(candidates.iter().map(|p| Cow::Borrowed(p.as_path()))) {
            match self {
                Program::Irohad => {
                    let _ = IROHAD_BIN.set(found.clone());
                }
                Program::Iroha => {
                    let _ = IROHA_BIN.set(found.clone());
                }
            }
            return Ok(found);
        }

        let candidates_txt = candidates
            .iter()
            .map(|p| p.display().to_string())
            .collect::<Vec<_>>()
            .join(", ");

        Err(eyre!(
            "Could not resolve path of `{name}` program. Have you built it?\n\
               Tried: {candidates_txt}\n  \
               Solutions:\n  \
               1. Run `cargo build -p {pkg}`\n  \
               2. Provide a different path via `{env}` env var"
        ))
    }

    pub fn resolve(&self) -> color_eyre::Result<PathBuf> {
        self.resolve_internal(None)
    }

    /// Async variant of [`Self::resolve`].
    ///
    /// Spawns a blocking task so that re-entrant builds and filesystem probing never block
    /// a Tokio runtime thread (which can otherwise starve timers and hang async tests).
    pub async fn resolve_async(&self) -> color_eyre::Result<PathBuf> {
        let program = *self;
        tokio::task::spawn_blocking(move || program.resolve_internal(None))
            .await
            .wrap_err("failed to join blocking task while resolving program")?
    }

    pub fn resolve_force_build(&self) -> color_eyre::Result<PathBuf> {
        self.resolve_internal(Some(false))
    }

    pub fn resolve_skip_build(&self) -> color_eyre::Result<PathBuf> {
        self.resolve_internal(Some(true))
    }
}

pub fn init_instruction_registry() {
    set_instruction_registry(iroha_data_model::instruction_registry::default());
}

impl Environment {
    /// Side effects:
    ///
    /// - Initialises logger (once)
    /// - Creates a temporary directory (keep: true)
    fn new() -> Self {
        init_logger_once();
        init_instruction_registry();
        let dir = generate_and_keep_temp_dir();
        Self { dir }
    }
}

#[derive(Debug)]
struct FilePermit {
    path: PathBuf,
}

impl Drop for FilePermit {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

struct NetworkPermit {
    _file_permit: FilePermit,
}

fn serialize_networks_enabled() -> bool {
    let Ok(raw) = std::env::var(SERIALIZE_NETWORKS_ENV) else {
        return false;
    };
    matches!(
        raw.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}

fn network_parallelism_limit() -> usize {
    if serialize_networks_enabled() {
        return 1;
    }
    if let Ok(raw) = std::env::var(NETWORK_PARALLELISM_ENV)
        && let Ok(parsed) = raw.trim().parse::<usize>()
        && parsed > 0
    {
        return parsed;
    }
    let cores = std::thread::available_parallelism()
        .map(std::num::NonZeroUsize::get)
        .unwrap_or(1);
    let per_network = DEFAULT_NETWORK_PARALLELISM_PEERS.max(1);
    cores.saturating_div(per_network).max(1)
}

fn test_concurrency_threads() -> usize {
    let cores = std::thread::available_parallelism()
        .map(std::num::NonZeroUsize::get)
        .unwrap_or(1);
    let networks = network_parallelism_limit().max(1);
    let peers = DEFAULT_NETWORK_PARALLELISM_PEERS.max(1);
    let total_peers = networks.saturating_mul(peers).max(1);
    let oversub = TEST_CONCURRENCY_OVERSUBSCRIPTION.max(1);
    let min_threads = cores.clamp(1, TEST_CONCURRENCY_MIN_THREADS);
    cores
        .saturating_mul(oversub)
        .saturating_div(total_peers)
        .max(min_threads)
}

fn permit_dir() -> PathBuf {
    std::env::var(NETWORK_PERMIT_DIR_ENV)
        .map(PathBuf::from)
        .unwrap_or_else(|_| std::env::temp_dir().join("iroha_test_network_permits"))
}

fn try_acquire_file_permit(limit: usize) -> Option<FilePermit> {
    if limit == 0 {
        return None;
    }
    let dir = permit_dir();
    fs::create_dir_all(&dir).expect("failed to create network permit directory");
    let pid = std::process::id();
    let started = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_secs();
    for slot in 0..limit {
        let path = dir.join(format!("permit-{slot}.lock"));
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(mut file) => {
                let _ = writeln!(file, "pid={pid}");
                let _ = writeln!(file, "started={started}");
                return Some(FilePermit { path });
            }
            Err(err) if err.kind() == ErrorKind::AlreadyExists => {
                if permit_is_stale(&path) {
                    let _ = fs::remove_file(&path);
                    if let Ok(mut file) = fs::OpenOptions::new()
                        .write(true)
                        .create_new(true)
                        .open(&path)
                    {
                        let _ = writeln!(file, "pid={pid}");
                        let _ = writeln!(file, "started={started}");
                        return Some(FilePermit { path });
                    }
                }
            }
            Err(err) if err.kind() == ErrorKind::NotFound => {
                let _ = fs::create_dir_all(&dir);
            }
            Err(_) => {}
        }
    }
    None
}

fn permit_is_stale(path: &Path) -> bool {
    if let Some(pid) = read_permit_pid(path)
        && let Some(alive) = pid_alive(pid)
    {
        return !alive;
    }
    let Ok(metadata) = fs::metadata(path) else {
        return false;
    };
    let Ok(modified) = metadata.modified() else {
        return false;
    };
    let Ok(age) = SystemTime::now().duration_since(modified) else {
        return false;
    };
    age > NETWORK_PERMIT_STALE_TTL
}

fn read_permit_pid(path: &Path) -> Option<u32> {
    let contents = fs::read_to_string(path).ok()?;
    for line in contents.lines() {
        let line = line.trim();
        if let Some(value) = line.strip_prefix("pid=")
            && let Ok(pid) = value.trim().parse::<u32>()
            && pid > 0
        {
            return Some(pid);
        }
    }
    None
}

#[cfg(unix)]
fn pid_alive(pid: u32) -> Option<bool> {
    let raw_pid = i32::try_from(pid).ok()?;
    match nix::sys::signal::kill(nix::unistd::Pid::from_raw(raw_pid), None) {
        Ok(()) => Some(true),
        Err(nix::errno::Errno::EPERM) => Some(true),
        Err(nix::errno::Errno::ESRCH) => Some(false),
        Err(_) => None,
    }
}

#[cfg(not(unix))]
fn pid_alive(_pid: u32) -> Option<bool> {
    None
}

fn acquire_network_permit() -> NetworkPermit {
    let mut waited = Duration::ZERO;
    let mut next_log = NETWORK_PERMIT_LOG_INTERVAL;
    loop {
        let limit = network_parallelism_limit();
        if let Some(file_permit) = try_acquire_file_permit(limit) {
            return NetworkPermit {
                _file_permit: file_permit,
            };
        }
        if waited >= next_log {
            info!(
                waited_ms = waited.as_millis(),
                limit, "waiting for test network permit"
            );
            next_log = next_log.saturating_add(NETWORK_PERMIT_LOG_INTERVAL);
        }
        std::thread::sleep(NETWORK_PERMIT_POLL_INTERVAL);
        waited = waited.saturating_add(NETWORK_PERMIT_POLL_INTERVAL);
    }
}

/// Network of peers
pub struct Network {
    env: Environment,
    peers: Vec<NetworkPeer>,

    block_time: Duration,
    commit_time: Duration,
    block_sync_gossip_period: Duration,
    peer_startup_timeout_override: Option<Duration>,
    consensus_profile: ConsensusBootstrapProfile,
    genesis_key_pair: KeyPair,

    genesis_isi: Vec<Vec<InstructionBox>>,
    genesis_post_topology_isi: Vec<Vec<InstructionBox>>,
    // Cache a single, deterministic genesis block per network instance to ensure
    // all peers that submit genesis use byte-for-byte identical content.
    cached_genesis: OnceLock<GenesisBlock>,
    config_layers: Vec<Table>,
    sumeragi_overrides: Vec<SumeragiParameter>,
    topology_entries: Vec<GenesisTopologyEntry>,
    auto_populate_trusted_peer_pops: bool,
    _permit: NetworkPermit,
}

impl Drop for Network {
    fn drop(&mut self) {
        let keep_tempdir = std::env::var_os(KEEP_TEMPDIR_ENV).is_some();
        if self.peers.iter().any(|peer| peer.is_running()) {
            let peers = self.peers.clone();
            let dir = self.env.dir.clone();
            let keep = keep_tempdir;
            std::thread::spawn(move || match runtime::Runtime::new() {
                Ok(rt) => rt.block_on(async {
                    for peer in peers {
                        if peer.is_running() {
                            let _ = peer.shutdown().await;
                        }
                    }
                    if keep {
                        info!(
                            dir = ?dir,
                            env = KEEP_TEMPDIR_ENV,
                            "preserving test network tempdir for debugging"
                        );
                    } else if let Err(err) = fs::remove_dir_all(&dir) {
                        warn!(
                            dir = ?dir,
                            ?err,
                            "failed to clean up test network tempdir"
                        );
                    }
                }),
                Err(err) => warn!(
                    dir = ?dir,
                    ?err,
                    "failed to create runtime for shutdown; peers may remain running"
                ),
            });
            return;
        }
        if keep_tempdir {
            info!(
                dir = ?self.env.dir,
                env = KEEP_TEMPDIR_ENV,
                "preserving test network tempdir for debugging"
            );
        } else if let Err(err) = fs::remove_dir_all(&self.env.dir) {
            warn!(
                dir = ?self.env.dir,
                ?err,
                "failed to clean up test network tempdir"
            );
        }
    }
}

#[derive(Debug, Clone)]
struct ConsensusBootstrapProfile {
    params: ConsensusGenesisParams,
    mode_tag: &'static str,
    bls_domain: &'static str,
    chain_id: ChainId,
    wire_proto_versions: Vec<u32>,
}

impl ConsensusBootstrapProfile {
    fn fingerprint(&self) -> [u8; 32] {
        compute_consensus_fingerprint_from_params(&self.chain_id, &self.params, self.mode_tag)
    }
}

impl Network {
    /// Path to the temporary directory holding configs and logs for this network.
    pub fn env_dir(&self) -> &Path {
        &self.env.dir
    }

    #[cfg(test)]
    fn consensus_bootstrap_profile(&self) -> ConsensusBootstrapProfile {
        self.consensus_profile.clone()
    }

    fn config_sumeragi_flag(&self, path: &[&str]) -> Option<bool> {
        self.config_layers.iter().rev().find_map(|layer| {
            let table = layer.get("sumeragi").and_then(Value::as_table)?;
            get_nested_value(table, path).and_then(Value::as_bool)
        })
    }

    fn parameter_flag<F>(&self, map: F) -> Option<bool>
    where
        F: Fn(&SumeragiParameter) -> Option<bool>,
    {
        self.sumeragi_overrides.iter().find_map(map)
    }

    fn log_startup_diagnostics(&self) {
        let config_da_enabled = self.config_sumeragi_flag(&["da", "enabled"]);
        let param_da_enabled = self.parameter_flag(|param| match param {
            SumeragiParameter::DaEnabled(value) => Some(*value),
            _ => None,
        });

        let handshake_fingerprint = self.consensus_profile.fingerprint();
        debug!(
            total_peers = self.peers.len(),
            consensus_da_enabled = self.consensus_profile.params.da_enabled,
            sumeragi_overrides = ?self.sumeragi_overrides,
            "sumeragi configuration snapshot prior to peer bootstrap"
        );

        info!(
            block_time = ?self.block_time,
            commit_time = ?self.commit_time,
            pipeline_time = ?self.pipeline_time(),
            block_sync_gossip_period = ?self.block_sync_gossip_period,
            config_da_enabled,
            param_da_enabled,
            handshake_mode = self.consensus_profile.mode_tag,
            handshake_bls_domain = self.consensus_profile.bls_domain,
            handshake_proto_versions = ?self.consensus_profile.wire_proto_versions,
            handshake_fingerprint = %format_args!("0x{}", hex_lower(&handshake_fingerprint)),
            "consensus bootstrap configuration"
        );

        if config_da_enabled != param_da_enabled {
            warn!(
                config_da_enabled,
                param_da_enabled,
                "Data availability enablement mismatch between config and parameters"
            );
        }
    }

    /// Add a peer to the network.
    pub fn add_peer(&mut self, peer: &NetworkPeer) {
        self.peers.push(peer.clone());
        if let Some(pop) = peer.genesis_pop() {
            self.topology_entries.push(pop);
        }
        self.cached_genesis = OnceLock::new();
    }

    /// Remove a peer from the network.
    pub fn remove_peer(&mut self, peer: &NetworkPeer) {
        self.peers.retain(|x| x != peer);
        if let Some(bls_pk) = peer.bls_public_key() {
            let bls_pk = bls_pk.clone();
            self.topology_entries
                .retain(|entry| entry.peer.public_key != bls_pk);
        }
        self.cached_genesis = OnceLock::new();
    }

    /// Access network peers
    pub fn peers(&self) -> &Vec<NetworkPeer> {
        &self.peers
    }

    /// Get a random peer in the network
    pub fn peer(&self) -> &NetworkPeer {
        let mut rng = rand::rng();
        self.peers
            .iter()
            .choose(&mut rng)
            .expect("there is at least one peer")
    }

    /// Access the environment of the network
    pub fn env(&self) -> &Environment {
        &self.env
    }

    /// Start all peers, waiting until they are up and have committed genesis (submitted by one of them).
    ///
    /// # Panics
    /// - If some peer was already started
    /// - If some peer exists early
    pub async fn start_all(&self) -> Result<&Self> {
        if self.peers.is_empty() {
            return Ok(self);
        }

        self.start_with_genesis_submitters([0]).await
    }

    /// Start peers with an explicit list of genesis submitter indices.
    ///
    /// Genesis submitters are started with a slight stagger to avoid overloading the
    /// network while still allowing multiple peers to race the initial submission.
    /// Replica peers (those not listed as genesis submitters) also ingest the same
    /// genesis block locally to guarantee deterministic bootstrap even if block sync
    /// support is unavailable.
    ///
    /// # Errors
    /// - If any submitter index is out of bounds.
    /// - If peer startup takes longer than [`Self::peer_startup_timeout`].
    pub async fn start_with_genesis_submitters<I>(&self, genesis_submitters: I) -> Result<&Self>
    where
        I: IntoIterator<Item = usize>,
    {
        let preflight = preflight_bind_addresses(
            self.peers
                .iter()
                .flat_map(|peer| [peer.p2p_address(), peer.api_address()]),
        );
        if let Err(err) = preflight {
            return Err(err).wrap_err("preflight bind failed for network peers");
        }

        // Ensure we resolve `irohad` once before spawning peers; caches for subsequent calls.
        // This may trigger a re-entrant build, so keep it off the async runtime threads.
        let _ = Program::Irohad.resolve_async().await?;

        let mut submitters: Vec<usize> = genesis_submitters.into_iter().collect();
        submitters.sort_unstable();
        submitters.dedup();

        if submitters.is_empty() && !self.peers.is_empty() {
            submitters.push(0);
        }

        if let Some(&idx) = submitters.iter().find(|&&idx| idx >= self.peers.len()) {
            return Err(eyre!(
                "genesis submitter index {idx} out of range for {} peers",
                self.peers.len()
            ));
        }

        let genesis_block = Arc::new(self.genesis());
        let genesis_order = Arc::new(submitters.clone());
        let genesis_lookup = Arc::new(
            submitters
                .iter()
                .enumerate()
                .map(|(pos, &idx)| (idx, pos))
                .collect::<HashMap<usize, usize>>(),
        );
        let startup_timeout = self.peer_startup_timeout();
        info!(
            total_peers = self.peers.len(),
            genesis_submitters = ?submitters,
            ?startup_timeout,
            "bootstrapping test network",
        );

        self.log_startup_diagnostics();

        let start_instant = Instant::now();

        let start_futures = self.peers.iter().enumerate().map(|(index, peer)| {
            let genesis_lookup = genesis_lookup.clone();
            let genesis_order = genesis_order.clone();
            let genesis_block = genesis_block.clone();
            async move {
                let stage = genesis_lookup.get(&index).copied();
                let mnemonic = peer.mnemonic().to_string();
                let role = if stage.is_some() {
                    "genesis"
                } else {
                    "replica"
                };

                info!(index, %mnemonic, role, "starting peer bootstrap");

                if let Some(stage_idx) = stage {
                    info!(
                        index,
                        %mnemonic,
                        role,
                        stage_idx,
                        total_submitters = genesis_order.len(),
                        "preparing genesis submitter",
                    );
                    if stage_idx > 0 {
                        let delay = Duration::from_millis(200)
                            .checked_mul(stage_idx as u32)
                            .unwrap_or(Duration::from_secs(u64::MAX));
                        info!(
                            index,
                            %mnemonic,
                            role,
                            stage_idx,
                            total_submitters = genesis_order.len(),
                            ?delay,
                            "staggering genesis submission",
                        );
                        if delay > Duration::ZERO {
                            tokio::time::sleep(delay).await;
                        }
                    }
                } else {
                    info!(
                        index,
                        %mnemonic,
                        role,
                        "providing replica with local genesis copy for bootstrap"
                    );
                }

                peer.start_checked(self.config_layers(), Some(genesis_block.as_ref()))
                    .await?;
                info!(
                    index,
                    %mnemonic,
                    role,
                    "peer started with genesis; waiting for block 1"
                );
                Self::wait_for_block_1_with_watchdog(peer, index, &mnemonic, role).await?;

                Ok::<(), color_eyre::Report>(())
            }
        });

        match timeout(
            startup_timeout,
            futures::future::try_join_all(start_futures),
        )
        .await
        {
            Ok(result) => {
                result?;
                self.verify_post_genesis_liveness().await?;
                info!(
                    elapsed = ?start_instant.elapsed(),
                    "all peers started and passed liveness guard"
                );
                Ok(self)
            }
            Err(_) => {
                let snapshot = self.startup_snapshot();
                warn!(?snapshot, "peer startup timed out");
                Err(eyre!(
                    "expected peers to start within timeout ({startup_timeout:?}); startup snapshot: [{}]",
                    Self::format_startup_snapshot(&snapshot),
                ))
            }
        }
    }

    async fn verify_post_genesis_liveness(&self) -> Result<()> {
        let window = post_genesis_liveness_window_env();
        if window == Duration::ZERO || self.peers.is_empty() {
            return Ok(());
        }

        let futures = self.peers.iter().enumerate().map(|(index, peer)| {
            let mnemonic = peer.mnemonic().to_string();
            let stdout = peer.latest_stdout_log_path();
            let stderr = peer.latest_stderr_log_path();
            let events = peer.events();
            async move {
                if let Some(kind) = detect_peer_termination(events, window).await {
                    Err(eyre!(
                        "peer {index} ({mnemonic}) terminated within {window:?} post-genesis window ({kind:?}); stdout={stdout:?} stderr={stderr:?}"
                    ))
                } else {
                    Ok::<(), color_eyre::Report>(())
                }
            }
        });

        futures::future::try_join_all(futures).await?;
        Ok(())
    }

    async fn wait_for_block_1_with_watchdog(
        peer: &NetworkPeer,
        index: usize,
        mnemonic: &str,
        role: &str,
    ) -> Result<()> {
        let mut latest_status: Option<iroha::client::Status> = None;
        let status_timeout = {
            let configured = client_status_timeout_env();
            if configured == Duration::ZERO {
                GENESIS_BLOCK_LOG_INTERVAL
            } else {
                configured.min(GENESIS_BLOCK_LOG_INTERVAL)
            }
        };
        let mut poll = tokio::time::interval(Duration::from_millis(250));
        poll.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        let mut watchdog = tokio::time::interval(GENESIS_BLOCK_LOG_INTERVAL);
        watchdog.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        let mut elapsed = Duration::ZERO;

        loop {
            if peer.has_committed_block(1) {
                info!(
                    index,
                    %mnemonic,
                    role,
                    waited = ?elapsed,
                    "observed block 1 via storage before status polling"
                );
                return Ok(());
            }
            tokio::select! {
                _ = poll.tick() => {
                    match tokio::time::timeout(status_timeout, peer.status()).await {
                        Ok(Ok(status)) => {
                            if status.blocks >= 1 {
                                info!(
                                    index,
                                    %mnemonic,
                                    role,
                                    waited = ?elapsed,
                                    status_blocks = status.blocks,
                                    status_blocks_non_empty = status.blocks_non_empty,
                                    "observed block 1 via status polling"
                                );
                                return Ok(());
                            }
                            latest_status = Some(status);
                            if peer.has_committed_block(1) {
                                info!(
                                    index,
                                    %mnemonic,
                                    role,
                                    waited = ?elapsed,
                                    "observed block 1 via storage inspection"
                                );
                                return Ok(());
                            }
                        }
                        Ok(Err(error)) => {
                            latest_status = None;
                            let stdout_log = peer.latest_stdout_log_path();
                            let stderr_log = peer.latest_stderr_log_path();
                            warn!(
                                index,
                                %mnemonic,
                                role,
                                ?error,
                                ?stdout_log,
                                ?stderr_log,
                                "status query failed while waiting for block 1"
                            );
                            if peer.has_committed_block(1) {
                                info!(
                                    index,
                                    %mnemonic,
                                    role,
                                    waited = ?elapsed,
                                    "observed block 1 via storage after status failure"
                                );
                                return Ok(());
                            }
                        }
                        Err(_) => {
                            latest_status = None;
                            let stdout_log = peer.latest_stdout_log_path();
                            let stderr_log = peer.latest_stderr_log_path();
                            warn!(
                                index,
                                %mnemonic,
                                role,
                                ?status_timeout,
                                ?stdout_log,
                                ?stderr_log,
                                "status query timed out while waiting for block 1"
                            );
                            if peer.has_committed_block(1) {
                                info!(
                                    index,
                                    %mnemonic,
                                    role,
                                    waited = ?elapsed,
                                    "observed block 1 via storage after status timeout"
                                );
                                return Ok(());
                            }
                        }
                    }
                }
                _ = watchdog.tick() => {
                    elapsed += GENESIS_BLOCK_LOG_INTERVAL;
                    if let Some(status) = &latest_status {
                        warn!(
                            index,
                            %mnemonic,
                            role,
                            waited = ?elapsed,
                            status_blocks = status.blocks,
                            status_blocks_non_empty = status.blocks_non_empty,
                            status_queue = status.queue_size,
                            status_view_changes = status.view_changes,
                            "still waiting for block 1 after genesis submission"
                        );
                    } else if peer.has_committed_block(1) {
                        info!(
                            index,
                            %mnemonic,
                            role,
                            waited = ?elapsed,
                            "observed block 1 via storage while status polling failed"
                        );
                        return Ok(());
                    } else {
                        warn!(
                            index,
                            %mnemonic,
                            role,
                            waited = ?elapsed,
                            "still waiting for block 1; no status snapshot available"
                        );
                    }
                }
            }
        }
    }

    /// Pipeline time of the network.
    ///
    /// Is relevant only if users haven't submitted [`SumeragiParameter`] changing it.
    /// Users should do it through a network method (which hasn't been necessary yet).
    pub fn pipeline_time(&self) -> Duration {
        self.block_time + self.commit_time
    }

    /// Block gossip period configured for the network overlay.
    pub fn block_sync_gossip_period(&self) -> Duration {
        self.block_sync_gossip_period
    }

    pub fn sync_timeout(&self) -> Duration {
        sync_timeout_env()
    }

    pub fn peer_startup_timeout(&self) -> Duration {
        let base = self
            .peer_startup_timeout_override
            .unwrap_or_else(peer_start_timeout_env);
        let peers = self.peers.len() as u128;
        if peers == 0 {
            return base;
        }

        // Allow at least 60 seconds per peer by default to accommodate slower DA/RBC startup
        // under host contention (e.g., multiple full peers bootstrapping simultaneously).
        let dynamic_secs = u128::from(PEER_STARTUP_TIMEOUT_PER_PEER_SECS)
            .saturating_mul(peers)
            .min(u128::from(u64::MAX));
        let dynamic = Duration::from_secs(dynamic_secs as u64);

        base.max(dynamic)
    }

    /// Capture a human-readable snapshot of the current startup state for all peers.
    pub fn startup_snapshot(&self) -> Vec<PeerStartupState> {
        self.peers
            .iter()
            .enumerate()
            .map(|(index, peer)| peer.startup_state(index))
            .collect()
    }

    fn format_startup_snapshot(snapshot: &[PeerStartupState]) -> String {
        snapshot
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// Get a client for the first peer in the network.
    pub fn client(&self) -> Client {
        self.peers
            .first()
            .expect("there is at least one peer")
            .client()
    }

    /// Chain ID of the network
    pub fn chain_id(&self) -> ChainId {
        config::chain_id()
    }

    /// Torii URLs for all peers in the network.
    pub fn torii_urls(&self) -> Vec<String> {
        self.peers.iter().map(NetworkPeer::torii_url).collect()
    }

    /// Base configuration of all peers.
    ///
    /// Includes `trusted_peers` parameter, containing all currently present peers.
    pub fn config_layers(&self) -> impl Iterator<Item = Cow<'_, Table>> {
        self.config_layers_with_additional_peers([])
    }

    /// Base configuration including the current peers and any additional peers provided.
    ///
    /// Useful for bootstrapping peers that were registered after the network was built by
    /// threading their PoP into `trusted_peers_pop` so they are not excluded from consensus.
    pub fn config_layers_with_additional_peers<'a>(
        &'a self,
        additional_peers: impl IntoIterator<Item = &'a NetworkPeer>,
    ) -> impl Iterator<Item = Cow<'a, Table>> {
        let extra: Vec<&NetworkPeer> = additional_peers.into_iter().collect();
        let mut trusted = self.trusted_peers();
        for peer in &extra {
            let _ = trusted.push(Peer::new(peer.p2p_address(), peer.network_peer_id()));
        }

        // Yield `trusted_peers` first so that any caller-provided layers can
        // reliably override it (e.g., relay/proxy topologies). Later layers in
        // `extends` win during config resolution.
        let trusted_peers: Vec<String> = trusted
            .iter()
            .map(|peer| format!("{}@{}", peer.id(), peer.address().to_literal()))
            .collect();

        let mut base_layer = Table::new().write(["trusted_peers"], trusted_peers);
        // Allow local tooling to bypass Torii pre-auth rate limits. Tests poll status
        // endpoints aggressively while waiting for block 1; without this allowlist the
        // pre-auth ban can trigger and break client traffic.
        base_layer = base_layer.write(
            ["torii", "preauth_allow_cidrs"],
            vec!["127.0.0.1/32", "::1/128"],
        );

        if self.auto_populate_trusted_peer_pops {
            let mut trusted_peers_pop: Vec<Value> = Vec::new();
            let mut seen = HashSet::new();

            for peer in self.peers.iter().chain(extra.into_iter()) {
                let (Some(bls_pk), Some(pop_bytes)) = (peer.bls_public_key(), peer.bls_pop())
                else {
                    continue;
                };
                if !seen.insert(bls_pk.clone()) {
                    continue;
                }

                let mut pop_entry = Table::new();
                pop_entry.insert("public_key".into(), Value::String(bls_pk.to_string()));
                pop_entry.insert(
                    "pop_hex".into(),
                    Value::String(format!("0x{}", hex_lower(pop_bytes))),
                );
                trusted_peers_pop.push(Value::Table(pop_entry));
            }
            if !trusted_peers_pop.is_empty() {
                base_layer =
                    base_layer.write(["trusted_peers_pop"], Value::Array(trusted_peers_pop));
            }
        }

        Some(Cow::Owned(base_layer))
            .into_iter()
            .chain(self.config_layers.iter().map(Cow::Borrowed))
    }

    /// Network genesis block.
    ///
    /// It uses the basic [`genesis_factory`] with [`Self::genesis_isi`],
    /// post-topology bootstrap instructions, and the network peer topology.
    pub fn genesis(&self) -> GenesisBlock {
        let config_layers: Vec<Table> = self.config_layers().map(Cow::into_owned).collect();
        let actual_config = self
            .peers
            .first()
            .and_then(|peer| resolve_actual_config(peer, &config_layers));
        let da_proof_policies = actual_config
            .as_ref()
            .map(|config| iroha_core::da::proof_policy_bundle(&config.nexus.lane_config));
        let nexus_config = actual_config.map(|config| config.nexus);
        let consensus_handshake_meta = consensus_handshake_parameter(&self.consensus_profile);

        if let Some(cached_genesis) = self.cached_genesis.get() {
            if genesis_has_consensus_handshake(cached_genesis, &consensus_handshake_meta) {
                return cached_genesis.clone();
            }
        }

        let genesis = config::genesis_with_keypair_and_post_topology_with_policies(
            self.genesis_isi.clone(),
            self.genesis_post_topology_isi.clone(),
            self.peers.iter().map(NetworkPeer::id).collect(),
            self.topology_entries.clone(),
            self.genesis_key_pair.clone(),
            da_proof_policies,
            nexus_config,
            Some(consensus_handshake_meta),
        );
        let _ = self.cached_genesis.set(genesis.clone());
        genesis
    }

    /// Genesis block instructions grouped by transaction
    pub fn genesis_isi(&self) -> &Vec<Vec<InstructionBox>> {
        &self.genesis_isi
    }

    /// BLS Proof-of-Possession entries for the current peer topology.
    pub fn topology_entries(&self) -> &[GenesisTopologyEntry] {
        &self.topology_entries
    }

    /// Shutdown running peers
    pub async fn shutdown(&self) -> &Self {
        self.peers
            .iter()
            .filter(|peer| peer.is_running())
            .map(|peer| peer.shutdown())
            .collect::<FuturesUnordered<_>>()
            .collect::<Vec<_>>()
            .await;
        self
    }

    fn trusted_peers(&self) -> UniqueVec<Peer> {
        self.peers
            .iter()
            .map(|x| Peer::new(x.p2p_address(), x.network_peer_id()))
            .collect()
    }

    /// Resolves when all _running_ peers have at least N blocks (non-empty in current policy)
    /// # Errors
    /// If this doesn't happen within a timeout.
    pub async fn ensure_blocks(&self, height: u64) -> Result<&Self> {
        match self
            .ensure_blocks_with(BlockHeight::predicate_total(height))
            .await
        {
            Ok(_) => {}
            Err(err) => {
                warn!(%err, %height, "block sync predicate failed; falling back to status polling");
                self.wait_for_blocks_via_status(height).await?;
            }
        }

        info!(%height, "network sync height");

        Ok(self)
    }

    pub async fn ensure_blocks_with<F: Fn(BlockHeight) -> bool>(&self, f: F) -> Result<&Self> {
        // Fast path: if storage already shows the required height for all running peers,
        // skip the async watchers to avoid long waits when status polling lags behind.
        let storage_satisfied = self.peers.iter().filter(|p| p.is_running()).all(|peer| {
            detect_block_height_from_storage(&peer.kura_store_dir(), 0)
                .map(&f)
                .unwrap_or(false)
        });
        if storage_satisfied {
            return Ok(self);
        }

        let snapshot_on_failure = || self.startup_snapshot();
        timeout(
            self.sync_timeout(),
            once_blocks_sync(self.peers.iter().filter(|x| x.is_running()), &f),
        )
        .await
        .map_err(|_| {
            eyre!(
                "Network overall height did not pass given predicate within timeout; env_dir={}, snapshot={}",
                self.env.dir.display(),
                Self::format_startup_snapshot(&snapshot_on_failure())
            )
        })?
        .map_err(|err| {
            eyre!(
                "block sync predicate failed; env_dir={}, err={err}",
                self.env.dir.display()
            )
        })?;

        Ok(self)
    }

    async fn wait_for_blocks_via_status(&self, height: u64) -> Result<()> {
        let deadline = Instant::now() + self.sync_timeout();
        loop {
            let mut satisfied = true;
            for peer in self.peers.iter().filter(|peer| peer.is_running()) {
                match peer.status().await {
                    Ok(status) => {
                        if status.blocks_non_empty < height {
                            satisfied = false;
                            break;
                        }
                    }
                    Err(err) => {
                        // Fall back to on-disk observation so scenarios can progress even if Torii
                        // is slow to accept HTTP connections.
                        if let Some(snapshot) =
                            detect_block_height_from_storage(&peer.dir.join("storage"), 0)
                        {
                            if snapshot.non_empty < height {
                                satisfied = false;
                                break;
                            }
                        } else {
                            satisfied = false;
                            if !peer.is_running() {
                                let stdout = peer.latest_stdout_log_path();
                                let stderr = peer.latest_stderr_log_path();
                                return Err(eyre!(
                                    "peer {} not running while waiting for block {height}; env_dir={}, stdout={stdout:?} stderr={stderr:?}, err={err}",
                                    peer.mnemonic(),
                                    self.env.dir.display()
                                ));
                            }
                            warn!(
                                ?err,
                                mnemonic = peer.mnemonic(),
                                "status poll failed while waiting for block {height}"
                            );
                            break;
                        }
                    }
                }
            }
            if satisfied {
                info!(%height, "network sync height via status");
                return Ok(());
            }
            if Instant::now() >= deadline {
                break;
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
        Err(eyre!(
            "expected to reach height={height}; env_dir={}",
            self.env.dir.display()
        ))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TerminationKind {
    Terminated,
    Killed,
    EventStreamClosed,
}

async fn detect_peer_termination(
    mut events: broadcast::Receiver<PeerLifecycleEvent>,
    window: Duration,
) -> Option<TerminationKind> {
    if window == Duration::ZERO {
        return None;
    }

    let timer = tokio::time::sleep(window);
    tokio::pin!(timer);

    loop {
        tokio::select! {
            _ = &mut timer => return None,
            event = events.recv() => match event {
                Ok(PeerLifecycleEvent::Terminated { .. }) => return Some(TerminationKind::Terminated),
                Ok(PeerLifecycleEvent::Killed) => return Some(TerminationKind::Killed),
                Ok(_) => continue,
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(broadcast::error::RecvError::Closed) => return Some(TerminationKind::EventStreamClosed),
            }
        }
    }
}

/// Determines how [`NetworkBuilder`] configures [`SmartContractParameter::Fuel`] in the genesis.
#[derive(Default)]
pub enum IvmFuelConfig {
    /// Do not set anything, i.e. let Iroha use its default value
    #[default]
    Unset,
    /// Set to a specific value
    Value(NonZero<u64>),
    /// Determine automatically based on the IVM samples build profile
    /// (received from [`iroha_test_samples::load_ivm_build_profile`]).
    ///
    /// If the profile is not optimized, the fuel will be increased, otherwise the same as
    /// [`IvmFuelConfig::Unset`].
    Auto,
}

/// Diagnostic snapshot describing the startup state of a peer.
#[derive(Debug, Clone)]
pub struct PeerStartupState {
    /// Index of the peer within the network builder order.
    pub index: usize,
    /// Mnemonic-derived human readable peer label.
    pub mnemonic: String,
    /// Whether the peer process is still running.
    pub is_running: bool,
    /// Latest observed block height (if any).
    pub last_block: Option<BlockHeight>,
    /// Latest log snapshot information (stdout/stderr paths and previews).
    pub logs: PeerLogSnapshot,
    /// Most recent `/status` response snapshot, if the peer responded.
    pub status_snapshot: Option<PeerStatusSnapshot>,
    /// Most recent `/status` error captured while polling for readiness.
    pub status_error: Option<String>,
    /// Unix timestamp in milliseconds when the status snapshot (success or error) was recorded.
    pub status_unix_timestamp_ms: Option<u128>,
    /// Snapshot of the peer's Kura storage layout.
    pub storage: PeerStorageSnapshot,
}

impl PeerStartupState {
    /// Whether the peer reported a status (i.e., the server started).
    pub fn server_started(&self) -> bool {
        self.last_block.is_some()
    }
}

impl fmt::Display for PeerStartupState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let block = self
            .last_block
            .map(|height| format!("total={} non_empty={}", height.total, height.non_empty))
            .unwrap_or_else(|| "none".to_string());

        write!(
            f,
            "peer#{idx}({name}) running={running} server_started={started} last_block={block}",
            idx = self.index,
            name = self.mnemonic,
            running = self.is_running,
            started = self.server_started(),
            block = block,
        )?;

        let formatted_ts = self
            .status_unix_timestamp_ms
            .map(|ms| format!("{ms}ms"))
            .unwrap_or_else(|| "unknown".to_string());
        if let Some(snapshot) = &self.status_snapshot {
            write!(
                f,
                "; status=ok(blocks={} non_empty={} queue={} view_changes={} peers={} txs={}/{} da_reschedule_total={})@{formatted_ts}",
                snapshot.blocks,
                snapshot.blocks_non_empty,
                snapshot.queue_size,
                snapshot.view_changes,
                snapshot.peers,
                snapshot.txs_approved,
                snapshot.txs_rejected,
                snapshot.da_reschedule_total,
            )?;
        } else if let Some(error) = &self.status_error {
            write!(
                f,
                "; status=error(\"{}\")@{formatted_ts}",
                sanitize_preview_for_display(error)
            )?;
        } else {
            write!(f, "; status=unavailable")?;
        }

        let stdout_log = self
            .logs
            .stdout_log
            .as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "none".to_string());
        let stderr_log = self
            .logs
            .stderr_log
            .as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "none".to_string());

        write!(
            f,
            "; logs=stdout={stdout_log} stderr={stderr_log} stderr_run={:?}",
            self.logs.stderr_run_id
        )?;

        if let Some(preview) = &self.logs.stderr_preview {
            write!(
                f,
                " stderr_tail=\"{}\" tail_lines={:?} total_lines={:?} truncated={}",
                sanitize_preview_for_display(preview),
                self.logs.stderr_preview_line_count,
                self.logs.stderr_total_lines,
                self.logs.stderr_truncated
            )?;
        }

        write!(
            f,
            "; storage=exists={} has_block1={} pipeline={:?} blocks={:?}",
            self.storage.store_exists,
            self.storage.has_block_1_artifact,
            self.storage.pipeline_entries,
            self.storage.blocks_entries,
        )
    }
}

/// Snapshot of a peer's log state.
#[derive(Debug, Clone, Default)]
pub struct PeerLogSnapshot {
    /// Path to the latest stdout log.
    pub stdout_log: Option<PathBuf>,
    /// Path to the latest stderr log (if the peer already exited).
    pub stderr_log: Option<PathBuf>,
    /// Preview of the stderr tail captured from the live stream.
    pub stderr_preview: Option<String>,
    /// Number of lines in the captured preview.
    pub stderr_preview_line_count: Option<usize>,
    /// Total number of stderr lines captured so far.
    pub stderr_total_lines: Option<usize>,
    /// Whether the preview was truncated.
    pub stderr_truncated: bool,
    /// Run identifier associated with the stderr preview.
    pub stderr_run_id: Option<usize>,
}

/// Snapshot of the last `/status` response observed while starting the peer.
#[derive(Debug, Clone, Default)]
pub struct PeerStatusSnapshot {
    pub peers: u64,
    pub blocks: u64,
    pub blocks_non_empty: u64,
    pub commit_time_ms: u64,
    pub queue_size: u64,
    pub view_changes: u32,
    pub txs_approved: u64,
    pub txs_rejected: u64,
    pub da_reschedule_total: u64,
}

impl From<&Status> for PeerStatusSnapshot {
    fn from(value: &Status) -> Self {
        Self {
            peers: value.peers,
            blocks: value.blocks,
            blocks_non_empty: value.blocks_non_empty,
            commit_time_ms: value.commit_time_ms,
            queue_size: value.queue_size,
            view_changes: value.view_changes,
            txs_approved: value.txs_approved,
            txs_rejected: value.txs_rejected,
            da_reschedule_total: value.da_reschedule_total,
        }
    }
}

/// Snapshot of the peer's Kura directory layout.
#[derive(Debug, Clone)]
pub struct PeerStorageSnapshot {
    pub kura_dir: PathBuf,
    pub store_exists: bool,
    pub has_block_1_artifact: bool,
    pub pipeline_entries: Vec<String>,
    pub blocks_entries: Vec<String>,
}

impl PeerStorageSnapshot {
    fn capture(kura_dir: PathBuf, has_block_1_artifact: bool) -> Self {
        let store_exists = kura_dir.exists();
        let pipeline_entries = pipeline_dirs(&kura_dir)
            .into_iter()
            .find(|dir| dir.exists())
            .map(|dir| snapshot_dir_entries(&dir, STORAGE_LISTING_LIMIT))
            .unwrap_or_default();
        let blocks_entries = snapshot_dir_entries(&kura_dir.join("blocks"), STORAGE_LISTING_LIMIT);
        Self {
            kura_dir,
            store_exists,
            has_block_1_artifact,
            pipeline_entries,
            blocks_entries,
        }
    }
}

#[derive(Debug, Default)]
struct LiveStderrState {
    run_id: Option<usize>,
    buffer: String,
}

impl LiveStderrState {
    fn reset(&mut self, run_id: usize) {
        self.run_id = Some(run_id);
        self.buffer.clear();
    }

    fn push_line(&mut self, line: &str) {
        self.buffer.push_str(line);
        self.buffer.push('\n');
    }
}

#[derive(Debug, Clone, Default)]
struct PeerStartupProbe {
    last_status: Option<PeerStatusSnapshot>,
    last_status_error: Option<String>,
    last_status_unix_ms: Option<u128>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum StatusSource {
    Http,
    Storage,
}

#[derive(Debug, Default)]
struct HttpStartGate {
    seen_http: bool,
}

impl HttpStartGate {
    fn http_seen(&self) -> bool {
        self.seen_http
    }

    /// Returns true exactly once, on the first HTTP-derived status observation.
    fn on_status(&mut self, source: StatusSource) -> bool {
        if self.seen_http {
            false
        } else if matches!(source, StatusSource::Http) {
            self.seen_http = true;
            true
        } else {
            false
        }
    }
}

fn snapshot_dir_entries(path: &Path, limit: usize) -> Vec<String> {
    let Ok(read_dir) = fs::read_dir(path) else {
        return Vec::new();
    };
    let mut names: Vec<String> = read_dir
        .flatten()
        .filter_map(|entry| entry.file_name().into_string().ok())
        .collect();
    names.sort();
    if names.len() > limit {
        let omitted = names.len() - limit;
        names.truncate(limit);
        names.push(format!("(+{omitted} more)"));
    }
    names
}

fn snapshot_snippet(value: &str) -> String {
    let mut buf = String::new();
    for (idx, ch) in value.chars().enumerate() {
        if idx >= SNAPSHOT_MESSAGE_SNIPPET_MAX_CHARS {
            buf.push('…');
            break;
        }
        buf.push(ch);
    }
    buf
}

fn sanitize_preview_for_display(value: &str) -> String {
    snapshot_snippet(&value.replace('\n', "\\n"))
}

async fn drain_log_lines<R, F>(
    output: R,
    mut file: File,
    mut fatal_rx: watch::Receiver<bool>,
    is_running: Arc<AtomicBool>,
    mut on_line: F,
    ready_notify: Option<Arc<Notify>>,
    label: &'static str,
) where
    R: AsyncRead + Unpin,
    F: FnMut(&str),
{
    let mut lines = BufReader::new(output).lines();
    loop {
        if *fatal_rx.borrow() || !is_running.load(Ordering::Relaxed) {
            break;
        }
        tokio::select! {
            line = lines.next_line() => match line {
                Ok(Some(line)) => {
                    on_line(&line);
                    if let Err(err) = file.write_all(line.as_bytes()).await {
                        error!(?err, log = label, "writing log line failed");
                        break;
                    }
                    if let Err(err) = file.write_all(b"\n").await {
                        error!(?err, log = label, "writing log newline failed");
                        break;
                    }
                    if let Err(err) = file.flush().await {
                        error!(?err, log = label, "flushing log file failed");
                        break;
                    }
                }
                Ok(None) => break,
                Err(err) => {
                    error!(?err, log = label, "reading log stream failed");
                    break;
                }
            },
            changed = fatal_rx.changed() => {
                if changed.is_err() {
                    break;
                }
                if *fatal_rx.borrow() {
                    break;
                }
            },
        }
    }
    if let Err(err) = file.flush().await {
        error!(?err, log = label, "flushing log file failed");
    }
    if let Some(notify) = ready_notify {
        notify.notify_waiters();
    }
}

/// Builder of [`Network`]
pub struct NetworkBuilder {
    env: Environment,
    n_peers: usize,
    config_layers: Vec<Table>,
    pipeline_time: Option<Duration>,
    peer_startup_timeout: Option<Duration>,
    ivm_fuel: IvmFuelConfig,
    genesis_isi: Vec<Vec<InstructionBox>>,
    genesis_post_topology_isi: Vec<Vec<InstructionBox>>,
    custom_genesis: Option<GenesisBuilderFn>,
    seed: Option<String>,
    genesis_key_pair: KeyPair,
    block_sync_gossip_period: Duration,
    sumeragi_parameters: Vec<SumeragiParameter>,
    sumeragi_da_enabled: Option<bool>,
    auto_populate_trusted_peer_pops: bool,
    npos_genesis_bootstrap_stake: Option<u64>,
}

fn bool_env_override(key: &str) -> Option<bool> {
    match std::env::var(key) {
        Ok(value) => {
            let trimmed = value.trim();
            if trimmed.eq_ignore_ascii_case("true") || trimmed == "1" {
                Some(true)
            } else if trimmed.eq_ignore_ascii_case("false") || trimmed == "0" {
                Some(false)
            } else {
                warn!(
                    key,
                    value = %value,
                    "ignoring invalid boolean environment override"
                );
                None
            }
        }
        Err(std::env::VarError::NotPresent) => None,
        Err(std::env::VarError::NotUnicode(_)) => {
            warn!(key, "ignoring non-unicode boolean environment override");
            None
        }
    }
}

fn merge_tables(dst: &mut Table, src: &Table) {
    for (key, value) in src {
        match value {
            Value::Table(src_table) => match dst.entry(key.clone()) {
                Entry::Occupied(mut entry) => {
                    if let Value::Table(dst_table) = entry.get_mut() {
                        merge_tables(dst_table, src_table);
                    } else {
                        entry.insert(Value::Table(src_table.clone()));
                    }
                }
                Entry::Vacant(entry) => {
                    entry.insert(Value::Table(src_table.clone()));
                }
            },
            _ => {
                dst.insert(key.clone(), value.clone());
            }
        }
    }
}

fn trusted_peers_layer_for_parse(
    peers: &[NetworkPeer],
    auto_populate_trusted_peer_pops: bool,
) -> Table {
    let trusted_peers: Vec<String> = peers
        .iter()
        .map(|peer| {
            format!(
                "{}@{}",
                peer.network_peer_id(),
                peer.p2p_address().to_literal()
            )
        })
        .collect();
    let mut base_layer = Table::new().write(["trusted_peers"], trusted_peers);

    if auto_populate_trusted_peer_pops {
        let mut trusted_peers_pop: Vec<Value> = Vec::new();
        let mut seen = HashSet::new();

        for peer in peers {
            let (Some(bls_pk), Some(pop_bytes)) = (peer.bls_public_key(), peer.bls_pop()) else {
                continue;
            };
            if !seen.insert(bls_pk.clone()) {
                continue;
            }

            let mut pop_entry = Table::new();
            pop_entry.insert("public_key".into(), Value::String(bls_pk.to_string()));
            pop_entry.insert(
                "pop_hex".into(),
                Value::String(format!("0x{}", hex_lower(pop_bytes))),
            );
            trusted_peers_pop.push(Value::Table(pop_entry));
        }

        if !trusted_peers_pop.is_empty() {
            base_layer = base_layer.write(["trusted_peers_pop"], Value::Array(trusted_peers_pop));
        }
    }

    base_layer
}

// Deterministic BLS keypair/PoP so consensus validation doesn't reject profile detection defaults.
const SORA_PROFILE_BLS_PUBLIC_KEY: &str = "ea01309060D021340617E9554CCBC2CF3CC3DB922A9BA323ABDF7C271FCC6EF69BE7A8DEBCA7D9E96C0F0089ABA22CDAADE4A2";
const SORA_PROFILE_BLS_PRIVATE_KEY: &str =
    "8926201CA347641228C3B79AA43839DEDC85FA51C0E8B9B6A00F6B0D6B0423E902973F";
static SORA_PROFILE_BLS_KEYPAIR: OnceLock<KeyPair> = OnceLock::new();
static SORA_PROFILE_BLS_POP_HEX: OnceLock<String> = OnceLock::new();
const SORA_PROFILE_STREAM_PUBLIC_KEY: &str =
    "ed01201C61FAF8FE94E253B93114240394F79A607B7FA55F9E5A41EBEC74B88055768B";
const SORA_PROFILE_STREAM_PRIVATE_KEY: &str =
    "802620282ED9F3CF92811C3818DBC4AE594ED59DC1A2F78E4241E31924E101D6B1FB83";
static SORA_PROFILE_STREAM_KEYPAIR: OnceLock<KeyPair> = OnceLock::new();

fn sora_profile_bls_pop_hex() -> &'static str {
    SORA_PROFILE_BLS_POP_HEX.get_or_init(|| {
        let bls_keypair = SORA_PROFILE_BLS_KEYPAIR.get_or_init(|| {
            let public_key: PublicKey = SORA_PROFILE_BLS_PUBLIC_KEY
                .parse()
                .expect("sora profile BLS public key should parse");
            let private_key: PrivateKey = SORA_PROFILE_BLS_PRIVATE_KEY
                .parse()
                .expect("sora profile BLS private key should parse");
            KeyPair::new(public_key, private_key).expect("sora profile BLS keypair should match")
        });
        let pop = iroha_crypto::bls_normal_pop_prove(bls_keypair.private_key())
            .expect("sora profile BLS PoP should generate");
        format!("0x{}", hex_lower(&pop))
    })
}

fn ensure_sora_profile_trusted_peer_pop(table: &mut Table) {
    let mut pop_entry = Table::new();
    pop_entry.insert(
        "public_key".into(),
        Value::String(SORA_PROFILE_BLS_PUBLIC_KEY.to_string()),
    );
    pop_entry.insert(
        "pop_hex".into(),
        Value::String(sora_profile_bls_pop_hex().to_string()),
    );
    let entry = Value::Table(pop_entry);

    match table.get_mut("trusted_peers_pop") {
        Some(Value::Array(entries)) => {
            let has_entry = entries.iter().any(|entry| {
                entry
                    .as_table()
                    .and_then(|table| table.get("public_key"))
                    .and_then(Value::as_str)
                    .is_some_and(|pk| pk == SORA_PROFILE_BLS_PUBLIC_KEY)
            });
            if !has_entry {
                entries.push(entry);
            }
        }
        None => {
            table.insert("trusted_peers_pop".into(), Value::Array(vec![entry]));
        }
        Some(_) => {}
    }
}

fn sora_profile_detection_defaults() -> Table {
    let bls_keypair = SORA_PROFILE_BLS_KEYPAIR.get_or_init(|| {
        let public_key: PublicKey = SORA_PROFILE_BLS_PUBLIC_KEY
            .parse()
            .expect("sora profile BLS public key should parse");
        let private_key: PrivateKey = SORA_PROFILE_BLS_PRIVATE_KEY
            .parse()
            .expect("sora profile BLS private key should parse");
        KeyPair::new(public_key, private_key).expect("sora profile BLS keypair should match")
    });
    let streaming_keypair = SORA_PROFILE_STREAM_KEYPAIR.get_or_init(|| {
        let public_key: PublicKey = SORA_PROFILE_STREAM_PUBLIC_KEY
            .parse()
            .expect("sora profile streaming public key should parse");
        let private_key: PrivateKey = SORA_PROFILE_STREAM_PRIVATE_KEY
            .parse()
            .expect("sora profile streaming private key should parse");
        KeyPair::new(public_key, private_key).expect("sora profile streaming keypair should match")
    });
    let p2p_literal = socket_addr!(127.0.0.1:1337).to_literal();
    let torii_literal = socket_addr!(127.0.0.1:8080).to_literal();
    let mut table = Table::new()
        .write("chain", chain_id().to_string())
        .write("public_key", bls_keypair.public_key().to_string())
        .write(
            "private_key",
            ExposedPrivateKey(bls_keypair.private_key().clone()).to_string(),
        )
        .write(
            ["streaming", "identity_public_key"],
            streaming_keypair.public_key().to_string(),
        )
        .write(
            ["streaming", "identity_private_key"],
            ExposedPrivateKey(streaming_keypair.private_key().clone()).to_string(),
        )
        .write(["network", "address"], p2p_literal.clone())
        .write(["network", "public_address"], p2p_literal)
        .write(["torii", "address"], torii_literal)
        .write(
            ["genesis", "public_key"],
            SAMPLE_GENESIS_ACCOUNT_KEYPAIR.public_key().to_string(),
        );
    ensure_sora_profile_trusted_peer_pop(&mut table);
    table
}

fn apply_streaming_identity_defaults_for_detection(merged: &mut Table) {
    // Profile detection does not depend on streaming identity, but config parsing does.
    // Always force the deterministic Ed25519 keys so BLS identities never trip validation here.
    let mut streaming = match merged.remove("streaming") {
        Some(Value::Table(table)) => table,
        _ => Table::new(),
    };
    streaming.insert(
        "identity_public_key".into(),
        Value::String(SORA_PROFILE_STREAM_PUBLIC_KEY.to_string()),
    );
    streaming.insert(
        "identity_private_key".into(),
        Value::String(SORA_PROFILE_STREAM_PRIVATE_KEY.to_string()),
    );
    merged.insert("streaming".into(), Value::Table(streaming));
}

fn merged_sora_profile_detection_config(config_layers: &[Table]) -> Table {
    let mut merged = sora_profile_detection_defaults();
    for layer in config_layers {
        merge_tables(&mut merged, layer);
    }
    apply_streaming_identity_defaults_for_detection(&mut merged);
    ensure_sora_profile_trusted_peer_pop(&mut merged);
    merged
}

fn raw_nexus_overrides(table: &Table) -> bool {
    let Some(nexus) = table.get("nexus").and_then(Value::as_table) else {
        return false;
    };
    if nexus.contains_key("lane_catalog") || nexus.contains_key("dataspace_catalog") {
        return true;
    }
    if let Some(policy) = nexus.get("routing_policy") {
        let Some(policy) = policy.as_table() else {
            return true;
        };
        let default_lane =
            i64::from(iroha_config::parameters::defaults::nexus::DEFAULT_ROUTING_LANE_INDEX);
        let default_lane_override = match policy.get("default_lane") {
            None => false,
            Some(value) => value.as_integer().map_or(true, |lane| lane != default_lane),
        };
        let default_dataspace_override = match policy.get("default_dataspace") {
            None => false,
            Some(value) => value.as_str().map_or(true, |alias| {
                alias != iroha_config::parameters::defaults::nexus::DEFAULT_DATASPACE_ALIAS
            }),
        };
        let rules_override = match policy.get("rules") {
            None => false,
            Some(value) => value.as_array().map_or(true, |rules| !rules.is_empty()),
        };
        if default_lane_override || default_dataspace_override || rules_override {
            return true;
        }
    }
    nexus
        .get("lane_count")
        .and_then(Value::as_integer)
        .is_some_and(|value| value > 1)
}

fn config_requires_sora_profile(config_layers: &[Table]) -> bool {
    // Inject required fields so profile detection can parse without the base layer.
    let merged = merged_sora_profile_detection_config(config_layers);
    let raw_sorafs_storage = read_bool(&merged, &["torii", "sorafs", "storage", "enabled"])
        .unwrap_or(false)
        || read_bool(&merged, &["sorafs", "storage", "enabled"]).unwrap_or(false);
    let raw_sorafs_discovery = read_bool(
        &merged,
        &["torii", "sorafs", "discovery", "discovery_enabled"],
    )
    .unwrap_or(false)
        || read_bool(&merged, &["sorafs", "discovery", "discovery_enabled"]).unwrap_or(false);
    let raw_sorafs_repair = read_bool(&merged, &["torii", "sorafs", "repair", "enabled"])
        .unwrap_or(false)
        || read_bool(&merged, &["sorafs", "repair", "enabled"]).unwrap_or(false);
    let raw_sorafs_gc = read_bool(&merged, &["torii", "sorafs", "gc", "enabled"]).unwrap_or(false)
        || read_bool(&merged, &["sorafs", "gc", "enabled"]).unwrap_or(false);
    let config = match ConfigReader::new()
        .with_env(MockEnv::default())
        .with_toml_source(TomlSource::inline(merged.clone()))
        .read_and_complete::<iroha_config::parameters::user::Root>()
    {
        Ok(user) => match user.parse() {
            Ok(parsed) => Some(parsed),
            Err(err) => {
                warn!(
                    ?err,
                    "failed to parse merged config for Sora profile detection; falling back to raw scan"
                );
                None
            }
        },
        Err(err) => {
            warn!(
                ?err,
                "failed to parse merged config for Sora profile detection; falling back to raw scan"
            );
            None
        }
    };
    if let Some(config) = config {
        let sorafs_storage = config.torii.sorafs_storage.enabled || raw_sorafs_storage;
        let sorafs_discovery =
            config.torii.sorafs_discovery.discovery_enabled || raw_sorafs_discovery;
        let sorafs_repair = config.torii.sorafs_repair.enabled || raw_sorafs_repair;
        let sorafs_gc = config.torii.sorafs_gc.enabled || raw_sorafs_gc;
        let nexus_requires_router = config.nexus.uses_multilane_catalogs();
        let nexus_lane_overrides = config.nexus.has_lane_overrides();
        sorafs_storage
            || sorafs_discovery
            || sorafs_repair
            || sorafs_gc
            || nexus_requires_router
            || nexus_lane_overrides
    } else {
        raw_sorafs_storage
            || raw_sorafs_discovery
            || raw_sorafs_repair
            || raw_sorafs_gc
            || raw_nexus_overrides(&merged)
    }
}

fn resolve_actual_config(
    peer: &NetworkPeer,
    config_layers: &[Table],
) -> Option<iroha_config::parameters::actual::Root> {
    let mut merged = peer.base_config_table();
    for layer in config_layers {
        merge_tables(&mut merged, layer);
    }
    parse_actual_config_for_genesis(merged, config_layers)
}

fn resolve_kura_store_dir(
    peer: &NetworkPeer,
    config_layers: &[Table],
) -> (PathBuf, String, String) {
    const DEFAULT_KURA_STORE_DIR_KEY: &str = "kura.store_dir (unresolved)";
    resolve_actual_config(peer, config_layers).map_or_else(
        || (
            peer.dir.join("storage"),
            DEFAULT_KURA_STORE_DIR_KEY.to_string(),
            "./storage".to_string(),
        ),
        |config| {
            let (store_dir, origin) = config.kura.store_dir.into_tuple();
            let value = store_dir.to_string_lossy().to_string();
            let resolved = if store_dir.is_absolute() {
                store_dir
            } else {
                peer.dir.join(store_dir)
            };
            (resolved, parameter_origin_to_string(&origin), value)
        },
    )
}

fn parse_actual_config_for_genesis(
    merged: Table,
    config_layers: &[Table],
) -> Option<iroha_config::parameters::actual::Root> {
    let user = match ConfigReader::new()
        .with_env(MockEnv::default())
        .with_toml_source(TomlSource::inline(merged))
        .read_and_complete::<iroha_config::parameters::user::Root>()
    {
        Ok(user) => user,
        Err(err) => {
            warn!(?err, "failed to read merged config for genesis config");
            return None;
        }
    };
    match user.parse() {
        Ok(mut config) => {
            if config_requires_sora_profile(config_layers) {
                config.apply_sora_profile();
            }
            config.apply_storage_budget();
            Some(config)
        }
        Err(err) => {
            warn!(?err, "failed to parse merged config for genesis config");
            None
        }
    }
}

#[cfg(test)]
fn resolve_da_proof_policies(
    peer: &NetworkPeer,
    config_layers: &[Table],
) -> Option<DaProofPolicyBundle> {
    resolve_actual_config(peer, config_layers)
        .map(|config| iroha_core::da::proof_policy_bundle(&config.nexus.lane_config))
}

fn apply_debug_rbc_defaults(table: &mut Table) {
    let Some(sumeragi) = table.get_mut("sumeragi").and_then(Value::as_table_mut) else {
        return;
    };
    let Some(debug) = sumeragi.get_mut("debug").and_then(Value::as_table_mut) else {
        return;
    };
    let Some(rbc) = debug.get_mut("rbc").and_then(Value::as_table_mut) else {
        return;
    };

    let defaults = [
        ("shuffle_chunks", Value::Boolean(false)),
        ("duplicate_inits", Value::Boolean(false)),
        ("corrupt_witness_ack", Value::Boolean(false)),
        ("corrupt_ready_signature", Value::Boolean(false)),
        ("drop_validator_mask", Value::Integer(0)),
        ("equivocate_chunk_mask", Value::Integer(0)),
        ("equivocate_validator_mask", Value::Integer(0)),
        ("conflicting_ready_mask", Value::Integer(0)),
        ("partial_chunk_mask", Value::Integer(0)),
    ];

    for (key, value) in defaults {
        rbc.entry(key.to_string()).or_insert(value);
    }
}

fn normalize_legacy_sumeragi_config(table: &mut Table) {
    let Some(sumeragi) = table.get_mut("sumeragi").and_then(Value::as_table_mut) else {
        return;
    };

    let mut move_key = |legacy_key: &str, path: &[&str], convert_seconds: bool| {
        if path.is_empty() {
            return;
        }
        let should_insert = get_nested_value(sumeragi, path).is_none();
        let Some(mut value) = sumeragi.remove(legacy_key) else {
            return;
        };
        if !should_insert {
            return;
        }
        if convert_seconds {
            value = match value {
                Value::Integer(secs) => Value::Integer(secs.saturating_mul(1_000)),
                other => other,
            };
        }
        let (parent_path, leaf_key) = path.split_at(path.len() - 1);
        let mut current = &mut *sumeragi;
        for segment in parent_path {
            let entry = current
                .entry((*segment).to_string())
                .or_insert_with(|| Value::Table(Table::new()));
            if !entry.is_table() {
                *entry = Value::Table(Table::new());
            }
            current = entry.as_table_mut().expect("entry is a table");
        }
        current.insert(leaf_key[0].to_string(), value);
    };

    move_key("collectors_k", &["collectors", "k"], false);
    move_key(
        "collectors_redundant_send_r",
        &["collectors", "redundant_send_r"],
        false,
    );
    move_key("da_enabled", &["da", "enabled"], false);
    move_key(
        "da_quorum_timeout_multiplier",
        &["advanced", "da", "quorum_timeout_multiplier"],
        false,
    );
    move_key(
        "da_availability_timeout_multiplier",
        &["advanced", "da", "availability_timeout_multiplier"],
        false,
    );
    move_key(
        "pacemaker_backoff_multiplier",
        &["advanced", "pacemaker", "backoff_multiplier"],
        false,
    );
    move_key(
        "pacemaker_rtt_floor_multiplier",
        &["advanced", "pacemaker", "rtt_floor_multiplier"],
        false,
    );
    move_key(
        "pacemaker_max_backoff_ms",
        &["advanced", "pacemaker", "max_backoff_ms"],
        false,
    );
    move_key(
        "pacemaker_jitter_frac_permille",
        &["advanced", "pacemaker", "jitter_frac_permille"],
        false,
    );
    move_key(
        "msg_channel_cap_blocks",
        &["advanced", "queues", "blocks"],
        false,
    );
    move_key(
        "rbc_chunk_max_bytes",
        &["advanced", "rbc", "chunk_max_bytes"],
        false,
    );
    move_key(
        "rbc_disk_store_max_bytes",
        &["advanced", "rbc", "disk_store_max_bytes"],
        false,
    );
    move_key(
        "rbc_disk_store_ttl_secs",
        &["advanced", "rbc", "disk_store_ttl_ms"],
        true,
    );
    move_key(
        "rbc_payload_chunks_per_tick",
        &["advanced", "rbc", "payload_chunks_per_tick"],
        false,
    );
    move_key(
        "rbc_pending_max_bytes",
        &["advanced", "rbc", "pending_max_bytes"],
        false,
    );
    move_key(
        "rbc_pending_max_chunks",
        &["advanced", "rbc", "pending_max_chunks"],
        false,
    );
    move_key(
        "rbc_pending_ttl_ms",
        &["advanced", "rbc", "pending_ttl_ms"],
        false,
    );
    move_key(
        "rbc_rebroadcast_sessions_per_tick",
        &["advanced", "rbc", "rebroadcast_sessions_per_tick"],
        false,
    );
    move_key(
        "rbc_session_ttl_secs",
        &["advanced", "rbc", "session_ttl_ms"],
        true,
    );
    move_key(
        "rbc_store_max_bytes",
        &["advanced", "rbc", "store_max_bytes"],
        false,
    );
    move_key(
        "rbc_store_max_sessions",
        &["advanced", "rbc", "store_max_sessions"],
        false,
    );
    move_key(
        "rbc_store_soft_bytes",
        &["advanced", "rbc", "store_soft_bytes"],
        false,
    );
    move_key(
        "rbc_store_soft_sessions",
        &["advanced", "rbc", "store_soft_sessions"],
        false,
    );
    move_key(
        "epoch_length_blocks",
        &["npos", "epoch_length_blocks"],
        false,
    );
    move_key(
        "use_stake_snapshot_roster",
        &["npos", "use_stake_snapshot_roster"],
        false,
    );
    move_key(
        "vrf_commit_deadline_offset",
        &["npos", "vrf", "commit_deadline_offset_blocks"],
        false,
    );
    move_key(
        "vrf_reveal_deadline_offset",
        &["npos", "vrf", "reveal_deadline_offset_blocks"],
        false,
    );
}

fn merged_sumeragi_config(config_layers: &[Table]) -> Table {
    let mut merged = Table::new();
    for layer in config_layers {
        if let Some(table) = layer.get("sumeragi").and_then(Value::as_table) {
            merge_tables(&mut merged, table);
        }
    }
    merged
}

fn get_nested_value<'a>(table: &'a Table, path: &[&str]) -> Option<&'a Value> {
    if path.is_empty() {
        return None;
    }
    let mut current = table.get(path[0])?;
    for segment in &path[1..] {
        current = current.as_table()?.get(*segment)?;
    }
    Some(current)
}

fn read_bool(table: &Table, path: &[&str]) -> Option<bool> {
    get_nested_value(table, path).and_then(Value::as_bool)
}

fn npos_timeout_override_from_table(table: &Table, path: &[&str]) -> Option<Duration> {
    let value = get_nested_value(table, path).and_then(Value::as_integer)?;
    if value <= 0 {
        return None;
    }
    let millis = u64::try_from(value).ok()?;
    Some(Duration::from_millis(millis))
}

fn npos_timeout_overrides_from_table(table: &Table) -> Option<SumeragiNposTimeoutOverrides> {
    let overrides = SumeragiNposTimeoutOverrides {
        propose: npos_timeout_override_from_table(
            table,
            &["advanced", "npos", "timeouts", "propose_ms"],
        ),
        prevote: npos_timeout_override_from_table(
            table,
            &["advanced", "npos", "timeouts", "prevote_ms"],
        ),
        precommit: npos_timeout_override_from_table(
            table,
            &["advanced", "npos", "timeouts", "precommit_ms"],
        ),
        exec: npos_timeout_override_from_table(table, &["advanced", "npos", "timeouts", "exec_ms"]),
        witness: npos_timeout_override_from_table(
            table,
            &["advanced", "npos", "timeouts", "witness_ms"],
        ),
        commit: npos_timeout_override_from_table(
            table,
            &["advanced", "npos", "timeouts", "commit_ms"],
        ),
        da: npos_timeout_override_from_table(table, &["advanced", "npos", "timeouts", "da_ms"]),
        aggregator: npos_timeout_override_from_table(
            table,
            &["advanced", "npos", "timeouts", "aggregator_ms"],
        ),
    };
    if overrides.propose.is_none()
        && overrides.prevote.is_none()
        && overrides.precommit.is_none()
        && overrides.exec.is_none()
        && overrides.witness.is_none()
        && overrides.commit.is_none()
        && overrides.da.is_none()
        && overrides.aggregator.is_none()
    {
        None
    } else {
        Some(overrides)
    }
}

fn replace_consensus_handshake_meta(genesis_isi: &mut Vec<Vec<InstructionBox>>) -> bool {
    let mut was_replaced = false;
    genesis_isi.iter_mut().for_each(|instructions| {
        let original_len = instructions.len();
        instructions.retain(|instruction| {
            let is_handshake_meta = instruction
                .as_any()
                .downcast_ref::<SetParameter>()
                .is_some_and(|set_param| {
                    matches!(
                        set_param.inner(),
                        Parameter::Custom(custom)
                            if custom.id() == &consensus_metadata::handshake_meta_id()
                    )
                });
            if is_handshake_meta {
                was_replaced = true;
            }
            !is_handshake_meta
        });
        if instructions.is_empty() && original_len > 0 {
            instructions.shrink_to_fit();
        }
    });
    was_replaced
}

fn genesis_instructions_contain_consensus_handshake_meta(
    genesis_isi: &[Vec<InstructionBox>],
    consensus_handshake_meta: &Parameter,
) -> bool {
    let expected_meta = match consensus_handshake_meta {
        Parameter::Custom(custom) if custom.id() == &consensus_metadata::handshake_meta_id() => {
            custom
        }
        _ => return false,
    };

    genesis_isi.iter().flat_map(|tx| tx.iter()).any(|instruction| {
        instruction
            .as_any()
            .downcast_ref::<SetParameter>()
            .is_some_and(|set_param| match set_param.inner() {
                Parameter::Custom(custom) => custom == expected_meta,
                _ => false,
            })
    })
}

fn genesis_has_consensus_handshake(block: &GenesisBlock, expected: &Parameter) -> bool {
    let expected_meta = match expected {
        Parameter::Custom(custom) if custom.id() == &consensus_metadata::handshake_meta_id() => {
            custom
        }
        _ => return false,
    };

    block.0.transactions_vec().iter().any(|tx| {
        match tx.instructions() {
            Executable::Instructions(instructions) => instructions.iter().any(|instruction| {
                instruction
                    .as_any()
                    .downcast_ref::<SetParameter>()
                    .is_some_and(|set_param| match set_param.inner() {
                        Parameter::Custom(custom) => custom == expected_meta,
                        _ => false,
                    })
            }),
            _ => false,
        }
    })
}

fn consensus_handshake_parameter(consensus_profile: &ConsensusBootstrapProfile) -> Parameter {
    let mode = match consensus_profile.mode_tag {
        NPOS_TAG => "Npos",
        _ => "Permissioned",
    };
    let mut handshake_fields = json::Map::new();
    handshake_fields.insert(
        "mode".to_string(),
        JsonValue::String(mode.to_string()),
    );
    handshake_fields.insert(
        "bls_domain".to_string(),
        JsonValue::String(consensus_profile.bls_domain.to_string()),
    );
    handshake_fields.insert(
        "wire_proto_versions".to_string(),
        json::to_value(&consensus_profile.wire_proto_versions)
            .expect("serialize handshake proto versions"),
    );
    handshake_fields.insert(
        "consensus_fingerprint".to_string(),
        JsonValue::String(format!("0x{}", hex_lower(&consensus_profile.fingerprint()))),
    );
    let handshake_payload = Json::from_norito_value_ref(&JsonValue::Object(handshake_fields))
        .expect("handshake metadata JSON must serialize");
    Parameter::Custom(CustomParameter::new(
        consensus_metadata::handshake_meta_id(),
        handshake_payload,
    ))
}

fn genesis_contains_npos_parameters(genesis_isi: &[Vec<InstructionBox>]) -> bool {
    let target = SumeragiNposParameters::parameter_id();
    genesis_isi
        .iter()
        .flat_map(|tx| tx.iter())
        .any(|instruction| {
            instruction
                .as_any()
                .downcast_ref::<SetParameter>()
                .and_then(|set_param| match set_param.inner() {
                    Parameter::Custom(custom) => Some(custom.id == target),
                    _ => None,
                })
                .unwrap_or(false)
        })
}

fn npos_params_from_genesis(genesis_isi: &[Vec<InstructionBox>]) -> Option<SumeragiNposParameters> {
    let mut found = None;
    for instruction in genesis_isi.iter().flat_map(|tx| tx.iter()) {
        let Some(set_param) = instruction.as_any().downcast_ref::<SetParameter>() else {
            continue;
        };
        let Parameter::Custom(custom) = set_param.inner() else {
            continue;
        };
        if let Some(params) = SumeragiNposParameters::from_custom_parameter(custom) {
            found = Some(params);
        }
    }
    found
}

fn resolve_npos_bootstrap_stake(genesis_isi: &[Vec<InstructionBox>], requested: u64) -> u64 {
    let min_self_bond = npos_params_from_genesis(genesis_isi)
        .map(|params| params.min_self_bond())
        .unwrap_or_else(|| SumeragiNposParameters::default().min_self_bond());
    requested.max(min_self_bond)
}

fn resolve_consensus_mode_from_config(table: &Table) -> ConsensusMode {
    let Some(raw) = table.get("consensus_mode").and_then(Value::as_str) else {
        return ConsensusMode::Permissioned;
    };
    if raw.eq_ignore_ascii_case("npos") {
        ConsensusMode::Npos
    } else if raw.eq_ignore_ascii_case("permissioned") {
        ConsensusMode::Permissioned
    } else {
        warn!(
            mode = raw,
            "unsupported consensus_mode override in test network config"
        );
        ConsensusMode::Permissioned
    }
}
impl Default for NetworkBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Test network builder
impl NetworkBuilder {
    /// Constructor
    pub fn new() -> Self {
        // Default to a fast localnet pipeline; use `with_default_pipeline_time` to
        // avoid injecting explicit on-chain timings when defaults are sufficient.
        let mut builder = Self {
            env: Environment::new(),
            n_peers: 1,
            config_layers: vec![],
            pipeline_time: Some(LOCALNET_PIPELINE_TIME),
            peer_startup_timeout: None,
            ivm_fuel: IvmFuelConfig::Auto,
            genesis_isi: vec![vec![]],
            genesis_post_topology_isi: Vec::new(),
            custom_genesis: None,
            seed: None,
            genesis_key_pair: SAMPLE_GENESIS_ACCOUNT_KEYPAIR.clone(),
            block_sync_gossip_period: DEFAULT_BLOCK_SYNC,
            sumeragi_parameters: Vec::new(),
            sumeragi_da_enabled: None,
            auto_populate_trusted_peer_pops: true,
            npos_genesis_bootstrap_stake: Some(SumeragiNposParameters::default().min_self_bond()),
        };
        if let Some(value) = bool_env_override(DA_ENABLED_ENV) {
            debug!(
                env = DA_ENABLED_ENV,
                value, "applying SUMERAGI_DA_ENABLED env override"
            );
            builder.sumeragi_da_enabled = Some(value);
        }
        let mut default_layer = Table::new();
        let mut writer = TomlWriter::new(&mut default_layer);
        // Scale per-peer thread pools to avoid oversubscribing the host when many
        // test networks run in parallel, but keep a minimum to prevent stalls.
        let concurrency_threads =
            i64::try_from(test_concurrency_threads()).expect("test concurrency threads fit in i64");
        writer
            .write(["nexus", "enabled"], false)
            .write(["telemetry_enabled"], true)
            .write(
                ["concurrency", "scheduler_min_threads"],
                concurrency_threads,
            )
            .write(
                ["concurrency", "scheduler_max_threads"],
                concurrency_threads,
            )
            .write(["concurrency", "rayon_global_threads"], concurrency_threads)
            .write(["pipeline", "workers"], concurrency_threads);
        apply_debug_rbc_defaults(&mut default_layer);
        builder.config_layers.push(default_layer);
        builder
    }

    /// Set the number of peers in the network.
    ///
    /// One by default.
    pub fn with_peers(mut self, n_peers: usize) -> Self {
        assert_ne!(n_peers, 0);
        self.n_peers = n_peers;
        self
    }

    /// Ensure the network has at least `min_peers` peers.
    ///
    /// If the current peer count is below `min_peers`, it is raised to that value.
    pub fn with_min_peers(mut self, min_peers: usize) -> Self {
        assert_ne!(min_peers, 0);
        if self.n_peers < min_peers {
            self.n_peers = min_peers;
        }
        self
    }

    /// Override the peer startup timeout for this network instance.
    ///
    /// Use this for slow hosts or heavy fixtures when peer bootstrap may exceed environment-level
    /// defaults. The timeout must be strictly positive.
    pub fn with_peer_startup_timeout(mut self, timeout: Duration) -> Self {
        assert!(timeout > Duration::ZERO, "startup timeout must be positive");
        self.peer_startup_timeout = Some(timeout);
        self
    }

    /// Set the total consensus pipeline time (block production + commit).
    ///
    /// The value is interpreted with millisecond precision. Internally we split it into
    /// [`SumeragiParameter::BlockTimeMs`] (roughly one third) and [`SumeragiParameter::CommitTimeMs`]
    /// (the remaining share) so that both parts stay positive and their sum matches the requested
    /// duration at millisecond granularity. The resulting timings are reflected by
    /// [`Network::pipeline_time`].
    ///
    /// # Panics
    /// - If `duration` is shorter than [`MIN_PIPELINE_TIME_MS`] milliseconds.
    /// - If `duration` exceeds `u64::MAX` milliseconds (cannot be encoded in genesis parameters).
    pub fn with_pipeline_time(mut self, duration: Duration) -> Self {
        let total_ms = duration.as_millis();
        assert!(
            total_ms >= u128::from(MIN_PIPELINE_TIME_MS),
            "pipeline time must be at least {MIN_PIPELINE_TIME_MS} ms (got {total_ms} ms)",
        );
        const MAX_PIPELINE_MS: u64 = u64::MAX;
        assert!(
            total_ms <= u128::from(MAX_PIPELINE_MS),
            "pipeline time must not exceed {MAX_PIPELINE_MS} ms",
        );
        self.pipeline_time = Some(duration);
        self
    }

    /// Do not overwrite default pipeline time ([`iroha_data_model::parameter::SumeragiParameters::default`]) in genesis.
    pub fn with_default_pipeline_time(mut self) -> Self {
        debug_assert!(DEFAULT_PIPELINE_TIME > Duration::from_secs(3));
        self.pipeline_time = None;
        self
    }

    /// Override the block gossip period used by block sync and gossip topics.
    ///
    /// Increasing the period introduces additional message delay between peers,
    /// which is useful when simulating unstable or high-latency links.
    /// The value must be strictly positive.
    pub fn with_block_sync_gossip_period(mut self, period: Duration) -> Self {
        assert!(
            period > Duration::ZERO,
            "block gossip period must be positive"
        );
        self.block_sync_gossip_period = period;
        self
    }

    fn push_sumeragi_parameter(&mut self, parameter: SumeragiParameter) {
        fn is_default(parameter: &SumeragiParameter) -> bool {
            match parameter {
                SumeragiParameter::DaEnabled(value) => !*value,
                _ => false,
            }
        }

        let discriminant = std::mem::discriminant(&parameter);
        self.sumeragi_parameters
            .retain(|existing| std::mem::discriminant(existing) != discriminant);
        if !is_default(&parameter) {
            self.sumeragi_parameters.push(parameter);
        }
    }

    /// Enable or disable data availability (RBC + availability QC gating) in the initial parameters.
    pub fn with_data_availability_enabled(mut self, enabled: bool) -> Self {
        self.sumeragi_da_enabled = Some(enabled);
        self.push_sumeragi_parameter(SumeragiParameter::DaEnabled(enabled));
        self
    }

    /// Automatically generate BLS key material and PoP records for trusted peers.
    ///
    /// Enabled by default; calling this method is only necessary when chaining builder combinators.
    /// The base config layer will include `trusted_peers_pop` entries aligning with the peers
    /// created by the builder.
    pub fn with_auto_populated_trusted_peers(mut self) -> Self {
        self.auto_populate_trusted_peer_pops = true;
        self
    }

    /// Override the NPoS bootstrap stake amount injected into genesis.
    ///
    /// This registers Nexus/IVM domains, a gas account, the default stake asset, and per-peer
    /// validator accounts funded with the stake amount, then activates them. NPoS bootstrap is
    /// enabled by default when the consensus mode is `npos`.
    pub fn with_npos_genesis_bootstrap(mut self, stake_amount: u64) -> Self {
        assert!(stake_amount > 0, "stake_amount must be non-zero");
        self.npos_genesis_bootstrap_stake = Some(stake_amount);
        self
    }

    /// Disable the NPoS bootstrap transaction injected into genesis.
    ///
    /// Use this when the caller already provides equivalent validator bootstrap instructions.
    pub fn without_npos_genesis_bootstrap(mut self) -> Self {
        self.npos_genesis_bootstrap_stake = None;
        self
    }

    /// Override the genesis signing key pair used to sign the manifest.
    pub fn with_genesis_keypair(mut self, key_pair: KeyPair) -> Self {
        self.genesis_key_pair = key_pair;
        self
    }

    /// Use the deterministic “real” genesis key material shared with the localnet fixtures.
    pub fn with_real_genesis_keypair(self) -> Self {
        self.with_genesis_keypair(REAL_GENESIS_ACCOUNT_KEYPAIR.clone())
    }

    /// Disable automatic trusted peer PoP entries.
    ///
    /// This is only useful for negative tests that explicitly exercise missing PoP scenarios.
    pub fn without_auto_populated_trusted_peers(mut self) -> Self {
        self.auto_populate_trusted_peer_pops = false;
        self
    }

    /// Add a new TOML configuration _layer_, using [`TomlWriter`] helper.
    ///
    /// Layers are composed using `extends` field in the final config file:
    ///
    /// ```toml
    /// extends = ["layer-1.toml", "layer-2.toml", "layer-3.toml"]
    /// ```
    ///
    /// Thus, layers are merged sequentially, with later ones overriding _conflicting_ parameters from earlier ones.
    ///
    /// # Example
    ///
    /// ```
    /// use iroha_test_network::NetworkBuilder;
    ///
    /// NetworkBuilder::new().with_config_layer(|t| {
    ///     t.write(["logger", "level"], "DEBUG");
    /// });
    /// ```
    pub fn with_config_layer<F>(mut self, f: F) -> Self
    where
        for<'a> F: FnOnce(&'a mut TomlWriter<'a>),
    {
        let mut table = Table::new();
        let mut writer = TomlWriter::new(&mut table);
        f(&mut writer);
        normalize_legacy_sumeragi_config(&mut table);
        apply_debug_rbc_defaults(&mut table);
        self.config_layers.push(table);
        self
    }

    /// Push a pre-built TOML configuration layer.
    pub fn with_config_table(mut self, table: Table) -> Self {
        let mut table = table;
        normalize_legacy_sumeragi_config(&mut table);
        apply_debug_rbc_defaults(&mut table);
        self.config_layers.push(table);
        self
    }

    /// Append an instruction to the last genesis transaction.
    pub fn with_genesis_instruction(mut self, isi: impl Into<InstructionBox>) -> Self {
        self.genesis_isi
            .last_mut()
            .expect("at least one transaction exists")
            .push(isi.into());
        self
    }

    /// Append a post-topology genesis transaction.
    ///
    /// The provided instructions run after peers/topology are registered.
    pub fn with_genesis_post_topology_isi(mut self, isi: Vec<InstructionBox>) -> Self {
        if !isi.is_empty() {
            self.genesis_post_topology_isi.push(isi);
        }
        self
    }

    /// Start a new empty transaction in the genesis block.
    pub fn next_genesis_transaction(mut self) -> Self {
        self.genesis_isi.push(Vec::new());
        self
    }

    /// Override the genesis block entirely using a custom builder.
    ///
    /// The provided closure receives the network topology (as peer IDs) and the
    /// corresponding Proof-of-Possession entries. It must return a fully signed
    /// genesis block. When set, the regular `genesis_isi` instructions are ignored
    /// and the resulting block is reused verbatim by all peers.
    pub fn with_genesis_block<F>(mut self, build: F) -> Self
    where
        F: Fn(UniqueVec<PeerId>, Vec<GenesisTopologyEntry>) -> GenesisBlock + Send + Sync + 'static,
    {
        self.custom_genesis = Some(Box::new(build));
        self.genesis_isi = vec![Vec::new()];
        self
    }

    pub fn with_base_seed(mut self, seed: impl ToString) -> Self {
        self.seed = Some(seed.to_string());
        self
    }

    /// Set [`IvmFuelConfig`].
    ///
    /// The builder defaults to [`IvmFuelConfig::Auto`], ensuring non-optimized IVM builds receive
    /// a higher fuel allowance unless explicitly overridden.
    pub fn with_ivm_fuel(mut self, config: IvmFuelConfig) -> Self {
        self.ivm_fuel = config;
        self
    }

    /// Build the [`Network`]. Doesn't start it.
    pub fn build(self) -> Network {
        let permit = acquire_network_permit();
        let NetworkBuilder {
            env,
            n_peers,
            mut config_layers,
            pipeline_time,
            peer_startup_timeout,
            ivm_fuel,
            mut genesis_isi,
            mut genesis_post_topology_isi,
            custom_genesis,
            seed,
            genesis_key_pair,
            block_sync_gossip_period,
            sumeragi_parameters,
            sumeragi_da_enabled,
            auto_populate_trusted_peer_pops,
            npos_genesis_bootstrap_stake,
        } = self;

        let mut sumeragi_parameters = sumeragi_parameters;
        let merged_sumeragi = merged_sumeragi_config(&config_layers);
        let default_da_enabled = true;
        let config_da_enabled = read_bool(&merged_sumeragi, &["da", "enabled"]);
        let mut da_enabled = sumeragi_da_enabled
            .or(config_da_enabled)
            .unwrap_or(default_da_enabled);
        if !da_enabled {
            warn!(
                builder_override = sumeragi_da_enabled,
                config_override = config_da_enabled,
                "iroha3 requires data availability; forcing DA enabled"
            );
            da_enabled = true;
        }
        debug!(
            n_peers,
            default_da_enabled,
            builder_override = sumeragi_da_enabled,
            config_override = config_da_enabled,
            resolved_da_enabled = da_enabled,
            "resolved DA setting for test network"
        );
        sumeragi_parameters.retain(|param| !matches!(param, SumeragiParameter::DaEnabled(_)));
        sumeragi_parameters.push(SumeragiParameter::DaEnabled(da_enabled));

        let use_sora_profile = config_requires_sora_profile(&config_layers);
        let mut consensus_mode = resolve_consensus_mode_from_config(&merged_sumeragi);
        if use_sora_profile && !matches!(consensus_mode, ConsensusMode::Npos) {
            warn!("Sora profile detection forces NPoS consensus mode in test-network genesis");
            consensus_mode = ConsensusMode::Npos;
        }

        let peers: Vec<_> = (0..n_peers)
            .map(|i| {
                let seed = seed.as_ref().map(|x| format!("{x}-peer-{i}"));
                NetworkPeerBuilder::new()
                    .with_seed(seed.as_ref().map(|x| x.as_bytes()))
                    .build(&env)
            })
            .collect();

        let peer_ids: UniqueVec<PeerId> = peers.iter().map(NetworkPeer::id).collect();
        let collected_entries: Vec<GenesisTopologyEntry> =
            peers.iter().filter_map(NetworkPeer::genesis_pop).collect();
        assert_eq!(
            collected_entries.len(),
            peers.len(),
            "every network peer must provide a BLS PoP"
        );

        let topology_entries: Vec<GenesisTopologyEntry> = collected_entries.clone();

        let peer_topology: Vec<PeerId> = peer_ids.iter().cloned().collect();
        let default_redundant_send_r = redundant_send_r_from_len(peer_ids.len());
        let effective_redundant_send_r = sumeragi_parameters
            .iter()
            .find_map(|param| match param {
                SumeragiParameter::RedundantSendR(value) => Some(*value),
                _ => None,
            })
            .unwrap_or(default_redundant_send_r);
        if !sumeragi_parameters
            .iter()
            .any(|param| matches!(param, SumeragiParameter::RedundantSendR(_)))
        {
            sumeragi_parameters.push(SumeragiParameter::RedundantSendR(
                effective_redundant_send_r,
            ));
        }
        let cached_genesis = OnceLock::new();
        if let Some(builder_fn) = custom_genesis.as_ref() {
            let mut block = builder_fn(peer_ids.clone(), topology_entries.clone());
            let genesis_key_pair = genesis_key_pair.clone();
            let genesis_account_id = AccountId::new(
                iroha_genesis::GENESIS_DOMAIN_ID.clone(),
                genesis_key_pair.public_key().clone(),
            );
            ensure_genesis_results(
                &mut block,
                &genesis_account_id,
                &peer_topology,
                &genesis_key_pair,
                None,
            );
            cached_genesis
                .set(block)
                .expect("custom genesis should be set exactly once");
        }

        let sumeragi_overrides = sumeragi_parameters.clone();

        // Determine the effective pipeline time we report to tests.
        // By default we inject a fast localnet pipeline into genesis; callers can opt out
        // via `with_default_pipeline_time`, which keeps the baked-in Sumeragi defaults
        // (2s block, 4s commit) without extra on-chain overrides.
        let (block_time, commit_time) = if let Some(duration) = pipeline_time {
            let total_ms_u128 = duration.as_millis();
            let total_ms = u64::try_from(total_ms_u128)
                .expect("pipeline time already validated to fit into u64 milliseconds");
            debug_assert!(total_ms >= MIN_PIPELINE_TIME_MS);

            let mut block_ms = total_ms / 3;
            if block_ms == 0 {
                block_ms = 1;
            }
            if block_ms >= total_ms {
                block_ms = total_ms - 1;
            }
            let mut commit_ms = total_ms - block_ms;
            if commit_ms == 0 {
                commit_ms = 1;
                if block_ms > 1 {
                    block_ms -= 1;
                }
            }
            debug_assert!(block_ms > 0);
            debug_assert!(commit_ms > 0);
            (
                Duration::from_millis(block_ms),
                Duration::from_millis(commit_ms),
            )
        } else {
            // Match Iroha defaults (2s block, 4s commit)
            (DEFAULT_BLOCK_TIME, DEFAULT_COMMIT_TIME)
        };

        let set_ivm_fuel = match ivm_fuel {
            IvmFuelConfig::Unset => None,
            IvmFuelConfig::Value(value) => Some(value),
            IvmFuelConfig::Auto => match iroha_test_samples::load_ivm_build_profile() {
                Some(profile) if profile.is_optimized() => None,
                Some(_) => Some(NON_OPTIMIZED_IVM_FUEL),
                None => Some(NON_OPTIMIZED_IVM_FUEL),
            },
        }
        .map(|value| {
            InstructionBox::from(SetParameter::new(Parameter::SmartContract(
                SmartContractParameter::Fuel(value),
            )))
        });
        let pipeline_time_ms = pipeline_time.map(|_| {
            let block_time_ms = u64::try_from(block_time.as_millis())
                .expect("block time fits into u64 milliseconds");
            let commit_time_ms = u64::try_from(commit_time.as_millis())
                .expect("commit time fits into u64 milliseconds");
            (block_time_ms, commit_time_ms)
        });

        let mut config_layers_for_parse = Vec::with_capacity(config_layers.len() + 2);
        config_layers_for_parse.push(
            Table::new()
                .write("chain", config::chain_id().to_string())
                .write(
                    ["genesis", "public_key"],
                    genesis_key_pair.public_key().to_string(),
                ),
        );
        config_layers_for_parse.push(trusted_peers_layer_for_parse(
            &peers,
            auto_populate_trusted_peer_pops,
        ));
        config_layers_for_parse.extend(config_layers.iter().cloned());

        let resolved_npos_config = peers
            .first()
            .and_then(|peer| resolve_actual_config(peer, &config_layers_for_parse));
        let consensus_chain_id = resolved_npos_config
            .as_ref()
            .map(|config| config.common.chain.clone())
            .unwrap_or_else(chain_id);

        let npos_params_from_config = |config: &iroha_config::parameters::actual::Root| {
            let npos = &config.sumeragi.npos;
            let collectors = &config.sumeragi.collectors;
            let chain_hash =
                CryptoHash::new(config.common.chain.clone().into_inner().as_bytes());
            let epoch_seed: [u8; 32] = chain_hash.into();
            let mut fallback = SumeragiNposParameters::default();
            fallback.epoch_length_blocks = npos.epoch_length_blocks.max(1);
            fallback.k_aggregators = u16::try_from(collectors.k)
                .expect("sumeragi.collectors.k exceeds u16 for NPoS fallback");
            fallback.redundant_send_r = collectors.redundant_send_r;
            fallback.epoch_seed = epoch_seed;
            fallback.vrf_commit_window_blocks = npos.vrf.commit_window_blocks;
            fallback.vrf_reveal_window_blocks = npos.vrf.reveal_window_blocks;
            fallback.max_validators = npos.election.max_validators;
            fallback.min_self_bond = npos.election.min_self_bond;
            fallback.min_nomination_bond = npos.election.min_nomination_bond;
            fallback.max_nominator_concentration_pct = npos.election.max_nominator_concentration_pct;
            fallback.seat_band_pct = npos.election.seat_band_pct;
            fallback.max_entity_correlation_pct = npos.election.max_entity_correlation_pct;
            fallback.finality_margin_blocks = npos.election.finality_margin_blocks;
            fallback.evidence_horizon_blocks = npos.reconfig.evidence_horizon_blocks;
            fallback.activation_lag_blocks = npos.reconfig.activation_lag_blocks;
            fallback.slashing_delay_blocks = npos.reconfig.slashing_delay_blocks;
            fallback
        };

        let mut parameter_prefix: Vec<InstructionBox> = Vec::new();
        if let Some(fuel) = set_ivm_fuel {
            parameter_prefix.push(fuel);
        }

        for parameter in &sumeragi_parameters {
            parameter_prefix.push(InstructionBox::from(SetParameter::new(
                Parameter::Sumeragi(*parameter),
            )));
        }

        if let Some((block_time_ms, commit_time_ms)) = pipeline_time_ms {
            // Set commit_time first so the intermediate parameter state remains valid.
            parameter_prefix.push(InstructionBox::from(SetParameter::new(
                Parameter::Sumeragi(SumeragiParameter::CommitTimeMs(commit_time_ms)),
            )));
            parameter_prefix.push(InstructionBox::from(SetParameter::new(
                Parameter::Sumeragi(SumeragiParameter::BlockTimeMs(block_time_ms)),
            )));
        }

        if matches!(consensus_mode, ConsensusMode::Npos)
            && !genesis_contains_npos_parameters(&genesis_isi)
        {
            let mut npos = resolved_npos_config
                .as_ref()
                .map(npos_params_from_config)
                .unwrap_or_else(SumeragiNposParameters::default);
            npos.redundant_send_r = effective_redundant_send_r;
            parameter_prefix.push(InstructionBox::from(SetParameter::new(Parameter::Custom(
                npos.into_custom_parameter(),
            ))));
        }

        {
            let first_tx = genesis_isi
                .first_mut()
                .expect("at least one genesis transaction exists");
            first_tx.splice(0..0, parameter_prefix);
        }

        let npos_bootstrap =
            npos_genesis_bootstrap_stake.filter(|_| matches!(consensus_mode, ConsensusMode::Npos));
        if let Some(stake_amount) = npos_bootstrap {
            let stake_amount = resolve_npos_bootstrap_stake(&genesis_isi, stake_amount);
            let nexus_domain: DomainId = "nexus".parse().expect("nexus domain");
            let ivm_domain: DomainId = "ivm".parse().expect("ivm domain");
            let stake_asset_id: AssetDefinitionId =
                "xor#nexus".parse().expect("stake asset definition");
            let gas_account_id =
                AccountId::new(ivm_domain.clone(), genesis_key_pair.public_key().clone());
            let gas_account_str = format!("{}@{ivm_domain}", genesis_key_pair.public_key());

            let mut bootstrap_layer = Table::new();
            let mut writer = TomlWriter::new(&mut bootstrap_layer);
            writer
                .write(["nexus", "enabled"], true)
                .write(
                    ["nexus", "staking", "stake_asset_id"],
                    stake_asset_id.to_string(),
                )
                .write(
                    ["nexus", "staking", "stake_escrow_account_id"],
                    gas_account_str.clone(),
                )
                .write(
                    ["nexus", "staking", "slash_sink_account_id"],
                    gas_account_str,
                );
            config_layers.push(bootstrap_layer);

            let definition = AssetDefinition::new(stake_asset_id.clone(), NumericSpec::default())
                .with_metadata(Metadata::default());

            let mut bootstrap_tx = vec![
                Register::domain(Domain::new(nexus_domain.clone())).into(),
                Register::domain(Domain::new(ivm_domain.clone())).into(),
                Register::account(Account::new(gas_account_id.clone())).into(),
                Register::asset_definition(definition).into(),
            ];

            for peer in &peer_ids {
                let validator_id = AccountId::new(nexus_domain.clone(), peer.public_key().clone());
                bootstrap_tx.push(Register::account(Account::new(validator_id.clone())).into());
                bootstrap_tx.push(
                    Mint::asset_numeric(
                        stake_amount,
                        AssetId::new(stake_asset_id.clone(), validator_id.clone()),
                    )
                    .into(),
                );
            }
            genesis_post_topology_isi.push(bootstrap_tx);

            let mut validator_tx = Vec::new();
            for peer in &peer_ids {
                let validator_id = AccountId::new(nexus_domain.clone(), peer.public_key().clone());
                validator_tx.push(
                    RegisterPublicLaneValidator {
                        lane_id: LaneId::SINGLE,
                        validator: validator_id.clone(),
                        stake_account: validator_id.clone(),
                        initial_stake: Numeric::from(stake_amount),
                        metadata: Metadata::default(),
                    }
                    .into(),
                );
                validator_tx.push(
                    ActivatePublicLaneValidator {
                        lane_id: LaneId::SINGLE,
                        validator: validator_id,
                    }
                    .into(),
                );
            }
            genesis_post_topology_isi.push(validator_tx);
        }

        // Build a preview genesis so we can derive the consensus fingerprint from the
        // actual on-chain parameters (including the built-in genesis scaffolding).
        let preview_genesis = genesis_factory(
            genesis_isi.clone(),
            peer_ids.clone(),
            topology_entries.clone(),
        );
        let mut parameter_state = iroha_data_model::parameter::Parameters::default();
        for tx in preview_genesis.0.external_transactions() {
            if let Executable::Instructions(batch) = tx.instructions() {
                for instruction in batch {
                    if let Some(set_param) = instruction.as_any().downcast_ref::<SetParameter>() {
                        parameter_state.set_parameter(set_param.inner().clone());
                    }
                }
            }
        }
        let npos_timeout_overrides = resolved_npos_config
            .as_ref()
            .map(|config| config.sumeragi.npos.timeouts_overrides)
            .or_else(|| npos_timeout_overrides_from_table(&merged_sumeragi));
        let default_timeout_base_ms = parameter_state
            .sumeragi()
            .block_time_ms()
            .max(parameter_state.sumeragi().min_finality_ms());
        let derive_npos_timeouts = |block_time_ms: u64| {
            let block_time_ms = block_time_ms.max(default_timeout_base_ms).max(1);
            let block_time = Duration::from_millis(block_time_ms);
            let timeouts = npos_timeout_overrides
                .map(|overrides| overrides.resolve(block_time))
                .unwrap_or_else(|| {
                    iroha_config::parameters::actual::SumeragiNposTimeouts::from_block_time(
                        block_time,
                    )
                });
            timeouts
        };

        let npos_payload = parameter_state
            .custom()
            .get(&SumeragiNposParameters::parameter_id())
            .and_then(SumeragiNposParameters::from_custom_parameter);
        let (epoch_length_blocks, npos_params) = match npos_payload {
            Some(npos) => {
                let timeouts = derive_npos_timeouts(parameter_state.sumeragi().block_time_ms());
                let duration_ms = |value: Duration| -> u64 {
                    let ms = value.as_millis();
                    u64::try_from(ms).expect("NPoS timeout exceeds millisecond range")
                };
                (
                    npos.epoch_length_blocks(),
                    Some(NposGenesisParams {
                        block_time_ms: parameter_state.sumeragi().block_time_ms(),
                        timeout_propose_ms: duration_ms(timeouts.propose),
                        timeout_prevote_ms: duration_ms(timeouts.prevote),
                        timeout_precommit_ms: duration_ms(timeouts.precommit),
                        timeout_commit_ms: duration_ms(timeouts.commit),
                        timeout_da_ms: duration_ms(timeouts.da),
                        timeout_aggregator_ms: duration_ms(timeouts.aggregator),
                        k_aggregators: npos.k_aggregators(),
                        redundant_send_r: npos.redundant_send_r(),
                        epoch_seed: npos.epoch_seed(),
                        vrf_commit_window_blocks: npos.vrf_commit_window_blocks(),
                        vrf_reveal_window_blocks: npos.vrf_reveal_window_blocks(),
                        max_validators: npos.max_validators(),
                        min_self_bond: npos.min_self_bond(),
                        min_nomination_bond: npos.min_nomination_bond(),
                        max_nominator_concentration_pct: npos.max_nominator_concentration_pct(),
                        seat_band_pct: npos.seat_band_pct(),
                        max_entity_correlation_pct: npos.max_entity_correlation_pct(),
                        finality_margin_blocks: npos.finality_margin_blocks(),
                        evidence_horizon_blocks: npos.evidence_horizon_blocks(),
                        activation_lag_blocks: npos.activation_lag_blocks(),
                        slashing_delay_blocks: npos.slashing_delay_blocks(),
                    }),
                )
            }
            None if matches!(consensus_mode, ConsensusMode::Npos) => {
                let npos = resolved_npos_config
                    .as_ref()
                    .expect("NPoS consensus requires resolved runtime config");
                let npos = npos_params_from_config(npos);
                let timeouts = derive_npos_timeouts(parameter_state.sumeragi().block_time_ms());
                let duration_ms = |value: Duration| -> u64 {
                    let ms = value.as_millis();
                    u64::try_from(ms).expect("NPoS timeout exceeds millisecond range")
                };
                (
                    npos.epoch_length_blocks,
                    Some(NposGenesisParams {
                        block_time_ms: parameter_state.sumeragi().block_time_ms(),
                        timeout_propose_ms: duration_ms(timeouts.propose),
                        timeout_prevote_ms: duration_ms(timeouts.prevote),
                        timeout_precommit_ms: duration_ms(timeouts.precommit),
                        timeout_commit_ms: duration_ms(timeouts.commit),
                        timeout_da_ms: duration_ms(timeouts.da),
                        timeout_aggregator_ms: duration_ms(timeouts.aggregator),
                        k_aggregators: npos.k_aggregators(),
                        redundant_send_r: npos.redundant_send_r(),
                        epoch_seed: npos.epoch_seed(),
                        vrf_commit_window_blocks: npos.vrf_commit_window_blocks(),
                        vrf_reveal_window_blocks: npos.vrf_reveal_window_blocks(),
                        max_validators: npos.max_validators(),
                        min_self_bond: npos.min_self_bond(),
                        min_nomination_bond: npos.min_nomination_bond(),
                        max_nominator_concentration_pct: npos.max_nominator_concentration_pct(),
                        seat_band_pct: npos.seat_band_pct(),
                        max_entity_correlation_pct: npos.max_entity_correlation_pct(),
                        finality_margin_blocks: npos.finality_margin_blocks(),
                        evidence_horizon_blocks: npos.evidence_horizon_blocks(),
                        activation_lag_blocks: npos.activation_lag_blocks(),
                        slashing_delay_blocks: npos.slashing_delay_blocks(),
                    }),
                )
            }
            None => (0, None),
        };

        let mut consensus_params = ConsensusGenesisParams {
            block_time_ms: parameter_state.sumeragi().block_time_ms(),
            commit_time_ms: parameter_state.sumeragi().commit_time_ms(),
            min_finality_ms: parameter_state.sumeragi().min_finality_ms(),
            max_clock_drift_ms: parameter_state.sumeragi().max_clock_drift_ms(),
            collectors_k: parameter_state.sumeragi().collectors_k(),
            redundant_send_r: parameter_state.sumeragi().collectors_redundant_send_r(),
            block_max_transactions: parameter_state.block().max_transactions().get(),
            da_enabled: parameter_state.sumeragi().da_enabled(),
            epoch_length_blocks,
            bls_domain: PERMISSIONED_BLS_DOMAIN.to_string(),
            npos: npos_params,
        };
        // Ensure the handshake caps mirror the builder-resolved flags even if the preview
        // genesis carries defaults different from the builder request.
        consensus_params.da_enabled = da_enabled;

        let mut consensus_mode_tag = PERMISSIONED_TAG;
        let mut consensus_bls_domain = PERMISSIONED_BLS_DOMAIN;
        if matches!(consensus_mode, ConsensusMode::Npos) {
            consensus_mode_tag = NPOS_TAG;
            consensus_bls_domain = NPOS_BLS_DOMAIN;
            consensus_params.bls_domain = NPOS_BLS_DOMAIN.to_string();
        }

        let consensus_profile = ConsensusBootstrapProfile {
            params: consensus_params,
            mode_tag: consensus_mode_tag,
            bls_domain: consensus_bls_domain,
            chain_id: consensus_chain_id.clone(),
            wire_proto_versions: vec![PROTO_VERSION],
        };

        debug!(
            profile_block_time_ms = consensus_profile.params.block_time_ms,
            profile_commit_time_ms = consensus_profile.params.commit_time_ms,
            profile_block_max_transactions = consensus_profile.params.block_max_transactions,
            profile_da_enabled = consensus_profile.params.da_enabled,
            profile_fingerprint = %format!("0x{}", hex_lower(&consensus_profile.fingerprint())),
            "resolved consensus profile for genesis"
        );

        let replaced_in_genesis = replace_consensus_handshake_meta(&mut genesis_isi);
        let replaced_in_post_topology =
            replace_consensus_handshake_meta(&mut genesis_post_topology_isi);
        let consensus_handshake_meta = consensus_handshake_parameter(&consensus_profile);
        if replaced_in_genesis || replaced_in_post_topology {
            debug!(
                replaced = replaced_in_genesis || replaced_in_post_topology,
                "replaced existing consensus_handshake_meta in genesis with computed profile"
            );
        }
        if !(genesis_instructions_contain_consensus_handshake_meta(
            &genesis_isi,
            &consensus_handshake_meta,
        ) || genesis_instructions_contain_consensus_handshake_meta(
            &genesis_post_topology_isi,
            &consensus_handshake_meta,
        )) {
            let instruction = InstructionBox::from(SetParameter::new(consensus_handshake_meta.clone()));
            if genesis_isi.is_empty() {
                genesis_isi.push(vec![instruction]);
            } else {
                genesis_isi[0].push(instruction);
            }
            debug!(
                inserted = true,
                "inserted computed consensus_handshake_meta into genesis instructions"
            );
        }

        let gossip_ms = i64::try_from(block_sync_gossip_period.as_millis())
            .expect("block gossip period fits in i64 milliseconds");

        let mut base_layer = config::base_iroha_config();
        base_layer = base_layer
            .write(["network", "block_gossip_period_ms"], gossip_ms)
            // Fan-out gossip to all peers so block sync converges quickly in multi-peer
            // integration scenarios (NPoS liveness, DA/RBC).
            .write(
                ["network", "block_gossip_size"],
                i64::try_from(peers.len()).unwrap_or(i64::MAX),
            );
        base_layer = base_layer.write(["sumeragi", "advanced", "queues", "blocks"], 512i64);
        if da_enabled {
            base_layer = base_layer
                .write(["sumeragi", "da", "enabled"], true)
                .write(
                    ["sumeragi", "advanced", "rbc", "store_max_sessions"],
                    DEFAULT_RBC_STORE_MAX_SESSIONS,
                )
                .write(
                    ["sumeragi", "advanced", "rbc", "store_soft_sessions"],
                    DEFAULT_RBC_STORE_SOFT_SESSIONS,
                )
                .write(
                    ["sumeragi", "advanced", "rbc", "store_max_bytes"],
                    DEFAULT_RBC_STORE_MAX_BYTES,
                )
                .write(
                    ["sumeragi", "advanced", "rbc", "store_soft_bytes"],
                    DEFAULT_RBC_STORE_SOFT_BYTES,
                );
        } else {
            base_layer = base_layer
                .write(["sumeragi", "da", "enabled"], false)
                .write(["sumeragi", "advanced", "rbc", "store_max_sessions"], 0i64)
                .write(["sumeragi", "advanced", "rbc", "store_soft_sessions"], 0i64)
                .write(["sumeragi", "advanced", "rbc", "store_max_bytes"], 0i64)
                .write(["sumeragi", "advanced", "rbc", "store_soft_bytes"], 0i64);
        }
        base_layer = base_layer
            .write(
                ["sumeragi", "advanced", "rbc", "chunk_max_bytes"],
                LOCALNET_RBC_CHUNK_MAX_BYTES,
            )
            // Test networks always provision BLS validator keys; drop the HSM binding requirement
            // so genesis peer registration succeeds.
            .write(["sumeragi", "keys", "require_hsm"], false)
            .write(
                ["genesis", "public_key"],
                genesis_key_pair.public_key().to_string(),
            )
            // Keep consensus permissive for integration tests: disable precommit QC requirement
            // so small networks can make progress even when peers start slowly.
            .write(["sumeragi", "finality", "require_precommit_qc"], false);
        base_layer = base_layer
            // Ensure BLS batching stays enabled so PoP-based peers can register and vote.
            .write(["pipeline", "signature_batch_max_bls"], 4i64)
            .write(["torii", "rbc_sampling", "enabled"], false)
            // Enable Norito-RPC for test networks so client-based flows keep working out of the box.
            .write(["torii", "transport", "norito_rpc", "stage"], "ga")
            .write(["torii", "transport", "norito_rpc", "enabled"], true);

        Network {
            env,
            peers,
            block_time,
            commit_time,
            block_sync_gossip_period,
            peer_startup_timeout_override: peer_startup_timeout,
            consensus_profile,
            genesis_key_pair,
            genesis_isi,
            genesis_post_topology_isi,
            cached_genesis,
            config_layers: Some(base_layer).into_iter().chain(config_layers).collect(),
            sumeragi_overrides,
            topology_entries,
            auto_populate_trusted_peer_pops,
            _permit: permit,
        }
    }

    /// Same as [`Self::build`], but also creates a [`Runtime`].
    ///
    /// This method exists for convenience in non-async tests.
    pub fn build_blocking(self) -> (Network, Runtime) {
        let rt = runtime::Builder::new_multi_thread()
            .thread_stack_size(32 * 1024 * 1024)
            .enable_all()
            .build()
            .unwrap();
        let network = self.build();
        (network, rt)
    }

    /// Build and start the network.
    ///
    /// Resolves when all peers are running and have committed genesis block.
    /// See [`Network::start_all`].
    pub async fn start(self) -> Result<Network> {
        let network = self.build();
        network.start_all().await?;
        Ok(network)
    }

    /// Combination of [`Self::build_blocking`] and [`Self::start`].
    pub fn start_blocking(self) -> Result<(Network, Runtime)> {
        let (network, rt) = self.build_blocking();
        rt.block_on(async { network.start_all().await })?;
        Ok((network, rt))
    }
}

/// A common signatory in the test network.
///
/// # Example
///
/// ```
/// use iroha_test_network::Signatory;
///
/// let _alice_kp = Signatory::Alice.key_pair();
/// ```
pub enum Signatory {
    Peer,
    Genesis,
    Alice,
}

impl Signatory {
    /// Get the associated key pair
    pub fn key_pair(&self) -> &KeyPair {
        match self {
            Signatory::Peer => &PEER_KEYPAIR,
            Signatory::Genesis => &SAMPLE_GENESIS_ACCOUNT_KEYPAIR,
            Signatory::Alice => &ALICE_KEYPAIR,
        }
        .deref()
    }
}

/// Running Iroha peer.
///
/// Aborts peer forcefully when dropped
#[derive(Debug)]
struct PeerRun {
    tasks: JoinSet<()>,
    shutdown: oneshot::Sender<()>,
    fatal_tx: watch::Sender<bool>,
    pid: Option<u32>,
}

/// Lifecycle events of a peer
#[derive(Copy, Clone, Debug)]
pub enum PeerLifecycleEvent {
    /// Process spawned
    Spawned,
    /// Server started to respond
    ServerStarted,
    /// Process terminated
    Terminated { status: ExitStatus },
    /// Process was killed
    Killed,
    /// Caught a related pipeline event
    BlockApplied { height: u64 },
}

#[derive(Debug, Clone)]
struct PeerStartContext {
    run_num: usize,
    config_path: PathBuf,
    genesis_path: Option<PathBuf>,
    stdout_path: PathBuf,
    stderr_path: PathBuf,
    kura_store_dir_key: String,
    kura_store_dir: PathBuf,
    kura_store_dir_value: String,
}

fn parameter_origin_to_string(origin: &ParameterOrigin) -> String {
    match origin {
        ParameterOrigin::File { id, path } => format!("{id} from file `{}`", path.display()),
        ParameterOrigin::Env { id, var } => format!("{id} from env `{var}`"),
        ParameterOrigin::Default { id } => format!("{id} (default)"),
        ParameterOrigin::Custom { message } => format!("custom: {message}"),
    }
}

impl PeerStartContext {
    fn summary(&self) -> String {
        format!(
            "run={}; config_path={}; genesis_path={}; stdout_path={}; stderr_path={}; kura_store_dir_key={}; kura_store_dir_value={}; kura_store_dir={}",
            self.run_num,
            self.config_path.display(),
            self.genesis_path
                .as_ref()
                .map_or_else(|| "<none>".to_string(), |path| path.display().to_string()),
            self.stdout_path.display(),
            self.stderr_path.display(),
            self.kura_store_dir_key,
            self.kura_store_dir_value,
            self.kura_store_dir.display(),
        )
    }
}

async fn wait_for_start_event(
    mut rx: broadcast::Receiver<PeerLifecycleEvent>,
) -> Option<PeerLifecycleEvent> {
    loop {
        match rx.recv().await {
            Ok(event @ PeerLifecycleEvent::ServerStarted)
            | Ok(event @ PeerLifecycleEvent::Terminated { .. })
            | Ok(event @ PeerLifecycleEvent::Killed) => return Some(event),
            Ok(_) => continue,
            Err(broadcast::error::RecvError::Lagged(_)) => continue,
            Err(broadcast::error::RecvError::Closed) => return None,
        }
    }
}

/// Controls execution of `irohad` child process.
///
/// While exists, allocates socket ports and a temporary directory (not cleared automatically).
///
/// It can be started and shut down repeatedly.
/// It stores configuration and logs for each run separately.
///
/// When dropped, aborts the child process (if it is running).
#[derive(Clone, Debug)]
pub struct NetworkPeer {
    mnemonic: String,
    span: tracing::Span,
    key_pair: KeyPair,
    streaming_key_pair: KeyPair,
    bls_key_pair: Option<KeyPair>,
    bls_pop: Option<Vec<u8>>,
    dir: PathBuf,
    run: Arc<Mutex<Option<PeerRun>>>,
    runs_count: Arc<AtomicUsize>,
    is_running: Arc<AtomicBool>,
    events: broadcast::Sender<PeerLifecycleEvent>,
    block_height: watch::Sender<Option<BlockHeight>>,
    stderr_live: Arc<StdMutex<LiveStderrState>>,
    startup_probe: Arc<StdMutex<PeerStartupProbe>>,
    start_context: Arc<StdMutex<Option<PeerStartContext>>>,
    // dropping these the last
    port_p2p: Arc<AllocatedPort>,
    port_api: Arc<AllocatedPort>,
}

impl NetworkPeer {
    fn record_probe_status(
        probe: &Arc<StdMutex<PeerStartupProbe>>,
        status: &Status,
    ) -> Option<PeerStatusSnapshot> {
        let snapshot = PeerStatusSnapshot::from(status);
        let mut probe = probe.lock().expect("startup probe should not be poisoned");
        probe.last_status = Some(snapshot.clone());
        probe.last_status_error = None;
        probe.last_status_unix_ms = Some(unix_timestamp_ms_now());
        Some(snapshot)
    }

    fn record_probe_error(probe: &Arc<StdMutex<PeerStartupProbe>>, error: &Report) {
        let mut probe = probe.lock().expect("startup probe should not be poisoned");
        probe.last_status_error = Some(snapshot_snippet(&format!("{error:?}")));
        probe.last_status_unix_ms = Some(unix_timestamp_ms_now());
    }

    fn last_status_peers(probe: &Arc<StdMutex<PeerStartupProbe>>) -> Option<u64> {
        probe
            .lock()
            .ok()
            .and_then(|probe| probe.last_status.as_ref().map(|snapshot| snapshot.peers))
    }
    fn startup_context_summary(&self) -> Option<String> {
        self.start_context
            .lock()
            .ok()
            .and_then(|context| context.as_ref().map(PeerStartContext::summary))
    }
    pub fn builder() -> NetworkPeerBuilder {
        NetworkPeerBuilder::new()
    }

    /// Spawn the child process.
    ///
    /// Passed configuration must contain network topology in the `trusted_peers` parameter.
    ///
    /// This function waits for peer server to start working,
    /// in particular it waits for `/status` response and connects to event stream.
    /// However it doesn't wait for genesis block to be committed.
    /// See [`Self::events`]/[`Self::once`]/[`Self::once_block`] to monitor peer's lifecycle.
    ///
    /// # Panics
    /// If peer was not started.
    pub async fn start<T: AsRef<Table>>(
        &self,
        config_layers: impl Iterator<Item = T>,
        genesis: Option<&GenesisBlock>,
    ) -> Result<()> {
        let preflight = preflight_bind_addresses([self.p2p_address(), self.api_address()]);
        if let Err(err) = preflight {
            return Err(err).wrap_err("preflight bind failed for peer");
        }

        let mut run_guard = self.run.lock().await;
        assert!(run_guard.is_none(), "already running");

        let run_num = self.runs_count.fetch_add(1, Ordering::Relaxed) + 1;
        let span = info_span!(parent: &self.span, "peer_run", run_num);
        let has_genesis = genesis.is_some();
        span.in_scope(|| info!(has_genesis, "Starting"));

        let storage_layers: Vec<Table> = config_layers.map(|layer| layer.as_ref().clone()).collect();
        let (storage_dir, storage_dir_key, storage_dir_value) =
            resolve_kura_store_dir(self, &storage_layers);

        self.prepare_kura_storage_dir(&storage_dir, has_genesis)?;

        {
            let mut live = self
                .stderr_live
                .lock()
                .expect("stderr live buffer should not be poisoned");
            live.reset(run_num);
        }
        {
            let mut probe = self
                .startup_probe
                .lock()
                .expect("startup probe should not be poisoned");
        *probe = PeerStartupProbe::default();
    }

        let config_layers: Vec<Table> = storage_layers;
        let config_path = self
            .write_run_config(config_layers.iter().map(Cow::Borrowed), genesis, run_num)
            .await?;
        let genesis_path = has_genesis.then(|| self.dir.join(format!("run-{run_num}-genesis.nrt")));
        let stdout_path = self.dir.join(format!("run-{run_num}-stdout.log"));
        let stderr_path = self.dir.join(format!("run-{run_num}-stderr.log"));
        {
            let mut startup_context = self
                .start_context
                .lock()
                .expect("startup context lock should not be poisoned");
            *startup_context = Some(PeerStartContext {
                run_num,
                config_path: config_path.clone(),
                genesis_path: genesis_path.clone(),
                stdout_path: stdout_path.clone(),
                stderr_path: stderr_path.clone(),
                kura_store_dir_key: storage_dir_key,
                kura_store_dir: storage_dir.clone(),
                kura_store_dir_value: storage_dir_value,
            });
        }
        let use_sora_profile = config_requires_sora_profile(&config_layers);

        let irohad = Program::Irohad.resolve_async().await?;
        let mut cmd = tokio::process::Command::new(irohad);
        strip_config_env_overrides(&mut cmd);
        cmd.stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .arg("--config")
            .arg(config_path)
            .arg("--terminal-colors=true");
        cmd.env("KURA_STORE_DIR", storage_dir.as_os_str());
        if use_sora_profile {
            cmd.arg("--sora");
        }
        if std::env::var_os("IROHA_SKIP_BIND_CHECKS").is_none() {
            cmd.env("IROHA_SKIP_BIND_CHECKS", "1");
        }
        cmd.current_dir(&self.dir);
        let mut child = cmd.spawn().wrap_err("failed to spawn `irohad`")?;
        let pid = child.id();
        let stderr_log_ready = Arc::new(Notify::new());
        let (fatal_tx, fatal_rx) = watch::channel(false);
        self.is_running.store(true, Ordering::Relaxed);
        let _ = self.events.send(PeerLifecycleEvent::Spawned);

        let mut tasks = JoinSet::<()>::new();

        {
            let tasks = &mut tasks;
            let fatal_rx = fatal_rx.clone();
            let is_running = self.is_running.clone();
            let output = child
                .stdout
                .take()
                .ok_or_else(|| eyre!("failed to capture child stdout"))?;
            let file = File::create(&stdout_path)
                .await
                .wrap_err("failed to create stdout log file")?;
            tasks.spawn(async move {
                drain_log_lines(output, file, fatal_rx, is_running, |_| {}, None, "stdout").await;
                // stdout logs are best-effort; no synchronization needed.
            });
        }
        {
            let tasks = &mut tasks;
            let span = span.clone();
            let fatal_rx = fatal_rx.clone();
            let is_running = self.is_running.clone();
            let output = child
                .stderr
                .take()
                .ok_or_else(|| eyre!("failed to capture child stderr"))?;
            let log_path = stderr_path.clone();
            let stderr_log_ready = Arc::clone(&stderr_log_ready);
            let stderr_live = Arc::clone(&self.stderr_live);
            tasks.spawn(async move {
                let buffer = PeerStderrBuffer::new(span, log_path.clone(), stderr_live);
                let file = match File::create(&log_path).await {
                    Ok(file) => file,
                    Err(err) => {
                        error!(?err, ?log_path, "failed to create stderr log file");
                        stderr_log_ready.notify_waiters();
                        return;
                    }
                };

                drain_log_lines(
                    output,
                    file,
                    fatal_rx,
                    is_running,
                    |line| buffer.push_line(line),
                    Some(stderr_log_ready),
                    "stderr",
                )
                .await;
            });
        }

        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
        let is_normal_shutdown_started = Arc::new(AtomicBool::new(false));
        let peer_exit = PeerExit {
            child,
            span: span.clone(),
            is_running: self.is_running.clone(),
            is_normal_shutdown_started: is_normal_shutdown_started.clone(),
            events: self.events.clone(),
            block_height: self.block_height.clone(),
            fatal_rx: fatal_rx.clone(),
            stderr_log_ready,
            stderr_live: self.stderr_live.clone(),
        };
        {
            let tasks = &mut tasks;
            tasks.spawn(
                async move {
                    if let Err(err) = peer_exit.monitor(shutdown_rx).await {
                        error!("something went very bad during peer exit monitoring: {err}");
                        panic!()
                    }
                }
                .instrument(span.clone()),
            );
        }

        {
            let tasks = &mut tasks;
            let client = self.client();
            let events_tx = self.events.clone();
            let block_height_tx = self.block_height.clone();
            let is_running = self.is_running.clone();
            let fatal_tx = fatal_tx.clone();
            let mut fatal_rx = fatal_rx.clone();
            let torii_addr = self.api_address().to_literal();
            let startup_probe = Arc::clone(&self.startup_probe);
            let startup_warn_gate = StartupWarnGate::new(STARTUP_STATUS_WARN_GRACE);
            tasks.spawn(
                async move {
                    let status_timeout = client_status_timeout_env();
                    let status_client = client.clone();
                    let storage_min_height = Arc::new(AtomicU64::new(0));
                    let mut last_progress: Instant;
                    let http_deadline = (status_timeout != Duration::ZERO)
                        .then(|| Instant::now() + status_timeout);
                    let mut http_gate = HttpStartGate::default();
                    let http_seen = Arc::new(AtomicBool::new(false));
                    if STARTUP_STATUS_WARN_GRACE > Duration::ZERO {
                        tokio::select! {
                            _ = tokio::time::sleep(STARTUP_STATUS_WARN_GRACE) => {}
                            changed = fatal_rx.changed() => {
                                if changed.is_ok() && *fatal_rx.borrow() {
                                    debug!("fatal notify received during startup grace");
                                }
                                return;
                            }
                        }
                    }
                    if *fatal_rx.borrow() {
                        return;
                    }
                    let warn_gate = startup_warn_gate.clone();
                    // Retry get_status with exponential backoff (50ms ..= 1s); abort if it takes
                    // longer than the configured timeout. If Torii is slow to accept connections,
                    // fall back to on-disk height observation so peers can still make progress.
                    let status_backoff = {
                        let storage_dir = storage_dir.clone();
                        let storage_min_height = Arc::clone(&storage_min_height);
                        let startup_probe = Arc::clone(&startup_probe);
                        let warn_gate = warn_gate.clone();
                        let http_seen = Arc::clone(&http_seen);
                        move || {
                            let client = status_client.clone();
                            let storage_dir = storage_dir.clone();
                            let min_height = storage_min_height.load(Ordering::Relaxed);
                            let startup_probe = Arc::clone(&startup_probe);
                            let warn_gate = warn_gate.clone();
                            let http_seen = Arc::clone(&http_seen);
                            async move {
                                let status = match spawn_blocking(move || client.get_status()).await
                                {
                                    Ok(status) => status,
                                    Err(join_error) => {
                                        let err = Report::new(join_error)
                                            .wrap_err("get status join failed");
                                        NetworkPeer::record_probe_error(&startup_probe, &err);
                                        log_status_warning(
                                            &warn_gate,
                                            || warn!(
                                                error = %err,
                                                debug = ?err,
                                                "get status failed"
                                            ),
                                            || debug!(
                                                error = %err,
                                                debug = ?err,
                                                "get status failed"
                                            ),
                                        );
                                        return Err(err);
                                    }
                                };
                                match status {
                                    Ok(status) => {
                                        let _ =
                                            NetworkPeer::record_probe_status(&startup_probe, &status);
                                        Ok((status, StatusSource::Http))
                                    }
                                    Err(err) => {
                                        NetworkPeer::record_probe_error(&startup_probe, &err);
                                        if status_error_is_connection_refused(&err)
                                            && !http_seen.load(Ordering::Relaxed)
                                        {
                                            debug!(
                                                error = %err,
                                                debug = ?err,
                                                "get status failed"
                                            );
                                        } else {
                                            log_status_warning(
                                                &warn_gate,
                                                || warn!(
                                                    error = %err,
                                                    debug = ?err,
                                                    "get status failed"
                                                ),
                                                || debug!(
                                                    error = %err,
                                                    debug = ?err,
                                                    "get status failed"
                                                ),
                                            );
                                        }
                                        if let Some(snapshot) =
                                            detect_block_height_from_storage(&storage_dir, min_height)
                                        {
                                            let mut status = Status {
                                                blocks: snapshot.total,
                                                blocks_non_empty: snapshot.non_empty,
                                                ..Status::default()
                                            };
                                            if let Some(peers) =
                                                NetworkPeer::last_status_peers(&startup_probe)
                                            {
                                                status.peers = peers;
                                                let _ = NetworkPeer::record_probe_status(
                                                    &startup_probe,
                                                    &status,
                                                );
                                            }
                                            log_status_warning(
                                                &warn_gate,
                                                || warn!(
                                                    snapshot = ?snapshot,
                                                    "using storage snapshot for initial status; Torii HTTP not reachable yet"
                                                ),
                                                || debug!(
                                                    snapshot = ?snapshot,
                                                    "using storage snapshot for initial status; Torii HTTP not reachable yet"
                                                ),
                                            );
                                            Ok((status, StatusSource::Storage))
                                        } else {
                                            Err(err)
                                        }
                                    }
                                }
                            }
                        }
                    };
                    let (status, source) = if status_timeout == Duration::ZERO {
                        tokio::select! {
                            status = retry_with_backoff(status_backoff) => status,
                            changed = fatal_rx.changed() => {
                                if changed.is_ok() && *fatal_rx.borrow() {
                                    debug!("fatal notify received while waiting for initial status");
                                }
                                return;
                            }
                        }
                    } else {
                        let status = tokio::select! {
                            status = retry_with_backoff_for(status_timeout, status_backoff) => status,
                            changed = fatal_rx.changed() => {
                                if changed.is_ok() && *fatal_rx.borrow() {
                                    debug!("fatal notify received while waiting for initial status");
                                }
                                return;
                            }
                        };
                        match status {
                            Ok(status) => status,
                            Err(_) => {
                                warn!(
                                    ?status_timeout,
                                    "timed out waiting for /status; falling back to storage snapshot"
                                );
                                let mut status = if let Some(snapshot) =
                                    detect_block_height_from_storage(&storage_dir, 0)
                                {
                                    Status {
                                        blocks: snapshot.total,
                                        blocks_non_empty: snapshot.non_empty,
                                        ..Status::default()
                                    }
                                } else {
                                    Status::default()
                                };
                                if let Some(peers) =
                                    NetworkPeer::last_status_peers(&startup_probe)
                                {
                                    status.peers = peers;
                                    let _ =
                                        NetworkPeer::record_probe_status(&startup_probe, &status);
                                }
                                (status, StatusSource::Storage)
                            }
                        }
                    };
                    last_progress = Instant::now();
                    let status_snapshot = status.clone();
                    let mut block_height = BlockHeight::from(status);
                    storage_min_height.store(block_height.total, Ordering::Relaxed);
                    if http_gate.on_status(source) {
                        http_seen.store(true, Ordering::Relaxed);
                        let _ = events_tx.send(PeerLifecycleEvent::ServerStarted);
                        info!(
                            ?status_snapshot,
                            torii_addr = %torii_addr.as_str(),
                            "server started via HTTP"
                        );
                    } else {
                        log_status_warning(
                            &warn_gate,
                            || warn!(
                                torii_addr = %torii_addr.as_str(),
                                ?status_snapshot,
                                "startup status derived from storage snapshot; waiting for Torii HTTP readiness"
                            ),
                            || debug!(
                                torii_addr = %torii_addr.as_str(),
                                ?status_snapshot,
                                "startup status derived from storage snapshot; waiting for Torii HTTP readiness"
                            ),
                        );
                    }
                    let _ = block_height_tx.send_replace(Some(block_height));

                    if block_height.total >= 1 {
                        info!(
                            snapshot = ?block_height,
                            "block watcher attached after genesis; subscribing from next height"
                        );
                        // Keep this task running so once_block* observers see future blocks.
                    }

                    // Avoid submitting synthetic transactions right after startup.
                    // Early side-effects here can cause racey counters in tests that fetch
                    // status via different codecs back-to-back.

                    loop {
                        const STATUS_FALLBACK_INTERVAL: Duration = Duration::from_millis(500);
                        let mut fallback_interval = tokio::time::interval(STATUS_FALLBACK_INTERVAL);
                        let poll_client = client.clone();

                        loop {
                            tokio::select! {
                                _ = fallback_interval.tick() => {
                                    if !is_running.load(Ordering::Relaxed) {
                                        break;
                                    }
                                    let poll_result = tokio::select! {
                                        result = spawn_blocking({
                                            let client = poll_client.clone();
                                            move || client.get_status()
                                        }) => result,
                                        changed = fatal_rx.changed() => {
                                            if changed.is_ok() && *fatal_rx.borrow() {
                                                debug!("fatal notify received during status poll");
                                            }
                                            return;
                                        }
                                    };
                                    let status = match poll_result {
                                        Ok(result) => result,
                                        Err(err) => {
                                            if warn_gate.should_warn() {
                                                warn!(error = %err, debug = ?err, "fallback status poll join error");
                                            } else {
                                                debug!(error = %err, debug = ?err, "fallback status poll join error");
                                            }
                                            continue;
                                        }
                                    };
                                    let status = match status {
                                        Ok(status) => {
                                            if http_gate.on_status(StatusSource::Http) {
                                                http_seen.store(true, Ordering::Relaxed);
                                                let _ = events_tx.send(PeerLifecycleEvent::ServerStarted);
                                                info!(
                                                    torii_addr = %torii_addr.as_str(),
                                                    "Torii HTTP became reachable"
                                                );
                                            }
                                            last_progress = Instant::now();
                                            let _ = NetworkPeer::record_probe_status(
                                                &startup_probe,
                                                &status,
                                            );
                                            status
                                        }
                                        Err(err) => {
                                        if status_error_is_connection_refused(&err)
                                            && !http_seen.load(Ordering::Relaxed)
                                        {
                                            debug!(
                                                error = %err,
                                                debug = ?err,
                                                "fallback status poll failed"
                                            );
                                        } else if warn_gate.should_warn() {
                                            warn!(
                                                error = %err,
                                                debug = ?err,
                                                "fallback status poll failed"
                                            );
                                        } else {
                                            debug!(
                                                error = %err,
                                                debug = ?err,
                                                "fallback status poll failed"
                                            );
                                        }
                                        // Fall back to on-disk observation so scenarios can progress even if Torii
                                        // is slow to accept HTTP connections.
                                        if let Some(snapshot) =
                                            detect_block_height_from_storage(&storage_dir, block_height.total)
                                                && (snapshot.total > block_height.total
                                                    || snapshot.non_empty > block_height.non_empty)
                                        {
                                            if let Some(peers) =
                                                NetworkPeer::last_status_peers(&startup_probe)
                                            {
                                                let status = Status {
                                                    blocks: snapshot.total,
                                                    blocks_non_empty: snapshot.non_empty,
                                                    peers,
                                                    ..Status::default()
                                                };
                                                let _ = NetworkPeer::record_probe_status(
                                                    &startup_probe,
                                                    &status,
                                                );
                                            }
                                            block_height = snapshot;
                                            block_height_tx.send_modify(|slot| match slot {
                                                Some(current) => *current = snapshot,
                                                None => *slot = Some(snapshot),
                                            });
                                            storage_min_height
                                                .store(block_height.total, Ordering::Relaxed);
                                            last_progress = Instant::now();
                                            continue;
                                        }
                                        if !http_gate.http_seen()
                                            && let Some(deadline) = http_deadline
                                            && Instant::now() >= deadline
                                        {
                                            warn!(
                                                torii_addr = %torii_addr.as_str(),
                                                ?status_timeout,
                                                "Torii HTTP never became reachable; requesting shutdown"
                                            );
                                            let _ = fatal_tx.send(true);
                                            return;
                                        }
                                        if status_timeout != Duration::ZERO
                                            && last_progress.elapsed() >= status_timeout
                                        {
                                            warn!(?status_timeout, "status watchdog expired; requesting shutdown");
                                            let _ = fatal_tx.send(true);
                                            return;
                                        }
                                        continue;
                                    }
                                };
                                    let snapshot = BlockHeight::from(status);
                                    if snapshot.total > block_height.total
                                        || snapshot.non_empty > block_height.non_empty
                                    {
                                        block_height = snapshot;
                                        storage_min_height
                                            .store(block_height.total, Ordering::Relaxed);
                                        block_height_tx.send_modify(|slot| match slot {
                                            Some(current) => {
                                                *current = snapshot;
                                            }
                                            None => *slot = Some(snapshot),
                                        });
                                    }
                                }
                                changed = fatal_rx.changed() => {
                                    if changed.is_ok() && *fatal_rx.borrow() {
                                        debug!("fatal notify received in blocks watchdog");
                                    }
                                    return;
                                }
                            }
                        }
                        if is_normal_shutdown_started.load(Ordering::Relaxed) {
                            info!("block stream closed normally after shutdown");
                            break
                        } else {
                            debug!("blocks stream closed without shutdown; retrying soon");
                            const RETRY: Duration = Duration::from_millis(1000);
                            tokio::time::sleep(RETRY).await;
                        }
                    }
                }
                .instrument(span),
            );
        }

        *run_guard = Some(PeerRun {
            tasks,
            shutdown: shutdown_tx,
            fatal_tx: fatal_tx.clone(),
            pid,
        });
        Ok(())
    }

    /// Forcefully kills the running peer
    ///
    /// # Panics
    /// If peer was not started.
    pub async fn shutdown(&self) {
        let mut guard = self.run.lock().await;
        let Some(mut run) = (*guard).take() else {
            panic!("peer is not running, nothing to shut down");
        };
        // Immediately drop the running flag so watchdog loops and status polls exit promptly.
        self.is_running.store(false, Ordering::Relaxed);
        // Wake any background watchers so they stop promptly during shutdown.
        let _ = run.fatal_tx.send(true);
        let _ = run.shutdown.send(());
        let join_all = async {
            while let Some(res) = run.tasks.join_next().await {
                if let Err(err) = res {
                    if err.is_cancelled() {
                        debug!("run task cancelled during shutdown");
                    } else if err.is_panic() {
                        warn!(error = %err, "run task panicked during shutdown");
                    }
                }
            }
        };
        if timeout(PEER_SHUTDOWN_TIMEOUT, join_all).await.is_err() {
            warn!("timed out waiting for peer tasks; aborting remaining tasks");
            if let Some(pid) = run.pid.filter(|pid| *pid > 0) {
                #[cfg(target_family = "unix")]
                {
                    use nix::{sys::signal, unistd::Pid};
                    if let Err(err) =
                        signal::kill(Pid::from_raw(pid as i32), signal::Signal::SIGKILL)
                    {
                        warn!(pid, error = %err, "failed to force-kill hung peer process");
                    }
                }
                #[cfg(not(target_family = "unix"))]
                {
                    warn!(
                        pid,
                        "unable to force-kill hung peer process on this platform"
                    );
                }
            }
            run.tasks.abort_all();
            let drain_aborted = async {
                while let Some(res) = run.tasks.join_next().await {
                    if let Err(err) = res
                        && err.is_panic()
                    {
                        warn!(error = %err, "aborted task panicked during shutdown");
                    }
                }
            };
            if timeout(PEER_SHUTDOWN_TIMEOUT, drain_aborted).await.is_err() {
                warn!("timed out waiting for aborted peer tasks; continuing shutdown");
            }
        }
    }

    /// Like [`Self::start`], but also ensures that server starts (responds to `/status`).
    ///
    /// Note: This method does not wait for the genesis block to be committed even if
    /// a genesis is provided. Use higher-level helpers (e.g., `Network::start_all` or
    /// explicit `once_block`) if you need to wait for block commits.
    pub async fn start_checked<T: AsRef<Table>>(
        &self,
        config_layers: impl Iterator<Item = T>,
        genesis: Option<&GenesisBlock>,
    ) -> Result<()> {
        let events = self.events();
        self.start(config_layers, genesis).await?;
        let context = self
            .startup_context_summary()
            .unwrap_or_else(|| "<startup context not initialized>".to_string());
        match wait_for_start_event(events).await {
            Some(PeerLifecycleEvent::ServerStarted) => Ok(()),
            Some(PeerLifecycleEvent::Terminated { status }) => {
                let err = if let Some(preview) = self.stderr_preview() {
                    eyre!("Peer exited unexpectedly ({status:?}); {context}; stderr preview:\n{preview}")
                } else {
                    eyre!("Peer exited unexpectedly ({status:?}); {context}")
                };
                Err(err)
            }
            Some(PeerLifecycleEvent::Killed) => {
                let err = if let Some(preview) = self.stderr_preview() {
                    eyre!("Peer was killed before startup; {context}; stderr preview:\n{preview}")
                } else {
                    eyre!("Peer was killed before startup; {context}")
                };
                Err(err)
            }
            None => Err(eyre!("Peer event channel closed before startup")),
            Some(PeerLifecycleEvent::Spawned | PeerLifecycleEvent::BlockApplied { .. }) => {
                unreachable!("wait_for_start_event filters out intermediate lifecycle events")
            }
        }
    }

    /// Subscribe on peer lifecycle events.
    pub fn events(&self) -> broadcast::Receiver<PeerLifecycleEvent> {
        self.events.subscribe()
    }

    /// Wait _once_ an event matches a predicate.
    ///
    /// ```ignore
    /// use iroha_test_network::{Network, NetworkBuilder, PeerLifecycleEvent};
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let network = NetworkBuilder::new().build();
    ///     let peer = network.peer();
    ///
    ///     tokio::join!(
    ///         peer.start(network.config_layers(), None),
    ///         peer.once(|event| matches!(event, PeerLifecycleEvent::ServerStarted))
    ///     );
    /// }
    /// ```
    ///
    /// It is a narrowed version of [`Self::events`].
    pub async fn once<F>(&self, f: F)
    where
        F: Fn(PeerLifecycleEvent) -> bool,
    {
        let mut rx = self.events();
        loop {
            tokio::select! {
                Ok(event) = rx.recv() => {
                    if f(event) { break }
                }
            }
        }
    }

    /// Wait until peer's block height reaches N (total blocks, including genesis).
    ///
    /// Resolves immediately if peer is already running _and_ has at least N blocks committed. This
    /// treats the genesis block as progress even if it is empty, avoiding hangs when waiting for
    /// `once_block(1)` on nodes that commit a structurally empty genesis.
    pub async fn once_block(&self, n: u64) {
        self.once_block_with(|height| height.total >= n).await
    }

    /// Wait until peer's block height passes the given predicate.
    ///
    /// Resolves immediately if peer is running _and_ the predicate passes.
    pub async fn once_block_with<F: Fn(BlockHeight) -> bool>(&self, f: F) {
        let mut recv = self.block_height.subscribe();

        if recv.borrow().map(&f).unwrap_or(false) {
            return;
        }

        if let Some(snapshot) = self.best_effort_block_height()
            && f(snapshot)
        {
            return;
        }

        let mut storage_poll = tokio::time::interval(Duration::from_millis(250));
        loop {
            tokio::select! {
                changed = recv.changed() => {
                    changed.expect("could fail only if the peer is dropped");

                    if recv.borrow_and_update().map(&f).unwrap_or(false) {
                        break;
                    }
                }
                _ = storage_poll.tick() => {
                    if let Some(snapshot) = self
                        .best_effort_block_height()
                        .filter(|snapshot| f(*snapshot))
                    {
                        self.block_height.send_modify(|slot| match slot {
                            Some(current) => {
                                if snapshot.total > current.total
                                    || snapshot.non_empty > current.non_empty
                                {
                                    *current = snapshot;
                                }
                            }
                            None => *slot = Some(snapshot),
                        });
                        break;
                    }
                }
            }
        }
    }

    /// Generated mnemonic string, useful for logs
    pub fn mnemonic(&self) -> &str {
        &self.mnemonic
    }

    fn has_committed_block(&self, height: u64) -> bool {
        if height == 0 {
            return false;
        }
        let storage_dir = self.dir.join("storage");
        pipeline_dirs(&storage_dir)
            .into_iter()
            .any(|dir| self.has_indexed_pipeline_sidecar(&dir, height))
    }

    fn has_indexed_pipeline_sidecar(&self, pipeline_dir: &Path, height: u64) -> bool {
        if height == 0 {
            return false;
        }
        let data_path = pipeline_dir.join(PIPELINE_SIDECARS_DATA_FILE);
        let index_path = pipeline_dir.join(PIPELINE_SIDECARS_INDEX_FILE);
        let Ok(index_meta) = std::fs::metadata(&index_path) else {
            return false;
        };
        if !data_path.exists() {
            return false;
        }
        let len = index_meta.len();
        if len == 0 || len % PIPELINE_INDEX_ENTRY_SIZE_U64 != 0 {
            return false;
        }
        let entries = len / PIPELINE_INDEX_ENTRY_SIZE_U64;
        if entries < height {
            return false;
        }

        let mut index = match std::fs::File::open(&index_path) {
            Ok(file) => file,
            Err(_) => return false,
        };
        let mut buf = [0u8; PIPELINE_INDEX_ENTRY_SIZE];
        let offset = (height - 1) * PIPELINE_INDEX_ENTRY_SIZE_U64;
        if index
            .seek(SeekFrom::Start(offset))
            .and_then(|_| index.read_exact(&mut buf))
            .is_err()
        {
            return false;
        }
        let len = u64::from_le_bytes(buf[8..].try_into().expect("len slice"));
        len != 0
    }

    fn pipeline_height_from_index(pipeline_dir: &Path) -> Option<u64> {
        let index_path = pipeline_dir.join(PIPELINE_SIDECARS_INDEX_FILE);
        let index_meta = fs::metadata(&index_path).ok()?;
        let len = index_meta.len();
        if len < PIPELINE_INDEX_ENTRY_SIZE_U64 || len % PIPELINE_INDEX_ENTRY_SIZE_U64 != 0 {
            return None;
        }

        let mut index = fs::File::open(&index_path).ok()?;
        let mut buf = [0u8; PIPELINE_INDEX_ENTRY_SIZE];
        let offset = len.saturating_sub(PIPELINE_INDEX_ENTRY_SIZE_U64);
        if index
            .seek(SeekFrom::Start(offset))
            .and_then(|_| index.read_exact(&mut buf))
            .is_err()
        {
            return None;
        }
        let last_len = u64::from_le_bytes(buf[8..].try_into().expect("len slice"));
        if last_len == 0 {
            return None;
        }

        Some(len / PIPELINE_INDEX_ENTRY_SIZE_U64)
    }

    pub fn public_key(&self) -> &PublicKey {
        self.key_pair.public_key()
    }

    pub fn streaming_key_pair(&self) -> &KeyPair {
        &self.streaming_key_pair
    }

    pub fn streaming_public_key(&self) -> &PublicKey {
        self.streaming_key_pair.public_key()
    }

    pub fn bls_key_pair(&self) -> Option<&KeyPair> {
        self.bls_key_pair.as_ref()
    }

    pub fn bls_public_key(&self) -> Option<&PublicKey> {
        self.bls_key_pair.as_ref().map(KeyPair::public_key)
    }

    pub fn bls_pop(&self) -> Option<&[u8]> {
        self.bls_pop.as_deref()
    }

    pub fn genesis_pop(&self) -> Option<GenesisTopologyEntry> {
        self.bls_public_key().and_then(|pk| {
            self.bls_pop()
                .map(|pop| GenesisTopologyEntry::new(PeerId::new(pk.clone()), pop.to_vec()))
        })
    }

    /// Generated [`PeerId`]
    pub fn id(&self) -> PeerId {
        self.network_peer_id()
    }

    /// [`PeerId`] representing the network identity (Ed25519) used for Torii/P2P.
    pub fn network_peer_id(&self) -> PeerId {
        PeerId::new(self.key_pair.public_key().clone())
    }

    pub fn p2p_address(&self) -> SocketAddr {
        socket_addr!(127.0.0.1:**self.port_p2p)
    }

    /// Torii HTTP API socket address (host + port).
    pub fn api_address(&self) -> SocketAddr {
        socket_addr!(127.0.0.1:**self.port_api)
    }

    /// Torii HTTP URL for this peer, e.g. `http://127.0.0.1:8080`.
    pub fn torii_url(&self) -> String {
        format!("http://{}", self.api_address())
    }

    /// Path to this peer's Kura store directory.
    ///
    /// By default tests configure Kura with `store_dir = "./storage"` relative to the peer run dir.
    /// This helper returns `<peer_dir>/storage` matching that configuration.
    pub fn kura_store_dir(&self) -> PathBuf {
        if let Ok(context) = self.start_context.lock() {
            if let Some(context) = context.as_ref() {
                return context.kura_store_dir.clone();
            }
        }
        self.dir.join("storage")
    }

    fn prepare_kura_storage_dir(&self, storage_dir: &Path, reset_for_bootstrap: bool) -> Result<()> {
        if reset_for_bootstrap {
            match fs::symlink_metadata(storage_dir) {
                Ok(meta) => {
                    if meta.is_dir() && !meta.file_type().is_symlink() {
                        fs::remove_dir_all(storage_dir).wrap_err_with(|| {
                            format!(
                                "failed to clear storage directory {} before bootstrap",
                                storage_dir.display()
                            )
                        })?;
                    } else {
                        fs::remove_file(storage_dir).or_else(|err| {
                            if err.kind() == ErrorKind::NotFound {
                                Ok(())
                            } else {
                                Err(err)
                            }
                        })?;
                    }
                }
                Err(err) if err.kind() != ErrorKind::NotFound => {
                    return Err(err).wrap_err_with(|| {
                        format!(
                            "failed to inspect storage path {} before bootstrap",
                            storage_dir.display()
                        )
                    });
                }
                Err(_) => {}
            }
        }

        match fs::symlink_metadata(storage_dir) {
            Ok(meta) => {
                if !meta.is_dir() {
                    fs::remove_file(storage_dir)
                        .or_else(|err| {
                            if err.kind() == ErrorKind::IsADirectory {
                                fs::remove_dir_all(storage_dir)
                            } else {
                                Err(err)
                            }
                        })
                        .wrap_err_with(|| {
                            format!(
                                "failed to remove non-directory path at {} before Kura startup",
                                storage_dir.display()
                            )
                        })?;
                }
            }
            Err(err) if err.kind() != ErrorKind::NotFound => {
                return Err(err).wrap_err_with(|| {
                    format!("failed to inspect storage path {}", storage_dir.display())
                })
            }
            Err(_) => {}
        }

        fs::create_dir_all(storage_dir).wrap_err_with(|| {
            format!(
                "failed to prepare Kura storage directory at {}",
                storage_dir.display()
            )
        })
    }

    fn storage_snapshot(&self) -> PeerStorageSnapshot {
        let kura_dir = self.kura_store_dir();
        let has_block_1 = self.has_committed_block(1);
        PeerStorageSnapshot::capture(kura_dir, has_block_1)
    }

    /// Check whether the peer is running
    pub fn is_running(&self) -> bool {
        self.is_running.load(Ordering::Relaxed)
    }

    /// Create a client to interact with this peer
    pub fn client_for(&self, account_id: &AccountId, account_private_key: PrivateKey) -> Client {
        println!(
            "TEST_NETWORK client for {} using port {}",
            self.mnemonic, self.port_api
        );
        let status_timeout = client_status_timeout_env();
        let request_timeout = client_request_timeout_env();
        let ttl = client_ttl_env(status_timeout);
        let config = ConfigReader::new()
            .with_toml_source(TomlSource::inline(
                Table::new()
                    .write("chain", config::chain_id().to_string())
                    .write(["account", "domain"], account_id.domain().to_string())
                    .write(
                        ["account", "public_key"],
                        account_id.signatory().to_string(),
                    )
                    .write(
                        ["account", "private_key"],
                        ExposedPrivateKey(account_private_key.clone()).to_string(),
                    )
                    .write(
                        ["transaction", "status_timeout_ms"],
                        i64::try_from(status_timeout.as_millis())
                            .expect("status timeout fits in i64"),
                    )
                    .write(
                        "torii_request_timeout_ms",
                        i64::try_from(request_timeout.as_millis())
                            .expect("request timeout fits in i64"),
                    )
                    .write(
                        ["transaction", "time_to_live_ms"],
                        i64::try_from(ttl.as_millis()).expect("ttl fits in i64"),
                    )
                    .write("torii_url", format!("http://127.0.0.1:{}", self.port_api)),
            ))
            .read_and_complete::<iroha::config::UserConfig>()
            .expect("peer client config should be valid")
            .parse()
            .expect("peer client config should be valid");

        let mut client = Client::new(config);
        client.set_operator_key_pair(self.key_pair.clone());
        client
    }

    /// Client for Alice. ([`Self::client_for`] + [`Signatory::Alice`])
    pub fn client(&self) -> Client {
        self.client_for(&ALICE_ID, ALICE_KEYPAIR.private_key().clone())
    }

    pub async fn status(&self) -> Result<Status> {
        let client = self.client();
        let result = spawn_blocking(move || client.get_status())
            .await
            .expect("should not panic");
        match &result {
            Ok(status) => self.record_status_success(status),
            Err(error) => self.record_status_failure(error),
        }
        result
    }

    fn record_status_success(&self, status: &Status) {
        let _ = Self::record_probe_status(&self.startup_probe, status);
    }

    fn record_status_failure(&self, error: &Report) {
        Self::record_probe_error(&self.startup_probe, error);
    }

    /// Best-effort block height based on the latest in-memory observation and disk layout.
    ///
    /// Prefer in-memory updates from the block watcher, then committed block hashes on disk.
    /// Pipeline sidecar indices are only used when no hashes are available.
    pub fn best_effort_block_height(&self) -> Option<BlockHeight> {
        let observed = *self.block_height.borrow();
        let current_total = observed.map(|height| height.total).unwrap_or(0);
        let from_storage = detect_block_height_from_storage(&self.kura_store_dir(), current_total);
        match (observed, from_storage) {
            (Some(current), Some(storage)) => Some(BlockHeight {
                total: current.total.max(storage.total),
                non_empty: current.non_empty.max(storage.non_empty),
            }),
            (Some(current), None) => Some(current),
            (None, Some(storage)) => Some(storage),
            (None, None) => None,
        }
    }

    /// Last observed peer count from `/status`, if any.
    pub fn last_known_peers(&self) -> Option<u64> {
        self.startup_probe
            .lock()
            .ok()
            .and_then(|probe| probe.last_status.as_ref().map(|snapshot| snapshot.peers))
    }

    /// Path to the most recent stdout log file for this peer run, if any.
    pub fn latest_stdout_log_path(&self) -> Option<PathBuf> {
        self.latest_run_log_path_suffix("stdout")
    }

    /// Path to the most recent stderr log file for this peer run, if any.
    pub fn latest_stderr_log_path(&self) -> Option<PathBuf> {
        self.latest_run_log_path_suffix("stderr")
    }

    fn latest_run_log_path_suffix(&self, which: &str) -> Option<PathBuf> {
        // Files are named as run-<n>-stdout.log or run-<n>-stderr.log
        let mut best: Option<(usize, PathBuf)> = None;
        if let Ok(read) = std::fs::read_dir(&self.dir) {
            for entry in read.flatten() {
                let path = entry.path();
                if let Some(name) = path.file_name().and_then(|s| s.to_str()) {
                    // quick prefix/suffix match
                    if name.starts_with("run-") && name.ends_with(&format!("-{which}.log")) {
                        // extract the number between run- and -<which>.log
                        let mid = &name[4..name.len() - (which.len() + 5)];
                        // mid is like "<n>"
                        if let Ok(n) = mid.parse::<usize>() {
                            match best {
                                Some((best_n, _)) if n <= best_n => {}
                                _ => best = Some((n, path.clone())),
                            }
                        }
                    }
                }
            }
        }
        best.map(|(_, p)| p)
    }

    /// Snapshot the most recent stderr output captured for this peer run.
    ///
    /// Returns a short preview (last few lines) to avoid flooding logs.
    fn stderr_preview(&self) -> Option<String> {
        let guard = self
            .stderr_live
            .lock()
            .expect("stderr live buffer should not be poisoned");
        summarize_peer_stderr(&guard.buffer).map(|summary| summary.preview)
    }

    fn log_snapshot(&self) -> PeerLogSnapshot {
        let stdout_log = self.latest_stdout_log_path();
        let stderr_log = self.latest_stderr_log_path();
        let (stderr_run_id, summary) = {
            let guard = self
                .stderr_live
                .lock()
                .expect("stderr live buffer should not be poisoned");
            (guard.run_id, summarize_peer_stderr(&guard.buffer))
        };
        let stderr_preview_line_count = summary.as_ref().map(|inner| inner.preview.lines().count());
        let stderr_total_lines = summary.as_ref().map(|inner| inner.total_lines);
        let stderr_truncated = summary.as_ref().is_some_and(|inner| inner.truncated);
        PeerLogSnapshot {
            stdout_log,
            stderr_log,
            stderr_preview: summary.map(|inner| inner.preview),
            stderr_preview_line_count,
            stderr_total_lines,
            stderr_truncated,
            stderr_run_id,
        }
    }

    pub fn blocks(&self) -> watch::Receiver<Option<BlockHeight>> {
        self.block_height.subscribe()
    }

    fn startup_state(&self, index: usize) -> PeerStartupState {
        let receiver = self.blocks();
        let last_block = *receiver.borrow();
        let probe = self
            .startup_probe
            .lock()
            .expect("startup probe should not be poisoned")
            .clone();
        PeerStartupState {
            index,
            mnemonic: self.mnemonic().to_string(),
            is_running: self.is_running(),
            last_block,
            logs: self.log_snapshot(),
            status_snapshot: probe.last_status,
            status_error: probe.last_status_error,
            status_unix_timestamp_ms: probe.last_status_unix_ms,
            storage: self.storage_snapshot(),
        }
    }

    fn write_base_config(&self) {
        let cfg = self.base_config_table();
        std::fs::write(
            self.dir.join("config.base.toml"),
            toml::to_string(&cfg).unwrap(),
        )
        .unwrap();
        self.ensure_rans_tables();
    }

    fn base_config_table(&self) -> Table {
        let p2p_literal = self.p2p_address().to_literal();
        let torii_literal = self.api_address().to_literal();
        Table::new()
            .write("public_key", self.key_pair.public_key().to_string())
            .write(
                "private_key",
                ExposedPrivateKey(self.key_pair.private_key().clone()).to_string(),
            )
            .write(
                ["streaming", "identity_public_key"],
                self.streaming_public_key().to_string(),
            )
            .write(
                ["streaming", "identity_private_key"],
                ExposedPrivateKey(self.streaming_key_pair.private_key().clone()).to_string(),
            )
            .write(["network", "address"], p2p_literal.clone())
            .write(["network", "public_address"], p2p_literal)
            .write(["torii", "address"], torii_literal)
            // Allow larger uploads for DA/IBC-heavy fixtures.
            .write(
                ["torii", "max_content_len"],
                toml::Value::Integer(16 * 1024 * 1024),
            )
    }

    fn ensure_rans_tables(&self) {
        let src = default_rans_tables_path();
        assert!(
            src.exists(),
            "missing codec rANS tables at {}; ensure codec/rans/tables/rans_seed0.toml is present",
            src.display()
        );
        let dst = self
            .dir
            .join("codec")
            .join("rans")
            .join("tables")
            .join("rans_seed0.toml");
        if let Some(parent) = dst.parent() {
            std::fs::create_dir_all(parent).expect("create codec/rans/tables dir");
        }
        std::fs::copy(src, dst).expect("copy deterministic rANS tables into peer dir");
    }

    fn canonical_genesis_bytes(block: &GenesisBlock) -> Result<Vec<u8>> {
        let framed = block
            .0
            .encode_wire()
            .map_err(color_eyre::Report::new)
            .wrap_err("encode genesis block with Norito header")?;
        let deframed =
            iroha_data_model::block::deframe_versioned_signed_block_bytes(framed.as_slice())
                .map_err(color_eyre::Report::new)
                .wrap_err("deframe genesis sanity check")?;
        let versioned = block.0.encode_versioned();
        assert_eq!(deframed.bare_versioned.as_ref(), versioned.as_slice());
        Ok(framed)
    }

    async fn write_run_config<T: AsRef<Table>>(
        &self,
        cfg_extra_layers: impl Iterator<Item = T>,
        genesis: Option<&GenesisBlock>,
        run: usize,
    ) -> Result<PathBuf> {
        // Recreate the base layer for every run to avoid stale/missing configs
        // when previous runs left the directory partially populated.
        self.write_base_config();

        let extra_layers: Vec<_> = cfg_extra_layers
            .enumerate()
            .map(|(i, table)| {
                let mut owned = table.as_ref().clone();
                apply_debug_rbc_defaults(&mut owned);
                (format!("run-{run}-config.layer-{i}.toml"), owned)
            })
            .collect();

        for (path, table) in &extra_layers {
            tokio::fs::write(self.dir.join(path), toml::to_string(table)?).await?;
        }

        let mut final_config = Table::new().write(
            "extends",
            // should be written on peer's initialisation
            iter::once("config.base.toml".to_string())
                .chain(extra_layers.into_iter().map(|(path, _)| path))
                .collect::<Vec<String>>(),
        );
        if let Some(block) = genesis {
            let path = self.dir.join(format!("run-{run}-genesis.nrt"));
            final_config =
                final_config.write(["genesis", "file"], path.to_string_lossy().to_string());
            // Ensure instruction/type registries are initialized before encoding.
            init_instruction_registry();
            let framed = Self::canonical_genesis_bytes(block)?;
            tokio::fs::write(path, framed).await?;
        }
        let path = self.dir.join(format!("run-{run}-config.toml"));
        tokio::fs::write(&path, toml::to_string(&final_config)?).await?;

        Ok(path)
    }
}

/// Retry an async operation with exponential backoff.
///
/// - Starts at 50ms and doubles up to a 1s cap.
/// - Retries indefinitely until the operation returns `Ok`.
async fn retry_with_backoff_for<F, Fut, T, E>(
    duration: Duration,
    op: F,
) -> Result<T, tokio::time::error::Elapsed>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, E>>,
{
    timeout(duration, retry_with_backoff(op)).await
}

async fn retry_with_backoff<F, Fut, T, E>(mut op: F) -> T
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, E>>,
{
    let mut delay = Duration::from_millis(50);
    loop {
        match op().await {
            Ok(value) => return value,
            Err(_) => {
                tokio::time::sleep(delay).await;
                delay = core::cmp::min(delay.saturating_mul(2), Duration::from_secs(1));
            }
        }
    }
}

/// Compare by ID
impl PartialEq for NetworkPeer {
    fn eq(&self, other: &Self) -> bool {
        self.key_pair.eq(&other.key_pair)
    }
}

pub struct NetworkPeerBuilder {
    mnemonic: String,
    seed: Option<Vec<u8>>,
}

impl NetworkPeerBuilder {
    #[allow(clippy::new_without_default)] // has side effects
    pub fn new() -> Self {
        Self {
            // `petname` may occasionally yield multi-word parts or stray newline characters.
            // Replace all whitespace with underscores to conform to `Name` restrictions.
            mnemonic: petname::petname(2, "_")
                .unwrap()
                .split_whitespace()
                .collect::<Vec<_>>()
                .join("_"),
            seed: None,
        }
    }

    pub fn with_seed(mut self, seed: Option<impl Into<Vec<u8>>>) -> Self {
        self.seed = seed.map(Into::into);
        self
    }

    pub fn build(self, env: &Environment) -> NetworkPeer {
        let NetworkPeerBuilder { mnemonic, seed } = self;

        let streaming_key_pair = seed
            .as_ref()
            .map(|seed_bytes| KeyPair::from_seed(seed_bytes.clone(), Algorithm::Ed25519))
            .unwrap_or_else(KeyPair::random);

        let bls_key = if let Some(mut seed_bytes) = seed.clone() {
            seed_bytes.extend_from_slice(b":bls");
            KeyPair::from_seed(seed_bytes, Algorithm::BlsNormal)
        } else {
            KeyPair::random_with_algorithm(Algorithm::BlsNormal)
        };
        let pop =
            iroha_crypto::bls_normal_pop_prove(bls_key.private_key()).expect("BLS PoP generation");
        let key_pair = bls_key.clone();
        let bls_key_pair = Some(bls_key);
        let bls_pop = Some(pop);
        let port_p2p = AllocatedPort::new();
        let port_api = AllocatedPort::new();

        let dir = env.dir.join(&mnemonic);
        std::fs::create_dir_all(&dir).unwrap();
        println!("TEST_NETWORK peer dir {} -> {}", mnemonic, dir.display());

        let (events, _rx) = broadcast::channel(32);
        let (block_height, _rx) = watch::channel(None);

        let span = info_span!("peer", mnemonic);
        span.in_scope(|| {
            info!(
                dir=%dir.display(),
                port_p2p=%port_p2p,
                port_api=%port_api,
                "Build peer",
            )
        });

        let peer = NetworkPeer {
            mnemonic,
            span,
            key_pair,
            streaming_key_pair,
            bls_key_pair,
            bls_pop,
            dir,
            run: Default::default(),
            runs_count: Default::default(),
            is_running: Default::default(),
            events,
            block_height,
            stderr_live: Arc::new(StdMutex::new(LiveStderrState::default())),
            startup_probe: Arc::new(StdMutex::new(PeerStartupProbe::default())),
            start_context: Arc::new(StdMutex::new(None)),
            port_p2p: Arc::new(port_p2p),
            port_api: Arc::new(port_api),
        };
        peer.write_base_config();
        peer
    }
}

/// Prints collected STDERR on drop.
///
/// Used to avoid loss of useful data in case of task abortion before it is printed directly.
struct PeerStderrBuffer {
    span: tracing::Span,
    buffer: Arc<StdMutex<LiveStderrState>>,
    log_path: PathBuf,
}

const PEER_STDERR_PREVIEW_MAX_LINES: usize = 25;
const PEER_STDERR_PREVIEW_MAX_CHARS: usize = 3_072;

struct StderrSummary {
    preview: String,
    truncated: bool,
    total_lines: usize,
}

fn summarize_peer_stderr(buffer: &str) -> Option<StderrSummary> {
    let trimmed = buffer.trim_end_matches('\n');
    if trimmed.is_empty() {
        return None;
    }

    let total_lines = trimmed.lines().count();
    let start_line = total_lines.saturating_sub(PEER_STDERR_PREVIEW_MAX_LINES);
    let mut truncated = start_line > 0;

    let mut preview = trimmed
        .lines()
        .skip(start_line)
        .collect::<Vec<_>>()
        .join("\n");

    let preview_char_count = preview.chars().count();
    if preview_char_count > PEER_STDERR_PREVIEW_MAX_CHARS {
        truncated = true;
        let mut tail: Vec<char> = preview
            .chars()
            .rev()
            .take(PEER_STDERR_PREVIEW_MAX_CHARS)
            .collect();
        tail.reverse();
        preview = tail.into_iter().collect();
    }

    Some(StderrSummary {
        preview,
        truncated,
        total_lines,
    })
}

impl PeerStderrBuffer {
    fn new(span: tracing::Span, log_path: PathBuf, buffer: Arc<StdMutex<LiveStderrState>>) -> Self {
        Self {
            span,
            buffer,
            log_path,
        }
    }

    fn push_line(&self, line: &str) {
        if let Ok(mut guard) = self.buffer.lock() {
            guard.push_line(line);
        }
    }
}

impl Drop for PeerStderrBuffer {
    fn drop(&mut self) {
        if let Ok(guard) = self.buffer.lock()
            && let Some(summary) = summarize_peer_stderr(&guard.buffer)
        {
            self.span.in_scope(|| {
                info!(
                    run = ?guard.run_id,
                    path = %self.log_path.display(),
                    total_lines = summary.total_lines,
                    truncated = summary.truncated,
                    "peer emitted stderr; full contents stored on disk"
                );
                if !summary.preview.is_empty() {
                    debug!(
                        run = ?guard.run_id,
                        truncated = summary.truncated,
                        preview_lines = summary.preview.lines().count(),
                        preview = %summary.preview,
                        "peer stderr tail preview"
                    );
                }
            });
        }
    }
}

#[cfg(test)]
mod post_genesis_liveness_tests {
    use tokio::sync::broadcast;

    use super::*;

    #[tokio::test]
    async fn detects_none_when_timer_expires() {
        let (_tx, rx) = broadcast::channel(4);
        assert!(
            detect_peer_termination(rx, Duration::from_millis(25))
                .await
                .is_none()
        );
    }

    #[tokio::test]
    async fn detects_killed_event() {
        let (tx, rx) = broadcast::channel(4);
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(10)).await;
            let _ = tx.send(PeerLifecycleEvent::Killed);
        });

        assert_eq!(
            detect_peer_termination(rx, Duration::from_secs(1)).await,
            Some(TerminationKind::Killed)
        );
    }

    #[test]
    fn advances_next_height_for_each_block() {
        let mut block_height = BlockHeight {
            total: 1,
            non_empty: 1,
        };
        let mut next_height = block_height.total.checked_add(1).expect("setup overflow");

        advance_block_height(&mut block_height, &mut next_height, 2, false);
        advance_block_height(&mut block_height, &mut next_height, 3, true);

        assert_eq!(block_height.total, 3);
        assert_eq!(block_height.non_empty, 2);
        assert_eq!(next_height, 4);
    }
}

#[cfg(test)]
mod start_event_tests {
    use tokio::sync::broadcast;

    use super::*;

    #[tokio::test]
    async fn waits_until_server_started_event() {
        let (tx, rx) = broadcast::channel(4);
        tokio::spawn(async move {
            let _ = tx.send(PeerLifecycleEvent::Spawned);
            let _ = tx.send(PeerLifecycleEvent::BlockApplied { height: 1 });
            let _ = tx.send(PeerLifecycleEvent::ServerStarted);
        });

        let event = wait_for_start_event(rx).await;
        assert!(matches!(event, Some(PeerLifecycleEvent::ServerStarted)));
    }
}

#[cfg(test)]
mod diagnostics_tests {
    use tempfile::tempdir;

    use super::*;

    #[test]
    fn snapshot_dir_entries_are_sorted_and_truncated() {
        let dir = tempdir().expect("tempdir");
        for name in ["z", "a", "c", "b", "y", "x"] {
            let path = dir.path().join(name);
            std::fs::write(path, name).expect("create file");
        }
        let entries = snapshot_dir_entries(dir.path(), 3);
        assert_eq!(entries[0], "a");
        assert_eq!(entries[1], "b");
        assert_eq!(entries[2], "c");
        assert!(
            entries.last().unwrap().starts_with("(+"),
            "should include truncation marker"
        );
    }

    #[test]
    fn snapshot_snippet_keeps_short_strings_intact() {
        let message = "ok";
        assert_eq!(snapshot_snippet(message), message);
    }

    #[test]
    fn snapshot_snippet_truncates_and_marks_long_messages() {
        let message = "a".repeat(SNAPSHOT_MESSAGE_SNIPPET_MAX_CHARS + 5);
        let snippet = snapshot_snippet(&message);
        assert_eq!(
            snippet.len(),
            SNAPSHOT_MESSAGE_SNIPPET_MAX_CHARS + '…'.len_utf8()
        );
        assert!(
            snippet.ends_with('…'),
            "snippet should mark truncation with an ellipsis"
        );
    }

    #[test]
    fn storage_snapshot_detects_existing_pipeline_entries() {
        let dir = tempdir().expect("tempdir");
        let storage = dir.path().join("storage");
        let blocks = storage.join("blocks").join("lane_000_default");
        let pipeline = blocks.join("pipeline");
        std::fs::create_dir_all(&pipeline).expect("pipeline dir");
        std::fs::create_dir_all(&blocks).expect("block dir");
        std::fs::write(pipeline.join(PIPELINE_SIDECARS_DATA_FILE), b"genesis")
            .expect("pipeline data file");
        std::fs::write(
            pipeline.join(PIPELINE_SIDECARS_INDEX_FILE),
            vec![0u8; PIPELINE_INDEX_ENTRY_SIZE_U64 as usize],
        )
        .expect("pipeline index file");

        let snapshot = PeerStorageSnapshot::capture(storage.clone(), true);
        assert!(snapshot.store_exists);
        assert!(snapshot.has_block_1_artifact);
        assert!(
            snapshot
                .pipeline_entries
                .iter()
                .any(|entry| entry == PIPELINE_SIDECARS_DATA_FILE),
            "expected pipeline snapshot to include sidecar data file"
        );
    }
}

#[cfg(test)]
mod shutdown_tests {
    use std::process::Stdio;

    use tempfile::tempdir;
    use tokio::fs::File;
    use tokio::io::{AsyncWriteExt, duplex};
    use tokio::process::Command;

    use super::*;

    #[cfg(target_family = "unix")]
    #[tokio::test]
    async fn shutdown_prefers_sigterm_before_sigquit() {
        let dir = tempdir().expect("tempdir");
        let signal_log = dir.path().join("signals.log");
        let mut cmd = Command::new("sh");
        cmd.arg("-c").arg(
            r#": > "$SIGNAL_LOG"; trap 'echo SIGTERM >> "$SIGNAL_LOG"; exit 0' TERM; trap 'echo SIGQUIT >> "$SIGNAL_LOG"; exit 0' QUIT; while true; do sleep 1; done"#,
        );
        cmd.env("SIGNAL_LOG", &signal_log);
        cmd.stdout(Stdio::null()).stderr(Stdio::null());
        let child = cmd.spawn().expect("spawn signal trapper");

        let (events, _rx) = broadcast::channel(4);
        let (block_height, _rx) = watch::channel(None);
        let (_fatal_tx, fatal_rx) = watch::channel(false);
        let mut peer_exit = PeerExit {
            child,
            span: tracing::Span::none(),
            is_running: Arc::new(AtomicBool::new(true)),
            is_normal_shutdown_started: Arc::new(AtomicBool::new(false)),
            events,
            block_height,
            fatal_rx,
            stderr_log_ready: Arc::new(Notify::new()),
            stderr_live: Arc::new(StdMutex::new(LiveStderrState::default())),
        };

        tokio::time::sleep(Duration::from_millis(50)).await;
        let _status = peer_exit
            .shutdown_or_kill()
            .await
            .expect("shutdown should complete");

        let log = std::fs::read_to_string(&signal_log).expect("read signal log");
        assert!(
            log.contains("SIGTERM"),
            "expected SIGTERM handler to run, log: {log:?}"
        );
        assert!(
            !log.contains("SIGQUIT"),
            "SIGQUIT should not be used for a responsive shutdown, log: {log:?}"
        );
    }

    #[tokio::test]
    async fn log_drain_exits_on_shutdown_notify() {
        let dir = tempdir().expect("tempdir");
        let log_path = dir.path().join("stdout.log");
        let file = File::create(&log_path).await.expect("create log file");
        let (mut writer, reader) = duplex(64);
        let (fatal_tx, fatal_rx) = watch::channel(false);
        let is_running = Arc::new(AtomicBool::new(true));
        let handle = tokio::spawn(drain_log_lines(
            reader,
            file,
            fatal_rx,
            is_running.clone(),
            |_| {},
            None,
            "stdout",
        ));

        writer.write_all(b"hello\n").await.expect("write line");
        writer.flush().await.expect("flush");
        is_running.store(false, Ordering::Relaxed);
        let _ = fatal_tx.send(true);

        tokio::time::timeout(Duration::from_secs(1), handle)
            .await
            .expect("log task should exit")
            .expect("log task should not panic");
    }
}

#[cfg(test)]
mod sora_profile_tests {
    use super::*;

    #[test]
    fn sora_profile_detection_defaults_parse_with_bls_keys() {
        let defaults = sora_profile_detection_defaults();
        let config =
            iroha_config::parameters::actual::Root::from_toml_source(TomlSource::inline(defaults))
                .expect("sora profile detection defaults should parse");
        assert_eq!(
            config.streaming.key_material.identity().algorithm(),
            iroha_crypto::Algorithm::Ed25519
        );
        let trusted = config.common.trusted_peers.value();
        let myself_pk = trusted.myself.id().public_key().clone();
        let pop = trusted
            .pops
            .get(&myself_pk)
            .expect("sora profile default must provide PoP for self");
        iroha_crypto::bls_normal_pop_verify(&myself_pk, pop)
            .expect("sora profile default PoP should verify");
    }

    #[test]
    fn sora_profile_detection_overrides_streaming_identity_keys() {
        let mut streaming = Table::new();
        streaming.insert(
            "identity_public_key".into(),
            Value::String(SORA_PROFILE_BLS_PUBLIC_KEY.to_string()),
        );
        streaming.insert(
            "identity_private_key".into(),
            Value::String(SORA_PROFILE_BLS_PRIVATE_KEY.to_string()),
        );
        let mut layer = Table::new();
        layer.insert("streaming".into(), Value::Table(streaming));

        let merged = merged_sora_profile_detection_config(&[layer]);
        let config =
            iroha_config::parameters::actual::Root::from_toml_source(TomlSource::inline(merged))
                .expect("merged sora profile detection config should parse");
        assert_eq!(
            config.streaming.key_material.identity().algorithm(),
            iroha_crypto::Algorithm::Ed25519
        );
    }

    #[test]
    fn sora_profile_detection_pop_survives_trusted_peers_pop_override() {
        let other = KeyPair::from_seed(b"sora-profile-pop-merge".to_vec(), Algorithm::BlsNormal);
        let other_pop =
            iroha_crypto::bls_normal_pop_prove(other.private_key()).expect("BLS PoP generation");
        let other_pk = other.public_key().to_string();

        let mut pop_entry = Table::new();
        pop_entry.insert("public_key".into(), Value::String(other_pk.clone()));
        pop_entry.insert(
            "pop_hex".into(),
            Value::String(format!("0x{}", hex_lower(&other_pop))),
        );
        let mut layer = Table::new();
        layer.insert(
            "trusted_peers_pop".into(),
            Value::Array(vec![Value::Table(pop_entry)]),
        );

        let merged = merged_sora_profile_detection_config(&[layer]);
        let entries = merged
            .get("trusted_peers_pop")
            .and_then(Value::as_array)
            .expect("trusted_peers_pop array");
        let mut has_default = false;
        let mut has_other = false;
        for entry in entries {
            let Some(table) = entry.as_table() else {
                continue;
            };
            if let Some(pk) = table.get("public_key").and_then(Value::as_str) {
                if pk == SORA_PROFILE_BLS_PUBLIC_KEY {
                    has_default = true;
                }
                if pk == other_pk {
                    has_other = true;
                }
            }
        }
        assert!(has_default, "sora profile PoP should be retained");
        assert!(has_other, "caller-supplied PoP should be retained");
    }

    #[test]
    fn sora_profile_detection_is_false_for_defaults() {
        assert!(!config_requires_sora_profile(&[Table::new()]));
    }

    #[test]
    fn sora_profile_detection_allows_enabled_nexus_without_overrides() {
        let mut nexus = toml::map::Map::new();
        nexus.insert("enabled".into(), toml::Value::Boolean(true));

        let mut table = Table::new();
        table.insert("nexus".into(), toml::Value::Table(nexus));

        assert!(!config_requires_sora_profile(&[table]));
    }

    #[test]
    fn sora_profile_detection_flags_nexus_lane_overrides() {
        let mut lane = toml::map::Map::new();
        lane.insert("alias".into(), toml::Value::String("lane0".into()));
        lane.insert("index".into(), toml::Value::Integer(0));
        let mut metadata = toml::map::Map::new();
        metadata.insert(
            "scheduler.teu_capacity".into(),
            toml::Value::String("262144".into()),
        );
        lane.insert("metadata".into(), toml::Value::Table(metadata));

        let mut fusion = toml::map::Map::new();
        fusion.insert("floor_teu".into(), toml::Value::Integer(131_072));
        fusion.insert("exit_teu".into(), toml::Value::Integer(262_144));

        let mut audit = toml::map::Map::new();
        audit.insert("sample_size".into(), toml::Value::Integer(1));
        audit.insert("window_count".into(), toml::Value::Integer(1));
        audit.insert("interval_ms".into(), toml::Value::Integer(60_000));

        let mut da = toml::map::Map::new();
        da.insert("q_in_slot_total".into(), toml::Value::Integer(1));
        da.insert("q_in_slot_per_ds_min".into(), toml::Value::Integer(1));
        da.insert("sample_size_base".into(), toml::Value::Integer(1));
        da.insert("sample_size_max".into(), toml::Value::Integer(1));
        da.insert("threshold_base".into(), toml::Value::Integer(1));
        da.insert("per_attester_shards".into(), toml::Value::Integer(1));
        da.insert("audit".into(), toml::Value::Table(audit));

        let mut nexus = toml::map::Map::new();
        nexus.insert("enabled".into(), toml::Value::Boolean(true));
        nexus.insert("lane_count".into(), toml::Value::Integer(1));
        nexus.insert(
            "lane_catalog".into(),
            toml::Value::Array(vec![toml::Value::Table(lane)]),
        );
        nexus.insert("fusion".into(), toml::Value::Table(fusion));
        nexus.insert("da".into(), toml::Value::Table(da));

        let mut table = Table::new();
        table.insert("nexus".into(), toml::Value::Table(nexus));

        assert!(config_requires_sora_profile(&[table]));
    }

    #[test]
    fn sora_profile_detection_ignores_default_routing_policy() {
        let mut policy = toml::map::Map::new();
        policy.insert("default_lane".into(), toml::Value::Integer(0));
        policy.insert(
            "default_dataspace".into(),
            toml::Value::String("universal".into()),
        );

        let mut nexus = toml::map::Map::new();
        nexus.insert("routing_policy".into(), toml::Value::Table(policy));

        let mut table = Table::new();
        table.insert("nexus".into(), toml::Value::Table(nexus));

        assert!(!config_requires_sora_profile(&[table]));
    }

    #[test]
    fn raw_nexus_overrides_ignores_default_routing_policy() {
        let mut policy = toml::map::Map::new();
        policy.insert(
            "default_lane".into(),
            toml::Value::Integer(i64::from(
                iroha_config::parameters::defaults::nexus::DEFAULT_ROUTING_LANE_INDEX,
            )),
        );
        policy.insert(
            "default_dataspace".into(),
            toml::Value::String(
                iroha_config::parameters::defaults::nexus::DEFAULT_DATASPACE_ALIAS.to_string(),
            ),
        );

        let mut nexus = toml::map::Map::new();
        nexus.insert("routing_policy".into(), toml::Value::Table(policy));

        let mut table = Table::new();
        table.insert("nexus".into(), toml::Value::Table(nexus));

        assert!(!raw_nexus_overrides(&table));
    }
}

#[cfg(test)]
mod retry_backoff_tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;

    #[tokio::test]
    async fn retry_with_backoff_for_succeeds_before_timeout() {
        let attempts = AtomicUsize::new(0);
        let result = retry_with_backoff_for(Duration::from_millis(500), || {
            let count = attempts.fetch_add(1, Ordering::Relaxed);
            async move {
                if count < 2 {
                    Err::<usize, ()>(())
                } else {
                    Ok(count)
                }
            }
        })
        .await
        .expect("should not time out");

        assert!(result >= 2);
    }

    #[tokio::test]
    async fn retry_with_backoff_for_times_out() {
        let attempts = AtomicUsize::new(0);
        let result = retry_with_backoff_for(Duration::from_millis(75), || {
            let _ = attempts.fetch_add(1, Ordering::Relaxed);
            async move { Err::<(), ()>(()) }
        })
        .await;

        assert!(result.is_err());
        assert!(attempts.load(Ordering::Relaxed) > 0);
    }
}

struct PeerExit {
    child: Child,
    span: tracing::Span,
    is_running: Arc<AtomicBool>,
    is_normal_shutdown_started: Arc<AtomicBool>,
    events: broadcast::Sender<PeerLifecycleEvent>,
    block_height: watch::Sender<Option<BlockHeight>>,
    fatal_rx: watch::Receiver<bool>,
    stderr_log_ready: Arc<Notify>,
    stderr_live: Arc<StdMutex<LiveStderrState>>,
}

impl PeerExit {
    async fn monitor(mut self, shutdown: oneshot::Receiver<()>) -> Result<()> {
        let status = if *self.fatal_rx.borrow() {
            self.span
                .in_scope(|| warn!("forcing peer shutdown after fatal signal"));
            self.shutdown_or_kill().await?
        } else {
            tokio::select! {
                status = self.child.wait() => status?,
                _ = shutdown => self.shutdown_or_kill().await?,
                changed = self.fatal_rx.changed() => {
                    if changed.is_ok() && *self.fatal_rx.borrow() {
                        self.span.in_scope(|| warn!("forcing peer shutdown after fatal signal"));
                    }
                    self.shutdown_or_kill().await?
                }
            }
        };

        self.await_log_flushes().await;
        println!("TEST_NETWORK peer exited with status {status:?}");
        self.dump_last_stderr();

        self.span.in_scope(|| info!(%status, "Peer terminated"));
        let _ = self.events.send(PeerLifecycleEvent::Terminated { status });
        self.is_running.store(false, Ordering::Relaxed);
        self.block_height.send_modify(|x| *x = None);

        Ok(())
    }

    async fn await_log_flushes(&self) {
        self.wait_log(&self.stderr_log_ready, "stderr").await;
    }

    async fn wait_log(&self, notify: &Arc<Notify>, label: &'static str) {
        if (timeout(LOG_FLUSH_TIMEOUT, notify.notified()).await).is_err() {
            self.span
                .in_scope(|| warn!(log = label, "timed out waiting for log flush"));
        }
    }

    async fn shutdown_or_kill(&mut self) -> Result<ExitStatus> {
        use nix::{sys::signal, unistd::Pid};
        const TIMEOUT: Duration = Duration::from_secs(5);
        const QUIT_GRACE: Duration = Duration::from_secs(1);

        self.is_normal_shutdown_started
            .store(true, Ordering::Relaxed);

        self.span.in_scope(|| info!("sending SIGTERM"));
        signal::kill(
            Pid::from_raw(self.child.id().ok_or(eyre!("race condition"))? as i32),
            signal::Signal::SIGTERM,
        )
        .wrap_err("failed to send SIGTERM")?;

        if let Ok(status) = timeout(TIMEOUT, self.child.wait()).await {
            self.span.in_scope(|| info!("exited gracefully"));
            return status.wrap_err("wait failure");
        };

        // If graceful shutdown stalls, attempt to capture a backtrace (where supported).
        #[cfg(target_family = "unix")]
        if let Some(pid) = self.child.id() {
            if let Err(err) = signal::kill(Pid::from_raw(pid as i32), signal::Signal::SIGQUIT)
                .map_err(Report::from)
            {
                self.span
                    .in_scope(|| warn!(?err, pid, "failed to send SIGQUIT before killing"));
            } else {
                self.span
                    .in_scope(|| debug!(pid, "sent SIGQUIT to peer for diagnostics"));
                if let Ok(status) = timeout(QUIT_GRACE, self.child.wait()).await {
                    self.span.in_scope(|| info!("exited after SIGQUIT"));
                    return status.wrap_err("wait failure");
                }
            }
        }

        self.span
            .in_scope(|| warn!("process didn't terminate after {TIMEOUT:?}, killing"));
        timeout(TIMEOUT, async move {
            self.child.kill().await.expect("not a recoverable failure");
            self.child.wait().await
        })
        .await
        .wrap_err("didn't terminate after SIGKILL")?
        .wrap_err("wait failure")
    }

    fn dump_last_stderr(&self) {
        let guard = self
            .stderr_live
            .lock()
            .expect("stderr live buffer should not be poisoned");
        if guard.buffer.is_empty() {
            eprintln!("TEST_NETWORK peer stderr was empty before exit");
            return;
        }
        let preview = summarize_peer_stderr(&guard.buffer)
            .map(|summary| summary.preview)
            .unwrap_or_else(|| "<stderr summary unavailable>".to_string());
        eprintln!("TEST_NETWORK peer stderr tail:\n{preview}");
    }
}

fn pipeline_dirs(storage_dir: &Path) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Ok(entries) = fs::read_dir(storage_dir.join("blocks")) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                dirs.push(path.join("pipeline"));
            }
        }
    }
    dirs
}

#[cfg(test)]
fn advance_block_height(
    block_height: &mut BlockHeight,
    next_height: &mut u64,
    observed_height: u64,
    is_empty: bool,
) {
    if observed_height != *next_height {
        warn!(
            expected = *next_height,
            observed = observed_height,
            "missed block height update; resynchronising block watcher"
        );
    }

    block_height.total = observed_height;
    if !is_empty {
        block_height.non_empty = block_height.non_empty.saturating_add(1);
        if block_height.non_empty > block_height.total {
            block_height.non_empty = block_height.total;
        }
    }
    *next_height = block_height
        .total
        .checked_add(1)
        .expect("block height overflow when subscribing to blocks");
}

/// Composite block height representation
#[derive(Debug, Copy, Clone)]
pub struct BlockHeight {
    /// Total blocks
    pub total: u64,
    /// Non-empty blocks
    pub non_empty: u64,
}

impl From<Status> for BlockHeight {
    fn from(value: Status) -> Self {
        Self {
            total: value.blocks,
            non_empty: value.blocks_non_empty,
        }
    }
}

impl BlockHeight {
    /// Shorthand to use with e.g. [`once_blocks_sync`].
    pub fn predicate_non_empty(non_empty_height: u64) -> impl Fn(BlockHeight) -> bool + Clone {
        move |value| value.non_empty >= non_empty_height
    }

    /// Predicate that waits for the overall block height, regardless of whether
    /// the blocks were empty.
    pub fn predicate_total(total_height: u64) -> impl Fn(BlockHeight) -> bool + Clone {
        move |value| value.total >= total_height
    }
}

fn detect_block_height_from_storage(storage_dir: &Path, current_total: u64) -> Option<BlockHeight> {
    let mut pipeline_height: Option<u64> = None;

    // Pipeline markers can advance ahead of committed blocks; only trust them if no hashes exist.
    for pipeline_dir in pipeline_dirs(storage_dir) {
        if let Some(height) = NetworkPeer::pipeline_height_from_index(&pipeline_dir) {
            pipeline_height = Some(pipeline_height.map_or(height, |prev| prev.max(height)));
        }
    }

    let mut hashes_height: Option<u64> = None;
    let mut saw_hashes = false;
    if let Ok(entries) = fs::read_dir(storage_dir.join("blocks")) {
        for entry in entries.flatten() {
            let hashes_path = entry.path().join("blocks.hashes");
            if let Ok(meta) = fs::metadata(&hashes_path) {
                saw_hashes = true;
                let blocks = meta.len() / 32;
                hashes_height = Some(hashes_height.map_or(blocks, |prev| prev.max(blocks)));
            }
        }
    }

    let max_height = if saw_hashes {
        hashes_height.unwrap_or(0)
    } else {
        pipeline_height.unwrap_or(0)
    };

    if max_height > current_total {
        Some(BlockHeight {
            total: max_height,
            non_empty: max_height,
        })
    } else {
        None
    }
}

/// Wait until [`NetworkPeer::once_block`] resolves for all peers.
///
/// Fails early if some peer terminates.
pub async fn once_blocks_sync(
    peers: impl Iterator<Item = &NetworkPeer>,
    f: impl Fn(BlockHeight) -> bool + Clone,
) -> Result<()> {
    let mut futures = peers
        .map(|x| {
            let f = f.clone();
            async move {
                let mut storage_poll = tokio::time::interval(Duration::from_millis(250));
                loop {
                    tokio::select! {
                        () = x.once_block_with(f.clone()) => {
                            return Ok(());
                        },
                        () = x.once(|e| matches!(e, PeerLifecycleEvent::Terminated { .. })) => {
                            return Err(eyre!("Peer terminated"));
                        },
                        _ = storage_poll.tick() => {
                            if let Some(snapshot) = detect_block_height_from_storage(&x.kura_store_dir(), 0)
                                && f(snapshot)
                            {
                                return Ok(());
                            }
                        }
                    }
                }
            }
        })
        .collect::<FuturesUnordered<_>>();

    loop {
        match futures.next().await {
            Some(Ok(())) => {}
            Some(Err(e)) => return Err(e),
            None => return Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;
    use std::{
        collections::HashSet,
        env,
        ffi::{OsStr, OsString},
        fs,
        io::{self, Write},
        sync::{
            Arc, Mutex,
            atomic::{AtomicBool, AtomicUsize, Ordering},
        },
        thread,
        time::Duration,
    };

    use iroha_config::parameters::defaults;
    use iroha_core::sumeragi::consensus::compute_consensus_fingerprint_from_params;
    use iroha_crypto::Algorithm;
    use iroha_data_model::{
        block::{
            decode_framed_signed_block, decode_versioned_signed_block,
            deframe_versioned_signed_block_bytes,
        },
        isi::{Instruction, SetParameter},
        parameter::{Parameter, system::consensus_metadata},
        transaction::Executable,
    };
    use iroha_version::{Version, codec::EncodeVersioned};
    use norito::json::Value as JsonValue;
    use tempfile::tempdir;
    use tokio::sync::{Mutex as AsyncMutex, MutexGuard as AsyncMutexGuard};
    use toml::Value as TomlValue;

    use super::*;

    static LOG_ENV_GUARD: AsyncMutex<()> = AsyncMutex::const_new(());
    /// Serializes async tests that override `TEST_NETWORK_BIN_*` variables so they
    /// cannot leak into concurrently running cases.
    static PROGRAM_BIN_ENV_GUARD: AsyncMutex<()> = AsyncMutex::const_new(());
    /// Serializes mutations of RBC/DA override env vars so tests stay deterministic.
    static SUMERAGI_ENV_GUARD: AsyncMutex<()> = AsyncMutex::const_new(());
    /// Serializes mutations of client timeout overrides.
    static CLIENT_ENV_GUARD: AsyncMutex<()> = AsyncMutex::const_new(());
    /// Serializes mutations of config env overrides so local parsing ignores host overrides.
    static CONFIG_ENV_GUARD: AsyncMutex<()> = AsyncMutex::const_new(());
    /// Serializes network permit env overrides so tests do not race on temp directories.
    static NETWORK_PERMIT_ENV_GUARD: AsyncMutex<()> = AsyncMutex::const_new(());

    fn lock_env_guard(mutex: &'static AsyncMutex<()>) -> AsyncMutexGuard<'static, ()> {
        mutex.blocking_lock()
    }

    async fn lock_env_guard_async(mutex: &'static AsyncMutex<()>) -> AsyncMutexGuard<'static, ()> {
        mutex.lock().await
    }

    #[derive(Clone)]
    struct BufferWriter(Arc<Mutex<Vec<u8>>>);

    impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for BufferWriter {
        type Writer = BufferGuard;
        fn make_writer(&'a self) -> Self::Writer {
            BufferGuard(self.0.clone())
        }
    }

    struct BufferGuard(Arc<Mutex<Vec<u8>>>);

    impl Write for BufferGuard {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            let mut guard = self.0.lock().unwrap();
            guard.extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    fn skip_network_tests(test_name: &str) -> bool {
        static LOOPBACK_BIND_ALLOWED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        let can_bind = *LOOPBACK_BIND_ALLOWED
            .get_or_init(|| std::net::TcpListener::bind(("127.0.0.1", 0)).is_ok());
        if can_bind {
            false
        } else {
            eprintln!("skipping {test_name}: environment denies binding TCP sockets on 127.0.0.1");
            true
        }
    }

    fn set_env_var<K, V>(key: K, value: V)
    where
        K: AsRef<OsStr>,
        V: AsRef<OsStr>,
    {
        unsafe { std::env::set_var(key, value) }
    }

    fn remove_env_var<K>(key: K)
    where
        K: AsRef<OsStr>,
    {
        unsafe { std::env::remove_var(key) }
    }

    struct EnvVarRestore {
        key: &'static str,
        previous: Option<OsString>,
    }

    impl EnvVarRestore {
        fn set<K: AsRef<OsStr>>(key: &'static str, value: K) -> Self {
            let previous = env::var_os(key);
            set_env_var(key, value);
            Self { key, previous }
        }
    }

    impl Drop for EnvVarRestore {
        fn drop(&mut self) {
            if let Some(value) = self.previous.take() {
                set_env_var(self.key, value);
            } else {
                remove_env_var(self.key);
            }
        }
    }

    #[test]
    fn config_env_override_keys_include_core_settings() {
        let keys = config_env_override_keys();
        assert!(keys.contains(&"API_ADDRESS"));
        assert!(keys.contains(&"P2P_ADDRESS"));
        assert!(keys.contains(&"CHAIN"));
        assert!(!keys.is_empty());
    }

    #[test]
    fn strip_config_env_overrides_marks_keys_for_removal() {
        struct DummyCommand {
            removed: Vec<String>,
        }

        impl CommandEnv for DummyCommand {
            fn env_remove(&mut self, key: &str) {
                self.removed.push(key.to_string());
            }
        }

        let mut cmd = DummyCommand {
            removed: Vec::new(),
        };
        strip_config_env_overrides(&mut cmd);
        let removed: HashSet<_> = cmd.removed.into_iter().collect();
        assert!(removed.contains("API_ADDRESS"));
        assert!(removed.contains("P2P_ADDRESS"));
        assert!(removed.contains("CHAIN"));
    }

    #[test]
    fn network_parallelism_env_override_applies() {
        let _guard = lock_env_guard(&NETWORK_PERMIT_ENV_GUARD);
        let _parallel_guard = EnvVarRestore::set(NETWORK_PARALLELISM_ENV, "2");
        let _serialize_guard = EnvVarRestore::set(SERIALIZE_NETWORKS_ENV, "0");
        assert_eq!(network_parallelism_limit(), 2);
    }

    #[test]
    fn serialization_overrides_parallelism_limit() {
        let _guard = lock_env_guard(&NETWORK_PERMIT_ENV_GUARD);
        let _parallel_guard = EnvVarRestore::set(NETWORK_PARALLELISM_ENV, "4");
        let _serialize_guard = EnvVarRestore::set(SERIALIZE_NETWORKS_ENV, "1");
        assert_eq!(network_parallelism_limit(), 1);
    }

    #[test]
    fn peer_startup_timeout_override_is_applied() {
        if skip_network_tests("peer_startup_timeout_override_is_applied") {
            return;
        }
        let network = build_with_isolated_permit(
            NetworkBuilder::new()
                .with_min_peers(4)
                .with_peer_startup_timeout(Duration::from_secs(300)),
        );
        assert_eq!(network.peer_startup_timeout(), Duration::from_secs(300));
    }

    #[test]
    fn peer_startup_timeout_applies_per_peer_floor() {
        if skip_network_tests("peer_startup_timeout_applies_per_peer_floor") {
            return;
        }
        let _timeout_guard = EnvVarRestore::set("IROHA_TEST_PEER_START_TIMEOUT_SECS", "10");
        let network = build_with_isolated_permit(NetworkBuilder::new().with_min_peers(4));
        let expected = Duration::from_secs(
            PEER_STARTUP_TIMEOUT_PER_PEER_SECS.saturating_mul(network.peers().len() as u64),
        );
        assert_eq!(network.peer_startup_timeout(), expected);
    }

    #[test]
    fn network_permit_creates_and_clears_lock_file() {
        let _guard = lock_env_guard(&NETWORK_PERMIT_ENV_GUARD);
        let dir = tempdir().expect("permit dir");
        let _dir_guard = EnvVarRestore::set(NETWORK_PERMIT_DIR_ENV, dir.path());
        let _parallel_guard = EnvVarRestore::set(NETWORK_PARALLELISM_ENV, "1");
        let _serialize_guard = EnvVarRestore::set(SERIALIZE_NETWORKS_ENV, "0");

        let permit = acquire_network_permit();
        let file_count = fs::read_dir(dir.path())
            .expect("permit dir listing")
            .count();
        assert_eq!(file_count, 1, "expected a single permit file");
        drop(permit);

        let file_count = fs::read_dir(dir.path())
            .expect("permit dir listing")
            .count();
        assert_eq!(file_count, 0, "permit file should be removed");
    }

    #[cfg(unix)]
    #[test]
    fn stale_permit_file_is_reclaimed() {
        let _guard = lock_env_guard(&NETWORK_PERMIT_ENV_GUARD);
        let dir = tempdir().expect("permit dir");
        let _dir_guard = EnvVarRestore::set(NETWORK_PERMIT_DIR_ENV, dir.path());
        let _parallel_guard = EnvVarRestore::set(NETWORK_PARALLELISM_ENV, "1");
        let _serialize_guard = EnvVarRestore::set(SERIALIZE_NETWORKS_ENV, "0");
        let path = dir.path().join("permit-0.lock");
        let mut file = fs::File::create(&path).expect("stale permit file");
        writeln!(file, "pid={}", i32::MAX).expect("write pid");

        let permit = try_acquire_file_permit(1).expect("expected reclaimed permit");
        drop(permit);
    }

    #[cfg(unix)]
    #[test]
    fn pid_alive_detects_current_and_dead_processes() {
        assert_eq!(pid_alive(std::process::id()), Some(true));
        assert_eq!(pid_alive(i32::MAX as u32), Some(false));
    }

    #[test]
    fn config_requires_sora_profile_ignores_env_overrides() {
        let _guard = lock_env_guard(&CONFIG_ENV_GUARD);
        struct EnvRestore {
            key: &'static str,
            previous: Option<OsString>,
        }

        impl EnvRestore {
            fn set(key: &'static str, value: OsString) -> Self {
                let previous = env::var_os(key);
                set_env_var(key, value);
                Self { key, previous }
            }

            fn clear(key: &'static str) -> Self {
                let previous = env::var_os(key);
                remove_env_var(key);
                Self { key, previous }
            }
        }

        impl Drop for EnvRestore {
            fn drop(&mut self) {
                if let Some(value) = self.previous.take() {
                    set_env_var(self.key, value);
                } else {
                    remove_env_var(self.key);
                }
            }
        }

        let _public_key_guard = EnvRestore::set(
            "PUBLIC_KEY",
            OsString::from(ALICE_KEYPAIR.public_key().to_string()),
        );
        let _private_key_guard = EnvRestore::clear("PRIVATE_KEY");

        let layer = Table::new().write(["torii", "sorafs", "storage", "enabled"], true);
        assert!(
            config_requires_sora_profile(&[layer]),
            "profile detection should not be influenced by host env overrides"
        );
    }

    #[tokio::test]
    async fn once_block_falls_back_to_storage_snapshot() {
        let dir = tempdir().expect("tempdir");
        let pipeline_dir = dir.path().join("storage/blocks/lane_000_default/pipeline");
        fs::create_dir_all(&pipeline_dir).expect("pipeline dir");

        let mut index =
            fs::File::create(pipeline_dir.join(PIPELINE_SIDECARS_INDEX_FILE)).expect("index file");
        for height in 1u64..=2 {
            index
                .write_all(&height.to_le_bytes())
                .expect("height entry");
            index.write_all(&1u64.to_le_bytes()).expect("len entry");
        }

        let (events_tx, _events_rx) = tokio::sync::broadcast::channel(4);
        let (block_height, _rx) = tokio::sync::watch::channel(None);

        let storage_root = dir.path().to_path_buf();

        let peer = NetworkPeer {
            mnemonic: "once-block-fallback".to_string(),
            span: tracing::Span::none(),
            key_pair: KeyPair::random(),
            streaming_key_pair: KeyPair::random(),
            bls_key_pair: None,
            bls_pop: None,
            dir: storage_root,
            run: Arc::new(tokio::sync::Mutex::new(None)),
            runs_count: Arc::new(AtomicUsize::new(0)),
            is_running: Arc::new(AtomicBool::new(true)),
            events: events_tx,
            block_height,
            stderr_live: Arc::new(StdMutex::new(LiveStderrState::default())),
            startup_probe: Arc::new(StdMutex::new(PeerStartupProbe::default())),
            start_context: Arc::new(StdMutex::new(None)),
            port_p2p: Arc::new(AllocatedPort::new()),
            port_api: Arc::new(AllocatedPort::new()),
        };

        let result = tokio::time::timeout(Duration::from_secs(1), peer.once_block(2)).await;
        assert!(
            result.is_ok(),
            "once_block should observe storage height via fallback"
        );
    }

    #[tokio::test]
    async fn wait_for_block_1_with_watchdog_uses_storage_on_status_failure() {
        let dir = tempdir().expect("tempdir");
        let pipeline_dir = dir.path().join("storage/blocks/lane_000_default/pipeline");
        fs::create_dir_all(&pipeline_dir).expect("pipeline dir");
        write_sidecar_index(&pipeline_dir, 1);

        let (events_tx, _events_rx) = tokio::sync::broadcast::channel(4);
        let (block_height, _rx) = tokio::sync::watch::channel(None);

        let storage_root = dir.path().to_path_buf();

        let peer = NetworkPeer {
            mnemonic: "wait-block-watchdog".to_string(),
            span: tracing::Span::none(),
            key_pair: KeyPair::random(),
            streaming_key_pair: KeyPair::random(),
            bls_key_pair: None,
            bls_pop: None,
            dir: storage_root,
            run: Arc::new(tokio::sync::Mutex::new(None)),
            runs_count: Arc::new(AtomicUsize::new(0)),
            is_running: Arc::new(AtomicBool::new(true)),
            events: events_tx,
            block_height,
            stderr_live: Arc::new(StdMutex::new(LiveStderrState::default())),
            startup_probe: Arc::new(StdMutex::new(PeerStartupProbe::default())),
            start_context: Arc::new(StdMutex::new(None)),
            port_p2p: Arc::new(AllocatedPort::new()),
            port_api: Arc::new(AllocatedPort::new()),
        };

        let mnemonic = peer.mnemonic().to_string();
        let result = tokio::time::timeout(
            Duration::from_secs(1),
            Network::wait_for_block_1_with_watchdog(&peer, 0, &mnemonic, "test"),
        )
        .await;
        assert!(
            result.is_ok(),
            "wait_for_block_1_with_watchdog should return when storage has block 1"
        );
    }

    /// Restores environment variable to its previous value when dropped.
    struct EnvVarGuard {
        key: &'static str,
        original: Option<OsString>,
    }

    impl EnvVarGuard {
        fn cleared(key: &'static str) -> Self {
            let original = env::var_os(key);
            remove_env_var(key);
            Self { key, original }
        }
    }

    #[test]
    fn cargo_build_enables_bundled_rans() {
        assert!(
            build_env_overrides()
                .iter()
                .any(|(key, value)| *key == "ENABLE_RANS_BUNDLES" && *value == "1")
        );
    }

    #[test]
    fn preflight_bind_detects_in_use_port() {
        let listener = match std::net::TcpListener::bind(("127.0.0.1", 0)) {
            Ok(listener) => listener,
            Err(err) => {
                if err.kind() == io::ErrorKind::PermissionDenied {
                    eprintln!("skipping preflight_bind_detects_in_use_port: {err}");
                    return;
                }
                panic!("unexpected error binding ephemeral port: {err}");
            }
        };
        let addr = listener
            .local_addr()
            .expect("listener should expose local address");
        let addr = SocketAddr::from(addr);

        let result = preflight_bind_addresses([addr]);
        match result {
            Err(err) if err.kind() == io::ErrorKind::AddrInUse => {}
            Err(err) if err.kind() == io::ErrorKind::PermissionDenied => {
                eprintln!("skipping preflight check: {err}");
            }
            Err(err) => panic!("unexpected preflight bind error: {err}"),
            Ok(()) => panic!("preflight should fail when port is already in use"),
        }
    }

    #[test]
    fn startup_warn_gate_waits_for_grace() {
        let grace = Duration::from_millis(25);
        let gate = StartupWarnGate::new(grace);
        assert!(!gate.should_warn());
        thread::sleep(grace + Duration::from_millis(10));
        assert!(gate.should_warn());
        // Subsequent warnings should be throttled until the interval elapses.
        assert!(!gate.should_warn());
        thread::sleep(STARTUP_STATUS_WARN_INTERVAL);
        assert!(gate.should_warn());
    }

    #[test]
    fn status_error_is_connection_refused_detects_io_error() {
        let err = std::io::Error::new(ErrorKind::ConnectionRefused, "refused");
        let report = Report::from(err);
        assert!(status_error_is_connection_refused(&report));
    }

    #[test]
    fn status_error_is_connection_refused_ignores_other_errors() {
        let err = std::io::Error::other("other");
        let report = Report::from(err);
        assert!(!status_error_is_connection_refused(&report));
    }

    #[test]
    fn client_status_timeout_defaults_are_generous() {
        let _guard = lock_env_guard(&CLIENT_ENV_GUARD);
        let _secs_guard = EnvVarGuard::cleared("IROHA_TEST_CLIENT_STATUS_TIMEOUT_SECS");
        let _ms_guard = EnvVarGuard::cleared("IROHA_TEST_CLIENT_STATUS_TIMEOUT_MS");

        assert_eq!(
            client_status_timeout_env(),
            CLIENT_STATUS_TIMEOUT_DEFAULT,
            "default client status timeout should tolerate slow integration runs",
        );
    }

    #[test]
    fn client_ttl_exceeds_status_timeout_by_default() {
        let _guard = lock_env_guard(&CLIENT_ENV_GUARD);
        let _status_secs_guard = EnvVarGuard::cleared("IROHA_TEST_CLIENT_STATUS_TIMEOUT_SECS");
        let _status_ms_guard = EnvVarGuard::cleared("IROHA_TEST_CLIENT_STATUS_TIMEOUT_MS");
        let _ttl_secs_guard = EnvVarGuard::cleared("IROHA_TEST_CLIENT_TTL_SECS");
        let _ttl_ms_guard = EnvVarGuard::cleared("IROHA_TEST_CLIENT_TTL_MS");

        let status_timeout = client_status_timeout_env();
        let ttl = client_ttl_env(status_timeout);
        assert_eq!(
            ttl, CLIENT_TTL_DEFAULT,
            "default TTL should stay above the status timeout cushion"
        );
    }

    #[tokio::test]
    async fn shutdown_resets_running_flag_even_if_monitor_is_absent() {
        if skip_network_tests("shutdown_resets_running_flag_even_if_monitor_is_absent") {
            return;
        }

        let env = Environment::new();
        let peer = NetworkPeer::builder().build(&env);

        let (shutdown_tx, _shutdown_rx) = tokio::sync::oneshot::channel();
        let tasks = tokio::task::JoinSet::new();
        let (fatal_tx, mut fatal_rx) = watch::channel(false);
        {
            let mut guard = peer.run.lock().await;
            *guard = Some(PeerRun {
                tasks,
                shutdown: shutdown_tx,
                fatal_tx: fatal_tx.clone(),
                pid: None,
            });
        }
        peer.is_running.store(true, Ordering::Relaxed);

        let notify_wait = fatal_rx.changed();
        tokio::pin!(notify_wait);
        peer.shutdown().await;

        assert!(!peer.is_running());
        assert!(peer.run.lock().await.is_none());
        tokio::time::timeout(Duration::from_secs(1), &mut notify_wait)
            .await
            .expect("shutdown should notify fatal listeners")
            .expect("fatal signal should be delivered");
    }

    impl Drop for EnvVarGuard {
        fn drop(&mut self) {
            if let Some(value) = self.original.as_ref() {
                set_env_var(self.key, value);
            } else {
                remove_env_var(self.key);
            }
        }
    }

    fn disable_sumeragi_env_overrides() -> EnvVarGuard {
        EnvVarGuard::cleared(DA_ENABLED_ENV)
    }

    #[test]
    fn write_base_config_uses_addr_literals() {
        let env = Environment::new();
        let peer = NetworkPeer::builder().build(&env);
        peer.write_base_config();
        let base_path = peer.dir.join("config.base.toml");
        let contents = fs::read_to_string(&base_path).expect("read base config");
        let parsed: TomlValue = toml::from_str(&contents).expect("parse config.toml");

        let network = parsed
            .get("network")
            .and_then(TomlValue::as_table)
            .expect("network table exists");
        let torii = parsed
            .get("torii")
            .and_then(TomlValue::as_table)
            .expect("torii table exists");

        for key in ["address", "public_address"] {
            let value = network
                .get(key)
                .and_then(TomlValue::as_str)
                .unwrap_or_else(|| panic!("{key} missing"));
            assert!(
                value.starts_with("addr:"),
                "{key} should be addr literal, got {value}"
            );
            let body = norito::literal::parse("addr", value).expect("parse addr literal");
            let port_p2p: u16 = **peer.port_p2p;
            assert!(
                body.ends_with(&port_p2p.to_string()),
                "{key} literal body should contain peer port"
            );
        }

        let torii_addr = torii
            .get("address")
            .and_then(TomlValue::as_str)
            .expect("torii.address present");
        assert!(
            torii_addr.starts_with("addr:"),
            "torii.address should be addr literal, got {torii_addr}"
        );
        let torii_body =
            norito::literal::parse("addr", torii_addr).expect("parse torii addr literal");
        let port_api: u16 = **peer.port_api;
        assert!(
            torii_body.ends_with(&port_api.to_string()),
            "torii literal body should contain API port"
        );
    }

    #[test]
    fn has_committed_block_detects_indexed_pipeline_layouts() {
        let env = Environment::new();
        let modern_peer = NetworkPeer::builder().build(&env);

        let modern_dir = modern_peer
            .dir
            .join("storage")
            .join("blocks")
            .join("lane_000_default")
            .join("pipeline");
        fs::create_dir_all(&modern_dir).expect("create modern pipeline dir");
        write_sidecar_index(&modern_dir, 1);
        assert!(modern_peer.has_committed_block(1));
        assert!(!modern_peer.has_committed_block(2));
    }

    #[test]
    fn detect_block_height_reads_lane_pipeline_index() {
        let env = Environment::new();
        let peer = NetworkPeer::builder().build(&env);
        let pipeline_dir = peer
            .dir
            .join("storage")
            .join("blocks")
            .join("lane_000_default")
            .join("pipeline");
        fs::create_dir_all(&pipeline_dir).expect("create lane pipeline dir");
        write_sidecar_index(&pipeline_dir, 3);

        let height =
            detect_block_height_from_storage(&peer.dir.join("storage"), 0).expect("detect height");
        assert_eq!(height.total, 3);
        assert_eq!(height.non_empty, 3);
    }

    #[test]
    fn detect_block_height_prefers_block_hashes_over_pipeline() {
        let env = Environment::new();
        let peer = NetworkPeer::builder().build(&env);
        let lane_dir = peer
            .dir
            .join("storage")
            .join("blocks")
            .join("lane_000_default");
        let pipeline_dir = lane_dir.join("pipeline");
        fs::create_dir_all(&pipeline_dir).expect("create lane pipeline dir");
        write_sidecar_index(&pipeline_dir, 3);
        fs::write(lane_dir.join("blocks.hashes"), vec![0u8; 32]).expect("write blocks hash file");

        let height =
            detect_block_height_from_storage(&peer.dir.join("storage"), 0).expect("detect height");
        assert_eq!(height.total, 1);
        assert_eq!(height.non_empty, 1);
    }

    #[test]
    fn best_effort_block_height_uses_storage_without_status() {
        let env = Environment::new();
        let peer = NetworkPeer::builder().build(&env);
        let pipeline_dir = peer
            .dir
            .join("storage")
            .join("blocks")
            .join("lane_000_default")
            .join("pipeline");
        fs::create_dir_all(&pipeline_dir).expect("create pipeline dir");
        write_sidecar_index(&pipeline_dir, 2);

        let height = peer.best_effort_block_height().expect("best-effort height");
        assert_eq!(height.total, 2);
        assert_eq!(height.non_empty, 2);
    }

    #[test]
    fn last_known_peers_reflects_recorded_status() {
        let env = Environment::new();
        let peer = NetworkPeer::builder().build(&env);
        let status = Status {
            peers: 5,
            blocks: 1,
            blocks_non_empty: 1,
            ..Status::default()
        };
        let _ = NetworkPeer::record_probe_status(&peer.startup_probe, &status);

        assert_eq!(peer.last_known_peers(), Some(5));
    }

    #[cfg(test)]
    fn write_sidecar_index(pipeline_dir: &Path, entries: u64) {
        let mut index_bytes = Vec::new();
        for i in 0..entries {
            index_bytes.extend_from_slice(&0u64.to_le_bytes());
            index_bytes.extend_from_slice(&(i + 1).to_le_bytes());
        }
        fs::write(pipeline_dir.join(PIPELINE_SIDECARS_DATA_FILE), b"sidecar")
            .expect("write sidecar data");
        fs::write(
            pipeline_dir.join(PIPELINE_SIDECARS_INDEX_FILE),
            &index_bytes,
        )
        .expect("write sidecar index");
    }

    #[test]
    fn write_base_config_copies_rans_tables() {
        let env = Environment::new();
        let peer = NetworkPeer::builder().build(&env);
        peer.write_base_config();
        let tables_path = peer
            .dir
            .join("codec")
            .join("rans")
            .join("tables")
            .join("rans_seed0.toml");
        assert!(
            tables_path.exists(),
            "expected deterministic rANS tables at {}",
            tables_path.display()
        );
    }

    #[test]
    fn trusted_peers_use_addr_literals() {
        let network = build_with_isolated_permit(NetworkBuilder::new().with_peers(2));
        let mut layers = network.config_layers();
        let base_layer = layers
            .next()
            .expect("base config layer present")
            .into_owned();
        let trusted_peers = base_layer
            .get("trusted_peers")
            .and_then(TomlValue::as_array)
            .expect("trusted_peers array present");
        assert!(
            !trusted_peers.is_empty(),
            "trusted_peers should contain entries"
        );

        for entry in trusted_peers {
            let peer_literal = entry
                .as_str()
                .unwrap_or_else(|| panic!("trusted_peers entry should be string"));
            let (_, addr_literal) = peer_literal
                .rsplit_once('@')
                .unwrap_or_else(|| panic!("trusted peer entry malformed: {peer_literal}"));
            assert!(
                addr_literal.starts_with("addr:"),
                "trusted peer address should be literal: {addr_literal}"
            );
            let _body =
                norito::literal::parse("addr", addr_literal).expect("parse trusted peer literal");
        }
    }

    #[test]
    fn with_min_peers_clamps_builder() {
        let network =
            build_with_isolated_permit(NetworkBuilder::new().with_peers(2).with_min_peers(4));
        assert_eq!(network.peers().len(), 4);

        let network =
            build_with_isolated_permit(NetworkBuilder::new().with_peers(5).with_min_peers(4));
        assert_eq!(network.peers().len(), 5);
    }

    #[test]
    fn enables_norito_rpc_ga_stage_for_test_networks() {
        let network = NetworkBuilder::new().build();
        let base_layer = network
            .config_layers()
            .find(|layer| {
                layer
                    .as_ref()
                    .get("torii")
                    .and_then(TomlValue::as_table)
                    .and_then(|torii| torii.get("transport"))
                    .is_some()
            })
            .expect("base config layer present")
            .into_owned();

        let torii_table = base_layer
            .get("torii")
            .and_then(TomlValue::as_table)
            .expect("torii table present");
        let transport_table = torii_table
            .get("transport")
            .and_then(TomlValue::as_table)
            .expect("torii.transport table present");
        let norito_rpc_table = transport_table
            .get("norito_rpc")
            .and_then(TomlValue::as_table)
            .expect("torii.transport.norito_rpc table present");

        let stage = norito_rpc_table
            .get("stage")
            .and_then(TomlValue::as_str)
            .expect("stage should be present");
        assert_eq!(
            stage, "ga",
            "default NetworkBuilder must enable GA Norito-RPC for tests"
        );
        let enabled = norito_rpc_table
            .get("enabled")
            .and_then(TomlValue::as_bool)
            .unwrap_or(false);
        assert!(
            enabled,
            "Norito-RPC must stay enabled for auto-built test networks"
        );
    }

    #[test]
    fn workspace_fingerprint_detects_source_modifications() {
        let temp = tempdir().expect("temporary workspace");
        let root = temp.path();
        fs::write(
            root.join("Cargo.toml"),
            "[workspace]\nmembers = [\"member\"]\n",
        )
        .expect("write manifest");
        fs::create_dir_all(root.join("member/src")).expect("create member src directory");
        let file = root.join("member/src/lib.rs");
        fs::write(&file, b"pub fn greet() {}\n").expect("write source file");

        let initial = workspace_fingerprint(root).expect("initial fingerprint");
        thread::sleep(Duration::from_millis(20));
        fs::write(&file, b"pub fn greet() { println!(\"hi\"); }\n").expect("update source file");
        let updated = workspace_fingerprint(root).expect("updated fingerprint");

        assert_ne!(initial, updated);
    }

    #[test]
    fn workspace_fingerprint_ignores_target_directory() {
        let temp = tempdir().expect("temporary workspace");
        let root = temp.path();
        fs::write(
            root.join("Cargo.toml"),
            "[workspace]\nmembers = [\"member\"]\n",
        )
        .expect("write manifest");
        fs::create_dir_all(root.join("member/src")).expect("create member src directory");
        fs::write(root.join("member/src/lib.rs"), b"pub fn greet() {}\n")
            .expect("write source file");

        fs::create_dir_all(root.join("member/target")).expect("create target directory");
        let artifact = root.join("member/target").join("artifact");
        fs::write(&artifact, b"one").expect("write artifact");

        let before = workspace_fingerprint(root).expect("initial fingerprint");
        thread::sleep(Duration::from_millis(20));
        fs::write(&artifact, b"two").expect("update artifact");
        let after = workspace_fingerprint(root).expect("post-artifact fingerprint");

        assert_eq!(before, after);

        thread::sleep(Duration::from_millis(20));
        fs::write(root.join("member/src/lib.rs"), b"pub fn greet() { 1 }\n")
            .expect("update source file");
        let final_fp = workspace_fingerprint(root).expect("final fingerprint");

        assert_ne!(after, final_fp);
    }

    #[test]
    fn workspace_fingerprint_respects_gitignore_directories() {
        let temp = tempdir().expect("temporary workspace");
        let root = temp.path();
        fs::write(
            root.join("Cargo.toml"),
            "[workspace]\nmembers = [\"member\"]\n",
        )
        .expect("write manifest");
        fs::write(root.join(".gitignore"), "member/ignored/\n").expect("write gitignore");
        fs::create_dir_all(root.join("member/src")).expect("create member src directory");
        fs::create_dir_all(root.join("member/ignored")).expect("create ignored directory");
        fs::write(root.join("member/src/lib.rs"), b"pub fn greet() {}\n")
            .expect("write source file");
        let ignored_file = root.join("member/ignored/data.bin");
        fs::write(&ignored_file, b"a").expect("write ignored file");

        let before = workspace_fingerprint(root).expect("initial fingerprint");
        thread::sleep(Duration::from_millis(20));
        fs::write(&ignored_file, b"b").expect("update ignored file");
        let after = workspace_fingerprint(root).expect("post-ignore fingerprint");

        assert_eq!(before, after);

        fs::write(root.join("member/src/lib.rs"), b"pub fn greet() { 2 }\n")
            .expect("update source file");
        let final_fp = workspace_fingerprint(root).expect("final fingerprint");

        assert_ne!(after, final_fp);
    }

    #[test]
    fn workspace_fingerprint_respects_gitignore_globs() {
        let temp = tempdir().expect("temporary workspace");
        let root = temp.path();
        fs::write(
            root.join("Cargo.toml"),
            "[workspace]\nmembers = [\"member\"]\n",
        )
        .expect("write manifest");
        fs::write(root.join(".gitignore"), "target*/\n*.log\n").expect("write gitignore");
        fs::create_dir_all(root.join("member/src")).expect("create member src directory");
        fs::write(root.join("member/src/lib.rs"), b"pub fn greet() {}\n")
            .expect("write source file");

        let target_dir = root.join("member/target-codex");
        fs::create_dir_all(&target_dir).expect("create target-codex directory");
        let target_artifact = target_dir.join("artifact");
        fs::write(&target_artifact, b"one").expect("write target artifact");

        let log_path = root.join("member/build.log");
        fs::write(&log_path, b"initial").expect("write log file");

        let before = workspace_fingerprint(root).expect("initial fingerprint");
        thread::sleep(Duration::from_millis(20));
        fs::write(&target_artifact, b"two").expect("update target artifact");
        fs::write(&log_path, b"updated").expect("update log file");
        let after = workspace_fingerprint(root).expect("post-ignore fingerprint");

        assert_eq!(before, after);

        thread::sleep(Duration::from_millis(20));
        fs::write(root.join("member/src/lib.rs"), b"pub fn greet() { 3 }\n")
            .expect("update source file");
        let final_fp = workspace_fingerprint(root).expect("final fingerprint");

        assert_ne!(after, final_fp);
    }

    #[test]
    fn fingerprint_with_build_args_changes_on_arg_differences() {
        let base = 42_u64;
        let args_a = vec![
            OsString::from("--features"),
            OsString::from("expensive-telemetry"),
        ];
        let args_b = vec![
            OsString::from("--features"),
            OsString::from("other-feature"),
        ];

        let fingerprint_a = fingerprint_with_build_args(base, &args_a);
        let fingerprint_b = fingerprint_with_build_args(base, &args_b);
        assert_ne!(fingerprint_a, fingerprint_b);
        assert_eq!(fingerprint_a, fingerprint_with_build_args(base, &args_a));
    }

    #[test]
    fn workspace_fingerprint_ignores_custom_target_dir_env() {
        let _guard = lock_env_guard(&PROGRAM_BIN_ENV_GUARD);
        let temp = tempdir().expect("temporary workspace");
        let root = temp.path();
        fs::write(
            root.join("Cargo.toml"),
            "[workspace]\nmembers = [\"member\"]\n",
        )
        .expect("write manifest");
        fs::create_dir_all(root.join("member/src")).expect("create member src directory");
        fs::write(root.join("member/src/lib.rs"), b"pub fn greet() {}\n")
            .expect("write source file");

        let _override_guard = EnvVarGuard::cleared(IROHA_TEST_TARGET_DIR_ENV);
        let _target_guard = EnvVarRestore::set("CARGO_TARGET_DIR", "member/build-output");
        let target_dir = root.join("member/build-output");
        fs::create_dir_all(&target_dir).expect("create target directory");
        let artifact = target_dir.join("artifact");
        fs::write(&artifact, b"one").expect("write target artifact");

        let before = workspace_fingerprint(root).expect("initial fingerprint");
        thread::sleep(Duration::from_millis(20));
        fs::write(&artifact, b"two").expect("update target artifact");
        let after = workspace_fingerprint(root).expect("post-artifact fingerprint");

        assert_eq!(before, after);

        thread::sleep(Duration::from_millis(20));
        fs::write(root.join("member/src/lib.rs"), b"pub fn greet() { 4 }\n")
            .expect("update source file");
        let final_fp = workspace_fingerprint(root).expect("final fingerprint");

        assert_ne!(after, final_fp);
    }

    #[test]
    fn workspace_fingerprint_ignores_test_target_dir_env() {
        let _guard = lock_env_guard(&PROGRAM_BIN_ENV_GUARD);
        let temp = tempdir().expect("temporary workspace");
        let root = temp.path();
        fs::write(
            root.join("Cargo.toml"),
            "[workspace]\nmembers = [\"member\"]\n",
        )
        .expect("write manifest");
        fs::create_dir_all(root.join("member/src")).expect("create member src directory");
        fs::write(root.join("member/src/lib.rs"), b"pub fn greet() {}\n")
            .expect("write source file");

        let _cargo_guard = EnvVarGuard::cleared("CARGO_TARGET_DIR");
        let _target_guard = EnvVarRestore::set(IROHA_TEST_TARGET_DIR_ENV, "member/test-output");
        let target_dir = root.join("member/test-output");
        fs::create_dir_all(&target_dir).expect("create test target directory");
        let artifact = target_dir.join("artifact");
        fs::write(&artifact, b"one").expect("write target artifact");

        let before = workspace_fingerprint(root).expect("initial fingerprint");
        thread::sleep(Duration::from_millis(20));
        fs::write(&artifact, b"two").expect("update target artifact");
        let after = workspace_fingerprint(root).expect("post-artifact fingerprint");

        assert_eq!(before, after);

        thread::sleep(Duration::from_millis(20));
        fs::write(root.join("member/src/lib.rs"), b"pub fn greet() { 5 }\n")
            .expect("update source file");
        let final_fp = workspace_fingerprint(root).expect("final fingerprint");

        assert_ne!(after, final_fp);
    }

    #[test]
    fn resolve_target_dir_prefers_test_override_and_namespaces_cargo() {
        let _guard = lock_env_guard(&PROGRAM_BIN_ENV_GUARD);
        let temp = tempdir().expect("temporary workspace");
        let root = temp.path();

        let _clear_test = EnvVarGuard::cleared(IROHA_TEST_TARGET_DIR_ENV);
        let _clear_cargo = EnvVarGuard::cleared("CARGO_TARGET_DIR");
        assert_eq!(
            resolve_target_dir(root),
            root.join("target").join(IROHA_TEST_TARGET_SUBDIR)
        );

        let _cargo_guard = EnvVarRestore::set("CARGO_TARGET_DIR", "cargo-target");
        assert_eq!(
            resolve_target_dir(root),
            root.join("cargo-target").join(IROHA_TEST_TARGET_SUBDIR)
        );

        let _test_guard = EnvVarRestore::set(IROHA_TEST_TARGET_DIR_ENV, "test-target");
        assert_eq!(resolve_target_dir(root), root.join("test-target"));
    }

    #[test]
    fn default_build_profile_respects_env_override() {
        let _guard = lock_env_guard(&PROGRAM_BIN_ENV_GUARD);
        let _clear_profile = EnvVarGuard::cleared("PROFILE");
        let _clear_override = EnvVarGuard::cleared(IROHA_TEST_BUILD_PROFILE_ENV);
        let default_profile = default_build_profile();
        assert_eq!(default_profile, "release");

        let _override_guard = EnvVarRestore::set("PROFILE", "release");
        assert_eq!(default_build_profile(), "release");

        let _override_guard = EnvVarRestore::set(IROHA_TEST_BUILD_PROFILE_ENV, "debug");
        assert_eq!(default_build_profile(), "debug");
    }

    #[cfg(unix)]
    #[test]
    fn ensure_binary_fresh_skips_redundant_builds() {
        let _guard = lock_env_guard(&PROGRAM_BIN_ENV_GUARD);
        let temp = tempdir().expect("temporary workspace");
        let root = temp.path();
        fs::write(
            root.join("Cargo.toml"),
            "[workspace]\nmembers = [\"member\"]\n",
        )
        .expect("write manifest");
        fs::create_dir_all(root.join("member/src")).expect("create member src directory");
        fs::write(root.join("member/src/lib.rs"), b"pub fn greet() {}\n")
            .expect("write source file");

        let target_dir = root.join("target");
        fs::create_dir_all(target_dir.join("debug")).expect("create target debug directory");
        let binary_path = target_dir.join("debug/dummy");
        fs::write(&binary_path, b"binary").expect("create dummy binary");

        let script = root.join("fake-cargo.sh");
        let script_contents = r#"#!/bin/sh
set -eu
if [ -n "${TEST_NETWORK_CARGO_LOG:-}" ]; then
  printf '%s\n' "$*" >> "${TEST_NETWORK_CARGO_LOG}"
fi
exit 0
"#;
        fs::write(&script, script_contents).expect("write fake cargo script");
        fs::set_permissions(&script, PermissionsExt::from_mode(0o755))
            .expect("make script executable");

        let log_path = root.join("build.log");
        struct EnvRestore {
            key: &'static str,
            previous: Option<String>,
        }
        impl EnvRestore {
            fn set(key: &'static str, value: &str) -> Self {
                let previous = env::var(key).ok();
                unsafe { std::env::set_var(key, value) };
                Self { key, previous }
            }
        }
        impl Drop for EnvRestore {
            fn drop(&mut self) {
                if let Some(ref value) = self.previous {
                    unsafe { std::env::set_var(self.key, value) };
                } else {
                    unsafe { std::env::remove_var(self.key) };
                }
            }
        }
        let script_env = script.to_string_lossy().into_owned();
        let log_env = log_path.to_string_lossy().into_owned();
        let _cargo_guard = EnvRestore::set("TEST_NETWORK_CARGO", &script_env);
        let _log_guard = EnvRestore::set("TEST_NETWORK_CARGO_LOG", &log_env);

        ensure_binary_fresh(
            root,
            "dummy_pkg",
            "dummy",
            &target_dir,
            "debug",
            &binary_path,
            true,
            &[],
        )
        .expect("initial build invocation");

        let first_log = fs::read_to_string(&log_path).expect("read build log after first run");
        assert!(
            !first_log.is_empty(),
            "fake cargo script should log its invocation"
        );

        ensure_binary_fresh(
            root,
            "dummy_pkg",
            "dummy",
            &target_dir,
            "debug",
            &binary_path,
            true,
            &[],
        )
        .expect("second build invocation should be skipped");

        let second_log = fs::read_to_string(&log_path).expect("read build log after second run");
        assert_eq!(
            first_log.lines().count(),
            second_log.lines().count(),
            "second resolve should not trigger an extra cargo invocation"
        );
    }

    #[cfg(unix)]
    #[test]
    fn ensure_binary_fresh_retries_after_e0460_by_cleaning_target_dir() {
        let _guard = lock_env_guard(&PROGRAM_BIN_ENV_GUARD);
        let temp = tempdir().expect("temporary workspace");
        let root = temp.path();
        fs::write(
            root.join("Cargo.toml"),
            "[workspace]\nmembers = [\"member\"]\n",
        )
        .expect("write manifest");
        fs::create_dir_all(root.join("member/src")).expect("create member src directory");
        fs::write(root.join("member/src/lib.rs"), b"pub fn greet() {}\n")
            .expect("write source file");

        let target_dir = root.join("target");
        let binary_path = target_dir.join("debug/dummy");

        let stale_path = target_dir.join("debug/deps/stale");
        fs::create_dir_all(stale_path.parent().expect("stale deps dir"))
            .expect("create stale deps directory");
        fs::write(&stale_path, b"stale").expect("write stale artifact");

        let script = root.join("fake-cargo-e0460.sh");
        let script_contents = r#"#!/bin/sh
set -eu
count_file="${TEST_NETWORK_CARGO_COUNT_FILE:?}"
bin_path="${TEST_NETWORK_DUMMY_BIN:?}"

count=0
if [ -f "$count_file" ]; then
  count="$(cat "$count_file")"
fi
count=$((count+1))
echo "$count" > "$count_file"

if [ "$count" -eq 1 ]; then
  echo "error[E0460]: found possibly newer version of crate \`norito\`" 1>&2
  echo "For more information about this error, try rustc --explain E0460." 1>&2
  exit 101
fi

mkdir -p "$(dirname "$bin_path")"
printf '%s\n' "binary" > "$bin_path"
exit 0
"#;
        fs::write(&script, script_contents).expect("write fake cargo script");
        fs::set_permissions(&script, PermissionsExt::from_mode(0o755))
            .expect("make script executable");

        let count_path = root.join("build-count.txt");

        struct EnvRestore {
            key: &'static str,
            previous: Option<String>,
        }
        impl EnvRestore {
            fn set(key: &'static str, value: &str) -> Self {
                let previous = env::var(key).ok();
                unsafe { std::env::set_var(key, value) };
                Self { key, previous }
            }
        }
        impl Drop for EnvRestore {
            fn drop(&mut self) {
                if let Some(ref value) = self.previous {
                    unsafe { std::env::set_var(self.key, value) };
                } else {
                    unsafe { std::env::remove_var(self.key) };
                }
            }
        }

        let script_env = script.to_string_lossy().into_owned();
        let count_env = count_path.to_string_lossy().into_owned();
        let bin_env = binary_path.to_string_lossy().into_owned();
        let _cargo_guard = EnvRestore::set("TEST_NETWORK_CARGO", &script_env);
        let _count_guard = EnvRestore::set("TEST_NETWORK_CARGO_COUNT_FILE", &count_env);
        let _bin_guard = EnvRestore::set("TEST_NETWORK_DUMMY_BIN", &bin_env);

        ensure_binary_fresh(
            root,
            "dummy_pkg",
            "dummy",
            &target_dir,
            "debug",
            &binary_path,
            true,
            &[],
        )
        .expect("retry build after E0460 should succeed");

        assert!(binary_path.exists(), "dummy binary should be created");
        assert!(
            !stale_path.exists(),
            "cleanup should remove stale build artifacts"
        );

        let count: u32 = fs::read_to_string(&count_path)
            .expect("read build count")
            .trim()
            .parse()
            .expect("parse build count");
        assert_eq!(
            count, 2,
            "fake cargo should be invoked twice (fail then retry)"
        );
    }

    #[cfg(unix)]
    #[test]
    fn ensure_binary_fresh_retries_after_e0463_by_cleaning_target_dir() {
        let _guard = lock_env_guard(&PROGRAM_BIN_ENV_GUARD);
        let temp = tempdir().expect("temporary workspace");
        let root = temp.path();
        fs::write(
            root.join("Cargo.toml"),
            "[workspace]\nmembers = [\"member\"]\n",
        )
        .expect("write manifest");
        fs::create_dir_all(root.join("member/src")).expect("create member src directory");
        fs::write(root.join("member/src/lib.rs"), b"pub fn greet() {}\n")
            .expect("write source file");

        let target_dir = root.join("target");
        let binary_path = target_dir.join("debug/dummy");

        let stale_path = target_dir.join("release/deps/stale");
        fs::create_dir_all(stale_path.parent().expect("stale deps dir"))
            .expect("create stale deps directory");
        fs::write(&stale_path, b"stale").expect("write stale artifact");

        let script = root.join("fake-cargo-e0463.sh");
        let script_contents = r#"#!/bin/sh
set -eu
count_file="${TEST_NETWORK_CARGO_COUNT_FILE:?}"
bin_path="${TEST_NETWORK_DUMMY_BIN:?}"

count=0
if [ -f "$count_file" ]; then
  count="$(cat "$count_file")"
fi
count=$((count+1))
echo "$count" > "$count_file"

if [ "$count" -eq 1 ]; then
  echo "error[E0463]: can't find crate for \`norito\`" 1>&2
  echo "For more information about this error, try \`rustc --explain E0463\`." 1>&2
  exit 101
fi

mkdir -p "$(dirname "$bin_path")"
printf '%s\n' "binary" > "$bin_path"
exit 0
"#;
        fs::write(&script, script_contents).expect("write fake cargo script");
        fs::set_permissions(&script, PermissionsExt::from_mode(0o755))
            .expect("make script executable");

        let count_path = root.join("build-count.txt");

        struct EnvRestore {
            key: &'static str,
            previous: Option<String>,
        }
        impl EnvRestore {
            fn set(key: &'static str, value: &str) -> Self {
                let previous = env::var(key).ok();
                unsafe { std::env::set_var(key, value) };
                Self { key, previous }
            }
        }
        impl Drop for EnvRestore {
            fn drop(&mut self) {
                if let Some(ref value) = self.previous {
                    unsafe { std::env::set_var(self.key, value) };
                } else {
                    unsafe { std::env::remove_var(self.key) };
                }
            }
        }

        let script_env = script.to_string_lossy().into_owned();
        let count_env = count_path.to_string_lossy().into_owned();
        let bin_env = binary_path.to_string_lossy().into_owned();
        let _cargo_guard = EnvRestore::set("TEST_NETWORK_CARGO", &script_env);
        let _count_guard = EnvRestore::set("TEST_NETWORK_CARGO_COUNT_FILE", &count_env);
        let _bin_guard = EnvRestore::set("TEST_NETWORK_DUMMY_BIN", &bin_env);

        ensure_binary_fresh(
            root,
            "dummy_pkg",
            "dummy",
            &target_dir,
            "debug",
            &binary_path,
            true,
            &[],
        )
        .expect("retry build after E0463 should succeed");

        assert!(binary_path.exists(), "dummy binary should be created");
        assert!(
            !stale_path.exists(),
            "cleanup should remove stale build artifacts"
        );

        let count: u32 = fs::read_to_string(&count_path)
            .expect("read build count")
            .trim()
            .parse()
            .expect("parse build count");
        assert_eq!(
            count, 2,
            "fake cargo should be invoked twice (fail then retry)"
        );
    }

    #[test]
    fn ensure_binary_fresh_refuses_reentrant_builds() {
        let _guard = lock_env_guard(&PROGRAM_BIN_ENV_GUARD);
        let temp = tempdir().expect("temporary workspace");
        let root = temp.path();
        fs::write(
            root.join("Cargo.toml"),
            "[workspace]\nmembers = [\"member\"]\n",
        )
        .expect("write manifest");
        fs::create_dir_all(root.join("member/src")).expect("create member src directory");
        fs::write(root.join("member/src/lib.rs"), b"pub fn greet() {}\n")
            .expect("write source file");

        let target_dir = root.join("target");
        let binary_path = target_dir.join("debug/dummy");

        let err = ensure_binary_fresh(
            root,
            "dummy_pkg",
            "dummy",
            &target_dir,
            "debug",
            &binary_path,
            false,
            &[],
        )
        .expect_err("re-entrant build should be rejected when building is disallowed");

        assert!(
            err.to_string().contains("cannot build `dummy`"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn reentrant_builds_enabled_under_cargo_by_default() {
        let _guard = lock_env_guard(&PROGRAM_BIN_ENV_GUARD);
        let _override_guard = EnvVarGuard::cleared("IROHA_TEST_ALLOW_REENTRANT_BUILD");

        assert!(allow_reentrant_build(true));
    }

    #[test]
    fn reentrant_builds_can_be_enabled_via_env() {
        let _guard = lock_env_guard(&PROGRAM_BIN_ENV_GUARD);
        let _override_guard = EnvVarGuard::cleared("IROHA_TEST_ALLOW_REENTRANT_BUILD");
        set_env_var("IROHA_TEST_ALLOW_REENTRANT_BUILD", "true");

        assert!(allow_reentrant_build(true));
    }

    #[test]
    fn reentrant_builds_can_be_enabled_via_numeric_env() {
        let _guard = lock_env_guard(&PROGRAM_BIN_ENV_GUARD);
        let _override_guard = EnvVarGuard::cleared("IROHA_TEST_ALLOW_REENTRANT_BUILD");
        set_env_var("IROHA_TEST_ALLOW_REENTRANT_BUILD", "1");

        assert!(allow_reentrant_build(true));
    }

    #[test]
    fn reentrant_builds_can_be_disabled_via_env() {
        let _guard = lock_env_guard(&PROGRAM_BIN_ENV_GUARD);
        let _override_guard = EnvVarGuard::cleared("IROHA_TEST_ALLOW_REENTRANT_BUILD");
        set_env_var("IROHA_TEST_ALLOW_REENTRANT_BUILD", "false");

        assert!(!allow_reentrant_build(true));
    }

    #[test]
    fn env_filter_defaults_to_warn() {
        let _guard = lock_env_guard(&LOG_ENV_GUARD);
        let original = env::var("RUST_LOG").ok();
        remove_env_var("RUST_LOG");

        let filter = env_filter_from_env_or_default();

        if let Some(value) = original {
            set_env_var("RUST_LOG", value);
        } else {
            remove_env_var("RUST_LOG");
        }

        assert_eq!(filter.to_string(), "warn");
    }

    #[test]
    fn env_filter_honors_rust_log_override() {
        let _guard = lock_env_guard(&LOG_ENV_GUARD);
        let original = env::var("RUST_LOG").ok();
        set_env_var("RUST_LOG", "warn");

        let filter = env_filter_from_env_or_default();

        if let Some(value) = original {
            set_env_var("RUST_LOG", value);
        } else {
            remove_env_var("RUST_LOG");
        }

        assert_eq!(filter.to_string(), "warn");
    }

    #[test]
    fn summarize_peer_stderr_ignores_empty_input() {
        assert!(summarize_peer_stderr("").is_none());
        assert!(summarize_peer_stderr("\n\n").is_none());
    }

    #[test]
    fn summarize_peer_stderr_truncates_to_tail_lines() {
        let input = (0..100)
            .map(|idx| format!("line {idx}"))
            .collect::<Vec<_>>()
            .join("\n");

        let summary = summarize_peer_stderr(&input).expect("summary should exist");

        assert!(summary.truncated);
        assert_eq!(summary.total_lines, 100);
        assert!(summary.preview.lines().count() <= PEER_STDERR_PREVIEW_MAX_LINES);
        assert!(summary.preview.ends_with("line 99"));
    }

    #[test]
    fn summarize_peer_stderr_limits_character_count() {
        let long_line = "x".repeat(PEER_STDERR_PREVIEW_MAX_CHARS + 10);
        let input = format!("first\n{long_line}");

        let summary = summarize_peer_stderr(&input).expect("summary should exist");

        assert!(summary.truncated);
        assert_eq!(summary.total_lines, 2);
        assert!(summary.preview.len() <= PEER_STDERR_PREVIEW_MAX_CHARS);
        assert!(summary.preview.ends_with('x'));
    }

    #[test]
    fn canonical_genesis_bytes_roundtrip_signed_block() {
        init_instruction_registry();
        let network = NetworkBuilder::new().build();
        let genesis = network.genesis();

        println!(
            "GENESIS contains {} transactions",
            network.genesis_isi().len()
        );
        for (tx_idx, tx) in network.genesis_isi().iter().enumerate() {
            println!("GENESIS tx {tx_idx} has {} instructions", tx.len());
            for (instr_idx, instr) in tx.iter().enumerate() {
                let type_name = Instruction::id(&**instr);
                println!("GENESIS instruction tx {tx_idx} idx {instr_idx}: {type_name}");
                let encoded = norito::to_bytes(instr).expect("encode genesis instruction");
                norito::from_bytes::<iroha_data_model::isi::InstructionBox>(&encoded)
                    .unwrap_or_else(|error| {
                        panic!(
                            "genesis instruction decode failed at tx #{tx_idx} instr #{instr_idx}: {error}"
                        )
                    });
            }
        }

        let wire =
            NetworkPeer::canonical_genesis_bytes(&genesis).expect("canonical genesis encoding");
        println!(
            "canonical wire header_flags_byte=0x{:02x}",
            wire[1 + norito::core::Header::SIZE - 1]
        );
        println!("wire prefix {:?}", &wire[..32.min(wire.len())]);
        let header_size = norito::core::Header::SIZE;
        println!(
            "payload prefix {:?}",
            &wire[header_size..(header_size + 32).min(wire.len())]
        );

        decode_framed_signed_block(&wire).expect("decode framed genesis block");
    }

    fn collect_set_parameters(block: &GenesisBlock) -> Vec<Parameter> {
        block
            .0
            .transactions_vec()
            .iter()
            .flat_map(|tx| match tx.instructions() {
                Executable::Instructions(instructions) => instructions
                    .iter()
                    .filter_map(|instruction| {
                        instruction
                            .as_any()
                            .downcast_ref::<SetParameter>()
                            .map(|set| set.inner().clone())
                    })
                    .collect::<Vec<_>>(),
                Executable::Ivm(_) => Vec::new(),
                Executable::IvmProved(_) => Vec::new(),
            })
            .collect()
    }

    fn consensus_handshake_payload(block: &GenesisBlock) -> Option<JsonValue> {
        let mut last = None;
        for parameter in collect_set_parameters(block) {
            if let Parameter::Custom(custom) = parameter
                && custom.id() == &consensus_metadata::handshake_meta_id()
                && let Ok(payload) = custom.payload().try_into_any()
            {
                last = Some(payload);
            }
        }
        last
    }

    fn consensus_fingerprint_from_block(block: &GenesisBlock) -> Option<String> {
        let JsonValue::Object(mut map) = consensus_handshake_payload(block)? else {
            return None;
        };
        match map.remove("consensus_fingerprint") {
            Some(JsonValue::String(fp)) => Some(fp),
            _ => None,
        }
    }

    #[test]
    fn startup_diagnostics_warn_on_mismatched_da_enabled() {
        if skip_network_tests("startup_diagnostics_warn_on_mismatched_da_enabled") {
            return;
        }
        let _sumeragi_guard = lock_env_guard(&SUMERAGI_ENV_GUARD);
        let _disable_da = disable_sumeragi_env_overrides();
        let network =
            build_with_isolated_permit(NetworkBuilder::new().with_peers(2).with_config_layer(
                |layer| {
                    layer.write(["sumeragi", "da", "enabled"], false);
                },
            ));

        let buffer = BufferWriter(Arc::new(Mutex::new(Vec::new())));
        let subscriber = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::WARN)
            .with_ansi(false)
            .with_writer(buffer.clone())
            .finish();

        tracing::subscriber::with_default(subscriber, || network.log_startup_diagnostics());

        let output = String::from_utf8(buffer.0.lock().unwrap().clone()).unwrap();
        assert!(
            output.contains("Data availability enablement mismatch"),
            "diagnostics should warn when config/parameter DA flags diverge"
        );
    }

    #[test]
    fn startup_diagnostics_silent_when_da_enabled_align() {
        if skip_network_tests("startup_diagnostics_silent_when_da_enabled_align") {
            return;
        }
        let _sumeragi_guard = lock_env_guard(&SUMERAGI_ENV_GUARD);
        let _disable_da = disable_sumeragi_env_overrides();
        let network = build_with_isolated_permit(NetworkBuilder::new().with_peers(2));

        let buffer = BufferWriter(Arc::new(Mutex::new(Vec::new())));
        let subscriber = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::WARN)
            .with_ansi(false)
            .with_writer(buffer.clone())
            .finish();

        tracing::subscriber::with_default(subscriber, || network.log_startup_diagnostics());

        let output = String::from_utf8(buffer.0.lock().unwrap().clone()).unwrap();
        assert!(
            !output.contains("Data availability enablement mismatch"),
            "diagnostics must stay silent when DA flags align"
        );
    }

    fn reconstructed_consensus_params(block: &GenesisBlock) -> ConsensusGenesisParams {
        let mut state = iroha_data_model::parameter::Parameters::default();
        for parameter in collect_set_parameters(block) {
            state.set_parameter(parameter);
        }
        let sumeragi = state.sumeragi();
        let block_params = state.block();
        ConsensusGenesisParams {
            block_time_ms: sumeragi.block_time_ms(),
            commit_time_ms: sumeragi.commit_time_ms(),
            min_finality_ms: sumeragi.min_finality_ms(),
            max_clock_drift_ms: sumeragi.max_clock_drift_ms(),
            collectors_k: sumeragi.collectors_k(),
            redundant_send_r: sumeragi.collectors_redundant_send_r(),
            block_max_transactions: block_params.max_transactions().get(),
            da_enabled: sumeragi.da_enabled(),
            epoch_length_blocks: 0,
            bls_domain: PERMISSIONED_BLS_DOMAIN.to_string(),
            npos: None,
        }
    }

    #[test]
    fn genesis_consensus_metadata_matches_runtime_profile() {
        init_instruction_registry();
        let network = build_with_isolated_permit(NetworkBuilder::new().with_peers(4));
        let genesis = network.genesis();
        let mut saw_da_enabled = None;
        for parameter in collect_set_parameters(&genesis) {
            if let Parameter::Sumeragi(SumeragiParameter::DaEnabled(value)) = parameter {
                saw_da_enabled = Some(value);
            }
        }
        let actual = consensus_fingerprint_from_block(&genesis)
            .expect("genesis should contain consensus fingerprint metadata");
        let profile = network.consensus_bootstrap_profile();
        assert_eq!(
            profile.chain_id,
            network.chain_id(),
            "bootstrap profile should reuse the network chain id"
        );
        assert_eq!(
            saw_da_enabled.unwrap_or(false),
            profile.params.da_enabled,
            "genesis should encode DA enablement"
        );
        let reconstructed = reconstructed_consensus_params(&genesis);
        eprintln!(
            "profile params = {:?}\nreconstructed params = {:?}",
            profile.params,
            reconstructed
        );
        assert_eq!(
            reconstructed.block_time_ms, profile.params.block_time_ms,
            "genesis parameters should preserve block timing"
        );
        assert_eq!(
            reconstructed.commit_time_ms, profile.params.commit_time_ms,
            "genesis parameters should preserve commit timing"
        );
        assert_eq!(
            reconstructed.max_clock_drift_ms, profile.params.max_clock_drift_ms,
            "genesis parameters should preserve max clock drift"
        );
        assert_eq!(
            reconstructed.collectors_k, profile.params.collectors_k,
            "genesis parameters should preserve collectors_k"
        );
        assert_eq!(
            reconstructed.redundant_send_r, profile.params.redundant_send_r,
            "genesis parameters should preserve redundant_send_r"
        );
        assert_eq!(
            reconstructed.block_max_transactions, profile.params.block_max_transactions,
            "genesis parameters should preserve block sizing"
        );
        let expected_bytes = compute_consensus_fingerprint_from_params(
            &network.chain_id(),
            &profile.params,
            profile.mode_tag,
        );
        let expected = format!("0x{}", hex_lower(&expected_bytes));
        let mut metadatas = Vec::<JsonValue>::new();
        for parameter in collect_set_parameters(&genesis) {
            if let Parameter::Custom(custom) = parameter
                && custom.id() == &consensus_metadata::handshake_meta_id()
                && let Ok(payload) = custom.payload().try_into_any()
            {
                metadatas.push(payload);
            }
        }
        eprintln!("consensus profile fingerprint = {expected}");
        eprintln!(
            "genesis consensus handshakes = {metadatas:#?} (count={})",
            metadatas.len()
        );
        assert_eq!(
            actual.to_ascii_lowercase(),
            expected,
            "consensus fingerprint mismatch: expected {expected}, got {actual}"
        );
    }

    #[test]
    fn genesis_consensus_metadata_tracks_npos_mode() {
        init_instruction_registry();
        let network =
            build_with_isolated_permit(NetworkBuilder::new().with_peers(4).with_config_layer(
                |layer| {
                    layer.write(["sumeragi", "consensus_mode"], "npos");
                },
            ));
        let profile = network.consensus_bootstrap_profile();
        assert_eq!(
            profile.mode_tag, NPOS_TAG,
            "network builder should detect NPoS consensus"
        );
        assert_eq!(
            profile.bls_domain, NPOS_BLS_DOMAIN,
            "NPoS handshake must use the NPoS BLS domain"
        );
        assert_eq!(
            profile.params.epoch_length_blocks,
            defaults::sumeragi::EPOCH_LENGTH_BLOCKS,
            "epoch length should follow config defaults when unspecified"
        );
        assert!(
            profile.params.npos.is_some(),
            "NPoS profile must embed NPoS genesis parameters"
        );

        let genesis = network.genesis();
        let payload = consensus_handshake_payload(&genesis)
            .expect("genesis should encode consensus handshake metadata");
        let JsonValue::Object(map) = payload else {
            panic!("handshake metadata must be encoded as a JSON object");
        };
        assert_eq!(
            map.get("mode").and_then(JsonValue::as_str),
            Some("Npos"),
            "handshake metadata should advertise NPoS mode"
        );
        assert_eq!(
            map.get("bls_domain").and_then(JsonValue::as_str),
            Some(NPOS_BLS_DOMAIN),
            "handshake metadata should encode the NPoS BLS domain"
        );
        let actual = consensus_fingerprint_from_block(&genesis)
            .expect("genesis should contain consensus fingerprint")
            .to_ascii_lowercase();
        let expected_bytes = compute_consensus_fingerprint_from_params(
            &network.chain_id(),
            &profile.params,
            profile.mode_tag,
        );
        let expected = format!("0x{}", hex_lower(&expected_bytes));
        assert_eq!(
            actual, expected,
            "NPoS fingerprint must match runtime profile"
        );
    }

    #[test]
    fn genesis_consensus_metadata_respects_npos_timeout_overrides() {
        init_instruction_registry();
        let network =
            build_with_isolated_permit(NetworkBuilder::new().with_peers(4).with_config_layer(
                |layer| {
                    layer
                        .write(["sumeragi", "consensus_mode"], "npos")
                        .write(
                            ["sumeragi", "advanced", "npos", "timeouts", "propose_ms"],
                            166i64,
                        )
                        .write(
                            ["sumeragi", "advanced", "npos", "timeouts", "prevote_ms"],
                            213i64,
                        )
                        .write(
                            ["sumeragi", "advanced", "npos", "timeouts", "precommit_ms"],
                            260i64,
                        )
                        .write(
                            ["sumeragi", "advanced", "npos", "timeouts", "commit_ms"],
                            355i64,
                        )
                        .write(
                            ["sumeragi", "advanced", "npos", "timeouts", "da_ms"],
                            307i64,
                        )
                        .write(
                            ["sumeragi", "advanced", "npos", "timeouts", "aggregator_ms"],
                            57i64,
                        );
                },
            ));
        let genesis = network.genesis();
        let actual = consensus_fingerprint_from_block(&genesis)
            .expect("genesis should contain consensus fingerprint")
            .to_ascii_lowercase();
        let profile = network.consensus_bootstrap_profile();
        let expected_bytes = compute_consensus_fingerprint_from_params(
            &network.chain_id(),
            &profile.params,
            profile.mode_tag,
        );
        let expected = format!("0x{}", hex_lower(&expected_bytes));
        assert_eq!(
            actual, expected,
            "consensus fingerprint must respect NPoS timeout overrides"
        );
    }

    #[test]
    fn genesis_embeds_da_proof_policies_from_config_layers() {
        init_instruction_registry();
        let network =
            build_with_isolated_permit(NetworkBuilder::new().with_peers(4).with_config_layer(
                |layer| {
                    let mut lane0 = Table::new();
                    lane0.insert("index".into(), Value::Integer(0));
                    lane0.insert("alias".into(), Value::String("alpha".to_string()));
                    lane0.insert("metadata".into(), Value::Table(Table::new()));
                    let mut lane1 = Table::new();
                    lane1.insert("index".into(), Value::Integer(1));
                    lane1.insert("alias".into(), Value::String("beta".to_string()));
                    lane1.insert("metadata".into(), Value::Table(Table::new()));
                    let lane_catalog = Value::Array(vec![Value::Table(lane0), Value::Table(lane1)]);
                    layer
                        .write(["nexus", "enabled"], true)
                        .write(["nexus", "lane_count"], 2i64)
                        .write(["nexus", "lane_catalog"], lane_catalog);
                },
            ));

        let config_layers: Vec<Table> = network.config_layers().map(Cow::into_owned).collect();
        let peer = network.peers().first().expect("network should have peers");
        let mut merged = peer.base_config_table();
        for layer in &config_layers {
            merge_tables(&mut merged, layer);
        }
        let nexus = merged
            .get("nexus")
            .and_then(Value::as_table)
            .expect("nexus table should exist");
        assert_eq!(
            nexus.get("enabled").and_then(Value::as_bool),
            Some(true),
            "config should enable nexus when lane_count is set"
        );
        assert_eq!(
            nexus.get("lane_count").and_then(Value::as_integer),
            Some(2),
            "config should retain the overridden lane_count"
        );

        let policies = resolve_da_proof_policies(peer, &config_layers)
            .expect("should resolve da proof policies");
        assert_eq!(policies.policies.len(), 2);
        let actual = resolve_actual_config(peer, &config_layers)
            .expect("should resolve full config for genesis");
        assert_eq!(
            actual.nexus.lane_config.entries().len(),
            2,
            "resolved lane config should preserve the lane catalog"
        );

        let genesis = network.genesis();
        assert_eq!(
            genesis.0.da_proof_policies(),
            Some(&policies),
            "genesis should embed da proof policies"
        );
        assert_eq!(
            genesis.0.header().da_proof_policies_hash(),
            Some(HashOf::new(&policies))
        );
    }

    #[test]
    fn resolve_actual_config_applies_sora_profile_when_required() {
        let config_layers = vec![Table::new().write(["sorafs", "storage", "enabled"], true)];

        assert!(
            config_requires_sora_profile(&config_layers),
            "SoraFS-enabled configs should trigger --sora profile detection"
        );

        let mut merged = sora_profile_detection_defaults();
        for layer in &config_layers {
            merge_tables(&mut merged, layer);
        }
        apply_streaming_identity_defaults_for_detection(&mut merged);
        ensure_sora_profile_trusted_peer_pop(&mut merged);

        let actual = parse_actual_config_for_genesis(merged, &config_layers)
            .expect("should resolve runtime-equivalent config");
        assert_eq!(
            actual.sumeragi.consensus_mode,
            ConsensusMode::Npos,
            "Sora profile should force NPoS consensus mode"
        );
        assert!(
            actual.nexus.enabled,
            "Sora profile should enable nexus in resolved config"
        );
        assert!(
            actual.nexus.lane_config.entries().len() > 1,
            "Sora profile should expand lane catalog beyond single-lane defaults"
        );
    }

    #[test]
    fn config_layers_include_trusted_peer_pop_and_bls() {
        use std::collections::{BTreeMap, BTreeSet};

        fn assert_trusted_entries(network: &Network) {
            let mut layers = network.config_layers();
            let base = layers.next().expect("base config layer").into_owned();

            let mut expected_pop = BTreeMap::new();
            for peer in network.peers() {
                expected_pop.insert(
                    peer.bls_public_key()
                        .expect("trusted peer should have auto-generated BLS key")
                        .to_string(),
                    format!(
                        "0x{}",
                        hex_lower(
                            peer.bls_pop()
                                .expect("trusted peer should have auto-generated PoP")
                        )
                    ),
                );
            }

            let expected_trusted: BTreeSet<String> = network
                .peers()
                .iter()
                .map(|peer| {
                    format!(
                        "{}@{}",
                        peer.network_peer_id(),
                        peer.p2p_address().to_literal()
                    )
                })
                .collect();

            let trusted_entries = base
                .get("trusted_peers")
                .and_then(toml::Value::as_array)
                .expect("trusted_peers array");
            let actual_trusted: BTreeSet<String> = trusted_entries
                .iter()
                .map(|value| {
                    value
                        .as_str()
                        .expect("trusted_peers entry string")
                        .to_string()
                })
                .collect();
            assert_eq!(actual_trusted, expected_trusted);

            let pop_entries = base
                .get("trusted_peers_pop")
                .and_then(toml::Value::as_array)
                .expect("trusted_peers_pop array");
            assert_eq!(pop_entries.len(), expected_pop.len());
            for entry in pop_entries {
                let table = entry.as_table().expect("pop entry table");
                let pk = table
                    .get("public_key")
                    .and_then(toml::Value::as_str)
                    .expect("pop public key");
                let pop_hex = table
                    .get("pop_hex")
                    .and_then(toml::Value::as_str)
                    .expect("pop hex");
                let expected = expected_pop.get(pk).expect("expected pop entry");
                assert_eq!(expected.as_str(), pop_hex);
            }
        }

        let default_network = NetworkBuilder::new().with_peers(3).build();
        assert_trusted_entries(&default_network);

        let explicit_network = NetworkBuilder::new()
            .with_peers(2)
            .with_auto_populated_trusted_peers()
            .build();
        assert_trusted_entries(&explicit_network);
    }

    #[test]
    fn config_layers_with_additional_peers_include_pop() {
        let network = NetworkBuilder::new().with_peers(1).build();
        let extra_peer = NetworkPeerBuilder::new().build(network.env());

        let mut layers = network.config_layers_with_additional_peers([&extra_peer]);
        let base = layers.next().expect("base config layer").into_owned();

        let pop_entries = base
            .get("trusted_peers_pop")
            .and_then(toml::Value::as_array)
            .expect("trusted_peers_pop array");
        let extra_pk = extra_peer
            .bls_public_key()
            .expect("extra peer should have BLS key")
            .to_string();
        assert!(
            pop_entries.iter().any(|entry| {
                entry
                    .get("public_key")
                    .and_then(toml::Value::as_str)
                    .map(|pk| pk == extra_pk)
                    .unwrap_or(false)
            }),
            "additional peer PoP should be threaded into trusted_peers_pop"
        );
    }

    #[test]
    fn trusted_peers_layer_for_parse_includes_pop_entries() {
        let env = Environment::new();
        let peers = vec![
            NetworkPeerBuilder::new().build(&env),
            NetworkPeerBuilder::new().build(&env),
        ];

        let layer = trusted_peers_layer_for_parse(&peers, true);
        let trusted_entries = layer
            .get("trusted_peers")
            .and_then(toml::Value::as_array)
            .expect("trusted_peers array");
        assert_eq!(trusted_entries.len(), peers.len());

        let pop_entries = layer
            .get("trusted_peers_pop")
            .and_then(toml::Value::as_array)
            .expect("trusted_peers_pop array");
        assert_eq!(pop_entries.len(), peers.len());

        for peer in &peers {
            let pk = peer
                .bls_public_key()
                .expect("peer should have BLS key")
                .to_string();
            assert!(
                pop_entries.iter().any(|entry| {
                    entry
                        .get("public_key")
                        .and_then(toml::Value::as_str)
                        .is_some_and(|value| value == pk)
                }),
                "trusted_peers_pop should include {pk}"
            );
        }

        let layer_without_pop = trusted_peers_layer_for_parse(&peers, false);
        assert!(
            layer_without_pop.get("trusted_peers_pop").is_none(),
            "trusted_peers_pop should be omitted when auto-populate is disabled"
        );
    }

    #[test]
    fn config_layers_allow_local_preauth_bypass() {
        let network = NetworkBuilder::new().build();
        let layers: Vec<_> = network.config_layers().collect();
        let allowlist = layers
            .iter()
            .find_map(|layer| {
                layer
                    .as_ref()
                    .get("torii")
                    .and_then(toml::Value::as_table)
                    .and_then(|torii| torii.get("preauth_allow_cidrs"))
                    .and_then(toml::Value::as_array)
            })
            .expect("preauth_allow_cidrs array");
        let entries: Vec<&str> = allowlist.iter().filter_map(toml::Value::as_str).collect();
        assert!(
            entries.contains(&"127.0.0.1/32"),
            "IPv4 loopback should bypass pre-auth gating"
        );
        assert!(
            entries.contains(&"::1/128"),
            "IPv6 loopback should bypass pre-auth gating"
        );
    }

    #[test]
    fn default_builder_disables_nexus() {
        let NetworkBuilder { config_layers, .. } = NetworkBuilder::new();
        let disabled = config_layers
            .iter()
            .any(|layer| read_bool(layer, &["nexus", "enabled"]) == Some(false));
        assert!(
            disabled,
            "default NetworkBuilder must set nexus.enabled=false"
        );
    }

    #[test]
    fn default_builder_scales_concurrency_defaults() {
        let NetworkBuilder { config_layers, .. } = NetworkBuilder::new();
        let base = config_layers
            .iter()
            .find(|layer| layer.get("concurrency").is_some())
            .expect("base config layer should include concurrency defaults");
        let concurrency = base
            .get("concurrency")
            .and_then(toml::Value::as_table)
            .expect("concurrency table");
        let expected =
            i64::try_from(test_concurrency_threads()).expect("test concurrency threads fit in i64");
        assert_eq!(
            concurrency
                .get("scheduler_min_threads")
                .and_then(toml::Value::as_integer),
            Some(expected)
        );
        assert_eq!(
            concurrency
                .get("scheduler_max_threads")
                .and_then(toml::Value::as_integer),
            Some(expected)
        );
        assert_eq!(
            concurrency
                .get("rayon_global_threads")
                .and_then(toml::Value::as_integer),
            Some(expected)
        );
        let pipeline = base
            .get("pipeline")
            .and_then(toml::Value::as_table)
            .expect("pipeline table");
        assert_eq!(
            pipeline.get("workers").and_then(toml::Value::as_integer),
            Some(expected)
        );
    }

    #[test]
    fn builder_config_layers_parse_with_required_genesis_fields() {
        let env = Environment::new();
        let peer = NetworkPeerBuilder::new().build(&env);
        let NetworkBuilder {
            config_layers,
            genesis_key_pair,
            ..
        } = NetworkBuilder::new();
        let bls_public_key = peer
            .bls_public_key()
            .expect("test peer should have BLS key");
        let bls_pop = peer.bls_pop().expect("test peer should have BLS PoP");
        let mut pop_entry = Table::new();
        pop_entry.insert(
            "public_key".into(),
            Value::String(bls_public_key.to_string()),
        );
        pop_entry.insert(
            "pop_hex".into(),
            Value::String(format!("0x{}", hex_lower(bls_pop))),
        );
        let trusted_peers_pop = Value::Array(vec![Value::Table(pop_entry)]);
        let mut layers = Vec::with_capacity(config_layers.len() + 1);
        layers.push(
            Table::new()
                .write("chain", config::chain_id().to_string())
                .write(
                    ["genesis", "public_key"],
                    genesis_key_pair.public_key().to_string(),
                )
                .write(["trusted_peers_pop"], trusted_peers_pop),
        );
        layers.extend(config_layers);

        let actual = resolve_actual_config(&peer, &layers);
        assert!(
            actual.is_some(),
            "builder config layers should parse once chain/genesis are provided"
        );
    }

    #[test]
    fn config_layers_normalize_legacy_sumeragi_keys() {
        let builder = NetworkBuilder::new()
            .with_config_layer(|layer| {
                layer
                    .write(["sumeragi", "collectors_k"], 2_i64)
                    .write(["sumeragi", "da_enabled"], true)
                    .write(["sumeragi", "epoch_length_blocks"], 12_i64)
                    .write(["sumeragi", "pacemaker_max_backoff_ms"], 12_000_i64)
                    .write(["sumeragi", "rbc_session_ttl_secs"], 5_i64);
            })
            .with_config_table({
                let mut sumeragi = Table::new();
                sumeragi.insert("collectors_redundant_send_r".into(), TomlValue::Integer(3));
                sumeragi.insert("rbc_disk_store_ttl_secs".into(), TomlValue::Integer(7));
                sumeragi.insert("rbc_pending_ttl_ms".into(), TomlValue::Integer(500));
                let mut table = Table::new();
                table.insert("sumeragi".into(), TomlValue::Table(sumeragi));
                table
            });
        let NetworkBuilder { config_layers, .. } = builder;
        let layer_from_writer = config_layers
            .get(1)
            .expect("custom layer from with_config_layer");
        let layer_from_table = config_layers
            .get(2)
            .expect("custom layer from with_config_table");

        let sumeragi_writer = layer_from_writer
            .get("sumeragi")
            .and_then(TomlValue::as_table)
            .expect("sumeragi table");
        assert!(!sumeragi_writer.contains_key("collectors_k"));
        assert_eq!(
            get_nested_value(sumeragi_writer, &["collectors", "k"]),
            Some(&TomlValue::Integer(2))
        );
        assert!(!sumeragi_writer.contains_key("da_enabled"));
        assert_eq!(
            get_nested_value(sumeragi_writer, &["da", "enabled"]),
            Some(&TomlValue::Boolean(true))
        );
        assert!(!sumeragi_writer.contains_key("epoch_length_blocks"));
        assert_eq!(
            get_nested_value(sumeragi_writer, &["npos", "epoch_length_blocks"]),
            Some(&TomlValue::Integer(12))
        );
        assert!(!sumeragi_writer.contains_key("pacemaker_max_backoff_ms"));
        assert_eq!(
            get_nested_value(
                sumeragi_writer,
                &["advanced", "pacemaker", "max_backoff_ms"]
            ),
            Some(&TomlValue::Integer(12_000))
        );
        assert!(!sumeragi_writer.contains_key("rbc_session_ttl_secs"));
        assert_eq!(
            get_nested_value(sumeragi_writer, &["advanced", "rbc", "session_ttl_ms"]),
            Some(&TomlValue::Integer(5_000))
        );

        let sumeragi_table = layer_from_table
            .get("sumeragi")
            .and_then(TomlValue::as_table)
            .expect("sumeragi table");
        assert!(!sumeragi_table.contains_key("collectors_redundant_send_r"));
        assert_eq!(
            get_nested_value(sumeragi_table, &["collectors", "redundant_send_r"]),
            Some(&TomlValue::Integer(3))
        );
        assert!(!sumeragi_table.contains_key("rbc_disk_store_ttl_secs"));
        assert_eq!(
            get_nested_value(sumeragi_table, &["advanced", "rbc", "disk_store_ttl_ms"]),
            Some(&TomlValue::Integer(7_000))
        );
        assert!(!sumeragi_table.contains_key("rbc_pending_ttl_ms"));
        assert_eq!(
            get_nested_value(sumeragi_table, &["advanced", "rbc", "pending_ttl_ms"]),
            Some(&TomlValue::Integer(500))
        );
    }

    #[test]
    fn config_layers_without_pop_excludes_bls_entries() {
        let network = NetworkBuilder::new()
            .without_auto_populated_trusted_peers()
            .with_peers(2)
            .build();
        let mut layers = network.config_layers();
        let _trusted = layers.next().expect("trusted peers layer");
        let base = layers.next().expect("base config layer").into_owned();
        dbg!(&base);

        assert!(base.get("trusted_peers_bls").is_none());
        assert!(base.get("trusted_peers_pop").is_none());
    }

    #[test]
    fn default_network_enables_da() {
        let _guard = lock_env_guard(&SUMERAGI_ENV_GUARD);
        let _disable_da = disable_sumeragi_env_overrides();
        let network = NetworkBuilder::new().build();

        assert!(
            network.consensus_bootstrap_profile().params.da_enabled,
            "default permissioned consensus profile should keep DA enabled"
        );

        let mut layers = network.config_layers();
        let _trusted = layers.next().expect("trusted peers layer");
        let base = layers.next().expect("base config layer").into_owned();
        let rbc_flag = base
            .get("sumeragi")
            .unwrap_or_else(|| {
                let keys = base.keys().cloned().collect::<Vec<_>>();
                panic!("missing sumeragi table; keys={keys:?}")
            })
            .as_table()
            .expect("sumeragi entry must be a table");
        let rbc_flag =
            get_nested_value(rbc_flag, &["da", "enabled"]).and_then(toml::Value::as_bool);
        assert_eq!(
            rbc_flag,
            Some(true),
            "base config should enable DA by default"
        );
    }

    #[test]
    fn base_config_increases_block_queue_capacity() {
        let _guard = lock_env_guard(&CONFIG_ENV_GUARD);
        let network = NetworkBuilder::new().build();

        let mut layers = network.config_layers();
        let _trusted = layers.next().expect("trusted peers layer");
        let base = layers.next().expect("base config layer").into_owned();
        let cap = base
            .get("sumeragi")
            .and_then(TomlValue::as_table)
            .and_then(|table| get_nested_value(table, &["advanced", "queues", "blocks"]))
            .and_then(TomlValue::as_integer);
        assert_eq!(
            cap,
            Some(512),
            "test network should raise block queue capacity to avoid dropped sync updates"
        );
    }

    #[test]
    fn default_network_sets_localnet_rbc_chunk_max_bytes() {
        let _guard = lock_env_guard(&SUMERAGI_ENV_GUARD);
        let _disable_da = disable_sumeragi_env_overrides();
        let network = NetworkBuilder::new().build();

        let mut layers = network.config_layers();
        let _trusted = layers.next().expect("trusted peers layer");
        let base = layers.next().expect("base config layer").into_owned();
        let rbc_chunk_max = base
            .get("sumeragi")
            .and_then(toml::Value::as_table)
            .and_then(|sumeragi| {
                get_nested_value(sumeragi, &["advanced", "rbc", "chunk_max_bytes"])
            })
            .and_then(toml::Value::as_integer);

        assert_eq!(
            rbc_chunk_max,
            Some(LOCALNET_RBC_CHUNK_MAX_BYTES),
            "base config should set a larger localnet RBC chunk size"
        );
    }

    #[tokio::test]
    async fn can_start_networks() {
        if skip_network_tests("can_start_networks") {
            return;
        }
        let (first_builder, second_builder) = {
            let _sumeragi_guard = lock_env_guard_async(&SUMERAGI_ENV_GUARD).await;
            let _disable_da = disable_sumeragi_env_overrides();
            (
                NetworkBuilder::new().with_peers(4),
                NetworkBuilder::new().with_peers(4),
            )
        };
        {
            let _program_guard = lock_env_guard_async(&PROGRAM_BIN_ENV_GUARD).await;
            tokio::time::timeout(
                Duration::from_secs(20 * 60),
                Program::Irohad.resolve_async(),
            )
            .await
            .expect("irohad binary resolution should not hang")
            .expect("irohad binary should resolve for network startup tests");
        }
        let first = build_with_isolated_permit_async(first_builder).await;
        let first_timeout = first
            .peer_startup_timeout()
            .saturating_add(Duration::from_secs(30));
        tokio::time::timeout(first_timeout, first.start_all())
            .await
            .expect("first network startup should complete within timeout")
            .unwrap();
        tokio::time::timeout(first_timeout, async {
            first.shutdown().await;
        })
        .await
        .expect("first network shutdown should complete within timeout");
        drop(first);
        // Single-peer DA startup is still stall-prone in integration paths; keep this
        // smoke test on quorum-representative topologies to avoid lock convoy hangs.
        let second = build_with_isolated_permit_async(second_builder).await;
        let second_timeout = second
            .peer_startup_timeout()
            .saturating_add(Duration::from_secs(30));
        tokio::time::timeout(second_timeout, second.start_all())
            .await
            .expect("second network startup should complete within timeout")
            .unwrap();
        tokio::time::timeout(second_timeout, async {
            second.shutdown().await;
        })
        .await
        .expect("second network shutdown should complete within timeout");
    }

    #[tokio::test]
    async fn start_fails_with_missing_binary() {
        if skip_network_tests("start_fails_with_missing_binary") {
            return;
        }
        let network = {
            let _sumeragi_guard = lock_env_guard_async(&SUMERAGI_ENV_GUARD).await;
            let _disable_da = disable_sumeragi_env_overrides();
            build_with_isolated_permit_async(NetworkBuilder::new()).await
        };
        let _program_guard = lock_env_guard_async(&PROGRAM_BIN_ENV_GUARD).await;
        const ENV: &str = PROGRAM_IROHAD_ENV;
        let old = std::env::var(ENV).ok();
        set_env_var(ENV, "non-existent-path");
        let res = tokio::time::timeout(Duration::from_secs(10), network.start_all())
            .await
            .expect("missing binary should fail startup quickly");
        assert!(res.is_err());
        if let Some(val) = old {
            set_env_var(ENV, val);
        } else {
            remove_env_var(ENV);
        }
    }

    #[tokio::test]
    async fn starts_single_peer_with_minimal_genesis_fallback() {
        if skip_network_tests("starts_single_peer_with_minimal_genesis_fallback") {
            return;
        }
        let builder = {
            let _sumeragi_guard = lock_env_guard_async(&SUMERAGI_ENV_GUARD).await;
            let _disable_da = disable_sumeragi_env_overrides();
            // Single-peer DA startup is still stall-prone in test environments.
            // Use a quorum-representative topology while preserving fallback-genesis coverage.
            NetworkBuilder::new().with_peers(4)
        };
        {
            let _program_guard = lock_env_guard_async(&PROGRAM_BIN_ENV_GUARD).await;
            tokio::time::timeout(
                Duration::from_secs(20 * 60),
                Program::Irohad.resolve_async(),
            )
            .await
            .expect("irohad binary resolution should not hang")
            .expect("irohad binary should resolve for fallback startup test");
        }
        // Intentionally avoid providing a default executor sample; in CI the
        // prebuilt samples are usually absent so JSON genesis will fail to
        // locate `defaults/executor.to` and the harness will fall back to a
        // minimal in-memory genesis. This test ensures that even with fallback
        // the peer starts and commits the genesis block.
        remove_env_var("IROHA_TEST_SKIP_BUILD"); // allow building if needed
        let network = build_with_isolated_permit_async(builder).await;
        let net = tokio::time::timeout(Duration::from_secs(90), network.start_all())
            .await
            .expect("fallback startup should complete within timeout");
        assert!(net.is_ok(), "network should start with fallback genesis");
    }

    #[test]
    fn ivm_fuel_config_defaults_to_unset() {
        assert!(matches!(IvmFuelConfig::default(), IvmFuelConfig::Unset));
    }

    #[test]
    fn default_builder_injects_da_enabled_param() {
        let _guard = lock_env_guard(&SUMERAGI_ENV_GUARD);
        let _disable_da = disable_sumeragi_env_overrides();
        let network = NetworkBuilder::new().build();
        let isi = network.genesis_isi();
        let has_da_enabled_set_parameter = isi.iter().flatten().any(|instruction| {
            instruction
                .as_any()
                .downcast_ref::<SetParameter>()
                .is_some_and(|set_param| {
                    matches!(
                        set_param.inner(),
                        Parameter::Sumeragi(SumeragiParameter::DaEnabled(true))
                    )
                })
        });
        assert!(
            has_da_enabled_set_parameter,
            "default builder must inject DA enablement parameter"
        );
    }

    #[test]
    fn npos_bootstrap_adds_validator_instructions() {
        init_instruction_registry();
        let stake_amount = SumeragiNposParameters::default().min_self_bond();
        let network = NetworkBuilder::new()
            .with_peers(2)
            .with_auto_populated_trusted_peers()
            .with_npos_genesis_bootstrap(stake_amount)
            .with_config_layer(|layer| {
                layer.write(["sumeragi", "consensus_mode"], "npos");
            })
            .build();
        let genesis = network.genesis();
        let mut has_register = false;
        let mut has_activate = false;
        for tx in genesis.0.transactions_vec() {
            if let Executable::Instructions(instructions) = tx.instructions() {
                for instruction in instructions {
                    if instruction
                        .as_any()
                        .downcast_ref::<RegisterPublicLaneValidator>()
                        .is_some()
                    {
                        has_register = true;
                    }
                    if instruction
                        .as_any()
                        .downcast_ref::<ActivatePublicLaneValidator>()
                        .is_some()
                    {
                        has_activate = true;
                    }
                }
            }
        }
        assert!(
            has_register && has_activate,
            "npos bootstrap should register and activate validators in genesis"
        );
    }

    #[test]
    fn default_npos_builder_bootstraps_validators() {
        init_instruction_registry();
        let network = NetworkBuilder::new()
            .with_peers(2)
            .with_auto_populated_trusted_peers()
            .with_config_layer(|layer| {
                layer.write(["sumeragi", "consensus_mode"], "npos");
            })
            .build();
        let genesis = network.genesis();
        let mut has_register = false;
        let mut has_activate = false;
        for tx in genesis.0.transactions_vec() {
            if let Executable::Instructions(instructions) = tx.instructions() {
                for instruction in instructions {
                    if instruction
                        .as_any()
                        .downcast_ref::<RegisterPublicLaneValidator>()
                        .is_some()
                    {
                        has_register = true;
                    }
                    if instruction
                        .as_any()
                        .downcast_ref::<ActivatePublicLaneValidator>()
                        .is_some()
                    {
                        has_activate = true;
                    }
                }
            }
        }
        assert!(
            has_register && has_activate,
            "default NPoS builder should bootstrap validators in genesis"
        );
    }

    #[test]
    fn without_npos_genesis_bootstrap_skips_validator_instructions() {
        init_instruction_registry();
        let network = build_with_isolated_permit(
            NetworkBuilder::new()
                .with_peers(2)
                .with_auto_populated_trusted_peers()
                .with_config_layer(|layer| {
                    layer.write(["sumeragi", "consensus_mode"], "npos");
                })
                .without_npos_genesis_bootstrap(),
        );
        let genesis = network.genesis();
        let mut has_register = false;
        let mut has_activate = false;
        for tx in genesis.0.transactions_vec() {
            if let Executable::Instructions(instructions) = tx.instructions() {
                for instruction in instructions {
                    if instruction
                        .as_any()
                        .downcast_ref::<RegisterPublicLaneValidator>()
                        .is_some()
                    {
                        has_register = true;
                    }
                    if instruction
                        .as_any()
                        .downcast_ref::<ActivatePublicLaneValidator>()
                        .is_some()
                    {
                        has_activate = true;
                    }
                }
            }
        }
        assert!(
            !has_register && !has_activate,
            "disabling NPoS bootstrap should not inject validator registration"
        );
    }

    #[test]
    fn post_topology_instructions_are_included_in_genesis() {
        init_instruction_registry();
        let domain_id: DomainId = "post_topology_test".parse().expect("domain");
        let network = build_with_isolated_permit(
            NetworkBuilder::new()
                .with_peers(1)
                .with_genesis_post_topology_isi(vec![
                    Register::domain(Domain::new(domain_id.clone())).into(),
                ]),
        );
        let genesis = network.genesis();
        let mut has_domain = false;
        for tx in genesis.0.transactions_vec() {
            if let Executable::Instructions(instructions) = tx.instructions() {
                for instruction in instructions {
                    if instruction
                        .as_any()
                        .downcast_ref::<RegisterBox>()
                        .is_some_and(|register| match register {
                            RegisterBox::Domain(domain) => domain.object.id == domain_id,
                            _ => false,
                        })
                    {
                        has_domain = true;
                    }
                }
            }
        }
        assert!(
            has_domain,
            "post-topology instructions should be present in genesis"
        );
    }

    #[test]
    fn npos_bootstrap_clamps_to_min_self_bond() {
        init_instruction_registry();
        let mut npos_params = SumeragiNposParameters::default();
        npos_params.min_self_bond = npos_params.min_self_bond().saturating_add(5_000);
        let expected = npos_params.min_self_bond;
        let network = build_with_isolated_permit(
            NetworkBuilder::new()
                .with_peers(1)
                .with_auto_populated_trusted_peers()
                .with_genesis_instruction(SetParameter::new(Parameter::Custom(
                    npos_params.into_custom_parameter(),
                )))
                .with_config_layer(|layer| {
                    layer.write(["sumeragi", "consensus_mode"], "npos");
                }),
        );
        let genesis = network.genesis();
        let mut seen = false;
        for tx in genesis.0.transactions_vec() {
            if let Executable::Instructions(instructions) = tx.instructions() {
                for instruction in instructions {
                    if let Some(register) = instruction
                        .as_any()
                        .downcast_ref::<RegisterPublicLaneValidator>()
                    {
                        seen = true;
                        assert_eq!(
                            register.initial_stake,
                            Numeric::from(expected),
                            "bootstrap stake must honor min_self_bond"
                        );
                    }
                }
            }
        }
        assert!(seen, "expected bootstrap validator registration in genesis");
    }

    #[test]
    fn npos_bootstrap_overrides_stake_accounts_in_config() {
        let stake_amount = SumeragiNposParameters::default().min_self_bond();
        let network = NetworkBuilder::new()
            .with_peers(1)
            .with_auto_populated_trusted_peers()
            .with_npos_genesis_bootstrap(stake_amount)
            .with_config_layer(|layer| {
                layer.write(["sumeragi", "consensus_mode"], "npos");
            })
            .build();

        let mut merged = Table::new();
        for layer in network.config_layers() {
            merge_tables(&mut merged, layer.as_ref());
        }

        let stake_escrow =
            get_nested_value(&merged, &["nexus", "staking", "stake_escrow_account_id"])
                .and_then(Value::as_str)
                .expect("stake_escrow_account_id should be present");
        let slash_sink = get_nested_value(&merged, &["nexus", "staking", "slash_sink_account_id"])
            .and_then(Value::as_str)
            .expect("slash_sink_account_id should be present");

        assert!(
            stake_escrow.parse::<AccountId>().is_ok(),
            "stake_escrow_account_id must parse as AccountId; got {stake_escrow}"
        );
        assert!(
            slash_sink.parse::<AccountId>().is_ok(),
            "slash_sink_account_id must parse as AccountId; got {slash_sink}"
        );
    }

    #[test]
    fn default_builder_uses_localnet_pipeline_time() {
        let network = NetworkBuilder::new().build();

        assert_eq!(network.pipeline_time(), LOCALNET_PIPELINE_TIME);

        let mut block_time_ms = None;
        let mut commit_time_ms = None;
        let first_tx = network
            .genesis_isi()
            .first()
            .expect("at least one transaction with parameters");
        for instruction in first_tx {
            if let Some(set_param) = instruction.as_any().downcast_ref::<SetParameter>()
                && let iroha_data_model::parameter::Parameter::Sumeragi(sumeragi) =
                    set_param.inner()
            {
                match sumeragi {
                    SumeragiParameter::BlockTimeMs(value) => block_time_ms = Some(*value),
                    SumeragiParameter::CommitTimeMs(value) => commit_time_ms = Some(*value),
                    _ => {}
                }
            }
        }

        let total_ms = LOCALNET_PIPELINE_TIME.as_millis() as u64;
        let expected_block = total_ms / 3;
        let expected_commit = total_ms - expected_block;

        assert_eq!(block_time_ms, Some(expected_block));
        assert_eq!(commit_time_ms, Some(expected_commit));
    }

    #[test]
    fn default_pipeline_time_exceeds_three_seconds() {
        let network = NetworkBuilder::new().with_default_pipeline_time().build();
        assert_eq!(network.pipeline_time(), DEFAULT_PIPELINE_TIME);
        assert!(
            network.pipeline_time() > Duration::from_secs(3),
            "default pipeline time should exceed three seconds"
        );
    }

    #[test]
    fn explicit_pipeline_time_injects_sumeragi_params() {
        let duration = Duration::from_secs(9);
        let network = NetworkBuilder::new().with_pipeline_time(duration).build();

        assert_eq!(network.pipeline_time(), duration);

        let mut block_time_ms = None;
        let mut commit_time_ms = None;
        let first_tx = network
            .genesis_isi()
            .first()
            .expect("at least one transaction with parameters");
        for instruction in first_tx {
            if let Some(set_param) = instruction.as_any().downcast_ref::<SetParameter>()
                && let iroha_data_model::parameter::Parameter::Sumeragi(sumeragi) =
                    set_param.inner()
            {
                match sumeragi {
                    SumeragiParameter::BlockTimeMs(value) => block_time_ms = Some(*value),
                    SumeragiParameter::CommitTimeMs(value) => commit_time_ms = Some(*value),
                    _ => {}
                }
            }
        }

        assert_eq!(block_time_ms, Some(3_000));
        assert_eq!(commit_time_ms, Some(6_000));
    }

    #[test]
    fn pipeline_time_rounding_preserves_total_duration() {
        if skip_network_tests("pipeline_time_rounding_preserves_total_duration") {
            return;
        }
        let duration = Duration::from_millis(9_500);
        let network =
            build_with_isolated_permit(NetworkBuilder::new().with_pipeline_time(duration));

        assert_eq!(network.pipeline_time(), duration);

        let expected_block = 9_500 / 3; // integer division -> 3166
        let expected_commit = 9_500 - expected_block;

        let mut block_time_ms = None;
        let mut commit_time_ms = None;
        let first_tx = network
            .genesis_isi()
            .first()
            .expect("at least one transaction with parameters");
        for instruction in first_tx {
            if let Some(set_param) = instruction.as_any().downcast_ref::<SetParameter>()
                && let iroha_data_model::parameter::Parameter::Sumeragi(sumeragi) =
                    set_param.inner()
            {
                match sumeragi {
                    SumeragiParameter::BlockTimeMs(value) => block_time_ms = Some(*value),
                    SumeragiParameter::CommitTimeMs(value) => commit_time_ms = Some(*value),
                    _ => {}
                }
            }
        }

        assert_eq!(block_time_ms, Some(expected_block));
        assert_eq!(commit_time_ms, Some(expected_commit));
        let total_ms = duration.as_millis() as u64;
        assert_eq!(expected_block + expected_commit, total_ms);
    }

    #[test]
    fn data_availability_parameter_is_injected() {
        let network = NetworkBuilder::new()
            .with_peers(2)
            .with_data_availability_enabled(true)
            .build();

        let mut saw_da_enabled = None;
        let first_tx = network
            .genesis_isi()
            .first()
            .expect("genesis must contain at least one transaction");

        for instruction in first_tx {
            if let Some(set_param) = instruction.as_any().downcast_ref::<SetParameter>()
                && let iroha_data_model::parameter::Parameter::Sumeragi(sumeragi) =
                    set_param.inner()
                && let SumeragiParameter::DaEnabled(value) = sumeragi
            {
                saw_da_enabled = Some(*value);
            }
        }

        assert_eq!(saw_da_enabled, Some(true));

        let mut layers = network.config_layers();
        let _trusted = layers.next().expect("trusted peers layer present");
        let base_layer = layers
            .next()
            .expect("base config layer present")
            .into_owned();

        let sumeragi = base_layer
            .get("sumeragi")
            .and_then(|value| value.as_table())
            .expect("sumeragi table present");
        assert_eq!(
            get_nested_value(sumeragi, &["da", "enabled"]).and_then(|value| value.as_bool()),
            Some(true)
        );
        assert_eq!(
            get_nested_value(sumeragi, &["advanced", "rbc", "store_max_sessions"])
                .and_then(|value| value.as_integer()),
            Some(DEFAULT_RBC_STORE_MAX_SESSIONS)
        );
    }

    #[test]
    fn block_sync_gossip_period_override_is_applied() {
        let period = Duration::from_millis(750);
        let network = NetworkBuilder::new()
            .with_block_sync_gossip_period(period)
            .build();

        let mut layers = network.config_layers();
        let _trusted = layers.next().expect("trusted peers layer present");
        let base_layer = layers
            .next()
            .expect("base config layer present")
            .into_owned();

        let network_section = base_layer
            .get("network")
            .and_then(|value| value.as_table())
            .expect("network table present");
        let period_value = network_section
            .get("block_gossip_period_ms")
            .and_then(|value| value.as_integer())
            .expect("block gossip period as integer");

        let expected = i64::try_from(period.as_millis()).expect("fits in i64");
        assert_eq!(period_value, expected);
    }

    #[test]
    fn builder_sets_ivm_fuel() {
        let builder = NetworkBuilder::new().with_ivm_fuel(IvmFuelConfig::Unset);
        assert!(matches!(builder.ivm_fuel, IvmFuelConfig::Unset));
    }

    #[test]
    fn peer_builder_mnemonic_has_no_whitespace() {
        let builder = NetworkPeerBuilder::new();
        assert!(builder.mnemonic.chars().all(|c| !c.is_whitespace()));
    }

    #[test]
    fn peer_id_uses_bls() {
        let env = Environment::new();
        let peer = NetworkPeerBuilder::new().build(&env);
        assert_eq!(peer.id().public_key().algorithm(), Algorithm::BlsNormal);
        assert_eq!(
            peer.streaming_public_key().algorithm(),
            Algorithm::Ed25519,
            "streaming identity should remain Ed25519 even with BLS peers"
        );
        assert!(
            peer.bls_public_key().is_some(),
            "expected BLS key material to remain available"
        );
    }

    #[test]
    fn base_config_sets_streaming_identity_keys() {
        let env = Environment::new();
        let peer = NetworkPeerBuilder::new().build(&env);
        let path = peer.dir.join("config.base.toml");
        let contents = std::fs::read_to_string(&path).expect("read base config");
        let table: toml::Table = toml::from_str(&contents).expect("parse base config");
        let streaming = table
            .get("streaming")
            .and_then(toml::Value::as_table)
            .expect("streaming table present");
        let identity_public = streaming
            .get("identity_public_key")
            .and_then(toml::Value::as_str)
            .expect("identity public key string");
        let parsed: iroha_crypto::PublicKey = identity_public.parse().expect("identity key parses");
        assert_eq!(
            parsed.algorithm(),
            Algorithm::Ed25519,
            "streaming identity must use Ed25519"
        );
        let identity_private = streaming
            .get("identity_private_key")
            .and_then(toml::Value::as_str)
            .expect("identity private key string");
        assert!(
            identity_private.starts_with("8026"),
            "private key should be hex-like multihash"
        );
    }

    #[test]
    fn uses_shared_instruction_registry() {
        init_instruction_registry();

        let instruction =
            RegisterBox::Domain(Register::domain(Domain::new("test".parse().unwrap())));
        let instruction_box: InstructionBox = instruction.into();
        let bytes = norito::to_bytes(&instruction_box).expect("encode");
        let decoded: InstructionBox = norito::decode_from_bytes(&bytes).expect("decode");
        assert_eq!(decoded, instruction_box);
    }

    #[test]
    fn program_resolve_uses_env_override_without_build() {
        let _guard = lock_env_guard(&PROGRAM_BIN_ENV_GUARD);
        // Point TEST_NETWORK_BIN_IROHA to a dummy file under repo root
        let repo = repo_root();
        let rel = PathBuf::from("target/test-bin-dummy/iroha-cli-dummy");
        let abs = repo.join(&rel);
        std::fs::create_dir_all(abs.parent().unwrap()).unwrap();
        std::fs::write(&abs, b"dummy").unwrap();

        let old_env = env::var(super::PROGRAM_IROHA_ENV).ok();
        set_env_var(super::PROGRAM_IROHA_ENV, rel.display().to_string());

        // Should resolve to the dummy file via env override
        let resolved = Program::Iroha
            .resolve_skip_build()
            .expect("resolve via env");
        assert_eq!(resolved, abs.canonicalize().unwrap());

        // Cleanup and restore environment
        if let Some(v) = old_env {
            set_env_var(super::PROGRAM_IROHA_ENV, v);
        } else {
            remove_env_var(super::PROGRAM_IROHA_ENV);
        }
        // Do not remove the dummy file to avoid races if other tests concurrently resolve;
        // it's under target/ and harmless.
    }

    #[tokio::test]
    async fn program_resolve_async_honors_env_override() {
        let _guard = lock_env_guard_async(&PROGRAM_BIN_ENV_GUARD).await;
        let repo = repo_root();
        let rel = PathBuf::from("target/test-bin-dummy/iroha-cli-dummy-async");
        let abs = repo.join(&rel);
        std::fs::create_dir_all(abs.parent().unwrap()).unwrap();
        std::fs::write(&abs, b"dummy").unwrap();

        let old_env = env::var(super::PROGRAM_IROHA_ENV).ok();
        set_env_var(super::PROGRAM_IROHA_ENV, rel.display().to_string());

        let resolved = Program::Iroha
            .resolve_async()
            .await
            .expect("resolve via env");
        assert_eq!(resolved, abs.canonicalize().unwrap());

        if let Some(v) = old_env {
            set_env_var(super::PROGRAM_IROHA_ENV, v);
        } else {
            remove_env_var(super::PROGRAM_IROHA_ENV);
        }
    }

    #[test]
    fn program_spec_irohad_uses_default_features() {
        let spec = Program::Irohad.spec();
        let args: Vec<String> = spec
            .build_args
            .iter()
            .map(|arg| arg.to_string_lossy().to_string())
            .collect();
        assert!(args.contains(&"--bin".to_string()));
        assert!(args.contains(&"iroha3d".to_string()));
        assert!(!args.contains(&"--features".to_string()));
    }

    fn build_with_isolated_permit(builder: NetworkBuilder) -> Network {
        let _guard = lock_env_guard(&NETWORK_PERMIT_ENV_GUARD);
        let dir = tempdir().expect("permit dir");
        let _dir_guard = EnvVarRestore::set(NETWORK_PERMIT_DIR_ENV, dir.path());
        let _parallel_guard = EnvVarRestore::set(NETWORK_PARALLELISM_ENV, "1");
        let _serialize_guard = EnvVarRestore::set(SERIALIZE_NETWORKS_ENV, "0");
        builder.build()
    }

    async fn build_with_isolated_permit_async(builder: NetworkBuilder) -> Network {
        let _guard = lock_env_guard_async(&NETWORK_PERMIT_ENV_GUARD).await;
        let dir = tempdir().expect("permit dir");
        let _dir_guard = EnvVarRestore::set(NETWORK_PERMIT_DIR_ENV, dir.path());
        let _parallel_guard = EnvVarRestore::set(NETWORK_PARALLELISM_ENV, "1");
        let _serialize_guard = EnvVarRestore::set(SERIALIZE_NETWORKS_ENV, "0");
        builder.build()
    }

    #[test]
    fn torii_url_uses_api_port() {
        let network = build_with_isolated_permit(NetworkBuilder::new());
        let peer = network.peer();
        let url = peer.torii_url();
        assert!(url.starts_with("http://127.0.0.1:"));
        let port_str = url.rsplit(':').next().expect("url has a port");
        let port: u16 = port_str.parse().expect("port is u16");
        assert_eq!(port, peer.api_address().port());
    }

    #[test]
    fn network_torii_urls_match_peers() {
        let network = build_with_isolated_permit(NetworkBuilder::new().with_peers(3));
        let urls = network.torii_urls();
        assert_eq!(urls.len(), network.peers().len());
        for (peer, url) in network.peers().iter().zip(urls.iter()) {
            assert!(url.starts_with("http://127.0.0.1:"));
            let port_str = url.rsplit(':').next().unwrap();
            let port: u16 = port_str.parse().unwrap();
            assert_eq!(port, peer.api_address().port());
        }
    }

    #[test]
    fn network_client_uses_first_peer() {
        let network = build_with_isolated_permit(NetworkBuilder::new().with_peers(3));
        let expected = network
            .peers()
            .first()
            .expect("network has peers")
            .api_address();
        let client = network.client();
        let expected_host = expected.host_str();
        assert_eq!(client.torii_url.host_str(), Some(expected_host.as_ref()));
        assert_eq!(
            client.torii_url.port_or_known_default(),
            Some(expected.port())
        );
    }

    #[test]
    fn http_start_gate_requires_http_source() {
        let mut gate = HttpStartGate::default();
        assert!(!gate.http_seen(), "gate starts without HTTP observations");
        assert!(!gate.on_status(StatusSource::Storage));
        assert!(!gate.http_seen(), "storage status must not flip readiness");
        assert!(gate.on_status(StatusSource::Http));
        assert!(gate.http_seen(), "first HTTP status should trip readiness");
        assert!(
            !gate.on_status(StatusSource::Http),
            "subsequent HTTP statuses should not retrigger"
        );
    }

    #[test]
    fn genesis_is_cached_and_deterministic() {
        // Repeated calls to `Network::genesis()` must return the exact same block
        // so that multiple peers submitting genesis use identical bytes.
        let network = NetworkBuilder::new().with_peers(4).build();
        let g1 = network.genesis();
        let g2 = network.genesis();

        // Compare encoded bytes to be strict about byte-for-byte equality
        let b1 = g1.0.encode_versioned();
        let b2 = g2.0.encode_versioned();
        assert_eq!(b1, b2, "genesis must be identical across calls");

        let f1 = g1.0.encode_wire().expect("encode genesis wire");
        let f2 = g2.0.encode_wire().expect("encode genesis wire");
        assert_eq!(f1, f2, "framed genesis must be identical across calls");
    }

    #[test]
    fn genesis_roundtrip_decodes() {
        init_instruction_registry();

        let network = NetworkBuilder::new().build();
        let block = network.genesis();

        let versioned = block.0.encode_versioned();
        let framed = block.0.encode_wire().expect("encode versioned genesis");
        if let Ok(dump_path) = env::var("IROHA_TEST_DUMP_GENESIS") {
            let dump_path = std::path::PathBuf::from(dump_path);
            if let Some(parent) = dump_path.parent() {
                std::fs::create_dir_all(parent).expect("create dump directory");
            }
            std::fs::write(&dump_path, &framed).expect("write genesis dump");
        }
        assert!(
            !framed.is_empty(),
            "versioned encoding includes at least a version byte"
        );

        let (_, payload) = framed
            .split_first()
            .expect("versioned payload has a prefix");
        assert!(
            payload.starts_with(norito::core::MAGIC.as_slice()),
            "payload must start with Norito magic header"
        );

        let header_index = 1 + norito::core::Header::SIZE - 1;
        assert_eq!(
            framed[header_index],
            norito::core::default_encode_flags(),
            "framed genesis must use canonical header flags",
        );
        let deframed =
            deframe_versioned_signed_block_bytes(&framed).expect("deframe framed genesis");
        assert_eq!(deframed.bytes.as_ref(), framed.as_slice());
        assert_eq!(deframed.bare_versioned.as_ref(), versioned.as_slice());

        let decoded =
            decode_versioned_signed_block(framed.as_slice()).expect("decode framed genesis");
        assert_eq!(
            decoded.version(),
            1,
            "decoded genesis must be a version 1 signed block"
        );
    }

    #[test]
    fn with_genesis_block_uses_custom_builder() {
        init_instruction_registry();

        let seen_topology: Arc<Mutex<Option<UniqueVec<PeerId>>>> = Arc::new(Mutex::new(None));
        let seen_pops: Arc<Mutex<Option<Vec<GenesisTopologyEntry>>>> = Arc::new(Mutex::new(None));
        let callback_topology = Arc::clone(&seen_topology);
        let callback_pops = Arc::clone(&seen_pops);

        let network =
            build_with_isolated_permit(NetworkBuilder::new().with_peers(2).with_genesis_block(
                move |topology, pops| {
                    *callback_topology
                        .lock()
                        .expect("callback topology mutex poisoned") = Some(topology.clone());
                    *callback_pops.lock().expect("callback pop mutex poisoned") =
                        Some(pops.clone());
                    genesis_factory(Vec::new(), topology, pops)
                },
            ));

        let produced = network.genesis();
        let recorded = seen_topology
            .lock()
            .expect("topology mutex poisoned")
            .clone()
            .expect("topology should be recorded");
        let recorded_pops = seen_pops
            .lock()
            .expect("pop mutex poisoned")
            .clone()
            .expect("topology pops should be recorded");
        let expected = genesis_factory(Vec::new(), recorded, recorded_pops);

        assert_eq!(
            produced.0.encode_versioned(),
            expected.0.encode_versioned(),
            "custom genesis builder should dictate the resulting block"
        );
    }
}
