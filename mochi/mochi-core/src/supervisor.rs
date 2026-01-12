//! Process supervision primitives for managing local `irohad` peers.
//!
//! The supervisor prepares filesystem layouts, generates a Kagami-aligned
//! default genesis manifest, and can launch or stop child `irohad` processes.

use std::{
    collections::{BTreeMap, HashMap, HashSet},
    env,
    ffi::{OsStr, OsString},
    fs::{self, OpenOptions},
    io::{self, BufRead, BufReader, Read, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    str::FromStr,
    sync::{Arc, Mutex},
    thread::{self, JoinHandle},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use iroha_crypto::{Algorithm, ExposedPrivateKey, Hash, KeyPair, PublicKey, bls_normal_pop_prove};
use iroha_data_model::{
    parameter::system::SumeragiConsensusMode,
    peer::PeerId,
    prelude::{AccountId, ChainId, DomainId},
};
use iroha_genesis::{GenesisTopologyEntry, RawGenesisTransaction};
use iroha_version::build_line::BuildLine;
use norito::json::{self, Map, Value};
use once_cell::sync::OnceCell;
use tokio::runtime::Handle;

use crate::{
    compose::SigningAuthority,
    config::{GenesisProfile, NetworkPaths, NetworkProfile, PortAllocator, ProfilePreset},
    genesis,
    logs::{LifecycleEvent, LogStreamKind, PeerLogStream},
    torii::{ManagedBlockStream, ManagedEventStream, ReadinessSmokePlan, ToriiClient, ToriiResult},
    vault::{SignerVault, SignerVaultError},
};

const DEFAULT_CHAIN_ID: &str = "mochi-local";
const DEFAULT_TORII_BASE_PORT: u16 = 8080;
const DEFAULT_P2P_BASE_PORT: u16 = 1337;
const GENESIS_FILE_NAME: &str = "genesis.json";
const SMOKE_ACCOUNT_DOMAIN: &str = "wonderland";
const SMOKE_MAX_ATTEMPTS: usize = 3;
fn timestamp_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_millis()
}

fn encode_hex(bytes: &[u8]) -> String {
    const TABLE: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(TABLE[(byte >> 4) as usize] as char);
        out.push(TABLE[(byte & 0x0F) as usize] as char);
    }
    out
}

/// Result alias for supervisor operations.
pub type Result<T> = std::result::Result<T, SupervisorError>;

/// Errors emitted while preparing or managing the supervisor.
#[derive(Debug, thiserror::Error)]
pub enum SupervisorError {
    /// Wrapper for I/O failures.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    /// Wrapper for TOML serialization failures.
    #[error("failed to render peer config: {0}")]
    Toml(#[from] toml::ser::Error),
    /// JSON serialization failures for genesis manifests.
    #[error("failed to serialize genesis manifest: {0}")]
    Norito(#[from] norito::json::Error),
    /// Genesis construction failures.
    #[error("failed to build genesis manifest: {0}")]
    Genesis(#[from] color_eyre::Report),
    /// External Kagami invocation failed while preparing genesis.
    #[error("failed to generate genesis manifest via `kagami`: {0}")]
    KagamiInvocation(String),
    /// Validation of a Kagami verify report failed.
    #[error("kagami verify rejected: {0}")]
    KagamiVerify(String),
    /// Failed to probe a binary version string.
    #[error("failed to probe `{binary}` version: {message}")]
    VersionProbeFailed { binary: String, message: String },
    /// Binary build line does not match the expected profile.
    #[error("binary build-line mismatch for `{binary}`: expected {expected} but detected {found}")]
    BuildLineMismatch {
        binary: String,
        expected: BuildLine,
        found: BuildLine,
    },
    /// Attempted to start an already running peer.
    #[error("peer `{alias}` already running")]
    PeerAlreadyRunning { alias: String },
    /// Attempted to stop a peer that is not running.
    #[error("peer `{alias}` not running")]
    PeerNotRunning { alias: String },
    /// Referenced peer alias is not part of the supervised topology.
    #[error("peer `{alias}` not found")]
    PeerUnknown { alias: String },
    /// Attempted to mutate storage for a peer that is still running.
    #[error("cannot modify storage for running peer `{alias}`")]
    PeerStillRunning { alias: String },
    /// Requested snapshot already exists.
    #[error("snapshot `{name}` already exists under `{root}`")]
    SnapshotExists { name: String, root: PathBuf },
    /// Required executable could not be located or auto-built.
    #[error("failed to locate `{binary}`: {message}")]
    BinaryUnavailable {
        binary: &'static str,
        message: String,
    },
    /// Failed to spawn the peer process.
    #[error("failed to spawn `{alias}`: {source}")]
    Spawn {
        alias: String,
        #[source]
        source: std::io::Error,
    },
    /// Failed to terminate the peer process.
    #[error("failed to terminate `{alias}`: {source}")]
    Terminate {
        alias: String,
        #[source]
        source: std::io::Error,
    },
    /// Failed to wait on a terminated peer process.
    #[error("failed to collect exit status for `{alias}`: {source}")]
    Wait {
        alias: String,
        #[source]
        source: std::io::Error,
    },
    /// Invalid configuration detected while loading supervisor artifacts.
    #[error("invalid configuration: {0}")]
    Config(String),
}

impl From<SignerVaultError> for SupervisorError {
    fn from(err: SignerVaultError) -> Self {
        match err {
            SignerVaultError::Io(err) => Self::Io(err),
            SignerVaultError::Json(err) => Self::Norito(err),
            SignerVaultError::InvalidEntry(message) => Self::Config(message),
        }
    }
}

/// Policy governing automatic restarts for managed peers.
#[derive(Debug, Clone, Copy)]
pub enum RestartPolicy {
    /// Never restart automatically.
    Never,
    /// Restart after failures up to a maximum number of attempts using an exponential backoff.
    OnFailure {
        /// Maximum number of restart attempts (1-based).
        max_restarts: usize,
        /// Base backoff applied to the first restart attempt.
        backoff: Duration,
    },
}

impl RestartPolicy {
    /// Determine whether another restart attempt is permitted.
    fn should_retry(self, attempt: usize) -> bool {
        if attempt == 0 {
            return false;
        }
        match self {
            RestartPolicy::Never => false,
            RestartPolicy::OnFailure { max_restarts, .. } => attempt <= max_restarts,
        }
    }

    /// Compute the backoff for the provided attempt (1-based).
    fn backoff_for(self, attempt: usize) -> Duration {
        if attempt == 0 {
            return Duration::ZERO;
        }
        match self {
            RestartPolicy::Never => Duration::ZERO,
            RestartPolicy::OnFailure { backoff, .. } => {
                if attempt <= 1 {
                    backoff
                } else {
                    let exponent = ((attempt - 1) as u32).min(4);
                    backoff.saturating_mul(1 << exponent)
                }
            }
        }
    }
}

impl Default for RestartPolicy {
    fn default() -> Self {
        RestartPolicy::OnFailure {
            max_restarts: 3,
            backoff: Duration::from_secs(1),
        }
    }
}

/// Describes where a binary path originated so UI layers can surface actionable hints.
#[derive(Debug, Clone, PartialEq, Eq)]
enum BinarySource {
    /// Explicit override supplied via config/CLI/env.
    Explicit,
    /// Discovered via a specific environment variable.
    Env(&'static str),
    /// Discovered via a Cargo-set binary env var.
    CargoEnv(&'static str),
    /// Located next to the currently running executable.
    CurrentExeNeighbor,
    /// Located under the workspace target directory (profile captured for context).
    WorkspaceTarget(String),
    /// Found on the system PATH.
    PathSearch,
    /// Default unresolved placeholder.
    AutoDefault,
    /// Produced by an on-demand cargo build.
    Built,
}

impl BinarySource {
    fn label(&self) -> String {
        match self {
            Self::Explicit => "explicit override".to_owned(),
            Self::Env(var) => format!("{var} environment variable"),
            Self::CargoEnv(var) => format!("{var} cargo env"),
            Self::CurrentExeNeighbor => "bundle directory".to_owned(),
            Self::WorkspaceTarget(profile) => format!("workspace target/{profile}"),
            Self::PathSearch => "PATH".to_owned(),
            Self::AutoDefault => "default lookup".to_owned(),
            Self::Built => "cargo build".to_owned(),
        }
    }
}

/// Version and build metadata discovered for a binary.
#[derive(Debug, Clone)]
pub struct BinaryVersionInfo {
    /// Logical name used by the supervisor.
    pub name: &'static str,
    /// Resolved path to the executable.
    pub path: PathBuf,
    /// Parsed version string when available.
    pub version: Option<String>,
    /// Inferred build line.
    pub build_line: BuildLine,
    /// Whether the binary should support DA/RBC.
    pub da_rbc_capable: bool,
    /// Raw stdout/stderr captured during probing.
    pub raw_output: Option<String>,
    /// Where the executable was discovered.
    source: BinarySource,
}

impl BinaryVersionInfo {
    /// Human-readable source label for display.
    pub fn source_label(&self) -> String {
        self.source.label()
    }
}

/// Structured summary emitted by `kagami verify`.
#[derive(Debug, Clone)]
pub struct KagamiVerifyReport {
    /// Profile the verification targeted.
    pub profile: GenesisProfile,
    /// Reported chain identifier (if present).
    pub chain_id: Option<String>,
    /// Reported collector quorum size.
    pub collectors_k: Option<u16>,
    /// Reported redundant send fanout.
    pub collectors_redundant_send_r: Option<u8>,
    /// VRF seed (hex) forwarded to Kagami, if any.
    pub vrf_seed_hex: Option<String>,
    /// Number of peers with PoP keys, when surfaced by Kagami.
    pub peers_with_pop: Option<u16>,
    /// Optional Kagami fingerprint or hash.
    pub fingerprint: Option<String>,
    /// Full stdout/stderr output from the verification command.
    pub raw_output: String,
}

impl KagamiVerifyReport {
    fn summary_fragment(&self) -> String {
        let mut parts = Vec::new();
        if let Some(chain_id) = &self.chain_id {
            parts.push(format!("chain {chain_id}"));
        }
        if let Some(fingerprint) = &self.fingerprint {
            parts.push(format!("fingerprint {fingerprint}"));
        }
        if let Some(k) = self.collectors_k {
            let r = self.collectors_redundant_send_r.unwrap_or_default();
            parts.push(format!("collectors k={k}, r={r}"));
        }
        parts.join(" • ")
    }
}

/// Compatibility and version summary for a prepared supervisor.
#[derive(Debug, Clone)]
pub struct CompatibilityReport {
    /// Build-line and version data for binaries.
    pub versions: Vec<BinaryVersionInfo>,
    /// Optional verification output from `kagami verify`.
    pub verify: Option<KagamiVerifyReport>,
    /// Chain identifier bound to the supervisor.
    pub chain_id: String,
    /// Genesis profile used to construct the network, if any.
    pub profile: Option<GenesisProfile>,
}

impl CompatibilityReport {
    /// One-line human readable summary suitable for the UI header.
    pub fn summary_line(&self) -> String {
        let mut fragments = Vec::new();
        if let Some(profile) = self.profile {
            fragments.push(format!("profile {}", profile.as_kagami_arg()));
        }
        if let Some(verify) = &self.verify {
            let frag = verify.summary_fragment();
            if !frag.is_empty() {
                fragments.push(frag);
            }
        }
        if fragments.is_empty() {
            format!("chain {}", self.chain_id)
        } else {
            format!("chain {} • {}", self.chain_id, fragments.join(" • "))
        }
    }
}

/// Paths to external binaries used by the supervisor.
#[derive(Debug, Clone)]
pub struct BinaryPaths {
    irohad: PathBuf,
    irohad_verified: bool,
    irohad_build_attempted: bool,
    irohad_auto: bool,
    irohad_source: BinarySource,
    kagami: PathBuf,
    kagami_verified: bool,
    kagami_build_attempted: bool,
    kagami_auto: bool,
    kagami_source: BinarySource,
    iroha_cli: PathBuf,
    iroha_cli_verified: bool,
    iroha_cli_build_attempted: bool,
    iroha_cli_auto: bool,
    iroha_cli_source: BinarySource,
    allow_builds: bool,
}

fn default_binary_entry(
    env_override: &'static str,
    cargo_env: &'static str,
    binary: &str,
) -> (PathBuf, bool, BinarySource) {
    let exe_name = format!("{binary}{}", env::consts::EXE_SUFFIX);

    let env_path = |key: &str| {
        env::var_os(key)
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
    };

    if let Some(path) = env_path(env_override) {
        return (path, false, BinarySource::Env(env_override));
    }
    if let Some(path) = env_path(cargo_env) {
        return (path, false, BinarySource::CargoEnv(cargo_env));
    }

    if let Ok(current) = env::current_exe()
        && let Some(dir) = current.parent()
    {
        let candidate = dir.join(&exe_name);
        if candidate.exists() {
            return (candidate, true, BinarySource::CurrentExeNeighbor);
        }
    }

    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    if let Some(workspace) = manifest_dir.parent().and_then(|dir| dir.parent()) {
        for profile in ["debug", "release"] {
            let candidate = workspace.join("target").join(profile).join(&exe_name);
            if candidate.exists() {
                return (
                    candidate,
                    true,
                    BinarySource::WorkspaceTarget(profile.to_owned()),
                );
            }
        }
    }

    (PathBuf::from(binary), true, BinarySource::AutoDefault)
}

fn default_irohad_entry() -> (PathBuf, bool, BinarySource) {
    const ENV_OVERRIDE: &str = "MOCHI_IROHAD";
    const CARGO_ENV: &str = "CARGO_BIN_EXE_irohad";
    default_binary_entry(ENV_OVERRIDE, CARGO_ENV, "irohad")
}

fn default_kagami_entry() -> (PathBuf, bool, BinarySource) {
    const ENV_OVERRIDE: &str = "MOCHI_KAGAMI";
    const CARGO_ENV: &str = "CARGO_BIN_EXE_kagami";
    default_binary_entry(ENV_OVERRIDE, CARGO_ENV, "kagami")
}

fn default_iroha_cli_entry() -> (PathBuf, bool, BinarySource) {
    const ENV_OVERRIDE: &str = "MOCHI_IROHA_CLI";
    const CARGO_ENV: &str = "CARGO_BIN_EXE_iroha_cli";
    default_binary_entry(ENV_OVERRIDE, CARGO_ENV, "iroha_cli")
}

impl BinaryPaths {
    /// Override the path to the `irohad` executable.
    pub fn irohad(mut self, path: impl Into<PathBuf>) -> Self {
        self.irohad = path.into();
        self.irohad_verified = false;
        self.irohad_build_attempted = false;
        self.irohad_auto = false;
        self.irohad_source = BinarySource::Explicit;
        self
    }

    /// Override the path to the `kagami` executable.
    pub fn kagami(mut self, path: impl Into<PathBuf>) -> Self {
        self.kagami = path.into();
        self.kagami_verified = false;
        self.kagami_build_attempted = false;
        self.kagami_auto = false;
        self.kagami_source = BinarySource::Explicit;
        self
    }

    /// Override the path to the `iroha_cli` executable.
    pub fn iroha_cli(mut self, path: impl Into<PathBuf>) -> Self {
        self.iroha_cli = path.into();
        self.iroha_cli_verified = false;
        self.iroha_cli_build_attempted = false;
        self.iroha_cli_auto = false;
        self.iroha_cli_source = BinarySource::Explicit;
        self
    }

    /// Enable or disable automatic cargo builds when binaries are missing.
    pub fn allow_auto_builds(mut self, allow: bool) -> Self {
        self.allow_builds = allow;
        self
    }

    fn verify_build_line(&mut self, expected: BuildLine) -> Result<()> {
        let versions = self.probe_versions()?;
        for info in &versions {
            if info.build_line != expected {
                return Err(SupervisorError::BuildLineMismatch {
                    binary: info.name.to_owned(),
                    expected,
                    found: info.build_line,
                });
            }
        }
        Ok(())
    }

    fn probe_versions(&mut self) -> Result<Vec<BinaryVersionInfo>> {
        let irohad = self.ensure_irohad_ready()?.to_path_buf();
        let irohad_source = self.irohad_source.clone();
        let kagami = self.ensure_kagami_ready()?.to_path_buf();
        let kagami_source = self.kagami_source.clone();
        let iroha_cli = self.ensure_iroha_cli_ready()?.to_path_buf();
        let iroha_cli_source = self.iroha_cli_source.clone();

        let entries = [
            ("irohad", irohad, irohad_source),
            ("kagami", kagami, kagami_source),
            ("iroha_cli", iroha_cli, iroha_cli_source),
        ];

        let mut versions = Vec::with_capacity(entries.len());
        for (name, path, source) in entries {
            versions.push(Self::probe_single_version(name, &path, source)?);
        }
        Ok(versions)
    }

    fn probe_single_version(
        name: &'static str,
        path: &Path,
        source: BinarySource,
    ) -> Result<BinaryVersionInfo> {
        let (raw_output, version) = probe_version_output(path, name)?;
        let build_line = infer_build_line(name, path, raw_output.as_deref());

        Ok(BinaryVersionInfo {
            name,
            path: path.to_path_buf(),
            version,
            build_line,
            da_rbc_capable: build_line.is_iroha3(),
            raw_output,
            source,
        })
    }

    fn irohad_executable(&self) -> &Path {
        &self.irohad
    }

    fn ensure_irohad_ready(&mut self) -> Result<&Path> {
        if self.irohad_verified && is_executable_file(&self.irohad) {
            return Ok(&self.irohad);
        }

        if is_explicit_path(&self.irohad) && is_executable_file(&self.irohad) {
            self.irohad_verified = true;
            self.irohad_source = BinarySource::Explicit;
            return Ok(&self.irohad);
        }

        if !is_explicit_path(&self.irohad) {
            if let Some(resolved) = resolve_name_on_path(self.irohad.as_os_str()) {
                self.irohad = resolved;
                self.irohad_verified = true;
                self.irohad_source = BinarySource::PathSearch;
                return Ok(&self.irohad);
            }
        } else if let Some(name) = self.irohad.file_name()
            && let Some(resolved) = resolve_name_on_path(name)
        {
            self.irohad = resolved;
            self.irohad_verified = true;
            self.irohad_source = BinarySource::PathSearch;
            return Ok(&self.irohad);
        }

        if self.allow_builds && self.irohad_auto && !self.irohad_build_attempted {
            self.irohad_build_attempted = true;
            if let Some(workspace) = workspace_root() {
                match try_build_irohad(&workspace) {
                    Ok(path) => {
                        self.irohad = path;
                        self.irohad_verified = true;
                        self.irohad_source = BinarySource::Built;
                        return Ok(&self.irohad);
                    }
                    Err(err) => return Err(err),
                }
            }
        }

        if is_executable_file(&self.irohad) {
            self.irohad_verified = true;
            return Ok(&self.irohad);
        }

        let message = if self.irohad_auto {
            format!(
                "looked for `{}` and searched on PATH; run `cargo build -p irohad` \
                 or set `MOCHI_IROHAD`/`binaries.irohad` to the executable",
                self.irohad.display()
            )
        } else {
            format!(
                "configured path `{}` is not executable; adjust \
                 `MOCHI_IROHAD`/`binaries.irohad` to point at a valid `irohad` binary",
                self.irohad.display()
            )
        };

        Err(SupervisorError::BinaryUnavailable {
            binary: "irohad",
            message,
        })
    }

    fn ensure_kagami_ready(&mut self) -> Result<&Path> {
        if self.kagami_verified && is_executable_file(&self.kagami) {
            return Ok(&self.kagami);
        }

        if is_explicit_path(&self.kagami) && is_executable_file(&self.kagami) {
            self.kagami_verified = true;
            self.kagami_source = BinarySource::Explicit;
            return Ok(&self.kagami);
        }

        if !is_explicit_path(&self.kagami) {
            if let Some(resolved) = resolve_name_on_path(self.kagami.as_os_str()) {
                self.kagami = resolved;
                self.kagami_verified = true;
                self.kagami_source = BinarySource::PathSearch;
                return Ok(&self.kagami);
            }
        } else if let Some(name) = self.kagami.file_name()
            && let Some(resolved) = resolve_name_on_path(name)
        {
            self.kagami = resolved;
            self.kagami_verified = true;
            self.kagami_source = BinarySource::PathSearch;
            return Ok(&self.kagami);
        }

        if self.allow_builds && self.kagami_auto && !self.kagami_build_attempted {
            self.kagami_build_attempted = true;
            if let Some(workspace) = workspace_root() {
                match try_build_kagami(&workspace) {
                    Ok(path) => {
                        self.kagami = path;
                        self.kagami_verified = true;
                        self.kagami_source = BinarySource::Built;
                        return Ok(&self.kagami);
                    }
                    Err(err) => return Err(err),
                }
            }
        }

        if is_executable_file(&self.kagami) {
            self.kagami_verified = true;
            return Ok(&self.kagami);
        }

        let message = if self.kagami_auto {
            format!(
                "looked for `{}` and searched on PATH; run `cargo build -p iroha_kagami` \
                 or set `MOCHI_KAGAMI`/`binaries.kagami` to the executable",
                self.kagami.display()
            )
        } else {
            format!(
                "configured path `{}` is not executable; adjust \
                 `MOCHI_KAGAMI`/`binaries.kagami` to point at a valid `kagami` binary",
                self.kagami.display()
            )
        };

        Err(SupervisorError::BinaryUnavailable {
            binary: "kagami",
            message,
        })
    }

    fn iroha_cli_executable(&self) -> &Path {
        &self.iroha_cli
    }

    fn ensure_iroha_cli_ready(&mut self) -> Result<&Path> {
        if self.iroha_cli_verified && is_executable_file(&self.iroha_cli) {
            return Ok(&self.iroha_cli);
        }

        if is_explicit_path(&self.iroha_cli) && is_executable_file(&self.iroha_cli) {
            self.iroha_cli_verified = true;
            self.iroha_cli_source = BinarySource::Explicit;
            return Ok(&self.iroha_cli);
        }

        if !is_explicit_path(&self.iroha_cli) {
            if let Some(resolved) = resolve_name_on_path(self.iroha_cli.as_os_str()) {
                self.iroha_cli = resolved;
                self.iroha_cli_verified = true;
                self.iroha_cli_source = BinarySource::PathSearch;
                return Ok(&self.iroha_cli);
            }
        } else if let Some(name) = self.iroha_cli.file_name()
            && let Some(resolved) = resolve_name_on_path(name)
        {
            self.iroha_cli = resolved;
            self.iroha_cli_verified = true;
            self.iroha_cli_source = BinarySource::PathSearch;
            return Ok(&self.iroha_cli);
        }

        if self.allow_builds && self.iroha_cli_auto && !self.iroha_cli_build_attempted {
            self.iroha_cli_build_attempted = true;
            if let Some(workspace) = workspace_root() {
                match try_build_iroha_cli(&workspace) {
                    Ok(path) => {
                        self.iroha_cli = path;
                        self.iroha_cli_verified = true;
                        self.iroha_cli_source = BinarySource::Built;
                        return Ok(&self.iroha_cli);
                    }
                    Err(err) => return Err(err),
                }
            }
        }

        if is_executable_file(&self.iroha_cli) {
            self.iroha_cli_verified = true;
            return Ok(&self.iroha_cli);
        }

        let message = if self.iroha_cli_auto {
            format!(
                "looked for `{}` and searched on PATH; run `cargo build -p iroha_cli` \
                 or set `MOCHI_IROHA_CLI`/`binaries.iroha_cli` to the executable",
                self.iroha_cli.display()
            )
        } else {
            format!(
                "configured path `{}` is not executable; adjust \
                 `MOCHI_IROHA_CLI`/`binaries.iroha_cli` to point at a valid `iroha_cli` binary",
                self.iroha_cli.display()
            )
        };

        Err(SupervisorError::BinaryUnavailable {
            binary: "iroha_cli",
            message,
        })
    }
}

impl Default for BinaryPaths {
    fn default() -> Self {
        let (irohad, irohad_auto, irohad_source) = default_irohad_entry();
        let (kagami, kagami_auto, kagami_source) = default_kagami_entry();
        let (iroha_cli, iroha_cli_auto, iroha_cli_source) = default_iroha_cli_entry();
        Self {
            irohad,
            irohad_verified: false,
            irohad_build_attempted: false,
            irohad_auto,
            irohad_source,
            kagami,
            kagami_verified: false,
            kagami_build_attempted: false,
            kagami_auto,
            kagami_source,
            iroha_cli,
            iroha_cli_verified: false,
            iroha_cli_build_attempted: false,
            iroha_cli_auto,
            iroha_cli_source,
            allow_builds: false,
        }
    }
}

fn probe_version_output(path: &Path, binary: &str) -> Result<(Option<String>, Option<String>)> {
    let output = Command::new(path)
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|err| SupervisorError::VersionProbeFailed {
            binary: binary.to_owned(),
            message: err.to_string(),
        })?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let mut raw_output = None;
    if !stdout.trim().is_empty() || !stderr.trim().is_empty() {
        let mut combined = stdout.trim_end().to_owned();
        let stderr_trimmed = stderr.trim_end();
        if !stderr_trimmed.is_empty() {
            if !combined.is_empty() {
                combined.push('\n');
            }
            combined.push_str(stderr_trimmed);
        }
        raw_output = Some(combined);
    }

    if !output.status.success() {
        let message = if let Some(raw) = &raw_output {
            format!("`{binary} --version` exited with {}: {raw}", output.status)
        } else {
            format!("`{binary} --version` exited with {}", output.status)
        };
        return Err(SupervisorError::VersionProbeFailed {
            binary: binary.to_owned(),
            message,
        });
    }

    let version = raw_output
        .as_deref()
        .and_then(|text| text.lines().find(|line| !line.trim().is_empty()))
        .map(|line| line.trim().to_owned());

    Ok((raw_output, version))
}

fn infer_build_line(name: &str, path: &Path, output: Option<&str>) -> BuildLine {
    if let Some(raw) = output {
        let lower = raw.to_ascii_lowercase();
        if lower.contains("iroha2") || lower.contains("iroha 2") {
            return BuildLine::Iroha2;
        }
        if lower.contains("iroha3") || lower.contains("iroha 3") || lower.contains("nexus") {
            return BuildLine::Iroha3;
        }
    }

    if let Some(stem) = path.file_stem().and_then(|value| value.to_str()) {
        let inferred = BuildLine::from_bin_name(stem);
        if inferred.is_iroha2() || inferred.is_iroha3() {
            return inferred;
        }
    }

    BuildLine::from_bin_name(name)
}

fn is_explicit_path(path: &Path) -> bool {
    path.has_root() || path.components().count() > 1
}

fn is_executable_file(path: &Path) -> bool {
    match fs::metadata(path) {
        Ok(meta) => meta.is_file(),
        Err(_) => false,
    }
}

fn resolve_name_on_path(name: &OsStr) -> Option<PathBuf> {
    let path_var = env::var_os("PATH")?;
    let mut candidates = Vec::with_capacity(2);
    candidates.push(OsString::from(name));

    let suffix = env::consts::EXE_SUFFIX;
    if !suffix.is_empty() {
        let has_suffix = name.to_string_lossy().ends_with(suffix);
        if !has_suffix {
            let mut with_suffix = OsString::from(name);
            with_suffix.push(suffix);
            candidates.push(with_suffix);
        }
    }

    for dir in env::split_paths(&path_var) {
        for candidate in &candidates {
            let full = dir.join(candidate);
            if is_executable_file(&full) {
                return Some(full);
            }
        }
    }

    None
}

fn workspace_root() -> Option<PathBuf> {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .and_then(|dir| dir.parent())
        .filter(|root| root.exists())
        .map(PathBuf::from)
}

fn try_build_irohad(workspace: &Path) -> Result<PathBuf> {
    let cargo = env::var_os("CARGO")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("cargo"));

    let mut command = Command::new(&cargo);
    command
        .current_dir(workspace)
        .arg("build")
        .arg("-p")
        .arg("irohad")
        .stdout(Stdio::null());

    // Preserve stderr so build failures surface in the parent console.
    let status = command
        .status()
        .map_err(|err| SupervisorError::BinaryUnavailable {
            binary: "irohad",
            message: format!("failed to invoke `{}`: {err}", cargo.display()),
        })?;

    if !status.success() {
        return Err(SupervisorError::BinaryUnavailable {
            binary: "irohad",
            message: format!("`cargo build -p irohad` exited with status {status}"),
        });
    }

    let target_root = env::var_os("CARGO_TARGET_DIR")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| workspace.join("target"));

    let exe_name = format!("irohad{}", env::consts::EXE_SUFFIX);
    let candidates = [
        target_root.join("debug").join(&exe_name),
        target_root.join("release").join(&exe_name),
    ];

    for candidate in candidates {
        if is_executable_file(&candidate) {
            return Ok(candidate);
        }
    }

    Err(SupervisorError::BinaryUnavailable {
        binary: "irohad",
        message: format!(
            "built `irohad` but could not find an executable under `{}`",
            target_root.display()
        ),
    })
}

fn try_build_kagami(workspace: &Path) -> Result<PathBuf> {
    let cargo = env::var_os("CARGO")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("cargo"));

    let mut command = Command::new(&cargo);
    command
        .current_dir(workspace)
        .arg("build")
        .arg("-p")
        .arg("iroha_kagami")
        .stdout(Stdio::null());

    let status = command
        .status()
        .map_err(|err| SupervisorError::BinaryUnavailable {
            binary: "kagami",
            message: format!("failed to invoke `{}`: {err}", cargo.display()),
        })?;

    if !status.success() {
        return Err(SupervisorError::BinaryUnavailable {
            binary: "kagami",
            message: format!("`cargo build -p iroha_kagami` exited with status {status}"),
        });
    }

    let target_root = env::var_os("CARGO_TARGET_DIR")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| workspace.join("target"));

    let exe_name = format!("kagami{}", env::consts::EXE_SUFFIX);
    let candidates = [
        target_root.join("debug").join(&exe_name),
        target_root.join("release").join(&exe_name),
    ];

    for candidate in candidates {
        if is_executable_file(&candidate) {
            return Ok(candidate);
        }
    }

    Err(SupervisorError::BinaryUnavailable {
        binary: "kagami",
        message: format!(
            "built `kagami` but could not find an executable under `{}`",
            target_root.display()
        ),
    })
}

fn try_build_iroha_cli(workspace: &Path) -> Result<PathBuf> {
    let cargo = env::var_os("CARGO")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("cargo"));

    let mut command = Command::new(&cargo);
    command
        .current_dir(workspace)
        .arg("build")
        .arg("-p")
        .arg("iroha_cli")
        .stdout(Stdio::null());

    let status = command
        .status()
        .map_err(|err| SupervisorError::BinaryUnavailable {
            binary: "iroha_cli",
            message: format!("failed to invoke `{}`: {err}", cargo.display()),
        })?;

    if !status.success() {
        return Err(SupervisorError::BinaryUnavailable {
            binary: "iroha_cli",
            message: format!("`cargo build -p iroha_cli` exited with status {status}"),
        });
    }

    let target_root = env::var_os("CARGO_TARGET_DIR")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| workspace.join("target"));

    for bin in ["iroha3", "iroha", "iroha2"] {
        let exe_name = format!("{bin}{}", env::consts::EXE_SUFFIX);
        for candidate in [
            target_root.join("debug").join(&exe_name),
            target_root.join("release").join(&exe_name),
        ] {
            if is_executable_file(&candidate) {
                return Ok(candidate);
            }
        }
    }

    Err(SupervisorError::BinaryUnavailable {
        binary: "iroha_cli",
        message: format!(
            "built `iroha_cli` but could not find an `iroha3`/`iroha` executable under `{}`",
            target_root.display()
        ),
    })
}

/// Builds a [`Supervisor`] with user-selected presets.
#[derive(Debug, Clone)]
pub struct SupervisorBuilder {
    profile: NetworkProfile,
    data_root: PathBuf,
    chain_id: String,
    torii_base_port: u16,
    p2p_base_port: u16,
    genesis_profile: Option<GenesisProfile>,
    vrf_seed_hex: Option<String>,
    binaries: BinaryPaths,
    restart_policy: RestartPolicy,
    auto_build_binaries: bool,
    nexus_config: Option<toml::Table>,
    sumeragi_config: Option<toml::Table>,
    torii_config: Option<toml::Table>,
}

impl SupervisorBuilder {
    /// Create a builder using one of the predefined presets.
    pub fn new(preset: ProfilePreset) -> Self {
        let profile = NetworkProfile::from_preset(preset);
        let data_root = default_data_root();
        Self {
            profile,
            data_root,
            chain_id: DEFAULT_CHAIN_ID.to_owned(),
            torii_base_port: DEFAULT_TORII_BASE_PORT,
            p2p_base_port: DEFAULT_P2P_BASE_PORT,
            genesis_profile: None,
            vrf_seed_hex: None,
            binaries: BinaryPaths::default(),
            restart_policy: RestartPolicy::default(),
            auto_build_binaries: false,
            nexus_config: None,
            sumeragi_config: None,
            torii_config: None,
        }
    }

    /// Override the default profile with a custom topology.
    pub fn with_profile(profile: NetworkProfile) -> Self {
        let data_root = default_data_root();
        Self {
            profile,
            data_root,
            chain_id: DEFAULT_CHAIN_ID.to_owned(),
            torii_base_port: DEFAULT_TORII_BASE_PORT,
            p2p_base_port: DEFAULT_P2P_BASE_PORT,
            genesis_profile: None,
            vrf_seed_hex: None,
            binaries: BinaryPaths::default(),
            restart_policy: RestartPolicy::default(),
            auto_build_binaries: false,
            nexus_config: None,
            sumeragi_config: None,
            torii_config: None,
        }
    }

    /// Override the network profile while preserving existing builder settings.
    pub fn set_profile(mut self, profile: NetworkProfile) -> Self {
        self.profile = profile;
        self
    }

    /// Override the profile using a preset value.
    pub fn profile_preset(self, preset: ProfilePreset) -> Self {
        let consensus_mode = self.profile.consensus_mode;
        let mut profile = NetworkProfile::from_preset(preset);
        profile.consensus_mode = consensus_mode;
        self.set_profile(profile)
    }

    /// Provide a custom data root directory that contains configs, logs, and snapshots.
    pub fn data_root(mut self, root: impl Into<PathBuf>) -> Self {
        self.data_root = root.into();
        self
    }

    /// Set a custom chain identifier for the generated configurations.
    pub fn chain_id(mut self, chain_id: impl Into<String>) -> Self {
        self.chain_id = chain_id.into();
        self
    }

    /// Select a Kagami genesis profile; also aligns the chain id and consensus mode for NPoS.
    pub fn genesis_profile(mut self, profile: GenesisProfile) -> Self {
        self.genesis_profile = Some(profile);
        self.profile.consensus_mode = SumeragiConsensusMode::Npos;
        let defaults = profile.defaults();
        self.chain_id = defaults.chain_id.to_owned();
        self
    }

    /// Provide an explicit VRF seed for Kagami genesis (hex, 32 bytes).
    pub fn vrf_seed_hex(mut self, seed: impl Into<String>) -> Self {
        self.vrf_seed_hex = Some(seed.into());
        self
    }

    /// Adjust the starting port for Torii bindings.
    pub fn torii_base_port(mut self, port: u16) -> Self {
        self.torii_base_port = port;
        self
    }

    /// Adjust the starting port for the P2P listener.
    pub fn p2p_base_port(mut self, port: u16) -> Self {
        self.p2p_base_port = port;
        self
    }

    /// Override the paths to external binaries.
    pub fn binaries(mut self, binaries: BinaryPaths) -> Self {
        self.binaries = binaries;
        self
    }

    /// Allow the supervisor to build missing binaries automatically.
    pub fn auto_build_binaries(mut self, allow: bool) -> Self {
        self.auto_build_binaries = allow;
        self
    }

    /// Override just the `irohad` binary path.
    pub fn irohad_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.binaries = self.binaries.clone().irohad(path);
        self
    }

    /// Override just the `kagami` binary path.
    pub fn kagami_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.binaries = self.binaries.clone().kagami(path);
        self
    }

    /// Override just the `iroha_cli` binary path.
    pub fn iroha_cli_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.binaries = self.binaries.clone().iroha_cli(path);
        self
    }

    /// Configure the restart policy for managed peers.
    pub fn restart_policy(mut self, policy: RestartPolicy) -> Self {
        self.restart_policy = policy;
        self
    }

    /// Override the generated Nexus configuration table.
    pub fn nexus_config(mut self, config: toml::Table) -> Self {
        self.nexus_config = Some(config);
        self
    }

    /// Enable or disable Nexus features in the rendered peer configs.
    pub fn nexus_enabled(mut self, enabled: bool) -> Self {
        set_table_bool(&mut self.nexus_config, "enabled", enabled);
        self
    }

    /// Override the configured Nexus lane count.
    pub fn nexus_lane_count(mut self, lane_count: u32) -> Self {
        set_table_u32(&mut self.nexus_config, "lane_count", lane_count);
        self
    }

    /// Override the generated Sumeragi configuration table.
    pub fn sumeragi_config(mut self, config: toml::Table) -> Self {
        self.sumeragi_config = Some(config);
        self
    }

    /// Enable or disable data availability in the rendered peer configs.
    pub fn sumeragi_da_enabled(mut self, enabled: bool) -> Self {
        set_table_bool(&mut self.sumeragi_config, "da_enabled", enabled);
        self
    }

    /// Override the generated Torii configuration table.
    pub fn torii_config(mut self, config: toml::Table) -> Self {
        self.torii_config = Some(config);
        self
    }

    /// Retrieve the profile that will be used when building the supervisor.
    pub fn profile(&self) -> &NetworkProfile {
        &self.profile
    }

    fn reserve_unique_port(
        allocator: &mut PortAllocator,
        reserved: &mut HashSet<u16>,
        label: &str,
    ) -> Result<u16> {
        loop {
            let port = allocator.allocate().map_err(|err| {
                SupervisorError::Config(format!("failed to allocate {label} port: {err}"))
            })?;
            if reserved.insert(port) {
                return Ok(port);
            }
        }
    }

    /// Finalize the builder and construct a supervisor instance.
    pub fn build(self) -> Result<Supervisor> {
        self.profile.validate().map_err(SupervisorError::Config)?;
        if self.genesis_profile.is_some()
            && self.profile.consensus_mode != SumeragiConsensusMode::Npos
        {
            return Err(SupervisorError::Config(
                "genesis_profile requires consensus_mode npos".to_owned(),
            ));
        }
        let paths = NetworkPaths::from_root(&self.data_root, &self.profile);
        paths.ensure()?;
        let mut binaries = self.binaries.allow_auto_builds(self.auto_build_binaries);
        let chain_id = if let Some(profile) = self.genesis_profile {
            let defaults = profile.defaults();
            if self.chain_id != defaults.chain_id {
                return Err(SupervisorError::Config(format!(
                    "genesis profile {profile:?} requires chain id `{}`; remove the chain override",
                    defaults.chain_id
                )));
            }
            defaults.chain_id.to_owned()
        } else {
            self.chain_id.clone()
        };
        let mut nexus_config = self.nexus_config.clone();
        let mut sumeragi_config = self.sumeragi_config.clone();
        let mut torii_config = self.torii_config.clone();
        normalize_peer_config_overrides(
            &mut nexus_config,
            &mut sumeragi_config,
            &mut torii_config,
        )?;
        let peer_config_overrides = PeerConfigOverrides {
            nexus: nexus_config,
            sumeragi: sumeragi_config,
            torii: torii_config,
        };

        if self.genesis_profile.is_some() {
            binaries.ensure_irohad_ready()?;
            binaries.ensure_kagami_ready()?;
            binaries.ensure_iroha_cli_ready()?;
            binaries.verify_build_line(BuildLine::Iroha3)?;
        }

        let mut torii_ports = PortAllocator::new(self.torii_base_port);
        let mut p2p_ports = PortAllocator::new(self.p2p_base_port);
        let mut reserved_ports = HashSet::new();

        let mut specs = Vec::with_capacity(self.profile.topology.peer_count);
        for index in 0..self.profile.topology.peer_count {
            let alias = format!("peer{index}");
            let torii_port =
                Self::reserve_unique_port(&mut torii_ports, &mut reserved_ports, "Torii")?;
            let p2p_port = Self::reserve_unique_port(&mut p2p_ports, &mut reserved_ports, "P2P")?;
            specs.push(PeerSpec::new(&paths, alias, torii_port, p2p_port)?);
        }

        let genesis = GenesisMaterial::create(
            &mut binaries,
            &paths,
            &chain_id,
            &specs,
            self.profile.consensus_mode,
            self.genesis_profile,
            self.vrf_seed_hex.as_deref(),
        )?;

        for spec in &specs {
            spec.write_config(&chain_id, &genesis, &specs, &peer_config_overrides)?;
        }

        let peers = specs
            .into_iter()
            .map(|spec| PeerHandle::prepared(spec, paths.logs_dir(), self.restart_policy))
            .collect::<Vec<_>>();

        let vault = SignerVault::new(&paths);
        let signers = vault.load_with_fallback();

        let mut supervisor = Supervisor {
            profile: self.profile,
            paths,
            chain_id,
            genesis,
            peers,
            signers,
            binaries,
            peer_config_overrides: peer_config_overrides.clone(),
            compatibility: None,
            irohad_ready: false,
            iroha_cli_ready: false,
        };
        supervisor.run_compatibility_checks()?;
        Ok(supervisor)
    }
}

fn set_table_bool(target: &mut Option<toml::Table>, key: &str, value: bool) {
    let table = target.get_or_insert_with(toml::Table::new);
    table.insert(key.to_owned(), toml::Value::Boolean(value));
}

fn set_table_u32(target: &mut Option<toml::Table>, key: &str, value: u32) {
    let table = target.get_or_insert_with(toml::Table::new);
    table.insert(key.to_owned(), toml::Value::Integer(i64::from(value)));
}

fn merge_table(target: &mut toml::Table, overlay: &toml::Table) {
    for (key, value) in overlay {
        target.insert(key.clone(), value.clone());
    }
}

fn lane_aliases(nexus: Option<&toml::Table>) -> BTreeMap<u32, String> {
    let Some(nexus) = nexus else {
        return BTreeMap::new();
    };
    let mut entries = BTreeMap::<u32, String>::new();
    let lane_count = nexus
        .get("lane_count")
        .and_then(toml::Value::as_integer)
        .and_then(|value| u32::try_from(value).ok())
        .unwrap_or(1);

    if let Some(values) = nexus.get("lane_catalog").and_then(toml::Value::as_array) {
        for (idx, value) in values.iter().enumerate() {
            let Some(table) = value.as_table() else {
                continue;
            };
            let index = table
                .get("index")
                .and_then(toml::Value::as_integer)
                .and_then(|raw| u32::try_from(raw).ok())
                .unwrap_or_else(|| u32::try_from(idx).unwrap_or(0));
            let alias = table
                .get("alias")
                .and_then(toml::Value::as_str)
                .map(str::to_owned)
                .unwrap_or_else(|| default_lane_alias(index));
            entries.insert(index, alias);
        }
    }

    if entries.is_empty() && lane_count > 1 {
        for index in 0..lane_count {
            entries.insert(index, default_lane_alias(index));
        }
    }

    entries
}

fn lane_alias_for_id(nexus: Option<&toml::Table>, lane_id: u32) -> Result<String> {
    let entries = lane_aliases(nexus);
    if let Some(alias) = entries.get(&lane_id) {
        return Ok(alias.clone());
    }
    if entries.is_empty() && lane_id == 0 {
        return Ok(default_lane_alias(0));
    }
    Err(SupervisorError::Config(format!(
        "nexus lane {lane_id} is not configured"
    )))
}

fn lane_path_comments(storage_root: &Path, nexus: Option<&toml::Table>) -> Vec<String> {
    let entries = lane_aliases(nexus);
    if entries.is_empty() {
        return Vec::new();
    }

    let mut comments = Vec::with_capacity(entries.len() * 3);
    for (lane_id, alias) in entries {
        let slug = lane_slug(&alias, lane_id);
        let kura_segment = format!("lane_{lane_id:03}_{slug}");
        let merge_segment = format!("lane_{lane_id:03}_{slug}_merge");
        let blocks_dir = storage_root.join("blocks").join(&kura_segment);
        let merge_log = storage_root
            .join("merge_ledger")
            .join(format!("{merge_segment}.log"));
        comments.push(format!("# mochi.lane[{lane_id}].alias = {alias}"));
        comments.push(format!(
            "# mochi.lane[{lane_id}].blocks_dir = {}",
            blocks_dir.display()
        ));
        comments.push(format!(
            "# mochi.lane[{lane_id}].merge_log = {}",
            merge_log.display()
        ));
    }
    comments
}

fn default_lane_alias(index: u32) -> String {
    if index == 0 {
        "default".to_owned()
    } else {
        format!("lane{index}")
    }
}

fn lane_slug(alias: &str, lane_id: u32) -> String {
    let mut slug = String::with_capacity(alias.len());
    let mut underscore_written = false;

    for ch in alias.chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
            underscore_written = false;
        } else if matches!(ch, '-' | ' ' | '_' | '.') {
            if !underscore_written {
                slug.push('_');
                underscore_written = true;
            }
        } else if !underscore_written {
            slug.push('_');
            underscore_written = true;
        }
    }

    let slug = slug.trim_matches('_').to_string();
    if slug.is_empty() {
        format!("lane{lane_id}")
    } else {
        slug
    }
}

fn normalize_peer_config_overrides(
    nexus: &mut Option<toml::Table>,
    sumeragi: &mut Option<toml::Table>,
    torii: &mut Option<toml::Table>,
) -> Result<()> {
    let mut nexus_enabled_effective = None;

    if let Some(table) = nexus.as_mut() {
        let enabled = parse_table_bool(table, "enabled", "nexus.enabled")?;
        let enabled_effective = enabled.unwrap_or(true);
        let lane_count = parse_table_u32(table, "lane_count", "nexus.lane_count")?;
        let lane_catalog = parse_table_array(table, "lane_catalog", "nexus.lane_catalog")?;
        let dataspace_catalog =
            parse_table_array(table, "dataspace_catalog", "nexus.dataspace_catalog")?;

        let lane_summary = lane_catalog
            .map(|values| LaneCatalogSummary::from_values(values))
            .transpose()?
            .unwrap_or_default();
        if let Some(catalog) = dataspace_catalog {
            ensure_table_entries(catalog, "nexus.dataspace_catalog")?;
        }
        let dataspace_len = dataspace_catalog.map_or(0, |values| values.len());

        let lane_count = if lane_count.is_some() {
            lane_count
        } else if lane_summary.len > 0 {
            let computed = lane_summary.max_index.unwrap_or(0).saturating_add(1);
            if computed == 0 {
                return Err(SupervisorError::Config(
                    "nexus.lane_catalog must contain at least one entry".to_owned(),
                ));
            }
            set_table_u32(nexus, "lane_count", computed);
            Some(computed)
        } else {
            lane_count
        };

        if !enabled_effective
            && (lane_count.unwrap_or(1) > 1 || lane_summary.len > 0 || dataspace_len > 0)
        {
            return Err(SupervisorError::Config(
                "nexus.enabled = false requires lane_count = 1 and empty lane/dataspace catalogs"
                    .to_owned(),
            ));
        }

        if let Some(count) = lane_count {
            if count == 0 {
                return Err(SupervisorError::Config(
                    "nexus.lane_count must be greater than zero".to_owned(),
                ));
            }
            if lane_summary.len > count as usize {
                return Err(SupervisorError::Config(format!(
                    "nexus.lane_count {count} is smaller than lane_catalog size {}",
                    lane_summary.len
                )));
            }
            if let Some(max_index) = lane_summary.max_index
                && max_index >= count
            {
                return Err(SupervisorError::Config(format!(
                    "nexus.lane_catalog index {max_index} exceeds lane_count {count}"
                )));
            }
        }

        nexus_enabled_effective = Some(enabled_effective);
    }

    let da_enabled = match sumeragi.as_ref() {
        Some(table) => parse_table_bool(table, "da_enabled", "sumeragi.da_enabled")?,
        None => None,
    };

    if matches!(nexus_enabled_effective, Some(true)) {
        match da_enabled {
            Some(true) => {}
            Some(false) => {
                return Err(SupervisorError::Config(
                    "nexus.enabled = true requires sumeragi.da_enabled = true".to_owned(),
                ));
            }
            None => {
                set_table_bool(sumeragi, "da_enabled", true);
            }
        }
    }

    if let Some(table) = torii.as_ref()
        && let Some(da_ingest) = table.get("da_ingest")
        && !matches!(da_ingest, toml::Value::Table(_))
    {
        return Err(SupervisorError::Config(
            "torii.da_ingest must be a table".to_owned(),
        ));
    }

    Ok(())
}

fn parse_table_bool(table: &toml::Table, key: &str, label: &str) -> Result<Option<bool>> {
    match table.get(key) {
        None => Ok(None),
        Some(toml::Value::Boolean(value)) => Ok(Some(*value)),
        Some(_) => Err(SupervisorError::Config(format!(
            "{label} must be a boolean"
        ))),
    }
}

fn parse_table_u32(table: &toml::Table, key: &str, label: &str) -> Result<Option<u32>> {
    match table.get(key) {
        None => Ok(None),
        Some(toml::Value::Integer(value)) => {
            if *value < 0 {
                return Err(SupervisorError::Config(format!(
                    "{label} must be a positive integer"
                )));
            }
            let unsigned = u64::try_from(*value)
                .map_err(|_| SupervisorError::Config(format!("{label} exceeds the u32 range")))?;
            u32::try_from(unsigned)
                .map_err(|_| SupervisorError::Config(format!("{label} exceeds the u32 range")))
                .map(Some)
        }
        Some(_) => Err(SupervisorError::Config(format!(
            "{label} must be an integer"
        ))),
    }
}

fn parse_table_array<'a>(
    table: &'a toml::Table,
    key: &str,
    label: &str,
) -> Result<Option<&'a Vec<toml::Value>>> {
    match table.get(key) {
        None => Ok(None),
        Some(toml::Value::Array(values)) => Ok(Some(values)),
        Some(_) => Err(SupervisorError::Config(format!(
            "{label} must be an array of tables"
        ))),
    }
}

fn ensure_table_entries(values: &[toml::Value], label: &str) -> Result<()> {
    for (idx, value) in values.iter().enumerate() {
        if !matches!(value, toml::Value::Table(_)) {
            return Err(SupervisorError::Config(format!(
                "{label}[{idx}] must be a table"
            )));
        }
    }
    Ok(())
}

#[derive(Debug, Default)]
struct LaneCatalogSummary {
    len: usize,
    max_index: Option<u32>,
}

impl LaneCatalogSummary {
    fn from_values(values: &[toml::Value]) -> Result<Self> {
        let mut summary = LaneCatalogSummary {
            len: values.len(),
            max_index: None,
        };
        for (idx, value) in values.iter().enumerate() {
            let toml::Value::Table(table) = value else {
                return Err(SupervisorError::Config(format!(
                    "nexus.lane_catalog[{idx}] must be a table"
                )));
            };
            let index = match table.get("index") {
                Some(toml::Value::Integer(raw)) => {
                    if *raw < 0 {
                        return Err(SupervisorError::Config(format!(
                            "nexus.lane_catalog[{idx}].index must be a positive integer"
                        )));
                    }
                    let unsigned = u64::try_from(*raw).map_err(|_| {
                        SupervisorError::Config(format!(
                            "nexus.lane_catalog[{idx}].index exceeds the u32 range"
                        ))
                    })?;
                    u32::try_from(unsigned).map_err(|_| {
                        SupervisorError::Config(format!(
                            "nexus.lane_catalog[{idx}].index exceeds the u32 range"
                        ))
                    })?
                }
                Some(_) => {
                    return Err(SupervisorError::Config(format!(
                        "nexus.lane_catalog[{idx}].index must be an integer"
                    )));
                }
                None => u32::try_from(idx).map_err(|_| {
                    SupervisorError::Config(format!(
                        "nexus.lane_catalog index {idx} exceeds the u32 range"
                    ))
                })?,
            };
            summary.max_index = Some(
                summary
                    .max_index
                    .map_or(index, |current| current.max(index)),
            );
        }
        Ok(summary)
    }
}

#[derive(Debug, Clone, Default)]
struct PeerConfigOverrides {
    nexus: Option<toml::Table>,
    sumeragi: Option<toml::Table>,
    torii: Option<toml::Table>,
}

impl PeerConfigOverrides {
    fn da_enabled(&self) -> bool {
        match self
            .sumeragi
            .as_ref()
            .and_then(|table| table.get("da_enabled"))
        {
            Some(toml::Value::Boolean(value)) => *value,
            _ => false,
        }
    }
}

/// Supervises a prepared set of peers for a local network.
#[derive(Debug)]
pub struct Supervisor {
    profile: NetworkProfile,
    paths: NetworkPaths,
    chain_id: String,
    genesis: GenesisMaterial,
    peers: Vec<PeerHandle>,
    signers: Vec<SigningAuthority>,
    binaries: BinaryPaths,
    peer_config_overrides: PeerConfigOverrides,
    compatibility: Option<CompatibilityReport>,
    irohad_ready: bool,
    iroha_cli_ready: bool,
}

impl Supervisor {
    /// Access the Nexus config overrides applied when rendering peer configs.
    pub fn nexus_config_overrides(&self) -> Option<&toml::Table> {
        self.peer_config_overrides.nexus.as_ref()
    }

    /// Access the Sumeragi config overrides applied when rendering peer configs.
    pub fn sumeragi_config_overrides(&self) -> Option<&toml::Table> {
        self.peer_config_overrides.sumeragi.as_ref()
    }

    /// Access the Torii config overrides applied when rendering peer configs.
    pub fn torii_config_overrides(&self) -> Option<&toml::Table> {
        self.peer_config_overrides.torii.as_ref()
    }

    fn ensure_irohad(&mut self) -> Result<&Path> {
        if !self.irohad_ready {
            let path = self.binaries.ensure_irohad_ready()?;
            // Store the resolved path back so subsequent calls reuse it.
            self.irohad_ready = true;
            return Ok(path);
        }
        Ok(self.binaries.irohad_executable())
    }

    fn irohad_path(&mut self) -> Result<PathBuf> {
        self.ensure_irohad().map(|path| path.to_path_buf())
    }

    fn ensure_iroha_cli(&mut self) -> Result<&Path> {
        if !self.iroha_cli_ready {
            let path = self.binaries.ensure_iroha_cli_ready()?;
            self.iroha_cli_ready = true;
            return Ok(path);
        }
        Ok(self.binaries.iroha_cli_executable())
    }

    /// Resolve the path to the configured `iroha_cli` binary, verifying it if necessary.
    pub fn iroha_cli_path(&mut self) -> Result<PathBuf> {
        self.ensure_iroha_cli().map(|path| path.to_path_buf())
    }

    fn refresh_peer_states_with(&mut self, irohad: &Path) {
        for peer in &mut self.peers {
            peer.refresh_state(irohad);
        }
    }

    /// Access metadata describing the topology and consensus profile the supervisor holds.
    pub fn profile(&self) -> &NetworkProfile {
        &self.profile
    }

    /// Returns the filesystem paths used by the supervisor.
    pub fn paths(&self) -> &NetworkPaths {
        &self.paths
    }

    /// Returns the configured chain identifier.
    pub fn chain_id(&self) -> &str {
        &self.chain_id
    }

    /// Path to the generated genesis manifest.
    pub fn genesis_manifest(&self) -> &Path {
        &self.genesis.manifest_path
    }

    /// Optional verification report emitted by `kagami verify` when a profile is selected.
    pub fn genesis_verify_report(&self) -> Option<&KagamiVerifyReport> {
        self.genesis.verify_report.as_ref()
    }

    /// Cached compatibility summary for the configured binaries and genesis.
    pub fn compatibility(&self) -> Option<&CompatibilityReport> {
        self.compatibility.as_ref()
    }

    /// Returns the prepared peers in their current states.
    pub fn peers(&self) -> &[PeerHandle] {
        &self.peers
    }

    /// Mutable access to the managed peers (primarily for UI refresh logic).
    pub fn peers_mut(&mut self) -> &mut [PeerHandle] {
        &mut self.peers
    }

    /// Available signing authorities for local transactions.
    pub fn signers(&self) -> &[SigningAuthority] {
        &self.signers
    }

    /// Access the signer vault handle for the current supervisor.
    #[must_use]
    pub fn signer_vault(&self) -> SignerVault {
        SignerVault::new(&self.paths)
    }

    /// Reload signing authorities from disk, applying fallback keys when necessary.
    pub fn reload_signers(&mut self) {
        let vault = SignerVault::new(&self.paths);
        self.signers = vault.load_with_fallback();
    }

    /// Persist new signing authorities and refresh the in-memory cache.
    pub fn save_signers(&mut self, signers: &[SigningAuthority]) -> Result<()> {
        let vault = SignerVault::new(&self.paths);
        vault.save(signers)?;
        self.signers = vault.load_with_fallback();
        Ok(())
    }

    /// Build a readiness smoke plan using the genesis signing authority.
    pub fn readiness_smoke_plan(&self, attempts: usize) -> Result<ReadinessSmokePlan> {
        self.readiness_smoke_plan_with_offset(attempts, 0)
    }

    /// Build a readiness smoke plan using the genesis signing authority and nonce offset.
    pub fn readiness_smoke_plan_with_offset(
        &self,
        attempts: usize,
        nonce_offset: usize,
    ) -> Result<ReadinessSmokePlan> {
        let signer = self.readiness_smoke_signer()?;
        ReadinessSmokePlan::for_signer_with_attempts_and_offset(
            &self.chain_id,
            &signer,
            attempts.max(1),
            nonce_offset,
        )
        .map_err(|err| SupervisorError::Config(err.to_string()))
    }

    /// Build a readiness smoke plan with the default retry budget.
    pub fn default_readiness_smoke_plan(&self) -> Result<ReadinessSmokePlan> {
        self.readiness_smoke_plan(SMOKE_MAX_ATTEMPTS)
    }

    /// Construct a Torii client for the specified peer alias.
    pub fn torii_client(&self, alias: &str) -> Option<ToriiClient> {
        self.peers
            .iter()
            .find(|peer| peer.alias() == alias)
            .and_then(|peer| peer.torii_client().ok())
    }

    /// Access the structured log stream for the given peer alias.
    pub fn log_stream(&self, alias: &str) -> Option<PeerLogStream> {
        self.peers
            .iter()
            .find(|peer| peer.alias() == alias)
            .map(|peer| peer.log_stream())
    }

    /// Create a managed block stream handle for the specified peer using the provided runtime.
    pub fn managed_block_stream(&self, alias: &str, handle: &Handle) -> Result<ManagedBlockStream> {
        let client = self
            .torii_client(alias)
            .ok_or_else(|| SupervisorError::PeerUnknown {
                alias: alias.to_owned(),
            })?;
        Ok(ManagedBlockStream::spawn(handle, alias.to_owned(), client))
    }

    /// Create a managed event stream handle for the specified peer using the provided runtime.
    pub fn managed_event_stream(&self, alias: &str, handle: &Handle) -> Result<ManagedEventStream> {
        let client = self
            .torii_client(alias)
            .ok_or_else(|| SupervisorError::PeerUnknown {
                alias: alias.to_owned(),
            })?;
        Ok(ManagedEventStream::spawn(handle, alias.to_owned(), client))
    }

    /// Refresh peer process state by polling for exited children.
    pub fn refresh_peer_states(&mut self) {
        if let Ok(path) = self.irohad_path() {
            self.refresh_peer_states_with(&path);
        }
    }

    fn readiness_smoke_signer(&self) -> Result<SigningAuthority> {
        let domain: DomainId = SMOKE_ACCOUNT_DOMAIN.parse().map_err(|err| {
            SupervisorError::Config(format!(
                "invalid readiness smoke account domain `{SMOKE_ACCOUNT_DOMAIN}`: {err}"
            ))
        })?;
        let account = self.genesis.account_in_domain(&domain);
        Ok(SigningAuthority::new(
            format!("Genesis readiness ({SMOKE_ACCOUNT_DOMAIN})"),
            account,
            self.genesis.key_pair().clone(),
        ))
    }

    /// Paths to the binaries used by the supervisor.
    pub fn binaries(&self) -> &BinaryPaths {
        &self.binaries
    }

    fn run_compatibility_checks(&mut self) -> Result<()> {
        let versions = self.binaries.probe_versions()?;
        if self.genesis.profile.is_some() {
            for info in &versions {
                if info.build_line != BuildLine::Iroha3 {
                    return Err(SupervisorError::BuildLineMismatch {
                        binary: info.name.to_owned(),
                        expected: BuildLine::Iroha3,
                        found: info.build_line,
                    });
                }
            }
            if let Some(report) = &self.genesis.verify_report {
                if let Some(chain_id) = &report.chain_id
                    && chain_id != &self.chain_id
                {
                    return Err(SupervisorError::KagamiVerify(format!(
                        "kagami verify reported chain `{chain_id}` but supervisor is configured for `{}`",
                        self.chain_id
                    )));
                }
                if let Some(expected_seed) = &self.genesis.vrf_seed_hex
                    && let Some(observed_seed) = &report.vrf_seed_hex
                    && observed_seed != expected_seed
                {
                    return Err(SupervisorError::KagamiVerify(format!(
                        "kagami verify reported vrf seed `{observed_seed}` but `{expected}` was configured",
                        expected = expected_seed
                    )));
                }
            }
        }

        self.compatibility = Some(CompatibilityReport {
            versions,
            verify: self.genesis.verify_report.clone(),
            chain_id: self.chain_id.clone(),
            profile: self.genesis.profile,
        });
        Ok(())
    }

    /// Start all peers managed by the supervisor.
    pub fn start_all(&mut self) -> Result<()> {
        self.run_compatibility_checks()?;
        let irohad_path = self.irohad_path()?;
        self.refresh_peer_states_with(&irohad_path);

        let mut started = Vec::new();
        for (idx, peer) in self.peers.iter_mut().enumerate() {
            match peer.start(&irohad_path, StartReason::Manual) {
                Ok(()) => started.push(idx),
                Err(err) => {
                    for index in started.into_iter().rev() {
                        let peer = &mut self.peers[index];
                        let _ = peer.stop();
                    }
                    return Err(err);
                }
            }
        }
        Ok(())
    }

    /// Stop all peers managed by the supervisor.
    pub fn stop_all(&mut self) -> Result<()> {
        let mut last_err = None;
        for peer in &mut self.peers {
            if let Err(err) = peer.stop()
                && !matches!(err, SupervisorError::PeerNotRunning { .. })
            {
                last_err = Some(err);
            }
        }
        if let Some(err) = last_err {
            Err(err)
        } else {
            Ok(())
        }
    }

    /// Check whether any peer process is currently running.
    pub fn is_any_running(&self) -> bool {
        self.peers.iter().any(PeerHandle::is_running)
    }

    /// Start a single peer by alias.
    pub fn start_peer(&mut self, alias: &str) -> Result<()> {
        self.run_compatibility_checks()?;
        let index = self
            .peers
            .iter()
            .position(|peer| peer.alias() == alias)
            .ok_or_else(|| SupervisorError::PeerUnknown {
                alias: alias.to_owned(),
            })?;
        let irohad_path = self.irohad_path()?;
        self.refresh_peer_states_with(&irohad_path);
        let peer = self
            .peers
            .get_mut(index)
            .expect("peer index should remain valid");
        peer.start(&irohad_path, StartReason::Manual)
    }

    /// Stop a single peer by alias.
    pub fn stop_peer(&mut self, alias: &str) -> Result<()> {
        let index = self
            .peers
            .iter()
            .position(|peer| peer.alias() == alias)
            .ok_or_else(|| SupervisorError::PeerUnknown {
                alias: alias.to_owned(),
            })?;
        let peer = self
            .peers
            .get_mut(index)
            .expect("peer index should remain valid");
        peer.stop()
    }

    /// Export the current network state into a timestamped snapshot directory.
    ///
    /// The snapshot contains peer storage directories, rendered configs, and
    /// the latest genesis manifest so users can quickly restore the network to
    /// its present state.
    pub fn export_snapshot(&mut self, label: Option<&str>) -> Result<PathBuf> {
        if self.is_any_running() {
            self.stop_all()?;
        }

        let root = self.paths.snapshots_dir();
        fs::create_dir_all(&root)?;

        let snapshot_name = label
            .and_then(sanitize_snapshot_label)
            .unwrap_or_else(default_snapshot_slug);
        let destination = root.join(&snapshot_name);
        if destination.exists() {
            return Err(SupervisorError::SnapshotExists {
                name: snapshot_name,
                root,
            });
        }

        fs::create_dir_all(&destination)?;
        let peers_root = destination.join("peers");
        fs::create_dir_all(&peers_root)?;

        for peer in &self.peers {
            let alias = peer.alias();
            let alias_dir = peers_root.join(alias);
            fs::create_dir_all(&alias_dir)?;

            let storage_dst = alias_dir.join("storage");
            copy_dir_recursive(peer.storage_dir(), &storage_dst)?;

            let snapshot_dst = alias_dir.join("snapshot");
            copy_dir_recursive(peer.snapshot_dir(), &snapshot_dst)?;

            let config_dst = alias_dir.join("config.toml");
            if let Some(parent) = config_dst.parent() {
                fs::create_dir_all(parent)?;
            }
            if peer.config_path().exists() {
                fs::copy(peer.config_path(), &config_dst)?;
            }

            let log_dst = alias_dir.join("latest.log");
            if peer.log_path().exists() {
                fs::copy(peer.log_path(), &log_dst)?;
            }
        }

        let genesis_dir = destination.join("genesis");
        fs::create_dir_all(&genesis_dir)?;
        if !self.genesis_manifest().exists() {
            return Err(SupervisorError::Config(format!(
                "missing genesis manifest `{}`; cannot export snapshot",
                self.genesis_manifest().display()
            )));
        }
        fs::copy(self.genesis_manifest(), genesis_dir.join(GENESIS_FILE_NAME))?;

        let genesis_hash = Hash::new(fs::read(self.genesis_manifest())?);

        let mut kura_hashes = Map::new();
        for peer in &self.peers {
            let hash = hash_directory(peer.storage_dir())?;
            kura_hashes.insert(peer.alias().to_owned(), Value::String(hash.to_string()));
        }

        let mut metadata = Map::new();
        metadata.insert("chain_id".to_owned(), Value::String(self.chain_id.clone()));
        metadata.insert(
            "created_at_ms".to_owned(),
            Value::Number((timestamp_ms() as u64).into()),
        );
        metadata.insert(
            "peer_count".to_owned(),
            Value::Number((self.peers.len() as u64).into()),
        );
        metadata.insert("snapshot".to_owned(), Value::String(snapshot_name));
        metadata.insert(
            "genesis_hash".to_owned(),
            Value::String(genesis_hash.to_string()),
        );
        metadata.insert("kura_hashes".to_owned(), Value::Object(kura_hashes));

        fs::write(
            destination.join("metadata.json"),
            json::to_vec_pretty(&Value::Object(metadata))?,
        )?;

        Ok(destination)
    }

    /// Restore a previously exported snapshot, rehydrating peer storage,
    /// snapshot directories, logs, and genesis manifests.
    pub fn restore_snapshot<P: AsRef<Path>>(&mut self, snapshot: P) -> Result<PathBuf> {
        let candidate = snapshot.as_ref();
        let snapshot_root = if candidate.is_absolute() {
            candidate.to_path_buf()
        } else {
            let under_root = self.paths.snapshots_dir().join(candidate);
            if under_root.exists() {
                under_root
            } else {
                candidate.to_path_buf()
            }
        };
        if !snapshot_root.exists() {
            return Err(SupervisorError::Config(format!(
                "snapshot `{}` does not exist",
                snapshot_root.display()
            )));
        }
        if !snapshot_root.is_dir() {
            return Err(SupervisorError::Config(format!(
                "snapshot `{}` is not a directory",
                snapshot_root.display()
            )));
        }

        let metadata = load_snapshot_metadata(&snapshot_root)?;
        if metadata.chain_id != self.chain_id {
            return Err(SupervisorError::Config(format!(
                "snapshot `{}` targets chain `{}` but the supervisor is configured for `{}`",
                snapshot_root.display(),
                metadata.chain_id,
                self.chain_id
            )));
        }
        let expected_peers = self.peers.len() as u64;
        if metadata.peer_count != expected_peers {
            return Err(SupervisorError::Config(format!(
                "snapshot `{}` recorded {recorded} peer(s) but the supervisor manages {expected_peers}",
                snapshot_root.display(),
                recorded = metadata.peer_count
            )));
        }

        let peers_root = snapshot_root.join("peers");
        if !peers_root.is_dir() {
            return Err(SupervisorError::Config(format!(
                "snapshot `{}` missing `peers/` directory",
                snapshot_root.display()
            )));
        }

        let genesis_src = snapshot_root.join("genesis").join(GENESIS_FILE_NAME);
        if !genesis_src.exists() {
            return Err(SupervisorError::Config(format!(
                "snapshot `{}` missing `genesis/{GENESIS_FILE_NAME}`",
                snapshot_root.display()
            )));
        }
        if !self.genesis_manifest().exists() {
            return Err(SupervisorError::Config(format!(
                "supervisor is missing genesis manifest `{}`",
                self.genesis_manifest().display()
            )));
        }
        let snapshot_genesis_hash = Hash::new(fs::read(&genesis_src)?);
        if snapshot_genesis_hash != metadata.genesis_hash {
            return Err(SupervisorError::Config(format!(
                "snapshot `{}` genesis hash {} does not match recorded metadata {}; refusing restore",
                snapshot_root.display(),
                snapshot_genesis_hash,
                metadata.genesis_hash
            )));
        }
        let current_genesis_hash = Hash::new(fs::read(self.genesis_manifest())?);
        if snapshot_genesis_hash != current_genesis_hash {
            return Err(SupervisorError::Config(format!(
                "snapshot `{}` genesis hash {} does not match current genesis hash {}; refusing restore",
                snapshot_root.display(),
                snapshot_genesis_hash,
                current_genesis_hash
            )));
        }

        for peer in &self.peers {
            let alias_dir = peers_root.join(peer.alias());
            if !alias_dir.is_dir() {
                return Err(SupervisorError::Config(format!(
                    "snapshot `{}` missing directory for peer `{}`",
                    snapshot_root.display(),
                    peer.alias()
                )));
            }
            let expected_hash = metadata.kura_hashes.get(peer.alias()).ok_or_else(|| {
                SupervisorError::Config(format!(
                    "snapshot `{}` metadata missing kura hash for peer `{}`",
                    snapshot_root.display(),
                    peer.alias()
                ))
            })?;
            let storage_src = alias_dir.join("storage");
            let actual_hash = hash_directory(&storage_src)?;
            if &actual_hash != expected_hash {
                return Err(SupervisorError::Config(format!(
                    "snapshot `{}` storage for peer `{}` failed integrity check: expected {} but found {}",
                    snapshot_root.display(),
                    peer.alias(),
                    expected_hash,
                    actual_hash
                )));
            }
        }

        let was_running = self.is_any_running();
        if was_running {
            self.stop_all()?;
        }

        for peer in &mut self.peers {
            let alias_dir = peers_root.join(peer.alias());
            peer.wipe_storage()?;

            copy_dir_recursive(&alias_dir.join("storage"), peer.storage_dir())?;
            copy_dir_recursive(&alias_dir.join("snapshot"), peer.snapshot_dir())?;
            copy_file_if_exists(&alias_dir.join("config.toml"), peer.config_path())?;
            copy_file_if_exists(&alias_dir.join("latest.log"), peer.log_path())?;
        }

        if let Some(parent) = self.genesis_manifest().parent() {
            fs::create_dir_all(parent)?;
        }
        fs::copy(&genesis_src, self.genesis_manifest())?;

        if was_running {
            self.start_all()?;
        }

        Ok(snapshot_root)
    }

    /// Wipe peer storage and regenerate the default genesis manifest.
    ///
    /// If peers were running before this call they are restarted afterwards.
    pub fn wipe_and_regenerate(&mut self) -> Result<()> {
        let was_running = self.is_any_running();
        if was_running {
            self.stop_all()?;
        }

        let mut specs = Vec::with_capacity(self.peers.len());
        for peer in &mut self.peers {
            peer.wipe_storage()?;
            specs.push(peer.spec.clone());
        }

        let genesis = GenesisMaterial::create(
            &mut self.binaries,
            &self.paths,
            &self.chain_id,
            &specs,
            self.profile.consensus_mode,
            self.genesis.profile,
            self.genesis.vrf_seed_hex.as_deref(),
        )?;
        for spec in &specs {
            spec.write_config(
                &self.chain_id,
                &genesis,
                &specs,
                &self.peer_config_overrides,
            )?;
        }

        for (peer, spec) in self.peers.iter_mut().zip(specs.into_iter()) {
            peer.replace_spec(spec);
        }
        self.genesis = genesis;
        self.compatibility = None;

        if was_running {
            self.start_all()?;
        }

        Ok(())
    }

    /// Wipe per-lane storage segments for the requested lane id.
    ///
    /// If peers were running before this call they are restarted afterwards.
    pub fn reset_lane_storage(&mut self, lane_id: u32) -> Result<()> {
        let alias = lane_alias_for_id(self.peer_config_overrides.nexus.as_ref(), lane_id)?;
        let was_running = self.is_any_running();
        if was_running {
            self.stop_all()?;
        }

        for peer in &self.peers {
            peer.wipe_lane_storage(lane_id, &alias)?;
        }

        if was_running {
            self.start_all()?;
        }

        Ok(())
    }
}

impl Drop for Supervisor {
    fn drop(&mut self) {
        let _ = self.stop_all();
    }
}

/// Lightweight metadata and state for a child `irohad` process.
pub struct PeerHandle {
    spec: PeerSpec,
    log_path: PathBuf,
    process: Option<Child>,
    state: PeerState,
    log_stream: PeerLogStream,
    torii_client: OnceCell<ToriiClient>,
    log_threads: Vec<JoinHandle<()>>,
    log_file: Option<Arc<Mutex<fs::File>>>,
    restart_policy: RestartPolicy,
    restart_attempts: usize,
    next_restart_at: Option<Instant>,
    manual_stop: bool,
}

#[derive(Debug, Clone, Copy)]
enum StartReason {
    Manual,
    Restart { attempt: usize },
}

impl StartReason {
    fn attempt(self) -> usize {
        match self {
            StartReason::Manual => 0,
            StartReason::Restart { attempt } => attempt,
        }
    }
}

impl std::fmt::Debug for PeerHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PeerHandle")
            .field("alias", &self.spec.alias)
            .field("config_path", &self.spec.config_path)
            .field("state", &self.state)
            .field("log_path", &self.log_path)
            .finish()
    }
}

impl PeerHandle {
    fn prepared(spec: PeerSpec, logs_dir: PathBuf, restart_policy: RestartPolicy) -> Self {
        let alias = spec.alias.clone();
        let log_path = logs_dir.join(format!("{alias}.log"));
        Self {
            spec,
            log_path,
            process: None,
            state: PeerState::Prepared,
            log_stream: PeerLogStream::new(alias),
            torii_client: OnceCell::new(),
            log_threads: Vec::new(),
            log_file: None,
            restart_policy,
            restart_attempts: 0,
            next_restart_at: None,
            manual_stop: false,
        }
    }

    fn is_running(&self) -> bool {
        matches!(self.state, PeerState::Running | PeerState::Restarting)
    }

    fn storage_dir(&self) -> &Path {
        &self.spec.storage_dir
    }

    fn snapshot_dir(&self) -> &Path {
        &self.spec.snapshot_dir
    }

    fn wipe_storage(&self) -> Result<()> {
        if self.is_running() {
            return Err(SupervisorError::PeerStillRunning {
                alias: self.spec.alias.clone(),
            });
        }
        if self.storage_dir().exists() {
            fs::remove_dir_all(self.storage_dir())?;
        }
        fs::create_dir_all(self.storage_dir())?;
        fs::create_dir_all(self.snapshot_dir())?;
        Ok(())
    }

    fn wipe_lane_storage(&self, lane_id: u32, alias: &str) -> Result<()> {
        if self.is_running() {
            return Err(SupervisorError::PeerStillRunning {
                alias: self.spec.alias.clone(),
            });
        }
        let slug = lane_slug(alias, lane_id);
        let blocks_dir = self
            .storage_dir()
            .join("blocks")
            .join(format!("lane_{lane_id:03}_{slug}"));
        let merge_log = self
            .storage_dir()
            .join("merge_ledger")
            .join(format!("lane_{lane_id:03}_{slug}_merge.log"));
        if blocks_dir.exists() {
            fs::remove_dir_all(&blocks_dir)?;
        }
        if merge_log.exists() {
            fs::remove_file(&merge_log)?;
        }
        Ok(())
    }

    fn replace_spec(&mut self, spec: PeerSpec) {
        self.spec = spec;
    }

    /// Stable identifier for the peer (e.g. `peer0`).
    pub fn alias(&self) -> &str {
        &self.spec.alias
    }

    /// Peer identifier derived from the generated key pair.
    pub fn peer_id(&self) -> PeerId {
        self.spec.peer_id()
    }

    /// Public Torii address advertised to clients.
    pub fn torii_address(&self) -> &str {
        &self.spec.torii_public
    }

    /// Public P2P address advertised to peers.
    pub fn p2p_address(&self) -> &str {
        &self.spec.p2p_public
    }

    /// Location of the rendered configuration file.
    pub fn config_path(&self) -> &Path {
        &self.spec.config_path
    }

    /// Path to the log file capturing stdout/stderr.
    pub fn log_path(&self) -> &Path {
        &self.log_path
    }

    /// Structured log stream associated with the peer.
    pub fn log_stream(&self) -> PeerLogStream {
        self.log_stream.clone()
    }

    /// Current lifecycle state for the peer.
    pub fn state(&self) -> PeerState {
        self.state
    }

    /// Generate a Torii client targeting this peer.
    pub fn torii_client(&self) -> ToriiResult<ToriiClient> {
        if let Some(client) = self.torii_client.get() {
            return Ok(client.clone());
        }
        let client = ToriiClient::new(self.spec.torii_base_http())?;
        if self.torii_client.set(client.clone()).is_ok() {
            Ok(client)
        } else {
            // Cell already initialised by another thread; fall back to the stored value.
            Ok(self.torii_client.get().cloned().unwrap_or(client))
        }
    }

    fn start(&mut self, irohad: &Path, reason: StartReason) -> Result<()> {
        match self.state {
            PeerState::Running | PeerState::Restarting => {
                return Err(SupervisorError::PeerAlreadyRunning {
                    alias: self.spec.alias.clone(),
                });
            }
            PeerState::Prepared | PeerState::Stopped => {}
        }

        self.manual_stop = false;
        if matches!(reason, StartReason::Manual) {
            self.restart_attempts = 0;
            self.next_restart_at = None;
        }

        self.teardown_log_threads();

        fs::create_dir_all(self.log_path.parent().unwrap_or_else(|| Path::new(".")))?;
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.log_path)?;
        let log_file = Arc::new(Mutex::new(file));
        self.log_file = Some(log_file.clone());

        let mut command = Command::new(irohad);
        command
            .arg("--config")
            .arg(&self.spec.config_path)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut child = command.spawn().map_err(|err| {
            let source = match err.kind() {
                io::ErrorKind::NotFound => {
                    let message = format!(
                        "{} (looked for `{}`); build `irohad` with \
                         `cargo build -p irohad` or set `MOCHI_IROHAD`/`binaries.irohad` \
                         to an absolute path",
                        err,
                        irohad.display()
                    );
                    io::Error::new(io::ErrorKind::NotFound, message)
                }
                _ => err,
            };
            SupervisorError::Spawn {
                alias: self.spec.alias.clone(),
                source,
            }
        })?;

        let stdout = child.stdout.take().ok_or_else(|| SupervisorError::Spawn {
            alias: self.spec.alias.clone(),
            source: io::Error::other("failed to capture stdout"),
        })?;
        let stderr = child.stderr.take().ok_or_else(|| SupervisorError::Spawn {
            alias: self.spec.alias.clone(),
            source: io::Error::other("failed to capture stderr"),
        })?;

        let stdout_stream = self.log_stream.clone();
        let stderr_stream = self.log_stream.clone();
        let stdout_handle = spawn_log_forwarder(
            LogStreamKind::Stdout,
            stdout,
            log_file.clone(),
            stdout_stream,
        );
        let stderr_handle = spawn_log_forwarder(
            LogStreamKind::Stderr,
            stderr,
            log_file.clone(),
            stderr_stream,
        );
        self.log_threads = vec![stdout_handle, stderr_handle];

        self.process = Some(child);
        self.state = PeerState::Running;
        let attempt = reason.attempt();
        self.emit_lifecycle_event(LifecycleEvent::Started { attempt });

        Ok(())
    }

    fn stop(&mut self) -> Result<()> {
        match (&mut self.process, self.state) {
            (Some(child), PeerState::Running) => {
                self.manual_stop = true;
                if let Err(err) = child.kill()
                    && err.kind() != std::io::ErrorKind::InvalidInput
                {
                    return Err(SupervisorError::Terminate {
                        alias: self.spec.alias.clone(),
                        source: err,
                    });
                }
                match child.wait() {
                    Ok(_) => {}
                    Err(err) => {
                        return Err(SupervisorError::Wait {
                            alias: self.spec.alias.clone(),
                            source: err,
                        });
                    }
                }
                self.process = None;
                self.teardown_log_threads();
                self.state = PeerState::Stopped;
                self.restart_attempts = 0;
                self.next_restart_at = None;
                self.emit_lifecycle_event(LifecycleEvent::StoppedByUser);
                self.manual_stop = false;
                Ok(())
            }
            (None, PeerState::Restarting) => {
                // Cancel a pending restart requested by the supervisor.
                self.restart_attempts = 0;
                self.next_restart_at = None;
                self.state = PeerState::Stopped;
                self.manual_stop = false;
                self.emit_lifecycle_event(LifecycleEvent::StoppedByUser);
                Ok(())
            }
            (None, PeerState::Running | PeerState::Prepared | PeerState::Stopped) => {
                Err(SupervisorError::PeerNotRunning {
                    alias: self.spec.alias.clone(),
                })
            }
            (Some(_), _) => {
                self.process = None;
                self.teardown_log_threads();
                self.state = PeerState::Stopped;
                Ok(())
            }
        }
    }

    fn refresh_state(&mut self, irohad: &Path) {
        if let (PeerState::Running, Some(child)) = (self.state, self.process.as_mut())
            && let Ok(Some(status)) = child.try_wait()
        {
            self.process = None;
            self.teardown_log_threads();
            let code = status.code();
            let success = status.success();
            self.emit_lifecycle_event(LifecycleEvent::Exited { code, success });
            self.state = PeerState::Stopped;

            if self.manual_stop {
                self.manual_stop = false;
                self.restart_attempts = 0;
                self.next_restart_at = None;
                return;
            }

            if success {
                self.restart_attempts = 0;
                self.next_restart_at = None;
            } else {
                self.schedule_restart(irohad);
            }
        }

        if matches!(self.state, PeerState::Restarting | PeerState::Stopped)
            && let Some(instant) = self.next_restart_at
            && Instant::now() >= instant
        {
            let attempt = self.restart_attempts;
            match self.start(irohad, StartReason::Restart { attempt }) {
                Ok(()) => {
                    self.state = PeerState::Running;
                    self.next_restart_at = None;
                    self.emit_lifecycle_event(LifecycleEvent::RestartSucceeded { attempt });
                }
                Err(err) => {
                    self.emit_lifecycle_event(LifecycleEvent::RestartFailed {
                        attempt,
                        error: err.to_string(),
                    });
                    self.schedule_restart(irohad);
                }
            }
        }
    }

    fn schedule_restart(&mut self, irohad: &Path) {
        let next_attempt = self.restart_attempts + 1;
        if !self.restart_policy.should_retry(next_attempt) {
            self.emit_lifecycle_event(LifecycleEvent::RestartAborted {
                attempt: next_attempt,
            });
            self.restart_attempts = 0;
            self.next_restart_at = None;
            self.state = PeerState::Stopped;
            return;
        }

        let delay = self.restart_policy.backoff_for(next_attempt);
        self.set_restart_timer(next_attempt, delay);

        if delay.is_zero() {
            let attempt = self.restart_attempts;
            match self.start(irohad, StartReason::Restart { attempt }) {
                Ok(()) => {
                    self.state = PeerState::Running;
                    self.next_restart_at = None;
                    self.emit_lifecycle_event(LifecycleEvent::RestartSucceeded { attempt });
                }
                Err(err) => {
                    self.emit_lifecycle_event(LifecycleEvent::RestartFailed {
                        attempt,
                        error: err.to_string(),
                    });

                    let upcoming_attempt = attempt + 1;
                    if self.restart_policy.should_retry(upcoming_attempt) {
                        let next_delay = self.restart_policy.backoff_for(upcoming_attempt);
                        self.set_restart_timer(upcoming_attempt, next_delay);
                    } else {
                        self.emit_lifecycle_event(LifecycleEvent::RestartAborted {
                            attempt: upcoming_attempt,
                        });
                        self.restart_attempts = 0;
                        self.next_restart_at = None;
                        self.state = PeerState::Stopped;
                    }
                }
            }
        }
    }

    fn teardown_log_threads(&mut self) {
        for handle in self.log_threads.drain(..) {
            let _ = handle.join();
        }
        self.log_file = None;
    }

    fn set_restart_timer(&mut self, attempt: usize, delay: Duration) {
        self.restart_attempts = attempt;
        self.state = PeerState::Restarting;
        self.next_restart_at = Some(Instant::now() + delay);
        self.emit_lifecycle_event(LifecycleEvent::RestartScheduled {
            attempt,
            delay_ms: delay.as_millis().min(u128::from(u64::MAX)) as u64,
        });
    }

    fn emit_lifecycle_event(&self, event: LifecycleEvent) {
        if let Some(file) = &self.log_file {
            let message = format_lifecycle(&event);
            write_log_record(file, LogStreamKind::System, &message);
        }
        self.log_stream.emit_lifecycle(event);
    }
}

/// Lifecycle state for a peer managed by the supervisor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PeerState {
    /// Files and configuration exist, process has not been launched.
    Prepared,
    /// Process is running.
    Running,
    /// Process is waiting for an automatic restart attempt.
    Restarting,
    /// Process was stopped or exited.
    Stopped,
}

impl PeerState {
    /// Human-readable label for UI presentation.
    pub fn label(self) -> &'static str {
        match self {
            PeerState::Prepared => "Prepared",
            PeerState::Running => "Running",
            PeerState::Restarting => "Restarting",
            PeerState::Stopped => "Stopped",
        }
    }
}

fn write_log_record(file: &Arc<Mutex<fs::File>>, kind: LogStreamKind, message: &str) {
    if let Ok(mut guard) = file.lock() {
        let sanitized = sanitize_message(message);
        let _ = writeln!(guard, "{}|{}|{}", timestamp_ms(), kind, sanitized);
    }
}

fn sanitize_message(message: &str) -> String {
    message.replace(['\n', '\r'], " ")
}

fn format_lifecycle(event: &LifecycleEvent) -> String {
    match event {
        LifecycleEvent::Started { attempt } if *attempt == 0 => "started".to_owned(),
        LifecycleEvent::Started { attempt } => format!("started (restart attempt={attempt})"),
        LifecycleEvent::Exited { code, success } => match code {
            Some(code) => format!("exited with code {code} (success={success})"),
            None => format!("exited without code (success={success})"),
        },
        LifecycleEvent::RestartScheduled { attempt, delay_ms } => {
            format!("restart scheduled (attempt={attempt}, delay_ms={delay_ms})")
        }
        LifecycleEvent::RestartSucceeded { attempt } => {
            format!("restart succeeded (attempt={attempt})")
        }
        LifecycleEvent::RestartFailed { attempt, error } => {
            format!("restart failed (attempt={attempt}): {error}")
        }
        LifecycleEvent::RestartAborted { attempt } => {
            format!("restart aborted after attempt={attempt}")
        }
        LifecycleEvent::StoppedByUser => "stopped by user".to_owned(),
    }
}

fn spawn_log_forwarder<R>(
    kind: LogStreamKind,
    reader: R,
    file: Arc<Mutex<fs::File>>,
    stream: PeerLogStream,
) -> JoinHandle<()>
where
    R: Read + Send + 'static,
{
    thread::spawn(move || {
        let mut reader = BufReader::new(reader);
        let mut buffer = String::new();
        loop {
            buffer.clear();
            match reader.read_line(&mut buffer) {
                Ok(0) => break,
                Ok(_) => {
                    let line = buffer.trim_end_matches(&['\r', '\n'][..]).to_owned();
                    write_log_record(&file, kind, &line);
                    stream.emit_line(kind, line);
                }
                Err(err) => {
                    let message = format!("log forwarder error: {err}");
                    write_log_record(&file, LogStreamKind::System, &message);
                    stream.emit_line(LogStreamKind::System, message);
                    break;
                }
            }
        }
    })
}

#[derive(Debug, Clone)]
struct PeerSpec {
    alias: String,
    torii_bind: String,
    torii_public: String,
    p2p_bind: String,
    p2p_public: String,
    config_path: PathBuf,
    storage_dir: PathBuf,
    snapshot_dir: PathBuf,
    keys: PeerKeys,
}

impl PeerSpec {
    fn new(paths: &NetworkPaths, alias: String, torii_port: u16, p2p_port: u16) -> Result<Self> {
        let peer_dir = paths.peer_dir(&alias);
        if peer_dir.exists() {
            fs::remove_dir_all(&peer_dir)?;
        }
        fs::create_dir_all(&peer_dir)?;

        let storage_dir = peer_dir.join("storage");
        fs::create_dir_all(&storage_dir)?;

        let snapshot_dir = storage_dir.join("snapshot");
        fs::create_dir_all(&snapshot_dir)?;

        let config_path = peer_dir.join("config.toml");

        let key_pair = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let (public_key, private_key) = key_pair.into_parts();
        let pop = bls_normal_pop_prove(&private_key)
            .map_err(|err| std::io::Error::other(err.to_string()))?;

        Ok(Self {
            alias,
            torii_bind: format!("0.0.0.0:{torii_port}"),
            torii_public: format!("127.0.0.1:{torii_port}"),
            p2p_bind: format!("0.0.0.0:{p2p_port}"),
            p2p_public: format!("127.0.0.1:{p2p_port}"),
            config_path,
            storage_dir,
            snapshot_dir,
            keys: PeerKeys {
                public_key,
                private_key: ExposedPrivateKey(private_key),
                pop,
            },
        })
    }

    fn write_config(
        &self,
        chain_id: &str,
        genesis: &GenesisMaterial,
        all_peers: &[PeerSpec],
        config_overrides: &PeerConfigOverrides,
    ) -> Result<()> {
        let mut root = toml::Table::new();
        root.insert("chain".into(), toml::Value::String(chain_id.to_owned()));
        root.insert(
            "public_key".into(),
            toml::Value::String(self.keys.public_key.to_string()),
        );
        root.insert(
            "private_key".into(),
            toml::Value::String(self.keys.private_key.to_string()),
        );

        let trusted = all_peers
            .iter()
            .map(|peer| toml::Value::String(peer.trusted_peer_entry()))
            .collect();
        root.insert("trusted_peers".into(), toml::Value::Array(trusted));
        let trusted_pops = all_peers
            .iter()
            .map(|peer| {
                let mut entry = toml::Table::new();
                entry.insert(
                    "public_key".into(),
                    toml::Value::String(peer.keys.public_key.to_string()),
                );
                entry.insert(
                    "pop_hex".into(),
                    toml::Value::String(encode_hex(peer.pop_bytes())),
                );
                toml::Value::Table(entry)
            })
            .collect();
        root.insert("trusted_peers_pop".into(), toml::Value::Array(trusted_pops));

        let mut network = toml::Table::new();
        network.insert("address".into(), toml::Value::String(self.p2p_bind.clone()));
        network.insert(
            "public_address".into(),
            toml::Value::String(self.p2p_public.clone()),
        );
        root.insert("network".into(), toml::Value::Table(network));

        let mut torii = toml::Table::new();
        if let Some(overrides) = config_overrides.torii.as_ref() {
            merge_table(&mut torii, overrides);
        }
        torii.insert(
            "address".into(),
            toml::Value::String(self.torii_bind.clone()),
        );
        let torii_dir = self.storage_dir.join("torii");
        torii.insert(
            "data_dir".into(),
            toml::Value::String(torii_dir.display().to_string()),
        );
        if config_overrides.da_enabled() {
            let entry = torii
                .entry("da_ingest")
                .or_insert_with(|| toml::Value::Table(toml::Table::new()));
            if let toml::Value::Table(da_ingest) = entry {
                if !da_ingest.contains_key("replay_cache_store_dir") {
                    da_ingest.insert(
                        "replay_cache_store_dir".into(),
                        toml::Value::String(torii_dir.join("da_replay").display().to_string()),
                    );
                }
                if !da_ingest.contains_key("manifest_store_dir") {
                    da_ingest.insert(
                        "manifest_store_dir".into(),
                        toml::Value::String(torii_dir.join("da_manifests").display().to_string()),
                    );
                }
            }
        }
        root.insert("torii".into(), toml::Value::Table(torii));

        let mut genesis_table = toml::Table::new();
        genesis_table.insert(
            "public_key".into(),
            toml::Value::String(genesis.public_key().to_string()),
        );
        genesis_table.insert(
            "file".into(),
            toml::Value::String(genesis.manifest_path.display().to_string()),
        );
        root.insert("genesis".into(), toml::Value::Table(genesis_table));

        let mut kura = toml::Table::new();
        kura.insert(
            "store_dir".into(),
            toml::Value::String(self.storage_dir.display().to_string()),
        );
        root.insert("kura".into(), toml::Value::Table(kura));

        let mut snapshot = toml::Table::new();
        snapshot.insert(
            "store_dir".into(),
            toml::Value::String(self.snapshot_dir.display().to_string()),
        );
        root.insert("snapshot".into(), toml::Value::Table(snapshot));

        if let Some(table) = config_overrides.sumeragi.as_ref()
            && !table.is_empty()
        {
            root.insert("sumeragi".into(), toml::Value::Table(table.clone()));
        }

        if let Some(table) = config_overrides.nexus.as_ref()
            && !table.is_empty()
        {
            root.insert("nexus".into(), toml::Value::Table(table.clone()));
        }

        let header = Self::config_header(
            chain_id,
            genesis,
            &self.storage_dir,
            config_overrides.nexus.as_ref(),
        );
        let config_str = toml::to_string_pretty(&toml::Value::Table(root))?;
        let rendered = format!("{header}\n\n{config_str}");
        fs::write(&self.config_path, rendered)?;
        Ok(())
    }

    fn config_header(
        chain_id: &str,
        genesis: &GenesisMaterial,
        storage_root: &Path,
        nexus: Option<&toml::Table>,
    ) -> String {
        let mut lines = Vec::new();
        lines.push(format!("# mochi.chain_id = {chain_id}"));
        if let Some(fingerprint) = genesis.consensus_fingerprint.as_deref() {
            lines.push(format!("# mochi.consensus_fingerprint = {fingerprint}"));
        }
        lines.extend(lane_path_comments(storage_root, nexus));
        lines.join("\n")
    }

    fn trusted_peer_entry(&self) -> String {
        format!("{}@{}", self.keys.public_key, self.p2p_public)
    }

    fn peer_id(&self) -> PeerId {
        self.keys.public_key.clone().into()
    }

    fn pop_bytes(&self) -> &[u8] {
        &self.keys.pop
    }

    fn torii_base_http(&self) -> String {
        format!("http://{}", self.torii_public)
    }
}

#[derive(Debug, Clone)]
struct PeerKeys {
    public_key: PublicKey,
    private_key: ExposedPrivateKey,
    pop: Vec<u8>,
}

#[derive(Debug)]
struct GenesisMaterial {
    key_pair: KeyPair,
    manifest_path: PathBuf,
    profile: Option<GenesisProfile>,
    vrf_seed_hex: Option<String>,
    verify_report: Option<KagamiVerifyReport>,
    consensus_fingerprint: Option<String>,
}

impl GenesisMaterial {
    fn create(
        binaries: &mut BinaryPaths,
        paths: &NetworkPaths,
        chain_id: &str,
        peers: &[PeerSpec],
        consensus_mode: SumeragiConsensusMode,
        genesis_profile: Option<GenesisProfile>,
        vrf_seed_hex: Option<&str>,
    ) -> Result<Self> {
        let genesis_dir = paths.genesis_dir();
        fs::create_dir_all(&genesis_dir)?;
        let manifest_path = genesis_dir.join(GENESIS_FILE_NAME);

        let key_pair = KeyPair::random();
        let manifest = Self::generate_manifest(
            binaries,
            &genesis_dir,
            chain_id,
            key_pair.public_key(),
            consensus_mode,
            genesis_profile,
            vrf_seed_hex,
        )?;
        let topology: Vec<GenesisTopologyEntry> = peers
            .iter()
            .map(|spec| GenesisTopologyEntry::new(spec.peer_id(), spec.pop_bytes().to_vec()))
            .collect();
        let manifest = genesis::with_topology(manifest, topology);
        let json = norito::json::to_vec_pretty(&manifest)?;
        fs::write(&manifest_path, json)?;

        let verify_report = if let Some(profile) = genesis_profile {
            Some(Self::verify_manifest_with_kagami(
                binaries,
                &manifest_path,
                profile,
                vrf_seed_hex,
            )?)
        } else {
            None
        };
        let consensus_fingerprint = verify_report
            .as_ref()
            .and_then(|report| report.fingerprint.clone())
            .or_else(|| manifest.consensus_fingerprint().map(str::to_owned))
            .or_else(|| {
                let normalized = manifest.clone().with_consensus_meta();
                normalized.consensus_fingerprint().map(str::to_owned)
            });

        Ok(Self {
            key_pair,
            manifest_path,
            profile: genesis_profile,
            vrf_seed_hex: vrf_seed_hex.map(|value| value.to_owned()),
            verify_report,
            consensus_fingerprint,
        })
    }

    fn generate_manifest(
        binaries: &mut BinaryPaths,
        genesis_dir: &Path,
        chain_id: &str,
        genesis_public_key: &PublicKey,
        consensus_mode: SumeragiConsensusMode,
        genesis_profile: Option<GenesisProfile>,
        vrf_seed_hex: Option<&str>,
    ) -> Result<RawGenesisTransaction> {
        if std::env::var_os("MOCHI_TEST_USE_INTERNAL_GENESIS").is_some() {
            if let Some(profile) = genesis_profile {
                return Err(SupervisorError::KagamiInvocation(format!(
                    "MOCHI_TEST_USE_INTERNAL_GENESIS cannot satisfy genesis profile {profile:?}; unset the override to invoke kagami"
                )));
            }
            // Test-only escape hatch so integration harnesses can reuse the internal
            // genesis helper without shelling out to Kagami.
            let chain = ChainId::from(chain_id.to_owned());
            return crate::genesis::default_manifest(
                chain,
                genesis_public_key,
                genesis_dir.to_path_buf(),
                consensus_mode,
                None,
            )
            .map_err(|err| {
                SupervisorError::KagamiInvocation(format!(
                    "failed to generate internal genesis manifest: {err}"
                ))
            });
        }

        if let Some(profile) = genesis_profile
            && profile.requires_seed()
            && vrf_seed_hex.is_none()
        {
            return Err(SupervisorError::KagamiInvocation(format!(
                "profile {profile:?} requires `--vrf-seed-hex <hex>` when generating genesis"
            )));
        }
        if genesis_profile.is_none() && vrf_seed_hex.is_some() {
            return Err(SupervisorError::KagamiInvocation(
                "`--vrf-seed-hex` requires a genesis profile (NPoS mode)".into(),
            ));
        }

        let kagami = binaries.ensure_kagami_ready()?;
        let mut command = Command::new(kagami);
        command
            .current_dir(genesis_dir)
            .arg("genesis")
            .arg("generate")
            .arg("--ivm-dir")
            .arg(".")
            .arg("--genesis-public-key")
            .arg(genesis_public_key.to_string())
            .arg("--chain-id")
            .arg(chain_id);

        if let Some(profile) = genesis_profile {
            command.arg("--profile").arg(profile.as_kagami_arg());
        }
        if consensus_mode != SumeragiConsensusMode::Permissioned {
            command.arg("--consensus-mode").arg(match consensus_mode {
                SumeragiConsensusMode::Permissioned => "permissioned",
                SumeragiConsensusMode::Npos => "npos",
            });
        }
        if let Some(seed) = vrf_seed_hex {
            command.arg("--vrf-seed-hex").arg(seed);
        }
        command
            .arg("default")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let output = command.output().map_err(|err| {
            SupervisorError::KagamiInvocation(format!("failed to invoke `kagami`: {err}"))
        })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(SupervisorError::KagamiInvocation(format!(
                "`kagami` exited with status {}: {stderr}",
                output.status
            )));
        }

        if genesis_profile.is_some() && !output.stderr.is_empty() {
            eprintln!("{}", String::from_utf8_lossy(&output.stderr));
        }

        if output.stdout.is_empty() {
            return Err(SupervisorError::KagamiInvocation(
                "`kagami` did not emit genesis JSON".into(),
            ));
        }

        let mut value: Value = norito::json::from_slice(&output.stdout).map_err(|err| {
            SupervisorError::KagamiInvocation(format!(
                "failed to parse `kagami` JSON output: {err}"
            ))
        })?;
        let object = value.as_object_mut().ok_or_else(|| {
            SupervisorError::KagamiInvocation("`kagami` JSON payload must be an object".into())
        })?;
        object.insert("chain".into(), Value::String(chain_id.to_owned()));

        let manifest: RawGenesisTransaction = norito::json::from_value(value).map_err(|err| {
            SupervisorError::KagamiInvocation(format!(
                "failed to decode genesis manifest from `kagami` output: {err}"
            ))
        })?;
        Ok(manifest)
    }

    fn verify_manifest_with_kagami(
        binaries: &mut BinaryPaths,
        manifest_path: &Path,
        profile: GenesisProfile,
        vrf_seed_hex: Option<&str>,
    ) -> Result<KagamiVerifyReport> {
        let kagami = binaries.ensure_kagami_ready()?;
        let mut command = Command::new(kagami);
        command
            .arg("verify")
            .arg("--profile")
            .arg(profile.as_kagami_arg())
            .arg("--genesis")
            .arg(manifest_path);

        if let Some(seed) = vrf_seed_hex {
            command.arg("--vrf-seed-hex").arg(seed);
        }

        command
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let output = command.output().map_err(|err| {
            SupervisorError::KagamiInvocation(format!("failed to invoke `kagami verify`: {err}"))
        })?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        if !output.status.success() {
            return Err(SupervisorError::KagamiInvocation(format!(
                "`kagami verify` exited with status {}: {stderr}",
                output.status
            )));
        }

        let mut combined = stdout.trim_end().to_owned();
        let stderr_trimmed = stderr.trim_end();
        if !stderr_trimmed.is_empty() {
            if !combined.is_empty() {
                combined.push('\n');
            }
            combined.push_str(stderr_trimmed);
        }

        Ok(parse_kagami_verify_output(profile, vrf_seed_hex, &combined))
    }

    fn public_key(&self) -> &PublicKey {
        self.key_pair.public_key()
    }

    fn key_pair(&self) -> &KeyPair {
        &self.key_pair
    }

    fn account_in_domain(&self, domain: &DomainId) -> AccountId {
        AccountId::new(domain.clone(), self.key_pair.public_key().clone())
    }
}

fn parse_keyed_value(line: &str, keys: &[&str]) -> Option<String> {
    let lower = line.to_ascii_lowercase();
    for key in keys {
        if let Some(start) = lower.find(key) {
            let remainder = &line[start + key.len()..];
            if let Some((_, value)) = remainder
                .split_once(':')
                .or_else(|| remainder.split_once('='))
            {
                let trimmed = value.trim().trim_matches([',', ';']);
                if !trimmed.is_empty() {
                    return Some(trimmed.to_owned());
                }
            }
        }
    }

    for token in line.split(|c: char| c.is_whitespace() || c == ',' || c == ';') {
        if let Some((key, value)) = token.split_once(['=', ':'])
            && keys
                .iter()
                .any(|candidate| key.eq_ignore_ascii_case(candidate))
        {
            let trimmed = value.trim().trim_matches([',', ';']);
            if !trimmed.is_empty() {
                return Some(trimmed.to_owned());
            }
        }
    }
    None
}

fn parse_kagami_verify_output(
    profile: GenesisProfile,
    vrf_seed_hex: Option<&str>,
    output: &str,
) -> KagamiVerifyReport {
    let mut chain_id = None;
    let mut collectors_k = None;
    let mut collectors_redundant_send_r = None;
    let mut peers_with_pop = None;
    let mut fingerprint = None;
    let mut vrf_seed = vrf_seed_hex.map(|value| value.to_owned());

    for line in output.lines() {
        if chain_id.is_none() {
            chain_id = parse_keyed_value(line, &["chain_id", "chain id", "chain"]);
        }
        if collectors_k.is_none() {
            collectors_k = parse_keyed_value(line, &["collectors_k", "collectors k", "k"])
                .and_then(|value| value.parse().ok());
        }
        if collectors_redundant_send_r.is_none() {
            collectors_redundant_send_r =
                parse_keyed_value(line, &["collectors_r", "collectors-r", "redundant", "r"])
                    .and_then(|value| value.parse().ok());
        }
        if peers_with_pop.is_none() {
            peers_with_pop = parse_keyed_value(
                line,
                &["peers_with_pop", "peers-with-pop", "pop_peers", "pop"],
            )
            .and_then(|value| value.parse().ok());
        }
        if fingerprint.is_none() {
            fingerprint = parse_keyed_value(line, &["fingerprint", "hash", "fingerprint_hex"]);
        }
        if vrf_seed.is_none() {
            vrf_seed = parse_keyed_value(line, &["vrf_seed_hex", "vrf-seed", "vrf_seed"]);
        }
    }

    KagamiVerifyReport {
        profile,
        chain_id,
        collectors_k,
        collectors_redundant_send_r,
        vrf_seed_hex: vrf_seed,
        peers_with_pop,
        fingerprint,
        raw_output: output.to_owned(),
    }
}

fn default_data_root() -> PathBuf {
    std::env::var_os("MOCHI_DATA_ROOT")
        .map(PathBuf::from)
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| std::env::temp_dir().join("mochi"))
}

fn default_snapshot_slug() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO);
    format!("snapshot-{}-{:03}", now.as_secs(), now.subsec_millis())
}

const SNAPSHOT_LABEL_MAX_LEN: usize = 64;

fn sanitize_snapshot_label(label: &str) -> Option<String> {
    let mut sanitized = String::with_capacity(label.len().min(SNAPSHOT_LABEL_MAX_LEN));
    let mut previous_was_sep = true;

    for ch in label.chars() {
        match ch {
            'a'..='z' | '0'..='9' => {
                if sanitized.len() == SNAPSHOT_LABEL_MAX_LEN {
                    break;
                }
                sanitized.push(ch);
                previous_was_sep = false;
            }
            'A'..='Z' => {
                if sanitized.len() == SNAPSHOT_LABEL_MAX_LEN {
                    break;
                }
                sanitized.push(ch.to_ascii_lowercase());
                previous_was_sep = false;
            }
            '-' | '_' | ' ' | '.' => {
                if !previous_was_sep && sanitized.len() < SNAPSHOT_LABEL_MAX_LEN {
                    sanitized.push('-');
                    previous_was_sep = true;
                }
            }
            _ => {
                // Skip unsupported characters but continue scanning so alphanumeric
                // runs can still be recovered.
            }
        }
    }

    let trimmed = sanitized.trim_matches('-');
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_owned())
    }
}

struct SnapshotMetadata {
    chain_id: String,
    peer_count: u64,
    genesis_hash: Hash,
    kura_hashes: HashMap<String, Hash>,
}

fn load_snapshot_metadata(root: &Path) -> Result<SnapshotMetadata> {
    let metadata_path = root.join("metadata.json");
    let bytes = fs::read(&metadata_path).map_err(|err| {
        SupervisorError::Config(format!(
            "failed to read snapshot metadata `{}`: {err}",
            metadata_path.display()
        ))
    })?;
    let value: Value = json::from_slice(&bytes).map_err(|err| {
        SupervisorError::Config(format!(
            "failed to parse snapshot metadata `{}`: {err}",
            metadata_path.display()
        ))
    })?;
    let object = value.as_object().ok_or_else(|| {
        SupervisorError::Config(format!(
            "snapshot metadata `{}` must be a JSON object",
            metadata_path.display()
        ))
    })?;
    let chain_id = object
        .get("chain_id")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            SupervisorError::Config(format!(
                "snapshot metadata `{}` missing `chain_id` string",
                metadata_path.display()
            ))
        })?;
    let peer_count = object
        .get("peer_count")
        .and_then(Value::as_u64)
        .ok_or_else(|| {
            SupervisorError::Config(format!(
                "snapshot metadata `{}` missing `peer_count` number",
                metadata_path.display()
            ))
        })?;
    let genesis_hash = object
        .get("genesis_hash")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            SupervisorError::Config(format!(
                "snapshot metadata `{}` missing `genesis_hash` string",
                metadata_path.display()
            ))
        })
        .and_then(|value| {
            Hash::from_str(value).map_err(|err| {
                SupervisorError::Config(format!(
                    "snapshot metadata `{}` has invalid `genesis_hash`: {err}",
                    metadata_path.display()
                ))
            })
        })?;

    let kura_hashes_value = object
        .get("kura_hashes")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            SupervisorError::Config(format!(
                "snapshot metadata `{}` missing `kura_hashes` map",
                metadata_path.display()
            ))
        })?;

    let mut kura_hashes = HashMap::new();
    for (alias, value) in kura_hashes_value {
        let hash_str = value.as_str().ok_or_else(|| {
            SupervisorError::Config(format!(
                "snapshot metadata `{}` contains non-string hash for `{alias}`",
                metadata_path.display()
            ))
        })?;
        let hash = Hash::from_str(hash_str).map_err(|err| {
            SupervisorError::Config(format!(
                "snapshot metadata `{}` has invalid hash for `{alias}`: {err}",
                metadata_path.display()
            ))
        })?;
        kura_hashes.insert(alias.clone(), hash);
    }
    Ok(SnapshotMetadata {
        chain_id: chain_id.to_owned(),
        peer_count,
        genesis_hash,
        kura_hashes,
    })
}

fn copy_file_if_exists(src: &Path, dst: &Path) -> std::io::Result<()> {
    if !src.exists() {
        return Ok(());
    }
    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::copy(src, dst)?;
    Ok(())
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
    fs::create_dir_all(dst)?;
    if !src.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let target = dst.join(entry.file_name());
        if file_type.is_dir() {
            copy_dir_recursive(&entry.path(), &target)?;
        } else if file_type.is_file() {
            fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

fn collect_files_recursive(root: &Path, files: &mut Vec<PathBuf>) -> io::Result<()> {
    if !root.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            collect_files_recursive(&path, files)?;
        } else if file_type.is_file() {
            files.push(path);
        }
    }
    Ok(())
}

fn normalized_relative_path(base: &Path, path: &Path) -> String {
    let relative = path.strip_prefix(base).unwrap_or(path);
    relative
        .components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

fn hash_directory(root: &Path) -> io::Result<Hash> {
    let mut files = Vec::new();
    collect_files_recursive(root, &mut files)?;
    files.sort_unstable();

    let mut accumulator = Vec::new();
    for file in files {
        let rel = normalized_relative_path(root, &file);
        let contents = fs::read(&file)?;
        let file_hash = Hash::new(&contents);
        accumulator.extend_from_slice(&(rel.len() as u64).to_le_bytes());
        accumulator.extend_from_slice(rel.as_bytes());
        accumulator.extend_from_slice(&(contents.len() as u64).to_le_bytes());
        accumulator.extend_from_slice(file_hash.as_ref());
    }

    Ok(Hash::new(&accumulator))
}

#[cfg(test)]
mod tests {
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;
    use std::{
        collections::HashSet,
        env,
        ffi::OsString,
        io::ErrorKind,
        net::TcpListener,
        path::Path,
        sync::{Mutex, OnceLock},
        time::Duration,
    };

    use iroha_crypto::PublicKey;
    use iroha_data_model::{peer::PeerId, prelude::DomainId};
    use tokio::runtime::Runtime;

    use super::*;

    struct EnvVarGuard {
        key: &'static str,
    }

    impl EnvVarGuard {
        fn set(key: &'static str, value: &std::ffi::OsStr) -> Self {
            // SAFETY: tests serialize environment mutation within a single thread.
            unsafe {
                env::set_var(key, value);
            }
            Self { key }
        }
    }

    impl Drop for EnvVarGuard {
        fn drop(&mut self) {
            // SAFETY: matching set_var executed in a controlled test environment.
            unsafe {
                env::remove_var(self.key);
            }
        }
    }

    struct RestoringEnvVarGuard {
        key: &'static str,
        previous: Option<OsString>,
    }

    impl RestoringEnvVarGuard {
        fn set(key: &'static str, value: &std::ffi::OsStr) -> Self {
            let previous = env::var_os(key);
            // SAFETY: tests serialize environment mutation within a single thread.
            unsafe {
                env::set_var(key, value);
            }
            Self { key, previous }
        }
    }

    impl Drop for RestoringEnvVarGuard {
        fn drop(&mut self) {
            // SAFETY: matching set_var executed in a controlled test environment.
            unsafe {
                if let Some(previous) = self.previous.take() {
                    env::set_var(self.key, previous);
                } else {
                    env::remove_var(self.key);
                }
            }
        }
    }

    fn write_version_stub(root: &Path, name: &str, build_line: &str) -> PathBuf {
        let script_path = root.join(format!("{name}.sh"));
        let script = format!(
            r#"#!/bin/sh
case "$1" in
  --version)
    echo "{name} {build_line}"
    exit 0
    ;;
  *)
    exit 0
    ;;
esac
"#
        );
        fs::write(&script_path, script).expect("write version stub");
        #[cfg(unix)]
        {
            let mut perms = fs::metadata(&script_path)
                .expect("version stub metadata")
                .permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&script_path, perms).expect("set version stub permissions");
        }
        script_path
    }

    #[cfg(unix)]
    fn write_cargo_build_stub(root: &Path) -> (PathBuf, PathBuf) {
        let stub_path = root.join("cargo_stub.sh");
        let log_path = root.join("cargo_stub.log");
        let script = r#"#!/bin/sh
set -eu
LOG_FILE="${MOCHI_TEST_CARGO_LOG:-}"
if [ -n "$LOG_FILE" ]; then
  printf '%s\n' "$*" >> "$LOG_FILE"
fi
TARGET="${CARGO_TARGET_DIR:-target}"
/bin/mkdir -p "$TARGET/debug"
case "$*" in
  *"-p irohad"*)
    BIN="$TARGET/debug/irohad"
    ;;
  *"-p iroha_kagami"*)
    BIN="$TARGET/debug/kagami"
    ;;
  *"-p iroha_cli"*)
    BIN="$TARGET/debug/iroha3"
    ;;
  *)
    BIN="$TARGET/debug/unknown"
    ;;
esac
/bin/cat > "$BIN" <<'EOF'
#!/bin/sh
if [ "$1" = "--version" ]; then
  bin="${0##*/}"
  echo "$bin iroha3"
  exit 0
fi
exit 0
EOF
/bin/chmod 755 "$BIN"
exit 0
"#
        .to_string();
        fs::write(&stub_path, script).expect("write cargo stub");
        let mut perms = fs::metadata(&stub_path)
            .expect("cargo stub metadata")
            .permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&stub_path, perms).expect("set cargo stub permissions");
        let _ = fs::File::create(&log_path);
        (stub_path, log_path)
    }

    #[cfg(unix)]
    fn write_cargo_failure_stub(root: &Path) -> PathBuf {
        let stub_path = root.join("cargo_fail_stub.sh");
        let script = r#"#!/bin/sh
exit 1
"#;
        fs::write(&stub_path, script).expect("write cargo failure stub");
        let mut perms = fs::metadata(&stub_path)
            .expect("cargo failure stub metadata")
            .permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&stub_path, perms).expect("set cargo failure stub permissions");
        stub_path
    }

    #[test]
    fn snapshot_label_sanitization_behaves() {
        assert_eq!(
            sanitize_snapshot_label("My First Snapshot!").as_deref(),
            Some("my-first-snapshot")
        );
        assert_eq!(
            sanitize_snapshot_label("weird__Label!!").as_deref(),
            Some("weird-label")
        );
        assert!(sanitize_snapshot_label("%%%").is_none());
    }

    #[test]
    fn snapshot_label_collapses_separators_and_clamps_length() {
        let label = "___  noisy--Label...with__mixed---separators   ";
        assert_eq!(
            sanitize_snapshot_label(label).as_deref(),
            Some("noisy-label-with-mixed-separators")
        );

        let long_label = "PREFIX".repeat(20);
        let sanitized = sanitize_snapshot_label(&long_label).expect("sanitized output");
        assert!(
            sanitized.len() <= SNAPSHOT_LABEL_MAX_LEN,
            "sanitized label should be clamped"
        );
        assert!(
            sanitized.starts_with("prefixprefixprefix"),
            "sanitized label should keep leading alphas in lower-case"
        );
    }

    #[test]
    fn copy_dir_recursive_handles_missing_sources() {
        let temp = tempfile::tempdir().expect("tempdir");
        let missing = temp.path().join("missing");
        let dest = temp.path().join("out");
        copy_dir_recursive(&missing, &dest).expect("copy missing source");
        assert!(dest.exists(), "destination directory should be created");
        let mut iter = fs::read_dir(&dest).expect("read destination dir");
        assert!(iter.next().is_none(), "destination should remain empty");
    }

    #[test]
    fn probe_version_output_parses_and_infers_build_line() {
        let temp = tempfile::tempdir().expect("tempdir");
        let script_path = temp.path().join("custom-bin.sh");
        let script = r#"#!/bin/sh
echo "custom-bin iroha3 3.2.1"
exit 0
"#;
        fs::write(&script_path, script).expect("write version script");
        #[cfg(unix)]
        {
            let mut perms = fs::metadata(&script_path)
                .expect("version script metadata")
                .permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&script_path, perms).expect("set version script perms");
        }

        let (raw, version) =
            probe_version_output(&script_path, "custom-bin").expect("version probe succeeds");
        assert_eq!(version.as_deref(), Some("custom-bin iroha3 3.2.1"));
        let build_line = infer_build_line("custom-bin", &script_path, raw.as_deref());
        assert_eq!(build_line, BuildLine::Iroha3);
    }

    #[test]
    fn probe_version_output_honors_iroha2_output() {
        let temp = tempfile::tempdir().expect("tempdir");
        let script_path = temp.path().join("weird-name.sh");
        let script = r#"#!/bin/sh
echo "my-custom iroha2 build"
exit 0
"#;
        fs::write(&script_path, script).expect("write version script");
        #[cfg(unix)]
        {
            let mut perms = fs::metadata(&script_path)
                .expect("version script metadata")
                .permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&script_path, perms).expect("set version script perms");
        }

        let (raw, _) =
            probe_version_output(&script_path, "weird-name").expect("version probe succeeds");
        let build_line = infer_build_line("weird-name", &script_path, raw.as_deref());
        assert_eq!(build_line, BuildLine::Iroha2);
    }

    #[test]
    fn compatibility_summary_includes_profile_and_fingerprint() {
        let report = CompatibilityReport {
            versions: Vec::new(),
            verify: Some(KagamiVerifyReport {
                profile: GenesisProfile::Iroha3Dev,
                chain_id: Some("test-chain".to_owned()),
                collectors_k: None,
                collectors_redundant_send_r: None,
                vrf_seed_hex: None,
                peers_with_pop: None,
                fingerprint: Some("fp123".to_owned()),
                raw_output: "fingerprint fp123".to_owned(),
            }),
            chain_id: "test-chain".to_owned(),
            profile: Some(GenesisProfile::Iroha3Dev),
        };

        let summary = report.summary_line();
        assert!(
            summary.contains("chain test-chain"),
            "summary should include chain id: {summary}"
        );
        assert!(
            summary.contains("profile iroha3-dev"),
            "summary should include profile slug: {summary}"
        );
        assert!(
            summary.contains("fingerprint fp123"),
            "summary should include verify fingerprint: {summary}"
        );
    }

    struct KagamiStub {
        _path_guard: EnvVarGuard,
        _log_guard: EnvVarGuard,
        _irohad_guard: EnvVarGuard,
        _iroha_cli_guard: EnvVarGuard,
        log_path: PathBuf,
    }

    impl KagamiStub {
        fn install(root: &Path) -> Self {
            let script_path = root.join("kagami_stub.sh");
            let script = r#"#!/bin/sh
if [ -n "$MOCHI_KAGAMI_LOG" ]; then
  printf 'args:%s\n' "$*" >> "$MOCHI_KAGAMI_LOG"
fi
case "$1" in
  --version)
    echo "kagami-stub iroha3"
    exit 0
    ;;
  verify)
    exit 0
    ;;
  genesis)
    ;;
  *)
    echo "unsupported kagami stub command: $1" >&2
    exit 1
    ;;
esac
cat <<'JSON'
{"chain":"00000000-0000-0000-0000-000000000000","ivm_dir":".","consensus_mode":"Permissioned","transactions":[{"instructions":[]}]}
JSON
"#;
            fs::write(&script_path, script).expect("write kagami stub");
            #[cfg(unix)]
            {
                let mut perms = fs::metadata(&script_path)
                    .expect("script metadata")
                    .permissions();
                perms.set_mode(0o755);
                fs::set_permissions(&script_path, perms).expect("set script perms");
            }
            let log_path = root.join("kagami_stub.log");
            let iroha_stub = write_version_stub(root, "iroha-stub", "iroha3");
            let path_guard = EnvVarGuard::set("MOCHI_KAGAMI", script_path.as_os_str());
            let log_guard = EnvVarGuard::set("MOCHI_KAGAMI_LOG", log_path.as_os_str());
            let irohad_guard = EnvVarGuard::set("MOCHI_IROHAD", iroha_stub.as_os_str());
            let iroha_cli_guard = EnvVarGuard::set("MOCHI_IROHA_CLI", iroha_stub.as_os_str());
            let _ = fs::File::create(&log_path);
            Self {
                _path_guard: path_guard,
                _log_guard: log_guard,
                _irohad_guard: irohad_guard,
                _iroha_cli_guard: iroha_cli_guard,
                log_path,
            }
        }

        fn log_path(&self) -> &Path {
            &self.log_path
        }
    }

    struct StandaloneKagamiStub {
        script_path: PathBuf,
        log_path: PathBuf,
        _irohad_guard: EnvVarGuard,
        _iroha_cli_guard: EnvVarGuard,
    }

    impl StandaloneKagamiStub {
        fn create(root: &Path) -> Self {
            let script_path = root.join("kagami_override.sh");
            let log_path = root.join("kagami_override.log");
            let script = r#"#!/bin/sh
set -e
SCRIPT_DIR="$(CDPATH= cd -- "$(dirname "$0")" && pwd)"
printf '%s\n' "$@" >> "$SCRIPT_DIR/kagami_override.log"
case "$1" in
  --version)
    echo "kagami-override iroha3"
    exit 0
    ;;
  verify)
    exit 0
    ;;
  genesis)
    ;;
  *)
    echo "unsupported kagami stub command: $1" >&2
    exit 1
    ;;
esac
cat <<'JSON'
{"chain":"00000000-0000-0000-0000-000000000000","ivm_dir":".","consensus_mode":"Permissioned","transactions":[{"instructions":[]}]}
JSON
"#;
            fs::write(&script_path, script).expect("write standalone kagami stub");
            #[cfg(unix)]
            {
                let mut perms = fs::metadata(&script_path)
                    .expect("standalone script metadata")
                    .permissions();
                perms.set_mode(0o755);
                fs::set_permissions(&script_path, perms)
                    .expect("set standalone script permissions");
            }
            let iroha_stub = write_version_stub(root, "kagami-override-iroha", "iroha3");
            let irohad_guard = EnvVarGuard::set("MOCHI_IROHAD", iroha_stub.as_os_str());
            let iroha_cli_guard = EnvVarGuard::set("MOCHI_IROHA_CLI", iroha_stub.as_os_str());
            Self {
                script_path,
                log_path,
                _irohad_guard: irohad_guard,
                _iroha_cli_guard: iroha_cli_guard,
            }
        }

        fn script_path(&self) -> &Path {
            &self.script_path
        }

        fn log_path(&self) -> &Path {
            &self.log_path
        }
    }

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn ports_available(context: &str) -> bool {
        match TcpListener::bind(("127.0.0.1", 0)) {
            Ok(listener) => {
                drop(listener);
                true
            }
            Err(err)
                if matches!(
                    err.kind(),
                    ErrorKind::PermissionDenied | ErrorKind::AddrNotAvailable
                ) =>
            {
                eprintln!("skipping {context}: {err}");
                false
            }
            Err(err) => panic!("{context}: {err}"),
        }
    }

    #[test]
    fn binary_paths_default_respects_env_override() {
        let temp = tempfile::NamedTempFile::new().expect("temp file");
        let override_path = temp.path().to_path_buf();
        let _guard = EnvVarGuard::set("MOCHI_IROHAD", override_path.as_os_str());
        let binaries = BinaryPaths::default();
        assert_eq!(binaries.irohad_executable(), override_path.as_path());
    }

    #[test]
    fn binary_paths_default_respects_cli_env_override() {
        let temp = tempfile::NamedTempFile::new().expect("temp file");
        let override_path = temp.path().to_path_buf();
        let _guard = EnvVarGuard::set("MOCHI_IROHA_CLI", override_path.as_os_str());
        let binaries = BinaryPaths::default();
        assert_eq!(binaries.iroha_cli_executable(), override_path.as_path());
    }

    #[cfg(unix)]
    #[test]
    fn binary_paths_auto_builds_when_enabled() {
        let _env = env_lock().lock().expect("env lock");
        let temp = tempfile::tempdir().expect("tempdir");
        let empty_path_dir = temp.path().join("empty-path");
        fs::create_dir_all(&empty_path_dir).expect("create empty path dir");
        let target_dir = temp.path().join("target");
        fs::create_dir_all(&target_dir).expect("create target dir");

        let (cargo_stub, cargo_log) = write_cargo_build_stub(temp.path());
        let _cargo_guard = RestoringEnvVarGuard::set("CARGO", cargo_stub.as_os_str());
        let _target_guard = RestoringEnvVarGuard::set("CARGO_TARGET_DIR", target_dir.as_os_str());
        let _path_guard = RestoringEnvVarGuard::set("PATH", empty_path_dir.as_os_str());
        let _log_guard = RestoringEnvVarGuard::set("MOCHI_TEST_CARGO_LOG", cargo_log.as_os_str());

        let mut binaries = BinaryPaths::default().allow_auto_builds(true);
        binaries.irohad = PathBuf::from("irohad");
        binaries.irohad_verified = false;
        binaries.irohad_build_attempted = false;
        binaries.irohad_auto = true;
        binaries.irohad_source = BinarySource::AutoDefault;
        binaries.kagami = PathBuf::from("kagami");
        binaries.kagami_verified = false;
        binaries.kagami_build_attempted = false;
        binaries.kagami_auto = true;
        binaries.kagami_source = BinarySource::AutoDefault;
        binaries.iroha_cli = PathBuf::from("iroha_cli");
        binaries.iroha_cli_verified = false;
        binaries.iroha_cli_build_attempted = false;
        binaries.iroha_cli_auto = true;
        binaries.iroha_cli_source = BinarySource::AutoDefault;

        let versions = binaries
            .probe_versions()
            .expect("probe versions should succeed");
        assert_eq!(versions.len(), 3);
        assert!(
            versions
                .iter()
                .all(|info| info.build_line == BuildLine::Iroha3),
            "auto-built binaries should report iroha3 build-line"
        );

        let log = fs::read_to_string(&cargo_log).expect("read cargo log");
        assert_eq!(
            log.lines().count(),
            3,
            "expected one cargo invocation per binary build"
        );

        let _ = binaries
            .probe_versions()
            .expect("second probe should succeed");
        let log = fs::read_to_string(&cargo_log).expect("read cargo log");
        assert_eq!(
            log.lines().count(),
            3,
            "second probe should not trigger additional cargo builds"
        );
    }

    #[cfg(unix)]
    #[test]
    fn binary_paths_auto_build_failure_surfaces_error() {
        let _env = env_lock().lock().expect("env lock");
        let temp = tempfile::tempdir().expect("tempdir");
        let empty_path_dir = temp.path().join("empty-path");
        fs::create_dir_all(&empty_path_dir).expect("create empty path dir");
        let target_dir = temp.path().join("target");
        fs::create_dir_all(&target_dir).expect("create target dir");

        let cargo_stub = write_cargo_failure_stub(temp.path());
        let _cargo_guard = RestoringEnvVarGuard::set("CARGO", cargo_stub.as_os_str());
        let _target_guard = RestoringEnvVarGuard::set("CARGO_TARGET_DIR", target_dir.as_os_str());
        let _path_guard = RestoringEnvVarGuard::set("PATH", empty_path_dir.as_os_str());

        let mut binaries = BinaryPaths::default().allow_auto_builds(true);
        binaries.irohad = PathBuf::from("irohad");
        binaries.irohad_verified = false;
        binaries.irohad_build_attempted = false;
        binaries.irohad_auto = true;
        binaries.irohad_source = BinarySource::AutoDefault;

        let err = binaries
            .ensure_irohad_ready()
            .expect_err("auto-build should surface failure");
        match err {
            SupervisorError::BinaryUnavailable { binary, message } => {
                assert_eq!(binary, "irohad");
                assert!(
                    message.contains("cargo build"),
                    "expected cargo build context, got `{message}`"
                );
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[cfg(unix)]
    #[test]
    fn binary_paths_rejects_build_line_mismatch() {
        let _env = env_lock().lock().expect("env lock");
        let temp = tempfile::tempdir().expect("tempdir");
        let iroha2_stub = write_version_stub(temp.path(), "iroha2-stub", "iroha2");
        let iroha3_stub = write_version_stub(temp.path(), "iroha3-stub", "iroha3");

        let mut binaries = BinaryPaths::default()
            .irohad(iroha2_stub)
            .kagami(iroha3_stub.clone())
            .iroha_cli(iroha3_stub);

        let err = binaries
            .verify_build_line(BuildLine::Iroha3)
            .expect_err("iroha2 binary should fail build-line verification");
        match err {
            SupervisorError::BuildLineMismatch {
                binary,
                expected,
                found,
            } => {
                assert_eq!(binary, "irohad");
                assert_eq!(expected, BuildLine::Iroha3);
                assert_eq!(found, BuildLine::Iroha2);
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn builder_creates_peer_configs() {
        if !ports_available("builder_creates_peer_configs") {
            return;
        }
        let _env = env_lock().lock().expect("env lock");
        let temp = tempfile::tempdir().expect("tempdir");
        let _stub = KagamiStub::install(temp.path());
        let supervisor = SupervisorBuilder::new(ProfilePreset::SinglePeer)
            .data_root(temp.path())
            .chain_id("test-chain")
            .torii_base_port(9000)
            .p2p_base_port(19000)
            .build()
            .expect("build supervisor");

        assert_eq!(supervisor.chain_id(), "test-chain");
        assert_eq!(supervisor.peers().len(), 1);

        let config_path = supervisor.peers()[0].config_path().to_path_buf();
        let contents = fs::read_to_string(config_path).expect("config readable");
        let value: toml::Table = toml::from_str(&contents).expect("valid toml");

        assert_eq!(
            value.get("chain").and_then(toml::Value::as_str),
            Some("test-chain")
        );
        assert_eq!(
            value
                .get("torii")
                .and_then(toml::Value::as_table)
                .and_then(|table| table.get("address"))
                .and_then(toml::Value::as_str),
            Some("0.0.0.0:9000")
        );
        assert_eq!(
            value
                .get("network")
                .and_then(toml::Value::as_table)
                .and_then(|table| table.get("public_address"))
                .and_then(toml::Value::as_str),
            Some("127.0.0.1:19000")
        );
    }

    #[test]
    fn builder_reserves_unique_ports_across_torii_and_p2p() {
        if !ports_available("builder_reserves_unique_ports_across_torii_and_p2p") {
            return;
        }
        let _env = env_lock().lock().expect("env lock");
        let temp = tempfile::tempdir().expect("tempdir");
        let _stub = KagamiStub::install(temp.path());
        let base_port = 32000;
        let supervisor = SupervisorBuilder::new(ProfilePreset::FourPeerBft)
            .data_root(temp.path())
            .torii_base_port(base_port)
            .p2p_base_port(base_port)
            .build()
            .expect("build supervisor");

        let mut seen_ports = HashSet::new();
        for peer in supervisor.peers() {
            for addr in [peer.torii_address(), peer.p2p_address()] {
                let port = addr
                    .parse::<std::net::SocketAddr>()
                    .map(|socket| socket.port())
                    .expect("address contains a port");
                assert!(
                    seen_ports.insert(port),
                    "port {port} should be unique across Torii and P2P assignments"
                );
            }
        }
    }

    #[test]
    fn multi_peer_trusted_peers_list_everyone() {
        if !ports_available("multi_peer_trusted_peers_list_everyone") {
            return;
        }
        let _env = env_lock().lock().expect("env lock");
        let temp = tempfile::tempdir().expect("tempdir");
        let _stub = KagamiStub::install(temp.path());
        let supervisor = SupervisorBuilder::new(ProfilePreset::FourPeerBft)
            .data_root(temp.path())
            .torii_base_port(8000)
            .p2p_base_port(17000)
            .build()
            .expect("build supervisor");

        assert_eq!(supervisor.peers().len(), 4);

        let first_config =
            fs::read_to_string(supervisor.peers()[0].config_path()).expect("config readable");
        let value: toml::Table = toml::from_str(&first_config).expect("valid toml");
        let trusted = value
            .get("trusted_peers")
            .and_then(toml::Value::as_array)
            .expect("array");
        assert_eq!(trusted.len(), 4);
    }

    #[test]
    fn custom_profile_supports_seven_peers_with_unique_ports() {
        if !ports_available("custom_profile_supports_seven_peers_with_unique_ports") {
            return;
        }
        let _env = env_lock().lock().expect("env lock");
        let temp = tempfile::tempdir().expect("tempdir");
        let _stub = KagamiStub::install(temp.path());
        let profile =
            NetworkProfile::custom(7, SumeragiConsensusMode::Permissioned).expect("profile");
        let supervisor = SupervisorBuilder::with_profile(profile)
            .data_root(temp.path())
            .torii_base_port(8100)
            .p2p_base_port(17100)
            .build()
            .expect("build supervisor");

        assert_eq!(supervisor.peers().len(), 7);

        let mut seen_ports = HashSet::new();
        for peer in supervisor.peers() {
            for addr in [peer.torii_address(), peer.p2p_address()] {
                let port = addr
                    .parse::<std::net::SocketAddr>()
                    .map(|socket| socket.port())
                    .expect("address contains a port");
                assert!(
                    seen_ports.insert(port),
                    "port {port} should be unique across Torii and P2P assignments"
                );
            }
        }

        let first_config =
            fs::read_to_string(supervisor.peers()[0].config_path()).expect("config readable");
        let value: toml::Table = toml::from_str(&first_config).expect("valid toml");
        let trusted = value
            .get("trusted_peers")
            .and_then(toml::Value::as_array)
            .expect("array");
        assert_eq!(trusted.len(), 7);
    }

    #[test]
    fn profile_preset_preserves_consensus_mode() {
        let profile = NetworkProfile::custom(3, SumeragiConsensusMode::Npos).expect("profile");
        let builder =
            SupervisorBuilder::with_profile(profile).profile_preset(ProfilePreset::SinglePeer);
        assert_eq!(builder.profile().preset, Some(ProfilePreset::SinglePeer));
        assert_eq!(builder.profile().topology.peer_count, 1);
        assert_eq!(
            builder.profile().consensus_mode,
            SumeragiConsensusMode::Npos
        );
    }

    #[test]
    fn build_rejects_genesis_profile_without_npos() {
        let temp = tempfile::tempdir().expect("tempdir");
        let profile =
            NetworkProfile::custom(1, SumeragiConsensusMode::Permissioned).expect("profile");
        let builder = SupervisorBuilder::with_profile(profile)
            .data_root(temp.path())
            .genesis_profile(GenesisProfile::Iroha3Dev)
            .set_profile(
                NetworkProfile::custom(1, SumeragiConsensusMode::Permissioned).expect("profile"),
            );

        let err = builder
            .build()
            .expect_err("expected consensus mode mismatch");
        assert!(
            err.to_string()
                .contains("genesis_profile requires consensus_mode npos"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn genesis_includes_topology() {
        if !ports_available("genesis_includes_topology") {
            return;
        }
        let _env = env_lock().lock().expect("env lock");
        let temp = tempfile::tempdir().expect("tempdir");
        let _stub = KagamiStub::install(temp.path());
        let supervisor = SupervisorBuilder::new(ProfilePreset::SinglePeer)
            .data_root(temp.path())
            .build()
            .expect("build supervisor");

        let bytes = fs::read(supervisor.genesis_manifest()).expect("genesis manifest readable");
        let manifest: norito::json::Value =
            norito::json::from_slice(&bytes).expect("parse genesis json");
        let transactions = manifest
            .get("transactions")
            .and_then(norito::json::Value::as_array)
            .expect("transactions array");
        let contains_topology = transactions.iter().any(|tx| {
            tx.get("topology")
                .and_then(norito::json::Value::as_array)
                .map(|entries| !entries.is_empty())
                .unwrap_or(false)
        });
        assert!(
            contains_topology,
            "genesis manifest should include topology transaction"
        );
    }

    #[test]
    fn genesis_generation_invokes_kagami() {
        if !ports_available("genesis_generation_invokes_kagami") {
            return;
        }
        let _env = env_lock().lock().expect("env lock");
        let temp = tempfile::tempdir().expect("tempdir");
        let stub = KagamiStub::install(temp.path());
        SupervisorBuilder::new(ProfilePreset::SinglePeer)
            .data_root(temp.path())
            .build()
            .expect("build supervisor");

        let log = fs::read_to_string(stub.log_path()).expect("kagami invocation log");
        assert!(
            log.contains("genesis") && log.contains("generate"),
            "expected kagami invocation to record subcommand, got `{log}`"
        );
        assert!(
            log.contains("--genesis-public-key"),
            "expected kagami invocation to record genesis public key argument"
        );
    }

    #[test]
    fn genesis_profile_and_seed_forward_to_kagami() {
        if !ports_available("genesis_profile_and_seed_forward_to_kagami") {
            return;
        }
        let _env = env_lock().lock().expect("env lock");
        let temp = tempfile::tempdir().expect("tempdir");
        let stub = KagamiStub::install(temp.path());
        let seed = "0102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f";

        SupervisorBuilder::new(ProfilePreset::SinglePeer)
            .data_root(temp.path())
            .genesis_profile(GenesisProfile::Iroha3Dev)
            .vrf_seed_hex(seed)
            .build()
            .expect("build supervisor");

        let log = fs::read_to_string(stub.log_path()).expect("kagami invocation log");
        assert!(
            log.contains("--profile") && log.contains("iroha3-dev"),
            "profile should be forwarded to kagami: {log}"
        );
        assert!(
            log.contains("--vrf-seed-hex") && log.contains(seed),
            "vrf seed should be forwarded to kagami: {log}"
        );
        assert!(
            log.contains("--consensus-mode") && log.contains("npos"),
            "npos mode should be pinned when a genesis profile is used: {log}"
        );
    }

    #[test]
    fn peer_config_records_chain_and_fingerprint_header() {
        if !ports_available("peer_config_records_chain_and_fingerprint_header") {
            return;
        }
        let _env = env_lock().lock().expect("env lock");
        let temp = tempfile::tempdir().expect("tempdir");
        let _stub = KagamiStub::install(temp.path());
        let supervisor = SupervisorBuilder::new(ProfilePreset::SinglePeer)
            .data_root(temp.path())
            .genesis_profile(GenesisProfile::Iroha3Dev)
            .build()
            .expect("build supervisor");

        let manifest =
            RawGenesisTransaction::from_path(supervisor.genesis_manifest()).expect("genesis");
        let fingerprint = manifest
            .consensus_fingerprint()
            .map(str::to_owned)
            .or_else(|| {
                let normalized = manifest.clone().with_consensus_meta();
                normalized.consensus_fingerprint().map(str::to_owned)
            })
            .expect("consensus fingerprint");

        let peer = supervisor.peers().first().expect("peer");
        let config_text = fs::read_to_string(peer.config_path()).expect("read config");
        let expected_chain = format!("# mochi.chain_id = {}", supervisor.chain_id());
        let expected_fingerprint = format!("# mochi.consensus_fingerprint = {fingerprint}");

        assert!(
            config_text.contains(&expected_chain),
            "config should record chain id header"
        );
        assert!(
            config_text.contains(&expected_fingerprint),
            "config should record consensus fingerprint header"
        );
    }

    #[test]
    fn readiness_smoke_plan_uses_genesis_signer_and_unique_nonces() {
        if !ports_available("readiness_smoke_plan_uses_genesis_signer_and_unique_nonces") {
            return;
        }
        let _env = env_lock().lock().expect("env lock");
        let temp = tempfile::tempdir().expect("tempdir");
        let _stub = KagamiStub::install(temp.path());
        let supervisor = SupervisorBuilder::new(ProfilePreset::SinglePeer)
            .data_root(temp.path())
            .build()
            .expect("build supervisor");

        let plan = supervisor
            .readiness_smoke_plan_with_offset(3, 2)
            .expect("build readiness plan");
        assert_eq!(plan.transactions.len(), 3);

        let expected_domain: DomainId = SMOKE_ACCOUNT_DOMAIN.parse().expect("parse domain id");
        let mut nonces = HashSet::new();
        for (idx, tx) in plan.transactions.iter().enumerate() {
            assert_eq!(tx.authority().domain, expected_domain);
            let nonce = tx.nonce().expect("nonce present");
            let nonce_value = u32::from(nonce);
            assert!(nonces.insert(nonce_value), "nonce should be unique");
            assert_eq!(
                nonce_value,
                (idx as u32) + 3,
                "nonce should incorporate offset"
            );
        }
    }

    #[test]
    fn export_snapshot_captures_storage_and_metadata() {
        if !ports_available("export_snapshot_captures_storage_and_metadata") {
            return;
        }
        let _env = env_lock().lock().expect("env lock");
        let temp = tempfile::tempdir().expect("tempdir");
        let _stub = KagamiStub::install(temp.path());
        let mut supervisor = SupervisorBuilder::new(ProfilePreset::FourPeerBft)
            .data_root(temp.path())
            .build()
            .expect("build supervisor");

        let mut peer_aliases = Vec::new();
        let mut expected_storage = Vec::new();
        for (idx, peer) in supervisor.peers().iter().enumerate() {
            let storage_file = peer.storage_dir().join(format!("sentinel-{idx}.bin"));
            let storage_contents = format!("peer-storage-{idx}");
            fs::write(&storage_file, &storage_contents).expect("write storage sentinel");

            let snapshot_marker = peer.snapshot_dir().join("marker.txt");
            fs::write(&snapshot_marker, b"snapshot-marker").expect("write snapshot marker");

            let log_path = peer.log_path();
            if let Some(parent) = log_path.parent() {
                fs::create_dir_all(parent).expect("create logs directory");
            }
            fs::write(log_path, format!("log-entry-{idx}")).expect("write peer log");
            peer_aliases.push(peer.alias().to_owned());
            expected_storage.push(storage_contents.into_bytes());
        }

        let snapshot_root = supervisor
            .export_snapshot(Some("Smoke Snapshot 2026"))
            .expect("export snapshot");

        assert_eq!(
            snapshot_root.file_name().unwrap(),
            std::ffi::OsStr::new("smoke-snapshot-2026"),
            "label should be sanitized before export"
        );
        assert!(snapshot_root.exists(), "snapshot directory should exist");

        let metadata_path = snapshot_root.join("metadata.json");
        let metadata_bytes = fs::read(&metadata_path).expect("read metadata");
        let metadata: Value =
            norito::json::from_slice(&metadata_bytes).expect("parse snapshot metadata");
        assert_eq!(
            metadata
                .get("chain_id")
                .and_then(Value::as_str)
                .expect("chain id present"),
            supervisor.chain_id()
        );
        assert_eq!(
            metadata
                .get("peer_count")
                .and_then(Value::as_u64)
                .expect("peer count present"),
            peer_aliases.len() as u64
        );
        assert!(
            metadata.get("created_at_ms").is_some(),
            "metadata should include creation timestamp"
        );
        let genesis_hash = metadata
            .get("genesis_hash")
            .and_then(Value::as_str)
            .expect("genesis hash present");
        let actual_genesis_hash =
            Hash::new(fs::read(supervisor.genesis_manifest()).expect("read genesis"));
        assert_eq!(
            genesis_hash,
            actual_genesis_hash.to_string(),
            "metadata should record the current genesis hash"
        );
        let kura_hashes = metadata
            .get("kura_hashes")
            .and_then(Value::as_object)
            .expect("kura hashes map present");
        assert_eq!(
            kura_hashes.len(),
            peer_aliases.len(),
            "kura hashes should track every peer"
        );

        for (idx, alias) in peer_aliases.iter().enumerate() {
            let alias_root = snapshot_root.join("peers").join(alias);
            let storage_copy = alias_root
                .join("storage")
                .join(format!("sentinel-{idx}.bin"));
            let snapshot_copy = alias_root.join("snapshot").join("marker.txt");
            let config_copy = alias_root.join("config.toml");
            let log_copy = alias_root.join("latest.log");

            assert_eq!(
                fs::read(&storage_copy).expect("storage copy should exist"),
                expected_storage[idx].as_slice(),
                "storage sentinel should be copied for peer {alias}"
            );
            assert_eq!(
                fs::read(&snapshot_copy).expect("snapshot copy should exist"),
                b"snapshot-marker",
                "snapshot marker should be copied for peer {alias}"
            );
            assert!(
                config_copy.exists(),
                "config should be exported for peer {alias}"
            );
            assert!(log_copy.exists(), "log should be exported for peer {alias}");
            let expected_hash =
                hash_directory(&alias_root.join("storage")).expect("hash copied storage");
            let recorded_hash = kura_hashes
                .get(alias)
                .and_then(Value::as_str)
                .expect("kura hash entry present");
            assert_eq!(
                recorded_hash,
                expected_hash.to_string(),
                "kura hash should match exported storage contents"
            );
        }

        let genesis_copy = snapshot_root.join("genesis").join(GENESIS_FILE_NAME);
        assert!(
            genesis_copy.exists(),
            "snapshot should include the current genesis manifest"
        );
    }

    #[test]
    fn export_snapshot_preserves_multilane_catalog_and_ports() {
        if !ports_available("export_snapshot_preserves_multilane_catalog_and_ports") {
            return;
        }
        let _env = env_lock().lock().expect("env lock");
        let temp = tempfile::tempdir().expect("tempdir");
        let _stub = KagamiStub::install(temp.path());

        let mut nexus = toml::Table::new();
        nexus.insert("enabled".into(), toml::Value::Boolean(true));
        let mut lane0 = toml::Table::new();
        lane0.insert("alias".into(), toml::Value::String("core".into()));
        lane0.insert("index".into(), toml::Value::Integer(0));
        lane0.insert("dataspace".into(), toml::Value::String("universal".into()));
        let mut lane1 = toml::Table::new();
        lane1.insert("alias".into(), toml::Value::String("governance".into()));
        lane1.insert("index".into(), toml::Value::Integer(1));
        lane1.insert("dataspace".into(), toml::Value::String("universal".into()));
        nexus.insert(
            "lane_catalog".into(),
            toml::Value::Array(vec![toml::Value::Table(lane0), toml::Value::Table(lane1)]),
        );
        let mut dataspace = toml::Table::new();
        dataspace.insert("alias".into(), toml::Value::String("universal".into()));
        dataspace.insert("id".into(), toml::Value::Integer(0));
        nexus.insert(
            "dataspace_catalog".into(),
            toml::Value::Array(vec![toml::Value::Table(dataspace)]),
        );

        let mut supervisor = SupervisorBuilder::new(ProfilePreset::FourPeerBft)
            .data_root(temp.path())
            .nexus_config(nexus)
            .build()
            .expect("build supervisor");

        let snapshot_root = supervisor
            .export_snapshot(Some("Multilane Snapshot"))
            .expect("export snapshot");

        let genesis_bytes = fs::read(snapshot_root.join("genesis").join(GENESIS_FILE_NAME))
            .expect("read snapshot genesis");
        let manifest: Value = norito::json::from_slice(&genesis_bytes).expect("parse genesis json");
        let chain = manifest
            .get("chain")
            .and_then(Value::as_str)
            .expect("chain field");
        assert_eq!(chain, supervisor.chain_id());

        let transactions = manifest
            .get("transactions")
            .and_then(Value::as_array)
            .expect("transactions array");
        let topology = transactions
            .iter()
            .filter_map(|tx| tx.get("topology").and_then(Value::as_array))
            .find(|entries| !entries.is_empty())
            .expect("non-empty topology transaction present");

        let actual_peer_ids: Vec<PeerId> = topology
            .iter()
            .map(|entry| {
                let decoded: GenesisTopologyEntry =
                    norito::json::from_value(entry.clone()).expect("topology entry should decode");
                decoded.peer
            })
            .collect();
        let expected_peer_ids: Vec<PeerId> = supervisor
            .peers()
            .iter()
            .map(|peer| peer.peer_id())
            .collect();
        assert_eq!(
            actual_peer_ids, expected_peer_ids,
            "snapshot genesis should preserve topology"
        );

        let peers_root = snapshot_root.join("peers");
        let mut seen_ports = HashSet::new();
        let mut genesis_files = HashSet::new();

        for peer in supervisor.peers() {
            let config_path = peers_root.join(peer.alias()).join("config.toml");
            let contents = fs::read_to_string(&config_path).expect("read snapshot config");
            let value: toml::Table = toml::from_str(&contents).expect("valid toml");

            let torii_addr = value
                .get("torii")
                .and_then(toml::Value::as_table)
                .and_then(|table| table.get("address"))
                .and_then(toml::Value::as_str)
                .expect("torii address");
            let network_addr = value
                .get("network")
                .and_then(toml::Value::as_table)
                .and_then(|table| table.get("address"))
                .and_then(toml::Value::as_str)
                .expect("network address");
            for addr in [torii_addr, network_addr] {
                let port = addr
                    .parse::<std::net::SocketAddr>()
                    .map(|socket| socket.port())
                    .expect("address contains a port");
                assert!(
                    seen_ports.insert(port),
                    "port {port} should be unique across exported configs"
                );
            }

            let genesis_file = value
                .get("genesis")
                .and_then(toml::Value::as_table)
                .and_then(|table| table.get("file"))
                .and_then(toml::Value::as_str)
                .expect("genesis file path");
            genesis_files.insert(genesis_file.to_owned());

            let nexus_table = value
                .get("nexus")
                .and_then(toml::Value::as_table)
                .expect("nexus config");
            assert_eq!(
                nexus_table.get("enabled").and_then(toml::Value::as_bool),
                Some(true)
            );
            assert_eq!(
                nexus_table
                    .get("lane_count")
                    .and_then(toml::Value::as_integer),
                Some(2)
            );
            let lane_catalog = nexus_table
                .get("lane_catalog")
                .and_then(toml::Value::as_array)
                .expect("lane catalog");
            assert_eq!(lane_catalog.len(), 2);
            let lane0 = lane_catalog[0].as_table().expect("lane0 table");
            assert_eq!(
                lane0.get("alias").and_then(toml::Value::as_str),
                Some("core")
            );
            assert_eq!(
                lane0.get("index").and_then(toml::Value::as_integer),
                Some(0)
            );
            let lane1 = lane_catalog[1].as_table().expect("lane1 table");
            assert_eq!(
                lane1.get("alias").and_then(toml::Value::as_str),
                Some("governance")
            );
            assert_eq!(
                lane1.get("index").and_then(toml::Value::as_integer),
                Some(1)
            );
            let dataspace_catalog = nexus_table
                .get("dataspace_catalog")
                .and_then(toml::Value::as_array)
                .expect("dataspace catalog");
            assert_eq!(dataspace_catalog.len(), 1);
            let dataspace = dataspace_catalog[0].as_table().expect("dataspace table");
            assert_eq!(
                dataspace.get("alias").and_then(toml::Value::as_str),
                Some("global")
            );
            assert_eq!(
                dataspace.get("id").and_then(toml::Value::as_integer),
                Some(0)
            );
        }

        assert_eq!(
            genesis_files.len(),
            1,
            "all peers should share the same genesis manifest path"
        );
    }

    #[test]
    fn restore_snapshot_replaces_storage_and_configs() {
        if !ports_available("restore_snapshot_replaces_storage_and_configs") {
            return;
        }
        let _env = env_lock().lock().expect("env lock");
        let temp = tempfile::tempdir().expect("tempdir");
        let _stub = KagamiStub::install(temp.path());
        let mut supervisor = SupervisorBuilder::new(ProfilePreset::SinglePeer)
            .data_root(temp.path())
            .build()
            .expect("build supervisor");

        let peer = &supervisor.peers()[0];
        let storage_dir = peer.storage_dir().to_path_buf();
        let snapshot_dir = peer.snapshot_dir().to_path_buf();
        let config_path = peer.config_path().to_path_buf();
        let log_path = peer.log_path().to_path_buf();
        let genesis_path = supervisor.genesis_manifest().to_path_buf();

        fs::write(storage_dir.join("marker.txt"), b"snapshot-data").expect("write storage marker");
        fs::write(snapshot_dir.join("inner.txt"), b"snapshot-inner").expect("write snapshot file");
        let original_config = fs::read(&config_path).expect("read original config");
        if let Some(parent) = log_path.parent() {
            fs::create_dir_all(parent).expect("create log directory");
        }
        fs::write(&log_path, b"snapshot-log").expect("write log file");

        let snapshot_root = supervisor
            .export_snapshot(Some("Restore Demo 2026"))
            .expect("export snapshot");

        fs::write(storage_dir.join("marker.txt"), b"mutated-storage")
            .expect("overwrite storage marker");
        fs::remove_file(snapshot_dir.join("inner.txt")).expect("remove snapshot file");
        fs::write(&config_path, b"mutated-config").expect("mutate config");
        fs::write(&log_path, b"mutated-log").expect("mutate log");

        supervisor
            .restore_snapshot(&snapshot_root)
            .expect("restore snapshot by path");

        assert_eq!(
            fs::read(storage_dir.join("marker.txt")).expect("read storage marker after restore"),
            b"snapshot-data"
        );
        assert!(
            snapshot_dir.join("inner.txt").exists(),
            "snapshot directory should be restored"
        );
        assert_eq!(
            fs::read(&config_path).expect("read restored config"),
            original_config
        );
        assert_eq!(
            fs::read(&log_path).expect("read restored log"),
            b"snapshot-log"
        );
        assert_eq!(
            fs::read(&genesis_path).expect("read restored genesis"),
            fs::read(snapshot_root.join("genesis").join(GENESIS_FILE_NAME))
                .expect("read snapshot genesis")
        );

        let snapshot_name = snapshot_root
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_string();
        fs::write(storage_dir.join("marker.txt"), b"mutated-again").expect("mutate storage");
        supervisor
            .restore_snapshot(snapshot_name.as_str())
            .expect("restore snapshot by label");
        assert_eq!(
            fs::read(storage_dir.join("marker.txt")).expect("read storage marker"),
            b"snapshot-data"
        );
    }

    #[test]
    fn restore_snapshot_rejects_genesis_hash_mismatch() {
        if !ports_available("restore_snapshot_rejects_genesis_hash_mismatch") {
            return;
        }
        let _env = env_lock().lock().expect("env lock");
        let temp = tempfile::tempdir().expect("tempdir");
        let _stub = KagamiStub::install(temp.path());
        let mut supervisor = SupervisorBuilder::new(ProfilePreset::SinglePeer)
            .data_root(temp.path())
            .build()
            .expect("build supervisor");

        let snapshot_root = supervisor
            .export_snapshot(Some("Genesis Hash Mismatch"))
            .expect("export snapshot");

        fs::write(supervisor.genesis_manifest(), b"mutated-genesis").expect("mutate genesis");

        let err = supervisor
            .restore_snapshot(&snapshot_root)
            .expect_err("restore should fail when genesis hash mismatches");
        match err {
            SupervisorError::Config(message) => assert!(
                message.contains("genesis hash"),
                "expected genesis hash mismatch message, got `{message}`"
            ),
            other => panic!("expected SupervisorError::Config, got {other:?}"),
        }
    }

    #[test]
    fn restore_snapshot_rejects_kura_hash_tampering() {
        if !ports_available("restore_snapshot_rejects_kura_hash_tampering") {
            return;
        }
        let _env = env_lock().lock().expect("env lock");
        let temp = tempfile::tempdir().expect("tempdir");
        let _stub = KagamiStub::install(temp.path());
        let mut supervisor = SupervisorBuilder::new(ProfilePreset::SinglePeer)
            .data_root(temp.path())
            .build()
            .expect("build supervisor");

        let snapshot_root = supervisor
            .export_snapshot(Some("Kura Tamper"))
            .expect("export snapshot");
        let alias = supervisor.peers()[0].alias().to_owned();
        let storage_copy = snapshot_root.join("peers").join(&alias).join("storage");
        fs::write(storage_copy.join("tamper.bin"), b"tampered").expect("mutate snapshot storage");

        let err = supervisor
            .restore_snapshot(&snapshot_root)
            .expect_err("restore should fail when kura hash mismatches metadata");
        match err {
            SupervisorError::Config(message) => assert!(
                message.contains("integrity check") && message.contains(&alias),
                "expected kura integrity error mentioning alias; got `{message}`"
            ),
            other => panic!("expected SupervisorError::Config, got {other:?}"),
        }
    }

    #[test]
    fn restore_snapshot_rejects_chain_mismatch() {
        if !ports_available("restore_snapshot_rejects_chain_mismatch") {
            return;
        }
        let _env = env_lock().lock().expect("env lock");
        let temp = tempfile::tempdir().expect("tempdir");
        let _stub = KagamiStub::install(temp.path());
        let mut supervisor = SupervisorBuilder::new(ProfilePreset::SinglePeer)
            .data_root(temp.path())
            .build()
            .expect("build supervisor");

        let snapshot_root = supervisor
            .export_snapshot(Some("Chain Mismatch"))
            .expect("export snapshot");
        let metadata_path = snapshot_root.join("metadata.json");
        let mut metadata: Value =
            norito::json::from_slice(&fs::read(&metadata_path).expect("read metadata"))
                .expect("parse metadata");
        metadata
            .as_object_mut()
            .expect("metadata should be an object")
            .insert("chain_id".into(), Value::String("other-chain".into()));
        fs::write(
            &metadata_path,
            json::to_vec_pretty(&metadata).expect("serialize metadata"),
        )
        .expect("write mutated metadata");

        fs::write(
            supervisor.peers()[0].storage_dir().join("marker.txt"),
            b"mutated",
        )
        .expect("mutate storage");

        let err = supervisor
            .restore_snapshot(&snapshot_root)
            .expect_err("restore should fail when chains mismatch");
        match err {
            SupervisorError::Config(message) => assert!(
                message.contains("other-chain"),
                "expected chain mismatch message, got `{message}`"
            ),
            other => panic!("expected SupervisorError::Config, got {other:?}"),
        }
    }

    #[test]
    fn supervisor_respects_explicit_kagami_override() {
        if !ports_available("supervisor_respects_explicit_kagami_override") {
            return;
        }
        let _env = env_lock().lock().expect("env lock");
        let temp = tempfile::tempdir().expect("tempdir");
        let stub = StandaloneKagamiStub::create(temp.path());
        SupervisorBuilder::new(ProfilePreset::SinglePeer)
            .data_root(temp.path())
            .kagami_path(stub.script_path())
            .build()
            .expect("build supervisor with explicit kagami path");

        let log = fs::read_to_string(stub.log_path()).expect("explicit kagami log");
        assert!(
            log.contains("--genesis-public-key"),
            "expected explicit kagami stub to capture genesis args, got `{log}`"
        );
    }

    #[test]
    fn supervisor_runs_kagami_verify_for_profile() {
        if !ports_available("supervisor_runs_kagami_verify_for_profile") {
            return;
        }
        let _env = env_lock().lock().expect("env lock");
        let temp = tempfile::tempdir().expect("tempdir");
        let stub = StandaloneKagamiStub::create(temp.path());
        let _guard = EnvVarGuard::set("MOCHI_KAGAMI", stub.script_path().as_os_str());

        let _supervisor = SupervisorBuilder::new(ProfilePreset::SinglePeer)
            .data_root(temp.path())
            .genesis_profile(GenesisProfile::Iroha3Dev)
            .build()
            .expect("build supervisor with kagami verify");

        let log = fs::read_to_string(stub.log_path()).expect("read kagami log");
        assert!(
            log.contains("genesis generate"),
            "expected kagami generate invocation, got `{log}`"
        );
        assert!(
            log.contains("verify"),
            "expected kagami verify invocation, got `{log}`"
        );
    }

    #[test]
    fn existing_peer_directories_are_cleaned_before_build() {
        if !ports_available("existing_peer_directories_are_cleaned_before_build") {
            return;
        }
        let _env = env_lock().lock().expect("env lock");
        let temp = tempfile::tempdir().expect("tempdir");

        let slug = NetworkProfile::from_preset(ProfilePreset::SinglePeer).slug();
        let root = temp.path().join(slug);
        let peer_dir = root.join("peers").join("peer0");
        let stale_file = peer_dir.join("stale.bin");
        fs::create_dir_all(&peer_dir).expect("create stale peer dir");
        fs::write(&stale_file, b"leftover").expect("write stale file");
        assert!(
            stale_file.exists(),
            "stale file should exist before supervisor build"
        );

        let _stub = KagamiStub::install(temp.path());
        let supervisor = SupervisorBuilder::new(ProfilePreset::SinglePeer)
            .data_root(temp.path())
            .build()
            .expect("build supervisor");

        assert!(
            !stale_file.exists(),
            "stale peer artefacts should be removed during build"
        );

        let rebuilt_storage = supervisor.peers()[0].spec.storage_dir.clone();
        assert!(
            rebuilt_storage.exists(),
            "storage directory should be recreated after cleanup"
        );
    }

    #[test]
    fn wipe_and_regenerate_resets_storage_and_genesis() {
        if !ports_available("wipe_and_regenerate_resets_storage_and_genesis") {
            return;
        }
        let _env = env_lock().lock().expect("env lock");
        let temp = tempfile::tempdir().expect("tempdir");
        let _stub = KagamiStub::install(temp.path());
        let mut supervisor = SupervisorBuilder::new(ProfilePreset::FourPeerBft)
            .data_root(temp.path())
            .build()
            .expect("build supervisor");

        for (idx, peer) in supervisor.peers().iter().enumerate() {
            let storage_file = peer.storage_dir().join(format!("leftover-{idx}.bin"));
            fs::write(&storage_file, b"stale").expect("write stale storage file");
            assert!(
                storage_file.exists(),
                "stale storage file should exist before wipe"
            );

            let snapshot_file = peer.snapshot_dir().join("stale.txt");
            fs::write(&snapshot_file, b"stale-snapshot").expect("write stale snapshot file");
            assert!(
                snapshot_file.exists(),
                "stale snapshot file should exist before wipe"
            );
        }

        let genesis_path = supervisor.genesis_manifest().to_path_buf();
        fs::write(&genesis_path, b"not-json").expect("corrupt genesis manifest");

        supervisor
            .wipe_and_regenerate()
            .expect("wipe and regenerate should succeed");

        let manifest_bytes = fs::read(&genesis_path).expect("read regenerated genesis");
        let manifest: Value =
            norito::json::from_slice(&manifest_bytes).expect("genesis should be valid JSON");
        assert_eq!(
            manifest
                .get("chain")
                .and_then(Value::as_str)
                .expect("chain field present"),
            supervisor.chain_id(),
            "regenerated genesis should carry supervisor chain id"
        );

        for (idx, peer) in supervisor.peers().iter().enumerate() {
            let storage_file = peer.storage_dir().join(format!("leftover-{idx}.bin"));
            assert!(
                !storage_file.exists(),
                "wipe should remove stale storage file for peer {}",
                peer.alias()
            );
            let snapshot_file = peer.snapshot_dir().join("stale.txt");
            assert!(
                !snapshot_file.exists(),
                "wipe should remove stale snapshot file for peer {}",
                peer.alias()
            );
        }
    }

    #[test]
    fn genesis_topology_matches_peer_configuration_across_presets() {
        if !ports_available("genesis_topology_matches_peer_configuration_across_presets") {
            return;
        }
        let _env = env_lock().lock().expect("env lock");
        let temp = tempfile::tempdir().expect("tempdir");
        let _stub = KagamiStub::install(temp.path());

        for preset in [ProfilePreset::SinglePeer, ProfilePreset::FourPeerBft] {
            let supervisor = SupervisorBuilder::new(preset)
                .data_root(temp.path())
                .build()
                .expect("build supervisor");

            let bytes = fs::read(supervisor.genesis_manifest()).expect("genesis manifest readable");
            let manifest: norito::json::Value =
                norito::json::from_slice(&bytes).expect("parse genesis json");
            let transactions = manifest
                .get("transactions")
                .and_then(norito::json::Value::as_array)
                .expect("transactions array");
            let topology = transactions
                .iter()
                .filter_map(|tx| tx.get("topology").and_then(norito::json::Value::as_array))
                .find(|entries| !entries.is_empty())
                .expect("non-empty topology transaction present");

            let actual_peer_ids: Vec<PeerId> = topology
                .iter()
                .map(|entry| {
                    norito::json::from_value(entry.clone()).expect("topology entry should decode")
                })
                .collect();
            let expected_peer_ids: Vec<PeerId> = supervisor
                .peers()
                .iter()
                .map(|peer| peer.peer_id())
                .collect();

            assert_eq!(
                actual_peer_ids, expected_peer_ids,
                "topology should mirror prepared peers for preset {preset:?}"
            );

            let chain = manifest
                .get("chain")
                .and_then(norito::json::Value::as_str)
                .expect("chain field");
            assert_eq!(
                chain,
                supervisor.chain_id(),
                "manifest chain id should match supervisor for preset {preset:?}"
            );
        }
    }

    #[test]
    fn peer_spec_peer_id_roundtrip() {
        let temp = tempfile::tempdir().expect("tempdir");
        let paths = NetworkPaths::from_root(temp.path(), &NetworkProfile::default());
        paths.ensure().expect("paths");
        let spec = PeerSpec::new(&paths, "peer0".into(), 8080, 1337).expect("peer spec");
        let peer_id = spec.peer_id();
        let parsed: PublicKey = peer_id.public_key().clone();
        assert_eq!(parsed, spec.keys.public_key);
    }

    #[test]
    fn normalize_peer_config_overrides_sets_lane_count_and_da_enabled() {
        let mut nexus = toml::Table::new();
        nexus.insert("enabled".into(), toml::Value::Boolean(true));
        let mut lane0 = toml::Table::new();
        lane0.insert("alias".into(), toml::Value::String("core".into()));
        lane0.insert("index".into(), toml::Value::Integer(0));
        let mut lane1 = toml::Table::new();
        lane1.insert("alias".into(), toml::Value::String("governance".into()));
        lane1.insert("index".into(), toml::Value::Integer(1));
        nexus.insert(
            "lane_catalog".into(),
            toml::Value::Array(vec![toml::Value::Table(lane0), toml::Value::Table(lane1)]),
        );
        let mut nexus = Some(nexus);
        let mut sumeragi = None;
        let mut torii = None;

        normalize_peer_config_overrides(&mut nexus, &mut sumeragi, &mut torii)
            .expect("normalize overrides");

        let nexus = nexus.expect("nexus config");
        assert_eq!(
            nexus.get("lane_count").and_then(toml::Value::as_integer),
            Some(2)
        );
        let sumeragi = sumeragi.expect("sumeragi config");
        assert!(matches!(
            sumeragi.get("da_enabled"),
            Some(toml::Value::Boolean(true))
        ));
    }

    #[test]
    fn normalize_peer_config_overrides_rejects_disabled_nexus_with_lanes() {
        let mut nexus = toml::Table::new();
        nexus.insert("enabled".into(), toml::Value::Boolean(false));
        nexus.insert("lane_count".into(), toml::Value::Integer(3));
        let mut nexus = Some(nexus);
        let mut sumeragi = None;
        let mut torii = None;

        let err = normalize_peer_config_overrides(&mut nexus, &mut sumeragi, &mut torii)
            .expect_err("disabled nexus should fail");
        match err {
            SupervisorError::Config(message) => assert!(
                message.contains("nexus.enabled = false"),
                "unexpected error: {message}"
            ),
            other => panic!("expected SupervisorError::Config, got {other:?}"),
        }
    }

    #[test]
    fn supervisor_exposes_config_overrides() {
        if !ports_available("supervisor_exposes_config_overrides") {
            return;
        }
        let _env = env_lock().lock().expect("env lock");
        let temp = tempfile::tempdir().expect("temp dir");
        let _stub = KagamiStub::install(temp.path());

        let mut nexus = toml::Table::new();
        nexus.insert("enabled".into(), toml::Value::Boolean(true));
        let mut sumeragi = toml::Table::new();
        sumeragi.insert("da_enabled".into(), toml::Value::Boolean(true));
        let mut torii = toml::Table::new();
        torii.insert(
            "address".into(),
            toml::Value::String("127.0.0.1:8080".to_owned()),
        );

        let supervisor = SupervisorBuilder::new(ProfilePreset::SinglePeer)
            .data_root(temp.path())
            .nexus_config(nexus)
            .sumeragi_config(sumeragi)
            .torii_config(torii)
            .build()
            .expect("build supervisor");

        let nexus = supervisor
            .nexus_config_overrides()
            .expect("nexus overrides");
        assert!(matches!(
            nexus.get("enabled"),
            Some(toml::Value::Boolean(true))
        ));
        let sumeragi = supervisor
            .sumeragi_config_overrides()
            .expect("sumeragi overrides");
        assert!(matches!(
            sumeragi.get("da_enabled"),
            Some(toml::Value::Boolean(true))
        ));
        let torii = supervisor
            .torii_config_overrides()
            .expect("torii overrides");
        assert!(matches!(
            torii.get("address"),
            Some(toml::Value::String(value)) if value == "127.0.0.1:8080"
        ));
    }

    #[test]
    fn lane_slug_sanitizes_alias() {
        assert_eq!(lane_slug("Core Lane", 0), "core_lane");
        assert_eq!(lane_slug("Gov+Ops", 2), "gov_ops");
        assert_eq!(lane_slug("---", 3), "lane3");
    }

    #[test]
    fn lane_path_comments_include_default_aliases_for_multilane() {
        let temp = tempfile::tempdir().expect("temp dir");
        let mut nexus = toml::Table::new();
        nexus.insert("enabled".into(), toml::Value::Boolean(true));
        nexus.insert("lane_count".into(), toml::Value::Integer(3));

        let comments = lane_path_comments(temp.path(), Some(&nexus));
        assert!(
            comments
                .iter()
                .any(|line| line.contains("mochi.lane[0].alias = default"))
        );
        assert!(
            comments
                .iter()
                .any(|line| line.contains("mochi.lane[1].alias = lane1"))
        );
        assert!(
            comments
                .iter()
                .any(|line| line.contains("mochi.lane[2].alias = lane2"))
        );
    }

    #[test]
    fn peer_spec_writes_nexus_and_da_overrides() {
        let temp = tempfile::tempdir().expect("temp dir");
        let profile = NetworkProfile::default();
        let paths = NetworkPaths::from_root(temp.path(), &profile);
        paths.ensure().expect("paths");
        let spec = PeerSpec::new(&paths, "peer0".into(), 8080, 1337).expect("peer spec");

        let manifest_path = paths.genesis_dir().join("genesis.json");
        fs::create_dir_all(paths.genesis_dir()).expect("genesis dir");
        fs::write(&manifest_path, b"{}").expect("write manifest");
        let genesis = GenesisMaterial {
            key_pair: KeyPair::random(),
            manifest_path: manifest_path.clone(),
            profile: None,
            vrf_seed_hex: None,
            verify_report: None,
            consensus_fingerprint: None,
        };

        let mut nexus = toml::Table::new();
        nexus.insert("enabled".into(), toml::Value::Boolean(true));
        nexus.insert("lane_count".into(), toml::Value::Integer(1));
        let mut sumeragi = toml::Table::new();
        sumeragi.insert("da_enabled".into(), toml::Value::Boolean(true));
        let overrides = PeerConfigOverrides {
            nexus: Some(nexus),
            sumeragi: Some(sumeragi),
            torii: None,
        };
        let specs = vec![spec.clone()];
        spec.write_config("demo-chain", &genesis, &specs, &overrides)
            .expect("write config");

        let contents = fs::read_to_string(&spec.config_path).expect("read config");
        let value: toml::Table = toml::from_str(&contents).expect("parse config");
        let nexus = value
            .get("nexus")
            .and_then(toml::Value::as_table)
            .expect("nexus table");
        assert!(matches!(
            nexus.get("enabled"),
            Some(toml::Value::Boolean(true))
        ));
        let sumeragi = value
            .get("sumeragi")
            .and_then(toml::Value::as_table)
            .expect("sumeragi table");
        assert!(matches!(
            sumeragi.get("da_enabled"),
            Some(toml::Value::Boolean(true))
        ));
        let torii = value
            .get("torii")
            .and_then(toml::Value::as_table)
            .expect("torii table");
        let expected_torii_dir = spec.storage_dir.join("torii").display().to_string();
        assert_eq!(
            torii.get("data_dir").and_then(toml::Value::as_str),
            Some(expected_torii_dir.as_str())
        );
        let da_ingest = torii
            .get("da_ingest")
            .and_then(toml::Value::as_table)
            .expect("da_ingest table");
        let expected_replay = spec
            .storage_dir
            .join("torii")
            .join("da_replay")
            .display()
            .to_string();
        assert_eq!(
            da_ingest
                .get("replay_cache_store_dir")
                .and_then(toml::Value::as_str),
            Some(expected_replay.as_str())
        );
        let expected_manifest = spec
            .storage_dir
            .join("torii")
            .join("da_manifests")
            .display()
            .to_string();
        assert_eq!(
            da_ingest
                .get("manifest_store_dir")
                .and_then(toml::Value::as_str),
            Some(expected_manifest.as_str())
        );
    }

    #[test]
    fn peer_spec_config_omits_da_ingest_when_da_disabled() {
        let temp = tempfile::tempdir().expect("temp dir");
        let profile = NetworkProfile::default();
        let paths = NetworkPaths::from_root(temp.path(), &profile);
        paths.ensure().expect("paths");
        let spec = PeerSpec::new(&paths, "peer0".into(), 8080, 1337).expect("peer spec");

        let manifest_path = paths.genesis_dir().join("genesis.json");
        fs::create_dir_all(paths.genesis_dir()).expect("genesis dir");
        fs::write(&manifest_path, b"{}").expect("write manifest");
        let genesis = GenesisMaterial {
            key_pair: KeyPair::random(),
            manifest_path: manifest_path.clone(),
            profile: None,
            vrf_seed_hex: None,
            verify_report: None,
            consensus_fingerprint: None,
        };

        let mut sumeragi = toml::Table::new();
        sumeragi.insert("da_enabled".into(), toml::Value::Boolean(false));
        let overrides = PeerConfigOverrides {
            nexus: None,
            sumeragi: Some(sumeragi),
            torii: None,
        };
        let specs = vec![spec.clone()];
        spec.write_config("demo-chain", &genesis, &specs, &overrides)
            .expect("write config");

        let contents = fs::read_to_string(&spec.config_path).expect("read config");
        let value: toml::Table = toml::from_str(&contents).expect("parse config");
        let torii = value
            .get("torii")
            .and_then(toml::Value::as_table)
            .expect("torii table");
        assert!(
            torii.get("da_ingest").is_none(),
            "da_ingest should be absent when DA is disabled"
        );
    }

    #[test]
    fn peer_spec_config_honors_torii_da_ingest_overrides() {
        let temp = tempfile::tempdir().expect("temp dir");
        let profile = NetworkProfile::default();
        let paths = NetworkPaths::from_root(temp.path(), &profile);
        paths.ensure().expect("paths");
        let spec = PeerSpec::new(&paths, "peer0".into(), 8080, 1337).expect("peer spec");

        let manifest_path = paths.genesis_dir().join("genesis.json");
        fs::create_dir_all(paths.genesis_dir()).expect("genesis dir");
        fs::write(&manifest_path, b"{}").expect("write manifest");
        let genesis = GenesisMaterial {
            key_pair: KeyPair::random(),
            manifest_path: manifest_path.clone(),
            profile: None,
            vrf_seed_hex: None,
            verify_report: None,
            consensus_fingerprint: None,
        };

        let mut sumeragi = toml::Table::new();
        sumeragi.insert("da_enabled".into(), toml::Value::Boolean(true));
        let mut da_ingest = toml::Table::new();
        da_ingest.insert(
            "replay_cache_store_dir".into(),
            toml::Value::String("/custom/replay".to_owned()),
        );
        da_ingest.insert(
            "manifest_store_dir".into(),
            toml::Value::String("/custom/manifests".to_owned()),
        );
        let mut torii = toml::Table::new();
        torii.insert("da_ingest".into(), toml::Value::Table(da_ingest));
        let overrides = PeerConfigOverrides {
            nexus: None,
            sumeragi: Some(sumeragi),
            torii: Some(torii),
        };
        let specs = vec![spec.clone()];
        spec.write_config("demo-chain", &genesis, &specs, &overrides)
            .expect("write config");

        let contents = fs::read_to_string(&spec.config_path).expect("read config");
        let value: toml::Table = toml::from_str(&contents).expect("parse config");
        let torii = value
            .get("torii")
            .and_then(toml::Value::as_table)
            .expect("torii table");
        let da_ingest = torii
            .get("da_ingest")
            .and_then(toml::Value::as_table)
            .expect("da_ingest table");
        assert_eq!(
            da_ingest
                .get("replay_cache_store_dir")
                .and_then(toml::Value::as_str),
            Some("/custom/replay")
        );
        assert_eq!(
            da_ingest
                .get("manifest_store_dir")
                .and_then(toml::Value::as_str),
            Some("/custom/manifests")
        );
    }

    #[test]
    fn peer_spec_config_header_includes_lane_paths() {
        let temp = tempfile::tempdir().expect("temp dir");
        let profile = NetworkProfile::default();
        let paths = NetworkPaths::from_root(temp.path(), &profile);
        paths.ensure().expect("paths");
        let spec = PeerSpec::new(&paths, "peer0".into(), 8080, 1337).expect("peer spec");

        let manifest_path = paths.genesis_dir().join("genesis.json");
        fs::create_dir_all(paths.genesis_dir()).expect("genesis dir");
        fs::write(&manifest_path, b"{}").expect("write manifest");
        let genesis = GenesisMaterial {
            key_pair: KeyPair::random(),
            manifest_path: manifest_path.clone(),
            profile: None,
            vrf_seed_hex: None,
            verify_report: None,
            consensus_fingerprint: None,
        };

        let mut lane0 = toml::Table::new();
        lane0.insert("alias".into(), toml::Value::String("Core Lane".into()));
        lane0.insert("index".into(), toml::Value::Integer(0));
        let mut lane1 = toml::Table::new();
        lane1.insert("alias".into(), toml::Value::String("Gov+Ops".into()));
        lane1.insert("index".into(), toml::Value::Integer(1));
        let mut nexus = toml::Table::new();
        nexus.insert("enabled".into(), toml::Value::Boolean(true));
        nexus.insert("lane_count".into(), toml::Value::Integer(2));
        nexus.insert(
            "lane_catalog".into(),
            toml::Value::Array(vec![toml::Value::Table(lane0), toml::Value::Table(lane1)]),
        );
        let overrides = PeerConfigOverrides {
            nexus: Some(nexus),
            sumeragi: None,
            torii: None,
        };
        let specs = vec![spec.clone()];
        spec.write_config("demo-chain", &genesis, &specs, &overrides)
            .expect("write config");

        let contents = fs::read_to_string(&spec.config_path).expect("read config");
        let lane0_slug = lane_slug("Core Lane", 0);
        let lane1_slug = lane_slug("Gov+Ops", 1);
        let lane0_blocks = spec
            .storage_dir
            .join("blocks")
            .join(format!("lane_000_{lane0_slug}"))
            .display()
            .to_string();
        let lane1_blocks = spec
            .storage_dir
            .join("blocks")
            .join(format!("lane_001_{lane1_slug}"))
            .display()
            .to_string();
        let lane0_merge = spec
            .storage_dir
            .join("merge_ledger")
            .join(format!("lane_000_{lane0_slug}_merge.log"))
            .display()
            .to_string();
        let lane1_merge = spec
            .storage_dir
            .join("merge_ledger")
            .join(format!("lane_001_{lane1_slug}_merge.log"))
            .display()
            .to_string();

        assert!(contents.contains("# mochi.lane[0].alias = Core Lane"));
        assert!(contents.contains("# mochi.lane[1].alias = Gov+Ops"));
        assert!(contents.contains(&format!("# mochi.lane[0].blocks_dir = {lane0_blocks}")));
        assert!(contents.contains(&format!("# mochi.lane[1].blocks_dir = {lane1_blocks}")));
        assert!(contents.contains(&format!("# mochi.lane[0].merge_log = {lane0_merge}")));
        assert!(contents.contains(&format!("# mochi.lane[1].merge_log = {lane1_merge}")));
    }

    #[test]
    fn reset_lane_storage_removes_lane_segments() {
        if !ports_available("reset_lane_storage_removes_lane_segments") {
            return;
        }
        let _env = env_lock().lock().expect("env lock");
        let temp = tempfile::tempdir().expect("temp dir");
        let _stub = KagamiStub::install(temp.path());

        let mut lane0 = toml::Table::new();
        lane0.insert("alias".into(), toml::Value::String("core".into()));
        lane0.insert("index".into(), toml::Value::Integer(0));
        let mut lane1 = toml::Table::new();
        lane1.insert("alias".into(), toml::Value::String("Gov+Ops".into()));
        lane1.insert("index".into(), toml::Value::Integer(1));
        let mut nexus = toml::Table::new();
        nexus.insert("enabled".into(), toml::Value::Boolean(true));
        nexus.insert("lane_count".into(), toml::Value::Integer(2));
        nexus.insert(
            "lane_catalog".into(),
            toml::Value::Array(vec![toml::Value::Table(lane0), toml::Value::Table(lane1)]),
        );
        let mut sumeragi = toml::Table::new();
        sumeragi.insert("da_enabled".into(), toml::Value::Boolean(true));

        let mut supervisor = SupervisorBuilder::new(ProfilePreset::SinglePeer)
            .data_root(temp.path())
            .nexus_config(nexus)
            .sumeragi_config(sumeragi)
            .build()
            .expect("build supervisor");

        let peer = supervisor.peers().first().expect("peer available");
        let lane0_slug = lane_slug("core", 0);
        let lane1_slug = lane_slug("Gov+Ops", 1);
        let lane0_blocks = peer
            .storage_dir()
            .join("blocks")
            .join(format!("lane_000_{lane0_slug}"));
        let lane1_blocks = peer
            .storage_dir()
            .join("blocks")
            .join(format!("lane_001_{lane1_slug}"));
        let lane0_merge = peer
            .storage_dir()
            .join("merge_ledger")
            .join(format!("lane_000_{lane0_slug}_merge.log"));
        let lane1_merge = peer
            .storage_dir()
            .join("merge_ledger")
            .join(format!("lane_001_{lane1_slug}_merge.log"));

        fs::create_dir_all(&lane0_blocks).expect("lane0 blocks");
        fs::create_dir_all(&lane1_blocks).expect("lane1 blocks");
        fs::create_dir_all(lane0_merge.parent().expect("merge parent")).expect("merge ledger dir");
        fs::write(&lane0_merge, b"lane0").expect("lane0 merge");
        fs::write(&lane1_merge, b"lane1").expect("lane1 merge");

        assert!(lane0_blocks.exists());
        assert!(lane1_blocks.exists());
        assert!(lane0_merge.exists());
        assert!(lane1_merge.exists());

        supervisor
            .reset_lane_storage(1)
            .expect("reset lane storage");

        assert!(lane0_blocks.exists(), "lane0 blocks should remain");
        assert!(lane0_merge.exists(), "lane0 merge should remain");
        assert!(!lane1_blocks.exists(), "lane1 blocks should be removed");
        assert!(!lane1_merge.exists(), "lane1 merge should be removed");
    }

    #[test]
    fn managed_block_stream_unknown_peer_errors() {
        if !ports_available("managed_block_stream_unknown_peer_errors") {
            return;
        }
        let runtime = Runtime::new().expect("runtime");
        let _env = env_lock().lock().expect("env lock");
        let temp = tempfile::tempdir().expect("tempdir");
        let _stub = KagamiStub::install(temp.path());
        let supervisor = SupervisorBuilder::new(ProfilePreset::SinglePeer)
            .data_root(temp.path())
            .build()
            .expect("build supervisor");

        let err = supervisor
            .managed_block_stream("missing", runtime.handle())
            .expect_err("unknown peer should fail");
        matches!(err, SupervisorError::PeerUnknown { .. });
    }

    #[test]
    fn managed_block_stream_returns_handle_for_peer() {
        if !ports_available("managed_block_stream_returns_handle_for_peer") {
            return;
        }
        let runtime = Runtime::new().expect("runtime");
        let _env = env_lock().lock().expect("env lock");
        let temp = tempfile::tempdir().expect("tempdir");
        let _stub = KagamiStub::install(temp.path());
        let supervisor = SupervisorBuilder::new(ProfilePreset::SinglePeer)
            .data_root(temp.path())
            .build()
            .expect("build supervisor");

        let stream = supervisor
            .managed_block_stream("peer0", runtime.handle())
            .expect("managed stream handle");

        assert_eq!(stream.alias(), "peer0");
        stream.abort();
    }

    #[test]
    fn restart_policy_backoff_scales() {
        let policy = RestartPolicy::OnFailure {
            max_restarts: 5,
            backoff: Duration::from_millis(500),
        };
        assert_eq!(policy.backoff_for(1), Duration::from_millis(500));
        assert_eq!(policy.backoff_for(2), Duration::from_millis(1000));
        assert_eq!(policy.backoff_for(3), Duration::from_millis(2000));
        assert_eq!(policy.backoff_for(6), Duration::from_millis(8000));
    }

    #[test]
    fn restart_policy_rejects_zero_attempt() {
        let policy = RestartPolicy::default();
        assert!(!policy.should_retry(0));
        assert_eq!(policy.backoff_for(0), Duration::ZERO);
    }

    #[test]
    fn manual_stop_cancels_pending_restart() {
        if !ports_available("manual_stop_cancels_pending_restart") {
            return;
        }
        let _env = env_lock().lock().expect("env lock");
        let temp = tempfile::tempdir().expect("tempdir");
        let _kagami = KagamiStub::install(temp.path());

        let irohad_stub = temp.path().join("irohad_stub.sh");
        let stub_script = r#"#!/bin/sh
exit 1
"#;
        fs::write(&irohad_stub, stub_script).expect("write irohad stub");
        #[cfg(unix)]
        {
            let mut perms = fs::metadata(&irohad_stub)
                .expect("stub metadata")
                .permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&irohad_stub, perms).expect("set stub perms");
        }
        let _irohad_guard = EnvVarGuard::set("MOCHI_IROHAD", irohad_stub.as_os_str());

        let mut supervisor = match SupervisorBuilder::new(ProfilePreset::SinglePeer)
            .data_root(temp.path())
            .restart_policy(RestartPolicy::OnFailure {
                max_restarts: 2,
                backoff: Duration::from_millis(200),
            })
            .build()
        {
            Ok(supervisor) => supervisor,
            Err(SupervisorError::Config(message))
                if message.contains("failed to allocate Torii port") =>
            {
                eprintln!("skipping manual_stop_cancels_pending_restart: {message}");
                return;
            }
            Err(err) => panic!("build supervisor: {err}"),
        };

        supervisor.start_peer("peer0").expect("start peer");
        // Stub exits immediately; refresh to observe the failure and schedule a restart.
        std::thread::sleep(Duration::from_millis(10));
        supervisor.refresh_peer_states();

        let peer = &supervisor.peers()[0];
        assert!(
            matches!(peer.state, PeerState::Restarting | PeerState::Stopped),
            "peer should schedule a restart after failure"
        );
        assert!(
            peer.next_restart_at.is_some(),
            "failure should set a restart timer"
        );

        supervisor
            .stop_peer("peer0")
            .expect("manual stop should succeed");

        let peer = &supervisor.peers()[0];
        assert!(
            peer.next_restart_at.is_none(),
            "restart timer should be cleared"
        );
        assert_eq!(peer.restart_attempts, 0);
        assert!(matches!(peer.state, PeerState::Stopped));

        // Allow enough time for the original backoff to elapse and confirm no restart occurs.
        std::thread::sleep(Duration::from_millis(250));
        supervisor.refresh_peer_states();

        let peer = &supervisor.peers()[0];
        assert!(
            peer.process.is_none(),
            "manual stop should keep the peer offline"
        );
        assert!(peer.next_restart_at.is_none());
        assert_eq!(peer.restart_attempts, 0);
        assert!(matches!(peer.state, PeerState::Stopped));
    }

    #[test]
    fn supervisor_exposes_log_stream() {
        if !ports_available("supervisor_exposes_log_stream") {
            return;
        }
        let _env = env_lock().lock().expect("env lock");
        let temp = tempfile::tempdir().expect("tempdir");
        let _stub = KagamiStub::install(temp.path());
        let supervisor = SupervisorBuilder::new(ProfilePreset::SinglePeer)
            .data_root(temp.path())
            .build()
            .expect("build supervisor");

        let stream = supervisor
            .log_stream("peer0")
            .expect("log stream should be available");
        assert_eq!(stream.alias(), "peer0");
    }

    #[test]
    fn start_peer_unknown_alias_errors() {
        if !ports_available("start_peer_unknown_alias_errors") {
            return;
        }
        let _env = env_lock().lock().expect("env lock");
        let temp = tempfile::tempdir().expect("tempdir");
        let _stub = KagamiStub::install(temp.path());
        let mut supervisor = match SupervisorBuilder::new(ProfilePreset::SinglePeer)
            .data_root(temp.path())
            .torii_base_port(20000)
            .p2p_base_port(30000)
            .build()
        {
            Ok(supervisor) => supervisor,
            Err(SupervisorError::Config(message))
                if message.contains("failed to allocate Torii port") =>
            {
                eprintln!("skipping start_peer_unknown_alias_errors: {message}");
                return;
            }
            Err(err) => panic!("build supervisor: {err}"),
        };

        let err = supervisor
            .start_peer("missing-peer")
            .expect_err("unknown peer should fail");
        assert!(matches!(err, SupervisorError::PeerUnknown { .. }));
    }

    #[test]
    fn stop_peer_unknown_alias_errors() {
        if !ports_available("stop_peer_unknown_alias_errors") {
            return;
        }
        let _env = env_lock().lock().expect("env lock");
        let temp = tempfile::tempdir().expect("tempdir");
        let _stub = KagamiStub::install(temp.path());
        let mut supervisor = match SupervisorBuilder::new(ProfilePreset::SinglePeer)
            .data_root(temp.path())
            .torii_base_port(20000)
            .p2p_base_port(30000)
            .build()
        {
            Ok(supervisor) => supervisor,
            Err(SupervisorError::Config(message))
                if message.contains("failed to allocate Torii port") =>
            {
                eprintln!("skipping stop_peer_unknown_alias_errors: {message}");
                return;
            }
            Err(err) => panic!("build supervisor: {err}"),
        };

        let err = supervisor
            .stop_peer("missing-peer")
            .expect_err("unknown peer should fail");
        assert!(matches!(err, SupervisorError::PeerUnknown { .. }));
    }
}
