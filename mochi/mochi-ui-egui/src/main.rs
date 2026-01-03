//! MOCHI egui desktop entry point.

mod config;

use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque},
    convert::TryFrom,
    env,
    ffi::OsString,
    fs,
    num::{NonZeroU32, NonZeroU64},
    path::{Path, PathBuf},
    process,
    str::FromStr,
    sync::{Arc, LazyLock, Mutex},
    time::{Duration, Instant, SystemTime, SystemTimeError, UNIX_EPOCH},
};

use config::{
    BinaryOverrides, BundleConfig, ResolvedBundleConfig, default_config_path, load_bundle_config,
    load_bundle_config_at,
};
use eframe::{
    App, CreationContext, NativeOptions, Storage,
    egui::{
        self, Button, Color32, ComboBox, CornerRadius, FontFamily, FontId, Frame, Layout, Margin,
        RichText, ScrollArea, Shadow, Stroke, TextStyle, Visuals, output::OutputCommand,
    },
};
use egui_plot::{Legend, Line, Plot, PlotPoint, PlotPoints};
use hex::encode_upper;
#[allow(unused_imports)]
use iroha_data_model::{
    ChainId,
    account::{
        AccountAdmissionMode, AccountAdmissionPolicy,
        admission::{ImplicitAccountCreationFee, ImplicitAccountFeeDestination},
    },
    asset::{AssetDefinitionId, AssetId, definition::Mintable},
    block::consensus::{
        SumeragiBlockSyncRosterStatus, SumeragiCommitCertificateStatus, SumeragiCommitQuorumStatus,
        SumeragiDaGateReason, SumeragiDaGateSatisfaction, SumeragiDaGateStatus,
        SumeragiKuraStoreStatus, SumeragiLaneGovernance, SumeragiMissingBlockFetchStatus,
        SumeragiPendingRbcStatus, SumeragiRbcStoreStatus, SumeragiStatusWire,
        SumeragiValidationRejectStatus, SumeragiViewChangeCauseStatus,
    },
    da::commitment::DaProofScheme,
    domain::{Domain, DomainId},
    events::{
        EventBox,
        pipeline::{BlockStatus, PipelineEventBox, TransactionStatus},
        trigger_completed::{TriggerCompletedEvent, TriggerCompletedOutcome},
    },
    isi::{InstructionBox, Register},
    nexus::{
        DataSpaceId, LaneConfig as LaneMetadata, LaneId, LaneLifecyclePlan, LaneRelayEnvelope,
        LaneStorageProfile, LaneVisibility,
    },
    parameter::system::SumeragiConsensusMode,
    prelude::{AccountId, Name, Numeric},
    role::RoleId,
    transaction::{SignedTransaction, TransactionBuilder},
};
use iroha_executor_data_model::isi::multisig::MultisigSpec;
use mochi_core::{
    BlockDecodeStage, BlockStreamDecodeError, BlockStreamEvent, BlockSummary, EventCategory,
    EventDecodeStage, EventStreamDecodeError, EventStreamEvent, EventSummary, ExposedPrivateKey,
    GenesisProfile, InstructionDraft, InstructionPermission, KeyPair, LifecycleEvent,
    LogStreamKind, ManagedBlockStream, ManagedEventStream, ManagedStatusStream, NetworkProfile,
    PeerLogEvent, PeerState, PrivateKey, ProfilePreset, SigningAuthority, StateCursor, StateEntry,
    StatePage, StateQueryKind, StatusStreamEvent, Supervisor, SupervisorBuilder, SupervisorError,
    ToriiClient, TransactionComposeOptions, TransactionPreview, compose_preview_with_options,
    development_signing_authorities, drafts_from_json_str, drafts_to_pretty_json, run_state_query,
    supervisor::RestartPolicy,
    torii::{
        ReadinessOptions, ReadinessSmokeOutcome, SmokeCommitOptions, StatusMetrics, ToriiErrorInfo,
        ToriiErrorKind, ToriiMetricsSnapshot, ToriiStatusSnapshot,
    },
};
use norito::json::{self, Map, Value};
use tokio::{
    runtime::{Handle, Runtime},
    sync::{
        broadcast::{Receiver as BroadcastReceiver, error::TryRecvError},
        mpsc::{self, UnboundedReceiver, UnboundedSender},
    },
};
use toml::{self, Table as TomlTable, Value as TomlValue};

const MAX_BLOCK_EVENTS: usize = 100;
const MAX_EVENT_EVENTS: usize = 200;
const MAX_LOG_EVENTS: usize = 200;
const STATUS_POLL_INTERVAL: Duration = Duration::from_secs(2);
const STATUS_STALE_MULTIPLIER: u32 = 3;
const STATUS_HISTORY_LEN: usize = 16;
const READINESS_TIMEOUT: Duration = Duration::from_secs(8);
const READINESS_POLL_INTERVAL: Duration = Duration::from_millis(150);
const SMOKE_TIMEOUT: Duration = Duration::from_secs(12);
const SMOKE_MAX_ATTEMPTS: usize = 3;
const EVENT_FILTER_STORAGE_KEY: &str = "mochi.event_filter";
const COMPOSER_DRAFT_SPACE_MANIFEST_TOUCH: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../fixtures/composer/draft_space_manifest_touch.json"
));
const COMPOSER_DRAFT_SPACE_MANIFEST_HANDLE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../fixtures/composer/draft_space_manifest_handle.json"
));
const COMPOSER_DRAFT_PIN_MANIFEST: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../fixtures/composer/draft_pin_manifest.json"
));
const COMPOSER_MULTISIG_INSTRUCTIONS: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../fixtures/composer/multisig_instructions.json"
));
const COMPOSER_MULTISIG_POLICY: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../fixtures/composer/multisig_policy.json"
));

static CLI_OVERRIDES: LazyLock<Mutex<CliOverrides>> =
    LazyLock::new(|| Mutex::new(CliOverrides::default()));

#[derive(Debug, Default, Clone)]
struct CliOverrides {
    data_root: Option<PathBuf>,
    profile: Option<NetworkProfile>,
    config_path: Option<PathBuf>,
    torii_start: Option<u16>,
    p2p_start: Option<u16>,
    chain_id: Option<String>,
    genesis_profile: Option<GenesisProfile>,
    vrf_seed_hex: Option<String>,
    binaries: BinaryOverrides,
    build_binaries: Option<bool>,
    readiness_smoke: Option<bool>,
    restart_policy: Option<RestartPolicy>,
    nexus_config: Option<toml::Table>,
    nexus_enabled: Option<bool>,
    nexus_lane_count: Option<u32>,
    sumeragi_da_enabled: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsedProfileOverride {
    profile: NetworkProfile,
    genesis_profile: Option<GenesisProfile>,
}

impl CliOverrides {
    fn apply_to(&self, mut builder: SupervisorBuilder) -> SupervisorBuilder {
        if let Some(root) = &self.data_root {
            builder = builder.data_root(root.clone());
        }
        if let Some(profile) = self.profile.as_ref() {
            if let Some(preset) = profile.preset {
                builder = builder.profile_preset(preset);
            } else {
                builder = builder.set_profile(profile.clone());
            }
        }
        if let Some(port) = self.torii_start {
            builder = builder.torii_base_port(port);
        }
        if let Some(port) = self.p2p_start {
            builder = builder.p2p_base_port(port);
        }
        if let Some(chain_id) = &self.chain_id {
            builder = builder.chain_id(chain_id.clone());
        }
        if let Some(profile) = self.genesis_profile {
            builder = builder.genesis_profile(profile);
        }
        if let Some(seed) = &self.vrf_seed_hex {
            builder = builder.vrf_seed_hex(seed.clone());
        }
        if let Some(path) = &self.binaries.irohad {
            builder = builder.irohad_path(path.clone());
        }
        if let Some(path) = &self.binaries.kagami {
            builder = builder.kagami_path(path.clone());
        }
        if let Some(path) = &self.binaries.iroha_cli {
            builder = builder.iroha_cli_path(path.clone());
        }
        if let Some(allow) = self.build_binaries {
            builder = builder.auto_build_binaries(allow);
        }
        if let Some(policy) = self.restart_policy {
            builder = builder.restart_policy(policy);
        }
        if let Some(nexus) = self.nexus_config.as_ref() {
            builder = builder.nexus_config(nexus.clone());
        }
        if let Some(enabled) = self.nexus_enabled {
            builder = builder.nexus_enabled(enabled);
        }
        if let Some(lane_count) = self.nexus_lane_count {
            builder = builder.nexus_lane_count(lane_count);
        }
        if let Some(enabled) = self.sumeragi_da_enabled {
            builder = builder.sumeragi_da_enabled(enabled);
        }
        builder
    }
}

struct StatusStreamHandle {
    stream: ManagedStatusStream,
    receiver: BroadcastReceiver<StatusStreamEvent>,
}

impl StatusStreamHandle {
    fn new(handle: &Handle, alias: String, client: ToriiClient) -> Self {
        let stream = ManagedStatusStream::spawn(handle, alias, client, STATUS_POLL_INTERVAL);
        let receiver = stream.subscribe();
        Self { stream, receiver }
    }

    fn abort(&mut self) {
        self.stream.abort();
    }
}

impl Drop for StatusStreamHandle {
    fn drop(&mut self) {
        self.stream.abort();
    }
}

#[derive(Debug)]
struct ParsedCli {
    overrides: CliOverrides,
    help: bool,
}

#[derive(Debug)]
struct CliParseError {
    message: String,
}

impl CliParseError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for CliParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for CliParseError {}

fn parse_cli_overrides() -> Result<ParsedCli, CliParseError> {
    let args: Vec<OsString> = env::args_os().skip(1).collect();
    let mut parsed = parse_cli_overrides_from(args)?;
    if parsed.help {
        return Ok(parsed);
    }

    let env_overrides = parse_env_overrides()?;
    parsed.overrides = merge_overrides(env_overrides, parsed.overrides);
    Ok(parsed)
}

fn parse_cli_overrides_from<I>(args: I) -> Result<ParsedCli, CliParseError>
where
    I: IntoIterator<Item = OsString>,
{
    let mut overrides = CliOverrides::default();
    let mut iter = args.into_iter();
    let mut restart_mode: Option<RestartModeFlag> = None;
    let mut restart_max: Option<usize> = None;
    let mut restart_backoff_ms: Option<u64> = None;

    while let Some(arg) = iter.next() {
        let flag = arg
            .into_string()
            .map_err(|_| CliParseError::new("flags must be valid UTF-8"))?;
        match flag.as_str() {
            "-h" | "--help" => {
                return Ok(ParsedCli {
                    overrides,
                    help: true,
                });
            }
            "--data-root" => {
                let value = next_value(&mut iter, "--data-root")?;
                overrides.data_root = Some(PathBuf::from(value));
            }
            "--profile" => {
                let value = next_value_string(&mut iter, "--profile")?;
                let parsed = parse_profile_override(&value)?;
                apply_profile_override(&mut overrides, parsed, "--profile")?;
            }
            "--config" => {
                let value = next_value(&mut iter, "--config")?;
                overrides.config_path = Some(PathBuf::from(value));
            }
            "--torii-start" => {
                let value = next_value_string(&mut iter, "--torii-start")?;
                overrides.torii_start = Some(parse_port_flag(&value, "--torii-start")?);
            }
            "--p2p-start" => {
                let value = next_value_string(&mut iter, "--p2p-start")?;
                overrides.p2p_start = Some(parse_port_flag(&value, "--p2p-start")?);
            }
            "--chain-id" => {
                let value = next_value_string(&mut iter, "--chain-id")?;
                let trimmed = value.trim();
                if trimmed.is_empty() {
                    return Err(CliParseError::new(
                        "--chain-id value must not be empty or whitespace",
                    ));
                }
                overrides.chain_id = Some(trimmed.to_owned());
            }
            "--genesis-profile" => {
                let value = next_value_string(&mut iter, "--genesis-profile")?;
                let profile = parse_genesis_profile_flag(&value)?;
                if let Some(existing) = overrides.genesis_profile
                    && existing != profile
                {
                    return Err(CliParseError::new(format!(
                        "--genesis-profile `{profile}` conflicts with `{existing}`"
                    )));
                }
                overrides.genesis_profile = Some(profile);
            }
            "--vrf-seed-hex" => {
                let value = next_value_string(&mut iter, "--vrf-seed-hex")?;
                overrides.vrf_seed_hex = Some(value);
            }
            "--nexus-config" => {
                let value = next_value_string(&mut iter, "--nexus-config")?;
                overrides.nexus_config = Some(parse_nexus_config_file(&value)?);
            }
            "--enable-nexus" => {
                overrides.nexus_enabled = Some(true);
            }
            "--disable-nexus" => {
                overrides.nexus_enabled = Some(false);
            }
            "--nexus-lane-count" => {
                let value = next_value_string(&mut iter, "--nexus-lane-count")?;
                overrides.nexus_lane_count = Some(parse_u32_flag(&value, "--nexus-lane-count")?);
            }
            "--enable-da" => {
                overrides.sumeragi_da_enabled = Some(true);
            }
            "--disable-da" => {
                overrides.sumeragi_da_enabled = Some(false);
            }
            "--irohad" => {
                let value = next_value(&mut iter, "--irohad")?;
                overrides.binaries.irohad = Some(PathBuf::from(value));
            }
            "--kagami" => {
                let value = next_value(&mut iter, "--kagami")?;
                overrides.binaries.kagami = Some(PathBuf::from(value));
            }
            "--iroha-cli" => {
                let value = next_value(&mut iter, "--iroha-cli")?;
                overrides.binaries.iroha_cli = Some(PathBuf::from(value));
            }
            "--build-binaries" => {
                overrides.build_binaries = Some(true);
            }
            "--no-build-binaries" => {
                overrides.build_binaries = Some(false);
            }
            "--enable-smoke" => {
                overrides.readiness_smoke = Some(true);
            }
            "--disable-smoke" => {
                overrides.readiness_smoke = Some(false);
            }
            "--restart-mode" => {
                let value = next_value_string(&mut iter, "--restart-mode")?;
                restart_mode = Some(parse_restart_mode_flag(&value)?);
            }
            "--restart-max" => {
                let value = next_value_string(&mut iter, "--restart-max")?;
                restart_max = Some(parse_usize_flag(&value, "--restart-max")?);
            }
            "--restart-backoff-ms" => {
                let value = next_value_string(&mut iter, "--restart-backoff-ms")?;
                restart_backoff_ms = Some(parse_u64_flag(&value, "--restart-backoff-ms")?);
            }
            other => {
                return Err(CliParseError::new(format!("unknown flag `{other}`")));
            }
        }
    }

    overrides.restart_policy =
        build_restart_policy_override(restart_mode, restart_max, restart_backoff_ms)?;

    Ok(ParsedCli {
        overrides,
        help: false,
    })
}

fn merge_overrides(env: CliOverrides, cli: CliOverrides) -> CliOverrides {
    CliOverrides {
        data_root: cli.data_root.or(env.data_root),
        profile: cli.profile.or(env.profile),
        config_path: cli.config_path.or(env.config_path),
        torii_start: cli.torii_start.or(env.torii_start),
        p2p_start: cli.p2p_start.or(env.p2p_start),
        chain_id: cli.chain_id.or(env.chain_id),
        genesis_profile: cli.genesis_profile.or(env.genesis_profile),
        vrf_seed_hex: cli.vrf_seed_hex.or(env.vrf_seed_hex),
        binaries: BinaryOverrides {
            irohad: cli.binaries.irohad.or(env.binaries.irohad),
            kagami: cli.binaries.kagami.or(env.binaries.kagami),
            iroha_cli: cli.binaries.iroha_cli.or(env.binaries.iroha_cli),
        },
        build_binaries: cli.build_binaries.or(env.build_binaries),
        readiness_smoke: cli.readiness_smoke.or(env.readiness_smoke),
        restart_policy: cli.restart_policy.or(env.restart_policy),
        nexus_config: cli.nexus_config.or(env.nexus_config),
        nexus_enabled: cli.nexus_enabled.or(env.nexus_enabled),
        nexus_lane_count: cli.nexus_lane_count.or(env.nexus_lane_count),
        sumeragi_da_enabled: cli.sumeragi_da_enabled.or(env.sumeragi_da_enabled),
    }
}

fn parse_env_overrides() -> Result<CliOverrides, CliParseError> {
    let mut overrides = CliOverrides::default();

    if let Some(root) = env_value("MOCHI_DATA_ROOT")? {
        overrides.data_root = Some(PathBuf::from(root));
    }
    if let Some(profile) = env_value("MOCHI_PROFILE")? {
        let parsed = parse_profile_override(&profile)?;
        apply_profile_override(&mut overrides, parsed, "MOCHI_PROFILE")?;
    }
    if let Some(chain_id) = env_value("MOCHI_CHAIN_ID")? {
        let trimmed = chain_id.trim();
        if trimmed.is_empty() {
            return Err(CliParseError::new(
                "MOCHI_CHAIN_ID value must not be empty or whitespace",
            ));
        }
        overrides.chain_id = Some(trimmed.to_owned());
    }
    if let Some(profile) = env_value("MOCHI_GENESIS_PROFILE")? {
        let parsed = parse_genesis_profile_flag(&profile)?;
        if let Some(existing) = overrides.genesis_profile
            && existing != parsed
        {
            return Err(CliParseError::new(format!(
                "MOCHI_GENESIS_PROFILE `{parsed}` conflicts with `{existing}`"
            )));
        }
        overrides.genesis_profile = Some(parsed);
    }
    if let Some(seed) = env_value("MOCHI_VRF_SEED_HEX")? {
        overrides.vrf_seed_hex = Some(seed);
    }
    if let Some(port) = env_value("MOCHI_TORII_START")? {
        overrides.torii_start = Some(parse_port_flag(&port, "MOCHI_TORII_START")?);
    }
    if let Some(port) = env_value("MOCHI_P2P_START")? {
        overrides.p2p_start = Some(parse_port_flag(&port, "MOCHI_P2P_START")?);
    }
    if let Some(value) = env_value("MOCHI_BUILD_BINARIES")? {
        overrides.build_binaries = Some(parse_bool_flag(&value, "MOCHI_BUILD_BINARIES")?);
    }
    if let Some(value) = env_value("MOCHI_READINESS_SMOKE")? {
        overrides.readiness_smoke = Some(parse_bool_flag(&value, "MOCHI_READINESS_SMOKE")?);
    }

    let env_restart_mode = env_value("MOCHI_RESTART_MODE")?
        .map(|value| parse_restart_mode_flag(&value))
        .transpose()?;
    let env_restart_max = env_value("MOCHI_RESTART_MAX")?
        .map(|value| parse_usize_flag(&value, "MOCHI_RESTART_MAX"))
        .transpose()?;
    let env_restart_backoff_ms = env_value("MOCHI_RESTART_BACKOFF_MS")?
        .map(|value| parse_u64_flag(&value, "MOCHI_RESTART_BACKOFF_MS"))
        .transpose()?;

    overrides.restart_policy =
        build_restart_policy_override(env_restart_mode, env_restart_max, env_restart_backoff_ms)?;

    Ok(overrides)
}

fn env_value(key: &str) -> Result<Option<String>, CliParseError> {
    match env::var(key) {
        Ok(value) => {
            if value.is_empty() {
                Ok(None)
            } else {
                Ok(Some(value))
            }
        }
        Err(env::VarError::NotPresent) => Ok(None),
        Err(env::VarError::NotUnicode(_)) => {
            Err(CliParseError::new(format!("{key} must be valid UTF-8")))
        }
    }
}

fn next_value<I>(iter: &mut I, flag: &str) -> Result<OsString, CliParseError>
where
    I: Iterator<Item = OsString>,
{
    iter.next()
        .ok_or_else(|| CliParseError::new(format!("expected value after {flag}")))
}

fn next_value_string<I>(iter: &mut I, flag: &str) -> Result<String, CliParseError>
where
    I: Iterator<Item = OsString>,
{
    next_value(iter, flag)?
        .into_string()
        .map_err(|_| CliParseError::new(format!("{flag} value must be valid UTF-8")))
}

fn apply_profile_override(
    overrides: &mut CliOverrides,
    parsed: ParsedProfileOverride,
    source: &str,
) -> Result<(), CliParseError> {
    overrides.profile = Some(parsed.profile);
    if let Some(genesis_profile) = parsed.genesis_profile {
        if let Some(existing) = overrides.genesis_profile
            && existing != genesis_profile
        {
            return Err(CliParseError::new(format!(
                "{source} genesis_profile `{genesis_profile}` conflicts with `{existing}`"
            )));
        }
        overrides.genesis_profile = Some(genesis_profile);
    }
    Ok(())
}

fn parse_profile_override(value: &str) -> Result<ParsedProfileOverride, CliParseError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(CliParseError::new("profile value must not be empty"));
    }
    if let Some(preset) = parse_profile_preset(trimmed) {
        return Ok(ParsedProfileOverride {
            profile: NetworkProfile::from_preset(preset),
            genesis_profile: None,
        });
    }
    if !trimmed.starts_with('{') && !trimmed.contains('=') {
        return Err(CliParseError::new(format!(
            "invalid profile `{trimmed}`; expected preset slug or inline TOML table"
        )));
    }
    let table_literal = if trimmed.starts_with('{') {
        trimmed.to_owned()
    } else {
        format!("{{ {trimmed} }}")
    };
    let doc = format!("profile = {table_literal}");
    let value: toml::Value = toml::from_str(&doc)
        .map_err(|err| CliParseError::new(format!("invalid profile table `{trimmed}`: {err}")))?;
    let root = value.as_table().ok_or_else(|| {
        CliParseError::new(format!(
            "invalid profile `{trimmed}`; expected an inline TOML table"
        ))
    })?;
    let table = root
        .get("profile")
        .and_then(TomlValue::as_table)
        .ok_or_else(|| {
            CliParseError::new(format!(
                "invalid profile `{trimmed}`; expected an inline TOML table"
            ))
        })?;
    parse_profile_table_override(table)
}

fn parse_profile_table_override(table: &TomlTable) -> Result<ParsedProfileOverride, CliParseError> {
    let peer_value = table
        .get("peer_count")
        .ok_or_else(|| CliParseError::new("profile override missing `peer_count`"))?;
    let peer_count = parse_profile_peer_count(peer_value)?;
    let consensus_value = table
        .get("consensus_mode")
        .ok_or_else(|| CliParseError::new("profile override missing `consensus_mode`"))?;
    let consensus_mode = parse_profile_consensus_mode(consensus_value)?;
    let genesis_profile = table
        .get("genesis_profile")
        .and_then(TomlValue::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.parse().map_err(|err: String| CliParseError::new(err)))
        .transpose()?;

    if genesis_profile.is_some() && consensus_mode != SumeragiConsensusMode::Npos {
        return Err(CliParseError::new(
            "profile override with genesis_profile requires consensus_mode = \"npos\"",
        ));
    }

    let profile = NetworkProfile::custom(peer_count, consensus_mode)
        .map_err(|err| CliParseError::new(format!("invalid profile override: {err}")))?;

    Ok(ParsedProfileOverride {
        profile,
        genesis_profile,
    })
}

fn parse_profile_peer_count(value: &TomlValue) -> Result<usize, CliParseError> {
    let raw = value
        .as_integer()
        .ok_or_else(|| CliParseError::new("profile override peer_count must be an integer"))?;
    if raw <= 0 {
        return Err(CliParseError::new(
            "profile override peer_count must be greater than zero",
        ));
    }
    let unsigned = u64::try_from(raw).map_err(|_| {
        CliParseError::new("profile override peer_count exceeds the supported range")
    })?;
    usize::try_from(unsigned)
        .map_err(|_| CliParseError::new("profile override peer_count exceeds the supported range"))
}

fn parse_profile_consensus_mode(value: &TomlValue) -> Result<SumeragiConsensusMode, CliParseError> {
    let raw = value
        .as_str()
        .ok_or_else(|| CliParseError::new("profile override consensus_mode must be a string"))?;
    let normalized = raw.trim().to_ascii_lowercase().replace('_', "-");
    match normalized.as_str() {
        "permissioned" | "permissioned-sumeragi" => Ok(SumeragiConsensusMode::Permissioned),
        "npos" => Ok(SumeragiConsensusMode::Npos),
        other => Err(CliParseError::new(format!(
            "profile override consensus_mode `{other}` is not supported"
        ))),
    }
}

fn parse_profile_preset(value: &str) -> Option<ProfilePreset> {
    match value {
        "single-peer" | "single_peer" | "singlepeer" => Some(ProfilePreset::SinglePeer),
        "four-peer-bft" | "four_peer_bft" | "fourpeerbft" => Some(ProfilePreset::FourPeerBft),
        _ => None,
    }
}

fn parse_genesis_profile_flag(value: &str) -> Result<GenesisProfile, CliParseError> {
    value.parse().map_err(CliParseError::new)
}

fn parse_nexus_config_file(path: &str) -> Result<toml::Table, CliParseError> {
    let contents = fs::read_to_string(path).map_err(|err| {
        CliParseError::new(format!("--nexus-config failed to read {path}: {err}"))
    })?;
    let value: toml::Value = toml::from_str(&contents).map_err(|err| {
        CliParseError::new(format!("--nexus-config failed to parse {path}: {err}"))
    })?;
    let Some(table) = value.as_table() else {
        return Err(CliParseError::new(format!(
            "--nexus-config {path} must contain a TOML table"
        )));
    };
    if let Some(nexus) = table.get("nexus") {
        let Some(nexus_table) = nexus.as_table() else {
            return Err(CliParseError::new(format!(
                "--nexus-config {path} expects `nexus` to be a TOML table"
            )));
        };
        Ok(nexus_table.clone())
    } else {
        Ok(table.clone())
    }
}

fn parse_port_flag(value: &str, flag: &str) -> Result<u16, CliParseError> {
    let port: u16 = value.parse().map_err(|_| {
        CliParseError::new(format!("{flag} expects an integer between 1 and 65535"))
    })?;
    if port == 0 {
        return Err(CliParseError::new(format!(
            "{flag} expects an integer between 1 and 65535"
        )));
    }
    Ok(port)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RestartModeFlag {
    Never,
    OnFailure,
}

fn parse_restart_mode_flag(value: &str) -> Result<RestartModeFlag, CliParseError> {
    match value.to_ascii_lowercase().as_str() {
        "never" => Ok(RestartModeFlag::Never),
        "on-failure" | "on_failure" | "onfailure" => Ok(RestartModeFlag::OnFailure),
        other => Err(CliParseError::new(format!(
            "--restart-mode expects `never` or `on-failure`, got `{other}`"
        ))),
    }
}

fn parse_usize_flag(value: &str, flag: &str) -> Result<usize, CliParseError> {
    value
        .parse::<usize>()
        .map_err(|_| CliParseError::new(format!("{flag} expects a non-negative integer")))
}

fn parse_u32_flag(value: &str, flag: &str) -> Result<u32, CliParseError> {
    value
        .parse::<u32>()
        .map_err(|_| CliParseError::new(format!("{flag} expects a non-negative integer")))
}

fn parse_u64_flag(value: &str, flag: &str) -> Result<u64, CliParseError> {
    value
        .parse::<u64>()
        .map_err(|_| CliParseError::new(format!("{flag} expects a non-negative integer")))
}

fn parse_bool_flag(value: &str, flag: &str) -> Result<bool, CliParseError> {
    match value.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Ok(true),
        "0" | "false" | "no" | "off" => Ok(false),
        other => Err(CliParseError::new(format!(
            "{flag} expects a boolean (true/false/1/0), got `{other}`"
        ))),
    }
}

fn default_restart_policy_params() -> (usize, u64) {
    match RestartPolicy::default() {
        RestartPolicy::OnFailure {
            max_restarts,
            backoff,
        } => {
            let millis = backoff.as_millis();
            let ms = if millis > u64::MAX as u128 {
                u64::MAX
            } else {
                millis as u64
            };
            (max_restarts, ms)
        }
        RestartPolicy::Never => (3, 1_000),
    }
}

fn build_restart_policy_override(
    mode: Option<RestartModeFlag>,
    max_restarts: Option<usize>,
    backoff_ms: Option<u64>,
) -> Result<Option<RestartPolicy>, CliParseError> {
    if mode.is_none() && max_restarts.is_none() && backoff_ms.is_none() {
        return Ok(None);
    }
    let (default_max, default_backoff) = default_restart_policy_params();
    match mode.unwrap_or(RestartModeFlag::OnFailure) {
        RestartModeFlag::Never => {
            if max_restarts.is_some() || backoff_ms.is_some() {
                return Err(CliParseError::new(
                    "--restart-mode never cannot be combined with --restart-max or --restart-backoff-ms",
                ));
            }
            Ok(Some(RestartPolicy::Never))
        }
        RestartModeFlag::OnFailure => {
            let attempts = max_restarts.unwrap_or(default_max);
            let delay = backoff_ms.unwrap_or(default_backoff);
            Ok(Some(RestartPolicy::OnFailure {
                max_restarts: attempts,
                backoff: Duration::from_millis(delay),
            }))
        }
    }
}

fn cli_overrides() -> CliOverrides {
    CLI_OVERRIDES
        .lock()
        .expect("cli overrides mutex poisoned")
        .clone()
}

fn set_cli_overrides(overrides: CliOverrides) {
    *CLI_OVERRIDES.lock().expect("cli overrides mutex poisoned") = overrides;
}

#[cfg(test)]
fn reset_cli_overrides_for_tests() {
    set_cli_overrides(CliOverrides::default());
}

fn print_cli_usage() {
    println!("MOCHI usage:");
    println!("  mochi [options]");
    println!("Options:");
    println!("  --data-root <path>           Override the supervisor data root.");
    println!(
        "  --profile <single-peer|four-peer-bft|{{ peer_count = 3, consensus_mode = \"permissioned\" }}>"
    );
    println!("                               Choose a preset or custom profile table.");
    println!("  --config <path>              Load overrides from a specific config file.");
    println!("  --torii-start <port>         Override the base Torii port.");
    println!("  --p2p-start <port>           Override the base P2P port.");
    println!("  --chain-id <string>          Override the generated chain id.");
    println!("  --genesis-profile <iroha3-dev|iroha3-testus|iroha3-nexus>");
    println!("                               Use a Kagami genesis preset.");
    println!("  --vrf-seed-hex <hex>         VRF seed (32-byte hex) for genesis profile.");
    println!("  --nexus-config <path>        Load Nexus lane/dataspace config from TOML.");
    println!("  --enable-nexus               Enable Nexus/multi-lane features.");
    println!("  --disable-nexus              Disable Nexus/multi-lane features.");
    println!("  --nexus-lane-count <count>   Override nexus.lane_count in generated configs.");
    println!("  --enable-da                  Enable data-availability gating.");
    println!("  --disable-da                 Disable data-availability gating.");
    println!("  --irohad <path>              Override the irohad binary path.");
    println!("  --kagami <path>              Override the kagami binary path.");
    println!("  --iroha-cli <path>           Override the iroha_cli binary path.");
    println!("  --build-binaries             Auto-build missing binaries via cargo.");
    println!("  --no-build-binaries          Disable auto-build of missing binaries.");
    println!("  --disable-smoke              Disable readiness smoke transactions.");
    println!("  --enable-smoke               Re-enable readiness smoke transactions.");
    println!("  --restart-mode <never|on-failure>");
    println!("                               Choose the peer restart policy.");
    println!("  --restart-max <attempts>     Override retry attempts in on-failure mode.");
    println!("  --restart-backoff-ms <millis>  Override the base retry backoff (ms).");
    println!("  -h, --help                   Show this help text.");
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ActiveView {
    Dashboard,
    Blocks,
    Events,
    Logs,
    State,
    Composer,
}

impl ActiveView {
    fn label(self) -> &'static str {
        match self {
            ActiveView::Dashboard => "Dashboard",
            ActiveView::Blocks => "Blocks",
            ActiveView::Events => "Events",
            ActiveView::Logs => "Logs",
            ActiveView::State => "State",
            ActiveView::Composer => "Composer",
        }
    }

    fn all() -> [ActiveView; 6] {
        [
            ActiveView::Dashboard,
            ActiveView::Blocks,
            ActiveView::Events,
            ActiveView::Logs,
            ActiveView::State,
            ActiveView::Composer,
        ]
    }
}

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

fn lag_to_usize(skipped: u64) -> usize {
    usize::try_from(skipped).unwrap_or(usize::MAX)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ComposerStep {
    Build,
    Raw,
    Preview,
}

impl ComposerStep {
    fn label(self) -> &'static str {
        match self {
            ComposerStep::Build => "1. Build",
            ComposerStep::Raw => "2. Raw JSON",
            ComposerStep::Preview => "3. Preview",
        }
    }

    fn all() -> [ComposerStep; 3] {
        [
            ComposerStep::Build,
            ComposerStep::Raw,
            ComposerStep::Preview,
        ]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ComposerInstructionKind {
    MintAsset,
    BurnAsset,
    TransferAsset,
    RegisterDomain,
    RegisterAccount,
    RegisterAssetDefinition,
    PublishSpaceDirectoryManifest,
    RegisterPinManifest,
    GrantRole,
    RevokeRole,
    AccountAdmissionPolicy,
    MultisigPropose,
}

impl ComposerInstructionKind {
    fn label(self) -> &'static str {
        match self {
            ComposerInstructionKind::MintAsset => "Mint asset",
            ComposerInstructionKind::BurnAsset => "Burn asset",
            ComposerInstructionKind::TransferAsset => "Transfer asset",
            ComposerInstructionKind::RegisterDomain => "Register domain",
            ComposerInstructionKind::RegisterAccount => "Register account",
            ComposerInstructionKind::RegisterAssetDefinition => "Register asset definition",
            ComposerInstructionKind::PublishSpaceDirectoryManifest => {
                "Publish space directory manifest"
            }
            ComposerInstructionKind::RegisterPinManifest => "Register pin manifest",
            ComposerInstructionKind::GrantRole => "Grant role",
            ComposerInstructionKind::RevokeRole => "Revoke role",
            ComposerInstructionKind::AccountAdmissionPolicy => "Account admission policy",
            ComposerInstructionKind::MultisigPropose => "Multisig proposal",
        }
    }

    fn all() -> [ComposerInstructionKind; 12] {
        [
            ComposerInstructionKind::MintAsset,
            ComposerInstructionKind::BurnAsset,
            ComposerInstructionKind::TransferAsset,
            ComposerInstructionKind::RegisterDomain,
            ComposerInstructionKind::RegisterAccount,
            ComposerInstructionKind::RegisterAssetDefinition,
            ComposerInstructionKind::PublishSpaceDirectoryManifest,
            ComposerInstructionKind::RegisterPinManifest,
            ComposerInstructionKind::GrantRole,
            ComposerInstructionKind::RevokeRole,
            ComposerInstructionKind::AccountAdmissionPolicy,
            ComposerInstructionKind::MultisigPropose,
        ]
    }

    fn permission(self) -> InstructionPermission {
        match self {
            ComposerInstructionKind::MintAsset => InstructionPermission::MintAsset,
            ComposerInstructionKind::BurnAsset => InstructionPermission::BurnAsset,
            ComposerInstructionKind::TransferAsset => InstructionPermission::TransferAsset,
            ComposerInstructionKind::RegisterDomain => InstructionPermission::RegisterDomain,
            ComposerInstructionKind::RegisterAccount => InstructionPermission::RegisterAccount,
            ComposerInstructionKind::RegisterAssetDefinition => {
                InstructionPermission::RegisterAssetDefinition
            }
            ComposerInstructionKind::PublishSpaceDirectoryManifest => {
                InstructionPermission::PublishSpaceDirectoryManifest
            }
            ComposerInstructionKind::RegisterPinManifest => {
                InstructionPermission::RegisterPinManifest
            }
            ComposerInstructionKind::GrantRole => InstructionPermission::GrantRole,
            ComposerInstructionKind::RevokeRole => InstructionPermission::RevokeRole,
            ComposerInstructionKind::AccountAdmissionPolicy => {
                InstructionPermission::AccountAdmissionPolicy
            }
            ComposerInstructionKind::MultisigPropose => InstructionPermission::MultisigPropose,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ComposerTemplate {
    MintRoseToSigner,
    MintCabbageToSigner,
    BurnRoseFromSigner,
    TransferRoseToTeammate,
    TransferRoseImplicitReceive,
    RegisterDomainSideGarden,
    RegisterAccountForDomain,
    RegisterAssetDefinitionLily,
    SpaceDirectoryAxtTouch,
    SpaceDirectoryAmxHandle,
    PinManifestSample,
    GrantCanRegisterDomain,
    RevokeCanRegisterDomain,
    AdmissionPolicyImplicitReceive,
    AdmissionPolicyExplicitOnly,
    MultisigProposeSample,
}

fn draft_fixture_payload(fixture: &str, field: &str) -> String {
    let value: Value = json::from_str(fixture).expect("composer draft fixture json");
    let Value::Object(map) = value else {
        panic!("composer draft fixture must be an object");
    };
    let payload = map
        .get(field)
        .unwrap_or_else(|| panic!("composer draft fixture missing field `{field}`"));
    json::to_string_pretty(payload).expect("composer draft fixture payload should encode")
}

impl ComposerTemplate {
    const fn label(self) -> &'static str {
        match self {
            ComposerTemplate::MintRoseToSigner => "Mint rose sample",
            ComposerTemplate::MintCabbageToSigner => "Mint cabbage sample",
            ComposerTemplate::BurnRoseFromSigner => "Burn rose sample",
            ComposerTemplate::TransferRoseToTeammate => "Transfer rose sample",
            ComposerTemplate::TransferRoseImplicitReceive => "Implicit receive sample",
            ComposerTemplate::RegisterDomainSideGarden => "Register domain sample",
            ComposerTemplate::RegisterAccountForDomain => "Register account sample",
            ComposerTemplate::RegisterAssetDefinitionLily => "Register asset def. sample",
            ComposerTemplate::SpaceDirectoryAxtTouch => "AXT touch manifest",
            ComposerTemplate::SpaceDirectoryAmxHandle => "AMX handle manifest",
            ComposerTemplate::PinManifestSample => "Pin manifest sample",
            ComposerTemplate::GrantCanRegisterDomain => "Grant registrar role",
            ComposerTemplate::RevokeCanRegisterDomain => "Revoke registrar role",
            ComposerTemplate::AdmissionPolicyImplicitReceive => "Implicit receive policy",
            ComposerTemplate::AdmissionPolicyExplicitOnly => "Explicit-only policy",
            ComposerTemplate::MultisigProposeSample => "Multisig propose sample",
        }
    }

    const fn description(self) -> &'static str {
        match self {
            ComposerTemplate::MintRoseToSigner => {
                "Prefill mint inputs using the bundled rose asset for the selected signer."
            }
            ComposerTemplate::MintCabbageToSigner => {
                "Prefill mint inputs using the bundled cabbage asset for the selected signer."
            }
            ComposerTemplate::BurnRoseFromSigner => {
                "Prefill burn inputs to retire rose supply held by the selected signer."
            }
            ComposerTemplate::TransferRoseToTeammate => {
                "Prefill a transfer from the selected signer to another development signer."
            }
            ComposerTemplate::TransferRoseImplicitReceive => {
                "Prefill a transfer that targets a new account to demo implicit receive."
            }
            ComposerTemplate::RegisterDomainSideGarden => {
                "Suggest registering the sample `side_garden` domain."
            }
            ComposerTemplate::RegisterAccountForDomain => {
                "Suggest registering `carol` in the selected signer's domain."
            }
            ComposerTemplate::RegisterAssetDefinitionLily => {
                "Suggest registering an infinitely mintable `lily` asset definition."
            }
            ComposerTemplate::SpaceDirectoryAxtTouch => {
                "Prefill a sample Space Directory manifest that allows AXT touches."
            }
            ComposerTemplate::SpaceDirectoryAmxHandle => {
                "Prefill a sample Space Directory manifest for AMX handle issuance."
            }
            ComposerTemplate::PinManifestSample => {
                "Prefill a sample SoraFS pin manifest registration."
            }
            ComposerTemplate::GrantCanRegisterDomain => {
                "Grant a sample registrar role (`can_register_domain`) to another signer."
            }
            ComposerTemplate::RevokeCanRegisterDomain => {
                "Revoke the registrar role from the sample teammate account."
            }
            ComposerTemplate::AdmissionPolicyImplicitReceive => {
                "Prefill an implicit-receive policy with caps, fee, and default role."
            }
            ComposerTemplate::AdmissionPolicyExplicitOnly => {
                "Prefill a policy that disables implicit account creation."
            }
            ComposerTemplate::MultisigProposeSample => {
                "Prefill a multisig proposal with a sample instruction batch."
            }
        }
    }

    fn options_for(kind: ComposerInstructionKind) -> &'static [ComposerTemplate] {
        const MINT: &[ComposerTemplate] = &[
            ComposerTemplate::MintRoseToSigner,
            ComposerTemplate::MintCabbageToSigner,
        ];
        const BURN: &[ComposerTemplate] = &[ComposerTemplate::BurnRoseFromSigner];
        const TRANSFER: &[ComposerTemplate] = &[
            ComposerTemplate::TransferRoseToTeammate,
            ComposerTemplate::TransferRoseImplicitReceive,
        ];
        const REGISTER_DOMAIN: &[ComposerTemplate] = &[ComposerTemplate::RegisterDomainSideGarden];
        const REGISTER_ACCOUNT: &[ComposerTemplate] = &[ComposerTemplate::RegisterAccountForDomain];
        const REGISTER_ASSET_DEF: &[ComposerTemplate] =
            &[ComposerTemplate::RegisterAssetDefinitionLily];
        const SPACE_DIRECTORY: &[ComposerTemplate] = &[
            ComposerTemplate::SpaceDirectoryAxtTouch,
            ComposerTemplate::SpaceDirectoryAmxHandle,
        ];
        const PIN_MANIFEST: &[ComposerTemplate] = &[ComposerTemplate::PinManifestSample];
        const GRANT_ROLE: &[ComposerTemplate] = &[ComposerTemplate::GrantCanRegisterDomain];
        const REVOKE_ROLE: &[ComposerTemplate] = &[ComposerTemplate::RevokeCanRegisterDomain];
        const ADMISSION_POLICY: &[ComposerTemplate] = &[
            ComposerTemplate::AdmissionPolicyImplicitReceive,
            ComposerTemplate::AdmissionPolicyExplicitOnly,
        ];
        const MULTISIG_PROPOSE: &[ComposerTemplate] = &[ComposerTemplate::MultisigProposeSample];

        match kind {
            ComposerInstructionKind::MintAsset => MINT,
            ComposerInstructionKind::BurnAsset => BURN,
            ComposerInstructionKind::TransferAsset => TRANSFER,
            ComposerInstructionKind::RegisterDomain => REGISTER_DOMAIN,
            ComposerInstructionKind::RegisterAccount => REGISTER_ACCOUNT,
            ComposerInstructionKind::RegisterAssetDefinition => REGISTER_ASSET_DEF,
            ComposerInstructionKind::PublishSpaceDirectoryManifest => SPACE_DIRECTORY,
            ComposerInstructionKind::RegisterPinManifest => PIN_MANIFEST,
            ComposerInstructionKind::GrantRole => GRANT_ROLE,
            ComposerInstructionKind::RevokeRole => REVOKE_ROLE,
            ComposerInstructionKind::AccountAdmissionPolicy => ADMISSION_POLICY,
            ComposerInstructionKind::MultisigPropose => MULTISIG_PROPOSE,
        }
    }

    fn apply(self, app: &mut MochiApp, available_signers: &[SigningAuthority]) {
        let signers: &[SigningAuthority] = if available_signers.is_empty() {
            development_signing_authorities()
        } else {
            available_signers
        };
        let signer = app
            .composer_selected_signer
            .and_then(|index| signers.get(index))
            .or_else(|| signers.first());

        match self {
            ComposerTemplate::MintRoseToSigner => {
                app.composer_instruction_kind = ComposerInstructionKind::MintAsset;
                let (owner, label) = signer
                    .map(|authority| (authority.account_id(), authority.label()))
                    .unwrap_or_else(|| {
                        let fallback = development_signing_authorities()
                            .first()
                            .expect("development authorities must not be empty");
                        (fallback.account_id(), fallback.label())
                    });
                app.composer_asset_id = template_asset_id("rose#wonderland", owner)
                    .unwrap_or_else(|| format!("rose#wonderland#{owner}"));
                app.composer_quantity = "10".to_owned();
                app.composer_destination_account.clear();
                app.last_info = Some(format!("Loaded rose mint template for {label}."));
            }
            ComposerTemplate::MintCabbageToSigner => {
                app.composer_instruction_kind = ComposerInstructionKind::MintAsset;
                let (owner, label) = signer
                    .map(|authority| (authority.account_id(), authority.label()))
                    .unwrap_or_else(|| {
                        let fallback = development_signing_authorities()
                            .first()
                            .expect("development authorities must not be empty");
                        (fallback.account_id(), fallback.label())
                    });
                app.composer_asset_id = template_asset_id("cabbage#garden_of_live_flowers", owner)
                    .unwrap_or_else(|| format!("cabbage#garden_of_live_flowers#{owner}"));
                app.composer_quantity = "5".to_owned();
                app.composer_destination_account.clear();
                app.last_info = Some(format!("Loaded cabbage mint template for {label}."));
            }
            ComposerTemplate::BurnRoseFromSigner => {
                app.composer_instruction_kind = ComposerInstructionKind::BurnAsset;
                let (owner, label) = signer
                    .map(|authority| (authority.account_id(), authority.label()))
                    .unwrap_or_else(|| {
                        let fallback = development_signing_authorities()
                            .first()
                            .expect("development authorities must not be empty");
                        (fallback.account_id(), fallback.label())
                    });
                app.composer_asset_id = template_asset_id("rose#wonderland", owner)
                    .unwrap_or_else(|| format!("rose#wonderland#{owner}"));
                app.composer_quantity = "1".to_owned();
                app.composer_destination_account.clear();
                app.last_info = Some(format!("Loaded rose burn template for {label}."));
            }
            ComposerTemplate::TransferRoseToTeammate => {
                app.composer_instruction_kind = ComposerInstructionKind::TransferAsset;
                let signer = signer.unwrap_or_else(|| {
                    development_signing_authorities()
                        .first()
                        .expect("development authorities must not be empty")
                });
                app.composer_asset_id = template_asset_id("rose#wonderland", signer.account_id())
                    .unwrap_or_else(|| format!("rose#wonderland#{}", signer.account_id()));
                app.composer_quantity = "2".to_owned();
                let destination = signers
                    .iter()
                    .find(|candidate| candidate.account_id() != signer.account_id())
                    .map(|authority| authority.account_id().to_string())
                    .unwrap_or_else(|| signer.account_id().to_string());
                app.composer_destination_account = destination;
                app.last_info = Some(format!(
                    "Loaded rose transfer template for {}.",
                    signer.label()
                ));
            }
            ComposerTemplate::TransferRoseImplicitReceive => {
                app.composer_instruction_kind = ComposerInstructionKind::TransferAsset;
                let signer = signer.unwrap_or_else(|| {
                    development_signing_authorities()
                        .first()
                        .expect("development authorities must not be empty")
                });
                let domain = signer.account_id().domain().to_string();
                app.composer_asset_id = template_asset_id("rose#wonderland", signer.account_id())
                    .unwrap_or_else(|| format!("rose#wonderland#{}", signer.account_id()));
                app.composer_quantity = "1".to_owned();
                app.composer_destination_account = format!("newcomer@{domain}");
                app.last_info = Some("Loaded implicit receive transfer template.".to_owned());
            }
            ComposerTemplate::RegisterDomainSideGarden => {
                app.composer_instruction_kind = ComposerInstructionKind::RegisterDomain;
                app.composer_domain_id = "side_garden".to_owned();
                app.last_info = Some("Loaded domain registration template.".to_owned());
            }
            ComposerTemplate::RegisterAccountForDomain => {
                app.composer_instruction_kind = ComposerInstructionKind::RegisterAccount;
                let domain = signer
                    .map(|authority| authority.account_id().domain().to_string())
                    .unwrap_or_else(|| "wonderland".to_owned());
                app.composer_account_id = format!("carol@{domain}");
                app.last_info = Some("Loaded account registration template.".to_owned());
            }
            ComposerTemplate::RegisterAssetDefinitionLily => {
                app.composer_instruction_kind = ComposerInstructionKind::RegisterAssetDefinition;
                let domain = signer
                    .map(|authority| authority.account_id().domain().to_string())
                    .unwrap_or_else(|| "wonderland".to_owned());
                app.composer_asset_definition_id = format!("lily#{domain}");
                app.composer_asset_definition_mintable = Mintable::Infinitely;
                app.composer_mintability_tokens = 1;
                app.last_info = Some("Loaded asset definition registration template.".to_owned());
            }
            ComposerTemplate::SpaceDirectoryAxtTouch => {
                app.composer_instruction_kind =
                    ComposerInstructionKind::PublishSpaceDirectoryManifest;
                app.composer_space_directory_manifest_json =
                    draft_fixture_payload(COMPOSER_DRAFT_SPACE_MANIFEST_TOUCH, "manifest");
                app.composer_pin_manifest_json.clear();
                app.last_info = Some("Loaded AXT touch manifest template.".to_owned());
            }
            ComposerTemplate::SpaceDirectoryAmxHandle => {
                app.composer_instruction_kind =
                    ComposerInstructionKind::PublishSpaceDirectoryManifest;
                app.composer_space_directory_manifest_json =
                    draft_fixture_payload(COMPOSER_DRAFT_SPACE_MANIFEST_HANDLE, "manifest");
                app.composer_pin_manifest_json.clear();
                app.last_info = Some("Loaded AMX handle manifest template.".to_owned());
            }
            ComposerTemplate::PinManifestSample => {
                app.composer_instruction_kind = ComposerInstructionKind::RegisterPinManifest;
                app.composer_pin_manifest_json =
                    draft_fixture_payload(COMPOSER_DRAFT_PIN_MANIFEST, "request");
                app.composer_space_directory_manifest_json.clear();
                app.last_info = Some("Loaded pin manifest template.".to_owned());
            }
            ComposerTemplate::GrantCanRegisterDomain => {
                app.composer_instruction_kind = ComposerInstructionKind::GrantRole;
                app.composer_role_id = "can_register_domain".to_owned();
                let recipient = signers
                    .iter()
                    .find(|candidate| {
                        Some(candidate.account_id()) != signer.map(SigningAuthority::account_id)
                    })
                    .or_else(|| development_signing_authorities().get(1))
                    .unwrap_or_else(|| {
                        development_signing_authorities()
                            .first()
                            .expect("development authorities must not be empty")
                    });
                app.composer_role_account = recipient.account_id().to_string();
                app.last_info = Some("Loaded role grant template.".to_owned());
            }
            ComposerTemplate::RevokeCanRegisterDomain => {
                app.composer_instruction_kind = ComposerInstructionKind::RevokeRole;
                app.composer_role_id = "can_register_domain".to_owned();
                let recipient = signers
                    .iter()
                    .find(|candidate| {
                        Some(candidate.account_id()) != signer.map(SigningAuthority::account_id)
                    })
                    .or_else(|| development_signing_authorities().get(1))
                    .unwrap_or_else(|| {
                        development_signing_authorities()
                            .first()
                            .expect("development authorities must not be empty")
                    });
                app.composer_role_account = recipient.account_id().to_string();
                app.last_info = Some("Loaded role revoke template.".to_owned());
            }
            ComposerTemplate::AdmissionPolicyImplicitReceive => {
                app.composer_instruction_kind = ComposerInstructionKind::AccountAdmissionPolicy;
                let domain = signer
                    .map(|authority| authority.account_id().domain().to_string())
                    .unwrap_or_else(|| "wonderland".to_owned());
                app.composer_admission_domain = domain.clone();
                app.composer_admission_mode = AccountAdmissionMode::ImplicitReceive;
                app.composer_admission_max_per_tx = "3".to_owned();
                app.composer_admission_max_per_block = "10".to_owned();
                app.composer_admission_fee_enabled = true;
                app.composer_admission_fee_asset = format!("rose#{domain}");
                app.composer_admission_fee_amount = "1".to_owned();
                app.composer_admission_fee_destination_burn = true;
                app.composer_admission_fee_destination_account.clear();
                app.composer_admission_min_initial_amounts =
                    format!("rose#{domain} = 5\ncabbage#{domain} = 1");
                app.composer_admission_default_role = "basic_user".to_owned();
                app.last_info = Some("Loaded implicit receive policy template.".to_owned());
            }
            ComposerTemplate::AdmissionPolicyExplicitOnly => {
                app.composer_instruction_kind = ComposerInstructionKind::AccountAdmissionPolicy;
                let domain = signer
                    .map(|authority| authority.account_id().domain().to_string())
                    .unwrap_or_else(|| "wonderland".to_owned());
                app.composer_admission_domain = domain;
                app.composer_admission_mode = AccountAdmissionMode::ExplicitOnly;
                app.composer_admission_max_per_tx.clear();
                app.composer_admission_max_per_block.clear();
                app.composer_admission_fee_enabled = false;
                app.composer_admission_fee_asset.clear();
                app.composer_admission_fee_amount.clear();
                app.composer_admission_fee_destination_burn = true;
                app.composer_admission_fee_destination_account.clear();
                app.composer_admission_min_initial_amounts.clear();
                app.composer_admission_default_role.clear();
                app.last_info = Some("Loaded explicit-only policy template.".to_owned());
            }
            ComposerTemplate::MultisigProposeSample => {
                app.composer_instruction_kind = ComposerInstructionKind::MultisigPropose;
                let domain = signer
                    .map(|authority| authority.account_id().domain().to_string())
                    .unwrap_or_else(|| "wonderland".to_owned());
                let owner = signer
                    .map(|authority| authority.account_id().to_string())
                    .unwrap_or_else(|| format!("alice@{domain}"));
                app.composer_multisig_account = format!("multisig@{domain}");
                let mut instructions = COMPOSER_MULTISIG_INSTRUCTIONS.trim().to_owned();
                instructions = instructions.replace("alice@wonderland", &owner);
                if let Some(other) = signers
                    .iter()
                    .find(|candidate| candidate.account_id().to_string() != owner)
                {
                    instructions =
                        instructions.replace("bob@wonderland", &other.account_id().to_string());
                }
                app.composer_multisig_instructions = instructions;
                app.composer_multisig_ttl_enabled = true;
                let mut policy_json = COMPOSER_MULTISIG_POLICY.trim().to_owned();
                policy_json = policy_json.replace("alice@wonderland", &owner);
                if let Some(other) = signers
                    .iter()
                    .find(|candidate| candidate.account_id().to_string() != owner)
                {
                    policy_json =
                        policy_json.replace("bob@wonderland", &other.account_id().to_string());
                }
                app.composer_multisig_ttl_ms = json::from_str::<MultisigSpec>(&policy_json)
                    .map(|policy| policy.transaction_ttl_ms.get())
                    .unwrap_or(3_600_000);
                app.composer_multisig_policy_json = policy_json;
                app.last_info = Some("Loaded multisig proposal template.".to_owned());
            }
        }

        app.composer_step = ComposerStep::Build;
        app.composer_preview = None;
        app.composer_submit_error = None;
        app.composer_submit_success = None;
        app.composer_error = None;
        app.last_error = None;
    }
}

fn template_asset_id(definition: &str, owner: &AccountId) -> Option<String> {
    let definition = definition.parse::<AssetDefinitionId>().ok()?;
    Some(AssetId::new(definition, owner.clone()).to_string())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MaintenanceDialog {
    ExportSnapshot,
    ConfirmReset,
    RestoreSnapshot,
    ResetLane,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MaintenanceTask {
    Snapshot,
    Reset,
    Restore,
    LaneReset,
}

#[derive(Debug, Clone)]
enum MaintenanceCommand {
    ExportSnapshot { label: Option<String> },
    Reset,
    Restore { target: String },
    ResetLane { lane_id: u32 },
}

impl MaintenanceCommand {
    fn task(&self) -> MaintenanceTask {
        match self {
            MaintenanceCommand::ExportSnapshot { .. } => MaintenanceTask::Snapshot,
            MaintenanceCommand::Reset => MaintenanceTask::Reset,
            MaintenanceCommand::Restore { .. } => MaintenanceTask::Restore,
            MaintenanceCommand::ResetLane { .. } => MaintenanceTask::LaneReset,
        }
    }
}

#[derive(Debug, Clone)]
struct MaintenanceBanner {
    color: Color32,
    text: String,
    show_spinner: bool,
    dismissable: bool,
}

#[derive(Debug, Clone, Default)]
enum MaintenanceState {
    #[default]
    Idle,
    Running(MaintenanceTask),
    Completed {
        message: String,
    },
    Failed {
        error: String,
    },
}

#[derive(Debug)]
enum MaintenanceOutcome {
    Snapshot(Result<PathBuf, String>),
    Reset(Result<(), String>),
    Restore(Result<PathBuf, String>),
    LaneReset(Result<(), String>),
}

#[derive(Debug)]
struct MaintenanceUpdate {
    supervisor: Supervisor,
    outcome: MaintenanceOutcome,
}

#[derive(Debug, Clone)]
struct LaneResetCandidate {
    lane_id: u32,
    alias: String,
    dataspace: String,
}

impl LaneResetCandidate {
    fn label(&self) -> String {
        format!(
            "lane {} ({})",
            self.lane_id,
            truncate(&format!("{} / {}", self.alias, self.dataspace), 48)
        )
    }
}

#[derive(Debug, Clone)]
struct LanePathPreview {
    lane_id: u32,
    alias: String,
    peer_alias: String,
    blocks_dir: PathBuf,
    merge_log: PathBuf,
}

#[derive(Debug, Clone)]
struct ReadinessRecord {
    outcome: ReadinessRecordOutcome,
    attempts: usize,
    total_elapsed: Duration,
    finished_at: Instant,
}

impl ReadinessRecord {
    fn ready(status: ToriiStatusSnapshot, elapsed: Duration, finished_at: Instant) -> Self {
        Self {
            outcome: ReadinessRecordOutcome::Ready { status },
            attempts: 0,
            total_elapsed: elapsed,
            finished_at,
        }
    }

    fn committed(outcome: ReadinessSmokeOutcome, finished_at: Instant) -> Self {
        let attempts = outcome.attempt.max(1);
        let elapsed = outcome.total_elapsed;
        Self {
            outcome: ReadinessRecordOutcome::Committed(outcome),
            attempts,
            total_elapsed: elapsed,
            finished_at,
        }
    }

    fn failed(
        error: ToriiErrorInfo,
        attempts: usize,
        elapsed: Duration,
        finished_at: Instant,
    ) -> Self {
        Self {
            outcome: ReadinessRecordOutcome::Failed { error },
            attempts: attempts.max(1),
            total_elapsed: elapsed,
            finished_at,
        }
    }

    fn label(&self) -> (String, Color32) {
        match &self.outcome {
            ReadinessRecordOutcome::Ready { status } => {
                let label = format!(
                    "Status ready (queue {}) in {:.1}s",
                    status.status.queue_size,
                    self.total_elapsed.as_secs_f32()
                );
                (label, Color32::from_rgb(80, 160, 80))
            }
            ReadinessRecordOutcome::Committed(outcome) => {
                let commit = &outcome.commit;
                let label = format!(
                    "Smoke committed at height {} (attempt {}) in {:.1}s",
                    commit.block_height,
                    outcome.attempt,
                    self.total_elapsed.as_secs_f32()
                );
                (label, Color32::from_rgb(80, 160, 80))
            }
            ReadinessRecordOutcome::Failed { error } => {
                let label = format!(
                    "Readiness failed after {} attempt(s): {}",
                    self.attempts,
                    error.label()
                );
                (label, error.color())
            }
        }
    }

    fn detail(&self) -> Option<String> {
        match &self.outcome {
            ReadinessRecordOutcome::Ready { status } => {
                Some(format!("queue {}", status.status.queue_size))
            }
            ReadinessRecordOutcome::Committed(outcome) => {
                let tx_hash = truncate(&outcome.commit.tx_hash.to_string(), 16);
                let mut detail = format!("height {} tx {}", outcome.commit.block_height, tx_hash);
                if let Some(status) = &outcome.status {
                    detail.push_str(&format!(" • queue {}", status.status.queue_size));
                }
                Some(detail)
            }
            ReadinessRecordOutcome::Failed { error } => error.detail.clone(),
        }
    }
}

#[derive(Debug, Clone)]
enum ReadinessRecordOutcome {
    Ready { status: ToriiStatusSnapshot },
    Committed(ReadinessSmokeOutcome),
    Failed { error: ToriiErrorInfo },
}

#[derive(Debug)]
enum ReadinessUpdate {
    Finished {
        alias: String,
        record: ReadinessRecord,
    },
}

#[derive(Debug, Clone, Default)]
struct PeerReadinessView {
    last_record: Option<ReadinessRecord>,
}

impl PeerReadinessView {
    fn update(&mut self, record: ReadinessRecord) {
        self.last_record = Some(record);
    }

    fn summary(&self) -> Option<(String, Color32)> {
        self.last_record.as_ref().map(ReadinessRecord::label)
    }

    fn detail(&self) -> Option<String> {
        self.last_record.as_ref().and_then(ReadinessRecord::detail)
    }

    fn age_seconds(&self) -> Option<f32> {
        self.last_record
            .as_ref()
            .map(|record| record.finished_at.elapsed().as_secs_f32())
    }
}

impl MaintenanceTask {
    fn running_message(self) -> &'static str {
        match self {
            MaintenanceTask::Snapshot => "Exporting snapshot…",
            MaintenanceTask::Reset => "Resetting local network…",
            MaintenanceTask::Restore => "Restoring snapshot…",
            MaintenanceTask::LaneReset => "Resetting lane storage…",
        }
    }
}

impl MaintenanceState {
    fn is_running(&self) -> bool {
        matches!(self, Self::Running(_))
    }

    fn banner(&self) -> Option<MaintenanceBanner> {
        match self {
            MaintenanceState::Idle => None,
            MaintenanceState::Running(task) => Some(MaintenanceBanner {
                color: Color32::from_rgb(200, 160, 64),
                text: task.running_message().to_owned(),
                show_spinner: true,
                dismissable: false,
            }),
            MaintenanceState::Completed { message, .. } => Some(MaintenanceBanner {
                color: Color32::from_rgb(80, 160, 80),
                text: message.clone(),
                show_spinner: false,
                dismissable: true,
            }),
            MaintenanceState::Failed { error, .. } => Some(MaintenanceBanner {
                color: Color32::from_rgb(200, 64, 64),
                text: error.clone(),
                show_spinner: false,
                dismissable: true,
            }),
        }
    }
}

fn main() -> eframe::Result<()> {
    #[cfg(target_os = "macos")]
    {
        const ENV: &str = "ICRATE_UNSAFE_DISABLE_RUNTIME_ASSERTS";
        if env::var_os(ENV).is_none() {
            match (|| -> std::io::Result<()> {
                let mut cmd = process::Command::new(env::current_exe()?);
                cmd.args(env::args_os().skip(1));
                cmd.env(ENV, "1");
                let status = cmd.status()?;
                process::exit(status.code().unwrap_or(1));
            })() {
                Ok(()) => unreachable!(),
                Err(err) => {
                    eprintln!("Failed to relaunch MOCHI with {ENV}=1: {err}");
                }
            }
        }
    }

    let parsed_cli = match parse_cli_overrides() {
        Ok(parsed) => parsed,
        Err(err) => {
            eprintln!("MOCHI: {err}");
            print_cli_usage();
            process::exit(1);
        }
    };

    if parsed_cli.help {
        print_cli_usage();
        return Ok(());
    }

    set_cli_overrides(parsed_cli.overrides);

    let options = NativeOptions::default();
    eframe::run_native(
        "MOCHI",
        options,
        Box::new(
            |cc| -> Result<Box<dyn App>, Box<dyn std::error::Error + Send + Sync>> {
                Ok(Box::new(MochiApp::new(cc)))
            },
        ),
    )
}

#[derive(Debug)]
struct ComposerSubmitUpdate {
    peer: String,
    result: Result<String, ToriiErrorInfo>,
}

#[derive(Debug)]
struct StateUpdate {
    peer: String,
    kind: StateQueryKind,
    reset: bool,
    result: Result<StatePage, String>,
}

#[derive(Debug, Clone)]
struct SignerEntryState {
    label: String,
    account: String,
    private_key: String,
    permissions: BTreeSet<InstructionPermission>,
    roles: String,
}

impl SignerEntryState {
    fn from_signer(signer: &SigningAuthority) -> Self {
        let private_key = ExposedPrivateKey(signer.key_pair().private_key().clone()).to_string();
        let permissions = signer.permissions().collect::<BTreeSet<_>>();
        let roles = signer
            .roles()
            .map(|role| role.to_string())
            .collect::<Vec<_>>()
            .join(", ");
        Self {
            label: signer.label().to_owned(),
            account: signer.account_id().to_string(),
            private_key,
            permissions,
            roles,
        }
    }
}

#[derive(Debug, Clone)]
struct SignerEntryForm {
    label: String,
    account: String,
    private_key: String,
    permissions: BTreeSet<InstructionPermission>,
    roles: String,
}

impl Default for SignerEntryForm {
    fn default() -> Self {
        Self {
            label: String::new(),
            account: String::new(),
            private_key: String::new(),
            permissions: InstructionPermission::all()
                .into_iter()
                .collect::<BTreeSet<_>>(),
            roles: String::new(),
        }
    }
}

impl SignerEntryForm {
    fn reset(&mut self) {
        *self = Self::default();
    }
}

#[derive(Debug)]
struct SignerVaultDialog {
    entries: Vec<SignerEntryState>,
    new_entry: SignerEntryForm,
    status: Option<String>,
    error: Option<String>,
    using_fallback: bool,
    dirty: bool,
}

impl SignerVaultDialog {
    fn load(supervisor: &Supervisor) -> Result<Self, String> {
        let vault = supervisor.signer_vault();
        let persisted = vault.load().map_err(|err| err.to_string())?;
        let using_fallback = persisted.is_empty();
        let entries = if using_fallback {
            supervisor
                .signers()
                .iter()
                .map(SignerEntryState::from_signer)
                .collect::<Vec<_>>()
        } else {
            persisted
                .iter()
                .map(SignerEntryState::from_signer)
                .collect::<Vec<_>>()
        };
        Ok(Self {
            entries,
            new_entry: SignerEntryForm::default(),
            status: None,
            error: None,
            using_fallback,
            dirty: false,
        })
    }

    fn mark_dirty(&mut self) {
        self.dirty = true;
        self.status = None;
    }
}

struct MochiApp {
    supervisor: Option<Supervisor>,
    supervisor_error: Option<SupervisorError>,
    bundle_config: Option<ResolvedBundleConfig>,
    cli_overrides: CliOverrides,
    last_error: Option<String>,
    last_info: Option<String>,
    theme_applied: bool,
    active_view: ActiveView,
    runtime: Runtime,
    block_stream: Option<ManagedBlockStream>,
    block_receiver: Option<BroadcastReceiver<BlockStreamEvent>>,
    block_events: Vec<DisplayEvent>,
    block_stream_peer: Option<String>,
    selected_peer: Option<String>,
    block_snapshots: HashMap<String, BlockStreamSnapshot>,
    event_stream: Option<ManagedEventStream>,
    event_receiver: Option<BroadcastReceiver<EventStreamEvent>>,
    event_events: Vec<EventDisplay>,
    event_stream_peer: Option<String>,
    event_selected_peer: Option<String>,
    event_snapshots: HashMap<String, EventSnapshot>,
    event_filter: EventFilterState,
    log_receiver: Option<BroadcastReceiver<PeerLogEvent>>,
    log_events: Vec<PeerLogEvent>,
    log_stream_peer: Option<String>,
    log_selected_peer: Option<String>,
    log_snapshots: HashMap<String, LogSnapshot>,
    log_filter: String,
    status_snapshots: HashMap<String, PeerStatusView>,
    status_streams: HashMap<String, StatusStreamHandle>,
    status_history: HashMap<String, VecDeque<StatusHistoryEntry>>,
    readiness_rx: UnboundedReceiver<ReadinessUpdate>,
    readiness_tx: UnboundedSender<ReadinessUpdate>,
    readiness_inflight: HashSet<String>,
    readiness_results: HashMap<String, PeerReadinessView>,
    state_selected_peer: Option<String>,
    state_query_kind: StateQueryKind,
    state_fetch_size: u32,
    state_tabs: StateTabs,
    state_rx: UnboundedReceiver<StateUpdate>,
    state_tx: UnboundedSender<StateUpdate>,
    composer_step: ComposerStep,
    composer_asset_id: String,
    composer_quantity: String,
    composer_chain_id: String,
    composer_ttl_override: bool,
    composer_ttl_ms: u64,
    composer_creation_override: bool,
    composer_creation_ms: u64,
    composer_nonce_override: bool,
    composer_nonce_value: u32,
    composer_selected_signer: Option<usize>,
    composer_destination_account: String,
    composer_domain_id: String,
    composer_account_id: String,
    composer_asset_definition_id: String,
    composer_asset_definition_mintable: Mintable,
    composer_mintability_tokens: u32,
    composer_role_id: String,
    composer_role_account: String,
    composer_admission_domain: String,
    composer_admission_mode: AccountAdmissionMode,
    composer_admission_max_per_tx: String,
    composer_admission_max_per_block: String,
    composer_admission_fee_enabled: bool,
    composer_admission_fee_asset: String,
    composer_admission_fee_amount: String,
    composer_admission_fee_destination_burn: bool,
    composer_admission_fee_destination_account: String,
    composer_admission_min_initial_amounts: String,
    composer_admission_default_role: String,
    composer_multisig_account: String,
    composer_multisig_instructions: String,
    composer_multisig_ttl_enabled: bool,
    composer_multisig_ttl_ms: u64,
    composer_multisig_policy_json: String,
    composer_space_directory_manifest_json: String,
    composer_pin_manifest_json: String,
    composer_instruction_kind: ComposerInstructionKind,
    composer_drafts: Vec<InstructionDraft>,
    composer_selected_peer: Option<String>,
    composer_preview: Option<TransactionPreview>,
    composer_error: Option<String>,
    composer_submit_error: Option<ToriiErrorInfo>,
    composer_submit_success: Option<String>,
    composer_submitting: bool,
    composer_raw_editor: String,
    composer_raw_error: Option<String>,
    composer_raw_dirty: bool,
    composer_submit_rx: UnboundedReceiver<ComposerSubmitUpdate>,
    composer_submit_tx: UnboundedSender<ComposerSubmitUpdate>,
    signer_dialog: Option<SignerVaultDialog>,
    maintenance_dialog: Option<MaintenanceDialog>,
    maintenance_state: MaintenanceState,
    maintenance_command: Option<MaintenanceCommand>,
    maintenance_inflight: Option<MaintenanceTask>,
    maintenance_rx: UnboundedReceiver<MaintenanceUpdate>,
    maintenance_tx: UnboundedSender<MaintenanceUpdate>,
    maintenance_snapshot_label: String,
    maintenance_restore_input: String,
    lane_reset_selection: Option<u32>,
    settings_dialog: bool,
    settings_data_root_input: String,
    settings_torii_port_input: String,
    settings_p2p_port_input: String,
    settings_chain_id_input: String,
    settings_profile_input: String,
    settings_nexus_enabled: bool,
    settings_nexus_lane_count_input: String,
    settings_nexus_lane_catalog_input: String,
    settings_nexus_dataspace_catalog_input: String,
    settings_sumeragi_da_enabled: bool,
    settings_torii_da_replay_dir_input: String,
    settings_torii_da_manifest_dir_input: String,
    settings_build_binaries: bool,
    settings_readiness_smoke: bool,
    settings_log_stdout: bool,
    settings_log_stderr: bool,
    settings_log_system: bool,
    settings_log_export_dir_input: String,
    log_export_dir: Option<PathBuf>,
    settings_state_export_dir_input: String,
    state_export_dir: Option<PathBuf>,
}

fn prepare_supervisor() -> (
    Option<Supervisor>,
    Option<SupervisorError>,
    Option<ResolvedBundleConfig>,
) {
    let overrides = cli_overrides();
    let config = match overrides.config_path.clone() {
        Some(path) => match load_bundle_config_at(path) {
            Ok(cfg) => Some(cfg),
            Err(err) => {
                eprintln!("MOCHI: {err}");
                None
            }
        },
        None => match load_bundle_config() {
            Ok(config) => config,
            Err(err) => {
                eprintln!("MOCHI: {err}");
                None
            }
        },
    };
    let mut builder = if let Some(profile) = overrides.profile.clone() {
        SupervisorBuilder::with_profile(profile)
    } else {
        SupervisorBuilder::new(ProfilePreset::SinglePeer)
    };

    if let Some(cfg) = config.as_ref() {
        builder = cfg.config.apply_to(builder);
    }

    builder = overrides.apply_to(builder);

    match builder.build() {
        Ok(supervisor) => (Some(supervisor), None, config),
        Err(err) => (None, Some(err), config),
    }
}

impl Default for MochiApp {
    fn default() -> Self {
        Self::with_event_filter(EventFilterState::default())
    }
}

impl MochiApp {
    fn new(cc: &CreationContext<'_>) -> Self {
        let filter = load_event_filter(cc.storage);
        Self::with_event_filter(filter)
    }

    fn with_event_filter(event_filter: EventFilterState) -> Self {
        let runtime = Runtime::new().expect("tokio runtime");
        let (state_tx, state_rx) = mpsc::unbounded_channel();
        let (composer_submit_tx, composer_submit_rx) = mpsc::unbounded_channel();
        let (maintenance_tx, maintenance_rx) = mpsc::unbounded_channel();
        let (readiness_tx, readiness_rx) = mpsc::unbounded_channel();
        let (supervisor, supervisor_error, bundle_config) = prepare_supervisor();
        let mut app = Self {
            supervisor,
            supervisor_error,
            bundle_config,
            cli_overrides: cli_overrides(),
            last_error: None,
            last_info: None,
            theme_applied: false,
            active_view: ActiveView::Dashboard,
            runtime,
            block_stream: None,
            block_receiver: None,
            block_events: Vec::new(),
            block_stream_peer: None,
            selected_peer: None,
            block_snapshots: HashMap::new(),
            event_stream: None,
            event_receiver: None,
            event_events: Vec::new(),
            event_stream_peer: None,
            event_selected_peer: None,
            event_snapshots: HashMap::new(),
            event_filter,
            log_receiver: None,
            log_events: Vec::new(),
            log_stream_peer: None,
            log_selected_peer: None,
            log_snapshots: HashMap::new(),
            log_filter: String::new(),
            status_snapshots: HashMap::new(),
            status_streams: HashMap::new(),
            status_history: HashMap::new(),
            readiness_rx,
            readiness_tx,
            readiness_inflight: HashSet::new(),
            readiness_results: HashMap::new(),
            state_selected_peer: None,
            state_query_kind: StateQueryKind::Accounts,
            state_fetch_size: 50,
            state_tabs: StateTabs::default(),
            state_rx,
            state_tx,
            composer_step: ComposerStep::Build,
            composer_asset_id: String::new(),
            composer_quantity: String::from("1"),
            composer_chain_id: String::new(),
            composer_ttl_override: false,
            composer_ttl_ms: 5_000,
            composer_creation_override: false,
            composer_creation_ms: current_unix_timestamp_ms(),
            composer_nonce_override: false,
            composer_nonce_value: 1,
            composer_selected_signer: None,
            composer_destination_account: String::new(),
            composer_domain_id: String::new(),
            composer_account_id: String::new(),
            composer_asset_definition_id: String::new(),
            composer_asset_definition_mintable: Mintable::Infinitely,
            composer_mintability_tokens: 1,
            composer_role_id: String::new(),
            composer_role_account: String::new(),
            composer_admission_domain: String::new(),
            composer_admission_mode: AccountAdmissionMode::ImplicitReceive,
            composer_admission_max_per_tx: String::new(),
            composer_admission_max_per_block: String::new(),
            composer_admission_fee_enabled: false,
            composer_admission_fee_asset: String::new(),
            composer_admission_fee_amount: String::new(),
            composer_admission_fee_destination_burn: true,
            composer_admission_fee_destination_account: String::new(),
            composer_admission_min_initial_amounts: String::new(),
            composer_admission_default_role: String::new(),
            composer_multisig_account: String::new(),
            composer_multisig_instructions: "[]".to_owned(),
            composer_multisig_ttl_enabled: false,
            composer_multisig_ttl_ms: 3_600_000,
            composer_multisig_policy_json: String::new(),
            composer_space_directory_manifest_json: String::new(),
            composer_pin_manifest_json: String::new(),
            composer_instruction_kind: ComposerInstructionKind::MintAsset,
            composer_drafts: Vec::new(),
            composer_selected_peer: None,
            composer_preview: None,
            composer_error: None,
            composer_submit_error: None,
            composer_submit_success: None,
            composer_submitting: false,
            composer_raw_editor: String::new(),
            composer_raw_error: None,
            composer_raw_dirty: false,
            composer_submit_rx,
            composer_submit_tx,
            signer_dialog: None,
            maintenance_dialog: None,
            maintenance_state: MaintenanceState::Idle,
            maintenance_command: None,
            maintenance_inflight: None,
            maintenance_rx,
            maintenance_tx,
            maintenance_snapshot_label: String::new(),
            maintenance_restore_input: String::new(),
            lane_reset_selection: None,
            settings_dialog: false,
            settings_data_root_input: String::new(),
            settings_torii_port_input: String::new(),
            settings_p2p_port_input: String::new(),
            settings_chain_id_input: String::new(),
            settings_profile_input: String::new(),
            settings_nexus_enabled: false,
            settings_nexus_lane_count_input: String::new(),
            settings_nexus_lane_catalog_input: String::new(),
            settings_nexus_dataspace_catalog_input: String::new(),
            settings_sumeragi_da_enabled: false,
            settings_torii_da_replay_dir_input: String::new(),
            settings_torii_da_manifest_dir_input: String::new(),
            settings_build_binaries: false,
            settings_readiness_smoke: true,
            settings_log_stdout: true,
            settings_log_stderr: true,
            settings_log_system: true,
            settings_log_export_dir_input: String::new(),
            log_export_dir: None,
            settings_state_export_dir_input: String::new(),
            state_export_dir: None,
        };
        app.sync_raw_editor_from_drafts();
        app.initialize_settings_from_supervisor();
        app.sync_log_export_dir_input();
        app.sync_state_export_dir_input();
        app
    }
}

#[cfg(test)]
fn socket_bind_available() -> bool {
    std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).is_ok()
}

#[cfg(test)]
mod cli_tests {
    use std::{
        env,
        ffi::OsString,
        path::{Path, PathBuf},
        sync::{Mutex, OnceLock},
    };

    use super::*;

    fn cli_env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    struct CliEnvGuard {
        key: &'static str,
        prev: Option<String>,
    }

    impl CliEnvGuard {
        fn set(key: &'static str, value: &str) -> Self {
            let prev = env::var(key).ok();
            // SAFETY: Tests serialise environment mutations via `cli_env_lock`.
            unsafe { env::set_var(key, value) };
            Self { key, prev }
        }
    }

    impl Drop for CliEnvGuard {
        fn drop(&mut self) {
            if let Some(prev) = self.prev.as_ref() {
                // SAFETY: Tests serialise environment mutations via `cli_env_lock`.
                unsafe { env::set_var(self.key, prev) };
            } else {
                // SAFETY: Tests serialise environment mutations via `cli_env_lock`.
                unsafe { env::remove_var(self.key) };
            }
        }
    }

    #[test]
    fn parse_cli_kagami_override_sets_path() {
        let args = vec![OsString::from("--kagami"), OsString::from("/tmp/kagami")];
        let parsed = parse_cli_overrides_from(args).expect("parse CLI");
        assert!(!parsed.help, "help should not be triggered");
        assert_eq!(
            parsed.overrides.binaries.kagami.as_deref(),
            Some(Path::new("/tmp/kagami"))
        );
    }

    #[test]
    fn parse_cli_config_override_sets_path() {
        let args = vec![
            OsString::from("--config"),
            OsString::from("/tmp/mochi.toml"),
        ];
        let parsed = parse_cli_overrides_from(args).expect("parse CLI");
        assert_eq!(
            parsed.overrides.config_path.as_deref(),
            Some(Path::new("/tmp/mochi.toml"))
        );
    }

    #[test]
    fn parse_cli_help_flag_short_circuits() {
        let parsed =
            parse_cli_overrides_from(vec![OsString::from("--help")]).expect("parse help flag");
        assert!(parsed.help, "help flag should be detected");
    }

    #[test]
    fn parse_cli_chain_id_override_sets_value() {
        let args = vec![OsString::from("--chain-id"), OsString::from("demo-chain")];
        let parsed = parse_cli_overrides_from(args).expect("parse CLI");
        assert_eq!(parsed.overrides.chain_id.as_deref(), Some("demo-chain"));
    }

    #[test]
    fn parse_cli_genesis_profile_sets_value() {
        let args = vec![
            OsString::from("--genesis-profile"),
            OsString::from("iroha3-dev"),
        ];
        let parsed = parse_cli_overrides_from(args).expect("parse CLI");
        assert_eq!(
            parsed.overrides.genesis_profile,
            Some(GenesisProfile::Iroha3Dev)
        );
    }

    #[test]
    fn parse_cli_profile_inline_table_sets_custom_profile() {
        let args = vec![
            OsString::from("--profile"),
            OsString::from("{ peer_count = 3, consensus_mode = \"permissioned\" }"),
        ];
        let parsed = parse_cli_overrides_from(args).expect("parse CLI");
        let profile = parsed.overrides.profile.expect("profile override");
        assert_eq!(profile.preset, None);
        assert_eq!(profile.topology.peer_count, 3);
        assert_eq!(profile.consensus_mode, SumeragiConsensusMode::Permissioned);
    }

    #[test]
    fn parse_cli_profile_inline_table_sets_genesis_profile() {
        let args = vec![
            OsString::from("--profile"),
            OsString::from(
                "{ peer_count = 4, consensus_mode = \"npos\", genesis_profile = \"iroha3-dev\" }",
            ),
        ];
        let parsed = parse_cli_overrides_from(args).expect("parse CLI");
        assert_eq!(
            parsed.overrides.genesis_profile,
            Some(GenesisProfile::Iroha3Dev)
        );
    }

    #[test]
    fn parse_cli_profile_genesis_conflict_errors() {
        let args = vec![
            OsString::from("--profile"),
            OsString::from(
                "{ peer_count = 4, consensus_mode = \"npos\", genesis_profile = \"iroha3-dev\" }",
            ),
            OsString::from("--genesis-profile"),
            OsString::from("iroha3-testus"),
        ];
        let err = parse_cli_overrides_from(args).expect_err("conflict should error");
        assert!(
            err.to_string().contains("conflicts"),
            "unexpected error message: {err}"
        );
    }

    #[test]
    fn parse_cli_vrf_seed_sets_value() {
        let args = vec![OsString::from("--vrf-seed-hex"), OsString::from("abcd")];
        let parsed = parse_cli_overrides_from(args).expect("parse CLI");
        assert_eq!(parsed.overrides.vrf_seed_hex.as_deref(), Some("abcd"));
    }

    #[test]
    fn parse_cli_nexus_config_sets_table() {
        let temp = tempfile::tempdir().expect("temp dir");
        let config_path = temp.path().join("nexus.toml");
        fs::write(
            &config_path,
            r#"
enabled = true
lane_count = 2
"#,
        )
        .expect("write nexus config");
        let args = vec![
            OsString::from("--nexus-config"),
            OsString::from(config_path.as_os_str()),
        ];
        let parsed = parse_cli_overrides_from(args).expect("parse CLI");
        let nexus = parsed.overrides.nexus_config.expect("nexus config");
        assert!(matches!(
            nexus.get("enabled"),
            Some(toml::Value::Boolean(true))
        ));
        assert_eq!(
            nexus.get("lane_count").and_then(toml::Value::as_integer),
            Some(2)
        );
    }

    #[test]
    fn parse_cli_nexus_flags_set_overrides() {
        let args = vec![
            OsString::from("--enable-nexus"),
            OsString::from("--nexus-lane-count"),
            OsString::from("3"),
        ];
        let parsed = parse_cli_overrides_from(args).expect("parse CLI");
        assert_eq!(parsed.overrides.nexus_enabled, Some(true));
        assert_eq!(parsed.overrides.nexus_lane_count, Some(3));
    }

    #[test]
    fn parse_cli_da_flags_set_overrides() {
        let parsed =
            parse_cli_overrides_from(vec![OsString::from("--disable-da")]).expect("parse CLI");
        assert_eq!(parsed.overrides.sumeragi_da_enabled, Some(false));
    }

    #[test]
    fn parse_cli_restart_mode_never_sets_policy() {
        let args = vec![OsString::from("--restart-mode"), OsString::from("never")];
        let parsed = parse_cli_overrides_from(args).expect("parse CLI");
        assert!(matches!(
            parsed.overrides.restart_policy,
            Some(RestartPolicy::Never)
        ));
    }

    #[test]
    fn parse_cli_restart_on_failure_overrides_attempts() {
        let args = vec![
            OsString::from("--restart-max"),
            OsString::from("5"),
            OsString::from("--restart-backoff-ms"),
            OsString::from("2500"),
        ];
        let parsed = parse_cli_overrides_from(args).expect("parse CLI");
        match parsed.overrides.restart_policy.expect("restart policy") {
            RestartPolicy::OnFailure {
                max_restarts,
                backoff,
            } => {
                assert_eq!(max_restarts, 5);
                assert_eq!(backoff, Duration::from_millis(2500));
            }
            RestartPolicy::Never => panic!("expected on-failure policy"),
        }
    }

    #[test]
    fn parse_cli_build_binaries_flag_enables_auto_build() {
        let parsed =
            parse_cli_overrides_from(vec![OsString::from("--build-binaries")]).expect("parse CLI");
        assert_eq!(parsed.overrides.build_binaries, Some(true));
    }

    #[test]
    fn parse_cli_no_build_binaries_flag_disables_auto_build() {
        let parsed = parse_cli_overrides_from(vec![OsString::from("--no-build-binaries")])
            .expect("parse CLI");
        assert_eq!(parsed.overrides.build_binaries, Some(false));
    }

    #[test]
    fn parse_cli_disable_smoke_flag_disables_readiness_smoke() {
        let parsed =
            parse_cli_overrides_from(vec![OsString::from("--disable-smoke")]).expect("parse CLI");
        assert_eq!(parsed.overrides.readiness_smoke, Some(false));
    }

    #[test]
    fn parse_cli_enable_smoke_flag_enables_readiness_smoke() {
        let parsed =
            parse_cli_overrides_from(vec![OsString::from("--enable-smoke")]).expect("parse CLI");
        assert_eq!(parsed.overrides.readiness_smoke, Some(true));
    }

    #[test]
    fn parse_cli_unknown_flag_errors() {
        let err = parse_cli_overrides_from(vec![OsString::from("--unknown")])
            .expect_err("unknown flag should error");
        assert!(
            err.to_string().contains("unknown flag"),
            "unexpected error message: {err}"
        );
    }

    #[test]
    fn env_profile_override_applies() {
        let _guard = cli_env_lock().lock().expect("env lock");
        let _profile = CliEnvGuard::set("MOCHI_PROFILE", "four-peer-bft");
        let overrides = parse_env_overrides().expect("parse env overrides");
        assert_eq!(
            overrides.profile,
            Some(NetworkProfile::from_preset(ProfilePreset::FourPeerBft))
        );
    }

    #[test]
    fn env_build_binaries_override_applies() {
        let _guard = cli_env_lock().lock().expect("env lock");
        let _build = CliEnvGuard::set("MOCHI_BUILD_BINARIES", "true");
        let overrides = parse_env_overrides().expect("parse env overrides");
        assert_eq!(overrides.build_binaries, Some(true));
    }

    #[test]
    fn env_readiness_smoke_override_applies() {
        let _guard = cli_env_lock().lock().expect("env lock");
        let _smoke = CliEnvGuard::set("MOCHI_READINESS_SMOKE", "false");
        let overrides = parse_env_overrides().expect("parse env overrides");
        assert_eq!(overrides.readiness_smoke, Some(false));
    }

    #[test]
    fn cli_flags_override_env_values() {
        let _guard = cli_env_lock().lock().expect("env lock");
        let _profile = CliEnvGuard::set("MOCHI_PROFILE", "four-peer-bft");
        let env_overrides = parse_env_overrides().expect("parse env overrides");

        let cli = parse_cli_overrides_from(vec![
            OsString::from("--profile"),
            OsString::from("single-peer"),
        ])
        .expect("parse CLI");

        let merged = merge_overrides(env_overrides, cli.overrides);
        assert_eq!(
            merged.profile,
            Some(NetworkProfile::from_preset(ProfilePreset::SinglePeer))
        );
    }

    #[cfg(unix)]
    fn write_kagami_override_stub(root: &Path) -> (PathBuf, PathBuf) {
        use std::os::unix::fs::PermissionsExt;

        let script_path = root.join("kagami_cli_override.sh");
        let log_path = root.join("kagami_cli_override.log");
        let script = r#"#!/bin/sh
set -e
SCRIPT_DIR="$(CDPATH= cd -- "$(dirname "$0")" && pwd)"
printf '%s\n' "$@" > "$SCRIPT_DIR/kagami_cli_override.log"
cat <<'JSON'
{"chain":"00000000-0000-0000-0000-000000000000","ivm_dir":".","transactions":[{"instructions":[]}]}
JSON
"#;
        std::fs::write(&script_path, script).expect("write kagami CLI override stub");
        let mut perms = std::fs::metadata(&script_path)
            .expect("override stub metadata")
            .permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&script_path, perms).expect("set override stub permissions");
        (script_path, log_path)
    }

    #[cfg(unix)]
    #[test]
    fn cli_overrides_apply_kagami_path_to_supervisor_builder() {
        if !super::socket_bind_available() {
            eprintln!("Skipping CLI override supervisor test due to socket restrictions");
            return;
        }
        let temp = tempfile::tempdir().expect("temp dir");
        let (script_path, log_path) = write_kagami_override_stub(temp.path());

        let mut overrides = CliOverrides::default();
        overrides.binaries.kagami = Some(script_path.clone());

        let builder = SupervisorBuilder::new(ProfilePreset::SinglePeer).data_root(temp.path());
        overrides
            .apply_to(builder)
            .build()
            .expect("build supervisor with CLI overrides");

        let log = std::fs::read_to_string(&log_path).expect("read CLI override kagami log");
        assert!(
            log.contains("--genesis-public-key"),
            "expected CLI override stub to capture genesis args, got `{log}`"
        );
    }
}

impl Drop for MochiApp {
    fn drop(&mut self) {
        if let Some(stream) = self.block_stream.take() {
            stream.abort();
        }
        if let Some(stream) = self.event_stream.take() {
            stream.abort();
        }
        self.block_receiver = None;
        self.event_receiver = None;
        self.log_receiver = None;
    }
}

#[derive(Clone, Copy)]
struct UiPalette {
    base: Color32,
    panel: Color32,
    panel_alt: Color32,
    surface: Color32,
    surface_hover: Color32,
    surface_active: Color32,
    border: Color32,
    text: Color32,
    text_muted: Color32,
    accent: Color32,
    accent_soft: Color32,
    success: Color32,
    warning: Color32,
    danger: Color32,
}

impl UiPalette {
    fn new() -> Self {
        Self {
            base: Color32::from_rgb(14, 18, 24),
            panel: Color32::from_rgb(18, 24, 32),
            panel_alt: Color32::from_rgb(24, 32, 42),
            surface: Color32::from_rgb(32, 42, 54),
            surface_hover: Color32::from_rgb(45, 58, 70),
            surface_active: Color32::from_rgb(56, 86, 110),
            border: Color32::from_rgb(46, 64, 84),
            text: Color32::from_rgb(226, 232, 240),
            text_muted: Color32::from_rgb(160, 172, 186),
            accent: Color32::from_rgb(88, 200, 190),
            accent_soft: Color32::from_rgb(34, 120, 128),
            success: Color32::from_rgb(120, 200, 160),
            warning: Color32::from_rgb(240, 192, 96),
            danger: Color32::from_rgb(220, 96, 92),
        }
    }
}

impl MochiApp {
    fn ensure_theme(&mut self, ctx: &egui::Context) {
        if self.theme_applied {
            return;
        }
        let palette = Self::palette();
        let mut visuals = Visuals::dark();
        visuals.extreme_bg_color = palette.base;
        visuals.faint_bg_color = palette.panel;
        visuals.panel_fill = palette.panel;
        visuals.window_fill = palette.panel;
        visuals.window_stroke = Stroke::new(1.0, palette.border);
        visuals.window_corner_radius = CornerRadius::same(12);
        visuals.menu_corner_radius = CornerRadius::same(8);
        visuals.selection.bg_fill = palette.accent;
        visuals.selection.stroke = Stroke::new(1.0, palette.accent);
        visuals.hyperlink_color = palette.accent;
        visuals.warn_fg_color = palette.warning;
        visuals.error_fg_color = palette.danger;
        visuals.weak_text_color = Some(palette.text_muted);
        visuals.text_edit_bg_color = Some(palette.surface);
        visuals.code_bg_color = palette.surface;
        visuals.window_shadow = Shadow {
            offset: [12, 16],
            blur: 18,
            spread: 0,
            color: Color32::from_black_alpha(96),
        };
        visuals.popup_shadow = Shadow {
            offset: [8, 10],
            blur: 12,
            spread: 0,
            color: Color32::from_black_alpha(96),
        };
        visuals.widgets.noninteractive.bg_fill = palette.panel_alt;
        visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, palette.border);
        visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, palette.text_muted);
        visuals.widgets.inactive.bg_fill = palette.surface;
        visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, palette.border);
        visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, palette.text);
        visuals.widgets.hovered.bg_fill = palette.surface_hover;
        visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, palette.accent);
        visuals.widgets.hovered.fg_stroke = Stroke::new(1.0, palette.text);
        visuals.widgets.active.bg_fill = palette.surface_active;
        visuals.widgets.active.bg_stroke = Stroke::new(1.0, palette.accent);
        visuals.widgets.active.fg_stroke = Stroke::new(1.0, palette.text);
        let widget_radius = CornerRadius::same(8);
        visuals.widgets.noninteractive.corner_radius = widget_radius;
        visuals.widgets.inactive.corner_radius = widget_radius;
        visuals.widgets.hovered.corner_radius = widget_radius;
        visuals.widgets.active.corner_radius = widget_radius;

        let mut style = (*ctx.style()).clone();
        style.visuals = visuals;
        style.spacing.item_spacing = egui::vec2(10.0, 8.0);
        style.spacing.button_padding = egui::vec2(10.0, 6.0);
        style.spacing.window_margin = Margin::symmetric(14, 12);
        style.spacing.menu_margin = Margin::symmetric(10, 8);
        style.spacing.interact_size = egui::vec2(40.0, 30.0);
        style.spacing.text_edit_width = 240.0;
        style.spacing.combo_width = 180.0;
        style.text_styles.insert(
            TextStyle::Heading,
            FontId::new(20.0, FontFamily::Proportional),
        );
        style
            .text_styles
            .insert(TextStyle::Body, FontId::new(15.0, FontFamily::Proportional));
        style.text_styles.insert(
            TextStyle::Button,
            FontId::new(14.0, FontFamily::Proportional),
        );
        style.text_styles.insert(
            TextStyle::Small,
            FontId::new(12.0, FontFamily::Proportional),
        );
        style.text_styles.insert(
            TextStyle::Monospace,
            FontId::new(13.0, FontFamily::Monospace),
        );
        ctx.set_style(style);
        self.theme_applied = true;
    }

    fn palette() -> UiPalette {
        UiPalette::new()
    }

    fn profile_name(supervisor: &Supervisor) -> String {
        supervisor.profile().label()
    }

    fn copy_text(ui: &mut egui::Ui, text: impl Into<String>) {
        let text = text.into();
        ui.output_mut(|output| {
            output.commands.push(OutputCommand::CopyText(text));
        });
    }

    fn poll_block_stream_events(&mut self) {
        if let Some(mut receiver) = self.block_receiver.take() {
            let mut keep_receiver = true;
            loop {
                match receiver.try_recv() {
                    Ok(event) => {
                        let alias = self.block_stream_peer.clone();
                        self.record_stream_event(alias.clone(), &event);
                        if matches!(event, BlockStreamEvent::Closed) {
                            Self::push_block_event(&mut self.block_events, alias, event);
                            self.block_stream = None;
                            self.block_stream_peer = None;
                            keep_receiver = false;
                            break;
                        } else {
                            Self::push_block_event(&mut self.block_events, alias, event);
                        }
                    }
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Lagged(skipped)) => {
                        let skipped = lag_to_usize(skipped);
                        let alias = self.block_stream_peer.clone();
                        self.record_stream_event(
                            alias.clone(),
                            &BlockStreamEvent::Lagged { skipped },
                        );
                        Self::push_block_event(
                            &mut self.block_events,
                            alias,
                            BlockStreamEvent::Lagged { skipped },
                        );
                    }
                    Err(TryRecvError::Closed) => {
                        let alias = self.block_stream_peer.clone();
                        self.record_stream_event(alias.clone(), &BlockStreamEvent::Closed);
                        Self::push_block_event(
                            &mut self.block_events,
                            alias,
                            BlockStreamEvent::Closed,
                        );
                        self.block_stream = None;
                        self.block_stream_peer = None;
                        keep_receiver = false;
                        break;
                    }
                }
            }

            if keep_receiver {
                self.block_receiver = Some(receiver);
            }
        }

        if self
            .block_stream
            .as_ref()
            .map(|stream| stream.is_finished())
            .unwrap_or(false)
        {
            if let Some(stream) = self.block_stream.take() {
                stream.abort();
            }
            let alias = self.block_stream_peer.clone();
            self.block_receiver = None;
            self.block_stream_peer = None;
            if !matches!(
                self.block_events.last().map(|entry| &entry.event),
                Some(BlockStreamEvent::Closed)
            ) {
                Self::push_block_event(&mut self.block_events, alias, BlockStreamEvent::Closed);
            }
        }
    }

    fn poll_event_stream_events(&mut self) {
        if let Some(mut receiver) = self.event_receiver.take() {
            let mut keep_receiver = true;
            loop {
                match receiver.try_recv() {
                    Ok(event) => {
                        let alias = self.event_stream_peer.clone();
                        self.record_event_stream_event(alias.clone(), &event);
                        if matches!(event, EventStreamEvent::Closed) {
                            Self::push_event_event(&mut self.event_events, alias, event);
                            self.event_stream = None;
                            self.event_stream_peer = None;
                            keep_receiver = false;
                            break;
                        } else {
                            Self::push_event_event(&mut self.event_events, alias, event);
                        }
                    }
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Lagged(skipped)) => {
                        let skipped = lag_to_usize(skipped);
                        let alias = self.event_stream_peer.clone();
                        let lag_event = EventStreamEvent::Lagged { skipped };
                        self.record_event_stream_event(alias.clone(), &lag_event);
                        Self::push_event_event(&mut self.event_events, alias, lag_event);
                    }
                    Err(TryRecvError::Closed) => {
                        let alias = self.event_stream_peer.clone();
                        let closed = EventStreamEvent::Closed;
                        self.record_event_stream_event(alias.clone(), &closed);
                        Self::push_event_event(&mut self.event_events, alias, closed);
                        self.event_stream = None;
                        self.event_stream_peer = None;
                        keep_receiver = false;
                        break;
                    }
                }
            }

            if keep_receiver {
                self.event_receiver = Some(receiver);
            }
        }

        if self
            .event_stream
            .as_ref()
            .map(|stream| stream.is_finished())
            .unwrap_or(false)
        {
            if let Some(stream) = self.event_stream.take() {
                stream.abort();
            }
            let alias = self.event_stream_peer.clone();
            self.event_receiver = None;
            self.event_stream_peer = None;
            let closed = EventStreamEvent::Closed;
            self.record_event_stream_event(alias.clone(), &closed);
            Self::push_event_event(&mut self.event_events, alias, closed);
        }
    }

    fn poll_log_events(&mut self) {
        if let Some(mut receiver) = self.log_receiver.take() {
            let mut keep_receiver = true;
            loop {
                match receiver.try_recv() {
                    Ok(event) => {
                        self.push_log_event(event);
                    }
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Lagged(skipped)) => {
                        if let Some(alias) = self.log_stream_peer.clone() {
                            let warning = Self::system_log_event(
                                &alias,
                                format!(
                                    "Log receiver lagged and skipped {} event(s).",
                                    lag_to_usize(skipped)
                                ),
                            );
                            self.push_log_event(warning);
                        }
                    }
                    Err(TryRecvError::Closed) => {
                        if let Some(alias) = self.log_stream_peer.clone() {
                            let closed =
                                Self::system_log_event(&alias, "Log stream closed.".to_owned());
                            self.push_log_event(closed);
                        }
                        self.log_stream_peer = None;
                        keep_receiver = false;
                        break;
                    }
                }
            }

            if keep_receiver {
                self.log_receiver = Some(receiver);
            }
        }
    }

    fn poll_maintenance_updates(&mut self, supervisor_slot: &mut Option<Supervisor>) {
        while let Ok(update) = self.maintenance_rx.try_recv() {
            *supervisor_slot = Some(update.supervisor);
            self.maintenance_inflight = None;
            match update.outcome {
                MaintenanceOutcome::Snapshot(Ok(path)) => {
                    let message = format!("Snapshot exported to {}", path.display());
                    self.last_info = Some(message.clone());
                    self.last_error = None;
                    self.complete_maintenance(message);
                    self.maintenance_dialog = None;
                    self.maintenance_snapshot_label.clear();
                }
                MaintenanceOutcome::Snapshot(Err(err)) => {
                    self.last_info = None;
                    self.last_error = Some(err.clone());
                    self.fail_maintenance(err);
                }
                MaintenanceOutcome::Reset(Ok(())) => {
                    self.reset_runtime_state_after_maintenance();
                    let message = "Local network reset and genesis regenerated.".to_owned();
                    self.last_info = Some(message.clone());
                    self.last_error = None;
                    self.complete_maintenance(message);
                    self.maintenance_dialog = None;
                    self.maintenance_snapshot_label.clear();
                    if let Some(supervisor) = supervisor_slot.as_ref()
                        && supervisor.is_any_running()
                    {
                        let aliases = supervisor
                            .peers()
                            .iter()
                            .filter(|peer| matches!(peer.state(), PeerState::Running))
                            .map(|peer| peer.alias().to_owned())
                            .collect::<Vec<_>>();
                        self.spawn_readiness_checks(supervisor, aliases);
                    }
                }
                MaintenanceOutcome::Reset(Err(err)) => {
                    self.last_info = None;
                    self.last_error = Some(err.clone());
                    self.fail_maintenance(err);
                }
                MaintenanceOutcome::Restore(Ok(path)) => {
                    self.reset_runtime_state_after_maintenance();
                    let message = format!("Snapshot restored from {}", path.display());
                    self.last_info = Some(message.clone());
                    self.last_error = None;
                    self.complete_maintenance(message);
                    self.maintenance_dialog = None;
                    self.maintenance_restore_input.clear();
                    if let Some(supervisor) = supervisor_slot.as_ref()
                        && supervisor.is_any_running()
                    {
                        let aliases = supervisor
                            .peers()
                            .iter()
                            .filter(|peer| matches!(peer.state(), PeerState::Running))
                            .map(|peer| peer.alias().to_owned())
                            .collect::<Vec<_>>();
                        self.spawn_readiness_checks(supervisor, aliases);
                    }
                }
                MaintenanceOutcome::Restore(Err(err)) => {
                    self.last_info = None;
                    self.last_error = Some(err.clone());
                    self.fail_maintenance(err);
                }
                MaintenanceOutcome::LaneReset(Ok(())) => {
                    self.reset_runtime_state_after_maintenance();
                    let message = "Lane reset complete.".to_owned();
                    self.last_info = Some(message.clone());
                    self.last_error = None;
                    self.complete_maintenance(message);
                    self.maintenance_dialog = None;
                    self.lane_reset_selection = None;
                    if let Some(supervisor) = supervisor_slot.as_ref()
                        && supervisor.is_any_running()
                    {
                        let aliases = supervisor
                            .peers()
                            .iter()
                            .filter(|peer| matches!(peer.state(), PeerState::Running))
                            .map(|peer| peer.alias().to_owned())
                            .collect::<Vec<_>>();
                        self.spawn_readiness_checks(supervisor, aliases);
                    }
                }
                MaintenanceOutcome::LaneReset(Err(err)) => {
                    self.last_info = None;
                    self.last_error = Some(err.clone());
                    self.fail_maintenance(err);
                }
            }
        }
    }

    fn schedule_pending_maintenance(&mut self, supervisor_slot: &mut Option<Supervisor>) {
        if self.maintenance_inflight.is_some() {
            return;
        }
        let Some(command) = self.maintenance_command.take() else {
            return;
        };
        let Some(supervisor) = supervisor_slot.take() else {
            self.maintenance_command = Some(command);
            return;
        };

        let tx = self.maintenance_tx.clone();
        let task = command.task();
        self.maintenance_inflight = Some(task);
        let handle = self.runtime.handle().clone();

        self.runtime.spawn_blocking(move || {
            let mut supervisor = supervisor;
            let outcome = match command {
                MaintenanceCommand::ExportSnapshot { label } => {
                    let result = supervisor
                        .export_snapshot(label.as_deref())
                        .map_err(|err| err.to_string());
                    MaintenanceOutcome::Snapshot(result)
                }
                MaintenanceCommand::Reset => {
                    let result = supervisor
                        .wipe_and_regenerate()
                        .map_err(|err| err.to_string());
                    MaintenanceOutcome::Reset(result)
                }
                MaintenanceCommand::Restore { target } => {
                    let result = supervisor
                        .restore_snapshot(target)
                        .map_err(|err| err.to_string());
                    MaintenanceOutcome::Restore(result)
                }
                MaintenanceCommand::ResetLane { lane_id } => {
                    let result = Self::reset_lane_with_lifecycle(&mut supervisor, lane_id, &handle);
                    MaintenanceOutcome::LaneReset(result)
                }
            };
            let update = MaintenanceUpdate {
                supervisor,
                outcome,
            };
            let _ = tx.send(update);
        });
    }

    fn poll_state_updates(&mut self) {
        loop {
            match self.state_rx.try_recv() {
                Ok(update) => self.handle_state_update(update),
                Err(mpsc::error::TryRecvError::Empty) => break,
                Err(mpsc::error::TryRecvError::Disconnected) => break,
            }
        }
    }

    fn handle_state_update(&mut self, update: StateUpdate) {
        let Some(current_peer) = self.state_selected_peer.as_deref() else {
            return;
        };
        if current_peer != update.peer {
            return;
        }
        let tab = self.state_tabs.get_mut(update.kind);
        tab.loading = false;
        match update.result {
            Ok(page) => {
                let StatePage {
                    entries,
                    remaining,
                    cursor,
                    ..
                } = page;
                let cache = StatePageCache { entries, remaining };
                if update.reset {
                    tab.pages = vec![cache];
                    tab.current_page = 0;
                } else {
                    tab.pages.push(cache);
                    tab.current_page = tab.pages.len().saturating_sub(1);
                }
                tab.cursor = cursor;
                tab.error = None;
                let index = tab.current_page;
                tab.select_page(index);
            }
            Err(err) => {
                if update.reset {
                    tab.reset_results();
                }
                tab.error = Some(err);
            }
        }
    }

    fn poll_composer_updates(&mut self) {
        loop {
            match self.composer_submit_rx.try_recv() {
                Ok(update) => self.handle_composer_update(update),
                Err(mpsc::error::TryRecvError::Empty) => break,
                Err(mpsc::error::TryRecvError::Disconnected) => break,
            }
        }
    }

    fn handle_composer_update(&mut self, update: ComposerSubmitUpdate) {
        self.composer_submitting = false;
        match update.result {
            Ok(hash) => {
                let message = format!("Submitted transaction {hash} to {}.", update.peer);
                self.composer_submit_error = None;
                self.composer_submit_success = Some(message.clone());
                self.last_info = Some(message);
                self.last_error = None;
            }
            Err(err) => {
                let label = err.label();
                let message = format!("Failed to submit transaction to {}: {label}", update.peer);
                self.composer_submit_error = Some(err);
                self.composer_submit_success = None;
                self.last_info = None;
                self.last_error = Some(message);
            }
        }
    }

    fn select_state_page(&mut self, kind: StateQueryKind, index: usize) {
        let tab = self.state_tabs.get_mut(kind);
        tab.select_page(index);
    }

    fn request_state_page(&mut self, supervisor: &Supervisor, reset: bool) {
        let kind = self.state_query_kind;
        let Some(peer) = self.state_selected_peer.clone() else {
            let tab = self.state_tabs.get_mut(kind);
            tab.loading = false;
            tab.error = Some("No peer selected for state query.".to_owned());
            return;
        };
        let Some(client) = supervisor.torii_client(&peer) else {
            let tab = self.state_tabs.get_mut(kind);
            tab.loading = false;
            tab.error = Some("Torii client unavailable for selected peer.".to_owned());
            return;
        };

        let fetch_size = NonZeroU64::new(self.state_fetch_size as u64);
        let cursor = {
            let tab = self.state_tabs.get_mut(kind);
            if reset {
                tab.reset_results();
            }
            tab.error = None;
            tab.loading = true;
            if reset { None } else { tab.cursor.clone() }
        };
        let tx = self.state_tx.clone();

        self.runtime.spawn(async move {
            let result = run_state_query(client, kind, cursor, fetch_size)
                .await
                .map_err(|err| err.to_string());
            let _ = tx.send(StateUpdate {
                peer,
                kind,
                reset,
                result,
            });
        });
    }

    fn sync_status_streams(&mut self, supervisor: &Supervisor, aliases: &[String]) {
        let desired: HashSet<String> = aliases.iter().cloned().collect();
        self.status_snapshots
            .retain(|alias, _| desired.contains(alias));
        self.status_history
            .retain(|alias, _| desired.contains(alias));
        self.status_streams.retain(|alias, handle| {
            if desired.contains(alias) {
                true
            } else {
                handle.abort();
                false
            }
        });

        let runtime_handle = self.runtime.handle().clone();
        for alias in aliases {
            if self.status_streams.contains_key(alias) {
                continue;
            }
            if let Some(client) = supervisor.torii_client(alias) {
                let stream = StatusStreamHandle::new(&runtime_handle, alias.clone(), client);
                self.status_streams.insert(alias.clone(), stream);
            }
        }
    }

    fn configured_build_binaries(&self) -> bool {
        self.cli_overrides
            .build_binaries
            .or_else(|| {
                self.bundle_config
                    .as_ref()
                    .and_then(|cfg| cfg.config.build_binaries)
            })
            .unwrap_or(false)
    }

    fn configured_readiness_smoke(&self) -> bool {
        self.cli_overrides
            .readiness_smoke
            .or_else(|| {
                self.bundle_config
                    .as_ref()
                    .and_then(|cfg| cfg.config.readiness_smoke)
            })
            .unwrap_or(true)
    }

    fn spawn_readiness_checks<I>(&mut self, supervisor: &Supervisor, aliases: I)
    where
        I: IntoIterator<Item = String>,
    {
        let smoke_enabled = self.configured_readiness_smoke();
        let status_options =
            ReadinessOptions::new(READINESS_TIMEOUT).with_poll_interval(READINESS_POLL_INTERVAL);
        let per_attempt_ms = (SMOKE_TIMEOUT.as_millis() / SMOKE_MAX_ATTEMPTS as u128).max(1);
        let smoke_commit_timeout = Duration::from_millis(per_attempt_ms as u64);
        let smoke_backoff = Duration::from_millis(150);
        for alias in aliases {
            if !self.readiness_inflight.insert(alias.clone()) {
                continue;
            }
            self.readiness_results
                .entry(alias.clone())
                .and_modify(|entry| {
                    entry.last_record = None;
                })
                .or_default();
            let Some(client) = supervisor.torii_client(&alias) else {
                let error = ToriiErrorInfo::new(
                    ToriiErrorKind::HttpTransport,
                    format!("Torii client unavailable for readiness probe on `{alias}`"),
                );
                let record = ReadinessRecord::failed(error, 0, Duration::ZERO, Instant::now());
                let _ = self.readiness_tx.send(ReadinessUpdate::Finished {
                    alias: alias.clone(),
                    record,
                });
                continue;
            };
            let tx = self.readiness_tx.clone();
            let alias_clone = alias.clone();
            let offset = supervisor
                .peers()
                .iter()
                .position(|peer| peer.alias() == alias)
                .unwrap_or(0);
            let attempts = SMOKE_MAX_ATTEMPTS.max(1);
            let nonce_offset = offset.saturating_mul(attempts);
            if !smoke_enabled {
                self.runtime.spawn(async move {
                    let started = Instant::now();
                    let record = match client.wait_for_ready(status_options).await {
                        Ok(snapshot) => {
                            ReadinessRecord::ready(snapshot, started.elapsed(), Instant::now())
                        }
                        Err(err) => ReadinessRecord::failed(
                            err.summarize(),
                            1,
                            started.elapsed(),
                            Instant::now(),
                        ),
                    };
                    let _ = tx.send(ReadinessUpdate::Finished {
                        alias: alias_clone,
                        record,
                    });
                });
                continue;
            }
            let mut plan = match supervisor.readiness_smoke_plan_with_offset(attempts, nonce_offset)
            {
                Ok(plan) => plan,
                Err(err) => {
                    let error = ToriiErrorInfo::with_detail(
                        ToriiErrorKind::HttpTransport,
                        "Failed to build readiness smoke plan",
                        format!("{err}"),
                    );
                    let record = ReadinessRecord::failed(error, 0, Duration::ZERO, Instant::now());
                    let _ = self.readiness_tx.send(ReadinessUpdate::Finished {
                        alias: alias.clone(),
                        record,
                    });
                    continue;
                }
            };
            plan.status_options = status_options;
            plan.commit_options = SmokeCommitOptions::new(smoke_commit_timeout);
            plan.backoff = smoke_backoff;
            let attempts = plan.transactions.len().max(1);
            self.runtime.spawn(async move {
                let started = Instant::now();
                let record = match client.wait_for_readiness_smoke(plan).await {
                    Ok(outcome) => ReadinessRecord::committed(outcome, Instant::now()),
                    Err(err) => ReadinessRecord::failed(
                        err.summarize(),
                        attempts,
                        started.elapsed(),
                        Instant::now(),
                    ),
                };
                let _ = tx.send(ReadinessUpdate::Finished {
                    alias: alias_clone,
                    record,
                });
            });
        }
    }

    fn poll_status_streams(&mut self) {
        let mut finished = Vec::new();
        let mut pending = Vec::new();
        for (alias, handle) in self.status_streams.iter_mut() {
            loop {
                match handle.receiver.try_recv() {
                    Ok(StatusStreamEvent::Snapshot {
                        snapshot,
                        sumeragi,
                        metrics,
                        metrics_error,
                    }) => {
                        pending.push((
                            alias.clone(),
                            StatusStreamEvent::Snapshot {
                                snapshot,
                                sumeragi,
                                metrics,
                                metrics_error,
                            },
                        ));
                    }
                    Ok(StatusStreamEvent::Error {
                        error,
                        consecutive_failures,
                    }) => {
                        pending.push((
                            alias.clone(),
                            StatusStreamEvent::Error {
                                error,
                                consecutive_failures,
                            },
                        ));
                    }
                    Ok(StatusStreamEvent::Closed) => {
                        finished.push(alias.clone());
                        break;
                    }
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Lagged(skipped)) => {
                        let error = ToriiErrorInfo::with_detail(
                            ToriiErrorKind::HttpTransport,
                            "Status stream lagged",
                            format!("{skipped} events skipped"),
                        );
                        pending.push((
                            alias.clone(),
                            StatusStreamEvent::Error {
                                error,
                                consecutive_failures: 0,
                            },
                        ));
                    }
                    Err(TryRecvError::Closed) => {
                        finished.push(alias.clone());
                        break;
                    }
                }
            }
        }
        for (alias, event) in pending {
            match event {
                StatusStreamEvent::Snapshot {
                    snapshot,
                    sumeragi,
                    metrics,
                    metrics_error,
                } => {
                    self.handle_status_snapshot_event(
                        &alias,
                        snapshot,
                        sumeragi,
                        metrics,
                        metrics_error,
                    );
                }
                StatusStreamEvent::Error { error, .. } => {
                    self.handle_status_error(&alias, error);
                }
                StatusStreamEvent::Closed => {}
            }
        }
        for alias in finished {
            if let Some(mut handle) = self.status_streams.remove(&alias) {
                handle.abort();
            }
        }
    }

    fn poll_readiness_updates(&mut self) {
        let mut updates = Vec::new();
        while let Ok(update) = self.readiness_rx.try_recv() {
            updates.push(update);
        }

        for update in updates {
            let ReadinessUpdate::Finished { alias, record } = update;
            let entry = self.readiness_results.entry(alias.clone()).or_default();
            entry.update(record.clone());
            self.readiness_inflight.remove(&alias);

            match &record.outcome {
                ReadinessRecordOutcome::Ready { status } => {
                    self.last_info = Some(format!(
                        "{alias} ready in {:.1}s (queue={}).",
                        record.total_elapsed.as_secs_f32(),
                        status.status.queue_size
                    ));
                    self.last_error = None;
                }
                ReadinessRecordOutcome::Committed(outcome) => {
                    let queue_depth = outcome
                        .status
                        .as_ref()
                        .map(|snapshot| snapshot.status.queue_size)
                        .unwrap_or(0);
                    self.last_info = Some(format!(
                        "{alias} ready in {:.1}s (queue={queue_depth}, attempt={}).",
                        record.total_elapsed.as_secs_f32(),
                        outcome.attempt
                    ));
                    self.last_error = None;
                }
                ReadinessRecordOutcome::Failed { error } => {
                    self.last_error = Some(format!("{alias} readiness failed: {}", error.message));
                    self.last_info = None;
                }
            }
        }
    }

    fn handle_status_snapshot_event(
        &mut self,
        alias: &str,
        snapshot: Arc<ToriiStatusSnapshot>,
        sumeragi: Option<Arc<SumeragiStatusWire>>,
        metrics: Option<Arc<ToriiMetricsSnapshot>>,
        metrics_error: Option<ToriiErrorInfo>,
    ) {
        let timestamp = snapshot.timestamp;
        let snapshot_value = (*snapshot).clone();
        let history_snapshot = snapshot_value.clone();
        let sumeragi_value = sumeragi.map(|value| (*value).clone());
        let metrics_value = metrics.as_ref().map(|value| value.as_ref().clone());

        let view = self.status_snapshots.entry(alias.to_owned()).or_default();
        view.record_snapshot(
            snapshot_value,
            sumeragi_value,
            metrics_value.clone(),
            metrics_error,
            timestamp,
        );

        let history = self.status_history.entry(alias.to_owned()).or_default();
        history.push_back(StatusHistoryEntry {
            timestamp,
            snapshot: history_snapshot,
            metrics: metrics_value,
        });
        if history.len() > STATUS_HISTORY_LEN {
            history.pop_front();
        }
    }

    fn handle_status_error(&mut self, alias: &str, error: ToriiErrorInfo) {
        let view = self.status_snapshots.entry(alias.to_owned()).or_default();
        view.record_error(error, Instant::now());
    }

    fn record_stream_event(&mut self, alias: Option<String>, event: &BlockStreamEvent) {
        let Some(alias) = alias else {
            return;
        };
        let snapshot = self.block_snapshots.entry(alias.clone()).or_default();
        snapshot.last_update = Some(Instant::now());
        match event {
            BlockStreamEvent::Block { summary, .. } => {
                snapshot.last_summary = Some(summary.clone());
                snapshot.last_error = None;
                snapshot.connected = true;
            }
            BlockStreamEvent::DecodeError { error } => {
                snapshot.last_error = Some(error.message.clone());
                snapshot.connected = false;
            }
            BlockStreamEvent::Lagged { skipped } => {
                snapshot.last_error =
                    Some(format!("Receiver lagged and skipped {skipped} frame(s)."));
            }
            BlockStreamEvent::Text { text } => {
                let lowered = text.to_ascii_lowercase();
                if lowered.contains("reconnecting") {
                    snapshot.connected = false;
                }
                if lowered.contains("reconnected") || lowered.contains("started") {
                    snapshot.connected = true;
                }
                if lowered.contains("stopped") {
                    snapshot.connected = false;
                }
                for line in text.lines().map(str::trim).filter(|line| !line.is_empty()) {
                    let event = Self::system_log_event(&alias, line.to_owned());
                    self.log_events.push(event);
                }
            }
            BlockStreamEvent::Closed => {
                snapshot.connected = false;
            }
        }
    }

    fn record_event_stream_event(&mut self, alias: Option<String>, event: &EventStreamEvent) {
        let Some(alias) = alias else {
            return;
        };
        let snapshot = self.event_snapshots.entry(alias.clone()).or_default();
        snapshot.last_update = Some(Instant::now());
        match event {
            EventStreamEvent::Event { summary, .. } => {
                snapshot.last_summary = Some(summary.clone());
                snapshot.last_error = None;
                snapshot.connected = true;
            }
            EventStreamEvent::DecodeError { error } => {
                snapshot.last_error = Some(error.message.clone());
                snapshot.connected = false;
            }
            EventStreamEvent::Lagged { skipped } => {
                snapshot.last_error = Some(format!(
                    "Receiver lagged and skipped {skipped} event frame(s)."
                ));
            }
            EventStreamEvent::Text { text } => {
                let lowered = text.to_ascii_lowercase();
                if lowered.contains("reconnecting") {
                    snapshot.connected = false;
                }
                if lowered.contains("reconnected") || lowered.contains("started") {
                    snapshot.connected = true;
                }
                if lowered.contains("stopped") {
                    snapshot.connected = false;
                }
            }
            EventStreamEvent::Closed => {
                snapshot.connected = false;
            }
        }
    }

    fn record_log_snapshot(&mut self, event: &PeerLogEvent) {
        let alias = Self::event_alias(event).to_owned();
        let label = match event {
            PeerLogEvent::Line { message, .. } => truncate(message, 80),
            PeerLogEvent::Lifecycle { event, .. } => Self::format_lifecycle_event(event),
        };
        let color = Self::log_event_color(event);
        let timestamp = match event {
            PeerLogEvent::Line { timestamp_ms, .. }
            | PeerLogEvent::Lifecycle { timestamp_ms, .. } => Some(*timestamp_ms),
        };
        let snapshot = self.log_snapshots.entry(alias).or_default();
        snapshot.label = label;
        snapshot.color = color;
        snapshot.timestamp = timestamp;
    }

    fn push_block_event(
        events: &mut Vec<DisplayEvent>,
        alias: Option<String>,
        event: BlockStreamEvent,
    ) {
        events.push(DisplayEvent { alias, event });
        if events.len() > MAX_BLOCK_EVENTS {
            events.remove(0);
        }
    }

    fn push_event_event(
        events: &mut Vec<EventDisplay>,
        alias: Option<String>,
        event: EventStreamEvent,
    ) {
        events.push(EventDisplay { alias, event });
        if events.len() > MAX_EVENT_EVENTS {
            events.remove(0);
        }
    }

    fn push_log_event(&mut self, event: PeerLogEvent) {
        self.record_log_snapshot(&event);
        if self.log_events.len() >= MAX_LOG_EVENTS {
            self.log_events.remove(0);
        }
        self.log_events.push(event);
    }

    fn stop_log_stream(&mut self, alias: Option<String>) {
        if let Some(alias) = alias {
            let stopped =
                Self::system_log_event(&alias, format!("Log stream stopped for {alias}."));
            self.push_log_event(stopped);
        }
        self.log_receiver = None;
        self.log_stream_peer = None;
    }

    fn log_event_kind(event: &PeerLogEvent) -> LogStreamKind {
        match event {
            PeerLogEvent::Line { kind, .. } => *kind,
            PeerLogEvent::Lifecycle { .. } => LogStreamKind::System,
        }
    }

    fn is_log_kind_enabled(&self, kind: LogStreamKind) -> bool {
        match kind {
            LogStreamKind::Stdout => self.settings_log_stdout,
            LogStreamKind::Stderr => self.settings_log_stderr,
            LogStreamKind::System => self.settings_log_system,
        }
    }

    fn format_block_event(entry: &DisplayEvent) -> String {
        let alias = entry.alias.as_deref().unwrap_or("unnamed");
        match &entry.event {
            BlockStreamEvent::Block {
                summary, raw_len, ..
            } => format!(
                "[{alias}] Block #{} | hash {} | tx {} (rejected {}) | triggers {} | sigs {} | view {} | raw {} bytes{}",
                summary.height,
                summary.hash_hex,
                summary.transaction_count,
                summary.rejected_transaction_count,
                summary.time_trigger_count,
                summary.signature_count,
                summary.view_change_index,
                raw_len,
                if summary.is_genesis { " | genesis" } else { "" }
            ),
            BlockStreamEvent::Text { text } => format!("[{alias}] {text}"),
            BlockStreamEvent::DecodeError { error } => format!(
                "[{alias}] Decode error at stage {} ({} bytes): {}",
                Self::format_decode_stage(error.stage),
                error.raw_len,
                error.message
            ),
            BlockStreamEvent::Lagged { skipped } => {
                format!("[{alias}] Warning: receiver lagged and skipped {skipped} block frame(s).")
            }
            BlockStreamEvent::Closed => {
                format!("[{alias}] Block stream closed; awaiting reconnection.")
            }
        }
    }

    fn render_event_line(entry: &EventDisplay) -> RenderedEventLine {
        let alias = entry.alias.as_deref().unwrap_or("unnamed");
        match &entry.event {
            EventStreamEvent::Event {
                summary,
                event,
                raw_len,
            } => {
                let base_kind = RenderedEventKind::Category(summary.category);
                match event.as_ref() {
                    EventBox::Pipeline(pipeline) => {
                        Self::render_pipeline_event(alias, base_kind, pipeline, *raw_len)
                    }
                    EventBox::Data(_) => {
                        let detail_text = summary
                            .detail
                            .clone()
                            .unwrap_or_else(|| "state change".to_owned());
                        let detail = Some(format!(
                            "{} • raw={}B",
                            truncate(&detail_text, 160),
                            raw_len
                        ));
                        RenderedEventLine::new(
                            alias,
                            format!("[{alias}] Data event — {}", truncate(&summary.label, 64)),
                            detail,
                            Color32::from_rgb(150, 200, 240),
                            base_kind,
                        )
                    }
                    EventBox::Time(time) => {
                        let interval = time.interval();
                        let detail = Some(format!(
                            "since={}ms • length={}ms • raw={}B",
                            interval.since().as_millis(),
                            interval.length().as_millis(),
                            raw_len
                        ));
                        RenderedEventLine::new(
                            alias,
                            format!("[{alias}] Time trigger interval"),
                            detail,
                            Color32::from_rgb(190, 170, 255),
                            base_kind,
                        )
                    }
                    EventBox::ExecuteTrigger(exec) => {
                        let trigger = exec.trigger_id().to_string();
                        let authority = exec.authority().to_string();
                        let args_preview = json::to_value(exec.args())
                            .ok()
                            .and_then(|value| json::to_string(&value).ok())
                            .map(|text| truncate(&text, 120));
                        let mut detail = format!(
                            "trigger={} • authority={} • raw={}B",
                            truncate(&trigger, 96),
                            truncate(&authority, 96),
                            raw_len
                        );
                        if let Some(args) = args_preview {
                            detail.push_str(" • args=");
                            detail.push_str(&args);
                        }
                        let badges = vec![
                            EventBadge::new(
                                "trigger",
                                trigger.clone(),
                                Some(truncate(&trigger, 96)),
                                Color32::from_rgb(170, 210, 255),
                            ),
                            EventBadge::new(
                                "authority",
                                authority.clone(),
                                Some(truncate(&authority, 96)),
                                Color32::from_rgb(160, 220, 180),
                            ),
                        ];
                        RenderedEventLine::new(
                            alias,
                            format!("[{alias}] Trigger execution requested"),
                            Some(detail),
                            Color32::from_rgb(140, 200, 255),
                            base_kind,
                        )
                        .with_badges(badges)
                    }
                    EventBox::TriggerCompleted(completed) => {
                        Self::render_trigger_completed(alias, completed, *raw_len, base_kind)
                    }
                }
            }
            EventStreamEvent::Text { text } => RenderedEventLine::new(
                alias,
                format!("[{alias}] {text}"),
                None,
                Color32::from_gray(190),
                RenderedEventKind::Text,
            ),
            EventStreamEvent::DecodeError { error } => {
                let summary = format!(
                    "[{alias}] Decode error at stage {}",
                    event_decode_stage_label(error.stage)
                );
                let detail = Some(format!(
                    "raw={}B • {}",
                    error.raw_len,
                    truncate(&error.message, 200)
                ));
                RenderedEventLine::new(
                    alias,
                    summary,
                    detail,
                    Color32::from_rgb(220, 90, 90),
                    RenderedEventKind::DecodeError,
                )
            }
            EventStreamEvent::Lagged { skipped } => RenderedEventLine::new(
                alias,
                format!("[{alias}] Warning: receiver lagged and skipped {skipped} event frame(s)."),
                None,
                Color32::from_rgb(230, 180, 60),
                RenderedEventKind::Lagged,
            ),
            EventStreamEvent::Closed => RenderedEventLine::new(
                alias,
                format!("[{alias}] Event stream closed; awaiting reconnection."),
                None,
                Color32::from_gray(180),
                RenderedEventKind::Closed,
            ),
        }
    }

    fn render_pipeline_event(
        alias: &str,
        base_kind: RenderedEventKind,
        pipeline: &PipelineEventBox,
        raw_len: usize,
    ) -> RenderedEventLine {
        match pipeline {
            PipelineEventBox::Transaction(tx) => {
                let hash_hex = encode_upper(tx.hash().as_ref());
                let height = tx.block_height().map(|h| h.get().to_string());
                let status = tx.status();
                let mut rejection_reason = None;
                let (label, color) = match status {
                    TransactionStatus::Queued => ("Queued", Color32::from_rgb(230, 190, 90)),
                    TransactionStatus::Expired => ("Expired", Color32::from_gray(160)),
                    TransactionStatus::Approved => ("Approved", Color32::from_rgb(120, 200, 140)),
                    TransactionStatus::Rejected(reason) => {
                        rejection_reason = Some(reason.to_string());
                        ("Rejected", Color32::from_rgb(225, 90, 90))
                    }
                };
                let mut detail = format!("hash={hash_hex} • raw={raw_len}B");
                if let Some(height) = &height {
                    detail.push_str(" • height=");
                    detail.push_str(height);
                }
                if let Some(reason) = rejection_reason.as_ref() {
                    detail.push_str(" • reason=");
                    detail.push_str(&truncate(reason, 160));
                }
                let mut badges = vec![EventBadge::new(
                    "hash",
                    hash_hex.clone(),
                    Some(truncate(&hash_hex, 24)),
                    Color32::from_rgb(160, 210, 255),
                )];
                if let Some(height) = &height {
                    badges.push(EventBadge::new(
                        "height",
                        height.clone(),
                        None,
                        Color32::from_rgb(170, 220, 170),
                    ));
                }
                if let Some(reason) = rejection_reason {
                    badges.push(EventBadge::new(
                        "reason",
                        reason.clone(),
                        Some(truncate(&reason, 160)),
                        Color32::from_rgb(255, 140, 140),
                    ));
                }
                RenderedEventLine::new(
                    alias,
                    format!(
                        "[{alias}] Transaction {label} — {}",
                        truncate(&hash_hex, 12)
                    ),
                    Some(detail),
                    color,
                    base_kind,
                )
                .with_badges(badges)
            }
            PipelineEventBox::Block(block) => {
                let header = block.header();
                let hash_hex = encode_upper(header.hash().as_ref());
                let status = block.status();
                let mut rejection_reason = None;
                let (status_label, color) = match status {
                    BlockStatus::Created => ("Created", Color32::from_rgb(110, 160, 220)),
                    BlockStatus::Approved => ("Approved", Color32::from_rgb(110, 200, 220)),
                    BlockStatus::Rejected(reason) => {
                        rejection_reason = Some(reason.to_string());
                        ("Rejected", Color32::from_rgb(225, 90, 90))
                    }
                    BlockStatus::Committed => ("Committed", Color32::from_rgb(120, 200, 140)),
                    BlockStatus::Applied => ("Applied", Color32::from_rgb(120, 200, 140)),
                };
                let mut detail = format!(
                    "height={} • view={} • hash={} • raw={}B",
                    header.height().get(),
                    header.view_change_index(),
                    hash_hex,
                    raw_len
                );
                if let Some(reason) = rejection_reason.as_ref() {
                    detail.push_str(" • reason=");
                    detail.push_str(&truncate(reason, 160));
                }
                let mut badges = vec![
                    EventBadge::new(
                        "hash",
                        hash_hex.clone(),
                        Some(truncate(&hash_hex, 24)),
                        Color32::from_rgb(150, 210, 255),
                    ),
                    EventBadge::new(
                        "height",
                        header.height().get().to_string(),
                        None,
                        Color32::from_rgb(170, 220, 170),
                    ),
                ];
                if let Some(reason) = rejection_reason {
                    badges.push(EventBadge::new(
                        "reason",
                        reason.clone(),
                        Some(truncate(&reason, 160)),
                        Color32::from_rgb(255, 140, 140),
                    ));
                }
                RenderedEventLine::new(
                    alias,
                    format!(
                        "[{alias}] Block {status_label} — height {}",
                        header.height().get()
                    ),
                    Some(detail),
                    color,
                    base_kind,
                )
                .with_badges(badges)
            }
            PipelineEventBox::Warning(warning) => {
                let payload = truncate(&format!("{warning:?}"), 160);
                let detail = Some(format!("{payload} • raw={raw_len}B"));
                RenderedEventLine::new(
                    alias,
                    format!("[{alias}] Pipeline warning"),
                    detail,
                    Color32::from_rgb(230, 190, 90),
                    base_kind,
                )
            }
            PipelineEventBox::Merge(merge) => {
                let payload = truncate(&format!("{merge:?}"), 160);
                let detail = Some(format!("{payload} • raw={raw_len}B"));
                RenderedEventLine::new(
                    alias,
                    format!("[{alias}] Merge ledger entry committed"),
                    detail,
                    Color32::from_rgb(170, 150, 240),
                    base_kind,
                )
            }
            PipelineEventBox::Witness(witness) => {
                let detail = Some(format!(
                    "height={} • view={} • epoch={} • raw={}B",
                    witness.height, witness.view, witness.epoch, raw_len
                ));
                RenderedEventLine::new(
                    alias,
                    format!(
                        "[{alias}] Execution witness received — height {}",
                        witness.height
                    ),
                    detail,
                    Color32::from_rgb(120, 200, 220),
                    base_kind,
                )
            }
        }
    }

    fn render_trigger_completed(
        alias: &str,
        completed: &TriggerCompletedEvent,
        raw_len: usize,
        base_kind: RenderedEventKind,
    ) -> RenderedEventLine {
        let outcome = completed.outcome();
        let (outcome_label, color, extra) = match outcome {
            TriggerCompletedOutcome::Success => ("Success", Color32::from_rgb(120, 200, 140), None),
            TriggerCompletedOutcome::Failure(reason) => (
                "Failure",
                Color32::from_rgb(225, 90, 90),
                Some(truncate(reason, 160)),
            ),
        };
        let entry_hash = encode_upper(completed.entrypoint_hash().as_ref());
        let trigger_id = completed.trigger_id().to_string();
        let mut detail = format!(
            "trigger={} • step={} • entrypoint={} • raw={}B",
            truncate(&trigger_id, 96),
            completed.step_index(),
            truncate(&entry_hash, 64),
            raw_len
        );
        if let Some(reason) = extra.as_ref() {
            detail.push_str(" • reason=");
            detail.push_str(reason);
        }
        let mut badges = vec![
            EventBadge::new(
                "trigger",
                trigger_id.clone(),
                Some(truncate(&trigger_id, 96)),
                Color32::from_rgb(170, 210, 255),
            ),
            EventBadge::new(
                "entry",
                entry_hash.clone(),
                Some(truncate(&entry_hash, 24)),
                Color32::from_rgb(180, 180, 255),
            ),
        ];
        if let Some(reason) = extra.clone() {
            badges.push(EventBadge::new(
                "reason",
                reason.clone(),
                Some(reason),
                Color32::from_rgb(255, 140, 140),
            ));
        }
        RenderedEventLine::new(
            alias,
            format!(
                "[{alias}] Trigger execution {outcome_label} — {}",
                completed.trigger_id()
            ),
            Some(detail),
            color,
            base_kind,
        )
        .with_badges(badges)
    }

    fn render_event_entry(
        &mut self,
        ui: &mut egui::Ui,
        entry: &EventDisplay,
        line: &RenderedEventLine,
        copy_label: &str,
    ) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            if ui
                .small_button("Copy")
                .on_hover_text("Copy event summary to clipboard")
                .clicked()
            {
                let mut text = line.summary.clone();
                if let Some(detail) = &line.detail {
                    text.push_str(" — ");
                    text.push_str(detail);
                }
                Self::copy_text(ui, text);
                self.last_info = Some(copy_label.to_owned());
            }
            if matches!(entry.event, EventStreamEvent::Event { .. })
                && ui
                    .small_button("JSON")
                    .on_hover_text("Copy structured event as JSON")
                    .clicked()
            {
                match event_json_string(&entry.event) {
                    Ok(json) => {
                        Self::copy_text(ui, json);
                        self.last_info = Some("Event JSON copied to clipboard.".to_owned());
                    }
                    Err(err) => {
                        self.last_error = Some(err);
                    }
                }
            }
            ui.vertical(|ui| {
                let summary_text = RichText::new(&line.summary)
                    .monospace()
                    .color(line.summary_color);
                ui.label(summary_text);
                if let Some(detail) = &line.detail {
                    ui.label(RichText::new(detail).small().color(Color32::from_gray(200)));
                }
                if !line.badges.is_empty() {
                    ui.add_space(2.0);
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing.x = 6.0;
                        for badge in &line.badges {
                            let text = format!("{}: {}", badge.label, badge.value);
                            ui.label(RichText::new(text).small().monospace().color(badge.color));
                        }
                    });
                }
            });
        });
    }

    fn format_decode_stage(stage: BlockDecodeStage) -> &'static str {
        match stage {
            BlockDecodeStage::Frame => "frame",
            BlockDecodeStage::Block => "payload",
            BlockDecodeStage::Stream => "stream",
        }
    }

    fn system_log_event(alias: &str, message: String) -> PeerLogEvent {
        PeerLogEvent::Line {
            alias: Arc::from(alias.to_owned()),
            kind: LogStreamKind::System,
            timestamp_ms: now_ms(),
            message,
        }
    }

    fn format_log_event(event: &PeerLogEvent) -> String {
        match event {
            PeerLogEvent::Line {
                alias,
                kind,
                timestamp_ms,
                message,
            } => format!(
                "[{}][{}][{}] {}",
                alias,
                Self::format_timestamp(*timestamp_ms),
                kind,
                message
            ),
            PeerLogEvent::Lifecycle {
                alias,
                event,
                timestamp_ms,
            } => format!(
                "[{}][{}][lifecycle] {}",
                alias,
                Self::format_timestamp(*timestamp_ms),
                Self::format_lifecycle_event(event)
            ),
        }
    }

    fn format_lifecycle_event(event: &LifecycleEvent) -> String {
        match event {
            LifecycleEvent::Started { attempt } if *attempt == 0 => "started".to_owned(),
            LifecycleEvent::Started { attempt } => {
                format!("restart started (attempt={attempt})")
            }
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

    fn log_event_color(event: &PeerLogEvent) -> Color32 {
        match event {
            PeerLogEvent::Line { kind, .. } => match kind {
                LogStreamKind::Stdout => Color32::from_gray(200),
                LogStreamKind::Stderr => Color32::from_rgb(200, 64, 64),
                LogStreamKind::System => Color32::from_gray(150),
            },
            PeerLogEvent::Lifecycle { .. } => Color32::from_rgb(64, 144, 200),
        }
    }

    fn event_alias(event: &PeerLogEvent) -> &str {
        match event {
            PeerLogEvent::Line { alias, .. } => alias.as_ref(),
            PeerLogEvent::Lifecycle { alias, .. } => alias.as_ref(),
        }
    }

    fn format_timestamp(timestamp_ms: u128) -> String {
        let seconds = timestamp_ms / 1000;
        let millis = (timestamp_ms % 1000) as u32;
        format!("{seconds}.{millis:03}s")
    }

    fn render_ready(&mut self, ui: &mut egui::Ui, supervisor: &mut Supervisor) {
        supervisor.refresh_peer_states();

        self.poll_block_stream_events();
        self.poll_event_stream_events();
        self.poll_log_events();
        self.poll_state_updates();
        self.poll_composer_updates();
        self.poll_readiness_updates();

        let peer_aliases: Vec<String> = supervisor
            .peers()
            .iter()
            .map(|peer| peer.alias().to_owned())
            .collect();

        self.sync_status_streams(supervisor, &peer_aliases);
        self.poll_status_streams();

        Self::ensure_selection(&mut self.selected_peer, &peer_aliases);
        Self::ensure_selection(&mut self.event_selected_peer, &peer_aliases);
        Self::ensure_selection(&mut self.log_selected_peer, &peer_aliases);
        Self::ensure_selection(&mut self.state_selected_peer, &peer_aliases);
        Self::ensure_selection(&mut self.composer_selected_peer, &peer_aliases);
        Self::ensure_signer_selection(&mut self.composer_selected_signer, supervisor.signers());

        if let Some(error) = &self.last_error {
            ui.colored_label(Color32::from_rgb(200, 64, 64), error);
        }
        if let Some(info) = &self.last_info {
            ui.colored_label(Color32::from_rgb(80, 160, 80), info);
        }
        if let Some(banner) = self.maintenance_state.banner() {
            let MaintenanceBanner {
                color,
                text,
                show_spinner,
                dismissable,
            } = banner;
            ui.horizontal(|ui| {
                ui.colored_label(color, text.as_str());
                if show_spinner {
                    ui.add(egui::Spinner::new());
                }
                if dismissable
                    && ui
                        .small_button("Dismiss")
                        .on_hover_text("Hide maintenance status banner")
                        .clicked()
                {
                    self.maintenance_state = MaintenanceState::Idle;
                }
            });
        }

        ui.add_space(6.0);
        self.render_view_tabs(ui);
        ui.add_space(12.0);

        let peer_rows = self.build_peer_rows(supervisor);
        let metrics = self.collect_dashboard_metrics(&peer_rows);

        self.render_overview_bar(ui, supervisor, &peer_rows, &metrics);

        ui.add_space(16.0);

        match self.active_view {
            ActiveView::Dashboard => {
                self.render_dashboard_view(ui, supervisor, &peer_rows, &peer_aliases, &metrics);
            }
            ActiveView::Blocks => self.render_blocks_view(ui, supervisor, &peer_aliases),
            ActiveView::Events => self.render_events_view(ui, supervisor, &peer_aliases),
            ActiveView::Logs => self.render_logs_view(ui, supervisor, &peer_aliases),
            ActiveView::State => self.render_state_view(ui, supervisor, &peer_aliases),
            ActiveView::Composer => self.render_composer_view(ui, supervisor, &peer_aliases),
        }
    }

    fn ensure_selection(selection: &mut Option<String>, peer_aliases: &[String]) {
        if let Some(current) = selection.as_ref()
            && !peer_aliases.iter().any(|alias| alias == current)
        {
            *selection = None;
        }
        if selection.is_none() {
            *selection = peer_aliases.first().cloned();
        }
    }

    fn ensure_signer_selection(selection: &mut Option<usize>, signers: &[SigningAuthority]) {
        if signers.is_empty() {
            *selection = None;
            return;
        }
        let max_index = signers.len() - 1;
        match selection {
            Some(index) if *index <= max_index => {}
            _ => *selection = Some(0),
        }
    }

    fn begin_maintenance(&mut self, task: MaintenanceTask) -> bool {
        if self.maintenance_state.is_running() {
            return false;
        }
        self.maintenance_state = MaintenanceState::Running(task);
        true
    }

    fn complete_maintenance(&mut self, message: String) {
        self.maintenance_state = MaintenanceState::Completed { message };
    }

    fn fail_maintenance(&mut self, error: String) {
        self.maintenance_state = MaintenanceState::Failed { error };
    }

    fn sync_log_export_dir_input(&mut self) {
        self.settings_log_export_dir_input = self
            .log_export_dir
            .as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_default();
    }

    fn sync_state_export_dir_input(&mut self) {
        self.settings_state_export_dir_input = self
            .state_export_dir
            .as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_default();
    }

    fn sync_raw_editor_from_drafts(&mut self) {
        match drafts_to_pretty_json(&self.composer_drafts) {
            Ok(text) => {
                self.composer_raw_editor = if text.trim().is_empty() {
                    "[]\n".to_owned()
                } else {
                    text
                };
                self.composer_raw_dirty = false;
                self.composer_raw_error = None;
            }
            Err(err) => {
                self.composer_raw_error = Some(err.to_string());
                self.composer_raw_dirty = false;
            }
        }
    }

    fn apply_raw_editor_to_drafts(&mut self) {
        let source = if self.composer_raw_editor.trim().is_empty() {
            "[]"
        } else {
            self.composer_raw_editor.as_str()
        };
        match drafts_from_json_str(source) {
            Ok(drafts) => {
                self.composer_drafts = drafts;
                self.composer_preview = None;
                self.composer_error = None;
                self.composer_submit_error = None;
                self.composer_submit_success = None;
                self.composer_raw_error = None;
                self.composer_raw_dirty = false;
                self.sync_raw_editor_from_drafts();
            }
            Err(err) => {
                self.composer_raw_error = Some(err.to_string());
            }
        }
    }

    fn reset_runtime_state_after_maintenance(&mut self) {
        if let Some(stream) = self.block_stream.take() {
            stream.abort();
        }
        if let Some(stream) = self.event_stream.take() {
            stream.abort();
        }
        self.block_receiver = None;
        self.event_receiver = None;
        if self.log_receiver.is_some() {
            self.stop_log_stream(self.log_stream_peer.clone());
        }
        self.block_events.clear();
        self.event_events.clear();
        self.log_events.clear();
        self.block_snapshots.clear();
        self.event_snapshots.clear();
        self.log_snapshots.clear();
        self.status_snapshots.clear();
        self.status_history.clear();
        for handle in self.status_streams.values_mut() {
            handle.abort();
        }
        self.status_streams.clear();
        self.readiness_inflight.clear();
        self.readiness_results.clear();
        self.block_stream_peer = None;
        self.event_stream_peer = None;
        self.selected_peer = None;
        self.event_selected_peer = None;
        self.log_selected_peer = None;
        self.state_selected_peer = None;
        self.state_tabs = StateTabs::default();
        self.composer_preview = None;
        self.composer_submit_error = None;
        self.composer_submit_success = None;
        self.sync_raw_editor_from_drafts();
    }

    fn initialize_settings_from_supervisor(&mut self) {
        let nexus_table = self
            .supervisor
            .as_ref()
            .and_then(|supervisor| supervisor.nexus_config_overrides())
            .or_else(|| {
                self.bundle_config
                    .as_ref()
                    .and_then(|cfg| cfg.config.nexus.as_ref())
            });
        let sumeragi_table = self
            .supervisor
            .as_ref()
            .and_then(|supervisor| supervisor.sumeragi_config_overrides())
            .or_else(|| {
                self.bundle_config
                    .as_ref()
                    .and_then(|cfg| cfg.config.sumeragi.as_ref())
            });
        let torii_table = self
            .supervisor
            .as_ref()
            .and_then(|supervisor| supervisor.torii_config_overrides())
            .or_else(|| {
                self.bundle_config
                    .as_ref()
                    .and_then(|cfg| cfg.config.torii.as_ref())
            });

        if let Some(supervisor) = self.supervisor.as_ref() {
            let base_root = Self::supervisor_base_data_root(supervisor);
            self.settings_data_root_input = base_root.display().to_string();
            if let Some(port) = Self::infer_torii_base_port(supervisor) {
                self.settings_torii_port_input = port.to_string();
            } else {
                self.settings_torii_port_input.clear();
            }
            if let Some(port) = Self::infer_p2p_base_port(supervisor) {
                self.settings_p2p_port_input = port.to_string();
            } else {
                self.settings_p2p_port_input.clear();
            }
            self.settings_chain_id_input = supervisor.chain_id().to_owned();
            self.settings_profile_input.clear();
            self.settings_build_binaries = self.configured_build_binaries();
            self.settings_readiness_smoke = self.configured_readiness_smoke();
        } else {
            self.settings_data_root_input.clear();
            self.settings_torii_port_input.clear();
            self.settings_p2p_port_input.clear();
            self.settings_chain_id_input.clear();
            self.settings_profile_input.clear();
            self.settings_build_binaries = self.configured_build_binaries();
            self.settings_readiness_smoke = self.configured_readiness_smoke();
        }

        self.settings_nexus_enabled = nexus_table
            .and_then(|table| table.get("enabled").and_then(TomlValue::as_bool))
            .unwrap_or_else(|| nexus_table.is_some());
        self.settings_nexus_lane_count_input = nexus_table
            .and_then(|table| table.get("lane_count").and_then(TomlValue::as_integer))
            .and_then(|value| u32::try_from(value).ok())
            .map(|value| value.to_string())
            .unwrap_or_default();
        self.settings_nexus_lane_catalog_input =
            Self::format_toml_array_input(nexus_table, "lane_catalog");
        self.settings_nexus_dataspace_catalog_input =
            Self::format_toml_array_input(nexus_table, "dataspace_catalog");

        self.settings_sumeragi_da_enabled = sumeragi_table
            .and_then(|table| table.get("da_enabled").and_then(TomlValue::as_bool))
            .unwrap_or(self.settings_nexus_enabled);
        self.settings_torii_da_replay_dir_input = torii_table
            .and_then(|table| table.get("da_ingest").and_then(TomlValue::as_table))
            .and_then(|ingest| {
                ingest
                    .get("replay_cache_store_dir")
                    .and_then(TomlValue::as_str)
            })
            .unwrap_or_default()
            .to_string();
        self.settings_torii_da_manifest_dir_input = torii_table
            .and_then(|table| table.get("da_ingest").and_then(TomlValue::as_table))
            .and_then(|ingest| ingest.get("manifest_store_dir").and_then(TomlValue::as_str))
            .unwrap_or_default()
            .to_string();
        self.lane_reset_selection = None;

        self.sync_log_export_dir_input();
        self.sync_state_export_dir_input();
    }

    fn supervisor_base_data_root(supervisor: &Supervisor) -> PathBuf {
        let mut root = supervisor.paths().root().to_path_buf();
        let slug = supervisor.profile().slug();
        if root.ends_with(Path::new(&slug)) {
            root.pop();
        }
        root
    }

    fn infer_torii_base_port(supervisor: &Supervisor) -> Option<u16> {
        supervisor.peers().first().and_then(|peer| {
            peer.torii_address()
                .rsplit(':')
                .next()
                .and_then(|port| port.parse::<u16>().ok())
        })
    }

    fn infer_p2p_base_port(supervisor: &Supervisor) -> Option<u16> {
        supervisor.peers().first().and_then(|peer| {
            peer.p2p_address()
                .rsplit(':')
                .next()
                .and_then(|port| port.parse::<u16>().ok())
        })
    }

    fn rebuild_supervisor(
        &mut self,
        data_root: Option<PathBuf>,
        torii_port: Option<u16>,
        p2p_port: Option<u16>,
        chain_id: Option<String>,
    ) -> Result<(), SupervisorError> {
        let profile = self
            .supervisor
            .as_ref()
            .map(|supervisor| supervisor.profile().clone())
            .unwrap_or_else(|| NetworkProfile::from_preset(ProfilePreset::SinglePeer));
        let mut builder = SupervisorBuilder::with_profile(profile);
        if let Some(cfg) = self.bundle_config.as_ref() {
            builder = cfg.config.apply_to(builder);
        }
        builder = self.cli_overrides.clone().apply_to(builder);
        if let Some(root) = data_root.clone() {
            builder = builder.data_root(root);
        }
        if let Some(port) = torii_port {
            builder = builder.torii_base_port(port);
        }
        if let Some(port) = p2p_port {
            builder = builder.p2p_base_port(port);
        }
        if let Some(chain) = chain_id {
            builder = builder.chain_id(chain);
        }
        if let Some(existing) = self.supervisor.as_ref() {
            builder = builder.binaries(existing.binaries().clone());
        }

        let mut previous = self.supervisor.take();
        let mut was_running = false;
        if let Some(old) = previous.as_mut() {
            was_running = old.is_any_running();
            let _ = old.stop_all();
        }
        self.reset_runtime_state_after_maintenance();

        match builder.build() {
            Ok(mut supervisor) => {
                if was_running && let Err(err) = supervisor.start_all() {
                    self.last_error = Some(format!("Failed to restart peers after rebuild: {err}"));
                }
                self.supervisor = Some(supervisor);
                self.supervisor_error = None;
                self.initialize_settings_from_supervisor();
                Ok(())
            }
            Err(err) => {
                if let Some(mut old) = previous {
                    if was_running {
                        let _ = old.start_all();
                    }
                    self.supervisor = Some(old);
                }
                self.initialize_settings_from_supervisor();
                Err(err)
            }
        }
    }

    fn apply_settings_changes(&mut self) -> Result<(), String> {
        let data_root_input = self.settings_data_root_input.trim();
        let data_root = if data_root_input.is_empty() {
            None
        } else {
            Some(PathBuf::from(data_root_input))
        };

        let torii_port = {
            let trimmed = self.settings_torii_port_input.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.parse::<u16>().map_err(|_| {
                    "Torii base port must be an integer between 1 and 65535".to_owned()
                })?)
            }
        };

        let p2p_port = {
            let trimmed = self.settings_p2p_port_input.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.parse::<u16>().map_err(|_| {
                    "P2P base port must be an integer between 1 and 65535".to_owned()
                })?)
            }
        };

        let chain_id = {
            let trimmed = self.settings_chain_id_input.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_owned())
            }
        };

        let log_export_dir = {
            let trimmed = self.settings_log_export_dir_input.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(PathBuf::from(trimmed))
            }
        };

        let state_export_dir = {
            let trimmed = self.settings_state_export_dir_input.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(PathBuf::from(trimmed))
            }
        };

        if self.settings_nexus_enabled && !self.settings_sumeragi_da_enabled {
            return Err(
                "Nexus lanes require data-availability gating (enable DA under Sumeragi)."
                    .to_owned(),
            );
        }

        let lane_count = Self::parse_lane_count_input(&self.settings_nexus_lane_count_input)?;
        let mut lane_catalog = Self::parse_toml_array_input(
            &self.settings_nexus_lane_catalog_input,
            "lane_catalog",
            "Lane catalog",
        )?;
        if matches!(lane_catalog.as_ref(), Some(values) if values.is_empty()) {
            lane_catalog = None;
        }
        let mut dataspace_catalog = Self::parse_toml_array_input(
            &self.settings_nexus_dataspace_catalog_input,
            "dataspace_catalog",
            "Dataspace catalog",
        )?;
        if matches!(dataspace_catalog.as_ref(), Some(values) if values.is_empty()) {
            dataspace_catalog = None;
        }

        let mut resolved = self
            .bundle_config
            .clone()
            .unwrap_or_else(|| ResolvedBundleConfig {
                config: BundleConfig::default(),
                path: self
                    .cli_overrides
                    .config_path
                    .clone()
                    .unwrap_or_else(default_config_path),
            });

        let profile_override_input = self.settings_profile_input.trim();
        let profile_override = if profile_override_input.is_empty() {
            None
        } else {
            let parsed = parse_profile_override(profile_override_input)
                .map_err(|err| format!("Profile override error: {err}"))?;
            Some(parsed)
        };

        resolved.config.set_data_root(data_root.clone());
        let current_profile = self
            .supervisor
            .as_ref()
            .map(|supervisor| supervisor.profile().clone())
            .unwrap_or_else(|| NetworkProfile::from_preset(ProfilePreset::SinglePeer));
        let effective_profile = if let Some(parsed) = profile_override.as_ref() {
            if let Some(preset) = parsed.profile.preset {
                let mut profile = NetworkProfile::from_preset(preset);
                profile.consensus_mode = current_profile.consensus_mode;
                profile
            } else {
                parsed.profile.clone()
            }
        } else {
            current_profile
        };
        resolved.config.set_profile(Some(effective_profile.clone()));
        if let Some(parsed) = profile_override.as_ref() {
            if let Some(genesis_profile) = parsed.genesis_profile {
                resolved.config.set_genesis_profile(Some(genesis_profile));
            } else if effective_profile.consensus_mode == SumeragiConsensusMode::Permissioned
                && resolved.config.genesis_profile.is_some()
            {
                return Err("Profile override uses permissioned consensus but genesis_profile is configured; clear genesis_profile in config/local.toml or switch to consensus_mode = \"npos\".".to_owned());
            }
        }
        resolved.config.set_torii_start(torii_port);
        resolved.config.set_p2p_start(p2p_port);
        resolved.config.set_chain_id(chain_id.clone());
        resolved
            .config
            .set_build_binaries(Some(self.settings_build_binaries));
        resolved
            .config
            .set_readiness_smoke(Some(self.settings_readiness_smoke));

        let has_lane_overrides = lane_count.is_some_and(|count| count > 1)
            || lane_catalog.is_some()
            || dataspace_catalog.is_some();
        if !self.settings_nexus_enabled && has_lane_overrides {
            return Err(
                "nexus.enabled = false requires lane_count = 1 and empty lane/dataspace catalogs"
                    .to_owned(),
            );
        }
        let mut nexus_table = resolved.config.nexus.clone().unwrap_or_default();
        if self.settings_nexus_enabled || has_lane_overrides || !nexus_table.is_empty() {
            if self.settings_nexus_enabled || has_lane_overrides {
                nexus_table.insert(
                    "enabled".into(),
                    TomlValue::Boolean(self.settings_nexus_enabled),
                );
            } else {
                nexus_table.remove("enabled");
            }
            match lane_count {
                Some(count) => {
                    nexus_table.insert("lane_count".into(), TomlValue::Integer(count.into()));
                }
                None => {
                    nexus_table.remove("lane_count");
                }
            }
            match lane_catalog {
                Some(values) => {
                    nexus_table.insert("lane_catalog".into(), TomlValue::Array(values));
                }
                None => {
                    nexus_table.remove("lane_catalog");
                }
            }
            match dataspace_catalog {
                Some(values) => {
                    nexus_table.insert("dataspace_catalog".into(), TomlValue::Array(values));
                }
                None => {
                    nexus_table.remove("dataspace_catalog");
                }
            }
            if nexus_table.is_empty() {
                resolved.config.nexus = None;
            } else {
                resolved.config.nexus = Some(nexus_table);
            }
        } else {
            resolved.config.nexus = None;
        }

        let mut sumeragi_table = resolved.config.sumeragi.clone().unwrap_or_default();
        sumeragi_table.insert(
            "da_enabled".into(),
            TomlValue::Boolean(self.settings_sumeragi_da_enabled),
        );
        resolved.config.sumeragi = Some(sumeragi_table);

        let torii_replay_dir = self.settings_torii_da_replay_dir_input.trim();
        let torii_manifest_dir = self.settings_torii_da_manifest_dir_input.trim();
        let mut torii_table = resolved.config.torii.clone().unwrap_or_default();
        let mut da_ingest_table = torii_table
            .get("da_ingest")
            .and_then(TomlValue::as_table)
            .cloned()
            .unwrap_or_default();
        if torii_replay_dir.is_empty() {
            da_ingest_table.remove("replay_cache_store_dir");
        } else {
            da_ingest_table.insert(
                "replay_cache_store_dir".into(),
                TomlValue::String(torii_replay_dir.to_owned()),
            );
        }
        if torii_manifest_dir.is_empty() {
            da_ingest_table.remove("manifest_store_dir");
        } else {
            da_ingest_table.insert(
                "manifest_store_dir".into(),
                TomlValue::String(torii_manifest_dir.to_owned()),
            );
        }
        if da_ingest_table.is_empty() {
            torii_table.remove("da_ingest");
        } else {
            torii_table.insert("da_ingest".into(), TomlValue::Table(da_ingest_table));
        }
        if torii_table.is_empty() {
            resolved.config.torii = None;
        } else {
            resolved.config.torii = Some(torii_table);
        }
        if let Err(err) = resolved.config.write_to_path(&resolved.path) {
            return Err(err.to_string());
        }
        self.bundle_config = Some(resolved);

        self.rebuild_supervisor(data_root, torii_port, p2p_port, chain_id)
            .map_err(|err| err.to_string())?;
        self.log_export_dir = log_export_dir;
        self.sync_log_export_dir_input();
        self.state_export_dir = state_export_dir;
        self.sync_state_export_dir_input();
        self.settings_dialog = false;
        Ok(())
    }

    fn mintable_label(mintable: Mintable) -> String {
        match mintable {
            Mintable::Infinitely => "Infinitely".to_owned(),
            Mintable::Once => "Once".to_owned(),
            Mintable::Not => "Not".to_owned(),
            Mintable::Limited(tokens) => format!("Limited({})", tokens.value()),
        }
    }

    fn preview_snapshot_label(raw: &str) -> Option<String> {
        let mut sanitized = String::with_capacity(raw.len());
        for ch in raw.chars() {
            match ch {
                'a'..='z' | '0'..='9' => sanitized.push(ch),
                'A'..='Z' => sanitized.push(ch.to_ascii_lowercase()),
                '-' | '_' => sanitized.push(ch),
                ' ' | '.' => sanitized.push('-'),
                _ => {}
            }
        }
        let trimmed = sanitized
            .trim_matches(|c: char| matches!(c, '-' | '_'))
            .to_owned();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    }

    fn format_toml_array_input(table: Option<&TomlTable>, key: &str) -> String {
        let Some(table) = table else {
            return String::new();
        };
        let Some(value) = table.get(key) else {
            return String::new();
        };
        let mut wrapper = TomlTable::new();
        wrapper.insert(key.to_owned(), value.clone());
        toml::to_string_pretty(&TomlValue::Table(wrapper))
            .map(|value| value.trim().to_string())
            .unwrap_or_default()
    }

    fn parse_toml_array_input(
        raw: &str,
        key: &str,
        label: &str,
    ) -> Result<Option<Vec<TomlValue>>, String> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Ok(None);
        }
        let parsed: TomlValue =
            toml::from_str(trimmed).map_err(|err| format!("{label} TOML parse error: {err}"))?;
        let Some(table) = parsed.as_table() else {
            return Err(format!("{label} must be a TOML table"));
        };
        let value = table
            .get(key)
            .or_else(|| {
                table
                    .get("nexus")
                    .and_then(TomlValue::as_table)
                    .and_then(|nexus| nexus.get(key))
            })
            .ok_or_else(|| format!("{label} must define `{key}` entries"))?;
        let Some(array) = value.as_array() else {
            return Err(format!("{label} must be an array of tables"));
        };
        for (idx, entry) in array.iter().enumerate() {
            if !entry.is_table() {
                return Err(format!("{label} entry {idx} must be a table"));
            }
        }
        Ok(Some(array.clone()))
    }

    fn parse_lane_count_input(raw: &str) -> Result<Option<u32>, String> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Ok(None);
        }
        let parsed = trimmed
            .parse::<u32>()
            .map_err(|_| "Lane count must be a positive integer within the u32 range".to_owned())?;
        if parsed == 0 {
            return Err("Lane count must be greater than zero".to_owned());
        }
        Ok(Some(parsed))
    }

    fn lane_reset_candidates(nexus: Option<&TomlTable>) -> Vec<LaneResetCandidate> {
        let Some(nexus) = nexus else {
            return Vec::new();
        };
        if matches!(
            nexus.get("enabled").and_then(TomlValue::as_bool),
            Some(false)
        ) {
            return Vec::new();
        }
        let snapshot = lane_catalog_snapshot(Some(nexus));
        snapshot
            .lane_ids()
            .into_iter()
            .map(|lane_id| LaneResetCandidate {
                lane_id,
                alias: snapshot.lane_alias(lane_id),
                dataspace: snapshot.dataspace_label(snapshot.lane_dataspace_id(lane_id)),
            })
            .collect()
    }

    fn lane_metadata_for_id(nexus: Option<&TomlTable>, lane_id: u32) -> LaneMetadata {
        let snapshot = lane_catalog_snapshot(nexus);
        let alias = snapshot.lane_alias(lane_id);
        let dataspace_id = snapshot.lane_dataspace_id(lane_id).unwrap_or(0);

        let mut metadata = LaneMetadata {
            id: LaneId::new(lane_id),
            alias,
            dataspace_id: DataSpaceId::new(u64::from(dataspace_id)),
            ..Default::default()
        };

        let table = nexus
            .and_then(|nexus| nexus.get("lane_catalog"))
            .and_then(TomlValue::as_array)
            .and_then(|entries| {
                entries.iter().find_map(|entry| {
                    let table = entry.as_table()?;
                    let entry_lane_id = table
                        .get("index")
                        .or_else(|| table.get("id"))
                        .and_then(toml_u32)
                        .unwrap_or(lane_id);
                    (entry_lane_id == lane_id).then_some(table)
                })
            });

        if let Some(table) = table {
            if let Some(description) = table.get("description").and_then(toml_string) {
                metadata.description = Some(description);
            }
            if let Some(lane_type) = table.get("lane_type").and_then(toml_string) {
                metadata.lane_type = Some(lane_type);
            }
            if let Some(governance) = table.get("governance").and_then(toml_string) {
                metadata.governance = Some(governance);
            }
            if let Some(settlement) = table.get("settlement").and_then(toml_string) {
                metadata.settlement = Some(settlement);
            }
            if let Some(visibility) = table
                .get("visibility")
                .and_then(toml_string)
                .and_then(|raw| raw.parse::<LaneVisibility>().ok())
            {
                metadata.visibility = visibility;
            }
            if let Some(storage) = table
                .get("storage")
                .or_else(|| table.get("storage_profile"))
                .and_then(toml_string)
                .and_then(|raw| raw.parse::<LaneStorageProfile>().ok())
            {
                metadata.storage = storage;
            }
            if let Some(scheme) = table
                .get("proof_scheme")
                .or_else(|| table.get("proof"))
                .and_then(toml_string)
                .and_then(|raw| raw.parse::<DaProofScheme>().ok())
            {
                metadata.proof_scheme = scheme;
            }
            if let Some(metadata_table) = table.get("metadata").and_then(TomlValue::as_table) {
                for (key, value) in metadata_table {
                    if let Some(text) = toml_string(value) {
                        metadata.metadata.insert(key.clone(), text);
                    }
                }
            }
        }

        metadata
    }

    fn lane_storage_preview(
        supervisor: &Supervisor,
        lane_id: u32,
        alias: &str,
    ) -> Option<LanePathPreview> {
        let peer = supervisor.peers().first()?;
        let peer_alias = peer.alias().to_owned();
        let storage_root = supervisor.paths().peer_dir(&peer_alias).join("storage");
        let slug = lane_slug(alias, lane_id);
        let blocks_dir = storage_root
            .join("blocks")
            .join(format!("lane_{lane_id:03}_{slug}"));
        let merge_log = storage_root
            .join("merge_ledger")
            .join(format!("lane_{lane_id:03}_{slug}_merge.log"));
        Some(LanePathPreview {
            lane_id,
            alias: alias.to_owned(),
            peer_alias,
            blocks_dir,
            merge_log,
        })
    }

    fn lane_path_previews(
        &self,
        lane_count: Option<u32>,
        lane_catalog: Option<&[TomlValue]>,
    ) -> Option<Vec<LanePathPreview>> {
        let base_root = if self.settings_data_root_input.trim().is_empty() {
            self.supervisor
                .as_ref()
                .map(Self::supervisor_base_data_root)
                .unwrap_or_else(std::env::temp_dir)
        } else {
            PathBuf::from(self.settings_data_root_input.trim())
        };
        let profile = self
            .supervisor
            .as_ref()
            .map(|supervisor| supervisor.profile().clone())
            .unwrap_or_else(|| NetworkProfile::from_preset(ProfilePreset::SinglePeer));
        let paths = mochi_core::config::NetworkPaths::from_root(base_root, &profile);
        let peer_alias = self
            .supervisor
            .as_ref()
            .and_then(|supervisor| supervisor.peers().first())
            .map(|peer| peer.alias().to_owned())
            .unwrap_or_else(|| "peer0".to_owned());
        let storage_root = paths.peer_dir(&peer_alias).join("storage");

        let mut aliases = BTreeMap::new();
        let mut max_index: Option<u32> = None;
        if let Some(entries) = lane_catalog {
            for (idx, entry) in entries.iter().enumerate() {
                let Some(table) = entry.as_table() else {
                    continue;
                };
                let lane_id = table
                    .get("index")
                    .or_else(|| table.get("id"))
                    .and_then(toml_u32)
                    .or_else(|| u32::try_from(idx).ok())
                    .unwrap_or(0);
                max_index = Some(max_index.map_or(lane_id, |prev| prev.max(lane_id)));
                let alias = table
                    .get("alias")
                    .and_then(toml_string)
                    .unwrap_or_else(|| default_lane_alias(lane_id));
                aliases.insert(lane_id, alias);
            }
        }

        let effective_count = lane_count
            .or_else(|| max_index.map(|value| value.saturating_add(1)))
            .unwrap_or(1);
        if aliases.is_empty() {
            if effective_count > 1 {
                for lane_id in 0..effective_count {
                    aliases.insert(lane_id, default_lane_alias(lane_id));
                }
            } else {
                aliases.insert(0, default_lane_alias(0));
            }
        } else if effective_count > aliases.len() as u32 {
            for lane_id in 0..effective_count {
                aliases
                    .entry(lane_id)
                    .or_insert_with(|| default_lane_alias(lane_id));
            }
        }

        let previews = aliases
            .into_iter()
            .map(|(lane_id, alias)| {
                let slug = lane_slug(&alias, lane_id);
                let blocks_dir = storage_root
                    .join("blocks")
                    .join(format!("lane_{lane_id:03}_{slug}"));
                let merge_log = storage_root
                    .join("merge_ledger")
                    .join(format!("lane_{lane_id:03}_{slug}_merge.log"));
                LanePathPreview {
                    lane_id,
                    alias,
                    peer_alias: peer_alias.clone(),
                    blocks_dir,
                    merge_log,
                }
            })
            .collect::<Vec<_>>();
        Some(previews)
    }

    fn reset_lane_with_lifecycle(
        supervisor: &mut Supervisor,
        lane_id: u32,
        handle: &Handle,
    ) -> Result<(), String> {
        Self::reset_lane_with_lifecycle_inner(supervisor, lane_id, |supervisor, plan| {
            let mut started_for_lifecycle = false;
            if !supervisor.is_any_running() {
                supervisor
                    .start_all()
                    .map_err(|err| format!("Failed to start peers for lane reset: {err}"))?;
                started_for_lifecycle = true;
            }

            let peer_alias = supervisor
                .peers()
                .iter()
                .find(|peer| matches!(peer.state(), PeerState::Running))
                .or_else(|| supervisor.peers().first())
                .map(|peer| peer.alias().to_owned())
                .ok_or_else(|| "No peers available for lane reset.".to_owned())?;
            let client = supervisor
                .torii_client(&peer_alias)
                .ok_or_else(|| format!("Missing Torii client for {peer_alias}"))?;
            let lifecycle_result = handle.block_on(async {
                let options = ReadinessOptions::new(READINESS_TIMEOUT)
                    .with_poll_interval(READINESS_POLL_INTERVAL);
                client
                    .wait_for_ready(options)
                    .await
                    .map_err(|err| format!("Torii readiness failed: {err}"))?;

                let mut backoff = Duration::from_millis(200);
                let attempts = 3;
                for attempt in 0..attempts {
                    match client.apply_lane_lifecycle(&plan).await {
                        Ok(_) => return Ok(()),
                        Err(_err) if attempt + 1 < attempts => {
                            tokio::time::sleep(backoff).await;
                            backoff = (backoff.saturating_mul(2)).min(Duration::from_secs(2));
                        }
                        Err(err) => {
                            return Err(format!("Lane lifecycle apply failed: {err}"));
                        }
                    }
                }
                Err("Lane lifecycle apply failed.".to_owned())
            });

            let stop_result = if started_for_lifecycle {
                supervisor
                    .stop_all()
                    .map_err(|err| format!("Failed to stop peers after lane reset: {err}"))
            } else {
                Ok(())
            };

            match (lifecycle_result, stop_result) {
                (Ok(()), Ok(())) => Ok(()),
                (Ok(()), Err(err)) => Err(err),
                (Err(err), Ok(())) => Err(err),
                (Err(err), Err(stop_err)) => Err(format!("{err} ({stop_err})")),
            }
        })
    }

    fn reset_lane_with_lifecycle_inner<F>(
        supervisor: &mut Supervisor,
        lane_id: u32,
        mut apply: F,
    ) -> Result<(), String>
    where
        F: FnMut(&mut Supervisor, LaneLifecyclePlan) -> Result<(), String>,
    {
        supervisor
            .reset_lane_storage(lane_id)
            .map_err(|err| err.to_string())?;
        let plan = LaneLifecyclePlan {
            additions: vec![Self::lane_metadata_for_id(
                supervisor.nexus_config_overrides(),
                lane_id,
            )],
            retire: vec![LaneId::new(lane_id)],
        };
        apply(supervisor, plan)
    }

    fn render_view_tabs(&mut self, ui: &mut egui::Ui) {
        let palette = Self::palette();
        Frame::new()
            .fill(palette.panel)
            .stroke(Stroke::new(1.0, palette.border))
            .corner_radius(CornerRadius::same(12))
            .inner_margin(Margin::symmetric(10, 8))
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing = egui::vec2(8.0, 8.0);
                ui.horizontal_wrapped(|ui| {
                    for view in ActiveView::all() {
                        let selected = self.active_view == view;
                        let (fill, stroke, text_color) = if selected {
                            (palette.accent_soft, palette.accent, palette.text)
                        } else {
                            (palette.surface, palette.border, palette.text_muted)
                        };
                        let label = RichText::new(view.label()).color(text_color).size(13.0);
                        let button = Button::selectable(selected, label)
                            .fill(fill)
                            .stroke(Stroke::new(1.0, stroke))
                            .corner_radius(CornerRadius::same(8))
                            .min_size(egui::vec2(72.0, 26.0));
                        if ui.add(button).clicked() {
                            self.active_view = view;
                        }
                    }
                });
            });
        ui.add_space(8.0);
    }

    fn build_peer_rows(&self, supervisor: &Supervisor) -> Vec<PeerRow> {
        supervisor
            .peers()
            .iter()
            .map(|peer| {
                let (api_base, api_error) = match peer.torii_client() {
                    Ok(client) => (Some(client.base_url().to_owned()), None),
                    Err(err) => (None, Some(err.to_string())),
                };
                PeerRow {
                    alias: peer.alias().to_owned(),
                    state: peer.state(),
                    torii: peer.torii_address().to_owned(),
                    api_base,
                    api_error,
                    config: peer.config_path().display().to_string(),
                    logs: peer.log_path().display().to_string(),
                }
            })
            .collect()
    }

    fn collect_dashboard_metrics(&self, peer_rows: &[PeerRow]) -> DashboardMetrics {
        let total_peers = peer_rows.len();
        let running_peers = peer_rows
            .iter()
            .filter(|row| matches!(row.state, PeerState::Running))
            .count();
        let connected_block_streams = self
            .block_snapshots
            .values()
            .filter(|snapshot| snapshot.connected)
            .count();
        let connected_event_streams = self
            .event_snapshots
            .values()
            .filter(|snapshot| snapshot.connected)
            .count();
        let pending_block_events = self.block_events.len();
        let pending_event_frames = self.event_events.len();
        let stored_logs = self.log_events.len();

        let mut latest_height = None;
        let mut total_tx = 0u64;
        let mut total_rejected_tx = 0u64;
        for snapshot in self.block_snapshots.values() {
            if let Some(summary) = &snapshot.last_summary {
                latest_height = Some(
                    latest_height
                        .map_or(summary.height, |current: u64| current.max(summary.height)),
                );
                total_tx += summary.transaction_count as u64;
                total_rejected_tx += summary.rejected_transaction_count as u64;
            }
        }

        let mut queue_total = 0.0f64;
        let mut queue_count = 0u64;
        let mut latency_total = 0.0f64;
        let mut latency_count = 0u64;
        for view in self.status_snapshots.values() {
            if let Some(snapshot) = view.last_snapshot.as_ref() {
                queue_total += snapshot.status.queue_size as f64;
                queue_count += 1;
                latency_total += snapshot.metrics.commit_latency_ms as f64;
                latency_count += 1;
            }
        }
        let avg_queue = if queue_count > 0 {
            Some(queue_total / queue_count as f64)
        } else {
            None
        };
        let avg_commit_latency_ms = if latency_count > 0 {
            Some(latency_total / latency_count as f64)
        } else {
            None
        };

        let mut tx_delta_total = 0u64;
        let mut timespan_total = 0f64;
        for history in self.status_history.values() {
            if history.len() < 2 {
                continue;
            }
            let first = history.front().unwrap();
            let last = history.back().unwrap();
            let duration = last
                .timestamp
                .saturating_duration_since(first.timestamp)
                .as_secs_f64();
            if duration <= f64::EPSILON {
                continue;
            }
            let delta = last
                .snapshot
                .status
                .txs_approved
                .saturating_sub(first.snapshot.status.txs_approved);
            tx_delta_total += delta;
            timespan_total += duration;
        }
        let throughput_per_sec = if timespan_total > f64::EPSILON {
            Some(tx_delta_total as f64 / timespan_total)
        } else {
            None
        };

        DashboardMetrics {
            total_peers,
            running_peers,
            connected_block_streams,
            connected_event_streams,
            latest_height,
            total_tx,
            total_rejected_tx,
            avg_queue,
            avg_commit_latency_ms,
            throughput_per_sec,
            pending_block_events,
            pending_event_frames,
            stored_logs,
        }
    }

    fn render_overview_bar(
        &mut self,
        ui: &mut egui::Ui,
        supervisor: &mut Supervisor,
        peer_rows: &[PeerRow],
        metrics: &DashboardMetrics,
    ) {
        let palette = Self::palette();
        let peers_value = metrics.peers_label();
        let latest_value = metrics.latest_height_text();
        let queue_value = metrics.queue_text();
        let latency_value = metrics.latency_text();
        let throughput_value = metrics.throughput_text();

        let stream_detail = format!(
            "{} block streams · {} event streams",
            metrics.connected_block_streams, metrics.connected_event_streams
        );
        let block_detail = format!(
            "tx {} · rejected {}",
            metrics.total_tx, metrics.total_rejected_tx
        );
        let queue_detail = format!("{} event frames buffered", metrics.pending_event_frames);
        let latency_detail = format!("{} block frames buffered", metrics.pending_block_events);
        let throughput_detail = format!("{} stored logs", metrics.stored_logs);

        Frame::new()
            .fill(palette.panel_alt)
            .stroke(Stroke::new(1.0, palette.border))
            .corner_radius(CornerRadius::same(10))
            .inner_margin(Margin::symmetric(14, 12))
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(14.0, 12.0);
                    self.render_stat_card(
                        ui,
                        "Peers Active",
                        &peers_value,
                        &stream_detail,
                        Color32::from_rgb(120, 190, 255),
                    );
                    self.render_stat_card(
                        ui,
                        "Latest Block",
                        &latest_value,
                        &block_detail,
                        Color32::from_rgb(144, 220, 164),
                    );
                    self.render_stat_card(
                        ui,
                        "Queue Size",
                        &queue_value,
                        &queue_detail,
                        Color32::from_rgb(250, 200, 128),
                    );
                    self.render_stat_card(
                        ui,
                        "Commit Latency",
                        &latency_value,
                        &latency_detail,
                        Color32::from_rgb(210, 170, 255),
                    );
                    self.render_stat_card(
                        ui,
                        "Throughput",
                        &throughput_value,
                        &throughput_detail,
                        Color32::from_rgb(255, 140, 200),
                    );
                });
                ui.add_space(10.0);
                self.render_control_bar(ui, supervisor, peer_rows);
                ui.add_space(6.0);
                let preset = Self::profile_name(supervisor);
                let footer = if let Some(report) = supervisor.compatibility() {
                    format!(
                        "{} • {} • Data root {}",
                        preset,
                        report.summary_line(),
                        supervisor.paths().root().display()
                    )
                } else {
                    format!(
                        "{} • Chain {} • Data root {}",
                        preset,
                        supervisor.chain_id(),
                        supervisor.paths().root().display()
                    )
                };
                ui.label(RichText::new(footer).size(12.0).color(palette.text_muted));
            });
    }

    fn render_control_bar(
        &mut self,
        ui: &mut egui::Ui,
        supervisor: &mut Supervisor,
        peer_rows: &[PeerRow],
    ) {
        let palette = Self::palette();
        let all_running = peer_rows
            .iter()
            .all(|row| matches!(row.state, PeerState::Running));
        let any_running = peer_rows
            .iter()
            .any(|row| matches!(row.state, PeerState::Running));
        let maintenance_busy = self.maintenance_state.is_running();
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(12.0, 8.0);
            ui.vertical(|ui| {
                ui.label(RichText::new("Peers").size(11.0).color(palette.text_muted));
                ui.horizontal(|ui| {
                    let start_btn = ui
                        .add_enabled(!all_running, egui::Button::new("Start all peers"))
                        .on_hover_text("Start every configured peer in this profile");
                    if start_btn.clicked() {
                        match supervisor.start_all() {
                            Ok(()) => {
                                self.last_info = Some("All peers started.".to_owned());
                                self.last_error = None;
                                let aliases = peer_rows.iter().map(|row| row.alias.clone());
                                self.spawn_readiness_checks(supervisor, aliases);
                            }
                            Err(err) => {
                                self.last_error = Some(format!("Start failed: {err}"));
                                self.last_info = None;
                            }
                        }
                    }
                    let stop_btn = ui
                        .add_enabled(any_running, egui::Button::new("Stop all peers"))
                        .on_hover_text("Stop all running peers in this profile");
                    if stop_btn.clicked() {
                        match supervisor.stop_all() {
                            Ok(()) => {
                                self.last_info = Some("All peers stopped.".to_owned());
                                self.last_error = None;
                            }
                            Err(err) => {
                                self.last_error = Some(format!("Stop failed: {err}"));
                                self.last_info = None;
                            }
                        }
                    }
                });
            });
            ui.vertical(|ui| {
                ui.label(
                    RichText::new("Maintenance")
                        .size(11.0)
                        .color(palette.text_muted),
                );
                let lane_reset_available =
                    !Self::lane_reset_candidates(supervisor.nexus_config_overrides()).is_empty();
                ui.horizontal(|ui| {
                    let export_btn = ui
                        .add_enabled(!maintenance_busy, egui::Button::new("Export snapshot"))
                        .on_hover_text("Export state to a snapshot folder");
                    if export_btn.clicked() {
                        self.maintenance_snapshot_label.clear();
                        self.maintenance_dialog = Some(MaintenanceDialog::ExportSnapshot);
                    }
                    let restore_btn = ui
                        .add_enabled(!maintenance_busy, egui::Button::new("Restore snapshot"))
                        .on_hover_text("Restore state from an existing snapshot");
                    if restore_btn.clicked() {
                        self.maintenance_restore_input.clear();
                        self.maintenance_dialog = Some(MaintenanceDialog::RestoreSnapshot);
                    }
                    let lane_reset_btn = ui
                        .add_enabled(
                            !maintenance_busy && lane_reset_available,
                            egui::Button::new("Reset lane"),
                        )
                        .on_hover_text(if lane_reset_available {
                            "Wipe lane storage and re-apply the lane catalog"
                        } else {
                            "No Nexus lane catalog configured"
                        });
                    if lane_reset_btn.clicked() {
                        self.lane_reset_selection = None;
                        self.maintenance_dialog = Some(MaintenanceDialog::ResetLane);
                    }
                    let reset_btn = ui
                        .add_enabled(!maintenance_busy, egui::Button::new("Wipe & re-genesis"))
                        .on_hover_text("Delete state and run a fresh genesis");
                    if reset_btn.clicked() {
                        self.maintenance_dialog = Some(MaintenanceDialog::ConfirmReset);
                    }
                    if maintenance_busy {
                        ui.add(egui::Spinner::new());
                    }
                });
            });
            ui.vertical(|ui| {
                ui.label(RichText::new("Config").size(11.0).color(palette.text_muted));
                if ui
                    .button("Settings")
                    .on_hover_text("Review supervisor defaults and log routing")
                    .clicked()
                {
                    self.initialize_settings_from_supervisor();
                    self.settings_dialog = true;
                }
            });
        });
    }

    fn render_settings_dialog(&mut self, ctx: &egui::Context) {
        if !self.settings_dialog {
            return;
        }
        let mut open = true;
        egui::Window::new("Settings")
            .collapsible(false)
            .resizable(false)
            .open(&mut open)
            .show(ctx, |ui| {
                ui.label("Adjust supervisor configuration for future launches.");
                ui.add_space(8.0);
                ui.label("Data root (blank = default temp directory):");
                ui.add(
                    egui::TextEdit::singleline(&mut self.settings_data_root_input)
                        .hint_text("/path/to/mochi-data"),
                );
                ui.add_space(8.0);
                ui.label("Torii base port (blank = keep current default):");
                ui.add(
                    egui::TextEdit::singleline(&mut self.settings_torii_port_input)
                        .hint_text("8080"),
                );
                ui.add_space(8.0);
                ui.label("P2P base port (blank = keep current default):");
                ui.add(
                    egui::TextEdit::singleline(&mut self.settings_p2p_port_input).hint_text("1337"),
                );
                ui.add_space(8.0);
                ui.label("Chain ID (blank = use default `mochi-local`):");
                ui.add(
                    egui::TextEdit::singleline(&mut self.settings_chain_id_input)
                        .hint_text("mochi-local"),
                );
                ui.add_space(10.0);
                if let Some(supervisor) = self.supervisor.as_ref() {
                    ui.label(format!(
                        "Current profile: {}",
                        Self::profile_name(supervisor)
                    ));
                } else {
                    ui.label("Current profile: unavailable");
                }
                ui.label(
                    "Profile override (preset slug or inline TOML table; blank = keep current):",
                );
                ui.add(
                    egui::TextEdit::singleline(&mut self.settings_profile_input).hint_text(
                        "single-peer | four-peer-bft | { peer_count = 3, consensus_mode = \"permissioned\" }",
                    ),
                );
                ui.small(
                    "Include genesis_profile for NPoS (e.g., { peer_count = 4, consensus_mode = \"npos\", genesis_profile = \"iroha3-dev\" }).",
                );
                ui.add_space(10.0);
                egui::CollapsingHeader::new("Nexus lanes and DA")
                    .default_open(true)
                    .show(ui, |ui| {
                        ui.checkbox(
                            &mut self.settings_nexus_enabled,
                            "Enable Nexus / multi-lane mode",
                        );
                        if self.settings_nexus_enabled {
                            self.settings_sumeragi_da_enabled = true;
                        }
                        ui.add_space(6.0);
                        ui.label("Lane count (blank = default 1):");
                        ui.add(
                            egui::TextEdit::singleline(&mut self.settings_nexus_lane_count_input)
                                .hint_text("3"),
                        );
                        ui.add_space(6.0);
                        ui.label("Lane catalog (TOML using [[lane_catalog]] entries):");
                        ui.add(
                            egui::TextEdit::multiline(
                                &mut self.settings_nexus_lane_catalog_input,
                            )
                            .desired_rows(6)
                            .hint_text("[[lane_catalog]]\nindex = 0\nalias = \"core\"\ndataspace = \"global\""),
                        );
                        ui.add_space(6.0);
                        ui.label("Dataspace catalog (TOML using [[dataspace_catalog]] entries):");
                        ui.add(
                            egui::TextEdit::multiline(
                                &mut self.settings_nexus_dataspace_catalog_input,
                            )
                            .desired_rows(4)
                            .hint_text("[[dataspace_catalog]]\nalias = \"global\"\nid = 0"),
                        );
                        ui.add_space(6.0);
                        ui.add_enabled(
                            !self.settings_nexus_enabled,
                            egui::Checkbox::new(
                                &mut self.settings_sumeragi_da_enabled,
                                "Enable data-availability gating (RBC + availability QC)",
                            ),
                        );
                        if self.settings_nexus_enabled {
                            ui.small("DA gating is required for Nexus lanes.");
                        }
                        ui.add_space(6.0);
                        ui.label("Torii DA replay cache dir (blank = per-peer default):");
                        ui.add(
                            egui::TextEdit::singleline(
                                &mut self.settings_torii_da_replay_dir_input,
                            )
                            .hint_text("/path/to/da_replay"),
                        );
                        ui.add_space(6.0);
                        ui.label("Torii DA manifest spool dir (blank = per-peer default):");
                        ui.add(
                            egui::TextEdit::singleline(
                                &mut self.settings_torii_da_manifest_dir_input,
                            )
                            .hint_text("/path/to/da_manifests"),
                        );
                        let lane_count = match Self::parse_lane_count_input(
                            &self.settings_nexus_lane_count_input,
                        ) {
                            Ok(value) => value,
                            Err(err) => {
                                ui.colored_label(Color32::from_rgb(200, 160, 64), err);
                                None
                            }
                        };
                        let lane_catalog = match Self::parse_toml_array_input(
                            &self.settings_nexus_lane_catalog_input,
                            "lane_catalog",
                            "Lane catalog",
                        ) {
                            Ok(value) => value,
                            Err(err) => {
                                ui.colored_label(Color32::from_rgb(200, 160, 64), err);
                                None
                            }
                        };
                        if let Some(previews) =
                            self.lane_path_previews(lane_count, lane_catalog.as_deref())
                            && !previews.is_empty()
                        {
                            ui.add_space(6.0);
                            ui.label("Lane storage preview (per peer):");
                            egui::Grid::new("mochi_lane_storage_preview")
                                .num_columns(3)
                                .striped(true)
                                .show(ui, |ui| {
                                    ui.label("Lane");
                                    ui.label("Blocks dir");
                                    ui.label("Merge log");
                                    ui.end_row();
                                    for preview in previews {
                                        ui.label(format!(
                                            "{} ({})",
                                            preview.lane_id, preview.alias
                                        ));
                                        ui.label(preview.blocks_dir.display().to_string());
                                        ui.label(preview.merge_log.display().to_string());
                                        ui.end_row();
                                    }
                                });
                        }
                        ui.small(
                            "Rendered peer configs include per-lane blocks/merge-log paths in the header.",
                        );
                    });
                ui.add_space(8.0);
                ui.checkbox(
                    &mut self.settings_build_binaries,
                    "Auto-build missing binaries (cargo build)",
                );
                ui.small(
                    "When enabled, MOCHI may run `cargo build` to build missing `irohad`, \
                     `kagami`, and `iroha_cli` binaries. Overrides: `--build-binaries`/`--no-build-binaries` \
                     or `MOCHI_BUILD_BINARIES`.",
                );
                ui.add_space(6.0);
                ui.checkbox(
                    &mut self.settings_readiness_smoke,
                    "Gate readiness on a transaction/block smoke",
                );
                ui.small(
                    "When enabled, MOCHI submits a signed transaction and waits for it to appear in the \
                     block stream before marking a peer ready. Overrides: `--disable-smoke`/`--enable-smoke` \
                     or `MOCHI_READINESS_SMOKE`.",
                );
                ui.add_space(8.0);
                ui.collapsing("Binary compatibility", |ui| {
                    let Some(supervisor) = self.supervisor.as_ref() else {
                        ui.label("Supervisor not prepared.");
                        return;
                    };
                    let Some(report) = supervisor.compatibility() else {
                        ui.label("Compatibility report unavailable.");
                        return;
                    };
                    ui.label(report.summary_line());
                    ui.add_space(6.0);
                    egui::Grid::new("mochi_binary_compat_grid")
                        .num_columns(2)
                        .striped(true)
                        .show(ui, |ui| {
                            for info in &report.versions {
                                ui.label(format!("{}:", info.name));
                                ui.label(format!(
                                    "{} ({}, {})",
                                    info.path.display(),
                                    info.build_line,
                                    info.source_label()
                                ));
                                ui.end_row();
                                if let Some(version) = info.version.as_ref() {
                                    ui.label("version:");
                                    ui.label(version);
                                    ui.end_row();
                                }
                            }
                        });
                    if let Some(verify) = report.verify.as_ref() {
                        ui.add_space(6.0);
                        ui.separator();
                        ui.label("kagami verify:");
                        ui.label(format!("profile: {}", verify.profile));
                        if let Some(chain_id) = verify.chain_id.as_ref() {
                            ui.label(format!("reported chain: {chain_id}"));
                        }
                        if let Some(fingerprint) = verify.fingerprint.as_ref() {
                            ui.label(format!("fingerprint: {fingerprint}"));
                        }
                    }
                });
                ui.add_space(12.0);
                ui.label("Log visibility:");
                ui.horizontal(|ui| {
                    ui.checkbox(&mut self.settings_log_stdout, "stdout");
                    ui.checkbox(&mut self.settings_log_stderr, "stderr");
                    ui.checkbox(&mut self.settings_log_system, "system");
                });
                ui.add_space(12.0);
                ui.label("Log export directory (blank = system temporary directory):");
                ui.add(
                    egui::TextEdit::singleline(&mut self.settings_log_export_dir_input)
                        .hint_text("/tmp/mochi_logs"),
                );
                ui.add_space(8.0);
                ui.label("State export directory (blank = system temporary directory):");
                ui.add(
                    egui::TextEdit::singleline(&mut self.settings_state_export_dir_input)
                        .hint_text("/tmp/mochi_state"),
                );
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    if ui.button("Apply changes").clicked() {
                        match self.apply_settings_changes() {
                            Ok(()) => {
                                self.last_info = Some(
                                    "Settings applied. Restarted peers if they were running."
                                        .to_owned(),
                                );
                                self.last_error = None;
                            }
                            Err(err) => {
                                self.last_error = Some(err);
                            }
                        }
                    }
                    if ui.button("Close").clicked() {
                        self.settings_dialog = false;
                    }
                });
            });
        if !open {
            self.settings_dialog = false;
        }
    }

    fn render_maintenance_dialog(&mut self, ctx: &egui::Context, supervisor: &mut Supervisor) {
        let Some(dialog) = self.maintenance_dialog else {
            return;
        };

        match dialog {
            MaintenanceDialog::ExportSnapshot => {
                let mut open = true;
                let busy = self.maintenance_state.is_running();
                egui::Window::new("Export snapshot")
                    .collapsible(false)
                    .resizable(false)
                    .open(&mut open)
                    .show(ctx, |ui| {
                        ui.label(
                            "Export the current network state to a snapshot folder under the data root.",
                        );
                        ui.add_space(8.0);
                        ui.label("Snapshot label (optional):");
                        ui.add(egui::TextEdit::singleline(
                            &mut self.maintenance_snapshot_label,
                        ));
                        if let Some(preview) =
                            Self::preview_snapshot_label(&self.maintenance_snapshot_label)
                        {
                            ui.small(format!("Resulting folder name: {preview}"));
                        } else if !self.maintenance_snapshot_label.trim().is_empty() {
                            ui.small(
                                "Label contains no valid characters; a timestamp will be used.",
                            );
                        } else {
                            ui.small("Leave blank to use an auto-generated timestamp.");
                        }
                        ui.small("Unsupported characters are stripped automatically.");
                        ui.add_space(12.0);
                        ui.horizontal(|ui| {
                            let create_btn =
                                ui.add_enabled(!busy, egui::Button::new("Create snapshot"));
                            if create_btn.clicked()
                                && self.begin_maintenance(MaintenanceTask::Snapshot)
                            {
                                let label = self.maintenance_snapshot_label.trim();
                                let label_opt = if label.is_empty() {
                                    None
                                } else {
                                    Some(label.to_owned())
                                };
                                self.maintenance_command = Some(MaintenanceCommand::ExportSnapshot {
                                    label: label_opt,
                                });
                                self.maintenance_dialog = None;
                                self.maintenance_snapshot_label.clear();
                            }
                            if ui
                                .add_enabled(!busy, egui::Button::new("Cancel"))
                                .clicked()
                            {
                                self.maintenance_dialog = None;
                                self.maintenance_snapshot_label.clear();
                                self.maintenance_command = None;
                                self.maintenance_inflight = None;
                                if matches!(
                                    self.maintenance_state,
                                    MaintenanceState::Running(MaintenanceTask::Snapshot)
                                ) {
                                    self.maintenance_state = MaintenanceState::Idle;
                                }
                            }
                            if busy {
                                ui.add(egui::Spinner::new());
                            }
                        });
                    });
                if !open {
                    self.maintenance_dialog = None;
                    self.maintenance_snapshot_label.clear();
                }
            }
            MaintenanceDialog::ConfirmReset => {
                let mut open = true;
                let busy = self.maintenance_state.is_running();
                egui::Window::new("Wipe & re-genesis")
                    .collapsible(false)
                    .resizable(false)
                    .open(&mut open)
                    .show(ctx, |ui| {
                        ui.label(RichText::new(
                            "This stops all peers, clears local storage, and regenerates genesis.",
                        ));
                        ui.label(RichText::new("Any unsaved state will be lost. Continue?"));
                        ui.add_space(12.0);
                        ui.horizontal(|ui| {
                            let wipe_btn =
                                ui.add_enabled(!busy, egui::Button::new("Wipe & regenerate"));
                            if wipe_btn.clicked() && self.begin_maintenance(MaintenanceTask::Reset)
                            {
                                self.maintenance_command = Some(MaintenanceCommand::Reset);
                                self.maintenance_dialog = None;
                            }
                            if ui.add_enabled(!busy, egui::Button::new("Cancel")).clicked() {
                                self.maintenance_dialog = None;
                                self.maintenance_command = None;
                                self.maintenance_inflight = None;
                                if matches!(
                                    self.maintenance_state,
                                    MaintenanceState::Running(MaintenanceTask::Reset)
                                ) {
                                    self.maintenance_state = MaintenanceState::Idle;
                                }
                            }
                            if busy {
                                ui.add(egui::Spinner::new());
                            }
                        });
                    });
                if !open {
                    self.maintenance_dialog = None;
                }
            }
            MaintenanceDialog::RestoreSnapshot => {
                let mut open = true;
                let busy = self.maintenance_state.is_running();
                egui::Window::new("Restore snapshot")
                    .collapsible(false)
                    .resizable(false)
                    .open(&mut open)
                    .show(ctx, |ui| {
                        ui.label(
                            "Restore peers, configs, logs, and genesis from an existing snapshot.",
                        );
                        ui.label(
                            "Provide either a snapshot folder name (e.g. smoke-snapshot-42) or an absolute path.",
                        );
                        ui.add_space(8.0);
                        ui.add(
                            egui::TextEdit::singleline(&mut self.maintenance_restore_input)
                                .hint_text("smoke-snapshot-42 or /path/to/snapshot"),
                        );
                        ui.add_space(12.0);
                        ui.horizontal(|ui| {
                            let trimmed_input = self.maintenance_restore_input.trim().to_owned();
                            let restore_btn = ui.add_enabled(
                                !busy && !trimmed_input.is_empty(),
                                egui::Button::new("Restore snapshot"),
                            );
                            if restore_btn.clicked()
                                && self.begin_maintenance(MaintenanceTask::Restore)
                            {
                                self.maintenance_command = Some(MaintenanceCommand::Restore {
                                    target: trimmed_input.clone(),
                                });
                                self.maintenance_dialog = None;
                                self.maintenance_restore_input.clear();
                            }
                            if ui
                                .add_enabled(!busy, egui::Button::new("Cancel"))
                                .clicked()
                            {
                                self.maintenance_dialog = None;
                                self.maintenance_command = None;
                                self.maintenance_inflight = None;
                                self.maintenance_restore_input.clear();
                                if matches!(
                                    self.maintenance_state,
                                    MaintenanceState::Running(MaintenanceTask::Restore)
                                ) {
                                    self.maintenance_state = MaintenanceState::Idle;
                                }
                            }
                            if busy {
                                ui.add(egui::Spinner::new());
                            }
                        });
                    });
                if !open {
                    self.maintenance_dialog = None;
                    self.maintenance_restore_input.clear();
                }
            }
            MaintenanceDialog::ResetLane => {
                let mut open = true;
                let busy = self.maintenance_state.is_running();
                let candidates = Self::lane_reset_candidates(supervisor.nexus_config_overrides());
                egui::Window::new("Reset lane")
                    .collapsible(false)
                    .resizable(false)
                    .open(&mut open)
                    .show(ctx, |ui| {
                        ui.label(
                            "Reset a single Nexus lane by wiping its storage and reapplying the lane catalog.",
                        );
                        ui.small(
                            "Peers may restart briefly; Torii must be reachable to apply the lifecycle plan.",
                        );
                        ui.add_space(8.0);
                        if candidates.is_empty() {
                            ui.colored_label(
                                Color32::from_rgb(200, 160, 64),
                                "No Nexus lane catalog entries are configured.",
                            );
                        } else {
                            let mut selected = self
                                .lane_reset_selection
                                .unwrap_or(candidates[0].lane_id);
                            ComboBox::from_id_salt("mochi_lane_reset_selector")
                                .selected_text(
                                    candidates
                                        .iter()
                                        .find(|entry| entry.lane_id == selected)
                                        .map(LaneResetCandidate::label)
                                        .unwrap_or_else(|| format!("lane {selected}")),
                                )
                                .show_ui(ui, |ui| {
                                    for candidate in &candidates {
                                        ui.selectable_value(
                                            &mut selected,
                                            candidate.lane_id,
                                            candidate.label(),
                                        );
                                    }
                                });
                            self.lane_reset_selection = Some(selected);
                            if let Some(candidate) = candidates
                                .iter()
                                .find(|entry| entry.lane_id == selected)
                            {
                                ui.add_space(6.0);
                                ui.small(format!(
                                    "Alias: {} • Dataspace: {}",
                                    candidate.alias, candidate.dataspace
                                ));
                                if let Some(preview) =
                                    Self::lane_storage_preview(supervisor, selected, &candidate.alias)
                                {
                                    ui.small(format!(
                                        "Peer {} blocks dir: {}",
                                        preview.peer_alias,
                                        preview.blocks_dir.display()
                                    ));
                                    ui.small(format!(
                                        "Peer {} merge log: {}",
                                        preview.peer_alias,
                                        preview.merge_log.display()
                                    ));
                                }
                            }
                        }
                        ui.add_space(12.0);
                        ui.horizontal(|ui| {
                            let reset_btn = ui.add_enabled(
                                !busy && !candidates.is_empty(),
                                egui::Button::new("Reset lane"),
                            );
                            if reset_btn.clicked()
                                && self.begin_maintenance(MaintenanceTask::LaneReset)
                                && let Some(lane_id) = self.lane_reset_selection
                            {
                                self.maintenance_command =
                                    Some(MaintenanceCommand::ResetLane { lane_id });
                                self.maintenance_dialog = None;
                            }
                            if ui.add_enabled(!busy, egui::Button::new("Cancel")).clicked() {
                                self.maintenance_dialog = None;
                                self.maintenance_command = None;
                                self.maintenance_inflight = None;
                                self.lane_reset_selection = None;
                                if matches!(
                                    self.maintenance_state,
                                    MaintenanceState::Running(MaintenanceTask::LaneReset)
                                ) {
                                    self.maintenance_state = MaintenanceState::Idle;
                                }
                            }
                            if busy {
                                ui.add(egui::Spinner::new());
                            }
                        });
                    });
                if !open {
                    self.maintenance_dialog = None;
                }
            }
        }
    }

    fn render_stat_card(
        &self,
        ui: &mut egui::Ui,
        title: &str,
        value: &str,
        detail: &str,
        accent: Color32,
    ) {
        let palette = Self::palette();
        Frame::new()
            .fill(palette.panel)
            .stroke(Stroke::new(1.0, accent))
            .corner_radius(CornerRadius::same(10))
            .inner_margin(Margin::symmetric(12, 10))
            .show(ui, |ui| {
                ui.label(RichText::new(title).size(11.0).color(accent).strong());
                ui.label(RichText::new(value).size(20.0).color(palette.text).strong());
                if !detail.is_empty() {
                    ui.label(RichText::new(detail).size(12.0).color(palette.text_muted));
                }
            });
    }

    fn render_dashboard_view(
        &mut self,
        ui: &mut egui::Ui,
        supervisor: &mut Supervisor,
        peer_rows: &[PeerRow],
        peer_aliases: &[String],
        metrics: &DashboardMetrics,
    ) {
        if peer_rows.is_empty() {
            ui.label("No peers configured for this profile.");
            return;
        }
        let nexus_table = supervisor.nexus_config_overrides().or_else(|| {
            self.bundle_config
                .as_ref()
                .and_then(|cfg| cfg.config.nexus.as_ref())
        });
        let lane_catalog = lane_catalog_snapshot(nexus_table);

        ui.columns(2, |columns| {
            columns[0].heading("Peer overview");
            columns[0].add_space(6.0);
            ScrollArea::vertical()
                .auto_shrink([false; 2])
                .show(&mut columns[0], |ui| {
                    for row in peer_rows {
                        self.render_peer_card(ui, supervisor, row);
                        ui.add_space(8.0);
                    }
                });

            columns[1].heading("Telemetry");
            columns[1].add_space(6.0);
            self.render_status_panel(&mut columns[1], peer_aliases, metrics, &lane_catalog);
            columns[1].add_space(12.0);
            self.render_activity_panel(&mut columns[1], metrics);
        });
    }

    fn render_peer_card(&mut self, ui: &mut egui::Ui, supervisor: &mut Supervisor, row: &PeerRow) {
        Frame::new()
            .fill(Color32::from_rgb(20, 28, 38))
            .stroke(Stroke::new(1.0, Color32::from_rgb(46, 64, 84)))
            .corner_radius(CornerRadius::same(10))
            .inner_margin(Margin::symmetric(12, 10))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.heading(&row.alias);
                    let state_color = Self::peer_state_color(row.state);
                    ui.colored_label(state_color, row.state.label());
                    ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                        let start_clicked = ui
                            .add_enabled(
                                !matches!(row.state, PeerState::Running),
                                egui::Button::new("Start"),
                            )
                            .clicked();
                        let stop_clicked = ui
                            .add_enabled(
                                matches!(row.state, PeerState::Running),
                                egui::Button::new("Stop"),
                            )
                            .clicked();
                        if start_clicked {
                            match supervisor.start_peer(&row.alias) {
                                Ok(()) => {
                                    self.last_info = Some(format!("Started {}.", row.alias));
                                    self.last_error = None;
                                    self.spawn_readiness_checks(
                                        supervisor,
                                        vec![row.alias.clone()],
                                    );
                                }
                                Err(err) => {
                                    self.last_error =
                                        Some(format!("Failed to start {}: {err}", row.alias));
                                    self.last_info = None;
                                }
                            }
                        }
                        if stop_clicked {
                            match supervisor.stop_peer(&row.alias) {
                                Ok(()) => {
                                    self.last_info = Some(format!("Stopped {}.", row.alias));
                                    self.last_error = None;
                                }
                                Err(err) => {
                                    self.last_error =
                                        Some(format!("Failed to stop {}: {err}", row.alias));
                                    self.last_info = None;
                                }
                            }
                        }
                    });
                });
                ui.add_space(4.0);
                ui.small(format!("Torii: {}", row.torii));
                if let Some(api) = &row.api_base {
                    ui.small(format!("API base: {api}"));
                } else if let Some(err) = &row.api_error {
                    ui.small(format!("API error: {err}"));
                }
                ui.small(format!("Config: {}", row.config));
                ui.small(format!("Logs: {}", row.logs));

                if let Some(snapshot) = self.block_snapshots.get(&row.alias) {
                    let (status_text, status_color) = snapshot.status_label();
                    ui.add_space(6.0);
                    ui.colored_label(status_color, status_text);
                    if let Some(summary) = &snapshot.last_summary {
                        ui.small(format!(
                            "Height {} • Tx {} • Rejected {} • Signatures {} • View {}",
                            summary.height,
                            summary.transaction_count,
                            summary.rejected_transaction_count,
                            summary.signature_count,
                            summary.view_change_index,
                        ));
                    } else {
                        ui.small("No block data yet.");
                    }
                    if let Some(error) = &snapshot.last_error {
                        ui.small(format!("Last block error: {error}"));
                    }
                }

                if let Some(status_snapshot) = self.status_snapshots.get(&row.alias) {
                    let (status_text, status_color) = status_snapshot.status_label();
                    ui.colored_label(status_color, status_text);
                    if let Some(delta) = status_snapshot.delta_summary() {
                        ui.small(delta);
                    }
                    if let Some(membership) = status_snapshot.membership_summary() {
                        ui.small(membership);
                    }
                    if let Some(sealed) = status_snapshot.sealed_summary() {
                        ui.colored_label(Color32::from_rgb(220, 140, 80), sealed);
                    }
                }

                if let Some(event_snapshot) = self.event_snapshots.get(&row.alias) {
                    let (status_text, status_color) = event_snapshot.status_label();
                    ui.colored_label(status_color, status_text);
                    ui.small(event_snapshot.summary_text());
                }

                if let Some(snapshot) = self.log_snapshots.get(&row.alias)
                    && !snapshot.label.is_empty()
                {
                    let ts = snapshot
                        .timestamp
                        .map(Self::format_timestamp)
                        .unwrap_or_else(|| "—".to_owned());
                    ui.colored_label(
                        snapshot.color,
                        format!("Latest log: {} ({ts})", snapshot.label),
                    );
                }

                let readiness_inflight = self.readiness_inflight.contains(&row.alias);
                if let Some(view) = self.readiness_results.get(&row.alias) {
                    ui.add_space(6.0);
                    if let Some((label, color)) = view.summary() {
                        ui.colored_label(color, label);
                    } else {
                        ui.colored_label(Color32::from_gray(170), "Readiness unknown");
                    }
                    if readiness_inflight {
                        ui.small("probe in progress…");
                    }
                    if let Some(detail) = view.detail() {
                        ui.small(detail);
                    }
                    if let Some(age) = view.age_seconds() {
                        ui.small(format!("checked {:.1}s ago", age));
                    }
                } else if readiness_inflight {
                    ui.add_space(6.0);
                    ui.colored_label(Color32::from_rgb(240, 192, 96), "Readiness checking…");
                }
            });
    }

    fn peer_state_color(state: PeerState) -> Color32 {
        let palette = Self::palette();
        match state {
            PeerState::Running => palette.success,
            PeerState::Stopped => palette.danger,
            _ => palette.warning,
        }
    }

    fn render_status_panel(
        &self,
        ui: &mut egui::Ui,
        peer_aliases: &[String],
        metrics: &DashboardMetrics,
        lane_catalog: &LaneCatalogSnapshot,
    ) {
        if self.status_history.is_empty() {
            ui.label("Waiting for telemetry updates…");
            return;
        }

        Frame::new()
            .fill(Color32::from_rgb(22, 30, 40))
            .stroke(Stroke::new(1.0, Color32::from_rgb(46, 64, 84)))
            .corner_radius(CornerRadius::same(10))
            .inner_margin(Margin::symmetric(12, 10))
            .show(ui, |ui| {
                Plot::new("mochi_queue_plot")
                    .legend(Legend::default())
                    .height(180.0)
                    .show(ui, |plot_ui| {
                        for alias in peer_aliases {
                            if let Some(points) = self.queue_plot_points(alias) {
                                plot_ui.line(Line::new(alias.clone(), points));
                            }
                        }
                    });
                ui.add_space(8.0);
                Plot::new("mochi_commit_latency_plot")
                    .legend(Legend::default())
                    .height(180.0)
                    .show(ui, |plot_ui| {
                        for alias in peer_aliases {
                            if let Some(points) = self.commit_latency_plot_points(alias) {
                                plot_ui.line(Line::new(alias.clone(), points));
                            }
                        }
                    });
                ui.add_space(8.0);
                Plot::new("mochi_throughput_plot")
                    .legend(Legend::default())
                    .height(180.0)
                    .show(ui, |plot_ui| {
                        for alias in peer_aliases {
                            if let Some(points) = self.throughput_plot_points(alias) {
                                plot_ui.line(Line::new(alias.clone(), points));
                            }
                        }
                    });
                ui.add_space(8.0);
                Plot::new("mochi_consensus_queue_plot")
                    .legend(Legend::default())
                    .height(180.0)
                    .show(ui, |plot_ui| {
                        for alias in peer_aliases {
                            if let Some(points) = self.consensus_queue_plot_points(alias) {
                                plot_ui.line(Line::new(alias.clone(), points));
                            }
                        }
                    });
                ui.add_space(8.0);
                Plot::new("mochi_view_change_plot")
                    .legend(Legend::default())
                    .height(140.0)
                    .show(ui, |plot_ui| {
                        for alias in peer_aliases {
                            if let Some(points) = self.view_change_plot_points(alias) {
                                plot_ui.line(Line::new(alias.clone(), points));
                            }
                        }
                    });
                ui.add_space(8.0);
                Plot::new("mochi_da_reschedule_plot")
                    .legend(Legend::default())
                    .height(140.0)
                    .show(ui, |plot_ui| {
                        for alias in peer_aliases {
                            if let Some(points) = self.reschedule_plot_points(alias) {
                                plot_ui.line(Line::new(alias.clone(), points));
                            }
                        }
                    });
                ui.add_space(6.0);
                ui.small(format!(
                    "Average queue: {} • Average commit latency: {} • Throughput: {}",
                    metrics.queue_text(),
                    metrics.latency_text(),
                    metrics.throughput_text()
                ));
                ui.add_space(4.0);
                for alias in peer_aliases {
                    if let Some(snapshot) = self.status_snapshots.get(alias) {
                        let (label, color) = snapshot.status_label();
                        ui.colored_label(color, format!("{alias}: {label}"));
                        if let Some(delta) = snapshot.delta_summary() {
                            ui.small(delta);
                        }
                        if let Some(sealed) = snapshot.sealed_summary() {
                            ui.colored_label(Color32::from_rgb(220, 140, 80), sealed);
                        }
                        if let Some(consensus) = snapshot.consensus_queue_summary() {
                            ui.small(consensus);
                        }
                        if let Some(storage) = snapshot.storage_summary() {
                            ui.small(storage);
                        }
                        if let Some((message, color)) = snapshot.metrics_error_label() {
                            ui.colored_label(color, message);
                        }
                        if let Some(view) = self.readiness_results.get(alias) {
                            if let Some((summary, color)) = view.summary() {
                                ui.colored_label(color, format!("{alias} readiness: {summary}"));
                            }
                            if let Some(detail) = view.detail() {
                                ui.small(detail);
                            }
                        }
                    }
                }
                ui.add_space(6.0);
                egui::CollapsingHeader::new("Lane status")
                    .default_open(false)
                    .show(ui, |ui| {
                        for alias in peer_aliases {
                            let Some(snapshot) = self.status_snapshots.get(alias) else {
                                continue;
                            };
                            let rows = snapshot.lane_status_rows(lane_catalog);
                            if rows.is_empty() {
                                continue;
                            }
                            ui.label(format!("Peer {alias}"));
                            egui::Grid::new(format!("mochi_lane_status_{alias}"))
                                .num_columns(8)
                                .striped(true)
                                .show(ui, |ui| {
                                    ui.label("Lane");
                                    ui.label("Dataspace");
                                    ui.label("Block");
                                    ui.label("Relay");
                                    ui.label("Lag");
                                    ui.label("RBC bytes");
                                    ui.label("DA cursor");
                                    ui.label("Relay ingest");
                                    ui.end_row();
                                    for row in rows {
                                        ui.label(format!("{} ({})", row.lane_id, row.alias));
                                        ui.label(&row.dataspace);
                                        ui.label(row.block_height_label());
                                        ui.label(row.relay_height_label());
                                        ui.label(row.relay_lag_label());
                                        ui.label(row.rbc_bytes_label());
                                        ui.label(row.da_cursor_label());
                                        ui.colored_label(
                                            row.relay_state.color(),
                                            row.relay_state.label(),
                                        );
                                        ui.end_row();
                                    }
                                });
                            ui.add_space(6.0);
                        }
                    });
            });
    }

    fn queue_plot_points(&self, alias: &str) -> Option<PlotPoints<'_>> {
        let history = self.status_history.get(alias)?;
        if history.len() < 2 {
            return None;
        }
        let first = history.front()?;
        let base = first.timestamp;
        let points: PlotPoints = history
            .iter()
            .map(|sample| {
                let x = sample
                    .timestamp
                    .saturating_duration_since(base)
                    .as_secs_f64();
                let y = sample.snapshot.status.queue_size as f64;
                [x, y]
            })
            .collect();
        Some(points)
    }

    fn commit_latency_plot_points(&self, alias: &str) -> Option<PlotPoints<'_>> {
        let history = self.status_history.get(alias)?;
        if history.len() < 2 {
            return None;
        }
        let first = history.front()?;
        let base = first.timestamp;
        let points: PlotPoints = history
            .iter()
            .map(|sample| {
                let x = sample
                    .timestamp
                    .saturating_duration_since(base)
                    .as_secs_f64();
                let y = sample.snapshot.metrics.commit_latency_ms as f64;
                [x, y]
            })
            .collect();
        Some(points)
    }

    fn throughput_plot_points(&self, alias: &str) -> Option<PlotPoints<'static>> {
        let history = self.status_history.get(alias)?;
        if history.len() < 2 {
            return None;
        }
        let base = history.front()?.timestamp;
        let mut previous = history.front()?;
        let mut samples = Vec::new();
        for current in history.iter().skip(1) {
            let elapsed = current
                .timestamp
                .saturating_duration_since(previous.timestamp)
                .as_secs_f64();
            if elapsed > f64::EPSILON {
                let since_base = current
                    .timestamp
                    .saturating_duration_since(base)
                    .as_secs_f64();
                let throughput = current.snapshot.metrics.tx_approved_delta as f64 / elapsed;
                samples.push(PlotPoint::new(since_base, throughput));
            }
            previous = current;
        }
        if samples.is_empty() {
            None
        } else {
            Some(PlotPoints::Owned(samples))
        }
    }

    fn consensus_queue_plot_points(&self, alias: &str) -> Option<PlotPoints<'static>> {
        self.metrics_plot_points(alias, |metrics| metrics.sumeragi_tx_queue_depth)
    }

    fn view_change_plot_points(&self, alias: &str) -> Option<PlotPoints<'static>> {
        self.status_delta_plot_points(alias, |metrics| metrics.view_change_delta.into())
    }

    fn reschedule_plot_points(&self, alias: &str) -> Option<PlotPoints<'static>> {
        self.status_delta_plot_points(alias, |metrics| metrics.da_reschedule_delta)
    }

    fn status_delta_plot_points<F>(&self, alias: &str, mut mapper: F) -> Option<PlotPoints<'static>>
    where
        F: FnMut(&StatusMetrics) -> u64,
    {
        let history = self.status_history.get(alias)?;
        if history.len() < 2 {
            return None;
        }
        let base = history.front()?.timestamp;
        let mut samples = Vec::new();
        for entry in history.iter().skip(1) {
            let metrics = &entry.snapshot.metrics;
            let magnitude = mapper(metrics);
            if magnitude == 0 {
                continue;
            }
            let elapsed = entry
                .timestamp
                .saturating_duration_since(base)
                .as_secs_f64();
            let interval_secs = (metrics.sample_interval_ms.max(1) as f64) / 1_000.0;
            let rate = magnitude as f64 / interval_secs;
            samples.push(PlotPoint::new(elapsed, rate));
        }
        if samples.is_empty() {
            None
        } else {
            Some(PlotPoints::Owned(samples))
        }
    }

    fn metrics_plot_points<F>(&self, alias: &str, mut value: F) -> Option<PlotPoints<'static>>
    where
        F: FnMut(&ToriiMetricsSnapshot) -> Option<f64>,
    {
        let history = self.status_history.get(alias)?;
        if history.is_empty() {
            return None;
        }
        let base = history.front()?.timestamp;
        let mut samples = Vec::new();
        for entry in history.iter() {
            let Some(metrics) = entry.metrics.as_ref() else {
                continue;
            };
            let Some(y) = value(metrics) else {
                continue;
            };
            let x = entry
                .timestamp
                .saturating_duration_since(base)
                .as_secs_f64();
            samples.push(PlotPoint::new(x, y));
        }
        if samples.len() < 2 {
            None
        } else {
            Some(PlotPoints::Owned(samples))
        }
    }

    fn render_activity_panel(&self, ui: &mut egui::Ui, metrics: &DashboardMetrics) {
        Frame::new()
            .fill(Color32::from_rgb(22, 30, 40))
            .stroke(Stroke::new(1.0, Color32::from_rgb(46, 64, 84)))
            .corner_radius(CornerRadius::same(10))
            .inner_margin(Margin::symmetric(12, 10))
            .show(ui, |ui| {
                ui.label(RichText::new("Recent block activity").strong());
                if self.block_events.is_empty() {
                    ui.small("No block events yet.");
                } else {
                    for event in self.block_events.iter().rev().take(5) {
                        ui.small(Self::format_block_event(event));
                    }
                }
                ui.add_space(6.0);
                ui.label(RichText::new("Recent event stream").strong());
                if self.event_events.is_empty() {
                    ui.small("No event frames yet.");
                } else {
                    for event in self.event_events.iter().rev().take(5) {
                        let line = Self::render_event_line(event);
                        ui.small(line.summary);
                    }
                }
                ui.add_space(6.0);
                ui.label(RichText::new("Latest logs").strong());
                if self.log_events.is_empty() {
                    ui.small("No logs captured yet.");
                } else {
                    for event in self.log_events.iter().rev().take(5) {
                        ui.small(Self::format_log_event(event));
                    }
                }
                ui.add_space(6.0);
                ui.small(format!(
                    "{} block frames buffered • {} event frames • {} log entries",
                    metrics.pending_block_events, metrics.pending_event_frames, metrics.stored_logs
                ));
            });
    }

    fn render_blocks_view(
        &mut self,
        ui: &mut egui::Ui,
        supervisor: &mut Supervisor,
        peer_aliases: &[String],
    ) {
        if peer_aliases.is_empty() {
            ui.label("No peers available for streaming.");
            return;
        }

        Self::ensure_selection(&mut self.selected_peer, peer_aliases);
        let mut chosen = self
            .selected_peer
            .clone()
            .unwrap_or_else(|| peer_aliases[0].clone());

        Frame::new()
            .fill(Color32::from_rgb(22, 30, 40))
            .stroke(Stroke::new(1.0, Color32::from_rgb(46, 64, 84)))
            .corner_radius(CornerRadius::same(10))
            .inner_margin(Margin::symmetric(12, 10))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Peer:");
                    ComboBox::from_id_salt("mochi_block_stream_peer_selector")
                        .selected_text(chosen.clone())
                        .show_ui(ui, |ui| {
                            for alias in peer_aliases {
                                ui.selectable_value(&mut chosen, alias.clone(), alias);
                            }
                        });
                });
            });

        if self.selected_peer.as_ref() != Some(&chosen) {
            self.selected_peer = Some(chosen.clone());
        }

        if let Some(snapshot) = self.block_snapshots.get(chosen.as_str()) {
            Frame::new()
                .fill(Color32::from_rgb(22, 30, 40))
                .stroke(Stroke::new(1.0, Color32::from_rgb(46, 64, 84)))
                .corner_radius(CornerRadius::same(10))
                .inner_margin(Margin::symmetric(12, 10))
                .show(ui, |ui| {
                    ui.label(format!(
                        "Connection: {}",
                        if snapshot.connected {
                            "active"
                        } else {
                            "inactive"
                        }
                    ));
                    if let Some(summary) = &snapshot.last_summary {
                        ui.small(format!("Last block height: {}", summary.height));
                        ui.small(format!("Tx count: {}", summary.transaction_count));
                        ui.small(format!(
                            "Rejected tx: {}",
                            summary.rejected_transaction_count
                        ));
                        ui.small(format!("Signatures: {}", summary.signature_count));
                        ui.small(format!("Result view: {}", summary.view_change_index));
                    } else {
                        ui.small("No blocks received yet.");
                    }
                    if let Some(error) = &snapshot.last_error {
                        ui.colored_label(
                            Color32::from_rgb(200, 64, 64),
                            format!("Last error: {error}"),
                        );
                    }
                });
        }

        if self.block_stream.is_none() {
            if ui
                .button(format!("Start streaming blocks from {chosen}"))
                .clicked()
            {
                let handle = self.runtime.handle().clone();
                match supervisor.managed_block_stream(&chosen, &handle) {
                    Ok(stream) => {
                        let receiver = stream.subscribe();
                        self.block_stream_peer = Some(chosen.clone());
                        self.block_receiver = Some(receiver);
                        let notice_event = BlockStreamEvent::Text {
                            text: format!(
                                "Block stream started for {chosen} (auto-reconnect enabled)."
                            ),
                        };
                        self.record_stream_event(Some(chosen.clone()), &notice_event);
                        Self::push_block_event(
                            &mut self.block_events,
                            Some(chosen.clone()),
                            notice_event,
                        );
                        self.block_stream = Some(stream);
                        self.last_error = None;
                        self.last_info = Some(format!("Block stream active for {chosen}."));
                    }
                    Err(err) => {
                        self.last_error =
                            Some(format!("Failed to start block stream for {chosen}: {err}"));
                        let error_event = BlockStreamEvent::DecodeError {
                            error: BlockStreamDecodeError {
                                stage: BlockDecodeStage::Stream,
                                raw_len: 0,
                                message: err.to_string(),
                            },
                        };
                        self.record_stream_event(Some(chosen.clone()), &error_event);
                        Self::push_block_event(
                            &mut self.block_events,
                            Some(chosen.clone()),
                            error_event,
                        );
                    }
                }
            }
        } else {
            let active_alias = self
                .block_stream_peer
                .clone()
                .unwrap_or_else(|| chosen.clone());
            ui.horizontal(|ui| {
                ui.label(format!("Streaming blocks from {active_alias}"));
                if ui.button("Stop stream").clicked() {
                    if let Some(stream) = self.block_stream.take() {
                        stream.abort();
                    }
                    self.block_receiver = None;
                    self.block_stream_peer = None;
                    let stopped_event = BlockStreamEvent::Text {
                        text: format!("Block stream stopped for {active_alias}."),
                    };
                    self.record_stream_event(Some(active_alias.clone()), &stopped_event);
                    Self::push_block_event(
                        &mut self.block_events,
                        Some(active_alias),
                        stopped_event,
                    );
                }
            });
        }

        Frame::new()
            .fill(Color32::from_rgb(22, 30, 40))
            .stroke(Stroke::new(1.0, Color32::from_rgb(46, 64, 84)))
            .corner_radius(CornerRadius::same(10))
            .inner_margin(Margin::symmetric(12, 10))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    if ui.button("Clear block buffer").clicked() {
                        self.block_events.clear();
                    }
                    ui.label(format!(
                        "Stored frames: {} (showing most recent 20)",
                        self.block_events.len()
                    ));
                });
                if self.block_events.is_empty() {
                    ui.label("No block events received yet.");
                } else {
                    ScrollArea::vertical().max_height(160.0).show(ui, |ui| {
                        for event in self.block_events.iter().rev().take(20) {
                            ui.small(Self::format_block_event(event));
                        }
                    });
                }
            });
    }

    fn render_events_view(
        &mut self,
        ui: &mut egui::Ui,
        supervisor: &mut Supervisor,
        peer_aliases: &[String],
    ) {
        if peer_aliases.is_empty() {
            ui.label("No peers available for event streaming.");
            return;
        }

        Self::ensure_selection(&mut self.event_selected_peer, peer_aliases);
        let mut chosen = self
            .event_selected_peer
            .clone()
            .unwrap_or_else(|| peer_aliases[0].clone());

        Frame::new()
            .fill(Color32::from_rgb(22, 30, 40))
            .stroke(Stroke::new(1.0, Color32::from_rgb(46, 64, 84)))
            .corner_radius(CornerRadius::same(10))
            .inner_margin(Margin::symmetric(12, 10))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Peer:");
                    ComboBox::from_id_salt("mochi_event_stream_peer_selector")
                        .selected_text(chosen.clone())
                        .show_ui(ui, |ui| {
                            for alias in peer_aliases {
                                ui.selectable_value(&mut chosen, alias.clone(), alias);
                            }
                        });
                });
            });

        if self.event_selected_peer.as_ref() != Some(&chosen) {
            self.event_selected_peer = Some(chosen.clone());
        }

        if let Some(snapshot) = self.event_snapshots.get(chosen.as_str()) {
            Frame::new()
                .fill(Color32::from_rgb(22, 30, 40))
                .stroke(Stroke::new(1.0, Color32::from_rgb(46, 64, 84)))
                .corner_radius(CornerRadius::same(10))
                .inner_margin(Margin::symmetric(12, 10))
                .show(ui, |ui| {
                    ui.label(format!(
                        "Connection: {}",
                        if snapshot.connected {
                            "active"
                        } else {
                            "inactive"
                        }
                    ));
                    if let Some(summary) = &snapshot.last_summary {
                        ui.small(format!(
                            "Last event: {} — {}",
                            summary.category.label(),
                            summary.label
                        ));
                        if let Some(detail) = &summary.detail {
                            ui.small(truncate(detail, 160));
                        }
                    } else {
                        ui.small("No events received yet.");
                    }
                    if let Some(error) = &snapshot.last_error {
                        ui.colored_label(
                            Color32::from_rgb(200, 64, 64),
                            format!("Last error: {error}"),
                        );
                    }
                });
        }

        if self.event_stream.is_none() {
            if ui
                .button(format!("Start streaming events from {chosen}"))
                .clicked()
            {
                let handle = self.runtime.handle().clone();
                match supervisor.managed_event_stream(&chosen, &handle) {
                    Ok(stream) => {
                        let receiver = stream.subscribe();
                        self.event_stream_peer = Some(chosen.clone());
                        self.event_receiver = Some(receiver);
                        let notice = EventStreamEvent::Text {
                            text: format!(
                                "Event stream started for {chosen} (auto-reconnect enabled)."
                            ),
                        };
                        self.record_event_stream_event(Some(chosen.clone()), &notice);
                        Self::push_event_event(
                            &mut self.event_events,
                            Some(chosen.clone()),
                            notice,
                        );
                        self.event_stream = Some(stream);
                        self.last_error = None;
                        self.last_info = Some(format!("Event stream active for {chosen}."));
                    }
                    Err(err) => {
                        self.last_error =
                            Some(format!("Failed to start event stream for {chosen}: {err}"));
                        let error_event = EventStreamEvent::DecodeError {
                            error: EventStreamDecodeError {
                                stage: EventDecodeStage::Stream,
                                raw_len: 0,
                                message: err.to_string(),
                            },
                        };
                        self.record_event_stream_event(Some(chosen.clone()), &error_event);
                        Self::push_event_event(
                            &mut self.event_events,
                            Some(chosen.clone()),
                            error_event,
                        );
                    }
                }
            }
        } else {
            let active_alias = self
                .event_stream_peer
                .clone()
                .unwrap_or_else(|| chosen.clone());
            ui.horizontal(|ui| {
                ui.label(format!("Streaming events from {active_alias}"));
                if ui.button("Stop stream").clicked() {
                    if let Some(stream) = self.event_stream.take() {
                        stream.abort();
                    }
                    self.event_receiver = None;
                    self.event_stream_peer = None;
                    let stopped_event = EventStreamEvent::Text {
                        text: format!("Event stream stopped for {active_alias}."),
                    };
                    self.record_event_stream_event(Some(active_alias.clone()), &stopped_event);
                    Self::push_event_event(
                        &mut self.event_events,
                        Some(active_alias),
                        stopped_event,
                    );
                }
            });
        }

        let rendered_events: Vec<RenderedEventLine> = self
            .event_events
            .iter()
            .map(Self::render_event_line)
            .collect();
        let mut alias_set: BTreeSet<String> = peer_aliases.iter().cloned().collect();
        alias_set.extend(rendered_events.iter().map(|line| line.alias.clone()));
        let alias_options: Vec<String> = alias_set.into_iter().collect();

        Frame::new()
            .fill(Color32::from_rgb(22, 30, 40))
            .stroke(Stroke::new(1.0, Color32::from_rgb(46, 64, 84)))
            .corner_radius(CornerRadius::same(10))
            .inner_margin(Margin::symmetric(12, 10))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Search:");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.event_filter.search)
                            .desired_width(220.0),
                    );
                    if ui.button("Reset filters").clicked() {
                        self.event_filter.reset();
                    }
                });
                ui.add_space(6.0);
                ui.horizontal_wrapped(|ui| {
                    ui.label("Categories:");
                    ui.checkbox(&mut self.event_filter.show_pipeline, "Pipeline");
                    ui.checkbox(&mut self.event_filter.show_data, "Data");
                    ui.checkbox(&mut self.event_filter.show_time, "Time");
                    ui.checkbox(
                        &mut self.event_filter.show_execute_trigger,
                        "Execute Trigger",
                    );
                    ui.checkbox(
                        &mut self.event_filter.show_trigger_completed,
                        "Trigger Completed",
                    );
                });
                ui.add_space(4.0);
                ui.horizontal_wrapped(|ui| {
                    ui.label("Stream frames:");
                    ui.checkbox(&mut self.event_filter.show_text, "Text");
                    ui.checkbox(&mut self.event_filter.show_decode_errors, "Decode errors");
                    ui.checkbox(&mut self.event_filter.show_lagged, "Lagged");
                    ui.checkbox(&mut self.event_filter.show_closed, "Closed");
                });
                if !alias_options.is_empty() {
                    ui.add_space(4.0);
                    ui.horizontal_wrapped(|ui| {
                        ui.label("Peers:");
                        let has_filters = self.event_filter.has_alias_filters();
                        if ui
                            .add_enabled(has_filters, egui::Button::new("Show all"))
                            .clicked()
                        {
                            self.event_filter.clear_aliases();
                        }
                        for alias in &alias_options {
                            let mut enabled = self.event_filter.alias_selected(alias);
                            if ui.checkbox(&mut enabled, alias).changed() {
                                self.event_filter.toggle_alias(alias, enabled);
                            }
                        }
                    });
                }
            });

        let query = self.event_filter.normalized_query();
        let matching_indices: Vec<usize> = rendered_events
            .iter()
            .enumerate()
            .filter(|(_, line)| self.event_filter.matches(line, query.as_deref()))
            .map(|(idx, _)| idx)
            .collect();
        let total_matching = matching_indices.len();
        let mut display_items: Vec<(usize, &RenderedEventLine)> = matching_indices
            .iter()
            .rev()
            .take(20)
            .map(|idx| (*idx, &rendered_events[*idx]))
            .collect();
        let text_export_enabled = total_matching > 0;
        let json_export_enabled = matching_indices.iter().any(|idx| {
            matches!(
                self.event_events.get(*idx).map(|entry| &entry.event),
                Some(EventStreamEvent::Event { .. })
            )
        });

        Frame::new()
            .fill(Color32::from_rgb(22, 30, 40))
            .stroke(Stroke::new(1.0, Color32::from_rgb(46, 64, 84)))
            .corner_radius(CornerRadius::same(10))
            .inner_margin(Margin::symmetric(12, 10))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    if ui.button("Clear event buffer").clicked() {
                        self.event_events.clear();
                        display_items.clear();
                    }
                    if ui
                        .add_enabled(
                            text_export_enabled,
                            egui::Button::new("Copy filtered (text)"),
                        )
                        .clicked()
                    {
                        let export = collect_event_text(&matching_indices, &rendered_events);
                        if export.is_empty() {
                            self.last_error = Some(
                                "No events matched the current filters; nothing copied.".to_owned(),
                            );
                        } else {
                            Self::copy_text(ui, export);
                            self.last_info =
                                Some("Filtered events copied to clipboard.".to_owned());
                        }
                    }
                    if ui
                        .add_enabled(
                            json_export_enabled,
                            egui::Button::new("Copy filtered (JSON)"),
                        )
                        .clicked()
                    {
                        match collect_event_json(&matching_indices, &self.event_events) {
                            Ok(json) => {
                                Self::copy_text(ui, json);
                                self.last_info =
                                    Some("Filtered structured events copied as JSON.".to_owned());
                            }
                            Err(err) => {
                                self.last_error = Some(err);
                            }
                        }
                    }
                    let showing = display_items.len();
                    ui.label(format!(
                        "Stored frames: {} (matching {total_matching}, showing newest {showing})",
                        self.event_events.len()
                    ));
                });
                if display_items.is_empty() {
                    ui.label(if self.event_events.is_empty() {
                        "No event frames received yet."
                    } else {
                        "No event frames match the current filters."
                    });
                } else {
                    ScrollArea::vertical().max_height(220.0).show(ui, |ui| {
                        ui.spacing_mut().item_spacing.y = 6.0;
                        for (idx, line) in display_items.drain(..) {
                            let entry = self.event_events[idx].clone();
                            self.render_event_entry(
                                ui,
                                &entry,
                                line,
                                "Event summary copied to clipboard.",
                            );
                            ui.separator();
                        }
                    });
                }
            });
    }

    fn render_state_entry(&mut self, ui: &mut egui::Ui, entry: StateEntry) {
        Frame::new()
            .fill(Color32::from_rgb(20, 28, 38))
            .stroke(Stroke::new(1.0, Color32::from_rgb(46, 64, 84)))
            .corner_radius(CornerRadius::same(8))
            .inner_margin(Margin::symmetric(12, 8))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    if ui
                        .small_button("Copy summary")
                        .on_hover_text("Copy the summary lines to the clipboard")
                        .clicked()
                    {
                        let mut summary =
                            format!("{}\n{}\n{}", entry.title, entry.subtitle, entry.detail);
                        if let Some(domain) = &entry.domain {
                            summary.push_str(&format!("\nDomain: {domain}"));
                        }
                        if let Some(owner) = &entry.owner {
                            summary.push_str(&format!("\nOwner: {owner}"));
                        }
                        if let Some(definition) = &entry.asset_definition {
                            summary.push_str(&format!("\nDefinition: {definition}"));
                        }
                        Self::copy_text(ui, summary);
                        self.last_info =
                            Some("State entry summary copied to clipboard.".to_owned());
                    }
                    if let Some(json) = entry.json.as_ref()
                        && ui
                            .small_button("Copy JSON")
                            .on_hover_text("Copy the Norito JSON payload")
                            .clicked()
                    {
                        Self::copy_text(ui, json.clone());
                        self.last_info = Some("State entry JSON copied to clipboard.".to_owned());
                    }
                    if let Some(bytes) = entry.norito_bytes.as_ref()
                        && ui
                            .small_button("Copy Norito")
                            .on_hover_text("Copy the Norito-encoded bytes (hex)")
                            .clicked()
                    {
                        let hex = encode_upper(bytes);
                        Self::copy_text(ui, hex);
                        self.last_info =
                            Some("State entry Norito bytes copied to clipboard.".to_owned());
                    }
                });

                ui.add_space(4.0);
                ui.label(RichText::new(&entry.title).strong());
                ui.small(&entry.subtitle);
                ui.small(&entry.detail);

                if entry.domain.is_some()
                    || entry.owner.is_some()
                    || entry.asset_definition.is_some()
                {
                    ui.add_space(4.0);
                    ui.horizontal_wrapped(|ui| {
                        if let Some(domain) = &entry.domain {
                            ui.label(
                                RichText::new(format!("Domain: {domain}"))
                                    .small()
                                    .color(Color32::from_rgb(150, 190, 255)),
                            );
                        }
                        if let Some(owner) = &entry.owner {
                            ui.label(
                                RichText::new(format!("Owner: {owner}"))
                                    .small()
                                    .color(Color32::from_rgb(160, 220, 160)),
                            );
                        }
                        if let Some(definition) = &entry.asset_definition {
                            ui.label(
                                RichText::new(format!("Definition: {definition}"))
                                    .small()
                                    .color(Color32::from_rgb(210, 180, 255)),
                            );
                        }
                    });
                }

                if let Some(json) = &entry.json {
                    ui.collapsing("JSON", |ui| {
                        ui.monospace(json);
                    });
                }

                ui.collapsing("Raw debug", |ui| {
                    ui.monospace(&entry.raw);
                });
            });
    }

    fn state_entry_matches(
        entry: &StateEntry,
        kind: StateQueryKind,
        search: Option<&str>,
        domain_filter: Option<&str>,
        owner_filter: Option<&str>,
        definition_filter: Option<&str>,
    ) -> bool {
        if let Some(query) = search
            && !entry.search_blob.contains(query)
        {
            return false;
        }

        if let Some(filter) = domain_filter {
            match entry.domain_lower.as_deref() {
                Some(domain) if domain.contains(filter) => {}
                _ => return false,
            }
        }

        if matches!(
            kind,
            StateQueryKind::Assets | StateQueryKind::AssetDefinitions | StateQueryKind::Domains
        ) && let Some(filter) = owner_filter
        {
            match entry.owner_lower.as_deref() {
                Some(owner) if owner.contains(filter) => {}
                _ => return false,
            }
        }

        if matches!(kind, StateQueryKind::Assets)
            && let Some(filter) = definition_filter
        {
            match entry.asset_definition_lower.as_deref() {
                Some(definition) if definition.contains(filter) => {}
                _ => return false,
            }
        }

        true
    }

    fn render_logs_view(
        &mut self,
        ui: &mut egui::Ui,
        supervisor: &mut Supervisor,
        peer_aliases: &[String],
    ) {
        if peer_aliases.is_empty() {
            ui.label("No peers available.");
            return;
        }

        Self::ensure_selection(&mut self.log_selected_peer, peer_aliases);
        let mut chosen = self
            .log_selected_peer
            .clone()
            .unwrap_or_else(|| peer_aliases[0].clone());

        Frame::new()
            .fill(Color32::from_rgb(22, 30, 40))
            .stroke(Stroke::new(1.0, Color32::from_rgb(46, 64, 84)))
            .corner_radius(CornerRadius::same(10))
            .inner_margin(Margin::symmetric(12, 10))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Peer:");
                    ComboBox::from_id_salt("mochi_log_peer_selector")
                        .selected_text(chosen.clone())
                        .show_ui(ui, |ui| {
                            for alias in peer_aliases {
                                ui.selectable_value(&mut chosen, alias.clone(), alias);
                            }
                        });
                });
            });

        if self.log_selected_peer.as_ref() != Some(&chosen) {
            self.log_selected_peer = Some(chosen.clone());
        }

        ui.horizontal(|ui| {
            if ui.button("Clear log buffer").clicked() {
                self.log_events.clear();
                self.log_snapshots.clear();
            }
            let follow_active = ui
                .add_enabled(
                    self.block_stream_peer.is_some(),
                    egui::Button::new("Follow active peer"),
                )
                .clicked();
            if follow_active {
                self.log_selected_peer = self.block_stream_peer.clone();
            }
            ui.label("Filter:");
            let response = egui::TextEdit::singleline(&mut self.log_filter).desired_width(200.0);
            ui.add(response);
            if ui.button("Clear filter").clicked() {
                self.log_filter.clear();
            }
        });

        if self.log_receiver.is_none() {
            if ui
                .button(format!("Start tailing logs from {chosen}"))
                .clicked()
                && !chosen.is_empty()
            {
                match supervisor.log_stream(&chosen) {
                    Some(stream) => {
                        let receiver = stream.subscribe();
                        self.log_receiver = Some(receiver);
                        self.log_stream_peer = Some(chosen.clone());
                        self.log_selected_peer = Some(chosen.clone());
                        let notice = Self::system_log_event(
                            &chosen,
                            format!("Log stream started for {chosen}."),
                        );
                        self.push_log_event(notice);
                        self.last_error = None;
                        self.last_info = Some(format!("Log stream active for {chosen}."));
                    }
                    None => {
                        self.last_error = Some(format!("No log stream available for {chosen}"));
                    }
                }
            }
        } else {
            let active_alias = self
                .log_stream_peer
                .clone()
                .unwrap_or_else(|| chosen.clone());
            ui.horizontal(|ui| {
                ui.label(format!("Tailing logs from {active_alias}"));
                if ui.button("Stop logs").clicked() {
                    self.stop_log_stream(Some(active_alias.clone()));
                }
                if !chosen.is_empty()
                    && chosen != active_alias
                    && ui.button(format!("Switch to {chosen}")).clicked()
                {
                    self.stop_log_stream(Some(active_alias.clone()));
                    match supervisor.log_stream(&chosen) {
                        Some(stream) => {
                            let receiver = stream.subscribe();
                            self.log_receiver = Some(receiver);
                            self.log_stream_peer = Some(chosen.clone());
                            self.log_selected_peer = Some(chosen.clone());
                            let notice = Self::system_log_event(
                                &chosen,
                                format!("Log stream started for {chosen}."),
                            );
                            self.push_log_event(notice);
                            self.last_error = None;
                            self.last_info = Some(format!("Log stream active for {chosen}."));
                        }
                        None => {
                            self.last_error = Some(format!("No log stream available for {chosen}"));
                        }
                    }
                }
            });
        }

        let display_alias = self
            .log_stream_peer
            .clone()
            .or_else(|| self.log_selected_peer.clone());
        let filter_query = self.log_filter.trim().to_ascii_lowercase();
        let filter_empty = filter_query.is_empty();
        let filtered_logs: Vec<(usize, String)> = self
            .log_events
            .iter()
            .enumerate()
            .filter_map(|(idx, event)| {
                if let Some(alias) = display_alias.as_deref()
                    && Self::event_alias(event) != alias
                {
                    return None;
                }
                if !self.is_log_kind_enabled(Self::log_event_kind(event)) {
                    return None;
                }
                let formatted = Self::format_log_event(event);
                if !filter_empty && !formatted.to_ascii_lowercase().contains(&filter_query) {
                    return None;
                }
                Some((idx, formatted))
            })
            .collect();

        if !filtered_logs.is_empty() {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                if ui
                    .small_button("Copy filtered logs")
                    .on_hover_text("Copy matching log lines to the clipboard")
                    .clicked()
                {
                    match collect_log_text(&filtered_logs) {
                        Ok(text) => {
                            Self::copy_text(ui, text);
                            self.last_info =
                                Some(format!("Copied {} log entries.", filtered_logs.len()));
                            self.last_error = None;
                        }
                        Err(err) => {
                            self.last_info = None;
                            self.last_error = Some(err);
                        }
                    }
                }
                if ui
                    .small_button("Copy latest 50")
                    .on_hover_text("Copy the newest 50 matching log lines")
                    .clicked()
                {
                    let count = filtered_logs.len().min(50);
                    let slice = &filtered_logs[filtered_logs.len() - count..];
                    match collect_log_text(slice) {
                        Ok(text) => {
                            Self::copy_text(ui, text);
                            self.last_info = Some(format!("Copied {count} recent log entries."));
                            self.last_error = None;
                        }
                        Err(err) => {
                            self.last_info = None;
                            self.last_error = Some(err);
                        }
                    }
                }
                if ui
                    .small_button("Save filtered logs")
                    .on_hover_text("Write matching log lines to a temporary export file")
                    .clicked()
                {
                    match save_logs_to_file(&filtered_logs, self.log_export_dir.as_deref()) {
                        Ok(path) => {
                            self.last_info = Some(format!(
                                "Saved {} log entries to {}",
                                filtered_logs.len(),
                                path.display()
                            ));
                            self.last_error = None;
                        }
                        Err(err) => {
                            self.last_info = None;
                            self.last_error = Some(err);
                        }
                    }
                }
                let visible = filtered_logs.len().min(60);
                ui.small(format!(
                    "Matching log entries: {} (showing newest {})",
                    filtered_logs.len(),
                    visible
                ));
            });
        }

        Frame::new()
            .fill(Color32::from_rgb(22, 30, 40))
            .stroke(Stroke::new(1.0, Color32::from_rgb(46, 64, 84)))
            .corner_radius(CornerRadius::same(10))
            .inner_margin(Margin::symmetric(12, 10))
            .show(ui, |ui| {
                if self.log_events.is_empty() {
                    ui.label("No log entries received yet.");
                    return;
                }
                if filtered_logs.is_empty() {
                    ui.label("No log entries match the current filters.");
                    return;
                }
                ScrollArea::vertical().max_height(180.0).show(ui, |ui| {
                    for (idx, text) in filtered_logs.iter().rev().take(60) {
                        let event = &self.log_events[*idx];
                        ui.colored_label(Self::log_event_color(event), text);
                    }
                });
            });
    }

    fn render_state_view(
        &mut self,
        ui: &mut egui::Ui,
        supervisor: &mut Supervisor,
        peer_aliases: &[String],
    ) {
        ui.heading("State explorer");
        if peer_aliases.is_empty() {
            ui.label("No peers available.");
            return;
        }

        Self::ensure_selection(&mut self.state_selected_peer, peer_aliases);

        let mut peer_changed = false;
        let mut kind_changed = false;
        let mut run_query_reset = false;

        let query_kinds = StateQueryKind::all();
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            let last_index = query_kinds.len().saturating_sub(1);
            for (index, kind) in query_kinds.iter().enumerate() {
                let selected = self.state_query_kind == *kind;
                let rounding = CornerRadius {
                    nw: if index == 0 { 8 } else { 0 },
                    ne: if index == last_index { 8 } else { 0 },
                    sw: if index == 0 { 8 } else { 0 },
                    se: if index == last_index { 8 } else { 0 },
                };
                let fill = if selected {
                    Color32::from_rgb(46, 64, 84)
                } else {
                    Color32::from_rgb(18, 26, 36)
                };
                let stroke_color = if selected {
                    Color32::from_rgb(110, 160, 220)
                } else {
                    Color32::from_rgb(46, 64, 84)
                };
                Frame::new()
                    .fill(fill)
                    .stroke(Stroke::new(1.0, stroke_color))
                    .corner_radius(rounding)
                    .inner_margin(Margin::symmetric(14, 8))
                    .show(ui, |ui| {
                        if ui.selectable_label(selected, kind.label()).clicked()
                            && self.state_query_kind != *kind
                        {
                            self.state_query_kind = *kind;
                            kind_changed = true;
                        }
                    });
                if index != last_index {
                    ui.add_space(4.0);
                }
            }
        });

        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.label("Peer");
            let current_label = self
                .state_selected_peer
                .as_deref()
                .unwrap_or(peer_aliases.first().map(String::as_str).unwrap_or("—"));
            ComboBox::from_id_salt("mochi_state_peer_selector")
                .selected_text(current_label)
                .show_ui(ui, |ui| {
                    for alias in peer_aliases {
                        let selected = self.state_selected_peer.as_deref() == Some(alias.as_str());
                        if ui.selectable_label(selected, alias).clicked()
                            && self.state_selected_peer.as_deref() != Some(alias.as_str())
                        {
                            self.state_selected_peer = Some(alias.clone());
                            peer_changed = true;
                        }
                    }
                });
        });

        let kind = self.state_query_kind;

        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.label("Fetch size");
            ui.add(
                egui::DragValue::new(&mut self.state_fetch_size)
                    .range(0..=1000)
                    .speed(1.0),
            );
            ui.add_space(12.0);
            if ui.button("Run query").clicked() {
                run_query_reset = true;
            }
            if self.state_tabs.get(kind).loading {
                ui.add(egui::Spinner::new());
            }
        });

        if peer_changed {
            self.state_tabs.reset_results_for_all();
        }

        if kind_changed {
            self.state_tabs.get_mut(kind).filter.adapt_to_kind(kind);
        }

        if peer_changed || kind_changed {
            run_query_reset = true;
        }

        {
            let tab = self.state_tabs.get_mut(kind);
            let mut domain_options = BTreeSet::new();
            let mut owner_options = BTreeSet::new();
            let mut definition_options = BTreeSet::new();
            let option_source: Box<dyn Iterator<Item = &StateEntry> + '_> = if tab.pages.is_empty()
            {
                Box::new(tab.entries.iter())
            } else {
                Box::new(tab.pages.iter().flat_map(|page| page.entries.iter()))
            };
            for entry in option_source {
                if let Some(domain) = &entry.domain {
                    domain_options.insert(domain.clone());
                }
                if let Some(owner) = &entry.owner {
                    owner_options.insert(owner.clone());
                }
                if let Some(definition) = &entry.asset_definition {
                    definition_options.insert(definition.clone());
                }
            }
            let domain_options: Vec<String> = domain_options.into_iter().collect();
            let owner_options: Vec<String> = owner_options.into_iter().collect();
            let definition_options: Vec<String> = definition_options.into_iter().collect();

            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.label("Search");
                ui.text_edit_singleline(&mut tab.filter.search);
                if ui.button("Reset filters").clicked() {
                    tab.filter.clear();
                    tab.filter.adapt_to_kind(kind);
                }
            });

            match kind {
                StateQueryKind::Accounts => {
                    render_filter_selector(ui, "Domain", &mut tab.filter.domain, &domain_options);
                }
                StateQueryKind::Assets => {
                    render_filter_selector(ui, "Domain", &mut tab.filter.domain, &domain_options);
                    render_filter_selector(ui, "Owner", &mut tab.filter.owner, &owner_options);
                    render_filter_selector(
                        ui,
                        "Definition",
                        &mut tab.filter.asset_definition,
                        &definition_options,
                    );
                }
                StateQueryKind::AssetDefinitions => {
                    render_filter_selector(ui, "Domain", &mut tab.filter.domain, &domain_options);
                    render_filter_selector(ui, "Owner", &mut tab.filter.owner, &owner_options);
                    tab.filter.asset_definition.clear();
                }
                StateQueryKind::Domains => {
                    render_filter_selector(ui, "Domain", &mut tab.filter.domain, &domain_options);
                    render_filter_selector(ui, "Owner", &mut tab.filter.owner, &owner_options);
                    tab.filter.asset_definition.clear();
                }
                StateQueryKind::Peers => {
                    tab.filter.domain.clear();
                    tab.filter.owner.clear();
                    tab.filter.asset_definition.clear();
                    ui.label("Peer queries can be filtered via search only.");
                }
            }
        }

        let snapshot = self.build_state_snapshot(kind);

        let mut fetch_next_page = false;
        let mut select_page: Option<usize> = None;
        let mut export_action: Option<StateExportAction> = None;

        ui.add_space(6.0);
        if snapshot.page_count > 0 {
            let mut summary = format!(
                "Page {} of {} • on page {}/{}",
                snapshot.current_page + 1,
                snapshot.page_count,
                snapshot.filtered_indices.len(),
                snapshot.total_entries,
            );
            if snapshot.filters_active {
                summary.push_str(&format!(
                    " • cached matches {}/{}",
                    snapshot.matching_cached_entries.len(),
                    snapshot.total_cached
                ));
            } else {
                summary.push_str(&format!(" • cached {}", snapshot.total_cached));
            }
            if let Some(remaining) = snapshot.remaining {
                summary.push_str(&format!(" • remaining {remaining}"));
            }
            ui.small(summary);
        } else if let Some(remaining) = snapshot.remaining {
            ui.small(format!("Remaining items: {remaining}"));
        }

        if let Some(error) = &snapshot.error {
            ui.colored_label(Color32::from_rgb(200, 64, 64), error);
        }

        if snapshot.page_count == 0 {
            if snapshot.loading {
                ui.add(egui::Spinner::new());
            } else if snapshot.error.is_none() {
                ui.label("No entries yet. Run a query to inspect state.");
            }
        } else {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                let has_prev = snapshot.current_page > 0;
                let has_cached_next = snapshot.current_page + 1 < snapshot.page_count;
                let can_fetch_next = snapshot.has_cursor && !snapshot.loading && !has_cached_next;
                if ui
                    .add_enabled(
                        has_prev && !snapshot.loading,
                        egui::Button::new("Previous page"),
                    )
                    .clicked()
                {
                    let new_index = snapshot.current_page.saturating_sub(1);
                    select_page = Some(new_index);
                }
                if ui
                    .add_enabled(
                        has_cached_next && !snapshot.loading,
                        egui::Button::new("Next page"),
                    )
                    .clicked()
                {
                    let new_index =
                        (snapshot.current_page + 1).min(snapshot.page_count.saturating_sub(1));
                    select_page = Some(new_index);
                }
                if ui
                    .add_enabled(can_fetch_next, egui::Button::new("Fetch next page"))
                    .clicked()
                {
                    fetch_next_page = true;
                }
                if snapshot.loading {
                    ui.add(egui::Spinner::new());
                }
            });

            if !snapshot.matching_cached_entries.is_empty() {
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    if ui
                        .small_button("Copy filtered JSON")
                        .on_hover_text("Copy matching entries as a Norito JSON array")
                        .clicked()
                    {
                        export_action = Some(StateExportAction::CopyJson(
                            snapshot.matching_cached_entries.clone(),
                        ));
                    }
                    if ui
                        .small_button("Copy filtered Norito")
                        .on_hover_text("Copy matching entries as Norito-encoded hex")
                        .clicked()
                    {
                        export_action = Some(StateExportAction::CopyNorito(
                            snapshot.matching_cached_entries.clone(),
                        ));
                    }
                    if ui
                        .small_button("Save filtered JSON")
                        .on_hover_text("Write matching entries to a Norito JSON file")
                        .clicked()
                    {
                        export_action = Some(StateExportAction::SaveJson(
                            snapshot.matching_cached_entries.clone(),
                        ));
                    }
                    if ui
                        .small_button("Save filtered Norito")
                        .on_hover_text("Write matching entries as Norito bytes to disk")
                        .clicked()
                    {
                        export_action = Some(StateExportAction::SaveNorito(
                            snapshot.matching_cached_entries.clone(),
                        ));
                    }
                });
            }

            ui.add_space(6.0);
            if snapshot.filters_active {
                if snapshot.matching_cached_entries.is_empty() {
                    ui.label("No cached entries match the current filters.");
                } else {
                    ScrollArea::vertical()
                        .auto_shrink([false; 2])
                        .show(ui, |ui| {
                            ui.spacing_mut().item_spacing.y = 8.0;
                            for entry in &snapshot.matching_cached_entries {
                                self.render_state_entry(ui, entry.clone());
                            }
                        });
                }
            } else if snapshot.entries_on_page.is_empty() {
                ui.label("No entries on this page.");
            } else if snapshot.filtered_indices.is_empty() {
                ui.label("No entries match the current filters.");
            } else {
                ScrollArea::vertical()
                    .auto_shrink([false; 2])
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing.y = 8.0;
                        for idx in snapshot.filtered_indices.iter().copied() {
                            if let Some(entry) = snapshot.entries_on_page.get(idx) {
                                self.render_state_entry(ui, entry.clone());
                            }
                        }
                    });
            }
        }

        if run_query_reset {
            self.request_state_page(supervisor, true);
        } else if fetch_next_page {
            self.request_state_page(supervisor, false);
        }
        if !run_query_reset && let Some(index) = select_page {
            self.select_state_page(kind, index);
        }
        if let Some(action) = export_action {
            self.apply_state_export_action(ui, action);
        }
    }

    fn build_state_snapshot(&self, kind: StateQueryKind) -> StateRenderSnapshot {
        let tab = self.state_tabs.get(kind);
        let search_query = tab.filter.search_query();
        let domain_filter = tab.filter.normalized_domain();
        let owner_filter = tab.filter.normalized_owner();
        let definition_filter = tab.filter.normalized_definition();
        let (filtered_indices, matching_cached_refs) = filter_state_entries(
            &tab.pages,
            &tab.entries,
            kind,
            search_query.as_deref(),
            domain_filter.as_deref(),
            owner_filter.as_deref(),
            definition_filter.as_deref(),
        );
        let matching_cached_entries = matching_cached_refs
            .iter()
            .map(|entry| (*entry).clone())
            .collect::<Vec<_>>();
        let filters_active = search_query.is_some()
            || domain_filter.is_some()
            || owner_filter.is_some()
            || definition_filter.is_some();
        StateRenderSnapshot {
            entries_on_page: tab.entries.clone(),
            matching_cached_entries,
            filtered_indices,
            filters_active,
            loading: tab.loading,
            error: tab.error.clone(),
            remaining: tab.remaining,
            current_page: tab.current_page,
            page_count: tab.pages.len(),
            total_entries: tab.entries.len(),
            total_cached: tab.total_cached(),
            has_cursor: tab.has_cursor(),
        }
    }

    fn apply_state_export_action(&mut self, ui: &mut egui::Ui, action: StateExportAction) {
        match action {
            StateExportAction::CopyJson(entries) => {
                let count = entries.len();
                let refs: Vec<&StateEntry> = entries.iter().collect();
                match collect_state_json(&refs) {
                    Ok(json_text) => {
                        Self::copy_text(ui, json_text);
                        self.last_info = Some(format!("Copied {count} state entries as JSON."));
                        self.last_error = None;
                    }
                    Err(err) => {
                        self.last_info = None;
                        self.last_error = Some(err);
                    }
                }
            }
            StateExportAction::CopyNorito(entries) => {
                let count = entries.len();
                let refs: Vec<&StateEntry> = entries.iter().collect();
                match collect_state_norito(&refs) {
                    Ok(hex_dump) => {
                        Self::copy_text(ui, hex_dump);
                        self.last_info =
                            Some(format!("Copied {count} state entries as Norito bytes."));
                        self.last_error = None;
                    }
                    Err(err) => {
                        self.last_info = None;
                        self.last_error = Some(err);
                    }
                }
            }
            StateExportAction::SaveJson(entries) => {
                let count = entries.len();
                let refs: Vec<&StateEntry> = entries.iter().collect();
                match save_state_json_to_file(&refs, self.state_export_dir.as_deref()) {
                    Ok(path) => {
                        self.last_info = Some(format!(
                            "Saved {count} state entries to {}.",
                            path.display()
                        ));
                        self.last_error = None;
                    }
                    Err(err) => {
                        self.last_info = None;
                        self.last_error = Some(err);
                    }
                }
            }
            StateExportAction::SaveNorito(entries) => {
                let count = entries.len();
                let refs: Vec<&StateEntry> = entries.iter().collect();
                match save_state_norito_to_file(&refs, self.state_export_dir.as_deref()) {
                    Ok(path) => {
                        self.last_info = Some(format!(
                            "Saved {count} state entries to {}.",
                            path.display()
                        ));
                        self.last_error = None;
                    }
                    Err(err) => {
                        self.last_info = None;
                        self.last_error = Some(err);
                    }
                }
            }
        }
    }

    fn render_composer_view(
        &mut self,
        ui: &mut egui::Ui,
        supervisor: &Supervisor,
        peer_aliases: &[String],
    ) {
        ui.heading("Transaction composer");
        if peer_aliases.is_empty() {
            ui.label("No peers available.");
            return;
        }

        let signers = supervisor.signers();
        if signers.is_empty() {
            ui.label("No signing authorities available.");
            return;
        }

        Self::ensure_selection(&mut self.composer_selected_peer, peer_aliases);
        Self::ensure_signer_selection(&mut self.composer_selected_signer, signers);

        if self.composer_chain_id.trim().is_empty() {
            self.composer_chain_id = supervisor.chain_id().to_owned();
        }

        ui.horizontal(|ui| {
            ui.label("Peer");
            let current_label = self
                .composer_selected_peer
                .as_deref()
                .unwrap_or_else(|| peer_aliases.first().map(String::as_str).unwrap_or("—"));
            ComboBox::from_id_salt("mochi_composer_peer_selector")
                .selected_text(current_label)
                .show_ui(ui, |ui| {
                    for alias in peer_aliases {
                        let alias_str = alias.as_str();
                        let selected = self.composer_selected_peer.as_deref() == Some(alias_str);
                        if ui.selectable_label(selected, alias_str).clicked()
                            && self.composer_selected_peer.as_deref() != Some(alias_str)
                        {
                            self.composer_selected_peer = Some(alias.clone());
                        }
                    }
                });
        });
        ui.add_space(6.0);

        ui.horizontal(|ui| {
            ui.label("Signing authority");
            let current_label = self
                .composer_selected_signer
                .and_then(|index| signers.get(index))
                .map(SigningAuthority::label)
                .unwrap_or("—");
            ComboBox::from_id_salt("mochi_composer_signer_selector")
                .selected_text(current_label)
                .show_ui(ui, |ui| {
                    for (index, signer) in signers.iter().enumerate() {
                        let selected = self.composer_selected_signer == Some(index);
                        if ui.selectable_label(selected, signer.label()).clicked()
                            && self.composer_selected_signer != Some(index)
                        {
                            self.composer_selected_signer = Some(index);
                            self.composer_preview = None;
                            self.composer_submit_error = None;
                            self.composer_submit_success = None;
                        }
                    }
                });
        });
        if let Some(selected) = self
            .composer_selected_signer
            .and_then(|index| signers.get(index))
        {
            let permissions = selected
                .permissions()
                .map(|permission| permission.label())
                .collect::<Vec<_>>()
                .join(", ");
            let roles = selected
                .roles()
                .map(|role| role.to_string())
                .collect::<Vec<_>>();
            let roles_label = if roles.is_empty() {
                "none configured".to_owned()
            } else {
                roles.join(", ")
            };
            ui.vertical(|ui| {
                ui.small(format!("Permissions: {permissions}"));
                ui.small(format!("Roles: {roles_label}"));
            });
        }
        let vault_exists = supervisor.signer_vault().exists();
        ui.horizontal(|ui| {
            if ui.button("Manage signing vault…").clicked() {
                self.open_signer_vault_dialog(supervisor);
            }
            if !vault_exists {
                ui.add_space(12.0);
                ui.small("Vault not configured — composer is using development signing keys.");
            }
        });
        ui.add_space(6.0);

        ui.horizontal(|ui| {
            ui.label("Chain ID");
            ui.text_edit_singleline(&mut self.composer_chain_id);
        });
        ui.add_space(6.0);
        self.render_composer_advanced_options(ui);

        ui.horizontal(|ui| {
            ui.label("Wizard");
            ui.spacing_mut().item_spacing.x = 12.0;
            for step in ComposerStep::all() {
                let selected = self.composer_step == step;
                if ui.selectable_label(selected, step.label()).clicked() {
                    self.composer_step = step;
                    if matches!(step, ComposerStep::Raw) && !self.composer_raw_dirty {
                        self.sync_raw_editor_from_drafts();
                    }
                }
            }
        });
        ui.separator();

        match self.composer_step {
            ComposerStep::Build => self.render_composer_build_step(ui, supervisor),
            ComposerStep::Raw => self.render_composer_raw_step(ui, supervisor),
            ComposerStep::Preview => self.render_composer_preview_step(ui, supervisor, signers),
        }
    }

    fn render_composer_advanced_options(&mut self, ui: &mut egui::Ui) {
        ui.add_space(4.0);
        egui::CollapsingHeader::new("Advanced transaction options")
            .id_salt("mochi_composer_advanced_options")
            .show(ui, |ui| {
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut self.composer_ttl_override, "Override TTL");
                        if self.composer_ttl_override {
                            ui.add(
                                egui::DragValue::new(&mut self.composer_ttl_ms)
                                    .range(0..=3_600_000)
                                    .suffix(" ms"),
                            );
                            if self.composer_ttl_ms == 0 {
                                ui.label(
                                    RichText::new("Expires immediately")
                                        .italics()
                                        .color(Color32::from_gray(140)),
                                );
                            }
                        } else {
                            ui.small("Use the node default TTL.");
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut self.composer_creation_override, "Freeze creation time");
                        if self.composer_creation_override {
                            ui.add(
                                egui::DragValue::new(&mut self.composer_creation_ms)
                                    .range(0..=u64::MAX)
                                    .suffix(" ms since epoch"),
                            );
                            if ui.button("Reset to now").clicked() {
                                self.composer_creation_ms = current_unix_timestamp_ms();
                            }
                        } else {
                            ui.small("Creation time will be set at preview.");
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut self.composer_nonce_override, "Custom nonce");
                        if self.composer_nonce_override {
                            ui.add(
                                egui::DragValue::new(&mut self.composer_nonce_value)
                                    .range(1..=u32::MAX),
                            );
                        } else {
                            ui.small("Nonce will follow node policy.");
                        }
                    });
                });
            });
        ui.add_space(6.0);
    }

    fn apply_composer_template(
        &mut self,
        template: ComposerTemplate,
        signers: &[SigningAuthority],
    ) {
        template.apply(self, signers);
    }

    fn open_signer_vault_dialog(&mut self, supervisor: &Supervisor) {
        match SignerVaultDialog::load(supervisor) {
            Ok(mut dialog) => {
                dialog.status = None;
                dialog.error = None;
                self.signer_dialog = Some(dialog);
            }
            Err(err) => {
                self.last_error = Some(format!("Failed to load signing vault: {err}"));
            }
        }
    }

    fn render_signer_vault_dialog(&mut self, ctx: &egui::Context, supervisor: &mut Supervisor) {
        let Some(dialog) = self.signer_dialog.as_mut() else {
            return;
        };
        let mut open = true;
        let mut request_close = false;
        egui::Window::new("Signing vault")
            .collapsible(false)
            .resizable(true)
            .default_width(520.0)
            .open(&mut open)
            .show(ctx, |ui| {
                ui.label("Configure signing authorities used by the transaction composer.");
                if dialog.using_fallback {
                    ui.colored_label(
                        Color32::from_rgb(200, 128, 32),
                        "No vault entries detected; the composer is currently using built-in development keys.",
                    );
                    ui.add_space(6.0);
                } else {
                    ui.add_space(4.0);
                }

                if let Some(error) = dialog.error.as_ref() {
                    ui.colored_label(Color32::from_rgb(200, 64, 64), error);
                    ui.add_space(4.0);
                } else if let Some(status) = dialog.status.as_ref() {
                    ui.colored_label(Color32::from_rgb(48, 160, 72), status);
                    ui.add_space(4.0);
                }

                ui.label(RichText::new("Configured authorities").strong());
                ui.add_space(4.0);

                if dialog.entries.is_empty() {
                    ui.label("No signing authorities configured yet.");
                } else {
                    let mut remove_indices = Vec::new();
                    let mut made_dirty = false;
                    ScrollArea::vertical()
                        .max_height(260.0)
                        .auto_shrink([false; 2])
                        .show(ui, |scroll| {
                            for idx in 0..dialog.entries.len() {
                                let entry = &mut dialog.entries[idx];
                                egui::Frame::group(scroll.style()).show(scroll, |ui| {
                                    ui.horizontal(|ui| {
                                        ui.label("Label");
                                        if ui.text_edit_singleline(&mut entry.label).changed() {
                                            made_dirty = true;
                                        }
                                    });
                                    ui.horizontal(|ui| {
                                        ui.label("Account");
                                        if ui.text_edit_singleline(&mut entry.account).changed() {
                                            made_dirty = true;
                                        }
                                    });
                                    ui.label("Permissions");
                                    ui.horizontal_wrapped(|ui| {
                                        for permission in InstructionPermission::all() {
                                            let mut allowed =
                                                entry.permissions.contains(&permission);
                                            if ui.checkbox(&mut allowed, permission.label()).changed()
                                            {
                                                if allowed {
                                                    entry.permissions.insert(permission);
                                                } else {
                                                    entry.permissions.remove(&permission);
                                                }
                                                made_dirty = true;
                                            }
                                        }
                                    });
                                    ui.horizontal(|ui| {
                                        ui.label("Roles");
                                        if ui.text_edit_singleline(&mut entry.roles).changed() {
                                            made_dirty = true;
                                        }
                                    });
                                    ui.small("Comma-separated role ids (optional).");
                                    ui.horizontal(|ui| {
                                        ui.label("Stored key");
                                        ui.small(Self::summarize_private_key(
                                            &entry.private_key,
                                        ));
                                    });
                                    ui.with_layout(Layout::right_to_left(egui::Align::Min), |ui| {
                                        if ui.button("Remove").clicked() {
                                            remove_indices.push(idx);
                                        }
                                    });
                                });
                                scroll.add_space(6.0);
                            }
                        });
                    if made_dirty {
                        dialog.mark_dirty();
                    }
                    if !remove_indices.is_empty() {
                        remove_indices.sort_unstable();
                        for idx in remove_indices.into_iter().rev() {
                            dialog.entries.remove(idx);
                        }
                        dialog.mark_dirty();
                    }
                }

                ui.add_space(8.0);
                ui.separator();
                ui.add_space(8.0);
                ui.label(RichText::new("Add new signer").strong());
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.label("Label");
                    ui.text_edit_singleline(&mut dialog.new_entry.label);
                });
                ui.horizontal(|ui| {
                    ui.label("Account");
                    ui.text_edit_singleline(&mut dialog.new_entry.account);
                });
                ui.horizontal(|ui| {
                    ui.label("Private key");
                    ui.add(
                        egui::TextEdit::singleline(&mut dialog.new_entry.private_key)
                            .password(true)
                            .hint_text("multihash hex"),
                    );
                });
                ui.horizontal_wrapped(|ui| {
                    ui.label("Permissions");
                });
                ui.horizontal_wrapped(|ui| {
                    for permission in InstructionPermission::all() {
                        let mut allowed = dialog.new_entry.permissions.contains(&permission);
                        if ui.checkbox(&mut allowed, permission.label()).changed() {
                            if allowed {
                                dialog.new_entry.permissions.insert(permission);
                            } else {
                                dialog.new_entry.permissions.remove(&permission);
                            }
                        }
                    }
                });
                ui.horizontal(|ui| {
                    ui.label("Roles");
                    ui.text_edit_singleline(&mut dialog.new_entry.roles);
                });
                ui.small("Comma-separated role ids (optional).");
                if ui.button("Add signer").clicked() {
                    match Self::entry_form_to_state(&dialog.new_entry) {
                        Ok(entry) => {
                            dialog.entries.push(entry);
                            dialog.new_entry.reset();
                            dialog.error = None;
                            dialog.status = None;
                            dialog.mark_dirty();
                        }
                        Err(err) => {
                            dialog.error = Some(err);
                        }
                    }
                }

                ui.add_space(12.0);
                ui.separator();
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button("Close").clicked() {
                        request_close = true;
                    }
                    ui.with_layout(Layout::right_to_left(egui::Align::Min), |ui| {
                        let save_button =
                            ui.add_enabled(dialog.dirty, egui::Button::new("Save changes"));
                        if save_button.clicked() {
                            match Self::signer_entries_to_signers(&dialog.entries) {
                                Ok(signers) => match supervisor.save_signers(&signers) {
                                    Ok(()) => {
                                        dialog.status =
                                            Some("Signing vault updated.".to_owned());
                                        dialog.error = None;
                                        dialog.dirty = false;
                                        dialog.using_fallback = false;
                                        if let Ok(fresh) = SignerVaultDialog::load(supervisor) {
                                            dialog.entries = fresh.entries;
                                            dialog.using_fallback = fresh.using_fallback;
                                        }
                                        dialog.new_entry.reset();
                                        Self::ensure_signer_selection(
                                            &mut self.composer_selected_signer,
                                            supervisor.signers(),
                                        );
                                        self.composer_preview = None;
                                    }
                                    Err(err) => {
                                        dialog.error =
                                            Some(format!("Failed to save vault: {err}"));
                                    }
                                },
                                Err(err) => {
                                    dialog.error = Some(err);
                                }
                            }
                        }
                    });
                });
            });
        if !open || request_close {
            self.signer_dialog = None;
        }
    }

    fn entry_form_to_state(form: &SignerEntryForm) -> Result<SignerEntryState, String> {
        let label = form.label.trim();
        if label.is_empty() {
            return Err("Signer label is required.".to_owned());
        }
        let account = form.account.trim();
        if account.is_empty() {
            return Err("Account identifier is required.".to_owned());
        }
        let private_key = form.private_key.trim();
        if private_key.is_empty() {
            return Err("Private key is required.".to_owned());
        }

        AccountId::from_str(account)
            .map_err(|err| format!("Invalid account `{account}`: {err}"))?;
        PrivateKey::from_str(private_key).map_err(|err| format!("Invalid private key: {err}"))?;

        if form.permissions.is_empty() {
            return Err("Select at least one permission for the signer.".to_owned());
        }
        Self::parse_role_list(&form.roles)?;

        Ok(SignerEntryState {
            label: label.to_owned(),
            account: account.to_owned(),
            private_key: private_key.to_owned(),
            permissions: form.permissions.clone(),
            roles: form.roles.trim().to_owned(),
        })
    }

    fn signer_entries_to_signers(
        entries: &[SignerEntryState],
    ) -> Result<Vec<SigningAuthority>, String> {
        let mut signers = Vec::with_capacity(entries.len());
        for entry in entries {
            let label = entry.label.trim();
            if label.is_empty() {
                return Err("Signer label cannot be empty.".to_owned());
            }
            let account_str = entry.account.trim();
            if account_str.is_empty() {
                return Err(format!("Signer `{label}` requires an account identifier."));
            }
            let account = AccountId::from_str(account_str)
                .map_err(|err| format!("Invalid account `{account_str}`: {err}"))?;
            if entry.permissions.is_empty() {
                return Err(format!(
                    "Signer `{label}` must permit at least one instruction."
                ));
            }
            let key_str = entry.private_key.trim();
            if key_str.is_empty() {
                return Err(format!("Signer `{label}` is missing a private key."));
            }
            let private_key = PrivateKey::from_str(key_str)
                .map_err(|err| format!("Invalid private key for `{label}`: {err}"))?;
            let key_pair = KeyPair::from_private_key(private_key)
                .map_err(|err| format!("Failed to construct key pair for `{label}`: {err}"))?;
            let roles = Self::parse_role_list(&entry.roles)?;
            signers.push(SigningAuthority::with_permissions_and_roles(
                label.to_owned(),
                account,
                key_pair,
                entry.permissions.iter().copied(),
                roles,
            ));
        }
        Ok(signers)
    }

    fn parse_role_list(raw: &str) -> Result<Vec<RoleId>, String> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Ok(Vec::new());
        }
        let mut roles = Vec::new();
        for token in trimmed.split(',') {
            let item = token.trim();
            if item.is_empty() {
                continue;
            }
            let role =
                RoleId::from_str(item).map_err(|err| format!("Invalid role `{item}`: {err}"))?;
            roles.push(role);
        }
        Ok(roles)
    }

    const fn admission_mode_label(mode: AccountAdmissionMode) -> &'static str {
        match mode {
            AccountAdmissionMode::ImplicitReceive => "Implicit receive",
            AccountAdmissionMode::ExplicitOnly => "Explicit only",
        }
    }

    fn parse_multisig_policy(raw: &str) -> Result<MultisigSpec, String> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err("Multisig policy JSON is empty.".to_owned());
        }
        json::from_str(trimmed).map_err(|err| format!("Invalid multisig policy JSON: {err}"))
    }

    fn parse_optional_u32(raw: &str, label: &str) -> Result<Option<u32>, String> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Ok(None);
        }
        trimmed
            .parse::<u32>()
            .map(Some)
            .map_err(|err| format!("{label} must be a number: {err}"))
    }

    fn parse_min_initial_amounts(
        raw: &str,
    ) -> Result<BTreeMap<AssetDefinitionId, Numeric>, String> {
        let mut amounts = BTreeMap::new();
        for (idx, line) in raw.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            let (asset_raw, amount_raw) = trimmed.split_once('=').ok_or_else(|| {
                format!("Line {} must use `<asset_definition> = <amount>`.", idx + 1)
            })?;
            let asset_raw = asset_raw.trim();
            let amount_raw = amount_raw.trim();
            if asset_raw.is_empty() || amount_raw.is_empty() {
                return Err(format!(
                    "Line {} must include both asset definition and amount.",
                    idx + 1
                ));
            }
            let asset = AssetDefinitionId::from_str(asset_raw)
                .map_err(|err| format!("Invalid asset definition `{asset_raw}`: {err}"))?;
            if amounts.contains_key(&asset) {
                return Err(format!("Duplicate entry for asset `{asset_raw}`."));
            }
            let amount = Numeric::from_str(amount_raw)
                .map_err(|err| format!("Invalid amount `{amount_raw}`: {err}"))?;
            amounts.insert(asset, amount);
        }
        Ok(amounts)
    }

    fn parse_account_admission_policy(
        &mut self,
    ) -> Result<(String, AccountAdmissionPolicy), String> {
        let domain_raw = self.composer_admission_domain.trim().to_owned();
        self.composer_admission_domain = domain_raw.clone();
        if domain_raw.is_empty() {
            return Err("Domain identifier is required.".to_owned());
        }
        let domain_id = DomainId::from_str(&domain_raw)
            .map_err(|err| format!("Invalid domain `{domain_raw}`: {err}"))?;

        let max_per_tx_raw = self.composer_admission_max_per_tx.trim().to_owned();
        self.composer_admission_max_per_tx = max_per_tx_raw.clone();
        let max_per_tx = Self::parse_optional_u32(&max_per_tx_raw, "Max per tx")?;

        let max_per_block_raw = self.composer_admission_max_per_block.trim().to_owned();
        self.composer_admission_max_per_block = max_per_block_raw.clone();
        let max_per_block = Self::parse_optional_u32(&max_per_block_raw, "Max per block")?;

        let implicit_creation_fee = if self.composer_admission_fee_enabled {
            let asset_raw = self.composer_admission_fee_asset.trim().to_owned();
            self.composer_admission_fee_asset = asset_raw.clone();
            if asset_raw.is_empty() {
                return Err("Fee asset definition is required.".to_owned());
            }
            let asset_definition_id = AssetDefinitionId::from_str(&asset_raw)
                .map_err(|err| format!("Invalid fee asset `{asset_raw}`: {err}"))?;
            let amount_raw = self.composer_admission_fee_amount.trim().to_owned();
            self.composer_admission_fee_amount = amount_raw.clone();
            if amount_raw.is_empty() {
                return Err("Fee amount is required.".to_owned());
            }
            let amount = Numeric::from_str(&amount_raw)
                .map_err(|err| format!("Invalid fee amount `{amount_raw}`: {err}"))?;
            let destination = if self.composer_admission_fee_destination_burn {
                ImplicitAccountFeeDestination::Burn
            } else {
                let account_raw = self
                    .composer_admission_fee_destination_account
                    .trim()
                    .to_owned();
                self.composer_admission_fee_destination_account = account_raw.clone();
                if account_raw.is_empty() {
                    return Err("Fee destination account is required.".to_owned());
                }
                let account = AccountId::from_str(&account_raw).map_err(|err| {
                    format!("Invalid fee destination account `{account_raw}`: {err}")
                })?;
                ImplicitAccountFeeDestination::Account(account)
            };
            Some(ImplicitAccountCreationFee {
                asset_definition_id,
                amount,
                destination,
            })
        } else {
            None
        };

        let min_initial_amounts =
            Self::parse_min_initial_amounts(&self.composer_admission_min_initial_amounts)?;

        let role_raw = self.composer_admission_default_role.trim().to_owned();
        self.composer_admission_default_role = role_raw.clone();
        let default_role_on_create = if role_raw.is_empty() {
            None
        } else {
            Some(
                RoleId::from_str(&role_raw)
                    .map_err(|err| format!("Invalid default role `{role_raw}`: {err}"))?,
            )
        };

        let policy = AccountAdmissionPolicy {
            mode: self.composer_admission_mode,
            max_implicit_creations_per_tx: max_per_tx,
            max_implicit_creations_per_block: max_per_block,
            implicit_creation_fee,
            min_initial_amounts,
            default_role_on_create,
        };

        Ok((domain_id.to_string(), policy))
    }

    fn summarize_private_key(key: &str) -> String {
        let trimmed = key.trim();
        if trimmed.is_empty() {
            "not set".to_owned()
        } else {
            format!("stored ({} chars)", trimmed.len())
        }
    }

    fn render_composer_build_step(&mut self, ui: &mut egui::Ui, supervisor: &Supervisor) {
        ui.label(RichText::new("Instruction builder").strong());
        ui.add_space(4.0);

        let signers = supervisor.signers();
        let selected_signer = self
            .composer_selected_signer
            .and_then(|index| signers.get(index));

        ui.horizontal(|ui| {
            ui.label("Kind");
            ComboBox::from_id_salt("mochi_composer_instruction_kind")
                .selected_text(self.composer_instruction_kind.label())
                .show_ui(ui, |ui| {
                    for kind in ComposerInstructionKind::all() {
                        let selected = self.composer_instruction_kind == kind;
                        let allowed = selected_signer
                            .map(|signer| signer.allows_permission(kind.permission()))
                            .unwrap_or(true);
                        let mut response =
                            ui.add_enabled(allowed, Button::new(kind.label()).selected(selected));
                        if !allowed {
                            let hover_text = if let Some(signer) = selected_signer {
                                format!(
                                    "{} lacks permission to {}.",
                                    signer.label(),
                                    kind.permission()
                                )
                            } else {
                                "Select a signing authority to enable actions.".to_owned()
                            };
                            response = response.on_hover_text(hover_text);
                        }
                        if response.clicked() && allowed && self.composer_instruction_kind != kind {
                            self.composer_instruction_kind = kind;
                            self.composer_error = None;
                        }
                    }
                });
        });

        let templates = ComposerTemplate::options_for(self.composer_instruction_kind);
        if !templates.is_empty() {
            ui.add_space(6.0);
            ui.horizontal_wrapped(|ui| {
                ui.label("Templates");
                ui.spacing_mut().item_spacing.x = 8.0;
                for template in templates {
                    let response = ui
                        .button(template.label())
                        .on_hover_text(template.description());
                    if response.clicked() {
                        self.apply_composer_template(*template, signers);
                    }
                }
            });
            ui.add_space(4.0);
        }

        match self.composer_instruction_kind {
            ComposerInstructionKind::MintAsset
            | ComposerInstructionKind::BurnAsset
            | ComposerInstructionKind::TransferAsset => {
                ui.horizontal(|ui| {
                    ui.label("Asset ID");
                    ui.text_edit_singleline(&mut self.composer_asset_id);
                });
                ui.horizontal(|ui| {
                    ui.label("Quantity");
                    ui.text_edit_singleline(&mut self.composer_quantity);
                });
            }
            _ => {}
        }

        if matches!(
            self.composer_instruction_kind,
            ComposerInstructionKind::TransferAsset
        ) {
            ui.horizontal(|ui| {
                ui.label("Destination");
                ui.text_edit_singleline(&mut self.composer_destination_account);
            });
            ui.small(
                "Implicit receives follow the domain admission policy (caps, fees, default role).",
            );
        }

        if matches!(
            self.composer_instruction_kind,
            ComposerInstructionKind::RegisterDomain
        ) {
            ui.horizontal(|ui| {
                ui.label("Domain ID");
                ui.text_edit_singleline(&mut self.composer_domain_id);
            });
        }

        if matches!(
            self.composer_instruction_kind,
            ComposerInstructionKind::RegisterAccount
        ) {
            ui.horizontal(|ui| {
                ui.label("Account ID");
                ui.text_edit_singleline(&mut self.composer_account_id);
            });
        }

        if matches!(
            self.composer_instruction_kind,
            ComposerInstructionKind::RegisterAssetDefinition
        ) {
            ui.horizontal(|ui| {
                ui.label("Definition ID");
                ui.text_edit_singleline(&mut self.composer_asset_definition_id);
            });
            ui.horizontal(|ui| {
                ui.label("Mintable");
                ComboBox::from_id_salt("mochi_composer_mintable_selector")
                    .selected_text(Self::mintable_label(
                        self.composer_asset_definition_mintable,
                    ))
                    .show_ui(ui, |ui| {
                        for option in [Mintable::Infinitely, Mintable::Once, Mintable::Not] {
                            let selected = self.composer_asset_definition_mintable == option;
                            if ui
                                .selectable_label(selected, Self::mintable_label(option))
                                .clicked()
                                && !selected
                            {
                                self.composer_asset_definition_mintable = option;
                            }
                        }
                        let limited_selected = matches!(
                            self.composer_asset_definition_mintable,
                            Mintable::Limited(_)
                        );
                        let limited_label =
                            format!("Limited({})", self.composer_mintability_tokens.max(1));
                        if ui
                            .selectable_label(limited_selected, limited_label)
                            .clicked()
                            && !limited_selected
                        {
                            let tokens = self.composer_mintability_tokens.max(1);
                            self.composer_mintability_tokens = tokens;
                            if let Ok(limited) = Mintable::limited_from_u32(tokens) {
                                self.composer_asset_definition_mintable = limited;
                            }
                        }
                    });
                if let Mintable::Limited(tokens) = self.composer_asset_definition_mintable {
                    self.composer_mintability_tokens = tokens.value();
                    ui.label("Allowed mints");
                    let response = ui.add(
                        egui::DragValue::new(&mut self.composer_mintability_tokens)
                            .range(1..=u32::MAX)
                            .suffix(" remaining"),
                    );
                    if response.changed() {
                        let tokens = self.composer_mintability_tokens.max(1);
                        self.composer_mintability_tokens = tokens;
                        if let Ok(limited) = Mintable::limited_from_u32(tokens) {
                            self.composer_asset_definition_mintable = limited;
                        }
                    }
                }
            });
        }

        if matches!(
            self.composer_instruction_kind,
            ComposerInstructionKind::PublishSpaceDirectoryManifest
        ) {
            ui.label("Space Directory manifest (JSON)");
            ui.add(
                egui::TextEdit::multiline(&mut self.composer_space_directory_manifest_json)
                    .desired_rows(8)
                    .hint_text("{ \"version\": 1, \"uaid\": \"uaid:...\", \"entries\": [ ... ] }"),
            );
            ui.small("AXT/AMX manifests define deterministic allowances for a UAID.");
        }

        if matches!(
            self.composer_instruction_kind,
            ComposerInstructionKind::RegisterPinManifest
        ) {
            ui.label("SoraFS pin manifest (JSON)");
            ui.add(
                egui::TextEdit::multiline(&mut self.composer_pin_manifest_json)
                    .desired_rows(8)
                    .hint_text("{ \"digest\": [..], \"chunker\": {..}, \"policy\": {..} }"),
            );
            ui.small("Pin intents require 32-byte digests and a pin policy.");
        }

        if matches!(
            self.composer_instruction_kind,
            ComposerInstructionKind::AccountAdmissionPolicy
        ) {
            ui.horizontal(|ui| {
                ui.label("Domain ID");
                ui.text_edit_singleline(&mut self.composer_admission_domain);
            });
            ui.horizontal(|ui| {
                ui.label("Mode");
                ComboBox::from_id_salt("mochi_composer_admission_mode")
                    .selected_text(Self::admission_mode_label(self.composer_admission_mode))
                    .show_ui(ui, |ui| {
                        for mode in [
                            AccountAdmissionMode::ImplicitReceive,
                            AccountAdmissionMode::ExplicitOnly,
                        ] {
                            let selected = self.composer_admission_mode == mode;
                            if ui
                                .selectable_label(selected, Self::admission_mode_label(mode))
                                .clicked()
                                && !selected
                            {
                                self.composer_admission_mode = mode;
                            }
                        }
                    });
            });
            ui.horizontal(|ui| {
                ui.label("Max per tx");
                ui.text_edit_singleline(&mut self.composer_admission_max_per_tx);
                ui.label("Max per block");
                ui.text_edit_singleline(&mut self.composer_admission_max_per_block);
            });
            ui.horizontal(|ui| {
                ui.checkbox(
                    &mut self.composer_admission_fee_enabled,
                    "Charge creation fee",
                );
            });
            if self.composer_admission_fee_enabled {
                ui.horizontal(|ui| {
                    ui.label("Fee asset definition");
                    ui.text_edit_singleline(&mut self.composer_admission_fee_asset);
                });
                ui.horizontal(|ui| {
                    ui.label("Fee amount");
                    ui.text_edit_singleline(&mut self.composer_admission_fee_amount);
                });
                ui.horizontal(|ui| {
                    ui.label("Fee destination");
                    let selected = if self.composer_admission_fee_destination_burn {
                        "Burn"
                    } else {
                        "Account"
                    };
                    ComboBox::from_id_salt("mochi_composer_fee_destination")
                        .selected_text(selected)
                        .show_ui(ui, |ui| {
                            if ui
                                .selectable_label(
                                    self.composer_admission_fee_destination_burn,
                                    "Burn",
                                )
                                .clicked()
                            {
                                self.composer_admission_fee_destination_burn = true;
                            }
                            if ui
                                .selectable_label(
                                    !self.composer_admission_fee_destination_burn,
                                    "Account",
                                )
                                .clicked()
                            {
                                self.composer_admission_fee_destination_burn = false;
                            }
                        });
                });
                if !self.composer_admission_fee_destination_burn {
                    ui.horizontal(|ui| {
                        ui.label("Destination account");
                        ui.text_edit_singleline(
                            &mut self.composer_admission_fee_destination_account,
                        );
                    });
                }
            }
            ui.label("Minimum initial amounts (one per line)");
            ui.add(
                egui::TextEdit::multiline(&mut self.composer_admission_min_initial_amounts)
                    .desired_rows(3)
                    .hint_text("rose#wonderland = 5"),
            );
            ui.horizontal(|ui| {
                ui.label("Default role on create");
                ui.text_edit_singleline(&mut self.composer_admission_default_role);
            });
            ui.small("Implicit creates grant the default role after AccountCreated when set.");
            if matches!(
                self.composer_admission_mode,
                AccountAdmissionMode::ExplicitOnly
            ) {
                ui.small("Implicit account creation is disabled for this domain.");
            }
        }

        if matches!(
            self.composer_instruction_kind,
            ComposerInstructionKind::GrantRole | ComposerInstructionKind::RevokeRole
        ) {
            ui.horizontal(|ui| {
                ui.label("Role ID");
                ui.text_edit_singleline(&mut self.composer_role_id);
            });
            ui.horizontal(|ui| {
                ui.label("Account ID");
                ui.text_edit_singleline(&mut self.composer_role_account);
            });
        }

        if matches!(
            self.composer_instruction_kind,
            ComposerInstructionKind::MultisigPropose
        ) {
            ui.horizontal(|ui| {
                ui.label("Multisig account");
                ui.text_edit_singleline(&mut self.composer_multisig_account);
            });
            ui.label("Proposal instructions (draft JSON)");
            ui.add(
                egui::TextEdit::multiline(&mut self.composer_multisig_instructions)
                    .desired_rows(6)
                    .hint_text("[ { \"kind\": \"mint_asset\", \"asset\": \"rose#wonderland#alice@wonderland\", \"quantity\": \"1\" } ]"),
            );
            ui.horizontal(|ui| {
                ui.checkbox(
                    &mut self.composer_multisig_ttl_enabled,
                    "Override proposal TTL",
                );
                if self.composer_multisig_ttl_enabled {
                    ui.add(
                        egui::DragValue::new(&mut self.composer_multisig_ttl_ms)
                            .range(1..=86_400_000)
                            .suffix(" ms"),
                    );
                }
            });
            ui.label("Multisig policy JSON (optional)");
            ui.add(
                egui::TextEdit::multiline(&mut self.composer_multisig_policy_json)
                    .desired_rows(4)
                    .hint_text("{ \"signatories\": { \"alice@wonderland\": 1 }, \"quorum\": 1, \"transaction_ttl_ms\": 3600000 }"),
            );
            if !self.composer_multisig_policy_json.trim().is_empty() {
                match Self::parse_multisig_policy(&self.composer_multisig_policy_json) {
                    Ok(policy) => {
                        ui.small(format!(
                            "Policy summary: {} signers, quorum {}, ttl cap {} ms.",
                            policy.signatories.len(),
                            policy.quorum.get(),
                            policy.transaction_ttl_ms.get()
                        ));
                        if self.composer_multisig_ttl_enabled
                            && self.composer_multisig_ttl_ms > policy.transaction_ttl_ms.get()
                        {
                            ui.colored_label(
                                Color32::from_rgb(200, 140, 32),
                                format!(
                                    "TTL override exceeds policy cap ({} ms).",
                                    policy.transaction_ttl_ms.get()
                                ),
                            );
                        }
                    }
                    Err(err) => {
                        ui.colored_label(Color32::from_rgb(200, 64, 64), err);
                    }
                }
            }
        }

        ui.add_space(6.0);
        ui.horizontal(|ui| {
            if ui.button("Add instruction to batch").clicked() {
                self.add_instruction_to_batch(Some(supervisor));
            }
            if ui
                .add_enabled(
                    !self.composer_drafts.is_empty(),
                    egui::Button::new("Clear batch"),
                )
                .clicked()
            {
                self.clear_instruction_batch();
            }
            if ui.button("Open raw editor").clicked() {
                if !self.composer_raw_dirty {
                    self.sync_raw_editor_from_drafts();
                }
                self.composer_step = ComposerStep::Raw;
            }
        });

        ui.add_space(6.0);
        if self.composer_drafts.is_empty() {
            ui.small("Draft instructions will appear here once they are added to the batch.");
        } else {
            ui.label(RichText::new("Draft instructions").strong());
            ui.add_space(4.0);
            let mut remove_index: Option<usize> = None;
            for (idx, draft) in self.composer_drafts.iter().enumerate() {
                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        ui.small(format!("{}.", idx + 1));
                        ui.label(draft.summary());
                        ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button("Remove").clicked() {
                                remove_index = Some(idx);
                            }
                        });
                    });
                });
                ui.add_space(4.0);
            }
            if let Some(idx) = remove_index {
                self.composer_drafts.remove(idx);
                self.composer_preview = None;
                self.composer_submit_error = None;
                self.composer_submit_success = None;
                self.sync_raw_editor_from_drafts();
            }
            if let Some(signer) = selected_signer
                && let Some(denied) = self
                    .composer_drafts
                    .iter()
                    .find(|draft| !signer.allows_permission(draft.permission()))
            {
                ui.colored_label(
                    Color32::from_rgb(200, 140, 32),
                    format!(
                        "{} cannot {} — remove or switch signer to proceed.",
                        signer.label(),
                        denied.permission()
                    ),
                );
            }
        }

        ui.add_space(6.0);
        if ui
            .add_enabled(
                !self.composer_drafts.is_empty(),
                egui::Button::new("Preview transaction"),
            )
            .clicked()
            && self.build_transaction_preview(supervisor)
        {
            self.composer_step = ComposerStep::Preview;
        }

        if let Some(err) = &self.composer_error {
            ui.colored_label(Color32::from_rgb(200, 64, 64), err);
        } else if self.composer_preview.is_none() && !self.composer_drafts.is_empty() {
            ui.small("Preview the transaction to inspect the encoded payload and signature.");
        }
    }

    fn render_composer_raw_step(&mut self, ui: &mut egui::Ui, supervisor: &Supervisor) {
        ui.label(RichText::new("Instruction batch (JSON)").strong());
        ui.add_space(4.0);

        if !self.composer_raw_dirty && self.composer_raw_editor.trim().is_empty() {
            self.sync_raw_editor_from_drafts();
        }

        let response = ui.add(
            egui::TextEdit::multiline(&mut self.composer_raw_editor)
                .code_editor()
                .desired_rows(12)
                .lock_focus(true),
        );
        if response.changed() {
            self.composer_raw_dirty = true;
        }

        ui.add_space(4.0);
        ui.horizontal(|ui| {
            if ui.button("Sync from drafts").clicked() {
                self.sync_raw_editor_from_drafts();
            }
            if ui.button("Apply JSON").clicked() {
                self.apply_raw_editor_to_drafts();
            }
            if ui.button("Apply + Preview").clicked() {
                self.apply_raw_editor_to_drafts();
                if self.composer_raw_error.is_none() && self.build_transaction_preview(supervisor) {
                    self.composer_step = ComposerStep::Preview;
                }
            }
        });

        if let Some(err) = &self.composer_raw_error {
            ui.colored_label(Color32::from_rgb(200, 64, 64), err);
        } else if self.composer_raw_dirty {
            ui.small("Unsaved edits — apply to refresh the draft list and preview.");
        } else {
            ui.small(format!(
                "{} instruction(s) in current batch.",
                self.composer_drafts.len()
            ));
        }
        ui.add_space(6.0);
        ui.small("Raw drafts follow the builder format. Unknown fields are ignored when applying.");
    }

    fn render_composer_preview_step(
        &mut self,
        ui: &mut egui::Ui,
        supervisor: &Supervisor,
        signers: &[SigningAuthority],
    ) {
        let Some(preview) = &self.composer_preview else {
            ui.small(
                "Build a preview from the builder or raw editor to inspect payload, signature, and submit to Torii.",
            );
            return;
        };

        ui.label(RichText::new("Preview summary").strong());
        ui.small(format!("Authority account: {}", preview.authority()));
        ui.small(format!("Transaction hash: {}", preview.hash()));
        if let Some(peer) = self.composer_selected_peer.as_deref() {
            ui.small(format!("Target peer: {peer}"));
        }
        if let Some(index) = self
            .composer_selected_signer
            .and_then(|idx| signers.get(idx))
        {
            ui.small(format!("Signer label: {}", index.label()));
            ui.small(format!("Signer account: {}", index.account_id()));
            ui.small(format!(
                "Signer public key: {}",
                index.key_pair().public_key()
            ));
        }

        let transaction_signature = preview.signed_transaction().signature().payload();
        let mut signature_hex = encode_upper(transaction_signature.payload());
        ui.add_space(6.0);
        ui.label(RichText::new("Signature (hex)").strong());
        ui.add(
            egui::TextEdit::multiline(&mut signature_hex)
                .code_editor()
                .desired_rows(2)
                .interactive(false)
                .lock_focus(true),
        );
        ui.small(format!(
            "Signature length: {} bytes",
            transaction_signature.payload().len()
        ));

        ui.add_space(6.0);
        ui.label(RichText::new("Norito payload (hex)").strong());
        let mut hex_payload = preview.encoded_hex().to_owned();
        ui.add(
            egui::TextEdit::multiline(&mut hex_payload)
                .code_editor()
                .desired_rows(4)
                .interactive(false)
                .lock_focus(true),
        );

        ui.add_space(6.0);
        ui.label(RichText::new("Instructions").strong());
        for (idx, instruction) in preview.instructions().iter().enumerate() {
            ui.small(format!("{idx}: {instruction}"));
        }

        ui.add_space(6.0);
        ui.label(RichText::new("Timing & nonce").strong());
        ui.small(format!(
            "Creation time: {} ms since Unix epoch",
            preview.creation_time().as_millis()
        ));
        if let Some(ttl) = preview.time_to_live() {
            ui.small(format!("Time-to-live: {} ms", ttl.as_millis()));
        } else {
            ui.small("Time-to-live: node default");
        }
        if let Some(nonce) = preview.nonce() {
            ui.small(format!("Nonce: {nonce}"));
        } else {
            ui.small("Nonce: node assigned");
        }

        ui.add_space(6.0);
        ui.horizontal(|ui| {
            let submit_enabled = !self.composer_submitting;
            if ui
                .add_enabled(submit_enabled, egui::Button::new("Submit to Torii"))
                .clicked()
            {
                self.submit_transaction(supervisor);
            }
            if self.composer_submitting {
                ui.add(egui::Spinner::new());
            }
            if ui.button("Back to builder").clicked() {
                self.composer_step = ComposerStep::Build;
            }
        });

        if let Some(success) = &self.composer_submit_success {
            ui.colored_label(Color32::from_rgb(80, 160, 80), success);
        }
        if let Some(error) = &self.composer_submit_error {
            ui.colored_label(error.color(), error.label());
        }

        ui.add_space(6.0);
        ui.small(
            RichText::new("Adjust the signing vault to switch the key used for submissions.")
                .italics(),
        );
    }

    fn build_transaction_preview(&mut self, supervisor: &Supervisor) -> bool {
        let chain = if self.composer_chain_id.trim().is_empty() {
            supervisor.chain_id().to_owned()
        } else {
            self.composer_chain_id.trim().to_owned()
        };
        self.composer_chain_id = chain.clone();

        if self.composer_drafts.is_empty() {
            self.composer_error =
                Some("Add at least one instruction before building a preview.".to_owned());
            self.composer_preview = None;
            return false;
        }

        let signers = supervisor.signers();
        Self::ensure_signer_selection(&mut self.composer_selected_signer, signers);
        let Some(index) = self.composer_selected_signer else {
            self.composer_error = Some("Select a signing authority before previewing.".to_owned());
            self.composer_preview = None;
            return false;
        };
        let Some(signer) = signers.get(index) else {
            self.composer_error = Some("Signing authority unavailable.".to_owned());
            self.composer_preview = None;
            return false;
        };
        if let Err(err) = signer.validate_drafts(&self.composer_drafts) {
            self.composer_preview = None;
            self.composer_error = Some(err.to_string());
            self.composer_submit_success = None;
            return false;
        }

        let mut options = TransactionComposeOptions::default();
        if self.composer_ttl_override {
            options = options.with_ttl(Duration::from_millis(self.composer_ttl_ms));
        }
        if self.composer_creation_override {
            options = options.with_creation_time(Duration::from_millis(self.composer_creation_ms));
        }
        if self.composer_nonce_override {
            if let Some(nonce) = NonZeroU32::new(self.composer_nonce_value) {
                options = options.with_nonce(nonce);
            } else {
                self.composer_preview = None;
                self.composer_error =
                    Some("Nonce must be greater than zero when overriding.".to_owned());
                self.composer_submit_success = None;
                return false;
            }
        }

        match compose_preview_with_options(&chain, &self.composer_drafts, signer, &options) {
            Ok(preview) => {
                self.composer_preview = Some(preview);
                self.composer_error = None;
                self.composer_submit_error = None;
                self.composer_submit_success = None;
                true
            }
            Err(err) => {
                self.composer_preview = None;
                self.composer_error = Some(err.to_string());
                self.composer_submit_success = None;
                false
            }
        }
    }

    fn add_instruction_to_batch(&mut self, supervisor: Option<&Supervisor>) {
        let signers = if let Some(supervisor) = supervisor {
            supervisor.signers()
        } else {
            mochi_core::development_signing_authorities()
        };
        let Some(index) = self.composer_selected_signer else {
            self.composer_error =
                Some("Select a signing authority before adding instructions.".to_owned());
            return;
        };
        let Some(signer) = signers.get(index) else {
            self.composer_error = Some("Signing authority unavailable.".to_owned());
            return;
        };
        let required_permission = self.composer_instruction_kind.permission();
        if !signer.allows_permission(required_permission) {
            self.composer_error = Some(format!(
                "{} cannot {}.",
                signer.label(),
                required_permission
            ));
            return;
        }

        let result = match self.composer_instruction_kind {
            ComposerInstructionKind::MintAsset => {
                let asset = self.composer_asset_id.trim().to_owned();
                self.composer_asset_id = asset.clone();
                if asset.is_empty() {
                    self.composer_error = Some("Asset identifier is required.".to_owned());
                    return;
                }
                let quantity = self.composer_quantity.trim().to_owned();
                self.composer_quantity = quantity.clone();
                if quantity.is_empty() {
                    self.composer_error = Some("Quantity is required.".to_owned());
                    return;
                }
                InstructionDraft::mint_from_input(&asset, &quantity)
            }
            ComposerInstructionKind::BurnAsset => {
                let asset = self.composer_asset_id.trim().to_owned();
                self.composer_asset_id = asset.clone();
                if asset.is_empty() {
                    self.composer_error = Some("Asset identifier is required.".to_owned());
                    return;
                }
                let quantity = self.composer_quantity.trim().to_owned();
                self.composer_quantity = quantity.clone();
                if quantity.is_empty() {
                    self.composer_error = Some("Quantity is required.".to_owned());
                    return;
                }
                InstructionDraft::burn_from_input(&asset, &quantity)
            }
            ComposerInstructionKind::TransferAsset => {
                let asset = self.composer_asset_id.trim().to_owned();
                self.composer_asset_id = asset.clone();
                if asset.is_empty() {
                    self.composer_error = Some("Asset identifier is required.".to_owned());
                    return;
                }
                let quantity = self.composer_quantity.trim().to_owned();
                self.composer_quantity = quantity.clone();
                if quantity.is_empty() {
                    self.composer_error = Some("Quantity is required.".to_owned());
                    return;
                }
                let destination = self.composer_destination_account.trim().to_owned();
                self.composer_destination_account = destination.clone();
                if destination.is_empty() {
                    self.composer_error =
                        Some("Destination account is required for transfers.".to_owned());
                    return;
                }
                InstructionDraft::transfer_from_input(&asset, &quantity, &destination)
            }
            ComposerInstructionKind::RegisterDomain => {
                let domain = self.composer_domain_id.trim().to_owned();
                self.composer_domain_id = domain.clone();
                if domain.is_empty() {
                    self.composer_error = Some("Domain identifier is required.".to_owned());
                    return;
                }
                InstructionDraft::register_domain_from_input(&domain)
            }
            ComposerInstructionKind::RegisterAccount => {
                let account = self.composer_account_id.trim().to_owned();
                self.composer_account_id = account.clone();
                if account.is_empty() {
                    self.composer_error = Some("Account identifier is required.".to_owned());
                    return;
                }
                InstructionDraft::register_account_from_input(&account)
            }
            ComposerInstructionKind::RegisterAssetDefinition => {
                let definition = self.composer_asset_definition_id.trim().to_owned();
                self.composer_asset_definition_id = definition.clone();
                if definition.is_empty() {
                    self.composer_error =
                        Some("Asset definition identifier is required.".to_owned());
                    return;
                }
                InstructionDraft::register_asset_definition_from_input(
                    &definition,
                    self.composer_asset_definition_mintable,
                )
            }
            ComposerInstructionKind::PublishSpaceDirectoryManifest => {
                let manifest = self
                    .composer_space_directory_manifest_json
                    .trim()
                    .to_owned();
                self.composer_space_directory_manifest_json = manifest.clone();
                if manifest.is_empty() {
                    self.composer_error = Some("Manifest JSON is required.".to_owned());
                    return;
                }
                InstructionDraft::publish_space_directory_manifest_from_json(&manifest)
            }
            ComposerInstructionKind::RegisterPinManifest => {
                let manifest = self.composer_pin_manifest_json.trim().to_owned();
                self.composer_pin_manifest_json = manifest.clone();
                if manifest.is_empty() {
                    self.composer_error = Some("Pin manifest JSON is required.".to_owned());
                    return;
                }
                InstructionDraft::register_pin_manifest_from_json(&manifest)
            }
            ComposerInstructionKind::GrantRole => {
                let role = self.composer_role_id.trim().to_owned();
                self.composer_role_id = role.clone();
                if role.is_empty() {
                    self.composer_error = Some("Role identifier is required.".to_owned());
                    return;
                }
                let account = self.composer_role_account.trim().to_owned();
                self.composer_role_account = account.clone();
                if account.is_empty() {
                    self.composer_error = Some("Target account is required.".to_owned());
                    return;
                }
                InstructionDraft::grant_role_from_input(&role, &account)
            }
            ComposerInstructionKind::RevokeRole => {
                let role = self.composer_role_id.trim().to_owned();
                self.composer_role_id = role.clone();
                if role.is_empty() {
                    self.composer_error = Some("Role identifier is required.".to_owned());
                    return;
                }
                let account = self.composer_role_account.trim().to_owned();
                self.composer_role_account = account.clone();
                if account.is_empty() {
                    self.composer_error = Some("Target account is required.".to_owned());
                    return;
                }
                InstructionDraft::revoke_role_from_input(&role, &account)
            }
            ComposerInstructionKind::AccountAdmissionPolicy => {
                let (domain, policy) = match self.parse_account_admission_policy() {
                    Ok(value) => value,
                    Err(err) => {
                        self.composer_error = Some(err);
                        return;
                    }
                };
                InstructionDraft::account_admission_policy_from_input(&domain, policy)
            }
            ComposerInstructionKind::MultisigPropose => {
                let account = self.composer_multisig_account.trim().to_owned();
                self.composer_multisig_account = account.clone();
                if account.is_empty() {
                    self.composer_error = Some("Multisig account is required.".to_owned());
                    return;
                }
                let instructions = self.composer_multisig_instructions.trim().to_owned();
                self.composer_multisig_instructions = instructions.clone();
                if instructions.is_empty() {
                    self.composer_error =
                        Some("Multisig proposal instructions are required.".to_owned());
                    return;
                }
                if self.composer_multisig_ttl_enabled
                    && !self.composer_multisig_policy_json.trim().is_empty()
                {
                    match Self::parse_multisig_policy(&self.composer_multisig_policy_json) {
                        Ok(policy) => {
                            let cap = policy.transaction_ttl_ms.get();
                            if self.composer_multisig_ttl_ms > cap {
                                self.composer_error =
                                    Some(format!("Proposal TTL exceeds policy cap ({cap} ms)."));
                                return;
                            }
                        }
                        Err(err) => {
                            self.composer_error = Some(err);
                            return;
                        }
                    }
                }
                let ttl = if self.composer_multisig_ttl_enabled {
                    Some(self.composer_multisig_ttl_ms)
                } else {
                    None
                };
                InstructionDraft::multisig_propose_from_json(&account, &instructions, ttl)
            }
        };

        match result {
            Ok(draft) => {
                self.composer_drafts.push(draft);
                if let Err(err) = signer.validate_drafts(&self.composer_drafts) {
                    self.composer_error = Some(err.to_string());
                    self.composer_drafts.pop();
                    return;
                }
                self.composer_error = None;
                self.composer_preview = None;
                self.composer_submit_error = None;
                self.composer_submit_success = None;
                self.sync_raw_editor_from_drafts();
            }
            Err(err) => {
                self.composer_error = Some(err.to_string());
            }
        }
    }

    fn clear_instruction_batch(&mut self) {
        self.composer_drafts.clear();
        self.composer_preview = None;
        self.composer_submit_error = None;
        self.composer_submit_success = None;
        self.composer_error = None;
        self.sync_raw_editor_from_drafts();
    }

    fn submit_transaction(&mut self, supervisor: &Supervisor) {
        if self.composer_submitting {
            return;
        }
        let Some(preview) = self.composer_preview.clone() else {
            self.composer_submit_error = Some(ToriiErrorInfo::new(
                ToriiErrorKind::InvalidEndpoint,
                "Preview a transaction before submitting.",
            ));
            self.composer_submit_success = None;
            return;
        };
        let Some(peer) = self.composer_selected_peer.clone() else {
            self.composer_submit_error = Some(ToriiErrorInfo::new(
                ToriiErrorKind::InvalidEndpoint,
                "Select a peer before submitting.",
            ));
            self.composer_submit_success = None;
            return;
        };
        let Some(client) = supervisor.torii_client(&peer) else {
            let message = "Torii client unavailable for selected peer.".to_owned();
            self.composer_submit_error = Some(ToriiErrorInfo::new(
                ToriiErrorKind::InvalidEndpoint,
                message.clone(),
            ));
            self.composer_submit_success = None;
            self.last_info = None;
            self.last_error = Some(message);
            return;
        };

        let bytes = preview.encoded_bytes();
        let hash = preview.hash().to_owned();
        let tx = self.composer_submit_tx.clone();

        self.composer_submitting = true;
        self.composer_submit_error = None;
        self.composer_submit_success = None;
        self.last_info = None;

        self.runtime.spawn(async move {
            let result = client
                .submit_transaction(&bytes)
                .await
                .map(|_| hash)
                .map_err(|err| err.summarize());
            let _ = tx.send(ComposerSubmitUpdate { peer, result });
        });
    }
}

fn unix_timestamp_ms_from_duration(duration: Result<Duration, SystemTimeError>) -> u64 {
    duration
        .unwrap_or(Duration::ZERO)
        .as_millis()
        .min(u128::from(u64::MAX)) as u64
}

fn unix_timestamp_ms(time: SystemTime) -> u64 {
    unix_timestamp_ms_from_duration(time.duration_since(UNIX_EPOCH))
}

fn current_unix_timestamp_ms() -> u64 {
    unix_timestamp_ms(SystemTime::now())
}

#[cfg(test)]
mod timestamp_tests {
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    use super::{current_unix_timestamp_ms, unix_timestamp_ms, unix_timestamp_ms_from_duration};

    #[test]
    fn unix_timestamp_ms_clamps_before_epoch() {
        let err = UNIX_EPOCH
            .duration_since(UNIX_EPOCH + Duration::from_millis(1))
            .unwrap_err();
        assert_eq!(unix_timestamp_ms_from_duration(Err(err)), 0);
    }

    #[test]
    fn current_timestamp_ms_matches_now_within_margin() {
        let lower_bound = unix_timestamp_ms(SystemTime::now());
        let actual = current_unix_timestamp_ms();
        let upper_bound = unix_timestamp_ms(SystemTime::now());

        // Accept a generous margin to avoid flakes on slow or skewed clocks.
        assert!(actual >= lower_bound.saturating_sub(60_000));
        assert!(actual <= upper_bound + 60_000);
    }
}

impl App for MochiApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.ensure_theme(ctx);
        let mut supervisor_opt = self.supervisor.take();
        self.poll_maintenance_updates(&mut supervisor_opt);
        self.schedule_pending_maintenance(&mut supervisor_opt);
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("MOCHI — Local Iroha Network Supervisor");
            if let Some(supervisor) = supervisor_opt.as_mut() {
                self.render_ready(ui, supervisor);
            } else if matches!(self.maintenance_state, MaintenanceState::Running(_)) {
                ui.label("Maintenance task running…");
            } else if let Some(err) = self.supervisor_error.as_ref() {
                ui.colored_label(
                    egui::Color32::from_rgb(200, 64, 64),
                    format!("Failed to prepare network: {err}"),
                );
                ui.small("Check filesystem permissions or override MOCHI_DATA_ROOT.");
            }
        });
        if self.settings_dialog {
            self.render_settings_dialog(ctx);
        }
        if let Some(supervisor) = supervisor_opt.as_mut() {
            self.render_maintenance_dialog(ctx, supervisor);
        }
        if let Some(supervisor) = supervisor_opt.as_mut() {
            self.render_signer_vault_dialog(ctx, supervisor);
        }
        self.schedule_pending_maintenance(&mut supervisor_opt);
        self.supervisor = supervisor_opt;
    }

    fn save(&mut self, storage: &mut dyn Storage) {
        if let Some(serialized) = serialize_event_filter(&self.event_filter) {
            storage.set_string(EVENT_FILTER_STORAGE_KEY, serialized);
        }
    }
}

#[derive(Clone)]
struct PeerRow {
    alias: String,
    state: PeerState,
    torii: String,
    api_base: Option<String>,
    api_error: Option<String>,
    config: String,
    logs: String,
}

#[derive(Debug, Default, Clone)]
struct DashboardMetrics {
    total_peers: usize,
    running_peers: usize,
    connected_block_streams: usize,
    connected_event_streams: usize,
    latest_height: Option<u64>,
    total_tx: u64,
    total_rejected_tx: u64,
    avg_queue: Option<f64>,
    avg_commit_latency_ms: Option<f64>,
    throughput_per_sec: Option<f64>,
    pending_block_events: usize,
    pending_event_frames: usize,
    stored_logs: usize,
}

impl DashboardMetrics {
    fn peers_label(&self) -> String {
        format!("{} / {}", self.running_peers, self.total_peers.max(1))
    }

    fn latest_height_text(&self) -> String {
        self.latest_height
            .map(|height| height.to_string())
            .unwrap_or_else(|| "—".to_owned())
    }

    fn queue_text(&self) -> String {
        self.avg_queue
            .map(|value| format!("{value:.1}"))
            .unwrap_or_else(|| "—".to_owned())
    }

    fn latency_text(&self) -> String {
        self.avg_commit_latency_ms
            .map(|value| format!("{value:.1} ms"))
            .unwrap_or_else(|| "—".to_owned())
    }

    fn throughput_text(&self) -> String {
        self.throughput_per_sec
            .map(|value| format!("{value:.2} tx/s"))
            .unwrap_or_else(|| "—".to_owned())
    }
}

struct DisplayEvent {
    alias: Option<String>,
    event: BlockStreamEvent,
}

#[derive(Debug, Clone)]
struct EventDisplay {
    alias: Option<String>,
    event: EventStreamEvent,
}

#[derive(Debug, Clone, Copy)]
enum RenderedEventKind {
    Category(EventCategory),
    Text,
    DecodeError,
    Lagged,
    Closed,
}

#[derive(Debug, Clone)]
struct EventBadge {
    label: &'static str,
    value: String,
    color: Color32,
    search_blob: String,
}

impl EventBadge {
    fn new(
        label: &'static str,
        raw_value: impl Into<String>,
        display: Option<String>,
        color: Color32,
    ) -> Self {
        let raw = raw_value.into();
        let display = display.unwrap_or_else(|| raw.clone());
        let search_blob = format!("{label} {raw}").to_ascii_lowercase();
        Self {
            label,
            value: display,
            color,
            search_blob,
        }
    }
}

#[derive(Debug, Clone)]
struct RenderedEventLine {
    alias: String,
    alias_lower: String,
    summary: String,
    detail: Option<String>,
    summary_color: Color32,
    kind: RenderedEventKind,
    search_blob: String,
    badges: Vec<EventBadge>,
}

impl RenderedEventLine {
    fn new(
        alias: &str,
        summary: String,
        detail: Option<String>,
        summary_color: Color32,
        kind: RenderedEventKind,
    ) -> Self {
        let search_blob = compose_search_blob(alias, &summary, detail.as_deref());
        Self {
            alias: alias.to_owned(),
            alias_lower: alias.to_ascii_lowercase(),
            summary,
            detail,
            summary_color,
            kind,
            search_blob,
            badges: Vec::new(),
        }
    }

    fn with_badges(mut self, badges: Vec<EventBadge>) -> Self {
        if !badges.is_empty() {
            for badge in &badges {
                self.search_blob.push(' ');
                self.search_blob.push_str(&badge.search_blob);
            }
        }
        self.badges = badges;
        self
    }
}

#[derive(Debug, Clone)]
struct EventFilterState {
    search: String,
    show_pipeline: bool,
    show_data: bool,
    show_time: bool,
    show_execute_trigger: bool,
    show_trigger_completed: bool,
    show_text: bool,
    show_decode_errors: bool,
    show_lagged: bool,
    show_closed: bool,
    alias_filters: BTreeSet<String>,
}

impl Default for EventFilterState {
    fn default() -> Self {
        Self {
            search: String::new(),
            show_pipeline: true,
            show_data: true,
            show_time: true,
            show_execute_trigger: true,
            show_trigger_completed: true,
            show_text: true,
            show_decode_errors: true,
            show_lagged: true,
            show_closed: true,
            alias_filters: BTreeSet::new(),
        }
    }
}

impl EventFilterState {
    fn reset(&mut self) {
        *self = Self::default();
    }

    fn normalized_query(&self) -> Option<String> {
        let trimmed = self.search.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_ascii_lowercase())
        }
    }

    fn clear_aliases(&mut self) {
        self.alias_filters.clear();
    }

    fn toggle_alias(&mut self, alias: &str, enabled: bool) {
        let key = alias.to_ascii_lowercase();
        if enabled {
            self.alias_filters.remove(&key);
        } else {
            self.alias_filters.insert(key);
        }
    }

    fn alias_selected(&self, alias: &str) -> bool {
        !self.alias_filters.contains(&alias.to_ascii_lowercase())
    }

    fn has_alias_filters(&self) -> bool {
        !self.alias_filters.is_empty()
    }

    fn matches(&self, line: &RenderedEventLine, query: Option<&str>) -> bool {
        if self.alias_filters.contains(&line.alias_lower) {
            return false;
        }
        let allowed = match line.kind {
            RenderedEventKind::Category(EventCategory::Pipeline) => self.show_pipeline,
            RenderedEventKind::Category(EventCategory::Data) => self.show_data,
            RenderedEventKind::Category(EventCategory::Time) => self.show_time,
            RenderedEventKind::Category(EventCategory::ExecuteTrigger) => self.show_execute_trigger,
            RenderedEventKind::Category(EventCategory::TriggerCompleted) => {
                self.show_trigger_completed
            }
            RenderedEventKind::Text => self.show_text,
            RenderedEventKind::DecodeError => self.show_decode_errors,
            RenderedEventKind::Lagged => self.show_lagged,
            RenderedEventKind::Closed => self.show_closed,
        };
        if !allowed {
            return false;
        }
        match query {
            Some(needle) => line.search_blob.contains(needle),
            None => true,
        }
    }

    fn to_json_value(&self) -> Value {
        let mut map = Map::new();
        map.insert("search".into(), Value::String(self.search.clone()));
        map.insert("show_pipeline".into(), Value::Bool(self.show_pipeline));
        map.insert("show_data".into(), Value::Bool(self.show_data));
        map.insert("show_time".into(), Value::Bool(self.show_time));
        map.insert(
            "show_execute_trigger".into(),
            Value::Bool(self.show_execute_trigger),
        );
        map.insert(
            "show_trigger_completed".into(),
            Value::Bool(self.show_trigger_completed),
        );
        map.insert("show_text".into(), Value::Bool(self.show_text));
        map.insert(
            "show_decode_errors".into(),
            Value::Bool(self.show_decode_errors),
        );
        map.insert("show_lagged".into(), Value::Bool(self.show_lagged));
        map.insert("show_closed".into(), Value::Bool(self.show_closed));
        let aliases = self
            .alias_filters
            .iter()
            .cloned()
            .map(Value::String)
            .collect::<Vec<_>>();
        map.insert("alias_filters".into(), Value::Array(aliases));
        Value::Object(map)
    }

    fn from_json_value(value: &Value) -> Option<Self> {
        let map = value.as_object()?;
        let mut filter = EventFilterState::default();
        if let Some(text) = map.get("search").and_then(Value::as_str) {
            filter.search = text.to_owned();
        }
        filter.show_pipeline = map
            .get("show_pipeline")
            .and_then(Value::as_bool)
            .unwrap_or(filter.show_pipeline);
        filter.show_data = map
            .get("show_data")
            .and_then(Value::as_bool)
            .unwrap_or(filter.show_data);
        filter.show_time = map
            .get("show_time")
            .and_then(Value::as_bool)
            .unwrap_or(filter.show_time);
        filter.show_execute_trigger = map
            .get("show_execute_trigger")
            .and_then(Value::as_bool)
            .unwrap_or(filter.show_execute_trigger);
        filter.show_trigger_completed = map
            .get("show_trigger_completed")
            .and_then(Value::as_bool)
            .unwrap_or(filter.show_trigger_completed);
        filter.show_text = map
            .get("show_text")
            .and_then(Value::as_bool)
            .unwrap_or(filter.show_text);
        filter.show_decode_errors = map
            .get("show_decode_errors")
            .and_then(Value::as_bool)
            .unwrap_or(filter.show_decode_errors);
        filter.show_lagged = map
            .get("show_lagged")
            .and_then(Value::as_bool)
            .unwrap_or(filter.show_lagged);
        filter.show_closed = map
            .get("show_closed")
            .and_then(Value::as_bool)
            .unwrap_or(filter.show_closed);
        if let Some(array) = map.get("alias_filters").and_then(Value::as_array) {
            let mut aliases = BTreeSet::new();
            for entry in array {
                if let Some(alias) = entry.as_str() {
                    aliases.insert(alias.to_ascii_lowercase());
                }
            }
            filter.alias_filters = aliases;
        }
        Some(filter)
    }
}

#[derive(Debug, Clone, Default)]
struct StateFilter {
    search: String,
    domain: String,
    owner: String,
    asset_definition: String,
}

#[derive(Debug, Clone, Default)]
struct StatePageCache {
    entries: Vec<StateEntry>,
    remaining: u64,
}

#[derive(Debug, Clone)]
struct StateTabState {
    kind: StateQueryKind,
    filter: StateFilter,
    entries: Vec<StateEntry>,
    pages: Vec<StatePageCache>,
    current_page: usize,
    cursor: Option<StateCursor>,
    remaining: Option<u64>,
    error: Option<String>,
    loading: bool,
}

impl StateTabState {
    fn new(kind: StateQueryKind) -> Self {
        let mut filter = StateFilter::default();
        filter.adapt_to_kind(kind);
        Self {
            kind,
            filter,
            entries: Vec::new(),
            pages: Vec::new(),
            current_page: 0,
            cursor: None,
            remaining: None,
            error: None,
            loading: false,
        }
    }

    fn reset_results(&mut self) {
        self.entries.clear();
        self.pages.clear();
        self.current_page = 0;
        self.cursor = None;
        self.remaining = None;
        self.error = None;
        self.loading = false;
    }

    fn reset_for_peer_change(&mut self) {
        self.reset_results();
        self.filter.adapt_to_kind(self.kind);
    }

    fn select_page(&mut self, index: usize) {
        if index >= self.pages.len() {
            return;
        }
        self.current_page = index;
        if let Some(cache) = self.pages.get(index) {
            self.entries = cache.entries.clone();
            self.remaining = (cache.remaining > 0).then_some(cache.remaining);
        } else {
            self.entries.clear();
            self.remaining = None;
        }
    }

    fn total_cached(&self) -> usize {
        if self.pages.is_empty() {
            self.entries.len()
        } else {
            self.pages.iter().map(|page| page.entries.len()).sum()
        }
    }

    fn has_cursor(&self) -> bool {
        self.cursor.is_some()
    }
}

#[derive(Debug)]
struct StateTabs {
    accounts: StateTabState,
    assets: StateTabState,
    asset_definitions: StateTabState,
    domains: StateTabState,
    peers: StateTabState,
}

impl Default for StateTabs {
    fn default() -> Self {
        Self {
            accounts: StateTabState::new(StateQueryKind::Accounts),
            assets: StateTabState::new(StateQueryKind::Assets),
            asset_definitions: StateTabState::new(StateQueryKind::AssetDefinitions),
            domains: StateTabState::new(StateQueryKind::Domains),
            peers: StateTabState::new(StateQueryKind::Peers),
        }
    }
}

impl StateTabs {
    fn get(&self, kind: StateQueryKind) -> &StateTabState {
        match kind {
            StateQueryKind::Accounts => &self.accounts,
            StateQueryKind::Assets => &self.assets,
            StateQueryKind::AssetDefinitions => &self.asset_definitions,
            StateQueryKind::Domains => &self.domains,
            StateQueryKind::Peers => &self.peers,
        }
    }

    fn get_mut(&mut self, kind: StateQueryKind) -> &mut StateTabState {
        let tab = match kind {
            StateQueryKind::Accounts => &mut self.accounts,
            StateQueryKind::Assets => &mut self.assets,
            StateQueryKind::AssetDefinitions => &mut self.asset_definitions,
            StateQueryKind::Domains => &mut self.domains,
            StateQueryKind::Peers => &mut self.peers,
        };
        tab.filter.adapt_to_kind(kind);
        tab
    }

    fn reset_results_for_all(&mut self) {
        self.accounts.reset_for_peer_change();
        self.assets.reset_for_peer_change();
        self.asset_definitions.reset_for_peer_change();
        self.domains.reset_for_peer_change();
        self.peers.reset_for_peer_change();
    }
}

#[derive(Debug, Clone)]
struct StateRenderSnapshot {
    entries_on_page: Vec<StateEntry>,
    matching_cached_entries: Vec<StateEntry>,
    filtered_indices: Vec<usize>,
    filters_active: bool,
    loading: bool,
    error: Option<String>,
    remaining: Option<u64>,
    current_page: usize,
    page_count: usize,
    total_entries: usize,
    total_cached: usize,
    has_cursor: bool,
}

#[derive(Debug)]
enum StateExportAction {
    CopyJson(Vec<StateEntry>),
    CopyNorito(Vec<StateEntry>),
    SaveJson(Vec<StateEntry>),
    SaveNorito(Vec<StateEntry>),
}

impl StateFilter {
    fn clear(&mut self) {
        *self = Self::default();
    }

    fn adapt_to_kind(&mut self, kind: StateQueryKind) {
        match kind {
            StateQueryKind::Accounts => {
                self.owner.clear();
                self.asset_definition.clear();
            }
            StateQueryKind::Assets => {}
            StateQueryKind::AssetDefinitions => {
                self.asset_definition.clear();
            }
            StateQueryKind::Domains => {
                self.owner.clear();
                self.asset_definition.clear();
            }
            StateQueryKind::Peers => {
                self.domain.clear();
                self.owner.clear();
                self.asset_definition.clear();
            }
        }
    }

    fn search_query(&self) -> Option<String> {
        let trimmed = self.search.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_ascii_lowercase())
        }
    }

    fn normalized_domain(&self) -> Option<String> {
        normalized_filter(&self.domain)
    }

    fn normalized_owner(&self) -> Option<String> {
        normalized_filter(&self.owner)
    }

    fn normalized_definition(&self) -> Option<String> {
        normalized_filter(&self.asset_definition)
    }
}

#[derive(Debug, Clone, Default)]
struct BlockStreamSnapshot {
    last_summary: Option<BlockSummary>,
    last_error: Option<String>,
    connected: bool,
    last_update: Option<Instant>,
}

impl BlockStreamSnapshot {
    fn status_label(&self) -> (String, Color32) {
        if let Some(error) = &self.last_error {
            (truncate(error, 48), Color32::from_rgb(200, 64, 64))
        } else if self.connected {
            ("Connected".to_owned(), Color32::from_rgb(80, 160, 80))
        } else if self.last_summary.is_some() {
            ("Idle".to_owned(), Color32::from_rgb(200, 160, 64))
        } else {
            ("Inactive".to_owned(), Color32::from_gray(150))
        }
    }
}

fn truncate(input: &str, max_len: usize) -> String {
    if input.len() <= max_len {
        input.to_owned()
    } else if max_len <= 1 {
        "…".to_owned()
    } else {
        format!("{}…", &input[..max_len.saturating_sub(1)])
    }
}

fn normalized_filter(input: &str) -> Option<String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_ascii_lowercase())
    }
}

fn render_filter_selector(ui: &mut egui::Ui, label: &str, value: &mut String, options: &[String]) {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.text_edit_singleline(value);
        if !options.is_empty() {
            ui.menu_button("Suggestions", |ui| {
                if ui.button("Clear selection").clicked() {
                    value.clear();
                    ui.close();
                }
                for option in options {
                    let option_label = option.as_str();
                    if ui.button(option_label).clicked() {
                        value.clear();
                        value.push_str(option_label);
                        ui.close();
                    }
                }
            });
        }
        if !value.is_empty()
            && ui
                .small_button("Clear")
                .on_hover_text("Clear this filter")
                .clicked()
        {
            value.clear();
        }
    });
}

fn filter_state_entries<'a>(
    pages: &'a [StatePageCache],
    current_entries: &'a [StateEntry],
    kind: StateQueryKind,
    search: Option<&str>,
    domain_filter: Option<&str>,
    owner_filter: Option<&str>,
    definition_filter: Option<&str>,
) -> (Vec<usize>, Vec<&'a StateEntry>) {
    let mut indices = Vec::new();
    for (idx, entry) in current_entries.iter().enumerate() {
        if MochiApp::state_entry_matches(
            entry,
            kind,
            search,
            domain_filter,
            owner_filter,
            definition_filter,
        ) {
            indices.push(idx);
        }
    }

    let matching_cached: Vec<&'a StateEntry> = if pages.is_empty() {
        current_entries
            .iter()
            .filter(|entry| {
                MochiApp::state_entry_matches(
                    entry,
                    kind,
                    search,
                    domain_filter,
                    owner_filter,
                    definition_filter,
                )
            })
            .collect()
    } else {
        pages
            .iter()
            .flat_map(|page| page.entries.iter())
            .filter(|entry| {
                MochiApp::state_entry_matches(
                    entry,
                    kind,
                    search,
                    domain_filter,
                    owner_filter,
                    definition_filter,
                )
            })
            .collect()
    };

    (indices, matching_cached)
}

fn compose_search_blob(alias: &str, summary: &str, detail: Option<&str>) -> String {
    let mut blob = String::new();
    blob.push_str(&alias.to_ascii_lowercase());
    blob.push(' ');
    blob.push_str(&summary.to_ascii_lowercase());
    if let Some(detail) = detail {
        blob.push(' ');
        blob.push_str(&detail.to_ascii_lowercase());
    }
    blob
}

fn serialize_event_filter(filter: &EventFilterState) -> Option<String> {
    json::to_string(&filter.to_json_value()).ok()
}

fn load_event_filter(storage: Option<&dyn Storage>) -> EventFilterState {
    let Some(storage) = storage else {
        return EventFilterState::default();
    };
    let Some(raw) = storage.get_string(EVENT_FILTER_STORAGE_KEY) else {
        return EventFilterState::default();
    };
    let Ok(parsed) = json::from_str::<Value>(&raw) else {
        return EventFilterState::default();
    };
    EventFilterState::from_json_value(&parsed).unwrap_or_default()
}

fn event_decode_stage_label(stage: EventDecodeStage) -> &'static str {
    match stage {
        EventDecodeStage::Frame => "frame",
        EventDecodeStage::Event => "payload",
        EventDecodeStage::Stream => "stream",
    }
}

fn event_json_value(event: &EventStreamEvent) -> Result<Value, String> {
    match event {
        EventStreamEvent::Event { event, .. } => json::to_value(event.as_ref())
            .map_err(|err| format!("Failed to encode event JSON: {err}")),
        _ => Err("Only structured events can be exported as JSON.".to_owned()),
    }
}

fn event_json_string(event: &EventStreamEvent) -> Result<String, String> {
    event_json_value(event).and_then(|value| {
        json::to_string_pretty(&value).map_err(|err| format!("Failed to encode event JSON: {err}"))
    })
}

fn collect_event_text(indices: &[usize], rendered: &[RenderedEventLine]) -> String {
    let mut lines = Vec::new();
    for &idx in indices.iter().rev() {
        if let Some(line) = rendered.get(idx) {
            let mut text = line.summary.clone();
            if let Some(detail) = &line.detail {
                text.push_str(" — ");
                text.push_str(detail);
            }
            if !line.badges.is_empty() {
                let badge_text = line
                    .badges
                    .iter()
                    .map(|badge| format!("{}={}", badge.label, badge.value))
                    .collect::<Vec<_>>()
                    .join(" • ");
                text.push_str(" — ");
                text.push_str(&badge_text);
            }
            lines.push(text);
        }
    }
    lines.join("\n")
}

fn collect_state_json(entries: &[&StateEntry]) -> Result<String, String> {
    if entries.is_empty() {
        return Err("No state entries selected for JSON export.".to_owned());
    }
    let mut values = Vec::with_capacity(entries.len());
    for entry in entries {
        let json_text = entry
            .json
            .as_ref()
            .ok_or_else(|| format!("State entry `{}` does not expose Norito JSON.", entry.title))?;
        let value = json::from_str(json_text)
            .map_err(|err| format!("Failed to parse Norito JSON for `{}`: {err}", entry.title))?;
        values.push(value);
    }
    json::to_string_pretty(&Value::Array(values))
        .map_err(|err| format!("Failed to encode Norito JSON array: {err}"))
}

fn collect_state_norito(entries: &[&StateEntry]) -> Result<String, String> {
    if entries.is_empty() {
        return Err("No state entries selected for Norito export.".to_owned());
    }
    let mut lines = Vec::with_capacity(entries.len());
    for entry in entries {
        let bytes = entry.norito_bytes.as_ref().ok_or_else(|| {
            format!(
                "State entry `{}` does not provide Norito-encoded bytes.",
                entry.title
            )
        })?;
        let hex = encode_upper(bytes);
        lines.push(format!("{}: {}", entry.title, hex));
    }
    Ok(lines.join("\n"))
}

fn state_export_directory(base_dir: Option<&Path>, suffix: &str) -> Result<PathBuf, String> {
    let dir = match base_dir {
        Some(path) => path.to_path_buf(),
        None => {
            let mut temp = std::env::temp_dir();
            temp.push("mochi_state");
            temp
        }
    };
    fs::create_dir_all(&dir)
        .map_err(|err| format!("Failed to prepare state export directory: {err}"))?;
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let mut path = dir.clone();
    path.push(format!("mochi_state_{timestamp}.{suffix}"));
    Ok(path)
}

fn save_state_json_to_file(
    entries: &[&StateEntry],
    base_dir: Option<&Path>,
) -> Result<PathBuf, String> {
    let path = state_export_directory(base_dir, "json")?;
    let contents = collect_state_json(entries)?;
    fs::write(&path, contents).map_err(|err| format!("Failed to write state export: {err}"))?;
    Ok(path)
}

fn save_state_norito_to_file(
    entries: &[&StateEntry],
    base_dir: Option<&Path>,
) -> Result<PathBuf, String> {
    let path = state_export_directory(base_dir, "norito")?;
    let contents = collect_state_norito(entries)?;
    fs::write(&path, contents).map_err(|err| format!("Failed to write state export: {err}"))?;
    Ok(path)
}

fn collect_log_text(entries: &[(usize, String)]) -> Result<String, String> {
    if entries.is_empty() {
        return Err("No log entries selected for export.".to_owned());
    }
    let mut buffer = String::new();
    for (index, (_, text)) in entries.iter().enumerate() {
        if index > 0 {
            buffer.push('\n');
        }
        buffer.push_str(text);
    }
    Ok(buffer)
}

fn save_logs_to_file(
    entries: &[(usize, String)],
    base_dir: Option<&Path>,
) -> Result<PathBuf, String> {
    if entries.is_empty() {
        return Err("No log entries selected for export.".to_owned());
    }
    let dir = match base_dir {
        Some(path) => path.to_path_buf(),
        None => {
            let mut temp = std::env::temp_dir();
            temp.push("mochi_logs");
            temp
        }
    };
    fs::create_dir_all(&dir)
        .map_err(|err| format!("Failed to prepare log export directory: {err}"))?;
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let mut path = dir.clone();
    path.push(format!("mochi_logs_{timestamp}.log"));
    let contents = collect_log_text(entries)?;
    fs::write(&path, contents).map_err(|err| format!("Failed to write log export: {err}"))?;
    Ok(path)
}

fn collect_event_json(indices: &[usize], events: &[EventDisplay]) -> Result<String, String> {
    let mut values = Vec::new();
    for &idx in indices.iter().rev() {
        let entry = events
            .get(idx)
            .ok_or_else(|| format!("Event index {idx} is out of range for buffered events."))?;
        match event_json_value(&entry.event) {
            Ok(value) => values.push(value),
            Err(err) if matches!(entry.event, EventStreamEvent::Event { .. }) => {
                return Err(err);
            }
            Err(_) => {}
        }
    }
    if values.is_empty() {
        return Err("No structured events are available for JSON export.".to_owned());
    }
    json::to_string_pretty(&Value::Array(values))
        .map_err(|err| format!("Failed to encode JSON array: {err}"))
}

#[derive(Debug, Clone)]
struct StatusError {
    info: ToriiErrorInfo,
}

trait ToriiErrorInfoUiExt {
    fn label(&self) -> String;
    fn color(&self) -> Color32;
}

fn reject_code_hint(code: &str) -> Option<&'static str> {
    match code {
        "PRTRY:TX_UNSUPPORTED_AUTHORITY" => Some("unsupported signer authority"),
        "PRTRY:TX_SIGNATURE_ALGO_DENIED" => Some("signature algorithm not permitted"),
        "PRTRY:TX_SIGNATURE_INVALID" => Some("signature verification failed"),
        "PRTRY:TX_SIGNATURE_MALFORMED" => Some("signature bytes malformed"),
        "PRTRY:TX_SIGNATURE_MISSING" => Some("missing signature bundle"),
        "PRTRY:TX_SIGNATURE_UNKNOWN_SIGNER" => Some("unknown signer in bundle"),
        "PRTRY:TX_SIGNATURE_INSUFFICIENT" => Some("insufficient multisig weight"),
        "PRTRY:TX_REJECTED" => Some("transaction rejected"),
        "PRTRY:INSTRUCTION_EXEC" => Some("instruction execution failed"),
        "PRTRY:IVM_EXEC" => Some("IVM execution failed"),
        "PRTRY:TRIGGER_EXEC" => Some("trigger execution failed"),
        "PRTRY:QUEUE_FULL" => Some("transaction queue full"),
        "PRTRY:QUEUE_RATE" => Some("transaction rate limited"),
        "PRTRY:ALREADY_COMMITTED" => Some("transaction already committed"),
        "PRTRY:ALREADY_ENQUEUED" => Some("transaction already enqueued"),
        "PRTRY:QUEUE_GOVERNANCE_INVALID" => Some("governance validation rejected"),
        "PRTRY:QUEUE_GOVERNANCE_REJECTED" => Some("governance proposal rejected"),
        "PRTRY:QUEUE_LANE_COMPLIANCE_DENIED" => Some("lane compliance denied"),
        "PRTRY:QUEUE_LANE_PRIVACY_PROOF_REJECTED" => Some("lane privacy proof rejected"),
        "PRTRY:QUEUE_UAID_NOT_BOUND" => Some("UAID not bound"),
        _ => {
            if code.starts_with("PRTRY:AXT_") || code.starts_with("AXT_") {
                return Some("AXT policy rejected");
            }
            if code.starts_with("PRTRY:QUEUE_") {
                return Some("transaction queue rejected");
            }
            if code.eq_ignore_ascii_case("certificate_expired") {
                return Some("certificate expired");
            }
            if code.eq_ignore_ascii_case("counter_conflict") {
                return Some("counter conflict");
            }
            if code.eq_ignore_ascii_case("allowance_exceeded") {
                return Some("allowance exceeded");
            }
            if code.eq_ignore_ascii_case("invoice_duplicate") {
                return Some("duplicate invoice");
            }
            None
        }
    }
}

impl ToriiErrorInfoUiExt for ToriiErrorInfo {
    fn label(&self) -> String {
        if let Some(code) = &self.reject_code {
            if let Some(hint) = reject_code_hint(code) {
                return format!("Rejected: {hint} ({code})");
            }
            return format!("Rejected: {code}");
        }
        if let Some(detail) = &self.detail {
            if detail.is_empty() {
                return self.message.clone();
            }
            if self.message.contains(detail) {
                return self.message.clone();
            }
            return format!("{} ({detail})", self.message);
        }
        self.message.clone()
    }

    fn color(&self) -> Color32 {
        match self.kind {
            ToriiErrorKind::Decode => Color32::from_rgb(200, 160, 64),
            ToriiErrorKind::Timeout => Color32::from_rgb(200, 160, 64),
            ToriiErrorKind::SmokeRejected => Color32::from_rgb(200, 64, 64),
            ToriiErrorKind::HttpTransport
            | ToriiErrorKind::UnexpectedStatus
            | ToriiErrorKind::WebSocket => Color32::from_rgb(200, 64, 64),
            ToriiErrorKind::InvalidBaseUrl
            | ToriiErrorKind::InvalidEndpoint
            | ToriiErrorKind::UnsupportedScheme
            | ToriiErrorKind::InvalidHeader
            | ToriiErrorKind::InvalidWebSocketRequest => Color32::from_gray(150),
        }
    }
}

impl StatusError {
    fn label(&self) -> String {
        self.info.label()
    }

    fn color(&self) -> Color32 {
        self.info.color()
    }
}

#[derive(Debug, Clone, Default)]
struct LaneCatalogSnapshot {
    lane_aliases: BTreeMap<u32, String>,
    lane_dataspaces: BTreeMap<u32, u32>,
    dataspace_aliases: BTreeMap<u32, String>,
    lane_count: Option<u32>,
}

impl LaneCatalogSnapshot {
    fn lane_ids(&self) -> BTreeSet<u32> {
        let mut ids: BTreeSet<u32> = self
            .lane_aliases
            .keys()
            .copied()
            .chain(self.lane_dataspaces.keys().copied())
            .collect();
        if ids.is_empty() {
            if let Some(count) = self.lane_count {
                if count > 0 {
                    ids.extend(0..count);
                }
            } else {
                ids.insert(0);
            }
        }
        ids
    }

    fn lane_alias(&self, lane_id: u32) -> String {
        self.lane_aliases
            .get(&lane_id)
            .cloned()
            .unwrap_or_else(|| default_lane_alias(lane_id))
    }

    fn lane_dataspace_id(&self, lane_id: u32) -> Option<u32> {
        self.lane_dataspaces.get(&lane_id).copied()
    }

    fn dataspace_label(&self, dataspace_id: Option<u32>) -> String {
        let Some(dataspace_id) = dataspace_id else {
            return "—".to_owned();
        };
        self.dataspace_aliases
            .get(&dataspace_id)
            .cloned()
            .or_else(|| {
                if dataspace_id == 0 {
                    Some("global".to_owned())
                } else {
                    None
                }
            })
            .unwrap_or_else(|| format!("id {dataspace_id}"))
    }
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

fn toml_u32(value: &TomlValue) -> Option<u32> {
    match value {
        TomlValue::Integer(raw) => u32::try_from(*raw).ok(),
        TomlValue::String(raw) => raw.parse::<u32>().ok(),
        _ => None,
    }
}

fn toml_string(value: &TomlValue) -> Option<String> {
    match value {
        TomlValue::String(raw) => Some(raw.clone()),
        _ => None,
    }
}

fn lane_catalog_snapshot(nexus: Option<&TomlTable>) -> LaneCatalogSnapshot {
    let mut snapshot = LaneCatalogSnapshot::default();
    let Some(nexus) = nexus else {
        return snapshot;
    };

    snapshot.lane_count = nexus.get("lane_count").and_then(toml_u32);

    if let Some(values) = nexus.get("dataspace_catalog").and_then(TomlValue::as_array) {
        for (idx, entry) in values.iter().enumerate() {
            let Some(table) = entry.as_table() else {
                continue;
            };
            let alias = table.get("alias").and_then(toml_string);
            let id = table
                .get("id")
                .or_else(|| table.get("index"))
                .and_then(toml_u32)
                .or_else(|| u32::try_from(idx).ok());
            if let (Some(alias), Some(id)) = (alias, id) {
                snapshot.dataspace_aliases.insert(id, alias);
            }
        }
    }

    if let Some(values) = nexus.get("lane_catalog").and_then(TomlValue::as_array) {
        for (idx, entry) in values.iter().enumerate() {
            let Some(table) = entry.as_table() else {
                continue;
            };
            let lane_id = table
                .get("index")
                .or_else(|| table.get("id"))
                .and_then(toml_u32)
                .or_else(|| u32::try_from(idx).ok())
                .unwrap_or(0);
            let alias = table
                .get("alias")
                .and_then(toml_string)
                .unwrap_or_else(|| default_lane_alias(lane_id));
            snapshot.lane_aliases.insert(lane_id, alias);

            let dataspace_id = table
                .get("dataspace_id")
                .and_then(toml_u32)
                .or_else(|| table.get("dataspace").and_then(toml_u32))
                .or_else(|| {
                    table
                        .get("dataspace")
                        .and_then(toml_string)
                        .and_then(|alias| {
                            snapshot
                                .dataspace_aliases
                                .iter()
                                .find(|(_, name)| *name == &alias)
                                .map(|(id, _)| *id)
                        })
                });
            if let Some(dataspace_id) = dataspace_id {
                snapshot.lane_dataspaces.insert(lane_id, dataspace_id);
            }
        }
    }

    if snapshot.lane_aliases.is_empty() {
        let count = snapshot.lane_count.unwrap_or(1);
        if count > 1 {
            for lane_id in 0..count {
                snapshot
                    .lane_aliases
                    .insert(lane_id, default_lane_alias(lane_id));
            }
        } else {
            snapshot.lane_aliases.insert(0, default_lane_alias(0));
        }
    } else if snapshot.lane_count.is_none() {
        let max = snapshot.lane_aliases.keys().copied().max().unwrap_or(0);
        snapshot.lane_count = Some(max.saturating_add(1));
    }

    snapshot
        .dataspace_aliases
        .entry(0)
        .or_insert_with(|| "global".to_owned());

    let lane_ids = snapshot.lane_ids();
    for lane_id in lane_ids {
        snapshot.lane_dataspaces.entry(lane_id).or_insert(0);
    }

    snapshot
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RelayIngestState {
    Waiting,
    MissingQc,
    MissingDa,
    MissingManifest,
    Ready,
}

impl RelayIngestState {
    fn label(self) -> &'static str {
        match self {
            RelayIngestState::Waiting => "waiting",
            RelayIngestState::MissingQc => "qc pending",
            RelayIngestState::MissingDa => "da pending",
            RelayIngestState::MissingManifest => "manifest pending",
            RelayIngestState::Ready => "ready",
        }
    }

    fn color(self) -> Color32 {
        match self {
            RelayIngestState::Ready => Color32::from_rgb(80, 160, 80),
            RelayIngestState::Waiting => Color32::from_gray(150),
            RelayIngestState::MissingQc
            | RelayIngestState::MissingDa
            | RelayIngestState::MissingManifest => Color32::from_rgb(200, 160, 64),
        }
    }
}

#[derive(Debug, Clone)]
struct LaneStatusRow {
    lane_id: u32,
    alias: String,
    dataspace: String,
    block_height: Option<u64>,
    relay_height: Option<u64>,
    relay_lag: Option<u64>,
    rbc_bytes: Option<u64>,
    da_cursor_epoch: Option<u64>,
    da_cursor_sequence: Option<u64>,
    relay_state: RelayIngestState,
}

impl LaneStatusRow {
    fn block_height_label(&self) -> String {
        self.block_height
            .map(|value| value.to_string())
            .unwrap_or_else(|| "—".to_owned())
    }

    fn relay_height_label(&self) -> String {
        self.relay_height
            .map(|value| value.to_string())
            .unwrap_or_else(|| "—".to_owned())
    }

    fn relay_lag_label(&self) -> String {
        self.relay_lag
            .map(|value| value.to_string())
            .unwrap_or_else(|| "—".to_owned())
    }

    fn rbc_bytes_label(&self) -> String {
        self.rbc_bytes
            .map(|value| value.to_string())
            .unwrap_or_else(|| "—".to_owned())
    }

    fn da_cursor_label(&self) -> String {
        match (self.da_cursor_epoch, self.da_cursor_sequence) {
            (Some(epoch), Some(seq)) => format!("e{epoch} s{seq}"),
            (Some(epoch), None) => format!("e{epoch}"),
            _ => "—".to_owned(),
        }
    }
}

#[derive(Debug, Clone, Default)]
struct PeerStatusView {
    last_snapshot: Option<ToriiStatusSnapshot>,
    last_error: Option<StatusError>,
    last_update: Option<Instant>,
    last_sumeragi: Option<SumeragiStatusWire>,
    last_metrics: Option<ToriiMetricsSnapshot>,
    last_metrics_error: Option<StatusError>,
}

impl PeerStatusView {
    fn record_snapshot(
        &mut self,
        snapshot: ToriiStatusSnapshot,
        sumeragi: Option<SumeragiStatusWire>,
        metrics: Option<ToriiMetricsSnapshot>,
        metrics_error: Option<ToriiErrorInfo>,
        timestamp: Instant,
    ) {
        self.last_snapshot = Some(snapshot);
        self.last_error = None;
        self.last_update = Some(timestamp);
        if let Some(snapshot) = sumeragi {
            self.last_sumeragi = Some(snapshot);
        }
        if let Some(metrics) = metrics {
            self.last_metrics = Some(metrics);
            self.last_metrics_error = None;
        } else if let Some(error) = metrics_error {
            self.last_metrics_error = Some(StatusError { info: error });
        }
    }

    fn record_error(&mut self, error: ToriiErrorInfo, timestamp: Instant) {
        self.last_error = Some(StatusError { info: error });
        self.last_update = Some(timestamp);
    }

    fn status_label(&self) -> (String, Color32) {
        if let Some(error) = &self.last_error {
            let label = truncate(&error.label(), 64);
            return (label, error.color());
        }
        if let Some(snapshot) = &self.last_snapshot {
            let status = &snapshot.status;
            let metrics = snapshot.metrics;
            let mut text = format!(
                "peers={} queue={} commit={}ms",
                status.peers, metrics.queue_size, metrics.commit_latency_ms
            );
            if let Some(delta) = self.delta_summary() {
                text.push(' ');
                text.push_str(&delta);
            }
            if let Some(sealed) = self.sealed_lane_count() {
                text.push(' ');
                text.push_str(&format!("sealed={sealed}"));
            }
            if self.is_stale() {
                text.push_str(" (stale)");
                return (truncate(&text, 80), Color32::from_gray(140));
            }
            let mut color = Color32::from_rgb(80, 160, 80);
            let queue = metrics.queue_size;
            let rejected_bias = metrics.tx_rejected_delta > metrics.tx_approved_delta
                && metrics.tx_rejected_delta > 0;
            if status.peers == 0 || queue > 25 || rejected_bias {
                color = Color32::from_rgb(200, 64, 64);
            } else if queue > 10 || metrics.tx_rejected_delta > 0 || metrics.view_change_delta > 0 {
                color = Color32::from_rgb(200, 160, 64);
            } else if self.sealed_lane_count().is_some() {
                color = Color32::from_rgb(220, 140, 80);
            }
            (truncate(&text, 80), color)
        } else {
            ("Unknown".to_owned(), Color32::from_gray(140))
        }
    }

    fn is_stale(&self) -> bool {
        self.last_update
            .map(|instant| instant.elapsed() > STATUS_POLL_INTERVAL * STATUS_STALE_MULTIPLIER)
            .unwrap_or(false)
    }

    fn delta_summary(&self) -> Option<String> {
        let snapshot = self.last_snapshot.as_ref()?;
        let metrics = snapshot.metrics;
        if !metrics.has_activity() {
            return None;
        }
        let mut parts = Vec::new();
        if metrics.tx_approved_delta > 0 || metrics.tx_rejected_delta > 0 {
            parts.push(format!(
                "tx +{} / -{}",
                metrics.tx_approved_delta, metrics.tx_rejected_delta
            ));
        }
        if metrics.queue_delta != 0 {
            parts.push(format!("queue {:+}", metrics.queue_delta));
        }
        if metrics.da_reschedule_delta > 0 {
            parts.push(format!("resched +{}", metrics.da_reschedule_delta));
        }
        if metrics.view_change_delta > 0 {
            parts.push(format!("view +{}", metrics.view_change_delta));
        }
        if parts.is_empty() {
            None
        } else {
            Some(format!("Δ{}", parts.join(" | ")))
        }
    }

    fn membership_summary(&self) -> Option<String> {
        let sumeragi = self.last_sumeragi.clone()?;
        let membership = sumeragi.membership;
        let hash = membership.view_hash.map(|bytes| {
            let hex = encode_upper(bytes);
            let truncated = hex.chars().take(16).collect::<String>();
            if hex.len() > 16 {
                format!("{truncated}...")
            } else {
                hex
            }
        });
        let hash_text = hash.unwrap_or_else(|| "—".to_owned());
        Some(format!(
            "Membership h{} v{} e{} hash {}",
            membership.height, membership.view, membership.epoch, hash_text
        ))
    }

    fn sealed_lane_count(&self) -> Option<u32> {
        let total = self
            .last_sumeragi
            .as_ref()
            .map(|wire| wire.lane_governance_sealed_total)
            .unwrap_or(0);
        (total > 0).then_some(total)
    }

    fn sealed_summary(&self) -> Option<String> {
        let sumeragi = self.last_sumeragi.as_ref()?;
        let total = sumeragi.lane_governance_sealed_total;
        if total == 0 {
            return None;
        }
        let aliases = &sumeragi.lane_governance_sealed_aliases;
        if aliases.is_empty() {
            return Some(format!("Sealed lanes: {total}"));
        }
        let mut truncated: Vec<String> = aliases
            .iter()
            .filter(|alias| !alias.is_empty())
            .map(|alias| truncate(alias, 24))
            .collect();
        if truncated.is_empty() {
            return Some(format!("Sealed lanes: {total}"));
        }
        let summary = if truncated.len() > 4 {
            let extra = truncated.len() - 3;
            let head = truncated.drain(..3).collect::<Vec<_>>().join(", ");
            format!("{head}, … +{extra}")
        } else {
            truncated.join(", ")
        };
        Some(format!("Sealed lanes: {total} ({summary})"))
    }

    fn consensus_queue_summary(&self) -> Option<String> {
        let metrics = self.last_metrics.as_ref()?;
        let depth = metrics.sumeragi_tx_queue_depth?;
        let capacity = metrics.sumeragi_tx_queue_capacity?;
        let mut summary = format!("Consensus queue {:.0}/{:.0}", depth, capacity);
        if metrics
            .sumeragi_tx_queue_saturated
            .map(|value| value >= 1.0)
            .unwrap_or(false)
        {
            summary.push_str(" (saturated)");
        }
        Some(summary)
    }

    fn storage_summary(&self) -> Option<String> {
        let metrics = self.last_metrics.as_ref()?;
        let hot = metrics.state_tiered_hot_entries?;
        let cold = metrics.state_tiered_cold_entries?;
        let mut summary = format!("Tiered state hot {:.0} • cold {:.0}", hot, cold);
        if let Some(bytes) = metrics.state_tiered_cold_bytes {
            let mib = bytes / (1024.0 * 1024.0);
            summary.push_str(&format!(" ({mib:.1} MiB cold)"));
        }
        Some(summary)
    }

    fn lane_status_rows(&self, catalog: &LaneCatalogSnapshot) -> Vec<LaneStatusRow> {
        let snapshot = self.last_snapshot.as_ref();
        let sumeragi = self.last_sumeragi.as_ref();
        let (Some(snapshot), Some(sumeragi)) = (snapshot, sumeragi) else {
            return Vec::new();
        };

        let mut commitments = BTreeMap::new();
        for commitment in &sumeragi.lane_commitments {
            commitments.insert(commitment.lane_id.as_u32(), commitment);
        }
        let mut relays = BTreeMap::new();
        for envelope in &sumeragi.lane_relay_envelopes {
            relays.insert(envelope.lane_id.as_u32(), envelope);
        }
        let mut governance = BTreeMap::new();
        for entry in &sumeragi.lane_governance {
            governance.insert(entry.lane_id.as_u32(), entry);
        }

        let mut da_cursors: BTreeMap<u32, (u64, u64)> = BTreeMap::new();
        for cursor in &snapshot.status.da_receipt_cursors {
            let entry = da_cursors
                .entry(cursor.lane_id)
                .or_insert((cursor.epoch, cursor.highest_sequence));
            if cursor.epoch > entry.0
                || (cursor.epoch == entry.0 && cursor.highest_sequence > entry.1)
            {
                *entry = (cursor.epoch, cursor.highest_sequence);
            }
        }

        let mut lane_ids = catalog.lane_ids();
        lane_ids.extend(commitments.keys().copied());
        lane_ids.extend(relays.keys().copied());
        lane_ids.extend(governance.keys().copied());
        lane_ids.extend(da_cursors.keys().copied());

        let mut rows = Vec::new();
        for lane_id in lane_ids {
            let alias = governance
                .get(&lane_id)
                .map(|entry| entry.alias.clone())
                .filter(|alias| !alias.is_empty())
                .unwrap_or_else(|| catalog.lane_alias(lane_id));
            let dataspace_id = catalog.lane_dataspace_id(lane_id).or_else(|| {
                relays
                    .get(&lane_id)
                    .and_then(|relay| u32::try_from(relay.dataspace_id.as_u64()).ok())
            });
            let dataspace = catalog.dataspace_label(dataspace_id);

            let block_height = commitments.get(&lane_id).map(|entry| entry.block_height);
            let relay_height = relays.get(&lane_id).map(|entry| entry.block_height);
            let relay_lag = match (block_height, relay_height) {
                (Some(block_height), Some(relay_height)) => {
                    Some(block_height.saturating_sub(relay_height))
                }
                _ => None,
            };
            let rbc_bytes = commitments
                .get(&lane_id)
                .map(|entry| entry.rbc_bytes_total)
                .or_else(|| relays.get(&lane_id).map(|entry| entry.rbc_bytes_total));
            let (da_cursor_epoch, da_cursor_sequence) = da_cursors
                .get(&lane_id)
                .map(|(epoch, sequence)| (Some(*epoch), Some(*sequence)))
                .unwrap_or((None, None));

            let relay_state =
                Self::relay_state_for_lane(relays.get(&lane_id), governance.get(&lane_id));

            rows.push(LaneStatusRow {
                lane_id,
                alias,
                dataspace,
                block_height,
                relay_height,
                relay_lag,
                rbc_bytes,
                da_cursor_epoch,
                da_cursor_sequence,
                relay_state,
            });
        }

        rows
    }

    fn relay_state_for_lane(
        relay: Option<&&LaneRelayEnvelope>,
        governance: Option<&&SumeragiLaneGovernance>,
    ) -> RelayIngestState {
        if let Some(relay) = relay {
            if relay.execution_qc.is_none() {
                return RelayIngestState::MissingQc;
            }
            if relay.da_commitment_hash.is_none() {
                return RelayIngestState::MissingDa;
            }
            if let Some(governance) = governance
                && governance.manifest_required
                && !governance.manifest_ready
            {
                return RelayIngestState::MissingManifest;
            }
            if relay.manifest_root.is_none()
                && governance
                    .map(|entry| entry.manifest_required)
                    .unwrap_or(false)
            {
                return RelayIngestState::MissingManifest;
            }
            RelayIngestState::Ready
        } else if governance
            .map(|entry| entry.manifest_required && !entry.manifest_ready)
            .unwrap_or(false)
        {
            RelayIngestState::MissingManifest
        } else {
            RelayIngestState::Waiting
        }
    }

    fn metrics_error_label(&self) -> Option<(String, Color32)> {
        let error = self.last_metrics_error.as_ref()?;
        Some((format!("Metrics: {}", error.label()), error.color()))
    }
}

#[derive(Debug, Clone)]
struct StatusHistoryEntry {
    timestamp: Instant,
    snapshot: ToriiStatusSnapshot,
    metrics: Option<ToriiMetricsSnapshot>,
}

#[derive(Debug, Clone)]
struct LogSnapshot {
    label: String,
    color: Color32,
    timestamp: Option<u128>,
}

impl Default for LogSnapshot {
    fn default() -> Self {
        Self {
            label: String::new(),
            color: Color32::from_gray(140),
            timestamp: None,
        }
    }
}

#[derive(Debug, Clone, Default)]
struct EventSnapshot {
    last_summary: Option<EventSummary>,
    last_error: Option<String>,
    connected: bool,
    last_update: Option<Instant>,
}

impl EventSnapshot {
    fn summary_text(&self) -> String {
        if let Some(summary) = &self.last_summary {
            let mut label = format!("{} — {}", summary.category.label(), summary.label);
            if let Some(detail) = &summary.detail {
                label.push_str(" | ");
                label.push_str(&truncate(detail, 120));
            }
            label
        } else {
            "—".to_owned()
        }
    }

    fn status_label(&self) -> (String, Color32) {
        if let Some(error) = &self.last_error {
            (truncate(error, 48), Color32::from_rgb(200, 64, 64))
        } else if self.connected {
            ("Connected".to_owned(), Color32::from_rgb(80, 160, 80))
        } else if self.last_summary.is_some() {
            ("Idle".to_owned(), Color32::from_rgb(200, 160, 64))
        } else {
            ("Inactive".to_owned(), Color32::from_gray(150))
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::VecDeque,
        num::NonZeroU64,
        path::Path,
        sync::Arc,
        time::{Duration, Instant},
    };

    use egui::{CentralPanel, Color32, Context, FontFamily, TextStyle};
    use iroha_data_model::{
        account::{AccountAdmissionMode, admission::ImplicitAccountFeeDestination},
        asset::{AssetDefinitionId, id::AssetId},
        block::{
            BlockHeader,
            consensus::{
                SumeragiDataspaceCommitment, SumeragiLaneCommitment, SumeragiLaneGovernance,
                SumeragiMembershipMismatchStatus, SumeragiMembershipStatus,
                SumeragiRuntimeUpgradeHook, SumeragiStatusWire,
            },
        },
        da::commitment::DaProofScheme,
        events::{
            EventBox,
            time::{TimeEvent, TimeInterval},
        },
        nexus::{DataSpaceId, LaneId, LaneRelayEnvelope, LaneStorageProfile, LaneVisibility},
        prelude::{AccountId, Hash, HashOf, Numeric},
        role::RoleId,
    };
    use iroha_test_samples::{ALICE_ID, ALICE_KEYPAIR};
    use mochi_core::{
        ExposedPrivateKey, TelemetryStatus, ToriiError, TxGossipSnapshot,
        torii::{GovernanceStatus, StatusMetrics, Uptime},
    };
    use norito::json::{self, Value};

    use super::{
        ActiveView, CliOverrides, InstructionPermission, MaintenanceCommand, MaintenanceState,
        MaintenanceTask, MochiApp, ProfilePreset, SignerEntryForm, SignerEntryState,
        StatePageCache, StateQueryKind, SupervisorBuilder, filter_state_entries,
        reset_cli_overrides_for_tests,
    };

    #[test]
    fn snapshot_label_preview_matches_expectations() {
        assert_eq!(
            MochiApp::preview_snapshot_label("My Snapshot!!!"),
            Some("my-snapshot".to_owned())
        );
        assert_eq!(
            MochiApp::preview_snapshot_label("   "),
            None,
            "blank labels should produce no preview"
        );
    }

    #[test]
    fn theme_palette_applied_to_visuals() {
        let mut app = MochiApp::default();
        let ctx = Context::default();
        app.ensure_theme(&ctx);
        let visuals = &ctx.style().visuals;
        let palette = MochiApp::palette();
        assert_eq!(visuals.panel_fill, palette.panel);
        assert_eq!(visuals.window_fill, palette.panel);
        assert_eq!(visuals.hyperlink_color, palette.accent);
        assert_eq!(visuals.selection.bg_fill, palette.accent);
        assert_eq!(visuals.widgets.inactive.bg_fill, palette.surface);
        assert_eq!(visuals.weak_text_color, Some(palette.text_muted));
        let style = ctx.style();
        let heading = style
            .text_styles
            .get(&TextStyle::Heading)
            .expect("heading style");
        assert_eq!(heading.family, FontFamily::Proportional);
        assert!((heading.size - 20.0).abs() < f32::EPSILON);
    }

    #[test]
    fn render_view_tabs_keeps_active_view() {
        let mut app = MochiApp::default();
        app.active_view = ActiveView::Logs;
        let ctx = Context::default();
        CentralPanel::default().show(&ctx, |ui| {
            app.render_view_tabs(ui);
        });
        assert_eq!(app.active_view, ActiveView::Logs);
    }

    #[test]
    fn render_overview_bar_smoke() {
        if !super::socket_bind_available() {
            eprintln!("Skipping overview bar test due to socket restrictions");
            return;
        }
        let _lock = env_lock().lock().expect("env lock");
        let temp = tempfile::tempdir().expect("temp dir");
        let kagami_stub = install_kagami_stub(temp.path());
        let irohad_stub = install_noop_stub(temp.path(), "irohad_ui_stub.sh");
        let _kagami_guard = TestEnvGuard::set("MOCHI_KAGAMI", &kagami_stub);
        let _irohad_guard = TestEnvGuard::set("MOCHI_IROHAD", &irohad_stub);
        let data_root = temp.path().join("ui-data");
        let _data_guard = TestEnvGuard::set("MOCHI_DATA_ROOT", &data_root);
        reset_cli_overrides_for_tests();

        let mut app = MochiApp::default();
        let mut supervisor = app.supervisor.take().expect("supervisor ready");
        let peer_rows = app.build_peer_rows(&supervisor);
        let metrics = app.collect_dashboard_metrics(&peer_rows);

        let ctx = Context::default();
        CentralPanel::default().show(&ctx, |ui| {
            app.render_overview_bar(ui, &mut supervisor, &peer_rows, &metrics);
        });

        assert!(!app.settings_dialog);
        app.supervisor = Some(supervisor);
    }

    #[test]
    fn cli_profile_override_reconfigures_builder() {
        let overrides = CliOverrides {
            profile: Some(NetworkProfile::from_preset(ProfilePreset::FourPeerBft)),
            ..Default::default()
        };
        let builder = overrides.apply_to(SupervisorBuilder::new(ProfilePreset::SinglePeer));
        assert_eq!(builder.profile().preset, Some(ProfilePreset::FourPeerBft));
        assert_eq!(builder.profile().topology.peer_count, 4);
    }

    #[test]
    fn maintenance_state_running_shows_spinner() {
        let banner = MaintenanceState::Running(MaintenanceTask::Snapshot)
            .banner()
            .expect("running state should surface banner");
        assert!(banner.show_spinner, "running banner should show spinner");
        assert!(
            !banner.dismissable,
            "running banner should not be dismissable"
        );
    }

    #[test]
    fn maintenance_state_completed_is_dismissable() {
        let banner = MaintenanceState::Completed {
            message: "Network reset complete".to_owned(),
        }
        .banner()
        .expect("completed state should surface banner");
        assert!(
            !banner.show_spinner,
            "completed banner should not show spinner"
        );
        assert!(banner.dismissable, "completed banner should be dismissable");
        assert!(
            banner.text.contains("Network reset"),
            "completed banner should retain completion message"
        );
    }

    #[test]
    fn entry_form_to_state_accepts_valid_inputs() {
        let form = SignerEntryForm {
            label: "Test signer".to_owned(),
            account: ALICE_ID.to_string(),
            private_key: ExposedPrivateKey(ALICE_KEYPAIR.private_key().clone()).to_string(),
            permissions: [InstructionPermission::MintAsset].into_iter().collect(),
            roles: String::new(),
        };

        let state = MochiApp::entry_form_to_state(&form)
            .expect("valid signer form should produce entry state");
        assert_eq!(state.label, "Test signer");
        assert_eq!(state.account, ALICE_ID.to_string());
        assert!(
            state
                .permissions
                .contains(&InstructionPermission::MintAsset)
        );
    }

    #[test]
    fn entry_form_to_state_rejects_missing_fields() {
        let form = SignerEntryForm {
            label: "Missing account".to_owned(),
            private_key: "deadbeef".to_owned(),
            ..Default::default()
        };
        let err = MochiApp::entry_form_to_state(&form)
            .expect_err("missing account should produce validation error");
        assert!(
            err.contains("Account identifier is required"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn signer_entries_to_signers_converts_entries() {
        let private_key = ExposedPrivateKey(ALICE_KEYPAIR.private_key().clone()).to_string();
        let entry = SignerEntryState {
            label: "Alice real".to_owned(),
            account: ALICE_ID.to_string(),
            private_key,
            permissions: InstructionPermission::all().into_iter().collect(),
            roles: String::new(),
        };

        let signers =
            MochiApp::signer_entries_to_signers(&[entry]).expect("expected successful conversion");
        assert_eq!(signers.len(), 1);
        let signer = &signers[0];
        assert_eq!(signer.label(), "Alice real");
        assert_eq!(signer.account_id(), &*ALICE_ID);
    }

    #[test]
    fn signer_entries_to_signers_rejects_empty_permissions() {
        let entry = SignerEntryState {
            label: "No perms".to_owned(),
            account: ALICE_ID.to_string(),
            private_key: ExposedPrivateKey(ALICE_KEYPAIR.private_key().clone()).to_string(),
            permissions: Default::default(),
            roles: String::new(),
        };

        let err = MochiApp::signer_entries_to_signers(&[entry])
            .expect_err("empty permission list should be rejected");
        assert!(
            err.contains("must permit at least one instruction"),
            "unexpected error message: {err}"
        );
    }

    #[test]
    fn parse_role_list_accepts_comma_separated_roles() {
        let roles =
            MochiApp::parse_role_list("auditor, basic_user").expect("role list should parse");
        assert_eq!(roles.len(), 2);
    }

    #[test]
    fn parse_role_list_rejects_invalid_roles() {
        let err =
            MochiApp::parse_role_list("not a role").expect_err("invalid roles should be rejected");
        assert!(
            err.contains("Invalid role"),
            "unexpected error message: {err}"
        );
    }

    #[test]
    fn parse_optional_u32_accepts_empty_and_numbers() {
        assert_eq!(
            MochiApp::parse_optional_u32("", "Max per tx").expect("empty ok"),
            None
        );
        assert_eq!(
            MochiApp::parse_optional_u32(" 7 ", "Max per tx").expect("parse ok"),
            Some(7)
        );
    }

    #[test]
    fn parse_lane_count_input_rejects_zero() {
        let err =
            MochiApp::parse_lane_count_input("0").expect_err("zero lane count should be rejected");
        assert!(err.contains("greater than zero"), "unexpected error: {err}");
    }

    #[test]
    fn parse_lane_count_input_accepts_numbers() {
        assert_eq!(
            MochiApp::parse_lane_count_input(" 3 ").expect("parse ok"),
            Some(3)
        );
        assert_eq!(
            MochiApp::parse_lane_count_input("").expect("empty ok"),
            None
        );
    }

    #[test]
    fn toml_helpers_extract_strings_and_numbers() {
        let value = TomlValue::String("alpha".to_owned());
        assert_eq!(toml_string(&value).as_deref(), Some("alpha"));
        assert_eq!(toml_u32(&TomlValue::Integer(7)), Some(7));
        assert_eq!(toml_u32(&TomlValue::String("12".to_owned())), Some(12));
    }

    #[test]
    fn lane_slug_matches_supervisor_logic() {
        assert_eq!(lane_slug("Core Lane", 0), "core_lane");
        assert_eq!(lane_slug("Gov+Ops", 1), "gov_ops");
        assert_eq!(lane_slug("---", 3), "lane3");
    }

    #[test]
    fn lane_catalog_snapshot_resolves_aliases_and_dataspaces() {
        let mut nexus = TomlTable::new();
        nexus.insert("enabled".into(), TomlValue::Boolean(true));
        nexus.insert("lane_count".into(), TomlValue::Integer(2));

        let mut lane0 = TomlTable::new();
        lane0.insert("index".into(), TomlValue::Integer(0));
        lane0.insert("alias".into(), TomlValue::String("core".into()));
        lane0.insert("dataspace".into(), TomlValue::String("global".into()));
        let mut lane1 = TomlTable::new();
        lane1.insert("index".into(), TomlValue::Integer(1));
        lane1.insert("alias".into(), TomlValue::String("ops".into()));
        lane1.insert("dataspace_id".into(), TomlValue::Integer(3));
        nexus.insert(
            "lane_catalog".into(),
            TomlValue::Array(vec![TomlValue::Table(lane0), TomlValue::Table(lane1)]),
        );

        let mut global = TomlTable::new();
        global.insert("alias".into(), TomlValue::String("global".into()));
        global.insert("id".into(), TomlValue::Integer(0));
        let mut private = TomlTable::new();
        private.insert("alias".into(), TomlValue::String("private".into()));
        private.insert("id".into(), TomlValue::Integer(3));
        nexus.insert(
            "dataspace_catalog".into(),
            TomlValue::Array(vec![TomlValue::Table(global), TomlValue::Table(private)]),
        );

        let snapshot = lane_catalog_snapshot(Some(&nexus));
        assert_eq!(snapshot.lane_alias(0), "core");
        assert_eq!(snapshot.lane_alias(1), "ops");
        assert_eq!(
            snapshot.dataspace_label(snapshot.lane_dataspace_id(1)),
            "private"
        );
    }

    #[test]
    fn lane_reset_candidates_skip_disabled_nexus() {
        let mut nexus = TomlTable::new();
        nexus.insert("enabled".into(), TomlValue::Boolean(false));
        nexus.insert("lane_count".into(), TomlValue::Integer(2));
        let candidates = MochiApp::lane_reset_candidates(Some(&nexus));
        assert!(
            candidates.is_empty(),
            "disabled nexus should yield no candidates"
        );
    }

    #[test]
    fn lane_metadata_for_id_reads_lane_fields() {
        let mut nexus = TomlTable::new();
        nexus.insert("enabled".into(), TomlValue::Boolean(true));

        let mut lane = TomlTable::new();
        lane.insert("index".into(), TomlValue::Integer(2));
        lane.insert("alias".into(), TomlValue::String("alpha".into()));
        lane.insert("dataspace".into(), TomlValue::String("global".into()));
        lane.insert("visibility".into(), TomlValue::String("restricted".into()));
        lane.insert(
            "storage".into(),
            TomlValue::String("commitment_only".into()),
        );
        lane.insert("proof_scheme".into(), TomlValue::String("kzg".into()));
        lane.insert("governance".into(), TomlValue::String("parliament".into()));
        let mut metadata = TomlTable::new();
        metadata.insert("tier".into(), TomlValue::String("gold".into()));
        lane.insert("metadata".into(), TomlValue::Table(metadata));
        nexus.insert(
            "lane_catalog".into(),
            TomlValue::Array(vec![TomlValue::Table(lane)]),
        );
        let mut dataspace = TomlTable::new();
        dataspace.insert("alias".into(), TomlValue::String("global".into()));
        dataspace.insert("id".into(), TomlValue::Integer(0));
        nexus.insert(
            "dataspace_catalog".into(),
            TomlValue::Array(vec![TomlValue::Table(dataspace)]),
        );

        let metadata = MochiApp::lane_metadata_for_id(Some(&nexus), 2);
        assert_eq!(metadata.id, LaneId::new(2));
        assert_eq!(metadata.alias, "alpha");
        assert_eq!(metadata.dataspace_id, DataSpaceId::new(0));
        assert_eq!(metadata.visibility, LaneVisibility::Restricted);
        assert_eq!(metadata.storage, LaneStorageProfile::CommitmentOnly);
        assert_eq!(metadata.proof_scheme, DaProofScheme::KzgBls12_381);
        assert_eq!(metadata.governance.as_deref(), Some("parliament"));
        assert_eq!(
            metadata.metadata.get("tier").map(String::as_str),
            Some("gold")
        );
    }

    #[test]
    fn lane_path_previews_include_slugged_paths() {
        if !super::socket_bind_available() {
            eprintln!("Skipping lane preview test due to socket restrictions");
            return;
        }
        let _lock = env_lock().lock().expect("env lock");
        let temp = tempfile::tempdir().expect("temp dir");
        let kagami_stub = install_kagami_stub(temp.path());
        let irohad_stub = install_noop_stub(temp.path(), "irohad_preview_stub.sh");
        let _kagami_guard = TestEnvGuard::set("MOCHI_KAGAMI", &kagami_stub);
        let _irohad_guard = TestEnvGuard::set("MOCHI_IROHAD", &irohad_stub);
        let data_root = temp.path().join("lane-preview-data");
        let _data_guard = TestEnvGuard::set("MOCHI_DATA_ROOT", &data_root);
        reset_cli_overrides_for_tests();

        let app = MochiApp::default();
        let mut lane = TomlTable::new();
        lane.insert("index".into(), TomlValue::Integer(0));
        lane.insert("alias".into(), TomlValue::String("Core Lane".into()));
        let lane_catalog = vec![TomlValue::Table(lane)];
        let previews = app
            .lane_path_previews(Some(1), Some(&lane_catalog))
            .expect("previews");
        assert_eq!(previews.len(), 1);
        let preview = &previews[0];
        let blocks = preview.blocks_dir.to_string_lossy();
        let merge = preview.merge_log.to_string_lossy();
        assert!(blocks.contains("lane_000_core_lane"));
        assert!(merge.contains("lane_000_core_lane_merge.log"));
    }

    #[test]
    fn reset_lane_with_lifecycle_hits_torii() {
        if !super::socket_bind_available() {
            eprintln!("Skipping lane reset test due to socket restrictions");
            return;
        }
        let _lock = env_lock().lock().expect("env lock");
        let (port, server_handle) = spawn_mock_torii_server();
        let port_str = port.to_string();

        let temp = tempfile::tempdir().expect("temp dir");
        let kagami_stub = install_kagami_stub(temp.path());
        let irohad_stub = install_noop_stub(temp.path(), "irohad_lane_reset_stub.sh");
        let _kagami_guard = TestEnvGuard::set("MOCHI_KAGAMI", &kagami_stub);
        let _irohad_guard = TestEnvGuard::set("MOCHI_IROHAD", &irohad_stub);
        let _torii_guard = TestEnvGuard::set_str("MOCHI_TORII_START", &port_str);
        let data_root = temp.path().join("lane-reset-data");
        let _data_guard = TestEnvGuard::set("MOCHI_DATA_ROOT", &data_root);
        reset_cli_overrides_for_tests();

        let mut app = MochiApp::default();
        let mut supervisor = app.supervisor.take().expect("supervisor ready");
        let handle = app.runtime.handle().clone();
        let result = MochiApp::reset_lane_with_lifecycle(&mut supervisor, 0, &handle);
        assert!(
            result.is_ok(),
            "lane reset should succeed against mock Torii: {result:?}"
        );
        let _ = server_handle.join();
    }

    #[test]
    fn reset_lane_with_lifecycle_inner_builds_retire_plan() {
        if !super::socket_bind_available() {
            eprintln!("Skipping lane reset inner test due to socket restrictions");
            return;
        }
        let _lock = env_lock().lock().expect("env lock");
        let temp = tempfile::tempdir().expect("temp dir");
        let kagami_stub = install_kagami_stub(temp.path());
        let irohad_stub = install_noop_stub(temp.path(), "irohad_lane_reset_inner_stub.sh");
        let _kagami_guard = TestEnvGuard::set("MOCHI_KAGAMI", &kagami_stub);
        let _irohad_guard = TestEnvGuard::set("MOCHI_IROHAD", &irohad_stub);
        let data_root = temp.path().join("lane-reset-inner-data");
        let _data_guard = TestEnvGuard::set("MOCHI_DATA_ROOT", &data_root);
        reset_cli_overrides_for_tests();

        let mut app = MochiApp::default();
        let mut supervisor = app.supervisor.take().expect("supervisor ready");
        let mut captured = None;
        let result = MochiApp::reset_lane_with_lifecycle_inner(&mut supervisor, 0, |_, plan| {
            captured = Some(plan);
            Ok(())
        });
        assert!(result.is_ok(), "lane reset inner should succeed");
        let plan = captured.expect("plan captured");
        assert_eq!(plan.additions.len(), 1);
        assert_eq!(plan.additions[0].id, LaneId::new(0));
        assert_eq!(plan.retire, vec![LaneId::new(0)]);
    }

    #[test]
    fn parse_min_initial_amounts_parses_lines() {
        let raw = "rose#wonderland = 5\ncabbage#wonderland = 1";
        let parsed = MochiApp::parse_min_initial_amounts(raw).expect("amounts parse");
        assert_eq!(parsed.len(), 2);
        let rose: AssetDefinitionId = "rose#wonderland".parse().unwrap();
        let cabbage: AssetDefinitionId = "cabbage#wonderland".parse().unwrap();
        assert_eq!(parsed.get(&rose), Some(&"5".parse::<Numeric>().unwrap()));
        assert_eq!(parsed.get(&cabbage), Some(&"1".parse::<Numeric>().unwrap()));
    }

    #[test]
    fn parse_multisig_policy_parses_json() {
        let json = r#"{
  "signatories": {
    "alice@wonderland": 1
  },
  "quorum": 1,
  "transaction_ttl_ms": 3600000
}"#;
        let spec = MochiApp::parse_multisig_policy(json).expect("policy should parse");
        assert!(spec.signatories.contains_key(&*ALICE_ID));
        assert_eq!(spec.quorum.get(), 1);
        assert_eq!(spec.transaction_ttl_ms.get(), 3_600_000);
    }

    #[test]
    fn admission_mode_label_matches_variants() {
        assert_eq!(
            MochiApp::admission_mode_label(AccountAdmissionMode::ImplicitReceive),
            "Implicit receive"
        );
        assert_eq!(
            MochiApp::admission_mode_label(AccountAdmissionMode::ExplicitOnly),
            "Explicit only"
        );
    }

    #[test]
    fn parse_account_admission_policy_builds_policy() {
        let mut app = MochiApp::default();
        app.composer_admission_domain = "wonderland".to_owned();
        app.composer_admission_mode = AccountAdmissionMode::ImplicitReceive;
        app.composer_admission_max_per_tx = "2".to_owned();
        app.composer_admission_max_per_block = "5".to_owned();
        app.composer_admission_fee_enabled = true;
        app.composer_admission_fee_asset = "rose#wonderland".to_owned();
        app.composer_admission_fee_amount = "1".to_owned();
        app.composer_admission_fee_destination_burn = false;
        app.composer_admission_fee_destination_account = "treasury@wonderland".to_owned();
        app.composer_admission_min_initial_amounts = "rose#wonderland = 5".to_owned();
        app.composer_admission_default_role = "basic_user".to_owned();

        let (domain, policy) = app
            .parse_account_admission_policy()
            .expect("policy should parse");
        assert_eq!(domain, "wonderland");
        assert_eq!(policy.mode, AccountAdmissionMode::ImplicitReceive);
        assert_eq!(policy.max_implicit_creations_per_tx, Some(2));
        assert_eq!(policy.max_implicit_creations_per_block, Some(5));
        let fee = policy.implicit_creation_fee.expect("fee configured");
        let asset: AssetDefinitionId = "rose#wonderland".parse().unwrap();
        assert_eq!(fee.asset_definition_id, asset);
        assert_eq!(fee.amount, "1".parse::<Numeric>().unwrap());
        let treasury: AccountId = "treasury@wonderland".parse().unwrap();
        match fee.destination {
            ImplicitAccountFeeDestination::Account(account) => assert_eq!(account, treasury),
            other => panic!("unexpected fee destination: {other:?}"),
        }
        let min_amounts = policy.min_initial_amounts;
        assert_eq!(min_amounts.len(), 1);
        let min_asset: AssetDefinitionId = "rose#wonderland".parse().unwrap();
        assert_eq!(
            min_amounts.get(&min_asset),
            Some(&"5".parse::<Numeric>().unwrap())
        );
        let expected_role: RoleId = "basic_user".parse().unwrap();
        assert_eq!(policy.default_role_on_create, Some(expected_role));
    }

    #[test]
    fn reject_code_hint_covers_queue_and_axt() {
        assert_eq!(
            super::reject_code_hint("PRTRY:QUEUE_FULL"),
            Some("transaction queue full")
        );
        assert_eq!(
            super::reject_code_hint("PRTRY:AXT_HANDLE_ERA"),
            Some("AXT policy rejected")
        );
    }

    #[test]
    fn maintenance_export_snapshot_creates_snapshot_directory() {
        if !super::socket_bind_available() {
            eprintln!("Skipping snapshot maintenance test due to socket restrictions");
            return;
        }
        let _lock = env_lock().lock().expect("env lock");
        let temp = tempfile::tempdir().expect("temp dir");

        let kagami_stub = install_kagami_stub(temp.path());
        let irohad_stub = install_noop_stub(temp.path(), "irohad_snapshot_stub.sh");
        let log_path = temp.path().join("kagami_snapshot.log");

        let _log_guard = TestEnvGuard::set("MOCHI_TEST_KAGAMI_LOG", &log_path);
        let _kagami_guard = TestEnvGuard::set("MOCHI_KAGAMI", &kagami_stub);
        let _irohad_guard = TestEnvGuard::set("MOCHI_IROHAD", &irohad_stub);

        let data_root = temp.path().join("snapshot-data");
        let _data_guard = TestEnvGuard::set("MOCHI_DATA_ROOT", &data_root);

        let mut app = MochiApp::default();
        let mut supervisor_slot = app.supervisor.take();
        let initial_invocations = genesis_invocation_count(&log_path);

        assert!(
            app.begin_maintenance(MaintenanceTask::Snapshot),
            "snapshot maintenance should start when idle"
        );
        let label = "Smoke Snapshot 42".to_owned();
        app.maintenance_command = Some(MaintenanceCommand::ExportSnapshot {
            label: Some(label.clone()),
        });
        app.schedule_pending_maintenance(&mut supervisor_slot);

        for _ in 0..100 {
            app.poll_maintenance_updates(&mut supervisor_slot);
            if !matches!(app.maintenance_state, MaintenanceState::Running(_)) {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }

        assert!(
            !matches!(app.maintenance_state, MaintenanceState::Running(_)),
            "snapshot maintenance did not finish in time"
        );
        assert!(
            supervisor_slot.is_some(),
            "supervisor should be restored after maintenance"
        );

        app.supervisor = supervisor_slot;
        let supervisor = app.supervisor.as_ref().expect("supervisor restored");
        match &app.maintenance_state {
            MaintenanceState::Completed { message } => {
                assert!(
                    message.contains("Snapshot exported"),
                    "expected completion message, got `{message}`"
                );
            }
            other => panic!("snapshot maintenance did not complete: {other:?}"),
        }

        let snapshots_dir = supervisor.paths().snapshots_dir();
        let snapshot_slug = "smoke-snapshot-42";
        let snapshot_root = snapshots_dir.join(snapshot_slug);
        assert!(
            snapshot_root.exists(),
            "expected snapshot directory {}",
            snapshot_root.display()
        );
        let metadata_bytes =
            fs::read(snapshot_root.join("metadata.json")).expect("read snapshot metadata");
        let metadata: Value = json::from_slice(&metadata_bytes).expect("parse snapshot metadata");
        assert_eq!(
            metadata.get("snapshot").and_then(Value::as_str),
            Some(snapshot_slug),
            "metadata should record sanitized snapshot slug"
        );
        assert_eq!(
            metadata.get("peer_count").and_then(Value::as_u64),
            Some(supervisor.peers().len() as u64),
            "metadata peer_count should match supervisor peer count"
        );

        let final_invocations = genesis_invocation_count(&log_path);
        assert_eq!(
            final_invocations, initial_invocations,
            "exporting snapshots must not trigger additional kagami invocations"
        );
    }

    #[test]
    fn maintenance_reset_invokes_kagami_and_cleans_storage() {
        if !super::socket_bind_available() {
            eprintln!("Skipping reset maintenance test due to socket restrictions");
            return;
        }
        let _lock = env_lock().lock().expect("env lock");
        let temp = tempfile::tempdir().expect("temp dir");

        let kagami_stub = install_kagami_stub(temp.path());
        let irohad_stub = install_noop_stub(temp.path(), "irohad_reset_stub.sh");
        let log_path = temp.path().join("kagami_reset.log");

        let _log_guard = TestEnvGuard::set("MOCHI_TEST_KAGAMI_LOG", &log_path);
        let _kagami_guard = TestEnvGuard::set("MOCHI_KAGAMI", &kagami_stub);
        let _irohad_guard = TestEnvGuard::set("MOCHI_IROHAD", &irohad_stub);

        let data_root = temp.path().join("reset-data");
        let _data_guard = TestEnvGuard::set("MOCHI_DATA_ROOT", &data_root);

        let mut app = MochiApp::default();
        let mut supervisor_slot = app.supervisor.take();

        {
            let supervisor = supervisor_slot.as_ref().expect("supervisor ready");
            for peer in supervisor.peers() {
                let storage_dir = supervisor.paths().peer_dir(peer.alias()).join("storage");
                fs::create_dir_all(&storage_dir).expect("ensure storage directory exists");
                fs::write(storage_dir.join("junk.bin"), b"junk").expect("write junk file");
            }
        }

        let baseline_invocations = genesis_invocation_count(&log_path);

        assert!(
            app.begin_maintenance(MaintenanceTask::Reset),
            "reset maintenance should start when idle"
        );
        app.maintenance_command = Some(MaintenanceCommand::Reset);
        app.schedule_pending_maintenance(&mut supervisor_slot);

        for _ in 0..120 {
            app.poll_maintenance_updates(&mut supervisor_slot);
            if !matches!(app.maintenance_state, MaintenanceState::Running(_)) {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }

        assert!(
            !matches!(app.maintenance_state, MaintenanceState::Running(_)),
            "reset maintenance did not finish in time"
        );
        assert!(
            supervisor_slot.is_some(),
            "supervisor should be restored after maintenance"
        );

        app.supervisor = supervisor_slot;
        let supervisor = app.supervisor.as_ref().expect("supervisor restored");
        match &app.maintenance_state {
            MaintenanceState::Completed { message } => {
                assert!(
                    message.contains("reset"),
                    "expected reset completion message, got `{message}`"
                );
            }
            other => panic!("reset maintenance did not complete: {other:?}"),
        }

        for peer in supervisor.peers() {
            let storage_dir = supervisor.paths().peer_dir(peer.alias()).join("storage");
            assert!(
                !storage_dir.join("junk.bin").exists(),
                "storage should remove junk for {}",
                peer.alias()
            );
            let snapshot_dir = storage_dir.join("snapshot");
            assert!(
                snapshot_dir.exists(),
                "snapshot directory should exist for {}",
                peer.alias()
            );
            let mut entries = fs::read_dir(&snapshot_dir).expect("snapshot dir entries");
            assert!(
                entries.next().is_none(),
                "snapshot directory should remain empty for {}",
                peer.alias()
            );
        }

        let final_invocations = genesis_invocation_count(&log_path);
        assert!(
            final_invocations > baseline_invocations,
            "wipe & re-genesis should invoke kagami again"
        );
    }

    #[test]
    fn maintenance_restore_snapshot_rehydrates_storage() {
        if !super::socket_bind_available() {
            eprintln!("Skipping restore maintenance test due to socket restrictions");
            return;
        }
        let _lock = env_lock().lock().expect("env lock");
        let temp = tempfile::tempdir().expect("temp dir");

        let kagami_stub = install_kagami_stub(temp.path());
        let irohad_stub = install_noop_stub(temp.path(), "irohad_restore_stub.sh");
        let log_path = temp.path().join("kagami_restore.log");

        let _log_guard = TestEnvGuard::set("MOCHI_TEST_KAGAMI_LOG", &log_path);
        let _kagami_guard = TestEnvGuard::set("MOCHI_KAGAMI", &kagami_stub);
        let _irohad_guard = TestEnvGuard::set("MOCHI_IROHAD", &irohad_stub);

        let data_root = temp.path().join("restore-data");
        let _data_guard = TestEnvGuard::set("MOCHI_DATA_ROOT", &data_root);

        let mut app = MochiApp::default();
        let mut supervisor_slot = app.supervisor.take();
        let supervisor = supervisor_slot.as_mut().expect("supervisor ready");

        let peer = supervisor.peers().first().expect("at least one peer");
        let storage_dir = supervisor.paths().peer_dir(peer.alias()).join("storage");
        fs::create_dir_all(&storage_dir).expect("create storage dir");
        let marker_path = storage_dir.join("marker.txt");
        fs::write(&marker_path, b"snapshot-data").expect("write snapshot data");

        let snapshot_root = supervisor
            .export_snapshot(Some("Restore Snapshot 7"))
            .expect("export snapshot");
        let slug = snapshot_root
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_string();

        fs::write(&marker_path, b"mutated-data").expect("mutate storage marker");

        assert!(
            app.begin_maintenance(MaintenanceTask::Restore),
            "restore maintenance should start when idle"
        );
        let target = slug.clone();
        app.maintenance_command = Some(MaintenanceCommand::Restore { target });
        app.schedule_pending_maintenance(&mut supervisor_slot);

        for _ in 0..120 {
            app.poll_maintenance_updates(&mut supervisor_slot);
            if !matches!(app.maintenance_state, MaintenanceState::Running(_)) {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }

        assert!(
            !matches!(app.maintenance_state, MaintenanceState::Running(_)),
            "restore maintenance did not finish in time"
        );
        assert!(
            supervisor_slot.is_some(),
            "supervisor should be restored after maintenance"
        );

        app.supervisor = supervisor_slot;
        let supervisor = app.supervisor.as_ref().expect("supervisor restored");
        match &app.maintenance_state {
            MaintenanceState::Completed { message } => {
                assert!(
                    message.contains("restored"),
                    "expected restore completion message, got `{message}`"
                );
            }
            other => panic!("restore maintenance did not complete: {other:?}"),
        }

        let restored_marker =
            fs::read(marker_path).expect("read storage marker after restore completed");
        assert_eq!(
            restored_marker, b"snapshot-data",
            "restore should rehydrate storage contents"
        );

        let snapshots_dir = supervisor.paths().snapshots_dir();
        assert!(
            snapshots_dir.join(&slug).exists(),
            "snapshot should remain available for future restores"
        );
    }

    fn sample_sumeragi_status_wire() -> SumeragiStatusWire {
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
            leader_index: 1,
            highest_qc_height: 10,
            highest_qc_view: 4,
            highest_qc_subject: None,
            locked_qc_height: 9,
            locked_qc_view: 3,
            locked_qc_subject: None,
            commit_certificate: SumeragiCommitCertificateStatus::default(),
            commit_quorum: SumeragiCommitQuorumStatus::default(),
            view_change_proof_accepted_total: 5,
            view_change_proof_stale_total: 6,
            view_change_proof_rejected_total: 7,
            view_change_suggest_total: 8,
            view_change_install_total: 9,
            view_change_causes: SumeragiViewChangeCauseStatus::default(),
            gossip_fallback_total: 2,
            block_created_dropped_by_lock_total: 1,
            block_created_hint_mismatch_total: 0,
            block_created_proposal_mismatch_total: 0,
            validation_reject_total: 0,
            validation_reject_reason: None,
            validation_rejects: SumeragiValidationRejectStatus::default(),
            peer_key_policy: Default::default(),
            block_sync_roster: SumeragiBlockSyncRosterStatus::default(),
            pacemaker_backpressure_deferrals_total: 1,
            commit_pipeline_tick_total: 0,
            da_reschedule_total: 0,
            missing_block_fetch: SumeragiMissingBlockFetchStatus {
                total: 0,
                last_targets: 0,
                last_dwell_ms: 0,
            },
            da_gate: SumeragiDaGateStatus {
                reason: SumeragiDaGateReason::None,
                last_satisfied: SumeragiDaGateSatisfaction::None,
                missing_availability_total: 0,
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
                evictions_total: 0,
                recent_evictions: Vec::new(),
            },
            pending_rbc: SumeragiPendingRbcStatus::default(),
            tx_queue_depth: 4,
            tx_queue_capacity: 128,
            tx_queue_saturated: false,
            epoch_length_blocks: 3600,
            epoch_commit_deadline_offset: 120,
            epoch_reveal_deadline_offset: 160,
            prf_epoch_seed: Some([0x55; 32]),
            prf_height: 10,
            prf_view: 4,
            vrf_penalty_epoch: 2,
            vrf_committed_no_reveal_total: 1,
            vrf_no_participation_total: 0,
            vrf_late_reveals_total: 1,
            consensus_penalties_applied_total: 0,
            consensus_penalties_pending: 0,
            vrf_penalties_applied_total: 0,
            vrf_penalties_pending: 0,
            membership: SumeragiMembershipStatus {
                height: 10,
                view: 4,
                epoch: 2,
                view_hash: Some([0xAB; 32]),
            },
            membership_mismatch: SumeragiMembershipMismatchStatus::default(),
            lane_commitments: vec![SumeragiLaneCommitment {
                block_height: 10,
                lane_id: LaneId::new(0),
                tx_count: 3,
                total_chunks: 4,
                rbc_bytes_total: 384,
                teu_total: 96,
                block_hash: HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed(
                    [0x90; Hash::LENGTH],
                )),
            }],
            dataspace_commitments: vec![SumeragiDataspaceCommitment {
                block_height: 10,
                lane_id: LaneId::new(0),
                dataspace_id: DataSpaceId::new(2),
                tx_count: 1,
                total_chunks: 2,
                rbc_bytes_total: 128,
                teu_total: 32,
                block_hash: HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed(
                    [0x91; Hash::LENGTH],
                )),
            }],
            lane_settlement_commitments: Vec::new(),
            lane_relay_envelopes: Vec::new(),
            lane_governance_sealed_total: 0,
            lane_governance_sealed_aliases: Vec::new(),
            lane_governance: vec![SumeragiLaneGovernance {
                lane_id: LaneId::new(0),
                alias: "alpha".to_owned(),
                governance: Some("parliament".to_owned()),
                manifest_required: true,
                manifest_ready: true,
                manifest_path: Some("/etc/iroha/lanes/alpha.json".to_owned()),
                validator_ids: vec!["alice@test".to_owned(), "bob@test".to_owned()],
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
        }
    }

    #[test]
    fn ensure_selection_picks_first_available() {
        let mut selection = Some("missing".to_owned());
        let aliases = vec!["alpha".to_owned(), "beta".to_owned()];
        MochiApp::ensure_selection(&mut selection, &aliases);
        assert_eq!(selection, Some("alpha".to_owned()));
    }

    #[test]
    fn collect_event_text_includes_summary_and_detail() {
        let rendered = vec![
            RenderedEventLine::new(
                "alpha",
                "[alpha] Transaction Rejected — ABC".to_owned(),
                Some("hash=ABCDEF • raw=128B".to_owned()),
                Color32::from_rgb(225, 90, 90),
                RenderedEventKind::Category(EventCategory::Pipeline),
            )
            .with_badges(vec![EventBadge::new(
                "reason",
                "invalid_signature".to_owned(),
                None,
                Color32::from_rgb(255, 140, 140),
            )]),
            RenderedEventLine::new(
                "alpha",
                "[alpha] Block Committed — height 1".to_owned(),
                None,
                Color32::from_rgb(110, 200, 220),
                RenderedEventKind::Category(EventCategory::Pipeline),
            ),
        ];
        let export = collect_event_text(&[0, 1], &rendered);
        let lines: Vec<&str> = export.lines().collect();
        assert_eq!(lines.len(), 2, "expected two exported lines");
        assert_eq!(
            lines[0], "[alpha] Block Committed — height 1",
            "newest event should appear first"
        );
        assert_eq!(
            lines[1],
            "[alpha] Transaction Rejected — ABC — hash=ABCDEF • raw=128B — reason=invalid_signature"
        );
    }

    #[test]
    fn collect_event_json_serializes_structured_events() {
        let time_interval = TimeInterval::new(Duration::from_millis(10), Duration::from_millis(5));
        let time_event = EventBox::Time(TimeEvent::new(time_interval));
        let summary = EventSummary {
            category: EventCategory::Time,
            label: "Interval".to_owned(),
            detail: Some("since=10ms length=5ms".to_owned()),
        };
        let structured_event = EventDisplay {
            alias: Some("alpha".to_owned()),
            event: EventStreamEvent::Event {
                summary,
                event: Arc::new(time_event),
                raw_len: 42,
            },
        };
        let as_json = collect_event_json(&[0], std::slice::from_ref(&structured_event))
            .expect("structured event should export to JSON");
        let parsed: Value =
            json::from_str(&as_json).expect("exported JSON should be parseable via Norito");
        let array = parsed.as_array().expect("export must be a JSON array");
        assert_eq!(array.len(), 1, "expected a single exported event payload");

        let text_event = EventDisplay {
            alias: Some("alpha".to_owned()),
            event: EventStreamEvent::Text {
                text: "note".to_owned(),
            },
        };
        assert!(
            collect_event_json(&[1], &[structured_event, text_event]).is_err(),
            "JSON export must fail when no structured events match"
        );
    }

    fn sample_state_entry(title: &str, bytes: Vec<u8>) -> super::StateEntry {
        let json_payload = format!("{{\"title\":\"{title}\"}}");
        super::StateEntry {
            title: title.to_owned(),
            subtitle: "subtitle".to_owned(),
            detail: "detail".to_owned(),
            raw: "{}".to_owned(),
            domain: None,
            domain_lower: None,
            owner: None,
            owner_lower: None,
            asset_definition: None,
            asset_definition_lower: None,
            json: Some(json_payload),
            norito_bytes: Some(bytes),
            search_blob: title.to_ascii_lowercase(),
        }
    }

    fn sample_state_entry_with_domain(
        title: &str,
        domain: &str,
        bytes: Vec<u8>,
    ) -> super::StateEntry {
        let mut entry = sample_state_entry(title, bytes);
        let lower = domain.to_ascii_lowercase();
        entry.domain = Some(domain.to_owned());
        entry.domain_lower = Some(lower.clone());
        entry.search_blob.push(' ');
        entry.search_blob.push_str(&lower);
        entry
    }

    #[test]
    fn collect_state_json_exports_array() {
        let entries = [
            sample_state_entry("alice@test", vec![0xAA, 0x01]),
            sample_state_entry("bob@test", vec![0xBB, 0x02]),
        ];
        let refs: Vec<&super::StateEntry> = entries.iter().collect();
        let json_text = super::collect_state_json(&refs).expect("export filtered state to json");
        let parsed: Value = json::from_str(&json_text).expect("parse exported Norito JSON");
        let array = parsed
            .as_array()
            .expect("exported state should be a JSON array");
        assert_eq!(array.len(), 2, "expected two exported state entries");
        assert_eq!(
            array[0].get("title").and_then(Value::as_str),
            Some("alice@test")
        );
        assert_eq!(
            array[1].get("title").and_then(Value::as_str),
            Some("bob@test")
        );
    }

    #[test]
    fn collect_state_norito_exports_hex_dump() {
        let entries = [sample_state_entry("alice@test", vec![0xAB, 0xCD])];
        let refs: Vec<&super::StateEntry> = entries.iter().collect();
        let dump = super::collect_state_norito(&refs).expect("export filtered state to norito");
        assert!(
            dump.contains("alice@test"),
            "export should include the entry title"
        );
        let mut parts = dump.split(':');
        let _ = parts.next().expect("title prefix should be present");
        let hex_section = parts
            .next()
            .expect("hex suffix should be present")
            .trim()
            .to_owned();
        assert!(
            !hex_section.is_empty(),
            "hex section should not be empty for Norito export"
        );
        assert!(
            hex_section.chars().all(|c| c.is_ascii_hexdigit()),
            "hex section should contain only hexadecimal digits"
        );
    }

    #[test]
    fn save_state_json_to_file_writes_filtered_entries() {
        let entries = [sample_state_entry("alice@test", vec![0x01, 0x02])];
        let refs: Vec<&super::StateEntry> = entries.iter().collect();
        let dir = tempfile::tempdir().expect("tempdir");
        let path =
            super::save_state_json_to_file(&refs, Some(dir.path())).expect("export state json");
        assert!(
            path.starts_with(dir.path()),
            "export path should reside within provided directory"
        );
        let written = std::fs::read_to_string(&path).expect("read exported state json");
        assert!(
            written.contains("alice@test"),
            "exported JSON should include entry identifier"
        );
    }

    #[test]
    fn save_state_json_to_file_rejects_empty_entries() {
        let entries: Vec<&super::StateEntry> = Vec::new();
        assert!(
            super::save_state_json_to_file(&entries, None).is_err(),
            "export should fail when no state entries are selected"
        );
    }

    #[test]
    fn save_state_norito_to_file_writes_filtered_entries() {
        let entries = [sample_state_entry("alice@test", vec![0x0A, 0x0B])];
        let refs: Vec<&super::StateEntry> = entries.iter().collect();
        let dir = tempfile::tempdir().expect("tempdir");
        let path =
            super::save_state_norito_to_file(&refs, Some(dir.path())).expect("export state norito");
        assert!(
            path.starts_with(dir.path()),
            "export path should reside within provided directory"
        );
        let written = std::fs::read_to_string(&path).expect("read exported state norito");
        assert!(
            written.contains("alice@test"),
            "exported Norito dump should include entry identifier"
        );
    }

    #[test]
    fn save_state_norito_to_file_rejects_empty_entries() {
        let entries: Vec<&super::StateEntry> = Vec::new();
        assert!(
            super::save_state_norito_to_file(&entries, None).is_err(),
            "export should fail when no state entries are selected"
        );
    }

    #[test]
    fn state_tab_select_page_updates_entries_and_remaining() {
        let mut tab = super::StateTabState::new(StateQueryKind::Accounts);
        let first = sample_state_entry("alice@test", vec![0xAA]);
        let second = sample_state_entry("bob@test", vec![0xBB]);
        tab.pages = vec![
            StatePageCache {
                entries: vec![first.clone()],
                remaining: 2,
            },
            StatePageCache {
                entries: vec![second.clone()],
                remaining: 0,
            },
        ];

        tab.select_page(0);
        assert_eq!(tab.entries.len(), 1, "expected a single entry on page 0");
        assert_eq!(
            tab.entries.first().map(|entry| entry.title.as_str()),
            Some("alice@test"),
            "selecting first page should surface corresponding entries"
        );
        assert_eq!(
            tab.remaining,
            Some(2),
            "remaining counter should be preserved when greater than zero"
        );

        tab.select_page(1);
        assert_eq!(tab.entries.len(), 1, "expected a single entry on page 1");
        assert_eq!(
            tab.entries.first().map(|entry| entry.title.as_str()),
            Some("bob@test"),
            "switching pages should update visible entries"
        );
        assert_eq!(
            tab.remaining, None,
            "remaining counter should drop to None when reported as zero"
        );
    }

    #[test]
    fn state_tabs_reset_results_preserves_filters() {
        let mut tabs = super::StateTabs::default();
        let tab = tabs.get_mut(StateQueryKind::Accounts);
        tab.filter.search = "alice".to_owned();
        tab.entries
            .push(sample_state_entry("alice@test", vec![0x01]));
        tab.pages.push(StatePageCache {
            entries: vec![sample_state_entry("alice@test", vec![0x02])],
            remaining: 1,
        });
        let peer_tab = tabs.get_mut(StateQueryKind::Peers);
        peer_tab
            .entries
            .push(sample_state_entry("peer#0", vec![0x03]));
        peer_tab.pages.push(StatePageCache {
            entries: vec![sample_state_entry("peer#1", vec![0x04])],
            remaining: 0,
        });

        tabs.reset_results_for_all();
        let tab = tabs.get(StateQueryKind::Accounts);
        assert!(
            tab.entries.is_empty(),
            "reset should drop cached entries for each tab"
        );
        assert!(
            tab.pages.is_empty(),
            "reset should clear page caches for each tab"
        );
        assert_eq!(
            tab.filter.search, "alice",
            "reset should not discard the active search query"
        );
        let peer_tab = tabs.get(StateQueryKind::Peers);
        assert!(
            peer_tab.entries.is_empty(),
            "reset should drop cached entries for the peers tab"
        );
        assert!(
            peer_tab.pages.is_empty(),
            "reset should clear cached pages for the peers tab"
        );
    }

    #[test]
    fn state_filter_adapts_peer_fields() {
        let mut filter = super::StateFilter {
            search: "peer".to_owned(),
            domain: "wonderland".to_owned(),
            owner: "alice@test".to_owned(),
            asset_definition: "rose#wonderland".to_owned(),
        };
        filter.adapt_to_kind(StateQueryKind::Peers);
        assert_eq!(
            filter.search, "peer",
            "adaptation should not clear the free-form search query"
        );
        assert!(
            filter.domain.is_empty(),
            "peer filter should not retain domain constraints"
        );
        assert!(
            filter.owner.is_empty(),
            "peer filter should not retain owner constraints"
        );
        assert!(
            filter.asset_definition.is_empty(),
            "peer filter should not retain asset definition constraints"
        );
    }

    #[test]
    fn filter_state_entries_collects_cached_matches() {
        let entry_page0 = sample_state_entry("alice@test", vec![0xAA]);
        let entry_page1 = sample_state_entry("bob@test", vec![0xBB]);
        let pages = vec![
            StatePageCache {
                entries: vec![entry_page0.clone()],
                remaining: 1,
            },
            StatePageCache {
                entries: vec![entry_page1.clone()],
                remaining: 0,
            },
        ];
        let current_entries = vec![entry_page0];
        let (page_indices, cached_matches) = filter_state_entries(
            &pages,
            &current_entries,
            StateQueryKind::Accounts,
            Some("bob"),
            None,
            None,
            None,
        );
        assert!(
            page_indices.is_empty(),
            "bob@test is not present on the selected page"
        );
        assert_eq!(
            cached_matches.len(),
            1,
            "expected a cached match sourced from another page"
        );
        assert_eq!(
            cached_matches[0].title, "bob@test",
            "cached match should reference the bob@test entry"
        );
    }

    #[test]
    fn filter_state_entries_falls_back_to_current_page() {
        let entry_page0 = sample_state_entry("alice@test", vec![0xAC]);
        let current_entries = vec![entry_page0.clone()];
        let pages: Vec<StatePageCache> = Vec::new();
        let (page_indices, cached_matches) = filter_state_entries(
            &pages,
            &current_entries,
            StateQueryKind::Accounts,
            Some("alice"),
            None,
            None,
            None,
        );
        assert_eq!(
            page_indices,
            vec![0],
            "alice@test should be matched on the current page"
        );
        assert_eq!(
            cached_matches.len(),
            1,
            "fallback to current page should return a single match"
        );
        assert_eq!(
            cached_matches[0].title, "alice@test",
            "cached results should include the local page entry"
        );
    }

    #[test]
    fn filter_state_entries_respects_domain_filter() {
        let entry_page0 =
            sample_state_entry_with_domain("alice@test", "wonderland", vec![0xDE, 0x01]);
        let entry_page1 = sample_state_entry_with_domain("bob@test", "narnia", vec![0xDE, 0x02]);
        let pages = vec![
            StatePageCache {
                entries: vec![entry_page0.clone()],
                remaining: 1,
            },
            StatePageCache {
                entries: vec![entry_page1.clone()],
                remaining: 0,
            },
        ];
        let current_entries = vec![entry_page0];
        let (page_indices, cached_matches) = filter_state_entries(
            &pages,
            &current_entries,
            StateQueryKind::Accounts,
            None,
            Some("narnia"),
            None,
            None,
        );
        assert!(
            page_indices.is_empty(),
            "domain filter should skip non-matching entries on the current page"
        );
        assert_eq!(
            cached_matches.len(),
            1,
            "domain filter should surface matches from cached pages"
        );
        assert_eq!(
            cached_matches[0].domain.as_deref(),
            Some("narnia"),
            "matched entry should report the requested domain"
        );
    }

    #[test]
    fn collect_log_text_joins_lines() {
        let entries = vec![
            (0usize, "[alpha] started".to_owned()),
            (1, "[alpha] running".to_owned()),
        ];
        let text = super::collect_log_text(&entries).expect("export log lines");
        assert_eq!(text, "[alpha] started\n[alpha] running");
    }

    #[test]
    fn collect_log_text_rejects_empty() {
        let entries: Vec<(usize, String)> = Vec::new();
        assert!(
            super::collect_log_text(&entries).is_err(),
            "export should fail when no logs are available"
        );
    }

    #[test]
    fn save_logs_to_file_writes_filtered_entries() {
        let entries = vec![
            (0usize, "[alpha] started".to_owned()),
            (1, "[alpha] running".to_owned()),
        ];
        let dir = tempfile::tempdir().expect("tempdir");
        let path =
            super::save_logs_to_file(&entries, Some(dir.path())).expect("save filtered logs");
        assert!(
            path.starts_with(dir.path()),
            "export should write inside the provided directory"
        );
        let written = std::fs::read_to_string(&path).expect("read exported logs");
        assert!(written.contains("[alpha] started"));
        assert!(written.contains("[alpha] running"));
    }

    #[test]
    fn save_logs_to_file_rejects_empty_entries() {
        let entries: Vec<(usize, String)> = Vec::new();
        assert!(
            super::save_logs_to_file(&entries, None).is_err(),
            "export should fail with no matching logs"
        );
    }

    #[test]
    fn log_kind_filter_respects_settings() {
        let mut app = MochiApp::default();
        app.settings_log_stdout = false;
        let stdout_event = PeerLogEvent::Line {
            alias: Arc::from("alpha"),
            kind: LogStreamKind::Stdout,
            timestamp_ms: 0,
            message: "stdout".to_owned(),
        };
        assert!(
            !app.is_log_kind_enabled(MochiApp::log_event_kind(&stdout_event)),
            "stdout events should be hidden when the toggle is disabled"
        );

        let system_event = PeerLogEvent::Lifecycle {
            alias: Arc::from("alpha"),
            event: LifecycleEvent::Started { attempt: 0 },
            timestamp_ms: 0,
        };
        assert!(
            app.is_log_kind_enabled(MochiApp::log_event_kind(&system_event)),
            "system events remain visible by default"
        );
    }

    #[test]
    fn event_filter_honours_alias_filters() {
        let mut filter = EventFilterState::default();
        let line = RenderedEventLine::new(
            "alpha",
            "[alpha] Text frame".to_owned(),
            None,
            Color32::from_gray(190),
            RenderedEventKind::Text,
        );
        assert!(
            filter.matches(&line, None),
            "default filter should allow events"
        );
        filter.toggle_alias("alpha", false);
        assert!(
            !filter.matches(&line, None),
            "disabled alias should be filtered out"
        );
        filter.toggle_alias("alpha", true);
        assert!(
            filter.matches(&line, None),
            "re-enabled alias should match again"
        );
    }

    #[test]
    fn event_filter_supports_multiple_peer_toggles() {
        let mut filter = EventFilterState::default();
        let alpha = RenderedEventLine::new(
            "alpha",
            "[alpha] text frame".to_owned(),
            None,
            Color32::from_gray(190),
            RenderedEventKind::Text,
        );
        let beta = RenderedEventLine::new(
            "beta",
            "[beta] text frame".to_owned(),
            None,
            Color32::from_gray(190),
            RenderedEventKind::Text,
        );
        assert!(filter.matches(&alpha, None));
        assert!(filter.matches(&beta, None));

        filter.toggle_alias("beta", false);
        assert!(
            filter.matches(&alpha, None),
            "unfiltered alias should remain visible"
        );
        assert!(
            !filter.matches(&beta, None),
            "disabled alias should be hidden"
        );
    }

    #[test]
    fn event_filter_alias_toggle_is_case_insensitive() {
        let mut filter = EventFilterState::default();
        filter.toggle_alias("BETA", false);
        let beta = RenderedEventLine::new(
            "beta",
            "[beta] text frame".to_owned(),
            None,
            Color32::from_gray(190),
            RenderedEventKind::Text,
        );
        assert!(
            !filter.matches(&beta, None),
            "alias filters should treat names case-insensitively"
        );
    }

    #[test]
    fn event_filter_serializes_and_restores_state() {
        let mut filter = EventFilterState {
            search: "Hash:ABC".to_owned(),
            show_decode_errors: false,
            ..EventFilterState::default()
        };
        filter.toggle_alias("alpha", false);
        let serialized = serialize_event_filter(&filter).expect("filter should serialize");
        let parsed: Value =
            json::from_str(&serialized).expect("serialized filter should be valid JSON");
        let restored =
            EventFilterState::from_json_value(&parsed).expect("filter should restore from JSON");
        assert_eq!(restored.search, filter.search);
        assert_eq!(restored.show_decode_errors, filter.show_decode_errors);
        assert!(
            !restored.alias_selected("alpha"),
            "alias selection should persist with lowercasing"
        );
    }

    #[test]
    fn collect_dashboard_metrics_counts_resources() {
        let mut app = MochiApp::default();
        let peer_rows = vec![
            PeerRow {
                alias: "alpha".to_owned(),
                state: PeerState::Running,
                torii: "http://alpha".to_owned(),
                api_base: Some("http://alpha".to_owned()),
                api_error: None,
                config: "config-alpha".to_owned(),
                logs: "logs-alpha".to_owned(),
            },
            PeerRow {
                alias: "beta".to_owned(),
                state: PeerState::Stopped,
                torii: "http://beta".to_owned(),
                api_base: Some("http://beta".to_owned()),
                api_error: None,
                config: "config-beta".to_owned(),
                logs: "logs-beta".to_owned(),
            },
        ];

        let summary = BlockSummary {
            height: 5,
            hash_hex: "hash".to_owned(),
            transaction_count: 3,
            rejected_transaction_count: 1,
            time_trigger_count: 0,
            signature_count: 2,
            view_change_index: 0,
            creation_time_ms: 42,
            is_genesis: false,
        };
        let block_snapshot = BlockStreamSnapshot {
            connected: true,
            last_summary: Some(summary),
            ..Default::default()
        };
        app.block_snapshots
            .insert("alpha".to_owned(), block_snapshot);
        app.block_events.push(DisplayEvent {
            alias: Some("alpha".to_owned()),
            event: BlockStreamEvent::Text {
                text: "started".to_owned(),
            },
        });

        let event_snapshot = EventSnapshot {
            connected: true,
            ..Default::default()
        };
        app.event_snapshots
            .insert("alpha".to_owned(), event_snapshot);
        app.event_events.push(EventDisplay {
            alias: Some("alpha".to_owned()),
            event: EventStreamEvent::Text {
                text: "ping".to_owned(),
            },
        });
        app.log_events
            .push(MochiApp::system_log_event("alpha", "log".to_owned()));

        let metrics = app.collect_dashboard_metrics(&peer_rows);
        assert_eq!(metrics.total_peers, 2);
        assert_eq!(metrics.running_peers, 1);
        assert_eq!(metrics.connected_block_streams, 1);
        assert_eq!(metrics.connected_event_streams, 1);
        assert_eq!(metrics.latest_height, Some(5));
        assert_eq!(metrics.total_tx, 3);
        assert_eq!(metrics.total_rejected_tx, 1);
        assert_eq!(metrics.pending_block_events, 1);
        assert_eq!(metrics.pending_event_frames, 1);
        assert_eq!(metrics.stored_logs, 1);
        assert!(metrics.avg_queue.is_none());
        assert!(metrics.avg_commit_latency_ms.is_none());
    }

    #[test]
    fn peer_status_view_captures_metrics_and_errors() {
        let mut view = PeerStatusView::default();
        let now = Instant::now();

        let initial = TelemetryStatus {
            peers: 2,
            blocks: 10,
            blocks_non_empty: 8,
            commit_time_ms: 45,
            da_reschedule_total: 2,
            txs_approved: 5,
            txs_rejected: 1,
            uptime: Uptime(Duration::from_secs(5)),
            view_changes: 0,
            queue_size: 4,
            crypto: Default::default(),
            stack: Default::default(),
            sumeragi: None,
            governance: GovernanceStatus::default(),
            teu_lane_commit: Vec::new(),
            teu_dataspace_backlog: Vec::new(),
            tx_gossip: TxGossipSnapshot::default(),
            sorafs_micropayments: Vec::new(),
            taikai_ingest: Vec::new(),
            taikai_alias_rotations: Vec::new(),
            da_receipt_cursors: Vec::new(),
        };
        let mut sumeragi_initial = sample_sumeragi_status_wire();
        sumeragi_initial.membership.height = 21;
        let initial_snapshot = ToriiStatusSnapshot {
            timestamp: now,
            status: initial.clone(),
            metrics: StatusMetrics::from_samples(None, &initial),
        };
        view.record_snapshot(initial_snapshot, Some(sumeragi_initial), None, None, now);
        assert!(view.delta_summary().is_none());
        let (label, color) = view.status_label();
        assert!(label.contains("peers=2"));
        assert!(label.contains("queue=4"));
        assert!(label.contains("commit=45ms"));
        assert_eq!(color, Color32::from_rgb(80, 160, 80));
        let membership_summary = view.membership_summary().expect("membership summary");
        assert!(membership_summary.contains("h21"));
        assert!(membership_summary.contains("hash ABABABABABABABAB..."));

        let updated = TelemetryStatus {
            peers: 3,
            blocks: 11,
            blocks_non_empty: 9,
            commit_time_ms: 120,
            da_reschedule_total: 5,
            txs_approved: 9,
            txs_rejected: 3,
            uptime: Uptime(Duration::from_secs(6)),
            view_changes: 1,
            queue_size: 9,
            crypto: Default::default(),
            stack: Default::default(),
            sumeragi: None,
            governance: GovernanceStatus::default(),
            teu_lane_commit: Vec::new(),
            teu_dataspace_backlog: Vec::new(),
            tx_gossip: TxGossipSnapshot::default(),
            sorafs_micropayments: Vec::new(),
            taikai_ingest: Vec::new(),
            taikai_alias_rotations: Vec::new(),
            da_receipt_cursors: Vec::new(),
        };
        let mut sumeragi_updated = sample_sumeragi_status_wire();
        sumeragi_updated.membership.height = 30;
        sumeragi_updated.membership.view_hash = None;
        let updated_snapshot = ToriiStatusSnapshot {
            timestamp: now + Duration::from_secs(2),
            status: updated.clone(),
            metrics: StatusMetrics::from_samples(Some(&initial), &updated),
        };
        view.record_snapshot(
            updated_snapshot,
            Some(sumeragi_updated),
            None,
            None,
            now + Duration::from_secs(2),
        );
        let delta = view.delta_summary().expect("delta summary");
        assert!(delta.contains("tx +4 / -2"));
        assert!(delta.contains("queue +5"));
        assert!(delta.contains("resched +3"));
        assert!(delta.contains("view +1"));
        let (label, color) = view.status_label();
        assert!(label.contains("peers=3"));
        assert!(label.contains("commit=120ms"));
        assert_eq!(color, Color32::from_rgb(200, 160, 64));
        let membership_summary = view.membership_summary().expect("membership summary");
        assert!(membership_summary.contains("h30"));
        assert!(membership_summary.contains("hash —"));

        let err_info = ToriiError::Decode("bad payload".to_owned()).summarize();
        view.record_error(err_info, now + Duration::from_secs(3));
        let (label, color) = view.status_label();
        assert!(label.to_ascii_lowercase().contains("decode"));
        assert_eq!(color, Color32::from_rgb(200, 160, 64));
        assert!(view.membership_summary().is_some());
    }

    #[test]
    fn peer_status_view_surfaces_sealed_lanes() {
        let mut view = PeerStatusView::default();
        let now = Instant::now();

        let status = TelemetryStatus {
            peers: 2,
            blocks: 10,
            blocks_non_empty: 8,
            commit_time_ms: 42,
            da_reschedule_total: 0,
            txs_approved: 4,
            txs_rejected: 0,
            uptime: Uptime(Duration::from_secs(5)),
            view_changes: 0,
            queue_size: 3,
            crypto: Default::default(),
            stack: Default::default(),
            sumeragi: None,
            governance: GovernanceStatus::default(),
            teu_lane_commit: Vec::new(),
            teu_dataspace_backlog: Vec::new(),
            tx_gossip: TxGossipSnapshot::default(),
            sorafs_micropayments: Vec::new(),
            taikai_ingest: Vec::new(),
            taikai_alias_rotations: Vec::new(),
            da_receipt_cursors: Vec::new(),
        };
        let snapshot = ToriiStatusSnapshot {
            timestamp: now,
            status: status.clone(),
            metrics: StatusMetrics::from_samples(None, &status),
        };

        let mut sumeragi = sample_sumeragi_status_wire();
        sumeragi.lane_governance_sealed_total = 2;
        sumeragi.lane_governance_sealed_aliases = vec![
            "archive".to_owned(),
            "payments".to_owned(),
            "vip".to_owned(),
            "ops".to_owned(),
            "extra".to_owned(),
        ];

        view.record_snapshot(snapshot, Some(sumeragi), None, None, now);

        let (label, color) = view.status_label();
        assert!(
            label.contains("sealed=2"),
            "label should surface sealed lane count: {label}"
        );
        assert_eq!(
            color,
            Color32::from_rgb(220, 140, 80),
            "status color should downgrade to amber when lanes remain sealed"
        );
        let summary = view.sealed_summary().expect("sealed summary");
        assert!(
            summary.contains("Sealed lanes: 2"),
            "summary should include sealed count: {summary}"
        );
        assert!(
            summary.contains("… +2"),
            "summary should collapse additional aliases: {summary}"
        );
    }

    #[test]
    fn lane_status_rows_surface_relay_lag_and_cursor() {
        let mut view = PeerStatusView::default();
        let now = Instant::now();
        let status = TelemetryStatus {
            da_receipt_cursors: vec![iroha_telemetry::metrics::DaReceiptCursorStatus {
                lane_id: 0,
                epoch: 2,
                highest_sequence: 7,
            }],
            ..TelemetryStatus::default()
        };
        let snapshot = ToriiStatusSnapshot {
            timestamp: now,
            status: status.clone(),
            metrics: StatusMetrics::from_samples(None, &status),
        };
        let mut sumeragi = sample_sumeragi_status_wire();
        let header = BlockHeader::new(NonZeroU64::new(9).expect("height"), None, None, None, 0, 0);
        let settlement = iroha_data_model::block::consensus::LaneBlockCommitment {
            block_height: 9,
            lane_id: LaneId::new(0),
            dataspace_id: DataSpaceId::new(0),
            tx_count: 1,
            total_local_micro: 0,
            total_xor_due_micro: 0,
            total_xor_after_haircut_micro: 0,
            total_xor_variance_micro: 0,
            swap_metadata: None,
            receipts: Vec::new(),
        };
        let envelope =
            LaneRelayEnvelope::new(header, None, None, settlement, 256).expect("envelope");
        sumeragi.lane_relay_envelopes = vec![envelope];

        view.record_snapshot(snapshot, Some(sumeragi), None, None, now);

        let rows = view.lane_status_rows(&lane_catalog_snapshot(None));
        assert_eq!(rows.len(), 1);
        let row = &rows[0];
        assert_eq!(row.lane_id, 0);
        assert_eq!(row.alias, "alpha");
        assert_eq!(row.relay_lag, Some(1));
        assert_eq!(row.rbc_bytes, Some(384));
        assert_eq!(row.da_cursor_label(), "e2 s7");
        assert!(matches!(row.relay_state, RelayIngestState::MissingQc));
    }

    #[test]
    fn composer_update_success_records_message() {
        let mut app = MochiApp::default();
        app.composer_submitting = true;
        app.handle_composer_update(ComposerSubmitUpdate {
            peer: "alpha".to_owned(),
            result: Ok("hash123".to_owned()),
        });
        assert!(!app.composer_submitting);
        assert_eq!(
            app.composer_submit_success.as_deref(),
            Some("Submitted transaction hash123 to alpha.")
        );
        assert!(app.composer_submit_error.is_none());
        assert_eq!(
            app.last_info.as_deref(),
            Some("Submitted transaction hash123 to alpha.")
        );
        assert!(app.last_error.is_none());
    }

    #[test]
    fn composer_update_failure_records_error() {
        let mut app = MochiApp::default();
        app.composer_submitting = true;
        let info = ToriiErrorInfo::new(ToriiErrorKind::HttpTransport, "network error");
        app.handle_composer_update(ComposerSubmitUpdate {
            peer: "beta".to_owned(),
            result: Err(info),
        });
        assert!(!app.composer_submitting);
        assert!(app.composer_submit_success.is_none());
        assert_eq!(
            app.composer_submit_error
                .as_ref()
                .map(|error| error.message.as_str()),
            Some("network error")
        );
        assert!(app.last_info.is_none());
        assert_eq!(
            app.last_error.as_deref(),
            Some("Failed to submit transaction to beta: network error")
        );
    }

    #[test]
    fn add_instruction_to_batch_appends_draft() {
        let mut app = MochiApp::default();
        app.composer_selected_signer = Some(0);
        let asset = AssetId::new("rose#wonderland".parse().unwrap(), ALICE_ID.clone());
        app.composer_asset_id = asset.to_string();
        app.composer_quantity = "5".to_owned();
        app.add_instruction_to_batch(None);
        assert_eq!(app.composer_drafts.len(), 1);
        assert!(app.composer_error.is_none());
    }

    #[test]
    fn transfer_without_destination_records_error() {
        let mut app = MochiApp::default();
        app.composer_instruction_kind = ComposerInstructionKind::TransferAsset;
        app.composer_selected_signer = Some(0);
        let asset = AssetId::new("rose#wonderland".parse().unwrap(), ALICE_ID.clone());
        app.composer_asset_id = asset.to_string();
        app.composer_quantity = "1".to_owned();
        app.add_instruction_to_batch(None);
        assert!(app.composer_drafts.is_empty(), "draft should not be added");
        assert!(
            app.composer_error
                .as_deref()
                .unwrap_or_default()
                .contains("Destination account"),
            "expected destination error message"
        );
    }

    #[test]
    fn add_instruction_respects_signer_permissions() {
        let mut app = MochiApp::default();
        app.composer_instruction_kind = ComposerInstructionKind::RegisterDomain;
        app.composer_domain_id = "side_garden".to_owned();
        // Bob is the second development signer and lacks register-domain permission.
        app.composer_selected_signer = Some(1);
        app.add_instruction_to_batch(None);
        assert!(
            app.composer_drafts.is_empty(),
            "unauthorised draft should not be added"
        );
        let message = app.composer_error.as_deref().unwrap_or_default().to_owned();
        assert!(
            message.contains("cannot register domains"),
            "expected permission error, got `{message}`"
        );
    }

    #[test]
    fn composer_template_prefills_mint_inputs() {
        if !super::socket_bind_available() {
            eprintln!("Skipping mint template test due to socket restrictions");
            return;
        }
        let _lock = env_lock().lock().expect("env lock");
        let temp = tempfile::tempdir().expect("tempdir");
        let kagami_stub = install_kagami_stub(temp.path());
        let irohad_stub = install_noop_stub(temp.path(), "irohad_stub.sh");
        let config_dir = temp.path().join("config");
        fs::create_dir_all(&config_dir).expect("config dir");
        let config_path = config_dir.join("local.toml");
        fs::write(&config_path, "[supervisor]\n").expect("write config stub");
        let data_root = temp
            .path()
            .join(format!("mochi-data-{}", std::process::id()));
        let _kagami_guard = TestEnvGuard::set("MOCHI_KAGAMI", &kagami_stub);
        let _irohad_guard = TestEnvGuard::set("MOCHI_IROHAD", &irohad_stub);
        let _config_guard = TestEnvGuard::set("MOCHI_CONFIG", &config_path);
        let _data_guard = TestEnvGuard::set("MOCHI_DATA_ROOT", &data_root);
        reset_cli_overrides_for_tests();

        let mut app = MochiApp::default();
        app.composer_selected_signer = Some(0);
        let signers = development_signing_authorities();

        app.apply_composer_template(ComposerTemplate::MintRoseToSigner, signers);

        assert_eq!(
            app.composer_instruction_kind,
            ComposerInstructionKind::MintAsset
        );
        assert!(
            app.composer_asset_id.contains("rose"),
            "expected rose asset id, got {}",
            app.composer_asset_id
        );
        assert_eq!(app.composer_quantity, "10");
        assert!(
            app.composer_destination_account.is_empty(),
            "mint template should not set destination"
        );
        assert!(
            app.last_info
                .as_deref()
                .unwrap_or_default()
                .contains("rose mint"),
            "should surface mint template info banner"
        );
    }

    #[test]
    fn composer_template_prefills_burn_inputs() {
        if !super::socket_bind_available() {
            eprintln!("Skipping burn template test due to socket restrictions");
            return;
        }
        let _lock = env_lock().lock().expect("env lock");
        let temp = tempfile::tempdir().expect("tempdir");
        let kagami_stub = install_kagami_stub(temp.path());
        let irohad_stub = install_noop_stub(temp.path(), "irohad_stub.sh");
        let config_dir = temp.path().join("config");
        fs::create_dir_all(&config_dir).expect("config dir");
        let config_path = config_dir.join("local.toml");
        fs::write(&config_path, "[supervisor]\n").expect("write config stub");
        let data_root = temp
            .path()
            .join(format!("mochi-data-{}", std::process::id()));
        let _kagami_guard = TestEnvGuard::set("MOCHI_KAGAMI", &kagami_stub);
        let _irohad_guard = TestEnvGuard::set("MOCHI_IROHAD", &irohad_stub);
        let _config_guard = TestEnvGuard::set("MOCHI_CONFIG", &config_path);
        let _data_guard = TestEnvGuard::set("MOCHI_DATA_ROOT", &data_root);
        reset_cli_overrides_for_tests();

        let mut app = MochiApp::default();
        app.composer_selected_signer = Some(0);
        let signers = development_signing_authorities();

        app.apply_composer_template(ComposerTemplate::BurnRoseFromSigner, signers);

        assert_eq!(
            app.composer_instruction_kind,
            ComposerInstructionKind::BurnAsset
        );
        assert!(
            app.composer_asset_id.contains("rose"),
            "expected rose asset id, got {}",
            app.composer_asset_id
        );
        assert_eq!(app.composer_quantity, "1");
        assert!(
            app.composer_destination_account.is_empty(),
            "burn template should not set destination"
        );
        assert!(
            app.last_info
                .as_deref()
                .unwrap_or_default()
                .contains("burn template"),
            "should surface burn template info banner"
        );
    }

    #[test]
    fn composer_template_prefills_transfer_inputs() {
        if !super::socket_bind_available() {
            eprintln!("Skipping transfer template test due to socket restrictions");
            return;
        }
        let _lock = env_lock().lock().expect("env lock");
        let temp = tempfile::tempdir().expect("tempdir");
        let kagami_stub = install_kagami_stub(temp.path());
        let irohad_stub = install_noop_stub(temp.path(), "irohad_stub.sh");
        let config_dir = temp.path().join("config");
        fs::create_dir_all(&config_dir).expect("config dir");
        let config_path = config_dir.join("local.toml");
        fs::write(&config_path, "[supervisor]\n").expect("write config stub");
        let data_root = temp
            .path()
            .join(format!("mochi-data-{}", std::process::id()));
        let _kagami_guard = TestEnvGuard::set("MOCHI_KAGAMI", &kagami_stub);
        let _irohad_guard = TestEnvGuard::set("MOCHI_IROHAD", &irohad_stub);
        let _config_guard = TestEnvGuard::set("MOCHI_CONFIG", &config_path);
        let _data_guard = TestEnvGuard::set("MOCHI_DATA_ROOT", &data_root);
        reset_cli_overrides_for_tests();

        let mut app = MochiApp::default();
        app.composer_selected_signer = Some(0);
        let signers = development_signing_authorities();

        app.apply_composer_template(ComposerTemplate::TransferRoseToTeammate, signers);

        assert_eq!(
            app.composer_instruction_kind,
            ComposerInstructionKind::TransferAsset
        );
        assert!(
            app.composer_asset_id.contains("rose"),
            "expected rose asset id, got {}",
            app.composer_asset_id
        );
        assert_eq!(app.composer_quantity, "2");
        assert!(
            !app.composer_destination_account.is_empty(),
            "transfer template must set a destination account"
        );
        assert_ne!(
            app.composer_destination_account,
            signers[0].account_id().to_string(),
            "destination should differ from the source signer"
        );
    }

    #[test]
    fn queue_plot_points_returns_points() {
        let mut app = MochiApp::default();
        let base = Instant::now();
        let mut history = VecDeque::new();
        let status_a = TelemetryStatus {
            queue_size: 1,
            txs_approved: 2,
            ..Default::default()
        };
        let snapshot_a = ToriiStatusSnapshot {
            timestamp: base,
            status: status_a.clone(),
            metrics: StatusMetrics::from_samples(None, &status_a),
        };
        history.push_back(StatusHistoryEntry {
            timestamp: base,
            snapshot: snapshot_a,
            metrics: None,
        });
        let mut status_b = status_a.clone();
        status_b.queue_size = 4;
        status_b.txs_approved = 5;
        let snapshot_b = ToriiStatusSnapshot {
            timestamp: base + Duration::from_secs(1),
            status: status_b.clone(),
            metrics: StatusMetrics::from_samples(Some(&status_a), &status_b),
        };
        history.push_back(StatusHistoryEntry {
            timestamp: base + Duration::from_secs(1),
            snapshot: snapshot_b,
            metrics: None,
        });
        app.status_history.insert("alpha".to_owned(), history);

        assert!(app.queue_plot_points("alpha").is_some());
    }

    #[test]
    fn commit_latency_plot_points_require_multiple_samples() {
        let mut app = MochiApp::default();
        assert!(
            app.commit_latency_plot_points("beta").is_none(),
            "no history should produce no plot"
        );

        let base = Instant::now();
        let mut history = VecDeque::new();
        let status_a = TelemetryStatus {
            commit_time_ms: 75,
            queue_size: 3,
            ..Default::default()
        };
        let snapshot_a = ToriiStatusSnapshot {
            timestamp: base,
            status: status_a.clone(),
            metrics: StatusMetrics::from_samples(None, &status_a),
        };
        history.push_back(StatusHistoryEntry {
            timestamp: base,
            snapshot: snapshot_a,
            metrics: None,
        });
        app.status_history.insert("beta".to_owned(), history);
        assert!(
            app.commit_latency_plot_points("beta").is_none(),
            "a single sample should not emit a plot"
        );

        let mut status_b = status_a.clone();
        status_b.commit_time_ms = 140;
        status_b.queue_size = 5;
        let snapshot_b = ToriiStatusSnapshot {
            timestamp: base + Duration::from_secs(1),
            status: status_b.clone(),
            metrics: StatusMetrics::from_samples(Some(&status_a), &status_b),
        };
        app.status_history
            .get_mut("beta")
            .expect("history must exist")
            .push_back(StatusHistoryEntry {
                timestamp: base + Duration::from_secs(1),
                snapshot: snapshot_b,
                metrics: None,
            });

        assert!(
            app.commit_latency_plot_points("beta").is_some(),
            "two samples should produce a commit latency plot"
        );
    }

    #[test]
    fn throughput_plot_points_returns_points() {
        let mut app = MochiApp::default();
        let base = Instant::now();
        let mut history = VecDeque::new();
        let status_a = TelemetryStatus {
            txs_approved: 8,
            ..Default::default()
        };
        let snapshot_a = ToriiStatusSnapshot {
            timestamp: base,
            status: status_a.clone(),
            metrics: StatusMetrics::from_samples(None, &status_a),
        };
        history.push_back(StatusHistoryEntry {
            timestamp: base,
            snapshot: snapshot_a,
            metrics: None,
        });
        let mut status_b = status_a.clone();
        status_b.txs_approved = 12;
        let snapshot_b = ToriiStatusSnapshot {
            timestamp: base + Duration::from_secs(2),
            status: status_b.clone(),
            metrics: StatusMetrics::from_samples(Some(&status_a), &status_b),
        };
        history.push_back(StatusHistoryEntry {
            timestamp: base + Duration::from_secs(2),
            snapshot: snapshot_b,
            metrics: None,
        });
        app.status_history.insert("alpha".to_owned(), history);
        assert!(
            app.throughput_plot_points("alpha").is_some(),
            "two samples should produce throughput points"
        );
    }

    #[test]
    fn consensus_queue_plot_points_require_metrics() {
        let mut app = MochiApp::default();
        let base = Instant::now();
        let mut history = VecDeque::new();
        let status = TelemetryStatus::default();
        let snapshot_a = ToriiStatusSnapshot {
            timestamp: base,
            status: status.clone(),
            metrics: StatusMetrics::from_samples(None, &status),
        };
        history.push_back(StatusHistoryEntry {
            timestamp: base,
            snapshot: snapshot_a,
            metrics: Some(sample_metrics_snapshot(base, 2.0, 8.0)),
        });
        let snapshot_b = ToriiStatusSnapshot {
            timestamp: base + Duration::from_secs(1),
            status,
            metrics: StatusMetrics::default(),
        };
        history.push_back(StatusHistoryEntry {
            timestamp: base + Duration::from_secs(1),
            snapshot: snapshot_b,
            metrics: Some(sample_metrics_snapshot(
                base + Duration::from_secs(1),
                5.0,
                8.0,
            )),
        });
        app.status_history.insert("alpha".to_owned(), history);
        assert!(
            app.consensus_queue_plot_points("alpha").is_some(),
            "two metrics samples should produce consensus queue points"
        );
    }

    #[test]
    fn view_change_plot_points_record_deltas() {
        let mut app = MochiApp::default();
        let base = Instant::now();
        let mut history = VecDeque::new();
        let status_a = TelemetryStatus {
            view_changes: 3,
            ..Default::default()
        };
        let mut metrics_a = StatusMetrics::from_samples(None, &status_a);
        metrics_a.sample_interval_ms = 1_000;
        let snapshot_a = ToriiStatusSnapshot {
            timestamp: base,
            status: status_a.clone(),
            metrics: metrics_a,
        };
        history.push_back(StatusHistoryEntry {
            timestamp: base,
            snapshot: snapshot_a,
            metrics: None,
        });
        let mut status_b = status_a.clone();
        status_b.view_changes = 6;
        let mut metrics_b = StatusMetrics::from_samples(Some(&status_a), &status_b);
        metrics_b.sample_interval_ms = 1_000;
        let snapshot_b = ToriiStatusSnapshot {
            timestamp: base + Duration::from_secs(1),
            status: status_b,
            metrics: metrics_b,
        };
        history.push_back(StatusHistoryEntry {
            timestamp: base + Duration::from_secs(1),
            snapshot: snapshot_b,
            metrics: None,
        });
        app.status_history.insert("alpha".to_owned(), history);
        assert!(
            app.view_change_plot_points("alpha").is_some(),
            "non-zero view change deltas should produce points"
        );
    }

    #[test]
    fn reschedule_plot_points_record_activity() {
        let mut app = MochiApp::default();
        let base = Instant::now();
        let mut history = VecDeque::new();
        let status_a = TelemetryStatus {
            da_reschedule_total: 5,
            ..Default::default()
        };
        let mut metrics_a = StatusMetrics::from_samples(None, &status_a);
        metrics_a.sample_interval_ms = 1_000;
        let snapshot_a = ToriiStatusSnapshot {
            timestamp: base,
            status: status_a.clone(),
            metrics: metrics_a,
        };
        history.push_back(StatusHistoryEntry {
            timestamp: base,
            snapshot: snapshot_a,
            metrics: None,
        });
        let mut status_b = status_a.clone();
        status_b.da_reschedule_total = 9;
        let mut metrics_b = StatusMetrics::from_samples(Some(&status_a), &status_b);
        metrics_b.sample_interval_ms = 500;
        let snapshot_b = ToriiStatusSnapshot {
            timestamp: base + Duration::from_millis(500),
            status: status_b,
            metrics: metrics_b,
        };
        history.push_back(StatusHistoryEntry {
            timestamp: base + Duration::from_millis(500),
            snapshot: snapshot_b,
            metrics: None,
        });
        app.status_history.insert("alpha".to_owned(), history);
        assert!(
            app.reschedule_plot_points("alpha").is_some(),
            "reschedule deltas should produce points"
        );
    }

    #[test]
    fn peer_status_view_summarises_metrics() {
        let mut view = PeerStatusView::default();
        let snapshot = ToriiStatusSnapshot {
            timestamp: Instant::now(),
            status: TelemetryStatus::default(),
            metrics: StatusMetrics::default(),
        };
        view.record_snapshot(
            snapshot.clone(),
            None,
            Some(sample_metrics_snapshot(Instant::now(), 3.0, 10.0)),
            None,
            Instant::now(),
        );
        let summary = view
            .consensus_queue_summary()
            .expect("consensus summary expected");
        assert!(
            summary.contains("3"),
            "summary should include queue depth: {summary}"
        );
        let storage = view.storage_summary().expect("storage summary expected");
        assert!(
            storage.contains("Tiered state"),
            "storage string should include Tiered state label"
        );

        view.record_snapshot(
            snapshot,
            None,
            None,
            Some(ToriiErrorInfo::new(
                ToriiErrorKind::HttpTransport,
                "Metrics unavailable",
            )),
            Instant::now(),
        );
        let (message, _) = view.metrics_error_label().expect("metrics error label");
        assert!(
            message.contains("Metrics"),
            "metrics error label should include prefix"
        );
    }

    fn sample_metrics_snapshot(
        timestamp: Instant,
        depth: f64,
        capacity: f64,
    ) -> ToriiMetricsSnapshot {
        ToriiMetricsSnapshot {
            timestamp,
            queue_size: None,
            view_changes: None,
            sumeragi_tx_queue_depth: Some(depth),
            sumeragi_tx_queue_capacity: Some(capacity),
            sumeragi_tx_queue_saturated: None,
            state_tiered_hot_entries: Some(4.0),
            state_tiered_cold_entries: Some(2.0),
            state_tiered_cold_bytes: Some(1024.0),
            uptime_since_genesis_ms: None,
        }
    }

    #[test]
    fn peer_state_color_matches_palette() {
        let palette = MochiApp::palette();
        assert_eq!(
            MochiApp::peer_state_color(PeerState::Running),
            palette.success
        );
        assert_eq!(
            MochiApp::peer_state_color(PeerState::Stopped),
            palette.danger
        );
        assert_eq!(
            MochiApp::peer_state_color(PeerState::Restarting),
            palette.warning
        );
    }

    use std::{
        env, fs,
        io::{Read, Write},
        net::TcpListener,
        path::PathBuf,
        sync::{Mutex, OnceLock},
    };

    use super::*;

    #[test]
    fn applying_settings_persists_config_and_rebuilds_supervisor() {
        if !super::socket_bind_available() {
            eprintln!("Skipping settings persistence test due to socket restrictions");
            return;
        }
        let _lock = env_lock().lock().expect("env lock");
        let temp = tempfile::tempdir().expect("tempdir");
        let config_dir = temp.path().join("config");
        fs::create_dir_all(&config_dir).expect("config dir");
        let config_path = config_dir.join("local.toml");
        fs::write(&config_path, "[supervisor]\n").expect("write starter config");

        let kagami_stub = install_kagami_stub(temp.path());
        let irohad_stub = install_noop_stub(temp.path(), "irohad_stub.sh");
        let _kagami_guard = TestEnvGuard::set("MOCHI_KAGAMI", &kagami_stub);
        let _irohad_guard = TestEnvGuard::set("MOCHI_IROHAD", &irohad_stub);
        let _config_guard = TestEnvGuard::set("MOCHI_CONFIG", &config_path);

        let initial_root = temp
            .path()
            .join(format!("mochi-data-{}", std::process::id()));
        let _data_guard = TestEnvGuard::set("MOCHI_DATA_ROOT", &initial_root);

        reset_cli_overrides_for_tests();

        let mut app = MochiApp::default();
        let resolved_path = app
            .bundle_config
            .as_ref()
            .map(|cfg| cfg.path.clone())
            .unwrap_or_else(|| config_path.clone());

        let new_root = temp.path().join("custom-root");
        app.settings_data_root_input = new_root.display().to_string();
        app.settings_torii_port_input = "15000".to_owned();
        app.settings_p2p_port_input = "16000".to_owned();
        app.settings_chain_id_input = "custom-chain".to_owned();
        app.settings_profile_input =
            "{ peer_count = 3, consensus_mode = \"permissioned\" }".to_owned();
        app.settings_nexus_enabled = true;
        app.settings_nexus_lane_count_input = "2".to_owned();
        app.settings_nexus_lane_catalog_input =
            "[[lane_catalog]]\nindex = 0\nalias = \"core\"\ndataspace = \"global\"".to_owned();
        app.settings_nexus_dataspace_catalog_input =
            "[[dataspace_catalog]]\nalias = \"global\"\nid = 0".to_owned();
        app.settings_sumeragi_da_enabled = true;
        let replay_dir = temp.path().join("da-replay");
        let manifest_dir = temp.path().join("da-manifests");
        let replay_dir_text = replay_dir.display().to_string();
        let manifest_dir_text = manifest_dir.display().to_string();
        app.settings_torii_da_replay_dir_input = replay_dir.display().to_string();
        app.settings_torii_da_manifest_dir_input = manifest_dir.display().to_string();
        let export_dir = temp.path().join("log-export");
        app.settings_log_export_dir_input = export_dir.display().to_string();
        let state_export_dir = temp.path().join("state-export");
        app.settings_state_export_dir_input = state_export_dir.display().to_string();

        app.apply_settings_changes()
            .expect("settings persistence should succeed");

        let bundle = app
            .bundle_config
            .as_ref()
            .expect("bundle config should be tracked after apply");
        if bundle.path != resolved_path {
            let expected_suffix = Path::new("config").join("local.toml");
            assert!(
                bundle.path.ends_with(&expected_suffix),
                "bundle config path should match override or default; got {}, expected {}",
                bundle.path.display(),
                resolved_path.display()
            );
        }
        assert_eq!(bundle.config.data_root.as_deref(), Some(new_root.as_path()));
        assert_eq!(bundle.config.torii_start, Some(15000));
        assert_eq!(bundle.config.p2p_start, Some(16000));
        assert_eq!(bundle.config.chain_id.as_deref(), Some("custom-chain"));
        let profile = bundle.config.profile.as_ref().expect("profile config");
        assert_eq!(profile.preset, None);
        assert_eq!(profile.topology.peer_count, 3);
        assert_eq!(profile.consensus_mode, SumeragiConsensusMode::Permissioned);
        let nexus = bundle.config.nexus.as_ref().expect("nexus config");
        assert_eq!(
            nexus.get("enabled").and_then(TomlValue::as_bool),
            Some(true)
        );
        assert_eq!(
            nexus.get("lane_count").and_then(TomlValue::as_integer),
            Some(2)
        );
        let lane_catalog = nexus
            .get("lane_catalog")
            .and_then(TomlValue::as_array)
            .expect("lane catalog array");
        assert_eq!(lane_catalog.len(), 1);
        let lane0 = lane_catalog[0].as_table().expect("lane table");
        assert_eq!(lane0.get("alias").and_then(TomlValue::as_str), Some("core"));
        let dataspace_catalog = nexus
            .get("dataspace_catalog")
            .and_then(TomlValue::as_array)
            .expect("dataspace catalog array");
        assert_eq!(dataspace_catalog.len(), 1);
        let dataspace = dataspace_catalog[0].as_table().expect("dataspace table");
        assert_eq!(
            dataspace.get("alias").and_then(TomlValue::as_str),
            Some("global")
        );
        let sumeragi = bundle.config.sumeragi.as_ref().expect("sumeragi config");
        assert_eq!(
            sumeragi.get("da_enabled").and_then(TomlValue::as_bool),
            Some(true)
        );
        let torii = bundle.config.torii.as_ref().expect("torii config");
        let da_ingest = torii
            .get("da_ingest")
            .and_then(TomlValue::as_table)
            .expect("da_ingest table");
        assert_eq!(
            da_ingest
                .get("replay_cache_store_dir")
                .and_then(TomlValue::as_str),
            Some(replay_dir_text.as_str())
        );
        assert_eq!(
            da_ingest
                .get("manifest_store_dir")
                .and_then(TomlValue::as_str),
            Some(manifest_dir_text.as_str())
        );
        assert_eq!(app.log_export_dir.as_deref(), Some(export_dir.as_path()));
        assert_eq!(
            app.state_export_dir.as_deref(),
            Some(state_export_dir.as_path())
        );

        let round_trip = crate::config::load_bundle_config()
            .expect("reload persisted config")
            .expect("config should exist on disk");
        assert_eq!(round_trip.path, bundle.path);
        assert_eq!(
            round_trip.config.data_root.as_deref(),
            Some(new_root.as_path())
        );
        assert_eq!(round_trip.config.torii_start, Some(15000));
        assert_eq!(round_trip.config.p2p_start, Some(16000));
        assert_eq!(round_trip.config.chain_id.as_deref(), Some("custom-chain"));
        let round_trip_profile = round_trip.config.profile.expect("profile config");
        assert_eq!(round_trip_profile.preset, None);
        assert_eq!(round_trip_profile.topology.peer_count, 3);
        assert_eq!(
            round_trip_profile.consensus_mode,
            SumeragiConsensusMode::Permissioned
        );
        let round_trip_nexus = round_trip.config.nexus.expect("nexus config");
        assert_eq!(
            round_trip_nexus.get("enabled").and_then(TomlValue::as_bool),
            Some(true)
        );
        let round_trip_sumeragi = round_trip.config.sumeragi.expect("sumeragi config");
        assert_eq!(
            round_trip_sumeragi
                .get("da_enabled")
                .and_then(TomlValue::as_bool),
            Some(true)
        );
        let round_trip_torii = round_trip.config.torii.expect("torii config");
        let round_trip_da = round_trip_torii
            .get("da_ingest")
            .and_then(TomlValue::as_table)
            .expect("da_ingest table");
        assert_eq!(
            round_trip_da
                .get("replay_cache_store_dir")
                .and_then(TomlValue::as_str),
            Some(replay_dir_text.as_str())
        );
        assert_eq!(
            round_trip_da
                .get("manifest_store_dir")
                .and_then(TomlValue::as_str),
            Some(manifest_dir_text.as_str())
        );
        let _ = fs::remove_file(&bundle.path);
        assert!(
            !app.settings_dialog,
            "settings dialog should close after a successful apply"
        );

        let supervisor = app
            .supervisor
            .as_ref()
            .expect("rebuild should leave a supervisor instance");
        assert_eq!(
            MochiApp::supervisor_base_data_root(supervisor),
            new_root,
            "rebuilt supervisor should adopt new data root"
        );
        assert_eq!(supervisor.chain_id(), "custom-chain");
        assert_eq!(
            app.settings_data_root_input,
            new_root.display().to_string(),
            "settings inputs should reflect rebuilt supervisor state"
        );
        assert_eq!(
            app.settings_log_export_dir_input,
            export_dir.display().to_string(),
            "log export directory input should reflect applied setting"
        );
        assert_eq!(
            app.settings_state_export_dir_input,
            state_export_dir.display().to_string(),
            "state export directory input should reflect applied setting"
        );
        assert_eq!(
            app.settings_chain_id_input, "custom-chain",
            "chain id input should reflect applied value"
        );
    }

    #[test]
    fn default_app_uses_single_peer_profile() {
        if !super::socket_bind_available() {
            eprintln!("Skipping default app supervisor test due to socket restrictions");
            return;
        }
        let _lock = env_lock().lock().expect("env lock");
        let temp = tempfile::tempdir().expect("tempdir");
        let kagami_stub = install_kagami_stub(temp.path());
        let irohad_stub = install_noop_stub(temp.path(), "irohad_stub.sh");
        let _kagami_guard = TestEnvGuard::set("MOCHI_KAGAMI", &kagami_stub);
        let _irohad_guard = TestEnvGuard::set("MOCHI_IROHAD", &irohad_stub);
        let data_root = temp
            .path()
            .join(format!("mochi-data-{}", std::process::id()));
        let _data_guard = TestEnvGuard::set("MOCHI_DATA_ROOT", &data_root);
        reset_cli_overrides_for_tests();
        let app = MochiApp::default();
        if let Some(err) = app.supervisor_error.as_ref() {
            panic!("default supervisor preparation should succeed: {err}");
        }
        let supervisor = app
            .supervisor
            .as_ref()
            .expect("default supervisor preparation should succeed");

        assert_eq!(
            supervisor.profile().topology.peer_count,
            1,
            "default topology must match single peer preset"
        );
        assert_eq!(supervisor.chain_id(), "mochi-local");
        assert!(app.last_error.is_none());
        assert!(!app.theme_applied);
        assert!(matches!(app.active_view, ActiveView::Dashboard));
        assert!(app.block_stream.is_none());
        assert!(app.block_receiver.is_none());
        assert!(app.block_events.is_empty());
        assert!(app.block_stream_peer.is_none());
        assert!(app.block_snapshots.is_empty());
        assert!(app.event_stream.is_none());
        assert!(app.event_receiver.is_none());
        assert!(app.event_events.is_empty());
        assert!(app.event_stream_peer.is_none());
        assert!(app.event_selected_peer.is_none());
        assert!(app.event_snapshots.is_empty());
        assert!(app.log_receiver.is_none());
        assert!(app.log_events.is_empty());
        assert!(app.log_stream_peer.is_none());
        assert!(app.log_snapshots.is_empty());
        assert!(app.log_filter.is_empty());
        assert!(app.status_snapshots.is_empty());
        assert!(app.status_streams.is_empty());
        assert!(
            matches!(app.maintenance_state, MaintenanceState::Idle),
            "maintenance state should start idle"
        );
        assert!(!app.settings_dialog);
        assert!(app.settings_log_stdout);
        assert!(app.settings_log_stderr);
        assert!(app.settings_log_system);
        assert!(app.log_export_dir.is_none());
        assert!(app.settings_log_export_dir_input.is_empty());
        assert!(app.state_export_dir.is_none());
        assert!(app.settings_state_export_dir_input.is_empty());
        assert_eq!(PathBuf::from(&app.settings_data_root_input), data_root);
        assert_eq!(app.settings_torii_port_input, "8080");
        assert_eq!(app.settings_p2p_port_input, "1337");
    }

    struct TestEnvGuard {
        key: &'static str,
        prev: Option<String>,
    }

    impl TestEnvGuard {
        fn set(key: &'static str, value: &Path) -> Self {
            let prev = env::var(key).ok();
            // SAFETY: Tests run single-threaded under an env lock, so mutating env vars is safe.
            unsafe { env::set_var(key, value) };
            Self { key, prev }
        }

        fn set_str(key: &'static str, value: &str) -> Self {
            let prev = env::var(key).ok();
            // SAFETY: Tests run single-threaded under an env lock, so mutating env vars is safe.
            unsafe { env::set_var(key, value) };
            Self { key, prev }
        }
    }

    impl Drop for TestEnvGuard {
        fn drop(&mut self) {
            if let Some(prev) = self.prev.as_ref() {
                unsafe { env::set_var(self.key, prev) };
            } else {
                unsafe { env::remove_var(self.key) };
            }
        }
    }

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn spawn_mock_torii_server() -> (u16, std::thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock torii");
        let port = listener.local_addr().expect("listener addr").port();
        let handle = std::thread::spawn(move || {
            for stream in listener.incoming().take(2) {
                let mut stream = match stream {
                    Ok(stream) => stream,
                    Err(_) => break,
                };
                let mut buffer = [0u8; 4096];
                let read = match stream.read(&mut buffer) {
                    Ok(read) => read,
                    Err(_) => continue,
                };
                if read == 0 {
                    continue;
                }
                let request = String::from_utf8_lossy(&buffer[..read]);
                let path = request
                    .lines()
                    .next()
                    .and_then(|line| line.split_whitespace().nth(1))
                    .unwrap_or("/");
                match path {
                    "/status" => {
                        let status = TelemetryStatus::default();
                        let body = norito::to_bytes(&status).expect("encode status");
                        let header = format!(
                            "HTTP/1.1 200 OK\r\nContent-Type: application/norito\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                            body.len()
                        );
                        let _ = stream.write_all(header.as_bytes());
                        let _ = stream.write_all(&body);
                    }
                    "/v1/nexus/lifecycle" => {
                        let body = b"{}";
                        let header = format!(
                            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                            body.len()
                        );
                        let _ = stream.write_all(header.as_bytes());
                        let _ = stream.write_all(body);
                    }
                    _ => {
                        let header = "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
                        let _ = stream.write_all(header.as_bytes());
                    }
                }
            }
        });
        (port, handle)
    }

    fn genesis_invocation_count(path: &Path) -> usize {
        if !path.exists() {
            return 0;
        }
        let contents =
            fs::read_to_string(path).unwrap_or_else(|err| panic!("read kagami log: {err}"));
        contents.lines().filter(|line| *line == "genesis").count()
    }

    fn install_kagami_stub(root: &Path) -> PathBuf {
        install_stub_script(
            root,
            "kagami_stub.sh",
            r#"#!/bin/sh
set -e
if [ "$1" = "genesis" ] && [ "$2" = "generate" ]; then
  LOG_FILE="${MOCHI_TEST_KAGAMI_LOG:-}"
  if [ -n "$LOG_FILE" ]; then
    printf '%s\n' "$@" >> "$LOG_FILE"
  fi
  cat <<'JSON'
{"chain":"00000000-0000-0000-0000-000000000000","ivm_dir":".","transactions":[{"instructions":[]}]}
JSON
else
  printf 'unexpected invocation: %s\n' "$0 $*" >&2
  exit 1
fi
"#,
        )
    }

    fn install_noop_stub(root: &Path, name: &str) -> PathBuf {
        install_stub_script(
            root,
            name,
            r#"#!/bin/sh
exit 0
"#,
        )
    }

    fn install_stub_script(root: &Path, name: &str, contents: &str) -> PathBuf {
        let path = root.join(name);
        fs::write(&path, contents).expect("write stub");
        make_executable(&path);
        path
    }

    #[cfg(unix)]
    fn make_executable(path: &Path) {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(path).expect("metadata").permissions();
        perms.set_mode(0o755);
        fs::set_permissions(path, perms).expect("set perms");
    }

    #[cfg(not(unix))]
    fn make_executable(_path: &Path) {}
}
