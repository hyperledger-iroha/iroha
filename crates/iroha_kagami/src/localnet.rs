//! Generate a bare-metal local network configuration (genesis, peer configs, scripts).

use std::{
    collections::BTreeSet,
    env,
    fs::{self, File},
    io::{BufWriter, Write},
    net::{Ipv4Addr, Ipv6Addr},
    num::{NonZeroU16, NonZeroU64},
    path::{Path, PathBuf},
};

use clap::{Args as ClapArgs, ValueEnum};
use color_eyre::eyre::{Result, WrapErr as _, eyre};
use iroha_core::sumeragi::network_topology::redundant_send_r_from_len;
use iroha_crypto::{ExposedPrivateKey, KeyPair};
use iroha_data_model::{
    isi::staking::{ActivatePublicLaneValidator, RegisterPublicLaneValidator},
    parameter::system::{
        SumeragiConsensusMode, SumeragiNposParameters, SumeragiParameter, SumeragiParameters,
    },
    peer::PeerId,
    prelude::*,
};
use iroha_genesis::{
    GenesisBuilder, GenesisTopologyEntry, RawGenesisTransaction, init_instruction_registry,
};
use iroha_primitives::addr::{SocketAddr, SocketAddrHost};
use iroha_test_samples::{ALICE_ID, REAL_GENESIS_ACCOUNT_KEYPAIR};
use iroha_version::BuildLine;

use crate::{
    Outcome, RunArgs,
    genesis::{ConsensusPolicy, generate_default, validate_consensus_mode_for_line},
    tui,
};

/// User-facing options for generating a bare-metal localnet.
#[derive(Debug, Clone)]
pub struct LocalnetOptions {
    /// Build line selector (Iroha2 vs Iroha3).
    pub build_line: BuildLine,
    /// Optional Sora profile selector (multi-lane / dataspace defaults).
    pub sora_profile: Option<SoraProfile>,
    /// Optional localnet performance profile (throughput presets).
    pub perf_profile: Option<LocalnetPerfProfile>,
    /// Number of peers to create (deterministic ordering).
    pub peers: NonZeroU16,
    /// Optional seed to make key/port generation reproducible.
    pub seed: Option<String>,
    /// Host interface to bind P2P and Torii listeners to (host/IP only, no port).
    pub bind_host: String,
    /// Host peers should gossip to and clients should dial (host/IP only, no port).
    pub public_host: String,
    /// Base Torii API port; each peer increments this by one.
    pub base_api_port: u16,
    /// Base P2P port; each peer increments this by one.
    pub base_p2p_port: u16,
    /// Output directory for configs, scripts, and genesis.
    pub out_dir: PathBuf,
    /// Additional wonderland accounts to pre-register beyond Alice.
    pub extra_accounts: u16,
    /// Extra asset specs to register and optionally mint.
    pub assets: Vec<AssetSpec>,
    /// Optional override for consensus block time (milliseconds).
    /// If unset alongside `commit_time_ms`, a fast localnet pipeline is injected.
    /// If only one of block/commit is set, the other is mirrored to keep the pipeline balanced.
    pub block_time_ms: Option<u64>,
    /// Optional override for consensus commit timeout (milliseconds).
    /// If unset alongside `block_time_ms`, a fast localnet pipeline is injected.
    /// If only one of block/commit is set, the other is mirrored to keep the pipeline balanced.
    pub commit_time_ms: Option<u64>,
    /// Optional override for consensus redundant send fanout (r).
    pub redundant_send_r: Option<u8>,
    /// Consensus mode to emit in genesis/configs.
    pub consensus_mode: SumeragiConsensusMode,
    /// Optional staged consensus mode to activate at `mode_activation_height`.
    pub next_consensus_mode: Option<SumeragiConsensusMode>,
    /// Optional activation height for switching to `next_consensus_mode`.
    pub mode_activation_height: Option<u64>,
}

/// Asset definition plus optional minting target for sample generation.
#[derive(Debug, Clone)]
pub struct AssetSpec {
    /// Canonical asset definition ID (`aid:<32-lower-hex>`).
    pub id: String,
    /// Account that should receive the minted supply.
    pub mint_to: ScopedAccountId,
    /// Quantity to mint for this asset definition.
    pub quantity: u64,
}

#[derive(Debug, Clone)]
enum HostKind {
    Ipv4(Ipv4Addr),
    Ipv6(Ipv6Addr),
    Name(String),
}

#[derive(Debug, Clone)]
struct CanonicalHost {
    kind: HostKind,
}

impl CanonicalHost {
    fn parse(raw: &str, field: &str) -> Result<Self> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(eyre!("`{field}` must not be empty"));
        }
        let has_prefix = trimmed.starts_with('[');
        let has_suffix = trimmed.ends_with(']');
        if has_prefix != has_suffix {
            return Err(eyre!("`{field}` has unmatched '[' or ']': `{raw}`"));
        }
        let unbracketed = if has_prefix && trimmed.len() >= 2 {
            &trimmed[1..trimmed.len() - 1]
        } else {
            trimmed
        };
        if unbracketed.is_empty() {
            return Err(eyre!("`{field}` must not be empty"));
        }
        if let Ok(ipv4) = unbracketed.parse::<Ipv4Addr>() {
            return Ok(Self {
                kind: HostKind::Ipv4(ipv4),
            });
        }
        if let Ok(ipv6) = unbracketed.parse::<Ipv6Addr>() {
            return Ok(Self {
                kind: HostKind::Ipv6(ipv6),
            });
        }
        if unbracketed.contains(':') {
            return Err(eyre!(
                "`{field}` must be a host name or IP literal without a port: `{raw}`"
            ));
        }
        Ok(Self {
            kind: HostKind::Name(unbracketed.to_ascii_lowercase()),
        })
    }

    fn addr_literal(&self, port: u16) -> String {
        let addr = match &self.kind {
            HostKind::Ipv4(ipv4) => SocketAddr::from((ipv4.octets(), port)),
            HostKind::Ipv6(ipv6) => SocketAddr::from((ipv6.segments(), port)),
            HostKind::Name(host) => SocketAddr::Host(SocketAddrHost {
                host: host.clone().into(),
                port,
            }),
        };
        addr.to_literal()
    }

    fn url_host(&self) -> String {
        match &self.kind {
            HostKind::Ipv4(ipv4) => ipv4.to_string(),
            HostKind::Ipv6(ipv6) => format!("[{ipv6}]"),
            HostKind::Name(host) => host.clone(),
        }
    }

    fn torii_url(&self, port: u16) -> String {
        format!("http://{}:{port}/", self.url_host())
    }
}

#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ConsensusModeArg {
    Permissioned,
    Npos,
}

impl From<ConsensusModeArg> for SumeragiConsensusMode {
    fn from(value: ConsensusModeArg) -> Self {
        match value {
            ConsensusModeArg::Permissioned => SumeragiConsensusMode::Permissioned,
            ConsensusModeArg::Npos => SumeragiConsensusMode::Npos,
        }
    }
}

/// SORA network profiles that influence localnet defaults.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SoraProfile {
    /// Dataspace-oriented defaults.
    Dataspace,
    /// Public dataspace (Nexus) defaults.
    Nexus,
}

impl SoraProfile {
    fn consensus_policy(self) -> ConsensusPolicy {
        match self {
            SoraProfile::Dataspace | SoraProfile::Nexus => ConsensusPolicy::PublicDataspace,
        }
    }
}

/// Localnet performance profiles for 10k TPS / 1s finality runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocalnetPerfProfile {
    /// 10k TPS / 1s finality baseline for permissioned mode.
    Throughput10kPermissioned,
    /// 10k TPS / 1s finality baseline for NPoS mode.
    Throughput10kNpos,
}

#[derive(Debug, Clone, Copy)]
struct LocalnetPerfProfileSpec {
    consensus_mode: SumeragiConsensusMode,
    block_time_ms: u64,
    commit_time_ms: u64,
    collectors_k: u16,
    redundant_send_r: Option<u8>,
    block_max_transactions: u64,
    stake_amount: u64,
}

impl LocalnetPerfProfile {
    fn spec(self) -> LocalnetPerfProfileSpec {
        let consensus_mode = match self {
            LocalnetPerfProfile::Throughput10kPermissioned => SumeragiConsensusMode::Permissioned,
            LocalnetPerfProfile::Throughput10kNpos => SumeragiConsensusMode::Npos,
        };
        LocalnetPerfProfileSpec {
            consensus_mode,
            block_time_ms: 1_000,
            commit_time_ms: 1_000,
            collectors_k: 3,
            redundant_send_r: None,
            block_max_transactions: LOCALNET_BLOCK_MAX_TRANSACTIONS,
            stake_amount: LOCALNET_STAKE_AMOUNT,
        }
    }

    fn consensus_mode(self) -> SumeragiConsensusMode {
        self.spec().consensus_mode
    }
}

#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SoraProfileArg {
    #[value(alias = "dataspaces")]
    Dataspace,
    #[value(alias = "public", alias = "sora-nexus", alias = "nexus-public")]
    Nexus,
}

impl From<SoraProfileArg> for SoraProfile {
    fn from(value: SoraProfileArg) -> Self {
        match value {
            SoraProfileArg::Dataspace => SoraProfile::Dataspace,
            SoraProfileArg::Nexus => SoraProfile::Nexus,
        }
    }
}

#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LocalnetPerfProfileArg {
    #[value(name = "10k-permissioned", alias = "throughput-10k-permissioned")]
    Throughput10kPermissioned,
    #[value(name = "10k-npos", alias = "throughput-10k-npos")]
    Throughput10kNpos,
}

impl From<LocalnetPerfProfileArg> for LocalnetPerfProfile {
    fn from(value: LocalnetPerfProfileArg) -> Self {
        match value {
            LocalnetPerfProfileArg::Throughput10kPermissioned => {
                LocalnetPerfProfile::Throughput10kPermissioned
            }
            LocalnetPerfProfileArg::Throughput10kNpos => LocalnetPerfProfile::Throughput10kNpos,
        }
    }
}

#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BuildLineArg {
    #[value(alias = "i2", alias = "2")]
    Iroha2,
    #[value(alias = "i3", alias = "3")]
    Iroha3,
}

impl From<BuildLineArg> for BuildLine {
    fn from(value: BuildLineArg) -> Self {
        match value {
            BuildLineArg::Iroha2 => BuildLine::Iroha2,
            BuildLineArg::Iroha3 => BuildLine::Iroha3,
        }
    }
}

impl std::fmt::Display for BuildLineArg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let label = match self {
            BuildLineArg::Iroha2 => "iroha2",
            BuildLineArg::Iroha3 => "iroha3",
        };
        f.write_str(label)
    }
}

const CHAIN_ID: &str = "00000000-0000-0000-0000-000000000000";
const GENESIS_SEED: &[u8; 7] = b"genesis";
/// Localnet uses larger channel caps to keep DA/RBC traffic from dropping at 1s block times.
const LOCALNET_MSG_CHANNEL_CAP_VOTES: usize = 8_192;
const LOCALNET_MSG_CHANNEL_CAP_BLOCK_PAYLOAD: usize = 256;
const LOCALNET_MSG_CHANNEL_CAP_RBC_CHUNKS: usize = 4_096;
const LOCALNET_MSG_CHANNEL_CAP_BLOCKS: usize = 512;
const LOCALNET_CONTROL_MSG_CHANNEL_CAP: usize = 1024;
/// Capacity for the inbound P2P subscriber queue in localnet configs.
const LOCALNET_P2P_SUBSCRIBER_QUEUE_CAP: usize = 16_384;
/// Delay outbound P2P dials at startup to avoid connection refused spam in localnet.
const LOCALNET_CONNECT_STARTUP_DELAY_MS: u64 = 2_000;
/// Default consensus ingress rate cap (msgs/sec) for localnet.
const LOCALNET_CONSENSUS_INGRESS_RATE_PER_SEC: u32 = 600;
/// Default consensus ingress burst cap (msgs) for localnet.
const LOCALNET_CONSENSUS_INGRESS_BURST: u32 = 600;
/// Default consensus ingress bytes/sec cap for localnet.
const LOCALNET_CONSENSUS_INGRESS_BYTES_PER_SEC: u32 = 134_217_728; // 128 MiB
/// Default consensus ingress bytes burst cap for localnet.
const LOCALNET_CONSENSUS_INGRESS_BYTES_BURST: u32 = 268_435_456; // 256 MiB
/// Default critical consensus ingress rate cap (msgs/sec) for localnet.
const LOCALNET_CONSENSUS_INGRESS_CRITICAL_RATE_PER_SEC: u32 = 600;
/// Default critical consensus ingress burst cap (msgs) for localnet.
const LOCALNET_CONSENSUS_INGRESS_CRITICAL_BURST: u32 = 600;
/// Default critical consensus ingress bytes/sec cap for localnet.
const LOCALNET_CONSENSUS_INGRESS_CRITICAL_BYTES_PER_SEC: u32 = 268_435_456; // 256 MiB
/// Default critical consensus ingress bytes burst cap for localnet.
const LOCALNET_CONSENSUS_INGRESS_CRITICAL_BYTES_BURST: u32 = 536_870_912; // 512 MiB
/// Transaction gossip cadence for 1s localnet pipelines (ms).
const LOCALNET_TX_GOSSIP_PERIOD_FAST_MS: u64 = 100;
/// Transaction gossip resend ticks for 1s localnet pipelines.
const LOCALNET_TX_GOSSIP_RESEND_TICKS_FAST: u32 = 1;
/// Default listener host for generated P2P and Torii services.
pub const DEFAULT_BIND_HOST: &str = "0.0.0.0";
/// Default advertised host for generated peers and client config.
pub const DEFAULT_PUBLIC_HOST: &str = "127.0.0.1";
/// Default total pipeline time (ms) injected for localnet when not overridden.
const LOCALNET_PIPELINE_TIME_MS: u64 = 1_000;
/// Localnet pacing governor lower bound (basis points).
const LOCALNET_PACING_GOVERNOR_MIN_FACTOR_BPS: u32 = 10_000;
/// Localnet pacing governor upper bound (basis points).
const LOCALNET_PACING_GOVERNOR_MAX_FACTOR_BPS: u32 = 10_000;
/// Baseline NPoS timeouts for localnet (keep in sync with config defaults).
/// Default DA commit-quorum timeout multiplier for localnet configs.
const LOCALNET_DA_QUORUM_TIMEOUT_MULTIPLIER: u32 = 1;
/// Default DA availability timeout multiplier for localnet configs.
/// Extra slack keeps advisory availability warnings from firing on fast pipelines.
const LOCALNET_DA_AVAILABILITY_TIMEOUT_MULTIPLIER: u32 = 1;
/// Default DA availability timeout floor (ms) for localnet configs.
const LOCALNET_DA_AVAILABILITY_TIMEOUT_FLOOR_MS: u64 = 2_000;
/// Default RBC chunk size for localnet payloads.
const LOCALNET_RBC_CHUNK_MAX_BYTES: usize = 256 * 1024;
/// Maximum RBC payload chunks broadcast per tick for localnet.
const LOCALNET_RBC_PAYLOAD_CHUNKS_PER_TICK: usize = 128;
/// Default queue capacity for localnet (safe-by-default).
///
/// This value intentionally trades peak stress throughput for bounded memory
/// usage when consensus stalls or clients oversubmit.
const LOCALNET_QUEUE_CAPACITY: usize = 20_000;
/// Queue capacity used for perf-profile localnets (keeps stress bursts in-memory).
const LOCALNET_PERF_QUEUE_CAPACITY: usize = 262_144;
/// Default transaction TTL in the queue for localnet (ms).
const LOCALNET_QUEUE_TTL_MS: u64 = 600_000;
/// Default lane TEU capacity for localnet scheduling (raises per-block budget).
const LOCALNET_LANE_TEU_CAPACITY: u32 = 1_000_000;
/// Default multiplier for proposal queue scan budgets on localnet.
const LOCALNET_PROPOSAL_QUEUE_SCAN_MULTIPLIER: usize = 4;
/// Default Torii tx rate limit (per authority) for localnet.
const LOCALNET_TORII_TX_RATE_PER_AUTHORITY_PER_SEC: u32 = 1_000_000;
/// Default Torii tx burst limit (per authority) for localnet.
const LOCALNET_TORII_TX_BURST_PER_AUTHORITY: u32 = 2_000_000;
/// Default Torii pre-auth rate limit (per IP) for localnet.
const LOCALNET_TORII_PREAUTH_RATE_PER_IP_PER_SEC: u32 = 1_000_000;
/// Default Torii pre-auth burst limit (per IP) for localnet.
const LOCALNET_TORII_PREAUTH_BURST_PER_IP: u32 = 2_000_000;
/// Torii pre-auth allowlist to keep localnet CLI traffic from tripping bans.
const LOCALNET_PREAUTH_ALLOW_CIDRS: [&str; 2] = ["127.0.0.0/8", "::1/128"];
/// Multiplier applied to block+commit time for localnet commit inflight timeout.
const LOCALNET_COMMIT_INFLIGHT_TIMEOUT_MULTIPLIER: u64 = 6;
/// Lower bound for localnet commit inflight timeout to avoid overly aggressive aborts.
const LOCALNET_COMMIT_INFLIGHT_TIMEOUT_MIN_MS: u64 = 4_000;
/// Upper bound for localnet commit inflight timeout to prevent long stalls.
const LOCALNET_COMMIT_INFLIGHT_TIMEOUT_MAX_MS: u64 = 10_000;
/// Default localnet telemetry toggle (mirrors config defaults).
const LOCALNET_TELEMETRY_ENABLED: bool = true;
/// Default localnet telemetry profile (mirrors config defaults).
const LOCALNET_TELEMETRY_PROFILE: &str = "extended";
/// Minimum peer count for Sora profile localnets (multi-lane/dataspace defaults).
const LOCALNET_SORA_MIN_PEERS: u16 = 4;
/// Divisor applied to derive the localnet NPoS aggregator fallback timeout.
/// Keep this at 1 so aggregators do not time out before quorum on fast pipelines.
/// Default max transactions per block for localnet (targets 10k TPS).
const LOCALNET_BLOCK_MAX_TRANSACTIONS: u64 = 10_000;
/// Default stake bonded per localnet validator (raised to meet min_self_bond).
const LOCALNET_STAKE_AMOUNT: u64 = 10_000;
const LOCALNET_NEXUS_DOMAIN: &str = "nexus";
const LOCALNET_IVM_DOMAIN: &str = "ivm";
const LOCALNET_STAKE_ASSET_NAME: &str = "xor";
const LOCALNET_SAMPLE_ASSET_DOMAIN: &str = "wonderland";
const LOCALNET_SAMPLE_ASSET_NAME: &str = "sample";
const LOCALNET_GAS_ACCOUNT_SEED: &[u8] = b"localnet-gas-account";
/// Default localnet client TTL (ms) to keep stress submissions from expiring prematurely.
const LOCALNET_CLIENT_TTL_MS: u64 = 600_000;
/// Default localnet client status timeout (ms); must stay <= TTL.
const LOCALNET_CLIENT_STATUS_TIMEOUT_MS: u64 = 300_000;
/// Default Kura fsync mode for localnet (performance-oriented).
const LOCALNET_KURA_FSYNC_MODE: &str = "off";
/// Ed25519 signature batch size for perf-profile localnets (0 disables batching).
const LOCALNET_SIGNATURE_BATCH_MAX_ED25519: usize = 64;
/// Logger filter for perf-profile localnets to avoid per-transaction log floods.
const LOCALNET_PERF_LOGGER_FILTER: &str = "info,iroha_torii::routing=warn";
const STREAM_ID_PUBLIC: &str =
    "ed01201C61FAF8FE94E253B93114240394F79A607B7FA55F9E5A41EBEC74B88055768B";
const STREAM_ID_PRIVATE: &str =
    "802620282ED9F3CF92811C3818DBC4AE594ED59DC1A2F78E4241E31924E101D6B1FB83";
const RANS_SEED0_TABLE: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../codec/rans/tables/rans_seed0.toml"
));

fn localnet_dataspace_fault_tolerance(peers: NonZeroU16) -> u32 {
    let peers = u32::from(peers.get());
    let fault_tolerance = peers.saturating_sub(1) / 3;
    fault_tolerance.max(1)
}

fn canonical_asset_definition_id(domain: &str, name: &str) -> AssetDefinitionId {
    AssetDefinitionId::new(
        domain
            .parse()
            .expect("static asset definition domain must remain valid"),
        name.parse()
            .expect("static asset definition name must remain valid"),
    )
}

pub(crate) fn canonical_asset_definition_literal(domain: &str, name: &str) -> String {
    canonical_asset_definition_id(domain, name).canonical_literal()
}

fn localnet_stake_asset_definition_id() -> AssetDefinitionId {
    canonical_asset_definition_id(LOCALNET_NEXUS_DOMAIN, LOCALNET_STAKE_ASSET_NAME)
}

fn localnet_stake_asset_literal() -> String {
    canonical_asset_definition_literal(LOCALNET_NEXUS_DOMAIN, LOCALNET_STAKE_ASSET_NAME)
}

fn localnet_sample_asset_literal() -> String {
    canonical_asset_definition_literal(LOCALNET_SAMPLE_ASSET_DOMAIN, LOCALNET_SAMPLE_ASSET_NAME)
}

/// Generate a bare-metal local network (no Docker): genesis, per-peer configs, start/stop scripts.
#[derive(ClapArgs, Debug, Clone)]
pub struct Args {
    /// Number of peers to generate.
    #[arg(long, short, value_name = "COUNT", default_value_t = NonZeroU16::new(4).unwrap())]
    peers: NonZeroU16,
    /// Optional UTF-8 seed for deterministic keys.
    #[arg(long, short)]
    seed: Option<String>,
    /// Select the build line (`iroha2` or `iroha3`) for DA/RBC defaults.
    /// Defaults to `iroha3`, so leaving `--consensus-mode` unset yields `npos`.
    #[arg(long, value_enum, value_name = "LINE", default_value_t = BuildLineArg::Iroha3)]
    build_line: BuildLineArg,
    /// Enable Sora profile defaults; `nexus` enforces public dataspace rules (NPoS).
    /// Requires `--build-line iroha3` and at least 4 peers.
    #[arg(long, value_enum, value_name = "PROFILE")]
    sora_profile: Option<SoraProfileArg>,
    /// Apply a localnet performance profile (10k TPS / 1s finality presets).
    #[arg(long, value_enum, value_name = "PROFILE")]
    perf_profile: Option<LocalnetPerfProfileArg>,
    /// Host to bind P2P and Torii listeners to (host/IP only, no port).
    #[arg(long, default_value = DEFAULT_BIND_HOST, value_name = "HOST")]
    bind_host: String,
    /// Host to advertise to peers and use for client Torii URL (host/IP only, no port).
    #[arg(long, default_value = DEFAULT_PUBLIC_HOST, value_name = "HOST")]
    public_host: String,
    /// Base Torii API port (per-peer increments by 1).
    #[arg(long, default_value_t = 8080)]
    base_api_port: u16,
    /// Base P2P port (per-peer increments by 1).
    #[arg(long, default_value_t = 1337)]
    base_p2p_port: u16,
    /// Output directory for configs/genesis/scripts.
    #[arg(long, short, value_name = "DIR")]
    out_dir: PathBuf,
    /// Extra accounts to pre-register (in wonderland).
    #[arg(long, default_value_t = 0)]
    extra_accounts: u16,
    /// Register a sample asset and mint to the default account.
    #[arg(long, default_value_t = false)]
    sample_asset: bool,
    /// Override the consensus block time (milliseconds) in generated manifests/configs.
    /// Leave unset to use the fast localnet pipeline defaults. If only one of
    /// `--block-time-ms`/`--commit-time-ms` is supplied, Kagami mirrors it to the other.
    #[arg(long, value_name = "MILLISECONDS", value_parser = clap::value_parser!(u64).range(1..))]
    block_time_ms: Option<u64>,
    /// Override the consensus commit timeout (milliseconds) in generated manifests/configs.
    /// Leave unset to use the fast localnet pipeline defaults. If only one of
    /// `--block-time-ms`/`--commit-time-ms` is supplied, Kagami mirrors it to the other.
    #[arg(long, value_name = "MILLISECONDS", value_parser = clap::value_parser!(u64).range(1..))]
    commit_time_ms: Option<u64>,
    /// Override redundant send fanout (r) for block payload dissemination.
    #[arg(long, value_name = "COUNT")]
    redundant_send_r: Option<u8>,
    /// Consensus mode to emit in genesis/configs.
    /// Defaults to `npos` on Iroha3 and `permissioned` on Iroha2.
    /// Localnet defaults to `--build-line iroha3`, so leaving this unset yields `npos`
    /// unless you set `--build-line iroha2` or `--consensus-mode permissioned`.
    /// Sora profile localnets require `npos` because the global merge ledger is NPoS.
    #[arg(long, value_enum, value_name = "MODE")]
    consensus_mode: Option<ConsensusModeArg>,
    /// Optional staged consensus mode to activate at `mode_activation_height`.
    #[arg(long, value_enum, value_name = "MODE")]
    next_consensus_mode: Option<ConsensusModeArg>,
    /// Optional activation height for switching to `next_consensus_mode`.
    #[arg(long, value_name = "HEIGHT")]
    mode_activation_height: Option<u64>,
}

impl<T: Write> RunArgs<T> for Args {
    fn run(self, writer: &mut BufWriter<T>) -> Outcome {
        let build_line = BuildLine::from(self.build_line);
        let sora_profile = self.sora_profile.map(SoraProfile::from);
        let perf_profile = self.perf_profile.map(LocalnetPerfProfile::from);
        let consensus_mode = self.consensus_mode.map_or_else(
            || {
                if let Some(perf_profile) = perf_profile {
                    return perf_profile.consensus_mode();
                }
                if build_line.is_iroha3() {
                    SumeragiConsensusMode::Npos
                } else {
                    SumeragiConsensusMode::Permissioned
                }
            },
            SumeragiConsensusMode::from,
        );
        let opts = LocalnetOptions {
            build_line,
            sora_profile,
            perf_profile,
            peers: self.peers,
            seed: self.seed,
            bind_host: self.bind_host,
            public_host: self.public_host,
            base_api_port: self.base_api_port,
            base_p2p_port: self.base_p2p_port,
            out_dir: self.out_dir,
            extra_accounts: self.extra_accounts,
            assets: if self.sample_asset {
                vec![AssetSpec {
                    id: localnet_sample_asset_literal(),
                    mint_to: ALICE_ID
                        .clone()
                        .to_account_id("wonderland".parse().expect("valid domain")),
                    quantity: 100,
                }]
            } else {
                vec![]
            },
            consensus_mode,
            next_consensus_mode: self.next_consensus_mode.map(Into::into),
            mode_activation_height: self.mode_activation_height,
            block_time_ms: self.block_time_ms,
            commit_time_ms: self.commit_time_ms,
            redundant_send_r: self.redundant_send_r,
        };
        generate_localnet(&opts, writer)
    }
}

struct Peer {
    public_key: iroha_crypto::PublicKey,
    private_key: iroha_crypto::ExposedPrivateKey,
    bls_public_key: iroha_crypto::PublicKey,
    bls_pop: Vec<u8>,
    api_port: u16,
    p2p_port: u16,
}

#[derive(Debug, Clone)]
struct ResolvedHosts {
    bind: CanonicalHost,
    public: CanonicalHost,
}

#[derive(Debug, Clone)]
struct BlsEntry {
    bls_pk: String,
    pop_hex: String,
}

/// Generate a self-contained localnet: configs, genesis, client config, scripts.
///
/// # Errors
/// Returns an error if port ranges are invalid or if config, genesis, or script files cannot be written.
pub fn generate_localnet<T: Write>(opts: &LocalnetOptions, writer: &mut BufWriter<T>) -> Outcome {
    generate_localnet_with_line(opts, writer)
}

#[allow(clippy::too_many_lines)]
fn validate_localnet_options(opts: &LocalnetOptions) -> Result<ResolvedHosts> {
    if let Some(block_ms) = opts.block_time_ms
        && block_ms == 0
    {
        return Err(eyre!("`--block-time-ms` must be greater than zero"));
    }
    if let Some(commit_ms) = opts.commit_time_ms
        && commit_ms == 0
    {
        return Err(eyre!("`--commit-time-ms` must be greater than zero"));
    }
    match (opts.next_consensus_mode, opts.mode_activation_height) {
        (Some(_), None) => {
            return Err(eyre!(
                "`--next-consensus-mode` requires `--mode-activation-height`"
            ));
        }
        (None, Some(_)) => {
            return Err(eyre!(
                "`--mode-activation-height` requires `--next-consensus-mode`"
            ));
        }
        _ => {}
    }
    if let (Some(block_ms), Some(commit_ms)) = (opts.block_time_ms, opts.commit_time_ms)
        && commit_ms < block_ms
    {
        return Err(eyre!(
            "`--commit-time-ms` ({commit_ms}) must be greater than or equal to `--block-time-ms` ({block_ms})"
        ));
    }
    if let Some(r) = opts.redundant_send_r
        && r == 0
    {
        return Err(eyre!("`--redundant-send-r` must be at least 1"));
    }
    if let Some(height) = opts.mode_activation_height
        && height == 0
    {
        return Err(eyre!(
            "`--mode-activation-height` must be greater than zero"
        ));
    }
    if opts.sora_profile.is_some() {
        if !opts.build_line.is_iroha3() {
            return Err(eyre!("`--sora-profile` requires `--build-line iroha3`"));
        }
        if opts.peers.get() < LOCALNET_SORA_MIN_PEERS {
            return Err(eyre!(
                "`--sora-profile` requires at least {LOCALNET_SORA_MIN_PEERS} peers"
            ));
        }
    }
    if let Some(perf_spec) = opts.perf_profile.map(LocalnetPerfProfile::spec) {
        if !opts.build_line.is_iroha3() {
            return Err(eyre!(
                "`--perf-profile` requires `--build-line iroha3` for 1s finality presets"
            ));
        }
        if opts.peers.get() < LOCALNET_SORA_MIN_PEERS {
            return Err(eyre!(
                "`--perf-profile` requires at least {LOCALNET_SORA_MIN_PEERS} peers"
            ));
        }
        if perf_spec.collectors_k > opts.peers.get() {
            return Err(eyre!(
                "`--perf-profile` collectors_k ({}) exceeds peer count ({})",
                perf_spec.collectors_k,
                opts.peers.get()
            ));
        }
        if opts.consensus_mode != perf_spec.consensus_mode {
            return Err(eyre!(
                "`--perf-profile` {:?} requires `--consensus-mode {}`",
                opts.perf_profile.expect("perf profile present"),
                match perf_spec.consensus_mode {
                    SumeragiConsensusMode::Permissioned => "permissioned",
                    SumeragiConsensusMode::Npos => "npos",
                }
            ));
        }
        if opts.sora_profile.is_some() && perf_spec.consensus_mode != SumeragiConsensusMode::Npos {
            return Err(eyre!(
                "`--perf-profile` permissioned preset cannot be combined with `--sora-profile`"
            ));
        }
    }
    if opts.sora_profile.is_some() && opts.consensus_mode != SumeragiConsensusMode::Npos {
        return Err(eyre!(
            "`--sora-profile` localnets require `--consensus-mode npos` because the global merge ledger is NPoS; use permissioned mode without `--sora-profile`"
        ));
    }
    let consensus_policy = opts
        .sora_profile
        .map_or(ConsensusPolicy::Any, SoraProfile::consensus_policy);
    validate_consensus_mode_for_line(
        opts.build_line,
        opts.consensus_mode,
        opts.next_consensus_mode,
        consensus_policy,
    )?;

    let bind = CanonicalHost::parse(&opts.bind_host, "--bind-host")?;
    let public = CanonicalHost::parse(&opts.public_host, "--public-host")?;

    Ok(ResolvedHosts { bind, public })
}

fn localnet_uses_npos(
    consensus_mode: SumeragiConsensusMode,
    next_consensus_mode: Option<SumeragiConsensusMode>,
) -> bool {
    matches!(consensus_mode, SumeragiConsensusMode::Npos)
        || matches!(next_consensus_mode, Some(SumeragiConsensusMode::Npos))
}

fn localnet_should_enable_nexus(sora_profile: Option<SoraProfile>, npos_bootstrap: bool) -> bool {
    sora_profile.is_some() || npos_bootstrap
}

#[derive(Debug, Clone, Copy)]
struct LocalnetTxGossipOverrides {
    period_ms: u64,
    resend_ticks: u32,
}

fn localnet_gas_account_id(genesis_public_key: &iroha_crypto::PublicKey) -> AccountId {
    let gas_key_pair = iroha_crypto::KeyPair::from_seed(
        genesis_public_key
            .to_string()
            .bytes()
            .chain(LOCALNET_GAS_ACCOUNT_SEED.iter().copied())
            .collect(),
        iroha_crypto::Algorithm::default(),
    );
    AccountId::new(gas_key_pair.public_key().clone())
}

fn account_id_raw_string(account_id: &AccountId) -> String {
    account_id.to_string()
}

fn account_id_runtime_literal(account_id: &AccountId) -> String {
    account_id_raw_string(account_id)
}

#[allow(clippy::too_many_lines)]
fn generate_localnet_with_line<T: Write>(
    opts: &LocalnetOptions,
    writer: &mut BufWriter<T>,
) -> Outcome {
    init_instruction_registry();
    let build_line = opts.build_line;
    let hosts = validate_localnet_options(opts)?;
    validate_port_ranges(opts.peers, opts.base_api_port, opts.base_p2p_port)?;
    fs::create_dir_all(&opts.out_dir).wrap_err("failed to create output directory for localnet")?;
    let out_dir = fs::canonicalize(&opts.out_dir).wrap_err_with(|| {
        format!(
            "failed to canonicalize output directory for localnet: {}",
            opts.out_dir.display()
        )
    })?;

    let seed_bytes = opts.seed.as_ref().map(String::as_bytes);
    let peers = build_peers(
        opts.peers.get(),
        seed_bytes,
        opts.base_api_port,
        opts.base_p2p_port,
    );

    tui::status("Generating genesis manifest");
    let da_rbc_enabled = build_line.is_iroha3();
    let npos_bootstrap = localnet_uses_npos(opts.consensus_mode, opts.next_consensus_mode);
    let perf_spec = opts.perf_profile.map(LocalnetPerfProfile::spec);
    let queue_capacity = if perf_spec.is_some() {
        LOCALNET_PERF_QUEUE_CAPACITY
    } else {
        LOCALNET_QUEUE_CAPACITY
    };
    let logger_filter = perf_spec.map(|_| LOCALNET_PERF_LOGGER_FILTER);
    let signature_batch_max_ed25519 = perf_spec.map(|_| LOCALNET_SIGNATURE_BATCH_MAX_ED25519);
    // Nexus stays enabled for Sora profiles and whenever NPoS bootstrap is requested.
    let nexus_enabled = localnet_should_enable_nexus(opts.sora_profile, npos_bootstrap);
    let dataspace_fault_tolerance =
        nexus_enabled.then(|| localnet_dataspace_fault_tolerance(opts.peers));
    let block_time_override = opts
        .block_time_ms
        .or_else(|| perf_spec.map(|spec| spec.block_time_ms));
    let commit_time_override = opts
        .commit_time_ms
        .or_else(|| perf_spec.map(|spec| spec.commit_time_ms));
    let (block_time_ms, commit_time_ms) =
        resolve_localnet_pipeline_times(block_time_override, commit_time_override);
    let tx_gossip_overrides = localnet_tx_gossip_overrides(
        block_time_ms.unwrap_or_else(|| SumeragiParameters::default().block_time_ms()),
    );
    let redundant_send_r = resolve_localnet_redundant_send_r(
        opts.redundant_send_r
            .or_else(|| perf_spec.and_then(|spec| spec.redundant_send_r)),
        da_rbc_enabled,
        opts.peers,
    );
    let collectors_k = perf_spec.map(|spec| spec.collectors_k);
    let block_max_transactions = perf_spec.map_or(LOCALNET_BLOCK_MAX_TRANSACTIONS, |spec| {
        spec.block_max_transactions
    });
    let requested_stake_amount = perf_spec.map(|spec| spec.stake_amount);
    let commit_inflight_timeout_ms =
        localnet_commit_inflight_timeout_ms(block_time_ms, commit_time_ms);
    let (genesis_public_key, genesis_private) = generate_genesis_key_pair(seed_bytes, GENESIS_SEED);
    let gas_account_id = if npos_bootstrap {
        Some(localnet_gas_account_id(&genesis_public_key))
    } else {
        None
    };
    let mut genesis = generate_raw_genesis(
        &genesis_public_key,
        opts.consensus_mode,
        opts.next_consensus_mode,
        opts.mode_activation_height,
        build_line,
    )?;
    if opts.extra_accounts > 0 || !opts.assets.is_empty() {
        genesis = extend_genesis(genesis, seed_bytes, opts.extra_accounts, &opts.assets)?;
    }
    genesis = apply_parameter_overrides(
        genesis,
        block_time_ms,
        commit_time_ms,
        redundant_send_r,
        collectors_k,
        block_max_transactions,
        opts.consensus_mode,
        opts.next_consensus_mode,
    );
    genesis = append_peer_pop(genesis, &peers);
    if npos_bootstrap {
        let gas_account_id = gas_account_id
            .as_ref()
            .expect("gas account id required for NPoS bootstrap");
        let stake_amount =
            localnet_npos_stake_amount(&genesis.effective_parameters(), requested_stake_amount);
        genesis = append_localnet_npos_bootstrap(genesis, &peers, gas_account_id, stake_amount)?;
    }
    genesis = apply_localnet_crypto_overrides(genesis, npos_bootstrap);
    let genesis_json_path = out_dir.join("genesis.json");
    let genesis_signed_path = out_dir.join("genesis.signed.nrt");
    let genesis = genesis
        .with_consensus_mode(opts.consensus_mode)
        .with_consensus_meta();
    write_genesis(
        &genesis,
        &genesis_public_key,
        genesis_private.clone(),
        &genesis_json_path,
        &genesis_signed_path,
    )?;
    tui::success("Genesis ready");

    tui::status("Writing peer configs");
    let gas_account_id = gas_account_id.as_ref().map(account_id_runtime_literal);
    let trusted = peers
        .iter()
        .map(|p| format!("{}@{}", p.public_key, hosts.public.addr_literal(p.p2p_port)))
        .collect::<Vec<_>>();
    let peer_telemetry_urls = peers
        .iter()
        .map(|p| hosts.public.torii_url(p.api_port))
        .collect::<Vec<_>>();
    let bls_entries = peers
        .iter()
        .map(|p| BlsEntry {
            bls_pk: p.bls_public_key.to_string(),
            pop_hex: format!("0x{}", hex::encode(&p.bls_pop)),
        })
        .collect::<Vec<_>>();
    for (idx, peer) in peers.iter().enumerate() {
        let kura_dir = out_dir.join("storage").join(format!("peer{idx}"));
        fs::create_dir_all(&kura_dir)
            .wrap_err_with(|| format!("failed to create kura dir {}", kura_dir.display()))?;
        let tiered_state_dir = kura_dir.join("tiered_state");
        fs::create_dir_all(&tiered_state_dir).wrap_err_with(|| {
            format!(
                "failed to create tiered state dir {}",
                tiered_state_dir.display()
            )
        })?;
        let da_store_dir = kura_dir.join("da_wsv_snapshots");
        fs::create_dir_all(&da_store_dir).wrap_err_with(|| {
            format!(
                "failed to create DA WSV snapshot dir {}",
                da_store_dir.display()
            )
        })?;
        let rendered = render_peer_config(
            peer,
            &trusted,
            &peer_telemetry_urls,
            &genesis_public_key,
            &genesis_signed_path,
            &bls_entries,
            &kura_dir,
            &tiered_state_dir,
            &da_store_dir,
            (&hosts.bind, &hosts.public),
            opts.consensus_mode,
            nexus_enabled,
            npos_bootstrap,
            dataspace_fault_tolerance,
            gas_account_id.as_deref(),
            da_rbc_enabled,
            redundant_send_r,
            collectors_k,
            commit_inflight_timeout_ms,
            tx_gossip_overrides,
            logger_filter,
            signature_batch_max_ed25519,
            queue_capacity,
        );
        let path = out_dir.join(format!("peer{idx}.toml"));
        fs::write(&path, rendered)
            .wrap_err_with(|| format!("failed to write {}", path.display()))?;
    }
    tui::success("Peer configs written");

    tui::status("Writing start/stop scripts");
    write_scripts(&out_dir, opts.peers.get(), build_line, nexus_enabled)?;

    tui::status("Copying rANS tables");
    copy_rans_tables(&out_dir)?;

    tui::status("Writing client config");
    write_client_config(&out_dir, opts.base_api_port, &hosts.public)?;
    tui::success("Localnet ready");

    writeln!(
        writer,
        "Localnet generated in {} (start with start.sh, stop with stop.sh)",
        out_dir.display()
    )?;
    Ok(())
}

fn resolve_localnet_pipeline_times(
    block_time_ms: Option<u64>,
    commit_time_ms: Option<u64>,
) -> (Option<u64>, Option<u64>) {
    match (block_time_ms, commit_time_ms) {
        (None, None) => {
            let (block_ms, commit_ms) = default_localnet_pipeline_times();
            (Some(block_ms), Some(commit_ms))
        }
        (Some(block_ms), None) => (Some(block_ms), Some(block_ms)),
        (None, Some(commit_ms)) => (Some(commit_ms), Some(commit_ms)),
        (Some(block_ms), Some(commit_ms)) => (Some(block_ms), Some(commit_ms)),
    }
}

fn default_localnet_pipeline_times() -> (u64, u64) {
    let total_ms = LOCALNET_PIPELINE_TIME_MS;
    let mut block_ms = total_ms / 3;
    if block_ms == 0 {
        block_ms = 1;
    }
    if block_ms >= total_ms {
        block_ms = total_ms.saturating_sub(1);
    }
    let mut commit_ms = total_ms.saturating_sub(block_ms);
    if commit_ms == 0 {
        commit_ms = 1;
        if block_ms > 1 {
            block_ms = block_ms.saturating_sub(1);
        }
    }
    (block_ms, commit_ms)
}

fn localnet_tx_gossip_overrides(block_time_ms: u64) -> Option<LocalnetTxGossipOverrides> {
    if block_time_ms > LOCALNET_PIPELINE_TIME_MS {
        return None;
    }
    Some(LocalnetTxGossipOverrides {
        period_ms: LOCALNET_TX_GOSSIP_PERIOD_FAST_MS,
        resend_ticks: LOCALNET_TX_GOSSIP_RESEND_TICKS_FAST,
    })
}

fn localnet_commit_inflight_timeout_ms(
    block_time_ms: Option<u64>,
    commit_time_ms: Option<u64>,
) -> u64 {
    let defaults = SumeragiParameters::default();
    let (block_time_ms, commit_time_ms) =
        resolve_localnet_pipeline_times(block_time_ms, commit_time_ms);
    let block_ms = block_time_ms.unwrap_or_else(|| defaults.block_time_ms());
    let commit_ms = commit_time_ms.unwrap_or_else(|| defaults.commit_time_ms());
    let total_ms = block_ms.saturating_add(commit_ms);
    let scaled = total_ms.saturating_mul(LOCALNET_COMMIT_INFLIGHT_TIMEOUT_MULTIPLIER);
    let capped = scaled.min(LOCALNET_COMMIT_INFLIGHT_TIMEOUT_MAX_MS);
    capped.max(LOCALNET_COMMIT_INFLIGHT_TIMEOUT_MIN_MS)
}

fn resolve_localnet_redundant_send_r(
    redundant_send_r: Option<u8>,
    da_rbc_enabled: bool,
    peer_count: NonZeroU16,
) -> Option<u8> {
    if redundant_send_r.is_none() && da_rbc_enabled {
        let peers = usize::from(peer_count.get());
        return Some(redundant_send_r_from_len(peers));
    }
    redundant_send_r
}

fn build_peers(count: u16, seed: Option<&[u8]>, base_api: u16, base_p2p: u16) -> Vec<Peer> {
    (0..count)
        .map(|nth| {
            let (bls_public, bls_secret, pop) = generate_bls_key_pair(seed, &nth.to_be_bytes());
            Peer {
                public_key: bls_public.clone(),
                private_key: bls_secret,
                bls_public_key: bls_public,
                bls_pop: pop,
                api_port: base_api + nth,
                p2p_port: base_p2p + nth,
            }
        })
        .collect()
}

fn validate_port_ranges(peers: NonZeroU16, base_api_port: u16, base_p2p_port: u16) -> Result<()> {
    if base_api_port == 0 {
        return Err(eyre!("base_api_port must be > 0"));
    }
    if base_p2p_port == 0 {
        return Err(eyre!("base_p2p_port must be > 0"));
    }

    let max_offset = u32::from(peers.get() - 1);
    let api_start = u32::from(base_api_port);
    let p2p_start = u32::from(base_p2p_port);
    let api_max = api_start + max_offset;
    if api_max > u32::from(u16::MAX) {
        return Err(eyre!(
            "base_api_port {} with {} peers exceeds u16 range",
            base_api_port,
            peers
        ));
    }
    let p2p_max = p2p_start + max_offset;
    if p2p_max > u32::from(u16::MAX) {
        return Err(eyre!(
            "base_p2p_port {} with {} peers exceeds u16 range",
            base_p2p_port,
            peers
        ));
    }

    let ranges_overlap = api_start <= p2p_max && p2p_start <= api_max;
    if ranges_overlap {
        return Err(eyre!(
            "base_api_port {} and base_p2p_port {} overlap for {} peers",
            base_api_port,
            base_p2p_port,
            peers
        ));
    }

    Ok(())
}

fn localnet_dataspace_catalog(fault_tolerance: u32) -> Vec<toml::Value> {
    use toml::{Table, Value};

    let fault_tolerance = i64::from(fault_tolerance);
    let mut catalog = Vec::new();
    for (alias, id, description) in [
        ("universal", 0_i64, "Single-lane data space"),
        ("governance", 1_i64, "Governance proposals & manifests"),
        ("zk", 2_i64, "Zero-knowledge proofs and attachments"),
    ] {
        let mut entry = Table::new();
        entry.insert("alias".into(), Value::String(alias.to_owned()));
        entry.insert("id".into(), Value::Integer(id));
        entry.insert("description".into(), Value::String(description.to_owned()));
        entry.insert("fault_tolerance".into(), Value::Integer(fault_tolerance));
        catalog.push(Value::Table(entry));
    }

    catalog
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn render_peer_config(
    peer: &Peer,
    trusted_peers: &[String],
    peer_telemetry_urls: &[String],
    genesis_public_key: &iroha_crypto::PublicKey,
    genesis_signed_path: &Path,
    bls_entries: &[BlsEntry],
    kura_store_dir: &Path,
    tiered_state_root: &Path,
    da_store_root: &Path,
    hosts: (&CanonicalHost, &CanonicalHost),
    consensus_mode: SumeragiConsensusMode,
    nexus_enabled: bool,
    npos_bootstrap: bool,
    dataspace_fault_tolerance: Option<u32>,
    gas_account_id: Option<&str>,
    da_rbc_enabled: bool,
    redundant_send_r: Option<u8>,
    collectors_k: Option<u16>,
    commit_inflight_timeout_ms: u64,
    tx_gossip_overrides: Option<LocalnetTxGossipOverrides>,
    logger_filter: Option<&str>,
    signature_batch_max_ed25519: Option<usize>,
    queue_capacity: usize,
) -> String {
    use toml::{Table, Value};

    let (bind_host, public_host) = hosts;

    let trusted_list = trusted_peers
        .iter()
        .cloned()
        .map(Value::String)
        .collect::<Vec<_>>();

    let pops = bls_entries
        .iter()
        .map(|entry| {
            let mut t = Table::new();
            t.insert("public_key".into(), Value::String(entry.bls_pk.clone()));
            t.insert(
                "pop_hex".into(),
                Value::String(entry.pop_hex.trim_start_matches("0x").to_owned()),
            );
            Value::Table(t)
        })
        .collect::<Vec<_>>();

    let mut root = Table::new();
    root.insert("chain".into(), Value::String(CHAIN_ID.to_owned()));
    root.insert(
        "private_key".into(),
        Value::String(peer.private_key.to_string()),
    );
    root.insert(
        "public_key".into(),
        Value::String(peer.public_key.to_string()),
    );
    root.insert("trusted_peers".into(), Value::Array(trusted_list));
    root.insert("trusted_peers_pop".into(), Value::Array(pops));
    root.insert(
        "telemetry_enabled".into(),
        Value::Boolean(LOCALNET_TELEMETRY_ENABLED),
    );
    root.insert(
        "telemetry_profile".into(),
        Value::String(LOCALNET_TELEMETRY_PROFILE.to_owned()),
    );

    let mut kura = Table::new();
    kura.insert(
        "store_dir".into(),
        Value::String(kura_store_dir.to_string_lossy().into_owned()),
    );
    kura.insert(
        "fsync_mode".into(),
        Value::String(LOCALNET_KURA_FSYNC_MODE.to_owned()),
    );
    root.insert("kura".into(), Value::Table(kura));

    let mut tiered_state = Table::new();
    tiered_state.insert(
        "cold_store_root".into(),
        Value::String(tiered_state_root.to_string_lossy().into_owned()),
    );
    if da_rbc_enabled {
        tiered_state.insert(
            "da_store_root".into(),
            Value::String(da_store_root.to_string_lossy().into_owned()),
        );
    }
    root.insert("tiered_state".into(), Value::Table(tiered_state));

    let consensus_mode_str = match consensus_mode {
        SumeragiConsensusMode::Permissioned => "permissioned",
        SumeragiConsensusMode::Npos => "npos",
    };

    let mut sumeragi = Table::new();
    sumeragi.insert(
        "consensus_mode".into(),
        Value::String(consensus_mode_str.to_owned()),
    );
    // Align runtime toggles with the build line: iroha3 requires DA (RBC + availability tracking);
    // other lines keep the generator defaults (currently disabled for iroha2, but can be flipped
    // manually if desired).
    let mut da = Table::new();
    da.insert("enabled".into(), Value::Boolean(da_rbc_enabled));
    sumeragi.insert("da".into(), Value::Table(da));

    let mut advanced = Table::new();
    let mut pacing_governor = Table::new();
    pacing_governor.insert(
        "min_factor_bps".into(),
        Value::Integer(i64::from(LOCALNET_PACING_GOVERNOR_MIN_FACTOR_BPS)),
    );
    pacing_governor.insert(
        "max_factor_bps".into(),
        Value::Integer(i64::from(LOCALNET_PACING_GOVERNOR_MAX_FACTOR_BPS)),
    );
    advanced.insert("pacing_governor".into(), Value::Table(pacing_governor));
    let mut da_advanced = Table::new();
    da_advanced.insert(
        "quorum_timeout_multiplier".into(),
        Value::Integer(i64::from(LOCALNET_DA_QUORUM_TIMEOUT_MULTIPLIER)),
    );
    da_advanced.insert(
        "availability_timeout_multiplier".into(),
        Value::Integer(i64::from(LOCALNET_DA_AVAILABILITY_TIMEOUT_MULTIPLIER)),
    );
    da_advanced.insert(
        "availability_timeout_floor_ms".into(),
        Value::Integer(
            i64::try_from(LOCALNET_DA_AVAILABILITY_TIMEOUT_FLOOR_MS)
                .expect("LOCALNET_DA_AVAILABILITY_TIMEOUT_FLOOR_MS fits i64"),
        ),
    );
    advanced.insert("da".into(), Value::Table(da_advanced));

    let mut rbc = Table::new();
    rbc.insert(
        "chunk_max_bytes".into(),
        Value::Integer(
            i64::try_from(LOCALNET_RBC_CHUNK_MAX_BYTES)
                .expect("LOCALNET_RBC_CHUNK_MAX_BYTES fits i64"),
        ),
    );
    rbc.insert(
        "payload_chunks_per_tick".into(),
        Value::Integer(
            i64::try_from(LOCALNET_RBC_PAYLOAD_CHUNKS_PER_TICK)
                .expect("LOCALNET_RBC_PAYLOAD_CHUNKS_PER_TICK fits i64"),
        ),
    );
    advanced.insert("rbc".into(), Value::Table(rbc));

    let msg_channel_cap_votes = i64::try_from(LOCALNET_MSG_CHANNEL_CAP_VOTES)
        .expect("LOCALNET_MSG_CHANNEL_CAP_VOTES fits i64");
    let msg_channel_cap_block_payload = i64::try_from(LOCALNET_MSG_CHANNEL_CAP_BLOCK_PAYLOAD)
        .expect("LOCALNET_MSG_CHANNEL_CAP_BLOCK_PAYLOAD fits i64");
    let msg_channel_cap_rbc_chunks = i64::try_from(LOCALNET_MSG_CHANNEL_CAP_RBC_CHUNKS)
        .expect("LOCALNET_MSG_CHANNEL_CAP_RBC_CHUNKS fits i64");
    let msg_channel_cap_blocks = i64::try_from(LOCALNET_MSG_CHANNEL_CAP_BLOCKS)
        .expect("LOCALNET_MSG_CHANNEL_CAP_BLOCKS fits i64");
    let control_msg_channel_cap = i64::try_from(LOCALNET_CONTROL_MSG_CHANNEL_CAP)
        .expect("LOCALNET_CONTROL_MSG_CHANNEL_CAP fits i64");
    let mut queues = Table::new();
    queues.insert("votes".into(), Value::Integer(msg_channel_cap_votes));
    queues.insert(
        "block_payload".into(),
        Value::Integer(msg_channel_cap_block_payload),
    );
    queues.insert(
        "rbc_chunks".into(),
        Value::Integer(msg_channel_cap_rbc_chunks),
    );
    queues.insert("blocks".into(), Value::Integer(msg_channel_cap_blocks));
    queues.insert("control".into(), Value::Integer(control_msg_channel_cap));
    advanced.insert("queues".into(), Value::Table(queues));
    sumeragi.insert("advanced".into(), Value::Table(advanced));

    let mut persistence = Table::new();
    persistence.insert(
        "commit_inflight_timeout_ms".into(),
        Value::Integer(
            i64::try_from(commit_inflight_timeout_ms).expect("commit_inflight_timeout_ms fits i64"),
        ),
    );
    sumeragi.insert("persistence".into(), Value::Table(persistence));

    let mut nexus = Table::new();
    nexus.insert("enabled".into(), Value::Boolean(nexus_enabled));
    let mut fusion = Table::new();
    fusion.insert(
        "exit_teu".into(),
        Value::Integer(i64::from(LOCALNET_LANE_TEU_CAPACITY)),
    );
    nexus.insert("fusion".into(), Value::Table(fusion));
    if npos_bootstrap {
        let gas_account_id = gas_account_id.expect("localnet gas account id required");
        let stake_asset_id = localnet_stake_asset_literal();
        let mut staking = Table::new();
        staking.insert(
            "stake_asset_id".into(),
            Value::String(stake_asset_id.clone()),
        );
        staking.insert(
            "stake_escrow_account_id".into(),
            Value::String(gas_account_id.to_owned()),
        );
        staking.insert(
            "slash_sink_account_id".into(),
            Value::String(gas_account_id.to_owned()),
        );
        nexus.insert("staking".into(), Value::Table(staking));

        let mut fees = Table::new();
        fees.insert("fee_asset_id".into(), Value::String(stake_asset_id));
        fees.insert(
            "fee_sink_account_id".into(),
            Value::String(gas_account_id.to_owned()),
        );
        nexus.insert("fees".into(), Value::Table(fees));
    }
    if let Some(fault_tolerance) = dataspace_fault_tolerance {
        let catalog = localnet_dataspace_catalog(fault_tolerance);
        nexus.insert("dataspace_catalog".into(), Value::Array(catalog));
    }
    root.insert("nexus".into(), Value::Table(nexus));

    let mut collectors = Table::new();
    if let Some(collectors_k) = collectors_k {
        collectors.insert("k".into(), Value::Integer(i64::from(collectors_k)));
    }
    if let Some(redundant_send_r) = redundant_send_r {
        collectors.insert(
            "redundant_send_r".into(),
            Value::Integer(i64::from(redundant_send_r)),
        );
    }
    if !collectors.is_empty() {
        sumeragi.insert("collectors".into(), Value::Table(collectors));
    }

    let mut block = Table::new();
    block.insert(
        "proposal_queue_scan_multiplier".into(),
        Value::Integer(
            i64::try_from(LOCALNET_PROPOSAL_QUEUE_SCAN_MULTIPLIER)
                .expect("LOCALNET_PROPOSAL_QUEUE_SCAN_MULTIPLIER fits i64"),
        ),
    );
    sumeragi.insert("block".into(), Value::Table(block));
    root.insert("sumeragi".into(), Value::Table(sumeragi));

    let mut pipeline = Table::new();
    if let Some(batch_max) = signature_batch_max_ed25519 {
        pipeline.insert(
            "signature_batch_max_ed25519".into(),
            Value::Integer(i64::try_from(batch_max).expect("batch size fits i64")),
        );
    }
    pipeline.insert("signature_batch_max_bls".into(), Value::Integer(4i64));
    if let Some(gas_account_id) = gas_account_id {
        let mut gas = Table::new();
        gas.insert(
            "tech_account_id".into(),
            Value::String(gas_account_id.to_owned()),
        );
        pipeline.insert("gas".into(), Value::Table(gas));
    }
    root.insert("pipeline".into(), Value::Table(pipeline));

    let mut queue = Table::new();
    queue.insert(
        "capacity".into(),
        Value::Integer(i64::try_from(queue_capacity).expect("queue capacity fits i64")),
    );
    queue.insert(
        "capacity_per_user".into(),
        Value::Integer(i64::try_from(queue_capacity).expect("queue capacity fits i64")),
    );
    queue.insert(
        "transaction_time_to_live_ms".into(),
        Value::Integer(i64::try_from(LOCALNET_QUEUE_TTL_MS).expect("queue ttl fits i64")),
    );
    root.insert("queue".into(), Value::Table(queue));

    if npos_bootstrap {
        let mut crypto = Table::new();
        let allowed_signing = [
            iroha_crypto::Algorithm::Ed25519,
            iroha_crypto::Algorithm::Secp256k1,
            iroha_crypto::Algorithm::BlsNormal,
        ];
        crypto.insert(
            "allowed_signing".into(),
            Value::Array(
                allowed_signing
                    .iter()
                    .map(|algo| Value::String(algo.as_static_str().to_owned()))
                    .collect(),
            ),
        );
        let mut curves = Table::new();
        let mut curve_ids = allowed_signing
            .iter()
            .filter_map(|algo| {
                iroha_data_model::account::curve::CurveId::try_from_algorithm(*algo).ok()
            })
            .map(|curve| i64::from(curve.as_u8()))
            .collect::<Vec<_>>();
        curve_ids.sort_unstable();
        curve_ids.dedup();
        curves.insert(
            "allowed_curve_ids".into(),
            Value::Array(curve_ids.into_iter().map(Value::Integer).collect()),
        );
        crypto.insert("curves".into(), Value::Table(curves));
        root.insert("crypto".into(), Value::Table(crypto));
    }

    let mut streaming = Table::new();
    streaming.insert(
        "identity_public_key".into(),
        Value::String(STREAM_ID_PUBLIC.to_owned()),
    );
    streaming.insert(
        "identity_private_key".into(),
        Value::String(STREAM_ID_PRIVATE.to_owned()),
    );
    root.insert("streaming".into(), Value::Table(streaming));

    let mut confidential = Table::new();
    confidential.insert("enabled".into(), Value::Boolean(true));
    confidential.insert("assume_valid".into(), Value::Boolean(false));
    root.insert("confidential".into(), Value::Table(confidential));

    let mut genesis = Table::new();
    genesis.insert(
        "file".into(),
        Value::String(genesis_signed_path.to_string_lossy().into_owned()),
    );
    genesis.insert(
        "public_key".into(),
        Value::String(genesis_public_key.to_string()),
    );
    root.insert("genesis".into(), Value::Table(genesis));

    let mut logger = Table::new();
    logger.insert("format".into(), Value::String("compact".into()));
    logger.insert("level".into(), Value::String("info".into()));
    if let Some(filter) = logger_filter {
        logger.insert("filter".into(), Value::String(filter.to_owned()));
    }
    root.insert("logger".into(), Value::Table(logger));

    let mut network = Table::new();
    network.insert(
        "address".into(),
        Value::String(bind_host.addr_literal(peer.p2p_port)),
    );
    network.insert(
        "public_address".into(),
        Value::String(public_host.addr_literal(peer.p2p_port)),
    );
    network.insert(
        "p2p_subscriber_queue_cap".into(),
        Value::Integer(
            i64::try_from(LOCALNET_P2P_SUBSCRIBER_QUEUE_CAP)
                .expect("LOCALNET_P2P_SUBSCRIBER_QUEUE_CAP fits i64"),
        ),
    );
    network.insert(
        "connect_startup_delay_ms".into(),
        Value::Integer(
            i64::try_from(LOCALNET_CONNECT_STARTUP_DELAY_MS)
                .expect("LOCALNET_CONNECT_STARTUP_DELAY_MS fits i64"),
        ),
    );
    network.insert(
        "consensus_ingress_rate_per_sec".into(),
        Value::Integer(i64::from(LOCALNET_CONSENSUS_INGRESS_RATE_PER_SEC)),
    );
    network.insert(
        "consensus_ingress_burst".into(),
        Value::Integer(i64::from(LOCALNET_CONSENSUS_INGRESS_BURST)),
    );
    network.insert(
        "consensus_ingress_bytes_per_sec".into(),
        Value::Integer(i64::from(LOCALNET_CONSENSUS_INGRESS_BYTES_PER_SEC)),
    );
    network.insert(
        "consensus_ingress_bytes_burst".into(),
        Value::Integer(i64::from(LOCALNET_CONSENSUS_INGRESS_BYTES_BURST)),
    );
    network.insert(
        "consensus_ingress_critical_rate_per_sec".into(),
        Value::Integer(i64::from(LOCALNET_CONSENSUS_INGRESS_CRITICAL_RATE_PER_SEC)),
    );
    network.insert(
        "consensus_ingress_critical_burst".into(),
        Value::Integer(i64::from(LOCALNET_CONSENSUS_INGRESS_CRITICAL_BURST)),
    );
    network.insert(
        "consensus_ingress_critical_bytes_per_sec".into(),
        Value::Integer(i64::from(LOCALNET_CONSENSUS_INGRESS_CRITICAL_BYTES_PER_SEC)),
    );
    network.insert(
        "consensus_ingress_critical_bytes_burst".into(),
        Value::Integer(i64::from(LOCALNET_CONSENSUS_INGRESS_CRITICAL_BYTES_BURST)),
    );
    if let Some(overrides) = tx_gossip_overrides {
        network.insert(
            "transaction_gossip_period_ms".into(),
            Value::Integer(
                i64::try_from(overrides.period_ms)
                    .expect("LOCALNET_TX_GOSSIP_PERIOD_FAST_MS fits i64"),
            ),
        );
        network.insert(
            "transaction_gossip_resend_ticks".into(),
            Value::Integer(i64::from(overrides.resend_ticks)),
        );
        network.insert(
            "transaction_gossip_public_target_reshuffle_ms".into(),
            Value::Integer(
                i64::try_from(overrides.period_ms)
                    .expect("LOCALNET_TX_GOSSIP_PERIOD_FAST_MS fits i64"),
            ),
        );
        network.insert(
            "transaction_gossip_restricted_target_reshuffle_ms".into(),
            Value::Integer(
                i64::try_from(overrides.period_ms)
                    .expect("LOCALNET_TX_GOSSIP_PERIOD_FAST_MS fits i64"),
            ),
        );
    }
    root.insert("network".into(), Value::Table(network));

    let mut torii = Table::new();
    torii.insert(
        "address".into(),
        Value::String(bind_host.addr_literal(peer.api_port)),
    );
    torii.insert(
        "peer_telemetry_urls".into(),
        Value::Array(
            peer_telemetry_urls
                .iter()
                .cloned()
                .map(Value::String)
                .collect::<Vec<_>>(),
        ),
    );
    torii.insert(
        "preauth_allow_cidrs".into(),
        Value::Array(
            LOCALNET_PREAUTH_ALLOW_CIDRS
                .iter()
                .map(|cidr| Value::String((*cidr).to_string()))
                .collect::<Vec<_>>(),
        ),
    );
    torii.insert(
        "preauth_rate_per_ip_per_sec".into(),
        Value::Integer(i64::from(LOCALNET_TORII_PREAUTH_RATE_PER_IP_PER_SEC)),
    );
    torii.insert(
        "preauth_burst_per_ip".into(),
        Value::Integer(i64::from(LOCALNET_TORII_PREAUTH_BURST_PER_IP)),
    );
    torii.insert(
        "api_allow_cidrs".into(),
        Value::Array(
            LOCALNET_PREAUTH_ALLOW_CIDRS
                .iter()
                .map(|cidr| Value::String((*cidr).to_string()))
                .collect::<Vec<_>>(),
        ),
    );
    torii.insert(
        "tx_rate_per_authority_per_sec".into(),
        Value::Integer(i64::from(LOCALNET_TORII_TX_RATE_PER_AUTHORITY_PER_SEC)),
    );
    torii.insert(
        "tx_burst_per_authority".into(),
        Value::Integer(i64::from(LOCALNET_TORII_TX_BURST_PER_AUTHORITY)),
    );
    torii.insert(
        "api_high_load_tx_threshold".into(),
        Value::Integer(i64::try_from(queue_capacity).expect("queue capacity fits i64")),
    );
    // torii.transport.norito_rpc
    let mut norito_rpc = Table::new();
    norito_rpc.insert("enabled".into(), Value::Boolean(true));
    norito_rpc.insert("require_mtls".into(), Value::Boolean(false));
    norito_rpc.insert("stage".into(), Value::String("ga".into()));
    norito_rpc.insert(
        "allowed_clients".into(),
        Value::Array(vec![Value::String("*".into())]),
    );
    let mut transport = Table::new();
    transport.insert("norito_rpc".into(), Value::Table(norito_rpc));
    torii.insert("transport".into(), Value::Table(transport));
    root.insert("torii".into(), Value::Table(torii));

    toml::to_string(&Value::Table(root)).expect("serializing peer config to TOML")
}

fn generate_raw_genesis(
    genesis_public_key: &iroha_crypto::PublicKey,
    consensus_mode: SumeragiConsensusMode,
    next_consensus_mode: Option<SumeragiConsensusMode>,
    mode_activation_height: Option<u64>,
    build_line: BuildLine,
) -> Result<RawGenesisTransaction> {
    let builder = GenesisBuilder::new_without_executor(ChainId::from(CHAIN_ID), PathBuf::from("."));
    generate_default(
        builder,
        genesis_public_key,
        None,
        consensus_mode,
        next_consensus_mode,
        mode_activation_height,
        None,
        None,
        build_line,
    )
}

fn extend_genesis(
    genesis: RawGenesisTransaction,
    seed_bytes: Option<&[u8]>,
    extra_accounts: u16,
    assets: &[AssetSpec],
) -> Result<RawGenesisTransaction> {
    let mut builder = genesis.into_builder().next_transaction();

    for idx in 0..extra_accounts {
        let (pk, _) = generate_account_key_pair(seed_bytes, &format!("acct{idx}").into_bytes());
        let domain_id: DomainId = "wonderland"
            .parse()
            .expect("default genesis must include wonderland domain");
        let account_id = ScopedAccountId::new(domain_id, pk.clone());
        builder = builder.append_instruction(Register::account(Account::new(account_id)));
    }

    for asset in assets {
        let asset_def: AssetDefinitionId = asset.id.parse().wrap_err("invalid asset id")?;
        let definition = AssetDefinition::new(asset_def.clone(), NumericSpec::default())
            .with_name(asset_def.to_string())
            .with_metadata(Metadata::default());
        builder = builder.append_instruction(Register::asset_definition(definition));
        if asset.quantity > 0 {
            builder = builder.append_instruction(Mint::asset_numeric(
                asset.quantity,
                AssetId::new(asset_def, asset.mint_to.clone().into()),
            ));
        }
    }

    Ok(builder.build_raw())
}

fn apply_localnet_npos_overrides(
    parameters: &mut Parameters,
    redundant_send_r: Option<u8>,
    collectors_k: Option<u16>,
) {
    let mut npos = parameters
        .custom()
        .get(&SumeragiNposParameters::parameter_id())
        .and_then(SumeragiNposParameters::from_custom_parameter)
        .unwrap_or_default();
    // Override seat band and bond to prevent validator drops on small localnets.
    npos.seat_band_pct = 100;
    npos.min_self_bond = 1;
    if let Some(redundant_send_r) = redundant_send_r {
        npos.redundant_send_r = redundant_send_r;
    }
    if let Some(collectors_k) = collectors_k {
        npos.k_aggregators = collectors_k;
    }

    parameters.set_parameter(Parameter::Custom(npos.into_custom_parameter()));
}

fn localnet_npos_stake_amount(parameters: &Parameters, requested: Option<u64>) -> u64 {
    let requested = requested.unwrap_or(LOCALNET_STAKE_AMOUNT);
    let min_self_bond = parameters
        .custom()
        .get(&SumeragiNposParameters::parameter_id())
        .and_then(SumeragiNposParameters::from_custom_parameter)
        .map_or(requested, |params| params.min_self_bond);
    requested.max(min_self_bond).max(1)
}

#[allow(clippy::too_many_arguments)]
fn apply_parameter_overrides(
    genesis: RawGenesisTransaction,
    block_time_ms: Option<u64>,
    commit_time_ms: Option<u64>,
    redundant_send_r: Option<u8>,
    collectors_k: Option<u16>,
    block_max_transactions: u64,
    consensus_mode: SumeragiConsensusMode,
    next_consensus_mode: Option<SumeragiConsensusMode>,
) -> RawGenesisTransaction {
    let include_npos = matches!(consensus_mode, SumeragiConsensusMode::Npos)
        || matches!(next_consensus_mode, Some(SumeragiConsensusMode::Npos));
    let mut parameters = genesis.effective_parameters();
    let block_max_transactions =
        NonZeroU64::new(block_max_transactions).expect("block_max_transactions must be non-zero");
    let should_update = block_time_ms.is_some()
        || commit_time_ms.is_some()
        || redundant_send_r.is_some()
        || collectors_k.is_some()
        || include_npos
        || parameters.block.max_transactions != block_max_transactions;
    if !should_update {
        return genesis;
    }

    parameters.block.max_transactions = block_max_transactions;
    if let Some(block_time_ms) = block_time_ms {
        parameters.sumeragi.block_time_ms = block_time_ms;
    }
    if let Some(commit_time_ms) = commit_time_ms {
        parameters.sumeragi.commit_time_ms = commit_time_ms;
    }
    if let Some(redundant_send_r) = redundant_send_r {
        parameters.sumeragi.collectors_redundant_send_r = redundant_send_r;
    }
    if let Some(collectors_k) = collectors_k {
        parameters.sumeragi.collectors_k = collectors_k;
    }
    if include_npos {
        apply_localnet_npos_overrides(&mut parameters, redundant_send_r, collectors_k);
    }

    // Use structured parameters so overrides land in the manifest parameters block.
    let mut builder = genesis.into_builder().next_transaction();
    for parameter in parameters.parameters() {
        builder = builder.append_parameter(parameter);
    }
    if let Some(mode) = parameters.sumeragi().next_mode() {
        builder = builder.append_parameter(Parameter::Sumeragi(SumeragiParameter::NextMode(mode)));
    }
    if let Some(height) = parameters.sumeragi().mode_activation_height() {
        builder = builder.append_parameter(Parameter::Sumeragi(
            SumeragiParameter::ModeActivationHeight(height),
        ));
    }

    builder.build_raw()
}

fn apply_localnet_crypto_overrides(
    genesis: RawGenesisTransaction,
    npos_bootstrap: bool,
) -> RawGenesisTransaction {
    if !npos_bootstrap {
        return genesis;
    }

    let mut crypto = genesis.crypto().clone();
    if !crypto
        .allowed_signing
        .iter()
        .any(|algo| matches!(algo, iroha_crypto::Algorithm::BlsNormal))
    {
        crypto
            .allowed_signing
            .push(iroha_crypto::Algorithm::BlsNormal);
    }
    crypto.allowed_signing.sort();
    crypto.allowed_signing.dedup();
    crypto.allowed_curve_ids = crypto
        .allowed_signing
        .iter()
        .filter_map(|algo| {
            iroha_data_model::account::curve::CurveId::try_from_algorithm(*algo).ok()
        })
        .map(iroha_data_model::account::curve::CurveId::as_u8)
        .collect();
    crypto.allowed_curve_ids.sort_unstable();
    crypto.allowed_curve_ids.dedup();

    genesis.into_builder().with_crypto(crypto).build_raw()
}

fn append_peer_pop(genesis: RawGenesisTransaction, peers: &[Peer]) -> RawGenesisTransaction {
    genesis
        .into_builder()
        .next_transaction()
        .set_topology(
            peers
                .iter()
                .map(|peer| {
                    GenesisTopologyEntry::new(
                        PeerId::new(peer.public_key.clone()),
                        peer.bls_pop.clone(),
                    )
                })
                .collect(),
        )
        .build_raw()
}

struct BootstrapRegistrations {
    domains: BTreeSet<DomainId>,
    accounts: BTreeSet<AccountId>,
    asset_defs: BTreeSet<AssetDefinitionId>,
}

impl BootstrapRegistrations {
    fn from_manifest(manifest: &RawGenesisTransaction) -> Self {
        let mut domains = BTreeSet::new();
        let mut accounts = BTreeSet::new();
        let mut asset_defs = BTreeSet::new();
        for instruction in manifest.instructions() {
            if let Some(register) = instruction.as_any().downcast_ref::<Register<Domain>>() {
                domains.insert(register.object.id.clone());
                continue;
            }
            if let Some(register) = instruction.as_any().downcast_ref::<Register<Account>>() {
                accounts.insert(register.object.id.clone());
                continue;
            }
            if let Some(register) = instruction
                .as_any()
                .downcast_ref::<Register<AssetDefinition>>()
            {
                asset_defs.insert(register.object.id.clone());
            }
        }
        Self {
            domains,
            accounts,
            asset_defs,
        }
    }
}

fn append_localnet_npos_bootstrap(
    genesis: RawGenesisTransaction,
    peers: &[Peer],
    gas_account_id: &AccountId,
    stake_amount: u64,
) -> Result<RawGenesisTransaction> {
    let nexus_domain: DomainId = LOCALNET_NEXUS_DOMAIN.parse()?;
    let ivm_domain: DomainId = LOCALNET_IVM_DOMAIN.parse()?;
    let stake_asset_id = localnet_stake_asset_definition_id();
    let mut registrations = BootstrapRegistrations::from_manifest(&genesis);

    let mut builder = genesis.into_builder().next_transaction();
    if !registrations.domains.contains(&nexus_domain) {
        builder = builder.append_instruction(Register::domain(Domain::new(nexus_domain.clone())));
        registrations.domains.insert(nexus_domain.clone());
    }
    if !registrations.domains.contains(&ivm_domain) {
        builder = builder.append_instruction(Register::domain(Domain::new(ivm_domain.clone())));
        registrations.domains.insert(ivm_domain.clone());
    }
    if !registrations.accounts.contains(gas_account_id) {
        builder = builder.append_instruction(Register::account(Account::new(
            gas_account_id.to_account_id(ivm_domain.clone()),
        )));
        registrations.accounts.insert(gas_account_id.clone());
    }

    if !registrations.asset_defs.contains(&stake_asset_id) {
        let definition = AssetDefinition::new(stake_asset_id.clone(), NumericSpec::default())
            .with_name("Localnet Stake".to_owned())
            .with_metadata(Metadata::default());
        builder = builder.append_instruction(Register::asset_definition(definition));
        registrations.asset_defs.insert(stake_asset_id.clone());
    }

    for peer in peers {
        let validator_id = AccountId::new(peer.public_key.clone());
        if !registrations.accounts.contains(&validator_id) {
            builder = builder.append_instruction(Register::account(Account::new(
                validator_id.to_account_id(nexus_domain.clone()),
            )));
            registrations.accounts.insert(validator_id.clone());
        }
        builder = builder.append_instruction(Mint::asset_numeric(
            stake_amount,
            AssetId::new(stake_asset_id.clone(), validator_id.clone()),
        ));
    }

    let mut builder = builder.next_transaction();
    for peer in peers {
        let validator_id = AccountId::new(peer.public_key.clone());
        builder = builder.append_instruction(RegisterPublicLaneValidator {
            lane_id: LaneId::SINGLE,
            validator: validator_id.clone(),
            stake_account: validator_id.clone(),
            initial_stake: Numeric::from(stake_amount),
            metadata: Metadata::default(),
        });
        builder = builder.append_instruction(ActivatePublicLaneValidator {
            lane_id: LaneId::SINGLE,
            validator: validator_id,
        });
    }

    Ok(builder.build_raw())
}

fn write_genesis(
    genesis: &RawGenesisTransaction,
    genesis_public_key: &iroha_crypto::PublicKey,
    genesis_private_key: ExposedPrivateKey,
    json_path: &Path,
    signed_path: &Path,
) -> Result<()> {
    let json = norito::json::to_json_pretty(genesis)?;
    fs::write(json_path, json).wrap_err("failed to write genesis.json")?;

    let genesis_key_pair = KeyPair::new(genesis_public_key.clone(), genesis_private_key.0)
        .wrap_err("make genesis key pair")?;
    let block = genesis
        .clone()
        .build_and_sign(&genesis_key_pair)
        .wrap_err("sign genesis block")?;
    let framed = block.0.encode_wire().wrap_err("frame genesis block")?;
    let mut file = BufWriter::new(File::create(signed_path)?);
    file.write_all(&framed)?;
    Ok(())
}

fn generate_genesis_key_pair(
    base_seed: Option<&[u8]>,
    extra_seed: &[u8],
) -> (iroha_crypto::PublicKey, ExposedPrivateKey) {
    if base_seed.is_none() {
        return (
            REAL_GENESIS_ACCOUNT_KEYPAIR.public_key().clone(),
            ExposedPrivateKey(REAL_GENESIS_ACCOUNT_KEYPAIR.private_key().clone()),
        );
    }

    let (public_key, private_key) = iroha_crypto::KeyPair::from_seed(
        base_seed
            .expect("covered by early return for None seeds")
            .iter()
            .chain(extra_seed)
            .copied()
            .collect::<Vec<_>>(),
        iroha_crypto::Algorithm::default(),
    )
    .into_parts();
    (public_key, ExposedPrivateKey(private_key))
}

fn generate_account_key_pair(
    base_seed: Option<&[u8]>,
    extra_seed: &[u8],
) -> (iroha_crypto::PublicKey, ExposedPrivateKey) {
    let (public_key, private_key) = base_seed.map_or_else(
        || {
            iroha_crypto::KeyPair::random_with_algorithm(iroha_crypto::Algorithm::default())
                .into_parts()
        },
        |seed| {
            iroha_crypto::KeyPair::from_seed(
                seed.iter().chain(extra_seed).copied().collect::<Vec<_>>(),
                iroha_crypto::Algorithm::default(),
            )
            .into_parts()
        },
    );
    (public_key, ExposedPrivateKey(private_key))
}

fn generate_bls_key_pair(
    base_seed: Option<&[u8]>,
    extra_seed: &[u8],
) -> (iroha_crypto::PublicKey, ExposedPrivateKey, Vec<u8>) {
    let kp = base_seed.map_or_else(
        || iroha_crypto::KeyPair::random_with_algorithm(iroha_crypto::Algorithm::BlsNormal),
        |seed| {
            let material = seed.iter().chain(extra_seed).copied().collect::<Vec<_>>();
            iroha_crypto::KeyPair::from_seed(material, iroha_crypto::Algorithm::BlsNormal)
        },
    );
    let pop = iroha_crypto::bls_normal_pop_prove(kp.private_key()).expect("generate BLS PoP");
    let (public_key, private_key) = kp.into_parts();
    (public_key, ExposedPrivateKey(private_key), pop)
}

fn repo_root_path() -> PathBuf {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .map_or_else(
            || PathBuf::from(env!("CARGO_MANIFEST_DIR")),
            Path::to_path_buf,
        );
    root.canonicalize().unwrap_or(root)
}

fn resolve_target_dir(repo_root: &Path, target_dir: Option<&str>) -> PathBuf {
    target_dir.map_or_else(
        || repo_root.join("target"),
        |path| {
            let target_dir = PathBuf::from(path);
            if target_dir.is_absolute() {
                target_dir
            } else {
                repo_root.join(target_dir)
            }
        },
    )
}

fn default_irohad_bin_paths() -> (PathBuf, PathBuf) {
    let repo_root = repo_root_path();
    let target_dir = resolve_target_dir(&repo_root, env::var("CARGO_TARGET_DIR").ok().as_deref());
    (
        target_dir.join("debug").join("irohad"),
        target_dir.join("release").join("irohad"),
    )
}

fn write_scripts(out_dir: &Path, peers: u16, build_line: BuildLine, sora_mode: bool) -> Result<()> {
    let start = out_dir.join("start.sh");
    let stop = out_dir.join("stop.sh");
    let (default_irohad_debug, default_irohad_release) = default_irohad_bin_paths();
    let mut start_file = BufWriter::new(File::create(&start)?);
    let sora_flag = if sora_mode { "--sora " } else { "" };
    writeln!(start_file, "#!/usr/bin/env bash")?;
    writeln!(start_file, "set -euo pipefail")?;
    writeln!(start_file, "DIR=$(cd \"$(dirname \"$0\")\" && pwd)")?;
    writeln!(start_file, "cd \"$DIR\"")?;
    writeln!(
        start_file,
        "export IROHA_BUILD_LINE=\"{}\"",
        build_line.as_str()
    )?;
    writeln!(
        start_file,
        "DEFAULT_IROHAD_BIN_DEBUG=\"{}\"",
        default_irohad_debug.display()
    )?;
    writeln!(
        start_file,
        "DEFAULT_IROHAD_BIN_RELEASE=\"{}\"",
        default_irohad_release.display()
    )?;
    writeln!(start_file, "if [ -z \"${{IROHAD_BIN:-}}\" ]; then")?;
    writeln!(
        start_file,
        "  if [ -x \"$DEFAULT_IROHAD_BIN_DEBUG\" ]; then"
    )?;
    writeln!(start_file, "    IROHAD_BIN=\"$DEFAULT_IROHAD_BIN_DEBUG\"")?;
    writeln!(
        start_file,
        "  elif [ -x \"$DEFAULT_IROHAD_BIN_RELEASE\" ]; then"
    )?;
    writeln!(start_file, "    IROHAD_BIN=\"$DEFAULT_IROHAD_BIN_RELEASE\"")?;
    writeln!(
        start_file,
        "  else\n    echo \"IROHAD_BIN not set and default ($DEFAULT_IROHAD_BIN_DEBUG or $DEFAULT_IROHAD_BIN_RELEASE) not found; build irohad or set IROHAD_BIN\" >&2\n    exit 1\n  fi"
    )?;
    writeln!(start_file, "fi")?;
    writeln!(
        start_file,
        "echo \"Using IROHAD_BIN=$IROHAD_BIN\" >&2\ncommand -v \"$IROHAD_BIN\" >/dev/null 2>&1 || {{ echo \"irohad binary not executable: $IROHAD_BIN\" >&2; exit 1; }}"
    )?;
    writeln!(start_file, "for i in $(seq 0 {}); do", peers - 1)?;
    writeln!(
        start_file,
        "  SNAPSHOT_STORE_DIR=\"$DIR/storage/peer${{i}}/snapshot\""
    )?;
    writeln!(start_file, "  mkdir -p \"$SNAPSHOT_STORE_DIR\"")?;
    writeln!(
        start_file,
        "  SNAPSHOT_STORE_DIR=\"$SNAPSHOT_STORE_DIR\" RUST_LOG=${{RUST_LOG:-info}} \"$IROHAD_BIN\" {sora_flag}--config \"$DIR/peer${{i}}.toml\" > \"$DIR/peer${{i}}.log\" 2>&1 &"
    )?;
    writeln!(start_file, "  echo $! > \"$DIR/peer${{i}}.pid\"")?;
    writeln!(
        start_file,
        "  echo \"peer$i pid $(cat $DIR/peer${{i}}.pid)\""
    )?;
    writeln!(start_file, "done")?;
    start_file.flush()?;

    let mut stop_file = BufWriter::new(File::create(&stop)?);
    writeln!(stop_file, "#!/usr/bin/env bash")?;
    writeln!(stop_file, "set -euo pipefail")?;
    writeln!(stop_file, "DIR=$(cd \"$(dirname \"$0\")\" && pwd)")?;
    writeln!(stop_file, "for pidfile in \"$DIR\"/peer*.pid; do")?;
    writeln!(stop_file, "  [ -f \"$pidfile\" ] || continue")?;
    writeln!(stop_file, "  kill \"$(cat \"$pidfile\")\" || true")?;
    writeln!(stop_file, "done")?;
    stop_file.flush()?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        fs::set_permissions(&start, PermissionsExt::from_mode(0o755))
            .wrap_err_with(|| format!("failed to mark {} executable", start.display()))?;
        fs::set_permissions(&stop, PermissionsExt::from_mode(0o755))
            .wrap_err_with(|| format!("failed to mark {} executable", stop.display()))?;
    }

    Ok(())
}

fn copy_rans_tables(out_dir: &Path) -> Result<()> {
    let repo_root = repo_root_path();
    let src = repo_root.join("codec/rans/tables");
    let dest = out_dir.join("codec/rans/tables");
    fs::create_dir_all(&dest)
        .wrap_err_with(|| format!("failed to create rANS tables directory {}", dest.display()))?;
    let mut copied_seed = false;
    if let Ok(entries) = fs::read_dir(&src) {
        for entry in entries {
            let entry = entry?;
            if entry.file_type()?.is_file() {
                let fname = entry.file_name();
                if fname == "rans_seed0.toml" {
                    copied_seed = true;
                }
                fs::copy(entry.path(), dest.join(fname)).wrap_err("copy rANS table file")?;
            }
        }
    }
    if !copied_seed {
        let seed_path = dest.join("rans_seed0.toml");
        fs::write(&seed_path, RANS_SEED0_TABLE).wrap_err("write embedded rANS table")?;
    }
    Ok(())
}

const CLIENT_ACCOUNT_DOMAIN: &str = "wonderland";
const CLIENT_ACCOUNT_PUBLIC: &str =
    "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03";
const CLIENT_ACCOUNT_PRIVATE: &str =
    "802620CCF31D85E3B32A4BEA59987CE0C78E3B8E2DB93881468AB2435FE45D5C9DCD53";

fn write_client_config(
    out_dir: &Path,
    base_api_port: u16,
    torii_host: &CanonicalHost,
) -> Result<()> {
    let path = out_dir.join("client.toml");
    // Render explicitly to avoid pretty-printer wrapping the long keys.
    let torii_host = torii_host.url_host();
    let rendered = format!(
        concat!(
            "chain = \"{chain}\"\n",
            "torii_url = \"http://{torii_host}:{torii_port}/\"\n",
            "\n",
            "[transaction]\n",
            "time_to_live_ms = {ttl_ms}\n",
            "status_timeout_ms = {status_timeout_ms}\n",
            "nonce = false\n",
            "\n",
            "[account]\n",
            "domain = \"{domain}\"\n",
            "private_key = \"{private_key}\"\n",
            "public_key  = \"{public_key}\"\n",
            "\n",
            "[basic_auth]\n",
            "password  = \"ilovetea\"\n",
            "web_login = \"mad_hatter\"\n",
        ),
        chain = CHAIN_ID,
        torii_port = base_api_port,
        torii_host = torii_host,
        ttl_ms = LOCALNET_CLIENT_TTL_MS,
        status_timeout_ms = LOCALNET_CLIENT_STATUS_TIMEOUT_MS,
        domain = CLIENT_ACCOUNT_DOMAIN,
        private_key = CLIENT_ACCOUNT_PRIVATE,
        public_key = CLIENT_ACCOUNT_PUBLIC,
    );

    fs::write(&path, rendered)
        .wrap_err_with(|| format!("failed to write client config to {}", path.display()))
}

#[cfg(test)]
mod tests {
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;
    use std::{
        env, fs,
        io::BufWriter,
        num::NonZeroU32,
        path::{Path, PathBuf},
        time::Duration,
    };

    use iroha_config::{
        base::toml::TomlSource, kura::FsyncMode, logger::Directives, parameters::actual,
    };
    use iroha_data_model::{
        block::{consensus::PROTO_VERSION, decode_framed_signed_block},
        isi::SetParameter,
        parameter::{
            Parameter,
            system::{
                Parameters, SumeragiConsensusMode, SumeragiNposParameters, consensus_metadata,
            },
        },
        transaction::Executable,
    };
    use norito::{derive::JsonDeserialize, json, literal};

    use super::*;

    fn default_redundant_send_r(peers: NonZeroU16) -> u8 {
        redundant_send_r_from_len(usize::from(peers.get()))
    }

    fn localnet_genesis_for_opts(opts: &LocalnetOptions) -> RawGenesisTransaction {
        let seed_bytes = opts.seed.as_ref().map(String::as_bytes);
        let peers = build_peers(
            opts.peers.get(),
            seed_bytes,
            opts.base_api_port,
            opts.base_p2p_port,
        );
        let da_rbc_enabled = opts.build_line.is_iroha3();
        let npos_bootstrap = localnet_uses_npos(opts.consensus_mode, opts.next_consensus_mode);
        let perf_spec = opts.perf_profile.map(LocalnetPerfProfile::spec);
        let redundant_send_r = resolve_localnet_redundant_send_r(
            opts.redundant_send_r
                .or_else(|| perf_spec.and_then(|spec| spec.redundant_send_r)),
            da_rbc_enabled,
            opts.peers,
        );
        let block_time_override = opts
            .block_time_ms
            .or_else(|| perf_spec.map(|spec| spec.block_time_ms));
        let commit_time_override = opts
            .commit_time_ms
            .or_else(|| perf_spec.map(|spec| spec.commit_time_ms));
        let (block_time_ms, commit_time_ms) =
            resolve_localnet_pipeline_times(block_time_override, commit_time_override);
        let collectors_k = perf_spec.map(|spec| spec.collectors_k);
        let block_max_transactions = perf_spec.map_or(LOCALNET_BLOCK_MAX_TRANSACTIONS, |spec| {
            spec.block_max_transactions
        });
        let requested_stake_amount = perf_spec.map(|spec| spec.stake_amount);
        let (genesis_public_key, _) = generate_genesis_key_pair(seed_bytes, GENESIS_SEED);
        let mut genesis = generate_raw_genesis(
            &genesis_public_key,
            opts.consensus_mode,
            opts.next_consensus_mode,
            opts.mode_activation_height,
            opts.build_line,
        )
        .expect("generate raw genesis");
        if opts.extra_accounts > 0 || !opts.assets.is_empty() {
            genesis = extend_genesis(genesis, seed_bytes, opts.extra_accounts, &opts.assets)
                .expect("extend genesis");
        }
        genesis = apply_parameter_overrides(
            genesis,
            block_time_ms,
            commit_time_ms,
            redundant_send_r,
            collectors_k,
            block_max_transactions,
            opts.consensus_mode,
            opts.next_consensus_mode,
        );
        genesis = append_peer_pop(genesis, &peers);
        if npos_bootstrap {
            let gas_account_id = localnet_gas_account_id(&genesis_public_key);
            let stake_amount =
                localnet_npos_stake_amount(&genesis.effective_parameters(), requested_stake_amount);
            genesis =
                append_localnet_npos_bootstrap(genesis, &peers, &gas_account_id, stake_amount)
                    .expect("append localnet NPoS bootstrap");
        }
        apply_localnet_crypto_overrides(genesis, npos_bootstrap)
    }

    fn genesis_json_from_path(path: &Path) -> json::Value {
        let contents = fs::read_to_string(path).expect("read genesis");
        json::from_str(&contents).expect("parse genesis json")
    }

    fn genesis_parameters(manifest: &json::Value) -> Parameters {
        let transactions = manifest
            .get("transactions")
            .and_then(json::Value::as_array)
            .expect("genesis transactions");
        let params_value = transactions
            .iter()
            .rev()
            .find_map(|tx| tx.get("parameters"))
            .expect("parameters entry");
        json::from_value(params_value.clone()).expect("parse genesis parameters")
    }

    fn genesis_consensus_mode(manifest: &json::Value) -> Option<SumeragiConsensusMode> {
        manifest
            .get("consensus_mode")
            .and_then(|value| json::from_value(value.clone()).ok())
    }

    #[test]
    fn generated_configs_parse_with_current_schema() {
        let temp = tempfile::tempdir().expect("make temp dir");
        let opts = LocalnetOptions {
            build_line: BuildLine::Iroha3,
            sora_profile: None,
            perf_profile: None,
            peers: NonZeroU16::new(2).expect("non-zero"),
            seed: Some("kagami-config-compat".to_owned()),
            bind_host: DEFAULT_BIND_HOST.to_owned(),
            public_host: DEFAULT_PUBLIC_HOST.to_owned(),
            base_api_port: 19080,
            base_p2p_port: 23337,
            out_dir: temp.path().to_path_buf(),
            extra_accounts: 0,
            assets: Vec::new(),
            block_time_ms: None,
            commit_time_ms: None,
            redundant_send_r: None,
            consensus_mode: SumeragiConsensusMode::Npos,
            next_consensus_mode: None,
            mode_activation_height: None,
        };

        generate_localnet(&opts, &mut BufWriter::new(Vec::new())).expect("generate localnet files");

        let source =
            TomlSource::from_file(temp.path().join("peer0.toml")).expect("read generated config");
        actual::Root::from_toml_source(source).expect("generated config must parse");
    }

    #[test]
    fn generated_configs_for_user_localnet_parse() {
        let temp = tempfile::tempdir().expect("make temp dir");
        let opts = LocalnetOptions {
            build_line: BuildLine::Iroha3,
            sora_profile: None,
            perf_profile: None,
            peers: NonZeroU16::new(4).expect("non-zero"),
            seed: Some("Iroha".to_owned()),
            bind_host: DEFAULT_PUBLIC_HOST.to_owned(),
            public_host: DEFAULT_PUBLIC_HOST.to_owned(),
            base_api_port: 29080,
            base_p2p_port: 33337,
            out_dir: temp.path().to_path_buf(),
            extra_accounts: 0,
            assets: vec![AssetSpec {
                id: localnet_sample_asset_literal(),
                mint_to: ALICE_ID
                    .clone()
                    .to_account_id(CLIENT_ACCOUNT_DOMAIN.parse().expect("client domain")),
                quantity: 100,
            }],
            block_time_ms: None,
            commit_time_ms: None,
            redundant_send_r: None,
            consensus_mode: SumeragiConsensusMode::Npos,
            next_consensus_mode: None,
            mode_activation_height: None,
        };

        generate_localnet(&opts, &mut BufWriter::new(Vec::new())).expect("generate localnet files");

        let source =
            TomlSource::from_file(temp.path().join("peer0.toml")).expect("read generated config");
        actual::Root::from_toml_source(source).expect("generated config must parse");
    }

    #[test]
    fn generated_peer_config_includes_required_addr_literals() {
        let temp = tempfile::tempdir().expect("make temp dir");
        let opts = LocalnetOptions {
            build_line: BuildLine::Iroha3,
            sora_profile: None,
            perf_profile: None,
            peers: NonZeroU16::new(2).expect("non-zero"),
            seed: Some("kagami-addr-literals".to_owned()),
            bind_host: DEFAULT_BIND_HOST.to_owned(),
            public_host: DEFAULT_PUBLIC_HOST.to_owned(),
            base_api_port: 21080,
            base_p2p_port: 24337,
            out_dir: temp.path().to_path_buf(),
            extra_accounts: 0,
            assets: Vec::new(),
            block_time_ms: None,
            commit_time_ms: None,
            redundant_send_r: None,
            consensus_mode: SumeragiConsensusMode::Npos,
            next_consensus_mode: None,
            mode_activation_height: None,
        };

        generate_localnet(&opts, &mut BufWriter::new(Vec::new())).expect("generate localnet files");

        let peer_cfg: toml::Value = toml::from_str(
            &fs::read_to_string(temp.path().join("peer0.toml"))
                .expect("read generated peer config"),
        )
        .expect("parse peer config");
        assert!(
            peer_cfg.get("public_key").is_some(),
            "public_key is required"
        );
        assert!(
            peer_cfg.get("private_key").is_some(),
            "private_key is required"
        );
        assert!(peer_cfg.get("genesis").is_some(), "genesis is required");

        let network = peer_cfg
            .get("network")
            .and_then(toml::Value::as_table)
            .expect("network table");
        let torii = peer_cfg
            .get("torii")
            .and_then(toml::Value::as_table)
            .expect("torii table");
        let addr_fields = [
            ("network.address", network.get("address")),
            ("network.public_address", network.get("public_address")),
            ("torii.address", torii.get("address")),
        ];
        for (label, value) in addr_fields {
            let literal = value
                .and_then(toml::Value::as_str)
                .unwrap_or_else(|| panic!("{label} is required"));
            let body =
                literal::parse("addr", literal).unwrap_or_else(|err| panic!("{label}: {err}"));
            assert!(
                body.contains(':'),
                "expected host:port in {label}, got {body}"
            );
        }
    }

    #[test]
    fn generated_peer_config_allows_bls_signing_for_npos() {
        let temp = tempfile::tempdir().expect("make temp dir");
        let opts = LocalnetOptions {
            build_line: BuildLine::Iroha3,
            sora_profile: None,
            perf_profile: None,
            peers: NonZeroU16::new(2).expect("non-zero"),
            seed: Some("kagami-crypto-allow".to_owned()),
            bind_host: DEFAULT_BIND_HOST.to_owned(),
            public_host: DEFAULT_PUBLIC_HOST.to_owned(),
            base_api_port: 31080,
            base_p2p_port: 35337,
            out_dir: temp.path().to_path_buf(),
            extra_accounts: 0,
            assets: Vec::new(),
            block_time_ms: None,
            commit_time_ms: None,
            redundant_send_r: None,
            consensus_mode: SumeragiConsensusMode::Npos,
            next_consensus_mode: None,
            mode_activation_height: None,
        };

        generate_localnet(&opts, &mut BufWriter::new(Vec::new())).expect("generate localnet files");

        let peer_cfg: toml::Value = toml::from_str(
            &fs::read_to_string(temp.path().join("peer0.toml"))
                .expect("read generated peer config"),
        )
        .expect("parse peer config");
        let allowed = peer_cfg
            .get("crypto")
            .and_then(|crypto| crypto.get("allowed_signing"))
            .and_then(|value| value.as_array())
            .expect("crypto.allowed_signing should be set for NPoS localnet");
        assert!(
            allowed
                .iter()
                .filter_map(|value| value.as_str())
                .any(|value| value.eq_ignore_ascii_case("bls_normal")),
            "allowed_signing must include bls_normal for NPoS localnet"
        );
    }

    #[test]
    fn generated_genesis_allows_bls_signing_for_npos() {
        let temp = tempfile::tempdir().expect("make temp dir");
        let opts = LocalnetOptions {
            build_line: BuildLine::Iroha3,
            sora_profile: None,
            perf_profile: None,
            peers: NonZeroU16::new(2).expect("non-zero"),
            seed: Some("kagami-genesis-crypto".to_owned()),
            bind_host: DEFAULT_BIND_HOST.to_owned(),
            public_host: DEFAULT_PUBLIC_HOST.to_owned(),
            base_api_port: 32080,
            base_p2p_port: 36337,
            out_dir: temp.path().to_path_buf(),
            extra_accounts: 0,
            assets: Vec::new(),
            block_time_ms: None,
            commit_time_ms: None,
            redundant_send_r: None,
            consensus_mode: SumeragiConsensusMode::Npos,
            next_consensus_mode: None,
            mode_activation_height: None,
        };

        generate_localnet(&opts, &mut BufWriter::new(Vec::new())).expect("generate localnet files");

        let manifest = localnet_genesis_for_opts(&opts);
        let crypto = manifest.crypto();
        assert!(
            crypto
                .allowed_signing
                .iter()
                .any(|algo| matches!(algo, iroha_crypto::Algorithm::BlsNormal)),
            "genesis allowed_signing must include bls_normal for NPoS localnet"
        );
        let bls_curve = iroha_data_model::account::curve::CurveId::try_from_algorithm(
            iroha_crypto::Algorithm::BlsNormal,
        )
        .expect("bls curve id");
        assert!(
            crypto.allowed_curve_ids.contains(&bls_curve.as_u8()),
            "genesis allowed_curve_ids must include bls_normal for NPoS localnet"
        );
    }

    #[test]
    fn generated_peer_configs_include_peer_telemetry_urls() {
        let temp = tempfile::tempdir().expect("make temp dir");
        let opts = LocalnetOptions {
            build_line: BuildLine::Iroha3,
            sora_profile: None,
            perf_profile: None,
            peers: NonZeroU16::new(2).expect("non-zero"),
            seed: Some("kagami-peer-telemetry".to_owned()),
            bind_host: DEFAULT_BIND_HOST.to_owned(),
            public_host: DEFAULT_PUBLIC_HOST.to_owned(),
            base_api_port: 19080,
            base_p2p_port: 23337,
            out_dir: temp.path().to_path_buf(),
            extra_accounts: 0,
            assets: Vec::new(),
            block_time_ms: None,
            commit_time_ms: None,
            redundant_send_r: None,
            consensus_mode: SumeragiConsensusMode::Npos,
            next_consensus_mode: None,
            mode_activation_height: None,
        };

        generate_localnet(&opts, &mut BufWriter::new(Vec::new())).expect("generate localnet files");

        let peer_cfg: toml::Value = toml::from_str(
            &fs::read_to_string(temp.path().join("peer0.toml"))
                .expect("read generated peer config"),
        )
        .expect("parse peer config");
        let urls = peer_cfg
            .get("torii")
            .and_then(toml::Value::as_table)
            .and_then(|torii| torii.get("peer_telemetry_urls"))
            .and_then(toml::Value::as_array)
            .expect("peer_telemetry_urls array");
        let urls = urls
            .iter()
            .filter_map(toml::Value::as_str)
            .collect::<Vec<_>>();
        assert_eq!(
            urls,
            vec!["http://127.0.0.1:19080/", "http://127.0.0.1:19081/"],
        );

        let allowlist = peer_cfg
            .get("torii")
            .and_then(toml::Value::as_table)
            .and_then(|torii| torii.get("preauth_allow_cidrs"))
            .and_then(toml::Value::as_array)
            .expect("preauth_allow_cidrs array");
        let allowlist = allowlist
            .iter()
            .filter_map(toml::Value::as_str)
            .collect::<Vec<_>>();
        assert_eq!(allowlist, LOCALNET_PREAUTH_ALLOW_CIDRS);

        let allowlist = peer_cfg
            .get("torii")
            .and_then(toml::Value::as_table)
            .and_then(|torii| torii.get("api_allow_cidrs"))
            .and_then(toml::Value::as_array)
            .expect("api_allow_cidrs array");
        let allowlist = allowlist
            .iter()
            .filter_map(toml::Value::as_str)
            .collect::<Vec<_>>();
        assert_eq!(allowlist, LOCALNET_PREAUTH_ALLOW_CIDRS);

        let telemetry_enabled = peer_cfg
            .get("telemetry_enabled")
            .and_then(toml::Value::as_bool)
            .expect("telemetry_enabled boolean");
        assert!(telemetry_enabled);

        let telemetry_profile = peer_cfg
            .get("telemetry_profile")
            .and_then(toml::Value::as_str)
            .expect("telemetry_profile string");
        assert_eq!(telemetry_profile, LOCALNET_TELEMETRY_PROFILE);
    }

    #[allow(clippy::too_many_lines)]
    #[test]
    fn generated_configs_set_localnet_channel_caps() {
        let temp = tempfile::tempdir().expect("make temp dir");
        let opts = LocalnetOptions {
            build_line: BuildLine::Iroha3,
            sora_profile: None,
            perf_profile: None,
            peers: NonZeroU16::new(2).expect("non-zero"),
            seed: Some("kagami-channel-caps".to_owned()),
            bind_host: DEFAULT_BIND_HOST.to_owned(),
            public_host: DEFAULT_PUBLIC_HOST.to_owned(),
            base_api_port: 20080,
            base_p2p_port: 24337,
            out_dir: temp.path().to_path_buf(),
            extra_accounts: 0,
            assets: Vec::new(),
            block_time_ms: None,
            commit_time_ms: None,
            redundant_send_r: None,
            consensus_mode: SumeragiConsensusMode::Npos,
            next_consensus_mode: None,
            mode_activation_height: None,
        };

        generate_localnet(&opts, &mut BufWriter::new(Vec::new())).expect("generate localnet files");

        let source =
            TomlSource::from_file(temp.path().join("peer0.toml")).expect("read generated config");
        let parsed = actual::Root::from_toml_source(source).expect("generated config must parse");

        assert_eq!(
            parsed.network.connect_startup_delay,
            Duration::from_millis(LOCALNET_CONNECT_STARTUP_DELAY_MS),
            "localnet should delay outbound dials to reduce startup connect errors"
        );
        assert_eq!(
            parsed.sumeragi.queues.votes, LOCALNET_MSG_CHANNEL_CAP_VOTES,
            "localnet should raise vote channel caps to avoid RBC stalls"
        );
        assert_eq!(
            parsed.sumeragi.queues.block_payload, LOCALNET_MSG_CHANNEL_CAP_BLOCK_PAYLOAD,
            "localnet should raise block payload caps to avoid RBC stalls"
        );
        assert_eq!(
            parsed.sumeragi.queues.rbc_chunks, LOCALNET_MSG_CHANNEL_CAP_RBC_CHUNKS,
            "localnet should raise RBC chunk caps to avoid drops"
        );
        assert_eq!(
            parsed.sumeragi.queues.blocks, LOCALNET_MSG_CHANNEL_CAP_BLOCKS,
            "localnet should raise block message caps to avoid drops"
        );
        assert_eq!(
            parsed.sumeragi.queues.control, LOCALNET_CONTROL_MSG_CHANNEL_CAP,
            "localnet should raise queues.control alongside data channels"
        );
        assert_eq!(
            parsed.sumeragi.da.quorum_timeout_multiplier, LOCALNET_DA_QUORUM_TIMEOUT_MULTIPLIER,
            "localnet should set DA commit-quorum timeout multiplier"
        );
        assert_eq!(
            parsed.sumeragi.da.availability_timeout_multiplier,
            LOCALNET_DA_AVAILABILITY_TIMEOUT_MULTIPLIER,
            "localnet should tune DA availability timeout multiplier"
        );
        assert_eq!(
            parsed.sumeragi.da.availability_timeout_floor,
            Duration::from_millis(LOCALNET_DA_AVAILABILITY_TIMEOUT_FLOOR_MS),
            "localnet should set DA availability timeout floor"
        );
        assert_eq!(
            parsed.sumeragi.rbc.chunk_max_bytes, LOCALNET_RBC_CHUNK_MAX_BYTES,
            "localnet should raise RBC chunk size for large payloads"
        );
        assert_eq!(
            parsed.sumeragi.rbc.payload_chunks_per_tick, LOCALNET_RBC_PAYLOAD_CHUNKS_PER_TICK,
            "localnet should pace RBC payload chunk fanout"
        );
        assert_eq!(
            parsed.sumeragi.pacing_governor.min_factor_bps, LOCALNET_PACING_GOVERNOR_MIN_FACTOR_BPS,
            "localnet should clamp pacing governor min factor"
        );
        assert_eq!(
            parsed.sumeragi.pacing_governor.max_factor_bps, LOCALNET_PACING_GOVERNOR_MAX_FACTOR_BPS,
            "localnet should clamp pacing governor max factor"
        );
        assert_eq!(
            parsed.queue.capacity.get(),
            LOCALNET_QUEUE_CAPACITY,
            "localnet should cap queue capacity by default"
        );
        assert_eq!(
            parsed.queue.capacity_per_user.get(),
            LOCALNET_QUEUE_CAPACITY,
            "localnet should cap per-user queue capacity by default"
        );
        assert_eq!(
            parsed.queue.transaction_time_to_live,
            Duration::from_millis(LOCALNET_QUEUE_TTL_MS),
            "localnet should set a bounded queue TTL by default"
        );
        assert_eq!(
            parsed.sumeragi.block.proposal_queue_scan_multiplier.get(),
            LOCALNET_PROPOSAL_QUEUE_SCAN_MULTIPLIER,
            "localnet should cap proposal scan work to keep block cadence"
        );
        assert_eq!(
            parsed.nexus.fusion.exit_teu, LOCALNET_LANE_TEU_CAPACITY,
            "localnet should raise lane TEU capacity for high-throughput blocks"
        );
        assert_eq!(
            parsed
                .torii
                .tx_rate_per_authority_per_sec
                .map(NonZeroU32::get),
            Some(LOCALNET_TORII_TX_RATE_PER_AUTHORITY_PER_SEC),
            "localnet should raise Torii tx rate limits"
        );
        assert_eq!(
            parsed.torii.tx_burst_per_authority.map(NonZeroU32::get),
            Some(LOCALNET_TORII_TX_BURST_PER_AUTHORITY),
            "localnet should raise Torii tx burst limits"
        );
        assert_eq!(
            parsed
                .torii
                .preauth_rate_per_ip_per_sec
                .map(NonZeroU32::get),
            Some(LOCALNET_TORII_PREAUTH_RATE_PER_IP_PER_SEC),
            "localnet should raise Torii pre-auth rate limits"
        );
        assert_eq!(
            parsed.torii.preauth_burst_per_ip.map(NonZeroU32::get),
            Some(LOCALNET_TORII_PREAUTH_BURST_PER_IP),
            "localnet should raise Torii pre-auth burst limits"
        );
        assert_eq!(
            parsed.torii.api_high_load_tx_threshold,
            Some(LOCALNET_QUEUE_CAPACITY),
            "localnet should configure API high-load threshold to match queue capacity"
        );
        assert_eq!(
            parsed.network.p2p_subscriber_queue_cap.get(),
            LOCALNET_P2P_SUBSCRIBER_QUEUE_CAP,
            "localnet should raise P2P subscriber queue cap for fast pipelines"
        );
        assert_eq!(
            parsed
                .network
                .consensus_ingress_rate_per_sec
                .map(NonZeroU32::get),
            Some(LOCALNET_CONSENSUS_INGRESS_RATE_PER_SEC),
            "localnet should raise consensus ingress rate caps"
        );
        assert_eq!(
            parsed.network.consensus_ingress_burst.map(NonZeroU32::get),
            Some(LOCALNET_CONSENSUS_INGRESS_BURST),
            "localnet should raise consensus ingress burst caps"
        );
        assert_eq!(
            parsed
                .network
                .consensus_ingress_bytes_per_sec
                .map(NonZeroU32::get),
            Some(LOCALNET_CONSENSUS_INGRESS_BYTES_PER_SEC),
            "localnet should raise consensus ingress bytes caps"
        );
        assert_eq!(
            parsed
                .network
                .consensus_ingress_bytes_burst
                .map(NonZeroU32::get),
            Some(LOCALNET_CONSENSUS_INGRESS_BYTES_BURST),
            "localnet should raise consensus ingress bytes burst caps"
        );
        assert_eq!(
            parsed
                .network
                .consensus_ingress_critical_rate_per_sec
                .map(NonZeroU32::get),
            Some(LOCALNET_CONSENSUS_INGRESS_CRITICAL_RATE_PER_SEC),
            "localnet should raise critical consensus ingress rate caps"
        );
        assert_eq!(
            parsed
                .network
                .consensus_ingress_critical_burst
                .map(NonZeroU32::get),
            Some(LOCALNET_CONSENSUS_INGRESS_CRITICAL_BURST),
            "localnet should raise critical consensus ingress burst caps"
        );
        assert_eq!(
            parsed
                .network
                .consensus_ingress_critical_bytes_per_sec
                .map(NonZeroU32::get),
            Some(LOCALNET_CONSENSUS_INGRESS_CRITICAL_BYTES_PER_SEC),
            "localnet should raise critical consensus ingress bytes caps"
        );
        assert_eq!(
            parsed
                .network
                .consensus_ingress_critical_bytes_burst
                .map(NonZeroU32::get),
            Some(LOCALNET_CONSENSUS_INGRESS_CRITICAL_BYTES_BURST),
            "localnet should raise critical consensus ingress bytes burst caps"
        );
        assert_eq!(
            parsed.transaction_gossiper.gossip_period,
            Duration::from_millis(LOCALNET_TX_GOSSIP_PERIOD_FAST_MS),
            "localnet should shorten tx gossip period for 1s pipelines"
        );
        assert_eq!(
            parsed.transaction_gossiper.gossip_resend_ticks.get(),
            LOCALNET_TX_GOSSIP_RESEND_TICKS_FAST,
            "localnet should lower tx gossip resend ticks for 1s pipelines"
        );
        assert_eq!(
            parsed
                .transaction_gossiper
                .dataspace
                .public_target_reshuffle,
            Duration::from_millis(LOCALNET_TX_GOSSIP_PERIOD_FAST_MS),
            "localnet should reshuffle public tx gossip targets quickly"
        );
        assert_eq!(
            parsed
                .transaction_gossiper
                .dataspace
                .restricted_target_reshuffle,
            Duration::from_millis(LOCALNET_TX_GOSSIP_PERIOD_FAST_MS),
            "localnet should reshuffle restricted tx gossip targets quickly"
        );
        let (block_ms, commit_ms) = resolve_localnet_pipeline_times(None, None);
        let block_ms = block_ms.expect("localnet defaults provide block_time_ms");
        let commit_ms = commit_ms.expect("localnet defaults provide commit_time_ms");
        let total_ms = block_ms.saturating_add(commit_ms);
        let scaled = total_ms.saturating_mul(LOCALNET_COMMIT_INFLIGHT_TIMEOUT_MULTIPLIER);
        let expected_timeout = scaled.clamp(
            LOCALNET_COMMIT_INFLIGHT_TIMEOUT_MIN_MS,
            LOCALNET_COMMIT_INFLIGHT_TIMEOUT_MAX_MS,
        );
        assert_eq!(
            parsed.sumeragi.persistence.commit_inflight_timeout,
            Duration::from_millis(expected_timeout),
            "localnet should tune commit inflight timeout to the fast pipeline"
        );
        assert_eq!(
            parsed.kura.fsync_mode,
            FsyncMode::Off,
            "localnet should disable Kura fsync for fast local runs"
        );
    }

    #[test]
    fn localnet_commit_inflight_timeout_scales_defaults() {
        let (block_ms, commit_ms) = resolve_localnet_pipeline_times(None, None);
        let total_ms = block_ms
            .expect("localnet defaults provide block_time_ms")
            .saturating_add(commit_ms.expect("localnet defaults provide commit_time_ms"));
        let scaled = total_ms.saturating_mul(LOCALNET_COMMIT_INFLIGHT_TIMEOUT_MULTIPLIER);
        let expected = scaled.clamp(
            LOCALNET_COMMIT_INFLIGHT_TIMEOUT_MIN_MS,
            LOCALNET_COMMIT_INFLIGHT_TIMEOUT_MAX_MS,
        );

        assert_eq!(localnet_commit_inflight_timeout_ms(None, None), expected);
    }

    #[test]
    fn localnet_pipeline_times_mirror_single_override() {
        let (block_ms, commit_ms) = resolve_localnet_pipeline_times(Some(1_500), None);
        assert_eq!(block_ms, Some(1_500));
        assert_eq!(commit_ms, Some(1_500));

        let (block_ms, commit_ms) = resolve_localnet_pipeline_times(None, Some(2_500));
        assert_eq!(block_ms, Some(2_500));
        assert_eq!(commit_ms, Some(2_500));
    }

    #[test]
    fn localnet_tx_gossip_overrides_follow_fast_pipeline() {
        let overrides =
            localnet_tx_gossip_overrides(LOCALNET_PIPELINE_TIME_MS).expect("fast pipeline");
        assert_eq!(overrides.period_ms, LOCALNET_TX_GOSSIP_PERIOD_FAST_MS);
        assert_eq!(overrides.resend_ticks, LOCALNET_TX_GOSSIP_RESEND_TICKS_FAST);
        assert!(
            localnet_tx_gossip_overrides(LOCALNET_PIPELINE_TIME_MS + 1).is_none(),
            "slow pipelines should keep default tx gossip cadence"
        );
    }

    #[test]
    fn perf_profile_permissioned_applies_collectors_and_pipeline() {
        let temp = tempfile::tempdir().expect("tmp dir");
        let opts = LocalnetOptions {
            build_line: BuildLine::Iroha3,
            sora_profile: None,
            perf_profile: Some(LocalnetPerfProfile::Throughput10kPermissioned),
            peers: NonZeroU16::new(4).expect("non-zero"),
            seed: Some("perf-profile-permissioned".to_owned()),
            bind_host: DEFAULT_BIND_HOST.to_owned(),
            public_host: DEFAULT_PUBLIC_HOST.to_owned(),
            base_api_port: 48080,
            base_p2p_port: 48337,
            out_dir: temp.path().to_path_buf(),
            extra_accounts: 0,
            assets: Vec::new(),
            block_time_ms: None,
            commit_time_ms: None,
            redundant_send_r: None,
            consensus_mode: SumeragiConsensusMode::Permissioned,
            next_consensus_mode: None,
            mode_activation_height: None,
        };

        generate_localnet(&opts, &mut BufWriter::new(Vec::new())).expect("generate localnet files");

        let source = TomlSource::from_file(temp.path().join("peer0.toml")).expect("read config");
        let parsed = actual::Root::from_toml_source(source).expect("config should parse");
        let expected_redundant_send_r = default_redundant_send_r(opts.peers);
        assert_eq!(parsed.sumeragi.collectors.k, 3);
        assert_eq!(
            parsed.sumeragi.collectors.redundant_send_r,
            expected_redundant_send_r
        );
        let expected_filter: Directives = LOCALNET_PERF_LOGGER_FILTER
            .parse()
            .expect("perf logger filter should parse");
        assert_eq!(parsed.logger.filter, Some(expected_filter));
        assert_eq!(
            parsed.pipeline.signature_batch_max_ed25519,
            LOCALNET_SIGNATURE_BATCH_MAX_ED25519
        );

        let genesis_path = temp.path().join("genesis.json");
        let manifest = genesis_json_from_path(&genesis_path);
        let params = genesis_parameters(&manifest);
        assert_eq!(params.sumeragi().block_time_ms(), 1_000);
        assert_eq!(params.sumeragi().commit_time_ms(), 1_000);
        assert_eq!(params.sumeragi().collectors_k(), 3);
        assert_eq!(
            params.sumeragi().collectors_redundant_send_r(),
            expected_redundant_send_r
        );
        assert_eq!(
            params.block.max_transactions.get(),
            LOCALNET_BLOCK_MAX_TRANSACTIONS
        );
    }

    #[test]
    fn perf_profile_npos_applies_k_aggregators() {
        let temp = tempfile::tempdir().expect("tmp dir");
        let opts = LocalnetOptions {
            build_line: BuildLine::Iroha3,
            sora_profile: None,
            perf_profile: Some(LocalnetPerfProfile::Throughput10kNpos),
            peers: NonZeroU16::new(4).expect("non-zero"),
            seed: Some("perf-profile-npos".to_owned()),
            bind_host: DEFAULT_BIND_HOST.to_owned(),
            public_host: DEFAULT_PUBLIC_HOST.to_owned(),
            base_api_port: 58080,
            base_p2p_port: 58337,
            out_dir: temp.path().to_path_buf(),
            extra_accounts: 0,
            assets: Vec::new(),
            block_time_ms: None,
            commit_time_ms: None,
            redundant_send_r: None,
            consensus_mode: SumeragiConsensusMode::Npos,
            next_consensus_mode: None,
            mode_activation_height: None,
        };

        generate_localnet(&opts, &mut BufWriter::new(Vec::new())).expect("generate localnet files");

        let source = TomlSource::from_file(temp.path().join("peer0.toml")).expect("read config");
        let parsed = actual::Root::from_toml_source(source).expect("config should parse");
        let expected_redundant_send_r = default_redundant_send_r(opts.peers);
        let expected_filter: Directives = LOCALNET_PERF_LOGGER_FILTER
            .parse()
            .expect("perf logger filter should parse");
        assert_eq!(parsed.logger.filter, Some(expected_filter));
        assert_eq!(
            parsed.pipeline.signature_batch_max_ed25519,
            LOCALNET_SIGNATURE_BATCH_MAX_ED25519
        );

        let genesis_path = temp.path().join("genesis.json");
        let manifest = genesis_json_from_path(&genesis_path);
        let params = genesis_parameters(&manifest);
        assert_eq!(params.sumeragi().collectors_k(), 3);
        assert_eq!(
            params.sumeragi().collectors_redundant_send_r(),
            expected_redundant_send_r
        );
        assert_eq!(params.sumeragi().block_time_ms(), 1_000);

        let npos = params
            .custom()
            .get(&SumeragiNposParameters::parameter_id())
            .and_then(SumeragiNposParameters::from_custom_parameter)
            .expect("npos parameters must be present");
        assert_eq!(npos.k_aggregators(), 3);
        assert_eq!(npos.redundant_send_r(), expected_redundant_send_r);
        assert_eq!(npos.seat_band_pct(), 100);
        assert_eq!(npos.min_self_bond(), 1);
    }

    #[test]
    fn validate_localnet_options_rejects_perf_profile_mismatch() {
        let opts = LocalnetOptions {
            build_line: BuildLine::Iroha3,
            sora_profile: None,
            perf_profile: Some(LocalnetPerfProfile::Throughput10kNpos),
            peers: NonZeroU16::new(4).expect("non-zero"),
            seed: None,
            bind_host: DEFAULT_BIND_HOST.to_owned(),
            public_host: DEFAULT_PUBLIC_HOST.to_owned(),
            base_api_port: 58080,
            base_p2p_port: 58337,
            out_dir: PathBuf::from("localnet"),
            extra_accounts: 0,
            assets: Vec::new(),
            block_time_ms: None,
            commit_time_ms: None,
            redundant_send_r: None,
            consensus_mode: SumeragiConsensusMode::Permissioned,
            next_consensus_mode: None,
            mode_activation_height: None,
        };

        let err = validate_localnet_options(&opts).expect_err("mismatch should fail");
        assert!(
            err.to_string().contains("perf-profile"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn genesis_key_defaults_to_real_keypair_when_unseeded() {
        let (public_key, _) = generate_genesis_key_pair(None, GENESIS_SEED);
        assert_eq!(
            public_key,
            REAL_GENESIS_ACCOUNT_KEYPAIR.public_key().clone()
        );
    }

    #[test]
    fn extra_account_keys_are_unique_when_unseeded() {
        let (first, _) = generate_account_key_pair(None, b"acct0");
        let (second, _) = generate_account_key_pair(None, b"acct1");
        assert_ne!(first, second);
    }

    #[derive(Debug, Clone, JsonDeserialize, PartialEq, Eq)]
    struct ConsensusHandshakeMetaTest {
        mode: String,
        bls_domain: String,
        wire_proto_versions: Vec<u32>,
        consensus_fingerprint: String,
    }

    #[test]
    fn generated_genesis_handshake_meta_decodes() {
        let temp = tempfile::tempdir().expect("make temp dir");
        let opts = LocalnetOptions {
            build_line: BuildLine::Iroha3,
            sora_profile: None,
            perf_profile: None,
            peers: NonZeroU16::new(4).expect("non-zero"),
            seed: Some("Iroha".to_owned()),
            bind_host: DEFAULT_PUBLIC_HOST.to_owned(),
            public_host: DEFAULT_PUBLIC_HOST.to_owned(),
            base_api_port: 29080,
            base_p2p_port: 33337,
            out_dir: temp.path().to_path_buf(),
            extra_accounts: 0,
            assets: vec![AssetSpec {
                id: localnet_sample_asset_literal(),
                mint_to: ALICE_ID
                    .clone()
                    .to_account_id(CLIENT_ACCOUNT_DOMAIN.parse().expect("client domain")),
                quantity: 100,
            }],
            block_time_ms: None,
            commit_time_ms: None,
            redundant_send_r: None,
            consensus_mode: SumeragiConsensusMode::Npos,
            next_consensus_mode: None,
            mode_activation_height: None,
        };

        generate_localnet(&opts, &mut BufWriter::new(Vec::new())).expect("generate localnet files");

        let genesis_path = temp.path().join("genesis.signed.nrt");
        let bytes = fs::read(&genesis_path).expect("read signed genesis");
        let block =
            decode_framed_signed_block(&bytes).expect("decode signed genesis from framed payload");

        let mut found = None;
        for tx in block.external_transactions() {
            if let Executable::Instructions(batch) = tx.instructions() {
                for instr in batch {
                    if let Some(set_param) = instr.as_any().downcast_ref::<SetParameter>()
                        && let Parameter::Custom(custom) = set_param.inner()
                        && custom.id() == &consensus_metadata::handshake_meta_id()
                    {
                        let meta: ConsensusHandshakeMetaTest = custom
                            .payload()
                            .try_into_any()
                            .expect("decode consensus_handshake_meta payload");
                        found = Some(meta);
                    }
                }
            }
        }

        let meta = found.expect("handshake metadata must be present");
        assert!(
            meta.wire_proto_versions.contains(&PROTO_VERSION),
            "missing expected wire proto version"
        );
        assert!(
            meta.consensus_fingerprint.starts_with("0x"),
            "fingerprint must be hex-prefixed"
        );
    }

    #[test]
    fn localnet_overrides_keep_da_enabled() {
        let temp = tempfile::tempdir().expect("tmp dir");
        let opts = LocalnetOptions {
            build_line: BuildLine::Iroha3,
            sora_profile: None,
            perf_profile: None,
            peers: NonZeroU16::new(2).expect("non-zero"),
            seed: Some("localnet-da-enabled".to_owned()),
            bind_host: DEFAULT_BIND_HOST.to_owned(),
            public_host: DEFAULT_PUBLIC_HOST.to_owned(),
            base_api_port: 28180,
            base_p2p_port: 28437,
            out_dir: temp.path().to_path_buf(),
            extra_accounts: 0,
            assets: Vec::new(),
            block_time_ms: None,
            commit_time_ms: None,
            redundant_send_r: None,
            consensus_mode: SumeragiConsensusMode::Npos,
            next_consensus_mode: None,
            mode_activation_height: None,
        };

        generate_localnet(&opts, &mut BufWriter::new(Vec::new())).expect("generate localnet");

        let manifest = localnet_genesis_for_opts(&opts);
        let params = manifest.effective_parameters();
        assert!(
            params.sumeragi().da_enabled(),
            "localnet should keep DA enabled for Iroha 3 defaults"
        );
        assert_eq!(
            params.sumeragi().collectors_redundant_send_r(),
            default_redundant_send_r(opts.peers),
            "localnet should preserve redundant send override"
        );
    }

    #[test]
    fn default_pipeline_time_injected_when_unset() {
        let temp = tempfile::tempdir().expect("tmp dir");
        let opts = LocalnetOptions {
            build_line: BuildLine::Iroha3,
            sora_profile: None,
            perf_profile: None,
            peers: NonZeroU16::new(2).expect("non-zero"),
            seed: Some("default-pipeline-time".to_owned()),
            bind_host: DEFAULT_BIND_HOST.to_owned(),
            public_host: DEFAULT_PUBLIC_HOST.to_owned(),
            base_api_port: 28090,
            base_p2p_port: 28357,
            out_dir: temp.path().to_path_buf(),
            extra_accounts: 0,
            assets: Vec::new(),
            block_time_ms: None,
            commit_time_ms: None,
            redundant_send_r: None,
            consensus_mode: SumeragiConsensusMode::Npos,
            next_consensus_mode: None,
            mode_activation_height: None,
        };

        generate_localnet(&opts, &mut BufWriter::new(Vec::new())).expect("generate localnet files");

        let genesis_path = temp.path().join("genesis.json");
        let manifest = genesis_json_from_path(&genesis_path);

        let (expected_block, expected_commit) = default_localnet_pipeline_times();
        let params = genesis_parameters(&manifest);
        assert_eq!(params.sumeragi().block_time_ms(), expected_block);
        assert_eq!(params.sumeragi().commit_time_ms(), expected_commit);
    }

    #[test]
    fn localnet_sets_block_max_transactions() {
        let temp = tempfile::tempdir().expect("tmp dir");
        let opts = LocalnetOptions {
            build_line: BuildLine::Iroha3,
            sora_profile: None,
            perf_profile: None,
            peers: NonZeroU16::new(2).expect("non-zero"),
            seed: Some("localnet-block-max".to_owned()),
            bind_host: DEFAULT_BIND_HOST.to_owned(),
            public_host: DEFAULT_PUBLIC_HOST.to_owned(),
            base_api_port: 30080,
            base_p2p_port: 30337,
            out_dir: temp.path().to_path_buf(),
            extra_accounts: 0,
            assets: Vec::new(),
            block_time_ms: None,
            commit_time_ms: None,
            redundant_send_r: None,
            consensus_mode: SumeragiConsensusMode::Npos,
            next_consensus_mode: None,
            mode_activation_height: None,
        };

        generate_localnet(&opts, &mut BufWriter::new(Vec::new())).expect("generate localnet files");

        let manifest = localnet_genesis_for_opts(&opts);
        let params = manifest.effective_parameters();
        assert_eq!(
            params.block().max_transactions().get(),
            LOCALNET_BLOCK_MAX_TRANSACTIONS,
            "localnet should raise max transactions per block"
        );
    }

    #[test]
    fn localnet_npos_bootstraps_public_lane_stake() {
        use std::collections::BTreeSet;

        let temp = tempfile::tempdir().expect("tmp dir");
        let opts = LocalnetOptions {
            build_line: BuildLine::Iroha3,
            sora_profile: None,
            perf_profile: None,
            peers: NonZeroU16::new(2).expect("non-zero"),
            seed: Some("localnet-npos-stake".to_owned()),
            bind_host: DEFAULT_BIND_HOST.to_owned(),
            public_host: DEFAULT_PUBLIC_HOST.to_owned(),
            base_api_port: 31080,
            base_p2p_port: 31337,
            out_dir: temp.path().to_path_buf(),
            extra_accounts: 0,
            assets: Vec::new(),
            block_time_ms: None,
            commit_time_ms: None,
            redundant_send_r: None,
            consensus_mode: SumeragiConsensusMode::Npos,
            next_consensus_mode: None,
            mode_activation_height: None,
        };

        generate_localnet(&opts, &mut BufWriter::new(Vec::new())).expect("generate localnet files");

        let manifest = localnet_genesis_for_opts(&opts);
        let mut validators = Vec::new();
        let mut activations = Vec::new();
        for instruction in manifest.instructions() {
            if let Some(register) = instruction
                .as_any()
                .downcast_ref::<RegisterPublicLaneValidator>()
            {
                validators.push(register);
            }
            if let Some(activate) = instruction
                .as_any()
                .downcast_ref::<ActivatePublicLaneValidator>()
            {
                activations.push(activate);
            }
        }
        assert_eq!(
            validators.len(),
            usize::from(opts.peers.get()),
            "expected one public-lane validator per peer"
        );
        assert_eq!(
            activations.len(),
            validators.len(),
            "expected one activation per public-lane validator"
        );
        let params = manifest.effective_parameters();
        let expected_stake_amount = localnet_npos_stake_amount(
            &params,
            opts.perf_profile
                .map(LocalnetPerfProfile::spec)
                .map(|spec| spec.stake_amount),
        );
        for register in &validators {
            assert_eq!(register.lane_id, LaneId::SINGLE);
            assert_eq!(register.validator, register.stake_account);
            assert_eq!(register.initial_stake, Numeric::from(expected_stake_amount));
        }
        for activate in &activations {
            assert_eq!(activate.lane_id, LaneId::SINGLE);
        }

        let peers = build_peers(
            opts.peers.get(),
            opts.seed.as_ref().map(String::as_bytes),
            opts.base_api_port,
            opts.base_p2p_port,
        );
        let expected: BTreeSet<_> = peers
            .iter()
            .map(|peer| AccountId::new(peer.public_key.clone()))
            .collect();
        let actual: BTreeSet<_> = validators
            .iter()
            .map(|register| register.validator.clone())
            .collect();
        assert_eq!(actual, expected, "validator roster should match peers");
        let actual_activations: BTreeSet<_> = activations
            .iter()
            .map(|activate| activate.validator.clone())
            .collect();
        assert_eq!(
            actual_activations, expected,
            "activation roster should match peers"
        );
    }

    #[test]
    fn localnet_npos_stake_amount_respects_min_self_bond() {
        let mut params = Parameters::default();
        let npos = SumeragiNposParameters {
            min_self_bond: LOCALNET_STAKE_AMOUNT + 1,
            ..Default::default()
        };
        params.set_parameter(Parameter::Custom(npos.into_custom_parameter()));

        let stake_amount = localnet_npos_stake_amount(&params, Some(LOCALNET_STAKE_AMOUNT));
        assert_eq!(stake_amount, npos.min_self_bond);
    }

    #[test]
    fn localnet_npos_validator_roster_and_quorum_match_peer_count() {
        use std::collections::BTreeSet;

        let temp = tempfile::tempdir().expect("tmp dir");
        let peer_count = NonZeroU16::new(7).expect("non-zero");
        let opts = LocalnetOptions {
            build_line: BuildLine::Iroha3,
            sora_profile: None,
            perf_profile: None,
            peers: peer_count,
            seed: Some("localnet-npos-quorum".to_owned()),
            bind_host: DEFAULT_BIND_HOST.to_owned(),
            public_host: DEFAULT_PUBLIC_HOST.to_owned(),
            base_api_port: 31080,
            base_p2p_port: 31337,
            out_dir: temp.path().to_path_buf(),
            extra_accounts: 0,
            assets: Vec::new(),
            block_time_ms: None,
            commit_time_ms: None,
            redundant_send_r: None,
            consensus_mode: SumeragiConsensusMode::Npos,
            next_consensus_mode: None,
            mode_activation_height: None,
        };

        generate_localnet(&opts, &mut BufWriter::new(Vec::new())).expect("generate localnet files");

        let manifest = localnet_genesis_for_opts(&opts);
        let validators: Vec<_> = manifest
            .instructions()
            .filter_map(|instruction| {
                instruction
                    .as_any()
                    .downcast_ref::<RegisterPublicLaneValidator>()
            })
            .collect();
        assert_eq!(
            validators.len(),
            usize::from(peer_count.get()),
            "expected one public-lane validator per peer"
        );

        let peers = build_peers(
            peer_count.get(),
            opts.seed.as_ref().map(String::as_bytes),
            opts.base_api_port,
            opts.base_p2p_port,
        );
        let expected: BTreeSet<_> = peers
            .iter()
            .map(|peer| AccountId::new(peer.public_key.clone()))
            .collect();
        let actual: BTreeSet<_> = validators
            .iter()
            .map(|register| register.validator.clone())
            .collect();
        assert_eq!(actual, expected, "validator roster should match peers");

        let peer_cfg: toml::Value = toml::from_str(
            &fs::read_to_string(temp.path().join("peer0.toml"))
                .expect("read generated peer config"),
        )
        .expect("parse peer config");
        let catalog = peer_cfg
            .get("nexus")
            .and_then(toml::Value::as_table)
            .and_then(|nexus| nexus.get("dataspace_catalog"))
            .and_then(toml::Value::as_array)
            .expect("nexus dataspace catalog");

        let fault_tolerance = localnet_dataspace_fault_tolerance(peer_count);
        let committee_size = fault_tolerance
            .checked_mul(3)
            .and_then(|value| value.checked_add(1))
            .expect("committee size");
        assert_eq!(
            committee_size,
            u32::from(peer_count.get()),
            "committee size should match peer count"
        );
        for entry in catalog {
            let entry = entry.as_table().expect("dataspace entry");
            assert_eq!(
                entry
                    .get("fault_tolerance")
                    .and_then(toml::Value::as_integer),
                Some(i64::from(fault_tolerance)),
                "fault tolerance should scale with peers"
            );
        }
    }

    #[test]
    fn block_time_override_mirrors_commit_time() {
        let temp = tempfile::tempdir().expect("tmp dir");
        let opts = LocalnetOptions {
            build_line: BuildLine::Iroha3,
            sora_profile: None,
            perf_profile: None,
            peers: NonZeroU16::new(2).expect("non-zero"),
            seed: Some("block-time-commit-default".to_owned()),
            bind_host: DEFAULT_BIND_HOST.to_owned(),
            public_host: DEFAULT_PUBLIC_HOST.to_owned(),
            base_api_port: 28080,
            base_p2p_port: 28337,
            out_dir: temp.path().to_path_buf(),
            extra_accounts: 0,
            assets: Vec::new(),
            block_time_ms: Some(1_000),
            commit_time_ms: None,
            redundant_send_r: None,
            consensus_mode: SumeragiConsensusMode::Npos,
            next_consensus_mode: None,
            mode_activation_height: None,
        };

        generate_localnet(&opts, &mut BufWriter::new(Vec::new())).expect("generate localnet files");

        let genesis_path = temp.path().join("genesis.json");
        let manifest = genesis_json_from_path(&genesis_path);
        let params = genesis_parameters(&manifest);
        assert_eq!(params.sumeragi().block_time_ms(), 1_000);
        assert_eq!(params.sumeragi().commit_time_ms(), 1_000);
    }

    #[test]
    fn npos_localnet_updates_redundant_send() {
        let temp = tempfile::tempdir().expect("tmp dir");
        let opts = LocalnetOptions {
            build_line: BuildLine::Iroha3,
            sora_profile: None,
            perf_profile: None,
            peers: NonZeroU16::new(2).expect("non-zero"),
            seed: Some("npos-timeouts".to_owned()),
            bind_host: DEFAULT_BIND_HOST.to_owned(),
            public_host: DEFAULT_PUBLIC_HOST.to_owned(),
            base_api_port: 38080,
            base_p2p_port: 38337,
            out_dir: temp.path().to_path_buf(),
            extra_accounts: 0,
            assets: Vec::new(),
            block_time_ms: Some(1_200),
            commit_time_ms: Some(1_600),
            redundant_send_r: Some(3),
            consensus_mode: SumeragiConsensusMode::Npos,
            next_consensus_mode: None,
            mode_activation_height: None,
        };

        generate_localnet(&opts, &mut BufWriter::new(Vec::new())).expect("generate localnet files");

        let genesis_path = temp.path().join("genesis.json");
        let manifest = genesis_json_from_path(&genesis_path);

        let params = genesis_parameters(&manifest);
        assert_eq!(params.sumeragi().block_time_ms(), 1_200);
        assert_eq!(params.sumeragi().commit_time_ms(), 1_600);
        let npos = params
            .custom()
            .get(&SumeragiNposParameters::parameter_id())
            .and_then(SumeragiNposParameters::from_custom_parameter)
            .expect("npos parameters must be present");
        assert_eq!(npos.redundant_send_r(), 3);
        assert_eq!(npos.seat_band_pct(), 100);
        assert_eq!(npos.min_self_bond(), 1);
    }

    #[test]
    fn npos_localnet_keeps_payload_for_fast_block_time() {
        let temp = tempfile::tempdir().expect("tmp dir");
        let opts = LocalnetOptions {
            build_line: BuildLine::Iroha3,
            sora_profile: None,
            perf_profile: None,
            peers: NonZeroU16::new(2).expect("non-zero"),
            seed: Some("npos-fast-timeouts".to_owned()),
            bind_host: DEFAULT_BIND_HOST.to_owned(),
            public_host: DEFAULT_PUBLIC_HOST.to_owned(),
            base_api_port: 38180,
            base_p2p_port: 38437,
            out_dir: temp.path().to_path_buf(),
            extra_accounts: 0,
            assets: Vec::new(),
            block_time_ms: Some(333),
            commit_time_ms: Some(667),
            redundant_send_r: None,
            consensus_mode: SumeragiConsensusMode::Npos,
            next_consensus_mode: None,
            mode_activation_height: None,
        };

        generate_localnet(&opts, &mut BufWriter::new(Vec::new())).expect("generate localnet files");

        let genesis_path = temp.path().join("genesis.json");
        let manifest = genesis_json_from_path(&genesis_path);
        let params = genesis_parameters(&manifest);
        assert_eq!(params.sumeragi().block_time_ms(), 333);
        assert_eq!(params.sumeragi().commit_time_ms(), 667);
        let npos = params
            .custom()
            .get(&SumeragiNposParameters::parameter_id())
            .and_then(SumeragiNposParameters::from_custom_parameter)
            .expect("npos parameters must be present");
        assert_eq!(npos.seat_band_pct(), 100);
        assert_eq!(npos.min_self_bond(), 1);
    }

    #[test]
    fn npos_localnet_includes_mode_activation() {
        let temp = tempfile::tempdir().expect("tmp dir");
        let opts = LocalnetOptions {
            build_line: BuildLine::Iroha2,
            sora_profile: None,
            perf_profile: None,
            peers: NonZeroU16::new(3).expect("non-zero"),
            seed: Some("npos-localnet".to_owned()),
            bind_host: DEFAULT_BIND_HOST.to_owned(),
            public_host: DEFAULT_PUBLIC_HOST.to_owned(),
            base_api_port: 18080,
            base_p2p_port: 17337,
            out_dir: temp.path().to_path_buf(),
            extra_accounts: 0,
            assets: Vec::new(),
            block_time_ms: None,
            commit_time_ms: None,
            redundant_send_r: None,
            consensus_mode: SumeragiConsensusMode::Permissioned,
            next_consensus_mode: Some(SumeragiConsensusMode::Npos),
            mode_activation_height: Some(5),
        };

        generate_localnet(&opts, &mut BufWriter::new(Vec::new()))
            .expect("generate npos localnet files");

        let genesis_path = temp.path().join("genesis.json");
        let manifest = genesis_json_from_path(&genesis_path);
        let params = genesis_parameters(&manifest);
        assert_eq!(
            params.sumeragi().next_mode(),
            Some(SumeragiConsensusMode::Npos),
            "expected NPoS next_mode in genesis"
        );
        assert_eq!(
            params.sumeragi().mode_activation_height(),
            Some(5),
            "expected mode_activation_height to be set in genesis"
        );

        let peer_cfg: toml::Value = toml::from_str(
            &fs::read_to_string(temp.path().join("peer0.toml"))
                .expect("read generated peer config"),
        )
        .expect("parse peer config");
        assert_eq!(
            peer_cfg
                .get("sumeragi")
                .and_then(toml::Value::as_table)
                .and_then(|s| s.get("consensus_mode"))
                .and_then(toml::Value::as_str),
            Some("permissioned")
        );
    }

    #[test]
    fn staged_cutover_preserves_permissioned_manifest_mode() {
        let temp = tempfile::tempdir().expect("tmp dir");
        let opts = LocalnetOptions {
            build_line: BuildLine::Iroha2,
            sora_profile: None,
            perf_profile: None,
            peers: NonZeroU16::new(3).expect("non-zero"),
            seed: Some("localnet-staged-cutover".to_owned()),
            bind_host: DEFAULT_BIND_HOST.to_owned(),
            public_host: DEFAULT_PUBLIC_HOST.to_owned(),
            base_api_port: 28080,
            base_p2p_port: 27337,
            out_dir: temp.path().to_path_buf(),
            extra_accounts: 0,
            assets: Vec::new(),
            block_time_ms: None,
            commit_time_ms: None,
            redundant_send_r: None,
            consensus_mode: SumeragiConsensusMode::Permissioned,
            next_consensus_mode: Some(SumeragiConsensusMode::Npos),
            mode_activation_height: Some(7),
        };

        generate_localnet(&opts, &mut BufWriter::new(Vec::new()))
            .expect("generate staged localnet");

        let genesis_path = temp.path().join("genesis.json");
        let manifest = genesis_json_from_path(&genesis_path);
        assert_eq!(
            genesis_consensus_mode(&manifest),
            Some(SumeragiConsensusMode::Permissioned),
            "manifest consensus_mode should reflect the current mode"
        );
        let params = genesis_parameters(&manifest);
        assert_eq!(
            params.sumeragi().next_mode(),
            Some(SumeragiConsensusMode::Npos),
            "manifest should preserve staged next_mode"
        );
        assert_eq!(
            params.sumeragi().mode_activation_height(),
            Some(7),
            "manifest should preserve mode_activation_height"
        );
    }

    #[test]
    fn staged_cutover_enables_nexus_in_peer_config() {
        let temp = tempfile::tempdir().expect("tmp dir");
        let opts = LocalnetOptions {
            build_line: BuildLine::Iroha2,
            sora_profile: None,
            perf_profile: None,
            peers: NonZeroU16::new(4).expect("non-zero"),
            seed: Some("localnet-staged-nexus".to_owned()),
            bind_host: DEFAULT_BIND_HOST.to_owned(),
            public_host: DEFAULT_PUBLIC_HOST.to_owned(),
            base_api_port: 28080,
            base_p2p_port: 27337,
            out_dir: temp.path().to_path_buf(),
            extra_accounts: 0,
            assets: Vec::new(),
            block_time_ms: None,
            commit_time_ms: None,
            redundant_send_r: None,
            consensus_mode: SumeragiConsensusMode::Permissioned,
            next_consensus_mode: Some(SumeragiConsensusMode::Npos),
            mode_activation_height: Some(9),
        };

        generate_localnet(&opts, &mut BufWriter::new(Vec::new()))
            .expect("generate staged localnet");

        let peer_cfg: toml::Value = toml::from_str(
            &fs::read_to_string(temp.path().join("peer0.toml"))
                .expect("read generated peer config"),
        )
        .expect("parse peer config");
        let nexus = peer_cfg
            .get("nexus")
            .and_then(toml::Value::as_table)
            .expect("nexus table");
        assert_eq!(
            nexus.get("enabled").and_then(toml::Value::as_bool),
            Some(true),
            "staged NPoS cutover should enable Nexus in peer config"
        );
        assert!(
            nexus.get("staking").is_some(),
            "staged NPoS cutover should configure Nexus staking"
        );
    }

    #[test]
    fn client_config_is_written_and_parsable() {
        let tmp = tempfile::tempdir().expect("tmp dir");
        let host =
            CanonicalHost::parse(DEFAULT_PUBLIC_HOST, "--public-host").expect("canonicalize host");
        write_client_config(tmp.path(), 8080, &host).expect("write client config");
        let contents =
            fs::read_to_string(tmp.path().join("client.toml")).expect("read client config");
        assert!(contents.contains("private_key = \"802620"));
        let value: toml::Value = toml::from_str(&contents).expect("parse client config");
        assert_eq!(
            value
                .get("torii_url")
                .and_then(toml::Value::as_str)
                .unwrap_or_default(),
            "http://127.0.0.1:8080/"
        );
        let account = value
            .get("account")
            .and_then(toml::Value::as_table)
            .expect("account table");
        assert_eq!(
            account.get("domain").and_then(toml::Value::as_str),
            Some(CLIENT_ACCOUNT_DOMAIN)
        );
    }

    #[test]
    fn canonical_host_formats_ipv6_literals_and_urls() {
        let host = CanonicalHost::parse("::1", "--public-host").expect("ipv6 host");
        let literal = host.addr_literal(8080);
        let body = literal::parse("addr", &literal).expect("parse addr literal");
        assert_eq!(body, "[::1]:8080");
        assert_eq!(host.url_host(), "[::1]");
    }

    #[test]
    fn canonical_host_lowercases_names() {
        let host = CanonicalHost::parse("LOCALHOST", "--public-host").expect("host");
        let literal = host.addr_literal(1337);
        let body = literal::parse("addr", &literal).expect("parse addr literal");
        assert_eq!(body, "localhost:1337");
        assert_eq!(host.url_host(), "localhost");
    }

    #[test]
    fn canonical_host_rejects_host_with_port() {
        let err = CanonicalHost::parse("127.0.0.1:8080", "--public-host")
            .expect_err("host with port should fail");
        assert!(err.to_string().contains("without a port"));
    }

    #[test]
    fn canonical_host_rejects_unbalanced_brackets() {
        let err = CanonicalHost::parse("[::1", "--public-host")
            .expect_err("missing closing bracket should fail");
        assert!(err.to_string().contains("unmatched"));
        let err = CanonicalHost::parse("::1]", "--public-host")
            .expect_err("missing opening bracket should fail");
        assert!(err.to_string().contains("unmatched"));
    }

    #[test]
    fn client_config_renders_ipv6_torii_url() {
        let tmp = tempfile::tempdir().expect("tmp dir");
        let host = CanonicalHost::parse("::1", "--public-host").expect("ipv6 host");
        write_client_config(tmp.path(), 8080, &host).expect("write client config");
        let contents =
            fs::read_to_string(tmp.path().join("client.toml")).expect("read client config");
        assert!(contents.contains("torii_url = \"http://[::1]:8080/\""));
    }

    #[test]
    fn build_line_controls_da_rbc_in_generated_artifacts() {
        fn assert_for_line(build_line: BuildLine, expected: bool) {
            let temp = tempfile::tempdir().expect("tmp dir");
            let consensus_mode = if build_line.is_iroha3() {
                SumeragiConsensusMode::Npos
            } else {
                SumeragiConsensusMode::Permissioned
            };
            let opts = LocalnetOptions {
                build_line,
                sora_profile: None,
                perf_profile: None,
                peers: NonZeroU16::new(2).expect("non-zero"),
                seed: Some(format!("da-rbc-{build_line:?}")),
                bind_host: DEFAULT_BIND_HOST.to_owned(),
                public_host: DEFAULT_PUBLIC_HOST.to_owned(),
                base_api_port: 19090,
                base_p2p_port: 23347,
                out_dir: temp.path().to_path_buf(),
                extra_accounts: 0,
                assets: Vec::new(),
                block_time_ms: None,
                commit_time_ms: None,
                redundant_send_r: None,
                consensus_mode,
                next_consensus_mode: None,
                mode_activation_height: None,
            };

            generate_localnet(&opts, &mut BufWriter::new(Vec::new()))
                .expect("generate localnet files");

            let peer_cfg: toml::Value = toml::from_str(
                &fs::read_to_string(temp.path().join("peer0.toml"))
                    .expect("read generated peer config"),
            )
            .expect("parse peer config");
            let sumeragi = peer_cfg
                .get("sumeragi")
                .and_then(toml::Value::as_table)
                .expect("sumeragi table");
            assert_eq!(
                sumeragi
                    .get("da")
                    .and_then(toml::Value::as_table)
                    .and_then(|da| da.get("enabled"))
                    .and_then(toml::Value::as_bool),
                Some(expected),
                "peer config should reflect build line"
            );
            let redundant = sumeragi
                .get("collectors")
                .and_then(toml::Value::as_table)
                .and_then(|collectors| collectors.get("redundant_send_r"))
                .and_then(toml::Value::as_integer)
                .and_then(|value| u8::try_from(value).ok());
            let expected_redundant = if expected {
                Some(default_redundant_send_r(opts.peers))
            } else {
                None
            };
            assert_eq!(
                redundant, expected_redundant,
                "localnet redundant send should track DA policy"
            );

            let manifest = genesis_json_from_path(&temp.path().join("genesis.json"));
            let params = genesis_parameters(&manifest);
            assert_eq!(
                params.sumeragi().da_enabled(),
                expected,
                "genesis da_enabled should track build line"
            );
            let expected_redundant = expected_redundant.unwrap_or_else(|| {
                Parameters::default()
                    .sumeragi()
                    .collectors_redundant_send_r()
            });
            assert_eq!(
                params.sumeragi().collectors_redundant_send_r(),
                expected_redundant,
                "genesis redundant send should track DA policy"
            );
        }

        assert_for_line(BuildLine::Iroha2, false);
        assert_for_line(BuildLine::Iroha3, true);
    }

    #[test]
    fn rejects_overflowing_port_ranges() {
        let opts = LocalnetOptions {
            build_line: BuildLine::Iroha3,
            sora_profile: None,
            perf_profile: None,
            peers: NonZeroU16::new(3).unwrap(),
            seed: None,
            bind_host: DEFAULT_BIND_HOST.to_string(),
            public_host: DEFAULT_PUBLIC_HOST.to_string(),
            base_api_port: u16::MAX,
            base_p2p_port: 10,
            out_dir: PathBuf::from("unused"),
            extra_accounts: 0,
            assets: Vec::new(),
            block_time_ms: None,
            commit_time_ms: None,
            redundant_send_r: None,
            consensus_mode: SumeragiConsensusMode::Npos,
            next_consensus_mode: None,
            mode_activation_height: None,
        };
        let mut sink = BufWriter::new(Vec::<u8>::new());
        let err = generate_localnet(&opts, &mut sink).expect_err("port overflow should fail");
        assert!(
            err.to_string().contains("base_api_port"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn rejects_overlapping_port_ranges() {
        let opts = LocalnetOptions {
            build_line: BuildLine::Iroha3,
            sora_profile: None,
            perf_profile: None,
            peers: NonZeroU16::new(2).unwrap(),
            seed: None,
            bind_host: DEFAULT_BIND_HOST.to_string(),
            public_host: DEFAULT_PUBLIC_HOST.to_string(),
            base_api_port: 1337,
            base_p2p_port: 1337,
            out_dir: PathBuf::from("unused"),
            extra_accounts: 0,
            assets: Vec::new(),
            block_time_ms: None,
            commit_time_ms: None,
            redundant_send_r: None,
            consensus_mode: SumeragiConsensusMode::Npos,
            next_consensus_mode: None,
            mode_activation_height: None,
        };
        let mut sink = BufWriter::new(Vec::<u8>::new());
        let err = generate_localnet(&opts, &mut sink).expect_err("overlapping ports should fail");
        assert!(
            err.to_string().contains("overlap"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn rejects_zero_ports() {
        let opts = LocalnetOptions {
            build_line: BuildLine::Iroha3,
            sora_profile: None,
            perf_profile: None,
            peers: NonZeroU16::new(1).unwrap(),
            seed: None,
            bind_host: DEFAULT_BIND_HOST.to_string(),
            public_host: DEFAULT_PUBLIC_HOST.to_string(),
            base_api_port: 0,
            base_p2p_port: 1000,
            out_dir: PathBuf::from("unused"),
            extra_accounts: 0,
            assets: Vec::new(),
            block_time_ms: None,
            commit_time_ms: None,
            redundant_send_r: None,
            consensus_mode: SumeragiConsensusMode::Npos,
            next_consensus_mode: None,
            mode_activation_height: None,
        };
        let mut sink = BufWriter::new(Vec::<u8>::new());
        let err = generate_localnet(&opts, &mut sink).expect_err("zero port should fail");
        assert!(
            err.to_string().contains("must be > 0"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn validate_localnet_options_rejects_missing_next_mode() {
        let opts = LocalnetOptions {
            build_line: BuildLine::Iroha2,
            sora_profile: None,
            perf_profile: None,
            peers: NonZeroU16::new(1).unwrap(),
            seed: None,
            bind_host: DEFAULT_BIND_HOST.to_string(),
            public_host: DEFAULT_PUBLIC_HOST.to_string(),
            base_api_port: 28080,
            base_p2p_port: 28337,
            out_dir: PathBuf::from("unused"),
            extra_accounts: 0,
            assets: Vec::new(),
            block_time_ms: None,
            commit_time_ms: None,
            redundant_send_r: None,
            consensus_mode: SumeragiConsensusMode::Permissioned,
            next_consensus_mode: None,
            mode_activation_height: Some(1),
        };
        let err = validate_localnet_options(&opts).expect_err("missing next mode should fail");
        assert!(
            err.to_string().contains("--next-consensus-mode"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn validate_localnet_options_rejects_zero_block_time() {
        let opts = LocalnetOptions {
            build_line: BuildLine::Iroha3,
            sora_profile: None,
            perf_profile: None,
            peers: NonZeroU16::new(1).unwrap(),
            seed: None,
            bind_host: DEFAULT_BIND_HOST.to_string(),
            public_host: DEFAULT_PUBLIC_HOST.to_string(),
            base_api_port: 28080,
            base_p2p_port: 28337,
            out_dir: PathBuf::from("unused"),
            extra_accounts: 0,
            assets: Vec::new(),
            block_time_ms: Some(0),
            commit_time_ms: None,
            redundant_send_r: None,
            consensus_mode: SumeragiConsensusMode::Npos,
            next_consensus_mode: None,
            mode_activation_height: None,
        };
        let err = validate_localnet_options(&opts).expect_err("zero block time should fail");
        assert!(
            err.to_string().contains("--block-time-ms"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn validate_localnet_options_rejects_zero_commit_time() {
        let opts = LocalnetOptions {
            build_line: BuildLine::Iroha3,
            sora_profile: None,
            perf_profile: None,
            peers: NonZeroU16::new(1).unwrap(),
            seed: None,
            bind_host: DEFAULT_BIND_HOST.to_string(),
            public_host: DEFAULT_PUBLIC_HOST.to_string(),
            base_api_port: 28080,
            base_p2p_port: 28337,
            out_dir: PathBuf::from("unused"),
            extra_accounts: 0,
            assets: Vec::new(),
            block_time_ms: None,
            commit_time_ms: Some(0),
            redundant_send_r: None,
            consensus_mode: SumeragiConsensusMode::Npos,
            next_consensus_mode: None,
            mode_activation_height: None,
        };
        let err = validate_localnet_options(&opts).expect_err("zero commit time should fail");
        assert!(
            err.to_string().contains("--commit-time-ms"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn validate_localnet_options_rejects_sora_profile_on_iroha2() {
        let opts = LocalnetOptions {
            build_line: BuildLine::Iroha2,
            sora_profile: Some(SoraProfile::Dataspace),
            perf_profile: None,
            peers: NonZeroU16::new(4).unwrap(),
            seed: None,
            bind_host: DEFAULT_BIND_HOST.to_string(),
            public_host: DEFAULT_PUBLIC_HOST.to_string(),
            base_api_port: 28080,
            base_p2p_port: 28337,
            out_dir: PathBuf::from("unused"),
            extra_accounts: 0,
            assets: Vec::new(),
            block_time_ms: None,
            commit_time_ms: None,
            redundant_send_r: None,
            consensus_mode: SumeragiConsensusMode::Permissioned,
            next_consensus_mode: None,
            mode_activation_height: None,
        };
        let err = validate_localnet_options(&opts).expect_err("sora profile should require iroha3");
        assert!(
            err.to_string().contains("--build-line iroha3"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn validate_localnet_options_rejects_sora_profile_with_too_few_peers() {
        let opts = LocalnetOptions {
            build_line: BuildLine::Iroha3,
            sora_profile: Some(SoraProfile::Dataspace),
            perf_profile: None,
            peers: NonZeroU16::new(3).unwrap(),
            seed: None,
            bind_host: DEFAULT_BIND_HOST.to_string(),
            public_host: DEFAULT_PUBLIC_HOST.to_string(),
            base_api_port: 28080,
            base_p2p_port: 28337,
            out_dir: PathBuf::from("unused"),
            extra_accounts: 0,
            assets: Vec::new(),
            block_time_ms: None,
            commit_time_ms: None,
            redundant_send_r: None,
            consensus_mode: SumeragiConsensusMode::Npos,
            next_consensus_mode: None,
            mode_activation_height: None,
        };
        let err =
            validate_localnet_options(&opts).expect_err("sora profile should enforce min peers");
        assert!(
            err.to_string().contains("at least 4 peers"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn validate_localnet_options_rejects_permissioned_on_sora_nexus() {
        let opts = LocalnetOptions {
            build_line: BuildLine::Iroha3,
            sora_profile: Some(SoraProfile::Nexus),
            perf_profile: None,
            peers: NonZeroU16::new(4).unwrap(),
            seed: None,
            bind_host: DEFAULT_BIND_HOST.to_string(),
            public_host: DEFAULT_PUBLIC_HOST.to_string(),
            base_api_port: 28080,
            base_p2p_port: 28337,
            out_dir: PathBuf::from("unused"),
            extra_accounts: 0,
            assets: Vec::new(),
            block_time_ms: None,
            commit_time_ms: None,
            redundant_send_r: None,
            consensus_mode: SumeragiConsensusMode::Permissioned,
            next_consensus_mode: None,
            mode_activation_height: None,
        };
        let err = validate_localnet_options(&opts).expect_err("sora nexus should require NPoS");
        assert!(
            err.to_string().contains("sora-profile"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn validate_localnet_options_rejects_permissioned_on_sora_dataspace() {
        let opts = LocalnetOptions {
            build_line: BuildLine::Iroha3,
            sora_profile: Some(SoraProfile::Dataspace),
            perf_profile: None,
            peers: NonZeroU16::new(4).unwrap(),
            seed: None,
            bind_host: DEFAULT_BIND_HOST.to_string(),
            public_host: DEFAULT_PUBLIC_HOST.to_string(),
            base_api_port: 28080,
            base_p2p_port: 28337,
            out_dir: PathBuf::from("unused"),
            extra_accounts: 0,
            assets: Vec::new(),
            block_time_ms: None,
            commit_time_ms: None,
            redundant_send_r: None,
            consensus_mode: SumeragiConsensusMode::Permissioned,
            next_consensus_mode: None,
            mode_activation_height: None,
        };
        let err = validate_localnet_options(&opts).expect_err("sora profile should require NPoS");
        assert!(
            err.to_string().contains("sora-profile"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn validate_localnet_options_allows_permissioned_on_iroha3() {
        let opts = LocalnetOptions {
            build_line: BuildLine::Iroha3,
            sora_profile: None,
            perf_profile: None,
            peers: NonZeroU16::new(1).unwrap(),
            seed: None,
            bind_host: DEFAULT_BIND_HOST.to_string(),
            public_host: DEFAULT_PUBLIC_HOST.to_string(),
            base_api_port: 28080,
            base_p2p_port: 28337,
            out_dir: PathBuf::from("unused"),
            extra_accounts: 0,
            assets: Vec::new(),
            block_time_ms: None,
            commit_time_ms: None,
            redundant_send_r: None,
            consensus_mode: SumeragiConsensusMode::Permissioned,
            next_consensus_mode: None,
            mode_activation_height: None,
        };
        validate_localnet_options(&opts).expect("permissioned should be allowed on Iroha3");
    }

    #[test]
    fn permissioned_iroha3_disables_nexus_in_peer_config() {
        let temp = tempfile::tempdir().expect("tmp dir");
        let opts = LocalnetOptions {
            build_line: BuildLine::Iroha3,
            sora_profile: None,
            perf_profile: None,
            peers: NonZeroU16::new(1).unwrap(),
            seed: Some("permissioned-iroha3".to_owned()),
            bind_host: DEFAULT_BIND_HOST.to_string(),
            public_host: DEFAULT_PUBLIC_HOST.to_string(),
            base_api_port: 28080,
            base_p2p_port: 28337,
            out_dir: temp.path().to_path_buf(),
            extra_accounts: 0,
            assets: Vec::new(),
            block_time_ms: None,
            commit_time_ms: None,
            redundant_send_r: None,
            consensus_mode: SumeragiConsensusMode::Permissioned,
            next_consensus_mode: None,
            mode_activation_height: None,
        };

        generate_localnet(&opts, &mut BufWriter::new(Vec::new()))
            .expect("generate permissioned iroha3 localnet");

        let peer_cfg: toml::Value = toml::from_str(
            &fs::read_to_string(temp.path().join("peer0.toml"))
                .expect("read generated peer config"),
        )
        .expect("parse peer config");
        let nexus_enabled = peer_cfg
            .get("nexus")
            .and_then(toml::Value::as_table)
            .and_then(|nexus| nexus.get("enabled"))
            .and_then(toml::Value::as_bool);
        assert_eq!(
            nexus_enabled,
            Some(false),
            "permissioned iroha3 localnet should disable nexus"
        );
    }

    #[test]
    fn npos_iroha3_without_sora_profile_enables_nexus_in_peer_config() {
        let temp = tempfile::tempdir().expect("tmp dir");
        let opts = LocalnetOptions {
            build_line: BuildLine::Iroha3,
            sora_profile: None,
            perf_profile: None,
            peers: NonZeroU16::new(1).unwrap(),
            seed: Some("npos-iroha3".to_owned()),
            bind_host: DEFAULT_BIND_HOST.to_string(),
            public_host: DEFAULT_PUBLIC_HOST.to_string(),
            base_api_port: 28080,
            base_p2p_port: 28337,
            out_dir: temp.path().to_path_buf(),
            extra_accounts: 0,
            assets: Vec::new(),
            block_time_ms: None,
            commit_time_ms: None,
            redundant_send_r: None,
            consensus_mode: SumeragiConsensusMode::Npos,
            next_consensus_mode: None,
            mode_activation_height: None,
        };

        generate_localnet(&opts, &mut BufWriter::new(Vec::new()))
            .expect("generate npos iroha3 localnet");

        let seed_bytes = opts.seed.as_ref().map(String::as_bytes);
        let (genesis_public_key, _) = generate_genesis_key_pair(seed_bytes, GENESIS_SEED);
        let gas_account_id = localnet_gas_account_id(&genesis_public_key);

        let peer_cfg: toml::Value = toml::from_str(
            &fs::read_to_string(temp.path().join("peer0.toml"))
                .expect("read generated peer config"),
        )
        .expect("parse peer config");
        let nexus = peer_cfg
            .get("nexus")
            .and_then(toml::Value::as_table)
            .expect("nexus table");
        let nexus_enabled = nexus.get("enabled").and_then(toml::Value::as_bool);
        assert_eq!(
            nexus_enabled,
            Some(true),
            "npos iroha3 localnet should enable nexus without a sora profile"
        );
        let gas_account_id = account_id_runtime_literal(&gas_account_id);
        let staking = nexus
            .get("staking")
            .and_then(toml::Value::as_table)
            .expect("nexus staking table");
        let expected_stake_asset_id = localnet_stake_asset_literal();
        assert_eq!(
            staking.get("stake_asset_id").and_then(toml::Value::as_str),
            Some(expected_stake_asset_id.as_str())
        );
        assert_eq!(
            staking
                .get("stake_escrow_account_id")
                .and_then(toml::Value::as_str),
            Some(gas_account_id.as_str())
        );
        assert_eq!(
            staking
                .get("slash_sink_account_id")
                .and_then(toml::Value::as_str),
            Some(gas_account_id.as_str())
        );
        let fees = nexus
            .get("fees")
            .and_then(toml::Value::as_table)
            .expect("nexus fees table");
        assert_eq!(
            fees.get("fee_asset_id").and_then(toml::Value::as_str),
            Some(expected_stake_asset_id.as_str())
        );
        assert_eq!(
            fees.get("fee_sink_account_id")
                .and_then(toml::Value::as_str),
            Some(gas_account_id.as_str())
        );

        let pipeline = peer_cfg
            .get("pipeline")
            .and_then(toml::Value::as_table)
            .expect("pipeline table");
        let pipeline_gas = pipeline
            .get("gas")
            .and_then(toml::Value::as_table)
            .expect("pipeline.gas table");
        assert_eq!(
            pipeline_gas
                .get("tech_account_id")
                .and_then(toml::Value::as_str),
            Some(gas_account_id.as_str())
        );
    }

    #[test]
    fn account_id_raw_string_parses_as_account_id() {
        let seed_bytes = Some(b"localnet-gas-parse".as_slice());
        let (genesis_public_key, _) = generate_genesis_key_pair(seed_bytes, GENESIS_SEED);
        let gas_account_id = localnet_gas_account_id(&genesis_public_key);
        let encoded = account_id_raw_string(&gas_account_id);
        let parsed = AccountId::parse_encoded(&encoded)
            .map(iroha_data_model::account::ParsedAccountId::into_account_id)
            .expect("account id parse");
        assert_eq!(parsed, gas_account_id);
    }

    #[test]
    fn account_id_runtime_literal_uses_encoded_literal() {
        let seed_bytes = Some(b"localnet-gas-runtime-literal".as_slice());
        let (genesis_public_key, _) = generate_genesis_key_pair(seed_bytes, GENESIS_SEED);
        let gas_account_id = localnet_gas_account_id(&genesis_public_key);
        let literal = account_id_runtime_literal(&gas_account_id);
        assert_eq!(literal, gas_account_id.to_string());
    }

    #[test]
    fn localnet_npos_bootstrap_does_not_re_register_genesis_account() {
        let temp = tempfile::tempdir().expect("tmp dir");
        let opts = LocalnetOptions {
            build_line: BuildLine::Iroha3,
            sora_profile: None,
            perf_profile: None,
            peers: NonZeroU16::new(4).expect("non-zero"),
            seed: Some("localnet-genesis-account-dedupe".to_owned()),
            bind_host: DEFAULT_BIND_HOST.to_owned(),
            public_host: DEFAULT_PUBLIC_HOST.to_owned(),
            base_api_port: 31080,
            base_p2p_port: 31337,
            out_dir: temp.path().to_path_buf(),
            extra_accounts: 0,
            assets: Vec::new(),
            block_time_ms: None,
            commit_time_ms: None,
            redundant_send_r: None,
            consensus_mode: SumeragiConsensusMode::Npos,
            next_consensus_mode: None,
            mode_activation_height: None,
        };

        let manifest = localnet_genesis_for_opts(&opts);
        let seed_bytes = opts.seed.as_ref().map(String::as_bytes);
        let (genesis_public_key, _) = generate_genesis_key_pair(seed_bytes, GENESIS_SEED);
        let genesis_account_id = AccountId::new(genesis_public_key.clone());
        let ivm_domain: DomainId = LOCALNET_IVM_DOMAIN.parse().expect("ivm domain");
        let ivm_genesis_registrations = manifest
            .instructions()
            .filter_map(|instruction| {
                instruction
                    .as_any()
                    .downcast_ref::<Register<Account>>()
                    .map(|register| (register.object.id.clone(), register.object.domain.clone()))
            })
            .filter(|(account_id, domain)| {
                account_id == &genesis_account_id && domain == &ivm_domain
            })
            .count();

        assert_eq!(
            ivm_genesis_registrations, 0,
            "expected NPoS bootstrap to avoid re-registering the genesis controller under ivm"
        );
    }

    #[test]
    fn validate_localnet_options_rejects_staged_cutover_on_iroha3() {
        let opts = LocalnetOptions {
            build_line: BuildLine::Iroha3,
            sora_profile: None,
            perf_profile: None,
            peers: NonZeroU16::new(1).unwrap(),
            seed: None,
            bind_host: DEFAULT_BIND_HOST.to_string(),
            public_host: DEFAULT_PUBLIC_HOST.to_string(),
            base_api_port: 28080,
            base_p2p_port: 28337,
            out_dir: PathBuf::from("unused"),
            extra_accounts: 0,
            assets: Vec::new(),
            block_time_ms: None,
            commit_time_ms: None,
            redundant_send_r: None,
            consensus_mode: SumeragiConsensusMode::Npos,
            next_consensus_mode: Some(SumeragiConsensusMode::Npos),
            mode_activation_height: Some(1),
        };
        let err = validate_localnet_options(&opts)
            .expect_err("staged cutover should be rejected on Iroha3");
        assert!(
            err.to_string().contains("staged consensus cutovers"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn relative_out_dir_paths_are_absolute_in_configs() {
        struct DirGuard {
            prev: PathBuf,
        }

        impl Drop for DirGuard {
            fn drop(&mut self) {
                env::set_current_dir(&self.prev).expect("restore current dir");
            }
        }

        let base = tempfile::tempdir().expect("tmp dir");
        let previous = env::current_dir().expect("current dir");
        env::set_current_dir(base.path()).expect("chdir into temp");
        let _guard = DirGuard { prev: previous };

        let opts = LocalnetOptions {
            build_line: BuildLine::Iroha3,
            sora_profile: None,
            perf_profile: None,
            peers: NonZeroU16::new(1).unwrap(),
            seed: Some("absolute-paths".to_owned()),
            bind_host: DEFAULT_BIND_HOST.to_owned(),
            public_host: DEFAULT_PUBLIC_HOST.to_owned(),
            base_api_port: 19081,
            base_p2p_port: 23338,
            out_dir: PathBuf::from("localnet"),
            extra_accounts: 0,
            assets: Vec::new(),
            block_time_ms: None,
            commit_time_ms: None,
            redundant_send_r: None,
            consensus_mode: SumeragiConsensusMode::Npos,
            next_consensus_mode: None,
            mode_activation_height: None,
        };

        generate_localnet(&opts, &mut BufWriter::new(Vec::new()))
            .expect("generate localnet with relative path");

        let out_dir = base.path().join("localnet");
        let peer_cfg = fs::read_to_string(out_dir.join("peer0.toml")).expect("read peer config");
        let parsed: toml::Value = toml::from_str(&peer_cfg).expect("parse peer config");
        let genesis_path = parsed
            .get("genesis")
            .and_then(toml::Value::as_table)
            .and_then(|t| t.get("file"))
            .and_then(toml::Value::as_str)
            .expect("genesis path");
        let kura_path = parsed
            .get("kura")
            .and_then(toml::Value::as_table)
            .and_then(|t| t.get("store_dir"))
            .and_then(toml::Value::as_str)
            .expect("kura store");
        let tiered_state = parsed
            .get("tiered_state")
            .and_then(toml::Value::as_table)
            .expect("tiered_state table");
        let tiered_root = tiered_state
            .get("cold_store_root")
            .and_then(toml::Value::as_str)
            .expect("tiered_state cold_store_root");
        let da_root = tiered_state
            .get("da_store_root")
            .and_then(toml::Value::as_str)
            .expect("tiered_state da_store_root");
        assert!(
            Path::new(genesis_path).is_absolute(),
            "genesis path should be absolute"
        );
        assert!(
            Path::new(kura_path).is_absolute(),
            "kura store path should be absolute"
        );
        assert!(
            Path::new(tiered_root).is_absolute(),
            "tiered_state cold_store_root should be absolute"
        );
        assert!(
            Path::new(da_root).is_absolute(),
            "tiered_state da_store_root should be absolute"
        );
    }

    #[cfg(unix)]
    #[test]
    fn start_and_stop_scripts_are_executable() {
        let temp = tempfile::tempdir().expect("tmp dir");
        write_scripts(temp.path(), 1, BuildLine::Iroha3, false).expect("write scripts");

        let start_path = temp.path().join("start.sh");
        let stop_path = temp.path().join("stop.sh");
        let start_mode = fs::metadata(&start_path)
            .expect("start metadata")
            .permissions()
            .mode();
        let stop_mode = fs::metadata(&stop_path)
            .expect("stop metadata")
            .permissions()
            .mode();
        assert_ne!(
            start_mode & 0o111,
            0,
            "start script should be marked executable"
        );
        assert_ne!(
            stop_mode & 0o111,
            0,
            "stop script should be marked executable"
        );

        let start_contents = fs::read_to_string(&start_path).expect("read start script");
        let (debug_path, release_path) = default_irohad_bin_paths();
        let expected_debug = format!("DEFAULT_IROHAD_BIN_DEBUG=\"{}\"", debug_path.display());
        let expected_release = format!("DEFAULT_IROHAD_BIN_RELEASE=\"{}\"", release_path.display());
        assert!(
            start_contents.lines().any(|line| line == expected_debug),
            "start script should set debug default"
        );
        assert!(
            start_contents.lines().any(|line| line == expected_release),
            "start script should set release default"
        );
        assert!(
            start_contents
                .lines()
                .any(|line| line == "export IROHA_BUILD_LINE=\"iroha3\""),
            "start script should export build line"
        );
    }

    #[cfg(unix)]
    #[test]
    fn start_script_includes_sora_flag_when_enabled() {
        let temp = tempfile::tempdir().expect("tmp dir");
        write_scripts(temp.path(), 1, BuildLine::Iroha3, true).expect("write scripts");
        let start_contents =
            fs::read_to_string(temp.path().join("start.sh")).expect("read start script");
        assert!(
            start_contents.contains(" --sora --config "),
            "start script should include --sora when profile enabled"
        );
    }

    #[test]
    fn copy_rans_tables_writes_seed_table() {
        let temp = tempfile::tempdir().expect("tmp dir");
        copy_rans_tables(temp.path()).expect("copy rANS tables");
        let seed_path = temp
            .path()
            .join("codec")
            .join("rans")
            .join("tables")
            .join("rans_seed0.toml");
        let bytes = fs::read(seed_path).expect("read rANS seed table");
        assert_eq!(bytes, RANS_SEED0_TABLE);
    }
}
