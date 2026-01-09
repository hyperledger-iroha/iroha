//! Bundle configuration loading for the MOCHI egui shell.
//!
//! The desktop bundle ships with `config/local.toml` so operators can override
//! data directories, port allocations, and external binary paths without
//! recompiling the application. This module discovers that file (or an explicit
//! `MOCHI_CONFIG` override), parses the TOML document, and yields a strongly
//! typed configuration for the UI to apply while constructing the supervisor.

use std::{
    convert::TryFrom,
    env, fmt, fs, io,
    path::{Path, PathBuf},
    time::Duration,
};

use iroha_data_model::parameter::system::SumeragiConsensusMode;
use mochi_core::{
    GenesisProfile, NetworkProfile, ProfilePreset, SupervisorBuilder, supervisor::RestartPolicy,
};

const DEFAULT_RESTART_MAX_RESTARTS: usize = 3;
const DEFAULT_RESTART_BACKOFF_MS: u64 = 1_000;
use toml::{Value, map::Map};

/// Overrides for external binaries referenced by the supervisor.
#[derive(Debug, Default, Clone)]
pub struct BinaryOverrides {
    /// Optional override for the `irohad` executable.
    pub irohad: Option<PathBuf>,
    /// Optional override for the `kagami` executable.
    pub kagami: Option<PathBuf>,
    /// Optional override for the `iroha_cli` executable (reserved for future use).
    pub iroha_cli: Option<PathBuf>,
}

/// Parsed configuration extracted from `config/local.toml`.
#[derive(Debug, Default, Clone)]
pub struct BundleConfig {
    /// Optional override for the data root directory.
    pub data_root: Option<PathBuf>,
    /// Optional profile override (preset or custom) for the supervised topology.
    pub profile: Option<NetworkProfile>,
    /// Optional chain identifier override for generated configs.
    pub chain_id: Option<String>,
    /// Optional Kagami genesis profile preset.
    pub genesis_profile: Option<GenesisProfile>,
    /// Optional VRF seed (hex) for Kagami genesis profiles.
    pub vrf_seed_hex: Option<String>,
    /// Optional starting Torii port.
    pub torii_start: Option<u16>,
    /// Optional starting P2P port.
    pub p2p_start: Option<u16>,
    /// Optional binary overrides.
    pub binaries: BinaryOverrides,
    /// Allow the supervisor to build missing binaries automatically.
    pub build_binaries: Option<bool>,
    /// Whether readiness checks should submit a smoke transaction and wait for a commit.
    pub readiness_smoke: Option<bool>,
    /// Optional restart policy override.
    pub restart_policy: Option<RestartPolicy>,
    /// Optional Nexus config overrides applied to generated peer configs.
    pub nexus: Option<Map<String, Value>>,
    /// Optional Sumeragi config overrides applied to generated peer configs.
    pub sumeragi: Option<Map<String, Value>>,
    /// Optional Torii config overrides applied to generated peer configs.
    pub torii: Option<Map<String, Value>>,
}

#[derive(Debug, Clone)]
pub struct ResolvedBundleConfig {
    pub config: BundleConfig,
    pub path: PathBuf,
}

#[derive(Debug, Clone)]
struct ParsedProfile {
    profile: NetworkProfile,
    genesis_profile: Option<GenesisProfile>,
}

impl BundleConfig {
    /// Applies overrides onto a [`SupervisorBuilder`](mochi_core::SupervisorBuilder).
    pub fn apply_to(&self, mut builder: SupervisorBuilder) -> SupervisorBuilder {
        if let Some(root) = self.data_root.as_ref() {
            builder = builder.data_root(root.clone());
        }
        if let Some(profile) = self.profile.as_ref() {
            builder = builder.set_profile(profile.clone());
        }
        if let Some(port) = self.torii_start {
            builder = builder.torii_base_port(port);
        }
        if let Some(port) = self.p2p_start {
            builder = builder.p2p_base_port(port);
        }
        if let Some(chain_id) = self.chain_id.as_ref() {
            builder = builder.chain_id(chain_id.clone());
        }
        if let Some(profile) = self.genesis_profile {
            builder = builder.genesis_profile(profile);
        }
        if let Some(seed) = self.vrf_seed_hex.as_ref() {
            builder = builder.vrf_seed_hex(seed.clone());
        }
        if let Some(path) = self.binaries.irohad.as_ref() {
            builder = builder.irohad_path(path.clone());
        }
        if let Some(path) = self.binaries.kagami.as_ref() {
            builder = builder.kagami_path(path.clone());
        }
        if let Some(path) = self.binaries.iroha_cli.as_ref() {
            builder = builder.iroha_cli_path(path.clone());
        }
        if let Some(allow) = self.build_binaries {
            builder = builder.auto_build_binaries(allow);
        }
        if let Some(policy) = self.restart_policy {
            builder = builder.restart_policy(policy);
        }
        if let Some(nexus) = self.nexus.as_ref() {
            builder = builder.nexus_config(nexus.clone());
        }
        if let Some(sumeragi) = self.sumeragi.as_ref() {
            builder = builder.sumeragi_config(sumeragi.clone());
        }
        if let Some(torii) = self.torii.as_ref() {
            builder = builder.torii_config(torii.clone());
        }
        builder
    }

    pub fn set_data_root(&mut self, value: Option<PathBuf>) {
        self.data_root = value;
    }

    pub fn set_profile(&mut self, value: Option<NetworkProfile>) {
        self.profile = value;
    }

    pub fn set_chain_id(&mut self, value: Option<String>) {
        self.chain_id = value;
    }

    pub fn set_build_binaries(&mut self, value: Option<bool>) {
        self.build_binaries = value;
    }

    pub fn set_readiness_smoke(&mut self, value: Option<bool>) {
        self.readiness_smoke = value;
    }

    #[allow(dead_code)]
    pub fn set_genesis_profile(&mut self, value: Option<GenesisProfile>) {
        self.genesis_profile = value;
    }

    #[allow(dead_code)]
    pub fn set_vrf_seed_hex(&mut self, value: Option<String>) {
        self.vrf_seed_hex = value;
    }

    pub fn set_torii_start(&mut self, value: Option<u16>) {
        self.torii_start = value;
    }

    pub fn set_p2p_start(&mut self, value: Option<u16>) {
        self.p2p_start = value;
    }

    pub fn set_restart_policy(&mut self, value: Option<RestartPolicy>) {
        self.restart_policy = value;
    }

    pub fn write_to_path(&self, path: &Path) -> Result<(), ConfigError> {
        let mut root = existing_root_table(path)?;

        let mut supervisor = root
            .get("supervisor")
            .and_then(Value::as_table)
            .cloned()
            .unwrap_or_default();
        match self.data_root.as_ref() {
            Some(root_path) => {
                supervisor.insert(
                    "data_root".into(),
                    Value::String(root_path.display().to_string()),
                );
            }
            None => {
                supervisor.remove("data_root");
            }
        }
        let mut profile_includes_genesis = false;
        match self.profile.as_ref() {
            Some(profile) => {
                if let Some(preset) = profile.preset {
                    supervisor.insert("profile".into(), Value::String(preset.slug().to_owned()));
                } else {
                    let mut profile_table = Map::new();
                    profile_table.insert(
                        "peer_count".into(),
                        Value::Integer(profile.topology.peer_count as i64),
                    );
                    profile_table.insert(
                        "consensus_mode".into(),
                        Value::String(match profile.consensus_mode {
                            SumeragiConsensusMode::Permissioned => "permissioned".to_owned(),
                            SumeragiConsensusMode::Npos => "npos".to_owned(),
                        }),
                    );
                    if let Some(profile) = self.genesis_profile {
                        profile_includes_genesis = true;
                        profile_table
                            .insert("genesis_profile".into(), Value::String(profile.to_string()));
                    }
                    supervisor.insert("profile".into(), Value::Table(profile_table));
                }
            }
            None => {
                supervisor.remove("profile");
            }
        }
        match self.chain_id.as_ref() {
            Some(chain) => {
                supervisor.insert("chain_id".into(), Value::String(chain.clone()));
            }
            None => {
                supervisor.remove("chain_id");
            }
        }
        match self.genesis_profile {
            Some(_) if profile_includes_genesis => {
                supervisor.remove("genesis_profile");
            }
            Some(profile) => {
                supervisor.insert("genesis_profile".into(), Value::String(profile.to_string()));
            }
            None => {
                supervisor.remove("genesis_profile");
            }
        }
        match self.vrf_seed_hex.as_ref() {
            Some(seed) => {
                supervisor.insert("vrf_seed_hex".into(), Value::String(seed.clone()));
            }
            None => {
                supervisor.remove("vrf_seed_hex");
            }
        }
        match self.build_binaries {
            Some(allow) => {
                supervisor.insert("build_binaries".into(), Value::Boolean(allow));
            }
            None => {
                supervisor.remove("build_binaries");
            }
        }
        match self.readiness_smoke {
            Some(value) => {
                supervisor.insert("readiness_smoke".into(), Value::Boolean(value));
            }
            None => {
                supervisor.remove("readiness_smoke");
            }
        }
        match self.restart_policy {
            Some(policy) => {
                supervisor.insert("restart".into(), Value::Table(render_restart_table(policy)));
            }
            None => {
                supervisor.remove("restart");
            }
        }
        if supervisor.is_empty() {
            root.remove("supervisor");
        } else {
            root.insert("supervisor".into(), Value::Table(supervisor));
        }

        let mut ports = root
            .get("ports")
            .and_then(Value::as_table)
            .cloned()
            .unwrap_or_default();
        match self.torii_start {
            Some(port) => {
                ports.insert("torii_start".into(), Value::Integer(port.into()));
            }
            None => {
                ports.remove("torii_start");
            }
        }
        match self.p2p_start {
            Some(port) => {
                ports.insert("p2p_start".into(), Value::Integer(port.into()));
            }
            None => {
                ports.remove("p2p_start");
            }
        }
        if ports.is_empty() {
            root.remove("ports");
        } else {
            root.insert("ports".into(), Value::Table(ports));
        }

        let mut binaries = root
            .get("binaries")
            .and_then(Value::as_table)
            .cloned()
            .unwrap_or_default();
        match self.binaries.irohad.as_ref() {
            Some(path) => {
                binaries.insert("irohad".into(), Value::String(path.display().to_string()));
            }
            None => {
                binaries.remove("irohad");
            }
        }
        match self.binaries.kagami.as_ref() {
            Some(path) => {
                binaries.insert("kagami".into(), Value::String(path.display().to_string()));
            }
            None => {
                binaries.remove("kagami");
            }
        }
        match self.binaries.iroha_cli.as_ref() {
            Some(path) => {
                binaries.insert(
                    "iroha_cli".into(),
                    Value::String(path.display().to_string()),
                );
            }
            None => {
                binaries.remove("iroha_cli");
            }
        }
        if binaries.is_empty() {
            root.remove("binaries");
        } else {
            root.insert("binaries".into(), Value::Table(binaries));
        }

        match self.nexus.as_ref() {
            Some(nexus) => {
                root.insert("nexus".into(), Value::Table(nexus.clone()));
            }
            None => {
                root.remove("nexus");
            }
        }

        match self.sumeragi.as_ref() {
            Some(sumeragi) => {
                root.insert("sumeragi".into(), Value::Table(sumeragi.clone()));
            }
            None => {
                root.remove("sumeragi");
            }
        }

        match self.torii.as_ref() {
            Some(torii) => {
                root.insert("torii".into(), Value::Table(torii.clone()));
            }
            None => {
                root.remove("torii");
            }
        }

        let text = toml::to_string_pretty(&Value::Table(root)).map_err(|err| {
            ConfigError::new(format!(
                "failed to serialize config {}: {err}",
                path.display()
            ))
        })?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|err| {
                ConfigError::new(format!(
                    "failed to create config directory {}: {err}",
                    parent.display()
                ))
            })?;
        }
        fs::write(path, text).map_err(|err| {
            ConfigError::new(format!("failed to write config {}: {err}", path.display()))
        })
    }
}

/// Errors emitted while loading the bundle configuration.
#[derive(Debug)]
pub struct ConfigError {
    message: String,
}

impl ConfigError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for ConfigError {}

/// Discover and parse `config/local.toml` if present.
pub fn load_bundle_config() -> Result<Option<ResolvedBundleConfig>, ConfigError> {
    for (path, explicit) in candidate_paths() {
        match fs::read_to_string(&path) {
            Ok(contents) => {
                let config = parse_bundle_config(&path, &contents)?;
                return Ok(Some(ResolvedBundleConfig { config, path }));
            }
            Err(err) => {
                if explicit || err.kind() != io::ErrorKind::NotFound {
                    return Err(ConfigError::new(format!(
                        "failed to read config {}: {err}",
                        path.display()
                    )));
                }
            }
        }
    }
    Ok(None)
}

/// Load configuration from an explicit path, returning defaults if the file is absent.
pub fn load_bundle_config_at(path: PathBuf) -> Result<ResolvedBundleConfig, ConfigError> {
    match fs::read_to_string(&path) {
        Ok(contents) => {
            let config = parse_bundle_config(&path, &contents)?;
            Ok(ResolvedBundleConfig { config, path })
        }
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(ResolvedBundleConfig {
            config: BundleConfig::default(),
            path,
        }),
        Err(err) => Err(ConfigError::new(format!(
            "failed to read config {}: {err}",
            path.display()
        ))),
    }
}

pub fn default_config_path() -> PathBuf {
    candidate_paths()
        .into_iter()
        .map(|(path, _)| path)
        .next()
        .unwrap_or_else(|| PathBuf::from("config").join("local.toml"))
}

fn candidate_paths() -> Vec<(PathBuf, bool)> {
    let mut paths = Vec::new();

    if let Some(value) = env::var_os("MOCHI_CONFIG").filter(|value| !value.is_empty()) {
        push_candidate(&mut paths, PathBuf::from(value), true);
    }

    if let Ok(exe) = env::current_exe()
        && let Some(dir) = exe.parent()
    {
        push_candidate(&mut paths, dir.join("config").join("local.toml"), false);
        if let Some(root) = dir.parent() {
            push_candidate(&mut paths, root.join("config").join("local.toml"), false);
        }
    }

    if let Ok(cwd) = env::current_dir() {
        push_candidate(&mut paths, cwd.join("config").join("local.toml"), false);
    }

    paths
}

fn push_candidate(paths: &mut Vec<(PathBuf, bool)>, candidate: PathBuf, explicit: bool) {
    if !paths.iter().any(|(path, _)| path == &candidate) {
        paths.push((candidate, explicit));
    }
}

fn parse_bundle_config(path: &Path, contents: &str) -> Result<BundleConfig, ConfigError> {
    let value: Value = toml::from_str(contents).map_err(|err| {
        ConfigError::new(format!("failed to parse config {}: {err}", path.display()))
    })?;
    let Some(table) = value.as_table() else {
        return Err(ConfigError::new(format!(
            "config {} must be a TOML table",
            path.display()
        )));
    };
    let base = path.parent().unwrap_or(Path::new("."));

    let mut config = BundleConfig::default();
    if let Some(supervisor) = table.get("supervisor").and_then(Value::as_table) {
        let mut profile_genesis = None;
        if let Some(data_root) = supervisor.get("data_root").and_then(Value::as_str) {
            let data_root = data_root.trim();
            if !data_root.is_empty() {
                config.data_root = Some(resolve_path(base, data_root));
            }
        }
        if let Some(build_binaries) = supervisor.get("build_binaries") {
            config.build_binaries = Some(parse_bool(
                path,
                "supervisor.build_binaries",
                build_binaries,
            )?);
        }
        if let Some(readiness_smoke) = supervisor.get("readiness_smoke") {
            config.readiness_smoke = Some(parse_bool(
                path,
                "supervisor.readiness_smoke",
                readiness_smoke,
            )?);
        }
        if let Some(profile_value) = supervisor.get("profile") {
            match profile_value {
                Value::String(profile) => {
                    let profile = profile.trim();
                    if !profile.is_empty() {
                        config.profile = Some(NetworkProfile::from_preset(
                            parse_profile(profile).map_err(|value| {
                                ConfigError::new(format!(
                                    "invalid profile `{value}` in {}",
                                    path.display()
                                ))
                            })?,
                        ));
                    }
                }
                Value::Table(table) => {
                    let parsed = parse_profile_table(path, table)?;
                    config.profile = Some(parsed.profile);
                    profile_genesis = parsed.genesis_profile;
                }
                _ => {
                    return Err(ConfigError::new(format!(
                        "config {} expected `supervisor.profile` as string or table",
                        path.display()
                    )));
                }
            }
        }
        if let Some(chain_id) = supervisor.get("chain_id").and_then(Value::as_str) {
            let trimmed = chain_id.trim();
            if !trimmed.is_empty() {
                config.chain_id = Some(trimmed.to_owned());
            }
        }
        if let Some(profile) = supervisor.get("genesis_profile").and_then(Value::as_str) {
            let trimmed = profile.trim();
            if !trimmed.is_empty() {
                if profile_genesis.is_some() {
                    return Err(ConfigError::new(format!(
                        "config {} sets genesis_profile in both `supervisor.profile` and `supervisor.genesis_profile`",
                        path.display()
                    )));
                }
                config.genesis_profile = Some(trimmed.parse().map_err(|err: String| {
                    ConfigError::new(format!(
                        "invalid genesis_profile `{trimmed}` in {}: {err}",
                        path.display()
                    ))
                })?);
            }
        }
        if config.genesis_profile.is_none() {
            config.genesis_profile = profile_genesis;
        }
        if let Some(seed) = supervisor.get("vrf_seed_hex").and_then(Value::as_str) {
            let trimmed = seed.trim();
            if !trimmed.is_empty() {
                config.vrf_seed_hex = Some(trimmed.to_owned());
            }
        }
        if let Some(restart) = supervisor.get("restart").and_then(Value::as_table) {
            config.set_restart_policy(Some(parse_restart_policy(path, restart)?));
        }
    }

    if let Some(ports) = table.get("ports").and_then(Value::as_table) {
        if let Some(port) = ports.get("torii_start") {
            config.torii_start = Some(parse_port(path, "ports.torii_start", port)?);
        }
        if let Some(port) = ports.get("p2p_start") {
            config.p2p_start = Some(parse_port(path, "ports.p2p_start", port)?);
        }
    }

    if let Some(binaries) = table.get("binaries").and_then(Value::as_table) {
        config.binaries.irohad =
            parse_path_override(base, path, "binaries.irohad", binaries.get("irohad"))?;
        config.binaries.kagami =
            parse_path_override(base, path, "binaries.kagami", binaries.get("kagami"))?;
        config.binaries.iroha_cli =
            parse_path_override(base, path, "binaries.iroha_cli", binaries.get("iroha_cli"))?;
    }

    if let Some(nexus_value) = table.get("nexus") {
        let Some(nexus) = nexus_value.as_table() else {
            return Err(ConfigError::new(format!(
                "config {} expected `nexus` as a table",
                path.display()
            )));
        };
        config.nexus = Some(nexus.clone());
    }

    if let Some(sumeragi_value) = table.get("sumeragi") {
        let Some(sumeragi) = sumeragi_value.as_table() else {
            return Err(ConfigError::new(format!(
                "config {} expected `sumeragi` as a table",
                path.display()
            )));
        };
        if let Some(da_enabled) = sumeragi.get("da_enabled") {
            let _ = parse_bool(path, "sumeragi.da_enabled", da_enabled)?;
        }
        config.sumeragi = Some(sumeragi.clone());
    }

    if let Some(torii_value) = table.get("torii") {
        let Some(torii) = torii_value.as_table() else {
            return Err(ConfigError::new(format!(
                "config {} expected `torii` as a table",
                path.display()
            )));
        };
        if let Some(da_ingest) = torii.get("da_ingest")
            && !matches!(da_ingest, Value::Table(_))
        {
            return Err(ConfigError::new(format!(
                "config {} expected `torii.da_ingest` as a table",
                path.display()
            )));
        }
        config.torii = Some(torii.clone());
    }

    Ok(config)
}

fn parse_profile_table(
    path: &Path,
    table: &Map<String, Value>,
) -> Result<ParsedProfile, ConfigError> {
    let peer_value = table.get("peer_count").ok_or_else(|| {
        ConfigError::new(format!(
            "config {} missing `supervisor.profile.peer_count`",
            path.display()
        ))
    })?;
    let peer_count = parse_peer_count(path, "supervisor.profile.peer_count", peer_value)?;
    let consensus_value = table.get("consensus_mode").ok_or_else(|| {
        ConfigError::new(format!(
            "config {} missing `supervisor.profile.consensus_mode`",
            path.display()
        ))
    })?;
    let consensus_mode =
        parse_consensus_mode(path, "supervisor.profile.consensus_mode", consensus_value)?;
    let genesis_profile = table
        .get("genesis_profile")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| {
            value.parse().map_err(|err: String| {
                ConfigError::new(format!(
                    "invalid genesis_profile `{value}` in {}: {err}",
                    path.display()
                ))
            })
        })
        .transpose()?;

    if genesis_profile.is_some() && consensus_mode != SumeragiConsensusMode::Npos {
        return Err(ConfigError::new(format!(
            "config {} requires `supervisor.profile.consensus_mode = \"npos\"` when using genesis_profile",
            path.display()
        )));
    }

    let profile = NetworkProfile::custom(peer_count, consensus_mode).map_err(|err| {
        ConfigError::new(format!(
            "invalid supervisor.profile in {}: {err}",
            path.display()
        ))
    })?;

    Ok(ParsedProfile {
        profile,
        genesis_profile,
    })
}

fn parse_port(path: &Path, key: &str, value: &Value) -> Result<u16, ConfigError> {
    let int = value.as_integer().ok_or_else(|| {
        ConfigError::new(format!(
            "config {} expected integer for `{key}`",
            path.display()
        ))
    })?;

    if !(1..=u16::MAX as i64).contains(&int) {
        return Err(ConfigError::new(format!(
            "config {} value {int} out of range for `{key}`",
            path.display()
        )));
    }

    Ok(int as u16)
}

fn parse_peer_count(path: &Path, key: &str, value: &Value) -> Result<usize, ConfigError> {
    let raw = parse_non_negative_integer(path, key, value)?;
    if raw == 0 {
        return Err(ConfigError::new(format!(
            "config {} value 0 out of range for `{key}`",
            path.display()
        )));
    }
    let unsigned = u64::try_from(raw).map_err(|_| {
        ConfigError::new(format!(
            "config {} value {raw} too large for `{key}`",
            path.display()
        ))
    })?;
    usize::try_from(unsigned).map_err(|_| {
        ConfigError::new(format!(
            "config {} value {raw} too large for `{key}`",
            path.display()
        ))
    })
}

fn parse_consensus_mode(
    path: &Path,
    key: &str,
    value: &Value,
) -> Result<SumeragiConsensusMode, ConfigError> {
    let raw = value.as_str().ok_or_else(|| {
        ConfigError::new(format!(
            "config {} expected string for `{key}`",
            path.display()
        ))
    })?;
    let normalized = raw.trim().to_ascii_lowercase().replace('_', "-");
    match normalized.as_str() {
        "permissioned" | "permissioned-sumeragi" => Ok(SumeragiConsensusMode::Permissioned),
        "npos" => Ok(SumeragiConsensusMode::Npos),
        other => Err(ConfigError::new(format!(
            "config {} has invalid consensus mode `{other}` for `{key}`",
            path.display()
        ))),
    }
}

fn parse_bool(path: &Path, key: &str, value: &Value) -> Result<bool, ConfigError> {
    value.as_bool().ok_or_else(|| {
        ConfigError::new(format!(
            "config {} expected boolean for `{key}`",
            path.display()
        ))
    })
}

fn parse_path_override(
    base: &Path,
    path: &Path,
    key: &str,
    value: Option<&Value>,
) -> Result<Option<PathBuf>, ConfigError> {
    let Some(raw) = value.and_then(Value::as_str) else {
        return Ok(None);
    };
    let raw = raw.trim();
    if raw.is_empty() {
        return Ok(None);
    }
    let resolved = resolve_path(base, raw);
    if resolved.is_relative() {
        return Err(ConfigError::new(format!(
            "config {} produced relative path `{}` for `{key}` after resolution",
            path.display(),
            resolved.display()
        )));
    }
    Ok(Some(resolved))
}

fn resolve_path(base: &Path, raw: &str) -> PathBuf {
    let candidate = PathBuf::from(raw);
    if candidate.is_absolute() {
        candidate
    } else {
        base.join(candidate)
            .canonicalize()
            .unwrap_or_else(|_| base.join(raw))
    }
}

fn parse_profile(value: &str) -> Result<ProfilePreset, String> {
    match value {
        "single-peer" | "single_peer" | "singlepeer" => Ok(ProfilePreset::SinglePeer),
        "four-peer-bft" | "four_peer_bft" | "fourpeerbft" => Ok(ProfilePreset::FourPeerBft),
        other => Err(other.to_owned()),
    }
}

fn existing_root_table(path: &Path) -> Result<Map<String, Value>, ConfigError> {
    match fs::read_to_string(path) {
        Ok(contents) => match toml::from_str::<Value>(&contents) {
            Ok(Value::Table(table)) => Ok(table),
            Ok(_) => Ok(Map::new()),
            Err(err) => Err(ConfigError::new(format!(
                "failed to parse existing config {}: {err}",
                path.display()
            ))),
        },
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(Map::new()),
        Err(err) => Err(ConfigError::new(format!(
            "failed to read config {}: {err}",
            path.display()
        ))),
    }
}

fn parse_restart_policy(
    path: &Path,
    table: &Map<String, Value>,
) -> Result<RestartPolicy, ConfigError> {
    let mode = table
        .get("mode")
        .and_then(Value::as_str)
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "on-failure".to_owned());

    match mode.as_str() {
        "never" => {
            if table.get("max_restarts").is_some() || table.get("backoff_ms").is_some() {
                return Err(ConfigError::new(format!(
                    "config {} cannot set `supervisor.restart.max_restarts` or `.backoff_ms` when mode is `never`",
                    path.display()
                )));
            }
            Ok(RestartPolicy::Never)
        }
        "on-failure" | "on_failure" | "onfailure" => {
            let max = table
                .get("max_restarts")
                .map(|value| parse_restart_count(path, value))
                .transpose()?
                .unwrap_or(DEFAULT_RESTART_MAX_RESTARTS);
            let backoff_ms = table
                .get("backoff_ms")
                .map(|value| parse_restart_backoff(path, value))
                .transpose()?
                .unwrap_or(DEFAULT_RESTART_BACKOFF_MS);
            Ok(RestartPolicy::OnFailure {
                max_restarts: max,
                backoff: Duration::from_millis(backoff_ms),
            })
        }
        other => Err(ConfigError::new(format!(
            "config {} has invalid `supervisor.restart.mode` `{}`",
            path.display(),
            other
        ))),
    }
}

fn parse_restart_count(path: &Path, value: &Value) -> Result<usize, ConfigError> {
    let raw = parse_non_negative_integer(path, "supervisor.restart.max_restarts", value)?;
    let unsigned = u64::try_from(raw).map_err(|_| {
        ConfigError::new(format!(
            "config {} value {} too large for `supervisor.restart.max_restarts`",
            path.display(),
            raw
        ))
    })?;
    usize::try_from(unsigned).map_err(|_| {
        ConfigError::new(format!(
            "config {} value {} too large for `supervisor.restart.max_restarts`",
            path.display(),
            raw
        ))
    })
}

fn parse_restart_backoff(path: &Path, value: &Value) -> Result<u64, ConfigError> {
    let raw = parse_non_negative_integer(path, "supervisor.restart.backoff_ms", value)?;
    u64::try_from(raw).map_err(|_| {
        ConfigError::new(format!(
            "config {} value {} too large for `supervisor.restart.backoff_ms`",
            path.display(),
            raw
        ))
    })
}

fn parse_non_negative_integer(path: &Path, key: &str, value: &Value) -> Result<i64, ConfigError> {
    let Some(int) = value.as_integer() else {
        return Err(ConfigError::new(format!(
            "config {} expected integer for `{key}`",
            path.display()
        )));
    };
    if int < 0 {
        return Err(ConfigError::new(format!(
            "config {} value {int} out of range for `{key}`",
            path.display()
        )));
    }
    Ok(int)
}

fn render_restart_table(policy: RestartPolicy) -> Map<String, Value> {
    let mut table = Map::new();
    match policy {
        RestartPolicy::Never => {
            table.insert("mode".into(), Value::String("never".into()));
        }
        RestartPolicy::OnFailure {
            max_restarts,
            backoff,
        } => {
            table.insert("mode".into(), Value::String("on-failure".into()));
            let max = i64::try_from(max_restarts).unwrap_or(i64::MAX);
            table.insert("max_restarts".into(), Value::Integer(max));
            table.insert("backoff_ms".into(), Value::Integer(duration_to_ms(backoff)));
        }
    }
    table
}

fn duration_to_ms(backoff: Duration) -> i64 {
    let millis = backoff.as_millis();
    if millis > i64::MAX as u128 {
        i64::MAX
    } else {
        millis as i64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_file(contents: &str) -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("local.toml");
        fs::write(&path, contents).expect("write config");
        (dir, path)
    }

    #[test]
    fn parse_valid_config_resolves_relative_paths() {
        let (dir, path) = temp_file(
            r#"
[supervisor]
data_root = "./data"
build_binaries = true
readiness_smoke = false
profile = "four-peer-bft"
chain_id = "dev-chain"
genesis_profile = "iroha3-dev"
vrf_seed_hex = "abcd"

[supervisor.restart]
mode = "on_failure"
max_restarts = 5
backoff_ms = 2500

[ports]
torii_start = 12000
p2p_start = 13000

[binaries]
irohad = "./bin/irohad"
kagami = "/opt/iroha/bin/kagami"
iroha_cli = "./tools/iroha_cli"
"#,
        );

        let config =
            parse_bundle_config(&path, &fs::read_to_string(&path).unwrap()).expect("config parsed");
        assert_eq!(
            config.profile,
            Some(NetworkProfile::from_preset(ProfilePreset::FourPeerBft))
        );
        assert_eq!(
            config.data_root.as_deref(),
            Some(dir.path().join("data").as_path())
        );
        assert_eq!(config.build_binaries, Some(true));
        assert_eq!(config.readiness_smoke, Some(false));
        assert_eq!(config.torii_start, Some(12000));
        assert_eq!(config.p2p_start, Some(13000));
        assert_eq!(config.chain_id.as_deref(), Some("dev-chain"));
        assert_eq!(config.genesis_profile, Some(GenesisProfile::Iroha3Dev));
        assert_eq!(config.vrf_seed_hex.as_deref(), Some("abcd"));
        assert_eq!(
            config.binaries.irohad.as_deref(),
            Some(dir.path().join("bin/irohad").as_path())
        );
        assert_eq!(
            config.binaries.kagami.as_deref(),
            Some(Path::new("/opt/iroha/bin/kagami"))
        );
        assert_eq!(
            config.binaries.iroha_cli.as_deref(),
            Some(dir.path().join("tools/iroha_cli").as_path())
        );
        match config.restart_policy.expect("restart policy") {
            RestartPolicy::OnFailure {
                max_restarts,
                backoff,
            } => {
                assert_eq!(max_restarts, 5);
                assert_eq!(backoff, Duration::from_millis(2500));
            }
            RestartPolicy::Never => panic!("expected on-failure restart policy"),
        }
    }

    #[test]
    fn parse_rejects_invalid_profile() {
        let (_dir, path) = temp_file(
            r#"
[supervisor]
profile = "unknown"
"#,
        );
        let err = parse_bundle_config(&path, &fs::read_to_string(&path).unwrap())
            .expect_err("invalid profile should fail");
        assert!(
            err.to_string().contains("invalid profile `unknown`"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn parse_rejects_invalid_genesis_profile() {
        let (_dir, path) = temp_file(
            r#"
[supervisor]
genesis_profile = "invalid"
"#,
        );
        let err = parse_bundle_config(&path, &fs::read_to_string(&path).unwrap())
            .expect_err("invalid genesis profile should fail");
        assert!(
            err.to_string().contains("genesis_profile `invalid`"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn parse_custom_profile_table() {
        let (_dir, path) = temp_file(
            r#"
[supervisor]
profile = { peer_count = 5, consensus_mode = "permissioned" }
"#,
        );
        let config =
            parse_bundle_config(&path, &fs::read_to_string(&path).unwrap()).expect("config parsed");
        let profile = config.profile.expect("profile expected");
        assert_eq!(profile.preset, None);
        assert_eq!(profile.topology.peer_count, 5);
        assert_eq!(profile.consensus_mode, SumeragiConsensusMode::Permissioned);
        assert!(config.genesis_profile.is_none());
    }

    #[test]
    fn parse_custom_profile_table_rejects_conflicting_genesis_profile() {
        let (_dir, path) = temp_file(
            r#"
[supervisor]
profile = { peer_count = 3, consensus_mode = "permissioned", genesis_profile = "iroha3-dev" }
"#,
        );
        let err = parse_bundle_config(&path, &fs::read_to_string(&path).unwrap())
            .expect_err("conflicting profile should fail");
        assert!(
            err.to_string().contains("consensus_mode = \"npos\""),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn parse_custom_profile_table_with_genesis_profile() {
        let (_dir, path) = temp_file(
            r#"
[supervisor]
profile = { peer_count = 3, consensus_mode = "npos", genesis_profile = "iroha3-dev" }
"#,
        );
        let config =
            parse_bundle_config(&path, &fs::read_to_string(&path).unwrap()).expect("config parsed");
        let profile = config.profile.expect("profile expected");
        assert_eq!(profile.preset, None);
        assert_eq!(profile.topology.peer_count, 3);
        assert_eq!(profile.consensus_mode, SumeragiConsensusMode::Npos);
        assert_eq!(config.genesis_profile, Some(GenesisProfile::Iroha3Dev));
    }

    #[test]
    fn parse_bundle_config_parses_nexus_sumeragi_and_torii_tables() {
        let (_dir, path) = temp_file(
            r#"
[nexus]
enabled = true
lane_count = 2

[[nexus.lane_catalog]]
index = 0
alias = "core"
dataspace = "universal"

[sumeragi]
da_enabled = true

[torii]
  [torii.da_ingest]
  replay_cache_store_dir = "./da/replay"
  manifest_store_dir = "./da/manifests"
"#,
        );

        let config =
            parse_bundle_config(&path, &fs::read_to_string(&path).unwrap()).expect("config parsed");
        let nexus = config.nexus.expect("nexus config");
        assert!(matches!(nexus.get("enabled"), Some(Value::Boolean(true))));
        assert_eq!(nexus.get("lane_count").and_then(Value::as_integer), Some(2));
        let sumeragi = config.sumeragi.expect("sumeragi config");
        assert!(matches!(
            sumeragi.get("da_enabled"),
            Some(Value::Boolean(true))
        ));
        let torii = config.torii.expect("torii config");
        let da_ingest = torii
            .get("da_ingest")
            .and_then(Value::as_table)
            .expect("da_ingest table");
        assert_eq!(
            da_ingest
                .get("replay_cache_store_dir")
                .and_then(Value::as_str),
            Some("./da/replay")
        );
        assert_eq!(
            da_ingest.get("manifest_store_dir").and_then(Value::as_str),
            Some("./da/manifests")
        );
    }

    #[test]
    fn parse_bundle_config_rejects_invalid_sumeragi_da_enabled() {
        let (_dir, path) = temp_file(
            r#"
[sumeragi]
da_enabled = "nope"
"#,
        );
        let err = parse_bundle_config(&path, &fs::read_to_string(&path).unwrap())
            .expect_err("invalid sumeragi config should fail");
        assert!(
            err.to_string().contains("sumeragi.da_enabled"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn parse_bundle_config_rejects_invalid_torii_da_ingest() {
        let (_dir, path) = temp_file(
            r#"
[torii]
da_ingest = "nope"
"#,
        );
        let err = parse_bundle_config(&path, &fs::read_to_string(&path).unwrap())
            .expect_err("invalid torii config should fail");
        assert!(
            err.to_string().contains("torii.da_ingest"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn load_bundle_config_at_returns_default_for_missing_file() {
        let temp = tempfile::tempdir().expect("temp dir");
        let path = temp.path().join("custom-config.toml");
        let resolved =
            load_bundle_config_at(path.clone()).expect("explicit config load should succeed");
        assert!(resolved.config.data_root.is_none());
        assert_eq!(resolved.path, path);
    }

    #[test]
    fn load_bundle_config_at_parses_existing_file() {
        let (dir, path) = temp_file(
            r#"
[supervisor]
data_root = "./bundle-root"
[supervisor.restart]
mode = "never"
"#,
        );
        let resolved =
            load_bundle_config_at(path.clone()).expect("explicit config load should succeed");
        assert_eq!(
            resolved.config.data_root.as_deref(),
            Some(dir.path().join("bundle-root").as_path())
        );
        assert_eq!(resolved.path, path);
        assert!(matches!(
            resolved.config.restart_policy,
            Some(RestartPolicy::Never)
        ));
    }

    #[test]
    fn load_uses_explicit_env_override() {
        let (dir, path) = temp_file(
            r#"
[supervisor]
data_root = "./env-data"
"#,
        );
        let _guard = EnvGuard::new("MOCHI_CONFIG", path.to_string_lossy());
        let resolved = load_bundle_config()
            .expect("config load")
            .expect("config present");
        let config = resolved.config;
        let actual_root = config
            .data_root
            .as_deref()
            .expect("data root should be populated by env override");
        let expected_root = dir.path().join("env-data");
        let normalise = |path: &Path| -> PathBuf {
            fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
        };
        assert_eq!(normalise(actual_root), normalise(&expected_root));
    }

    #[test]
    fn write_roundtrips_bundle_config() {
        let temp = tempfile::tempdir().expect("temp dir");
        let path = temp.path().join("config/local.toml");
        let mut config = BundleConfig::default();
        config.set_data_root(Some(temp.path().join("data")));
        config.set_profile(Some(NetworkProfile::from_preset(ProfilePreset::SinglePeer)));
        config.set_chain_id(Some("local-test".to_owned()));
        config.set_build_binaries(Some(true));
        config.set_readiness_smoke(Some(false));
        config.set_genesis_profile(Some(GenesisProfile::Iroha3Dev));
        config.set_vrf_seed_hex(Some("abcd".to_owned()));
        config.set_torii_start(Some(12000));
        config.set_p2p_start(Some(13000));
        config.binaries.irohad = Some(temp.path().join("bin/irohad"));
        config.binaries.kagami = Some(temp.path().join("bin/kagami"));
        config.binaries.iroha_cli = Some(temp.path().join("bin/iroha_cli"));
        let mut nexus = Map::new();
        nexus.insert("enabled".into(), Value::Boolean(true));
        nexus.insert("lane_count".into(), Value::Integer(2));
        let mut lane = Map::new();
        lane.insert("alias".into(), Value::String("core".into()));
        lane.insert("index".into(), Value::Integer(0));
        nexus.insert(
            "lane_catalog".into(),
            Value::Array(vec![Value::Table(lane)]),
        );
        config.nexus = Some(nexus);
        let mut sumeragi = Map::new();
        sumeragi.insert("da_enabled".into(), Value::Boolean(true));
        sumeragi.insert("msg_channel_cap_votes".into(), Value::Integer(16));
        config.sumeragi = Some(sumeragi);
        let mut torii = Map::new();
        let mut da_ingest = Map::new();
        da_ingest.insert(
            "replay_cache_store_dir".into(),
            Value::String("./da/replay".into()),
        );
        da_ingest.insert(
            "manifest_store_dir".into(),
            Value::String("./da/manifests".into()),
        );
        torii.insert("da_ingest".into(), Value::Table(da_ingest));
        config.torii = Some(torii);
        config.set_restart_policy(Some(RestartPolicy::OnFailure {
            max_restarts: 9,
            backoff: Duration::from_secs(2),
        }));
        config.write_to_path(&path).expect("write bundle config");

        let contents = fs::read_to_string(&path).expect("read config back");
        let parsed = parse_bundle_config(&path, &contents).expect("parse written config");
        assert_eq!(parsed.data_root, config.data_root);
        assert_eq!(parsed.profile, config.profile);
        assert_eq!(parsed.chain_id, config.chain_id);
        assert_eq!(parsed.build_binaries, config.build_binaries);
        assert_eq!(parsed.readiness_smoke, config.readiness_smoke);
        assert_eq!(parsed.genesis_profile, config.genesis_profile);
        assert_eq!(parsed.vrf_seed_hex, config.vrf_seed_hex);
        assert_eq!(parsed.torii_start, config.torii_start);
        assert_eq!(parsed.p2p_start, config.p2p_start);
        assert_eq!(parsed.binaries.irohad, config.binaries.irohad);
        assert_eq!(parsed.binaries.kagami, config.binaries.kagami);
        assert_eq!(parsed.binaries.iroha_cli, config.binaries.iroha_cli);
        assert_eq!(parsed.nexus, config.nexus);
        assert_eq!(parsed.sumeragi, config.sumeragi);
        assert_eq!(parsed.torii, config.torii);
        match parsed.restart_policy.expect("restart policy parsed") {
            RestartPolicy::OnFailure {
                max_restarts,
                backoff,
            } => {
                assert_eq!(max_restarts, 9);
                assert_eq!(backoff, Duration::from_secs(2));
            }
            RestartPolicy::Never => panic!("expected on-failure restart policy"),
        }
    }

    #[test]
    fn write_roundtrips_custom_profile_with_genesis_profile() {
        let temp = tempfile::tempdir().expect("temp dir");
        let path = temp.path().join("config/local.toml");
        let mut config = BundleConfig::default();
        let profile = NetworkProfile::custom(3, SumeragiConsensusMode::Npos).expect("profile");
        config.set_profile(Some(profile));
        config.set_genesis_profile(Some(GenesisProfile::Iroha3Dev));
        config.write_to_path(&path).expect("write bundle config");

        let contents = fs::read_to_string(&path).expect("read config back");
        let value: Value = toml::from_str(&contents).expect("parse config");
        let supervisor = value
            .get("supervisor")
            .and_then(Value::as_table)
            .expect("supervisor table");
        assert!(
            supervisor.get("genesis_profile").is_none(),
            "custom profile should inline genesis_profile"
        );
        let profile_table = supervisor
            .get("profile")
            .and_then(Value::as_table)
            .expect("profile table");
        assert_eq!(
            profile_table.get("peer_count").and_then(Value::as_integer),
            Some(3)
        );
        assert_eq!(
            profile_table.get("consensus_mode").and_then(Value::as_str),
            Some("npos")
        );
        assert_eq!(
            profile_table.get("genesis_profile").and_then(Value::as_str),
            Some("iroha3-dev")
        );

        let parsed = parse_bundle_config(&path, &contents).expect("parse written config");
        assert_eq!(parsed.profile, config.profile);
        assert_eq!(parsed.genesis_profile, config.genesis_profile);
    }

    #[test]
    fn parse_restart_policy_defaults_to_on_failure() {
        let (dir, path) = temp_file(
            r#"
[supervisor]
data_root = "./root"

[supervisor.restart]
max_restarts = 0
"#,
        );
        let parsed =
            parse_bundle_config(&path, &fs::read_to_string(&path).unwrap()).expect("parse config");
        match parsed.restart_policy.expect("restart policy") {
            RestartPolicy::OnFailure {
                max_restarts,
                backoff,
            } => {
                assert_eq!(max_restarts, 0);
                assert_eq!(backoff, Duration::from_millis(DEFAULT_RESTART_BACKOFF_MS));
            }
            RestartPolicy::Never => panic!("expected on-failure policy"),
        }
        drop(dir);
    }

    #[test]
    fn parse_restart_policy_never_rejects_extra_fields() {
        let (_dir, path) = temp_file(
            r#"
[supervisor.restart]
mode = "never"
backoff_ms = 1000
"#,
        );
        let err = parse_bundle_config(&path, &fs::read_to_string(&path).unwrap())
            .expect_err("invalid restart policy");
        assert!(
            err.to_string()
                .contains("`supervisor.restart.max_restarts` or `.backoff_ms`")
        );
    }

    #[test]
    fn apply_to_overrides_profile() {
        let mut config = BundleConfig::default();
        config.set_profile(Some(NetworkProfile::from_preset(
            ProfilePreset::FourPeerBft,
        )));
        let builder = config.apply_to(SupervisorBuilder::new(ProfilePreset::SinglePeer));
        assert_eq!(builder.profile().preset, Some(ProfilePreset::FourPeerBft));
        assert_eq!(builder.profile().topology.peer_count, 4);
    }

    #[test]
    fn write_preserves_unrelated_fields() {
        let temp = tempfile::tempdir().expect("temp dir");
        let config_dir = temp.path().join("config");
        fs::create_dir_all(&config_dir).expect("config dir");
        let path = config_dir.join("local.toml");
        let data_root = temp.path().join("existing");
        let irohad = temp.path().join("bin/irohad");
        fs::write(
            &path,
            format!(
                r#"
note = "keep-root"

[supervisor]
data_root = "{data_root}"
profile = "single-peer"
chain_id = "existing-chain"
custom = "preserve-me"

[ports]
torii_start = 11000
custom_flag = true

[binaries]
irohad = "{irohad}"
extra = "/opt/keep/binary"

[nexus]
enabled = true
lane_count = 2
extra_lane = "keep"

[sumeragi]
da_enabled = true
extra_setting = "keep"
"#,
                data_root = data_root.display(),
                irohad = irohad.display(),
            ),
        )
        .expect("write starter config");

        let mut config =
            parse_bundle_config(&path, &fs::read_to_string(&path).unwrap()).expect("parse");
        config.set_torii_start(Some(12000));
        config.set_p2p_start(None);
        config.set_chain_id(None);
        config
            .write_to_path(&path)
            .expect("rewrite should preserve unrelated fields");

        let contents = fs::read_to_string(&path).expect("read rewritten config");
        assert!(
            contents.contains("note = \"keep-root\""),
            "top-level keys must remain untouched"
        );
        assert!(
            contents.contains("custom = \"preserve-me\""),
            "unknown supervisor fields must be preserved"
        );
        assert!(
            contents.contains("custom_flag = true"),
            "unknown ports fields must be preserved"
        );
        assert!(
            contents.contains("extra = \"/opt/keep/binary\""),
            "unknown binaries fields must be preserved"
        );
        assert!(
            contents.contains("extra_lane = \"keep\""),
            "nexus fields must be preserved"
        );
        assert!(
            contents.contains("extra_setting = \"keep\""),
            "sumeragi fields must be preserved"
        );
        assert!(
            contents.contains("torii_start = 12000"),
            "torii_start should update to new value"
        );
        assert!(
            !contents.contains("p2p_start"),
            "p2p_start should be removed from config when unset"
        );
        assert!(
            !contents.contains("chain_id"),
            "chain_id should be removed when cleared"
        );
    }

    struct EnvGuard {
        key: &'static str,
        prev: Option<String>,
    }

    impl EnvGuard {
        fn new(key: &'static str, value: impl AsRef<str>) -> Self {
            let prev = env::var(key).ok();
            // SAFETY: Tests run single-threaded and values are owned strings.
            unsafe { env::set_var(key, value.as_ref()) };
            Self { key, prev }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            if let Some(prev) = self.prev.as_ref() {
                unsafe { env::set_var(self.key, prev) };
            } else {
                unsafe { env::remove_var(self.key) };
            }
        }
    }

    #[test]
    fn parse_profile_mappings_cover_aliases() {
        assert_eq!(
            parse_profile("single-peer").ok(),
            Some(ProfilePreset::SinglePeer)
        );
        assert_eq!(
            parse_profile("single_peer").ok(),
            Some(ProfilePreset::SinglePeer)
        );
        assert_eq!(
            parse_profile("singlepeer").ok(),
            Some(ProfilePreset::SinglePeer)
        );
        assert_eq!(
            parse_profile("four-peer-bft").ok(),
            Some(ProfilePreset::FourPeerBft)
        );
        assert!(parse_profile("invalid").is_err());
    }
}
