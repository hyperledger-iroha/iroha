//! Command selection, validated startup overrides and profile input for the desktop shell.

use super::config::BinaryOverrides;
use iroha_data_model::parameter::system::SumeragiConsensusMode;
use iroha_model_base::chain::ChainId;
use mochi_core::{
    GenesisProfile, NetworkProfile, ProfilePreset, SupervisorBuilder, sandbox_root_for_workspace,
    supervisor::RestartPolicy,
};
use std::{env, ffi::OsString, fs, path::PathBuf, time::Duration};
use toml::{Table as TomlTable, Value as TomlValue};

#[derive(Debug, Default, Clone)]
pub(super) struct CliOverrides {
    pub(super) workspace_root: Option<PathBuf>,
    pub(super) data_root: Option<PathBuf>,
    pub(super) profile: Option<NetworkProfile>,
    pub(super) config_path: Option<PathBuf>,
    pub(super) torii_start: Option<u16>,
    pub(super) p2p_start: Option<u16>,
    pub(super) chain_id: Option<String>,
    pub(super) genesis_profile: Option<GenesisProfile>,
    pub(super) vrf_seed_hex: Option<String>,
    pub(super) binaries: BinaryOverrides,
    pub(super) build_binaries: Option<bool>,
    pub(super) readiness_smoke: Option<bool>,
    pub(super) readiness_timeout: Option<Duration>,
    pub(super) restart_policy: Option<RestartPolicy>,
    pub(super) nexus_config: Option<toml::Table>,
    pub(super) nexus_lane_count: Option<u32>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ParsedProfileOverride {
    pub(super) profile: NetworkProfile,
    pub(super) genesis_profile: Option<GenesisProfile>,
}
impl CliOverrides {
    pub(super) fn apply_to(&self, mut builder: SupervisorBuilder) -> SupervisorBuilder {
        if let Some(root) = &self.data_root {
            builder = builder.data_root(root.clone());
        } else if let Some(root) = &self.workspace_root {
            builder = builder.data_root(sandbox_root_for_workspace(root));
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
        if let Some(profile) = self.genesis_profile {
            builder = builder.genesis_profile(profile);
        }
        if let Some(chain_id) = &self.chain_id {
            builder = builder.chain_id(chain_id.clone());
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
        if let Some(allow) = self.build_binaries {
            builder = builder.auto_build_binaries(allow);
        }
        if let Some(policy) = self.restart_policy {
            builder = builder.restart_policy(policy);
        }
        if let Some(nexus) = self.nexus_config.as_ref() {
            builder = builder.nexus_config(nexus.clone());
        }
        if let Some(lane_count) = self.nexus_lane_count {
            builder = builder.nexus_lane_count(lane_count);
        }
        builder
    }
}
#[derive(Debug)]
pub(super) struct ParsedCli {
    pub(super) command: CliCommand,
    pub(super) overrides: CliOverrides,
    pub(super) help: bool,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CliCommand {
    Gui,
    SandboxServe,
    SandboxWipeRehearsal,
}
#[derive(Debug)]
pub(super) struct CliParseError {
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
pub(super) fn parse_cli_overrides() -> Result<ParsedCli, CliParseError> {
    let args: Vec<OsString> = env::args_os().skip(1).collect();
    let mut parsed = parse_cli_overrides_from(args)?;
    if parsed.help {
        return Ok(parsed);
    }
    let env_overrides = parse_env_overrides()?;
    parsed.overrides = merge_overrides(env_overrides, parsed.overrides);
    Ok(parsed)
}
pub(super) fn parse_cli_overrides_from<I>(args: I) -> Result<ParsedCli, CliParseError>
where
    I: IntoIterator<Item = OsString>,
{
    let (command, args) = extract_cli_command(args)?;
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
                    command,
                    overrides,
                    help: true,
                });
            }
            "--workspace-root" => {
                let value = next_value(&mut iter, "--workspace-root")?;
                overrides.workspace_root = Some(PathBuf::from(value));
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
                overrides.chain_id = Some(parse_chain_id_override(&value, "--chain-id")?);
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
                overrides.vrf_seed_hex = Some(parse_vrf_seed_override(&value, "--vrf-seed-hex")?);
            }
            "--nexus-config" => {
                let value = next_value_string(&mut iter, "--nexus-config")?;
                overrides.nexus_config = Some(parse_nexus_config_file(&value)?);
            }
            "--nexus-lane-count" => {
                let value = next_value_string(&mut iter, "--nexus-lane-count")?;
                overrides.nexus_lane_count = Some(parse_u32_flag(&value, "--nexus-lane-count")?);
            }
            "--irohad" => {
                let value = next_value(&mut iter, "--irohad")?;
                overrides.binaries.irohad = Some(PathBuf::from(value));
            }
            "--kagami" => {
                let value = next_value(&mut iter, "--kagami")?;
                overrides.binaries.kagami = Some(PathBuf::from(value));
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
            "--readiness-timeout-ms" => {
                let value = next_value_string(&mut iter, "--readiness-timeout-ms")?;
                overrides.readiness_timeout = Some(parse_positive_millis_flag(
                    &value,
                    "--readiness-timeout-ms",
                )?);
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
        command,
        overrides,
        help: false,
    })
}
fn extract_cli_command<I>(args: I) -> Result<(CliCommand, Vec<OsString>), CliParseError>
where
    I: IntoIterator<Item = OsString>,
{
    let mut args: Vec<OsString> = args.into_iter().collect();
    let Some(first) = args.first() else {
        return Ok((CliCommand::Gui, args));
    };
    let first = first
        .to_str()
        .ok_or_else(|| CliParseError::new("arguments must be valid UTF-8"))?;
    if first == "sandbox" {
        let Some(second) = args.get(1) else {
            return Err(CliParseError::new("expected a sandbox subcommand"));
        };
        let second = second
            .to_str()
            .ok_or_else(|| CliParseError::new("arguments must be valid UTF-8"))?;
        let command = match second {
            "serve" => CliCommand::SandboxServe,
            "rehearse-wipe-and-regenerate" => CliCommand::SandboxWipeRehearsal,
            _ => {
                return Err(CliParseError::new(format!(
                    "unknown sandbox subcommand `{second}`"
                )));
            }
        };
        args.drain(0..2);
        return Ok((command, args));
    }
    Ok((CliCommand::Gui, args))
}
fn merge_overrides(env: CliOverrides, cli: CliOverrides) -> CliOverrides {
    CliOverrides {
        workspace_root: cli.workspace_root.or(env.workspace_root),
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
        },
        build_binaries: cli.build_binaries.or(env.build_binaries),
        readiness_smoke: cli.readiness_smoke.or(env.readiness_smoke),
        readiness_timeout: cli.readiness_timeout.or(env.readiness_timeout),
        restart_policy: cli.restart_policy.or(env.restart_policy),
        nexus_config: cli.nexus_config.or(env.nexus_config),
        nexus_lane_count: cli.nexus_lane_count.or(env.nexus_lane_count),
    }
}
pub(super) fn parse_env_overrides() -> Result<CliOverrides, CliParseError> {
    let mut overrides = CliOverrides::default();
    if let Some(root) = env_value("MOCHI_WORKSPACE_ROOT")? {
        overrides.workspace_root = Some(PathBuf::from(root));
    }
    if let Some(root) = env_value("MOCHI_DATA_ROOT")? {
        overrides.data_root = Some(PathBuf::from(root));
    }
    if let Some(profile) = env_value("MOCHI_PROFILE")? {
        let parsed = parse_profile_override(&profile)?;
        apply_profile_override(&mut overrides, parsed, "MOCHI_PROFILE")?;
    }
    if let Some(chain_id) = env_value("MOCHI_CHAIN_ID")? {
        overrides.chain_id = Some(parse_chain_id_override(&chain_id, "MOCHI_CHAIN_ID")?);
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
        overrides.vrf_seed_hex = Some(parse_vrf_seed_override(&seed, "MOCHI_VRF_SEED_HEX")?);
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
    if let Some(value) = env_value("MOCHI_READINESS_TIMEOUT_MS")? {
        overrides.readiness_timeout = Some(parse_positive_millis_flag(
            &value,
            "MOCHI_READINESS_TIMEOUT_MS",
        )?);
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
pub(super) fn parse_profile_override(value: &str) -> Result<ParsedProfileOverride, CliParseError> {
    if value.is_empty() {
        return Err(CliParseError::new("profile value must not be empty"));
    }
    if let Some(preset) = parse_profile_preset(value) {
        return Ok(ParsedProfileOverride {
            profile: NetworkProfile::from_preset(preset),
            genesis_profile: None,
        });
    }
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(CliParseError::new("profile value must not be empty"));
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
    if let Some(field) = table.keys().find(|field| {
        !matches!(
            field.as_str(),
            "peer_count" | "consensus_mode" | "genesis_profile"
        )
    }) {
        return Err(CliParseError::new(format!(
            "profile override contains unknown field `{field}`"
        )));
    }
    let peer_value = table
        .get("peer_count")
        .ok_or_else(|| CliParseError::new("profile override missing `peer_count`"))?;
    let peer_count = parse_profile_peer_count(peer_value)?;
    let consensus_value = table
        .get("consensus_mode")
        .ok_or_else(|| CliParseError::new("profile override missing `consensus_mode`"))?;
    let consensus_mode = parse_profile_consensus_mode(consensus_value)?;
    let genesis_profile = match table.get("genesis_profile") {
        None => None,
        Some(TomlValue::String(value)) if !value.is_empty() => Some(
            value
                .parse()
                .map_err(|err: String| CliParseError::new(err))?,
        ),
        Some(TomlValue::String(_)) => {
            return Err(CliParseError::new(
                "profile override genesis_profile must not be empty",
            ));
        }
        Some(_) => {
            return Err(CliParseError::new(
                "profile override genesis_profile must be a string",
            ));
        }
    };
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
    match raw {
        "permissioned" => Ok(SumeragiConsensusMode::Permissioned),
        "npos" => Ok(SumeragiConsensusMode::Npos),
        other => Err(CliParseError::new(format!(
            "profile override consensus_mode `{other}` is not supported"
        ))),
    }
}
pub(super) fn parse_profile_preset(value: &str) -> Option<ProfilePreset> {
    match value {
        "four-peer-bft" => Some(ProfilePreset::FourPeerBft),
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
    let invalid_root = || {
        CliParseError::new(format!(
            "--nexus-config {path} must contain exactly one `[nexus]` TOML table"
        ))
    };
    let Some(table) = value.as_table() else {
        return Err(invalid_root());
    };
    if table.len() != 1 || !table.contains_key("nexus") {
        return Err(invalid_root());
    }
    table["nexus"].as_table().cloned().ok_or_else(invalid_root)
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
fn parse_chain_id_override(value: &str, source: &str) -> Result<String, CliParseError> {
    value
        .parse::<ChainId>()
        .map(|chain_id| chain_id.to_string())
        .map_err(|error| CliParseError::new(format!("invalid {source} value: {error}")))
}
fn parse_vrf_seed_override(value: &str, source: &str) -> Result<String, CliParseError> {
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(CliParseError::new(format!(
            "{source} must contain exactly 64 hexadecimal characters"
        )));
    }
    Ok(value.to_owned())
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RestartModeFlag {
    Never,
    OnFailure,
}
fn parse_restart_mode_flag(value: &str) -> Result<RestartModeFlag, CliParseError> {
    match value {
        "never" => Ok(RestartModeFlag::Never),
        "on-failure" => Ok(RestartModeFlag::OnFailure),
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
fn parse_positive_millis_flag(value: &str, flag: &str) -> Result<Duration, CliParseError> {
    let millis = parse_u64_flag(value, flag)?;
    if millis == 0 {
        return Err(CliParseError::new(format!(
            "{flag} expects an integer greater than zero"
        )));
    }
    Ok(Duration::from_millis(millis))
}
fn parse_bool_flag(value: &str, flag: &str) -> Result<bool, CliParseError> {
    match value {
        "true" => Ok(true),
        "false" => Ok(false),
        other => Err(CliParseError::new(format!(
            "{flag} expects `true` or `false`, got `{other}`"
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
pub(super) fn print_cli_usage() {
    println!("MOCHI usage:");
    println!("  mochi [options]");
    println!("  mochi sandbox serve [options]");
    println!("  mochi sandbox rehearse-wipe-and-regenerate [options]");
    println!("Options:");
    println!(
        "  --workspace-root <path>      Workspace root; Mochi stores runtime state under .mochi/sandbox."
    );
    println!("  --data-root <path>           Override the supervisor data root.");
    println!("  --profile <four-peer-bft|{{ peer_count = 7, consensus_mode = \"permissioned\" }}>");
    println!("                               Choose a preset or custom profile table.");
    println!("  --config <path>              Load overrides from a specific config file.");
    println!("  --torii-start <port>         Override the base Torii port.");
    println!("  --p2p-start <port>           Override the base P2P port.");
    println!("  --chain-id <string>          Override the generated chain id.");
    println!("  --genesis-profile <iroha3-dev|iroha3-taira>");
    println!("                               Use a Kagami genesis preset.");
    println!("  --vrf-seed-hex <hex>         VRF seed (32-byte hex) for genesis profile.");
    println!("  --nexus-config <path>        Load Nexus lane/dataspace config from TOML.");
    println!("  --nexus-lane-count <count>   Override nexus.lane_count in generated configs.");
    println!("  --irohad <path>              Override the iroha3d binary path.");
    println!("  --kagami <path>              Override the kagami binary path.");
    println!("  --build-binaries             Auto-build missing binaries via cargo.");
    println!("  --no-build-binaries          Disable auto-build of missing binaries.");
    println!("  --disable-smoke              Disable readiness smoke transactions.");
    println!("  --enable-smoke               Re-enable readiness smoke transactions.");
    println!("  --readiness-timeout-ms <millis>");
    println!("                               Bound cold-start readiness (default: 60000).");
    println!("  --restart-mode <never|on-failure>");
    println!("                               Choose the peer restart policy.");
    println!("  --restart-max <attempts>     Override retry attempts in on-failure mode.");
    println!("  --restart-backoff-ms <millis>  Override the base retry backoff (ms).");
    println!("  -h, --help                   Show this help text.");
}

#[cfg(test)]
mod tests;
