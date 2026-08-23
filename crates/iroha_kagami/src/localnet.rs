//! Generate a bare-metal local network configuration (genesis, peer configs, scripts).
use crate::{
    Outcome, RunArgs,
    genesis::{
        ConsensusPolicy, generate_default, profile::known_chain_discriminant_for_chain_id,
        validate_consensus_mode,
    },
    tui,
};
use clap::{Args as ClapArgs, ValueEnum};
use color_eyre::eyre::{Result, WrapErr as _, eyre};
use iroha_config::{base::toml::TomlSource, parameters::actual};
use iroha_core::zk::confidential_v2;
use iroha_crypto::{ExposedPrivateKey, Hash, HashOf, KeyPair};
#[cfg(test)]
use iroha_data_model::isi::UnregisterBox;
use iroha_data_model::{
    account::address::ChainDiscriminantGuard,
    alias_setup::{
        AccountAliasName, AccountAliasRoleV1, AccountProvisionV1, AliasAccountIntentV1,
        AliasDataSpaceIntentV1, AliasDomainIntentV1, AliasIntentV1, AliasLeaseAcquisitionV1,
        AliasQuoteGuardV1, AliasSetupPlanRequestV1, ResolvedAccountAliasV1, ResolvedDataSpaceV1,
        ResolvedDomainV1,
    },
    asset::AssetDefinitionAlias,
    block::{
        BlockHeader,
        consensus_v2::{MAX_VALIDATORS_PER_HEIGHT, is_valid_committee_size},
    },
    da::commitment::DaProofPolicyBundle,
    isi::{
        GrantBox, RegisterBox, RevokeBox, SetAssetDefinitionAlias,
        alias_setup::EnsureAlias,
        nexus::{
            ActivateFeeSponsorProgramRevision, CreateFeeSponsorProgram,
            EnrollFeeSponsorBeneficiary, FundFeeSponsorProgram, StageFeeSponsorProgramRevision,
        },
        staking::{ActivatePublicLaneValidator, RegisterPublicLaneValidator},
        verifying_keys,
    },
    nexus::{
        DataSpaceId, FeeSponsorAssetBudget, FeeSponsorEligibility,
        FeeSponsorNativeInstructionSelector, FeeSponsorProgram, FeeSponsorProgramId,
        FeeSponsorProgramRevision, FeeSponsorRule, FeeSponsorRuleEffect, FeeSponsorRuleSelector,
    },
    parameter::{
        custom::{CustomParameter, CustomParameterId},
        system::{SumeragiConsensusMode, SumeragiNposParameters},
    },
    peer::PeerId,
    prelude::*,
    proof::{VerifyingKeyId, VerifyingKeyRecord},
};
use iroha_executor_data_model::permission::{
    account::{
        AccountAliasPermissionScope, CanManageAccountAlias, CanRegisterAccount,
        CanResolveAccountAlias,
    },
    asset::CanMintAssetWithDefinition,
    governance::CanEnactGovernance,
    nexus::{
        CanEnrollFeeSponsorProgram, CanPublishSpaceDirectoryManifest,
        CanPublishSpaceDirectoryManifestForAccountDomain,
    },
    query::CanReadRestrictedDataspace,
};
use iroha_genesis::{
    GenesisBuilder, GenesisTopologyEntry, RawGenesisTransaction, SIGNED_GENESIS_MAX_BYTES_V1,
    init_instruction_registry, read_signed_genesis, validate_genesis_manifest_json,
};
use iroha_primitives::addr::{SocketAddr, SocketAddrHost};
use iroha_primitives::json::Json;
use iroha_primitives::numeric::{Numeric, Quantity};
use iroha_test_samples::ALICE_ID;
#[cfg(test)]
use iroha_test_samples::REAL_GENESIS_ACCOUNT_KEYPAIR;
use rand::{TryRngCore as _, rngs::OsRng};
use std::{
    collections::BTreeSet,
    env,
    fs::{self, File},
    io::{BufWriter, Write},
    net::{Ipv4Addr, Ipv6Addr},
    num::{NonZeroU16, NonZeroU64},
    path::{Path, PathBuf},
};
use zeroize::{Zeroize as _, Zeroizing};
/// User-facing options for generating a bare-metal localnet.
#[derive(Debug, Clone)]
pub struct LocalnetOptions {
    /// Optional Sora profile selector (multi-lane / dataspace defaults).
    pub sora_profile: Option<SoraProfile>,
    /// Optional localnet performance profile (throughput presets).
    pub perf_profile: Option<LocalnetPerfProfile>,
    /// Number of peers to create (deterministic ordering, minimum four).
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
    /// Additional asset specs to register and optionally mint on top of the built-in localnet asset set.
    pub assets: Vec<AssetSpec>,
    /// Optional signed-genesis block cadence override in milliseconds.
    /// If unset, localnet uses a one-second cadence.
    pub block_cadence_ms: Option<u64>,
    /// Consensus mode to commit in signed genesis.
    pub consensus_mode: SumeragiConsensusMode,
}
impl Drop for LocalnetOptions {
    fn drop(&mut self) {
        if let Some(seed) = self.seed.as_mut() {
            seed.zeroize();
        }
    }
}
/// Asset definition plus optional minting target for sample generation.
#[derive(Debug, Clone)]
pub struct AssetSpec {
    /// Canonical asset definition ID (unprefixed Base58 address).
    pub id: String,
    /// Human-readable display name for the asset definition.
    pub name: String,
    /// Optional leased alias binding to attach after registration.
    pub alias: Option<String>,
    /// Account that should own the asset definition after genesis completes.
    pub owned_by: AccountId,
    /// Account that should receive the minted supply.
    pub mint_to: AccountId,
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
    /// State Bank of Pakistan restricted dataspace defaults.
    PrivateSbp,
    /// Central Bank of the UAE restricted dataspace defaults.
    PrivateCbuae,
    /// Public dataspace (Nexus) defaults.
    Nexus,
}
impl SoraProfile {
    fn consensus_policy(self) -> ConsensusPolicy {
        match self {
            SoraProfile::Dataspace
            | SoraProfile::PrivateSbp
            | SoraProfile::PrivateCbuae
            | SoraProfile::Nexus => ConsensusPolicy::PublicDataspace,
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
    block_cadence_ms: u64,
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
            block_cadence_ms: 1_000,
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
/// Canonical restricted-dataspace presets supported by the localnet generator.
#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PrivateDataspaceArg {
    /// State Bank of Pakistan dataspace (id 10, lane 3).
    Sbp,
    /// Central Bank of the UAE dataspace (id 12, lane 4).
    Cbuae,
}
impl From<SoraProfileArg> for SoraProfile {
    fn from(value: SoraProfileArg) -> Self {
        match value {
            SoraProfileArg::Dataspace => SoraProfile::Dataspace,
            SoraProfileArg::Nexus => SoraProfile::Nexus,
        }
    }
}
fn resolve_sora_profile(
    profile: Option<SoraProfileArg>,
    private_dataspace: Option<PrivateDataspaceArg>,
) -> Result<Option<SoraProfile>> {
    match (profile, private_dataspace) {
        (None, None) => Ok(None),
        (Some(profile), None) => Ok(Some(profile.into())),
        (Some(SoraProfileArg::Dataspace), Some(PrivateDataspaceArg::Sbp)) => {
            Ok(Some(SoraProfile::PrivateSbp))
        }
        (Some(SoraProfileArg::Dataspace), Some(PrivateDataspaceArg::Cbuae)) => {
            Ok(Some(SoraProfile::PrivateCbuae))
        }
        (Some(SoraProfileArg::Nexus) | None, Some(_)) => Err(eyre!(
            "`--private-dataspace` requires `--sora-profile dataspace`"
        )),
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
pub(crate) fn consensus_mode_label(mode: SumeragiConsensusMode) -> &'static str {
    match mode {
        SumeragiConsensusMode::Permissioned => "permissioned",
        SumeragiConsensusMode::Npos => "npos",
    }
}
const DEFAULT_CHAIN_ID: &str = "00000000-0000-0000-0000-000000000000";
pub(crate) const GENESIS_SEED: &[u8; 7] = b"genesis";
const SORANET_TRANSPORT_SEED_DOMAIN: &[u8] = b"iroha:kagami:localnet:soranet-transport:v1|";
const STREAMING_IDENTITY_SEED_DOMAIN: &[u8] = b"iroha:kagami:localnet:streaming-identity:v1|";
/// Serialized reducer command queue capacity for generated localnets.
const LOCALNET_SUMERAGI_QUEUE_COMMANDS: usize = 8_192;
/// Certified-body and block-sync outer-ingress capacity for generated localnets.
///
/// This inherits the production 5N+3H geometry at the protocol's maximum
/// validator roster: five owners per validator and three per authenticated
/// non-validator source. Identityless ingress owns no partition.
const LOCALNET_SUMERAGI_QUEUE_BODIES: usize =
    iroha_config::parameters::defaults::sumeragi::QUEUE_BODY_CAPACITY.get();
/// Authenticated non-validator fair-ingress lanes for generated localnets.
const LOCALNET_SUMERAGI_AUTHENTICATED_NON_VALIDATOR_SOURCES: usize =
    iroha_config::parameters::defaults::sumeragi::QUEUE_AUTHENTICATED_NON_VALIDATOR_SOURCE_CAPACITY
        .get();
/// Total P2P connection bound for generated localnets.
///
/// This admits the other thirty validators in the maximum legal committee plus
/// two authenticated non-validator sources. Lifecycle ownership is bounded
/// separately by the configured fair-ingress source population.
const LOCALNET_MAX_TOTAL_CONNECTIONS: usize =
    MAX_VALIDATORS_PER_HEIGHT - 1 + LOCALNET_SUMERAGI_AUTHENTICATED_NON_VALIDATOR_SOURCES;
/// Per-source canonical outer-ingress wire bytes for generated localnets.
const LOCALNET_SUMERAGI_QUEUE_BODY_SOURCE_BYTES: usize =
    iroha_config::parameters::defaults::sumeragi::QUEUE_BODY_SOURCE_BYTES.get();
/// Payload-chunk ingress and orphan-buffer capacity for generated localnets.
const LOCALNET_SUMERAGI_QUEUE_CHUNKS: usize = 4_096;
/// Reconstructed bodies waiting for reducer delivery in generated localnets.
const LOCALNET_SUMERAGI_QUEUE_READY_BODIES: usize = 256;
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
fn localnet_sumeragi_body_bytes(validator_count: usize) -> Result<usize> {
    if validator_count > MAX_VALIDATORS_PER_HEIGHT {
        return Err(eyre!(
            "localnet validator count {validator_count} exceeds the Sumeragi v2 protocol maximum of {MAX_VALIDATORS_PER_HEIGHT}"
        ));
    }
    if !is_valid_committee_size(validator_count) {
        return Err(eyre!(
            "localnet validator count {validator_count} is not an exact Sumeragi v2 3f+1 committee in the supported range 4..={MAX_VALIDATORS_PER_HEIGHT}"
        ));
    }
    let effect_work_capacity = (LOCALNET_SUMERAGI_QUEUE_COMMANDS
        / iroha_config::parameters::defaults::sumeragi::V2_RUNTIME_COMPLETION_RESERVE_DIVISOR)
        .max(1);
    actual::sumeragi_v2_lifecycle_capacity_geometry(
        validator_count,
        effect_work_capacity,
        LOCALNET_SUMERAGI_QUEUE_BODIES,
        LOCALNET_SUMERAGI_AUTHENTICATED_NON_VALIDATOR_SOURCES,
    )
    .wrap_err("localnet Sumeragi lifecycle capacity geometry is inadmissible")?;
    let shared_ownership_capacity = actual::sumeragi_v2_exact_output_shared_ownership_capacity(
        effect_work_capacity,
        LOCALNET_SUMERAGI_QUEUE_BODIES,
    )
    .wrap_err("localnet Sumeragi exact-output shared capacity overflowed")?;
    actual::validate_sumeragi_v2_exact_output_geometry(
        shared_ownership_capacity,
        LOCALNET_MAX_TOTAL_CONNECTIONS,
    )
    .wrap_err("localnet Sumeragi exact-output geometry is inadmissible")?;
    actual::sumeragi_v2_body_ingress_required_byte_capacity(
        validator_count,
        LOCALNET_SUMERAGI_AUTHENTICATED_NON_VALIDATOR_SOURCES,
        LOCALNET_SUMERAGI_QUEUE_BODY_SOURCE_BYTES,
    )
    .ok_or_else(|| eyre!("localnet Sumeragi outer-ingress wire-byte capacity overflow"))
}
/// Transaction gossip cadence for 1s localnet pipelines (ms).
const LOCALNET_TX_GOSSIP_PERIOD_FAST_MS: u64 = 100;
/// Transaction gossip resend ticks for 1s localnet pipelines.
const LOCALNET_TX_GOSSIP_RESEND_TICKS_FAST: u32 = 1;
/// Tx gossip frame cap for localnets so large public transactions still fit.
const LOCALNET_MAX_FRAME_BYTES_TX_GOSSIP_NEXUS: usize = 1_048_576;
/// Base P2P frame cap for generated localnets.
///
/// Localnets use the production 17 MiB cap because certified-body recovery can
/// carry the full recommended 16 MiB payload plus its manifest, relay wrapper,
/// and AEAD overhead. A smaller development-only cap can deadlock block sync.
const LOCALNET_MAX_FRAME_BYTES: usize =
    iroha_config::parameters::defaults::network::MAX_FRAME_BYTES.get();
/// Consensus message frame cap for generated localnets.
const LOCALNET_MAX_FRAME_BYTES_CONSENSUS: usize =
    iroha_config::parameters::defaults::network::MAX_FRAME_BYTES_CONSENSUS.get();
/// Block-sync frame cap for generated localnets.
const LOCALNET_MAX_FRAME_BYTES_BLOCK_SYNC: usize =
    iroha_config::parameters::defaults::network::MAX_FRAME_BYTES_BLOCK_SYNC.get();
/// Control-message frame cap for generated localnets.
///
/// This carries maximal consensus-safety proposals and timeout certificates.
const LOCALNET_MAX_FRAME_BYTES_CONTROL: usize =
    iroha_config::parameters::defaults::network::MAX_FRAME_BYTES_CONTROL.get();
/// Peer-gossip frame cap for generated localnets.
const LOCALNET_MAX_FRAME_BYTES_PEER_GOSSIP: usize = 65_536;
/// Health-check frame cap for generated localnets.
const LOCALNET_MAX_FRAME_BYTES_HEALTH: usize = 32_768;
/// Miscellaneous frame cap for generated localnets.
const LOCALNET_MAX_FRAME_BYTES_OTHER: usize = 131_072;
/// Default listener host for generated P2P and Torii services.
pub const DEFAULT_BIND_HOST: &str = "0.0.0.0";
/// Default advertised host for generated peers and client config.
pub const DEFAULT_PUBLIC_HOST: &str = "127.0.0.1";
/// Default total pipeline time (ms) injected for localnet when not overridden.
const LOCALNET_PIPELINE_TIME_MS: u64 = 1_000;
/// Default queue capacity for localnet (safe-by-default).
///
/// This value intentionally trades peak stress throughput for bounded memory
/// usage when consensus stalls or clients oversubmit.
const LOCALNET_QUEUE_CAPACITY: usize = 20_000;
/// Queue capacity used for perf-profile localnets.
///
/// The queue also enforces a retained-byte budget, which is the binding limit for
/// high-throughput localnet bursts. Keep the count cap only high enough to avoid
/// count-based rejection before the byte guard engages; larger values preallocate
/// fixed queue slots that sit mostly empty under the byte budget.
const LOCALNET_PERF_QUEUE_CAPACITY: usize = 4_096;
/// Runtime proposal cap used by perf-profile localnets.
///
/// The on-chain block parameter remains 10k for throughput targets, but local
/// development nodes should not assemble thousand-transaction RS16 proposals
/// while the queue is saturated.
const LOCALNET_PERF_RUNTIME_BLOCK_MAX_TRANSACTIONS: usize = 256;
/// Default transaction TTL in the queue for localnet (ms).
const LOCALNET_QUEUE_TTL_MS: u64 = 600_000;
/// Default lane TEU capacity for localnet scheduling (raises per-block budget).
const LOCALNET_LANE_TEU_CAPACITY: u32 = 50_000_000;
/// Default IVM gas budget per block for Taira/localnet stress profiles.
const LOCALNET_IVM_GAS_LIMIT_PER_BLOCK: u64 = 50_000_000;
/// Default IVM gas price for localnet fee assets.
const LOCALNET_IVM_GAS_UNITS_PER_GAS: u64 = 1;
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
/// Torii request body cap emitted explicitly in localnet configs.
const LOCALNET_TORII_MAX_CONTENT_LEN: u64 =
    iroha_config::parameters::defaults::torii::MAX_CONTENT_LEN.0;
/// Torii pre-auth allowlist to keep localnet CLI traffic from tripping bans.
const LOCALNET_PREAUTH_ALLOW_CIDRS: [&str; 2] = ["127.0.0.0/8", "::1/128"];
/// Exact Torii transport sources trusted for internal localnet reads and routing.
const LOCALNET_INTERNAL_API_TRUSTED_CIDRS: [&str; 2] = ["127.0.0.1/32", "::1/128"];
/// Default localnet telemetry toggle (mirrors config defaults).
const LOCALNET_TELEMETRY_ENABLED: bool = true;
/// Default localnet telemetry profile (mirrors config defaults).
const LOCALNET_TELEMETRY_PROFILE: &str = "extended";
/// Minimum peer count for generated localnets.
const LOCALNET_MIN_PEERS: u16 = 4;
/// Divisor applied to derive the localnet NPoS aggregator fallback timeout.
/// Keep this at 1 so aggregators do not time out before quorum on fast pipelines.
/// Default max transactions per block for localnet (targets 10k TPS).
const LOCALNET_BLOCK_MAX_TRANSACTIONS: u64 = 10_000;
/// Default stake bonded per localnet validator (raised to meet min_self_bond).
const LOCALNET_STAKE_AMOUNT: u64 = 10_000;
const LOCALNET_FAUCET_AUTHORITY_BALANCE: u64 = 1_000_000_000;
const LOCALNET_FEE_SPONSOR_PROGRAM_NAME: &str = "default";
const LOCALNET_FEE_SPONSOR_VAULT_BALANCE: u64 = 100_000_000;
const LOCALNET_FEE_SPONSOR_PER_TRANSACTION: u64 = 1_000_000;
const LOCALNET_FEE_SPONSOR_PER_BLOCK: u64 = 10_000_000;
const LOCALNET_FEE_SPONSOR_PER_PROGRAM_EPOCH: u64 = 100_000_000;
const LOCALNET_FEE_SPONSOR_PER_BENEFICIARY_EPOCH: u64 = 50_000_000;
const LOCALNET_FEE_SPONSOR_RESERVE_FLOOR: u64 = 10_000_000;
const LOCALNET_FEE_SPONSOR_EPOCH_BLOCKS: u64 = 3_600;
const LOCALNET_ONBOARDING_CREDENTIAL_ID: &str = "local-dev";
const LOCALNET_OPERATOR_ALIAS: &str = "operator@wonderland.universal";
const LOCALNET_ALIAS_SETUP_INTENT_FILE: &str = "alias-setup.intent.json";
const LOCALNET_ALIAS_SETUP_PAYER_BALANCE: u64 = 10;
const LOCALNET_ALIAS_SETUP_POLICY_VERSION: u16 = 1;
const LOCALNET_RUNTIME_DIRECTORY: &str = "runtime";
const LOCALNET_OPERATOR_SIGNER_KEY_FILE: &str = "operator-signer.key";
const LOCALNET_ONBOARDING_SIGNER_KEY_FILE: &str = "onboarding-signer.key";
const LOCALNET_ONBOARDING_TOKEN_FILE: &str = "onboarding.token";
const LOCALNET_FAUCET_AMOUNT: &str = "25000";
const LOCALNET_FAUCET_POW_DIFFICULTY_BITS: i64 = 8;
const LOCALNET_FAUCET_POW_SCRYPT_LOG_N: i64 = 13;
const LOCALNET_FAUCET_POW_SCRYPT_R: i64 = 8;
const LOCALNET_FAUCET_POW_SCRYPT_P: i64 = 1;
const LOCALNET_FAUCET_POW_MAX_ANCHOR_AGE_BLOCKS: i64 = 6;
const LOCALNET_FAUCET_POW_ADAPTIVE_LOOKBACK_BLOCKS: i64 = 64;
const LOCALNET_FAUCET_POW_ADAPTIVE_CLAIMS_PER_EXTRA_BIT: i64 = 4;
const LOCALNET_FAUCET_POW_ADAPTIVE_MAX_EXTRA_BITS: i64 = 2;
const LOCALNET_PRIVATE_SNS_LEASE_PAYMENT: &str = "0.5";
const LOCALNET_NEXUS_DOMAIN: &str = "nexus.universal";
const LOCALNET_IVM_DOMAIN: &str = "ivm.universal";
const LOCALNET_UNIVERSAL_DOMAIN: &str = "universal.universal";
const LOCALNET_STAKE_ASSET_NAME: &str = "xor";
const LOCALNET_SAMPLE_ASSET_DOMAIN: &str = "wonderland.universal";
pub(crate) const LOCALNET_SAMPLE_ASSET_NAME: &str = "sample";
const LOCALNET_REQUESTED_ASSET_INITIAL_QUANTITY: u64 = 1_000_000_000;
const LOCALNET_KAGEMUSHA_ASSET_ID: &str = "7EAD8EFYUx1aVKZPUU1fyKvr8dF1";
const LOCALNET_KAGEMUSHA_ASSET_NAME: &str = "usd";
const LOCALNET_KAGEMUSHA_ASSET_ALIAS: &str = "usd#wonderland.universal";
const LOCALNET_KAGEMUSHA_INITIAL_QUANTITY: u64 = 100;
const LOCALNET_GAS_ACCOUNT_SEED: &[u8] = b"localnet-gas-account";
/// Minimum faucet reserve before startup auto-mints a replenishment.
const LOCALNET_FEE_ASSET_RESERVE_MIN: u128 = 1_000_000_000_000_000_000_000_000;
/// Target faucet reserve restored by the startup wrapper when the floor is crossed.
const LOCALNET_FEE_ASSET_RESERVE_TARGET: u128 = 10_000_000_000_000_000_000_000_000;
/// Default localnet client TTL (ms) to keep stress submissions from expiring prematurely.
const LOCALNET_CLIENT_TTL_MS: u64 = 600_000;
/// Default localnet client status timeout (ms); must stay <= TTL.
const LOCALNET_CLIENT_STATUS_TIMEOUT_MS: u64 = 300_000;
/// Default Kura fsync mode for localnet (performance-oriented).
const LOCALNET_KURA_FSYNC_MODE: &str = "batched";
/// Aggregate Nexus storage cap for each disposable localnet peer (1 GiB).
///
/// Production nodes derive a filesystem-aware budget with reserved headroom. A generated
/// localnet owns short-lived storage under its output directory, so an explicit small cap avoids
/// applying that host-wide production policy to a throwaway network.
const LOCALNET_NEXUS_STORAGE_BUDGET_BYTES: u64 = 1024 * 1024 * 1024;
/// Ed25519 signature batch size for perf-profile localnets (0 disables batching).
const LOCALNET_SIGNATURE_BATCH_MAX_ED25519: usize = 64;
/// Logger filter for perf-profile localnets to avoid per-transaction log floods.
const LOCALNET_PERF_LOGGER_FILTER: &str = "info,iroha_torii::routing=warn";
const RANS_SEED0_TABLE: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../codec/rans/tables/rans_seed0.toml"
));
const LOCALNET_RANS_TABLE_RELATIVE_PATH: &str = "codec/rans/tables/rans_seed0.toml";
fn localnet_dataspace_fault_tolerance(peers: NonZeroU16) -> u32 {
    let peers = u32::from(peers.get());
    let fault_tolerance = peers.saturating_sub(1) / 3;
    fault_tolerance.max(1)
}
const LOCALNET_PAYNET_ALIAS_DATASPACE_ID: u64 = 10;
const LOCALNET_CBUAE_ALIAS_DATASPACE_ID: u64 = 12;
const LOCALNET_PAYNET_ALIAS_LANE_INDEX: u32 = 3;
const LOCALNET_CBUAE_ALIAS_LANE_INDEX: u32 = 4;
const LOCALNET_NEXUS_ALIAS_LANE_COUNT: i64 = 5;
#[cfg(test)]
const LOCALNET_PAYNET_ALIAS_LANE_COUNT: i64 = 4;
#[derive(Debug, Clone, Copy)]
struct PrivateDataspaceRoute {
    matcher: &'static str,
    description: &'static str,
}
#[derive(Debug, Clone, Copy)]
struct PrivateDataspaceSpec {
    alias: &'static str,
    id: u64,
    lane_index: u32,
    dataspace_description: &'static str,
    lane_description: &'static str,
    account_routes: &'static [PrivateDataspaceRoute],
    transfer_routes: &'static [PrivateDataspaceRoute],
}
const PAYNET_ACCOUNT_ROUTES: &[PrivateDataspaceRoute] = &[
    PrivateDataspaceRoute {
        matcher: "*@paynet",
        description: "Route *@paynet account traffic to paynet lane",
    },
    PrivateDataspaceRoute {
        matcher: "*@mibank.paynet",
        description: "Route *@mibank.paynet account traffic to paynet lane",
    },
];
const SBP_ACCOUNT_ROUTES: &[PrivateDataspaceRoute] = &[
    PrivateDataspaceRoute {
        matcher: "*@sbp",
        description: "Route SBP authority traffic to the SBP lane",
    },
    PrivateDataspaceRoute {
        matcher: "*@hbl.sbp",
        description: "Route HBL alias-scope traffic inside the SBP dataspace to the SBP lane",
    },
    PrivateDataspaceRoute {
        matcher: "*@ubl.sbp",
        description: "Route UBL alias-scope traffic inside the SBP dataspace to the SBP lane",
    },
];
const SBP_TRANSFER_ROUTES: &[PrivateDataspaceRoute] = &[
    PrivateDataspaceRoute {
        matcher: "transfer::asset@sbp",
        description: "Route transfer destination alias scope sbp to the SBP lane",
    },
    PrivateDataspaceRoute {
        matcher: "transfer::asset@hbl.sbp",
        description: "Route transfer destination alias scope hbl.sbp inside the SBP dataspace to the SBP lane",
    },
    PrivateDataspaceRoute {
        matcher: "transfer::asset@ubl.sbp",
        description: "Route transfer destination alias scope ubl.sbp inside the SBP dataspace to the SBP lane",
    },
];
const CBUAE_ACCOUNT_ROUTES: &[PrivateDataspaceRoute] = &[PrivateDataspaceRoute {
    matcher: "*@cbuae",
    description: "Route CBUAE authority traffic to the CBUAE lane",
}];
const CBUAE_TRANSFER_ROUTES: &[PrivateDataspaceRoute] = &[PrivateDataspaceRoute {
    matcher: "transfer::asset@cbuae",
    description: "Route transfer destination alias scope cbuae to the CBUAE lane",
}];
const SBP_BOOTSTRAP_DOMAINS: &[&str] = &["hbl.sbp", "ubl.sbp"];
fn private_dataspace_spec(sora_profile: Option<SoraProfile>) -> Option<PrivateDataspaceSpec> {
    match sora_profile? {
        SoraProfile::Dataspace => Some(PrivateDataspaceSpec {
            alias: "paynet",
            id: LOCALNET_PAYNET_ALIAS_DATASPACE_ID,
            lane_index: LOCALNET_PAYNET_ALIAS_LANE_INDEX,
            dataspace_description: "Private central-bank digital-currency dataspace",
            lane_description: "Private central-bank digital-currency dataspace lane",
            account_routes: PAYNET_ACCOUNT_ROUTES,
            transfer_routes: &[],
        }),
        SoraProfile::PrivateSbp => Some(PrivateDataspaceSpec {
            alias: "sbp",
            id: LOCALNET_PAYNET_ALIAS_DATASPACE_ID,
            lane_index: LOCALNET_PAYNET_ALIAS_LANE_INDEX,
            dataspace_description: "State Bank of Pakistan dataspace",
            lane_description: "State Bank of Pakistan private lane",
            account_routes: SBP_ACCOUNT_ROUTES,
            transfer_routes: SBP_TRANSFER_ROUTES,
        }),
        SoraProfile::PrivateCbuae => Some(PrivateDataspaceSpec {
            alias: "cbuae",
            id: LOCALNET_CBUAE_ALIAS_DATASPACE_ID,
            lane_index: LOCALNET_CBUAE_ALIAS_LANE_INDEX,
            dataspace_description: "CBUAE dataspace",
            lane_description: "CBUAE private lane",
            account_routes: CBUAE_ACCOUNT_ROUTES,
            transfer_routes: CBUAE_TRANSFER_ROUTES,
        }),
        SoraProfile::Nexus => None,
    }
}
fn localnet_uses_alias_multilane_catalog(sora_profile: Option<SoraProfile>) -> bool {
    matches!(
        sora_profile,
        Some(
            SoraProfile::Nexus
                | SoraProfile::Dataspace
                | SoraProfile::PrivateSbp
                | SoraProfile::PrivateCbuae
        )
    )
}
fn canonical_asset_definition_id(domain: &str, name: &str) -> AssetDefinitionId {
    AssetDefinitionId::derive_from_components(
        DomainId::parse_fully_qualified(domain)
            .expect("static asset definition domain must remain valid"),
        name.parse()
            .expect("static asset definition name must remain valid"),
    )
}
pub(crate) fn canonical_asset_definition_literal(domain: &str, name: &str) -> String {
    canonical_asset_definition_id(domain, name).canonical_address()
}
fn localnet_stake_asset_definition_id() -> AssetDefinitionId {
    canonical_asset_definition_id(LOCALNET_NEXUS_DOMAIN, LOCALNET_STAKE_ASSET_NAME)
}
fn localnet_stake_asset_literal() -> String {
    canonical_asset_definition_literal(LOCALNET_NEXUS_DOMAIN, LOCALNET_STAKE_ASSET_NAME)
}
fn localnet_fee_asset_definition_id() -> AssetDefinitionId {
    canonical_asset_definition_id(LOCALNET_UNIVERSAL_DOMAIN, LOCALNET_STAKE_ASSET_NAME)
}
fn localnet_fee_asset_literal() -> String {
    canonical_asset_definition_literal(LOCALNET_UNIVERSAL_DOMAIN, LOCALNET_STAKE_ASSET_NAME)
}
fn localnet_fee_sponsor_program_id(sponsor: &AccountId) -> FeeSponsorProgramId {
    FeeSponsorProgramId::new(
        sponsor.clone(),
        LOCALNET_FEE_SPONSOR_PROGRAM_NAME
            .parse()
            .expect("static localnet fee sponsor program name must parse"),
    )
}
fn localnet_fee_sponsor_revision(
    program_id: FeeSponsorProgramId,
    fee_asset_id: AssetDefinitionId,
) -> FeeSponsorProgramRevision {
    let native = |wire_id: &str| {
        FeeSponsorRuleSelector::NativeInstruction(FeeSponsorNativeInstructionSelector {
            wire_id: wire_id.to_owned(),
            asset_definition_id: None,
        })
    };
    FeeSponsorProgramRevision {
        program_id,
        revision: 1,
        eligibility: FeeSponsorEligibility::EnrolledOnly,
        rules: vec![FeeSponsorRule {
            id: "onboarding"
                .parse()
                .expect("static localnet sponsor rule name must parse"),
            effect: FeeSponsorRuleEffect::Allow,
            selectors: vec![
                native(RegisterBox::WIRE_ID),
                native(GrantBox::WIRE_ID),
                native("iroha.alias.ensure"),
                native("nexus::EnrollFeeSponsorBeneficiary"),
                native(std::any::type_name::<
                    iroha_data_model::isi::space_directory::PublishSpaceDirectoryManifest,
                >()),
                native("iroha.account.alias.primary.compare_and_set"),
            ],
        }],
        asset_budgets: vec![FeeSponsorAssetBudget {
            asset_definition_id: fee_asset_id,
            per_transaction: Quantity::from(LOCALNET_FEE_SPONSOR_PER_TRANSACTION),
            per_block: Quantity::from(LOCALNET_FEE_SPONSOR_PER_BLOCK),
            per_program_epoch: Quantity::from(LOCALNET_FEE_SPONSOR_PER_PROGRAM_EPOCH),
            per_beneficiary_epoch: Quantity::from(LOCALNET_FEE_SPONSOR_PER_BENEFICIARY_EPOCH),
            reserve_floor: Quantity::from(LOCALNET_FEE_SPONSOR_RESERVE_FLOOR),
            epoch_length_blocks: NonZeroU64::new(LOCALNET_FEE_SPONSOR_EPOCH_BLOCKS)
                .expect("static sponsor epoch length must be non-zero"),
        }],
    }
}
const LOCALNET_FEE_ZK_VK_BACKEND: &str = "halo2/ipa";
const LOCALNET_FEE_ZK_VK_UNSHIELD_NAME: &str = "vk_unshield";
const LOCALNET_FEE_ASSET_SCALE: u32 = 9;
fn localnet_fee_vk_unshield_id() -> VerifyingKeyId {
    VerifyingKeyId::new(LOCALNET_FEE_ZK_VK_BACKEND, LOCALNET_FEE_ZK_VK_UNSHIELD_NAME)
}
fn localnet_confidential_fee_vk_record(name: &str, version: u32) -> Result<VerifyingKeyRecord> {
    match name {
        LOCALNET_FEE_ZK_VK_UNSHIELD_NAME => {
            confidential_v2::confidential_unshield_v2_vk_record(name, version)
                .map_err(|error| eyre!(error))
        }
        _ => Err(eyre!("unknown localnet confidential verifier name: {name}")),
    }
}
fn localnet_confidential_fee_vk_registrations() -> Result<[(VerifyingKeyId, VerifyingKeyRecord); 1]>
{
    Ok([(
        localnet_fee_vk_unshield_id(),
        localnet_confidential_fee_vk_record(LOCALNET_FEE_ZK_VK_UNSHIELD_NAME, 2)?,
    )])
}
fn localnet_sample_asset_literal() -> String {
    canonical_asset_definition_literal(LOCALNET_SAMPLE_ASSET_DOMAIN, LOCALNET_SAMPLE_ASSET_NAME)
}
fn localnet_kagemusha_asset_literal() -> String {
    LOCALNET_KAGEMUSHA_ASSET_ID.to_owned()
}
fn localnet_kagemusha_asset_spec_for_client(client_account_id: &AccountId) -> AssetSpec {
    AssetSpec {
        id: localnet_kagemusha_asset_literal(),
        name: LOCALNET_KAGEMUSHA_ASSET_NAME.to_owned(),
        alias: Some(LOCALNET_KAGEMUSHA_ASSET_ALIAS.to_owned()),
        owned_by: client_account_id.clone(),
        mint_to: client_account_id.clone(),
        quantity: LOCALNET_KAGEMUSHA_INITIAL_QUANTITY,
    }
}
fn requested_localnet_asset_spec(asset_definition_id: &str) -> Result<AssetSpec> {
    let id = asset_definition_id.trim();
    if id.is_empty() {
        return Err(eyre!("asset definition id must not be empty"));
    }
    AssetDefinitionId::parse_address_literal(id)
        .wrap_err_with(|| format!("invalid asset definition id `{id}`"))?;
    let client_account_id = localnet_client_account_id();
    Ok(AssetSpec {
        id: id.to_owned(),
        name: format!("Localnet asset {id}"),
        alias: None,
        owned_by: client_account_id.clone(),
        mint_to: client_account_id,
        quantity: LOCALNET_REQUESTED_ASSET_INITIAL_QUANTITY,
    })
}
#[cfg(test)]
fn effective_localnet_assets(extra_assets: &[AssetSpec]) -> Vec<AssetSpec> {
    effective_localnet_assets_for_client(extra_assets, &localnet_client_account_id())
}
fn effective_localnet_assets_for_client(
    extra_assets: &[AssetSpec],
    client_account_id: &AccountId,
) -> Vec<AssetSpec> {
    let mut assets = Vec::with_capacity(extra_assets.len() + 1);
    let mut seen_asset_ids = BTreeSet::new();
    let built_in = localnet_kagemusha_asset_spec_for_client(client_account_id);
    seen_asset_ids.insert(built_in.id.clone());
    assets.push(built_in);
    for asset in extra_assets {
        if seen_asset_ids.insert(asset.id.clone()) {
            let mut asset = asset.clone();
            let default_client = localnet_client_account_id();
            if asset.owned_by == default_client {
                asset.owned_by = client_account_id.clone();
            }
            if asset.mint_to == default_client {
                asset.mint_to = client_account_id.clone();
            }
            assets.push(asset);
        }
    }
    assets
}
/// Generate a bare-metal local network (no Docker): genesis, per-peer configs, start/stop scripts.
#[derive(ClapArgs, Debug, Clone)]
pub struct Args {
    /// Number of peers to generate (minimum four).
    #[arg(long, short, value_name = "COUNT", default_value_t = NonZeroU16::new(4).unwrap())]
    peers: NonZeroU16,
    /// Optional UTF-8 seed for deterministic keys.
    #[arg(long, short)]
    seed: Option<String>,
    /// Generate every private key from a fresh OS-random, process-local seed.
    ///
    /// The seed is never accepted through argv, written to the generated
    /// bundle, or printed. This mode is intended for real first-release
    /// custody; use `--seed` only for reproducible development fixtures.
    #[arg(long, conflicts_with = "seed")]
    fresh_random_keys: bool,
    /// Canonical chain identifier written into genesis, peer configs, and the client config.
    #[arg(long, value_name = "CHAIN_ID", default_value = DEFAULT_CHAIN_ID)]
    chain_id: String,
    /// Enable Sora profile defaults; `nexus` enforces public dataspace rules (NPoS).
    /// Requires at least 4 peers.
    #[arg(long, value_enum, value_name = "PROFILE")]
    sora_profile: Option<SoraProfileArg>,
    /// Select an exact restricted dataspace preset for the `dataspace` Sora profile.
    #[arg(long, value_enum, value_name = "DATASPACE", requires = "sora_profile")]
    private_dataspace: Option<PrivateDataspaceArg>,
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
    /// Register the optional sample asset and mint to the default account.
    /// The built-in Kagemusha asset is always emitted.
    #[arg(long, default_value_t = false)]
    sample_asset: bool,
    /// Register additional asset definition IDs owned by the generated client signer.
    /// Repeat the flag to register more than one asset definition. A localnet reserve is minted
    /// to the generated client signer for each requested asset definition.
    #[arg(long, value_name = "ASSET_DEFINITION_ID")]
    asset_definition_id: Vec<String>,
    /// Override the immutable signed block cadence in milliseconds.
    /// Leave unset to use the one-second localnet cadence.
    #[arg(long, value_name = "MILLISECONDS", value_parser = clap::value_parser!(u64).range(1..))]
    block_cadence_ms: Option<u64>,
    /// Consensus mode to emit in genesis/configs.
    /// Defaults to `permissioned` for generic localnets.
    /// Sora profile localnets and perf profiles require `npos`.
    /// Sora profile localnets require `npos` because the global merge ledger is NPoS.
    #[arg(long, value_enum, value_name = "MODE")]
    consensus_mode: Option<ConsensusModeArg>,
}
fn fresh_localnet_seed() -> Result<String> {
    let mut raw = [0_u8; 32];
    OsRng
        .try_fill_bytes(&mut raw)
        .wrap_err("failed to obtain OS entropy for fresh localnet custody")?;
    let encoded = hex::encode(raw);
    raw.zeroize();
    Ok(encoded)
}
fn resolve_requested_consensus_mode(
    explicit_mode: Option<ConsensusModeArg>,
    perf_profile: Option<LocalnetPerfProfile>,
) -> SumeragiConsensusMode {
    explicit_mode.map_or_else(
        || {
            perf_profile.map_or(SumeragiConsensusMode::Permissioned, |profile| {
                profile.consensus_mode()
            })
        },
        SumeragiConsensusMode::from,
    )
}
impl<T: Write> RunArgs<T> for Args {
    fn run(self, writer: &mut BufWriter<T>) -> Outcome {
        let sora_profile = resolve_sora_profile(self.sora_profile, self.private_dataspace)?;
        let perf_profile = self.perf_profile.map(LocalnetPerfProfile::from);
        let consensus_mode = resolve_requested_consensus_mode(self.consensus_mode, perf_profile);
        let mut assets = if self.sample_asset {
            vec![AssetSpec {
                id: localnet_sample_asset_literal(),
                name: LOCALNET_SAMPLE_ASSET_NAME.to_owned(),
                alias: None,
                owned_by: ALICE_ID.clone(),
                mint_to: ALICE_ID.clone(),
                quantity: 100,
            }]
        } else {
            vec![]
        };
        for asset_definition_id in self.asset_definition_id {
            assets.push(requested_localnet_asset_spec(&asset_definition_id)?);
        }
        let fresh_random_keys = self.fresh_random_keys;
        let chain_id = self.chain_id;
        let seed = if fresh_random_keys {
            Some(fresh_localnet_seed()?)
        } else {
            self.seed
        };
        let mut opts = LocalnetOptions {
            sora_profile,
            perf_profile,
            peers: self.peers,
            seed,
            bind_host: self.bind_host,
            public_host: self.public_host,
            base_api_port: self.base_api_port,
            base_p2p_port: self.base_p2p_port,
            out_dir: self.out_dir,
            extra_accounts: self.extra_accounts,
            assets,
            consensus_mode,
            block_cadence_ms: self.block_cadence_ms,
        };
        let outcome = generate_localnet_inner(&opts, writer, fresh_random_keys, Some(&chain_id));
        if fresh_random_keys && let Some(seed) = opts.seed.as_mut() {
            seed.zeroize();
        }
        outcome
    }
}
struct Peer {
    public_key: iroha_crypto::PublicKey,
    private_key: iroha_crypto::ExposedPrivateKey,
    soranet_transport_public_key: iroha_crypto::PublicKey,
    soranet_transport_private_key: iroha_crypto::ExposedPrivateKey,
    streaming_public_key: iroha_crypto::PublicKey,
    streaming_private_key: iroha_crypto::ExposedPrivateKey,
    bls_public_key: iroha_crypto::PublicKey,
    bls_pop: Vec<u8>,
    api_port: u16,
    p2p_port: u16,
}
struct LocalnetPeerStoragePaths {
    kura: PathBuf,
    state: PathBuf,
    soracloud_runtime: PathBuf,
    tiered_state: PathBuf,
    da_store: PathBuf,
    streaming_sessions: PathBuf,
    streaming_soranet_spool: PathBuf,
    streaming_soravpn_spool: PathBuf,
    soranet_ticket_revocations: PathBuf,
    torii: PathBuf,
    torii_da_replay_cache: PathBuf,
    torii_da_manifests: PathBuf,
    sorafs: PathBuf,
    sorafs_por: PathBuf,
}
impl LocalnetPeerStoragePaths {
    fn new(out_dir: &Path, peer_index: usize) -> Self {
        let state = out_dir.join("state").join(format!("peer{peer_index}"));
        let streaming = state.join("streaming");
        let torii = state.join("torii");
        let sorafs = state.join("sorafs");
        Self {
            kura: out_dir.join("storage").join(format!("peer{peer_index}")),
            soracloud_runtime: state.join("soracloud_runtime"),
            tiered_state: state.join("tiered_state"),
            da_store: state.join("da_wsv_snapshots"),
            streaming_sessions: streaming.clone(),
            streaming_soranet_spool: streaming.join("soranet_routes"),
            streaming_soravpn_spool: state.join("streaming").join("soravpn_routes"),
            soranet_ticket_revocations: state.join("soranet").join("ticket_revocations.norito"),
            torii_da_replay_cache: torii.join("da_replay"),
            torii_da_manifests: torii.join("da_manifests"),
            torii,
            sorafs_por: sorafs.join("por"),
            sorafs,
            state,
        }
    }
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
    generate_localnet_inner(opts, writer, false, None)
}
#[allow(clippy::too_many_lines)]
fn validate_localnet_options(opts: &LocalnetOptions) -> Result<ResolvedHosts> {
    if let Some(block_ms) = opts.block_cadence_ms
        && block_ms == 0
    {
        return Err(eyre!("`--block-cadence-ms` must be greater than zero"));
    }
    let validator_count = usize::from(opts.peers.get());
    if opts.peers.get() < LOCALNET_MIN_PEERS {
        return Err(eyre!(
            "`--peers` must be at least {LOCALNET_MIN_PEERS} so generated localnets exercise a representative revision-4 committee with mandatory RS16 data availability"
        ));
    }
    if validator_count > MAX_VALIDATORS_PER_HEIGHT {
        return Err(eyre!(
            "`--peers` ({validator_count}) exceeds the Sumeragi v2 protocol maximum validator roster of {MAX_VALIDATORS_PER_HEIGHT}"
        ));
    }
    if !is_valid_committee_size(validator_count) {
        return Err(eyre!(
            "`--peers` ({validator_count}) must form an exact Sumeragi v2 3f+1 validator committee in the supported range 4..={MAX_VALIDATORS_PER_HEIGHT}"
        ));
    }
    if let Some(perf_spec) = opts.perf_profile.map(LocalnetPerfProfile::spec) {
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
    validate_consensus_mode(opts.consensus_mode, consensus_policy)?;
    let bind = CanonicalHost::parse(&opts.bind_host, "--bind-host")?;
    let public = CanonicalHost::parse(&opts.public_host, "--public-host")?;
    Ok(ResolvedHosts { bind, public })
}
fn localnet_uses_npos(consensus_mode: SumeragiConsensusMode) -> bool {
    matches!(consensus_mode, SumeragiConsensusMode::Npos)
}
#[derive(Debug, Clone, Copy)]
struct LocalnetTxGossipOverrides {
    period_ms: u64,
    resend_ticks: u32,
}
fn localnet_gas_account_id(genesis_public_key: &iroha_crypto::PublicKey) -> Result<AccountId> {
    let gas_key_pair = iroha_crypto::KeyPair::try_from_seed(
        genesis_public_key
            .to_string()
            .bytes()
            .chain(LOCALNET_GAS_ACCOUNT_SEED.iter().copied())
            .collect(),
        iroha_crypto::Algorithm::default(),
    )
    .wrap_err("failed to derive localnet gas account key pair")?;
    Ok(AccountId::new(gas_key_pair.public_key().clone()))
}
fn account_id_raw_string(account_id: &AccountId) -> String {
    account_id.to_string()
}
fn account_id_runtime_literal(account_id: &AccountId, chain_discriminant: Option<u16>) -> String {
    chain_discriminant.map_or_else(
        || account_id_raw_string(account_id),
        |discriminant| {
            account_id
                .to_i105_for_discriminant(discriminant)
                .expect("known localnet account id must render for requested chain discriminant")
        },
    )
}
fn account_literal_for_chain_discriminant(raw: &str, chain_discriminant: u16) -> String {
    let account_id = AccountId::parse_encoded(raw)
        .map(iroha_data_model::account::ParsedAccountId::into_account_id)
        .expect("known account literal must parse");
    account_id_runtime_literal(&account_id, Some(chain_discriminant))
}
#[cfg(test)]
fn localnet_client_account_literal(chain_discriminant: Option<u16>) -> String {
    account_id_runtime_literal(&localnet_client_account_id(), chain_discriminant)
}
#[allow(clippy::too_many_lines)]
fn generate_localnet_inner<T: Write>(
    opts: &LocalnetOptions,
    writer: &mut BufWriter<T>,
    redact_seed_metadata: bool,
    chain_id: Option<&str>,
) -> Outcome {
    init_instruction_registry();
    let hosts = validate_localnet_options(opts)?;
    validate_port_ranges(opts.peers, opts.base_api_port, opts.base_p2p_port)?;
    if redact_seed_metadata {
        crate::secure_fs::prepare_empty_private_directory(&opts.out_dir)
            .wrap_err("prepare fresh localnet private output directory")?;
    } else {
        fs::create_dir_all(&opts.out_dir)
            .wrap_err("failed to create output directory for localnet")?;
    }
    let out_dir = fs::canonicalize(&opts.out_dir).wrap_err_with(|| {
        format!(
            "failed to canonicalize output directory for localnet: {}",
            opts.out_dir.display()
        )
    })?;
    write_localnet_gitignore(&out_dir)?;
    tui::status("Copying rANS tables");
    let rans_tables_path = copy_rans_tables(&out_dir)?;
    let seed_bytes = opts.seed.as_ref().map(String::as_bytes);
    let chain_id = resolve_localnet_chain_id(chain_id)?;
    let chain_discriminant = known_chain_discriminant_for_chain_id(&chain_id);
    // Keep every account literal and permission payload emitted by this localnet
    // generation scoped to the selected chain.  Applying the guard only while
    // rendering/parsing peer configs is too late: the genesis and alias intent
    // have already serialized account IDs by then.
    let _chain_discriminant = chain_discriminant.map(ChainDiscriminantGuard::enter);
    let peers = build_peers(
        opts.peers.get(),
        seed_bytes,
        opts.base_api_port,
        opts.base_p2p_port,
    )
    .wrap_err("failed to generate localnet peer keys")?;
    let lane_manifest_directory =
        write_localnet_lane_manifests(&out_dir, opts.sora_profile, &peers, chain_discriminant)?;
    let client_identity = localnet_ephemeral_identity(seed_bytes, b"operator-root")?;
    let onboarding_identity = localnet_ephemeral_identity(seed_bytes, b"onboarding-root")?;
    let runtime_bundle =
        write_localnet_runtime_bundle(&out_dir, &client_identity, &onboarding_identity)?;
    let sumeragi_body_bytes = localnet_sumeragi_body_bytes(peers.len())?;
    tui::status("Generating genesis manifest");
    let npos_bootstrap = localnet_uses_npos(opts.consensus_mode);
    let sora_profile_enabled = opts.sora_profile.is_some();
    let mcp_enabled = sora_profile_enabled;
    let perf_spec = opts.perf_profile.map(LocalnetPerfProfile::spec);
    let queue_capacity = if perf_spec.is_some() {
        LOCALNET_PERF_QUEUE_CAPACITY
    } else {
        LOCALNET_QUEUE_CAPACITY
    };
    let logger_filter = perf_spec.map(|_| LOCALNET_PERF_LOGGER_FILTER);
    let signature_batch_max_ed25519 = perf_spec.map(|_| LOCALNET_SIGNATURE_BATCH_MAX_ED25519);
    let runtime_block_max_transactions =
        perf_spec.map(|_| LOCALNET_PERF_RUNTIME_BLOCK_MAX_TRANSACTIONS);
    // Sora profiles and NPoS bootstrap emit a dataspace catalog. Nexus itself is mandatory.
    let dataspace_fault_tolerance = (opts.sora_profile.is_some() || npos_bootstrap)
        .then(|| localnet_dataspace_fault_tolerance(opts.peers));
    let block_cadence_override = opts
        .block_cadence_ms
        .or_else(|| perf_spec.map(|spec| spec.block_cadence_ms));
    let block_cadence_ms = block_cadence_override.unwrap_or(LOCALNET_PIPELINE_TIME_MS);
    let tx_gossip_overrides = localnet_tx_gossip_overrides(block_cadence_ms);
    let block_max_transactions = perf_spec.map_or(LOCALNET_BLOCK_MAX_TRANSACTIONS, |spec| {
        spec.block_max_transactions
    });
    let requested_stake_amount = perf_spec.map(|spec| spec.stake_amount);
    let (genesis_public_key, genesis_private) = generate_genesis_key_pair(seed_bytes, GENESIS_SEED)
        .wrap_err("failed to generate localnet genesis key pair")?;
    let genesis_account_id = AccountId::new(genesis_public_key.clone());
    let assets = effective_localnet_assets_for_client(&opts.assets, &client_identity.account_id);
    let gas_account_id = if npos_bootstrap {
        Some(localnet_gas_account_id(&genesis_public_key)?)
    } else {
        None
    };
    let mut genesis = generate_raw_genesis(&genesis_public_key, opts.consensus_mode, &chain_id)?;
    if opts.extra_accounts > 0 || !assets.is_empty() {
        genesis = extend_genesis(
            genesis,
            &genesis_account_id,
            seed_bytes,
            opts.extra_accounts,
            &assets,
        )?;
    }
    genesis = append_localnet_service_accounts(
        genesis,
        &[&client_identity.account_id, &onboarding_identity.account_id],
    );
    genesis = append_localnet_alias_fee_bootstrap(
        genesis,
        &genesis_account_id,
        &client_identity.account_id,
        &onboarding_identity.account_id,
    );
    genesis = apply_parameter_overrides(
        genesis,
        Some(block_cadence_ms),
        block_max_transactions,
        opts.consensus_mode,
    );
    genesis = append_localnet_contract_permissions_for_client(
        genesis,
        &genesis_account_id,
        &client_identity.account_id,
    );
    genesis = append_localnet_onboarding_permissions(genesis, &onboarding_identity.account_id)?;
    genesis = append_peer_pop(genesis, &peers);
    if npos_bootstrap {
        let gas_account_id = gas_account_id
            .as_ref()
            .expect("gas account id required for NPoS bootstrap");
        let stake_amount =
            localnet_npos_stake_amount(&genesis.effective_parameters()?, requested_stake_amount);
        genesis = append_localnet_npos_bootstrap_for_services(
            genesis,
            &peers,
            gas_account_id,
            &stake_amount,
            opts.sora_profile,
            &genesis_account_id,
            &client_identity.account_id,
            &onboarding_identity.account_id,
        )?;
        genesis = append_private_dataspace_genesis_bootstrap_for_client(
            genesis,
            opts.sora_profile,
            &genesis_account_id,
            &client_identity.account_id,
        )?;
    }
    genesis = apply_localnet_crypto_overrides(genesis, npos_bootstrap);
    let alias_setup_request =
        localnet_alias_setup_request(&genesis_account_id, &client_identity.account_id)?;
    let append_alias_setup_to_current_transaction = npos_bootstrap
        && matches!(
            opts.sora_profile,
            Some(SoraProfile::PrivateSbp | SoraProfile::PrivateCbuae)
        );
    genesis = append_localnet_alias_setup(
        genesis,
        &alias_setup_request,
        append_alias_setup_to_current_transaction,
    );
    let alias_setup_intent_path =
        write_localnet_alias_setup_intent(&out_dir, &alias_setup_request)?;
    let genesis_json_path = out_dir.join("genesis.json");
    let genesis_signed_path = out_dir.join("genesis.signed.nrt");
    let genesis_expected_hash_path = out_dir.join(GENESIS_EXPECTED_HASH_FILE);
    let gas_account_id = gas_account_id
        .as_ref()
        .map(|account_id| account_id_runtime_literal(account_id, chain_discriminant));
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
    let client_account_literal = client_identity.account_literal(chain_discriminant);
    // Runtime signer authorities must use the localnet chain's canonical
    // address prefix whenever the chain has a known discriminant.
    let operator_account_literal = client_identity.account_literal(chain_discriminant);
    let onboarding_account_literal = onboarding_identity.account_literal(chain_discriminant);
    let bootstrap_peer = peers
        .first()
        .expect("localnet always has at least one peer");
    let bootstrap_paths = LocalnetPeerStoragePaths::new(&out_dir, 0);
    let bootstrap_config = render_peer_config(
        bootstrap_peer,
        &trusted,
        &peer_telemetry_urls,
        &genesis_public_key,
        &genesis_signed_path,
        HashOf::from_untyped_unchecked(Hash::new(b"Kagami localnet policy-derivation placeholder")),
        &bls_entries,
        &bootstrap_paths,
        &rans_tables_path,
        &chain_id,
        chain_discriminant,
        (&hosts.bind, &hosts.public),
        RenderPeerFeatures {
            mcp_enabled,
            npos_bootstrap,
            operator_account: &operator_account_literal,
            operator_private_key_file: &runtime_bundle.operator_signer_key,
            onboarding_account: &onboarding_account_literal,
            onboarding_private_key_file: &runtime_bundle.onboarding_signer_key,
            onboarding_token_hash: &runtime_bundle.onboarding_token_hash,
        },
        opts.sora_profile,
        lane_manifest_directory.as_deref(),
        dataspace_fault_tolerance,
        gas_account_id.as_deref(),
        tx_gossip_overrides,
        logger_filter,
        signature_batch_max_ed25519,
        runtime_block_max_transactions,
        queue_capacity,
        sumeragi_body_bytes,
    );
    let config = parse_localnet_peer_config(&bootstrap_config)?;
    let da_proof_policies = Some(resolve_localnet_da_proof_policies(&config));
    let confidential_policy_hash =
        iroha_core::state::compute_genesis_confidential_policy_hash(&config.zk);
    let genesis = genesis
        .with_consensus_mode(opts.consensus_mode)
        .with_consensus_meta();
    let genesis_public_key_path = out_dir.join(GENESIS_PUBLIC_KEY_FILE);
    let genesis_private_key_path = out_dir.join(GENESIS_PRIVATE_KEY_FILE);
    write_genesis_key_files(
        &genesis_public_key_path,
        &genesis_private_key_path,
        &genesis_public_key,
        &genesis_private,
        redact_seed_metadata,
    )?;
    let genesis_expected_hash = write_genesis(GenesisWriteContext {
        manifest: &genesis,
        public_key: &genesis_public_key,
        private_key: genesis_private.clone(),
        config: &config,
        chain_discriminant,
        json_path: &genesis_json_path,
        signed_path: &genesis_signed_path,
        policies: GenesisConsensusPolicies {
            da_proof_policies,
            confidential_policy_hash,
        },
    })?;
    write_and_validate_genesis_expected_hash(
        &genesis_expected_hash_path,
        &genesis_signed_path,
        genesis_expected_hash,
    )?;
    tui::status("Genesis staged and bootstrap-validated");
    tui::status("Writing peer configs");
    for (idx, peer) in peers.iter().enumerate() {
        let paths = LocalnetPeerStoragePaths::new(&out_dir, idx);
        fs::create_dir_all(&paths.kura)
            .wrap_err_with(|| format!("failed to create kura dir {}", paths.kura.display()))?;
        fs::create_dir_all(&paths.state).wrap_err_with(|| {
            format!("failed to create peer state dir {}", paths.state.display())
        })?;
        fs::create_dir_all(&paths.tiered_state).wrap_err_with(|| {
            format!(
                "failed to create tiered state dir {}",
                paths.tiered_state.display()
            )
        })?;
        fs::create_dir_all(&paths.da_store).wrap_err_with(|| {
            format!(
                "failed to create DA WSV snapshot dir {}",
                paths.da_store.display()
            )
        })?;
        let rendered = render_peer_config(
            peer,
            &trusted,
            &peer_telemetry_urls,
            &genesis_public_key,
            &genesis_signed_path,
            genesis_expected_hash,
            &bls_entries,
            &paths,
            &rans_tables_path,
            &chain_id,
            chain_discriminant,
            (&hosts.bind, &hosts.public),
            RenderPeerFeatures {
                mcp_enabled,
                npos_bootstrap,
                operator_account: &operator_account_literal,
                operator_private_key_file: &runtime_bundle.operator_signer_key,
                onboarding_account: &onboarding_account_literal,
                onboarding_private_key_file: &runtime_bundle.onboarding_signer_key,
                onboarding_token_hash: &runtime_bundle.onboarding_token_hash,
            },
            opts.sora_profile,
            lane_manifest_directory.as_deref(),
            dataspace_fault_tolerance,
            gas_account_id.as_deref(),
            tx_gossip_overrides,
            logger_filter,
            signature_batch_max_ed25519,
            runtime_block_max_transactions,
            queue_capacity,
            sumeragi_body_bytes,
        );
        let parsed_config = parse_localnet_peer_config(&rendered).wrap_err_with(|| {
            format!("generated validator config peer{idx}.toml failed Config/Catalog validation")
        })?;
        if parsed_config.genesis.expected_hash != genesis_expected_hash {
            return Err(eyre!(
                "generated validator config peer{idx}.toml has genesis hash {}, expected {}",
                parsed_config.genesis.expected_hash,
                genesis_expected_hash
            ));
        }
        let path = out_dir.join(format!("peer{idx}.toml"));
        write_owner_only_localnet_file(&path, rendered.as_bytes())
            .wrap_err_with(|| format!("write validator config {}", path.display()))?;
    }
    tui::status("Peer configs written and validated");
    tui::status("Writing start/stop scripts");
    let fee_asset_definition_id = localnet_fee_asset_literal();
    write_scripts(
        &out_dir,
        opts.peers.get(),
        sora_profile_enabled,
        &client_account_literal,
        &fee_asset_definition_id,
    )?;
    tui::status("Writing client config");
    write_client_config(
        &out_dir,
        opts.base_api_port,
        &hosts.public,
        &chain_id,
        genesis_expected_hash,
        chain_discriminant,
        &client_identity,
    )?;
    let primary_torii_url = hosts.public.torii_url(opts.base_api_port);
    let client_config_path = out_dir.join("client.toml");
    let start_path = out_dir.join("start.sh");
    let stop_path = out_dir.join("stop.sh");
    write_localnet_readme(
        &out_dir,
        &chain_id,
        if redact_seed_metadata {
            None
        } else {
            opts.seed.as_deref()
        },
        redact_seed_metadata,
        opts.consensus_mode,
        opts.peers.get(),
        &primary_torii_url,
        &genesis_json_path,
        &genesis_signed_path,
        &genesis_expected_hash_path,
        &genesis_public_key_path,
        &genesis_private_key_path,
        &client_config_path,
        &start_path,
        &stop_path,
        &client_identity.account_literal(chain_discriminant),
        &onboarding_identity.account_id.to_string(),
        &runtime_bundle,
        &alias_setup_intent_path,
    )?;
    if redact_seed_metadata {
        crate::secure_fs::harden_private_tree_with_owner_executables(
            &out_dir,
            &[&start_path, &stop_path],
        )
        .wrap_err("harden fresh localnet private artifact tree")?;
    }
    tui::success("Localnet ready");
    writeln!(writer, "out_dir: {}", out_dir.display())?;
    writeln!(writer, "chain_id: {}", chain_id)?;
    writeln!(
        writer,
        "consensus_mode: {}",
        consensus_mode_label(opts.consensus_mode)
    )?;
    writeln!(writer, "peers: {}", opts.peers.get())?;
    writeln!(writer, "torii_url: {}", primary_torii_url)?;
    writeln!(writer, "genesis_json: {}", genesis_json_path.display())?;
    writeln!(writer, "genesis_signed: {}", genesis_signed_path.display())?;
    writeln!(
        writer,
        "genesis_expected_hash: {}",
        genesis_expected_hash_path.display()
    )?;
    writeln!(
        writer,
        "genesis_public_key: {}",
        genesis_public_key_path.display()
    )?;
    writeln!(
        writer,
        "genesis_private_key: {}",
        genesis_private_key_path.display()
    )?;
    writeln!(writer, "client_config: {}", client_config_path.display())?;
    writeln!(
        writer,
        "alias_setup_intent: {}",
        alias_setup_intent_path.display()
    )?;
    writeln!(
        writer,
        "operator_signer_key: {}",
        runtime_bundle.operator_signer_key.display()
    )?;
    writeln!(
        writer,
        "onboarding_signer_key: {}",
        runtime_bundle.onboarding_signer_key.display()
    )?;
    writeln!(
        writer,
        "onboarding_token_file: {}",
        runtime_bundle.onboarding_token_file.display()
    )?;
    writeln!(writer, "start_script: {}", start_path.display())?;
    writeln!(writer, "stop_script: {}", stop_path.display())?;
    writeln!(writer, "guide: {}", out_dir.join("README.md").display())?;
    writeln!(
        writer,
        "next_start: cd {} && {}",
        out_dir.display(),
        localnet_script_command(redact_seed_metadata, "start.sh")
    )?;
    writeln!(writer, "next_health: curl -sf {}health", primary_torii_url)?;
    writeln!(
        writer,
        "next_stop: cd {} && {}",
        out_dir.display(),
        localnet_script_command(redact_seed_metadata, "stop.sh")
    )?;
    Ok(())
}
fn localnet_tx_gossip_overrides(block_cadence_ms: u64) -> Option<LocalnetTxGossipOverrides> {
    if block_cadence_ms > LOCALNET_PIPELINE_TIME_MS {
        return None;
    }
    Some(LocalnetTxGossipOverrides {
        period_ms: LOCALNET_TX_GOSSIP_PERIOD_FAST_MS,
        resend_ticks: LOCALNET_TX_GOSSIP_RESEND_TICKS_FAST,
    })
}
fn build_peers(count: u16, seed: Option<&[u8]>, base_api: u16, base_p2p: u16) -> Result<Vec<Peer>> {
    (0..count)
        .map(|nth| {
            let (bls_public, bls_secret, pop) = generate_bls_key_pair(seed, &nth.to_be_bytes())
                .wrap_err_with(|| format!("failed to generate BLS key pair for peer {nth}"))?;
            let (soranet_transport_public_key, soranet_transport_private_key) =
                generate_soranet_transport_key_pair(seed, &nth.to_be_bytes()).wrap_err_with(
                    || format!("failed to generate SoraNet transport key pair for peer {nth}"),
                )?;
            let (streaming_public_key, streaming_private_key) =
                generate_streaming_identity_key_pair(seed, &nth.to_be_bytes()).wrap_err_with(
                    || format!("failed to generate streaming identity key pair for peer {nth}"),
                )?;
            Ok(Peer {
                public_key: bls_public.clone(),
                private_key: bls_secret,
                soranet_transport_public_key,
                soranet_transport_private_key,
                streaming_public_key,
                streaming_private_key,
                bls_public_key: bls_public,
                bls_pop: pop,
                api_port: base_api + nth,
                p2p_port: base_p2p + nth,
            })
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
fn localnet_dataspace_catalog(
    sora_profile: Option<SoraProfile>,
    fault_tolerance: u32,
) -> Vec<toml::Value> {
    use toml::{Table, Value};
    let fault_tolerance = i64::from(fault_tolerance);
    let mut universal = Table::new();
    universal.insert("alias".into(), Value::String("universal".to_owned()));
    universal.insert("id".into(), Value::Integer(0));
    universal.insert(
        "description".into(),
        Value::String(
            "Shared public data space for core, governance, and zero-knowledge lanes".to_owned(),
        ),
    );
    universal.insert("fault_tolerance".into(), Value::Integer(fault_tolerance));
    let mut catalog = vec![Value::Table(universal)];
    let mut extra_dataspaces = match sora_profile {
        Some(SoraProfile::Nexus) => vec![
            (
                "paynet",
                i64::try_from(LOCALNET_PAYNET_ALIAS_DATASPACE_ID)
                    .expect("PAYNET dataspace id fits i64"),
                "PayNet private dataspace",
            ),
            (
                "nexus",
                i64::try_from(LOCALNET_CBUAE_ALIAS_DATASPACE_ID)
                    .expect("CBUAE dataspace id fits i64"),
                "Nexus service alias dataspace",
            ),
        ],
        Some(SoraProfile::Dataspace | SoraProfile::PrivateSbp | SoraProfile::PrivateCbuae)
        | None => Vec::new(),
    };
    if let Some(spec) = private_dataspace_spec(sora_profile) {
        extra_dataspaces.push((
            spec.alias,
            i64::try_from(spec.id).expect("private dataspace id fits i64"),
            spec.dataspace_description,
        ));
    }
    for (alias, id, description) in extra_dataspaces {
        let mut entry = Table::new();
        entry.insert("alias".into(), Value::String(alias.to_owned()));
        entry.insert("id".into(), Value::Integer(id));
        entry.insert(
            "manifest_hash".into(),
            Value::String(localnet_dataspace_manifest_hash(id)),
        );
        entry.insert("description".into(), Value::String(description.to_owned()));
        entry.insert("fault_tolerance".into(), Value::Integer(fault_tolerance));
        catalog.push(Value::Table(entry));
    }
    catalog
}
#[derive(crate::json_macros::JsonSerialize)]
struct LocalnetLaneManifestValidator {
    validator: String,
    peer_id: String,
}
#[derive(crate::json_macros::JsonSerialize)]
struct LocalnetLaneManifest {
    lane: String,
    governance: String,
    version: u32,
    validators: Vec<LocalnetLaneManifestValidator>,
    quorum: u32,
}
fn write_localnet_lane_manifests(
    out_dir: &Path,
    sora_profile: Option<SoraProfile>,
    peers: &[Peer],
    chain_discriminant: Option<u16>,
) -> Result<Option<PathBuf>> {
    let Some(spec) = private_dataspace_spec(sora_profile) else {
        return Ok(None);
    };
    let manifest_directory = out_dir.join("lane-manifests");
    fs::create_dir(&manifest_directory).wrap_err_with(|| {
        format!(
            "failed to create localnet lane manifest directory {}",
            manifest_directory.display()
        )
    })?;
    let validators = peers
        .iter()
        .map(|peer| {
            let account_id = AccountId::new(peer.public_key.clone());
            LocalnetLaneManifestValidator {
                validator: account_id_runtime_literal(&account_id, chain_discriminant),
                peer_id: PeerId::from(peer.public_key.clone()).to_string(),
            }
        })
        .collect::<Vec<_>>();
    let peer_count = u32::try_from(validators.len())
        .map_err(|_| eyre!("localnet lane manifest validator count exceeds u32"))?;
    let quorum = peer_count
        .checked_mul(2)
        .map(|value| value / 3)
        .and_then(|value| value.checked_add(1))
        .ok_or_else(|| eyre!("localnet lane manifest quorum overflow"))?;
    if usize::try_from(quorum).map_or(true, |value| value > validators.len()) {
        return Err(eyre!(
            "localnet lane manifest quorum {quorum} exceeds {} validators",
            validators.len()
        ));
    }
    let manifest = LocalnetLaneManifest {
        lane: spec.alias.to_owned(),
        governance: "parliament".to_owned(),
        version: 1,
        validators,
        quorum,
    };
    let raw = norito::json::to_json_pretty(&manifest).wrap_err_with(|| {
        format!(
            "serialize localnet {} lane manifest",
            spec.alias.to_uppercase()
        )
    })?;
    let manifest_path = manifest_directory.join(format!("{}.manifest.json", spec.alias));
    fs::write(&manifest_path, raw).wrap_err_with(|| {
        format!(
            "failed to write localnet {} lane manifest {}",
            spec.alias.to_uppercase(),
            manifest_path.display()
        )
    })?;
    Ok(Some(manifest_directory))
}
fn localnet_dataspace_manifest_hash(id: i64) -> String {
    use std::fmt::Write as _;
    let id = u64::try_from(id).expect("dataspace id must be non-negative");
    let mut hex = String::with_capacity(64);
    for byte in id.to_le_bytes() {
        write!(&mut hex, "{byte:02x}").expect("writing to String should not fail");
    }
    hex.push_str("000000000000000000000000000000000000000000000000");
    hex
}
#[expect(
    clippy::too_many_lines,
    reason = "the canonical lane matrices stay together so profile ordering remains auditable"
)]
fn localnet_lane_catalog(sora_profile: Option<SoraProfile>) -> Option<(i64, Vec<toml::Value>)> {
    use toml::{Table, Value};
    if !localnet_uses_alias_multilane_catalog(sora_profile) {
        return None;
    }
    let private_profile = matches!(
        sora_profile,
        Some(SoraProfile::PrivateSbp | SoraProfile::PrivateCbuae)
    );
    let mut lane_specs = if private_profile {
        vec![
            (
                0_i64,
                "core",
                "Primary public lane",
                "universal",
                "public",
                None,
            ),
            (
                1_i64,
                "governance",
                "Governance lane",
                "universal",
                "public",
                None,
            ),
            (
                2_i64,
                "zk",
                "Zero-knowledge lane",
                "universal",
                "public",
                None,
            ),
        ]
    } else {
        vec![
            (
                0_i64,
                "core",
                "Primary execution lane",
                "universal",
                "public",
                None,
            ),
            (
                1_i64,
                "governance",
                "Governance & parliament traffic",
                "universal",
                "public",
                None,
            ),
            (
                2_i64,
                "zk",
                "Zero-knowledge attachments",
                "universal",
                "public",
                None,
            ),
        ]
    };
    let lane_count = match sora_profile {
        Some(SoraProfile::Nexus) => {
            lane_specs.extend([
                (
                    i64::from(LOCALNET_PAYNET_ALIAS_LANE_INDEX),
                    "paynet",
                    "PayNet private dataspace lane",
                    "paynet",
                    "public",
                    None,
                ),
                (
                    i64::from(LOCALNET_CBUAE_ALIAS_LANE_INDEX),
                    "nexus",
                    "Nexus service alias lane",
                    "nexus",
                    "public",
                    None,
                ),
            ]);
            LOCALNET_NEXUS_ALIAS_LANE_COUNT
        }
        Some(SoraProfile::Dataspace | SoraProfile::PrivateSbp | SoraProfile::PrivateCbuae) => {
            let spec = private_dataspace_spec(sora_profile)
                .expect("private dataspace profile must have a typed specification");
            lane_specs.push((
                i64::from(spec.lane_index),
                spec.alias,
                spec.lane_description,
                spec.alias,
                "restricted",
                Some("parliament"),
            ));
            i64::from(spec.lane_index) + 1
        }
        None => return None,
    };
    let mut catalog = Vec::new();
    for (index, alias, description, dataspace, visibility, governance) in lane_specs {
        let mut entry = Table::new();
        entry.insert("index".into(), Value::Integer(index));
        entry.insert("alias".into(), Value::String(alias.to_owned()));
        entry.insert("description".into(), Value::String(description.to_owned()));
        entry.insert("dataspace".into(), Value::String(dataspace.to_owned()));
        entry.insert("visibility".into(), Value::String(visibility.to_owned()));
        if let Some(governance) = governance {
            entry.insert("governance".into(), Value::String(governance.to_owned()));
        }
        entry.insert("metadata".into(), Value::Table(Table::new()));
        catalog.push(Value::Table(entry));
    }
    Some((lane_count, catalog))
}
#[allow(clippy::items_after_statements)]
#[expect(
    clippy::too_many_lines,
    reason = "the canonical routing matrices stay together so first-match ordering remains auditable"
)]
fn localnet_routing_policy(sora_profile: Option<SoraProfile>) -> Option<toml::Table> {
    use toml::{Table, Value};
    if !localnet_uses_alias_multilane_catalog(sora_profile) {
        return None;
    }
    fn rule(
        lane: u32,
        dataspace: &str,
        matcher_key: &str,
        matcher_value: &str,
        description: Option<&str>,
    ) -> toml::Value {
        let mut matcher = Table::new();
        matcher.insert(
            matcher_key.to_owned(),
            Value::String(matcher_value.to_owned()),
        );
        let description = description.map_or_else(
            || match matcher_key {
                "instruction" => match matcher_value {
                    "governance" => {
                        "Route governance instructions to the governance lane".to_owned()
                    }
                    "smartcontract::deploy" => {
                        "Route contract deployments to the zk lane for proof tracking".to_owned()
                    }
                    _ => format!("Route {matcher_value} instructions to the {dataspace} lane"),
                },
                "account" => format!("Route {matcher_value} account traffic to {dataspace} lane"),
                _ => format!("Route {matcher_key}={matcher_value} traffic to {dataspace} lane"),
            },
            str::to_owned,
        );
        matcher.insert("description".into(), Value::String(description));
        let mut rule = Table::new();
        rule.insert("lane".into(), Value::Integer(i64::from(lane)));
        rule.insert("dataspace".into(), Value::String(dataspace.to_owned()));
        rule.insert("matcher".into(), Value::Table(matcher));
        Value::Table(rule)
    }
    let rules = match sora_profile {
        Some(SoraProfile::Nexus) => vec![
            rule(1, "universal", "instruction", "governance", None),
            rule(2, "universal", "instruction", "smartcontract::deploy", None),
            rule(
                LOCALNET_PAYNET_ALIAS_LANE_INDEX,
                "paynet",
                "account",
                "*@paynet",
                None,
            ),
            rule(
                LOCALNET_PAYNET_ALIAS_LANE_INDEX,
                "paynet",
                "account",
                "*@*.paynet",
                None,
            ),
        ],
        Some(SoraProfile::Dataspace) => {
            let spec = private_dataspace_spec(sora_profile)
                .expect("dataspace profile must have a typed specification");
            let mut rules = vec![
                rule(1, "universal", "instruction", "governance", None),
                rule(2, "universal", "instruction", "smartcontract::deploy", None),
            ];
            rules.extend(spec.account_routes.iter().map(|route| {
                rule(
                    spec.lane_index,
                    spec.alias,
                    "account",
                    route.matcher,
                    Some(route.description),
                )
            }));
            rules
        }
        Some(SoraProfile::PrivateSbp | SoraProfile::PrivateCbuae) => {
            let spec = private_dataspace_spec(sora_profile)
                .expect("private dataspace profile must have a typed specification");
            let mut rules = spec
                .account_routes
                .iter()
                .map(|route| {
                    rule(
                        spec.lane_index,
                        spec.alias,
                        "account",
                        route.matcher,
                        Some(route.description),
                    )
                })
                .collect::<Vec<_>>();
            rules.extend([
                rule(
                    1,
                    "universal",
                    "instruction",
                    "governance",
                    Some(
                        "Route public governance instructions to the governance lane after private authority routes",
                    ),
                ),
                rule(
                    2,
                    "universal",
                    "instruction",
                    "smartcontract::deploy",
                    Some(
                        "Route public smart-contract deployment to the zk lane after private authority routes",
                    ),
                ),
            ]);
            rules.extend(spec.transfer_routes.iter().map(|route| {
                rule(
                    spec.lane_index,
                    spec.alias,
                    "instruction",
                    route.matcher,
                    Some(route.description),
                )
            }));
            rules
        }
        None => return None,
    };
    let mut policy = Table::new();
    policy.insert("default_lane".into(), Value::Integer(0));
    policy.insert(
        "default_dataspace".into(),
        Value::String("universal".to_owned()),
    );
    policy.insert("rules".into(), Value::Array(rules));
    Some(policy)
}
fn localnet_public_validator_lanes(sora_profile: Option<SoraProfile>) -> Vec<LaneId> {
    // Static lanes sharing one physical dataspace share the lowest stake-elected owner. The
    // universal governance and ZK lanes therefore inherit lane 0's validator pool, while a
    // restricted non-universal lane is governed by its authenticated lane manifest.
    let mut lanes = vec![LaneId::SINGLE];
    match sora_profile {
        Some(SoraProfile::Nexus) => {
            lanes.push(LaneId::new(LOCALNET_PAYNET_ALIAS_LANE_INDEX));
            lanes.push(LaneId::new(LOCALNET_CBUAE_ALIAS_LANE_INDEX));
        }
        Some(SoraProfile::Dataspace | SoraProfile::PrivateSbp | SoraProfile::PrivateCbuae)
        | None => {}
    }
    lanes
}
fn resolve_localnet_chain_id(configured: Option<&str>) -> Result<String> {
    let chain_id = configured.unwrap_or(DEFAULT_CHAIN_ID).trim();
    if chain_id.is_empty() {
        return Err(eyre!("`--chain-id` must not be empty"));
    }
    chain_id
        .parse::<ChainId>()
        .wrap_err("`--chain-id` must be canonical")?;
    Ok(chain_id.to_owned())
}
#[derive(Clone, Copy)]
struct RenderPeerFeatures<'a> {
    mcp_enabled: bool,
    npos_bootstrap: bool,
    operator_account: &'a str,
    operator_private_key_file: &'a Path,
    onboarding_account: &'a str,
    onboarding_private_key_file: &'a Path,
    onboarding_token_hash: &'a [u8; 32],
}
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn render_peer_config(
    peer: &Peer,
    trusted_peers: &[String],
    peer_telemetry_urls: &[String],
    genesis_public_key: &iroha_crypto::PublicKey,
    genesis_signed_path: &Path,
    genesis_expected_hash: HashOf<BlockHeader>,
    bls_entries: &[BlsEntry],
    storage_paths: &LocalnetPeerStoragePaths,
    rans_tables_path: &Path,
    chain_id: &str,
    chain_discriminant: Option<u16>,
    hosts: (&CanonicalHost, &CanonicalHost),
    features: RenderPeerFeatures<'_>,
    sora_profile: Option<SoraProfile>,
    lane_manifest_directory: Option<&Path>,
    dataspace_fault_tolerance: Option<u32>,
    gas_account_id: Option<&str>,
    tx_gossip_overrides: Option<LocalnetTxGossipOverrides>,
    logger_filter: Option<&str>,
    signature_batch_max_ed25519: Option<usize>,
    runtime_block_max_transactions: Option<usize>,
    queue_capacity: usize,
    sumeragi_body_bytes: usize,
) -> String {
    use iroha_config::parameters::defaults::streaming::{
        self as streaming_defaults, codec as codec_defaults,
    };
    use toml::{Table, Value};
    let (bind_host, public_host) = hosts;
    let RenderPeerFeatures {
        mcp_enabled,
        npos_bootstrap,
        operator_account,
        operator_private_key_file,
        onboarding_account,
        onboarding_private_key_file,
        onboarding_token_hash,
    } = features;
    let localnet_operator_account = operator_account.to_owned();
    let genesis_account = AccountId::new(genesis_public_key.clone());
    let genesis_account_literal = account_id_runtime_literal(&genesis_account, chain_discriminant);
    let fee_sponsor_program_id =
        format!("{genesis_account_literal}/{LOCALNET_FEE_SPONSOR_PROGRAM_NAME}");
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
    root.insert("chain".into(), Value::String(chain_id.to_owned()));
    if let Some(chain_discriminant) = chain_discriminant {
        root.insert(
            "chain_discriminant".into(),
            Value::Integer(i64::from(chain_discriminant)),
        );
    }
    root.insert(
        "private_key".into(),
        Value::String(peer.private_key.to_string()),
    );
    root.insert(
        "public_key".into(),
        Value::String(peer.public_key.to_string()),
    );
    root.insert(
        "soranet_transport_private_key".into(),
        Value::String(peer.soranet_transport_private_key.to_string()),
    );
    root.insert(
        "soranet_transport_public_key".into(),
        Value::String(peer.soranet_transport_public_key.to_string()),
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
        Value::String(storage_paths.kura.to_string_lossy().into_owned()),
    );
    kura.insert(
        "fsync_mode".into(),
        Value::String(LOCALNET_KURA_FSYNC_MODE.to_owned()),
    );
    root.insert("kura".into(), Value::Table(kura));
    let mut soracloud_runtime = Table::new();
    soracloud_runtime.insert(
        "state_dir".into(),
        Value::String(
            storage_paths
                .soracloud_runtime
                .to_string_lossy()
                .into_owned(),
        ),
    );
    root.insert("soracloud_runtime".into(), Value::Table(soracloud_runtime));
    let mut tiered_state = Table::new();
    tiered_state.insert(
        "cold_store_root".into(),
        Value::String(storage_paths.tiered_state.to_string_lossy().into_owned()),
    );
    tiered_state.insert(
        "da_store_root".into(),
        Value::String(storage_paths.da_store.to_string_lossy().into_owned()),
    );
    root.insert("tiered_state".into(), Value::Table(tiered_state));
    let mut sumeragi = Table::new();
    sumeragi.insert("role".into(), Value::String("validator".to_owned()));
    let mut queues = Table::new();
    queues.insert(
        "commands".into(),
        Value::Integer(
            i64::try_from(LOCALNET_SUMERAGI_QUEUE_COMMANDS)
                .expect("localnet command queue fits i64"),
        ),
    );
    queues.insert(
        "authenticated_non_validator_sources".into(),
        Value::Integer(
            i64::try_from(LOCALNET_SUMERAGI_AUTHENTICATED_NON_VALIDATOR_SOURCES)
                .expect("localnet authenticated non-validator source count fits i64"),
        ),
    );
    queues.insert(
        "bodies".into(),
        Value::Integer(
            i64::try_from(LOCALNET_SUMERAGI_QUEUE_BODIES).expect("localnet body queue fits i64"),
        ),
    );
    queues.insert(
        "body_bytes".into(),
        Value::Integer(
            i64::try_from(sumeragi_body_bytes)
                .expect("localnet aggregate outer-ingress wire-byte budget fits i64"),
        ),
    );
    queues.insert(
        "body_source_bytes".into(),
        Value::Integer(
            i64::try_from(LOCALNET_SUMERAGI_QUEUE_BODY_SOURCE_BYTES)
                .expect("localnet per-source outer-ingress wire-byte budget fits i64"),
        ),
    );
    queues.insert(
        "chunks".into(),
        Value::Integer(
            i64::try_from(LOCALNET_SUMERAGI_QUEUE_CHUNKS).expect("localnet chunk queue fits i64"),
        ),
    );
    queues.insert(
        "ready_bodies".into(),
        Value::Integer(
            i64::try_from(LOCALNET_SUMERAGI_QUEUE_READY_BODIES)
                .expect("localnet ready-body queue fits i64"),
        ),
    );
    sumeragi.insert("queues".into(), Value::Table(queues));
    let mut keys = Table::new();
    keys.insert(
        "allowed_algorithms".into(),
        Value::Array(vec![Value::String("bls_normal".to_owned())]),
    );
    keys.insert(
        "allowed_hsm_providers".into(),
        Value::Array(
            iroha_config::parameters::defaults::sumeragi::key_allowed_hsm_providers()
                .into_iter()
                .map(Value::String)
                .collect(),
        ),
    );
    sumeragi.insert("keys".into(), Value::Table(keys));
    let mut nexus = Table::new();
    if npos_bootstrap {
        let mut storage = Table::new();
        storage.insert(
            "local_budget_bytes".into(),
            Value::Integer(
                i64::try_from(LOCALNET_NEXUS_STORAGE_BUDGET_BYTES)
                    .expect("localnet Nexus storage budget fits i64"),
            ),
        );
        nexus.insert("storage".into(), Value::Table(storage));
    }
    let mut fusion = Table::new();
    fusion.insert(
        "exit_teu".into(),
        Value::Integer(i64::from(LOCALNET_LANE_TEU_CAPACITY)),
    );
    nexus.insert("fusion".into(), Value::Table(fusion));
    if npos_bootstrap {
        let gas_account_id = gas_account_id.expect("localnet gas account id required");
        let stake_asset_id = localnet_stake_asset_literal();
        let fee_asset_id = localnet_fee_asset_literal();
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
        fees.insert("fee_asset_id".into(), Value::String(fee_asset_id));
        fees.insert("base_fee".into(), Value::String("0".to_owned()));
        fees.insert("per_byte_fee".into(), Value::String("0".to_owned()));
        fees.insert(
            "per_instruction_fee".into(),
            Value::String("0.001".to_owned()),
        );
        fees.insert(
            "per_gas_unit_fee".into(),
            Value::String("0.00005".to_owned()),
        );
        fees.insert("settlement_mode".into(), Value::String("direct".to_owned()));
        fees.insert(
            "fee_sink_account_id".into(),
            Value::String(gas_account_id.to_owned()),
        );
        fees.insert(
            "sponsor_vault_custody_account_id".into(),
            Value::String(gas_account_id.to_owned()),
        );
        nexus.insert("fees".into(), Value::Table(fees));
    }
    if let Some((lane_count, lane_catalog)) = localnet_lane_catalog(sora_profile) {
        nexus.insert("lane_count".into(), Value::Integer(lane_count));
        nexus.insert("lane_catalog".into(), Value::Array(lane_catalog));
    }
    if let Some(fault_tolerance) = dataspace_fault_tolerance {
        let catalog = localnet_dataspace_catalog(sora_profile, fault_tolerance);
        nexus.insert("dataspace_catalog".into(), Value::Array(catalog));
    }
    if let Some(policy) = localnet_routing_policy(sora_profile) {
        nexus.insert("routing_policy".into(), Value::Table(policy));
    }
    if let Some(manifest_directory) = lane_manifest_directory {
        assert!(
            private_dataspace_spec(sora_profile).is_some(),
            "lane manifests are only generated for restricted dataspace profiles"
        );
        let mut registry = Table::new();
        registry.insert(
            "manifest_directory".into(),
            Value::String(manifest_directory.to_string_lossy().into_owned()),
        );
        nexus.insert("registry".into(), Value::Table(registry));
        let mut parliament = Table::new();
        parliament.insert(
            "module_type".into(),
            Value::String("parliament_sortition_jit".to_owned()),
        );
        let mut parliament_params = Table::new();
        parliament_params.insert(
            "selection".into(),
            Value::String("multibody_sortition".to_owned()),
        );
        parliament_params.insert("approval_flow".into(), Value::String("jit".to_owned()));
        parliament.insert("params".into(), Value::Table(parliament_params));
        let mut modules = Table::new();
        modules.insert("parliament".into(), Value::Table(parliament));
        let mut governance = Table::new();
        governance.insert(
            "default_module".into(),
            Value::String("parliament".to_owned()),
        );
        governance.insert("modules".into(), Value::Table(modules));
        nexus.insert("governance".into(), Value::Table(governance));
    }
    root.insert("nexus".into(), Value::Table(nexus));
    let mut block = Table::new();
    if let Some(max_transactions) = runtime_block_max_transactions {
        block.insert(
            "max_transactions".into(),
            Value::Integer(
                i64::try_from(max_transactions).expect("runtime block max transactions fits i64"),
            ),
        );
    }
    block.insert(
        "max_payload_bytes".into(),
        Value::Integer(
            i64::try_from(
                iroha_config::parameters::defaults::sumeragi::BLOCK_MAX_PAYLOAD_BYTES.get(),
            )
            .expect("payload limit fits i64"),
        ),
    );
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
        Value::String(peer.streaming_public_key.to_string()),
    );
    streaming.insert(
        "identity_private_key".into(),
        Value::String(peer.streaming_private_key.to_string()),
    );
    streaming.insert(
        "session_store_dir".into(),
        Value::String(
            storage_paths
                .streaming_sessions
                .to_string_lossy()
                .into_owned(),
        ),
    );
    let mut streaming_soranet = Table::new();
    streaming_soranet.insert(
        "enabled".into(),
        Value::Boolean(streaming_defaults::soranet::ENABLED),
    );
    streaming_soranet.insert(
        "exit_multiaddr".into(),
        Value::String(streaming_defaults::soranet::EXIT_MULTIADDR.to_owned()),
    );
    if let Some(padding_budget_ms) = streaming_defaults::soranet::padding_budget_ms() {
        streaming_soranet.insert(
            "padding_budget_ms".into(),
            Value::Integer(i64::from(padding_budget_ms)),
        );
    }
    streaming_soranet.insert(
        "access_kind".into(),
        Value::String(streaming_defaults::soranet::ACCESS_KIND.to_owned()),
    );
    streaming_soranet.insert(
        "channel_salt".into(),
        Value::String(streaming_defaults::soranet::CHANNEL_SALT.to_owned()),
    );
    streaming_soranet.insert(
        "provision_spool_dir".into(),
        Value::String(
            storage_paths
                .streaming_soranet_spool
                .to_string_lossy()
                .into_owned(),
        ),
    );
    streaming_soranet.insert(
        "provision_spool_max_bytes".into(),
        Value::Integer(
            i64::try_from(streaming_defaults::soranet::PROVISION_SPOOL_MAX_BYTES.get())
                .expect("streaming SoraNet spool maximum fits i64"),
        ),
    );
    streaming_soranet.insert(
        "provision_window_segments".into(),
        Value::Integer(
            i64::try_from(streaming_defaults::soranet::PROVISION_WINDOW_SEGMENTS)
                .expect("streaming SoraNet provision window fits i64"),
        ),
    );
    streaming_soranet.insert(
        "provision_queue_capacity".into(),
        Value::Integer(
            i64::try_from(streaming_defaults::soranet::PROVISION_QUEUE_CAPACITY)
                .expect("streaming SoraNet provision queue fits i64"),
        ),
    );
    streaming.insert("soranet".into(), Value::Table(streaming_soranet));
    let mut streaming_soravpn = Table::new();
    streaming_soravpn.insert(
        "provision_spool_dir".into(),
        Value::String(
            storage_paths
                .streaming_soravpn_spool
                .to_string_lossy()
                .into_owned(),
        ),
    );
    streaming_soravpn.insert(
        "provision_spool_max_bytes".into(),
        Value::Integer(
            i64::try_from(streaming_defaults::soravpn::PROVISION_SPOOL_MAX_BYTES.get())
                .expect("streaming SoraVPN spool maximum fits i64"),
        ),
    );
    streaming.insert("soravpn".into(), Value::Table(streaming_soravpn));
    let mut streaming_codec = Table::new();
    streaming_codec.insert(
        "cabac_mode".into(),
        Value::String(codec_defaults::CABAC_MODE.to_owned()),
    );
    streaming_codec.insert(
        "trellis_blocks".into(),
        Value::Array(
            codec_defaults::trellis_blocks()
                .into_iter()
                .map(|size| Value::Integer(i64::from(size)))
                .collect(),
        ),
    );
    streaming_codec.insert(
        "rans_tables_path".into(),
        Value::String(rans_tables_path.to_string_lossy().into_owned()),
    );
    streaming_codec.insert(
        "entropy_mode".into(),
        Value::String(codec_defaults::entropy_mode()),
    );
    streaming_codec.insert(
        "bundle_width".into(),
        Value::Integer(i64::from(codec_defaults::bundle_width())),
    );
    streaming_codec.insert(
        "bundle_accel".into(),
        Value::String(codec_defaults::bundle_accel()),
    );
    streaming.insert("codec".into(), Value::Table(streaming_codec));
    root.insert("streaming".into(), Value::Table(streaming));
    let mut sorafs_storage = Table::new();
    if sora_profile.is_some() {
        // `iroha3d --sora` enables embedded storage after ordinary TOML parsing unless the
        // operator explicitly selected a storage value. Localnets do not provision the governed
        // compliance controller or native signer providers, so keep storage disabled explicitly.
        sorafs_storage.insert("enabled".into(), Value::Boolean(false));
    }
    // Validator durability queues beneath the SoraFS root remain active even when provider
    // storage workers are disabled, so every generated peer must own a disjoint root.
    sorafs_storage.insert(
        "data_dir".into(),
        Value::String(storage_paths.sorafs.to_string_lossy().into_owned()),
    );
    let mut sorafs = Table::new();
    sorafs.insert("storage".into(), Value::Table(sorafs_storage));
    let mut sorafs_por = Table::new();
    sorafs_por.insert(
        "state_dir".into(),
        Value::String(storage_paths.sorafs_por.to_string_lossy().into_owned()),
    );
    sorafs.insert("por".into(), Value::Table(sorafs_por));
    root.insert("sorafs".into(), Value::Table(sorafs));
    if let Some(chain_discriminant) = chain_discriminant {
        let mut governance = Table::new();
        let citizenship_escrow_account = account_id_runtime_literal(
            &iroha_config::parameters::defaults::governance::citizenship_escrow_account_id(),
            Some(chain_discriminant),
        );
        let bond_escrow_account = account_id_runtime_literal(
            &iroha_config::parameters::defaults::governance::bond_escrow_account_id(),
            Some(chain_discriminant),
        );
        let slash_receiver_account = account_id_runtime_literal(
            &iroha_config::parameters::defaults::governance::slash_receiver_account_id(),
            Some(chain_discriminant),
        );
        governance.insert(
            "citizenship_escrow_account".into(),
            Value::String(citizenship_escrow_account),
        );
        governance.insert(
            "bond_escrow_account".into(),
            Value::String(bond_escrow_account),
        );
        governance.insert(
            "slash_receiver_account".into(),
            Value::String(slash_receiver_account.clone()),
        );
        governance.insert(
            "viral_incentive_pool_account".into(),
            Value::String(slash_receiver_account.clone()),
        );
        governance.insert(
            "viral_escrow_account".into(),
            Value::String(slash_receiver_account),
        );
        let telemetry_submitters =
            iroha_config::parameters::defaults::governance::sorafs_telemetry::submitters()
                .into_iter()
                .map(|literal| {
                    Value::String(account_literal_for_chain_discriminant(
                        &literal,
                        chain_discriminant,
                    ))
                })
                .collect();
        let mut sorafs_telemetry = Table::new();
        sorafs_telemetry.insert("submitters".into(), Value::Array(telemetry_submitters));
        governance.insert("sorafs_telemetry".into(), Value::Table(sorafs_telemetry));
        root.insert("gov".into(), Value::Table(governance));
    }
    let mut confidential = Table::new();
    confidential.insert("enabled".into(), Value::Boolean(true));
    confidential.insert("assume_valid".into(), Value::Boolean(false));
    root.insert("confidential".into(), Value::Table(confidential));
    let mut halo2 = Table::new();
    halo2.insert("enabled".into(), Value::Boolean(true));
    let mut zk = Table::new();
    zk.insert("halo2".into(), Value::Table(halo2));
    root.insert("zk".into(), Value::Table(zk));
    let mut genesis = Table::new();
    genesis.insert(
        "file".into(),
        Value::String(genesis_signed_path.to_string_lossy().into_owned()),
    );
    genesis.insert(
        "public_key".into(),
        Value::String(genesis_public_key.to_string()),
    );
    genesis.insert(
        "expected_hash".into(),
        Value::String(norito::literal::format(
            "hash",
            &genesis_expected_hash.to_string().to_ascii_uppercase(),
        )),
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
        "max_total_connections".into(),
        Value::Integer(
            i64::try_from(LOCALNET_MAX_TOTAL_CONNECTIONS)
                .expect("LOCALNET_MAX_TOTAL_CONNECTIONS fits i64"),
        ),
    );
    network.insert(
        "p2p_subscriber_queue_cap".into(),
        Value::Integer(
            i64::try_from(LOCALNET_P2P_SUBSCRIBER_QUEUE_CAP)
                .expect("LOCALNET_P2P_SUBSCRIBER_QUEUE_CAP fits i64"),
        ),
    );
    network.insert(
        "max_frame_bytes".into(),
        Value::Integer(i64::try_from(LOCALNET_MAX_FRAME_BYTES).expect("frame cap fits i64")),
    );
    network.insert(
        "max_frame_bytes_consensus".into(),
        Value::Integer(
            i64::try_from(LOCALNET_MAX_FRAME_BYTES_CONSENSUS)
                .expect("consensus frame cap fits i64"),
        ),
    );
    network.insert(
        "max_frame_bytes_control".into(),
        Value::Integer(
            i64::try_from(LOCALNET_MAX_FRAME_BYTES_CONTROL).expect("control frame cap fits i64"),
        ),
    );
    network.insert(
        "max_frame_bytes_block_sync".into(),
        Value::Integer(
            i64::try_from(LOCALNET_MAX_FRAME_BYTES_BLOCK_SYNC)
                .expect("block sync frame cap fits i64"),
        ),
    );
    network.insert(
        "max_frame_bytes_tx_gossip".into(),
        Value::Integer(
            i64::try_from(LOCALNET_MAX_FRAME_BYTES_TX_GOSSIP_NEXUS)
                .expect("tx gossip frame cap fits i64"),
        ),
    );
    network.insert(
        "max_frame_bytes_peer_gossip".into(),
        Value::Integer(
            i64::try_from(LOCALNET_MAX_FRAME_BYTES_PEER_GOSSIP)
                .expect("peer gossip frame cap fits i64"),
        ),
    );
    network.insert(
        "max_frame_bytes_health".into(),
        Value::Integer(
            i64::try_from(LOCALNET_MAX_FRAME_BYTES_HEALTH).expect("health frame cap fits i64"),
        ),
    );
    network.insert(
        "max_frame_bytes_other".into(),
        Value::Integer(
            i64::try_from(LOCALNET_MAX_FRAME_BYTES_OTHER).expect("other frame cap fits i64"),
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
    let mut soranet_pow = Table::new();
    soranet_pow.insert(
        "revocation_store_path".into(),
        Value::String(
            storage_paths
                .soranet_ticket_revocations
                .to_string_lossy()
                .into_owned(),
        ),
    );
    let mut soranet_handshake = Table::new();
    soranet_handshake.insert("pow".into(), Value::Table(soranet_pow));
    network.insert("soranet_handshake".into(), Value::Table(soranet_handshake));
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
        "data_dir".into(),
        Value::String(storage_paths.torii.to_string_lossy().into_owned()),
    );
    let mut da_ingest = Table::new();
    da_ingest.insert(
        "replay_cache_store_dir".into(),
        Value::String(
            storage_paths
                .torii_da_replay_cache
                .to_string_lossy()
                .into_owned(),
        ),
    );
    da_ingest.insert(
        "manifest_store_dir".into(),
        Value::String(
            storage_paths
                .torii_da_manifests
                .to_string_lossy()
                .into_owned(),
        ),
    );
    torii.insert("da_ingest".into(), Value::Table(da_ingest));
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
        "api_rate_limit_bypass_cidrs".into(),
        Value::Array(
            LOCALNET_PREAUTH_ALLOW_CIDRS
                .iter()
                .map(|cidr| Value::String((*cidr).to_string()))
                .collect::<Vec<_>>(),
        ),
    );
    torii.insert(
        "internal_api_trusted_cidrs".into(),
        Value::Array(
            LOCALNET_INTERNAL_API_TRUSTED_CIDRS
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
    torii.insert(
        "max_content_len".into(),
        Value::Integer(
            i64::try_from(LOCALNET_TORII_MAX_CONTENT_LEN)
                .expect("LOCALNET_TORII_MAX_CONTENT_LEN fits i64"),
        ),
    );
    if mcp_enabled {
        let mut mcp = Table::new();
        mcp.insert("enabled".into(), Value::Boolean(true));
        mcp.insert("profile".into(), Value::String("writer".into()));
        mcp.insert("expose_operator_routes".into(), Value::Boolean(false));
        mcp.insert(
            "allow_tool_prefixes".into(),
            Value::Array(vec![Value::String("iroha.".into())]),
        );
        torii.insert("mcp".into(), Value::Table(mcp));
    }
    let mut account_onboarding = Table::new();
    account_onboarding.insert(
        "authority".into(),
        Value::String(onboarding_account.to_owned()),
    );
    account_onboarding.insert(
        "private_key_file".into(),
        Value::String(onboarding_private_key_file.to_string_lossy().into_owned()),
    );
    account_onboarding.insert("lease_term_years".into(), Value::Integer(1));
    account_onboarding.insert("additional_permissions".into(), Value::Array(Vec::new()));
    let mut credential_scope = Table::new();
    credential_scope.insert(
        "domain".into(),
        Value::String(CLIENT_ACCOUNT_DOMAIN.to_owned()),
    );
    let mut credential = Table::new();
    credential.insert(
        "id".into(),
        Value::String(LOCALNET_ONBOARDING_CREDENTIAL_ID.to_owned()),
    );
    credential.insert("scope".into(), Value::Table(credential_scope));
    credential.insert(
        "token_hash".into(),
        Value::String(format!("blake3:{}", hex::encode(onboarding_token_hash))),
    );
    account_onboarding.insert(
        "credentials".into(),
        Value::Array(vec![Value::Table(credential)]),
    );
    if npos_bootstrap {
        account_onboarding.insert(
            "fee_sponsor_program_id".into(),
            Value::String(fee_sponsor_program_id),
        );
    }
    torii.insert(
        "account_onboarding".into(),
        Value::Table(account_onboarding),
    );
    let mut faucet = Table::new();
    faucet.insert("enabled".into(), Value::Boolean(true));
    faucet.insert(
        "authority".into(),
        Value::String(localnet_operator_account.clone()),
    );
    faucet.insert(
        "private_key_file".into(),
        Value::String(operator_private_key_file.to_string_lossy().into_owned()),
    );
    faucet.insert(
        "asset_definition_id".into(),
        Value::String(localnet_fee_asset_literal()),
    );
    faucet.insert(
        "amount".into(),
        Value::String(LOCALNET_FAUCET_AMOUNT.to_owned()),
    );
    faucet.insert(
        "pow_difficulty_bits".into(),
        Value::Integer(LOCALNET_FAUCET_POW_DIFFICULTY_BITS),
    );
    faucet.insert(
        "pow_scrypt_log_n".into(),
        Value::Integer(LOCALNET_FAUCET_POW_SCRYPT_LOG_N),
    );
    faucet.insert(
        "pow_scrypt_r".into(),
        Value::Integer(LOCALNET_FAUCET_POW_SCRYPT_R),
    );
    faucet.insert(
        "pow_scrypt_p".into(),
        Value::Integer(LOCALNET_FAUCET_POW_SCRYPT_P),
    );
    faucet.insert(
        "pow_max_anchor_age_blocks".into(),
        Value::Integer(LOCALNET_FAUCET_POW_MAX_ANCHOR_AGE_BLOCKS),
    );
    faucet.insert(
        "pow_adaptive_lookback_blocks".into(),
        Value::Integer(LOCALNET_FAUCET_POW_ADAPTIVE_LOOKBACK_BLOCKS),
    );
    faucet.insert(
        "pow_adaptive_claims_per_extra_bit".into(),
        Value::Integer(LOCALNET_FAUCET_POW_ADAPTIVE_CLAIMS_PER_EXTRA_BIT),
    );
    faucet.insert(
        "pow_adaptive_max_extra_bits".into(),
        Value::Integer(LOCALNET_FAUCET_POW_ADAPTIVE_MAX_EXTRA_BITS),
    );
    // Local generated networks do not have finalized public Taira VRF seed material.
    faucet.insert("pow_vrf_seed_enabled".into(), Value::Boolean(false));
    torii.insert("faucet".into(), Value::Table(faucet));
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
    chain_id: &str,
) -> Result<RawGenesisTransaction> {
    let chain_id = chain_id
        .parse::<ChainId>()
        .wrap_err("localnet chain id must be canonical")?;
    let npos_epoch_seed = matches!(consensus_mode, SumeragiConsensusMode::Npos)
        .then(|| localnet_npos_epoch_seed(&chain_id));
    let builder = GenesisBuilder::new_without_executor(chain_id, PathBuf::from("."));
    generate_default(
        builder,
        genesis_public_key,
        None,
        consensus_mode,
        None,
        npos_epoch_seed,
    )
}
fn extend_genesis(
    genesis: RawGenesisTransaction,
    genesis_account_id: &AccountId,
    seed_bytes: Option<&[u8]>,
    extra_accounts: u16,
    assets: &[AssetSpec],
) -> Result<RawGenesisTransaction> {
    let mut registrations = BootstrapRegistrations::from_manifest(&genesis);
    let mut builder = genesis.into_builder().next_transaction();
    for idx in 0..extra_accounts {
        let (pk, _) = generate_account_key_pair(seed_bytes, &format!("acct{idx}").into_bytes())
            .wrap_err_with(|| format!("failed to generate localnet extra account key {idx}"))?;
        let account_id = AccountId::new(pk.clone());
        if registrations.accounts.insert(account_id.clone()) {
            builder = builder.append_instruction(Register::account(Account::new(account_id)));
        }
    }
    for asset in assets {
        if registrations.accounts.insert(asset.owned_by.clone()) {
            builder =
                builder.append_instruction(Register::account(Account::new(asset.owned_by.clone())));
        }
        if registrations.accounts.insert(asset.mint_to.clone()) {
            builder =
                builder.append_instruction(Register::account(Account::new(asset.mint_to.clone())));
        }
        let asset_def = AssetDefinitionId::parse_address_literal(&asset.id)
            .wrap_err("invalid asset definition id")?;
        let definition = AssetDefinition::new(
            asset_def.clone(),
            asset.name.clone(),
            NumericSpec::default(),
            iroha_data_model::asset::AssetBalancePolicy::Global,
            None,
        )
        .with_metadata(Metadata::default());
        builder = builder.append_instruction(Register::asset_definition(definition));
        if let Some(alias_literal) = asset.alias.as_deref() {
            let alias = alias_literal
                .parse::<AssetDefinitionAlias>()
                .wrap_err("invalid asset definition alias")?;
            builder = builder.append_instruction(SetAssetDefinitionAlias::bind(
                asset_def.clone(),
                alias,
                None,
            ));
        }
        if asset.quantity > 0 {
            builder = builder.append_instruction(Mint::asset_quantity(
                asset.quantity,
                AssetId::new(asset_def.clone(), asset.mint_to.clone()),
            ));
        }
        if asset.owned_by != *genesis_account_id {
            builder = builder.append_instruction(Transfer::asset_definition(
                genesis_account_id.clone(),
                asset_def,
                asset.owned_by.clone(),
            ));
        }
    }
    Ok(builder.build_raw())
}
fn localnet_npos_epoch_seed(chain_id: &ChainId) -> [u8; 32] {
    let mut epoch_seed: [u8; 32] =
        Hash::new(format!("iroha:localnet:npos-epoch-seed:v1:{chain_id}")).into();
    if epoch_seed == [0; 32] {
        epoch_seed[0] = 1;
    }
    epoch_seed
}
fn apply_localnet_npos_overrides(parameters: &mut Parameters, chain_id: &ChainId) {
    let mut npos = parameters
        .custom()
        .get(&SumeragiNposParameters::parameter_id())
        .and_then(SumeragiNposParameters::from_custom_parameter)
        .unwrap_or_default();
    // Override seat band and bond to prevent validator drops on small localnets.
    npos.seat_band_pct = 100;
    npos.min_self_bond = 1_u64.into();
    npos.epoch_seed = localnet_npos_epoch_seed(chain_id);
    parameters.set_parameter(Parameter::Custom(npos.into_custom_parameter()));
}
fn localnet_custom_parameter_id(name: &str) -> CustomParameterId {
    CustomParameterId::new(
        name.parse()
            .expect("constant custom parameter name is valid"),
    )
}
fn localnet_ivm_gas_units_per_gas_payload(asset: &str) -> Json {
    let payload = format!(
        concat!(
            r#"[{{"asset":"{asset}","#,
            r#""units_per_gas":{units},"#,
            r#""twap_local_per_xor":"1","#,
            r#""liquidity_profile":"tier2","#,
            r#""volatility_class":"stable"}}]"#
        ),
        asset = asset,
        units = LOCALNET_IVM_GAS_UNITS_PER_GAS
    );
    Json::from_str_norito(&payload).expect("localnet gas-rate payload must be valid JSON")
}
fn apply_localnet_ivm_gas_limit_override(parameters: &mut Parameters) {
    let gas_param_id = localnet_custom_parameter_id("ivm_gas_limit_per_block");
    let gas_param = CustomParameter::new(gas_param_id, Json::new(LOCALNET_IVM_GAS_LIMIT_PER_BLOCK));
    parameters.set_parameter(Parameter::Custom(gas_param));
}
fn apply_localnet_ivm_gas_fee_overrides(parameters: &mut Parameters) {
    let fee_asset_id = localnet_fee_asset_literal();
    let accepted_assets = CustomParameter::new(
        localnet_custom_parameter_id("ivm_gas_accepted_assets"),
        Json::new(vec![fee_asset_id.clone()]),
    );
    parameters.set_parameter(Parameter::Custom(accepted_assets));
    let units_per_gas = CustomParameter::new(
        localnet_custom_parameter_id("ivm_gas_units_per_gas"),
        localnet_ivm_gas_units_per_gas_payload(&fee_asset_id),
    );
    parameters.set_parameter(Parameter::Custom(units_per_gas));
}
fn localnet_npos_stake_amount(parameters: &Parameters, requested: Option<u64>) -> Quantity {
    let requested = Quantity::from(requested.unwrap_or(LOCALNET_STAKE_AMOUNT));
    let min_self_bond = parameters
        .custom()
        .get(&SumeragiNposParameters::parameter_id())
        .and_then(SumeragiNposParameters::from_custom_parameter)
        .map_or_else(|| requested.clone(), |params| params.min_self_bond);
    requested.max(min_self_bond).max(Quantity::from(1_u64))
}
fn apply_parameter_overrides(
    genesis: RawGenesisTransaction,
    block_cadence_ms: Option<u64>,
    block_max_transactions: u64,
    consensus_mode: SumeragiConsensusMode,
) -> RawGenesisTransaction {
    let include_npos = matches!(consensus_mode, SumeragiConsensusMode::Npos);
    let mut parameters = genesis
        .effective_parameters()
        .expect("generated localnet genesis has one structured parameter block");
    let fee_asset_id = localnet_fee_asset_literal();
    let gas_limit_param_id = localnet_custom_parameter_id("ivm_gas_limit_per_block");
    let block_max_transactions =
        NonZeroU64::new(block_max_transactions).expect("block_max_transactions must be non-zero");
    let gas_fee_params_need_update = if include_npos {
        let accepted_assets_param_id = localnet_custom_parameter_id("ivm_gas_accepted_assets");
        let units_per_gas_param_id = localnet_custom_parameter_id("ivm_gas_units_per_gas");
        let accepted_assets_payload = Json::new(vec![fee_asset_id.clone()]);
        let units_per_gas_payload = localnet_ivm_gas_units_per_gas_payload(&fee_asset_id);
        parameters
            .custom()
            .get(&accepted_assets_param_id)
            .map(CustomParameter::payload)
            != Some(&accepted_assets_payload)
            || parameters
                .custom()
                .get(&units_per_gas_param_id)
                .map(CustomParameter::payload)
                != Some(&units_per_gas_payload)
    } else {
        false
    };
    let should_update = block_cadence_ms.is_some()
        || include_npos
        || parameters
            .custom()
            .get(&gas_limit_param_id)
            .and_then(|custom| custom.payload().try_into_any_norito::<u64>().ok())
            != Some(LOCALNET_IVM_GAS_LIMIT_PER_BLOCK)
        || gas_fee_params_need_update
        || parameters.block.max_transactions != block_max_transactions;
    if !should_update {
        return genesis;
    }
    parameters.block.max_transactions = block_max_transactions;
    if let Some(block_cadence_ms) = block_cadence_ms {
        parameters.sumeragi.block_cadence_ms =
            NonZeroU64::new(block_cadence_ms).expect("validated non-zero block cadence");
    }
    if include_npos {
        apply_localnet_npos_overrides(&mut parameters, genesis.chain_id());
    }
    apply_localnet_ivm_gas_limit_override(&mut parameters);
    if include_npos {
        apply_localnet_ivm_gas_fee_overrides(&mut parameters);
    }
    let mut builder = genesis.into_builder();
    if let Some(block_cadence_ms) = block_cadence_ms {
        builder = builder.with_block_cadence_ms(
            NonZeroU64::new(block_cadence_ms).expect("validated non-zero block cadence"),
        );
    }
    let pending_parameters = parameters.parameters().collect::<Vec<_>>();
    if !pending_parameters.is_empty() {
        builder = builder.next_transaction();
        for parameter in pending_parameters {
            builder = builder.append_parameter(parameter);
        }
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
#[cfg(test)]
fn append_localnet_contract_permissions(
    genesis: RawGenesisTransaction,
    genesis_account_id: &AccountId,
) -> RawGenesisTransaction {
    append_localnet_contract_permissions_for_client(
        genesis,
        genesis_account_id,
        &localnet_client_account_id(),
    )
}
fn append_localnet_service_accounts(
    genesis: RawGenesisTransaction,
    service_accounts: &[&AccountId],
) -> RawGenesisTransaction {
    let mut registered = genesis
        .instructions()
        .filter_map(|instruction| {
            let register = instruction.as_any().downcast_ref::<RegisterBox>()?;
            let RegisterBox::Account(register) = register else {
                return None;
            };
            Some(register.object.id.clone())
        })
        .collect::<BTreeSet<_>>();
    let mut builder = genesis.into_builder().next_transaction();
    for account_id in service_accounts {
        if registered.insert((*account_id).clone()) {
            builder =
                builder.append_instruction(Register::account(Account::new((*account_id).clone())));
        }
    }
    builder.build_raw()
}
fn append_localnet_alias_fee_bootstrap(
    genesis: RawGenesisTransaction,
    genesis_account_id: &AccountId,
    operator_account_id: &AccountId,
    onboarding_account_id: &AccountId,
) -> RawGenesisTransaction {
    let mut registrations = BootstrapRegistrations::from_manifest(&genesis);
    let universal_domain = DomainId::parse_fully_qualified(LOCALNET_UNIVERSAL_DOMAIN)
        .expect("static universal domain must remain canonical");
    let fee_asset_id = localnet_fee_asset_definition_id();
    // Continue the service-account transaction: these universal fee instructions consume
    // those accounts, and sharing their boundary keeps staged genesis within the protocol cap.
    let mut builder = genesis.into_builder();
    if registrations.domains.insert(universal_domain.clone()) {
        builder = builder.append_instruction(Register::domain(Domain::new(universal_domain)));
    }
    if registrations.asset_defs.insert(fee_asset_id.clone()) {
        let definition = AssetDefinition::new(
            fee_asset_id.clone(),
            "XOR".to_owned(),
            NumericSpec::fractional(LOCALNET_FEE_ASSET_SCALE),
            iroha_data_model::asset::AssetBalancePolicy::Global,
            None,
        )
        .with_metadata(Metadata::default());
        builder = builder.append_instruction(Register::asset_definition(definition));
    }
    builder = builder.append_instruction(Mint::asset_quantity(
        LOCALNET_ALIAS_SETUP_PAYER_BALANCE,
        AssetId::new(fee_asset_id.clone(), genesis_account_id.clone()),
    ));
    if operator_account_id != genesis_account_id {
        let operator_fee_asset = AssetId::new(fee_asset_id.clone(), operator_account_id.clone());
        builder = builder.append_instruction(Mint::asset_quantity(
            LOCALNET_ALIAS_SETUP_PAYER_BALANCE,
            operator_fee_asset,
        ));
        // The generated start script maintains this reserve, and Torii's faucet
        // transfers claims from it under the same operator authority.
        builder = builder.append_instruction(Grant::account_permission(
            CanMintAssetWithDefinition {
                asset_definition: fee_asset_id.clone(),
            },
            operator_account_id.clone(),
        ));
    }
    if onboarding_account_id != genesis_account_id && onboarding_account_id != operator_account_id {
        builder = builder.append_instruction(Mint::asset_quantity(
            LOCALNET_ALIAS_SETUP_PAYER_BALANCE,
            AssetId::new(fee_asset_id, onboarding_account_id.clone()),
        ));
    }
    builder.build_raw()
}
fn localnet_alias_setup_request(
    genesis_account_id: &AccountId,
    operator_account_id: &AccountId,
) -> Result<AliasSetupPlanRequestV1> {
    let dataspace_id = DataSpaceId::UNIVERSAL;
    let dataspace = ResolvedDataSpaceV1::new("universal".parse()?, dataspace_id);
    let domain = ResolvedDomainV1::new(
        DomainId::parse_fully_qualified(CLIENT_ACCOUNT_DOMAIN)?,
        dataspace_id,
    );
    let alias = ResolvedAccountAliasV1::new(
        LOCALNET_OPERATOR_ALIAS.parse::<AccountAliasName>()?,
        dataspace_id,
    );
    let guard = AliasQuoteGuardV1 {
        expected_policy_version: LOCALNET_ALIAS_SETUP_POLICY_VERSION,
        expected_payment_asset: localnet_fee_asset_definition_id(),
        max_amount: Quantity::from(LOCALNET_ALIAS_SETUP_PAYER_BALANCE),
        valid_until_ms: u64::MAX,
    };
    let acquisition = AliasLeaseAcquisitionV1::new(1, None);
    Ok(AliasSetupPlanRequestV1::new(vec![
        EnsureAlias::new(
            AliasIntentV1::Dataspace(AliasDataSpaceIntentV1 {
                dataspace,
                owner: genesis_account_id.clone(),
            }),
            acquisition,
            guard.clone(),
        ),
        EnsureAlias::new(
            AliasIntentV1::Domain(AliasDomainIntentV1 {
                domain,
                owner: genesis_account_id.clone(),
            }),
            acquisition,
            guard.clone(),
        ),
        EnsureAlias::new(
            AliasIntentV1::AccountAlias(AliasAccountIntentV1 {
                alias,
                target_account: operator_account_id.clone(),
                provision: AccountProvisionV1::Existing,
                role: AccountAliasRoleV1::Primary,
            }),
            acquisition,
            guard,
        ),
    ]))
}
fn append_localnet_alias_setup(
    genesis: RawGenesisTransaction,
    request: &AliasSetupPlanRequestV1,
    append_to_current_transaction: bool,
) -> RawGenesisTransaction {
    let mut builder = genesis.into_builder();
    if !append_to_current_transaction {
        builder = builder.next_transaction();
    }
    for ensure in request.intents.iter().cloned() {
        builder = builder.append_instruction(ensure);
    }
    builder.build_raw()
}
fn write_localnet_alias_setup_intent(
    out_dir: &Path,
    request: &AliasSetupPlanRequestV1,
) -> Result<PathBuf> {
    let path = out_dir.join(LOCALNET_ALIAS_SETUP_INTENT_FILE);
    let json = norito::json::to_json_pretty(request)
        .wrap_err("encode generated alias setup intent as canonical JSON")?;
    fs::write(&path, json)
        .wrap_err_with(|| format!("write generated alias setup intent {}", path.display()))?;
    Ok(path)
}
fn append_localnet_onboarding_permissions(
    genesis: RawGenesisTransaction,
    onboarding_account_id: &AccountId,
) -> Result<RawGenesisTransaction> {
    let domain = DomainId::parse_fully_qualified(CLIENT_ACCOUNT_DOMAIN)?;
    let permissions = [
        Permission::from(CanManageAccountAlias {
            scope: AccountAliasPermissionScope::Domain(domain.clone()),
        }),
        Permission::from(CanRegisterAccount {
            domain: domain.clone(),
        }),
        Permission::from(CanPublishSpaceDirectoryManifestForAccountDomain {
            dataspace: DataSpaceId::UNIVERSAL,
            domain,
        }),
    ];
    let mut existing = genesis
        .instructions()
        .filter_map(|instruction| {
            let grant = instruction.as_any().downcast_ref::<GrantBox>()?;
            let GrantBox::Permission(grant) = grant else {
                return None;
            };
            Some((grant.destination().clone(), grant.object().clone()))
        })
        .collect::<BTreeSet<_>>();
    // The preceding contract grants and these onboarding grants all target the universal
    // execution world, so keep them in one routing and staging boundary.
    let mut builder = genesis.into_builder();
    for permission in permissions {
        if existing.insert((onboarding_account_id.clone(), permission.clone())) {
            builder = builder.append_instruction(Grant::account_permission(
                permission,
                onboarding_account_id.clone(),
            ));
        }
    }
    Ok(builder.build_raw())
}
fn append_localnet_contract_permissions_for_client(
    genesis: RawGenesisTransaction,
    genesis_account_id: &AccountId,
    client_account_id: &AccountId,
) -> RawGenesisTransaction {
    let enact_governance: Permission = CanEnactGovernance.into();
    let manage_offline_escrow = Permission::new("CanManageOfflineEscrow".into(), Json::new(()));
    let manage_verifying_keys = Permission::new("CanManageVerifyingKeys".into(), Json::new(()));
    let manage_account_alias: Permission = CanManageAccountAlias {
        scope: AccountAliasPermissionScope::Dataspace(DataSpaceId::UNIVERSAL),
    }
    .into();
    let publish_manifest: Permission = CanPublishSpaceDirectoryManifest {
        dataspace: DataSpaceId::UNIVERSAL,
    }
    .into();
    let mut seen: BTreeSet<(AccountId, Permission)> = genesis
        .instructions()
        .filter_map(|instruction| {
            let grant = instruction.as_any().downcast_ref::<GrantBox>()?;
            let GrantBox::Permission(grant_permission) = grant else {
                return None;
            };
            Some((
                grant_permission.destination().clone(),
                grant_permission.object().clone(),
            ))
        })
        .collect();
    let mut grants = Vec::new();
    let mut push_unique = |permission: Permission, destination: AccountId| {
        if seen.insert((destination.clone(), permission.clone())) {
            grants.push((permission, destination));
        }
    };
    push_unique(enact_governance, ALICE_ID.clone());
    push_unique(manage_verifying_keys.clone(), genesis_account_id.clone());
    push_unique(manage_verifying_keys, client_account_id.clone());
    push_unique(manage_account_alias, client_account_id.clone());
    push_unique(publish_manifest, client_account_id.clone());
    if *client_account_id != *ALICE_ID {
        push_unique(manage_offline_escrow, client_account_id.clone());
    }
    let mut builder = genesis.into_builder();
    for (permission, destination) in grants {
        builder = builder.append_instruction(Grant::account_permission(permission, destination));
    }
    builder.build_raw()
}
struct BootstrapRegistrations {
    domains: BTreeSet<DomainId>,
    accounts: BTreeSet<AccountId>,
    asset_defs: BTreeSet<AssetDefinitionId>,
    zk_assets: BTreeSet<AssetDefinitionId>,
    verifying_keys: BTreeSet<VerifyingKeyId>,
}
impl BootstrapRegistrations {
    fn from_manifest(manifest: &RawGenesisTransaction) -> Self {
        let mut domains = BTreeSet::new();
        let mut accounts = BTreeSet::new();
        let mut asset_defs = BTreeSet::new();
        let mut zk_assets = BTreeSet::new();
        let mut verifying_keys = BTreeSet::new();
        for instruction in manifest.instructions() {
            if let Some(register) = instruction
                .as_any()
                .downcast_ref::<iroha_data_model::isi::zk::RegisterZkAsset>()
            {
                zk_assets.insert(register.asset().clone());
                continue;
            }
            if let Some(register) = instruction
                .as_any()
                .downcast_ref::<verifying_keys::RegisterVerifyingKey>()
            {
                verifying_keys.insert(register.id.clone());
                continue;
            }
            let Some(register) = instruction.as_any().downcast_ref::<RegisterBox>() else {
                continue;
            };
            match register {
                RegisterBox::Domain(register) => {
                    domains.insert(register.object.id.clone());
                }
                RegisterBox::Account(register) => {
                    accounts.insert(register.object.id.clone());
                }
                RegisterBox::AssetDefinition(register) => {
                    asset_defs.insert(register.object.id.clone());
                }
                _ => {}
            }
        }
        Self {
            domains,
            accounts,
            asset_defs,
            zk_assets,
            verifying_keys,
        }
    }
}
#[cfg(test)]
#[allow(clippy::too_many_lines)]
fn append_localnet_npos_bootstrap(
    genesis: RawGenesisTransaction,
    peers: &[Peer],
    gas_account_id: &AccountId,
    stake_amount: &Quantity,
    sora_profile: Option<SoraProfile>,
    genesis_account_id: &AccountId,
) -> Result<RawGenesisTransaction> {
    append_localnet_npos_bootstrap_for_client(
        genesis,
        peers,
        gas_account_id,
        stake_amount,
        sora_profile,
        genesis_account_id,
        &localnet_client_account_id(),
    )
}
#[cfg(test)]
fn append_localnet_npos_bootstrap_for_client(
    genesis: RawGenesisTransaction,
    peers: &[Peer],
    gas_account_id: &AccountId,
    stake_amount: &Quantity,
    sora_profile: Option<SoraProfile>,
    genesis_account_id: &AccountId,
    client_account_id: &AccountId,
) -> Result<RawGenesisTransaction> {
    append_localnet_npos_bootstrap_for_services(
        genesis,
        peers,
        gas_account_id,
        stake_amount,
        sora_profile,
        genesis_account_id,
        client_account_id,
        client_account_id,
    )
}
#[expect(
    clippy::too_many_arguments,
    clippy::too_many_lines,
    reason = "the ordered NPoS bootstrap matrix stays linear so transaction ordering remains auditable"
)]
fn append_localnet_npos_bootstrap_for_services(
    genesis: RawGenesisTransaction,
    peers: &[Peer],
    gas_account_id: &AccountId,
    stake_amount: &Quantity,
    sora_profile: Option<SoraProfile>,
    genesis_account_id: &AccountId,
    client_account_id: &AccountId,
    onboarding_account_id: &AccountId,
) -> Result<RawGenesisTransaction> {
    let nexus_domain = DomainId::parse_fully_qualified(LOCALNET_NEXUS_DOMAIN)?;
    let ivm_domain = DomainId::parse_fully_qualified(LOCALNET_IVM_DOMAIN)?;
    let universal_domain = DomainId::parse_fully_qualified(LOCALNET_UNIVERSAL_DOMAIN)?;
    let stake_asset_id = localnet_stake_asset_definition_id();
    let fee_asset_id = localnet_fee_asset_definition_id();
    let public_validator_lanes = localnet_public_validator_lanes(sora_profile);
    let lane_count = u64::try_from(public_validator_lanes.len())
        .expect("public validator lane count must fit in u64");
    let stake_mint_amount = stake_amount
        .try_mul_decimal(&Numeric::from(lane_count))
        .map_err(|error| eyre!("localnet validator stake mint amount overflow: {error}"))?;
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
    if !registrations.domains.contains(&universal_domain) {
        builder =
            builder.append_instruction(Register::domain(Domain::new(universal_domain.clone())));
        registrations.domains.insert(universal_domain.clone());
    }
    if !registrations.accounts.contains(gas_account_id) {
        builder =
            builder.append_instruction(Register::account(Account::new(gas_account_id.clone())));
        registrations.accounts.insert(gas_account_id.clone());
    }
    if !registrations.asset_defs.contains(&stake_asset_id) {
        let definition = AssetDefinition::new(
            stake_asset_id.clone(),
            "Localnet Stake".to_owned(),
            NumericSpec::default(),
            iroha_data_model::asset::AssetBalancePolicy::Global,
            None,
        )
        .with_metadata(Metadata::default());
        builder = builder.append_instruction(Register::asset_definition(definition));
        registrations.asset_defs.insert(stake_asset_id.clone());
    }
    if !registrations.asset_defs.contains(&fee_asset_id) {
        let definition = AssetDefinition::new(
            fee_asset_id.clone(),
            "XOR".to_owned(),
            NumericSpec::fractional(LOCALNET_FEE_ASSET_SCALE),
            iroha_data_model::asset::AssetBalancePolicy::Global,
            None,
        )
        .with_metadata(Metadata::default());
        builder = builder.append_instruction(Register::asset_definition(definition));
        registrations.asset_defs.insert(fee_asset_id.clone());
    }
    let fee_vk_unshield_id = localnet_fee_vk_unshield_id();
    for (id, record) in localnet_confidential_fee_vk_registrations()? {
        if registrations.verifying_keys.insert(id.clone()) {
            builder =
                builder.append_instruction(verifying_keys::RegisterVerifyingKey { id, record });
        }
    }
    if !registrations.zk_assets.contains(&fee_asset_id) {
        builder = builder.append_instruction(iroha_data_model::isi::zk::RegisterZkAsset::new(
            fee_asset_id.clone(),
            Some(fee_vk_unshield_id),
            None,
        ));
        registrations.zk_assets.insert(fee_asset_id.clone());
    }
    for peer in peers {
        let validator_id = AccountId::new(peer.public_key.clone());
        if !registrations.accounts.contains(&validator_id) {
            builder =
                builder.append_instruction(Register::account(Account::new(validator_id.clone())));
            registrations.accounts.insert(validator_id.clone());
        }
        builder = builder.append_instruction(Mint::asset_quantity(
            stake_mint_amount.clone(),
            AssetId::new(stake_asset_id.clone(), validator_id.clone()),
        ));
        builder = builder.append_instruction(Mint::asset_quantity(
            stake_amount.clone(),
            AssetId::new(fee_asset_id.clone(), validator_id.clone()),
        ));
    }
    if !registrations.accounts.contains(client_account_id) {
        builder =
            builder.append_instruction(Register::account(Account::new(client_account_id.clone())));
        registrations.accounts.insert(client_account_id.clone());
    }
    if !registrations.accounts.contains(onboarding_account_id) {
        builder = builder.append_instruction(Register::account(Account::new(
            onboarding_account_id.clone(),
        )));
        registrations.accounts.insert(onboarding_account_id.clone());
    }
    builder = builder.append_instruction(Mint::asset_quantity(
        LOCALNET_FAUCET_AUTHORITY_BALANCE,
        AssetId::new(fee_asset_id.clone(), client_account_id.clone()),
    ));
    if onboarding_account_id != client_account_id {
        builder = builder.append_instruction(Mint::asset_quantity(
            LOCALNET_FAUCET_AUTHORITY_BALANCE,
            AssetId::new(fee_asset_id.clone(), onboarding_account_id.clone()),
        ));
    }
    let fee_sponsor_program_id = localnet_fee_sponsor_program_id(genesis_account_id);
    let fee_sponsor_revision =
        localnet_fee_sponsor_revision(fee_sponsor_program_id.clone(), fee_asset_id.clone());
    fee_sponsor_revision
        .validate()
        .map_err(|error| eyre!("invalid localnet fee sponsor revision: {error}"))?;
    builder = builder.append_instruction(Mint::asset_quantity(
        LOCALNET_FEE_SPONSOR_VAULT_BALANCE,
        AssetId::new(fee_asset_id.clone(), genesis_account_id.clone()),
    ));
    builder = builder.append_instruction(CreateFeeSponsorProgram {
        program: FeeSponsorProgram::new(fee_sponsor_program_id.clone(), genesis_account_id.clone()),
    });
    builder = builder.append_instruction(StageFeeSponsorProgramRevision {
        revision: fee_sponsor_revision,
    });
    builder = builder.append_instruction(EnrollFeeSponsorBeneficiary {
        program_id: fee_sponsor_program_id.clone(),
        beneficiary: client_account_id.clone(),
    });
    if onboarding_account_id != client_account_id {
        builder = builder.append_instruction(EnrollFeeSponsorBeneficiary {
            program_id: fee_sponsor_program_id.clone(),
            beneficiary: onboarding_account_id.clone(),
        });
    }
    builder = builder.append_instruction(FundFeeSponsorProgram {
        program_id: fee_sponsor_program_id.clone(),
        asset_definition_id: fee_asset_id,
        amount: Quantity::from(LOCALNET_FEE_SPONSOR_VAULT_BALANCE),
    });
    builder = builder.append_instruction(ActivateFeeSponsorProgramRevision {
        program_id: fee_sponsor_program_id.clone(),
        revision: 1,
        activate_at_height: 1,
    });
    let enroll_permission = CanEnrollFeeSponsorProgram {
        program_id: fee_sponsor_program_id,
    };
    builder = builder.append_instruction(Grant::account_permission(
        enroll_permission.clone(),
        client_account_id.clone(),
    ));
    if onboarding_account_id != client_account_id {
        builder = builder.append_instruction(Grant::account_permission(
            enroll_permission,
            onboarding_account_id.clone(),
        ));
    }
    for lane_id in public_validator_lanes {
        builder = builder.next_transaction();
        for peer in peers {
            let validator_id = AccountId::new(peer.public_key.clone());
            builder = builder.append_instruction(RegisterPublicLaneValidator {
                lane_id,
                validator: validator_id.clone(),
                peer_id: PeerId::from(peer.public_key.clone()),
                stake_account: validator_id.clone(),
                initial_stake: stake_amount.clone(),
                metadata: Metadata::default(),
            });
            builder = builder.append_instruction(ActivatePublicLaneValidator {
                lane_id,
                validator: validator_id,
            });
        }
    }
    Ok(builder.build_raw())
}
#[allow(clippy::too_many_lines)]
fn append_private_dataspace_genesis_bootstrap_for_client(
    genesis: RawGenesisTransaction,
    sora_profile: Option<SoraProfile>,
    genesis_account_id: &AccountId,
    client_account_id: &AccountId,
) -> Result<RawGenesisTransaction> {
    let domains: &[&str] = match sora_profile {
        Some(SoraProfile::PrivateSbp) => SBP_BOOTSTRAP_DOMAINS,
        Some(SoraProfile::PrivateCbuae) => &[],
        _ => return Ok(genesis),
    };
    let spec = private_dataspace_spec(sora_profile)
        .expect("private bootstrap profiles must have a private dataspace spec");
    let payment_amount: Quantity = LOCALNET_PRIVATE_SNS_LEASE_PAYMENT
        .parse()
        .map_err(|error| eyre!("invalid localnet private SNS lease payment: {error}"))?;
    let private_dataspace = DataSpaceId::new(spec.id);
    let acquisition = AliasLeaseAcquisitionV1::new(1, None);
    let quote_guard = AliasQuoteGuardV1 {
        expected_policy_version: LOCALNET_ALIAS_SETUP_POLICY_VERSION,
        expected_payment_asset: localnet_fee_asset_definition_id(),
        max_amount: payment_amount,
        valid_until_ms: u64::MAX,
    };
    let mut ensure_aliases = vec![EnsureAlias::new(
        AliasIntentV1::Dataspace(AliasDataSpaceIntentV1 {
            dataspace: ResolvedDataSpaceV1::new(spec.alias.parse()?, private_dataspace),
            owner: client_account_id.clone(),
        }),
        acquisition,
        quote_guard.clone(),
    )];
    for domain in domains {
        ensure_aliases.push(EnsureAlias::new(
            AliasIntentV1::Domain(AliasDomainIntentV1 {
                domain: ResolvedDomainV1::new(
                    DomainId::parse_fully_qualified(domain)?,
                    private_dataspace,
                ),
                owner: client_account_id.clone(),
            }),
            acquisition,
            quote_guard.clone(),
        ));
    }
    // Genesis executes these private-resource intents under the genesis authority while
    // retaining the client as their explicit owner. Install only the exact scopes required
    // in an ephemeral role, then remove that role before the transaction commits. A direct
    // domain-scoped grant cannot bootstrap a missing domain because grant execution resolves
    // the domain before the following `EnsureAlias` has a chance to create it.
    let mut temporary_genesis_permissions = ensure_aliases
        .iter()
        .map(|ensure| match &ensure.intent {
            AliasIntentV1::Dataspace(intent) => Permission::from(CanManageAccountAlias {
                scope: AccountAliasPermissionScope::Dataspace(intent.dataspace.dataspace_id),
            }),
            AliasIntentV1::Domain(intent) => Permission::from(CanManageAccountAlias {
                scope: AccountAliasPermissionScope::Domain(intent.domain.canonical_name.clone()),
            }),
            AliasIntentV1::AccountAlias(_) => {
                unreachable!("private genesis bootstrap contains no account-alias intents")
            }
        })
        .collect::<Vec<_>>();
    let mut seen_permissions = BTreeSet::<(AccountId, Permission)>::new();
    // Genesis bootstrap also pre-seeds management scopes for registered account labels.
    // Treat those as pre-existing so cleanup never revokes authority the manifest already had.
    for instruction in genesis.instructions() {
        let Some(RegisterBox::Account(register)) =
            instruction.as_any().downcast_ref::<RegisterBox>()
        else {
            continue;
        };
        let Some(label) = register.object().label() else {
            continue;
        };
        if label.dataspace != private_dataspace {
            continue;
        }
        seen_permissions.insert((
            genesis_account_id.clone(),
            Permission::from(CanManageAccountAlias {
                scope: AccountAliasPermissionScope::Dataspace(private_dataspace),
            }),
        ));
        if let Some(domain) = &label.domain {
            seen_permissions.insert((
                genesis_account_id.clone(),
                Permission::from(CanManageAccountAlias {
                    scope: AccountAliasPermissionScope::Domain(DomainId::parse_fully_qualified(
                        &format!("{}.{}", domain.name(), spec.alias),
                    )?),
                }),
            ));
        }
    }
    // Apply explicit grants and revokes in manifest order on top of the pre-seeded label
    // scopes. This distinguishes authority that remains present from a historical grant that
    // was already revoked before the private bootstrap transaction.
    for instruction in genesis.instructions() {
        if let Some(GrantBox::Permission(grant)) = instruction.as_any().downcast_ref::<GrantBox>() {
            seen_permissions.insert((grant.destination().clone(), grant.object().clone()));
        }
        if let Some(RevokeBox::Permission(revoke)) =
            instruction.as_any().downcast_ref::<RevokeBox>()
        {
            seen_permissions.remove(&(revoke.destination().clone(), revoke.object().clone()));
        }
    }
    temporary_genesis_permissions.retain(|permission| {
        seen_permissions.insert((genesis_account_id.clone(), permission.clone()))
    });
    let temporary_genesis_role_id: RoleId = format!(
        "private_{}_dataspace_{}_alias_bootstrap",
        spec.alias,
        private_dataspace.as_u64()
    )
    .parse()
    .expect("private localnet aliases must produce a valid role id");
    if genesis.instructions().any(|instruction| {
        instruction
            .as_any()
            .downcast_ref::<RegisterBox>()
            .is_some_and(|register| match register {
                RegisterBox::Role(register) => {
                    register.object().inner().id == temporary_genesis_role_id
                }
                _ => false,
            })
    }) {
        return Err(eyre!(
            "private-dataspace bootstrap refuses a pre-existing temporary setup role `{temporary_genesis_role_id}`"
        ));
    }
    let restricted_read_permission = Permission::from(CanReadRestrictedDataspace {
        dataspace: private_dataspace,
    });
    if seen_permissions.contains(&(
        client_account_id.clone(),
        restricted_read_permission.clone(),
    )) {
        return Err(eyre!(
            "private-dataspace bootstrap requires explicit restricted-read grants in both authorization worlds; refusing an ambiguous pre-existing grant for `{client_account_id}`"
        ));
    }
    let restricted_reader_role_id =
        crate::genesis::private_dataspace_reader_role_id(spec.alias, private_dataspace);
    if genesis.instructions().any(|instruction| {
        instruction
            .as_any()
            .downcast_ref::<RegisterBox>()
            .is_some_and(|register| match register {
                RegisterBox::Role(register) => {
                    let role = register.object();
                    role.inner().id == restricted_reader_role_id
                        || role
                            .inner()
                            .permissions()
                            .any(|permission| permission == &restricted_read_permission)
                }
                _ => false,
            })
    }) {
        return Err(eyre!(
            "private-dataspace bootstrap refuses a pre-existing restricted-reader role for `{client_account_id}`"
        ));
    }
    let mut builder = genesis.into_builder().next_transaction();
    let temporary_genesis_role = temporary_genesis_permissions.iter().cloned().fold(
        Role::new(
            temporary_genesis_role_id.clone(),
            genesis_account_id.clone(),
        ),
        |role, permission| role.add_permission(permission),
    );
    builder = builder.append_instruction(Register::role(temporary_genesis_role));
    for ensure in ensure_aliases {
        builder = builder.append_instruction(ensure);
    }
    builder = builder.append_instruction(Unregister::role(temporary_genesis_role_id));
    builder = builder.append_instruction(Grant::account_permission(
        restricted_read_permission.clone(),
        client_account_id.clone(),
    ));
    let universal_permissions = vec![
        Permission::from(CanManageAccountAlias {
            scope: AccountAliasPermissionScope::Dataspace(DataSpaceId::UNIVERSAL),
        }),
        Permission::from(CanResolveAccountAlias {
            scope: AccountAliasPermissionScope::Dataspace(DataSpaceId::UNIVERSAL),
        }),
    ];
    let universal_permissions = universal_permissions
        .into_iter()
        .filter(|permission| {
            seen_permissions.insert((client_account_id.clone(), permission.clone()))
        })
        .collect::<Vec<_>>();
    // Keep the universal ingress role and ancillary universal permissions separate from the
    // private EnsureAlias transaction so the router never collapses either authorization world
    // into the universal coordinator. The private world receives the direct grant above, while
    // Torii's universal ingress hop reads the same capability from this native role.
    builder = builder
        .next_transaction()
        .append_instruction(Register::role(
            Role::new(restricted_reader_role_id, client_account_id.clone())
                .add_permission(restricted_read_permission),
        ));
    for permission in universal_permissions {
        builder = builder.append_instruction(Grant::account_permission(
            permission,
            client_account_id.clone(),
        ));
    }
    Ok(builder.build_raw())
}
struct GenesisConsensusPolicies {
    da_proof_policies: Option<DaProofPolicyBundle>,
    confidential_policy_hash: [u8; 32],
}
struct GenesisWriteContext<'a> {
    manifest: &'a RawGenesisTransaction,
    public_key: &'a iroha_crypto::PublicKey,
    private_key: ExposedPrivateKey,
    config: &'a actual::Root,
    chain_discriminant: Option<u16>,
    json_path: &'a Path,
    signed_path: &'a Path,
    policies: GenesisConsensusPolicies,
}
fn write_genesis(context: GenesisWriteContext<'_>) -> Result<HashOf<BlockHeader>> {
    let GenesisWriteContext {
        manifest,
        public_key,
        private_key,
        config,
        chain_discriminant,
        json_path,
        signed_path,
        policies,
    } = context;
    let chain_discriminant =
        chain_discriminant.unwrap_or_else(iroha_data_model::account::address::chain_discriminant);
    let genesis = manifest.clone().with_chain_discriminant(chain_discriminant);
    let _chain_discriminant = Some(ChainDiscriminantGuard::enter(chain_discriminant));
    let json = norito::json::to_json_pretty(&genesis)?;
    validate_genesis_manifest_json(json.as_bytes())
        .wrap_err("generated genesis.json exceeds fixed resource bounds")?;
    fs::write(json_path, json).wrap_err("failed to write genesis.json")?;
    drop(genesis);
    // Sign the exact persisted manifest. Custom JSON parameter payloads can have a different
    // textual key order before and after the manifest's JSON round trip; signing the reloaded
    // form keeps genesis.json and genesis.signed.nrt semantically and canonically aligned.
    let persisted_genesis = RawGenesisTransaction::from_path(json_path)
        .wrap_err("failed to reload persisted genesis.json before signing")?;
    let genesis_key_pair =
        KeyPair::new(public_key.clone(), private_key.0).wrap_err("make genesis key pair")?;
    let (bound_manifest, block) = crate::genesis::bind_and_sign_staged_sumeragi_v2_context(
        persisted_genesis,
        &genesis_key_pair,
        Some(config),
        policies.da_proof_policies,
        policies.confidential_policy_hash,
        None,
    )
    .wrap_err("stage and sign genesis block")?;
    let mut bound_json =
        norito::json::to_json_pretty(&bound_manifest).wrap_err("encode bound genesis manifest")?;
    bound_json.push('\n');
    validate_genesis_manifest_json(bound_json.as_bytes())
        .wrap_err("bound genesis.json exceeds fixed resource bounds")?;
    fs::write(json_path, bound_json).wrap_err("write bound genesis.json")?;
    drop(bound_manifest);
    let expected_hash = block.0.hash();
    let framed = block.0.encode_wire().wrap_err("frame genesis block")?;
    drop(block);
    if framed.len() > SIGNED_GENESIS_MAX_BYTES_V1 {
        return Err(eyre!(
            "generated signed genesis body is {} bytes, exceeding the {}-byte first-release limit",
            framed.len(),
            SIGNED_GENESIS_MAX_BYTES_V1
        ));
    }
    let mut file = BufWriter::new(File::create(signed_path)?);
    file.write_all(&framed)?;
    Ok(expected_hash)
}
fn write_and_validate_genesis_expected_hash(
    expected_hash_path: &Path,
    signed_path: &Path,
    expected_hash: HashOf<BlockHeader>,
) -> Result<()> {
    let decoded = read_signed_genesis(signed_path)
        .wrap_err("read and decode the generated signed genesis body")?;
    if decoded.hash() != expected_hash {
        return Err(eyre!(
            "generated signed genesis body hashes to {}, expected {}",
            decoded.hash(),
            expected_hash
        ));
    }
    let canonical = expected_hash.to_string();
    let canonical_bytes = canonical.as_bytes();
    let has_canonical_syntax = canonical_bytes.len() == 64
        && canonical_bytes
            .iter()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
        && canonical_bytes.last().is_some_and(|byte| {
            matches!(*byte, b'1' | b'3' | b'5' | b'7' | b'9' | b'b' | b'd' | b'f')
        });
    if !has_canonical_syntax {
        return Err(eyre!(
            "generated genesis hash is not a canonical Iroha hash record: {canonical}"
        ));
    }
    let record = format!("{canonical}\n");
    write_owner_only_localnet_file(expected_hash_path, record.as_bytes()).wrap_err_with(|| {
        format!(
            "write exact genesis hash file {}",
            expected_hash_path.display()
        )
    })?;
    let persisted = fs::read_to_string(expected_hash_path).wrap_err_with(|| {
        format!(
            "read exact genesis hash file {}",
            expected_hash_path.display()
        )
    })?;
    if persisted != record {
        return Err(eyre!(
            "persisted exact genesis hash file is not the canonical generated record"
        ));
    }
    let parsed = persisted
        .strip_suffix('\n')
        .expect("canonical record always ends in a newline")
        .parse::<HashOf<BlockHeader>>()
        .wrap_err("parse persisted exact genesis hash")?;
    if parsed != expected_hash {
        return Err(eyre!(
            "persisted exact genesis hash changed from {expected_hash} to {parsed}"
        ));
    }
    Ok(())
}
fn write_genesis_key_files(
    public_path: &Path,
    private_path: &Path,
    public_key: &iroha_crypto::PublicKey,
    private_key: &ExposedPrivateKey,
    private_tree: bool,
) -> Result<()> {
    let canonical = Zeroizing::new(
        private_key
            .try_to_multihash_string()
            .wrap_err("encode genesis private key")?,
    );
    let mut raw = Zeroizing::new(Vec::with_capacity(canonical.len() + 1));
    raw.extend_from_slice(canonical.as_bytes());
    raw.push(b'\n');
    if private_tree {
        crate::secure_fs::write_private_file_atomic(private_path, raw.as_slice())
            .wrap_err("write genesis private key")?;
    } else {
        write_owner_only_localnet_file(private_path, raw.as_slice())
            .wrap_err("write genesis private key")?;
    }
    let mut public = public_key.to_string();
    public.push('\n');
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    let mut file = options
        .open(public_path)
        .wrap_err_with(|| format!("create genesis public-key file {}", public_path.display()))?;
    file.write_all(public.as_bytes())
        .wrap_err_with(|| format!("write genesis public-key file {}", public_path.display()))
}
fn parse_localnet_peer_config(rendered_config: &str) -> Result<actual::Root> {
    let table = rendered_config.parse::<toml::Table>().map_err(|err| {
        eyre!(
            "failed to parse rendered peer config as TOML while deriving consensus policies: {err}"
        )
    })?;
    // Scope validation to the chain used to render account-typed fields.
    let chain_discriminant = table
        .get("chain_discriminant")
        .and_then(toml::Value::as_integer)
        .and_then(|value| u16::try_from(value).ok());
    let _chain_discriminant = chain_discriminant.map(ChainDiscriminantGuard::enter);
    actual::Root::from_toml_source(TomlSource::inline(table)).map_err(|err| {
        eyre!("failed to parse generated peer config while deriving consensus policies: {err:?}")
    })
}
fn resolve_localnet_da_proof_policies(config: &actual::Root) -> DaProofPolicyBundle {
    iroha_core::da::proof_policy_bundle(&config.nexus.lane_config)
}
pub(crate) fn generate_genesis_key_pair(
    base_seed: Option<&[u8]>,
    extra_seed: &[u8],
) -> Result<(iroha_crypto::PublicKey, ExposedPrivateKey)> {
    let key_pair = match base_seed {
        Some(base_seed) => iroha_crypto::KeyPair::try_from_seed(
            base_seed
                .iter()
                .chain(extra_seed)
                .copied()
                .collect::<Vec<_>>(),
            iroha_crypto::Algorithm::default(),
        )?,
        #[cfg(test)]
        None => KeyPair::from(REAL_GENESIS_ACCOUNT_KEYPAIR.private_key().clone()),
        #[cfg(not(test))]
        None => {
            iroha_crypto::KeyPair::try_random_with_algorithm(iroha_crypto::Algorithm::default())?
        }
    };
    let (public_key, private_key) = key_pair.into_parts();
    Ok((public_key, ExposedPrivateKey(private_key)))
}
fn generate_account_key_pair(
    base_seed: Option<&[u8]>,
    extra_seed: &[u8],
) -> Result<(iroha_crypto::PublicKey, ExposedPrivateKey)> {
    let key_pair = match base_seed {
        Some(seed) => iroha_crypto::KeyPair::try_from_seed(
            seed.iter().chain(extra_seed).copied().collect::<Vec<_>>(),
            iroha_crypto::Algorithm::default(),
        )?,
        None => {
            iroha_crypto::KeyPair::try_random_with_algorithm(iroha_crypto::Algorithm::default())?
        }
    };
    let (public_key, private_key) = key_pair.into_parts();
    Ok((public_key, ExposedPrivateKey(private_key)))
}
fn generate_bls_key_pair(
    base_seed: Option<&[u8]>,
    extra_seed: &[u8],
) -> Result<(iroha_crypto::PublicKey, ExposedPrivateKey, Vec<u8>)> {
    let kp = match base_seed {
        Some(seed) => {
            let material = seed.iter().chain(extra_seed).copied().collect::<Vec<_>>();
            iroha_crypto::KeyPair::try_from_seed(material, iroha_crypto::Algorithm::BlsNormal)?
        }
        None => {
            iroha_crypto::KeyPair::try_random_with_algorithm(iroha_crypto::Algorithm::BlsNormal)?
        }
    };
    let pop = iroha_crypto::bls_normal_pop_prove(kp.private_key())?;
    let (public_key, private_key) = kp.into_parts();
    Ok((public_key, ExposedPrivateKey(private_key), pop))
}
fn generate_soranet_transport_key_pair(
    base_seed: Option<&[u8]>,
    peer_index: &[u8],
) -> Result<(iroha_crypto::PublicKey, ExposedPrivateKey)> {
    generate_peer_ed25519_key_pair(base_seed, SORANET_TRANSPORT_SEED_DOMAIN, peer_index)
}
fn generate_streaming_identity_key_pair(
    base_seed: Option<&[u8]>,
    peer_index: &[u8],
) -> Result<(iroha_crypto::PublicKey, ExposedPrivateKey)> {
    generate_peer_ed25519_key_pair(base_seed, STREAMING_IDENTITY_SEED_DOMAIN, peer_index)
}
fn generate_peer_ed25519_key_pair(
    base_seed: Option<&[u8]>,
    seed_domain: &[u8],
    peer_index: &[u8],
) -> Result<(iroha_crypto::PublicKey, ExposedPrivateKey)> {
    let key_pair = match base_seed {
        Some(seed) => KeyPair::try_from_seed(
            seed.iter()
                .chain(seed_domain)
                .chain(peer_index)
                .copied()
                .collect::<Vec<_>>(),
            iroha_crypto::Algorithm::Ed25519,
        )?,
        None => KeyPair::try_random_with_algorithm(iroha_crypto::Algorithm::Ed25519)?,
    };
    let (public_key, private_key) = key_pair.into_parts();
    Ok((public_key, ExposedPrivateKey(private_key)))
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
        target_dir.join("debug").join("iroha3d"),
        target_dir.join("release").join("iroha3d"),
    )
}
fn write_scripts(
    out_dir: &Path,
    peers: u16,
    sora_profile_enabled: bool,
    client_account_literal: &str,
    fee_asset_definition_id: &str,
) -> Result<()> {
    let start = out_dir.join("start.sh");
    let stop = out_dir.join("stop.sh");
    write_start_script(
        &start,
        peers,
        sora_profile_enabled,
        client_account_literal,
        fee_asset_definition_id,
    )?;
    write_stop_script(&stop)?;
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
#[allow(clippy::similar_names, clippy::too_many_lines)]
fn write_start_script(
    start: &Path,
    peers: u16,
    sora_profile_enabled: bool,
    client_account_literal: &str,
    fee_asset_definition_id: &str,
) -> Result<()> {
    let (default_irohad_debug, default_irohad_release) = default_irohad_bin_paths();
    let default_iroha_debug = default_irohad_debug.with_file_name("iroha");
    let default_iroha_release = default_irohad_release.with_file_name("iroha");
    let mut start_file = BufWriter::new(File::create(start)?);
    let sora_flag = if sora_profile_enabled { "--sora " } else { "" };
    let sora_mode_env = if sora_profile_enabled { "1" } else { "0" };
    writeln!(start_file, "#!/usr/bin/env bash")?;
    writeln!(start_file, "set -euo pipefail")?;
    writeln!(start_file, "umask 077")?;
    writeln!(start_file, "DIR=$(cd \"$(dirname \"$0\")\" && pwd)")?;
    writeln!(start_file, "cd \"$DIR\"")?;
    writeln!(start_file, "pid_is_running() {{")?;
    writeln!(start_file, "  pid=\"$1\"")?;
    writeln!(
        start_file,
        "  case \"$pid\" in ''|*[!0-9]*) return 1 ;; esac"
    )?;
    writeln!(start_file, "  command -v ps >/dev/null 2>&1 || return 0")?;
    writeln!(start_file, "  ps -p \"$pid\" -o pid= >/dev/null 2>&1")?;
    writeln!(start_file, "}}")?;
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
    writeln!(
        start_file,
        "DEFAULT_IROHA_CLI_DEBUG=\"{}\"",
        default_iroha_debug.display()
    )?;
    writeln!(
        start_file,
        "DEFAULT_IROHA_CLI_RELEASE=\"{}\"",
        default_iroha_release.display()
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
        "  else\n    echo \"IROHAD_BIN not set and default ($DEFAULT_IROHAD_BIN_DEBUG or $DEFAULT_IROHAD_BIN_RELEASE) not found; build iroha3d or set IROHAD_BIN\" >&2\n    exit 1\n  fi"
    )?;
    writeln!(start_file, "fi")?;
    writeln!(
        start_file,
        "echo \"Using IROHAD_BIN=$IROHAD_BIN\" >&2\nIROHAD_BIN_RESOLVED=\"$(command -v \"$IROHAD_BIN\" 2>/dev/null || true)\"\nif [ -z \"$IROHAD_BIN_RESOLVED\" ]; then\n  echo \"iroha3d binary not executable: $IROHAD_BIN\" >&2\n  exit 1\nfi\nIROHAD_BIN_DIR=\"$(cd -- \"$(dirname -- \"$IROHAD_BIN_RESOLVED\")\" && pwd)\"\nIROHAD_BIN=\"$IROHAD_BIN_DIR/$(basename -- \"$IROHAD_BIN_RESOLVED\")\"\nIROHA_CLI_FROM_IROHAD=\"$IROHAD_BIN_DIR/iroha\""
    )?;
    writeln!(start_file, "IROHA_CLI=\"${{IROHA_CLI:-}}\"")?;
    writeln!(start_file, "if [ -z \"$IROHA_CLI\" ]; then")?;
    writeln!(start_file, "  if [ -x \"$IROHA_CLI_FROM_IROHAD\" ]; then")?;
    writeln!(start_file, "    IROHA_CLI=\"$IROHA_CLI_FROM_IROHAD\"")?;
    writeln!(
        start_file,
        "  elif [ -x \"$DEFAULT_IROHA_CLI_RELEASE\" ]; then"
    )?;
    writeln!(start_file, "    IROHA_CLI=\"$DEFAULT_IROHA_CLI_RELEASE\"")?;
    writeln!(
        start_file,
        "  elif [ -x \"$DEFAULT_IROHA_CLI_DEBUG\" ]; then"
    )?;
    writeln!(start_file, "    IROHA_CLI=\"$DEFAULT_IROHA_CLI_DEBUG\"")?;
    writeln!(start_file, "  fi")?;
    writeln!(start_file, "fi")?;
    writeln!(
        start_file,
        "if [ -n \"$IROHA_CLI\" ] && [ ! -x \"$IROHA_CLI\" ]; then"
    )?;
    writeln!(
        start_file,
        "  echo \"iroha CLI not executable: $IROHA_CLI\" >&2"
    )?;
    writeln!(start_file, "  exit 1")?;
    writeln!(start_file, "fi")?;
    writeln!(start_file, "FAUCET_ACCOUNT=\"{}\"", client_account_literal)?;
    writeln!(
        start_file,
        "FAUCET_ASSET_DEFINITION_ID=\"{}\"",
        fee_asset_definition_id
    )?;
    writeln!(
        start_file,
        "FAUCET_RESERVE_MIN=\"{}\"",
        LOCALNET_FEE_ASSET_RESERVE_MIN
    )?;
    writeln!(
        start_file,
        "FAUCET_RESERVE_TARGET=\"{}\"",
        LOCALNET_FEE_ASSET_RESERVE_TARGET
    )?;
    writeln!(
        start_file,
        "FAUCET_RESERVE_RETRIES=\"${{IROHA_LOCALNET_FAUCET_RESERVE_RETRIES:-30}}\""
    )?;
    writeln!(start_file, "for i in $(seq 0 {}); do", peers - 1)?;
    writeln!(
        start_file,
        "  SNAPSHOT_STORE_DIR=\"$DIR/state/peer${{i}}/snapshot\""
    )?;
    writeln!(start_file, "  PIDFILE=\"$DIR/peer${{i}}.pid\"")?;
    writeln!(start_file, "  if [ -f \"$PIDFILE\" ]; then")?;
    writeln!(
        start_file,
        "    existing_pid=\"$(cat \"$PIDFILE\" 2>/dev/null || true)\""
    )?;
    writeln!(
        start_file,
        "    if [ -n \"$existing_pid\" ] && pid_is_running \"$existing_pid\"; then"
    )?;
    writeln!(
        start_file,
        "      echo \"peer$i already running with pid $existing_pid\" >&2"
    )?;
    writeln!(start_file, "      exit 1")?;
    writeln!(start_file, "    fi")?;
    writeln!(start_file, "    rm -f \"$PIDFILE\"")?;
    writeln!(start_file, "  fi")?;
    writeln!(start_file, "  mkdir -p \"$SNAPSHOT_STORE_DIR/generations\"")?;
    writeln!(start_file, "  if command -v python3 >/dev/null 2>&1; then")?;
    writeln!(
        start_file,
        "    peer_pid=$(mkdir -p \"$DIR/state/peer${{i}}\" && cd \"$DIR/state/peer${{i}}\" && SNAPSHOT_STORE_DIR=\"$SNAPSHOT_STORE_DIR\" LOG_LEVEL=\"${{LOG_LEVEL:-info}}\" LOG_FILTER=\"${{LOG_FILTER:-}}\" IROHAD_BIN=\"$IROHAD_BIN\" IROHA_PEER_CONFIG=\"$DIR/peer${{i}}.toml\" IROHA_PEER_LOG=\"$DIR/peer${{i}}.log\" IROHA_SORA_MODE=\"{sora_mode_env}\" python3 - <<'PY'"
    )?;
    writeln!(start_file, "import os")?;
    writeln!(start_file, "import subprocess")?;
    writeln!(start_file)?;
    writeln!(start_file, "env = os.environ.copy()")?;
    writeln!(start_file, "cmd = [env[\"IROHAD_BIN\"]]")?;
    writeln!(start_file, "if env.get(\"IROHA_SORA_MODE\") == \"1\":")?;
    writeln!(start_file, "    cmd.append(\"--sora\")")?;
    writeln!(
        start_file,
        "cmd.extend([\"--config\", env[\"IROHA_PEER_CONFIG\"]])"
    )?;
    writeln!(
        start_file,
        "log = open(env[\"IROHA_PEER_LOG\"], \"ab\", buffering=0)"
    )?;
    writeln!(
        start_file,
        "process = subprocess.Popen(cmd, stdout=log, stderr=subprocess.STDOUT, env=env, close_fds=True, start_new_session=True)"
    )?;
    writeln!(start_file, "print(process.pid)")?;
    writeln!(start_file, "PY")?;
    writeln!(start_file, "    )")?;
    writeln!(start_file, "  else")?;
    writeln!(start_file, "    (")?;
    writeln!(
        start_file,
        "      mkdir -p \"$DIR/state/peer${{i}}\" && cd \"$DIR/state/peer${{i}}\""
    )?;
    writeln!(
        start_file,
        "      exec nohup env SNAPSHOT_STORE_DIR=\"$SNAPSHOT_STORE_DIR\" LOG_LEVEL=\"${{LOG_LEVEL:-info}}\" LOG_FILTER=\"${{LOG_FILTER:-}}\" \"$IROHAD_BIN\" {sora_flag}--config \"$DIR/peer${{i}}.toml\""
    )?;
    writeln!(start_file, "    ) > \"$DIR/peer${{i}}.log\" 2>&1 &")?;
    writeln!(start_file, "    peer_pid=$!")?;
    writeln!(start_file, "    disown \"$peer_pid\" 2>/dev/null || true")?;
    writeln!(start_file, "  fi")?;
    writeln!(start_file, "  echo \"$peer_pid\" > \"$PIDFILE\"")?;
    writeln!(start_file, "  echo \"peer$i pid $(cat \"$PIDFILE\")\"")?;
    writeln!(start_file, "done")?;
    writeln!(start_file, "ensure_faucet_reserve() {{")?;
    writeln!(
        start_file,
        "  [ \"$FAUCET_RESERVE_RETRIES\" != \"0\" ] || {{ echo \"Skipping faucet reserve check: retries disabled\" >&2; return 0; }}"
    )?;
    writeln!(
        start_file,
        "  [ -n \"$IROHA_CLI\" ] || {{ echo \"Skipping faucet reserve check: iroha CLI unavailable\" >&2; return 0; }}"
    )?;
    writeln!(
        start_file,
        "  for _ in $(seq 1 \"$FAUCET_RESERVE_RETRIES\"); do"
    )?;
    writeln!(
        start_file,
        "    if asset_json=\"$($IROHA_CLI --machine -c \"$DIR/client.toml\" --output-format json ledger asset get --definition \"$FAUCET_ASSET_DEFINITION_ID\" --account \"$FAUCET_ACCOUNT\" 2>/dev/null)\"; then"
    )?;
    writeln!(
        start_file,
        "      current_value=\"$(printf '%s' \"$asset_json\" | python3 -c 'import json, sys; print(json.load(sys.stdin)[\"value\"])')\""
    )?;
    writeln!(
        start_file,
        "      mint_amount=\"$(python3 -c 'from decimal import Decimal; import sys; current = Decimal(sys.argv[1]); minimum = Decimal(sys.argv[2]); target = Decimal(sys.argv[3]); print(\"\" if current >= minimum else format(target - current, \"f\"))' \"$current_value\" \"$FAUCET_RESERVE_MIN\" \"$FAUCET_RESERVE_TARGET\")\""
    )?;
    writeln!(start_file, "      if [ -z \"$mint_amount\" ]; then")?;
    writeln!(
        start_file,
        "        echo \"Faucet reserve healthy at $current_value\" >&2"
    )?;
    writeln!(start_file, "        return 0")?;
    writeln!(start_file, "      fi")?;
    writeln!(
        start_file,
        "      echo \"Faucet reserve $current_value below floor $FAUCET_RESERVE_MIN; minting $mint_amount to restore $FAUCET_RESERVE_TARGET\" >&2"
    )?;
    writeln!(
        start_file,
        "      $IROHA_CLI --machine -c \"$DIR/client.toml\" --fee-payer authority --output-format json ledger asset mint --definition \"$FAUCET_ASSET_DEFINITION_ID\" --account \"$FAUCET_ACCOUNT\" --quantity \"$mint_amount\" > \"$DIR/faucet-topup.last.json\""
    )?;
    writeln!(start_file, "      return 0")?;
    writeln!(start_file, "    fi")?;
    writeln!(start_file, "    sleep 1")?;
    writeln!(start_file, "  done")?;
    writeln!(
        start_file,
        "  echo \"Skipping faucet reserve check: Torii did not become readable in time\" >&2"
    )?;
    writeln!(start_file, "}}")?;
    writeln!(start_file, "ensure_faucet_reserve")?;
    Ok(start_file.flush()?)
}
fn write_stop_script(stop: &Path) -> Result<()> {
    let mut stop_file = BufWriter::new(File::create(stop)?);
    writeln!(stop_file, "#!/usr/bin/env bash")?;
    writeln!(stop_file, "set -euo pipefail")?;
    writeln!(stop_file, "umask 077")?;
    writeln!(stop_file, "DIR=$(cd \"$(dirname \"$0\")\" && pwd)")?;
    writeln!(stop_file, "pid_matches_peer() {{")?;
    writeln!(stop_file, "  pid=\"$1\"")?;
    writeln!(stop_file, "  config=\"$2\"")?;
    writeln!(
        stop_file,
        "  case \"$pid\" in ''|*[!0-9]*) return 1 ;; esac"
    )?;
    writeln!(stop_file, "  command -v ps >/dev/null 2>&1 || return 0")?;
    writeln!(
        stop_file,
        "  command_line=\"$(ps -p \"$pid\" -o command= 2>/dev/null || true)\""
    )?;
    writeln!(stop_file, "  [ -n \"$command_line\" ] || return 1")?;
    writeln!(
        stop_file,
        "  printf '%s' \"$command_line\" | grep -F -- \"--config $config\" >/dev/null \\"
    )?;
    writeln!(
        stop_file,
        "    || printf '%s' \"$command_line\" | grep -F -- \"--config=$config\" >/dev/null"
    )?;
    writeln!(stop_file, "}}")?;
    writeln!(stop_file, "pid_is_running() {{")?;
    writeln!(stop_file, "  pid=\"$1\"")?;
    writeln!(
        stop_file,
        "  case \"$pid\" in ''|*[!0-9]*) return 1 ;; esac"
    )?;
    writeln!(stop_file, "  command -v ps >/dev/null 2>&1 || return 1")?;
    writeln!(stop_file, "  ps -p \"$pid\" -o pid= >/dev/null 2>&1")?;
    writeln!(stop_file, "}}")?;
    writeln!(stop_file, "for pidfile in \"$DIR\"/peer*.pid; do")?;
    writeln!(stop_file, "  [ -f \"$pidfile\" ] || continue")?;
    writeln!(
        stop_file,
        "  pid=\"$(cat \"$pidfile\" 2>/dev/null || true)\""
    )?;
    writeln!(stop_file, "  if [ -z \"$pid\" ]; then")?;
    writeln!(stop_file, "    rm -f \"$pidfile\"")?;
    writeln!(stop_file, "    continue")?;
    writeln!(stop_file, "  fi")?;
    writeln!(stop_file, "  case \"$pid\" in")?;
    writeln!(stop_file, "    ''|*[!0-9]*)")?;
    writeln!(
        stop_file,
        "      echo \"removing malformed pidfile $pidfile (pid=$pid)\" >&2"
    )?;
    writeln!(stop_file, "      rm -f \"$pidfile\"")?;
    writeln!(stop_file, "      continue")?;
    writeln!(stop_file, "      ;;")?;
    writeln!(stop_file, "  esac")?;
    writeln!(stop_file, "  if ! pid_is_running \"$pid\"; then")?;
    writeln!(stop_file, "    rm -f \"$pidfile\"")?;
    writeln!(stop_file, "    continue")?;
    writeln!(stop_file, "  fi")?;
    writeln!(stop_file, "  peer_name=\"$(basename \"$pidfile\" .pid)\"")?;
    writeln!(stop_file, "  config=\"$DIR/${{peer_name}}.toml\"")?;
    writeln!(
        stop_file,
        "  if ! pid_matches_peer \"$pid\" \"$config\"; then"
    )?;
    writeln!(
        stop_file,
        "    echo \"leaving $pidfile in place: live pid $pid does not match $config\" >&2"
    )?;
    writeln!(stop_file, "    continue")?;
    writeln!(stop_file, "  fi")?;
    writeln!(stop_file, "  kill \"$pid\" 2>/dev/null || true")?;
    writeln!(stop_file, "  for _ in $(seq 1 40); do")?;
    writeln!(stop_file, "    if pid_is_running \"$pid\"; then")?;
    writeln!(stop_file, "      sleep 0.25")?;
    writeln!(stop_file, "    else")?;
    writeln!(stop_file, "      break")?;
    writeln!(stop_file, "    fi")?;
    writeln!(stop_file, "  done")?;
    writeln!(stop_file, "  if pid_is_running \"$pid\"; then")?;
    writeln!(
        stop_file,
        "    echo \"leaving $pidfile in place: localnet peer $peer_name pid $pid is still running\" >&2"
    )?;
    writeln!(stop_file, "    continue")?;
    writeln!(stop_file, "  fi")?;
    writeln!(stop_file, "  rm -f \"$pidfile\"")?;
    writeln!(stop_file, "done")?;
    Ok(stop_file.flush()?)
}
fn copy_rans_tables(out_dir: &Path) -> Result<PathBuf> {
    let canonical_out_dir = fs::canonicalize(out_dir).wrap_err_with(|| {
        format!(
            "failed to canonicalize localnet output directory {}",
            out_dir.display()
        )
    })?;
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
    let seed_path = out_dir.join(LOCALNET_RANS_TABLE_RELATIVE_PATH);
    if !copied_seed {
        fs::write(&seed_path, RANS_SEED0_TABLE).wrap_err("write embedded rANS table")?;
    }
    let canonical_seed_path = fs::canonicalize(&seed_path).wrap_err_with(|| {
        format!(
            "failed to canonicalize generated rANS table {}",
            seed_path.display()
        )
    })?;
    if !canonical_seed_path.starts_with(&canonical_out_dir) {
        return Err(eyre!(
            "generated rANS table escaped localnet output directory: {}",
            canonical_seed_path.display()
        ));
    }
    Ok(canonical_seed_path)
}
const CLIENT_ACCOUNT_DOMAIN: &str = "wonderland.universal";
const CLIENT_ACCOUNT_PUBLIC: &str =
    "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03";
#[cfg(test)]
const CLIENT_ACCOUNT_PRIVATE: &str =
    "802620CCF31D85E3B32A4BEA59987CE0C78E3B8E2DB93881468AB2435FE45D5C9DCD53";
/// Public genesis verifier key emitted for runtime configuration.
pub const GENESIS_PUBLIC_KEY_FILE: &str = "genesis.public_key";
/// Exact signed-genesis consensus-header hash emitted for runtime configuration.
pub const GENESIS_EXPECTED_HASH_FILE: &str = "genesis.expected_hash";
/// Owner-only genesis signing key emitted for offline custody.
pub const GENESIS_PRIVATE_KEY_FILE: &str = "genesis.private_key";
struct LocalnetClientIdentity {
    account_id: AccountId,
    public_key: iroha_crypto::PublicKey,
    private_key: Zeroizing<String>,
}
impl LocalnetClientIdentity {
    fn account_literal(&self, chain_discriminant: Option<u16>) -> String {
        account_id_runtime_literal(&self.account_id, chain_discriminant)
    }
}
struct LocalnetRuntimeBundle {
    operator_signer_key: PathBuf,
    onboarding_signer_key: PathBuf,
    onboarding_token_file: PathBuf,
    onboarding_token_hash: [u8; 32],
}
fn localnet_ephemeral_identity(
    base_seed: Option<&[u8]>,
    identity_label: &[u8],
) -> Result<LocalnetClientIdentity> {
    let (public_key, private_key) = generate_account_key_pair(base_seed, identity_label)?;
    Ok(LocalnetClientIdentity {
        account_id: AccountId::new(public_key.clone()),
        public_key,
        private_key: Zeroizing::new(private_key.to_string()),
    })
}
fn write_private_key_sidecar(path: &Path, private_key: &str) -> Result<()> {
    let mut contents = Zeroizing::new(Vec::with_capacity(private_key.len() + 1));
    contents.extend_from_slice(private_key.as_bytes());
    contents.push(b'\n');
    crate::secure_fs::write_private_file_atomic(path, contents.as_slice())
        .wrap_err_with(|| format!("write private signer key {}", path.display()))
}
fn write_localnet_runtime_bundle(
    out_dir: &Path,
    operator: &LocalnetClientIdentity,
    onboarding: &LocalnetClientIdentity,
) -> Result<LocalnetRuntimeBundle> {
    let runtime_dir = out_dir.join(LOCALNET_RUNTIME_DIRECTORY);
    crate::secure_fs::prepare_empty_private_directory(&runtime_dir)
        .wrap_err("prepare localnet runtime credential directory")?;
    let operator_signer_key = runtime_dir.join(LOCALNET_OPERATOR_SIGNER_KEY_FILE);
    let onboarding_signer_key = runtime_dir.join(LOCALNET_ONBOARDING_SIGNER_KEY_FILE);
    let onboarding_token_file = runtime_dir.join(LOCALNET_ONBOARDING_TOKEN_FILE);
    write_private_key_sidecar(&operator_signer_key, operator.private_key.as_str())?;
    write_private_key_sidecar(&onboarding_signer_key, onboarding.private_key.as_str())?;
    let mut token_entropy = [0_u8; 32];
    OsRng
        .try_fill_bytes(&mut token_entropy)
        .wrap_err("obtain OS entropy for the localnet onboarding token")?;
    let token = Zeroizing::new(format!("iroha-localnet-{}", hex::encode(token_entropy)));
    token_entropy.zeroize();
    let onboarding_token_hash = *blake3::hash(token.as_bytes()).as_bytes();
    crate::secure_fs::write_private_file_atomic(&onboarding_token_file, token.as_bytes())
        .wrap_err("write localnet onboarding token")?;
    Ok(LocalnetRuntimeBundle {
        operator_signer_key,
        onboarding_signer_key,
        onboarding_token_file,
        onboarding_token_hash,
    })
}
fn write_localnet_gitignore(out_dir: &Path) -> Result<()> {
    let path = out_dir.join(".gitignore");
    fs::write(
        &path,
        concat!(
            "# Kagami localnets contain private signing material and runtime tokens.\n",
            "*\n",
            "!.gitignore\n",
        ),
    )
    .wrap_err_with(|| format!("write protective ignore file {}", path.display()))
}
#[cfg(test)]
#[path = "localnet/client_identity_test_support.rs"]
mod localnet_test_helpers;
#[cfg(test)]
use localnet_test_helpers::localnet_client_identity;
fn localnet_client_account_id() -> AccountId {
    let public_key = CLIENT_ACCOUNT_PUBLIC
        .parse()
        .expect("localnet client public key must parse");
    AccountId::new(public_key)
}
fn write_owner_only_localnet_file(path: &Path, contents: &[u8]) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::{OpenOptionsExt as _, PermissionsExt as _};
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true).mode(0o600);
        let mut file = options
            .open(path)
            .wrap_err_with(|| format!("create owner-only file {}", path.display()))?;
        file.set_permissions(fs::Permissions::from_mode(0o600))
            .wrap_err_with(|| format!("protect owner-only file {}", path.display()))?;
        file.write_all(contents)
            .wrap_err_with(|| format!("write owner-only file {}", path.display()))?;
        Ok(())
    }
    #[cfg(not(unix))]
    {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .wrap_err_with(|| format!("create localnet file {}", path.display()))?;
        file.write_all(contents)
            .wrap_err_with(|| format!("write localnet file {}", path.display()))
    }
}
fn write_client_config(
    out_dir: &Path,
    base_api_port: u16,
    torii_host: &CanonicalHost,
    chain_id: &str,
    genesis_expected_hash: HashOf<BlockHeader>,
    chain_discriminant: Option<u16>,
    client: &LocalnetClientIdentity,
) -> Result<()> {
    let path = out_dir.join("client.toml");
    // Render explicitly to avoid pretty-printer wrapping the long keys.
    let torii_host = torii_host.url_host();
    let chain_discriminant_line = chain_discriminant.map_or_else(String::new, |value| {
        format!("chain_discriminant = {value}\n")
    });
    let network_id = norito::literal::format(
        "hash",
        &genesis_expected_hash.to_string().to_ascii_uppercase(),
    );
    let rendered = format!(
        concat!(
            "chain = \"{chain}\"\n",
            "network_id = \"{network_id}\"\n",
            "torii_url = \"http://{torii_host}:{torii_port}/\"\n",
            "\n",
            "[transaction]\n",
            "time_to_live_ms = {ttl_ms}\n",
            "status_timeout_ms = {status_timeout_ms}\n",
            "nonce = false\n",
            "\n",
            "[account]\n",
            "domain = \"{domain}\"\n",
            "{chain_discriminant_line}",
            "private_key = \"{private_key}\"\n",
            "public_key  = \"{public_key}\"\n",
            "\n",
            "[basic_auth]\n",
            "password  = \"ilovetea\"\n",
            "web_login = \"mad_hatter\"\n",
        ),
        chain = chain_id,
        network_id = network_id,
        torii_port = base_api_port,
        torii_host = torii_host,
        ttl_ms = LOCALNET_CLIENT_TTL_MS,
        status_timeout_ms = LOCALNET_CLIENT_STATUS_TIMEOUT_MS,
        domain = CLIENT_ACCOUNT_DOMAIN,
        chain_discriminant_line = chain_discriminant_line,
        private_key = client.private_key.as_str(),
        public_key = client.public_key,
    );
    write_owner_only_localnet_file(&path, rendered.as_bytes())
        .wrap_err_with(|| format!("write operator client config {}", path.display()))?;
    Ok(())
}
#[allow(clippy::too_many_arguments)]
fn write_localnet_readme(
    out_dir: &Path,
    chain_id: &str,
    seed: Option<&str>,
    private_custody: bool,
    consensus_mode: SumeragiConsensusMode,
    peers: u16,
    torii_url: &str,
    genesis_json_path: &Path,
    genesis_signed_path: &Path,
    genesis_expected_hash_path: &Path,
    genesis_public_key_path: &Path,
    genesis_private_key_path: &Path,
    client_config_path: &Path,
    start_path: &Path,
    stop_path: &Path,
    operator_account_id: &str,
    onboarding_account_id: &str,
    runtime_bundle: &LocalnetRuntimeBundle,
    alias_setup_intent_path: &Path,
) -> Result<()> {
    let readme_path = out_dir.join("README.md");
    let start_command = localnet_script_command(private_custody, "start.sh");
    let stop_command = localnet_script_command(private_custody, "stop.sh");
    let seed_line = seed
        .map(|seed| {
            format!(
                "- Base seed BLAKE3 fingerprint: `{}`\n",
                blake3::hash(seed.as_bytes()).to_hex()
            )
        })
        .unwrap_or_default();
    let rendered = format!(
        concat!(
            "# Kagami Localnet\n\n",
            "- Chain ID: `{chain_id}`\n",
            "{seed_line}",
            "- Consensus mode: `{consensus_mode}`\n",
            "- Peer count: `{peers}`\n",
            "- Primary Torii URL: `{torii_url}`\n",
            "- Genesis JSON: `{genesis_json}`\n",
            "- Signed genesis: `{genesis_signed}`\n",
            "- Approved exact genesis hash: `{genesis_expected_hash}`\n",
            "- Genesis verifier key: `{genesis_public_key}`\n",
            "- Owner-held genesis signing key: `{genesis_private_key}`\n",
            "- Client config: `{client_config}`\n\n",
            "## Built-in App API bootstrap\n\n",
            "- Kagemusha asset definition: `{kagemusha_asset}`\n",
            "- Kagemusha asset alias: `{kagemusha_alias}`\n",
            "- Ephemeral operator authority: `{operator_account_id}`\n",
            "- Ephemeral onboarding authority: `{onboarding_account_id}`\n",
            "- Operator signer sidecar: `{operator_signer_key}`\n",
            "- Onboarding signer sidecar: `{onboarding_signer_key}`\n",
            "- Onboarding API token sidecar: `{onboarding_token_file}`\n",
            "- Secret-free alias setup intent: `{alias_setup_intent}`\n",
            "- Offline escrow account: deterministic account derived from the exact genesis network id and asset definition\n",
            "- Generated peer configs enable structural `torii.account_onboarding` and Kagemusha escrow routing\n",
            "- Runtime credentials are owner-only files; read the token from its sidecar when calling sponsored onboarding\n\n",
            "Run `kagami docker` without `--seed` against this directory to validate the exact ",
            "validator identities, PoPs, signed body, verifier key, and expected hash as one ",
            "authoritative prepared bundle. The resulting Compose manifest embeds only read-only ",
            "paths to the three public runtime artifacts. The signing key is never mounted at ",
            "runtime; keep it offline and never commit it.\n\n",
            "- Start script: `{start_script}`\n",
            "- Stop script: `{stop_script}`\n\n",
            "## Next steps\n\n",
            "```bash\n",
            "cd {out_dir}\n",
            "{start_command}\n",
            "curl -sf {torii_url}health\n",
            "{stop_command}\n",
            "```\n",
            "Logs are written to `peerN.log` files next to the generated configs.\n",
        ),
        chain_id = chain_id,
        seed_line = seed_line,
        consensus_mode = consensus_mode_label(consensus_mode),
        peers = peers,
        torii_url = torii_url,
        genesis_json = genesis_json_path.display(),
        genesis_signed = genesis_signed_path.display(),
        genesis_expected_hash = genesis_expected_hash_path.display(),
        genesis_public_key = genesis_public_key_path.display(),
        genesis_private_key = genesis_private_key_path.display(),
        client_config = client_config_path.display(),
        kagemusha_asset = LOCALNET_KAGEMUSHA_ASSET_ID,
        kagemusha_alias = LOCALNET_KAGEMUSHA_ASSET_ALIAS,
        operator_account_id = operator_account_id,
        onboarding_account_id = onboarding_account_id,
        operator_signer_key = runtime_bundle.operator_signer_key.display(),
        onboarding_signer_key = runtime_bundle.onboarding_signer_key.display(),
        onboarding_token_file = runtime_bundle.onboarding_token_file.display(),
        alias_setup_intent = alias_setup_intent_path.display(),
        start_script = start_path.display(),
        stop_script = stop_path.display(),
        out_dir = out_dir.display(),
        start_command = start_command,
        stop_command = stop_command,
    );
    fs::write(&readme_path, rendered).wrap_err_with(|| {
        format!(
            "failed to write localnet guide to {}",
            readme_path.display()
        )
    })
}
fn localnet_script_command(private_custody: bool, script_name: &str) -> String {
    if private_custody {
        format!("bash ./{script_name}")
    } else {
        format!("./{script_name}")
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use iroha_config::{
        base::toml::TomlSource, kura::FsyncMode, logger::Directives, parameters::actual,
    };
    use iroha_data_model::{
        block::{consensus_v2::PROTOCOL_VERSION, decode_framed_signed_block},
        isi::{GrantBox, MintBox, SetParameter, TransferBox},
        parameter::{
            Parameter,
            system::{
                Parameters, SumeragiConsensusMode, SumeragiNposParameters, consensus_metadata,
            },
        },
        transaction::Executable,
    };
    use iroha_executor_data_model::permission::account::CanDelegateAccountAliasResolution;
    use norito::{json, literal};
    #[cfg(unix)]
    use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};
    use std::{
        env, fs,
        io::BufWriter,
        path::{Path, PathBuf},
    };
    include!("localnet/runtime_artifact_tests.rs");
    fn localnet_genesis_for_opts(opts: &LocalnetOptions) -> RawGenesisTransaction {
        localnet_genesis_for_opts_and_client(opts, &localnet_client_account_id())
    }
    fn localnet_genesis_for_opts_and_client(
        opts: &LocalnetOptions,
        client_account_id: &AccountId,
    ) -> RawGenesisTransaction {
        let default_client_account_id = localnet_client_account_id();
        let uses_default_client = client_account_id == &default_client_account_id;
        let seed_bytes = opts.seed.as_ref().map(String::as_bytes);
        let peers = build_peers(
            opts.peers.get(),
            seed_bytes,
            opts.base_api_port,
            opts.base_p2p_port,
        )
        .expect("test localnet peer key generation should succeed");
        let npos_bootstrap = localnet_uses_npos(opts.consensus_mode);
        let perf_spec = opts.perf_profile.map(LocalnetPerfProfile::spec);
        let block_cadence_override = opts
            .block_cadence_ms
            .or_else(|| perf_spec.map(|spec| spec.block_cadence_ms));
        let block_cadence_ms = Some(block_cadence_override.unwrap_or(LOCALNET_PIPELINE_TIME_MS));
        let block_max_transactions = perf_spec.map_or(LOCALNET_BLOCK_MAX_TRANSACTIONS, |spec| {
            spec.block_max_transactions
        });
        let requested_stake_amount = perf_spec.map(|spec| spec.stake_amount);
        let (genesis_public_key, _) = generate_genesis_key_pair(seed_bytes, GENESIS_SEED)
            .expect("test localnet genesis key generation should succeed");
        let genesis_account_id = AccountId::new(genesis_public_key.clone());
        let assets = if uses_default_client {
            effective_localnet_assets(&opts.assets)
        } else {
            effective_localnet_assets_for_client(&opts.assets, client_account_id)
        };
        let mut genesis =
            generate_raw_genesis(&genesis_public_key, opts.consensus_mode, DEFAULT_CHAIN_ID)
                .expect("generate raw genesis");
        if opts.extra_accounts > 0 || !assets.is_empty() {
            genesis = extend_genesis(
                genesis,
                &genesis_account_id,
                seed_bytes,
                opts.extra_accounts,
                &assets,
            )
            .expect("extend genesis");
        }
        genesis = apply_parameter_overrides(
            genesis,
            block_cadence_ms,
            block_max_transactions,
            opts.consensus_mode,
        );
        genesis = if uses_default_client {
            append_localnet_contract_permissions(genesis, &genesis_account_id)
        } else {
            append_localnet_contract_permissions_for_client(
                genesis,
                &genesis_account_id,
                client_account_id,
            )
        };
        genesis = append_peer_pop(genesis, &peers);
        if npos_bootstrap {
            let gas_account_id = localnet_gas_account_id(&genesis_public_key)
                .expect("test localnet gas account derivation should succeed");
            let stake_amount = localnet_npos_stake_amount(
                &genesis
                    .effective_parameters()
                    .expect("generated localnet genesis has one structured parameter block"),
                requested_stake_amount,
            );
            genesis = if uses_default_client {
                append_localnet_npos_bootstrap(
                    genesis,
                    &peers,
                    &gas_account_id,
                    &stake_amount,
                    opts.sora_profile,
                    &genesis_account_id,
                )
            } else {
                append_localnet_npos_bootstrap_for_client(
                    genesis,
                    &peers,
                    &gas_account_id,
                    &stake_amount,
                    opts.sora_profile,
                    &genesis_account_id,
                    client_account_id,
                )
            }
            .expect("append localnet NPoS bootstrap");
            genesis = append_private_dataspace_genesis_bootstrap_for_client(
                genesis,
                opts.sora_profile,
                &genesis_account_id,
                client_account_id,
            )
            .expect("append private-dataspace genesis bootstrap");
        }
        apply_localnet_crypto_overrides(genesis, npos_bootstrap)
    }
    include!("localnet/private_profile_bootstrap_tests.rs");
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
    #[test]
    fn generated_configs_parse_with_current_schema() {
        let temp = tempfile::tempdir().expect("make temp dir");
        let opts = LocalnetOptions {
            sora_profile: None,
            perf_profile: None,
            peers: NonZeroU16::new(4).expect("non-zero"),
            seed: Some("kagami-config-compat".to_owned()),
            bind_host: DEFAULT_BIND_HOST.to_owned(),
            public_host: DEFAULT_PUBLIC_HOST.to_owned(),
            base_api_port: 19080,
            base_p2p_port: 23337,
            out_dir: temp.path().to_path_buf(),
            extra_accounts: 0,
            assets: Vec::new(),
            block_cadence_ms: None,
            consensus_mode: SumeragiConsensusMode::Npos,
        };
        generate_localnet(&opts, &mut BufWriter::new(Vec::new())).expect("generate localnet files");
        let start_script =
            fs::read_to_string(temp.path().join("start.sh")).expect("read generated start script");
        assert!(
            start_script.contains("IROHA_SORA_MODE=\"0\""),
            "plain NPoS must not request the post-parse Sora profile"
        );
        assert!(
            !start_script.contains(" --sora --config "),
            "plain NPoS fallback startup must not add --sora"
        );
        let expected_hash_record = fs::read_to_string(temp.path().join(GENESIS_EXPECTED_HASH_FILE))
            .expect("read generated exact genesis hash");
        assert!(expected_hash_record.ends_with('\n'));
        assert_eq!(expected_hash_record.matches('\n').count(), 1);
        let expected_hash = expected_hash_record
            .strip_suffix('\n')
            .expect("hash record has final newline")
            .parse::<HashOf<BlockHeader>>()
            .expect("exact genesis hash parses");
        assert_eq!(expected_hash_record, format!("{expected_hash}\n"));
        let signed = fs::read(temp.path().join("genesis.signed.nrt"))
            .expect("read generated signed genesis");
        let decoded = decode_framed_signed_block(&signed).expect("decode generated signed genesis");
        assert_eq!(decoded.hash(), expected_hash);
        for index in 0..opts.peers.get() {
            let source = TomlSource::from_file(temp.path().join(format!("peer{index}.toml")))
                .expect("read generated config");
            let config =
                actual::Root::from_toml_source(source).expect("generated config must parse");
            assert_eq!(config.genesis.expected_hash, expected_hash);
        }
    }
    #[test]
    fn generated_configs_for_user_localnet_parse() {
        let temp = tempfile::tempdir().expect("make temp dir");
        let opts = LocalnetOptions {
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
                name: LOCALNET_SAMPLE_ASSET_NAME.to_owned(),
                alias: None,
                owned_by: ALICE_ID.clone(),
                mint_to: ALICE_ID.clone(),
                quantity: 100,
            }],
            block_cadence_ms: None,
            consensus_mode: SumeragiConsensusMode::Npos,
        };
        generate_localnet(&opts, &mut BufWriter::new(Vec::new())).expect("generate localnet files");
        let source =
            TomlSource::from_file(temp.path().join("peer0.toml")).expect("read generated config");
        actual::Root::from_toml_source(source).expect("generated config must parse");
    }
    #[test]
    fn requested_localnet_asset_spec_trims_and_uses_client_owner_with_initial_reserve() {
        let asset_id = localnet_sample_asset_literal();
        let spec = requested_localnet_asset_spec(&format!("  {asset_id}  ")).expect("asset spec");
        let client_account_id = localnet_client_account_id();
        assert_eq!(spec.id, asset_id);
        assert_eq!(spec.alias, None);
        assert_eq!(spec.owned_by, client_account_id);
        assert_eq!(spec.mint_to, spec.owned_by);
        assert_eq!(spec.quantity, LOCALNET_REQUESTED_ASSET_INITIAL_QUANTITY);
    }
    #[test]
    fn requested_localnet_asset_spec_rejects_blank_or_invalid_asset_definition_id() {
        assert!(requested_localnet_asset_spec("   ").is_err());
        assert!(requested_localnet_asset_spec("not-valid").is_err());
    }
    #[test]
    fn generated_localnet_registers_requested_asset_definition_for_client_owner() {
        let requested_asset_literal = localnet_sample_asset_literal();
        let opts = LocalnetOptions {
            sora_profile: None,
            perf_profile: None,
            peers: NonZeroU16::new(4).expect("non-zero"),
            seed: Some("requested-asset-bootstrap".to_owned()),
            bind_host: DEFAULT_PUBLIC_HOST.to_owned(),
            public_host: DEFAULT_PUBLIC_HOST.to_owned(),
            base_api_port: 29080,
            base_p2p_port: 33337,
            out_dir: PathBuf::from("unused"),
            extra_accounts: 0,
            assets: vec![
                requested_localnet_asset_spec(&requested_asset_literal).expect("asset spec"),
            ],
            block_cadence_ms: None,
            consensus_mode: SumeragiConsensusMode::Npos,
        };
        let manifest = localnet_genesis_for_opts(&opts);
        let requested_asset_id = AssetDefinitionId::parse_address_literal(&requested_asset_literal)
            .expect("requested asset id");
        let client_account_id = localnet_client_account_id();
        let requested_asset = AssetId::new(requested_asset_id.clone(), client_account_id.clone());
        let has_definition = manifest.instructions().any(|instruction| {
            instruction
                .as_any()
                .downcast_ref::<RegisterBox>()
                .is_some_and(|register| {
                    matches!(
                        register,
                        RegisterBox::AssetDefinition(register)
                            if register.object().id == requested_asset_id
                    )
                })
        });
        assert!(
            has_definition,
            "localnet must register requested asset definitions"
        );
        let has_owner_transfer = manifest.instructions().any(|instruction| {
            instruction
                .as_any()
                .downcast_ref::<TransferBox>()
                .is_some_and(|transfer| match transfer {
                    TransferBox::AssetDefinition(transfer_asset) => {
                        transfer_asset.object() == &requested_asset_id
                            && transfer_asset.destination() == &client_account_id
                    }
                    _ => false,
                })
        });
        assert!(
            has_owner_transfer,
            "requested asset definition ownership must transfer to the generated client signer"
        );
        let has_initial_mint = manifest.instructions().any(|instruction| {
            instruction
                .as_any()
                .downcast_ref::<MintBox>()
                .is_some_and(|mint| match mint {
                    MintBox::Asset(mint_asset) => mint_asset.destination() == &requested_asset,
                    _ => false,
                })
        });
        assert!(
            has_initial_mint,
            "requested asset definitions must mint an initial reserve to the generated client signer"
        );
    }
    #[test]
    #[allow(clippy::too_many_lines)]
    fn generated_localnet_bootstraps_universal_kagemusha_asset() {
        let opts = LocalnetOptions {
            sora_profile: None,
            perf_profile: None,
            peers: NonZeroU16::new(4).expect("non-zero"),
            seed: Some("kagemusha-bootstrap".to_owned()),
            bind_host: DEFAULT_PUBLIC_HOST.to_owned(),
            public_host: DEFAULT_PUBLIC_HOST.to_owned(),
            base_api_port: 29080,
            base_p2p_port: 33337,
            out_dir: PathBuf::from("unused"),
            extra_accounts: 0,
            assets: Vec::new(),
            block_cadence_ms: None,
            consensus_mode: SumeragiConsensusMode::Npos,
        };
        let manifest = localnet_genesis_for_opts(&opts);
        let kagemusha_asset_id =
            AssetDefinitionId::parse_address_literal(LOCALNET_KAGEMUSHA_ASSET_ID)
                .expect("Kagemusha asset id");
        let kagemusha_alias = LOCALNET_KAGEMUSHA_ASSET_ALIAS
            .parse::<AssetDefinitionAlias>()
            .expect("Kagemusha asset alias");
        let client_account_id = localnet_client_account_id();
        let (genesis_public_key, _) =
            generate_genesis_key_pair(opts.seed.as_ref().map(String::as_bytes), GENESIS_SEED)
                .expect("test localnet genesis key generation should succeed");
        let genesis_account_id = AccountId::new(genesis_public_key);
        let expected_explicit_manage_offline_escrow_grants =
            usize::from(client_account_id != *ALICE_ID);
        let expected_mint_destination =
            AssetId::new(kagemusha_asset_id.clone(), client_account_id.clone());
        let has_definition = manifest.instructions().any(|instruction| {
            instruction
                .as_any()
                .downcast_ref::<RegisterBox>()
                .is_some_and(|register| {
                    matches!(
                        register,
                        RegisterBox::AssetDefinition(register)
                            if register.object().id == kagemusha_asset_id
                    )
                })
        });
        assert!(
            has_definition,
            "localnet must register the built-in asset used by universal Kagemusha protocols"
        );
        let has_alias_binding = manifest.instructions().any(|instruction| {
            instruction
                .as_any()
                .downcast_ref::<SetAssetDefinitionAlias>()
                .is_some_and(|set_alias| {
                    set_alias.asset_definition_id() == &kagemusha_asset_id
                        && set_alias.alias().as_ref() == Some(&kagemusha_alias)
                })
        });
        assert!(
            has_alias_binding,
            "localnet must bind the built-in Kagemusha asset alias"
        );
        let has_initial_mint = manifest.instructions().any(|instruction| {
            instruction
                .as_any()
                .downcast_ref::<MintBox>()
                .is_some_and(|mint| match mint {
                    MintBox::Asset(mint_asset) => {
                        mint_asset.destination() == &expected_mint_destination
                    }
                    _ => false,
                })
        });
        assert!(
            has_initial_mint,
            "localnet must mint the built-in Kagemusha asset to the client signer"
        );
        let has_owner_transfer = manifest.instructions().any(|instruction| {
            instruction
                .as_any()
                .downcast_ref::<TransferBox>()
                .is_some_and(|transfer| match transfer {
                    TransferBox::AssetDefinition(transfer_asset) => {
                        transfer_asset.object() == &kagemusha_asset_id
                            && transfer_asset.destination() == &client_account_id
                    }
                    _ => false,
                })
        });
        assert!(
            has_owner_transfer,
            "localnet must transfer Kagemusha asset ownership to the client signer"
        );
        let mut has_alias_manage = false;
        let mut has_manifest_publish = false;
        let mut manage_offline_escrow_grants = 0usize;
        let mut total_manage_offline_escrow_grants = 0usize;
        let mut genesis_manage_verifying_keys_grants = 0usize;
        let mut client_manage_verifying_keys_grants = 0usize;
        let mut total_manage_verifying_keys_grants = 0usize;
        for instruction in manifest.instructions() {
            let Some(grant) = instruction.as_any().downcast_ref::<GrantBox>() else {
                continue;
            };
            let GrantBox::Permission(grant_permission) = grant else {
                continue;
            };
            let permission_name: &str = grant_permission.object().name();
            if permission_name == "CanManageOfflineEscrow" {
                total_manage_offline_escrow_grants =
                    total_manage_offline_escrow_grants.saturating_add(1);
            }
            if permission_name == "CanManageVerifyingKeys" {
                total_manage_verifying_keys_grants =
                    total_manage_verifying_keys_grants.saturating_add(1);
            }
            if permission_name == "CanManageVerifyingKeys"
                && grant_permission.destination() == &genesis_account_id
            {
                genesis_manage_verifying_keys_grants =
                    genesis_manage_verifying_keys_grants.saturating_add(1);
            }
            if grant_permission.destination() != &client_account_id {
                continue;
            }
            match permission_name {
                "CanManageAccountAlias" => has_alias_manage = true,
                "CanManageOfflineEscrow" => {
                    manage_offline_escrow_grants = manage_offline_escrow_grants.saturating_add(1);
                }
                "CanManageVerifyingKeys" => {
                    client_manage_verifying_keys_grants =
                        client_manage_verifying_keys_grants.saturating_add(1);
                }
                "CanPublishSpaceDirectoryManifest" => has_manifest_publish = true,
                _ => {}
            }
        }
        assert!(
            has_alias_manage,
            "localnet client signer must be able to manage account aliases for onboarding"
        );
        assert!(
            has_manifest_publish,
            "localnet client signer must be able to publish onboarding manifests"
        );
        assert_eq!(
            manage_offline_escrow_grants, expected_explicit_manage_offline_escrow_grants,
            "localnet must only emit an explicit CanManageOfflineEscrow grant when the client signer is not Alice"
        );
        assert_eq!(
            total_manage_offline_escrow_grants, expected_explicit_manage_offline_escrow_grants,
            "localnet genesis must not emit duplicate explicit CanManageOfflineEscrow grants"
        );
        assert_eq!(
            genesis_manage_verifying_keys_grants, 1,
            "localnet genesis must grant CanManageVerifyingKeys to the genesis signer exactly once"
        );
        assert_eq!(
            client_manage_verifying_keys_grants, 1,
            "localnet genesis must grant CanManageVerifyingKeys to the maintenance client signer exactly once"
        );
        let expected_total_manage_verifying_keys_grants = if client_account_id == genesis_account_id
        {
            1
        } else {
            2
        };
        assert_eq!(
            total_manage_verifying_keys_grants, expected_total_manage_verifying_keys_grants,
            "localnet genesis must not emit duplicate CanManageVerifyingKeys grants"
        );
    }
    #[test]
    fn permissioned_localnet_genesis_deduplicates_offline_escrow_grant() {
        let opts = LocalnetOptions {
            sora_profile: None,
            perf_profile: None,
            peers: NonZeroU16::new(4).expect("non-zero"),
            seed: Some("permissioned-offline-escrow-dedup".to_owned()),
            bind_host: DEFAULT_PUBLIC_HOST.to_owned(),
            public_host: DEFAULT_PUBLIC_HOST.to_owned(),
            base_api_port: 29080,
            base_p2p_port: 33337,
            out_dir: PathBuf::from("unused"),
            extra_accounts: 0,
            assets: Vec::new(),
            block_cadence_ms: None,
            consensus_mode: SumeragiConsensusMode::Permissioned,
        };
        let manifest = localnet_genesis_for_opts(&opts);
        let client_account_id = localnet_client_account_id();
        let expected_explicit_manage_offline_escrow_grants =
            usize::from(client_account_id != *ALICE_ID);
        let offline_escrow_grants = manifest
            .instructions()
            .filter_map(|instruction| instruction.as_any().downcast_ref::<GrantBox>())
            .filter_map(|grant| match grant {
                GrantBox::Permission(grant_permission) => Some(grant_permission),
                _ => None,
            })
            .filter(|grant_permission| grant_permission.destination() == &client_account_id)
            .filter(|grant_permission| grant_permission.object().name() == "CanManageOfflineEscrow")
            .count();
        assert_eq!(
            offline_escrow_grants, expected_explicit_manage_offline_escrow_grants,
            "permissioned localnet genesis must not duplicate the offline escrow manager grant and must skip the redundant Alice bootstrap grant"
        );
    }
    #[test]
    #[allow(clippy::too_many_lines)]
    fn generated_localnet_needs_no_backend_offline_switch() {
        let temp = tempfile::tempdir().expect("make temp dir");
        #[cfg(unix)]
        fs::set_permissions(temp.path(), fs::Permissions::from_mode(0o700))
            .expect("make localnet output directory owner-held");
        let opts = LocalnetOptions {
            sora_profile: None,
            perf_profile: None,
            peers: NonZeroU16::new(4).expect("non-zero"),
            seed: Some("kagemusha-config".to_owned()),
            bind_host: DEFAULT_PUBLIC_HOST.to_owned(),
            public_host: DEFAULT_PUBLIC_HOST.to_owned(),
            base_api_port: 29080,
            base_p2p_port: 33337,
            out_dir: temp.path().canonicalize().expect("canonical temp dir"),
            extra_accounts: 0,
            assets: Vec::new(),
            block_cadence_ms: None,
            consensus_mode: SumeragiConsensusMode::Npos,
        };
        let mut command_output = BufWriter::new(Vec::new());
        generate_localnet_inner(&opts, &mut command_output, true, None)
            .expect("generate fresh-custody localnet files");
        let command_output = String::from_utf8(
            command_output
                .into_inner()
                .expect("flush localnet command output"),
        )
        .expect("localnet command output is UTF-8");
        let peer_config_text =
            fs::read_to_string(temp.path().join("peer0.toml")).expect("read generated peer config");
        let peer_cfg: toml::Value = toml::from_str(&peer_config_text).expect("parse peer config");
        let operator =
            localnet_ephemeral_identity(opts.seed.as_deref().map(str::as_bytes), b"operator-root")
                .expect("derive generated operator");
        let onboarding_identity = localnet_ephemeral_identity(
            opts.seed.as_deref().map(str::as_bytes),
            b"onboarding-root",
        )
        .expect("derive generated onboarding signer");
        let onboarding_account_id = onboarding_identity.account_literal(None);
        let onboarding = peer_cfg
            .get("torii")
            .and_then(toml::Value::as_table)
            .and_then(|torii| torii.get("account_onboarding"))
            .and_then(toml::Value::as_table)
            .expect("torii.account_onboarding table");
        assert_eq!(
            onboarding.get("authority").and_then(toml::Value::as_str),
            Some(onboarding_account_id.as_str())
        );
        assert_ne!(onboarding_identity.account_id, operator.account_id);
        assert!(onboarding.get("enabled").is_none());
        assert!(onboarding.get("private_key").is_none());
        let signer_path = onboarding
            .get("private_key_file")
            .and_then(toml::Value::as_str)
            .map(PathBuf::from)
            .expect("runtime-only signer key path");
        assert_eq!(
            signer_path,
            temp.path()
                .canonicalize()
                .expect("canonical temp dir")
                .join(LOCALNET_RUNTIME_DIRECTORY)
                .join(LOCALNET_ONBOARDING_SIGNER_KEY_FILE)
        );
        let credentials = onboarding
            .get("credentials")
            .and_then(toml::Value::as_array)
            .expect("onboarding credentials");
        assert_eq!(credentials.len(), 1);
        let credential = credentials[0].as_table().expect("credential table");
        assert_eq!(
            credential.get("id").and_then(toml::Value::as_str),
            Some(LOCALNET_ONBOARDING_CREDENTIAL_ID)
        );
        let scope = credential
            .get("scope")
            .and_then(toml::Value::as_table)
            .expect("credential scope");
        assert_eq!(
            scope.get("domain").and_then(toml::Value::as_str),
            Some(CLIENT_ACCOUNT_DOMAIN)
        );
        let raw_token = fs::read_to_string(
            temp.path()
                .join(LOCALNET_RUNTIME_DIRECTORY)
                .join(LOCALNET_ONBOARDING_TOKEN_FILE),
        )
        .expect("read runtime-only onboarding token");
        assert_eq!(
            raw_token,
            raw_token.trim_end(),
            "the runtime token file must contain only the exact header credential"
        );
        assert!(!peer_config_text.contains(&raw_token));
        assert!(!peer_config_text.contains(operator.private_key.as_str()));
        assert!(!peer_config_text.contains(onboarding_identity.private_key.as_str()));
        assert!(!command_output.contains(&raw_token));
        assert!(!command_output.contains(operator.private_key.as_str()));
        assert!(!command_output.contains(onboarding_identity.private_key.as_str()));
        for peer_index in 0..opts.peers.get() {
            let config = fs::read_to_string(temp.path().join(format!("peer{peer_index}.toml")))
                .expect("read validator config for output redaction check");
            let config: toml::Value = toml::from_str(&config).expect("parse validator config");
            let private_key = config
                .get("private_key")
                .and_then(toml::Value::as_str)
                .expect("validator private key");
            assert!(!command_output.contains(private_key));
        }
        assert!(command_output.contains("onboarding_signer_key:"));
        assert!(command_output.contains("onboarding_token_file:"));
        assert!(command_output.contains("alias_setup_intent:"));
        let expected_digest = format!("blake3:{}", blake3::hash(raw_token.as_bytes()).to_hex());
        assert_eq!(
            credential.get("token_hash").and_then(toml::Value::as_str),
            Some(expected_digest.as_str())
        );
        let configured_program = onboarding
            .get("fee_sponsor_program_id")
            .and_then(toml::Value::as_str)
            .expect("exact onboarding fee sponsor program");
        let configured_program = configured_program
            .parse::<FeeSponsorProgramId>()
            .expect("canonical onboarding fee sponsor program id");
        let (genesis_public_key, _) =
            generate_genesis_key_pair(opts.seed.as_deref().map(str::as_bytes), GENESIS_SEED)
                .expect("derive expected genesis sponsor");
        assert_eq!(
            configured_program,
            localnet_fee_sponsor_program_id(&AccountId::new(genesis_public_key))
        );
        let manifest = RawGenesisTransaction::from_path(temp.path().join("genesis.json"))
            .expect("parse generated genesis");
        let setup_transaction = manifest
            .transactions()
            .last()
            .expect("alias setup transaction");
        let setup_instructions = setup_transaction
            .instructions()
            .iter()
            .map(|instruction| {
                instruction
                    .as_any()
                    .downcast_ref::<EnsureAlias>()
                    .cloned()
                    .expect("final transaction contains only EnsureAlias instructions")
            })
            .collect::<Vec<_>>();
        assert_eq!(setup_instructions.len(), 3);
        assert!(matches!(
            &setup_instructions[0].intent,
            AliasIntentV1::Dataspace(_)
        ));
        assert!(matches!(
            &setup_instructions[1].intent,
            AliasIntentV1::Domain(_)
        ));
        assert!(matches!(
            &setup_instructions[2].intent,
            AliasIntentV1::AccountAlias(_)
        ));
        let setup_intent_json =
            fs::read_to_string(temp.path().join(LOCALNET_ALIAS_SETUP_INTENT_FILE))
                .expect("read secret-free setup intent");
        assert!(!setup_intent_json.contains(raw_token.trim_end()));
        assert!(!setup_intent_json.contains(onboarding_identity.private_key.as_str()));
        let setup_request: AliasSetupPlanRequestV1 =
            norito::json::from_str(&setup_intent_json).expect("parse generated setup intent");
        assert_eq!(setup_request.intents, setup_instructions);
        let registrations = BootstrapRegistrations::from_manifest(&manifest);
        assert!(
            registrations
                .accounts
                .contains(&onboarding_identity.account_id),
            "onboarding signer must exist before Torii starts"
        );
        let domain = DomainId::parse_fully_qualified(CLIENT_ACCOUNT_DOMAIN)
            .expect("local onboarding domain");
        let expected_onboarding_permissions = BTreeSet::from([
            Permission::from(CanManageAccountAlias {
                scope: AccountAliasPermissionScope::Domain(domain.clone()),
            }),
            Permission::from(CanRegisterAccount {
                domain: domain.clone(),
            }),
            Permission::from(CanPublishSpaceDirectoryManifestForAccountDomain {
                dataspace: DataSpaceId::UNIVERSAL,
                domain,
            }),
            Permission::from(CanEnrollFeeSponsorProgram {
                program_id: configured_program.clone(),
            }),
        ]);
        let actual_onboarding_permissions = manifest
            .instructions()
            .filter_map(|instruction| instruction.as_any().downcast_ref::<GrantBox>())
            .filter_map(|grant| match grant {
                GrantBox::Permission(grant) => Some(grant),
                _ => None,
            })
            .filter(|grant| grant.destination() == &onboarding_identity.account_id)
            .map(|grant| grant.object().clone())
            .collect::<BTreeSet<_>>();
        assert_eq!(
            actual_onboarding_permissions, expected_onboarding_permissions,
            "generated onboarding must use only exact execution capabilities"
        );
        let onboarding_fee_asset = AssetId::new(
            localnet_fee_asset_definition_id(),
            onboarding_identity.account_id.clone(),
        );
        assert!(manifest.instructions().any(|instruction| {
            instruction
                .as_any()
                .downcast_ref::<MintBox>()
                .is_some_and(|mint| match mint {
                    MintBox::Asset(mint) => mint.destination() == &onboarding_fee_asset,
                    _ => false,
                })
        }));
        assert!(onboarding.get("fee_sponsor_account").is_none());
        assert!(onboarding.get("fee_sponsor_policy").is_none());
        assert_eq!(
            fs::read_to_string(temp.path().join(".gitignore")).expect("protective ignore file"),
            concat!(
                "# Kagami localnets contain private signing material and runtime tokens.\n",
                "*\n",
                "!.gitignore\n",
            )
        );
        #[cfg(unix)]
        {
            fn assert_private_tree_modes(root: &Path, path: &Path) {
                let metadata = fs::symlink_metadata(path).expect("private tree entry metadata");
                let relative = path.strip_prefix(root).expect("entry below localnet root");
                if metadata.is_dir() {
                    assert_eq!(
                        metadata.permissions().mode() & 0o777,
                        0o700,
                        "private directory must be owner-only: {}",
                        relative.display()
                    );
                    for entry in fs::read_dir(path).expect("read private localnet directory") {
                        let entry = entry.expect("read private localnet entry");
                        assert_private_tree_modes(root, &entry.path());
                    }
                    return;
                }
                assert!(
                    metadata.is_file(),
                    "fresh localnet must not contain special entries: {}",
                    relative.display()
                );
                assert_eq!(
                    metadata.nlink(),
                    1,
                    "fresh localnet files must be single-link: {}",
                    relative.display()
                );
                let expected_mode = if matches!(relative.to_str(), Some("start.sh" | "stop.sh")) {
                    0o700
                } else {
                    0o600
                };
                assert_eq!(
                    metadata.permissions().mode() & 0o777,
                    expected_mode,
                    "fresh localnet entry has the wrong custody mode: {}",
                    relative.display()
                );
            }
            assert_private_tree_modes(temp.path(), temp.path());
        }
        assert_eq!(
            peer_cfg
                .get("settlement")
                .and_then(toml::Value::as_table)
                .and_then(|settlement| settlement.get("offline")),
            None,
            "offline protocol support is universal and must not be represented as a localnet opt-in"
        );
    }
    #[test]
    fn generated_peer_config_includes_required_addr_literals() {
        let temp = tempfile::tempdir().expect("make temp dir");
        let opts = LocalnetOptions {
            sora_profile: None,
            perf_profile: None,
            peers: NonZeroU16::new(4).expect("non-zero"),
            seed: Some("kagami-addr-literals".to_owned()),
            bind_host: DEFAULT_BIND_HOST.to_owned(),
            public_host: DEFAULT_PUBLIC_HOST.to_owned(),
            base_api_port: 21080,
            base_p2p_port: 24337,
            out_dir: temp.path().to_path_buf(),
            extra_accounts: 0,
            assets: Vec::new(),
            block_cadence_ms: None,
            consensus_mode: SumeragiConsensusMode::Npos,
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
        assert!(
            peer_cfg.get("soranet_transport_public_key").is_some(),
            "soranet_transport_public_key is required"
        );
        assert!(
            peer_cfg.get("soranet_transport_private_key").is_some(),
            "soranet_transport_private_key is required"
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
    fn generated_peers_use_dedicated_deterministic_transport_and_streaming_identities() {
        let seed = b"kagami-transport-identity-test";
        let peers = build_peers(4, Some(seed), 21_080, 24_337).expect("build peers");
        let replay = build_peers(4, Some(seed), 21_080, 24_337).expect("rebuild peers");
        let mut transport_public_keys = std::collections::BTreeSet::new();
        let mut streaming_public_keys = std::collections::BTreeSet::new();
        for (peer, replay_peer) in peers.iter().zip(&replay) {
            KeyPair::new(
                peer.soranet_transport_public_key.clone(),
                peer.soranet_transport_private_key.0.clone(),
            )
            .expect("generated SoraNet transport key pair must match");
            assert_eq!(
                peer.soranet_transport_public_key
                    .try_algorithm()
                    .expect("transport public-key algorithm"),
                iroha_crypto::Algorithm::Ed25519
            );
            assert_ne!(peer.soranet_transport_public_key, peer.public_key);
            assert_eq!(
                peer.soranet_transport_public_key,
                replay_peer.soranet_transport_public_key
            );
            assert_eq!(
                peer.soranet_transport_private_key.to_string(),
                replay_peer.soranet_transport_private_key.to_string()
            );
            assert!(
                transport_public_keys.insert(peer.soranet_transport_public_key.clone()),
                "each localnet peer must receive a unique SoraNet transport identity"
            );
            KeyPair::new(
                peer.streaming_public_key.clone(),
                peer.streaming_private_key.0.clone(),
            )
            .expect("generated streaming identity key pair must match");
            assert_eq!(
                peer.streaming_public_key
                    .try_algorithm()
                    .expect("streaming public-key algorithm"),
                iroha_crypto::Algorithm::Ed25519
            );
            assert_ne!(peer.streaming_public_key, peer.public_key);
            assert_ne!(
                peer.streaming_public_key, peer.soranet_transport_public_key,
                "streaming control-plane and SoraNet transport identities must be domain-separated"
            );
            assert_eq!(
                peer.streaming_public_key, replay_peer.streaming_public_key,
                "seeded streaming identities must be reproducible"
            );
            assert_eq!(
                peer.streaming_private_key.to_string(),
                replay_peer.streaming_private_key.to_string(),
                "seeded streaming private keys must be reproducible"
            );
            assert!(
                streaming_public_keys.insert(peer.streaming_public_key.clone()),
                "each localnet peer must receive a unique streaming identity"
            );
        }
    }
    #[test]
    fn generated_peer_config_allows_bls_signing_for_npos() {
        let temp = tempfile::tempdir().expect("make temp dir");
        let opts = LocalnetOptions {
            sora_profile: None,
            perf_profile: None,
            peers: NonZeroU16::new(4).expect("non-zero"),
            seed: Some("kagami-crypto-allow".to_owned()),
            bind_host: DEFAULT_BIND_HOST.to_owned(),
            public_host: DEFAULT_PUBLIC_HOST.to_owned(),
            base_api_port: 31080,
            base_p2p_port: 35337,
            out_dir: temp.path().to_path_buf(),
            extra_accounts: 0,
            assets: Vec::new(),
            block_cadence_ms: None,
            consensus_mode: SumeragiConsensusMode::Npos,
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
            sora_profile: None,
            perf_profile: None,
            peers: NonZeroU16::new(4).expect("non-zero"),
            seed: Some("kagami-genesis-crypto".to_owned()),
            bind_host: DEFAULT_BIND_HOST.to_owned(),
            public_host: DEFAULT_PUBLIC_HOST.to_owned(),
            base_api_port: 32080,
            base_p2p_port: 36337,
            out_dir: temp.path().to_path_buf(),
            extra_accounts: 0,
            assets: Vec::new(),
            block_cadence_ms: None,
            consensus_mode: SumeragiConsensusMode::Npos,
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
            sora_profile: None,
            perf_profile: None,
            peers: NonZeroU16::new(4).expect("non-zero"),
            seed: Some("kagami-peer-telemetry".to_owned()),
            bind_host: DEFAULT_BIND_HOST.to_owned(),
            public_host: DEFAULT_PUBLIC_HOST.to_owned(),
            base_api_port: 19080,
            base_p2p_port: 23337,
            out_dir: temp.path().to_path_buf(),
            extra_accounts: 0,
            assets: Vec::new(),
            block_cadence_ms: None,
            consensus_mode: SumeragiConsensusMode::Npos,
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
            vec![
                "http://127.0.0.1:19080/",
                "http://127.0.0.1:19081/",
                "http://127.0.0.1:19082/",
                "http://127.0.0.1:19083/",
            ],
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
            .and_then(|torii| torii.get("api_rate_limit_bypass_cidrs"))
            .and_then(toml::Value::as_array)
            .expect("api_rate_limit_bypass_cidrs array");
        let allowlist = allowlist
            .iter()
            .filter_map(toml::Value::as_str)
            .collect::<Vec<_>>();
        assert_eq!(allowlist, LOCALNET_PREAUTH_ALLOW_CIDRS);
        let internal_trust = peer_cfg
            .get("torii")
            .and_then(toml::Value::as_table)
            .and_then(|torii| torii.get("internal_api_trusted_cidrs"))
            .and_then(toml::Value::as_array)
            .expect("internal_api_trusted_cidrs array");
        let internal_trust = internal_trust
            .iter()
            .filter_map(toml::Value::as_str)
            .collect::<Vec<_>>();
        assert_eq!(internal_trust, LOCALNET_INTERNAL_API_TRUSTED_CIDRS);
        assert_eq!(
            peer_cfg
                .get("torii")
                .and_then(toml::Value::as_table)
                .and_then(|torii| torii.get("max_content_len"))
                .and_then(toml::Value::as_integer),
            Some(
                i64::try_from(LOCALNET_TORII_MAX_CONTENT_LEN)
                    .expect("LOCALNET_TORII_MAX_CONTENT_LEN fits i64")
            ),
            "localnet configs should pin the resolved Torii body-cap default explicitly"
        );
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
    #[test]
    fn generated_sora_profile_peer_config_includes_mcp_writer_profile() {
        let temp = tempfile::tempdir().expect("make temp dir");
        let opts = LocalnetOptions {
            sora_profile: Some(SoraProfile::Nexus),
            perf_profile: None,
            peers: NonZeroU16::new(4).expect("non-zero"),
            seed: Some("kagami-taira-mcp".to_owned()),
            bind_host: DEFAULT_BIND_HOST.to_owned(),
            public_host: DEFAULT_PUBLIC_HOST.to_owned(),
            base_api_port: 29080,
            base_p2p_port: 33337,
            out_dir: temp.path().to_path_buf(),
            extra_accounts: 0,
            assets: Vec::new(),
            block_cadence_ms: None,
            consensus_mode: SumeragiConsensusMode::Npos,
        };
        generate_localnet(&opts, &mut BufWriter::new(Vec::new())).expect("generate localnet files");
        let peer_cfg: toml::Value = toml::from_str(
            &fs::read_to_string(temp.path().join("peer0.toml"))
                .expect("read generated peer config"),
        )
        .expect("parse peer config");
        let mcp = peer_cfg
            .get("torii")
            .and_then(toml::Value::as_table)
            .and_then(|torii| torii.get("mcp"))
            .and_then(toml::Value::as_table)
            .expect("torii.mcp table");
        assert_eq!(
            peer_cfg
                .get("torii")
                .and_then(toml::Value::as_table)
                .and_then(|torii| torii.get("max_content_len"))
                .and_then(toml::Value::as_integer),
            Some(
                i64::try_from(LOCALNET_TORII_MAX_CONTENT_LEN)
                    .expect("LOCALNET_TORII_MAX_CONTENT_LEN fits i64")
            ),
            "Sora-profile localnet should pin the resolved Torii body-cap default explicitly"
        );
        assert_eq!(
            mcp.get("enabled").and_then(toml::Value::as_bool),
            Some(true)
        );
        assert_eq!(
            mcp.get("profile").and_then(toml::Value::as_str),
            Some("writer")
        );
        let network = peer_cfg
            .get("network")
            .and_then(toml::Value::as_table)
            .expect("network table");
        assert_eq!(
            network
                .get("max_frame_bytes_tx_gossip")
                .and_then(toml::Value::as_integer),
            Some(
                i64::try_from(LOCALNET_MAX_FRAME_BYTES_TX_GOSSIP_NEXUS)
                    .expect("LOCALNET_MAX_FRAME_BYTES_TX_GOSSIP_NEXUS fits i64")
            ),
            "sora-profile localnet should raise tx gossip frame cap for large public writes"
        );
        assert_eq!(
            mcp.get("expose_operator_routes")
                .and_then(toml::Value::as_bool),
            Some(false)
        );
        let allow_prefixes = mcp
            .get("allow_tool_prefixes")
            .and_then(toml::Value::as_array)
            .expect("allow_tool_prefixes array")
            .iter()
            .filter_map(toml::Value::as_str)
            .collect::<Vec<_>>();
        assert_eq!(allow_prefixes, vec!["iroha."]);
    }
    #[test]
    #[expect(
        clippy::too_many_lines,
        reason = "the test audits the complete generated Sumeragi v2 schema and its prohibited legacy fields"
    )]
    fn generated_configs_use_strict_sumeragi_v2_schema() {
        let temp = tempfile::tempdir().expect("make temp dir");
        let opts = LocalnetOptions {
            sora_profile: None,
            perf_profile: None,
            peers: NonZeroU16::new(4).expect("non-zero"),
            seed: Some("kagami-channel-caps".to_owned()),
            bind_host: DEFAULT_BIND_HOST.to_owned(),
            public_host: DEFAULT_PUBLIC_HOST.to_owned(),
            base_api_port: 20080,
            base_p2p_port: 24337,
            out_dir: temp.path().to_path_buf(),
            extra_accounts: 0,
            assets: Vec::new(),
            block_cadence_ms: None,
            consensus_mode: SumeragiConsensusMode::Npos,
        };
        generate_localnet(&opts, &mut BufWriter::new(Vec::new())).expect("generate localnet files");
        let peer_cfg: toml::Value = toml::from_str(
            &fs::read_to_string(temp.path().join("peer0.toml")).expect("read generated config"),
        )
        .expect("parse generated config");
        let network = peer_cfg
            .get("network")
            .and_then(toml::Value::as_table)
            .expect("network table");
        let max_total_connections = network
            .get("max_total_connections")
            .and_then(toml::Value::as_integer)
            .and_then(|value| usize::try_from(value).ok())
            .expect("positive network connection bound");
        assert_eq!(
            max_total_connections, LOCALNET_MAX_TOTAL_CONNECTIONS,
            "the localnet connection envelope must be explicit"
        );
        assert_eq!(
            network
                .get("max_frame_bytes")
                .and_then(toml::Value::as_integer),
            Some(i64::try_from(LOCALNET_MAX_FRAME_BYTES).expect("production frame cap fits i64")),
            "the global encrypted frame cap must carry maximal topic plaintext plus P2P framing"
        );
        for (key, expected) in [
            (
                "max_frame_bytes_consensus",
                LOCALNET_MAX_FRAME_BYTES_CONSENSUS,
            ),
            (
                "max_frame_bytes_block_sync",
                LOCALNET_MAX_FRAME_BYTES_BLOCK_SYNC,
            ),
        ] {
            assert_eq!(
                network.get(key).and_then(toml::Value::as_integer),
                Some(i64::try_from(expected).expect("topic plaintext frame cap fits i64")),
                "{key} must remain within the global encrypted frame ceiling"
            );
        }
        assert_eq!(
            network
                .get("max_frame_bytes_control")
                .and_then(toml::Value::as_integer),
            Some(
                i64::try_from(LOCALNET_MAX_FRAME_BYTES_CONTROL)
                    .expect("control frame cap fits i64")
            ),
            "the control topic must carry worst-case proposals and timeout certificates"
        );
        let sumeragi = peer_cfg
            .get("sumeragi")
            .and_then(toml::Value::as_table)
            .expect("sumeragi table");
        assert!(
            !sumeragi.contains_key("round_timeout_ms"),
            "round timing is derived from signed genesis cadence"
        );
        assert_eq!(
            sumeragi.get("role").and_then(toml::Value::as_str),
            Some("validator")
        );
        let queues = sumeragi
            .get("queues")
            .and_then(toml::Value::as_table)
            .expect("sumeragi queues");
        assert_eq!(
            queues.get("commands").and_then(toml::Value::as_integer),
            Some(i64::try_from(LOCALNET_SUMERAGI_QUEUE_COMMANDS).expect("queue fits i64"))
        );
        assert_eq!(
            queues
                .get("authenticated_non_validator_sources")
                .and_then(toml::Value::as_integer),
            Some(
                i64::try_from(LOCALNET_SUMERAGI_AUTHENTICATED_NON_VALIDATOR_SOURCES)
                    .expect("source count fits i64")
            )
        );
        assert_eq!(
            queues.get("bodies").and_then(toml::Value::as_integer),
            Some(i64::try_from(LOCALNET_SUMERAGI_QUEUE_BODIES).expect("queue fits i64"))
        );
        assert_eq!(
            queues.get("body_bytes").and_then(toml::Value::as_integer),
            Some(198 * 1024 * 1024)
        );
        assert_eq!(
            queues
                .get("body_source_bytes")
                .and_then(toml::Value::as_integer),
            Some(
                i64::try_from(LOCALNET_SUMERAGI_QUEUE_BODY_SOURCE_BYTES)
                    .expect("source budget fits i64")
            )
        );
        assert_eq!(
            queues.get("chunks").and_then(toml::Value::as_integer),
            Some(i64::try_from(LOCALNET_SUMERAGI_QUEUE_CHUNKS).expect("queue fits i64"))
        );
        assert_eq!(
            queues.get("ready_bodies").and_then(toml::Value::as_integer),
            Some(i64::try_from(LOCALNET_SUMERAGI_QUEUE_READY_BODIES).expect("queue fits i64"))
        );
        let effect_work_capacity = (LOCALNET_SUMERAGI_QUEUE_COMMANDS
            / iroha_config::parameters::defaults::sumeragi::V2_RUNTIME_COMPLETION_RESERVE_DIVISOR)
            .max(1);
        actual::sumeragi_v2_lifecycle_capacity_geometry(
            usize::from(opts.peers.get()),
            effect_work_capacity,
            LOCALNET_SUMERAGI_QUEUE_BODIES,
            LOCALNET_SUMERAGI_AUTHENTICATED_NON_VALIDATOR_SOURCES,
        )
        .expect("generated localnet lifecycle geometry must remain admissible");
        let keys = sumeragi
            .get("keys")
            .and_then(toml::Value::as_table)
            .expect("sumeragi keys");
        assert_eq!(
            keys.get("allowed_algorithms")
                .and_then(toml::Value::as_array)
                .and_then(|algorithms| algorithms.first())
                .and_then(toml::Value::as_str),
            Some("bls_normal")
        );
        for retired in [
            "consensus_mode",
            "protocol_version",
            "da",
            "advanced",
            "recovery",
            "collectors",
            "rbc",
            "pacing_governor",
            "persistence",
        ] {
            assert!(
                !sumeragi.contains_key(retired),
                "generated v2 config must not contain retired sumeragi.{retired}"
            );
        }
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
    fn perf_profile_permissioned_applies_bounded_runtime_limits() {
        let temp = tempfile::tempdir().expect("tmp dir");
        let opts = LocalnetOptions {
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
            block_cadence_ms: None,
            consensus_mode: SumeragiConsensusMode::Permissioned,
        };
        generate_localnet(&opts, &mut BufWriter::new(Vec::new())).expect("generate localnet files");
        let source = TomlSource::from_file(temp.path().join("peer0.toml")).expect("read config");
        let parsed = actual::Root::from_toml_source(source).expect("config should parse");
        assert_eq!(
            parsed.queue.capacity.get(),
            LOCALNET_PERF_QUEUE_CAPACITY,
            "perf localnet should keep the fixed queue allocation bounded"
        );
        assert_eq!(
            parsed.queue.capacity_per_user.get(),
            LOCALNET_PERF_QUEUE_CAPACITY,
            "perf localnet per-user capacity should match the bounded queue capacity"
        );
        assert_eq!(
            parsed.torii.api_high_load_tx_threshold,
            Some(LOCALNET_PERF_QUEUE_CAPACITY),
            "perf localnet should expose backpressure at the bounded queue capacity"
        );
        assert_eq!(
            parsed.sumeragi.block.max_transactions.get(),
            LOCALNET_PERF_RUNTIME_BLOCK_MAX_TRANSACTIONS,
            "perf localnet should cap runtime proposal assembly below the semantic block max"
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
        assert_eq!(params.sumeragi().block_cadence_ms().get(), 1_000);
        assert_eq!(
            params.block.max_transactions.get(),
            LOCALNET_BLOCK_MAX_TRANSACTIONS
        );
    }
    #[test]
    fn perf_profile_npos_applies_election_and_runtime_limits() {
        let temp = tempfile::tempdir().expect("tmp dir");
        let opts = LocalnetOptions {
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
            block_cadence_ms: None,
            consensus_mode: SumeragiConsensusMode::Npos,
        };
        generate_localnet(&opts, &mut BufWriter::new(Vec::new())).expect("generate localnet files");
        let source = TomlSource::from_file(temp.path().join("peer0.toml")).expect("read config");
        let parsed = actual::Root::from_toml_source(source).expect("config should parse");
        let expected_filter: Directives = LOCALNET_PERF_LOGGER_FILTER
            .parse()
            .expect("perf logger filter should parse");
        assert_eq!(parsed.logger.filter, Some(expected_filter));
        assert_eq!(
            parsed.pipeline.signature_batch_max_ed25519,
            LOCALNET_SIGNATURE_BATCH_MAX_ED25519
        );
        assert_eq!(
            parsed.sumeragi.block.max_transactions.get(),
            LOCALNET_PERF_RUNTIME_BLOCK_MAX_TRANSACTIONS,
            "NPoS perf localnet should use the same bounded runtime proposal cap"
        );
        let genesis_path = temp.path().join("genesis.json");
        let manifest = genesis_json_from_path(&genesis_path);
        let params = genesis_parameters(&manifest);
        assert_eq!(params.sumeragi().block_cadence_ms().get(), 1_000);
        let npos = params
            .custom()
            .get(&SumeragiNposParameters::parameter_id())
            .and_then(SumeragiNposParameters::from_custom_parameter)
            .expect("npos parameters must be present");
        assert_eq!(npos.seat_band_pct(), 100);
        assert_eq!(npos.min_self_bond(), &Quantity::from(1_u64));
    }
    #[test]
    fn validate_localnet_options_rejects_perf_profile_mismatch() {
        let opts = LocalnetOptions {
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
            block_cadence_ms: None,
            consensus_mode: SumeragiConsensusMode::Permissioned,
        };
        let err = validate_localnet_options(&opts).expect_err("mismatch should fail");
        assert!(
            err.to_string().contains("perf-profile"),
            "unexpected error: {err}"
        );
    }
    #[test]
    fn test_build_uses_explicit_fixture_for_unseeded_genesis_helpers() {
        let (public_key, _) = generate_genesis_key_pair(None, GENESIS_SEED)
            .expect("unseeded genesis key fixture should succeed");
        assert_eq!(
            public_key,
            REAL_GENESIS_ACCOUNT_KEYPAIR.public_key().clone()
        );
    }
    #[test]
    fn extra_account_keys_are_unique_when_unseeded() {
        let (first, _) =
            generate_account_key_pair(None, b"acct0").expect("first random account key");
        let (second, _) =
            generate_account_key_pair(None, b"acct1").expect("second random account key");
        assert_ne!(first, second);
    }
    type ConsensusHandshakeMetaTest =
        iroha_data_model::parameter::system::ConsensusHandshakeMetadata;
    #[test]
    fn generated_genesis_handshake_meta_decodes() {
        let temp = tempfile::tempdir().expect("make temp dir");
        let opts = LocalnetOptions {
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
                name: LOCALNET_SAMPLE_ASSET_NAME.to_owned(),
                alias: None,
                owned_by: ALICE_ID.clone(),
                mint_to: ALICE_ID.clone(),
                quantity: 100,
            }],
            block_cadence_ms: None,
            consensus_mode: SumeragiConsensusMode::Npos,
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
        assert_eq!(
            meta.wire_protocol_version,
            u32::from(PROTOCOL_VERSION),
            "unexpected wire proto version"
        );
        assert!(
            meta.consensus_fingerprint.to_string().starts_with("0x"),
            "fingerprint must be hex-prefixed"
        );
    }
    #[test]
    fn localnet_signed_genesis_uses_first_release_npos_context() {
        let temp = tempfile::tempdir().expect("tmp dir");
        let opts = LocalnetOptions {
            sora_profile: None,
            perf_profile: None,
            peers: NonZeroU16::new(4).expect("non-zero"),
            seed: Some("localnet-da-enabled".to_owned()),
            bind_host: DEFAULT_BIND_HOST.to_owned(),
            public_host: DEFAULT_PUBLIC_HOST.to_owned(),
            base_api_port: 28180,
            base_p2p_port: 28437,
            out_dir: temp.path().to_path_buf(),
            extra_accounts: 0,
            assets: Vec::new(),
            block_cadence_ms: None,
            consensus_mode: SumeragiConsensusMode::Npos,
        };
        generate_localnet(&opts, &mut BufWriter::new(Vec::new())).expect("generate localnet");
        let manifest = localnet_genesis_for_opts(&opts);
        assert_eq!(manifest.consensus_mode(), SumeragiConsensusMode::Npos);
        assert!(manifest.consensus_fingerprint().is_some());
    }
    #[test]
    fn default_block_cadence_is_injected_when_unset() {
        let temp = tempfile::tempdir().expect("tmp dir");
        let opts = LocalnetOptions {
            sora_profile: None,
            perf_profile: None,
            peers: NonZeroU16::new(4).expect("non-zero"),
            seed: Some("default-pipeline-time".to_owned()),
            bind_host: DEFAULT_BIND_HOST.to_owned(),
            public_host: DEFAULT_PUBLIC_HOST.to_owned(),
            base_api_port: 28090,
            base_p2p_port: 28357,
            out_dir: temp.path().to_path_buf(),
            extra_accounts: 0,
            assets: Vec::new(),
            block_cadence_ms: None,
            consensus_mode: SumeragiConsensusMode::Npos,
        };
        generate_localnet(&opts, &mut BufWriter::new(Vec::new())).expect("generate localnet files");
        let genesis_path = temp.path().join("genesis.json");
        let manifest = genesis_json_from_path(&genesis_path);
        let params = genesis_parameters(&manifest);
        assert_eq!(
            params.sumeragi().block_cadence_ms().get(),
            LOCALNET_PIPELINE_TIME_MS
        );
    }
    #[test]
    fn localnet_sets_block_max_transactions() {
        let temp = tempfile::tempdir().expect("tmp dir");
        let opts = LocalnetOptions {
            sora_profile: None,
            perf_profile: None,
            peers: NonZeroU16::new(4).expect("non-zero"),
            seed: Some("localnet-block-max".to_owned()),
            bind_host: DEFAULT_BIND_HOST.to_owned(),
            public_host: DEFAULT_PUBLIC_HOST.to_owned(),
            base_api_port: 30080,
            base_p2p_port: 30337,
            out_dir: temp.path().to_path_buf(),
            extra_accounts: 0,
            assets: Vec::new(),
            block_cadence_ms: None,
            consensus_mode: SumeragiConsensusMode::Npos,
        };
        generate_localnet(&opts, &mut BufWriter::new(Vec::new())).expect("generate localnet files");
        let manifest = localnet_genesis_for_opts(&opts);
        let params = manifest
            .effective_parameters()
            .expect("generated localnet genesis has one structured parameter block");
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
            sora_profile: None,
            perf_profile: None,
            peers: NonZeroU16::new(4).expect("non-zero"),
            seed: Some("localnet-npos-stake".to_owned()),
            bind_host: DEFAULT_BIND_HOST.to_owned(),
            public_host: DEFAULT_PUBLIC_HOST.to_owned(),
            base_api_port: 31080,
            base_p2p_port: 31337,
            out_dir: temp.path().to_path_buf(),
            extra_accounts: 0,
            assets: Vec::new(),
            block_cadence_ms: None,
            consensus_mode: SumeragiConsensusMode::Npos,
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
        let params = manifest
            .effective_parameters()
            .expect("generated localnet genesis has one structured parameter block");
        let expected_stake_amount = localnet_npos_stake_amount(
            &params,
            opts.perf_profile
                .map(LocalnetPerfProfile::spec)
                .map(|spec| spec.stake_amount),
        );
        for register in &validators {
            assert_eq!(register.lane_id, LaneId::SINGLE);
            assert_eq!(register.validator, register.stake_account);
            assert_eq!(register.initial_stake, expected_stake_amount);
        }
        for activate in &activations {
            assert_eq!(activate.lane_id, LaneId::SINGLE);
        }
        let peers = build_peers(
            opts.peers.get(),
            opts.seed.as_ref().map(String::as_bytes),
            opts.base_api_port,
            opts.base_p2p_port,
        )
        .expect("test localnet peer key generation should succeed");
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
            min_self_bond: (LOCALNET_STAKE_AMOUNT + 1).into(),
            ..Default::default()
        };
        let expected = npos.min_self_bond.clone();
        params.set_parameter(Parameter::Custom(npos.into_custom_parameter()));
        let stake_amount = localnet_npos_stake_amount(&params, Some(LOCALNET_STAKE_AMOUNT));
        assert_eq!(stake_amount, expected);
    }
    fn assert_localnet_dataspace_catalog_quorum(out_dir: &Path, peer_count: NonZeroU16) {
        let peer_cfg: toml::Value = toml::from_str(
            &fs::read_to_string(out_dir.join("peer0.toml")).expect("read generated peer config"),
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
            let alias = entry
                .get("alias")
                .and_then(toml::Value::as_str)
                .expect("dataspace alias");
            let id = entry
                .get("id")
                .and_then(toml::Value::as_integer)
                .expect("dataspace id");
            assert_eq!(
                entry
                    .get("fault_tolerance")
                    .and_then(toml::Value::as_integer),
                Some(i64::from(fault_tolerance)),
                "fault tolerance should scale with peers"
            );
            if alias == "universal" {
                assert!(
                    entry.get("manifest_hash").is_none(),
                    "universal dataspace keeps the reserved id without a manifest hash"
                );
            } else {
                let expected_manifest = localnet_dataspace_manifest_hash(id);
                assert_eq!(
                    entry.get("manifest_hash").and_then(toml::Value::as_str),
                    Some(expected_manifest.as_str()),
                    "non-universal dataspaces must carry an id-derived manifest hash"
                );
            }
        }
    }
    #[test]
    fn localnet_npos_validator_roster_and_quorum_match_peer_count() {
        use std::collections::BTreeSet;
        let temp = tempfile::tempdir().expect("tmp dir");
        let peer_count = NonZeroU16::new(7).expect("non-zero");
        let opts = LocalnetOptions {
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
            block_cadence_ms: None,
            consensus_mode: SumeragiConsensusMode::Npos,
        };
        generate_localnet(&opts, &mut BufWriter::new(Vec::new())).expect("generate localnet files");
        let peer_cfg: toml::Value = toml::from_str(
            &fs::read_to_string(temp.path().join("peer0.toml"))
                .expect("read seven-validator peer config"),
        )
        .expect("parse seven-validator peer config");
        let queues = peer_cfg
            .get("sumeragi")
            .and_then(toml::Value::as_table)
            .and_then(|sumeragi| sumeragi.get("queues"))
            .and_then(toml::Value::as_table)
            .expect("seven-validator Sumeragi queues");
        assert_eq!(
            queues
                .get("body_source_bytes")
                .and_then(toml::Value::as_integer),
            Some(
                i64::try_from(LOCALNET_SUMERAGI_QUEUE_BODY_SOURCE_BYTES)
                    .expect("source budget fits i64")
            )
        );
        assert_eq!(
            queues.get("body_bytes").and_then(toml::Value::as_integer),
            Some(297 * 1024 * 1024),
            "seven validators and two authenticated non-validator sources each need one isolated body quota"
        );
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
        )
        .expect("test localnet peer key generation should succeed");
        let expected: BTreeSet<_> = peers
            .iter()
            .map(|peer| AccountId::new(peer.public_key.clone()))
            .collect();
        let actual: BTreeSet<_> = validators
            .iter()
            .map(|register| register.validator.clone())
            .collect();
        assert_eq!(actual, expected, "validator roster should match peers");
        assert_localnet_dataspace_catalog_quorum(temp.path(), peer_count);
    }
    #[test]
    fn localnet_body_ingress_budget_enforces_protocol_roster_limit() {
        for validator_count in [4, MAX_VALIDATORS_PER_HEIGHT] {
            assert_eq!(
                localnet_sumeragi_body_bytes(validator_count)
                    .expect("every legal endpoint roster must remain representable"),
                (validator_count + LOCALNET_SUMERAGI_AUTHENTICATED_NON_VALIDATOR_SOURCES)
                    * LOCALNET_SUMERAGI_QUEUE_BODY_SOURCE_BYTES,
                "body ingress bytes must scale once per isolated authenticated source"
            );
        }
        let geometry_error = localnet_sumeragi_body_bytes(5)
            .expect_err("a non-3f+1 roster must fail before capacity arithmetic");
        assert!(
            geometry_error
                .to_string()
                .contains("exact Sumeragi v2 3f+1"),
            "unexpected error: {geometry_error}"
        );
        let error = localnet_sumeragi_body_bytes(MAX_VALIDATORS_PER_HEIGHT + 1)
            .expect_err("an oversized roster must fail before capacity arithmetic");
        assert!(
            error
                .to_string()
                .contains("exceeds the Sumeragi v2 protocol maximum"),
            "unexpected error: {error}"
        );
    }
    include!("localnet/profile_policy_tests.rs");
    #[test]
    #[allow(clippy::too_many_lines)]
    fn nexus_localnet_alias_lanes_bind_dataspaces_and_seed_validators() {
        use std::collections::{BTreeMap, BTreeSet};
        let temp = tempfile::tempdir().expect("tmp dir");
        let peer_count = NonZeroU16::new(4).expect("non-zero");
        let opts = LocalnetOptions {
            sora_profile: Some(SoraProfile::Nexus),
            perf_profile: None,
            peers: peer_count,
            seed: Some("localnet-nexus-alias-lanes".to_owned()),
            bind_host: DEFAULT_BIND_HOST.to_owned(),
            public_host: DEFAULT_PUBLIC_HOST.to_owned(),
            base_api_port: 32080,
            base_p2p_port: 32337,
            out_dir: temp.path().to_path_buf(),
            extra_accounts: 0,
            assets: Vec::new(),
            block_cadence_ms: None,
            consensus_mode: SumeragiConsensusMode::Npos,
        };
        generate_localnet(&opts, &mut BufWriter::new(Vec::new())).expect("generate localnet");
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
            nexus
                .get("storage")
                .and_then(toml::Value::as_table)
                .and_then(|storage| storage.get("local_budget_bytes"))
                .and_then(toml::Value::as_integer),
            Some(
                i64::try_from(LOCALNET_NEXUS_STORAGE_BUDGET_BYTES)
                    .expect("localnet Nexus storage budget fits i64")
            ),
            "nexus localnet should use an explicit disposable storage budget"
        );
        assert_eq!(
            nexus.get("lane_count").and_then(toml::Value::as_integer),
            Some(LOCALNET_NEXUS_ALIAS_LANE_COUNT),
            "nexus profile should declare explicit alias-aware lane count"
        );
        let lane_catalog = nexus
            .get("lane_catalog")
            .and_then(toml::Value::as_array)
            .expect("nexus lane catalog");
        let lanes_by_alias: BTreeMap<_, _> = lane_catalog
            .iter()
            .map(|entry| {
                let entry = entry.as_table().expect("lane entry");
                let alias = entry
                    .get("alias")
                    .and_then(toml::Value::as_str)
                    .expect("lane alias")
                    .to_owned();
                let dataspace = entry
                    .get("dataspace")
                    .and_then(toml::Value::as_str)
                    .expect("lane dataspace")
                    .to_owned();
                let visibility = entry
                    .get("visibility")
                    .and_then(toml::Value::as_str)
                    .expect("lane visibility")
                    .to_owned();
                (alias, (dataspace, visibility))
            })
            .collect();
        assert_eq!(
            lanes_by_alias.get("paynet"),
            Some(&("paynet".to_owned(), "public".to_owned()))
        );
        assert_eq!(
            lanes_by_alias.get("nexus"),
            Some(&("nexus".to_owned(), "public".to_owned()))
        );
        assert_eq!(
            lanes_by_alias.get("governance"),
            Some(&("universal".to_owned(), "public".to_owned()))
        );
        assert_eq!(
            lanes_by_alias.get("zk"),
            Some(&("universal".to_owned(), "public".to_owned()))
        );
        let dataspace_catalog = nexus
            .get("dataspace_catalog")
            .and_then(toml::Value::as_array)
            .expect("nexus dataspace catalog");
        let dataspaces_by_alias: BTreeMap<_, _> = dataspace_catalog
            .iter()
            .map(|entry| {
                let entry = entry.as_table().expect("dataspace entry");
                let alias = entry
                    .get("alias")
                    .and_then(toml::Value::as_str)
                    .expect("dataspace alias")
                    .to_owned();
                let id = entry
                    .get("id")
                    .and_then(toml::Value::as_integer)
                    .expect("dataspace id");
                (alias, id)
            })
            .collect();
        assert_eq!(
            dataspaces_by_alias
                .keys()
                .map(String::as_str)
                .collect::<BTreeSet<_>>(),
            BTreeSet::from(["nexus", "paynet", "universal"]),
            "logical governance and zk lanes must not create localnet dataspaces"
        );
        assert_eq!(
            dataspaces_by_alias.get("paynet"),
            Some(
                &i64::try_from(LOCALNET_PAYNET_ALIAS_DATASPACE_ID)
                    .expect("PAYNET dataspace id fits i64")
            )
        );
        assert_eq!(
            dataspaces_by_alias.get("nexus"),
            Some(
                &i64::try_from(LOCALNET_CBUAE_ALIAS_DATASPACE_ID)
                    .expect("CBUAE dataspace id fits i64")
            )
        );
        let manifest = localnet_genesis_for_opts(&opts);
        let mut registrations_by_lane = BTreeMap::<u32, usize>::new();
        let mut activations_by_lane = BTreeMap::<u32, usize>::new();
        for instruction in manifest.instructions() {
            if let Some(register) = instruction
                .as_any()
                .downcast_ref::<RegisterPublicLaneValidator>()
            {
                *registrations_by_lane
                    .entry(register.lane_id.as_u32())
                    .or_default() += 1;
            }
            if let Some(activate) = instruction
                .as_any()
                .downcast_ref::<ActivatePublicLaneValidator>()
            {
                *activations_by_lane
                    .entry(activate.lane_id.as_u32())
                    .or_default() += 1;
            }
        }
        let expected_lanes = BTreeSet::from([
            LaneId::SINGLE.as_u32(),
            LOCALNET_PAYNET_ALIAS_LANE_INDEX,
            LOCALNET_CBUAE_ALIAS_LANE_INDEX,
        ]);
        assert_eq!(
            registrations_by_lane
                .keys()
                .copied()
                .collect::<BTreeSet<_>>(),
            expected_lanes,
            "nexus localnet should seed validators for each public alias lane"
        );
        assert_eq!(
            activations_by_lane.keys().copied().collect::<BTreeSet<_>>(),
            expected_lanes,
            "nexus localnet should activate validators for each public alias lane"
        );
        for lane in expected_lanes {
            assert_eq!(
                registrations_by_lane.get(&lane),
                Some(&usize::from(peer_count.get())),
                "expected one validator registration per peer on lane {lane}"
            );
            assert_eq!(
                activations_by_lane.get(&lane),
                Some(&usize::from(peer_count.get())),
                "expected one validator activation per peer on lane {lane}"
            );
        }
    }
    #[test]
    fn private_dataspace_cli_selector_is_typed_and_fail_closed() {
        use clap::Parser as _;
        #[derive(clap::Parser)]
        struct TestArgs {
            #[command(flatten)]
            localnet: Args,
        }
        let parsed = TestArgs::try_parse_from([
            "kagami-localnet-test",
            "--out-dir",
            "/tmp/kagami-localnet-test",
            "--sora-profile",
            "dataspace",
            "--private-dataspace",
            "sbp",
        ])
        .expect("parse typed SBP private dataspace selector");
        assert_eq!(
            resolve_sora_profile(
                parsed.localnet.sora_profile,
                parsed.localnet.private_dataspace,
            )
            .expect("resolve typed SBP private dataspace selector"),
            Some(SoraProfile::PrivateSbp)
        );
        assert!(
            TestArgs::try_parse_from([
                "kagami-localnet-test",
                "--out-dir",
                "/tmp/kagami-localnet-test",
                "--private-dataspace",
                "cbuae",
            ])
            .is_err(),
            "private dataspace selection must require an explicit Sora profile"
        );
        assert!(
            resolve_sora_profile(
                Some(SoraProfileArg::Nexus),
                Some(PrivateDataspaceArg::Cbuae),
            )
            .is_err(),
            "private dataspace selection must reject the public Nexus profile"
        );
    }
    #[test]
    fn localnet_cli_accepts_an_explicit_canonical_chain_id() {
        use clap::Parser as _;
        #[derive(clap::Parser)]
        struct TestArgs {
            #[command(flatten)]
            localnet: Args,
        }
        let parsed = TestArgs::try_parse_from([
            "kagami-localnet-test",
            "--out-dir",
            "/tmp/kagami-localnet-test",
            "--chain-id",
            "fc56984b-2be7-431d-840e-21514d1883f0",
        ])
        .expect("parse explicit localnet chain id");
        assert_eq!(
            resolve_localnet_chain_id(Some(&parsed.localnet.chain_id))
                .expect("resolve explicit localnet chain id"),
            "fc56984b-2be7-431d-840e-21514d1883f0"
        );
        assert!(resolve_localnet_chain_id(Some("   ")).is_err());
    }
    #[test]
    #[allow(clippy::too_many_lines)]
    fn private_dataspace_profiles_match_the_pk_routing_contract() {
        fn expected_dataspace(
            alias: &str,
            id: i64,
            description: &str,
            fault_tolerance: i64,
        ) -> toml::Value {
            let mut entry = toml::Table::new();
            entry.insert("alias".into(), toml::Value::String(alias.to_owned()));
            entry.insert("id".into(), toml::Value::Integer(id));
            if id != 0 {
                entry.insert(
                    "manifest_hash".into(),
                    toml::Value::String(localnet_dataspace_manifest_hash(id)),
                );
            }
            entry.insert(
                "description".into(),
                toml::Value::String(description.to_owned()),
            );
            entry.insert(
                "fault_tolerance".into(),
                toml::Value::Integer(fault_tolerance),
            );
            toml::Value::Table(entry)
        }
        fn expected_lane(
            index: i64,
            alias: &str,
            description: &str,
            dataspace: &str,
            visibility: &str,
            governance: Option<&str>,
        ) -> toml::Value {
            let mut entry = toml::Table::new();
            entry.insert("index".into(), toml::Value::Integer(index));
            entry.insert("alias".into(), toml::Value::String(alias.to_owned()));
            entry.insert(
                "description".into(),
                toml::Value::String(description.to_owned()),
            );
            entry.insert(
                "dataspace".into(),
                toml::Value::String(dataspace.to_owned()),
            );
            entry.insert(
                "visibility".into(),
                toml::Value::String(visibility.to_owned()),
            );
            if let Some(governance) = governance {
                entry.insert(
                    "governance".into(),
                    toml::Value::String(governance.to_owned()),
                );
            }
            entry.insert("metadata".into(), toml::Value::Table(toml::Table::new()));
            toml::Value::Table(entry)
        }
        struct Case {
            profile: SoraProfile,
            alias: &'static str,
            id: i64,
            lane: i64,
            lane_count: i64,
            dataspace_description: &'static str,
            lane_description: &'static str,
            routes: &'static [(&'static str, &'static str, i64, &'static str)],
        }
        const SBP_ROUTES: &[(&str, &str, i64, &str)] = &[
            ("account", "*@sbp", 3, "sbp"),
            ("account", "*@hbl.sbp", 3, "sbp"),
            ("account", "*@ubl.sbp", 3, "sbp"),
            ("instruction", "governance", 1, "universal"),
            ("instruction", "smartcontract::deploy", 2, "universal"),
            ("instruction", "transfer::asset@sbp", 3, "sbp"),
            ("instruction", "transfer::asset@hbl.sbp", 3, "sbp"),
            ("instruction", "transfer::asset@ubl.sbp", 3, "sbp"),
        ];
        const CBUAE_ROUTES: &[(&str, &str, i64, &str)] = &[
            ("account", "*@cbuae", 4, "cbuae"),
            ("instruction", "governance", 1, "universal"),
            ("instruction", "smartcontract::deploy", 2, "universal"),
            ("instruction", "transfer::asset@cbuae", 4, "cbuae"),
        ];
        let cases = [
            Case {
                profile: SoraProfile::PrivateSbp,
                alias: "sbp",
                id: 10,
                lane: 3,
                lane_count: 4,
                dataspace_description: "State Bank of Pakistan dataspace",
                lane_description: "State Bank of Pakistan private lane",
                routes: SBP_ROUTES,
            },
            Case {
                profile: SoraProfile::PrivateCbuae,
                alias: "cbuae",
                id: 12,
                lane: 4,
                lane_count: 5,
                dataspace_description: "CBUAE dataspace",
                lane_description: "CBUAE private lane",
                routes: CBUAE_ROUTES,
            },
        ];
        for case in cases {
            let profile = Some(case.profile);
            let dataspace_catalog = localnet_dataspace_catalog(profile, 1);
            assert_eq!(
                dataspace_catalog,
                vec![
                    expected_dataspace(
                        "universal",
                        0,
                        "Shared public data space for core, governance, and zero-knowledge lanes",
                        1,
                    ),
                    expected_dataspace(case.alias, case.id, case.dataspace_description, 1,),
                ],
                "private dataspace catalog must exactly match the canonical PK catalog"
            );
            let (lane_count, lane_catalog) =
                localnet_lane_catalog(profile).expect("private lane catalog");
            assert_eq!(lane_count, case.lane_count);
            assert_eq!(
                lane_catalog,
                vec![
                    expected_lane(
                        0,
                        "core",
                        "Primary public lane",
                        "universal",
                        "public",
                        None,
                    ),
                    expected_lane(
                        1,
                        "governance",
                        "Governance lane",
                        "universal",
                        "public",
                        None,
                    ),
                    expected_lane(2, "zk", "Zero-knowledge lane", "universal", "public", None,),
                    expected_lane(
                        case.lane,
                        case.alias,
                        case.lane_description,
                        case.alias,
                        "restricted",
                        Some("parliament"),
                    ),
                ],
                "private lane catalog must exactly match the canonical PK catalog (CBUAE leaves lane 3 sparse)"
            );
            let routing = localnet_routing_policy(profile).expect("private routing policy");
            let observed = routing
                .get("rules")
                .and_then(toml::Value::as_array)
                .expect("private routing rules")
                .iter()
                .map(|rule| {
                    let rule = rule.as_table().expect("routing rule table");
                    let matcher = rule
                        .get("matcher")
                        .and_then(toml::Value::as_table)
                        .expect("routing matcher");
                    let (matcher_kind, matcher_value) = ["account", "instruction"]
                        .into_iter()
                        .find_map(|kind| {
                            matcher
                                .get(kind)
                                .and_then(toml::Value::as_str)
                                .map(|value| (kind.to_owned(), value.to_owned()))
                        })
                        .expect("account or instruction routing matcher");
                    (
                        matcher_kind,
                        matcher_value,
                        rule.get("lane")
                            .and_then(toml::Value::as_integer)
                            .expect("routing lane"),
                        rule.get("dataspace")
                            .and_then(toml::Value::as_str)
                            .expect("routing dataspace")
                            .to_owned(),
                    )
                })
                .collect::<Vec<_>>();
            let expected = case
                .routes
                .iter()
                .map(|(kind, matcher, lane, dataspace)| {
                    (
                        (*kind).to_owned(),
                        (*matcher).to_owned(),
                        *lane,
                        (*dataspace).to_owned(),
                    )
                })
                .collect::<Vec<_>>();
            assert_eq!(
                observed, expected,
                "routing identity and order must be exact"
            );
            assert_eq!(
                localnet_public_validator_lanes(profile),
                vec![LaneId::SINGLE],
                "only the canonical owner of the shared universal physical dataspace may receive mutable NPoS staking state"
            );
        }
    }
    #[test]
    fn private_dataspace_manifests_use_the_selected_lane_alias() {
        use std::collections::BTreeSet;
        let peers = build_peers(4, Some(b"private-dataspace-manifest"), 34_080, 34_337)
            .expect("build deterministic manifest validators");
        let expected_peer_ids = peers
            .iter()
            .map(|peer| PeerId::from(peer.public_key.clone()).to_string())
            .collect::<BTreeSet<_>>();
        for (profile, alias) in [
            (SoraProfile::PrivateSbp, "sbp"),
            (SoraProfile::PrivateCbuae, "cbuae"),
        ] {
            let temp = tempfile::tempdir().expect("tmp dir");
            let manifest_directory =
                write_localnet_lane_manifests(temp.path(), Some(profile), &peers, None)
                    .expect("write private lane manifest")
                    .expect("private lane manifest directory");
            assert_eq!(manifest_directory, temp.path().join("lane-manifests"));
            let manifest_files = fs::read_dir(&manifest_directory)
                .expect("read private lane manifest directory")
                .map(|entry| {
                    entry
                        .expect("private lane manifest directory entry")
                        .file_name()
                        .to_string_lossy()
                        .into_owned()
                })
                .collect::<Vec<_>>();
            assert_eq!(manifest_files, vec![format!("{alias}.manifest.json")]);
            let manifest: json::Value = json::from_str(
                &fs::read_to_string(manifest_directory.join(format!("{alias}.manifest.json")))
                    .expect("read private lane manifest"),
            )
            .expect("parse private lane manifest");
            let manifest_fields = manifest
                .as_object()
                .expect("private lane manifest object")
                .keys()
                .map(String::as_str)
                .collect::<BTreeSet<_>>();
            assert_eq!(
                manifest_fields,
                BTreeSet::from(["governance", "lane", "quorum", "validators", "version"])
            );
            assert_eq!(
                manifest.get("lane").and_then(json::Value::as_str),
                Some(alias)
            );
            assert_eq!(
                manifest.get("governance").and_then(json::Value::as_str),
                Some("parliament")
            );
            assert_eq!(
                manifest.get("quorum").and_then(json::Value::as_u64),
                Some(3)
            );
            assert_eq!(
                manifest.get("version").and_then(json::Value::as_u64),
                Some(1)
            );
            let manifest_peer_ids = manifest
                .get("validators")
                .and_then(json::Value::as_array)
                .expect("private lane manifest validators")
                .iter()
                .map(|validator| {
                    validator
                        .get("peer_id")
                        .and_then(json::Value::as_str)
                        .expect("private lane manifest peer id")
                        .to_owned()
                })
                .collect::<BTreeSet<_>>();
            assert_eq!(manifest_peer_ids, expected_peer_ids);
        }
    }
    #[test]
    #[allow(clippy::too_many_lines)]
    fn dataspace_localnet_binds_paynet_restricted_lane_before_genesis_signing() {
        use std::collections::{BTreeMap, BTreeSet};
        let temp = tempfile::tempdir().expect("tmp dir");
        let peer_count = NonZeroU16::new(4).expect("non-zero");
        let opts = LocalnetOptions {
            sora_profile: Some(SoraProfile::Dataspace),
            perf_profile: None,
            peers: peer_count,
            seed: Some("localnet-paynet-dataspace-lane".to_owned()),
            bind_host: DEFAULT_BIND_HOST.to_owned(),
            public_host: DEFAULT_PUBLIC_HOST.to_owned(),
            base_api_port: 34080,
            base_p2p_port: 34337,
            out_dir: temp.path().to_path_buf(),
            extra_accounts: 0,
            assets: Vec::new(),
            block_cadence_ms: None,
            consensus_mode: SumeragiConsensusMode::Npos,
        };
        generate_localnet(&opts, &mut BufWriter::new(Vec::new())).expect("generate localnet");
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
            nexus.get("lane_count").and_then(toml::Value::as_integer),
            Some(LOCALNET_PAYNET_ALIAS_LANE_COUNT),
            "dataspace profile should declare the PAYNET lane before genesis signing"
        );
        let lane_catalog = nexus
            .get("lane_catalog")
            .and_then(toml::Value::as_array)
            .expect("nexus lane catalog");
        let lanes_by_alias: BTreeMap<_, _> = lane_catalog
            .iter()
            .map(|entry| {
                let entry = entry.as_table().expect("lane entry");
                let alias = entry
                    .get("alias")
                    .and_then(toml::Value::as_str)
                    .expect("lane alias")
                    .to_owned();
                let dataspace = entry
                    .get("dataspace")
                    .and_then(toml::Value::as_str)
                    .expect("lane dataspace")
                    .to_owned();
                let visibility = entry
                    .get("visibility")
                    .and_then(toml::Value::as_str)
                    .expect("lane visibility")
                    .to_owned();
                let governance = entry
                    .get("governance")
                    .and_then(toml::Value::as_str)
                    .map(str::to_owned);
                (alias, (dataspace, visibility, governance))
            })
            .collect();
        assert_eq!(
            lanes_by_alias.get("paynet"),
            Some(&(
                "paynet".to_owned(),
                "restricted".to_owned(),
                Some("parliament".to_owned()),
            ))
        );
        let registry = nexus
            .get("registry")
            .and_then(toml::Value::as_table)
            .expect("dataspace lane manifest registry");
        let manifest_directory = registry
            .get("manifest_directory")
            .and_then(toml::Value::as_str)
            .expect("dataspace lane manifest directory");
        assert_eq!(
            fs::canonicalize(manifest_directory).expect("canonical manifest directory"),
            fs::canonicalize(temp.path().join("lane-manifests"))
                .expect("canonical expected manifest directory")
        );
        let governance = nexus
            .get("governance")
            .and_then(toml::Value::as_table)
            .expect("dataspace governance catalog");
        assert_eq!(
            governance
                .get("default_module")
                .and_then(toml::Value::as_str),
            Some("parliament")
        );
        assert_eq!(
            governance
                .get("modules")
                .and_then(toml::Value::as_table)
                .and_then(|modules| modules.get("parliament"))
                .and_then(toml::Value::as_table)
                .and_then(|module| module.get("module_type"))
                .and_then(toml::Value::as_str),
            Some("parliament_sortition_jit")
        );
        let parliament_params = governance
            .get("modules")
            .and_then(toml::Value::as_table)
            .and_then(|modules| modules.get("parliament"))
            .and_then(toml::Value::as_table)
            .and_then(|module| module.get("params"))
            .and_then(toml::Value::as_table)
            .expect("parliament governance params");
        assert_eq!(
            parliament_params
                .get("selection")
                .and_then(toml::Value::as_str),
            Some("multibody_sortition")
        );
        assert_eq!(
            parliament_params
                .get("approval_flow")
                .and_then(toml::Value::as_str),
            Some("jit")
        );
        let paynet_manifest: json::Value = json::from_str(
            &fs::read_to_string(temp.path().join("lane-manifests/paynet.manifest.json"))
                .expect("read PAYNET lane manifest"),
        )
        .expect("parse PAYNET lane manifest");
        assert_eq!(
            paynet_manifest.get("lane").and_then(json::Value::as_str),
            Some("paynet")
        );
        assert_eq!(
            paynet_manifest
                .get("governance")
                .and_then(json::Value::as_str),
            Some("parliament")
        );
        assert_eq!(
            paynet_manifest
                .get("validators")
                .and_then(json::Value::as_array)
                .map(Vec::len),
            Some(usize::from(peer_count.get()))
        );
        assert_eq!(
            paynet_manifest.get("quorum").and_then(json::Value::as_u64),
            Some(3)
        );
        let dataspace_catalog = nexus
            .get("dataspace_catalog")
            .and_then(toml::Value::as_array)
            .expect("nexus dataspace catalog");
        let dataspaces_by_alias: BTreeMap<_, _> = dataspace_catalog
            .iter()
            .map(|entry| {
                let entry = entry.as_table().expect("dataspace entry");
                let alias = entry
                    .get("alias")
                    .and_then(toml::Value::as_str)
                    .expect("dataspace alias")
                    .to_owned();
                let id = entry
                    .get("id")
                    .and_then(toml::Value::as_integer)
                    .expect("dataspace id");
                (alias, id)
            })
            .collect();
        assert_eq!(
            dataspaces_by_alias.get("paynet"),
            Some(
                &i64::try_from(LOCALNET_PAYNET_ALIAS_DATASPACE_ID)
                    .expect("PAYNET dataspace id fits i64")
            )
        );
        let routing_policy = nexus
            .get("routing_policy")
            .and_then(toml::Value::as_table)
            .expect("routing policy");
        let account_rules: BTreeSet<_> = routing_policy
            .get("rules")
            .and_then(toml::Value::as_array)
            .expect("routing rules")
            .iter()
            .filter_map(|rule| {
                rule.as_table()
                    .and_then(|rule| rule.get("matcher"))
                    .and_then(toml::Value::as_table)
                    .and_then(|matcher| matcher.get("account"))
                    .and_then(toml::Value::as_str)
                    .map(str::to_owned)
            })
            .collect();
        assert_eq!(
            account_rules,
            BTreeSet::from(["*@paynet".to_owned(), "*@mibank.paynet".to_owned()])
        );
        let manifest = localnet_genesis_for_opts(&opts);
        let validator_lanes = manifest
            .instructions()
            .filter_map(|instruction| {
                instruction
                    .as_any()
                    .downcast_ref::<RegisterPublicLaneValidator>()
                    .map(|register| register.lane_id.as_u32())
            })
            .collect::<BTreeSet<_>>();
        assert_eq!(
            validator_lanes,
            BTreeSet::from([LaneId::SINGLE.as_u32()]),
            "restricted PAYNET dataspace lane must not be bootstrapped as a public stake-elected lane"
        );
        let source = TomlSource::from_file(temp.path().join("peer0.toml")).expect("read config");
        let parsed = actual::Root::from_toml_source(source).expect("config should parse");
        let expected_hash = iroha_core::da::proof_policy_bundle_hash(&parsed.nexus.lane_config);
        let bytes = fs::read(temp.path().join("genesis.signed.nrt")).expect("read signed genesis");
        let block =
            decode_framed_signed_block(&bytes).expect("decode signed genesis from framed payload");
        assert_eq!(
            block.header().da_proof_policies_hash(),
            Some(expected_hash),
            "signed genesis should embed the PAYNET dataspace proof policy bundle from peer config"
        );
    }
    #[test]
    fn nexus_localnet_signed_genesis_uses_peer_config_da_proof_policies() {
        let temp = tempfile::tempdir().expect("tmp dir");
        let opts = LocalnetOptions {
            sora_profile: Some(SoraProfile::Nexus),
            perf_profile: None,
            peers: NonZeroU16::new(4).expect("non-zero"),
            seed: Some("localnet-nexus-da-proof-policy".to_owned()),
            bind_host: DEFAULT_BIND_HOST.to_owned(),
            public_host: DEFAULT_PUBLIC_HOST.to_owned(),
            base_api_port: 33080,
            base_p2p_port: 33337,
            out_dir: temp.path().to_path_buf(),
            extra_accounts: 0,
            assets: Vec::new(),
            block_cadence_ms: None,
            consensus_mode: SumeragiConsensusMode::Npos,
        };
        generate_localnet(&opts, &mut BufWriter::new(Vec::new())).expect("generate localnet");
        let source = TomlSource::from_file(temp.path().join("peer0.toml")).expect("read config");
        let parsed = actual::Root::from_toml_source(source).expect("config should parse");
        let expected_hash = iroha_core::da::proof_policy_bundle_hash(&parsed.nexus.lane_config);
        let expected_confidential_policy_hash =
            iroha_core::state::compute_genesis_confidential_policy_hash(&parsed.zk);
        let bytes = fs::read(temp.path().join("genesis.signed.nrt")).expect("read signed genesis");
        let block =
            decode_framed_signed_block(&bytes).expect("decode signed genesis from framed payload");
        assert_eq!(
            block.header().da_proof_policies_hash(),
            Some(expected_hash),
            "signed genesis should embed the same DA proof policy bundle as peer configs",
        );
        assert_eq!(
            block
                .header()
                .confidential_features()
                .expect("signed genesis should carry confidential feature digest")
                .zk_policy_hash,
            Some(expected_confidential_policy_hash),
            "signed genesis should embed the same genesis confidential policy as peer configs",
        );
    }
    #[test]
    fn permissioned_localnet_pins_gas_limit_without_enabling_gas_fees() {
        let temp = tempfile::tempdir().expect("tmp dir");
        let opts = LocalnetOptions {
            sora_profile: None,
            perf_profile: None,
            peers: NonZeroU16::new(4).expect("non-zero"),
            seed: Some("permissioned-gas-metering-only".to_owned()),
            bind_host: DEFAULT_BIND_HOST.to_owned(),
            public_host: DEFAULT_PUBLIC_HOST.to_owned(),
            base_api_port: 28080,
            base_p2p_port: 28337,
            out_dir: temp.path().to_path_buf(),
            extra_accounts: 0,
            assets: Vec::new(),
            block_cadence_ms: Some(1_000),
            consensus_mode: SumeragiConsensusMode::Permissioned,
        };
        generate_localnet(&opts, &mut BufWriter::new(Vec::new())).expect("generate localnet files");
        let manifest = genesis_json_from_path(&temp.path().join("genesis.json"));
        let params = genesis_parameters(&manifest);
        let gas_limit: u64 = params
            .custom()
            .get(&localnet_custom_parameter_id("ivm_gas_limit_per_block"))
            .expect("permissioned localnet should pin the IVM gas limit")
            .payload()
            .try_into_any_norito()
            .expect("gas limit payload should decode");
        assert_eq!(gas_limit, LOCALNET_IVM_GAS_LIMIT_PER_BLOCK);
        assert!(
            !params
                .custom()
                .contains_key(&localnet_custom_parameter_id("ivm_gas_accepted_assets")),
            "permissioned localnet must not enable gas fee assets without bootstrapping XOR"
        );
        assert!(
            !params
                .custom()
                .contains_key(&localnet_custom_parameter_id("ivm_gas_units_per_gas")),
            "permissioned localnet must not override peer gas rates to a charging value"
        );
    }
    #[test]
    fn block_cadence_override_is_signed_into_genesis() {
        let temp = tempfile::tempdir().expect("tmp dir");
        let opts = LocalnetOptions {
            sora_profile: None,
            perf_profile: None,
            peers: NonZeroU16::new(4).expect("non-zero"),
            seed: Some("block-time-commit-default".to_owned()),
            bind_host: DEFAULT_BIND_HOST.to_owned(),
            public_host: DEFAULT_PUBLIC_HOST.to_owned(),
            base_api_port: 28080,
            base_p2p_port: 28337,
            out_dir: temp.path().to_path_buf(),
            extra_accounts: 0,
            assets: Vec::new(),
            block_cadence_ms: Some(1_000),
            consensus_mode: SumeragiConsensusMode::Npos,
        };
        generate_localnet(&opts, &mut BufWriter::new(Vec::new())).expect("generate localnet files");
        let genesis_path = temp.path().join("genesis.json");
        let manifest = genesis_json_from_path(&genesis_path);
        let params = genesis_parameters(&manifest);
        assert_eq!(params.sumeragi().block_cadence_ms().get(), 1_000);
        let gas_param_id = localnet_custom_parameter_id("ivm_gas_limit_per_block");
        let gas_limit: u64 = params
            .custom()
            .get(&gas_param_id)
            .expect("localnet should pin the IVM gas limit")
            .payload()
            .try_into_any_norito()
            .expect("gas limit payload should decode");
        assert_eq!(gas_limit, LOCALNET_IVM_GAS_LIMIT_PER_BLOCK);
        let expected_fee_asset = localnet_fee_asset_literal();
        let accepted_assets: Vec<String> = params
            .custom()
            .get(&localnet_custom_parameter_id("ivm_gas_accepted_assets"))
            .expect("localnet should pin accepted IVM gas assets")
            .payload()
            .try_into_any_norito()
            .expect("accepted assets payload should decode");
        assert_eq!(accepted_assets, vec![expected_fee_asset.clone()]);
        let units_per_gas = params
            .custom()
            .get(&localnet_custom_parameter_id("ivm_gas_units_per_gas"))
            .expect("localnet should pin IVM gas rates")
            .payload();
        assert_eq!(
            units_per_gas,
            &localnet_ivm_gas_units_per_gas_payload(&expected_fee_asset)
        );
    }
    #[test]
    fn npos_localnet_keeps_payload_for_fast_block_cadence() {
        let temp = tempfile::tempdir().expect("tmp dir");
        let opts = LocalnetOptions {
            sora_profile: None,
            perf_profile: None,
            peers: NonZeroU16::new(4).expect("non-zero"),
            seed: Some("npos-fast-timeouts".to_owned()),
            bind_host: DEFAULT_BIND_HOST.to_owned(),
            public_host: DEFAULT_PUBLIC_HOST.to_owned(),
            base_api_port: 38180,
            base_p2p_port: 38437,
            out_dir: temp.path().to_path_buf(),
            extra_accounts: 0,
            assets: Vec::new(),
            block_cadence_ms: Some(333),
            consensus_mode: SumeragiConsensusMode::Npos,
        };
        generate_localnet(&opts, &mut BufWriter::new(Vec::new())).expect("generate localnet files");
        let genesis_path = temp.path().join("genesis.json");
        let manifest = genesis_json_from_path(&genesis_path);
        let params = genesis_parameters(&manifest);
        assert_eq!(params.sumeragi().block_cadence_ms().get(), 333);
        let npos = params
            .custom()
            .get(&SumeragiNposParameters::parameter_id())
            .and_then(SumeragiNposParameters::from_custom_parameter)
            .expect("npos parameters must be present");
        assert_eq!(npos.seat_band_pct(), 100);
        assert_eq!(npos.min_self_bond(), &Quantity::from(1_u64));
    }
    #[test]
    fn npos_localnet_keeps_genesis_under_transaction_cap() {
        let temp = tempfile::tempdir().expect("tmp dir");
        let opts = LocalnetOptions {
            sora_profile: None,
            perf_profile: None,
            peers: NonZeroU16::new(4).expect("non-zero"),
            seed: Some("npos-genesis-cap".to_owned()),
            bind_host: DEFAULT_BIND_HOST.to_owned(),
            public_host: DEFAULT_PUBLIC_HOST.to_owned(),
            base_api_port: 38280,
            base_p2p_port: 38537,
            out_dir: temp.path().to_path_buf(),
            extra_accounts: 0,
            assets: Vec::new(),
            block_cadence_ms: Some(333),
            consensus_mode: SumeragiConsensusMode::Npos,
        };
        generate_localnet(&opts, &mut BufWriter::new(Vec::new())).expect("generate localnet files");
        let genesis_path = temp.path().join("genesis.json");
        let transactions = RawGenesisTransaction::from_path(&genesis_path)
            .expect("parse generated genesis")
            .normalize()
            .expect("normalize generated genesis")
            .transactions;
        assert!(
            transactions.len() <= 16,
            "localnet genesis must stay within the block validation transaction cap"
        );
    }
    #[test]
    fn client_config_is_written_and_parsable() {
        let tmp = tempfile::tempdir().expect("tmp dir");
        let host =
            CanonicalHost::parse(DEFAULT_PUBLIC_HOST, "--public-host").expect("canonicalize host");
        write_client_config(
            tmp.path(),
            8080,
            &host,
            DEFAULT_CHAIN_ID,
            HashOf::from_untyped_unchecked(Hash::prehashed([0x42; Hash::LENGTH])),
            None,
            &localnet_client_identity(None, false).expect("default client"),
        )
        .expect("write client config");
        let contents =
            fs::read_to_string(tmp.path().join("client.toml")).expect("read client config");
        assert!(contents.contains("private_key = \"802620"));
        let value: toml::Value = toml::from_str(&contents).expect("parse client config");
        let network_id = value
            .get("network_id")
            .and_then(toml::Value::as_str)
            .expect("network id")
            .parse::<NetworkId>()
            .expect("canonical network id");
        assert_eq!(network_id.as_bytes(), &[0x42; Hash::LENGTH]);
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
        assert!(
            !account.contains_key("chain_discriminant"),
            "default localnet client config should not force an I105 prefix"
        );
    }
    #[test]
    fn client_config_records_chain_discriminant_when_known() {
        let tmp = tempfile::tempdir().expect("tmp dir");
        let host =
            CanonicalHost::parse(DEFAULT_PUBLIC_HOST, "--public-host").expect("canonicalize host");
        write_client_config(
            tmp.path(),
            8080,
            &host,
            DEFAULT_CHAIN_ID,
            HashOf::from_untyped_unchecked(Hash::prehashed([0x42; Hash::LENGTH])),
            Some(369),
            &localnet_client_identity(None, false).expect("default client"),
        )
        .expect("write client config");
        let contents =
            fs::read_to_string(tmp.path().join("client.toml")).expect("read client config");
        let value: toml::Value = toml::from_str(&contents).expect("parse client config");
        let account = value
            .get("account")
            .and_then(toml::Value::as_table)
            .expect("account table");
        assert_eq!(
            account
                .get("chain_discriminant")
                .and_then(toml::Value::as_integer),
            Some(369)
        );
    }
    #[test]
    fn generated_permissioned_localnet_grants_operator_exact_fee_asset_mint_permission() {
        let temp = tempfile::tempdir().expect("make temp dir");
        let opts = LocalnetOptions {
            sora_profile: None,
            perf_profile: None,
            peers: NonZeroU16::new(4).expect("non-zero"),
            seed: Some("permissioned-faucet-mint-permission".to_owned()),
            bind_host: DEFAULT_PUBLIC_HOST.to_owned(),
            public_host: DEFAULT_PUBLIC_HOST.to_owned(),
            base_api_port: 29080,
            base_p2p_port: 33337,
            out_dir: temp.path().to_path_buf(),
            extra_accounts: 0,
            assets: Vec::new(),
            block_cadence_ms: None,
            consensus_mode: SumeragiConsensusMode::Permissioned,
        };
        generate_localnet(&opts, &mut BufWriter::new(Vec::new())).expect("generate localnet");
        let operator =
            localnet_ephemeral_identity(opts.seed.as_deref().map(str::as_bytes), b"operator-root")
                .expect("derive generated operator");
        let expected_permission = CanMintAssetWithDefinition {
            asset_definition: localnet_fee_asset_definition_id(),
        };
        let manifest = RawGenesisTransaction::from_path(temp.path().join("genesis.json"))
            .expect("parse generated genesis");
        let operator_mint_permissions = manifest
            .instructions()
            .filter_map(|instruction| instruction.as_any().downcast_ref::<GrantBox>())
            .filter_map(|grant| match grant {
                GrantBox::Permission(grant) if grant.destination() == &operator.account_id => {
                    CanMintAssetWithDefinition::try_from(grant.object()).ok()
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(operator_mint_permissions, vec![expected_permission]);
    }
    #[test]
    fn generated_nexus_localnet_mints_fee_asset_to_client_signer() {
        let temp = tempfile::tempdir().expect("make temp dir");
        let opts = LocalnetOptions {
            sora_profile: Some(SoraProfile::Nexus),
            perf_profile: None,
            peers: NonZeroU16::new(4).expect("non-zero"),
            seed: Some("Iroha".to_owned()),
            bind_host: DEFAULT_PUBLIC_HOST.to_owned(),
            public_host: DEFAULT_PUBLIC_HOST.to_owned(),
            base_api_port: 29080,
            base_p2p_port: 33337,
            out_dir: temp.path().to_path_buf(),
            extra_accounts: 0,
            assets: Vec::new(),
            block_cadence_ms: None,
            consensus_mode: SumeragiConsensusMode::Npos,
        };
        generate_localnet(&opts, &mut BufWriter::new(Vec::new())).expect("generate localnet");
        let contents =
            fs::read_to_string(temp.path().join("genesis.json")).expect("read generated genesis");
        let client_account_id = account_id_runtime_literal(
            &localnet_client_account_id(),
            /* chain_discriminant */ None,
        );
        let expected_mint = format!(
            "\"destination\": \"{}#{}\"",
            localnet_fee_asset_literal(),
            client_account_id
        );
        assert!(
            contents.contains(&expected_mint),
            "generated genesis should fund the client signer fee asset"
        );
        assert!(
            contents.contains(&LOCALNET_FAUCET_AUTHORITY_BALANCE.to_string()),
            "generated genesis should give the reused local faucet signer enough XOR for repeated claims"
        );
    }
    #[test]
    fn npos_localnet_seeds_exact_onboarding_fee_sponsor_program() {
        let opts = LocalnetOptions {
            sora_profile: Some(SoraProfile::Nexus),
            perf_profile: None,
            peers: NonZeroU16::new(4).expect("non-zero"),
            seed: Some("typed-fee-sponsor".to_owned()),
            bind_host: DEFAULT_PUBLIC_HOST.to_owned(),
            public_host: DEFAULT_PUBLIC_HOST.to_owned(),
            base_api_port: 29_080,
            base_p2p_port: 33_337,
            out_dir: PathBuf::from("unused"),
            extra_accounts: 0,
            assets: Vec::new(),
            block_cadence_ms: None,
            consensus_mode: SumeragiConsensusMode::Npos,
        };
        let (genesis_public_key, _) =
            generate_genesis_key_pair(opts.seed.as_deref().map(str::as_bytes), GENESIS_SEED)
                .expect("derive expected genesis sponsor");
        let expected_program = localnet_fee_sponsor_program_id(&AccountId::new(genesis_public_key));
        let expected_client = localnet_client_account_id();
        let manifest = localnet_genesis_for_opts(&opts);
        let created = manifest
            .instructions()
            .filter_map(|instruction| {
                instruction
                    .as_any()
                    .downcast_ref::<CreateFeeSponsorProgram>()
            })
            .collect::<Vec<_>>();
        assert_eq!(created.len(), 1);
        assert_eq!(created[0].program().id, expected_program);
        let staged = manifest
            .instructions()
            .filter_map(|instruction| {
                instruction
                    .as_any()
                    .downcast_ref::<StageFeeSponsorProgramRevision>()
            })
            .collect::<Vec<_>>();
        assert_eq!(staged.len(), 1);
        assert_eq!(staged[0].revision().program_id, expected_program);
        assert_eq!(staged[0].revision().revision, 1);
        staged[0]
            .revision()
            .validate()
            .expect("localnet sponsor revision must validate");
        let enrollment = manifest
            .instructions()
            .filter_map(|instruction| {
                instruction
                    .as_any()
                    .downcast_ref::<EnrollFeeSponsorBeneficiary>()
            })
            .find(|enrollment| enrollment.program_id() == &expected_program)
            .expect("client enrollment for exact localnet program");
        assert_eq!(enrollment.beneficiary(), &expected_client);
        let has_exact_permission = manifest
            .instructions()
            .filter_map(|instruction| instruction.as_any().downcast_ref::<GrantBox>())
            .filter_map(|grant| match grant {
                GrantBox::Permission(grant) if grant.destination() == &expected_client => {
                    CanEnrollFeeSponsorProgram::try_from(grant.object()).ok()
                }
                _ => None,
            })
            .any(|permission| permission.program_id == expected_program);
        assert!(has_exact_permission);
    }
    #[test]
    fn generated_nexus_localnet_serves_xor_faucet_from_client_signer() {
        let temp = tempfile::tempdir().expect("make temp dir");
        let opts = LocalnetOptions {
            sora_profile: Some(SoraProfile::Nexus),
            perf_profile: None,
            peers: NonZeroU16::new(4).expect("non-zero"),
            seed: Some("localnet-faucet-config".to_owned()),
            bind_host: DEFAULT_PUBLIC_HOST.to_owned(),
            public_host: DEFAULT_PUBLIC_HOST.to_owned(),
            base_api_port: 29080,
            base_p2p_port: 33337,
            out_dir: temp.path().to_path_buf(),
            extra_accounts: 0,
            assets: Vec::new(),
            block_cadence_ms: None,
            consensus_mode: SumeragiConsensusMode::Npos,
        };
        generate_localnet(&opts, &mut BufWriter::new(Vec::new())).expect("generate localnet");
        let peer_cfg: toml::Value = toml::from_str(
            &fs::read_to_string(temp.path().join("peer0.toml")).expect("read peer config"),
        )
        .expect("parse peer config");
        let faucet = peer_cfg
            .get("torii")
            .and_then(toml::Value::as_table)
            .and_then(|torii| torii.get("faucet"))
            .and_then(toml::Value::as_table)
            .expect("torii faucet table");
        assert_eq!(
            faucet.get("enabled").and_then(toml::Value::as_bool),
            Some(true)
        );
        let expected_authority = localnet_client_account_literal(None);
        let expected_fee_asset = localnet_fee_asset_literal();
        assert_eq!(
            faucet.get("authority").and_then(toml::Value::as_str),
            Some(expected_authority.as_str())
        );
        assert!(faucet.get("private_key").is_none());
        assert_eq!(
            faucet
                .get("private_key_file")
                .and_then(toml::Value::as_str)
                .map(PathBuf::from),
            Some(
                temp.path()
                    .canonicalize()
                    .expect("canonical localnet root")
                    .join(LOCALNET_RUNTIME_DIRECTORY)
                    .join(LOCALNET_OPERATOR_SIGNER_KEY_FILE)
            )
        );
        assert_eq!(
            faucet
                .get("asset_definition_id")
                .and_then(toml::Value::as_str),
            Some(expected_fee_asset.as_str())
        );
        assert_eq!(
            faucet.get("amount").and_then(toml::Value::as_str),
            Some(LOCALNET_FAUCET_AMOUNT)
        );
        assert_eq!(
            faucet
                .get("pow_vrf_seed_enabled")
                .and_then(toml::Value::as_bool),
            Some(false)
        );
    }
    #[test]
    #[allow(clippy::too_many_lines)]
    fn generated_nexus_localnet_keeps_fee_asset_convertible_for_taira_wallets() {
        let temp = tempfile::tempdir().expect("make temp dir");
        let opts = LocalnetOptions {
            sora_profile: Some(SoraProfile::Nexus),
            perf_profile: None,
            peers: NonZeroU16::new(4).expect("non-zero"),
            seed: Some("localnet-fee-asset-convertible".to_owned()),
            bind_host: DEFAULT_PUBLIC_HOST.to_owned(),
            public_host: DEFAULT_PUBLIC_HOST.to_owned(),
            base_api_port: 29080,
            base_p2p_port: 33337,
            out_dir: temp.path().to_path_buf(),
            extra_accounts: 0,
            assets: Vec::new(),
            block_cadence_ms: None,
            consensus_mode: SumeragiConsensusMode::Npos,
        };
        generate_localnet(&opts, &mut BufWriter::new(Vec::new())).expect("generate localnet");
        let peer_cfg: toml::Value = toml::from_str(
            &fs::read_to_string(temp.path().join("peer0.toml")).expect("read peer config"),
        )
        .expect("parse peer config");
        assert_eq!(
            peer_cfg
                .get("zk")
                .and_then(toml::Value::as_table)
                .and_then(|zk| zk.get("halo2"))
                .and_then(toml::Value::as_table)
                .and_then(|halo2| halo2.get("enabled"))
                .and_then(toml::Value::as_bool),
            Some(true),
            "generated TAIRA configs must enable Halo2 verification for shielded sends"
        );
        let manifest = genesis_json_from_path(&temp.path().join("genesis.json"));
        let raw_genesis = RawGenesisTransaction::from_path(temp.path().join("genesis.json"))
            .expect("parse genesis");
        let fee_asset_id = localnet_fee_asset_literal();
        let fee_asset = manifest
            .get("transactions")
            .and_then(json::Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|tx| tx.get("instructions").and_then(json::Value::as_array))
            .flatten()
            .find_map(|instruction| {
                let asset = instruction
                    .get("Register")
                    .and_then(|register| register.get("AssetDefinition"))?;
                (asset.get("id").and_then(json::Value::as_str) == Some(fee_asset_id.as_str()))
                    .then_some(asset)
            })
            .expect("fee asset definition registration");
        assert_eq!(
            fee_asset.get("name").and_then(json::Value::as_str),
            Some("XOR"),
            "generated fee asset should surface as XOR in TAIRA UIs"
        );
        assert_eq!(
            fee_asset
                .get("spec")
                .and_then(|spec| spec.get("scale"))
                .and_then(json::Value::as_u64),
            Some(u64::from(LOCALNET_FEE_ASSET_SCALE)),
            "generated fee asset must use nano-XOR scale for fees and SNS charges"
        );
        assert!(
            fee_asset.get("confidential_policy").is_none(),
            "asset registration must not bypass canonical confidential verifier activation"
        );
        let unshield_vk_id = localnet_fee_vk_unshield_id();
        let zk_registration = raw_genesis
            .instructions()
            .find_map(|instruction| {
                instruction
                    .as_any()
                    .downcast_ref::<iroha_data_model::isi::zk::RegisterZkAsset>()
            })
            .expect("generated fee asset must emit a RegisterZkAsset instruction");
        assert!(
            zk_registration.asset() == &localnet_fee_asset_definition_id(),
            "generated fee asset must emit a RegisterZkAsset instruction for shield flows"
        );
        assert_eq!(
            zk_registration.vk_unshield(),
            &Some(unshield_vk_id.clone()),
            "generated fee asset must advertise an unshield verifier for withdrawals"
        );
        let vk_registrations = raw_genesis
            .instructions()
            .filter_map(|instruction| {
                instruction
                    .as_any()
                    .downcast_ref::<verifying_keys::RegisterVerifyingKey>()
            })
            .collect::<Vec<_>>();
        assert!(
            vk_registrations.iter().any(|register| {
                register.id == unshield_vk_id
                    && register.record.is_active()
                    && register.record.key.is_some()
                    && register.record.max_proof_bytes > 0
                    && register.record.circuit_id
                        == confidential_v2::CONFIDENTIAL_UNSHIELD_V2_CIRCUIT_ID
            }),
            "generated fee asset must register an active confidential unshield verifier"
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
        write_client_config(
            tmp.path(),
            8080,
            &host,
            DEFAULT_CHAIN_ID,
            HashOf::from_untyped_unchecked(Hash::prehashed([0x42; Hash::LENGTH])),
            None,
            &localnet_client_identity(None, false).expect("default client"),
        )
        .expect("write client config");
        let contents =
            fs::read_to_string(tmp.path().join("client.toml")).expect("read client config");
        assert!(contents.contains("torii_url = \"http://[::1]:8080/\""));
    }
    #[test]
    fn localnet_readme_records_only_base_seed_fingerprint_when_present() {
        let tmp = tempfile::tempdir().expect("tmp dir");
        let runtime_bundle = LocalnetRuntimeBundle {
            operator_signer_key: tmp.path().join(LOCALNET_OPERATOR_SIGNER_KEY_FILE),
            onboarding_signer_key: tmp.path().join(LOCALNET_ONBOARDING_SIGNER_KEY_FILE),
            onboarding_token_file: tmp.path().join(LOCALNET_ONBOARDING_TOKEN_FILE),
            onboarding_token_hash: [0; 32],
        };
        write_localnet_readme(
            tmp.path(),
            DEFAULT_CHAIN_ID,
            Some("Iroha"),
            false,
            SumeragiConsensusMode::Npos,
            4,
            "http://127.0.0.1:29080/",
            &tmp.path().join("genesis.json"),
            &tmp.path().join("genesis.signed.nrt"),
            &tmp.path().join(GENESIS_EXPECTED_HASH_FILE),
            &tmp.path().join(GENESIS_PUBLIC_KEY_FILE),
            &tmp.path().join(GENESIS_PRIVATE_KEY_FILE),
            &tmp.path().join("client.toml"),
            &tmp.path().join("start.sh"),
            &tmp.path().join("stop.sh"),
            &localnet_client_account_literal(None),
            &localnet_client_account_literal(None),
            &runtime_bundle,
            &tmp.path().join(LOCALNET_ALIAS_SETUP_INTENT_FILE),
        )
        .expect("write readme");
        let contents = fs::read_to_string(tmp.path().join("README.md")).expect("read readme");
        let fingerprint = blake3::hash(b"Iroha").to_hex();
        assert!(contents.contains(&format!("- Base seed BLAKE3 fingerprint: `{fingerprint}`")));
        assert!(!contents.contains("- Base seed: `Iroha`"));
        assert!(!contents.contains("`Iroha`"));
        assert!(contents.contains(LOCALNET_KAGEMUSHA_ASSET_ALIAS));
        assert!(contents.contains("genesis.expected_hash"));
        assert!(contents.contains("`kagami docker` without `--seed`"));
        assert!(!contents.contains("IROHA_GENESIS_SIGNED_FILE"));
        assert!(!contents.contains("IROHA_GENESIS_EXPECTED_HASH_FILE"));
        assert!(!contents.contains("IROHA_GENESIS_PRIVATE_KEY_FILE"));
        assert!(contents.contains("- Ephemeral operator authority: `"));
        assert!(contents.contains("- Ephemeral onboarding authority: `"));
        assert!(
            contents.contains(
                "- Offline escrow account: deterministic account derived from the exact genesis network id and asset definition"
            )
        );
        assert!(!contents.contains("Localnet app authority / escrow account"));
    }
    #[test]
    fn private_custody_readme_invokes_lifecycle_scripts_through_bash() {
        let tmp = tempfile::tempdir().expect("tmp dir");
        let runtime_bundle = LocalnetRuntimeBundle {
            operator_signer_key: tmp.path().join(LOCALNET_OPERATOR_SIGNER_KEY_FILE),
            onboarding_signer_key: tmp.path().join(LOCALNET_ONBOARDING_SIGNER_KEY_FILE),
            onboarding_token_file: tmp.path().join(LOCALNET_ONBOARDING_TOKEN_FILE),
            onboarding_token_hash: [0; 32],
        };
        write_localnet_readme(
            tmp.path(),
            DEFAULT_CHAIN_ID,
            None,
            true,
            SumeragiConsensusMode::Npos,
            4,
            "http://127.0.0.1:29080/",
            &tmp.path().join("genesis.json"),
            &tmp.path().join("genesis.signed.nrt"),
            &tmp.path().join(GENESIS_EXPECTED_HASH_FILE),
            &tmp.path().join(GENESIS_PUBLIC_KEY_FILE),
            &tmp.path().join(GENESIS_PRIVATE_KEY_FILE),
            &tmp.path().join("client.toml"),
            &tmp.path().join("start.sh"),
            &tmp.path().join("stop.sh"),
            &localnet_client_account_literal(None),
            &localnet_client_account_literal(None),
            &runtime_bundle,
            &tmp.path().join(LOCALNET_ALIAS_SETUP_INTENT_FILE),
        )
        .expect("write private-custody readme");
        let contents = fs::read_to_string(tmp.path().join("README.md")).expect("read readme");
        assert!(contents.lines().any(|line| line == "bash ./start.sh"));
        assert!(contents.lines().any(|line| line == "bash ./stop.sh"));
        assert!(!contents.lines().any(|line| line == "sh ./start.sh"));
        assert!(!contents.lines().any(|line| line == "sh ./stop.sh"));
        assert!(!contents.lines().any(|line| line == "./start.sh"));
        assert!(!contents.lines().any(|line| line == "./stop.sh"));
    }
    #[test]
    fn start_script_resolves_irohad_bin_before_entering_peer_directories() {
        for sora_profile_enabled in [false, true] {
            let tmp = tempfile::tempdir().expect("tmp dir");
            let start = tmp.path().join("start.sh");
            write_start_script(
                &start,
                4,
                sora_profile_enabled,
                "operator",
                "gas",
            )
            .expect("write start script");
            let contents = fs::read_to_string(&start).expect("read start script");

            // A relative IROHAD_BIN must be made absolute while the CWD is
            // still the net dir, or the per-peer cd below breaks it.
            let reassembly = contents
                .find("IROHAD_BIN=\"$IROHAD_BIN_DIR/$(basename -- \"$IROHAD_BIN_RESOLVED\")\"")
                .expect("absolute IROHAD_BIN reassembly present");
            let first_peer_cd = contents
                .find("cd \"$DIR/state/peer${i}\"")
                .expect("per-peer working directory present");
            assert!(reassembly < first_peer_cd);

            // Both launch paths enter the peer's own working directory.
            assert!(contents.contains(
                "peer_pid=$(mkdir -p \"$DIR/state/peer${i}\" && cd \"$DIR/state/peer${i}\" && "
            ));
            assert!(contents.contains(
                "    (\n      mkdir -p \"$DIR/state/peer${i}\" && cd \"$DIR/state/peer${i}\"\n      exec nohup env SNAPSHOT_STORE_DIR="
            ));
            assert!(contents.contains(") > \"$DIR/peer${i}.log\" 2>&1 &"));

            let syntax = std::process::Command::new("bash")
                .arg("-n")
                .arg(&start)
                .status()
                .expect("run bash -n");
            assert!(syntax.success(), "generated start.sh must parse");
        }
    }
    #[test]
    fn fresh_localnet_seed_uses_independent_256_bit_os_entropy() {
        let first = fresh_localnet_seed().expect("first OS-random seed");
        let second = fresh_localnet_seed().expect("second OS-random seed");
        assert_eq!(first.len(), 64);
        assert_eq!(second.len(), 64);
        assert!(first.bytes().all(|byte| byte.is_ascii_hexdigit()));
        assert!(second.bytes().all(|byte| byte.is_ascii_hexdigit()));
        assert_ne!(first, second);
        let peer_index = 0_u16.to_be_bytes();
        let (first_public, first_private) =
            generate_streaming_identity_key_pair(Some(first.as_bytes()), &peer_index)
                .expect("derive first fresh streaming identity");
        let (second_public, second_private) =
            generate_streaming_identity_key_pair(Some(second.as_bytes()), &peer_index)
                .expect("derive second fresh streaming identity");
        assert_ne!(first_public, second_public);
        assert_ne!(first_private.to_string(), second_private.to_string());
    }
    #[test]
    fn onboarding_tokens_remain_random_with_reproducible_identity_keys() {
        let operator = localnet_ephemeral_identity(Some(b"fixed-localnet-seed"), b"operator-root")
            .expect("derive operator identity");
        let onboarding =
            localnet_ephemeral_identity(Some(b"fixed-localnet-seed"), b"onboarding-root")
                .expect("derive onboarding identity");
        let first = tempfile::tempdir().expect("first runtime parent");
        let second = tempfile::tempdir().expect("second runtime parent");
        let first_bundle = write_localnet_runtime_bundle(first.path(), &operator, &onboarding)
            .expect("write first runtime bundle");
        let second_bundle = write_localnet_runtime_bundle(second.path(), &operator, &onboarding)
            .expect("write second runtime bundle");
        assert_ne!(
            first_bundle.onboarding_token_hash, second_bundle.onboarding_token_hash,
            "a reproducible key seed must never make API tokens reproducible"
        );
        let first_token = fs::read_to_string(first_bundle.onboarding_token_file)
            .expect("read first onboarding token");
        let second_token = fs::read_to_string(second_bundle.onboarding_token_file)
            .expect("read second onboarding token");
        assert_ne!(first_token, second_token);
    }
    fn mandatory_da_localnet_options(out_dir: PathBuf) -> LocalnetOptions {
        LocalnetOptions {
            sora_profile: None,
            perf_profile: None,
            peers: NonZeroU16::new(4).expect("non-zero"),
            seed: Some("mandatory-da".to_owned()),
            bind_host: DEFAULT_BIND_HOST.to_owned(),
            public_host: DEFAULT_PUBLIC_HOST.to_owned(),
            base_api_port: 19090,
            base_p2p_port: 23347,
            out_dir,
            extra_accounts: 0,
            assets: Vec::new(),
            block_cadence_ms: None,
            consensus_mode: SumeragiConsensusMode::Npos,
        }
    }
    fn assert_da_is_protocol_invariant_not_configuration() {
        let temp = tempfile::tempdir().expect("tmp dir");
        let opts = mandatory_da_localnet_options(temp.path().to_path_buf());
        generate_localnet(&opts, &mut BufWriter::new(Vec::new())).expect("generate localnet files");
        let peer_cfg: toml::Value = toml::from_str(
            &fs::read_to_string(temp.path().join("peer0.toml"))
                .expect("read generated peer config"),
        )
        .expect("parse peer config");
        let sumeragi = peer_cfg
            .get("sumeragi")
            .and_then(toml::Value::as_table)
            .expect("sumeragi table");
        for retired in ["da", "collectors", "rbc"] {
            assert!(
                !sumeragi.contains_key(retired),
                "node-local v2 config must not contain sumeragi.{retired}"
            );
        }
        let manifest_json = fs::read_to_string(temp.path().join("genesis.json"))
            .expect("read generated genesis manifest");
        assert!(!manifest_json.contains("da_enabled"));
        assert!(!manifest_json.contains("collectors_k"));
    }
    #[test]
    fn localnet_omits_retired_da_configuration() {
        assert_da_is_protocol_invariant_not_configuration();
    }
    #[test]
    fn rejects_overflowing_port_ranges() {
        let opts = LocalnetOptions {
            sora_profile: None,
            perf_profile: None,
            peers: NonZeroU16::new(4).unwrap(),
            seed: None,
            bind_host: DEFAULT_BIND_HOST.to_string(),
            public_host: DEFAULT_PUBLIC_HOST.to_string(),
            base_api_port: u16::MAX,
            base_p2p_port: 10,
            out_dir: PathBuf::from("unused"),
            extra_accounts: 0,
            assets: Vec::new(),
            block_cadence_ms: None,
            consensus_mode: SumeragiConsensusMode::Npos,
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
            sora_profile: None,
            perf_profile: None,
            peers: NonZeroU16::new(4).unwrap(),
            seed: None,
            bind_host: DEFAULT_BIND_HOST.to_string(),
            public_host: DEFAULT_PUBLIC_HOST.to_string(),
            base_api_port: 1337,
            base_p2p_port: 1337,
            out_dir: PathBuf::from("unused"),
            extra_accounts: 0,
            assets: Vec::new(),
            block_cadence_ms: None,
            consensus_mode: SumeragiConsensusMode::Npos,
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
            sora_profile: None,
            perf_profile: None,
            peers: NonZeroU16::new(4).unwrap(),
            seed: None,
            bind_host: DEFAULT_BIND_HOST.to_string(),
            public_host: DEFAULT_PUBLIC_HOST.to_string(),
            base_api_port: 0,
            base_p2p_port: 1000,
            out_dir: PathBuf::from("unused"),
            extra_accounts: 0,
            assets: Vec::new(),
            block_cadence_ms: None,
            consensus_mode: SumeragiConsensusMode::Npos,
        };
        let mut sink = BufWriter::new(Vec::<u8>::new());
        let err = generate_localnet(&opts, &mut sink).expect_err("zero port should fail");
        assert!(
            err.to_string().contains("must be > 0"),
            "unexpected error: {err}"
        );
    }
    #[test]
    fn validate_localnet_options_rejects_zero_block_cadence() {
        let opts = LocalnetOptions {
            sora_profile: None,
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
            block_cadence_ms: Some(0),
            consensus_mode: SumeragiConsensusMode::Npos,
        };
        let err = validate_localnet_options(&opts).expect_err("zero block cadence should fail");
        assert!(
            err.to_string().contains("--block-cadence-ms"),
            "unexpected error: {err}"
        );
    }
    #[test]
    fn validate_localnet_options_rejects_roster_above_protocol_maximum() {
        let oversized =
            u16::try_from(MAX_VALIDATORS_PER_HEIGHT + 1).expect("protocol test boundary fits u16");
        let opts = LocalnetOptions {
            sora_profile: None,
            perf_profile: None,
            peers: NonZeroU16::new(oversized).expect("non-zero"),
            seed: None,
            bind_host: DEFAULT_BIND_HOST.to_string(),
            public_host: DEFAULT_PUBLIC_HOST.to_string(),
            base_api_port: 28_080,
            base_p2p_port: 28_337,
            out_dir: PathBuf::from("unused"),
            extra_accounts: 0,
            assets: Vec::new(),
            block_cadence_ms: None,
            consensus_mode: SumeragiConsensusMode::Npos,
        };
        let error = validate_localnet_options(&opts)
            .expect_err("the CLI must reject a roster above the wire-protocol limit");
        let expected = format!(
            "`--peers` ({oversized}) exceeds the Sumeragi v2 protocol maximum validator roster of {MAX_VALIDATORS_PER_HEIGHT}"
        );
        assert!(
            error.to_string().contains(&expected),
            "unexpected error: {error}"
        );
    }
    #[test]
    fn validate_localnet_options_rejects_non_three_f_plus_one_roster() {
        let opts = LocalnetOptions {
            sora_profile: None,
            perf_profile: None,
            peers: NonZeroU16::new(5).expect("non-zero"),
            seed: None,
            bind_host: DEFAULT_BIND_HOST.to_string(),
            public_host: DEFAULT_PUBLIC_HOST.to_string(),
            base_api_port: 28_080,
            base_p2p_port: 28_337,
            out_dir: PathBuf::from("unused"),
            extra_accounts: 0,
            assets: Vec::new(),
            block_cadence_ms: None,
            consensus_mode: SumeragiConsensusMode::Npos,
        };
        let error = validate_localnet_options(&opts)
            .expect_err("the CLI must reject a non-3f+1 validator roster");
        assert!(
            error.to_string().contains("exact Sumeragi v2 3f+1"),
            "unexpected error: {error}"
        );
    }
    #[test]
    fn validate_localnet_options_rejects_every_profile_with_too_few_peers() {
        let opts = LocalnetOptions {
            sora_profile: None,
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
            block_cadence_ms: None,
            consensus_mode: SumeragiConsensusMode::Npos,
        };
        let err = validate_localnet_options(&opts)
            .expect_err("every generated localnet should enforce the minimum peer count");
        assert!(
            err.to_string().contains("`--peers` must be at least 4"),
            "unexpected error: {err}"
        );
    }
    #[test]
    fn validate_localnet_options_rejects_permissioned_on_sora_nexus() {
        let opts = LocalnetOptions {
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
            block_cadence_ms: None,
            consensus_mode: SumeragiConsensusMode::Permissioned,
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
            block_cadence_ms: None,
            consensus_mode: SumeragiConsensusMode::Permissioned,
        };
        let err = validate_localnet_options(&opts).expect_err("sora profile should require NPoS");
        assert!(
            err.to_string().contains("sora-profile"),
            "unexpected error: {err}"
        );
    }
    #[test]
    fn validate_localnet_options_allows_permissioned_localnet() {
        let opts = LocalnetOptions {
            sora_profile: None,
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
            block_cadence_ms: None,
            consensus_mode: SumeragiConsensusMode::Permissioned,
        };
        validate_localnet_options(&opts).expect("permissioned localnet should be allowed");
    }
    #[test]
    fn permissioned_localnet_uses_mandatory_nexus_default() {
        let temp = tempfile::tempdir().expect("tmp dir");
        let opts = LocalnetOptions {
            sora_profile: None,
            perf_profile: None,
            peers: NonZeroU16::new(4).unwrap(),
            seed: Some("permissioned-localnet".to_owned()),
            bind_host: DEFAULT_BIND_HOST.to_string(),
            public_host: DEFAULT_PUBLIC_HOST.to_string(),
            base_api_port: 28080,
            base_p2p_port: 28337,
            out_dir: temp.path().to_path_buf(),
            extra_accounts: 0,
            assets: Vec::new(),
            block_cadence_ms: None,
            consensus_mode: SumeragiConsensusMode::Permissioned,
        };
        generate_localnet(&opts, &mut BufWriter::new(Vec::new()))
            .expect("generate permissioned localnet");
        let peer_cfg: toml::Value = toml::from_str(
            &fs::read_to_string(temp.path().join("peer0.toml"))
                .expect("read generated peer config"),
        )
        .expect("parse peer config");
        let nexus = peer_cfg
            .get("nexus")
            .and_then(toml::Value::as_table)
            .expect("nexus table");
        assert!(
            !nexus.contains_key("enabled"),
            "generated configs must not expose the retired Nexus availability switch"
        );
        assert!(
            nexus.get("storage").is_none(),
            "disabled nexus localnet should not emit a storage budget"
        );
    }
    #[test]
    #[allow(clippy::too_many_lines)] // End-to-end config assertions are kept together for this localnet scenario.
    fn npos_without_sora_profile_uses_mandatory_nexus() {
        let temp = tempfile::tempdir().expect("tmp dir");
        let opts = LocalnetOptions {
            sora_profile: None,
            perf_profile: None,
            peers: NonZeroU16::new(4).unwrap(),
            seed: Some("npos-localnet".to_owned()),
            bind_host: DEFAULT_BIND_HOST.to_string(),
            public_host: DEFAULT_PUBLIC_HOST.to_string(),
            base_api_port: 28080,
            base_p2p_port: 28337,
            out_dir: temp.path().to_path_buf(),
            extra_accounts: 0,
            assets: Vec::new(),
            block_cadence_ms: None,
            consensus_mode: SumeragiConsensusMode::Npos,
        };
        generate_localnet(&opts, &mut BufWriter::new(Vec::new())).expect("generate npos localnet");
        let seed_bytes = opts.seed.as_ref().map(String::as_bytes);
        let (genesis_public_key, _) = generate_genesis_key_pair(seed_bytes, GENESIS_SEED)
            .expect("test localnet genesis key generation should succeed");
        let gas_account_id = localnet_gas_account_id(&genesis_public_key)
            .expect("test localnet gas account derivation should succeed");
        let peer_cfg: toml::Value = toml::from_str(
            &fs::read_to_string(temp.path().join("peer0.toml"))
                .expect("read generated peer config"),
        )
        .expect("parse peer config");
        let nexus = peer_cfg
            .get("nexus")
            .and_then(toml::Value::as_table)
            .expect("nexus table");
        assert!(
            !nexus.contains_key("enabled"),
            "generated configs must rely on mandatory Nexus"
        );
        assert_eq!(
            nexus
                .get("storage")
                .and_then(toml::Value::as_table)
                .and_then(|storage| storage.get("local_budget_bytes"))
                .and_then(toml::Value::as_integer),
            Some(
                i64::try_from(LOCALNET_NEXUS_STORAGE_BUDGET_BYTES)
                    .expect("localnet Nexus storage budget fits i64")
            ),
            "npos iroha3 localnet should use an explicit disposable storage budget"
        );
        let gas_account_id = account_id_runtime_literal(&gas_account_id, None);
        let staking = nexus
            .get("staking")
            .and_then(toml::Value::as_table)
            .expect("nexus staking table");
        let expected_stake_asset_id = localnet_stake_asset_literal();
        let expected_fee_asset_id = localnet_fee_asset_literal();
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
            Some(expected_fee_asset_id.as_str())
        );
        assert_eq!(
            fees.get("base_fee").and_then(toml::Value::as_str),
            Some("0")
        );
        assert_eq!(
            fees.get("per_instruction_fee")
                .and_then(toml::Value::as_str),
            Some("0.001")
        );
        assert_eq!(
            fees.get("per_gas_unit_fee").and_then(toml::Value::as_str),
            Some("0.00005")
        );
        assert_eq!(
            fees.get("settlement_mode").and_then(toml::Value::as_str),
            Some("direct")
        );
        assert_eq!(
            fees.get("fee_sink_account_id")
                .and_then(toml::Value::as_str),
            Some(gas_account_id.as_str())
        );
        assert_eq!(
            fees.get("sponsor_vault_custody_account_id")
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
    include!("localnet/private_fee_and_account_tests.rs");
    #[test]
    fn account_id_runtime_literal_uses_encoded_literal() {
        let seed_bytes = Some(b"localnet-gas-runtime-literal".as_slice());
        let (genesis_public_key, _) = generate_genesis_key_pair(seed_bytes, GENESIS_SEED)
            .expect("test localnet genesis key generation should succeed");
        let gas_account_id = localnet_gas_account_id(&genesis_public_key)
            .expect("test localnet gas account derivation should succeed");
        let literal = account_id_runtime_literal(&gas_account_id, None);
        assert_eq!(literal, gas_account_id.to_string());
    }
    #[test]
    fn account_id_runtime_literal_respects_requested_chain_discriminant() {
        let seed_bytes = Some(b"localnet-gas-runtime-taira".as_slice());
        let (genesis_public_key, _) = generate_genesis_key_pair(seed_bytes, GENESIS_SEED)
            .expect("test localnet genesis key generation should succeed");
        let gas_account_id = localnet_gas_account_id(&genesis_public_key)
            .expect("test localnet gas account derivation should succeed");
        let literal = account_id_runtime_literal(&gas_account_id, Some(369));
        assert!(
            literal.starts_with("test"),
            "expected testnet i105 literal, got {literal}"
        );
        assert!(
            !literal.starts_with("sora"),
            "localnet runtime literal must not use mainnet prefix under Taira"
        );
    }
    #[test]
    fn default_sorafs_telemetry_submitters_match_self_service_policy() {
        let submitters =
            iroha_config::parameters::defaults::governance::sorafs_telemetry::submitters();
        assert!(
            submitters.is_empty(),
            "self-service telemetry should not pin default submitter accounts"
        );
    }
    #[test]
    fn localnet_npos_bootstrap_does_not_re_register_genesis_account() {
        let temp = tempfile::tempdir().expect("tmp dir");
        let opts = LocalnetOptions {
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
            block_cadence_ms: None,
            consensus_mode: SumeragiConsensusMode::Npos,
        };
        let manifest = localnet_genesis_for_opts(&opts);
        let seed_bytes = opts.seed.as_ref().map(String::as_bytes);
        let (genesis_public_key, _) = generate_genesis_key_pair(seed_bytes, GENESIS_SEED)
            .expect("test localnet genesis key generation should succeed");
        let genesis_account_id = AccountId::new(genesis_public_key.clone());
        let ivm_genesis_registrations = manifest
            .instructions()
            .filter_map(|instruction| instruction.as_any().downcast_ref::<Register<Account>>())
            .filter(|register| register.object.id == genesis_account_id)
            .count();
        assert_eq!(
            ivm_genesis_registrations, 0,
            "expected NPoS bootstrap to avoid re-registering the genesis controller under ivm"
        );
    }
    include!("localnet/path_and_script_tests.rs");
    #[cfg(unix)]
    #[test]
    fn start_script_includes_sora_flag_when_enabled() {
        let temp = tempfile::tempdir().expect("tmp dir");
        let client_account_literal = localnet_client_account_literal(None);
        let fee_asset_definition_id = localnet_fee_asset_literal();
        write_scripts(
            temp.path(),
            1,
            true,
            &client_account_literal,
            &fee_asset_definition_id,
        )
        .expect("write scripts");
        let start_contents =
            fs::read_to_string(temp.path().join("start.sh")).expect("read start script");
        assert!(
            start_contents.contains(" --sora --config "),
            "start script should include --sora when profile enabled"
        );
    }
    // Keep the generated rANS table contract tests in a focused child under `localnet::tests`.
    include!("localnet/rans_table_tests.rs");
    #[test]
    fn localnet_perf_profile_keeps_matching_consensus_mode() {
        let mode =
            resolve_requested_consensus_mode(None, Some(LocalnetPerfProfile::Throughput10kNpos));
        assert_eq!(mode, SumeragiConsensusMode::Npos);
    }
}
