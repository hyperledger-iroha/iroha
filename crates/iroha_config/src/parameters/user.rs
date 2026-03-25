//! User configuration view.
//!
//! Contains structures in a format that is convenient from the user perspective. It is less strict
//! and not necessarily valid upon successful parsing of the user-provided content.
#![allow(
    clippy::too_many_lines,
    clippy::or_fun_call,
    clippy::manual_assert,
    clippy::useless_let_if_seq,
    clippy::manual_clamp,
    clippy::absurd_extreme_comparisons,
    clippy::trivially_copy_pass_by_ref,
    clippy::struct_excessive_bools,
    clippy::assertions_on_constants,
    clippy::field_reassign_with_default
)]
//! It begins with [`Root`], containing sub-modules.

// This module's usage is documented in high detail in the Configuration Reference
// (`docs/source/references/configuration.md`).
#![allow(clippy::doc_markdown, clippy::doc_link_with_quotes)]
use std::{
    borrow::Cow,
    collections::{BTreeMap, BTreeSet},
    convert::{Infallible, TryFrom, TryInto},
    fmt::Debug,
    fs, io,
    num::{NonZeroU16, NonZeroU32, NonZeroU64, NonZeroUsize},
    path::{Path, PathBuf},
    str::FromStr,
    sync::{LazyLock, Mutex, MutexGuard},
    time::Duration,
};

use error_stack::{Report, ResultExt};
#[cfg(test)]
use iroha_config_base::ParameterId;
use iroha_config_base::{
    ParameterOrigin, ReadConfig, WithOrigin,
    attach::ConfigValueAndOrigin,
    env::FromEnvStr,
    read::{ConfigReader, FinalWrap, ReadConfig as ReadConfigTrait},
    util::{Bytes, DurationMs, Emitter, EmitterResultExt},
};
use iroha_data_model::{
    sorafs::capacity::ProviderId,
    soranet::vpn::{VpnExitClassV1, VpnFlowLabelV1},
};
use nonzero_ext::nonzero;
use rust_decimal::Decimal;
use thiserror::Error;

type Result<T, E> = core::result::Result<T, Report<[E]>>;
type KyberKeyInputs = (Vec<u8>, ParameterOrigin, Vec<u8>, ParameterOrigin);
const MIN_TIMER_INTERVAL: Duration = Duration::from_millis(100);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct ApiVersionLabel {
    major: u16,
    minor: u16,
}

impl ApiVersionLabel {
    fn parse(raw: &str) -> Option<Self> {
        let trimmed = raw.trim().trim_start_matches('v');
        let mut parts = trimmed.split('.');
        let major = parts.next()?.parse::<u16>().ok()?;
        let minor = parts.next().unwrap_or("0").parse::<u16>().ok()?;
        if parts.next().is_some() {
            return None;
        }
        Some(Self { major, minor })
    }

    fn render(&self) -> String {
        format!("{}.{}", self.major, self.minor)
    }
}

fn normalize_version_list(raw: Vec<String>, field: &str) -> Vec<ApiVersionLabel> {
    let mut versions = Vec::new();
    let mut invalid = Vec::new();
    for label in raw {
        match ApiVersionLabel::parse(&label) {
            Some(parsed) => versions.push(parsed),
            None => invalid.push(label),
        }
    }
    if !invalid.is_empty() {
        panic!(
            "invalid semantic version(s) in `{}`: {}; expected labels like `1.0` or `v1.1`",
            field,
            invalid.join(", ")
        );
    }
    if versions.is_empty() {
        panic!("`{field}` must contain at least one semantic version (major.minor)");
    }
    versions.sort();
    versions.dedup();
    versions
}

fn normalize_jdg_signature_schemes(raw: Vec<String>) -> BTreeSet<JdgSignatureScheme> {
    let mut schemes = BTreeSet::new();
    let mut invalid = Vec::new();
    for label in raw {
        match label.parse::<JdgSignatureScheme>() {
            Ok(scheme) => {
                schemes.insert(scheme);
            }
            Err(_) => invalid.push(label),
        }
    }
    if !invalid.is_empty() {
        panic!(
            "invalid governance.jdg_signature_schemes: {}; expected `simple_threshold` or `bls_normal_aggregate`",
            invalid.join(", ")
        );
    }
    if schemes.is_empty() {
        panic!("governance.jdg_signature_schemes must contain at least one scheme");
    }
    schemes
}

fn parse_account_id_literal(raw: &str, context: &str) -> AccountId {
    AccountId::parse_encoded(raw).map_or_else(
        |err| panic!("{context}: {err}"),
        iroha_data_model::account::ParsedAccountId::into_account_id,
    )
}

static ACCOUNT_ADDRESS_PARSE_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

enum KyberKeyConfig {
    Absent,
    Present(KyberKeyInputs),
}
use hex::FromHex;
use iroha_crypto::{
    Algorithm, ExposedPrivateKey, Hash, HashOf, KeyPair, PrivateKey, PublicKey,
    soranet::handshake::{
        DEFAULT_CLIENT_CAPABILITIES, DEFAULT_DESCRIPTOR_COMMIT, DEFAULT_RELAY_CAPABILITIES,
    },
    streaming::{KeyMaterialError, STREAMING_DEFAULT_KEM_SUITE, StreamingKeyMaterial},
};
use iroha_data_model::{
    ChainId, Level,
    account::{AccountId, curve::CurveId},
    asset::prelude::AssetDefinitionId,
    block::BlockHeader,
    compute::{
        ComputeAuthPolicy, ComputeFeeSplit, ComputePriceAmplifiers, ComputePriceDeltaBounds,
        ComputePriceRiskClass, ComputePriceWeights, ComputeResourceBudget, ComputeSandboxRules,
        ComputeSponsorPolicy,
    },
    content::ContentAuthMode,
    da::{
        commitment::DaProofScheme,
        prelude::DaStripeLayout,
        types::{BlobClass, DaRentPolicyV1, GovernanceTag, RetentionPolicy},
    },
    hijiri::{
        FeeMultiplierBand as ModelFeeMultiplierBand, FeePolicyError as ModelFeePolicyError,
        HijiriFeePolicy as ModelHijiriFeePolicy, Q16 as ModelQ16,
    },
    jurisdiction::JdgSignatureScheme,
    name::Name,
    nexus::{
        DataSpaceCatalog, DataSpaceId, DataSpaceMetadata, LaneCatalog, LaneConfig, LaneId,
        LaneStorageProfile, LaneVisibility, UniversalAccountId,
    },
    peer::{Peer, PeerId},
    role::RoleId,
    sorafs::{pin_registry::StorageClass as SorafsStorageClass, pricing::PricingScheduleRecord},
    taikai::TaikaiAvailabilityClass,
};
use iroha_primitives::{addr::SocketAddr, numeric::Numeric, unique_vec::UniqueVec};
use norito::{
    json::{self, JsonDeserialize, JsonSerialize, Map, Value},
    streaming::{BUNDLED_RANS_GPU_BUILD_AVAILABLE, EntropyMode, load_bundle_tables_from_toml},
};
use soranet_pq::{MlKemSuite, SuiteParseError};
use url::Url;

use crate::{
    kura::{FsyncMode as KuraFsyncMode, InitMode as KuraInitMode},
    logger::{Directives, Format as LoggerFormat},
    parameters::{actual, defaults},
    snapshot::Mode as SnapshotMode,
};

/// P2P relay role configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RelayMode {
    /// Relay disabled; full mesh expected.
    #[default]
    Disabled,
    /// Relay hub; accept spokes and forward traffic.
    Hub,
    /// Relay spoke; dial only the hub and rely on forwarding.
    Spoke,
    /// Relay assist; connect directly when possible but keep a hub connection for relay fallback.
    ///
    /// This is useful when only some peers must rely on a relay (e.g., behind NAT/firewalls),
    /// while others still form direct connections.
    Assist,
}

impl json::JsonSerialize for RelayMode {
    fn json_serialize(&self, out: &mut String) {
        let variant = match self {
            Self::Disabled => "disabled",
            Self::Hub => "hub",
            Self::Spoke => "spoke",
            Self::Assist => "assist",
        };
        json::write_json_string(variant, out);
    }
}

impl json::JsonDeserialize for RelayMode {
    fn json_deserialize(
        parser: &mut json::Parser<'_>,
    ) -> ::core::result::Result<Self, json::Error> {
        let text = parser.parse_string()?;
        match text.to_ascii_lowercase().as_str() {
            "disabled" => Ok(Self::Disabled),
            "hub" => Ok(Self::Hub),
            "spoke" => Ok(Self::Spoke),
            "assist" => Ok(Self::Assist),
            other => Err(json::Error::InvalidField {
                field: "relay_mode".into(),
                message: format!("expected disabled, hub, spoke, or assist, got {other}"),
            }),
        }
    }
}

// --- Move peer-related helper types upfront so they are visible to derives below ---

/// Trusted peers list (user view)
/// User-level configuration container.
#[derive(Debug, Clone)]
pub struct TrustedPeers(UniqueVec<Peer>);

// (moved to top of file to avoid derive resolution issues)

/// Wrapper around a list of algorithm identifiers allowing env parsing.
#[derive(Debug, Clone, Default)]
pub struct AlgorithmListConfig(pub Vec<String>);

impl AlgorithmListConfig {
    fn into_vec(self) -> Vec<String> {
        self.0
    }
}

impl From<AlgorithmListConfig> for Vec<String> {
    fn from(value: AlgorithmListConfig) -> Self {
        value.into_vec()
    }
}

impl From<Vec<String>> for AlgorithmListConfig {
    fn from(value: Vec<String>) -> Self {
        AlgorithmListConfig(value)
    }
}

impl json::JsonDeserialize for AlgorithmListConfig {
    fn json_deserialize(
        parser: &mut json::Parser<'_>,
    ) -> ::core::result::Result<Self, json::Error> {
        let list = Vec::<String>::json_deserialize(parser)?;
        Ok(AlgorithmListConfig(list))
    }
}

impl FromEnvStr for AlgorithmListConfig {
    type Error = Infallible;

    fn from_env_str(value: Cow<'_, str>) -> std::result::Result<Self, Self::Error> {
        let list = value
            .split(',')
            .map(str::trim)
            .filter(|item| !item.is_empty())
            .map(ToOwned::to_owned)
            .collect();
        Ok(AlgorithmListConfig(list))
    }
}

/// Wrapper for provider-scoped submitter allowlists parsed from env/config.
#[derive(Debug, Clone, Default)]
pub struct PerProviderSubmittersConfig(pub BTreeMap<String, AlgorithmListConfig>);

impl From<PerProviderSubmittersConfig> for BTreeMap<String, AlgorithmListConfig> {
    fn from(value: PerProviderSubmittersConfig) -> Self {
        value.0
    }
}

impl FromEnvStr for PerProviderSubmittersConfig {
    type Error = io::Error;

    fn from_env_str(value: Cow<'_, str>) -> std::result::Result<Self, Self::Error> {
        let mut map = BTreeMap::new();
        if value.is_empty() {
            return Ok(Self(map));
        }
        for entry in value.split(';') {
            let entry = entry.trim();
            if entry.is_empty() {
                continue;
            }
            let (provider, submitters) = entry.split_once('=').ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "expected provider=submitters pair",
                )
            })?;
            let parsed = AlgorithmListConfig::from_env_str(Cow::Owned(submitters.to_owned()))
                .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err.to_string()))?;
            map.insert(provider.trim().to_owned(), parsed);
        }
        Ok(Self(map))
    }
}

impl json::JsonDeserialize for PerProviderSubmittersConfig {
    fn json_deserialize(
        parser: &mut json::Parser<'_>,
    ) -> ::core::result::Result<Self, json::Error> {
        let inner = BTreeMap::<String, AlgorithmListConfig>::json_deserialize(parser)?;
        Ok(Self(inner))
    }
}

/// Boolean wrapper that accepts common truthy/falsy strings when parsing from env.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Boolish(pub bool);

/// Error returned when parsing a boolean-ish environment value fails.
#[derive(Debug, Error, PartialEq, Eq, Copy, Clone)]
#[error("provided string was not a recognised boolean (expected true/false/1/0/on/off/yes/no)")]
pub struct BoolishParseError;

impl From<bool> for Boolish {
    fn from(value: bool) -> Self {
        Self(value)
    }
}

impl From<Boolish> for bool {
    fn from(value: Boolish) -> Self {
        value.0
    }
}

impl FromEnvStr for Boolish {
    type Error = BoolishParseError;

    fn from_env_str(value: Cow<'_, str>) -> std::result::Result<Self, Self::Error> {
        let normalized = value.trim().to_ascii_lowercase();
        match normalized.as_str() {
            "true" | "1" | "yes" | "on" => Ok(Self(true)),
            "false" | "0" | "no" | "off" => Ok(Self(false)),
            _ => Err(BoolishParseError),
        }
    }
}

impl json::JsonSerialize for Boolish {
    fn json_serialize(&self, out: &mut String) {
        json::JsonSerialize::json_serialize(&self.0, out);
    }
}

impl json::JsonDeserialize for Boolish {
    fn json_deserialize(
        parser: &mut json::Parser<'_>,
    ) -> ::core::result::Result<Self, json::Error> {
        bool::json_deserialize(parser).map(Self)
    }
}

/// Wrapper around a list of curve identifiers allowing env parsing.
#[derive(Debug, Clone, Default)]
pub struct CurveIdListConfig(pub Vec<u8>);

impl CurveIdListConfig {
    fn into_vec(self) -> Vec<u8> {
        self.0
    }

    fn default_allowed() -> Self {
        Self(defaults::crypto::allowed_curve_ids())
    }
}

impl From<Vec<u8>> for CurveIdListConfig {
    fn from(value: Vec<u8>) -> Self {
        CurveIdListConfig(value)
    }
}

/// Intrinsic dispatch policy for SM acceleration (`auto`/`force-enable`/`force-disable`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SmIntrinsicsPolicyConfig {
    /// Let the runtime decide based on hardware support.
    #[default]
    Auto,
    /// Force-enable SM intrinsics when available.
    ForceEnable,
    /// Disable SM intrinsics even when the platform supports them.
    ForceDisable,
}

impl SmIntrinsicsPolicyConfig {}

#[derive(Debug, Error)]
#[error("invalid SM intrinsics policy `{0}` (expected auto, force-enable, or force-disable)")]
/// Error returned when an SM intrinsics policy string cannot be parsed.
pub struct SmIntrinsicsPolicyParseError(String);

impl FromStr for SmIntrinsicsPolicyConfig {
    type Err = SmIntrinsicsPolicyParseError;

    fn from_str(raw: &str) -> core::result::Result<Self, Self::Err> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "auto" => Ok(Self::Auto),
            "force-enable" | "force_enable" | "enable" | "on" | "true" | "1" => {
                Ok(Self::ForceEnable)
            }
            "force-disable" | "force_disable" | "disable" | "off" | "false" | "0" => {
                Ok(Self::ForceDisable)
            }
            other => Err(SmIntrinsicsPolicyParseError(other.to_owned())),
        }
    }
}

impl From<&str> for SmIntrinsicsPolicyConfig {
    fn from(value: &str) -> Self {
        value.parse().unwrap_or_default()
    }
}

impl json::JsonDeserialize for SmIntrinsicsPolicyConfig {
    fn json_deserialize(
        parser: &mut json::Parser<'_>,
    ) -> ::core::result::Result<Self, json::Error> {
        let value = parser.parse_string()?;
        value.parse().map_err(|_| json::Error::InvalidField {
            field: "crypto.sm_intrinsics".into(),
            message: format!("expected auto, force-enable, or force-disable, got {value}"),
        })
    }
}

impl From<SmIntrinsicsPolicyConfig> for actual::SmIntrinsicsPolicy {
    fn from(policy: SmIntrinsicsPolicyConfig) -> Self {
        match policy {
            SmIntrinsicsPolicyConfig::Auto => Self::Auto,
            SmIntrinsicsPolicyConfig::ForceEnable => Self::ForceEnable,
            SmIntrinsicsPolicyConfig::ForceDisable => Self::ForceDisable,
        }
    }
}

#[cfg(test)]
mod sm_intrinsics_policy_config_tests {
    use std::str::FromStr;

    use super::SmIntrinsicsPolicyConfig;

    #[test]
    fn parses_force_disable_and_enable() {
        assert!(matches!(
            SmIntrinsicsPolicyConfig::from_str("force-disable"),
            Ok(SmIntrinsicsPolicyConfig::ForceDisable)
        ));
        assert!(matches!(
            SmIntrinsicsPolicyConfig::from_str("force-enable"),
            Ok(SmIntrinsicsPolicyConfig::ForceEnable)
        ));
    }

    #[test]
    fn rejects_unknown_policy() {
        assert!(SmIntrinsicsPolicyConfig::from_str("neon-only").is_err());
    }
}

impl From<CurveIdListConfig> for Vec<u8> {
    fn from(value: CurveIdListConfig) -> Self {
        value.into_vec()
    }
}

impl json::JsonDeserialize for CurveIdListConfig {
    fn json_deserialize(
        parser: &mut json::Parser<'_>,
    ) -> ::core::result::Result<Self, json::Error> {
        let list = Vec::<u8>::json_deserialize(parser)?;
        Ok(CurveIdListConfig(list))
    }
}

impl FromEnvStr for CurveIdListConfig {
    type Error = core::num::ParseIntError;

    fn from_env_str(value: Cow<'_, str>) -> std::result::Result<Self, Self::Error> {
        let mut list = Vec::new();
        for entry in value.split(',') {
            let trimmed = entry.trim();
            if trimmed.is_empty() {
                continue;
            }
            let parsed = trimmed.parse::<u8>()?;
            list.push(parsed);
        }
        Ok(CurveIdListConfig(list))
    }
}

/// Hex-encoded byte vector helper used for capability vectors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HexBytes(pub Vec<u8>);

impl From<Vec<u8>> for HexBytes {
    fn from(value: Vec<u8>) -> Self {
        Self(value)
    }
}

impl From<HexBytes> for Vec<u8> {
    fn from(value: HexBytes) -> Self {
        value.0
    }
}

impl JsonSerialize for HexBytes {
    fn json_serialize(&self, out: &mut String) {
        let encoded = hex::encode(&self.0);
        json::write_json_string(&encoded, out);
    }
}

impl JsonDeserialize for HexBytes {
    fn json_deserialize(parser: &mut json::Parser<'_>) -> core::result::Result<Self, json::Error> {
        let encoded = parser.parse_string()?;
        hex::decode(encoded.as_bytes())
            .map(HexBytes)
            .map_err(|err| json::Error::Message(err.to_string()))
    }
}

impl FromEnvStr for HexBytes {
    type Error = hex::FromHexError;

    fn from_env_str(value: Cow<'_, str>) -> std::result::Result<Self, Self::Error> {
        Vec::from_hex(value.as_ref()).map(HexBytes)
    }
}

/// Wrapper for ML-KEM suite strings parsed from configuration sources.
#[derive(Debug, Clone, Copy)]
pub struct MlKemSuiteParam(pub MlKemSuite);

impl MlKemSuiteParam {
    fn into_suite(self) -> MlKemSuite {
        self.0
    }
}

impl From<MlKemSuiteParam> for MlKemSuite {
    fn from(value: MlKemSuiteParam) -> Self {
        value.0
    }
}

impl From<MlKemSuite> for MlKemSuiteParam {
    fn from(value: MlKemSuite) -> Self {
        Self(value)
    }
}

impl JsonSerialize for MlKemSuiteParam {
    fn json_serialize(&self, out: &mut String) {
        json::write_json_string(&self.0.to_string(), out);
    }
}

impl JsonDeserialize for MlKemSuiteParam {
    fn json_deserialize(parser: &mut json::Parser<'_>) -> core::result::Result<Self, json::Error> {
        let raw = parser.parse_string()?;
        match MlKemSuite::from_str(raw.as_str()) {
            Ok(suite) => Ok(Self(suite)),
            Err(SuiteParseError(invalid)) => Err(json::Error::Message(format!(
                "unsupported mlkem suite `{invalid}`; expected {}",
                MlKemSuite::ALL
                    .iter()
                    .map(std::string::ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(", ")
            ))),
        }
    }
}

impl FromEnvStr for MlKemSuiteParam {
    type Error = SuiteParseError;

    fn from_env_str(value: Cow<'_, str>) -> core::result::Result<Self, Self::Error> {
        MlKemSuite::from_str(value.as_ref()).map(Self)
    }
}

#[derive(Debug)]
struct ChainIdInConfig(ChainId);

impl json::JsonSerialize for TrustedPeers {
    fn json_serialize(&self, out: &mut String) {
        self.0.json_serialize(out);
    }
}

impl json::JsonDeserialize for TrustedPeers {
    fn json_deserialize(
        parser: &mut json::Parser<'_>,
    ) -> ::core::result::Result<Self, json::Error> {
        let peers = <UniqueVec<Peer> as json::JsonDeserialize>::json_deserialize(parser)?;
        Ok(Self(peers))
    }
}

impl json::JsonDeserialize for ChainIdInConfig {
    fn json_deserialize(
        parser: &mut json::Parser<'_>,
    ) -> ::core::result::Result<Self, json::Error> {
        let chain = <ChainId as json::JsonDeserialize>::json_deserialize(parser)?;
        Ok(Self(chain))
    }
}

impl FromEnvStr for ChainIdInConfig {
    type Error = Infallible;

    fn from_env_str(value: Cow<'_, str>) -> std::result::Result<Self, Self::Error>
    where
        Self: Sized,
    {
        Ok(Self(ChainId::from(value)))
    }
}

/// User-level configuration container for `Root`.
#[derive(Debug, ReadConfig)]
pub struct Root {
    #[config(env = "CHAIN")]
    chain: ChainIdInConfig,
    #[config(env = "PUBLIC_KEY")]
    public_key: WithOrigin<PublicKey>,
    #[config(env = "PRIVATE_KEY")]
    private_key: WithOrigin<PrivateKey>,
    #[config(env = "TRUSTED_PEERS", default)]
    trusted_peers: WithOrigin<TrustedPeers>,
    #[config(
        env = "DEFAULT_ACCOUNT_DOMAIN_LABEL",
        default = "defaults::common::default_account_domain_label()"
    )]
    default_account_domain_label: WithOrigin<String>,
    #[config(
        env = "CHAIN_DISCRIMINANT",
        default = "defaults::common::chain_discriminant()"
    )]
    chain_discriminant: WithOrigin<u16>,
    /// BLS Proof-of-Possession entries for trusted peers.
    /// PoP entries must cover every BLS validator in `trusted_peers` and are verified
    /// during config parsing; incomplete or invalid PoPs are rejected.
    #[config(env = "TRUSTED_PEERS_POP", default)]
    trusted_peers_pop: TrustedPeerPops,
    #[config(nested)]
    genesis: Genesis,
    #[config(nested)]
    kura: Kura,
    #[config(nested)]
    sumeragi: Sumeragi,
    #[config(nested)]
    network: Network,
    #[config(nested)]
    logger: Logger,
    #[config(nested)]
    queue: Queue,
    #[config(nested)]
    nexus: Nexus,
    #[config(nested)]
    snapshot: Snapshot,
    /// Master telemetry on/off switch. Defaults to on.
    #[config(env = "TELEMETRY_ENABLED", default = "defaults::telemetry::ENABLED")]
    telemetry_enabled: bool,
    /// High-level telemetry profile controlling capability bundles.
    #[config(env = "TELEMETRY_PROFILE", default = "TelemetryProfile::Operator")]
    telemetry_profile: TelemetryProfile,
    telemetry: Option<Telemetry>,
    /// Telemetry redaction policy.
    #[config(nested)]
    telemetry_redaction: TelemetryRedaction,
    /// Telemetry integrity policy (hash chaining + optional signing key).
    #[config(nested)]
    telemetry_integrity: TelemetryIntegrity,
    #[config(nested)]
    dev_telemetry: DevTelemetry,
    #[config(nested)]
    torii: Torii,
    #[config(nested)]
    soracloud_runtime: SoracloudRuntime,
    #[config(nested)]
    sorafs: Sorafs,
    #[config(nested)]
    pipeline: Pipeline,
    #[config(nested)]
    tiered_state: TieredState,
    #[config(nested)]
    compute: Compute,
    #[config(nested)]
    content: Content,
    #[config(nested)]
    oracle: Oracle,
    #[config(nested)]
    confidential: Confidential,
    #[config(nested)]
    streaming: Streaming,
    #[config(nested)]
    crypto: Crypto,
    #[config(nested)]
    settlement: Settlement,
    /// Norito codec configuration
    #[config(nested)]
    norito: Norito,
    #[config(nested)]
    hijiri: Hijiri,
    #[config(nested)]
    fraud_monitoring: FraudMonitoring,
    #[config(nested)]
    zk: Zk,
    /// Governance parameters (verifying keys, policies)
    #[config(nested)]
    gov: Governance,
    /// Network Time Service parameters.
    #[config(nested)]
    nts: Nts,
    /// Hardware acceleration settings for IVM (optional; defaults enable all backends).
    #[config(nested)]
    accel: Acceleration,
    /// IVM banner and beep controls (default: show + beep enabled).
    #[config(nested)]
    ivm: Ivm,
    /// Concurrency settings for thread pools.
    #[config(nested)]
    concurrency: Concurrency,
}

/// User-level enumeration translating `ParseError` settings.
#[derive(thiserror::Error, Debug, Copy, Clone)]
pub enum ParseError {
    /// Key pair (public/private) values could not be combined into a valid key pair.
    #[error("Failed to construct the key pair")]
    BadKeyPair,
    /// Nexus configuration contained invalid lane or dataspace definitions.
    #[error("Invalid Nexus lane/dataspace configuration")]
    InvalidNexusConfig,
    /// Sumeragi consensus parameters failed validation.
    #[error("Invalid Sumeragi consensus configuration")]
    InvalidSumeragiConfig,
    /// Hijiri-specific parameters were inconsistent or malformed.
    #[error("Invalid Hijiri configuration")]
    InvalidHijiriConfig,
    /// Streaming configuration block lacked required or valid values.
    #[error("Invalid streaming configuration")]
    InvalidStreamingConfig,
    /// Telemetry configuration contained invalid or unsupported values.
    #[error("Invalid telemetry configuration")]
    InvalidTelemetryConfig,
    /// Settlement engine configuration was invalid or incomplete.
    #[error("Invalid settlement configuration")]
    InvalidSettlementConfig,
    /// Cryptography configuration contained invalid or unsupported values.
    #[error("Invalid crypto configuration")]
    InvalidCryptoConfig,
    /// Compute lane configuration contained invalid or inconsistent values.
    #[error("Invalid compute configuration")]
    InvalidComputeConfig,
    /// IVM configuration contained invalid or inconsistent values.
    #[error("Invalid IVM configuration")]
    InvalidIvmConfig,
    /// Concurrency configuration contained invalid values.
    #[error("Invalid concurrency configuration")]
    InvalidConcurrencyConfig,
    /// Common address-related configuration contained invalid values.
    #[error("Invalid common configuration")]
    InvalidCommonConfig,
}

struct AccountAddressParseScope {
    original_default_domain_label: std::sync::Arc<str>,
    original_chain_discriminant: u16,
    _lock: MutexGuard<'static, ()>,
}

impl AccountAddressParseScope {
    fn enter(
        default_domain_label: &str,
        chain_discriminant: u16,
        emitter: &mut Emitter<ParseError>,
    ) -> Self {
        let lock = ACCOUNT_ADDRESS_PARSE_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let original_default_domain_label =
            iroha_data_model::account::address::default_domain_name();
        let original_chain_discriminant = iroha_data_model::account::address::chain_discriminant();
        if let Err(err) = iroha_data_model::account::address::set_default_domain_name(
            default_domain_label.to_owned(),
        ) {
            emitter.emit(Report::new(ParseError::InvalidCommonConfig).attach(format!(
                "invalid default_account_domain_label `{default_domain_label}`: {err}",
            )));
        }
        iroha_data_model::account::address::set_chain_discriminant(chain_discriminant);
        Self {
            original_default_domain_label,
            original_chain_discriminant,
            _lock: lock,
        }
    }
}

impl Drop for AccountAddressParseScope {
    fn drop(&mut self) {
        let _ = iroha_data_model::account::address::set_default_domain_name(
            self.original_default_domain_label.to_string(),
        );
        iroha_data_model::account::address::set_chain_discriminant(
            self.original_chain_discriminant,
        );
    }
}

impl Root {
    fn parse_trusted_peer_pops(
        entries: &[TrustedPeerPop],
        emitter: &mut Emitter<ParseError>,
    ) -> BTreeMap<PublicKey, Vec<u8>> {
        let mut pops = BTreeMap::new();
        for entry in entries {
            let pk = &entry.public_key;
            if pk.algorithm() != Algorithm::BlsNormal {
                emitter.emit(
                    Report::new(ParseError::InvalidSumeragiConfig)
                        .attach(format!("trusted_peers_pop entry uses non-BLS key: {pk}")),
                );
                continue;
            }

            let pop_bytes =
                match hex::decode(entry.pop_hex.trim_start_matches("0x")) {
                    Ok(bytes) => bytes,
                    Err(err) => {
                        emitter.emit(Report::new(ParseError::InvalidSumeragiConfig).attach(
                            format!("trusted_peers_pop entry has invalid hex for {pk}: {err}"),
                        ));
                        continue;
                    }
                };

            if let Err(err) = iroha_crypto::bls_normal_pop_verify(pk, &pop_bytes) {
                emitter.emit(
                    Report::new(ParseError::InvalidSumeragiConfig).attach(format!(
                        "trusted_peers_pop entry has invalid PoP for {pk}: {err}"
                    )),
                );
                continue;
            }

            if pops.insert(pk.clone(), pop_bytes).is_some() {
                emitter.emit(
                    Report::new(ParseError::InvalidSumeragiConfig)
                        .attach(format!("trusted_peers_pop entry duplicated for {pk}")),
                );
            }
        }
        pops
    }

    fn validate_trusted_peer_pops(
        trusted: &actual::TrustedPeers,
        emitter: &mut Emitter<ParseError>,
    ) {
        let mut roster_keys: BTreeSet<PublicKey> = BTreeSet::new();
        let mut non_bls: Vec<String> = Vec::new();
        for peer in std::iter::once(&trusted.myself).chain(trusted.others.iter()) {
            let pk = peer.id().public_key();
            if pk.algorithm() == Algorithm::BlsNormal {
                roster_keys.insert(pk.clone());
            } else {
                non_bls.push(pk.to_string());
            }
        }
        if !non_bls.is_empty() {
            emitter.emit(
                Report::new(ParseError::InvalidSumeragiConfig).attach(format!(
                    "trusted_peers contains non-BLS validator keys: {non_bls:?}"
                )),
            );
        }

        let missing: Vec<_> = roster_keys
            .iter()
            .filter(|pk| !trusted.pops.contains_key(*pk))
            .map(ToString::to_string)
            .collect();
        if !missing.is_empty() {
            emitter.emit(
                Report::new(ParseError::InvalidSumeragiConfig).attach(format!(
                    "trusted_peers_pop missing PoPs for roster keys: {missing:?}"
                )),
            );
        }

        let extras: Vec<_> = trusted
            .pops
            .keys()
            .filter(|pk| !roster_keys.contains(*pk))
            .map(ToString::to_string)
            .collect();
        if !extras.is_empty() {
            emitter.emit(
                Report::new(ParseError::InvalidSumeragiConfig).attach(format!(
                    "trusted_peers_pop contains keys not in trusted_peers: {extras:?}"
                )),
            );
        }
    }

    /// Parses user configuration view into the internal repr.
    ///
    /// # Errors
    /// If any invalidity found.
    /// Convert this user configuration into the runtime representation.
    #[allow(clippy::too_many_lines)]
    pub fn parse(self) -> Result<actual::Root, ParseError> {
        let mut emitter = Emitter::new();
        let _account_address_scope = AccountAddressParseScope::enter(
            self.default_account_domain_label.value(),
            *self.chain_discriminant.value(),
            &mut emitter,
        );

        let (private_key, private_key_origin) = self.private_key.into_tuple();
        let (public_key, public_key_origin) = self.public_key.into_tuple();
        let key_pair = iroha_crypto::KeyPair::new(public_key, private_key)
            .attach(ConfigValueAndOrigin::new("[REDACTED]", public_key_origin))
            .attach(ConfigValueAndOrigin::new("[REDACTED]", private_key_origin))
            .change_context(ParseError::BadKeyPair)
            .ok_or_emit(&mut emitter);
        if let Some(key_pair) = key_pair.as_ref()
            && key_pair.public_key().algorithm() != Algorithm::BlsNormal
        {
            emitter.emit(
                Report::new(ParseError::InvalidSumeragiConfig)
                    .attach("public_key/private_key must be BLS-normal for consensus"),
            );
        }

        let (network, block_sync, transaction_gossiper) = self.network.parse();
        let Some(key_pair_ref) = key_pair.as_ref() else {
            panic!("Key pair is missing");
        };
        let peer = Peer::new(
            network.address.value().clone(),
            key_pair_ref.public_key().clone(),
        );
        let trusted_peers = self.trusted_peers.map(|x| {
            let others = x.0.into_iter().filter(|p| p.id() != peer.id()).collect();
            let pops = Self::parse_trusted_peer_pops(&self.trusted_peers_pop.0, &mut emitter);
            let trusted = actual::TrustedPeers {
                myself: peer.clone(),
                others,
                pops,
            };
            Self::validate_trusted_peer_pops(&trusted, &mut emitter);
            trusted
        });

        let genesis = self.genesis.into();

        let kura = self.kura.parse();

        let logger = self.logger;
        let queue = self.queue;
        let snapshot = self.snapshot;
        let dev_telemetry = self.dev_telemetry;
        let (
            sorafs_storage,
            sorafs_discovery,
            sorafs_repair,
            sorafs_gc,
            sorafs_quota,
            sorafs_alias_cache,
            sorafs_gateway,
            sorafs_por,
        ) = self.sorafs.parse();
        let (mut torii, live_query_store) = self.torii.parse();
        let soracloud_runtime = self.soracloud_runtime.parse();
        let telemetry = self.telemetry.map(actual::Telemetry::from);
        let telemetry_profile = if self.telemetry_enabled {
            actual::TelemetryProfile::from(self.telemetry_profile)
        } else {
            actual::TelemetryProfile::Disabled
        };
        let telemetry_redaction = self.telemetry_redaction.parse(&mut emitter);
        let telemetry_integrity = self.telemetry_integrity.parse(&mut emitter);

        let sumeragi = self.sumeragi.parse(&mut emitter);
        if let Some(ref sumeragi) = sumeragi {
            let roster_retention =
                u64::try_from(kura.block_sync_roster_retention.get()).unwrap_or(u64::MAX);
            if sumeragi.npos.reconfig.evidence_horizon_blocks > roster_retention {
                emitter.emit(
                    Report::new(ParseError::InvalidSumeragiConfig).attach(format!(
                        "kura.block_sync_roster_retention ({}) must be >= sumeragi.npos.reconfig.evidence_horizon_blocks ({})",
                        kura.block_sync_roster_retention.get(),
                        sumeragi.npos.reconfig.evidence_horizon_blocks
                    )),
                );
            }
        }
        let pipeline = self.pipeline.parse();
        let tiered_state = self.tiered_state.parse();
        let compute = self.compute.parse(&mut emitter);
        let content = self.content.parse();
        let oracle = self.oracle.parse();
        let ivm = self.ivm.parse();
        let mut zk = self.zk.parse();
        let concurrency = self.concurrency.parse();
        let gov = self.gov.parse();
        let nts = self.nts.parse();
        let nexus = self.nexus.parse(&mut emitter);
        let confidential = self.confidential.parse();
        let streaming = if let Some(ref key_pair) = key_pair {
            self.streaming.parse(key_pair, &mut emitter)
        } else {
            None
        };
        torii.sorafs_storage = sorafs_storage;
        torii.sorafs_discovery = sorafs_discovery;
        torii.sorafs_repair = sorafs_repair;
        torii.sorafs_gc = sorafs_gc;
        torii.sorafs_quota = sorafs_quota;
        torii.sorafs_alias_cache = sorafs_alias_cache;
        torii.sorafs_gateway = sorafs_gateway;
        torii.sorafs_por = sorafs_por;
        let crypto = self.crypto.parse(&mut emitter);
        let settlement = self.settlement.parse(&mut emitter);
        let hijiri = self.hijiri.parse(&mut emitter);

        if let Err(err) = concurrency.validate() {
            emitter.emit(err);
        }
        if let Some(ref compute) = compute
            && !compute
                .resource_profiles
                .contains_key(&ivm.memory_budget_profile)
        {
            emitter.emit(Report::new(ParseError::InvalidIvmConfig).attach(format!(
                "ivm.memory_budget_profile `{}` missing from compute.resource_profiles",
                ivm.memory_budget_profile
            )));
        }

        zk.max_proof_size_bytes = confidential.max_proof_size_bytes;
        zk.max_nullifiers_per_tx = confidential.max_nullifiers_per_tx;
        zk.max_commitments_per_tx = confidential.max_commitments_per_tx;
        zk.max_confidential_ops_per_block = confidential.max_confidential_ops_per_block;
        zk.verify_timeout = confidential.verify_timeout;
        zk.max_anchor_age_blocks = confidential.max_anchor_age_blocks;
        zk.max_proof_bytes_block = confidential.max_proof_bytes_block;
        zk.max_verify_calls_per_tx = confidential.max_verify_calls_per_tx;
        zk.max_verify_calls_per_block = confidential.max_verify_calls_per_block;
        zk.max_public_inputs = confidential.max_public_inputs;
        zk.reorg_depth_bound = confidential.reorg_depth_bound;
        zk.policy_transition_delay_blocks = confidential.policy_transition_delay_blocks;
        zk.policy_transition_window_blocks = confidential.policy_transition_window_blocks;
        zk.tree_roots_history_len = confidential.tree_roots_history_len;
        zk.tree_frontier_checkpoint_interval = confidential.tree_frontier_checkpoint_interval;
        zk.registry_max_vk_entries = confidential.registry_max_vk_entries;
        zk.registry_max_params_entries = confidential.registry_max_params_entries;
        zk.registry_max_delta_per_block = confidential.registry_max_delta_per_block;
        zk.gas = confidential.gas;

        emitter.into_result()?;

        let sumeragi =
            sumeragi.expect("sumeragi configuration should be valid when emitter succeeds");
        let nexus = nexus.expect("nexus configuration should be valid when emitter succeeds");
        let hijiri = hijiri.expect("Hijiri configuration should be valid when emitter succeeds");
        let streaming =
            streaming.expect("streaming configuration should be valid when emitter succeeds");
        let compute = compute.expect("compute configuration should be valid when emitter succeeds");

        let key_pair = key_pair.unwrap();
        let peer = actual::Common {
            chain: self.chain.0,
            key_pair,
            peer,
            trusted_peers,
            default_account_domain_label: self.default_account_domain_label,
            chain_discriminant: self.chain_discriminant,
        };

        let mut root = actual::Root {
            common: peer,
            network,
            genesis,
            torii,
            soracloud_runtime,
            kura,
            sumeragi,
            block_sync,
            transaction_gossiper,
            live_query_store,
            logger,
            queue: queue.parse(),
            nexus,
            snapshot,
            telemetry_enabled: self.telemetry_enabled,
            telemetry_profile,
            telemetry,
            telemetry_redaction,
            telemetry_integrity,
            dev_telemetry,
            pipeline,
            tiered_state,
            compute,
            content,
            oracle,
            ivm,
            norito: self.norito.parse(),
            hijiri,
            fraud_monitoring: self.fraud_monitoring.parse(),
            zk,
            gov,
            nts,
            accel: self.accel.parse(),
            concurrency,
            streaming,
            crypto,
            settlement,
            confidential,
        };
        root.apply_storage_budget();
        Ok(root)
    }
}

/// Telemetry capability bundles selectable by configuration.
/// User-level enumeration translating `TelemetryProfile` settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, strum::EnumString, strum::Display)]
#[strum(serialize_all = "snake_case")]
pub enum TelemetryProfile {
    /// Disable telemetry entirely.
    Disabled,
    /// Enable lightweight operator metrics and status endpoints.
    #[default]
    Operator,
    /// Enable operator metrics plus expensive exporters (e.g., Prometheus scraping).
    Extended,
    /// Enable operator metrics plus developer sinks (JSON/file dumps, tracing bridges).
    Developer,
    /// Enable all telemetry features supported by the build.
    Full,
}
impl json::JsonSerialize for TelemetryProfile {
    fn json_serialize(&self, out: &mut String) {
        json::write_json_string(&self.to_string(), out);
    }
}

impl json::JsonDeserialize for TelemetryProfile {
    fn json_deserialize(
        parser: &mut json::Parser<'_>,
    ) -> ::core::result::Result<Self, json::Error> {
        let text = parser.parse_string()?;
        Self::from_str(&text).map_err(|err| json::Error::InvalidField {
            field: "telemetry_profile".into(),
            message: err.to_string(),
        })
    }
}

/// Telemetry redaction modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, strum::EnumString, strum::Display)]
#[strum(serialize_all = "snake_case")]
pub enum TelemetryRedactionMode {
    /// Redact all sensitive fields; ignore allow-list entries.
    #[default]
    Strict,
    /// Redact sensitive fields unless explicitly allow-listed.
    Allowlist,
    /// Disable telemetry redaction (developer-only).
    Disabled,
}

impl json::JsonSerialize for TelemetryRedactionMode {
    fn json_serialize(&self, out: &mut String) {
        json::write_json_string(&self.to_string(), out);
    }
}

impl json::JsonDeserialize for TelemetryRedactionMode {
    fn json_deserialize(
        parser: &mut json::Parser<'_>,
    ) -> ::core::result::Result<Self, json::Error> {
        let text = parser.parse_string()?;
        Self::from_str(&text).map_err(|err| json::Error::InvalidField {
            field: "telemetry_redaction.mode".into(),
            message: err.to_string(),
        })
    }
}

fn default_telemetry_redaction_mode() -> TelemetryRedactionMode {
    TelemetryRedactionMode::from_str(defaults::telemetry::redaction::MODE)
        .expect("invalid telemetry redaction default")
}

/// User-level configuration container for telemetry redaction.
#[derive(Debug, Clone, ReadConfig)]
pub struct TelemetryRedaction {
    /// Redaction mode (default: `strict`).
    #[config(default = "default_telemetry_redaction_mode()")]
    pub mode: TelemetryRedactionMode,
    /// Optional allow-list of field names that may bypass keyword redaction.
    #[config(default)]
    pub allowlist: Vec<String>,
}

impl TelemetryRedaction {
    fn parse(self, emitter: &mut Emitter<ParseError>) -> actual::TelemetryRedaction {
        let mut allowlist = Vec::with_capacity(self.allowlist.len());
        for entry in self.allowlist {
            let trimmed = entry.trim();
            if trimmed.is_empty() {
                emitter.emit(
                    Report::new(ParseError::InvalidTelemetryConfig)
                        .attach("telemetry_redaction.allowlist entry cannot be empty"),
                );
                continue;
            }
            allowlist.push(trimmed.to_string());
        }
        actual::TelemetryRedaction {
            mode: self.mode.into(),
            allowlist,
        }
    }
}

/// User-level configuration container for telemetry integrity.
#[derive(Debug, Clone, ReadConfig)]
pub struct TelemetryIntegrity {
    /// Enable hash-chained telemetry exports.
    #[config(default = "defaults::telemetry::integrity::ENABLED")]
    pub enabled: bool,
    /// Optional directory for integrity state snapshots (enables continuity across restarts).
    pub state_dir: Option<WithOrigin<PathBuf>>,
    /// Optional hex-encoded 32-byte signing key for keyed hashes.
    pub signing_key_hex: Option<String>,
    /// Optional key identifier for rotation workflows.
    pub signing_key_id: Option<String>,
}

impl TelemetryIntegrity {
    fn parse(self, emitter: &mut Emitter<ParseError>) -> actual::TelemetryIntegrity {
        let signing_key =
            self.signing_key_hex
                .and_then(|raw| match parse_telemetry_signing_key(&raw) {
                    Ok(key) => Some(key),
                    Err(message) => {
                        emitter
                            .emit(Report::new(ParseError::InvalidTelemetryConfig).attach(message));
                        None
                    }
                });

        let state_dir = self.state_dir.map(|path| path.resolve_relative_path());

        actual::TelemetryIntegrity {
            enabled: self.enabled,
            signing_key,
            signing_key_id: self.signing_key_id,
            state_dir,
        }
    }
}

fn parse_telemetry_signing_key(raw: &str) -> core::result::Result<[u8; 32], String> {
    let trimmed = raw.trim();
    let trimmed = trimmed.strip_prefix("0x").unwrap_or(trimmed);
    let bytes = hex::decode(trimmed)
        .map_err(|err| format!("telemetry_integrity.signing_key_hex must be hex-encoded: {err}"))?;
    if bytes.len() != 32 {
        return Err(format!(
            "telemetry_integrity.signing_key_hex must be 32 bytes (got {})",
            bytes.len()
        ));
    }
    let mut key = [0_u8; 32];
    key.copy_from_slice(&bytes);
    Ok(key)
}

/// Verifying key reference (user view)
/// User-level configuration container for `VerifyingKeyRef`.
#[derive(Debug, ReadConfig, Clone, norito::JsonDeserialize)]
pub struct VerifyingKeyRef {
    /// Backend identifier, e.g., "halo2/ipa" or "groth16/bn254".
    #[config(env = "GOV_VK_BACKEND", default)]
    pub backend: String,
    /// Name within backend namespace, e.g., "ballot_v1".
    #[config(env = "GOV_VK_NAME", default)]
    pub name: String,
}

impl VerifyingKeyRef {
    fn parse(self) -> actual::VerifyingKeyRef {
        actual::VerifyingKeyRef {
            backend: self.backend,
            name: self.name,
        }
    }
}

/// Governance configuration (user view)
/// User-level configuration container for `Governance`.
#[derive(Debug, ReadConfig, Clone, norito::JsonDeserialize)]
pub struct SorafsPinPolicyConstraints {
    /// Minimum replicas floor enforced for approved manifests.
    #[config(
        env = "GOV_SORAFS_PIN_POLICY_MIN_REPLICAS_FLOOR",
        default = "crate::parameters::defaults::governance::sorafs_pin_policy::MIN_REPLICAS_FLOOR"
    )]
    pub min_replicas_floor: u16,
    /// Optional maximum replicas ceiling (inclusive).
    #[config(env = "GOV_SORAFS_PIN_POLICY_MAX_REPLICAS_CEILING")]
    pub max_replicas_ceiling: Option<u16>,
    /// Optional retention epoch cap (inclusive).
    #[config(env = "GOV_SORAFS_PIN_POLICY_MAX_RETENTION_EPOCH")]
    pub max_retention_epoch: Option<u64>,
    /// Allowed storage classes (case-insensitive). `None` permits any class.
    #[config(env = "GOV_SORAFS_PIN_POLICY_ALLOWED_STORAGE_CLASSES")]
    pub allowed_storage_classes: Option<AlgorithmListConfig>,
}

impl Default for SorafsPinPolicyConstraints {
    fn default() -> Self {
        Self {
            min_replicas_floor:
                crate::parameters::defaults::governance::sorafs_pin_policy::MIN_REPLICAS_FLOOR,
            max_replicas_ceiling:
                crate::parameters::defaults::governance::sorafs_pin_policy::MAX_REPLICAS_CEILING,
            max_retention_epoch:
                crate::parameters::defaults::governance::sorafs_pin_policy::MAX_RETENTION_EPOCH,
            allowed_storage_classes: None,
        }
    }
}

impl SorafsPinPolicyConstraints {
    fn parse(self) -> actual::SorafsPinPolicyConstraints {
        let allowed_storage_classes = self.allowed_storage_classes.map(|classes| {
            classes
                .into_vec()
                .into_iter()
                .map(|class| match class.trim().to_ascii_lowercase().as_str() {
                    "hot" => SorafsStorageClass::Hot,
                    "warm" => SorafsStorageClass::Warm,
                    "cold" => SorafsStorageClass::Cold,
                    other => panic!(
                        "Invalid governance.sorafs_pin_policy.allowed_storage_classes entry `{other}`; expected hot, warm, or cold"
                    ),
                })
                .collect::<BTreeSet<_>>()
        });

        actual::SorafsPinPolicyConstraints {
            min_replicas_floor: self.min_replicas_floor,
            max_replicas_ceiling: self.max_replicas_ceiling,
            max_retention_epoch: self.max_retention_epoch,
            allowed_storage_classes,
        }
    }
}

/// User-level configuration for SoraFS under-delivery penalties.
#[derive(Debug, ReadConfig, Clone, Copy, norito::JsonDeserialize)]
pub struct SorafsPenaltyPolicy {
    /// Minimum utilisation ratio (basis points) before counting a strike.
    #[config(
        env = "GOV_SORAFS_PENALTY_UTILISATION_FLOOR_BPS",
        default = "crate::parameters::defaults::governance::sorafs_penalty::UTILISATION_FLOOR_BPS"
    )]
    pub utilisation_floor_bps: u16,
    /// Minimum uptime ratio (basis points) before counting a strike.
    #[config(
        env = "GOV_SORAFS_PENALTY_UPTIME_FLOOR_BPS",
        default = "crate::parameters::defaults::governance::sorafs_penalty::UPTIME_FLOOR_BPS"
    )]
    pub uptime_floor_bps: u16,
    /// Minimum PoR success ratio (basis points) before counting a strike.
    #[config(
        env = "GOV_SORAFS_PENALTY_POR_FLOOR_BPS",
        default = "crate::parameters::defaults::governance::sorafs_penalty::POR_SUCCESS_FLOOR_BPS"
    )]
    pub por_success_floor_bps: u16,
    /// Number of consecutive strikes required before slashing.
    #[config(
        env = "GOV_SORAFS_PENALTY_STRIKE_THRESHOLD",
        default = "crate::parameters::defaults::governance::sorafs_penalty::STRIKE_THRESHOLD"
    )]
    pub strike_threshold: u32,
    /// Fraction of bonded collateral removed per penalty (basis points).
    #[config(
        env = "GOV_SORAFS_PENALTY_BOND_BPS",
        default = "crate::parameters::defaults::governance::sorafs_penalty::PENALTY_BOND_BPS"
    )]
    pub penalty_bond_bps: u16,
    /// Cooldown window count (settlement windows) enforced between penalties.
    #[config(
        env = "GOV_SORAFS_PENALTY_COOLDOWN_WINDOWS",
        default = "crate::parameters::defaults::governance::sorafs_penalty::COOLDOWN_WINDOWS"
    )]
    pub cooldown_windows: u32,
    /// Maximum PDP failures tolerated within a telemetry window before forcing a strike.
    #[config(
        env = "GOV_SORAFS_PENALTY_MAX_PDP_FAILURES",
        default = "crate::parameters::defaults::governance::sorafs_penalty::MAX_PDP_FAILURES"
    )]
    pub max_pdp_failures: u32,
    /// Maximum PoTR SLA breaches tolerated within a telemetry window before forcing a strike.
    #[config(
        env = "GOV_SORAFS_PENALTY_MAX_POTR_BREACHES",
        default = "crate::parameters::defaults::governance::sorafs_penalty::MAX_POTR_BREACHES"
    )]
    pub max_potr_breaches: u32,
}

impl Default for SorafsPenaltyPolicy {
    fn default() -> Self {
        Self {
            utilisation_floor_bps:
                crate::parameters::defaults::governance::sorafs_penalty::UTILISATION_FLOOR_BPS,
            uptime_floor_bps:
                crate::parameters::defaults::governance::sorafs_penalty::UPTIME_FLOOR_BPS,
            por_success_floor_bps:
                crate::parameters::defaults::governance::sorafs_penalty::POR_SUCCESS_FLOOR_BPS,
            strike_threshold:
                crate::parameters::defaults::governance::sorafs_penalty::STRIKE_THRESHOLD,
            penalty_bond_bps:
                crate::parameters::defaults::governance::sorafs_penalty::PENALTY_BOND_BPS,
            cooldown_windows:
                crate::parameters::defaults::governance::sorafs_penalty::COOLDOWN_WINDOWS,
            max_pdp_failures:
                crate::parameters::defaults::governance::sorafs_penalty::MAX_PDP_FAILURES,
            max_potr_breaches:
                crate::parameters::defaults::governance::sorafs_penalty::MAX_POTR_BREACHES,
        }
    }
}

impl SorafsPenaltyPolicy {
    fn parse(self) -> actual::SorafsPenaltyPolicy {
        actual::SorafsPenaltyPolicy {
            utilisation_floor_bps: self.utilisation_floor_bps,
            uptime_floor_bps: self.uptime_floor_bps,
            por_success_floor_bps: self.por_success_floor_bps,
            strike_threshold: self.strike_threshold,
            penalty_bond_bps: self.penalty_bond_bps,
            cooldown_windows: self.cooldown_windows,
            max_pdp_failures: self.max_pdp_failures,
            max_potr_breaches: self.max_potr_breaches,
        }
    }
}

/// User-level configuration for SoraFS repair escalation governance policy.
#[derive(Debug, ReadConfig, Clone, Copy, norito::JsonDeserialize)]
pub struct RepairEscalationPolicyV1 {
    /// Approval quorum (basis points) required for escalation/slash decisions.
    #[config(
        env = "GOV_SORAFS_REPAIR_QUORUM_BPS",
        default = "crate::parameters::defaults::governance::sorafs_repair_escalation::QUORUM_BPS"
    )]
    pub quorum_bps: u16,
    /// Minimum number of distinct voters required to resolve a decision.
    #[config(
        env = "GOV_SORAFS_REPAIR_MINIMUM_VOTERS",
        default = "crate::parameters::defaults::governance::sorafs_repair_escalation::MINIMUM_VOTERS"
    )]
    pub minimum_voters: u32,
    /// Dispute window in seconds after escalation before governance finalizes.
    #[config(
        env = "GOV_SORAFS_REPAIR_DISPUTE_WINDOW_SECS",
        default = "crate::parameters::defaults::governance::sorafs_repair_escalation::DISPUTE_WINDOW_SECS"
    )]
    pub dispute_window_secs: u64,
    /// Appeal window in seconds after approval before a decision is final.
    #[config(
        env = "GOV_SORAFS_REPAIR_APPEAL_WINDOW_SECS",
        default = "crate::parameters::defaults::governance::sorafs_repair_escalation::APPEAL_WINDOW_SECS"
    )]
    pub appeal_window_secs: u64,
    /// Maximum slash penalty allowed for repair escalations (nano-XOR).
    #[config(
        env = "GOV_SORAFS_REPAIR_MAX_PENALTY_NANO",
        default = "crate::parameters::defaults::governance::sorafs_repair_escalation::MAX_PENALTY_NANO"
    )]
    pub max_penalty_nano: u128,
}

impl Default for RepairEscalationPolicyV1 {
    fn default() -> Self {
        Self {
            quorum_bps:
                crate::parameters::defaults::governance::sorafs_repair_escalation::QUORUM_BPS,
            minimum_voters:
                crate::parameters::defaults::governance::sorafs_repair_escalation::MINIMUM_VOTERS,
            dispute_window_secs: crate::parameters::defaults::governance::sorafs_repair_escalation::DISPUTE_WINDOW_SECS,
            appeal_window_secs: crate::parameters::defaults::governance::sorafs_repair_escalation::APPEAL_WINDOW_SECS,
            max_penalty_nano: crate::parameters::defaults::governance::sorafs_repair_escalation::MAX_PENALTY_NANO,
        }
    }
}

impl RepairEscalationPolicyV1 {
    fn parse(self) -> actual::RepairEscalationPolicyV1 {
        actual::RepairEscalationPolicyV1 {
            quorum_bps: self.quorum_bps.min(10_000),
            minimum_voters: self.minimum_voters.max(1),
            dispute_window_secs: self.dispute_window_secs,
            appeal_window_secs: self.appeal_window_secs,
            max_penalty_nano: self.max_penalty_nano,
        }
    }
}

/// Telemetry authentication and replay policy for SoraFS capacity windows.
#[derive(Debug, ReadConfig, Clone)]
pub struct SorafsTelemetryPolicy {
    /// Require telemetry submissions to originate from an allow-list.
    #[config(
        env = "SORAFS_TELEMETRY_REQUIRE_SUBMITTER",
        default = "defaults::governance::sorafs_telemetry::REQUIRE_SUBMITTER"
    )]
    pub require_submitter: bool,
    /// Require a nonce per telemetry window for replay detection.
    /// When disabled, windows without a nonce are accepted but provided nonces are still checked.
    #[config(
        env = "SORAFS_TELEMETRY_REQUIRE_NONCE",
        default = "defaults::governance::sorafs_telemetry::REQUIRE_NONCE"
    )]
    pub require_nonce: bool,
    /// Maximum tolerated gap between telemetry windows (seconds).
    #[config(
        env = "SORAFS_TELEMETRY_MAX_WINDOW_GAP_SECS",
        default = "defaults::governance::sorafs_telemetry::MAX_WINDOW_GAP_SECS"
    )]
    pub max_window_gap_secs: u64,
    /// Reject zero-capacity telemetry windows.
    #[config(
        env = "SORAFS_TELEMETRY_REJECT_ZERO_CAPACITY",
        default = "defaults::governance::sorafs_telemetry::REJECT_ZERO_CAPACITY"
    )]
    pub reject_zero_capacity: bool,
    /// Account ids authorised to submit telemetry when enforcement is enabled.
    #[config(
        env = "SORAFS_TELEMETRY_SUBMITTERS",
        default = "AlgorithmListConfig::from(defaults::governance::sorafs_telemetry::submitters())"
    )]
    pub submitters: AlgorithmListConfig,
    /// Provider-specific submitter overrides keyed by provider identifier (hex).
    #[config(
        env = "SORAFS_TELEMETRY_PER_PROVIDER_SUBMITTERS",
        default = "PerProviderSubmittersConfig::default()"
    )]
    pub per_provider_submitters: PerProviderSubmittersConfig,
}

impl Default for SorafsTelemetryPolicy {
    fn default() -> Self {
        Self {
            require_submitter: defaults::governance::sorafs_telemetry::REQUIRE_SUBMITTER,
            require_nonce: defaults::governance::sorafs_telemetry::REQUIRE_NONCE,
            max_window_gap_secs: defaults::governance::sorafs_telemetry::MAX_WINDOW_GAP_SECS,
            reject_zero_capacity: defaults::governance::sorafs_telemetry::REJECT_ZERO_CAPACITY,
            submitters: AlgorithmListConfig::from(
                defaults::governance::sorafs_telemetry::submitters(),
            ),
            per_provider_submitters: PerProviderSubmittersConfig::default(),
        }
    }
}

impl SorafsTelemetryPolicy {
    fn parse(self) -> actual::SorafsTelemetryPolicy {
        let submitters = self
            .submitters
            .into_vec()
            .into_iter()
            .map(|account| {
                parse_account_id_literal(&account, "invalid SoraFS telemetry submitter account id")
            })
            .collect();
        actual::SorafsTelemetryPolicy {
            require_submitter: self.require_submitter,
            require_nonce: self.require_nonce,
            max_window_gap: Duration::from_secs(self.max_window_gap_secs),
            reject_zero_capacity: self.reject_zero_capacity,
            submitters,
            per_provider_submitters: self
                .per_provider_submitters
                .0
                .into_iter()
                .map(|(provider, submitters)| {
                    let bytes = hex::decode(provider.trim_start_matches("0x"))
                        .unwrap_or_else(|err| panic!("invalid provider id {provider}: {err}"));
                    let array: [u8; 32] = bytes
                        .try_into()
                        .unwrap_or_else(|_| panic!("provider id {provider} must be 32 bytes"));
                    let provider_id = ProviderId::new(array);
                    let parsed_submitters = submitters
                        .into_vec()
                        .into_iter()
                        .map(|account| {
                            parse_account_id_literal(
                                &account,
                                "invalid SoraFS telemetry submitter account id",
                            )
                        })
                        .collect();
                    (provider_id, parsed_submitters)
                })
                .collect(),
        }
    }
}

/// Citizen service discipline (user view).
#[derive(Debug, ReadConfig, Clone)]
pub struct CitizenServiceDiscipline {
    /// Cooldown (blocks) enforced after a citizen accepts a seat.
    #[config(
        env = "GOV_CITIZEN_SEAT_COOLDOWN_BLOCKS",
        default = "defaults::governance::citizen_service::SEAT_COOLDOWN_BLOCKS"
    )]
    pub seat_cooldown_blocks: u64,
    /// Maximum seats a single citizen may occupy in one epoch.
    #[config(
        env = "GOV_CITIZEN_MAX_SEATS_PER_EPOCH",
        default = "defaults::governance::citizen_service::MAX_SEATS_PER_EPOCH"
    )]
    pub max_seats_per_epoch: u32,
    /// Number of declines that do not trigger slashing per epoch.
    #[config(
        env = "GOV_CITIZEN_FREE_DECLINES_PER_EPOCH",
        default = "defaults::governance::citizen_service::FREE_DECLINES_PER_EPOCH"
    )]
    pub free_declines_per_epoch: u32,
    /// Slash percentage applied when declines exceed the free allowance (basis points).
    #[config(
        env = "GOV_CITIZEN_DECLINE_SLASH_BPS",
        default = "defaults::governance::citizen_service::DECLINE_SLASH_BPS"
    )]
    pub decline_slash_bps: u16,
    /// Slash percentage applied when a citizen fails to appear (basis points).
    #[config(
        env = "GOV_CITIZEN_NO_SHOW_SLASH_BPS",
        default = "defaults::governance::citizen_service::NO_SHOW_SLASH_BPS"
    )]
    pub no_show_slash_bps: u16,
    /// Slash percentage applied when misconduct is recorded (basis points).
    #[config(
        env = "GOV_CITIZEN_MISCONDUCT_SLASH_BPS",
        default = "defaults::governance::citizen_service::MISCONDUCT_SLASH_BPS"
    )]
    pub misconduct_slash_bps: u16,
    /// Optional bond multipliers keyed by governance role name.
    #[config(default = "defaults::governance::citizen_service::role_bond_multipliers()")]
    pub role_bond_multipliers: BTreeMap<String, u64>,
}

impl CitizenServiceDiscipline {
    fn parse(self) -> actual::CitizenServiceDiscipline {
        actual::CitizenServiceDiscipline {
            seat_cooldown_blocks: self.seat_cooldown_blocks,
            max_seats_per_epoch: self.max_seats_per_epoch,
            free_declines_per_epoch: self.free_declines_per_epoch,
            decline_slash_bps: self.decline_slash_bps,
            no_show_slash_bps: self.no_show_slash_bps,
            misconduct_slash_bps: self.misconduct_slash_bps,
            role_bond_multipliers: self.role_bond_multipliers,
        }
    }
}

impl Default for CitizenServiceDiscipline {
    fn default() -> Self {
        Self {
            seat_cooldown_blocks: defaults::governance::citizen_service::SEAT_COOLDOWN_BLOCKS,
            max_seats_per_epoch: defaults::governance::citizen_service::MAX_SEATS_PER_EPOCH,
            free_declines_per_epoch: defaults::governance::citizen_service::FREE_DECLINES_PER_EPOCH,
            decline_slash_bps: defaults::governance::citizen_service::DECLINE_SLASH_BPS,
            no_show_slash_bps: defaults::governance::citizen_service::NO_SHOW_SLASH_BPS,
            misconduct_slash_bps: defaults::governance::citizen_service::MISCONDUCT_SLASH_BPS,
            role_bond_multipliers: defaults::governance::citizen_service::role_bond_multipliers(),
        }
    }
}

/// Runtime-upgrade provenance enforcement modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, strum::EnumString, strum::Display)]
#[strum(serialize_all = "snake_case")]
pub enum RuntimeUpgradeProvenanceMode {
    /// Provenance is optional; when provided it is verified.
    #[default]
    Optional,
    /// Provenance is required for runtime upgrade manifests.
    Required,
}

impl json::JsonSerialize for RuntimeUpgradeProvenanceMode {
    fn json_serialize(&self, out: &mut String) {
        json::write_json_string(&self.to_string(), out);
    }
}

impl json::JsonDeserialize for RuntimeUpgradeProvenanceMode {
    fn json_deserialize(
        parser: &mut json::Parser<'_>,
    ) -> ::core::result::Result<Self, json::Error> {
        let text = parser.parse_string()?;
        Self::from_str(&text).map_err(|err| json::Error::InvalidField {
            field: "governance.runtime_upgrade_provenance.mode".into(),
            message: err.to_string(),
        })
    }
}

fn default_runtime_upgrade_provenance_mode() -> RuntimeUpgradeProvenanceMode {
    RuntimeUpgradeProvenanceMode::from_str(defaults::governance::RUNTIME_UPGRADE_PROVENANCE_MODE)
        .expect("invalid runtime upgrade provenance mode default")
}

/// Runtime-upgrade provenance policy (user view).
#[derive(Debug, ReadConfig, Clone)]
pub struct RuntimeUpgradeProvenance {
    /// Whether provenance is required for runtime upgrade manifests.
    #[config(default = "default_runtime_upgrade_provenance_mode()")]
    pub mode: RuntimeUpgradeProvenanceMode,
    /// Require at least one SBOM digest entry when provenance is present/required.
    #[config(default = "defaults::governance::RUNTIME_UPGRADE_PROVENANCE_REQUIRE_SBOM")]
    pub require_sbom: bool,
    /// Require a non-empty SLSA attestation when provenance is present/required.
    #[config(default = "defaults::governance::RUNTIME_UPGRADE_PROVENANCE_REQUIRE_SLSA")]
    pub require_slsa: bool,
    /// Minimum number of trusted signatures required when provenance is present/required.
    #[config(default = "defaults::governance::RUNTIME_UPGRADE_PROVENANCE_SIGNATURE_THRESHOLD")]
    pub signature_threshold: usize,
    /// Trusted signer public keys (multihash).
    #[config(default)]
    pub trusted_signers: Vec<String>,
}

impl RuntimeUpgradeProvenance {
    fn parse(self) -> actual::RuntimeUpgradeProvenancePolicy {
        if matches!(self.mode, RuntimeUpgradeProvenanceMode::Optional)
            && (self.require_sbom || self.require_slsa || self.signature_threshold > 0)
        {
            panic!(
                "governance.runtime_upgrade_provenance.mode=optional cannot require sbom/slsa/signatures"
            );
        }
        let mut trusted_signers = BTreeSet::new();
        for signer in self.trusted_signers {
            let pk = PublicKey::from_str(signer.trim()).unwrap_or_else(|err| {
                panic!("invalid runtime upgrade provenance signer {signer}: {err}");
            });
            trusted_signers.insert(pk);
        }
        if self.signature_threshold > 0 {
            assert!(
                !trusted_signers.is_empty(),
                "governance.runtime_upgrade_provenance.signature_threshold requires trusted_signers"
            );
            assert!(
                self.signature_threshold <= trusted_signers.len(),
                "governance.runtime_upgrade_provenance.signature_threshold exceeds trusted_signers"
            );
        }
        actual::RuntimeUpgradeProvenancePolicy {
            mode: self.mode.into(),
            require_sbom: self.require_sbom,
            require_slsa: self.require_slsa,
            trusted_signers,
            signature_threshold: self.signature_threshold,
        }
    }
}

impl Default for RuntimeUpgradeProvenance {
    fn default() -> Self {
        Self {
            mode: default_runtime_upgrade_provenance_mode(),
            require_sbom: defaults::governance::RUNTIME_UPGRADE_PROVENANCE_REQUIRE_SBOM,
            require_slsa: defaults::governance::RUNTIME_UPGRADE_PROVENANCE_REQUIRE_SLSA,
            signature_threshold:
                defaults::governance::RUNTIME_UPGRADE_PROVENANCE_SIGNATURE_THRESHOLD,
            trusted_signers: Vec::new(),
        }
    }
}

/// Governance configuration (user view).
#[derive(Debug, ReadConfig, Clone)]
pub struct Governance {
    /// Default ZK ballot verifying key (backend/name). Optional.
    pub vk_ballot: Option<VerifyingKeyRef>,
    /// Default ZK tally verifying key (backend/name). Optional.
    pub vk_tally: Option<VerifyingKeyRef>,
    /// Asset definition id used for governance bonds and voting locks.
    #[config(
        env = "GOV_VOTING_ASSET_ID",
        default = "defaults::governance::voting_asset_id()"
    )]
    pub voting_asset_id: String,
    /// Asset definition id used for citizenship bonds.
    #[config(
        env = "GOV_CITIZENSHIP_ASSET_ID",
        default = "defaults::governance::citizenship_asset_id()"
    )]
    pub citizenship_asset_id: String,
    /// Minimum amount (smallest units) required to register as a citizen.
    #[config(
        env = "GOV_CITIZENSHIP_BOND_AMOUNT",
        default = "defaults::governance::citizenship_bond_amount()"
    )]
    pub citizenship_bond_amount: u128,
    /// Escrow account that custodies citizenship bonds.
    #[config(
        env = "GOV_CITIZENSHIP_ESCROW_ACCOUNT",
        default = "defaults::governance::citizenship_escrow_account()"
    )]
    pub citizenship_escrow_account: String,
    /// Minimum amount (smallest units) required to submit a ballot.
    #[config(env = "GOV_MIN_BOND_AMOUNT", default = "150")]
    pub min_bond_amount: u128,
    /// Escrow account that custodians governance bonds (slashing/unlock path).
    #[config(
        env = "GOV_BOND_ESCROW_ACCOUNT",
        default = "defaults::governance::bond_escrow_account()"
    )]
    pub bond_escrow_account: String,
    /// Account that receives slashed governance bonds (may mirror escrow).
    #[config(
        env = "GOV_SLASH_RECEIVER_ACCOUNT",
        default = "defaults::governance::slash_receiver_account()"
    )]
    pub slash_receiver_account: String,
    /// Slash percentage for double-vote attempts (basis points, 0–10_000).
    #[config(
        env = "GOV_SLASH_DOUBLE_VOTE_BPS",
        default = "defaults::governance::slash_policy::DOUBLE_VOTE_BPS"
    )]
    pub slash_double_vote_bps: u16,
    /// Slash percentage applied when ballot proofs are invalid (basis points).
    #[config(
        env = "GOV_SLASH_INVALID_PROOF_BPS",
        default = "defaults::governance::slash_policy::MISCONDUCT_BPS"
    )]
    pub slash_invalid_proof_bps: u16,
    /// Slash percentage applied when eligibility proofs mismatch (basis points).
    #[config(
        env = "GOV_SLASH_INELIGIBLE_PROOF_BPS",
        default = "defaults::governance::slash_policy::INELIGIBLE_PROOF_BPS"
    )]
    pub slash_ineligible_proof_bps: u16,
    /// Minimum TEU balance required to accept alias attestations.
    #[config(
        env = "GOV_ALIAS_TEU_MINIMUM",
        default = "crate::parameters::defaults::governance::ALIAS_TEU_MINIMUM"
    )]
    pub alias_teu_minimum: u128,
    /// Emit alias frontier telemetry and stats.
    #[config(
        env = "GOV_ALIAS_FRONTIER_TELEMETRY",
        default = "crate::parameters::defaults::governance::ALIAS_FRONTIER_TELEMETRY"
    )]
    pub alias_frontier_telemetry: bool,
    /// Emit debug tracing for governance pipeline progression (dev/test only).
    #[config(
        env = "GOV_DEBUG_TRACE_PIPELINE",
        default = "crate::parameters::defaults::governance::DEBUG_TRACE_PIPELINE"
    )]
    pub debug_trace_pipeline: bool,
    /// Allowed JDG signature schemes for attestation validation.
    #[config(default = "defaults::governance::jdg_signature_schemes()")]
    pub jdg_signature_schemes: Vec<String>,
    /// Runtime upgrade provenance enforcement policy.
    #[config(nested)]
    pub runtime_upgrade_provenance: RuntimeUpgradeProvenance,
    /// Citizen service discipline knobs (cooldowns, slashing, seat caps).
    #[config(nested)]
    pub citizen_service: CitizenServiceDiscipline,
    /// Account supplying viral incentive payouts and sender bonuses.
    #[config(
        env = "GOV_VIRAL_INCENTIVE_POOL_ACCOUNT",
        default = "defaults::governance::viral_incentive_pool_account()"
    )]
    pub viral_incentive_pool_account: String,
    /// Account holding pending viral escrows for unbound handles.
    #[config(
        env = "GOV_VIRAL_ESCROW_ACCOUNT",
        default = "defaults::governance::viral_escrow_account()"
    )]
    pub viral_escrow_account: String,
    /// Asset definition used for viral rewards and escrows.
    #[config(
        env = "GOV_VIRAL_REWARD_ASSET_ID",
        default = "defaults::governance::viral_reward_asset_id()"
    )]
    pub viral_reward_asset_id: String,
    /// Amount paid for each valid binding claim.
    #[config(
        env = "GOV_VIRAL_FOLLOW_REWARD_AMOUNT",
        default = "defaults::governance::viral_follow_reward_amount()"
    )]
    pub viral_follow_reward_amount: Numeric,
    /// Sender bonus applied on first delivery.
    #[config(
        env = "GOV_VIRAL_SENDER_BONUS_AMOUNT",
        default = "defaults::governance::viral_sender_bonus_amount()"
    )]
    pub viral_sender_bonus_amount: Numeric,
    /// Maximum rewards a UAID may claim per day.
    #[config(
        env = "GOV_VIRAL_MAX_DAILY_CLAIMS_PER_UAID",
        default = "defaults::governance::VIRAL_MAX_DAILY_CLAIMS_PER_UAID"
    )]
    pub viral_max_daily_claims_per_uaid: u32,
    /// Maximum rewards allowed per binding (lifetime).
    #[config(
        env = "GOV_VIRAL_MAX_CLAIMS_PER_BINDING",
        default = "defaults::governance::VIRAL_MAX_CLAIMS_PER_BINDING"
    )]
    pub viral_max_claims_per_binding: u32,
    /// Daily reward budget (includes bonuses).
    #[config(
        env = "GOV_VIRAL_DAILY_BUDGET",
        default = "defaults::governance::viral_daily_budget()"
    )]
    pub viral_daily_budget: Numeric,
    /// Optional promotion start timestamp (ms since Unix epoch).
    #[config(env = "GOV_VIRAL_PROMO_START_MS")]
    pub viral_promo_start_ms: Option<u64>,
    /// Optional promotion end timestamp (ms since Unix epoch).
    #[config(env = "GOV_VIRAL_PROMO_END_MS")]
    pub viral_promo_end_ms: Option<u64>,
    /// Campaign-wide cap across the promotion window (0 = unlimited).
    #[config(
        env = "GOV_VIRAL_CAMPAIGN_CAP",
        default = "defaults::governance::viral_campaign_cap()"
    )]
    pub viral_campaign_cap: Numeric,
    /// Deny-list of UAIDs that cannot claim viral rewards.
    #[config(default = "Vec::new()")]
    pub viral_deny_uaids: Vec<String>,
    /// Deny-list of binding hashes that cannot claim viral rewards.
    #[config(default = "Vec::new()")]
    pub viral_deny_binding_hashes: Vec<String>,
    /// Halt switch for viral incentives (governance override).
    #[config(default = "false")]
    pub viral_halt: bool,
    /// SoraFS pin policy constraints enforced during manifest admission.
    #[config(nested)]
    pub sorafs_pin_policy: SorafsPinPolicyConstraints,
    /// SoraFS under-delivery penalty policy.
    #[config(nested)]
    pub sorafs_penalty: SorafsPenaltyPolicy,
    /// SoraFS repair escalation governance policy.
    #[config(nested)]
    pub sorafs_repair_escalation: RepairEscalationPolicyV1,
    /// SoraFS telemetry authentication/replay policy.
    #[config(nested)]
    pub sorafs_telemetry: SorafsTelemetryPolicy,
    /// Static SoraFS provider→owner bindings (hex provider id → account id).
    #[config(default = "BTreeMap::new()")]
    pub sorafs_provider_owners: BTreeMap<String, String>,
    /// Conviction step in blocks. duration/step increases conviction by 1.
    #[config(env = "GOV_CONVICTION_STEP_BLOCKS", default = "100")]
    pub conviction_step_blocks: u64,
    /// Maximum conviction multiplier.
    #[config(env = "GOV_MAX_CONVICTION", default = "6")]
    pub max_conviction: u64,
    /// Minimum enactment delay in blocks when scheduling a referendum.
    #[config(env = "GOV_MIN_ENACTMENT_DELAY", default = "20")]
    pub min_enactment_delay: u64,
    /// Referendum window span in blocks.
    #[config(env = "GOV_WINDOW_SPAN", default = "100")]
    pub window_span: u64,
    /// Enable non‑ZK quadratic voting globally (plain ballots)
    #[config(env = "GOV_PLAIN_VOTING_ENABLED", default = "false")]
    pub plain_voting_enabled: bool,
    /// Approval threshold numerator (approve/(approve+reject) >= num/den)
    #[config(env = "GOV_APPROVAL_Q_NUM", default = "1")]
    pub approval_threshold_q_num: u64,
    /// Approval threshold denominator
    #[config(env = "GOV_APPROVAL_Q_DEN", default = "2")]
    pub approval_threshold_q_den: u64,
    /// Minimum turnout required (approve+reject+abstain)
    #[config(env = "GOV_MIN_TURNOUT", default = "0")]
    pub min_turnout: u128,
    /// Size of the sortition council committee.
    #[config(
        env = "GOV_PARLIAMENT_COMMITTEE_SIZE",
        default = "crate::parameters::defaults::governance::PARLIAMENT_COMMITTEE_SIZE"
    )]
    pub parliament_committee_size: usize,
    /// Length of a council term in blocks.
    #[config(
        env = "GOV_PARLIAMENT_TERM_BLOCKS",
        default = "crate::parameters::defaults::governance::PARLIAMENT_TERM_BLOCKS"
    )]
    pub parliament_term_blocks: u64,
    /// Minimum required stake to qualify for sortition (in smallest units).
    #[config(
        env = "GOV_PARLIAMENT_MIN_STAKE",
        default = "crate::parameters::defaults::governance::PARLIAMENT_MIN_STAKE"
    )]
    pub parliament_min_stake: u128,
    /// Asset definition id that provides voting stake eligibility.
    #[config(
        env = "GOV_PARLIAMENT_ELIGIBILITY_ASSET_ID",
        default = "crate::parameters::defaults::governance::parliament_eligibility_asset_id()"
    )]
    pub parliament_eligibility_asset_id: String,
    /// Number of alternates to draw per term (None = committee size).
    #[config(env = "GOV_PARLIAMENT_ALTERNATE_SIZE")]
    pub parliament_alternate_size: Option<usize>,
    /// Council quorum requirement expressed in basis points (ceil-divided).
    #[config(
        env = "GOV_PARLIAMENT_QUORUM_BPS",
        default = "crate::parameters::defaults::governance::PARLIAMENT_QUORUM_BPS"
    )]
    pub parliament_quorum_bps: u16,
    /// Rules Committee size.
    #[config(
        env = "GOV_RULES_COMMITTEE_SIZE",
        default = "crate::parameters::defaults::governance::PARLIAMENT_RULES_COMMITTEE_SIZE"
    )]
    pub rules_committee_size: usize,
    /// Agenda Council size.
    #[config(
        env = "GOV_AGENDA_COUNCIL_SIZE",
        default = "crate::parameters::defaults::governance::PARLIAMENT_AGENDA_COUNCIL_SIZE"
    )]
    pub agenda_council_size: usize,
    /// Interest Panel size.
    #[config(
        env = "GOV_INTEREST_PANEL_SIZE",
        default = "crate::parameters::defaults::governance::PARLIAMENT_INTEREST_PANEL_SIZE"
    )]
    pub interest_panel_size: usize,
    /// Review Panel size.
    #[config(
        env = "GOV_REVIEW_PANEL_SIZE",
        default = "crate::parameters::defaults::governance::PARLIAMENT_REVIEW_PANEL_SIZE"
    )]
    pub review_panel_size: usize,
    /// Policy Jury size.
    #[config(
        env = "GOV_POLICY_JURY_SIZE",
        default = "crate::parameters::defaults::governance::PARLIAMENT_POLICY_JURY_SIZE"
    )]
    pub policy_jury_size: usize,
    /// Oversight Committee size.
    #[config(
        env = "GOV_OVERSIGHT_COMMITTEE_SIZE",
        default = "crate::parameters::defaults::governance::PARLIAMENT_OVERSIGHT_COMMITTEE_SIZE"
    )]
    pub oversight_committee_size: usize,
    /// MPC/FMA board size.
    #[config(
        env = "GOV_FMA_COMMITTEE_SIZE",
        default = "crate::parameters::defaults::governance::PARLIAMENT_FMA_COMMITTEE_SIZE"
    )]
    pub fma_committee_size: usize,
    /// SLA (blocks) from proposal submission to referendum opening.
    #[config(env = "GOV_PIPELINE_STUDY_SLA_BLOCKS")]
    pub pipeline_study_sla_blocks: Option<u64>,
    /// SLA (blocks) allotted to the referendum voting window.
    #[config(env = "GOV_PIPELINE_REVIEW_SLA_BLOCKS")]
    pub pipeline_review_sla_blocks: Option<u64>,
    /// SLA (blocks) allotted to record the referendum decision.
    #[config(env = "GOV_PIPELINE_DECISION_SLA_BLOCKS")]
    pub pipeline_decision_sla_blocks: Option<u64>,
    /// SLA (blocks) to enact an approved proposal after decision.
    #[config(env = "GOV_PIPELINE_ENACTMENT_SLA_BLOCKS")]
    pub pipeline_enactment_sla_blocks: Option<u64>,
    /// SLA (blocks) for rules committee approvals.
    #[config(env = "GOV_PIPELINE_RULES_SLA_BLOCKS")]
    pub pipeline_rules_sla_blocks: Option<u64>,
    /// SLA (blocks) for agenda council scheduling.
    #[config(env = "GOV_PIPELINE_AGENDA_SLA_BLOCKS")]
    pub pipeline_agenda_sla_blocks: Option<u64>,
}

impl Default for Governance {
    fn default() -> Self {
        Self {
            vk_ballot: None,
            vk_tally: None,
            voting_asset_id: defaults::governance::voting_asset_id(),
            citizenship_asset_id: defaults::governance::citizenship_asset_id(),
            citizenship_bond_amount: defaults::governance::citizenship_bond_amount(),
            citizenship_escrow_account: defaults::governance::citizenship_escrow_account(),
            min_bond_amount: 150,
            bond_escrow_account: defaults::governance::bond_escrow_account(),
            slash_receiver_account: defaults::governance::slash_receiver_account(),
            slash_double_vote_bps: defaults::governance::slash_policy::DOUBLE_VOTE_BPS,
            slash_invalid_proof_bps: defaults::governance::slash_policy::MISCONDUCT_BPS,
            slash_ineligible_proof_bps: defaults::governance::slash_policy::INELIGIBLE_PROOF_BPS,
            alias_teu_minimum: defaults::governance::ALIAS_TEU_MINIMUM,
            alias_frontier_telemetry: defaults::governance::ALIAS_FRONTIER_TELEMETRY,
            debug_trace_pipeline: defaults::governance::DEBUG_TRACE_PIPELINE,
            jdg_signature_schemes: defaults::governance::jdg_signature_schemes(),
            runtime_upgrade_provenance: RuntimeUpgradeProvenance::default(),
            citizen_service: CitizenServiceDiscipline::default(),
            viral_incentive_pool_account: defaults::governance::viral_incentive_pool_account(),
            viral_escrow_account: defaults::governance::viral_escrow_account(),
            viral_reward_asset_id: defaults::governance::viral_reward_asset_id(),
            viral_follow_reward_amount: defaults::governance::viral_follow_reward_amount(),
            viral_sender_bonus_amount: defaults::governance::viral_sender_bonus_amount(),
            viral_max_daily_claims_per_uaid: defaults::governance::VIRAL_MAX_DAILY_CLAIMS_PER_UAID,
            viral_max_claims_per_binding: defaults::governance::VIRAL_MAX_CLAIMS_PER_BINDING,
            viral_daily_budget: defaults::governance::viral_daily_budget(),
            viral_promo_start_ms: defaults::governance::viral_promo_start_ms(),
            viral_promo_end_ms: defaults::governance::viral_promo_end_ms(),
            viral_campaign_cap: defaults::governance::viral_campaign_cap(),
            viral_deny_uaids: Vec::new(),
            viral_deny_binding_hashes: Vec::new(),
            viral_halt: false,
            sorafs_pin_policy: SorafsPinPolicyConstraints::default(),
            sorafs_penalty: SorafsPenaltyPolicy::default(),
            sorafs_repair_escalation: RepairEscalationPolicyV1::default(),
            sorafs_telemetry: SorafsTelemetryPolicy::default(),
            sorafs_provider_owners: BTreeMap::new(),
            conviction_step_blocks: 100,
            max_conviction: 6,
            min_enactment_delay: 20,
            window_span: 100,
            plain_voting_enabled: false,
            approval_threshold_q_num: 1,
            approval_threshold_q_den: 2,
            min_turnout: 0,
            parliament_committee_size: defaults::governance::PARLIAMENT_COMMITTEE_SIZE,
            parliament_term_blocks: defaults::governance::PARLIAMENT_TERM_BLOCKS,
            parliament_min_stake: defaults::governance::PARLIAMENT_MIN_STAKE,
            parliament_eligibility_asset_id: defaults::governance::parliament_eligibility_asset_id(
            ),
            parliament_alternate_size: defaults::governance::PARLIAMENT_ALTERNATE_SIZE,
            parliament_quorum_bps: defaults::governance::PARLIAMENT_QUORUM_BPS,
            rules_committee_size: defaults::governance::PARLIAMENT_RULES_COMMITTEE_SIZE,
            agenda_council_size: defaults::governance::PARLIAMENT_AGENDA_COUNCIL_SIZE,
            interest_panel_size: defaults::governance::PARLIAMENT_INTEREST_PANEL_SIZE,
            review_panel_size: defaults::governance::PARLIAMENT_REVIEW_PANEL_SIZE,
            policy_jury_size: defaults::governance::PARLIAMENT_POLICY_JURY_SIZE,
            oversight_committee_size: defaults::governance::PARLIAMENT_OVERSIGHT_COMMITTEE_SIZE,
            fma_committee_size: defaults::governance::PARLIAMENT_FMA_COMMITTEE_SIZE,
            pipeline_study_sla_blocks: None,
            pipeline_review_sla_blocks: None,
            pipeline_decision_sla_blocks: None,
            pipeline_enactment_sla_blocks: None,
            pipeline_rules_sla_blocks: None,
            pipeline_agenda_sla_blocks: None,
        }
    }
}

impl Governance {
    /// Convert user-supplied governance settings into the runtime representation.
    pub fn parse(self) -> actual::Governance {
        let citizen_service = self.citizen_service.parse();
        citizen_service.assert_valid();
        assert!(
            (1..=10_000).contains(&self.parliament_quorum_bps),
            "parliament_quorum_bps must be within 1..=10_000 (basis points)"
        );
        let viral_incentives = actual::ViralIncentives {
            incentive_pool_account: parse_account_id_literal(
                &self.viral_incentive_pool_account,
                "invalid viral incentive pool account id",
            ),
            escrow_account: parse_account_id_literal(
                &self.viral_escrow_account,
                "invalid viral escrow account id",
            ),
            reward_asset_definition_id: self
                .viral_reward_asset_id
                .parse()
                .expect("invalid viral reward asset id"),
            follow_reward_amount: self.viral_follow_reward_amount,
            sender_bonus_amount: self.viral_sender_bonus_amount,
            max_daily_claims_per_uaid: self.viral_max_daily_claims_per_uaid,
            max_claims_per_binding: self.viral_max_claims_per_binding,
            daily_budget: self.viral_daily_budget,
            halt: self.viral_halt,
            promo_starts_at_ms: self.viral_promo_start_ms,
            promo_ends_at_ms: self.viral_promo_end_ms,
            campaign_cap: self.viral_campaign_cap,
            deny_uaids: self
                .viral_deny_uaids
                .iter()
                .map(|uaid| uaid.parse().expect("invalid viral deny UAID"))
                .collect(),
            deny_binding_digests: self
                .viral_deny_binding_hashes
                .iter()
                .map(|hash| Hash::from_str(hash).expect("invalid viral deny binding digest"))
                .collect(),
        };
        if let (Some(start), Some(end)) = (
            viral_incentives.promo_starts_at_ms,
            viral_incentives.promo_ends_at_ms,
        ) {
            assert!(
                start < end,
                "viral promotion start must precede end (start={start}, end={end})"
            );
        }
        let jdg_signature_schemes = normalize_jdg_signature_schemes(self.jdg_signature_schemes);
        let runtime_upgrade_provenance = self.runtime_upgrade_provenance.parse();
        actual::Governance {
            vk_ballot: self.vk_ballot.map(VerifyingKeyRef::parse),
            vk_tally: self.vk_tally.map(VerifyingKeyRef::parse),
            voting_asset_id: self
                .voting_asset_id
                .parse()
                .expect("invalid governance voting asset id"),
            citizenship_asset_id: self
                .citizenship_asset_id
                .parse()
                .expect("invalid citizenship asset id"),
            citizenship_bond_amount: self.citizenship_bond_amount,
            citizenship_escrow_account: parse_account_id_literal(
                &self.citizenship_escrow_account,
                "invalid citizenship escrow account id",
            ),
            min_bond_amount: self.min_bond_amount,
            bond_escrow_account: parse_account_id_literal(
                &self.bond_escrow_account,
                "invalid governance bond escrow account id",
            ),
            slash_receiver_account: parse_account_id_literal(
                &self.slash_receiver_account,
                "invalid governance slash receiver account id",
            ),
            slash_double_vote_bps: self.slash_double_vote_bps,
            slash_invalid_proof_bps: self.slash_invalid_proof_bps,
            slash_ineligible_proof_bps: self.slash_ineligible_proof_bps,
            alias_teu_minimum: self.alias_teu_minimum,
            alias_frontier_telemetry: self.alias_frontier_telemetry,
            debug_trace_pipeline: self.debug_trace_pipeline,
            jdg_signature_schemes,
            runtime_upgrade_provenance,
            citizen_service,
            viral_incentives,
            sorafs_pin_policy: self.sorafs_pin_policy.parse(),
            sorafs_pricing: PricingScheduleRecord::launch_default(),
            sorafs_penalty: self.sorafs_penalty.parse(),
            sorafs_repair_escalation: self.sorafs_repair_escalation.parse(),
            sorafs_telemetry: self.sorafs_telemetry.parse(),
            sorafs_provider_owners: {
                let mut bindings = BTreeMap::new();
                for (provider, owner) in self.sorafs_provider_owners {
                    let trimmed = provider.trim_start_matches("0x");
                    let bytes = hex::decode(trimmed).unwrap_or_else(|err| {
                        panic!("invalid SoraFS provider id {provider}: {err}");
                    });
                    let array: [u8; 32] = bytes.try_into().unwrap_or_else(|_| {
                        panic!("SoraFS provider id {provider} must be 32 bytes")
                    });
                    let provider_id = ProviderId::new(array);
                    let owner_id = parse_account_id_literal(
                        &owner,
                        &format!("invalid SoraFS provider owner {owner}"),
                    );
                    if bindings.insert(provider_id, owner_id).is_some() {
                        panic!("duplicate SoraFS provider binding for {provider}");
                    }
                }
                bindings
            },
            conviction_step_blocks: self.conviction_step_blocks,
            max_conviction: self.max_conviction,
            min_enactment_delay: self.min_enactment_delay,
            window_span: self.window_span,
            plain_voting_enabled: self.plain_voting_enabled,
            approval_threshold_q_num: self.approval_threshold_q_num,
            approval_threshold_q_den: self.approval_threshold_q_den,
            min_turnout: self.min_turnout,
            parliament_committee_size: self.parliament_committee_size,
            parliament_term_blocks: self.parliament_term_blocks,
            parliament_min_stake: self.parliament_min_stake,
            parliament_eligibility_asset_id: self
                .parliament_eligibility_asset_id
                .parse()
                .expect("invalid parliament eligibility asset id"),
            parliament_alternate_size: self.parliament_alternate_size,
            parliament_quorum_bps: self.parliament_quorum_bps,
            rules_committee_size: self.rules_committee_size,
            agenda_council_size: self.agenda_council_size,
            interest_panel_size: self.interest_panel_size,
            review_panel_size: self.review_panel_size,
            policy_jury_size: self.policy_jury_size,
            oversight_committee_size: self.oversight_committee_size,
            fma_committee_size: self.fma_committee_size,
            pipeline_study_sla_blocks: self
                .pipeline_study_sla_blocks
                .unwrap_or(self.min_enactment_delay),
            pipeline_review_sla_blocks: self.pipeline_review_sla_blocks.unwrap_or(self.window_span),
            pipeline_decision_sla_blocks: self.pipeline_decision_sla_blocks.unwrap_or(1),
            pipeline_enactment_sla_blocks: self
                .pipeline_enactment_sla_blocks
                .unwrap_or(self.window_span.saturating_mul(2)),
            pipeline_rules_sla_blocks: self
                .pipeline_rules_sla_blocks
                .unwrap_or(defaults::governance::PIPELINE_RULES_SLA_BLOCKS),
            pipeline_agenda_sla_blocks: self
                .pipeline_agenda_sla_blocks
                .unwrap_or(defaults::governance::PIPELINE_AGENDA_SLA_BLOCKS),
        }
    }
}

#[cfg(test)]
mod governance_tests {
    use super::*;

    #[test]
    fn debug_trace_pipeline_defaults_false() {
        let cfg = Governance::default();
        assert!(!cfg.debug_trace_pipeline);
        let parsed = cfg.parse();
        assert!(!parsed.debug_trace_pipeline);
    }

    #[test]
    fn debug_trace_pipeline_roundtrips() {
        let mut cfg = Governance::default();
        cfg.debug_trace_pipeline = true;
        let parsed = cfg.parse();
        assert!(parsed.debug_trace_pipeline);
    }

    #[test]
    fn jdg_signature_schemes_defaults_to_simple_threshold() {
        let cfg = Governance::default();
        let parsed = cfg.parse();
        assert!(
            parsed
                .jdg_signature_schemes
                .contains(&JdgSignatureScheme::SimpleThreshold)
        );
    }

    #[test]
    fn jdg_signature_schemes_accepts_aliases() {
        let mut cfg = Governance::default();
        cfg.jdg_signature_schemes = vec!["simple".to_string(), "bls-aggregate".to_string()];
        let parsed = cfg.parse();
        assert!(
            parsed
                .jdg_signature_schemes
                .contains(&JdgSignatureScheme::SimpleThreshold)
        );
        assert!(
            parsed
                .jdg_signature_schemes
                .contains(&JdgSignatureScheme::BlsNormalAggregate)
        );
    }
}

/// User-level configuration container for `Concurrency`.
#[derive(Debug, ReadConfig, Clone, Copy)]
#[allow(clippy::struct_field_names)]
pub struct Concurrency {
    /// Minimum scheduler worker threads (0 = auto/physical cores)
    #[config(
        env = "CONCURRENCY_SCHEDULER_MIN",
        default = "defaults::concurrency::SCHEDULER_MIN"
    )]
    pub scheduler_min_threads: usize,
    /// Maximum scheduler worker threads (0 = auto/physical cores)
    #[config(
        env = "CONCURRENCY_SCHEDULER_MAX",
        default = "defaults::concurrency::SCHEDULER_MAX"
    )]
    pub scheduler_max_threads: usize,
    /// Global Rayon thread pool size (0 = auto/physical cores)
    #[config(
        env = "CONCURRENCY_RAYON_GLOBAL",
        default = "defaults::concurrency::RAYON_GLOBAL"
    )]
    pub rayon_global_threads: usize,
    /// Stack size (bytes) for scheduler worker threads.
    #[config(
        env = "CONCURRENCY_SCHEDULER_STACK_BYTES",
        default = "defaults::concurrency::SCHEDULER_STACK_BYTES"
    )]
    pub scheduler_stack_bytes: usize,
    /// Stack size (bytes) for prover worker threads.
    #[config(
        env = "CONCURRENCY_PROVER_STACK_BYTES",
        default = "defaults::concurrency::PROVER_STACK_BYTES"
    )]
    pub prover_stack_bytes: usize,
    /// Guest stack size (bytes) for IVM instances.
    #[config(
        env = "CONCURRENCY_GUEST_STACK_BYTES",
        default = "defaults::concurrency::GUEST_STACK_BYTES"
    )]
    pub guest_stack_bytes: u64,
    /// Gas→stack multiplier (bytes of stack available per unit of gas).
    #[config(
        env = "CONCURRENCY_GAS_TO_STACK_MULTIPLIER",
        default = "defaults::concurrency::GAS_TO_STACK_MULTIPLIER"
    )]
    pub gas_to_stack_multiplier: u64,
}

impl Concurrency {
    /// Convert this user configuration into the runtime representation.
    pub fn parse(self) -> actual::Concurrency {
        actual::Concurrency {
            scheduler_min_threads: self.scheduler_min_threads,
            scheduler_max_threads: self.scheduler_max_threads,
            rayon_global_threads: self.rayon_global_threads,
            scheduler_stack_bytes: self.scheduler_stack_bytes,
            prover_stack_bytes: self.prover_stack_bytes,
            guest_stack_bytes: self.guest_stack_bytes,
            gas_to_stack_multiplier: self.gas_to_stack_multiplier,
        }
    }
}

/// Network Time Service enforcement modes (user view).
#[derive(Debug, Clone, Copy, PartialEq, Eq, strum::EnumString, strum::Display)]
#[strum(serialize_all = "snake_case")]
pub enum NtsEnforcementMode {
    /// Log unhealthy NTS status but accept time-sensitive transactions.
    Warn,
    /// Reject time-sensitive transactions when NTS is unhealthy.
    Reject,
}

impl NtsEnforcementMode {
    fn into_actual(self) -> actual::NtsEnforcementMode {
        match self {
            Self::Warn => actual::NtsEnforcementMode::Warn,
            Self::Reject => actual::NtsEnforcementMode::Reject,
        }
    }
}

impl json::JsonSerialize for NtsEnforcementMode {
    fn json_serialize(&self, out: &mut String) {
        json::write_json_string(&self.to_string(), out);
    }
}

impl json::JsonDeserialize for NtsEnforcementMode {
    fn json_deserialize(
        parser: &mut json::Parser<'_>,
    ) -> ::core::result::Result<Self, json::Error> {
        let text = parser.parse_string()?;
        Self::from_str(&text).map_err(|err| json::Error::InvalidField {
            field: "nts.enforcement_mode".into(),
            message: err.to_string(),
        })
    }
}

/// Network Time Service (user view).
/// User-level configuration container for `Nts`.
#[derive(Debug, ReadConfig, Clone, Copy)]
pub struct Nts {
    /// Sampling interval for peer time probes (ms, clamped to >= 100ms).
    #[config(default = "defaults::time::NTS_SAMPLE_INTERVAL.into()")]
    pub sample_interval_ms: DurationMs,
    /// Maximum peers to sample per round.
    #[config(
        env = "NTS_SAMPLE_CAP_PER_ROUND",
        default = "defaults::time::NTS_SAMPLE_CAP_PER_ROUND"
    )]
    pub sample_cap_per_round: usize,
    /// Maximum acceptable round-trip time (milliseconds) for samples.
    #[config(env = "NTS_MAX_RTT_MS", default = "defaults::time::NTS_MAX_RTT_MS")]
    pub max_rtt_ms: u64,
    /// Trim percent for median aggregation (0–45 allowed; 10 typical).
    #[config(env = "NTS_TRIM_PERCENT", default = "defaults::time::NTS_TRIM_PERCENT")]
    pub trim_percent: u8,
    /// Per-peer ring buffer capacity for samples.
    #[config(
        env = "NTS_PER_PEER_BUFFER",
        default = "defaults::time::NTS_PER_PEER_BUFFER"
    )]
    pub per_peer_buffer: usize,
    /// Enable EMA smoothing of network offset.
    #[config(
        env = "NTS_SMOOTHING_ENABLED",
        default = "defaults::time::NTS_SMOOTHING_ENABLED"
    )]
    pub smoothing_enabled: bool,
    /// EMA alpha in [0,1]; higher means more responsive.
    #[config(
        env = "NTS_SMOOTHING_ALPHA",
        default = "defaults::time::NTS_SMOOTHING_ALPHA"
    )]
    pub smoothing_alpha: f64,
    /// Maximum allowed adjustment per minute (ms) when smoothing.
    #[config(
        env = "NTS_MAX_ADJUST_MS_PER_MIN",
        default = "defaults::time::NTS_MAX_ADJUST_MS_PER_MIN"
    )]
    pub max_adjust_ms_per_min: u64,
    /// Minimum number of peer samples required before NTS is considered healthy.
    #[config(env = "NTS_MIN_SAMPLES", default = "defaults::time::NTS_MIN_SAMPLES")]
    pub min_samples: usize,
    /// Maximum absolute offset (ms) before NTS is considered unhealthy (0 disables).
    #[config(
        env = "NTS_MAX_OFFSET_MS",
        default = "defaults::time::NTS_MAX_OFFSET_MS"
    )]
    pub max_offset_ms: u64,
    /// Maximum confidence (MAD) in ms before NTS is considered unhealthy (0 disables).
    #[config(
        env = "NTS_MAX_CONFIDENCE_MS",
        default = "defaults::time::NTS_MAX_CONFIDENCE_MS"
    )]
    pub max_confidence_ms: u64,
    /// Enforcement mode for unhealthy NTS (warn/reject).
    #[config(
        env = "NTS_ENFORCEMENT_MODE",
        default = "defaults::time::NTS_ENFORCEMENT_MODE.parse().unwrap()"
    )]
    pub enforcement_mode: NtsEnforcementMode,
}

/// Hardware acceleration settings for IVM (user view).
/// User-level configuration container for `Acceleration`.
#[derive(Debug, ReadConfig, Clone, Copy)]
pub struct Acceleration {
    /// Enable SIMD acceleration (NEON/AVX/SSE) when available.
    #[config(env = "ACCEL_ENABLE_SIMD", default = "defaults::accel::ENABLE_SIMD")]
    pub enable_simd: bool,
    /// Enable CUDA backend when compiled and available.
    #[config(env = "ACCEL_ENABLE_CUDA", default = "defaults::accel::ENABLE_CUDA")]
    pub enable_cuda: bool,
    /// Enable Metal backend when compiled and available (macOS).
    #[config(env = "ACCEL_ENABLE_METAL", default = "defaults::accel::ENABLE_METAL")]
    pub enable_metal: bool,
    /// Maximum number of GPUs to initialize (0 = auto/no cap).
    #[config(env = "ACCEL_MAX_GPUS", default = "defaults::accel::MAX_GPUS")]
    pub max_gpus: usize,
    /// Minimum number of leaves to use GPU for Merkle leaf hashing (0 = auto default).
    #[config(
        env = "ACCEL_MERKLE_MIN_LEAVES_GPU",
        default = "defaults::accel::MERKLE_MIN_LEAVES_GPU"
    )]
    pub merkle_min_leaves_gpu: usize,
    /// Override for Metal backend minimum Merkle leaf count (0 inherits GPU default).
    #[config(env = "ACCEL_MERKLE_MIN_LEAVES_METAL", default = "0")]
    pub merkle_min_leaves_metal: usize,
    /// Override for CUDA backend minimum Merkle leaf count (0 inherits GPU default).
    #[config(env = "ACCEL_MERKLE_MIN_LEAVES_CUDA", default = "0")]
    pub merkle_min_leaves_cuda: usize,
    /// Prefer CPU SHA2 for trees up to this many leaves (per-arch). 0 = keep defaults.
    #[config(env = "ACCEL_PREFER_CPU_SHA2_MAX_AARCH64", default = "0")]
    pub prefer_cpu_sha2_max_leaves_aarch64: usize,
    /// Prefer CPU SHA2 for trees up to this many leaves on x86 hosts (0 keeps defaults).
    #[config(env = "ACCEL_PREFER_CPU_SHA2_MAX_X86", default = "0")]
    pub prefer_cpu_sha2_max_leaves_x86: usize,
}

impl Acceleration {
    fn parse(self) -> actual::Acceleration {
        actual::Acceleration {
            enable_simd: self.enable_simd,
            enable_cuda: self.enable_cuda,
            enable_metal: self.enable_metal,
            max_gpus: if self.max_gpus == 0 {
                None
            } else {
                Some(self.max_gpus)
            },
            merkle_min_leaves_gpu: if self.merkle_min_leaves_gpu == 0 {
                defaults::accel::MERKLE_MIN_LEAVES_GPU
            } else {
                self.merkle_min_leaves_gpu
            },
            merkle_min_leaves_metal: if self.merkle_min_leaves_metal == 0 {
                None
            } else {
                Some(self.merkle_min_leaves_metal)
            },
            merkle_min_leaves_cuda: if self.merkle_min_leaves_cuda == 0 {
                None
            } else {
                Some(self.merkle_min_leaves_cuda)
            },
            prefer_cpu_sha2_max_leaves_aarch64: if self.prefer_cpu_sha2_max_leaves_aarch64 == 0 {
                None
            } else {
                Some(self.prefer_cpu_sha2_max_leaves_aarch64)
            },
            prefer_cpu_sha2_max_leaves_x86: if self.prefer_cpu_sha2_max_leaves_x86 == 0 {
                None
            } else {
                Some(self.prefer_cpu_sha2_max_leaves_x86)
            },
        }
    }
}

impl Nts {
    fn parse(self) -> actual::Nts {
        actual::Nts {
            sample_interval: self.sample_interval_ms.get().max(MIN_TIMER_INTERVAL),
            sample_cap_per_round: self.sample_cap_per_round,
            max_rtt_ms: self.max_rtt_ms,
            trim_percent: self.trim_percent,
            per_peer_buffer: self.per_peer_buffer,
            smoothing_enabled: self.smoothing_enabled,
            smoothing_alpha: self.smoothing_alpha,
            max_adjust_ms_per_min: self.max_adjust_ms_per_min,
            min_samples: self.min_samples,
            max_offset_ms: self.max_offset_ms,
            max_confidence_ms: self.max_confidence_ms,
            enforcement_mode: self.enforcement_mode.into_actual(),
        }
    }
}

/// User-level configuration for admitting `Executable::IvmProved` (proof-carrying IVM overlays).
#[derive(Debug, ReadConfig, Clone)]
pub struct IvmProvedExecution {
    /// Whether `Executable::IvmProved` is accepted by the execution pipeline.
    ///
    /// Default is `false` until a full end-to-end IVM execution proof system is shipped.
    #[config(default = "defaults::pipeline::ivm_proved::ENABLED")]
    pub enabled: bool,
    /// Skip deterministic IVM replay for circuits that are known to prove full IVM execution semantics.
    #[config(default = "defaults::pipeline::ivm_proved::SKIP_REPLAY")]
    pub skip_replay: bool,
    /// Allowlist of circuit IDs accepted for `Executable::IvmProved`.
    ///
    /// An empty allowlist rejects all proved executions, even when `enabled = true`.
    #[config(default = "Vec::new()")]
    pub allowed_circuits: Vec<String>,
}

/// User-level configuration container for `Pipeline`.
#[derive(Debug, ReadConfig)]
#[allow(clippy::struct_excessive_bools)]
pub struct Pipeline {
    /// Enable dynamic prepass for IVM access-set derivation.
    #[config(
        env = "PIPELINE_DYNAMIC_PREPASS",
        default = "defaults::pipeline::DYNAMIC_PREPASS"
    )]
    pub dynamic_prepass: bool,
    /// Enable caching of derived access sets for IVM manifest hints.
    #[config(
        env = "PIPELINE_ACCESS_SET_CACHE_ENABLED",
        default = "defaults::pipeline::ACCESS_SET_CACHE_ENABLED"
    )]
    pub access_set_cache_enabled: bool,
    /// Enable parallel per-transaction overlay construction.
    #[config(
        env = "PIPELINE_PARALLEL_OVERLAY",
        default = "defaults::pipeline::PARALLEL_OVERLAY"
    )]
    pub parallel_overlay: bool,
    /// Number of worker threads to use for overlay construction (0 = auto).
    #[config(env = "PIPELINE_WORKERS", default = "defaults::pipeline::WORKERS")]
    pub workers: usize,
    /// Capacity for the stateless validation cache (0 = disabled).
    #[config(
        env = "PIPELINE_STATELESS_CACHE_CAP",
        default = "defaults::pipeline::STATELESS_CACHE_CAP"
    )]
    pub stateless_cache_cap: usize,
    /// Enable parallel application of overlays (per conflict-free layer).
    #[config(
        env = "PIPELINE_PARALLEL_APPLY",
        default = "defaults::pipeline::PARALLEL_APPLY"
    )]
    pub parallel_apply: bool,
    /// Use BinaryHeap-based ready queue in the deterministic scheduler (dev/bench only).
    #[config(
        env = "PIPELINE_READY_QUEUE_HEAP",
        default = "defaults::pipeline::READY_QUEUE_HEAP"
    )]
    pub ready_queue_heap: bool,
    /// Optional GPU key bucketing (stable radix on (key, tx_idx, rw_flag)).
    /// Deterministic CPU fallback is always available. Off by default.
    #[config(
        env = "PIPELINE_GPU_KEY_BUCKET",
        default = "defaults::pipeline::GPU_KEY_BUCKET"
    )]
    pub gpu_key_bucket: bool,
    /// Emit scheduler input/output traces for deterministic tie-break debugging (dev/testing only).
    #[config(
        env = "PIPELINE_DEBUG_TRACE_SCHEDULER_INPUTS",
        default = "defaults::pipeline::DEBUG_TRACE_SCHEDULER_INPUTS"
    )]
    pub debug_trace_scheduler_inputs: bool,
    /// Emit transaction evaluation traces during overlay application (dev/testing only).
    #[config(
        env = "PIPELINE_DEBUG_TRACE_TX_EVAL",
        default = "defaults::pipeline::DEBUG_TRACE_TX_EVAL"
    )]
    pub debug_trace_tx_eval: bool,
    /// Maximum size for deterministic signature micro-batches (0 disables; historical alias for Ed25519).
    #[config(
        env = "PIPELINE_SIGNATURE_BATCH_MAX",
        default = "defaults::pipeline::SIGNATURE_BATCH_MAX"
    )]
    pub signature_batch_max: usize,
    /// Per-scheme caps (0 disables) for signature batch verification.
    #[config(
        env = "PIPELINE_SIGNATURE_BATCH_MAX_ED25519",
        default = "defaults::pipeline::SIGNATURE_BATCH_MAX_ED25519"
    )]
    pub signature_batch_max_ed25519: usize,
    /// Maximum batch size for secp256k1 signature verification.
    #[config(
        env = "PIPELINE_SIGNATURE_BATCH_MAX_SECP256K1",
        default = "defaults::pipeline::SIGNATURE_BATCH_MAX_SECP256K1"
    )]
    pub signature_batch_max_secp256k1: usize,
    /// Maximum batch size for PQC signature verification.
    #[config(
        env = "PIPELINE_SIGNATURE_BATCH_MAX_PQC",
        default = "defaults::pipeline::SIGNATURE_BATCH_MAX_PQC"
    )]
    pub signature_batch_max_pqc: usize,
    /// Maximum batch size for BLS signature verification.
    #[config(
        env = "PIPELINE_SIGNATURE_BATCH_MAX_BLS",
        default = "defaults::pipeline::SIGNATURE_BATCH_MAX_BLS"
    )]
    pub signature_batch_max_bls: usize,
    /// IVM pre-decode cache capacity (decoded streams kept in memory).
    #[config(
        env = "PIPELINE_CACHE_SIZE",
        default = "defaults::pipeline::CACHE_SIZE"
    )]
    pub cache_size: usize,
    /// Maximum decoded instructions retained per cached entry (0 = unlimited).
    #[config(
        env = "PIPELINE_IVM_CACHE_MAX_DECODED_OPS",
        default = "defaults::pipeline::IVM_CACHE_MAX_DECODED_OPS"
    )]
    pub ivm_cache_max_decoded_ops: usize,
    /// Approximate byte budget for cached pre-decode entries (bytes, 0 = unlimited).
    #[config(
        env = "PIPELINE_IVM_CACHE_MAX_BYTES",
        default = "defaults::pipeline::IVM_CACHE_MAX_BYTES"
    )]
    pub ivm_cache_max_bytes: usize,
    /// Rayon worker cap for prover/trace verification (0 = number of physical cores).
    #[config(
        env = "PIPELINE_IVM_PROVER_THREADS",
        default = "defaults::pipeline::IVM_PROVER_THREADS"
    )]
    pub ivm_prover_threads: usize,
    /// Maximum instructions allowed per overlay (0 = unlimited).
    #[config(
        env = "PIPELINE_OVERLAY_MAX_INSTRUCTIONS",
        default = "defaults::pipeline::OVERLAY_MAX_INSTRUCTIONS"
    )]
    pub overlay_max_instructions: usize,
    /// Maximum serialized Norito bytes allowed per overlay (0 = unlimited).
    #[config(
        env = "PIPELINE_OVERLAY_MAX_BYTES",
        default = "defaults::pipeline::OVERLAY_MAX_BYTES"
    )]
    pub overlay_max_bytes: u64,
    /// Execute overlay instructions in chunks of this size (at least 1).
    #[config(
        env = "PIPELINE_OVERLAY_CHUNK_INSTRUCTIONS",
        default = "defaults::pipeline::OVERLAY_CHUNK_INSTRUCTIONS"
    )]
    pub overlay_chunk_instructions: usize,
    /// `Executable::IvmProved` admission/verification settings.
    #[config(nested)]
    pub ivm_proved: IvmProvedExecution,
    /// Gas-fee configuration (accepted assets, conversion, and tech account).
    #[config(nested)]
    pub gas: Gas,
    /// Admission-time ceiling for `ProgramMetadata.max_cycles` (0 disables the check).
    #[config(
        env = "PIPELINE_IVM_MAX_CYCLES_UPPER_BOUND",
        default = "defaults::pipeline::IVM_MAX_CYCLES_UPPER_BOUND"
    )]
    pub ivm_max_cycles_upper_bound: u64,
    /// Maximum decoded Kotodama instructions accepted during admission (0 = unlimited).
    #[config(
        env = "PIPELINE_IVM_MAX_DECODED_INSTRUCTIONS",
        default = "defaults::pipeline::IVM_MAX_DECODED_INSTRUCTIONS"
    )]
    pub ivm_max_decoded_instructions: u64,
    /// Maximum decoded Kotodama byte length accepted during admission (0 = unlimited).
    #[config(
        env = "PIPELINE_IVM_MAX_DECODED_BYTES",
        default = "defaults::pipeline::IVM_MAX_DECODED_BYTES"
    )]
    pub ivm_max_decoded_bytes: u64,
    /// Maximum transactions allowed in the quarantine lane per block (0 = disabled).
    #[config(
        env = "PIPELINE_QUARANTINE_MAX_TXS_PER_BLOCK",
        default = "defaults::pipeline::QUARANTINE_MAX_TXS_PER_BLOCK"
    )]
    pub quarantine_max_txs_per_block: usize,
    /// Per-transaction cycle cap in the quarantine lane (0 = unlimited).
    #[config(
        env = "PIPELINE_QUARANTINE_TX_MAX_CYCLES",
        default = "defaults::pipeline::QUARANTINE_TX_MAX_CYCLES"
    )]
    pub quarantine_tx_max_cycles: u64,
    /// Per-transaction wall-clock budget in milliseconds for the quarantine lane (0 = unlimited).
    #[config(
        env = "PIPELINE_QUARANTINE_TX_MAX_MILLIS",
        default = "defaults::pipeline::QUARANTINE_TX_MAX_MILLIS"
    )]
    pub quarantine_tx_max_millis: u64,
    /// Default cursor mode for server-facing query endpoints: "ephemeral" or "stored".
    #[config(
        env = "PIPELINE_QUERY_DEFAULT_CURSOR_MODE",
        default = "defaults::pipeline::QUERY_DEFAULT_CURSOR_MODE.parse().unwrap()"
    )]
    pub query_default_cursor_mode: QueryCursorMode,
    /// Maximum fetch size for iterable queries executed inside the IVM.
    #[config(
        env = "PIPELINE_QUERY_MAX_FETCH_SIZE",
        default = "defaults::pipeline::QUERY_MAX_FETCH_SIZE"
    )]
    pub query_max_fetch_size: u64,
    /// Minimum gas units required to use stored cursor mode (0 = disabled).
    #[config(
        env = "PIPELINE_QUERY_STORED_MIN_GAS_UNITS",
        default = "defaults::pipeline::QUERY_STORED_MIN_GAS_UNITS"
    )]
    pub query_stored_min_gas_units: u64,
    /// AMX per-dataspace execution budget in milliseconds.
    #[config(
        env = "PIPELINE_AMX_PER_DATASPACE_BUDGET_MS",
        default = "defaults::pipeline::AMX_PER_DATASPACE_BUDGET_MS"
    )]
    pub amx_per_dataspace_budget_ms: u64,
    /// AMX group execution budget across dataspaces in milliseconds.
    #[config(
        env = "PIPELINE_AMX_GROUP_BUDGET_MS",
        default = "defaults::pipeline::AMX_GROUP_BUDGET_MS"
    )]
    pub amx_group_budget_ms: u64,
    /// Estimated nanoseconds per instruction used for AMX budgeting.
    #[config(
        env = "PIPELINE_AMX_PER_INSTRUCTION_NS",
        default = "defaults::pipeline::AMX_PER_INSTRUCTION_NS"
    )]
    pub amx_per_instruction_ns: u64,
    /// Estimated nanoseconds per memory access used for AMX budgeting.
    #[config(
        env = "PIPELINE_AMX_PER_MEMORY_ACCESS_NS",
        default = "defaults::pipeline::AMX_PER_MEMORY_ACCESS_NS"
    )]
    pub amx_per_memory_access_ns: u64,
    /// Estimated nanoseconds per syscall used for AMX budgeting.
    #[config(
        env = "PIPELINE_AMX_PER_SYSCALL_NS",
        default = "defaults::pipeline::AMX_PER_SYSCALL_NS"
    )]
    pub amx_per_syscall_ns: u64,
}

/// User-level configuration container for `TieredState`.
#[derive(Debug, ReadConfig, Clone)]
pub struct TieredState {
    /// Enable tiered state snapshots.
    #[config(
        env = "TIERED_STATE_ENABLED",
        default = "defaults::tiered_state::ENABLED"
    )]
    pub enabled: bool,
    /// Maximum number of keys kept hot (0 = unlimited).
    #[config(
        env = "TIERED_STATE_HOT_RETAINED_KEYS",
        default = "defaults::tiered_state::HOT_RETAINED_KEYS"
    )]
    pub hot_retained_keys: usize,
    /// Hot-tier byte budget based on deterministic in-memory WSV sizing (0 = unlimited).
    /// Grace retention may temporarily exceed this budget.
    #[config(
        env = "TIERED_STATE_HOT_RETAINED_BYTES",
        default = "defaults::tiered_state::HOT_RETAINED_BYTES"
    )]
    pub hot_retained_bytes: Bytes<u64>,
    /// Minimum snapshots to retain newly hot entries before demotion (0 = disabled).
    #[config(
        env = "TIERED_STATE_HOT_RETAINED_GRACE_SNAPSHOTS",
        default = "defaults::tiered_state::HOT_RETAINED_GRACE_SNAPSHOTS"
    )]
    pub hot_retained_grace_snapshots: u64,
    /// Optional on-disk root for cold tier spill files.
    #[config(env = "TIERED_STATE_COLD_STORE_ROOT")]
    pub cold_store_root: Option<PathBuf>,
    /// Optional on-disk root for DA-backed cold tier spill files.
    #[config(env = "TIERED_STATE_DA_STORE_ROOT")]
    pub da_store_root: Option<PathBuf>,
    /// Number of snapshots to retain on disk (0 = keep all).
    #[config(
        env = "TIERED_STATE_MAX_SNAPSHOTS",
        default = "defaults::tiered_state::MAX_SNAPSHOTS"
    )]
    pub max_snapshots: usize,
    /// Optional cold-tier byte budget across snapshots (0 = unlimited).
    #[config(
        env = "TIERED_STATE_MAX_COLD_BYTES",
        default = "defaults::tiered_state::MAX_COLD_BYTES"
    )]
    pub max_cold_bytes: Bytes<u64>,
}

impl TieredState {
    fn parse(self) -> actual::TieredState {
        actual::TieredState {
            enabled: self.enabled,
            hot_retained_keys: self.hot_retained_keys,
            hot_retained_bytes: self.hot_retained_bytes,
            hot_retained_grace_snapshots: self.hot_retained_grace_snapshots,
            cold_store_root: self.cold_store_root,
            da_store_root: self.da_store_root,
            max_snapshots: self.max_snapshots,
            max_cold_bytes: self.max_cold_bytes,
        }
    }
}

/// User-level configuration container for the compute lane.
#[derive(Debug, ReadConfig, Clone, norito::JsonDeserialize)]
pub struct Compute {
    /// Enable the compute lane/gateway.
    #[config(env = "COMPUTE_ENABLED", default = "defaults::compute::ENABLED")]
    pub enabled: bool,
    /// Namespaces allowed for compute routes.
    #[config(default = "defaults::compute::default_namespaces()")]
    pub namespaces: Vec<Name>,
    /// Default TTL applied to compute calls (slots).
    #[config(default = "defaults::compute::default_ttl_slots()")]
    pub default_ttl_slots: NonZeroU64,
    /// Maximum TTL accepted for compute calls (slots).
    #[config(default = "defaults::compute::max_ttl_slots()")]
    pub max_ttl_slots: NonZeroU64,
    /// Maximum request payload size (bytes).
    #[config(default = "defaults::compute::MAX_REQUEST_BYTES")]
    pub max_request_bytes: Bytes<u64>,
    /// Maximum response payload size (bytes).
    #[config(default = "defaults::compute::MAX_RESPONSE_BYTES")]
    pub max_response_bytes: Bytes<u64>,
    /// Per-call gas cap applied at admission.
    #[config(default = "defaults::compute::max_gas_per_call()")]
    pub max_gas_per_call: NonZeroU64,
    /// Resource profiles advertised by the node.
    #[config(default = "defaults::compute::resource_profiles()")]
    pub resource_profiles: BTreeMap<Name, ComputeResourceBudget>,
    /// Default resource profile used when routes omit an override.
    #[config(default = "defaults::compute::default_resource_profile()")]
    pub default_resource_profile: Name,
    /// Price families mapping cycles/egress weights into compute units.
    #[config(default = "defaults::compute::price_families()")]
    pub price_families: BTreeMap<Name, ComputePriceWeights>,
    /// Default price family for routes without an explicit family.
    #[config(default = "defaults::compute::default_price_family()")]
    pub default_price_family: Name,
    /// Authentication policy enforced when routes omit an override.
    #[config(default = "defaults::compute::default_auth_policy()")]
    pub auth_policy: ComputeAuthPolicy,
    /// Sandbox rules applied to compute execution.
    #[config(default = "defaults::compute::sandbox_rules()")]
    pub sandbox: ComputeSandboxRules,
    /// Economic settings for compute pricing and sponsorship.
    #[config(default = "ComputeEconomics::default()")]
    pub economics: ComputeEconomics,
    /// Service-level objectives applied to the compute gateway.
    #[config(default = "ComputeSlo::default()")]
    pub slo: ComputeSlo,
}

impl Compute {
    fn parse(self, emitter: &mut Emitter<ParseError>) -> Option<actual::Compute> {
        let mut ok = true;

        if self.namespaces.is_empty() {
            emitter.emit(
                Report::new(ParseError::InvalidComputeConfig)
                    .attach("compute.namespaces must not be empty"),
            );
            ok = false;
        }

        if self.default_ttl_slots > self.max_ttl_slots {
            emitter.emit(
                Report::new(ParseError::InvalidComputeConfig).attach(format!(
                    "compute.default_ttl_slots ({}) exceeds compute.max_ttl_slots ({})",
                    self.default_ttl_slots, self.max_ttl_slots
                )),
            );
            ok = false;
        }

        let namespaces: BTreeSet<Name> = self.namespaces.into_iter().collect();

        if !self
            .resource_profiles
            .contains_key(&self.default_resource_profile)
        {
            emitter.emit(
                Report::new(ParseError::InvalidComputeConfig).attach(format!(
                    "compute.default_resource_profile `{}` missing from compute.resource_profiles",
                    self.default_resource_profile
                )),
            );
            ok = false;
        }

        if !self.price_families.contains_key(&self.default_price_family) {
            emitter.emit(
                Report::new(ParseError::InvalidComputeConfig).attach(format!(
                    "compute.default_price_family `{}` missing from compute.price_families",
                    self.default_price_family
                )),
            );
            ok = false;
        }

        if !ok {
            return None;
        }

        let economics = self.economics.parse(&self.price_families, emitter)?;
        let slo = self.slo.parse(emitter)?;

        Some(actual::Compute {
            enabled: self.enabled,
            namespaces,
            default_ttl_slots: self.default_ttl_slots,
            max_ttl_slots: self.max_ttl_slots,
            max_request_bytes: self.max_request_bytes,
            max_response_bytes: self.max_response_bytes,
            max_gas_per_call: self.max_gas_per_call,
            resource_profiles: self.resource_profiles,
            default_resource_profile: self.default_resource_profile,
            price_families: self.price_families,
            default_price_family: self.default_price_family,
            auth_policy: self.auth_policy,
            sandbox: self.sandbox,
            economics,
            slo,
        })
    }
}

/// User-level configuration for the content lane.
#[derive(Debug, ReadConfig, Clone)]
pub struct Content {
    /// Maximum tarball size accepted for a bundle (bytes).
    #[config(default = "defaults::content::MAX_BUNDLE_BYTES")]
    pub max_bundle_bytes: u64,
    /// Maximum number of files in an archive.
    #[config(default = "defaults::content::MAX_FILES")]
    pub max_files: u32,
    /// Maximum allowed path length per file.
    #[config(default = "defaults::content::MAX_PATH_LEN")]
    pub max_path_len: u32,
    /// Maximum retention window (blocks) for expiring bundles.
    #[config(default = "defaults::content::MAX_RETENTION_BLOCKS")]
    pub max_retention_blocks: u64,
    /// Chunk size (bytes) used during ingestion.
    #[config(default = "defaults::content::CHUNK_SIZE_BYTES")]
    pub chunk_size_bytes: u32,
    /// Accounts permitted to publish bundles (empty = allow all).
    #[config(default)]
    pub publish_allow_accounts: Vec<AccountId>,
    /// Maximum requests per second served by the content gateway.
    #[config(default = "defaults::content::MAX_REQUESTS_PER_SECOND")]
    pub max_requests_per_second: u32,
    /// Burst size for request token buckets.
    #[config(default = "defaults::content::REQUEST_BURST")]
    pub request_burst: u32,
    /// Maximum egress bytes per second served by the content gateway.
    #[config(default = "defaults::content::MAX_EGRESS_BYTES_PER_SECOND")]
    pub max_egress_bytes_per_second: u32,
    /// Burst size for egress token buckets (bytes).
    #[config(default = "defaults::content::EGRESS_BURST_BYTES")]
    pub egress_burst_bytes: u64,
    /// Default Cache-Control max-age (seconds) applied to bundles.
    #[config(default = "defaults::content::DEFAULT_CACHE_MAX_AGE_SECS")]
    pub default_cache_max_age_secs: u32,
    /// Upper bound for Cache-Control max-age (seconds).
    #[config(default = "defaults::content::MAX_CACHE_MAX_AGE_SECS")]
    pub max_cache_max_age_secs: u32,
    /// Whether bundles are immutable by default.
    #[config(default = "defaults::content::IMMUTABLE_BUNDLES")]
    pub immutable_bundles: bool,
    /// Default auth mode (`public`, `role:<role_id>`, or `sponsor:<uaid>`).
    #[config(default = "defaults::content::default_auth_mode()")]
    pub default_auth_mode: String,
    /// Target p50 latency (milliseconds) for content responses.
    #[config(default = "defaults::content::TARGET_P50_LATENCY_MS")]
    pub target_p50_latency_ms: u32,
    /// Target p99 latency (milliseconds) for content responses.
    #[config(default = "defaults::content::TARGET_P99_LATENCY_MS")]
    pub target_p99_latency_ms: u32,
    /// Target availability (basis points) for content responses.
    #[config(default = "defaults::content::TARGET_AVAILABILITY_BPS")]
    pub target_availability_bps: u32,
    /// Optional PoW difficulty (leading zero bits, 0 = disabled).
    #[config(default = "defaults::content::POW_DIFFICULTY_BITS")]
    pub pow_difficulty_bits: u8,
    /// Header carrying PoW tokens.
    #[config(default = "defaults::content::default_pow_header()")]
    pub pow_header: String,
    /// Default DA stripe layout applied to content bundles.
    #[config(default = "defaults::content::default_stripe_layout()")]
    pub stripe_layout: DaStripeLayout,
}

impl Content {
    fn parse(self) -> actual::Content {
        let max_cache_max_age_secs = self.max_cache_max_age_secs.max(1);
        let default_cache_max_age_secs = self
            .default_cache_max_age_secs
            .min(max_cache_max_age_secs)
            .max(1);
        let pow_header = if self.pow_header.trim().is_empty() {
            defaults::content::default_pow_header()
        } else {
            self.pow_header
        };
        actual::Content {
            max_bundle_bytes: self.max_bundle_bytes,
            max_files: self.max_files,
            max_path_len: self.max_path_len,
            max_retention_blocks: self.max_retention_blocks,
            chunk_size_bytes: self.chunk_size_bytes,
            publish_allow_accounts: self.publish_allow_accounts,
            limits: actual::ContentLimits {
                max_requests_per_second: NonZeroU32::new(self.max_requests_per_second)
                    .unwrap_or(nonzero!(1_u32)),
                request_burst: NonZeroU32::new(self.request_burst).unwrap_or(nonzero!(1_u32)),
                max_egress_bytes_per_second: NonZeroU64::new(u64::from(
                    self.max_egress_bytes_per_second,
                ))
                .unwrap_or(nonzero!(1_u64)),
                egress_burst_bytes: NonZeroU64::new(self.egress_burst_bytes)
                    .unwrap_or(nonzero!(1_u64)),
            },
            default_cache_max_age_secs,
            max_cache_max_age_secs,
            immutable_bundles: self.immutable_bundles,
            default_auth_mode: parse_content_auth_mode(&self.default_auth_mode),
            slo: actual::ContentSlo {
                target_p50_latency_ms: NonZeroU32::new(self.target_p50_latency_ms)
                    .unwrap_or(nonzero!(1_u32)),
                target_p99_latency_ms: NonZeroU32::new(self.target_p99_latency_ms)
                    .unwrap_or(nonzero!(1_u32)),
                target_availability_bps: NonZeroU32::new(self.target_availability_bps)
                    .unwrap_or(nonzero!(1_u32)),
            },
            pow: actual::ContentPow {
                difficulty_bits: self.pow_difficulty_bits,
                header_name: pow_header,
            },
            stripe_layout: self.stripe_layout,
        }
    }
}

fn parse_content_auth_mode(raw: &str) -> ContentAuthMode {
    let trimmed = raw.trim();
    if trimmed.eq_ignore_ascii_case("public") {
        return ContentAuthMode::Public;
    }
    if let Some(role_str) = trimmed.strip_prefix("role:") {
        let role = RoleId::from_str(role_str.trim()).unwrap_or_else(|err| {
            panic!("invalid content.default_auth_mode role `{role_str}`: {err}");
        });
        return ContentAuthMode::RoleGate(role);
    }
    if let Some(uaid_str) = trimmed.strip_prefix("sponsor:") {
        let cleaned = uaid_str
            .trim()
            .strip_prefix("uaid:")
            .unwrap_or_else(|| uaid_str.trim());
        let uaid = UniversalAccountId::from_str(cleaned).unwrap_or_else(|err| {
            panic!("invalid content.default_auth_mode sponsor `{uaid_str}`: {err}");
        });
        return ContentAuthMode::Sponsor(uaid);
    }
    panic!(
        "invalid content.default_auth_mode value `{trimmed}`; expected `public`, `role:<role_id>`, or `sponsor:<uaid>`"
    );
}

/// Economic settings for the compute lane (pricing, bounds, sponsorship).
#[derive(Debug, ReadConfig, Clone, norito::JsonDeserialize)]
pub struct ComputeEconomics {
    /// Maximum compute units that may be charged per call.
    #[config(default = "defaults::compute::max_cu_per_call()")]
    pub max_cu_per_call: NonZeroU64,
    /// Maximum amplification ratio (response/ingress) permitted.
    #[config(default = "defaults::compute::max_amplification_ratio()")]
    pub max_amplification_ratio: NonZeroU32,
    /// Fee split across burn/validators/providers (basis points).
    #[config(default = "defaults::compute::fee_split()")]
    pub fee_split: ComputeFeeSplit,
    /// Sponsor policy caps for subsidised calls.
    #[config(default = "defaults::compute::sponsor_policy()")]
    pub sponsor_policy: ComputeSponsorPolicy,
    /// Price delta bounds per risk class.
    #[config(default = "defaults::compute::price_bounds()")]
    pub price_bounds: BTreeMap<ComputePriceRiskClass, ComputePriceDeltaBounds>,
    /// Risk class mapping for price families.
    #[config(default = "defaults::compute::price_risk_classes()")]
    pub price_risk_classes: BTreeMap<Name, ComputePriceRiskClass>,
    /// Multipliers applied for GPU/TEE/best-effort execution classes.
    #[config(default = "defaults::compute::price_amplifiers()")]
    pub price_amplifiers: ComputePriceAmplifiers,
}

impl ComputeEconomics {
    fn parse(
        self,
        price_families: &BTreeMap<Name, ComputePriceWeights>,
        emitter: &mut Emitter<ParseError>,
    ) -> Option<actual::ComputeEconomics> {
        let mut ok = true;

        if self.fee_split.total_bps() != ComputeFeeSplit::BPS_DENOMINATOR {
            emitter.emit(
                Report::new(ParseError::InvalidComputeConfig).attach(format!(
                    "compute.economics.fee_split must sum to {} bps (got {})",
                    ComputeFeeSplit::BPS_DENOMINATOR,
                    self.fee_split.total_bps()
                )),
            );
            ok = false;
        }

        if self.sponsor_policy.max_daily_cu < self.sponsor_policy.max_cu_per_call {
            emitter.emit(
                Report::new(ParseError::InvalidComputeConfig).attach(format!(
                    "compute.economics.sponsor_policy.max_daily_cu ({}) must be >= max_cu_per_call ({})",
                    self.sponsor_policy.max_daily_cu, self.sponsor_policy.max_cu_per_call
                )),
            );
            ok = false;
        }

        for family in price_families.keys() {
            if !self.price_risk_classes.contains_key(family) {
                emitter.emit(
                    Report::new(ParseError::InvalidComputeConfig).attach(format!(
                        "compute.economics.price_risk_classes missing entry for price family `{family}`"
                    )),
                );
                ok = false;
            }
        }

        for class in self.price_risk_classes.values() {
            if !self.price_bounds.contains_key(class) {
                emitter.emit(
                    Report::new(ParseError::InvalidComputeConfig).attach(format!(
                        "compute.economics.price_bounds missing entry for risk class `{class:?}`"
                    )),
                );
                ok = false;
            }
        }

        for family in self.price_risk_classes.keys() {
            if !price_families.contains_key(family) {
                emitter.emit(
                    Report::new(ParseError::InvalidComputeConfig).attach(format!(
                        "compute.economics.price_risk_classes defines `{family}` but compute.price_families does not"
                    )),
                );
                ok = false;
            }
        }

        if !ok {
            return None;
        }

        Some(actual::ComputeEconomics {
            max_cu_per_call: self.max_cu_per_call,
            max_amplification_ratio: self.max_amplification_ratio,
            fee_split: self.fee_split,
            sponsor_policy: self.sponsor_policy,
            price_bounds: self.price_bounds,
            price_risk_classes: self.price_risk_classes,
            price_amplifiers: self.price_amplifiers,
            price_family_baseline: price_families.clone(),
        })
    }
}

impl Default for ComputeEconomics {
    fn default() -> Self {
        Self {
            max_cu_per_call: defaults::compute::max_cu_per_call(),
            max_amplification_ratio: defaults::compute::max_amplification_ratio(),
            fee_split: defaults::compute::fee_split(),
            sponsor_policy: defaults::compute::sponsor_policy(),
            price_bounds: defaults::compute::price_bounds(),
            price_risk_classes: defaults::compute::price_risk_classes(),
            price_amplifiers: defaults::compute::price_amplifiers(),
        }
    }
}

/// User-facing compute SLO configuration.
#[derive(Debug, ReadConfig, Clone, Copy, norito::JsonDeserialize)]
pub struct ComputeSlo {
    /// Maximum in-flight requests per route.
    #[config(default = "defaults::compute::max_inflight_per_route()")]
    pub max_inflight_per_route: NonZeroUsize,
    /// Maximum queued requests per route (beyond in-flight).
    #[config(default = "defaults::compute::queue_depth_per_route()")]
    pub queue_depth_per_route: NonZeroUsize,
    /// Maximum allowed requests per second (token-bucket).
    #[config(default = "defaults::compute::max_requests_per_second()")]
    pub max_requests_per_second: NonZeroU32,
    /// Target p50 latency budget in milliseconds.
    #[config(default = "defaults::compute::target_p50_latency_ms()")]
    pub target_p50_latency_ms: NonZeroU64,
    /// Target p95 latency budget in milliseconds.
    #[config(default = "defaults::compute::target_p95_latency_ms()")]
    pub target_p95_latency_ms: NonZeroU64,
    /// Target p99 latency budget in milliseconds.
    #[config(default = "defaults::compute::target_p99_latency_ms()")]
    pub target_p99_latency_ms: NonZeroU64,
}

impl ComputeSlo {
    fn parse(self, emitter: &mut Emitter<ParseError>) -> Option<actual::ComputeSlo> {
        let mut ok = true;

        if self.queue_depth_per_route < self.max_inflight_per_route {
            emitter.emit(
                Report::new(ParseError::InvalidComputeConfig).attach(format!(
                    "compute.slo.queue_depth_per_route ({}) must be >= max_inflight_per_route ({})",
                    self.queue_depth_per_route, self.max_inflight_per_route
                )),
            );
            ok = false;
        }

        if self.target_p50_latency_ms > self.target_p95_latency_ms
            || self.target_p95_latency_ms > self.target_p99_latency_ms
        {
            emitter.emit(
                Report::new(ParseError::InvalidComputeConfig)
                    .attach("compute.slo latency targets must satisfy p50 <= p95 <= p99"),
            );
            ok = false;
        }

        if !ok {
            return None;
        }

        Some(actual::ComputeSlo {
            max_inflight_per_route: self.max_inflight_per_route,
            queue_depth_per_route: self.queue_depth_per_route,
            max_requests_per_second: self.max_requests_per_second,
            target_p50_latency_ms: self.target_p50_latency_ms,
            target_p95_latency_ms: self.target_p95_latency_ms,
            target_p99_latency_ms: self.target_p99_latency_ms,
        })
    }
}

impl Default for ComputeSlo {
    fn default() -> Self {
        Self {
            max_inflight_per_route: defaults::compute::max_inflight_per_route(),
            queue_depth_per_route: defaults::compute::queue_depth_per_route(),
            max_requests_per_second: defaults::compute::max_requests_per_second(),
            target_p50_latency_ms: defaults::compute::target_p50_latency_ms(),
            target_p95_latency_ms: defaults::compute::target_p95_latency_ms(),
            target_p99_latency_ms: defaults::compute::target_p99_latency_ms(),
        }
    }
}

/// User-level configuration for oracle aggregation.
#[derive(Debug, ReadConfig, Clone, norito::JsonDeserialize)]
pub struct Oracle {
    /// Maximum number of feed events retained per feed (oldest entries are pruned).
    #[config(
        env = "ORACLE_HISTORY_DEPTH",
        default = "defaults::oracle::history_depth()"
    )]
    pub history_depth: NonZeroUsize,
    /// Asset used to reward oracle providers.
    #[config(default = "defaults::oracle::reward_asset()")]
    pub reward_asset: AssetDefinitionId,
    /// Account that funds oracle rewards.
    #[config(default = "defaults::oracle::reward_pool()")]
    pub reward_pool: AccountId,
    /// Fixed reward amount for inlier observations.
    #[config(default = "defaults::oracle::reward_amount()")]
    pub reward_amount: Numeric,
    /// Asset debited when slashing providers.
    #[config(default = "defaults::oracle::slash_asset()")]
    pub slash_asset: AssetDefinitionId,
    /// Account credited when penalties are collected.
    #[config(default = "defaults::oracle::slash_receiver()")]
    pub slash_receiver: AccountId,
    /// Penalty applied to outlier observations.
    #[config(default = "defaults::oracle::slash_outlier_amount()")]
    pub slash_outlier_amount: Numeric,
    /// Penalty applied to explicit error observations.
    #[config(default = "defaults::oracle::slash_error_amount()")]
    pub slash_error_amount: Numeric,
    /// Penalty applied when a provider misses a slot.
    #[config(default = "defaults::oracle::slash_no_show_amount()")]
    pub slash_no_show_amount: Numeric,
    /// Asset staked as a bond when opening disputes.
    #[config(default = "defaults::oracle::dispute_bond_asset()")]
    pub dispute_bond_asset: AssetDefinitionId,
    /// Bond amount required to open a dispute.
    #[config(default = "defaults::oracle::dispute_bond_amount()")]
    pub dispute_bond_amount: Numeric,
    /// Reward paid to successful challengers.
    #[config(default = "defaults::oracle::dispute_reward_amount()")]
    pub dispute_reward_amount: Numeric,
    /// Penalty for frivolous disputes.
    #[config(default = "defaults::oracle::frivolous_slash_amount()")]
    pub frivolous_slash_amount: Numeric,
    /// SLA (blocks) for the intake stage.
    #[config(default = "defaults::oracle::intake_sla_blocks()")]
    pub intake_sla_blocks: u64,
    /// SLA (blocks) for the rules committee stage.
    #[config(default = "defaults::oracle::rules_sla_blocks()")]
    pub rules_sla_blocks: u64,
    /// SLA (blocks) for the COP review stage.
    #[config(default = "defaults::oracle::cop_sla_blocks()")]
    pub cop_sla_blocks: u64,
    /// SLA (blocks) for the technical audit stage.
    #[config(default = "defaults::oracle::technical_sla_blocks()")]
    pub technical_sla_blocks: u64,
    /// SLA (blocks) for the policy jury stage.
    #[config(default = "defaults::oracle::policy_jury_sla_blocks()")]
    pub policy_jury_sla_blocks: u64,
    /// SLA (blocks) for the enactment stage.
    #[config(default = "defaults::oracle::enact_sla_blocks()")]
    pub enact_sla_blocks: u64,
    /// Intake approvals required.
    #[config(default = "defaults::oracle::intake_min_votes()")]
    pub intake_min_votes: NonZeroUsize,
    /// Rules committee approvals required.
    #[config(default = "defaults::oracle::rules_min_votes()")]
    pub rules_min_votes: NonZeroUsize,
    /// COP approvals for low-class changes.
    #[config(default = "defaults::oracle::cop_low_votes()")]
    pub cop_low_votes: NonZeroUsize,
    /// COP approvals for medium-class changes.
    #[config(default = "defaults::oracle::cop_medium_votes()")]
    pub cop_medium_votes: NonZeroUsize,
    /// COP approvals for high-class changes.
    #[config(default = "defaults::oracle::cop_high_votes()")]
    pub cop_high_votes: NonZeroUsize,
    /// Technical audit approvals required.
    #[config(default = "defaults::oracle::technical_min_votes()")]
    pub technical_min_votes: NonZeroUsize,
    /// Policy jury approvals for low-class changes.
    #[config(default = "defaults::oracle::policy_jury_low_votes()")]
    pub policy_jury_low_votes: NonZeroUsize,
    /// Policy jury approvals for medium-class changes.
    #[config(default = "defaults::oracle::policy_jury_medium_votes()")]
    pub policy_jury_medium_votes: NonZeroUsize,
    /// Policy jury approvals for high-class changes.
    #[config(default = "defaults::oracle::policy_jury_high_votes()")]
    pub policy_jury_high_votes: NonZeroUsize,
    /// Feed identifier expected for twitter follow attestations.
    #[config(default = "defaults::oracle::twitter_binding_feed_id()")]
    pub twitter_binding_feed_id: Name,
    /// Pepper identifier that must match keyed-hash attestations.
    #[config(default = "defaults::oracle::twitter_binding_pepper_id()")]
    pub twitter_binding_pepper_id: String,
    /// Maximum TTL accepted for twitter follow attestations (milliseconds).
    #[config(default = "defaults::oracle::twitter_binding_max_ttl_ms()")]
    pub twitter_binding_max_ttl_ms: u64,
    /// Minimum TTL accepted for twitter follow attestations (milliseconds).
    #[config(default = "defaults::oracle::twitter_binding_min_ttl_ms()")]
    pub twitter_binding_min_ttl_ms: u64,
    /// Minimum spacing between updates for the same twitter binding (milliseconds).
    #[config(default = "defaults::oracle::twitter_binding_min_update_spacing_ms()")]
    pub twitter_binding_min_update_spacing_ms: u64,
}

impl Oracle {
    fn parse(self) -> actual::Oracle {
        actual::Oracle {
            history_depth: self.history_depth,
            economics: actual::OracleEconomics {
                reward_asset: self.reward_asset,
                reward_pool: self.reward_pool,
                reward_amount: self.reward_amount,
                slash_asset: self.slash_asset,
                slash_receiver: self.slash_receiver,
                slash_outlier_amount: self.slash_outlier_amount,
                slash_error_amount: self.slash_error_amount,
                slash_no_show_amount: self.slash_no_show_amount,
                dispute_bond_asset: self.dispute_bond_asset,
                dispute_bond_amount: self.dispute_bond_amount,
                dispute_reward_amount: self.dispute_reward_amount,
                frivolous_slash_amount: self.frivolous_slash_amount,
            },
            governance: actual::OracleGovernance {
                intake_sla_blocks: self.intake_sla_blocks,
                rules_sla_blocks: self.rules_sla_blocks,
                cop_sla_blocks: self.cop_sla_blocks,
                technical_sla_blocks: self.technical_sla_blocks,
                policy_jury_sla_blocks: self.policy_jury_sla_blocks,
                enact_sla_blocks: self.enact_sla_blocks,
                intake_min_votes: self.intake_min_votes,
                rules_min_votes: self.rules_min_votes,
                cop_min_votes: actual::OracleChangeThresholds {
                    low: self.cop_low_votes,
                    medium: self.cop_medium_votes,
                    high: self.cop_high_votes,
                },
                technical_min_votes: self.technical_min_votes,
                policy_jury_min_votes: actual::OracleChangeThresholds {
                    low: self.policy_jury_low_votes,
                    medium: self.policy_jury_medium_votes,
                    high: self.policy_jury_high_votes,
                },
            },
            twitter_binding: actual::OracleTwitterBinding {
                feed_id: self.twitter_binding_feed_id,
                pepper_id: self.twitter_binding_pepper_id,
                max_ttl_ms: self.twitter_binding_max_ttl_ms,
                min_ttl_ms: self.twitter_binding_min_ttl_ms,
                min_update_spacing_ms: self.twitter_binding_min_update_spacing_ms,
            },
        }
    }
}

impl IvmProvedExecution {
    fn parse(self) -> actual::IvmProvedExecution {
        actual::IvmProvedExecution {
            enabled: self.enabled,
            skip_replay: self.skip_replay,
            allowed_circuits: self.allowed_circuits,
        }
    }
}

impl Pipeline {
    fn parse(self) -> actual::Pipeline {
        actual::Pipeline {
            ivm_proved: self.ivm_proved.parse(),
            dynamic_prepass: self.dynamic_prepass,
            access_set_cache_enabled: self.access_set_cache_enabled,
            parallel_overlay: self.parallel_overlay,
            workers: self.workers,
            stateless_cache_cap: self.stateless_cache_cap,
            parallel_apply: self.parallel_apply,
            ready_queue_heap: self.ready_queue_heap,
            gpu_key_bucket: self.gpu_key_bucket,
            debug_trace_scheduler_inputs: self.debug_trace_scheduler_inputs,
            debug_trace_tx_eval: self.debug_trace_tx_eval,
            signature_batch_max: self.signature_batch_max,
            signature_batch_max_ed25519: self.signature_batch_max_ed25519,
            signature_batch_max_secp256k1: self.signature_batch_max_secp256k1,
            signature_batch_max_pqc: self.signature_batch_max_pqc,
            cache_size: self.cache_size,
            ivm_cache_max_decoded_ops: self.ivm_cache_max_decoded_ops,
            ivm_cache_max_bytes: self.ivm_cache_max_bytes,
            ivm_prover_threads: self.ivm_prover_threads,
            signature_batch_max_bls: self.signature_batch_max_bls,
            overlay_max_instructions: self.overlay_max_instructions,
            overlay_max_bytes: self.overlay_max_bytes,
            overlay_chunk_instructions: self.overlay_chunk_instructions,
            gas: self.gas.parse(),
            ivm_max_cycles_upper_bound: self.ivm_max_cycles_upper_bound,
            ivm_max_decoded_instructions: self.ivm_max_decoded_instructions,
            ivm_max_decoded_bytes: self.ivm_max_decoded_bytes,
            quarantine_max_txs_per_block: self.quarantine_max_txs_per_block,
            quarantine_tx_max_cycles: self.quarantine_tx_max_cycles,
            quarantine_tx_max_millis: self.quarantine_tx_max_millis,
            query_default_cursor_mode: self.query_default_cursor_mode.into_actual(),
            query_max_fetch_size: self.query_max_fetch_size,
            query_stored_min_gas_units: self.query_stored_min_gas_units,
            amx_per_dataspace_budget_ms: self.amx_per_dataspace_budget_ms,
            amx_group_budget_ms: self.amx_group_budget_ms,
            amx_per_instruction_ns: self.amx_per_instruction_ns,
            amx_per_memory_access_ns: self.amx_per_memory_access_ns,
            amx_per_syscall_ns: self.amx_per_syscall_ns,
        }
    }
}

/// Cursor handling mode for server-facing iterable queries (user view).
/// User-level enumeration translating `QueryCursorMode` settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, strum::EnumString, strum::Display)]
#[strum(serialize_all = "snake_case")]
pub enum QueryCursorMode {
    /// Keep cursor state only in memory; abandon it when the client disconnects.
    Ephemeral,
    /// Persist cursor state so it survives node restarts and client reconnects.
    Stored,
}

impl QueryCursorMode {
    fn into_actual(self) -> actual::QueryCursorMode {
        match self {
            QueryCursorMode::Ephemeral => actual::QueryCursorMode::Ephemeral,
            QueryCursorMode::Stored => actual::QueryCursorMode::Stored,
        }
    }
}

impl json::JsonSerialize for QueryCursorMode {
    fn json_serialize(&self, out: &mut String) {
        json::write_json_string(&self.to_string(), out);
    }
}

impl json::JsonDeserialize for QueryCursorMode {
    fn json_deserialize(
        parser: &mut json::Parser<'_>,
    ) -> ::core::result::Result<Self, json::Error> {
        let text = parser.parse_string()?;
        Self::from_str(&text).map_err(|err| json::Error::InvalidField {
            field: "query_cursor_mode".into(),
            message: err.to_string(),
        })
    }
}

#[cfg(test)]
mod pipeline_tests {
    use super::*;
    use crate::parameters::defaults;

    fn pipeline_with_debug(trace_scheduler_inputs: bool, trace_tx_eval: bool) -> Pipeline {
        Pipeline {
            dynamic_prepass: defaults::pipeline::DYNAMIC_PREPASS,
            access_set_cache_enabled: defaults::pipeline::ACCESS_SET_CACHE_ENABLED,
            parallel_overlay: defaults::pipeline::PARALLEL_OVERLAY,
            workers: defaults::pipeline::WORKERS,
            stateless_cache_cap: defaults::pipeline::STATELESS_CACHE_CAP,
            parallel_apply: defaults::pipeline::PARALLEL_APPLY,
            ready_queue_heap: defaults::pipeline::READY_QUEUE_HEAP,
            gpu_key_bucket: defaults::pipeline::GPU_KEY_BUCKET,
            debug_trace_scheduler_inputs: trace_scheduler_inputs,
            debug_trace_tx_eval: trace_tx_eval,
            signature_batch_max: defaults::pipeline::SIGNATURE_BATCH_MAX,
            signature_batch_max_ed25519: defaults::pipeline::SIGNATURE_BATCH_MAX_ED25519,
            signature_batch_max_secp256k1: defaults::pipeline::SIGNATURE_BATCH_MAX_SECP256K1,
            signature_batch_max_pqc: defaults::pipeline::SIGNATURE_BATCH_MAX_PQC,
            signature_batch_max_bls: defaults::pipeline::SIGNATURE_BATCH_MAX_BLS,
            cache_size: defaults::pipeline::CACHE_SIZE,
            ivm_cache_max_decoded_ops: defaults::pipeline::IVM_CACHE_MAX_DECODED_OPS,
            ivm_cache_max_bytes: defaults::pipeline::IVM_CACHE_MAX_BYTES,
            ivm_prover_threads: defaults::pipeline::IVM_PROVER_THREADS,
            overlay_max_instructions: defaults::pipeline::OVERLAY_MAX_INSTRUCTIONS,
            overlay_max_bytes: defaults::pipeline::OVERLAY_MAX_BYTES,
            overlay_chunk_instructions: defaults::pipeline::OVERLAY_CHUNK_INSTRUCTIONS,
            ivm_proved: IvmProvedExecution {
                enabled: defaults::pipeline::ivm_proved::ENABLED,
                skip_replay: defaults::pipeline::ivm_proved::SKIP_REPLAY,
                allowed_circuits: Vec::new(),
            },
            gas: Gas {
                tech_account_id: defaults::pipeline::GAS_TECH_ACCOUNT_ID.to_string(),
                accepted_assets: Vec::new(),
                units_per_gas: Vec::new(),
            },
            ivm_max_cycles_upper_bound: defaults::pipeline::IVM_MAX_CYCLES_UPPER_BOUND,
            ivm_max_decoded_instructions: defaults::pipeline::IVM_MAX_DECODED_INSTRUCTIONS,
            ivm_max_decoded_bytes: defaults::pipeline::IVM_MAX_DECODED_BYTES,
            quarantine_max_txs_per_block: defaults::pipeline::QUARANTINE_MAX_TXS_PER_BLOCK,
            quarantine_tx_max_cycles: defaults::pipeline::QUARANTINE_TX_MAX_CYCLES,
            quarantine_tx_max_millis: defaults::pipeline::QUARANTINE_TX_MAX_MILLIS,
            query_default_cursor_mode: QueryCursorMode::Ephemeral,
            query_max_fetch_size: defaults::pipeline::QUERY_MAX_FETCH_SIZE,
            query_stored_min_gas_units: defaults::pipeline::QUERY_STORED_MIN_GAS_UNITS,
            amx_per_dataspace_budget_ms: defaults::pipeline::AMX_PER_DATASPACE_BUDGET_MS,
            amx_group_budget_ms: defaults::pipeline::AMX_GROUP_BUDGET_MS,
            amx_per_instruction_ns: defaults::pipeline::AMX_PER_INSTRUCTION_NS,
            amx_per_memory_access_ns: defaults::pipeline::AMX_PER_MEMORY_ACCESS_NS,
            amx_per_syscall_ns: defaults::pipeline::AMX_PER_SYSCALL_NS,
        }
    }

    #[test]
    fn pipeline_parse_maps_debug_flags() {
        let baseline = pipeline_with_debug(false, false).parse();
        assert!(!baseline.debug_trace_scheduler_inputs);
        assert!(!baseline.debug_trace_tx_eval);

        let debug_enabled = pipeline_with_debug(true, true).parse();
        assert!(debug_enabled.debug_trace_scheduler_inputs);
        assert!(debug_enabled.debug_trace_tx_eval);
    }
}

/// Zero-knowledge configuration section.
/// User-level configuration container for `Zk`.
#[derive(Debug, ReadConfig, Clone)]
pub struct Zk {
    #[config(nested)]
    /// Halo2 circuit/runtime configuration.
    pub halo2: Halo2,
    #[config(nested)]
    /// FASTPQ prover configuration.
    pub fastpq: Fastpq,
    #[config(nested)]
    /// Native STARK/FRI verification configuration.
    pub stark: Stark,
    /// Maximum number of recent shielded Merkle roots kept per asset.
    #[config(
        env = "ZK_ROOT_HISTORY_CAP",
        default = "defaults::zk::ledger::ROOT_HISTORY_CAP"
    )]
    pub root_history_cap: usize,
    /// Maximum number of recent ballot ciphertexts kept per election.
    #[config(
        env = "ZK_BALLOT_HISTORY_CAP",
        default = "defaults::zk::vote::BALLOT_HISTORY_CAP"
    )]
    pub ballot_history_cap: usize,
    /// Include explicit empty-tree root for assets with no commitments.
    #[config(
        env = "ZK_EMPTY_ROOT_ON_EMPTY",
        default = "defaults::zk::ledger::EMPTY_ROOT_ON_EMPTY"
    )]
    pub empty_root_on_empty: bool,
    /// Depth to use when computing the explicit empty-tree root.
    #[config(
        env = "ZK_MERKLE_DEPTH",
        default = "defaults::zk::ledger::EMPTY_ROOT_DEPTH"
    )]
    pub merkle_depth: u8,
    /// Maximum accepted proof size for stateless pre-verification (bytes).
    #[config(
        env = "ZK_PREVERIFY_MAX_BYTES",
        default = "defaults::zk::preverify::MAX_BYTES"
    )]
    pub preverify_max_bytes: usize,
    /// Soft byte-budget for stateless pre-verification (0 = unlimited).
    #[config(
        env = "ZK_PREVERIFY_BUDGET_BYTES",
        default = "defaults::zk::preverify::BUDGET_BYTES"
    )]
    pub preverify_budget_bytes: u64,
    /// Maximum number of recent proof records to retain per backend (0 = unlimited).
    #[config(
        env = "ZK_PROOF_HISTORY_CAP",
        default = "defaults::zk::proof::RECORD_HISTORY_CAP"
    )]
    pub proof_history_cap: usize,
    /// Minimum number of recent blocks to retain for proof records (age-based pruning).
    #[config(
        env = "ZK_PROOF_RETENTION_GRACE_BLOCKS",
        default = "defaults::zk::proof::RETENTION_GRACE_BLOCKS"
    )]
    pub proof_retention_grace_blocks: u64,
    /// Maximum number of proof records pruned in one pass (0 = unlimited).
    #[config(
        env = "ZK_PROOF_PRUNE_BATCH",
        default = "defaults::zk::proof::PRUNE_BATCH_SIZE"
    )]
    pub proof_prune_batch: usize,
    /// Maximum length of a bridge proof range (`end_height - start_height + 1`, 0 = unlimited).
    #[config(
        env = "ZK_BRIDGE_PROOF_MAX_RANGE_LEN",
        default = "defaults::zk::proof::BRIDGE_MAX_RANGE_LEN"
    )]
    pub bridge_proof_max_range_len: u64,
    /// Maximum age (in blocks) a bridge proof's end height may trail the current block (0 = unlimited).
    #[config(
        env = "ZK_BRIDGE_PROOF_MAX_PAST_AGE_BLOCKS",
        default = "defaults::zk::proof::BRIDGE_MAX_PAST_AGE_BLOCKS"
    )]
    pub bridge_proof_max_past_age_blocks: u64,
    /// Maximum future drift (in blocks) a bridge proof's end height may lead the current block (0 = unlimited).
    #[config(
        env = "ZK_BRIDGE_PROOF_MAX_FUTURE_DRIFT_BLOCKS",
        default = "defaults::zk::proof::BRIDGE_MAX_FUTURE_DRIFT_BLOCKS"
    )]
    pub bridge_proof_max_future_drift_blocks: u64,
    /// Poseidon parameter set identifier to embed into confidential policies (if any).
    #[config(env = "ZK_POSEIDON_PARAMS_ID")]
    pub poseidon_params_id: Option<u32>,
    /// Pedersen parameter set identifier to embed into confidential policies (if any).
    #[config(env = "ZK_PEDERSEN_PARAMS_ID")]
    pub pedersen_params_id: Option<u32>,
    /// Optional verifying key reference used for Kaigi roster join proofs.
    pub kaigi_roster_join_vk: Option<VerifyingKeyRef>,
    /// Optional verifying key reference used for Kaigi roster leave proofs.
    pub kaigi_roster_leave_vk: Option<VerifyingKeyRef>,
    /// Optional verifying key reference used for Kaigi usage commitment proofs.
    pub kaigi_usage_vk: Option<VerifyingKeyRef>,
}

impl Zk {
    fn parse(self) -> actual::Zk {
        actual::Zk {
            halo2: self.halo2.parse(),
            fastpq: self.fastpq.parse(),
            stark: self.stark.parse(),
            root_history_cap: self.root_history_cap,
            ballot_history_cap: self.ballot_history_cap,
            empty_root_on_empty: self.empty_root_on_empty,
            merkle_depth: self.merkle_depth,
            preverify_max_bytes: self.preverify_max_bytes,
            preverify_budget_bytes: self.preverify_budget_bytes,
            proof_history_cap: self.proof_history_cap,
            proof_retention_grace_blocks: self.proof_retention_grace_blocks,
            proof_prune_batch: self.proof_prune_batch,
            bridge_proof_max_range_len: self.bridge_proof_max_range_len,
            bridge_proof_max_past_age_blocks: self.bridge_proof_max_past_age_blocks,
            bridge_proof_max_future_drift_blocks: self.bridge_proof_max_future_drift_blocks,
            poseidon_params_id: self.poseidon_params_id,
            pedersen_params_id: self.pedersen_params_id,
            kaigi_roster_join_vk: self.kaigi_roster_join_vk.map(VerifyingKeyRef::parse),
            kaigi_roster_leave_vk: self.kaigi_roster_leave_vk.map(VerifyingKeyRef::parse),
            kaigi_usage_vk: self.kaigi_usage_vk.map(VerifyingKeyRef::parse),
            max_proof_size_bytes: defaults::confidential::MAX_PROOF_SIZE_BYTES,
            max_nullifiers_per_tx: defaults::confidential::MAX_NULLIFIERS_PER_TX,
            max_commitments_per_tx: defaults::confidential::MAX_COMMITMENTS_PER_TX,
            max_confidential_ops_per_block: defaults::confidential::MAX_CONFIDENTIAL_OPS_PER_BLOCK,
            verify_timeout: defaults::confidential::VERIFY_TIMEOUT,
            max_anchor_age_blocks: defaults::confidential::MAX_ANCHOR_AGE_BLOCKS,
            max_proof_bytes_block: defaults::confidential::MAX_PROOF_BYTES_BLOCK,
            max_verify_calls_per_tx: defaults::confidential::MAX_VERIFY_CALLS_PER_TX,
            max_verify_calls_per_block: defaults::confidential::MAX_VERIFY_CALLS_PER_BLOCK,
            max_public_inputs: defaults::confidential::MAX_PUBLIC_INPUTS,
            reorg_depth_bound: defaults::confidential::REORG_DEPTH_BOUND,
            policy_transition_delay_blocks: defaults::confidential::POLICY_TRANSITION_DELAY_BLOCKS,
            policy_transition_window_blocks:
                defaults::confidential::POLICY_TRANSITION_WINDOW_BLOCKS,
            tree_roots_history_len: defaults::confidential::TREE_ROOTS_HISTORY_LEN,
            tree_frontier_checkpoint_interval:
                defaults::confidential::TREE_FRONTIER_CHECKPOINT_INTERVAL,
            registry_max_vk_entries: defaults::confidential::REGISTRY_MAX_VK_ENTRIES,
            registry_max_params_entries: defaults::confidential::REGISTRY_MAX_PARAMS_ENTRIES,
            registry_max_delta_per_block: defaults::confidential::REGISTRY_MAX_DELTA_PER_BLOCK,
            gas: actual::ConfidentialGas {
                proof_base: defaults::confidential::gas::PROOF_BASE,
                per_public_input: defaults::confidential::gas::PER_PUBLIC_INPUT,
                per_proof_byte: defaults::confidential::gas::PER_PROOF_BYTE,
                per_nullifier: defaults::confidential::gas::PER_NULLIFIER,
                per_commitment: defaults::confidential::gas::PER_COMMITMENT,
            },
        }
    }
}

/// Execution mode override for the FASTPQ prover.
#[derive(Debug, Clone, Copy, PartialEq, Eq, strum::EnumString, strum::Display)]
#[strum(serialize_all = "snake_case")]
pub enum FastpqExecutionMode {
    /// Detect available accelerators and pick CPU/GPU automatically.
    Auto,
    /// Force CPU execution regardless of detected accelerators.
    Cpu,
    /// Prefer GPU execution; falls back to CPU if kernels are unavailable.
    Gpu,
}

impl FastpqExecutionMode {
    fn into_actual(self) -> actual::FastpqExecutionMode {
        match self {
            FastpqExecutionMode::Auto => actual::FastpqExecutionMode::Auto,
            FastpqExecutionMode::Cpu => actual::FastpqExecutionMode::Cpu,
            FastpqExecutionMode::Gpu => actual::FastpqExecutionMode::Gpu,
        }
    }
}

/// Poseidon pipeline override for the FASTPQ prover (user view).
#[derive(Debug, Clone, Copy, PartialEq, Eq, strum::EnumString, strum::Display)]
#[strum(serialize_all = "snake_case")]
pub enum FastpqPoseidonMode {
    /// Follow the execution mode (default behaviour).
    Auto,
    /// Force CPU hashing even when FFT/LDE run on the GPU.
    Cpu,
    /// Prefer GPU hashing regardless of the global execution mode (falls back to CPU if unavailable).
    Gpu,
}

impl FastpqPoseidonMode {
    fn into_actual(self) -> actual::FastpqPoseidonMode {
        match self {
            FastpqPoseidonMode::Auto => actual::FastpqPoseidonMode::Auto,
            FastpqPoseidonMode::Cpu => actual::FastpqPoseidonMode::Cpu,
            FastpqPoseidonMode::Gpu => actual::FastpqPoseidonMode::Gpu,
        }
    }
}

impl json::JsonSerialize for FastpqExecutionMode {
    fn json_serialize(&self, out: &mut String) {
        json::write_json_string(&self.to_string(), out);
    }
}

impl json::JsonDeserialize for FastpqExecutionMode {
    fn json_deserialize(
        parser: &mut json::Parser<'_>,
    ) -> ::core::result::Result<Self, json::Error> {
        let text = parser.parse_string()?;
        Self::from_str(&text).map_err(|err| json::Error::InvalidField {
            field: "fastpq.execution_mode".into(),
            message: err.to_string(),
        })
    }
}

impl json::JsonSerialize for FastpqPoseidonMode {
    fn json_serialize(&self, out: &mut String) {
        json::write_json_string(&self.to_string(), out);
    }
}

impl json::JsonDeserialize for FastpqPoseidonMode {
    fn json_deserialize(
        parser: &mut json::Parser<'_>,
    ) -> ::core::result::Result<Self, json::Error> {
        let text = parser.parse_string()?;
        Self::from_str(&text).map_err(|err| json::Error::InvalidField {
            field: "fastpq.poseidon_mode".into(),
            message: err.to_string(),
        })
    }
}

/// FASTPQ prover configuration (user view).
#[derive(Debug, ReadConfig, Clone)]
pub struct Fastpq {
    /// Execution mode used when initialising the FASTPQ prover backend.
    #[config(
        env = "FASTPQ_EXECUTION_MODE",
        default = "defaults::zk::fastpq::EXECUTION_MODE.parse().unwrap()"
    )]
    pub execution_mode: FastpqExecutionMode,
    /// Poseidon pipeline override (defaults to the execution mode when unset).
    #[config(
        env = "FASTPQ_POSEIDON_MODE",
        default = "defaults::zk::fastpq::POSEIDON_MODE.parse().unwrap()"
    )]
    pub poseidon_mode: FastpqPoseidonMode,
    /// Optional device-class label exported via telemetry (e.g., `apple-m4`, `xeon-rtx-sm80`).
    #[config(env = "FASTPQ_DEVICE_CLASS")]
    pub device_class: Option<String>,
    /// Optional chip-family label derived from benchmark metadata (e.g., `m4`, `xeon-icelake`).
    #[config(env = "FASTPQ_CHIP_FAMILY")]
    pub chip_family: Option<String>,
    /// Optional GPU kind label (e.g., `integrated`, `discrete`).
    #[config(env = "FASTPQ_GPU_KIND")]
    pub gpu_kind: Option<String>,
    /// Optional Metal queue fan-out override (1–4 queues).
    #[config(env = "FASTPQ_METAL_QUEUE_FANOUT")]
    pub metal_queue_fanout: Option<usize>,
    /// Optional Metal queue column threshold override (positive total columns).
    #[config(env = "FASTPQ_METAL_COLUMN_THRESHOLD")]
    pub metal_queue_column_threshold: Option<u32>,
    /// Optional cap on concurrent Metal command buffers (None = heuristic).
    #[config(env = "FASTPQ_METAL_MAX_IN_FLIGHT")]
    pub metal_max_in_flight: Option<usize>,
    /// Optional override for Metal threadgroup width (None = pipeline default).
    #[config(env = "FASTPQ_METAL_THREADGROUP")]
    pub metal_threadgroup_width: Option<u64>,
    /// Enable per-dispatch Metal tracing (developer diagnostic; defaults off).
    #[config(
        env = "FASTPQ_METAL_TRACE",
        default = "defaults::zk::fastpq::METAL_TRACE"
    )]
    pub metal_trace: bool,
    /// Emit verbose Metal device enumeration logs (developer diagnostic; defaults off).
    #[config(
        env = "FASTPQ_DEBUG_METAL_ENUM",
        default = "Boolish::from(defaults::zk::fastpq::METAL_DEBUG_ENUM)"
    )]
    pub metal_debug_enum: Boolish,
    /// Emit verbose fused Poseidon failure diagnostics (developer diagnostic; defaults off).
    #[config(
        env = "FASTPQ_DEBUG_FUSED",
        default = "Boolish::from(defaults::zk::fastpq::METAL_DEBUG_FUSED)"
    )]
    pub metal_debug_fused: Boolish,
}

impl Fastpq {
    fn parse(self) -> actual::Fastpq {
        fn sanitize_label(value: Option<String>) -> Option<String> {
            value.and_then(|value| {
                let trimmed = value.trim();
                if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed.to_owned())
                }
            })
        }

        actual::Fastpq {
            execution_mode: self.execution_mode.into_actual(),
            poseidon_mode: self.poseidon_mode.into_actual(),
            device_class: sanitize_label(self.device_class),
            chip_family: sanitize_label(self.chip_family),
            gpu_kind: sanitize_label(self.gpu_kind),
            metal_queue_fanout: self.metal_queue_fanout.inspect(|value| {
                assert!(
                    (1..=4).contains(value),
                    "fastpq.metal_queue_fanout must be between 1 and 4 (received {value})"
                );
            }),
            metal_queue_column_threshold: self.metal_queue_column_threshold.inspect(|value| {
                assert!(
                    *value != 0,
                    "fastpq.metal_queue_column_threshold must be greater than zero"
                );
            }),
            metal_max_in_flight: self.metal_max_in_flight.inspect(|value| {
                assert!(
                    *value != 0,
                    "fastpq.metal_max_in_flight must be greater than zero"
                );
            }),
            metal_threadgroup_width: self.metal_threadgroup_width.inspect(|value| {
                assert!(
                    *value != 0,
                    "fastpq.metal_threadgroup_width must be greater than zero"
                );
            }),
            metal_trace: self.metal_trace,
            metal_debug_enum: self.metal_debug_enum.into(),
            metal_debug_fused: self.metal_debug_fused.into(),
        }
    }
}

/// IVM/runtime presentation toggles (user view).
#[derive(Debug, ReadConfig, Clone)]
pub struct Ivm {
    /// Compute resource profile name used to cap IVM guest stack budgets.
    #[config(default = "defaults::ivm::memory_budget_profile()")]
    pub memory_budget_profile: Name,
    /// Startup banner settings.
    #[config(nested)]
    pub banner: Banner,
}

impl Ivm {
    fn parse(self) -> actual::Ivm {
        actual::Ivm {
            memory_budget_profile: self.memory_budget_profile,
            banner: self.banner.parse(),
        }
    }
}

impl Default for Ivm {
    fn default() -> Self {
        Self {
            memory_budget_profile: defaults::ivm::memory_budget_profile(),
            banner: Banner::default(),
        }
    }
}

/// Startup banner settings.
#[derive(Debug, ReadConfig, Clone, Copy)]
pub struct Banner {
    /// Whether to render the Norito/IVM startup banner on daemon launch.
    #[config(default = "defaults::ivm::banner::show()")]
    pub show: bool,
    /// Whether to play the retro startup tune when built with the optional `beep` feature.
    #[config(default = "defaults::ivm::banner::beep()")]
    pub beep: bool,
}

impl Banner {
    fn parse(self) -> actual::Banner {
        actual::Banner {
            show: self.show,
            beep: self.beep,
        }
    }
}

impl Default for Banner {
    fn default() -> Self {
        Self {
            show: defaults::ivm::banner::show(),
            beep: defaults::ivm::banner::beep(),
        }
    }
}

/// Norito codec configuration (user view).
///
/// Runtime overrides are ignored by the codec; these values exist to surface
/// the canonical profile in configuration files. Nodes that require different
/// heuristics must rebuild Norito with a custom profile.
/// User-level configuration container for `Norito`.
#[derive(Debug, ReadConfig, Clone, Copy)]
pub struct Norito {
    /// Minimum payload size (bytes) to attempt CPU zstd.
    #[config(
        env = "NORITO_MIN_COMPRESS_BYTES_CPU",
        default = "defaults::norito::MIN_COMPRESS_BYTES_CPU"
    )]
    pub min_compress_bytes_cpu: usize,
    /// Minimum payload size (bytes) to attempt GPU zstd when available.
    #[config(
        env = "NORITO_MIN_COMPRESS_BYTES_GPU",
        default = "defaults::norito::MIN_COMPRESS_BYTES_GPU"
    )]
    pub min_compress_bytes_gpu: usize,
    /// zstd level for medium-size payloads.
    #[config(
        env = "NORITO_ZSTD_LEVEL_SMALL",
        default = "defaults::norito::ZSTD_LEVEL_SMALL"
    )]
    pub zstd_level_small: i32,
    /// zstd level for large payloads.
    #[config(
        env = "NORITO_ZSTD_LEVEL_LARGE",
        default = "defaults::norito::ZSTD_LEVEL_LARGE"
    )]
    pub zstd_level_large: i32,
    /// GPU zstd level.
    #[config(
        env = "NORITO_ZSTD_LEVEL_GPU",
        default = "defaults::norito::ZSTD_LEVEL_GPU"
    )]
    pub zstd_level_gpu: i32,
    /// Size threshold distinguishing small vs large for CPU zstd level.
    #[config(
        env = "NORITO_LARGE_THRESHOLD",
        default = "defaults::norito::LARGE_THRESHOLD"
    )]
    pub large_threshold: usize,
    /// Allow GPU compression offload when compiled and available.
    #[config(
        env = "NORITO_ALLOW_GPU_COMPRESSION",
        default = "defaults::norito::ALLOW_GPU_COMPRESSION"
    )]
    pub allow_gpu_compression: bool,
    /// Reject Norito archives whose declared length exceeds this bound (bytes).
    #[config(
        env = "NORITO_MAX_ARCHIVE_LEN",
        default = "defaults::norito::MAX_ARCHIVE_LEN"
    )]
    pub max_archive_len: u64,
    /// Small-N threshold for AoS vs NCB adaptive selection in Norito columnar helpers.
    #[config(
        env = "NORITO_AOS_NCB_SMALL_N",
        default = "defaults::norito::AOS_NCB_SMALL_N"
    )]
    pub aos_ncb_small_n: usize,
}

impl Norito {
    fn parse(self) -> actual::Norito {
        actual::Norito {
            min_compress_bytes_cpu: self.min_compress_bytes_cpu,
            min_compress_bytes_gpu: self.min_compress_bytes_gpu,
            zstd_level_small: self.zstd_level_small,
            zstd_level_large: self.zstd_level_large,
            zstd_level_gpu: self.zstd_level_gpu,
            large_threshold: self.large_threshold,
            allow_gpu_compression: self.allow_gpu_compression,
            max_archive_len: self.max_archive_len,
            aos_ncb_small_n: self.aos_ncb_small_n,
        }
    }
}

/// CABAC runtime mode requested via configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CabacMode {
    /// CABAC stays disabled even if the binary contains the code paths.
    #[default]
    Disabled,
    /// CABAC can be negotiated per manifest when the build flag is present.
    Adaptive,
    /// CABAC is forced for all profiles (requires licensing + build flag).
    Forced,
}

impl std::fmt::Display for CabacMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let text = match self {
            Self::Disabled => "disabled",
            Self::Adaptive => "adaptive",
            Self::Forced => "forced",
        };
        f.write_str(text)
    }
}

impl FromStr for CabacMode {
    type Err = ();

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "disabled" => Ok(Self::Disabled),
            "adaptive" => Ok(Self::Adaptive),
            "forced" => Ok(Self::Forced),
            _ => Err(()),
        }
    }
}

impl json::JsonSerialize for CabacMode {
    fn json_serialize(&self, out: &mut String) {
        json::write_json_string(&self.to_string(), out);
    }
}

impl json::JsonDeserialize for CabacMode {
    fn json_deserialize(
        parser: &mut json::Parser<'_>,
    ) -> ::core::result::Result<Self, json::Error> {
        let text = parser.parse_string()?;
        Self::from_str(&text).map_err(|()| json::Error::InvalidField {
            field: "streaming.codec.cabac_mode".into(),
            message: format!("invalid CABAC mode `{text}`"),
        })
    }
}

/// User-facing codec toggles (CABAC/trellis gating, rANS artefact path).
#[derive(Debug, Clone, norito::JsonDeserialize)]
pub struct StreamingCodec {
    /// Requested CABAC runtime mode.
    pub cabac_mode: WithOrigin<CabacMode>,
    /// Block sizes that may use trellis scans (empty until claim-avoidance ships).
    pub trellis_blocks: WithOrigin<Vec<u16>>,
    /// Path to the deterministic SignedRansTablesV1 artefact shared across encoders/decoders.
    pub rans_tables_path: WithOrigin<PathBuf>,
    /// Entropy coder advertised in manifests/headers.
    pub entropy_mode: WithOrigin<String>,
    /// Bundle width for the bundled rANS path.
    pub bundle_width: WithOrigin<u8>,
    /// Preferred acceleration backend for bundles.
    pub bundle_accel: WithOrigin<String>,
}

impl ReadConfigTrait for StreamingCodec {
    fn read(reader: &mut ConfigReader) -> FinalWrap<Self>
    where
        Self: Sized,
    {
        let cabac_mode = reader
            .read_parameter::<CabacMode>(["cabac_mode"])
            .value_or_else(|| CabacMode::Disabled)
            .finish_with_origin();
        let trellis_blocks = reader
            .read_parameter::<Vec<u16>>(["trellis_blocks"])
            .value_or_else(Vec::new)
            .finish_with_origin();
        let rans_tables_path = reader
            .read_parameter::<PathBuf>(["rans_tables_path"])
            .value_or_else(defaults::streaming::codec::rans_tables_path)
            .finish_with_origin();
        let entropy_mode = reader
            .read_parameter::<String>(["entropy_mode"])
            .value_or_else(defaults::streaming::codec::entropy_mode)
            .finish_with_origin();
        let bundle_width = reader
            .read_parameter::<u8>(["bundle_width"])
            .value_or_else(defaults::streaming::codec::bundle_width)
            .finish_with_origin();
        let bundle_accel = reader
            .read_parameter::<String>(["bundle_accel"])
            .value_or_else(defaults::streaming::codec::bundle_accel)
            .finish_with_origin();

        FinalWrap::value_fn(move || Self {
            cabac_mode: cabac_mode.unwrap(),
            trellis_blocks: trellis_blocks.unwrap(),
            rans_tables_path: rans_tables_path.unwrap(),
            entropy_mode: entropy_mode.unwrap(),
            bundle_width: bundle_width.unwrap(),
            bundle_accel: bundle_accel.unwrap(),
        })
    }
}

impl StreamingCodec {
    /// Convert the user configuration into the runtime codec toggles, emitting diagnostics for invalid combinations.
    pub fn parse(self, emitter: &mut Emitter<ParseError>) -> Option<actual::StreamingCodec> {
        let cabac_mode = Self::resolve_cabac_mode(self.cabac_mode, emitter)?;
        let trellis_blocks = Self::validate_trellis_blocks(self.trellis_blocks, emitter)?;
        let (rans_tables_path, rans_tables_origin) =
            Self::validate_rans_tables_path(self.rans_tables_path, emitter)?;
        let entropy_mode = Self::parse_entropy_mode(self.entropy_mode, emitter)?;
        let tables_max_width =
            Self::bundle_tables_max_width(&rans_tables_path, rans_tables_origin, emitter)?;
        let bundle_width = Self::validate_bundle_width(
            self.bundle_width,
            entropy_mode,
            tables_max_width,
            emitter,
        )?;
        let bundle_accel = Self::parse_bundle_accel(self.bundle_accel, entropy_mode, emitter)?;

        Some(actual::StreamingCodec {
            cabac_mode,
            trellis_block_sizes: trellis_blocks,
            rans_tables_path,
            entropy_mode,
            bundle_width,
            bundle_accel,
        })
    }

    fn resolve_cabac_mode(
        cabac_mode: WithOrigin<CabacMode>,
        emitter: &mut Emitter<ParseError>,
    ) -> Option<actual::CabacMode> {
        let (mode, origin) = cabac_mode.into_tuple();
        if matches!(mode, CabacMode::Adaptive | CabacMode::Forced)
            && !norito::streaming::CABAC_BUILD_AVAILABLE
        {
            emitter.emit(
                Report::new(ParseError::InvalidStreamingConfig)
                    .attach(
                        "streaming.codec.cabac_mode may only be set to `adaptive` or `forced` \
                         when the binary is built with ENABLE_CABAC=1",
                    )
                    .attach(ConfigValueAndOrigin::new(mode.to_string(), origin)),
            );
            return None;
        }

        Some(match mode {
            CabacMode::Disabled => actual::CabacMode::Disabled,
            CabacMode::Adaptive => actual::CabacMode::Adaptive,
            CabacMode::Forced => actual::CabacMode::Forced,
        })
    }

    fn validate_trellis_blocks(
        trellis_blocks: WithOrigin<Vec<u16>>,
        emitter: &mut Emitter<ParseError>,
    ) -> Option<Vec<u16>> {
        let (blocks, origin) = trellis_blocks.into_tuple();
        if !blocks.is_empty() && !norito::streaming::TRELLIS_BUILD_AVAILABLE {
            emitter.emit(
                Report::new(ParseError::InvalidStreamingConfig)
                    .attach(
                        "streaming.codec.trellis_blocks requires the build-time ENABLE_TRELLIS=1 flag; \
                         keep the list empty until the trellis profile is approved",
                    )
                    .attach(ConfigValueAndOrigin::new(format!("{blocks:?}"), origin)),
            );
            return None;
        }
        for block in &blocks {
            if !matches!(*block, 16 | 32) {
                emitter.emit(
                    Report::new(ParseError::InvalidStreamingConfig).attach(format!(
                        "streaming.codec.trellis_blocks entries must be 16 or 32 (found {block})"
                    )),
                );
                return None;
            }
        }
        Some(blocks)
    }

    fn validate_rans_tables_path(
        rans_tables_path: WithOrigin<PathBuf>,
        emitter: &mut Emitter<ParseError>,
    ) -> Option<(PathBuf, ParameterOrigin)> {
        let (path, origin) = rans_tables_path.into_tuple();
        let resolved = if Path::new(&path).is_file() {
            Some(path.clone())
        } else if path.is_relative() {
            let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .and_then(Path::parent)
                .map(Path::to_path_buf);
            workspace_root
                .as_ref()
                .map(|root| root.join(&path))
                .filter(|candidate| candidate.is_file())
        } else {
            None
        };

        let Some(resolved) = resolved else {
            emitter.emit(
                Report::new(ParseError::InvalidStreamingConfig)
                    .attach(
                        "streaming.codec.rans_tables_path must point to a SignedRansTablesV1 TOML artefact",
                    )
                    .attach(ConfigValueAndOrigin::new(path.display().to_string(), origin)),
            );
            return None;
        };

        Some((resolved, origin))
    }

    fn bundle_tables_max_width(
        rans_tables_path: &Path,
        origin: ParameterOrigin,
        emitter: &mut Emitter<ParseError>,
    ) -> Option<u8> {
        match load_bundle_tables_from_toml(rans_tables_path) {
            Ok(tables) => Some(tables.max_width()),
            Err(source) => {
                emitter.emit(
                    Report::new(ParseError::InvalidStreamingConfig)
                        .attach(
                            "streaming.codec.rans_tables_path must reference a valid SignedRansTablesV1 bundle tables artefact",
                        )
                        .attach(ConfigValueAndOrigin::new(
                            rans_tables_path.display().to_string(),
                            origin,
                        ))
                        .attach(source),
                );
                None
            }
        }
    }

    fn parse_entropy_mode(
        entropy_mode: WithOrigin<String>,
        emitter: &mut Emitter<ParseError>,
    ) -> Option<EntropyMode> {
        let (mode_text, origin) = entropy_mode.into_tuple();
        let normalized = mode_text.trim().to_ascii_lowercase();
        if normalized != "rans_bundled" && normalized != "rans-bundled" {
            emitter.emit(
                Report::new(ParseError::InvalidStreamingConfig)
                    .attach(
                        "streaming.codec.entropy_mode must be `rans_bundled` for the first release",
                    )
                    .attach(ConfigValueAndOrigin::new(mode_text, origin)),
            );
            return None;
        }

        if !norito::streaming::BUNDLED_RANS_BUILD_AVAILABLE {
            emitter.emit(
                Report::new(ParseError::InvalidStreamingConfig)
                    .attach(
                        "streaming.codec.entropy_mode=`rans_bundled` requires builds compiled with ENABLE_RANS_BUNDLES=1",
                    )
                    .attach(ConfigValueAndOrigin::new(mode_text, origin)),
            );
            return None;
        }

        let mode = EntropyMode::from_str(&normalized).ok()?;
        debug_assert_eq!(mode, EntropyMode::RansBundled);

        Some(mode)
    }

    fn validate_bundle_width(
        bundle_width: WithOrigin<u8>,
        entropy_mode: EntropyMode,
        available_width: u8,
        emitter: &mut Emitter<ParseError>,
    ) -> Option<u8> {
        if entropy_mode != EntropyMode::RansBundled {
            emitter.emit(
                Report::new(ParseError::InvalidStreamingConfig)
                    .attach("streaming.codec.entropy_mode must resolve to the bundled profile"),
            );
            return None;
        }
        let (width, origin) = bundle_width.into_tuple();
        if width == 0 || width > available_width {
            emitter.emit(
                Report::new(ParseError::InvalidStreamingConfig)
                    .attach(format!(
                        "streaming.codec.bundle_width must be within 1..={available_width} to match the \
                             bundled rANS tables"
                    ))
                    .attach(ConfigValueAndOrigin::new(width.to_string(), origin)),
            );
            return None;
        }
        if entropy_mode == EntropyMode::RansBundled && width < 2 {
            emitter.emit(
                Report::new(ParseError::InvalidStreamingConfig)
                    .attach(
                        "streaming.codec.bundle_width must be at least 2 when \
                         entropy_mode = `rans_bundled`",
                    )
                    .attach(ConfigValueAndOrigin::new(width.to_string(), origin)),
            );
            return None;
        }

        Some(width)
    }

    fn parse_bundle_accel(
        bundle_accel: WithOrigin<String>,
        entropy_mode: EntropyMode,
        emitter: &mut Emitter<ParseError>,
    ) -> Option<actual::BundleAcceleration> {
        if entropy_mode != EntropyMode::RansBundled {
            emitter.emit(
                Report::new(ParseError::InvalidStreamingConfig)
                    .attach("streaming.codec.entropy_mode must resolve to the bundled profile"),
            );
            return None;
        }
        let (raw_value, origin) = bundle_accel.into_tuple();
        let normalized = raw_value.trim().to_ascii_lowercase().replace('-', "_");
        let accel = match normalized.as_str() {
            "none" => actual::BundleAcceleration::None,
            "cpu_simd" | "cpusimd" => actual::BundleAcceleration::CpuSimd,
            "gpu" => actual::BundleAcceleration::Gpu,
            _ => {
                emitter.emit(
                    Report::new(ParseError::InvalidStreamingConfig)
                        .attach(
                            "streaming.codec.bundle_accel must be one of \
                             `none`, `cpu_simd`, or `gpu`",
                        )
                        .attach(ConfigValueAndOrigin::new(raw_value, origin)),
                );
                return None;
            }
        };
        if accel == actual::BundleAcceleration::Gpu && !BUNDLED_RANS_GPU_BUILD_AVAILABLE {
            emitter.emit(
                Report::new(ParseError::InvalidStreamingConfig)
                    .attach(
                        "streaming.codec.bundle_accel = \"gpu\" requires a GPU-enabled \
                         bundled build (`ENABLE_RANS_BUNDLES=1` plus `codec-gpu-metal` or \
                         `codec-gpu-cuda`); use cpu_simd/none on community builds",
                    )
                    .attach(ConfigValueAndOrigin::new(raw_value, origin)),
            );
            return None;
        }
        Some(accel)
    }
}

#[derive(Debug, thiserror::Error)]
enum ParseQ16Error {
    #[error("value must not be empty")]
    Empty,
    #[error("negative values are not supported")]
    Negative,
    #[error("invalid fixed-point format")]
    InvalidFormat,
    #[error("integer part exceeds 65535")]
    IntegerOverflow,
    #[error("fractional part exceeds 9 digits")]
    TooManyFractionDigits,
    #[error("fractional component overflows the Q16 range")]
    FractionOverflow,
}

fn parse_q16_decimal(value: &str) -> std::result::Result<ModelQ16, ParseQ16Error> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(ParseQ16Error::Empty);
    }
    if trimmed.starts_with('-') {
        return Err(ParseQ16Error::Negative);
    }
    let normalized = trimmed.replace('_', "");
    let mut parts = normalized.split('.');
    let int_part = parts.next().ok_or(ParseQ16Error::InvalidFormat)?;
    let frac_part = parts.next();
    if parts.next().is_some() {
        return Err(ParseQ16Error::InvalidFormat);
    }
    let integer = if int_part.is_empty() {
        0u32
    } else {
        int_part
            .parse::<u32>()
            .map_err(|_| ParseQ16Error::InvalidFormat)?
    };
    if integer > 0xFFFF {
        return Err(ParseQ16Error::IntegerOverflow);
    }
    let mut raw = integer << 16;
    if let Some(frac_str) = frac_part {
        if frac_str.is_empty() {
            return Err(ParseQ16Error::InvalidFormat);
        }
        if !frac_str.chars().all(|c| c.is_ascii_digit()) {
            return Err(ParseQ16Error::InvalidFormat);
        }
        let digits = frac_str.len();
        if digits > 9 {
            return Err(ParseQ16Error::TooManyFractionDigits);
        }
        let frac_value = frac_str
            .parse::<u128>()
            .map_err(|_| ParseQ16Error::InvalidFormat)?;
        let pow = u32::try_from(digits).unwrap_or_else(|_| unreachable!("digits <= 9"));
        let denom = 10_u128.pow(pow);
        let numerator = frac_value
            .checked_mul(1u128 << 16)
            .ok_or(ParseQ16Error::FractionOverflow)?;
        let rounded = (numerator + denom / 2) / denom;
        if rounded > 0xFFFF {
            return Err(ParseQ16Error::FractionOverflow);
        }
        let rounded_u32 = u32::try_from(rounded).map_err(|_| ParseQ16Error::FractionOverflow)?;
        raw |= rounded_u32;
    }
    Ok(ModelQ16::from_raw(raw))
}

#[derive(Debug, thiserror::Error)]
enum HijiriPolicyError {
    #[error("invalid penalty cap: {0}")]
    PenaltyCap(#[source] ParseQ16Error),
    #[error("band {index}: invalid max_risk value: {source}")]
    MaxRisk {
        index: usize,
        #[source]
        source: ParseQ16Error,
    },
    #[error("band {index}: invalid multiplier value: {source}")]
    Multiplier {
        index: usize,
        #[source]
        source: ParseQ16Error,
    },
    #[error("band {index}: {source}")]
    Band {
        index: usize,
        #[source]
        source: ModelFeePolicyError,
    },
    #[error("invalid fee policy: {0}")]
    Policy(#[source] ModelFeePolicyError),
}

/// User-level configuration container for `FeeMultiplierBandConfig`.
#[derive(Debug, ReadConfig, Clone, norito::JsonDeserialize)]
pub struct FeeMultiplierBandConfig {
    /// Upper bound on borrower risk (encoded as Q16) covered by this band.
    pub max_risk: String,
    /// Multiplicative fee adjustment (Q16) applied within this band.
    pub multiplier: String,
}

/// User-level configuration container for `HijiriFeePolicyConfig`.
#[derive(Debug, ReadConfig, Clone, norito::JsonDeserialize)]
pub struct HijiriFeePolicyConfig {
    #[config(default)]
    /// Ordered set of fee multiplier bands.
    pub bands: Vec<FeeMultiplierBandConfig>,
    /// Maximum penalty multiplier allowed for delinquent borrowers.
    pub penalty_cap: String,
}

impl HijiriFeePolicyConfig {
    fn into_actual(self) -> std::result::Result<ModelHijiriFeePolicy, HijiriPolicyError> {
        let penalty_cap =
            parse_q16_decimal(&self.penalty_cap).map_err(HijiriPolicyError::PenaltyCap)?;
        let mut bands_actual = Vec::with_capacity(self.bands.len());
        for (index, band) in self.bands.into_iter().enumerate() {
            let max_risk = parse_q16_decimal(&band.max_risk)
                .map_err(|source| HijiriPolicyError::MaxRisk { index, source })?;
            let multiplier = parse_q16_decimal(&band.multiplier)
                .map_err(|source| HijiriPolicyError::Multiplier { index, source })?;
            let actual_band = ModelFeeMultiplierBand::new(max_risk, multiplier)
                .map_err(|source| HijiriPolicyError::Band { index, source })?;
            bands_actual.push(actual_band);
        }
        ModelHijiriFeePolicy::new(bands_actual, penalty_cap).map_err(HijiriPolicyError::Policy)
    }
}

/// User-level configuration container for `Hijiri`.
#[derive(Debug, ReadConfig, Clone, norito::JsonDeserialize)]
pub struct Hijiri {
    /// Optional Hijiri fee policy that overrides the default pricing.
    pub fee_policy: Option<HijiriFeePolicyConfig>,
}

impl Hijiri {
    fn parse(self, emitter: &mut Emitter<ParseError>) -> Option<actual::Hijiri> {
        let fee_policy = self
            .fee_policy
            .map(|policy| {
                policy.into_actual().map_err(|err| {
                    Report::new(ParseError::InvalidHijiriConfig).attach(err.to_string())
                })
            })
            .transpose()
            .ok_or_emit(emitter)?;
        Some(actual::Hijiri::new(fee_policy))
    }
}

/// User-level enumeration translating `FraudRiskBand` settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, strum::EnumString, strum::Display)]
#[strum(serialize_all = "snake_case")]
pub enum FraudRiskBand {
    /// Indicates low fraud risk assessments.
    Low,
    /// Indicates medium fraud risk assessments.
    Medium,
    /// Indicates high fraud risk assessments.
    High,
    /// Indicates critical fraud risk assessments.
    Critical,
}

impl json::JsonSerialize for FraudRiskBand {
    fn json_serialize(&self, out: &mut String) {
        json::write_json_string(&self.to_string(), out);
    }
}

impl json::JsonDeserialize for FraudRiskBand {
    fn json_deserialize(
        parser: &mut json::Parser<'_>,
    ) -> ::core::result::Result<Self, json::Error> {
        let raw = parser.parse_string()?;
        Self::from_str(&raw).map_err(|err| json::Error::InvalidField {
            field: "fraud_monitoring.required_minimum_band".into(),
            message: err.to_string(),
        })
    }
}

/// User-level configuration container for `FraudMonitoring`.
#[derive(Debug, ReadConfig, Clone, norito::JsonDeserialize)]
pub struct FraudMonitoring {
    #[config(
        env = "FRAUD_MONITORING_ENABLED",
        default = "defaults::fraud_monitoring::ENABLED"
    )]
    /// Master enable switch for the fraud monitoring client.
    pub enabled: bool,
    #[config(default = "Vec::new()")]
    /// Fraud monitoring REST endpoints the node should query.
    pub service_endpoints: Vec<Url>,
    #[config(
        env = "FRAUD_MONITORING_CONNECT_TIMEOUT_MS",
        default = "defaults::fraud_monitoring::CONNECT_TIMEOUT.into()"
    )]
    /// Connection timeout for reaching a fraud monitoring endpoint.
    pub connect_timeout_ms: DurationMs,
    #[config(
        env = "FRAUD_MONITORING_REQUEST_TIMEOUT_MS",
        default = "defaults::fraud_monitoring::REQUEST_TIMEOUT.into()"
    )]
    /// Timeout applied to an in-flight fraud monitoring request.
    pub request_timeout_ms: DurationMs,
    #[config(
        env = "FRAUD_MONITORING_MISSING_ASSESSMENT_GRACE_SECS",
        default = "defaults::fraud_monitoring::MISSING_ASSESSMENT_GRACE_SECS"
    )]
    /// Grace period (seconds) tolerated without an updated risk assessment.
    pub missing_assessment_grace_secs: u64,
    /// Lowest acceptable risk band before flagging an account as non-compliant.
    pub required_minimum_band: Option<FraudRiskBand>,
    #[config(default = "Vec::new()")]
    /// Registered attesters whose signatures must verify assessment envelopes.
    pub attesters: Vec<FraudAttester>,
}

impl FraudMonitoring {
    fn parse(self) -> actual::FraudMonitoring {
        actual::FraudMonitoring::new(
            self.enabled,
            self.service_endpoints,
            self.connect_timeout_ms.get(),
            self.request_timeout_ms.get(),
            self.missing_assessment_grace_secs,
            self.required_minimum_band.map(actual::FraudRiskBand::from),
            self.attesters
                .into_iter()
                .map(|attester| actual::FraudAttester {
                    engine_id: attester.engine_id,
                    public_key: attester.public_key,
                })
                .collect(),
        )
    }
}

/// User-level definition of a fraud assessment attester.
#[derive(Debug, ReadConfig, Clone, norito::JsonDeserialize)]
pub struct FraudAttester {
    /// Engine identifier embedded in signed assessments.
    pub engine_id: String,
    /// Public key used to verify signatures (hex/base64 supported by PublicKey parser).
    pub public_key: iroha_crypto::PublicKey,
}

/// Supported curves for Halo2 verification (user view).
/// User-level enumeration translating `ZkCurve` settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, strum::EnumString, strum::Display)]
#[strum(serialize_all = "snake_case")]
pub enum ZkCurve {
    /// Uses the Pallas curve (Pasta cycle) for Halo2 circuits.
    #[strum(serialize = "toy_p61_additive")]
    Pallas,
    /// Uses the Pasta curve (Vesta/Pallas cycle) for Halo2 circuits.
    Pasta,
    /// Uses the Goldilocks curve for Halo2 circuits.
    Goldilocks,
    /// Uses the BN254 curve for Halo2 circuits.
    Bn254,
}

impl ZkCurve {
    fn into_actual(self) -> actual::ZkCurve {
        match self {
            ZkCurve::Pallas => actual::ZkCurve::Pallas,
            ZkCurve::Pasta => actual::ZkCurve::Pasta,
            ZkCurve::Goldilocks => actual::ZkCurve::Goldilocks,
            ZkCurve::Bn254 => actual::ZkCurve::Bn254,
        }
    }
}

impl json::JsonSerialize for ZkCurve {
    fn json_serialize(&self, out: &mut String) {
        json::write_json_string(&self.to_string(), out);
    }
}

impl json::JsonDeserialize for ZkCurve {
    fn json_deserialize(
        parser: &mut json::Parser<'_>,
    ) -> ::core::result::Result<Self, json::Error> {
        let text = parser.parse_string()?;
        Self::from_str(&text).map_err(|err| json::Error::InvalidField {
            field: "zk_curve".into(),
            message: err.to_string(),
        })
    }
}

/// Transparent PCS backend kinds.
/// User-level enumeration translating `ZkHalo2Backend` settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, strum::EnumString, strum::Display)]
#[strum(serialize_all = "snake_case")]
pub enum ZkHalo2Backend {
    /// Halo2 backend powered by the inner-product argument (IPA) commitment scheme.
    Ipa,
}

impl ZkHalo2Backend {
    fn into_actual(self) -> actual::Halo2Backend {
        match self {
            ZkHalo2Backend::Ipa => actual::Halo2Backend::Ipa,
        }
    }
}

impl json::JsonSerialize for ZkHalo2Backend {
    fn json_serialize(&self, out: &mut String) {
        json::write_json_string(&self.to_string(), out);
    }
}

impl json::JsonDeserialize for ZkHalo2Backend {
    fn json_deserialize(
        parser: &mut json::Parser<'_>,
    ) -> ::core::result::Result<Self, json::Error> {
        let text = parser.parse_string()?;
        Self::from_str(&text).map_err(|err| json::Error::InvalidField {
            field: "zk_halo2_backend".into(),
            message: err.to_string(),
        })
    }
}

/// Halo2 transparent verification configuration (user view).
/// User-level configuration container for `Halo2`.
#[derive(Debug, ReadConfig, Clone, Copy)]
pub struct Halo2 {
    /// Enable Halo2 verification in hosts.
    #[config(env = "ZK_HALO2_ENABLED", default = "defaults::zk::halo2::ENABLED")]
    pub enabled: bool,
    /// Select the curve backend.
    #[config(
        env = "ZK_HALO2_CURVE",
        default = "defaults::zk::halo2::CURVE.parse().unwrap()"
    )]
    pub curve: ZkCurve,
    /// Select the transparent PCS backend.
    #[config(
        env = "ZK_HALO2_BACKEND",
        default = "defaults::zk::halo2::BACKEND.parse().unwrap()"
    )]
    pub backend: ZkHalo2Backend,
    /// Maximum circuit size exponent k (domain size N = 2^k).
    #[config(env = "ZK_HALO2_MAX_K", default = "defaults::zk::halo2::MAX_K")]
    pub max_k: u32,
    /// Soft time budget for a single verification (ms).
    #[config(
        env = "ZK_HALO2_VERIFIER_BUDGET_MS",
        default = "defaults::zk::halo2::VERIFIER_BUDGET_MS"
    )]
    pub verifier_budget_ms: u64,
    /// Maximum proofs per batch verification.
    #[config(
        env = "ZK_HALO2_VERIFIER_MAX_BATCH",
        default = "defaults::zk::halo2::VERIFIER_MAX_BATCH"
    )]
    pub verifier_max_batch: u32,
    /// Number of worker threads serving ZK lane verification (0 = auto).
    #[config(
        env = "ZK_HALO2_VERIFIER_WORKER_THREADS",
        default = "defaults::zk::halo2::VERIFIER_WORKER_THREADS"
    )]
    pub verifier_worker_threads: usize,
    /// Capacity of the ZK lane verifier queue (0 = auto-derived).
    #[config(
        env = "ZK_HALO2_VERIFIER_QUEUE_CAP",
        default = "defaults::zk::halo2::VERIFIER_QUEUE_CAP"
    )]
    pub verifier_queue_cap: usize,
    /// Maximum enqueue wait for ZK lane admission under saturation (ms).
    #[config(
        env = "ZK_HALO2_VERIFIER_ENQUEUE_WAIT_MS",
        default = "defaults::zk::halo2::VERIFIER_ENQUEUE_WAIT_MS"
    )]
    pub verifier_enqueue_wait_ms: u64,
    /// Capacity of the in-memory retry ring used for important ZK lane tasks.
    #[config(
        env = "ZK_HALO2_VERIFIER_RETRY_RING_CAP",
        default = "defaults::zk::halo2::VERIFIER_RETRY_RING_CAP"
    )]
    pub verifier_retry_ring_cap: usize,
    /// Maximum retry rounds for an item in the ZK lane retry ring.
    #[config(
        env = "ZK_HALO2_VERIFIER_RETRY_MAX_ATTEMPTS",
        default = "defaults::zk::halo2::VERIFIER_RETRY_MAX_ATTEMPTS"
    )]
    pub verifier_retry_max_attempts: u32,
    /// Retry scheduler tick interval for the ZK lane (ms).
    #[config(
        env = "ZK_HALO2_VERIFIER_RETRY_TICK_MS",
        default = "defaults::zk::halo2::VERIFIER_RETRY_TICK_MS"
    )]
    pub verifier_retry_tick_ms: u64,
    /// Maximum accepted Norito envelope payload length (bytes).
    #[config(
        env = "ZK_HALO2_MAX_ENVELOPE_BYTES",
        default = "defaults::zk::halo2::MAX_ENVELOPE_BYTES"
    )]
    pub max_envelope_bytes: usize,
    /// Maximum accepted proof payload length (bytes).
    #[config(
        env = "ZK_HALO2_MAX_PROOF_BYTES",
        default = "defaults::zk::halo2::MAX_PROOF_BYTES"
    )]
    pub max_proof_bytes: usize,
    /// Maximum allowed transcript label length (bytes).
    #[config(
        env = "ZK_HALO2_MAX_TRANSCRIPT_LABEL_LEN",
        default = "defaults::zk::halo2::MAX_TRANSCRIPT_LABEL_LEN"
    )]
    pub max_transcript_label_len: usize,
    /// Require transcript labels to be ASCII.
    #[config(
        env = "ZK_HALO2_ENFORCE_TRANSCRIPT_LABEL_ASCII",
        default = "defaults::zk::halo2::ENFORCE_TRANSCRIPT_LABEL_ASCII"
    )]
    pub enforce_transcript_label_ascii: bool,
}

impl Halo2 {
    fn parse(self) -> actual::Halo2 {
        actual::Halo2 {
            enabled: self.enabled,
            curve: self.curve.into_actual(),
            backend: self.backend.into_actual(),
            max_k: self.max_k,
            verifier_budget_ms: self.verifier_budget_ms,
            verifier_max_batch: self.verifier_max_batch,
            verifier_worker_threads: self.verifier_worker_threads,
            verifier_queue_cap: self.verifier_queue_cap,
            verifier_enqueue_wait_ms: self.verifier_enqueue_wait_ms,
            verifier_retry_ring_cap: self.verifier_retry_ring_cap,
            verifier_retry_max_attempts: self.verifier_retry_max_attempts,
            verifier_retry_tick_ms: self.verifier_retry_tick_ms,
            max_envelope_bytes: self.max_envelope_bytes,
            max_proof_bytes: self.max_proof_bytes,
            max_transcript_label_len: self.max_transcript_label_len,
            enforce_transcript_label_ascii: self.enforce_transcript_label_ascii,
        }
    }
}

/// Native STARK/FRI verification configuration (user view).
/// User-level configuration container for `Stark`.
#[derive(Debug, ReadConfig, Clone, Copy)]
pub struct Stark {
    /// Enable native STARK/FRI verification in hosts.
    ///
    /// Note: runtime enablement requires binaries built with `zk-stark`.
    #[config(env = "ZK_STARK_ENABLED", default = "defaults::zk::stark::ENABLED")]
    pub enabled: bool,
    /// Maximum accepted proof payload length (bytes).
    #[config(
        env = "ZK_STARK_MAX_PROOF_BYTES",
        default = "defaults::zk::stark::MAX_PROOF_BYTES"
    )]
    pub max_proof_bytes: usize,
}

impl Stark {
    fn parse(self) -> actual::Stark {
        actual::Stark {
            enabled: self.enabled,
            max_proof_bytes: self.max_proof_bytes,
        }
    }
}

/// User-level configuration container for `Gas`.
#[derive(Debug, ReadConfig, Clone, norito::JsonDeserialize)]
pub struct Gas {
    /// System-owned technical account that receives gas fee transfers.
    #[config(
        env = "PIPELINE_GAS_TECH_ACCOUNT_ID",
        default = "defaults::pipeline::GAS_TECH_ACCOUNT_ID.to_string()"
    )]
    pub tech_account_id: String,
    /// Allowlist of accepted gas assets (Asset IDs strings, e.g., "asset:gas/G_exit@ivm.core/v2").
    #[config(default)]
    pub accepted_assets: Vec<String>,
    /// Deterministic conversion: minimal asset units per one gas unit for each accepted asset.
    #[config(default)]
    #[allow(clippy::struct_field_names)]
    pub units_per_gas: Vec<GasRate>,
}

impl Gas {
    fn parse(self) -> actual::Gas {
        actual::Gas {
            tech_account_id: self.tech_account_id,
            accepted_assets: self.accepted_assets,
            units_per_gas: self
                .units_per_gas
                .into_iter()
                .map(|r| {
                    let asset = r.asset;
                    let twap = r
                        .twap_local_per_xor
                        .as_deref()
                        .map_or(Decimal::ONE, |value| {
                            Decimal::from_str(value).unwrap_or_else(|error| {
                                panic!(
                                    "invalid pipeline.gas.units_per_gas twap `{value}` for asset `{asset}`: {error}"
                                )
                            })
                        });
                    let liquidity = r.liquidity_profile.as_deref().map_or_else(
                        actual::GasLiquidity::default,
                        |value| {
                            actual::GasLiquidity::from_str(value).unwrap_or_else(|()| {
                                panic!(
                                    "invalid pipeline.gas.units_per_gas liquidity `{value}` for asset `{asset}`"
                                )
                            })
                        },
                    );
                    let volatility = r.volatility_class.as_deref().map_or_else(
                        actual::GasVolatility::default,
                        |value| {
                            actual::GasVolatility::from_str(value).unwrap_or_else(|()| {
                                panic!(
                                    "invalid pipeline.gas.units_per_gas volatility `{value}` for asset `{asset}`"
                                )
                            })
                        },
                    );
                    actual::GasRate {
                        asset,
                        units_per_gas: r.units_per_gas,
                        twap_local_per_xor: twap,
                        liquidity,
                        volatility,
                    }
                })
                .collect(),
        }
    }
}

/// User-level configuration container for `GasRate`.
#[derive(Debug, ReadConfig, Clone, norito::JsonDeserialize)]
pub struct GasRate {
    /// Asset ID string (must be present in accepted_assets)
    pub asset: String,
    /// Asset minimal units per one gas unit (deterministic integer mapping)
    pub units_per_gas: u64,
    /// Override for the TWAP in local units per XOR.
    pub twap_local_per_xor: Option<String>,
    /// Override for the liquidity profile (`tier1`, `tier2`, `tier3`).
    pub liquidity_profile: Option<String>,
    /// Override for the volatility bucket (`stable`, `elevated`, `dislocated`).
    pub volatility_class: Option<String>,
}

/// User-level configuration container for `Genesis`.
#[derive(Debug, ReadConfig)]
pub struct Genesis {
    /// Public key expected to sign the genesis block payload.
    #[config(env = "GENESIS_PUBLIC_KEY")]
    pub public_key: WithOrigin<PublicKey>,
    /// Optional path to the genesis block payload (`.json` or `.norito`).
    #[config(env = "GENESIS")]
    pub file: Option<WithOrigin<PathBuf>>,
    /// Optional path to genesis manifest JSON for startup validation.
    #[config(env = "GENESIS_MANIFEST_JSON")]
    pub manifest_json: Option<WithOrigin<PathBuf>>,
    /// Optional expected genesis block hash used during bootstrap preflight.
    #[config(env = "GENESIS_EXPECTED_HASH")]
    pub expected_hash: Option<WithOrigin<HashOf<BlockHeader>>>,
    /// Optional explicit peer allowlist that may serve genesis during bootstrap. Defaults to trusted peers.
    #[config(default)]
    pub bootstrap_allowlist: Vec<PeerId>,
    /// Maximum genesis payload size accepted from peers during bootstrap.
    #[config(default = "defaults::genesis::BOOTSTRAP_MAX_BYTES")]
    pub bootstrap_max_bytes: Bytes<u64>,
    /// Minimum interval between serving bootstrap responses to avoid abuse.
    #[config(default = "defaults::genesis::BOOTSTRAP_RESPONSE_THROTTLE.into()")]
    pub bootstrap_response_throttle_ms: DurationMs,
    /// Timeout per bootstrap round-trip (preflight or payload).
    #[config(default = "defaults::genesis::BOOTSTRAP_REQUEST_TIMEOUT.into()")]
    pub bootstrap_request_timeout_ms: DurationMs,
    /// Retry/backoff interval between bootstrap attempts.
    #[config(default = "defaults::genesis::BOOTSTRAP_RETRY_INTERVAL.into()")]
    pub bootstrap_retry_interval_ms: DurationMs,
    /// Maximum bootstrap attempts before failing.
    #[config(default = "defaults::genesis::BOOTSTRAP_MAX_ATTEMPTS")]
    pub bootstrap_max_attempts: u32,
    /// Whether to attempt network bootstrap when local genesis is missing.
    #[config(default = "true")]
    pub bootstrap_enabled: bool,
}

impl From<Genesis> for actual::Genesis {
    fn from(genesis: Genesis) -> Self {
        actual::Genesis {
            public_key: genesis.public_key.into_value(),
            file: genesis.file,
            manifest_json: genesis.manifest_json,
            expected_hash: genesis.expected_hash.map(WithOrigin::into_value),
            bootstrap_allowlist: genesis.bootstrap_allowlist,
            bootstrap_max_bytes: genesis.bootstrap_max_bytes.get(),
            bootstrap_response_throttle: genesis.bootstrap_response_throttle_ms.get(),
            bootstrap_request_timeout: genesis.bootstrap_request_timeout_ms.get(),
            bootstrap_retry_interval: genesis.bootstrap_retry_interval_ms.get(),
            bootstrap_max_attempts: genesis.bootstrap_max_attempts,
            bootstrap_enabled: genesis.bootstrap_enabled,
        }
    }
}

/// User-level configuration container for `Kura`.
#[derive(Debug, ReadConfig)]
pub struct Kura {
    /// Initialisation mode controlling whether to reuse or reset existing storage.
    #[config(env = "KURA_INIT_MODE", default)]
    pub init_mode: KuraInitMode,
    /// Directory where Kura stores blocks and auxiliary indices.
    #[config(
        env = "KURA_STORE_DIR",
        default = "PathBuf::from(defaults::kura::STORE_DIR)"
    )]
    pub store_dir: WithOrigin<PathBuf>,
    /// Maximum on-disk footprint for Kura (bytes, 0 = unlimited).
    #[config(
        env = "KURA_MAX_DISK_USAGE_BYTES",
        default = "defaults::kura::MAX_DISK_USAGE_BYTES"
    )]
    pub max_disk_usage_bytes: Bytes<u64>,
    /// Number of most-recent blocks kept in memory for fast access.
    #[config(
        env = "KURA_BLOCKS_IN_MEMORY",
        default = "defaults::kura::BLOCKS_IN_MEMORY"
    )]
    pub blocks_in_memory: NonZeroUsize,
    /// Number of recent roster records retained for block-sync validation.
    #[config(
        env = "KURA_BLOCK_SYNC_ROSTER_RETENTION",
        default = "defaults::kura::BLOCK_SYNC_ROSTER_RETENTION"
    )]
    pub block_sync_roster_retention: NonZeroUsize,
    /// Number of recent roster sidecars retained alongside the block store.
    #[config(
        env = "KURA_ROSTER_SIDECAR_RETENTION",
        default = "defaults::kura::ROSTER_SIDECAR_RETENTION"
    )]
    pub roster_sidecar_retention: NonZeroUsize,
    /// Capacity of the merge-ledger cache used during compaction.
    #[config(
        env = "KURA_MERGE_LEDGER_CACHE_CAPACITY",
        default = "defaults::kura::MERGE_LEDGER_CACHE_CAPACITY"
    )]
    pub merge_ledger_cache_capacity: usize,
    /// Fsync policy for block persistence.
    #[config(env = "KURA_FSYNC_MODE", default = "defaults::kura::FSYNC_MODE")]
    pub fsync_mode: KuraFsyncMode,
    /// Interval for batched fsync operations.
    #[config(
        env = "KURA_FSYNC_INTERVAL_MS",
        default = "defaults::kura::FSYNC_INTERVAL.into()"
    )]
    pub fsync_interval_ms: DurationMs,
    /// Debug controls for development/testing scenarios.
    #[config(nested)]
    pub debug: KuraDebug,
}

impl Kura {
    fn parse(self) -> actual::Kura {
        let Self {
            init_mode,
            store_dir,
            max_disk_usage_bytes,
            blocks_in_memory,
            block_sync_roster_retention,
            roster_sidecar_retention,
            merge_ledger_cache_capacity,
            fsync_mode,
            fsync_interval_ms,
            debug:
                KuraDebug {
                    output_new_blocks: debug_output_new_blocks,
                },
        } = self;

        actual::Kura {
            init_mode,
            store_dir,
            max_disk_usage_bytes,
            blocks_in_memory,
            block_sync_roster_retention,
            roster_sidecar_retention,
            debug_output_new_blocks,
            merge_ledger_cache_capacity,
            fsync_mode,
            fsync_interval: fsync_interval_ms.0,
        }
    }
}

/// User-level configuration container for `KuraDebug`.
#[derive(Debug, Clone, Copy, ReadConfig)]
pub struct KuraDebug {
    #[config(env = "KURA_DEBUG_OUTPUT_NEW_BLOCKS", default)]
    output_new_blocks: bool,
}

/// User-level configuration for adaptive observability hooks in Sumeragi.
#[derive(Debug, Clone, Copy, ReadConfig)]
pub struct AdaptiveObservability {
    /// Enable adaptive mitigation of collector fan-out and pacemaker intervals when telemetry degrades.
    #[config(
        env = "SUMERAGI_ADAPTIVE_OBSERVABILITY_ENABLED",
        default = "defaults::sumeragi::ADAPTIVE_OBSERVABILITY_ENABLED"
    )]
    pub enabled: bool,
    /// QC latency (ms) above which mitigation is triggered.
    #[config(
        env = "SUMERAGI_ADAPTIVE_QC_LATENCY_ALERT_MS",
        default = "defaults::sumeragi::ADAPTIVE_QC_LATENCY_ALERT_MS"
    )]
    pub qc_latency_alert_ms: u64,
    /// Minimum number of missing-availability warnings observed between checks to trigger mitigation.
    #[config(
        env = "SUMERAGI_ADAPTIVE_DA_RESCHEDULE_BURST",
        default = "defaults::sumeragi::ADAPTIVE_DA_RESCHEDULE_BURST"
    )]
    pub da_reschedule_burst: u64,
    /// Additional pacemaker interval (ms) applied when mitigation is active.
    #[config(
        env = "SUMERAGI_ADAPTIVE_PACEMAKER_EXTRA_MS",
        default = "defaults::sumeragi::ADAPTIVE_PACEMAKER_EXTRA_MS"
    )]
    pub pacemaker_extra_ms: u64,
    /// Redundant collector fan-out cap when mitigation is active.
    #[config(
        env = "SUMERAGI_ADAPTIVE_COLLECTOR_REDUNDANT_R",
        default = "defaults::sumeragi::ADAPTIVE_COLLECTOR_REDUNDANT_R"
    )]
    pub collector_redundant_r: u8,
    /// Cooldown (ms) before mitigation can re-trigger or reset.
    #[config(
        env = "SUMERAGI_ADAPTIVE_COOLDOWN_MS",
        default = "defaults::sumeragi::ADAPTIVE_COOLDOWN_MS"
    )]
    pub cooldown_ms: u64,
}

/// User-level configuration for deterministic pacing governor.
#[derive(Debug, Clone, Copy, ReadConfig)]
pub struct SumeragiPacingGovernor {
    /// Number of recent blocks to sample for pressure evaluation.
    #[config(
        env = "SUMERAGI_PACING_GOVERNOR_WINDOW_BLOCKS",
        default = "defaults::sumeragi::PACING_GOVERNOR_WINDOW_BLOCKS"
    )]
    pub window_blocks: usize,
    /// View-change pressure threshold (permille of view-change increments per block).
    #[config(
        env = "SUMERAGI_PACING_GOVERNOR_VIEW_CHANGE_PRESSURE_PERMILLE",
        default = "defaults::sumeragi::PACING_GOVERNOR_VIEW_CHANGE_PRESSURE_PERMILLE"
    )]
    pub view_change_pressure_permille: u32,
    /// View-change clear threshold (permille of view-change increments per block).
    #[config(
        env = "SUMERAGI_PACING_GOVERNOR_VIEW_CHANGE_CLEAR_PERMILLE",
        default = "defaults::sumeragi::PACING_GOVERNOR_VIEW_CHANGE_CLEAR_PERMILLE"
    )]
    pub view_change_clear_permille: u32,
    /// Commit spacing pressure threshold (permille of target block time).
    #[config(
        env = "SUMERAGI_PACING_GOVERNOR_COMMIT_SPACING_PRESSURE_PERMILLE",
        default = "defaults::sumeragi::PACING_GOVERNOR_COMMIT_SPACING_PRESSURE_PERMILLE"
    )]
    pub commit_spacing_pressure_permille: u32,
    /// Commit spacing clear threshold (permille of target block time).
    #[config(
        env = "SUMERAGI_PACING_GOVERNOR_COMMIT_SPACING_CLEAR_PERMILLE",
        default = "defaults::sumeragi::PACING_GOVERNOR_COMMIT_SPACING_CLEAR_PERMILLE"
    )]
    pub commit_spacing_clear_permille: u32,
    /// Pacing-factor increase step (basis points).
    #[config(
        env = "SUMERAGI_PACING_GOVERNOR_STEP_UP_BPS",
        default = "defaults::sumeragi::PACING_GOVERNOR_STEP_UP_BPS"
    )]
    pub step_up_bps: u32,
    /// Pacing-factor decrease step (basis points).
    #[config(
        env = "SUMERAGI_PACING_GOVERNOR_STEP_DOWN_BPS",
        default = "defaults::sumeragi::PACING_GOVERNOR_STEP_DOWN_BPS"
    )]
    pub step_down_bps: u32,
    /// Minimum pacing-factor bound (basis points).
    #[config(
        env = "SUMERAGI_PACING_GOVERNOR_MIN_FACTOR_BPS",
        default = "defaults::sumeragi::PACING_GOVERNOR_MIN_FACTOR_BPS"
    )]
    pub min_factor_bps: u32,
    /// Maximum pacing-factor bound (basis points).
    #[config(
        env = "SUMERAGI_PACING_GOVERNOR_MAX_FACTOR_BPS",
        default = "defaults::sumeragi::PACING_GOVERNOR_MAX_FACTOR_BPS"
    )]
    pub max_factor_bps: u32,
}

/// User-level configuration container for `SumeragiModeFlip`.
#[derive(Debug, Clone, Copy, ReadConfig)]
pub struct SumeragiModeFlip {
    /// Enable runtime consensus mode flips driven by on-chain parameters and activation heights.
    #[config(
        env = "SUMERAGI_MODE_FLIP_ENABLED",
        default = "defaults::sumeragi::MODE_FLIP_ENABLED"
    )]
    pub enabled: bool,
}

/// User-level configuration container for `SumeragiCollectors`.
#[derive(Debug, Clone, Copy, ReadConfig)]
pub struct SumeragiCollectors {
    /// Number of collectors (K) concurrently eligible to aggregate votes.
    #[config(
        env = "SUMERAGI_COLLECTORS_K",
        default = "defaults::sumeragi::COLLECTORS_K"
    )]
    pub k: usize,
    /// Redundant send fanout (r): how many distinct collectors a validator
    /// will send its vote to over time on timeouts. On-chain parameters are
    /// authoritative; config fallback remains 1 if unset.
    #[config(
        env = "SUMERAGI_COLLECTORS_REDUNDANT_SEND_R",
        default = "defaults::sumeragi::COLLECTORS_REDUNDANT_SEND_R"
    )]
    pub redundant_send_r: u8,
    /// Additional topology fanout alongside collector routing (0 = disabled).
    #[config(
        env = "SUMERAGI_COLLECTORS_PARALLEL_TOPOLOGY_FANOUT",
        default = "defaults::sumeragi::COLLECTORS_PARALLEL_TOPOLOGY_FANOUT"
    )]
    pub parallel_topology_fanout: usize,
}

/// User-level configuration container for `SumeragiBlock`.
#[derive(Debug, Clone, Copy, ReadConfig)]
pub struct SumeragiBlock {
    /// Optional cap on transactions included in a block (`None` = unlimited).
    #[config(env = "SUMERAGI_BLOCK_MAX_TRANSACTIONS")]
    pub max_transactions: Option<NonZeroUsize>,
    /// Optional cap on block gas limit when commit time is fast (`None` = disabled).
    #[config(env = "SUMERAGI_BLOCK_FAST_GAS_LIMIT_PER_BLOCK")]
    pub fast_gas_limit_per_block: Option<NonZeroU64>,
    /// Optional cap on payload bytes per block when RBC is disabled (`None` = unlimited).
    #[config(env = "SUMERAGI_BLOCK_MAX_PAYLOAD_BYTES")]
    pub max_payload_bytes: Option<NonZeroUsize>,
    /// Multiplier applied to the proposal queue scan budget (relative to max tx per block).
    #[config(
        env = "SUMERAGI_PROPOSAL_QUEUE_SCAN_MULTIPLIER",
        default = "defaults::sumeragi::PROPOSAL_QUEUE_SCAN_MULTIPLIER"
    )]
    pub proposal_queue_scan_multiplier: NonZeroUsize,
}

/// User-level configuration container for advanced Sumeragi overrides.
///
/// These knobs are intended for targeted tuning; defaults are provided elsewhere
/// and should be sufficient for typical deployments.
#[derive(Debug, Clone, ReadConfig)]
pub struct SumeragiAdvanced {
    /// Consensus queue capacities (override-only).
    #[config(nested)]
    pub queues: SumeragiQueues,
    /// Worker-loop scheduling limits (override-only).
    #[config(nested)]
    pub worker: SumeragiWorker,
    /// Pacemaker tuning (override-only).
    #[config(nested)]
    pub pacemaker: SumeragiPacemaker,
    /// Deterministic pacing governor overrides.
    #[config(nested)]
    pub pacing_governor: SumeragiPacingGovernor,
    /// DA timeout multipliers/floor overrides.
    #[config(nested)]
    pub da: SumeragiDaAdvanced,
    /// RBC tuning overrides.
    #[config(nested)]
    pub rbc: SumeragiRbc,
    /// NPoS timeout overrides (derived from on-chain `SumeragiParameters.block_time_ms` when unset).
    #[config(nested)]
    pub npos: SumeragiAdvancedNpos,
}

/// User-level configuration container for `SumeragiQueues`.
#[derive(Debug, Clone, Copy, ReadConfig)]
pub struct SumeragiQueues {
    /// Capacity for the vote message channel.
    #[config(default = "defaults::sumeragi::MSG_CHANNEL_CAP_VOTES")]
    pub votes: usize,
    /// Capacity for the block payload channel.
    #[config(default = "defaults::sumeragi::MSG_CHANNEL_CAP_BLOCK_PAYLOAD")]
    pub block_payload: usize,
    /// Capacity for the RBC chunk channel.
    #[config(default = "defaults::sumeragi::MSG_CHANNEL_CAP_RBC_CHUNKS")]
    pub rbc_chunks: usize,
    /// Capacity for the fast-path block message channel (BlockCreated, FetchPendingBlock, RBC INIT, params).
    #[config(default = "defaults::sumeragi::MSG_CHANNEL_CAP_BLOCKS")]
    pub blocks: usize,
    /// Capacity for Sumeragi control-message channel.
    #[config(default = "defaults::sumeragi::CONTROL_MSG_CHANNEL_CAP")]
    pub control: usize,
}

/// User-level configuration container for `SumeragiWorker`.
#[derive(Debug, Clone, Copy, ReadConfig)]
pub struct SumeragiWorker {
    /// Cap (ms) on the worker loop's per-iteration time budget.
    #[config(
        env = "SUMERAGI_WORKER_ITERATION_BUDGET_CAP_MS",
        default = "defaults::sumeragi::WORKER_ITERATION_BUDGET_CAP_MS"
    )]
    pub iteration_budget_cap_ms: u64,
    /// Cap (ms) on the worker loop's per-iteration drain budget.
    #[config(
        env = "SUMERAGI_WORKER_ITERATION_DRAIN_BUDGET_CAP_MS",
        default = "defaults::sumeragi::WORKER_ITERATION_DRAIN_BUDGET_CAP_MS"
    )]
    pub iteration_drain_budget_cap_ms: u64,
    /// Cap (ms) on per-tick proposal/commit work (0 disables).
    #[config(
        env = "SUMERAGI_WORKER_TICK_WORK_BUDGET_CAP_MS",
        default = "defaults::sumeragi::WORKER_TICK_WORK_BUDGET_CAP_MS"
    )]
    pub tick_work_budget_cap_ms: u64,
    /// Enable per-queue parallel ingress workers for the Sumeragi loop.
    #[config(
        env = "SUMERAGI_WORKER_PARALLEL_INGRESS",
        default = "defaults::sumeragi::WORKER_PARALLEL_INGRESS"
    )]
    pub parallel_ingress: bool,
    /// Validation worker threads for pre-vote checks (0 = auto).
    #[config(
        env = "SUMERAGI_VALIDATION_WORKER_THREADS",
        default = "defaults::sumeragi::VALIDATION_WORKER_THREADS"
    )]
    pub validation_worker_threads: usize,
    /// Validation work queue capacity per worker (0 = auto).
    #[config(
        env = "SUMERAGI_VALIDATION_WORK_QUEUE_CAP",
        default = "defaults::sumeragi::VALIDATION_WORK_QUEUE_CAP"
    )]
    pub validation_work_queue_cap: usize,
    /// Validation result queue capacity (shared, 0 = auto).
    #[config(
        env = "SUMERAGI_VALIDATION_RESULT_QUEUE_CAP",
        default = "defaults::sumeragi::VALIDATION_RESULT_QUEUE_CAP"
    )]
    pub validation_result_queue_cap: usize,
    /// Divisor used to derive queue-full inline-validation cutover from fast-timeout.
    #[config(
        env = "SUMERAGI_VALIDATION_QUEUE_FULL_INLINE_CUTOVER_DIVISOR",
        default = "defaults::sumeragi::VALIDATION_QUEUE_FULL_INLINE_CUTOVER_DIVISOR"
    )]
    pub validation_queue_full_inline_cutover_divisor: u32,
    /// QC verify worker threads (0 = auto).
    #[config(
        env = "SUMERAGI_QC_VERIFY_WORKER_THREADS",
        default = "defaults::sumeragi::QC_VERIFY_WORKER_THREADS"
    )]
    pub qc_verify_worker_threads: usize,
    /// QC verify work queue capacity per worker (0 = auto).
    #[config(
        env = "SUMERAGI_QC_VERIFY_WORK_QUEUE_CAP",
        default = "defaults::sumeragi::QC_VERIFY_WORK_QUEUE_CAP"
    )]
    pub qc_verify_work_queue_cap: usize,
    /// QC verify result queue capacity (shared, 0 = auto).
    #[config(
        env = "SUMERAGI_QC_VERIFY_RESULT_QUEUE_CAP",
        default = "defaults::sumeragi::QC_VERIFY_RESULT_QUEUE_CAP"
    )]
    pub qc_verify_result_queue_cap: usize,
    /// Cap on deferred vote-validation backlog before dropping inbound votes.
    #[config(
        env = "SUMERAGI_VALIDATION_PENDING_CAP",
        default = "defaults::sumeragi::VALIDATION_PENDING_CAP"
    )]
    pub validation_pending_cap: usize,
    /// Vote burst cap when block payload backlog is pending.
    #[config(
        env = "SUMERAGI_WORKER_VOTE_BURST_CAP_WITH_PAYLOAD_BACKLOG",
        default = "defaults::sumeragi::WORKER_VOTE_BURST_CAP_WITH_PAYLOAD_BACKLOG"
    )]
    pub vote_burst_cap_with_payload_backlog: usize,
    /// Maximum urgent actor-gate streak before yielding to DA-critical work.
    #[config(
        env = "SUMERAGI_WORKER_MAX_URGENT_BEFORE_DA_CRITICAL",
        default = "defaults::sumeragi::WORKER_MAX_URGENT_BEFORE_DA_CRITICAL"
    )]
    pub max_urgent_before_da_critical: u32,
}

/// User-level configuration container for `SumeragiPacemaker`.
#[derive(Debug, Clone, Copy, ReadConfig)]
pub struct SumeragiPacemaker {
    /// Pacemaker backoff multiplier for view-change increments (>=1).
    #[config(
        env = "SUMERAGI_PACEMAKER_BACKOFF_MULTIPLIER",
        default = "defaults::sumeragi::PACEMAKER_BACKOFF_MULTIPLIER"
    )]
    pub backoff_multiplier: u32,
    /// Pacemaker RTT floor multiplier applied to avg RTT (>=1).
    #[config(
        env = "SUMERAGI_PACEMAKER_RTT_FLOOR_MULTIPLIER",
        default = "defaults::sumeragi::PACEMAKER_RTT_FLOOR_MULTIPLIER"
    )]
    pub rtt_floor_multiplier: u32,
    /// Pacemaker maximum backoff cap in milliseconds.
    #[config(
        env = "SUMERAGI_PACEMAKER_MAX_BACKOFF_MS",
        default = "defaults::sumeragi::PACEMAKER_MAX_BACKOFF_MS"
    )]
    pub max_backoff_ms: u64,
    /// Pacemaker jitter band size as permille of the backoff window (0..=1000). 0 disables jitter.
    #[config(
        env = "SUMERAGI_PACEMAKER_JITTER_FRAC_PERMILLE",
        default = "defaults::sumeragi::PACEMAKER_JITTER_FRAC_PERMILLE"
    )]
    pub jitter_frac_permille: u32,
    /// Grace period (ms) before a pending block counts as stalled for pacemaker backpressure.
    #[config(
        env = "SUMERAGI_PACEMAKER_PENDING_STALL_GRACE_MS",
        default = "defaults::sumeragi::PACEMAKER_PENDING_STALL_GRACE_MS"
    )]
    pub pending_stall_grace_ms: u64,
    /// Allow fast quorum reschedules in DA mode when payloads are locally available.
    #[config(
        env = "SUMERAGI_PACEMAKER_DA_FAST_RESCHEDULE",
        default = "defaults::sumeragi::PACEMAKER_DA_FAST_RESCHEDULE"
    )]
    pub da_fast_reschedule: bool,
    /// Soft limit for blocking pending blocks before pacemaker backpressure defers proposals.
    /// 0 keeps strict gating (any pending block defers).
    #[config(
        env = "SUMERAGI_PACEMAKER_ACTIVE_PENDING_SOFT_LIMIT",
        default = "defaults::sumeragi::PACEMAKER_ACTIVE_PENDING_SOFT_LIMIT"
    )]
    pub active_pending_soft_limit: usize,
    /// Soft limit for unresolved RBC backlog sessions before pacemaker backpressure defers proposals.
    /// 0 keeps strict gating (any backlog session defers).
    #[config(
        env = "SUMERAGI_PACEMAKER_RBC_BACKLOG_SESSION_SOFT_LIMIT",
        default = "defaults::sumeragi::PACEMAKER_RBC_BACKLOG_SESSION_SOFT_LIMIT"
    )]
    pub rbc_backlog_session_soft_limit: usize,
    /// Soft limit for missing RBC chunks before pacemaker backpressure defers proposals.
    /// 0 keeps strict gating (any missing chunks defers).
    #[config(
        env = "SUMERAGI_PACEMAKER_RBC_BACKLOG_CHUNK_SOFT_LIMIT",
        default = "defaults::sumeragi::PACEMAKER_RBC_BACKLOG_CHUNK_SOFT_LIMIT"
    )]
    pub rbc_backlog_chunk_soft_limit: usize,
}

/// User-level configuration container for DA timeout overrides.
#[derive(Debug, Clone, Copy, ReadConfig)]
pub struct SumeragiDaAdvanced {
    /// Multiplier for DA commit-quorum timeouts.
    #[config(
        env = "SUMERAGI_DA_QUORUM_TIMEOUT_MULTIPLIER",
        default = "defaults::sumeragi::DA_QUORUM_TIMEOUT_MULTIPLIER"
    )]
    pub quorum_timeout_multiplier: u32,
    /// Multiplier for availability timeouts in DA mode.
    #[config(
        env = "SUMERAGI_DA_AVAILABILITY_TIMEOUT_MULTIPLIER",
        default = "defaults::sumeragi::DA_AVAILABILITY_TIMEOUT_MULTIPLIER"
    )]
    pub availability_timeout_multiplier: u32,
    /// Floor (ms) for availability timeouts in DA mode.
    #[config(
        env = "SUMERAGI_DA_AVAILABILITY_TIMEOUT_FLOOR_MS",
        default = "defaults::sumeragi::DA_AVAILABILITY_TIMEOUT_FLOOR_MS"
    )]
    pub availability_timeout_floor_ms: u64,
}

/// User-level configuration container for `SumeragiDa`.
#[derive(Debug, Clone, Copy, ReadConfig)]
pub struct SumeragiDa {
    /// Enable data availability for consensus (RBC + availability QC gating).
    #[config(
        env = "SUMERAGI_DA_ENABLED",
        default = "defaults::sumeragi::DA_ENABLED"
    )]
    pub enabled: bool,
    /// Maximum DA commitments (blobs) permitted in a single block.
    #[config(
        env = "SUMERAGI_DA_MAX_COMMITMENTS_PER_BLOCK",
        default = "defaults::sumeragi::DA_MAX_COMMITMENTS_PER_BLOCK"
    )]
    pub max_commitments_per_block: usize,
    /// Maximum DA proof openings permitted in a single block (aggregate cap).
    #[config(
        env = "SUMERAGI_DA_MAX_PROOF_OPENINGS_PER_BLOCK",
        default = "defaults::sumeragi::DA_MAX_PROOF_OPENINGS_PER_BLOCK"
    )]
    pub max_proof_openings_per_block: usize,
}

/// User-level configuration container for `SumeragiPersistence`.
#[derive(Debug, Clone, Copy, ReadConfig)]
pub struct SumeragiPersistence {
    /// Interval between kura persistence retry attempts in milliseconds.
    #[config(
        env = "SUMERAGI_KURA_STORE_RETRY_INTERVAL_MS",
        default = "defaults::sumeragi::KURA_STORE_RETRY_INTERVAL_MS"
    )]
    pub kura_retry_interval_ms: u64,
    /// Maximum number of kura persistence retry attempts before aborting the block.
    #[config(
        env = "SUMERAGI_KURA_STORE_RETRY_MAX_ATTEMPTS",
        default = "defaults::sumeragi::KURA_STORE_RETRY_MAX_ATTEMPTS"
    )]
    pub kura_retry_max_attempts: u32,
    /// Timeout (ms) for inflight commit jobs before aborting.
    #[config(
        env = "SUMERAGI_COMMIT_INFLIGHT_TIMEOUT_MS",
        default = "defaults::sumeragi::COMMIT_INFLIGHT_TIMEOUT_MS"
    )]
    pub commit_inflight_timeout_ms: u64,
    /// Commit worker work-queue capacity.
    #[config(
        env = "SUMERAGI_COMMIT_WORK_QUEUE_CAP",
        default = "defaults::sumeragi::COMMIT_WORK_QUEUE_CAP"
    )]
    pub commit_work_queue_cap: usize,
    /// Commit worker result-queue capacity.
    #[config(
        env = "SUMERAGI_COMMIT_RESULT_QUEUE_CAP",
        default = "defaults::sumeragi::COMMIT_RESULT_QUEUE_CAP"
    )]
    pub commit_result_queue_cap: usize,
}

/// User-level configuration container for `SumeragiRecovery`.
#[derive(Debug, Clone, Copy, ReadConfig)]
pub struct SumeragiRecovery {
    /// Deterministic per-height recovery attempt cap before hard escalation.
    #[config(
        env = "SUMERAGI_RECOVERY_HEIGHT_ATTEMPT_CAP",
        default = "defaults::sumeragi::RECOVERY_HEIGHT_ATTEMPT_CAP"
    )]
    pub height_attempt_cap: u32,
    /// Deterministic per-height recovery dwell window before hard escalation (milliseconds).
    #[config(
        env = "SUMERAGI_RECOVERY_HEIGHT_WINDOW_MS",
        default = "defaults::sumeragi::RECOVERY_HEIGHT_WINDOW_MS"
    )]
    pub height_window_ms: u64,
    /// Hash-miss threshold before escalating dependency recovery to range pull.
    #[config(
        env = "SUMERAGI_RECOVERY_HASH_MISS_CAP_BEFORE_RANGE_PULL",
        default = "defaults::sumeragi::RECOVERY_HASH_MISS_CAP_BEFORE_RANGE_PULL"
    )]
    pub hash_miss_cap_before_range_pull: u32,
    /// Deterministic wait window before rotating after a missing-QC recovery attempt (milliseconds).
    #[config(
        env = "SUMERAGI_RECOVERY_MISSING_QC_REACQUIRE_WINDOW_MS",
        default = "defaults::sumeragi::RECOVERY_MISSING_QC_REACQUIRE_WINDOW_MS"
    )]
    pub missing_qc_reacquire_window_ms: u64,
    /// Maximum forced self-proposal attempts allowed for a single (height, view).
    #[config(
        env = "SUMERAGI_RECOVERY_MAX_FORCED_PROPOSAL_ATTEMPTS_PER_VIEW",
        default = "defaults::sumeragi::RECOVERY_MAX_FORCED_PROPOSAL_ATTEMPTS_PER_VIEW"
    )]
    pub max_forced_proposal_attempts_per_view: u32,
    /// Rotate immediately when the missing-QC reacquire window is exhausted.
    #[config(
        env = "SUMERAGI_RECOVERY_ROTATE_AFTER_REACQUIRE_EXHAUSTED",
        default = "defaults::sumeragi::RECOVERY_ROTATE_AFTER_REACQUIRE_EXHAUSTED"
    )]
    pub rotate_after_reacquire_exhausted: bool,
    /// Missing-block fetch attempts before falling back to the full commit topology.
    /// A value of 0 disables signer preference.
    #[config(
        env = "SUMERAGI_MISSING_BLOCK_SIGNER_FALLBACK_ATTEMPTS",
        default = "defaults::sumeragi::MISSING_BLOCK_SIGNER_FALLBACK_ATTEMPTS"
    )]
    pub missing_block_signer_fallback_attempts: u32,
    /// Per-attempt multiplier applied to missing-block retry windows (>=1).
    #[config(
        env = "SUMERAGI_RECOVERY_MISSING_BLOCK_RETRY_BACKOFF_MULTIPLIER",
        default = "defaults::sumeragi::RECOVERY_MISSING_BLOCK_RETRY_BACKOFF_MULTIPLIER"
    )]
    pub missing_block_retry_backoff_multiplier: u32,
    /// Ceiling applied to missing-block retry windows after backoff (milliseconds).
    #[config(
        env = "SUMERAGI_RECOVERY_MISSING_BLOCK_RETRY_BACKOFF_CAP_MS",
        default = "defaults::sumeragi::RECOVERY_MISSING_BLOCK_RETRY_BACKOFF_CAP_MS"
    )]
    pub missing_block_retry_backoff_cap_ms: u64,
    /// Backlog-aware multiplier applied to quorum-reschedule grace windows.
    #[config(
        env = "SUMERAGI_VIEW_CHANGE_BACKLOG_EXTENSION_FACTOR",
        default = "defaults::sumeragi::VIEW_CHANGE_BACKLOG_EXTENSION_FACTOR"
    )]
    pub view_change_backlog_extension_factor: f64,
    /// Maximum additional quorum-reschedule grace window under backlog (milliseconds).
    #[config(
        env = "SUMERAGI_VIEW_CHANGE_BACKLOG_EXTENSION_CAP_MS",
        default = "defaults::sumeragi::VIEW_CHANGE_BACKLOG_EXTENSION_CAP_MS"
    )]
    pub view_change_backlog_extension_cap_ms: u64,
    /// TTL for deferred QC missing-payload recovery before escalation (milliseconds).
    #[config(
        env = "SUMERAGI_DEFERRED_QC_TTL_MS",
        default = "defaults::sumeragi::DEFERRED_QC_TTL_MS"
    )]
    pub deferred_qc_ttl_ms: u64,
    /// Deterministic per-height missing-block retry cap before hard escalation.
    #[config(
        env = "SUMERAGI_MISSING_BLOCK_HEIGHT_ATTEMPT_CAP",
        default = "defaults::sumeragi::MISSING_BLOCK_HEIGHT_ATTEMPT_CAP"
    )]
    pub missing_block_height_attempt_cap: u32,
    /// Deterministic per-height missing-block dwell cap before hard escalation (milliseconds).
    #[config(
        env = "SUMERAGI_MISSING_BLOCK_HEIGHT_TTL_MS",
        default = "defaults::sumeragi::MISSING_BLOCK_HEIGHT_TTL_MS"
    )]
    pub missing_block_height_ttl_ms: u64,
    /// Sidecar mismatch retries before final-drop and canonical-only rebuild.
    #[config(
        env = "SUMERAGI_SIDECAR_MISMATCH_RETRY_CAP",
        default = "defaults::sumeragi::SIDECAR_MISMATCH_RETRY_CAP"
    )]
    pub sidecar_mismatch_retry_cap: u32,
    /// Sidecar mismatch TTL before final-drop (milliseconds).
    #[config(
        env = "SUMERAGI_SIDECAR_MISMATCH_TTL_MS",
        default = "defaults::sumeragi::SIDECAR_MISMATCH_TTL_MS"
    )]
    pub sidecar_mismatch_ttl_ms: u64,
    /// Hash-miss threshold before escalating dependency recovery to range pull.
    #[config(
        env = "SUMERAGI_RANGE_PULL_ESCALATION_AFTER_HASH_MISSES",
        default = "defaults::sumeragi::RANGE_PULL_ESCALATION_AFTER_HASH_MISSES"
    )]
    pub range_pull_escalation_after_hash_misses: u32,
    /// Height margin used to prune stale missing-block requests after head advances.
    #[config(
        env = "SUMERAGI_RECOVERY_MISSING_REQUEST_STALE_HEIGHT_MARGIN",
        default = "defaults::sumeragi::RECOVERY_MISSING_REQUEST_STALE_HEIGHT_MARGIN"
    )]
    pub missing_request_stale_height_margin: u64,
    /// Maximum deferred block-sync updates retained in memory.
    #[config(
        env = "SUMERAGI_RECOVERY_PENDING_BLOCK_SYNC_CAP",
        default = "defaults::sumeragi::RECOVERY_PENDING_BLOCK_SYNC_CAP"
    )]
    pub pending_block_sync_cap: usize,
    /// Maximum cached proposal entries retained in memory.
    #[config(
        env = "SUMERAGI_RECOVERY_PENDING_PROPOSAL_CAP",
        default = "defaults::sumeragi::RECOVERY_PENDING_PROPOSAL_CAP"
    )]
    pub pending_proposal_cap: usize,
    /// Missing-block fetch attempts before switching to aggressive topology fanout.
    #[config(
        env = "SUMERAGI_RECOVERY_MISSING_FETCH_AGGRESSIVE_AFTER_ATTEMPTS",
        default = "defaults::sumeragi::RECOVERY_MISSING_FETCH_AGGRESSIVE_AFTER_ATTEMPTS"
    )]
    pub missing_fetch_aggressive_after_attempts: u32,
}

/// User-level configuration container for deterministic transport fanout.
#[derive(Debug, Clone, Copy, ReadConfig)]
pub struct SumeragiFanout {
    /// Validator-set size threshold where deterministic active-subset fanout engages.
    #[config(
        env = "SUMERAGI_FANOUT_LARGE_SET_THRESHOLD",
        default = "defaults::sumeragi::FANOUT_LARGE_SET_THRESHOLD"
    )]
    pub large_set_threshold: u32,
    /// Number of finalized blocks to inspect when scoring validator activity.
    #[config(
        env = "SUMERAGI_FANOUT_ACTIVITY_LOOKBACK_BLOCKS",
        default = "defaults::sumeragi::FANOUT_ACTIVITY_LOOKBACK_BLOCKS"
    )]
    pub activity_lookback_blocks: u32,
}

/// User-level configuration container for `SumeragiGating`.
#[derive(Debug, Clone, Copy, ReadConfig)]
pub struct SumeragiGating {
    /// Maximum height delta accepted for inbound consensus messages (0 disables future gating).
    #[config(
        env = "SUMERAGI_CONSENSUS_FUTURE_HEIGHT_WINDOW",
        default = "defaults::sumeragi::CONSENSUS_FUTURE_HEIGHT_WINDOW"
    )]
    pub future_height_window: u64,
    /// Maximum view delta accepted for inbound consensus messages (0 disables future gating).
    #[config(
        env = "SUMERAGI_CONSENSUS_FUTURE_VIEW_WINDOW",
        default = "defaults::sumeragi::CONSENSUS_FUTURE_VIEW_WINDOW"
    )]
    pub future_view_window: u64,
    /// Invalid signature count before temporarily suppressing a signer (0 disables).
    #[config(
        env = "SUMERAGI_INVALID_SIG_PENALTY_THRESHOLD",
        default = "defaults::sumeragi::INVALID_SIG_PENALTY_THRESHOLD"
    )]
    pub invalid_sig_penalty_threshold: u32,
    /// Window (ms) for invalid signature penalty counting.
    #[config(
        env = "SUMERAGI_INVALID_SIG_PENALTY_WINDOW_MS",
        default = "defaults::sumeragi::INVALID_SIG_PENALTY_WINDOW_MS"
    )]
    pub invalid_sig_penalty_window_ms: u64,
    /// Cooldown (ms) applied after invalid signature penalties trigger.
    #[config(
        env = "SUMERAGI_INVALID_SIG_PENALTY_COOLDOWN_MS",
        default = "defaults::sumeragi::INVALID_SIG_PENALTY_COOLDOWN_MS"
    )]
    pub invalid_sig_penalty_cooldown_ms: u64,
    /// Consecutive membership mismatches required before alerting.
    #[config(default = "defaults::sumeragi::MEMBERSHIP_MISMATCH_ALERT_THRESHOLD")]
    pub membership_mismatch_alert_threshold: u32,
    /// Whether to drop consensus messages from peers with repeated membership mismatches.
    #[config(default = "defaults::sumeragi::MEMBERSHIP_MISMATCH_FAIL_CLOSED")]
    pub membership_mismatch_fail_closed: bool,
}

/// User-level configuration container for `SumeragiRbc`.
#[derive(Debug, Clone, Copy, ReadConfig)]
pub struct SumeragiRbc {
    /// RBC per-chunk maximum bytes.
    #[config(
        env = "SUMERAGI_RBC_CHUNK_MAX_BYTES",
        default = "defaults::sumeragi::RBC_CHUNK_MAX_BYTES"
    )]
    pub chunk_max_bytes: usize,
    /// Optional fanout cap for RBC chunk broadcasts (null = auto).
    #[config(env = "SUMERAGI_RBC_CHUNK_FANOUT")]
    pub chunk_fanout: Option<NonZeroUsize>,
    /// Maximum pending RBC chunks stashed before INIT is observed.
    #[config(
        env = "SUMERAGI_RBC_PENDING_MAX_CHUNKS",
        default = "defaults::sumeragi::RBC_PENDING_MAX_CHUNKS"
    )]
    pub pending_max_chunks: usize,
    /// Maximum pending RBC bytes per session before INIT is observed.
    #[config(
        env = "SUMERAGI_RBC_PENDING_MAX_BYTES",
        default = "defaults::sumeragi::RBC_PENDING_MAX_BYTES"
    )]
    pub pending_max_bytes: usize,
    /// Maximum pending RBC sessions stashed before INIT is observed.
    #[config(
        env = "SUMERAGI_RBC_PENDING_SESSION_LIMIT",
        default = "defaults::sumeragi::RBC_PENDING_SESSION_LIMIT"
    )]
    pub pending_session_limit: usize,
    /// TTL (milliseconds) for pending RBC messages awaiting INIT.
    #[config(
        env = "SUMERAGI_RBC_PENDING_TTL_MS",
        default = "defaults::sumeragi::RBC_PENDING_TTL_MS"
    )]
    pub pending_ttl_ms: u64,
    /// RBC session TTL in milliseconds (inactive sessions pruned after this).
    #[config(
        env = "SUMERAGI_RBC_SESSION_TTL_MS",
        default = "defaults::sumeragi::RBC_SESSION_TTL_MS"
    )]
    pub session_ttl_ms: u64,
    /// Maximum RBC sessions rebroadcast per tick (prevents message storms).
    #[config(
        env = "SUMERAGI_RBC_REBROADCAST_SESSIONS_PER_TICK",
        default = "defaults::sumeragi::RBC_REBROADCAST_SESSIONS_PER_TICK"
    )]
    pub rebroadcast_sessions_per_tick: usize,
    /// Maximum RBC payload chunks broadcast per tick (limits burst fanout).
    #[config(
        env = "SUMERAGI_RBC_PAYLOAD_CHUNKS_PER_TICK",
        default = "defaults::sumeragi::RBC_PAYLOAD_CHUNKS_PER_TICK"
    )]
    pub payload_chunks_per_tick: usize,
    /// Maximum number of RBC session summaries persisted to disk.
    #[config(
        env = "SUMERAGI_RBC_STORE_MAX_SESSIONS",
        default = "defaults::sumeragi::RBC_STORE_MAX_SESSIONS"
    )]
    pub store_max_sessions: usize,
    /// Soft quota for persisted RBC sessions; beyond this compaction/back-pressure engages.
    #[config(
        env = "SUMERAGI_RBC_STORE_SOFT_SESSIONS",
        default = "defaults::sumeragi::RBC_STORE_SOFT_SESSIONS"
    )]
    pub store_soft_sessions: usize,
    /// Maximum total disk bytes allocated for persisted RBC session payloads.
    #[config(
        env = "SUMERAGI_RBC_STORE_MAX_BYTES",
        default = "defaults::sumeragi::RBC_STORE_MAX_BYTES"
    )]
    pub store_max_bytes: usize,
    /// Soft quota for persisted RBC payload bytes; beyond this compaction/back-pressure engages.
    #[config(
        env = "SUMERAGI_RBC_STORE_SOFT_BYTES",
        default = "defaults::sumeragi::RBC_STORE_SOFT_BYTES"
    )]
    pub store_soft_bytes: usize,
    /// Disk-backed RBC chunk retention TTL (milliseconds).
    #[config(
        env = "SUMERAGI_RBC_DISK_STORE_TTL_MS",
        default = "defaults::sumeragi::RBC_DISK_STORE_TTL_MS"
    )]
    pub disk_store_ttl_ms: u64,
    /// Maximum bytes allocated for disk-backed RBC chunk persistence.
    #[config(
        env = "SUMERAGI_RBC_DISK_STORE_MAX_BYTES",
        default = "defaults::sumeragi::RBC_DISK_STORE_MAX_BYTES"
    )]
    pub disk_store_max_bytes: u64,
}

/// User-level configuration container for `SumeragiFinality`.
#[derive(Debug, Clone, Copy, ReadConfig)]
pub struct SumeragiFinality {
    /// Proof policy (off/zk_parent).
    #[config(env = "SUMERAGI_PROOF_POLICY", default = "ProofPolicy::Off")]
    pub proof_policy: ProofPolicy,
    /// Cap for in-memory commit certificate history (used for status/finality proofs).
    #[config(
        env = "SUMERAGI_COMMIT_CERT_HISTORY_CAP",
        default = "defaults::sumeragi::COMMIT_CERT_HISTORY_CAP"
    )]
    pub commit_cert_history_cap: usize,
    /// Zk parent-proving depth (0 disables zk finality).
    #[config(
        env = "SUMERAGI_ZK_FINALITY_K",
        default = "defaults::sumeragi::ZK_FINALITY_K"
    )]
    pub zk_finality_k: u8,
    /// Require `PrecommitQC` presence before committing a block (consensus path).
    #[config(
        env = "SUMERAGI_REQUIRE_PRECOMMIT_QC",
        default = "defaults::sumeragi::REQUIRE_PRECOMMIT_QC"
    )]
    pub require_precommit_qc: bool,
}

/// User-level configuration container for `SumeragiKeys`.
#[derive(Debug, Clone, ReadConfig)]
pub struct SumeragiKeys {
    /// Minimum lead time (blocks) between publishing a new consensus key and activation.
    #[config(
        env = "SUMERAGI_KEY_ACTIVATION_LEAD_BLOCKS",
        default = "defaults::sumeragi::KEY_ACTIVATION_LEAD_BLOCKS"
    )]
    pub activation_lead_blocks: u64,
    /// Overlap/grace window (blocks) permitting dual-signing during rotation.
    #[config(
        env = "SUMERAGI_KEY_OVERLAP_GRACE_BLOCKS",
        default = "defaults::sumeragi::KEY_OVERLAP_GRACE_BLOCKS"
    )]
    pub overlap_grace_blocks: u64,
    /// Expiry grace window (blocks) after declared expiry.
    #[config(
        env = "SUMERAGI_KEY_EXPIRY_GRACE_BLOCKS",
        default = "defaults::sumeragi::KEY_EXPIRY_GRACE_BLOCKS"
    )]
    pub expiry_grace_blocks: u64,
    /// Require HSM binding for consensus/committee keys.
    #[config(
        env = "SUMERAGI_KEY_REQUIRE_HSM",
        default = "defaults::sumeragi::KEY_REQUIRE_HSM"
    )]
    pub require_hsm: bool,
    /// Allowed algorithms for consensus/committee keys.
    #[config(default = "defaults::sumeragi::key_allowed_algorithms()")]
    pub allowed_algorithms: Vec<Algorithm>,
    /// Allowed HSM providers for consensus/committee keys.
    #[config(default = "defaults::sumeragi::key_allowed_hsm_providers()")]
    pub allowed_hsm_providers: Vec<String>,
}

/// User-level configuration container for `Sumeragi`.
#[derive(Debug, Clone, ReadConfig)]
pub struct Sumeragi {
    /// Node role: `validator` (default) or `observer`.
    #[config(env = "NODE_ROLE", default = "NodeRole::Validator")]
    pub role: NodeRole,
    /// Runtime consensus mode: `permissioned` (default) or `npos`.
    #[config(
        env = "SUMERAGI_CONSENSUS_MODE",
        default = "ConsensusMode::Permissioned"
    )]
    pub consensus_mode: ConsensusMode,
    /// Mode-flip guardrails.
    #[config(nested)]
    pub mode_flip: SumeragiModeFlip,
    /// Collector selection knobs.
    #[config(nested)]
    pub collectors: SumeragiCollectors,
    /// Block assembly limits.
    #[config(nested)]
    pub block: SumeragiBlock,
    /// Advanced consensus overrides.
    #[config(nested)]
    pub advanced: SumeragiAdvanced,
    /// Data-availability configuration.
    #[config(nested)]
    pub da: SumeragiDa,
    /// Persistence/retry configuration.
    #[config(nested)]
    pub persistence: SumeragiPersistence,
    /// Recovery behavior.
    #[config(nested)]
    pub recovery: SumeragiRecovery,
    /// Deterministic transport fanout behavior.
    #[config(nested)]
    pub fanout: SumeragiFanout,
    /// Ingress gating and penalties.
    #[config(nested)]
    pub gating: SumeragiGating,
    /// Finality/proof configuration.
    #[config(nested)]
    pub finality: SumeragiFinality,
    /// Consensus key rotation and HSM policy.
    #[config(nested)]
    pub keys: SumeragiKeys,
    /// Adaptive observability/auto-mitigation knobs.
    #[config(nested)]
    pub adaptive_observability: AdaptiveObservability,
    /// NPoS-specific tuning knobs (only effective in `npos` mode).
    #[config(nested)]
    pub npos: SumeragiNpos,
    /// Debug-only overrides for injecting faults or altering behaviour.
    #[config(nested)]
    pub debug: SumeragiDebug,
}

/// NPoS consensus tunables exposed via configuration.
/// User-level configuration container for `SumeragiNpos`.
#[derive(Debug, Clone, Copy, ReadConfig, norito::JsonDeserialize)]
pub struct SumeragiNpos {
    /// Epoch length in blocks.
    #[config(default = "defaults::sumeragi::EPOCH_LENGTH_BLOCKS")]
    pub epoch_length_blocks: u64,
    /// Use stake snapshot provider for epoch validator roster.
    #[config(
        env = "SUMERAGI_USE_STAKE_SNAPSHOT_ROSTER",
        default = "defaults::sumeragi::USE_STAKE_SNAPSHOT_ROSTER"
    )]
    pub use_stake_snapshot_roster: bool,
    /// VRF commit/reveal window configuration.
    /// Unset values are derived from `epoch_length_blocks`.
    #[config(nested)]
    pub vrf: SumeragiNposVrf,
    /// Election policy knobs.
    #[config(nested)]
    pub election: SumeragiNposElection,
    /// Reconfiguration governance knobs.
    #[config(nested)]
    pub reconfig: SumeragiNposReconfig,
}

/// User-level configuration container for advanced NPoS overrides.
#[derive(Debug, Clone, Copy, ReadConfig, norito::JsonDeserialize)]
pub struct SumeragiAdvancedNpos {
    /// Per-phase pacemaker timeouts.
    /// Unset values are derived from on-chain `SumeragiParameters.block_time_ms`.
    #[config(nested)]
    pub timeouts: SumeragiNposTimeouts,
}

/// NPoS pacemaker timeout configuration (milliseconds).
/// Unset or zero values are derived from on-chain `SumeragiParameters.block_time_ms`.
/// User-level configuration container for `SumeragiNposTimeouts`.
#[allow(clippy::struct_field_names)]
#[derive(Debug, Clone, Copy, ReadConfig, norito::JsonDeserialize)]
pub struct SumeragiNposTimeouts {
    /// Timeout for the proposal broadcast phase (ms).
    pub propose_ms: Option<u64>,
    /// Timeout for collecting prevotes (ms).
    pub prevote_ms: Option<u64>,
    /// Timeout for collecting precommits (ms).
    pub precommit_ms: Option<u64>,
    /// Timeout for execution acknowledgement aggregation (ms).
    pub exec_ms: Option<u64>,
    /// Timeout for witness availability enforcement (ms).
    pub witness_ms: Option<u64>,
    /// Timeout for final commit broadcasts (ms).
    pub commit_ms: Option<u64>,
    /// Timeout allocated to data-availability recovery (ms).
    pub da_ms: Option<u64>,
    /// Timeout for BLS aggregation and dissemination (ms).
    pub aggregator_ms: Option<u64>,
}

/// VRF commit/reveal windows for epoch randomness.
/// Unset values are derived from `epoch_length_blocks`.
/// User-level configuration container for `SumeragiNposVrf`.
#[allow(clippy::struct_field_names)]
#[derive(Debug, Clone, Copy, ReadConfig, norito::JsonDeserialize)]
pub struct SumeragiNposVrf {
    /// Number of blocks allotted for VRF commit submissions.
    pub commit_window_blocks: Option<u64>,
    /// Number of blocks allotted for VRF reveal submissions.
    pub reveal_window_blocks: Option<u64>,
    /// Commit deadline offset from epoch start (blocks).
    pub commit_deadline_offset_blocks: Option<u64>,
    /// Reveal deadline offset from epoch start (blocks).
    pub reveal_deadline_offset_blocks: Option<u64>,
}

/// Election policy defaults for validator selection.
/// User-level configuration container for `SumeragiNposElection`.
#[derive(Debug, Clone, Copy, ReadConfig, norito::JsonDeserialize)]
pub struct SumeragiNposElection {
    /// Maximum number of validators to elect (0 = unlimited).
    #[config(default = "defaults::sumeragi::npos::MAX_VALIDATORS")]
    pub max_validators: u32,
    /// Minimum self-bond (in asset units) required for a validator candidate.
    #[config(default = "defaults::sumeragi::npos::MIN_SELF_BOND")]
    pub min_self_bond: u64,
    /// Minimum nomination bond required for delegators.
    #[config(default = "defaults::sumeragi::npos::MIN_NOMINATION_BOND")]
    pub min_nomination_bond: u64,
    /// Maximum share of nominations a single validator may accumulate (percentage).
    #[config(default = "defaults::sumeragi::npos::MAX_NOMINATOR_CONCENTRATION_PCT")]
    pub max_nominator_concentration_pct: u8,
    /// Seat padding band (percentage) for stake-based selection fairness.
    #[config(default = "defaults::sumeragi::npos::SEAT_BAND_PCT")]
    pub seat_band_pct: u8,
    /// Maximum allowable stake correlation for entities (percentage).
    #[config(default = "defaults::sumeragi::npos::MAX_ENTITY_CORRELATION_PCT")]
    pub max_entity_correlation_pct: u8,
    /// Finality margin (blocks) required before activating a newly elected set.
    #[config(default = "defaults::sumeragi::npos::FINALITY_MARGIN_BLOCKS")]
    pub finality_margin_blocks: u64,
}

/// Reconfiguration pipeline defaults.
/// User-level configuration container for `SumeragiNposReconfig`.
#[derive(Debug, Clone, Copy, ReadConfig, norito::JsonDeserialize)]
#[allow(clippy::struct_field_names)]
pub struct SumeragiNposReconfig {
    /// Number of blocks to retain for evidence horizon when processing reconfigurations.
    #[config(default = "defaults::sumeragi::npos::RECONFIG_EVIDENCE_HORIZON_BLOCKS")]
    pub evidence_horizon_blocks: u64,
    /// Blocks between governance approval and activation of a new validator set.
    #[config(default = "defaults::sumeragi::npos::RECONFIG_ACTIVATION_LAG_BLOCKS")]
    pub activation_lag_blocks: u64,
    /// Slashing delay in blocks before evidence penalties apply.
    #[config(default = "defaults::sumeragi::npos::SLASHING_DELAY_BLOCKS")]
    pub slashing_delay_blocks: u64,
}

/// Node role in consensus participation (user view).
/// User-level enumeration translating `NodeRole` settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, strum::EnumString, strum::Display)]
#[strum(serialize_all = "snake_case")]
pub enum NodeRole {
    /// Full validator participating in block production and voting.
    Validator,
    /// Observer node receiving blocks without voting rights.
    Observer,
}

impl json::JsonSerialize for NodeRole {
    fn json_serialize(&self, out: &mut String) {
        json::write_json_string(&self.to_string(), out);
    }
}

impl json::JsonDeserialize for NodeRole {
    fn json_deserialize(
        parser: &mut json::Parser<'_>,
    ) -> ::core::result::Result<Self, json::Error> {
        let text = parser.parse_string()?;
        Self::from_str(&text).map_err(|err| json::Error::InvalidField {
            field: "node_role".into(),
            message: err.to_string(),
        })
    }
}

/// Consensus mode (user view).
/// User-level enumeration translating `ConsensusMode` settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, strum::EnumString, strum::Display)]
#[strum(serialize_all = "snake_case")]
pub enum ConsensusMode {
    /// Permissioned consensus with static validator set.
    Permissioned,
    /// Nominated proof-of-stake consensus with rotating validator set.
    Npos,
}

impl json::JsonSerialize for ConsensusMode {
    fn json_serialize(&self, out: &mut String) {
        json::write_json_string(&self.to_string(), out);
    }
}

impl json::JsonDeserialize for ConsensusMode {
    fn json_deserialize(
        parser: &mut json::Parser<'_>,
    ) -> ::core::result::Result<Self, json::Error> {
        let text = parser.parse_string()?;
        Self::from_str(&text).map_err(|err| json::Error::InvalidField {
            field: "consensus_mode".into(),
            message: err.to_string(),
        })
    }
}

/// Proof policy (user view)
/// User-level enumeration translating `ProofPolicy` settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, strum::EnumString, strum::Display)]
#[strum(serialize_all = "snake_case")]
pub enum ProofPolicy {
    /// Disable proof propagation.
    Off,
    /// Require parent zero-knowledge proof.
    ZkParent,
}

impl json::JsonSerialize for ProofPolicy {
    fn json_serialize(&self, out: &mut String) {
        json::write_json_string(&self.to_string(), out);
    }
}

impl json::JsonDeserialize for ProofPolicy {
    fn json_deserialize(
        parser: &mut json::Parser<'_>,
    ) -> ::core::result::Result<Self, json::Error> {
        let text = parser.parse_string()?;
        Self::from_str(&text).map_err(|err| json::Error::InvalidField {
            field: "proof_policy".into(),
            message: err.to_string(),
        })
    }
}

// Defined at the top of the file

impl FromEnvStr for TrustedPeers {
    type Error = json::Error;

    fn from_env_str(value: Cow<'_, str>) -> std::result::Result<Self, Self::Error>
    where
        Self: Sized,
    {
        norito::json::from_json(value.as_ref())
    }
}

#[derive(Debug, Clone, Default)]
struct TrustedPeerPops(Vec<TrustedPeerPop>);

impl json::JsonDeserialize for TrustedPeerPops {
    fn json_deserialize(
        parser: &mut json::Parser<'_>,
    ) -> ::core::result::Result<Self, json::Error> {
        let entries = Vec::<TrustedPeerPop>::json_deserialize(parser)?;
        Ok(Self(entries))
    }
}

impl FromEnvStr for TrustedPeerPops {
    type Error = json::Error;

    fn from_env_str(value: Cow<'_, str>) -> std::result::Result<Self, Self::Error>
    where
        Self: Sized,
    {
        norito::json::from_json(value.as_ref())
    }
}

impl Default for TrustedPeers {
    fn default() -> Self {
        Self(UniqueVec::new())
    }
}

/// Public-key + `PoP` pair (user view)
/// User-level configuration container for `TrustedPeerPop`.
#[derive(Debug, Clone, norito::JsonDeserialize)]
pub struct TrustedPeerPop {
    /// Public key of the peer
    pub public_key: iroha_crypto::PublicKey,
    /// `PoP` bytes as hex string (e.g., "0x...") or plain hex.
    pub pop_hex: String,
}

#[cfg(test)]
mod trusted_peers_pop_env_tests {
    use std::{borrow::Cow, str::FromStr};

    use super::*;

    const PUBLIC_KEY_HEX: &str = "ea01309060D021340617E9554CCBC2CF3CC3DB922A9BA323ABDF7C271FCC6EF69BE7A8DEBCA7D9E96C0F0089ABA22CDAADE4A2";

    #[test]
    fn trusted_peers_pop_from_env_parses_json() {
        let json = format!(r#"[{{"public_key":"{PUBLIC_KEY_HEX}","pop_hex":"deadbeef"}}]"#);
        let parsed =
            TrustedPeerPops::from_env_str(Cow::Owned(json)).expect("parse trusted_peers_pop env");
        assert_eq!(parsed.0.len(), 1);
        assert_eq!(
            parsed.0[0].public_key,
            iroha_crypto::PublicKey::from_str(PUBLIC_KEY_HEX).expect("public key")
        );
        assert_eq!(parsed.0[0].pop_hex, "deadbeef");
    }
}

impl AdaptiveObservability {
    fn parse(self, emitter: &mut Emitter<ParseError>) -> Option<actual::AdaptiveObservability> {
        let Self {
            enabled,
            qc_latency_alert_ms,
            da_reschedule_burst,
            pacemaker_extra_ms,
            collector_redundant_r,
            cooldown_ms,
        } = self;

        let mut ok = true;
        if qc_latency_alert_ms == 0 {
            emitter.emit(Report::new(ParseError::InvalidSumeragiConfig).attach(
                "sumeragi.adaptive_observability.qc_latency_alert_ms must be greater than zero",
            ));
            ok = false;
        }
        if da_reschedule_burst == 0 {
            emitter.emit(Report::new(ParseError::InvalidSumeragiConfig).attach(
                "sumeragi.adaptive_observability.da_reschedule_burst must be greater than zero",
            ));
            ok = false;
        }
        if collector_redundant_r == 0 {
            emitter.emit(Report::new(ParseError::InvalidSumeragiConfig).attach(
                "sumeragi.adaptive_observability.collector_redundant_r must be greater than zero",
            ));
            ok = false;
        }
        if cooldown_ms == 0 {
            emitter.emit(
                Report::new(ParseError::InvalidSumeragiConfig).attach(
                    "sumeragi.adaptive_observability.cooldown_ms must be greater than zero",
                ),
            );
            ok = false;
        }

        if !ok {
            return None;
        }

        Some(actual::AdaptiveObservability {
            enabled,
            qc_latency_alert_ms,
            da_reschedule_burst,
            pacemaker_extra_ms,
            collector_redundant_r,
            cooldown_ms,
        })
    }
}

impl From<actual::AdaptiveObservability> for AdaptiveObservability {
    fn from(value: actual::AdaptiveObservability) -> Self {
        Self {
            enabled: value.enabled,
            qc_latency_alert_ms: value.qc_latency_alert_ms,
            da_reschedule_burst: value.da_reschedule_burst,
            pacemaker_extra_ms: value.pacemaker_extra_ms,
            collector_redundant_r: value.collector_redundant_r,
            cooldown_ms: value.cooldown_ms,
        }
    }
}

impl SumeragiPacingGovernor {
    fn parse(self, emitter: &mut Emitter<ParseError>) -> Option<actual::SumeragiPacingGovernor> {
        let Self {
            window_blocks,
            view_change_pressure_permille,
            view_change_clear_permille,
            commit_spacing_pressure_permille,
            commit_spacing_clear_permille,
            step_up_bps,
            step_down_bps,
            min_factor_bps,
            max_factor_bps,
        } = self;

        let mut ok = true;
        if window_blocks < 2 {
            emitter.emit(
                Report::new(ParseError::InvalidSumeragiConfig)
                    .attach("sumeragi.advanced.pacing_governor.window_blocks must be at least 2"),
            );
            ok = false;
        }
        if step_up_bps == 0 {
            emitter.emit(
                Report::new(ParseError::InvalidSumeragiConfig).attach(
                    "sumeragi.advanced.pacing_governor.step_up_bps must be greater than zero",
                ),
            );
            ok = false;
        }
        if step_down_bps == 0 {
            emitter.emit(Report::new(ParseError::InvalidSumeragiConfig).attach(
                "sumeragi.advanced.pacing_governor.step_down_bps must be greater than zero",
            ));
            ok = false;
        }
        if min_factor_bps < 10_000 {
            emitter.emit(Report::new(ParseError::InvalidSumeragiConfig).attach(
                "sumeragi.advanced.pacing_governor.min_factor_bps must be at least 10_000",
            ));
            ok = false;
        }
        if max_factor_bps < min_factor_bps {
            emitter.emit(Report::new(ParseError::InvalidSumeragiConfig).attach(
                "sumeragi.advanced.pacing_governor.max_factor_bps must be >= min_factor_bps",
            ));
            ok = false;
        }
        if view_change_pressure_permille == 0 {
            emitter.emit(Report::new(ParseError::InvalidSumeragiConfig).attach(
                "sumeragi.advanced.pacing_governor.view_change_pressure_permille must be greater than zero",
            ));
            ok = false;
        }
        if view_change_clear_permille > view_change_pressure_permille {
            emitter.emit(Report::new(ParseError::InvalidSumeragiConfig).attach(
                "sumeragi.advanced.pacing_governor.view_change_clear_permille must be <= view_change_pressure_permille",
            ));
            ok = false;
        }
        if commit_spacing_pressure_permille == 0 {
            emitter.emit(Report::new(ParseError::InvalidSumeragiConfig).attach(
                "sumeragi.advanced.pacing_governor.commit_spacing_pressure_permille must be greater than zero",
            ));
            ok = false;
        }
        if commit_spacing_clear_permille > commit_spacing_pressure_permille {
            emitter.emit(Report::new(ParseError::InvalidSumeragiConfig).attach(
                "sumeragi.advanced.pacing_governor.commit_spacing_clear_permille must be <= commit_spacing_pressure_permille",
            ));
            ok = false;
        }

        if !ok {
            return None;
        }

        Some(actual::SumeragiPacingGovernor {
            window_blocks,
            view_change_pressure_permille,
            view_change_clear_permille,
            commit_spacing_pressure_permille,
            commit_spacing_clear_permille,
            step_up_bps,
            step_down_bps,
            min_factor_bps,
            max_factor_bps,
        })
    }
}

impl From<actual::SumeragiPacingGovernor> for SumeragiPacingGovernor {
    fn from(value: actual::SumeragiPacingGovernor) -> Self {
        Self {
            window_blocks: value.window_blocks,
            view_change_pressure_permille: value.view_change_pressure_permille,
            view_change_clear_permille: value.view_change_clear_permille,
            commit_spacing_pressure_permille: value.commit_spacing_pressure_permille,
            commit_spacing_clear_permille: value.commit_spacing_clear_permille,
            step_up_bps: value.step_up_bps,
            step_down_bps: value.step_down_bps,
            min_factor_bps: value.min_factor_bps,
            max_factor_bps: value.max_factor_bps,
        }
    }
}

impl Sumeragi {
    fn parse(self, emitter: &mut Emitter<ParseError>) -> Option<actual::Sumeragi> {
        let Self {
            role,
            consensus_mode,
            mode_flip,
            collectors,
            block,
            advanced,
            da,
            persistence,
            recovery,
            fanout,
            gating,
            finality,
            keys,
            adaptive_observability,
            npos,
            debug,
        } = self;
        let SumeragiAdvanced {
            queues,
            worker,
            pacemaker,
            pacing_governor,
            da: da_advanced,
            rbc,
            npos: npos_advanced,
        } = advanced;

        let collectors_ok = if collectors.k == 0 {
            emitter.emit(
                Report::new(ParseError::InvalidSumeragiConfig)
                    .attach("sumeragi.collectors.k must be greater than zero"),
            );
            false
        } else {
            true
        };
        let redundant_ok = if collectors.redundant_send_r == 0 {
            emitter.emit(
                Report::new(ParseError::InvalidSumeragiConfig)
                    .attach("sumeragi.collectors.redundant_send_r must be greater than zero"),
            );
            false
        } else {
            true
        };
        let queues_ok = if queues.votes == 0
            || queues.block_payload == 0
            || queues.rbc_chunks == 0
            || queues.blocks == 0
            || queues.control == 0
        {
            emitter.emit(
                Report::new(ParseError::InvalidSumeragiConfig)
                    .attach("sumeragi.advanced.queues.* must be greater than zero"),
            );
            false
        } else {
            true
        };
        let worker_budget_ok = if worker.iteration_budget_cap_ms == 0 {
            emitter.emit(Report::new(ParseError::InvalidSumeragiConfig).attach(
                "sumeragi.advanced.worker.iteration_budget_cap_ms must be greater than zero",
            ));
            false
        } else {
            true
        };
        let worker_drain_budget_ok = if worker.iteration_drain_budget_cap_ms == 0 {
            emitter.emit(Report::new(ParseError::InvalidSumeragiConfig).attach(
                "sumeragi.advanced.worker.iteration_drain_budget_cap_ms must be greater than zero",
            ));
            false
        } else {
            true
        };
        let validation_threads_ok = true;
        let validation_work_queue_ok = true;
        let validation_result_queue_ok = true;
        let validation_queue_full_inline_cutover_divisor_ok = if worker
            .validation_queue_full_inline_cutover_divisor
            == 0
        {
            emitter.emit(Report::new(ParseError::InvalidSumeragiConfig).attach(
                    "sumeragi.advanced.worker.validation_queue_full_inline_cutover_divisor must be greater than zero",
                ));
            false
        } else {
            true
        };
        let validation_pending_cap_ok = if worker.validation_pending_cap == 0 {
            emitter.emit(Report::new(ParseError::InvalidSumeragiConfig).attach(
                "sumeragi.advanced.worker.validation_pending_cap must be greater than zero",
            ));
            false
        } else {
            true
        };
        let vote_burst_cap_ok = if worker.vote_burst_cap_with_payload_backlog == 0 {
            emitter.emit(Report::new(ParseError::InvalidSumeragiConfig).attach(
                "sumeragi.advanced.worker.vote_burst_cap_with_payload_backlog must be greater than zero",
            ));
            false
        } else {
            true
        };
        let urgent_da_streak_ok = if worker.max_urgent_before_da_critical == 0 {
            emitter.emit(Report::new(ParseError::InvalidSumeragiConfig).attach(
                "sumeragi.advanced.worker.max_urgent_before_da_critical must be greater than zero",
            ));
            false
        } else {
            true
        };
        let pacemaker_backoff_ok = if pacemaker.backoff_multiplier == 0
            || pacemaker.rtt_floor_multiplier == 0
            || pacemaker.max_backoff_ms == 0
        {
            emitter.emit(
                Report::new(ParseError::InvalidSumeragiConfig)
                    .attach("sumeragi.advanced.pacemaker.backoff_multiplier, rtt_floor_multiplier, and max_backoff_ms must be greater than zero"),
            );
            false
        } else {
            true
        };
        let da_enabled_ok = if da.enabled {
            true
        } else {
            emitter.emit(
                Report::new(ParseError::InvalidSumeragiConfig)
                    .attach("sumeragi.da.enabled must be true for this release"),
            );
            false
        };
        let da_quorum_multiplier_ok = if da_advanced.quorum_timeout_multiplier == 0 {
            emitter.emit(Report::new(ParseError::InvalidSumeragiConfig).attach(
                "sumeragi.advanced.da.quorum_timeout_multiplier must be greater than zero",
            ));
            false
        } else {
            true
        };
        let da_availability_multiplier_ok = if da_advanced.availability_timeout_multiplier == 0 {
            emitter.emit(Report::new(ParseError::InvalidSumeragiConfig).attach(
                "sumeragi.advanced.da.availability_timeout_multiplier must be greater than zero",
            ));
            false
        } else {
            true
        };
        let da_caps_ok = if da.max_commitments_per_block == 0 {
            emitter.emit(
                Report::new(ParseError::InvalidSumeragiConfig)
                    .attach("sumeragi.da.max_commitments_per_block must be greater than zero"),
            );
            false
        } else {
            true
        };
        let da_openings_ok = if da.max_proof_openings_per_block == 0 {
            emitter.emit(
                Report::new(ParseError::InvalidSumeragiConfig)
                    .attach("sumeragi.da.max_proof_openings_per_block must be greater than zero"),
            );
            false
        } else {
            true
        };
        let kura_retry_ok = if persistence.kura_retry_max_attempts == 0
            || persistence.kura_retry_interval_ms == 0
        {
            emitter.emit(Report::new(ParseError::InvalidSumeragiConfig).attach(
                "sumeragi.persistence.kura_retry_max_attempts and kura_retry_interval_ms must be greater than zero",
            ));
            false
        } else {
            true
        };
        let commit_inflight_ok = if persistence.commit_inflight_timeout_ms == 0 {
            emitter.emit(Report::new(ParseError::InvalidSumeragiConfig).attach(
                "sumeragi.persistence.commit_inflight_timeout_ms must be greater than zero",
            ));
            false
        } else {
            true
        };
        let commit_work_queue_ok = if persistence.commit_work_queue_cap == 0 {
            emitter
                .emit(Report::new(ParseError::InvalidSumeragiConfig).attach(
                    "sumeragi.persistence.commit_work_queue_cap must be greater than zero",
                ));
            false
        } else {
            true
        };
        let commit_result_queue_ok =
            if persistence.commit_result_queue_cap == 0 {
                emitter.emit(Report::new(ParseError::InvalidSumeragiConfig).attach(
                    "sumeragi.persistence.commit_result_queue_cap must be greater than zero",
                ));
                false
            } else {
                true
            };
        let height_attempt_cap_ok = if recovery.height_attempt_cap == 0 {
            emitter.emit(
                Report::new(ParseError::InvalidSumeragiConfig)
                    .attach("sumeragi.recovery.height_attempt_cap must be greater than zero"),
            );
            false
        } else {
            true
        };
        let height_window_ok = if recovery.height_window_ms == 0 {
            emitter.emit(
                Report::new(ParseError::InvalidSumeragiConfig)
                    .attach("sumeragi.recovery.height_window_ms must be greater than zero"),
            );
            false
        } else {
            true
        };
        let hash_miss_cap_ok = if recovery.hash_miss_cap_before_range_pull == 0 {
            emitter.emit(Report::new(ParseError::InvalidSumeragiConfig).attach(
                "sumeragi.recovery.hash_miss_cap_before_range_pull must be greater than zero",
            ));
            false
        } else {
            true
        };
        let missing_qc_reacquire_window_ok = if recovery.missing_qc_reacquire_window_ms == 0 {
            emitter.emit(Report::new(ParseError::InvalidSumeragiConfig).attach(
                "sumeragi.recovery.missing_qc_reacquire_window_ms must be greater than zero",
            ));
            false
        } else {
            true
        };
        let max_forced_proposal_attempts_ok = if recovery.max_forced_proposal_attempts_per_view == 0
        {
            emitter.emit(Report::new(ParseError::InvalidSumeragiConfig).attach(
                "sumeragi.recovery.max_forced_proposal_attempts_per_view must be greater than zero",
            ));
            false
        } else {
            true
        };
        let backlog_extension_factor_ok =
            if !recovery.view_change_backlog_extension_factor.is_finite()
                || recovery.view_change_backlog_extension_factor < 1.0
            {
                emitter.emit(Report::new(ParseError::InvalidSumeragiConfig).attach(
                "sumeragi.recovery.view_change_backlog_extension_factor must be finite and >= 1.0",
            ));
                false
            } else {
                true
            };
        let backlog_extension_cap_ok = if recovery.view_change_backlog_extension_cap_ms == 0 {
            emitter.emit(Report::new(ParseError::InvalidSumeragiConfig).attach(
                "sumeragi.recovery.view_change_backlog_extension_cap_ms must be greater than zero",
            ));
            false
        } else {
            true
        };
        let deferred_qc_ttl_ok = if recovery.deferred_qc_ttl_ms == 0 {
            emitter.emit(
                Report::new(ParseError::InvalidSumeragiConfig)
                    .attach("sumeragi.recovery.deferred_qc_ttl_ms must be greater than zero"),
            );
            false
        } else {
            true
        };
        let missing_block_height_attempt_cap_ok = if recovery.missing_block_height_attempt_cap == 0
        {
            emitter.emit(Report::new(ParseError::InvalidSumeragiConfig).attach(
                "sumeragi.recovery.missing_block_height_attempt_cap must be greater than zero",
            ));
            false
        } else {
            true
        };
        let missing_block_height_ttl_ok =
            if recovery.missing_block_height_ttl_ms == 0 {
                emitter.emit(Report::new(ParseError::InvalidSumeragiConfig).attach(
                    "sumeragi.recovery.missing_block_height_ttl_ms must be greater than zero",
                ));
                false
            } else {
                true
            };
        let sidecar_mismatch_retry_cap_ok =
            if recovery.sidecar_mismatch_retry_cap == 0 {
                emitter.emit(Report::new(ParseError::InvalidSumeragiConfig).attach(
                    "sumeragi.recovery.sidecar_mismatch_retry_cap must be greater than zero",
                ));
                false
            } else {
                true
            };
        let sidecar_mismatch_ttl_ok = if recovery.sidecar_mismatch_ttl_ms == 0 {
            emitter.emit(
                Report::new(ParseError::InvalidSumeragiConfig)
                    .attach("sumeragi.recovery.sidecar_mismatch_ttl_ms must be greater than zero"),
            );
            false
        } else {
            true
        };
        let range_pull_escalation_after_hash_misses_ok = if recovery
            .range_pull_escalation_after_hash_misses
            == 0
        {
            emitter.emit(Report::new(ParseError::InvalidSumeragiConfig).attach(
                    "sumeragi.recovery.range_pull_escalation_after_hash_misses must be greater than zero",
                ));
            false
        } else {
            true
        };
        let pending_block_sync_cap_ok = if recovery.pending_block_sync_cap == 0 {
            emitter.emit(
                Report::new(ParseError::InvalidSumeragiConfig)
                    .attach("sumeragi.recovery.pending_block_sync_cap must be greater than zero"),
            );
            false
        } else {
            true
        };
        let pending_proposal_cap_ok = if recovery.pending_proposal_cap == 0 {
            emitter.emit(
                Report::new(ParseError::InvalidSumeragiConfig)
                    .attach("sumeragi.recovery.pending_proposal_cap must be greater than zero"),
            );
            false
        } else {
            true
        };
        let missing_fetch_aggressive_after_attempts_ok = if recovery
            .missing_fetch_aggressive_after_attempts
            == 0
        {
            emitter.emit(Report::new(ParseError::InvalidSumeragiConfig).attach(
                    "sumeragi.recovery.missing_fetch_aggressive_after_attempts must be greater than zero",
                ));
            false
        } else {
            true
        };
        let fanout_threshold_ok = if fanout.large_set_threshold == 0 {
            emitter.emit(
                Report::new(ParseError::InvalidSumeragiConfig)
                    .attach("sumeragi.fanout.large_set_threshold must be greater than zero"),
            );
            false
        } else {
            true
        };
        let fanout_lookback_ok = if fanout.activity_lookback_blocks == 0 {
            emitter.emit(
                Report::new(ParseError::InvalidSumeragiConfig)
                    .attach("sumeragi.fanout.activity_lookback_blocks must be greater than zero"),
            );
            false
        } else {
            true
        };
        let membership_mismatch_threshold_ok = if gating.membership_mismatch_alert_threshold == 0 {
            emitter.emit(Report::new(ParseError::InvalidSumeragiConfig).attach(
                "sumeragi.gating.membership_mismatch_alert_threshold must be greater than zero",
            ));
            false
        } else {
            true
        };
        let rbc_chunk_max_ok = if rbc.chunk_max_bytes == 0 {
            emitter.emit(
                Report::new(ParseError::InvalidSumeragiConfig)
                    .attach("sumeragi.advanced.rbc.chunk_max_bytes must be greater than zero"),
            );
            false
        } else {
            true
        };
        let pending_caps_ok = if rbc.pending_max_chunks == 0 || rbc.pending_max_bytes == 0 {
            emitter.emit(Report::new(ParseError::InvalidSumeragiConfig).attach(
                "sumeragi.advanced.rbc.pending_max_chunks and pending_max_bytes must be greater than zero",
            ));
            false
        } else if rbc.pending_ttl_ms == 0 {
            emitter.emit(
                Report::new(ParseError::InvalidSumeragiConfig)
                    .attach("sumeragi.advanced.rbc.pending_ttl_ms must be greater than zero"),
            );
            false
        } else if rbc.pending_max_bytes < rbc.chunk_max_bytes {
            emitter.emit(Report::new(ParseError::InvalidSumeragiConfig).attach(
                "sumeragi.advanced.rbc.pending_max_bytes must be at least sumeragi.advanced.rbc.chunk_max_bytes",
            ));
            false
        } else if rbc.pending_session_limit == 0 {
            emitter.emit(
                Report::new(ParseError::InvalidSumeragiConfig).attach(
                    "sumeragi.advanced.rbc.pending_session_limit must be greater than zero",
                ),
            );
            false
        } else if rbc.session_ttl_ms == 0 {
            emitter.emit(
                Report::new(ParseError::InvalidSumeragiConfig)
                    .attach("sumeragi.advanced.rbc.session_ttl_ms must be greater than zero"),
            );
            false
        } else {
            true
        };
        let rbc_rebroadcast_budget_ok = if rbc.rebroadcast_sessions_per_tick == 0 {
            emitter.emit(Report::new(ParseError::InvalidSumeragiConfig).attach(
                "sumeragi.advanced.rbc.rebroadcast_sessions_per_tick must be greater than zero",
            ));
            false
        } else {
            true
        };
        let rbc_payload_budget_ok =
            if rbc.payload_chunks_per_tick == 0 {
                emitter.emit(Report::new(ParseError::InvalidSumeragiConfig).attach(
                    "sumeragi.advanced.rbc.payload_chunks_per_tick must be greater than zero",
                ));
                false
            } else {
                true
            };

        let adaptive_observability = adaptive_observability.parse(emitter)?;
        let pacing_governor = pacing_governor.parse(emitter)?;
        let npos = npos.parse(npos_advanced.timeouts, emitter)?;

        if !(collectors_ok
            && redundant_ok
            && queues_ok
            && worker_budget_ok
            && worker_drain_budget_ok
            && validation_threads_ok
            && validation_work_queue_ok
            && validation_result_queue_ok
            && validation_queue_full_inline_cutover_divisor_ok
            && validation_pending_cap_ok
            && vote_burst_cap_ok
            && urgent_da_streak_ok
            && pacemaker_backoff_ok
            && da_enabled_ok
            && da_quorum_multiplier_ok
            && da_availability_multiplier_ok
            && da_caps_ok
            && da_openings_ok
            && kura_retry_ok
            && commit_inflight_ok
            && commit_work_queue_ok
            && commit_result_queue_ok
            && height_attempt_cap_ok
            && height_window_ok
            && hash_miss_cap_ok
            && missing_qc_reacquire_window_ok
            && max_forced_proposal_attempts_ok
            && backlog_extension_factor_ok
            && backlog_extension_cap_ok
            && deferred_qc_ttl_ok
            && missing_block_height_attempt_cap_ok
            && missing_block_height_ttl_ok
            && sidecar_mismatch_retry_cap_ok
            && sidecar_mismatch_ttl_ok
            && range_pull_escalation_after_hash_misses_ok
            && pending_block_sync_cap_ok
            && pending_proposal_cap_ok
            && missing_fetch_aggressive_after_attempts_ok
            && fanout_threshold_ok
            && fanout_lookback_ok
            && membership_mismatch_threshold_ok
            && rbc_chunk_max_ok
            && pending_caps_ok
            && rbc_rebroadcast_budget_ok
            && rbc_payload_budget_ok)
        {
            return None;
        }

        let key_algorithms: BTreeSet<Algorithm> = if keys.allowed_algorithms.is_empty() {
            emitter.emit(
                Report::new(ParseError::InvalidSumeragiConfig)
                    .attach("sumeragi.keys.allowed_algorithms must not be empty"),
            );
            None
        } else {
            Some(keys.allowed_algorithms.into_iter().collect())
        }?;
        if !key_algorithms.contains(&Algorithm::BlsNormal) {
            emitter.emit(
                Report::new(ParseError::InvalidSumeragiConfig)
                    .attach("sumeragi.keys.allowed_algorithms must include BlsNormal"),
            );
            return None;
        }

        let key_providers: BTreeSet<String> =
            if keys.require_hsm && keys.allowed_hsm_providers.is_empty() {
                emitter.emit(Report::new(ParseError::InvalidSumeragiConfig).attach(
                    "sumeragi.keys.allowed_hsm_providers must not be empty when HSM is required",
                ));
                None
            } else {
                Some(
                    keys.allowed_hsm_providers
                        .into_iter()
                        .map(|s| s.trim().to_owned())
                        .filter(|s| !s.is_empty())
                        .collect::<BTreeSet<_>>(),
                )
            }?;

        let proof_policy = match finality.proof_policy {
            ProofPolicy::Off => actual::ProofPolicy::Off,
            ProofPolicy::ZkParent => actual::ProofPolicy::ZkParent,
        };
        let recovery_height_attempt_cap = if recovery.height_attempt_cap
            == defaults::sumeragi::RECOVERY_HEIGHT_ATTEMPT_CAP
            && recovery.missing_block_height_attempt_cap
                != defaults::sumeragi::MISSING_BLOCK_HEIGHT_ATTEMPT_CAP
        {
            recovery.missing_block_height_attempt_cap
        } else {
            recovery.height_attempt_cap
        };
        let recovery_height_window_ms = if recovery.height_window_ms
            == defaults::sumeragi::RECOVERY_HEIGHT_WINDOW_MS
            && recovery.missing_block_height_ttl_ms
                != defaults::sumeragi::MISSING_BLOCK_HEIGHT_TTL_MS
        {
            recovery.missing_block_height_ttl_ms
        } else {
            recovery.height_window_ms
        };
        let recovery_hash_miss_cap_before_range_pull = if recovery.hash_miss_cap_before_range_pull
            == defaults::sumeragi::RECOVERY_HASH_MISS_CAP_BEFORE_RANGE_PULL
            && recovery.range_pull_escalation_after_hash_misses
                != defaults::sumeragi::RANGE_PULL_ESCALATION_AFTER_HASH_MISSES
        {
            recovery.range_pull_escalation_after_hash_misses
        } else {
            recovery.hash_miss_cap_before_range_pull
        };

        Some(actual::Sumeragi {
            role: match role {
                NodeRole::Validator => actual::NodeRole::Validator,
                NodeRole::Observer => actual::NodeRole::Observer,
            },
            consensus_mode: match consensus_mode {
                ConsensusMode::Permissioned => actual::ConsensusMode::Permissioned,
                ConsensusMode::Npos => actual::ConsensusMode::Npos,
            },
            mode_flip: actual::SumeragiModeFlip {
                enabled: mode_flip.enabled,
            },
            collectors: actual::SumeragiCollectors {
                k: collectors.k,
                redundant_send_r: collectors.redundant_send_r,
                parallel_topology_fanout: collectors.parallel_topology_fanout,
            },
            block: actual::SumeragiBlock {
                max_transactions: block.max_transactions,
                fast_gas_limit_per_block: block.fast_gas_limit_per_block,
                max_payload_bytes: block.max_payload_bytes,
                proposal_queue_scan_multiplier: block.proposal_queue_scan_multiplier,
            },
            queues: actual::SumeragiQueues {
                votes: queues.votes,
                block_payload: queues.block_payload,
                rbc_chunks: queues.rbc_chunks,
                blocks: queues.blocks,
                control: queues.control,
            },
            worker: actual::SumeragiWorker {
                iteration_budget_cap: std::time::Duration::from_millis(
                    worker.iteration_budget_cap_ms,
                ),
                iteration_drain_budget_cap: std::time::Duration::from_millis(
                    worker.iteration_drain_budget_cap_ms,
                ),
                tick_work_budget_cap: std::time::Duration::from_millis(
                    worker.tick_work_budget_cap_ms,
                ),
                parallel_ingress: worker.parallel_ingress,
                validation_worker_threads: worker.validation_worker_threads,
                validation_work_queue_cap: worker.validation_work_queue_cap,
                validation_result_queue_cap: worker.validation_result_queue_cap,
                validation_queue_full_inline_cutover_divisor: worker
                    .validation_queue_full_inline_cutover_divisor,
                qc_verify_worker_threads: worker.qc_verify_worker_threads,
                qc_verify_work_queue_cap: worker.qc_verify_work_queue_cap,
                qc_verify_result_queue_cap: worker.qc_verify_result_queue_cap,
                validation_pending_cap: worker.validation_pending_cap,
                vote_burst_cap_with_payload_backlog: worker.vote_burst_cap_with_payload_backlog,
                max_urgent_before_da_critical: worker.max_urgent_before_da_critical,
            },
            pacemaker: actual::SumeragiPacemaker {
                backoff_multiplier: pacemaker.backoff_multiplier,
                rtt_floor_multiplier: pacemaker.rtt_floor_multiplier,
                max_backoff: std::time::Duration::from_millis(pacemaker.max_backoff_ms),
                jitter_frac_permille: pacemaker.jitter_frac_permille,
                pending_stall_grace: std::time::Duration::from_millis(
                    pacemaker.pending_stall_grace_ms,
                ),
                da_fast_reschedule: pacemaker.da_fast_reschedule,
                active_pending_soft_limit: pacemaker.active_pending_soft_limit,
                rbc_backlog_session_soft_limit: pacemaker.rbc_backlog_session_soft_limit,
                rbc_backlog_chunk_soft_limit: pacemaker.rbc_backlog_chunk_soft_limit,
            },
            pacing_governor,
            da: actual::SumeragiDa {
                enabled: da.enabled,
                quorum_timeout_multiplier: da_advanced.quorum_timeout_multiplier,
                availability_timeout_multiplier: da_advanced.availability_timeout_multiplier,
                availability_timeout_floor: std::time::Duration::from_millis(
                    da_advanced.availability_timeout_floor_ms,
                ),
                max_commitments_per_block: da.max_commitments_per_block,
                max_proof_openings_per_block: da.max_proof_openings_per_block,
            },
            persistence: actual::SumeragiPersistence {
                kura_retry_interval: std::time::Duration::from_millis(
                    persistence.kura_retry_interval_ms,
                ),
                kura_retry_max_attempts: persistence.kura_retry_max_attempts,
                commit_inflight_timeout: std::time::Duration::from_millis(
                    persistence.commit_inflight_timeout_ms,
                ),
                commit_work_queue_cap: persistence.commit_work_queue_cap,
                commit_result_queue_cap: persistence.commit_result_queue_cap,
            },
            recovery: actual::SumeragiRecovery {
                height_attempt_cap: recovery_height_attempt_cap,
                height_window: std::time::Duration::from_millis(recovery_height_window_ms),
                hash_miss_cap_before_range_pull: recovery_hash_miss_cap_before_range_pull,
                missing_qc_reacquire_window: std::time::Duration::from_millis(
                    recovery.missing_qc_reacquire_window_ms.max(1),
                ),
                max_forced_proposal_attempts_per_view: recovery
                    .max_forced_proposal_attempts_per_view
                    .max(1),
                rotate_after_reacquire_exhausted: recovery.rotate_after_reacquire_exhausted,
                missing_block_signer_fallback_attempts: recovery
                    .missing_block_signer_fallback_attempts,
                missing_block_retry_backoff_multiplier: recovery
                    .missing_block_retry_backoff_multiplier
                    .max(1),
                missing_block_retry_backoff_cap: std::time::Duration::from_millis(
                    recovery.missing_block_retry_backoff_cap_ms.max(1),
                ),
                view_change_backlog_extension_factor: recovery.view_change_backlog_extension_factor,
                view_change_backlog_extension_cap: std::time::Duration::from_millis(
                    recovery.view_change_backlog_extension_cap_ms,
                ),
                deferred_qc_ttl: std::time::Duration::from_millis(recovery.deferred_qc_ttl_ms),
                missing_block_height_attempt_cap: recovery_height_attempt_cap,
                missing_block_height_ttl: std::time::Duration::from_millis(
                    recovery_height_window_ms,
                ),
                sidecar_mismatch_retry_cap: recovery.sidecar_mismatch_retry_cap,
                sidecar_mismatch_ttl: std::time::Duration::from_millis(
                    recovery.sidecar_mismatch_ttl_ms,
                ),
                range_pull_escalation_after_hash_misses: recovery_hash_miss_cap_before_range_pull,
                missing_request_stale_height_margin: recovery.missing_request_stale_height_margin,
                pending_block_sync_cap: recovery.pending_block_sync_cap.max(1),
                pending_proposal_cap: recovery.pending_proposal_cap.max(1),
                missing_fetch_aggressive_after_attempts: recovery
                    .missing_fetch_aggressive_after_attempts
                    .max(1),
            },
            fanout: actual::SumeragiFanout {
                large_set_threshold: fanout.large_set_threshold,
                activity_lookback_blocks: fanout.activity_lookback_blocks,
            },
            gating: actual::SumeragiGating {
                future_height_window: gating.future_height_window,
                future_view_window: gating.future_view_window,
                invalid_sig_penalty_threshold: gating.invalid_sig_penalty_threshold,
                invalid_sig_penalty_window: std::time::Duration::from_millis(
                    gating.invalid_sig_penalty_window_ms,
                ),
                invalid_sig_penalty_cooldown: std::time::Duration::from_millis(
                    gating.invalid_sig_penalty_cooldown_ms,
                ),
                membership_mismatch_alert_threshold: gating.membership_mismatch_alert_threshold,
                membership_mismatch_fail_closed: gating.membership_mismatch_fail_closed,
            },
            rbc: actual::SumeragiRbc {
                chunk_max_bytes: rbc.chunk_max_bytes,
                chunk_fanout: rbc.chunk_fanout,
                pending_max_chunks: rbc.pending_max_chunks,
                pending_max_bytes: rbc.pending_max_bytes,
                pending_session_limit: rbc.pending_session_limit,
                pending_ttl: std::time::Duration::from_millis(rbc.pending_ttl_ms),
                session_ttl: std::time::Duration::from_millis(rbc.session_ttl_ms),
                rebroadcast_sessions_per_tick: rbc.rebroadcast_sessions_per_tick,
                payload_chunks_per_tick: rbc.payload_chunks_per_tick,
                store_max_sessions: rbc.store_max_sessions,
                store_soft_sessions: rbc.store_soft_sessions,
                store_max_bytes: rbc.store_max_bytes,
                store_soft_bytes: rbc.store_soft_bytes,
                disk_store_ttl: std::time::Duration::from_millis(rbc.disk_store_ttl_ms),
                disk_store_max_bytes: rbc.disk_store_max_bytes,
            },
            finality: actual::SumeragiFinality {
                proof_policy,
                commit_cert_history_cap: finality.commit_cert_history_cap,
                zk_finality_k: finality.zk_finality_k,
                require_precommit_qc: finality.require_precommit_qc,
            },
            keys: actual::SumeragiKeys {
                activation_lead_blocks: keys.activation_lead_blocks,
                overlap_grace_blocks: keys.overlap_grace_blocks,
                expiry_grace_blocks: keys.expiry_grace_blocks,
                require_hsm: keys.require_hsm,
                allowed_algorithms: key_algorithms,
                allowed_hsm_providers: key_providers,
            },
            npos,
            adaptive_observability,
            debug: actual::SumeragiDebug {
                force_soft_fork: debug.force_soft_fork,
                disable_background_worker: debug.disable_background_worker,
                rbc: actual::SumeragiDebugRbc {
                    drop_every_nth_chunk: debug.rbc.drop_every_nth_chunk,
                    shuffle_chunks: debug.rbc.shuffle_chunks,
                    duplicate_inits: debug.rbc.duplicate_inits,
                    force_deliver_quorum_one: debug.rbc.force_deliver_quorum_one,
                    corrupt_witness_ack: debug.rbc.corrupt_witness_ack,
                    corrupt_ready_signature: debug.rbc.corrupt_ready_signature,
                    drop_validator_mask: debug.rbc.drop_validator_mask,
                    equivocate_chunk_mask: debug.rbc.equivocate_chunk_mask,
                    equivocate_validator_mask: debug.rbc.equivocate_validator_mask,
                    conflicting_ready_mask: debug.rbc.conflicting_ready_mask,
                    partial_chunk_mask: debug.rbc.partial_chunk_mask,
                },
            },
        })
    }
}

fn scale_ratio_at_least_one(value: u64, numerator: u64, denominator: u64) -> u64 {
    let scaled = (u128::from(value) * u128::from(numerator) + (u128::from(denominator) / 2))
        / u128::from(denominator);
    let scaled = u64::try_from(scaled).unwrap_or(u64::MAX);
    scaled.max(1)
}

fn derive_vrf_window_blocks(epoch_length_blocks: u64, default_window_blocks: u64) -> u64 {
    scale_ratio_at_least_one(
        epoch_length_blocks,
        default_window_blocks,
        defaults::sumeragi::EPOCH_LENGTH_BLOCKS,
    )
}

impl SumeragiNpos {
    fn parse(
        self,
        timeouts: SumeragiNposTimeouts,
        emitter: &mut Emitter<ParseError>,
    ) -> Option<actual::SumeragiNpos> {
        let Self {
            epoch_length_blocks,
            use_stake_snapshot_roster,
            vrf,
            election,
            reconfig,
        } = self;

        let epoch_length_ok = if epoch_length_blocks == 0 {
            emitter.emit(
                Report::new(ParseError::InvalidSumeragiConfig)
                    .attach("sumeragi.npos.epoch_length_blocks must be greater than zero"),
            );
            false
        } else {
            true
        };

        let timeouts_overrides = timeouts.parse_overrides(emitter);
        let vrf = vrf.parse(epoch_length_blocks, emitter)?;
        let election = election.parse(emitter)?;
        let reconfig = reconfig.parse(emitter)?;

        if !epoch_length_ok {
            return None;
        }

        Some(actual::SumeragiNpos {
            timeouts_overrides,
            vrf,
            election,
            reconfig,
            epoch_length_blocks,
            use_stake_snapshot_roster,
        })
    }
}

impl SumeragiNposTimeouts {
    fn parse_overrides(
        self,
        _emitter: &mut Emitter<ParseError>,
    ) -> actual::SumeragiNposTimeoutOverrides {
        let to_override = |value: Option<u64>| -> Option<Duration> {
            match value {
                Some(0) | None => None,
                Some(value) => Some(Duration::from_millis(value)),
            }
        };

        actual::SumeragiNposTimeoutOverrides {
            propose: to_override(self.propose_ms),
            prevote: to_override(self.prevote_ms),
            precommit: to_override(self.precommit_ms),
            exec: to_override(self.exec_ms),
            witness: to_override(self.witness_ms),
            commit: to_override(self.commit_ms),
            da: to_override(self.da_ms),
            aggregator: to_override(self.aggregator_ms),
        }
    }
}

impl SumeragiNposVrf {
    fn parse(
        self,
        epoch_length_blocks: u64,
        emitter: &mut Emitter<ParseError>,
    ) -> Option<actual::SumeragiNposVrf> {
        let mut valid = true;

        let mut resolve_window =
            |value: Option<u64>, field: &'static str, derived: u64| -> u64 {
                value.map_or(derived, |value| {
                    if value == 0 {
                        emitter.emit(Report::new(ParseError::InvalidSumeragiConfig).attach(
                            format!("sumeragi.npos.vrf.{field} must be greater than zero"),
                        ));
                        valid = false;
                    }
                    value
                })
            };

        let commit_window_blocks = resolve_window(
            self.commit_window_blocks,
            "commit_window_blocks",
            derive_vrf_window_blocks(
                epoch_length_blocks,
                defaults::sumeragi::npos::VRF_COMMIT_WINDOW_BLOCKS,
            ),
        );
        let reveal_window_blocks = resolve_window(
            self.reveal_window_blocks,
            "reveal_window_blocks",
            derive_vrf_window_blocks(
                epoch_length_blocks,
                defaults::sumeragi::npos::VRF_REVEAL_WINDOW_BLOCKS,
            ),
        );

        if !valid {
            return None;
        }

        let commit_deadline_offset_blocks = self
            .commit_deadline_offset_blocks
            .unwrap_or(commit_window_blocks);
        let reveal_deadline_offset_blocks = self
            .reveal_deadline_offset_blocks
            .unwrap_or(commit_window_blocks.saturating_add(reveal_window_blocks));

        Some(actual::SumeragiNposVrf {
            commit_window_blocks,
            reveal_window_blocks,
            commit_deadline_offset_blocks,
            reveal_deadline_offset_blocks,
        })
    }
}

impl SumeragiNposElection {
    fn parse(self, emitter: &mut Emitter<ParseError>) -> Option<actual::SumeragiNposElection> {
        let max_validators = self.max_validators;
        let min_self_bond_ok = if self.min_self_bond == 0 {
            emitter.emit(
                Report::new(ParseError::InvalidSumeragiConfig)
                    .attach("sumeragi.npos.election.min_self_bond must be greater than zero"),
            );
            false
        } else {
            true
        };
        let min_nomination_ok = if self.min_nomination_bond == 0 {
            emitter
                .emit(Report::new(ParseError::InvalidSumeragiConfig).attach(
                    "sumeragi.npos.election.min_nomination_bond must be greater than zero",
                ));
            false
        } else {
            true
        };
        let finality_margin_ok =
            if self.finality_margin_blocks == 0 {
                emitter.emit(Report::new(ParseError::InvalidSumeragiConfig).attach(
                    "sumeragi.npos.election.finality_margin_blocks must be greater than zero",
                ));
                false
            } else {
                true
            };

        let mut check_pct = |value: u8, field: &str| {
            if value > 100 {
                emitter.emit(
                    Report::new(ParseError::InvalidSumeragiConfig).attach(format!(
                        "sumeragi.npos.election.{field} must be between 0 and 100"
                    )),
                );
                false
            } else {
                true
            }
        };

        let concentration_ok = check_pct(
            self.max_nominator_concentration_pct,
            "max_nominator_concentration_pct",
        );
        let seat_band_ok = check_pct(self.seat_band_pct, "seat_band_pct");
        let correlation_ok = check_pct(
            self.max_entity_correlation_pct,
            "max_entity_correlation_pct",
        );

        if !(min_self_bond_ok
            && min_nomination_ok
            && finality_margin_ok
            && concentration_ok
            && seat_band_ok
            && correlation_ok)
        {
            return None;
        }

        Some(actual::SumeragiNposElection {
            max_validators,
            min_self_bond: self.min_self_bond,
            min_nomination_bond: self.min_nomination_bond,
            max_nominator_concentration_pct: self.max_nominator_concentration_pct,
            seat_band_pct: self.seat_band_pct,
            max_entity_correlation_pct: self.max_entity_correlation_pct,
            finality_margin_blocks: self.finality_margin_blocks,
        })
    }
}

impl SumeragiNposReconfig {
    fn parse(self, emitter: &mut Emitter<ParseError>) -> Option<actual::SumeragiNposReconfig> {
        let evidence_ok = if self.evidence_horizon_blocks == 0 {
            emitter.emit(Report::new(ParseError::InvalidSumeragiConfig).attach(
                "sumeragi.npos.reconfig.evidence_horizon_blocks must be greater than zero",
            ));
            false
        } else {
            true
        };
        let activation_ok =
            if self.activation_lag_blocks == 0 {
                emitter.emit(Report::new(ParseError::InvalidSumeragiConfig).attach(
                    "sumeragi.npos.reconfig.activation_lag_blocks must be greater than zero",
                ));
                false
            } else {
                true
            };
        let slashing_ok =
            if self.slashing_delay_blocks == 0 {
                emitter.emit(Report::new(ParseError::InvalidSumeragiConfig).attach(
                    "sumeragi.npos.reconfig.slashing_delay_blocks must be greater than zero",
                ));
                false
            } else {
                true
            };

        if !(evidence_ok && activation_ok && slashing_ok) {
            return None;
        }

        Some(actual::SumeragiNposReconfig {
            evidence_horizon_blocks: self.evidence_horizon_blocks,
            activation_lag_blocks: self.activation_lag_blocks,
            slashing_delay_blocks: self.slashing_delay_blocks,
        })
    }
}

#[cfg(test)]
mod sumeragi_npos_tests {
    use super::*;

    #[test]
    fn npos_parse_converts_to_durations() {
        let timeouts = SumeragiNposTimeouts {
            propose_ms: Some(200),
            prevote_ms: Some(210),
            precommit_ms: Some(220),
            exec_ms: Some(225),
            witness_ms: Some(228),
            commit_ms: Some(230),
            da_ms: Some(310),
            aggregator_ms: Some(120),
        };
        let user = SumeragiNpos {
            epoch_length_blocks: 256,
            use_stake_snapshot_roster: true,
            vrf: SumeragiNposVrf {
                commit_window_blocks: Some(42),
                reveal_window_blocks: Some(13),
                commit_deadline_offset_blocks: Some(5),
                reveal_deadline_offset_blocks: Some(8),
            },
            election: SumeragiNposElection {
                max_validators: 7,
                min_self_bond: 10_000,
                min_nomination_bond: 50,
                max_nominator_concentration_pct: 55,
                seat_band_pct: 18,
                max_entity_correlation_pct: 27,
                finality_margin_blocks: 12,
            },
            reconfig: SumeragiNposReconfig {
                evidence_horizon_blocks: 256,
                activation_lag_blocks: 2,
                slashing_delay_blocks: 9,
            },
        };

        let mut emitter = Emitter::new();
        let actual = user
            .parse(timeouts, &mut emitter)
            .expect("configuration should be valid");
        emitter
            .into_result()
            .expect("no validation errors expected");
        assert_eq!(actual.epoch_length_blocks, 256);
        assert!(actual.use_stake_snapshot_roster);
        assert_eq!(
            actual.timeouts_overrides.propose,
            Some(std::time::Duration::from_millis(200))
        );
        assert_eq!(
            actual.timeouts_overrides.exec,
            Some(std::time::Duration::from_millis(225))
        );
        assert_eq!(
            actual.timeouts_overrides.witness,
            Some(std::time::Duration::from_millis(228))
        );
        assert_eq!(
            actual.timeouts_overrides.aggregator,
            Some(std::time::Duration::from_millis(120))
        );
        assert_eq!(actual.vrf.commit_window_blocks, 42);
        assert_eq!(actual.vrf.reveal_window_blocks, 13);
        assert_eq!(actual.vrf.commit_deadline_offset_blocks, 5);
        assert_eq!(actual.vrf.reveal_deadline_offset_blocks, 8);
        assert_eq!(actual.election.max_validators, 7);
        assert_eq!(actual.election.min_self_bond, 10_000);
        assert_eq!(actual.election.min_nomination_bond, 50);
        assert_eq!(actual.election.seat_band_pct, 18);
        assert_eq!(actual.election.finality_margin_blocks, 12);
        assert_eq!(actual.reconfig.evidence_horizon_blocks, 256);
        assert_eq!(actual.reconfig.activation_lag_blocks, 2);
        assert_eq!(actual.reconfig.slashing_delay_blocks, 9);
    }

    #[test]
    fn npos_requires_positive_epoch_length() {
        let timeouts = SumeragiNposTimeouts {
            propose_ms: Some(1),
            prevote_ms: Some(1),
            precommit_ms: Some(1),
            exec_ms: Some(1),
            witness_ms: Some(1),
            commit_ms: Some(1),
            da_ms: Some(1),
            aggregator_ms: Some(1),
        };
        let user = SumeragiNpos {
            epoch_length_blocks: 0,
            use_stake_snapshot_roster: false,
            vrf: SumeragiNposVrf {
                commit_window_blocks: Some(1),
                reveal_window_blocks: Some(1),
                commit_deadline_offset_blocks: Some(1),
                reveal_deadline_offset_blocks: Some(2),
            },
            election: SumeragiNposElection {
                max_validators: 1,
                min_self_bond: 1,
                min_nomination_bond: 1,
                max_nominator_concentration_pct: 10,
                seat_band_pct: 10,
                max_entity_correlation_pct: 10,
                finality_margin_blocks: 1,
            },
            reconfig: SumeragiNposReconfig {
                evidence_horizon_blocks: 1,
                activation_lag_blocks: 1,
                slashing_delay_blocks: 1,
            },
        };

        let mut emitter = Emitter::new();
        assert!(user.parse(timeouts, &mut emitter).is_none());
        let report = emitter.into_result().expect_err("validation should fail");
        let message = format!("{report:?}");
        assert!(message.contains("sumeragi.npos.epoch_length_blocks must be greater than zero"));
    }

    #[test]
    fn npos_timeouts_zero_clears_overrides() {
        let timeouts = SumeragiNposTimeouts {
            propose_ms: Some(0),
            prevote_ms: None,
            precommit_ms: None,
            exec_ms: None,
            witness_ms: None,
            commit_ms: None,
            da_ms: None,
            aggregator_ms: None,
        };
        let user = SumeragiNpos {
            epoch_length_blocks: 1,
            use_stake_snapshot_roster: false,
            vrf: SumeragiNposVrf {
                commit_window_blocks: Some(1),
                reveal_window_blocks: Some(1),
                commit_deadline_offset_blocks: Some(1),
                reveal_deadline_offset_blocks: Some(2),
            },
            election: SumeragiNposElection {
                max_validators: 4,
                min_self_bond: 1,
                min_nomination_bond: 1,
                max_nominator_concentration_pct: 10,
                seat_band_pct: 10,
                max_entity_correlation_pct: 10,
                finality_margin_blocks: 1,
            },
            reconfig: SumeragiNposReconfig {
                evidence_horizon_blocks: 1,
                activation_lag_blocks: 1,
                slashing_delay_blocks: 1,
            },
        };

        let mut emitter = Emitter::new();
        let actual = user
            .parse(timeouts, &mut emitter)
            .expect("configuration should be valid");
        emitter
            .into_result()
            .expect("no validation errors expected");
        assert!(actual.timeouts_overrides.propose.is_none());
    }

    #[test]
    fn npos_vrf_derive_from_epoch_length() {
        let timeouts = SumeragiNposTimeouts {
            propose_ms: Some(350),
            prevote_ms: Some(450),
            precommit_ms: Some(550),
            exec_ms: Some(150),
            witness_ms: Some(150),
            commit_ms: Some(750),
            da_ms: Some(650),
            aggregator_ms: Some(120),
        };
        let user = SumeragiNpos {
            epoch_length_blocks: 7_200,
            use_stake_snapshot_roster: false,
            vrf: SumeragiNposVrf {
                commit_window_blocks: None,
                reveal_window_blocks: None,
                commit_deadline_offset_blocks: None,
                reveal_deadline_offset_blocks: None,
            },
            election: SumeragiNposElection {
                max_validators: 7,
                min_self_bond: 10_000,
                min_nomination_bond: 50,
                max_nominator_concentration_pct: 55,
                seat_band_pct: 18,
                max_entity_correlation_pct: 27,
                finality_margin_blocks: 12,
            },
            reconfig: SumeragiNposReconfig {
                evidence_horizon_blocks: 256,
                activation_lag_blocks: 2,
                slashing_delay_blocks: 9,
            },
        };

        let mut emitter = Emitter::new();
        let actual = user
            .parse(timeouts, &mut emitter)
            .expect("configuration should be valid");
        emitter
            .into_result()
            .expect("no validation errors expected");

        assert_eq!(actual.vrf.commit_window_blocks, 200);
        assert_eq!(actual.vrf.reveal_window_blocks, 80);
        assert_eq!(actual.vrf.commit_deadline_offset_blocks, 200);
        assert_eq!(actual.vrf.reveal_deadline_offset_blocks, 280);
    }
}
/// User-level configuration container for `SumeragiDebugRbc`.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Copy, Clone, Default, ReadConfig, norito::JsonDeserialize)]
pub struct SumeragiDebugRbc {
    /// Drop every Nth chunk (when present) to simulate packet loss.
    pub drop_every_nth_chunk: Option<NonZeroU32>,
    /// Shuffle chunk transmission order deterministically for stress tests.
    #[config(default)]
    pub shuffle_chunks: bool,
    /// Broadcast duplicate RBC init frames for debug scenarios.
    #[config(default)]
    pub duplicate_inits: bool,
    /// Force RBC DELIVER quorum to 1 for deterministic recovery tests.
    #[config(default)]
    #[norito(default)]
    pub force_deliver_quorum_one: bool,
    /// Corrupt witness ACK payloads when debug mode is enabled.
    #[config(default)]
    pub corrupt_witness_ack: bool,
    /// Corrupt READY signatures when debug mode is enabled.
    #[config(default)]
    pub corrupt_ready_signature: bool,
    /// Bitmask of validators to drop entirely from RBC participation.
    #[config(default)]
    pub drop_validator_mask: u64,
    /// Bitmask of chunks that should be equivocated.
    #[config(default)]
    pub equivocate_chunk_mask: u64,
    /// Bitmask of validators that should equivocate on chunks.
    #[config(default)]
    pub equivocate_validator_mask: u64,
    /// Bitmask of validators emitting conflicting READY messages.
    #[config(default)]
    pub conflicting_ready_mask: u64,
    /// Bitmask of chunks withheld from broadcast.
    #[config(default)]
    pub partial_chunk_mask: u64,
}

/// User-level configuration container for `SumeragiDebug`.
#[derive(Debug, Copy, Clone, Default, ReadConfig, norito::JsonDeserialize)]
pub struct SumeragiDebug {
    /// Force a soft fork condition for testing recovery paths.
    #[config(default)]
    pub force_soft_fork: bool,
    /// Disable the Sumeragi background worker thread.
    #[config(default)]
    pub disable_background_worker: bool,
    /// RBC-specific debug toggles.
    #[config(default)]
    pub rbc: SumeragiDebugRbc,
}

/// SoraNet handshake configuration (user view).
#[derive(Debug, Clone, ReadConfig)]
pub struct SoranetHandshake {
    #[config(default = "Self::default_descriptor_commit()")]
    descriptor_commit: WithOrigin<HexBytes>,
    #[config(default = "Self::default_client_capabilities()")]
    client_capabilities: WithOrigin<HexBytes>,
    #[config(default = "Self::default_relay_capabilities()")]
    relay_capabilities: WithOrigin<HexBytes>,
    #[config(default = "Self::default_trust_gossip()")]
    trust_gossip: bool,
    #[config(default = "Self::default_kem_id()")]
    kem_id: u8,
    /// Optional textual override for the ML-KEM suite (takes precedence over `kem_id`).
    kem_suite: Option<WithOrigin<MlKemSuiteParam>>,
    #[config(default = "Self::default_sig_id()")]
    sig_id: u8,
    resume_hash: Option<WithOrigin<HexBytes>>,
    #[config(nested)]
    pow: SoranetHandshakePow,
}

impl SoranetHandshake {
    const fn default_kem_id() -> u8 {
        1
    }

    const fn default_sig_id() -> u8 {
        1
    }

    fn default_descriptor_commit() -> HexBytes {
        HexBytes(DEFAULT_DESCRIPTOR_COMMIT.to_vec())
    }

    fn default_client_capabilities() -> HexBytes {
        HexBytes(DEFAULT_CLIENT_CAPABILITIES.to_vec())
    }

    fn default_relay_capabilities() -> HexBytes {
        HexBytes(DEFAULT_RELAY_CAPABILITIES.to_vec())
    }

    const fn default_trust_gossip() -> bool {
        true
    }

    fn parse(self) -> actual::SoranetHandshake {
        let Self {
            descriptor_commit,
            client_capabilities,
            relay_capabilities,
            trust_gossip,
            kem_id,
            kem_suite,
            sig_id,
            resume_hash,
            pow,
        } = self;

        let resolved_suite = kem_suite.map_or_else(
            || MlKemSuite::from_kem_id(kem_id).unwrap_or(STREAMING_DEFAULT_KEM_SUITE),
            |with_origin| with_origin.into_value().into_suite(),
        );
        let resolved_kem_id = resolved_suite.kem_id();

        actual::SoranetHandshake {
            descriptor_commit: descriptor_commit.map(Into::into),
            client_capabilities: client_capabilities.map(Into::into),
            relay_capabilities: relay_capabilities.map(Into::into),
            trust_gossip,
            kem_id: resolved_kem_id,
            sig_id,
            resume_hash: resume_hash.map(|value| value.map(Into::into)),
            pow: pow.parse(),
        }
    }
}

#[cfg(test)]
mod accel_tests {
    use super::*;

    #[test]
    fn accel_parse_respects_enable_simd_flag() {
        let user = Acceleration {
            enable_simd: false,
            enable_cuda: true,
            enable_metal: true,
            max_gpus: 0,
            merkle_min_leaves_gpu: 0,
            merkle_min_leaves_metal: 0,
            merkle_min_leaves_cuda: 0,
            prefer_cpu_sha2_max_leaves_aarch64: 0,
            prefer_cpu_sha2_max_leaves_x86: 0,
        };

        let actual = user.parse();
        assert!(!actual.enable_simd);
        assert!(actual.enable_cuda);
        assert_eq!(actual.max_gpus, None);
        assert_eq!(
            actual.merkle_min_leaves_gpu,
            defaults::accel::MERKLE_MIN_LEAVES_GPU
        );
    }
}

/// PoW admission settings supplied at the user configuration layer.
#[derive(Debug, Clone, ReadConfig)]
pub struct SoranetHandshakePow {
    #[config(default)]
    required: bool,
    #[config(default = "Self::default_difficulty()")]
    difficulty: u16,
    #[config(default = "Self::default_max_future_skew()")]
    max_future_skew_secs: u64,
    #[config(default = "Self::default_min_ticket_ttl()")]
    min_ticket_ttl_secs: u64,
    #[config(default = "Self::default_ticket_ttl()")]
    ticket_ttl_secs: u64,
    #[config(default = "Self::default_revocation_store_capacity()")]
    revocation_store_capacity: u64,
    #[config(default = "Self::default_revocation_store_ttl()")]
    revocation_store_ttl_secs: u64,
    #[config(default = "Self::default_revocation_store_path()")]
    revocation_store_path: PathBuf,
    /// ML-DSA-44 public key for verifying signed PoW tickets.
    signed_ticket_public_key_hex: Option<WithOrigin<HexBytes>>,
    #[config(nested)]
    puzzle: SoranetHandshakePuzzle,
}

impl SoranetHandshakePow {
    const fn default_difficulty() -> u16 {
        0
    }

    const fn default_max_future_skew() -> u64 {
        300
    }

    const fn default_min_ticket_ttl() -> u64 {
        30
    }

    const fn default_ticket_ttl() -> u64 {
        60
    }

    const fn default_revocation_store_capacity() -> u64 {
        8_192
    }

    const fn default_revocation_store_ttl() -> u64 {
        900
    }

    fn default_revocation_store_path() -> PathBuf {
        PathBuf::from("./storage/soranet/ticket_revocations.norito")
    }

    fn parse(self) -> actual::SoranetPow {
        let Self {
            required,
            difficulty,
            max_future_skew_secs,
            min_ticket_ttl_secs,
            ticket_ttl_secs,
            revocation_store_capacity,
            revocation_store_ttl_secs,
            revocation_store_path,
            signed_ticket_public_key_hex,
            puzzle,
        } = self;

        let min_ticket_ttl = Duration::from_secs(min_ticket_ttl_secs.max(1));
        let mut max_future_skew = Duration::from_secs(max_future_skew_secs.max(1));
        if max_future_skew < min_ticket_ttl {
            max_future_skew = min_ticket_ttl;
        }
        let mut ticket_ttl = Duration::from_secs(ticket_ttl_secs.max(1));
        if ticket_ttl < min_ticket_ttl {
            ticket_ttl = min_ticket_ttl;
        }
        if ticket_ttl > max_future_skew {
            ticket_ttl = max_future_skew;
        }

        let difficulty = u8::try_from(difficulty.clamp(0, u16::from(u8::MAX))).unwrap_or(u8::MAX);
        let revocation_store_capacity =
            usize::try_from(revocation_store_capacity.max(1)).unwrap_or(usize::MAX);
        let revocation_max_ttl = Duration::from_secs(revocation_store_ttl_secs.max(1));

        actual::SoranetPow {
            required,
            difficulty,
            max_future_skew,
            min_ticket_ttl,
            ticket_ttl,
            revocation_store_capacity,
            revocation_max_ttl,
            revocation_store_path: revocation_store_path.to_string_lossy().into_owned().into(),
            signed_ticket_public_key: signed_ticket_public_key_hex
                .map(|value| value.into_value().into()),
            puzzle: puzzle.parse(),
        }
    }
}

/// Puzzle configuration supplied at the user level.
#[derive(Debug, Clone, Copy, ReadConfig)]
pub struct SoranetHandshakePuzzle {
    #[config(default = "Self::default_enabled()")]
    enabled: bool,
    #[config(default = "Self::default_memory_kib()")]
    memory_kib: u32,
    #[config(default = "Self::default_time_cost()")]
    time_cost: u32,
    #[config(default = "Self::default_lanes()")]
    lanes: u32,
}

impl SoranetHandshakePuzzle {
    const fn default_memory_kib() -> u32 {
        64 * 1024
    }

    const fn default_enabled() -> bool {
        true
    }

    const fn default_time_cost() -> u32 {
        2
    }

    const fn default_lanes() -> u32 {
        1
    }

    fn parse(self) -> Option<actual::SoranetPuzzle> {
        if !self.enabled {
            return None;
        }
        let memory = NonZeroU32::new(self.memory_kib.max(4_096)).unwrap();
        let time_cost = NonZeroU32::new(self.time_cost.max(1)).unwrap();
        let lanes = NonZeroU32::new(self.lanes.clamp(1, 16)).unwrap();
        Some(actual::SoranetPuzzle {
            memory_kib: memory,
            time_cost,
            lanes,
        })
    }
}

/// User-level configuration container for SoraNet privacy telemetry.
#[derive(Debug, Clone, Copy, ReadConfig)]
pub struct SoranetPrivacy {
    /// Duration of each telemetry bucket in seconds.
    #[config(default = "actual::SoranetPrivacy::DEFAULT_BUCKET_SECS")]
    pub bucket_secs: u64,
    /// Minimum contributing handshakes required before publishing a bucket.
    #[config(default = "actual::SoranetPrivacy::DEFAULT_MIN_HANDSHAKES")]
    pub min_handshakes: u64,
    /// Buckets to delay publication after the first contributor.
    #[config(default = "actual::SoranetPrivacy::DEFAULT_FLUSH_DELAY_BUCKETS")]
    pub flush_delay_buckets: u64,
    /// Buckets after which pending data is force-flushed.
    #[config(default = "actual::SoranetPrivacy::DEFAULT_FORCE_FLUSH_BUCKETS")]
    pub force_flush_buckets: u64,
    /// Maximum number of completed buckets retained in memory.
    #[config(default = "actual::SoranetPrivacy::DEFAULT_MAX_COMPLETED_BUCKETS")]
    pub max_completed_buckets: usize,
    /// Maximum bucket lag tolerated for collector shares before suppression.
    #[config(default = "actual::SoranetPrivacy::DEFAULT_MAX_SHARE_LAG_BUCKETS")]
    pub max_share_lag_buckets: u64,
    /// Expected collector shares per bucket.
    #[config(default = "actual::SoranetPrivacy::DEFAULT_EXPECTED_SHARES")]
    pub expected_shares: u16,
    /// Capacity of the privacy event buffer (events).
    #[config(default = "actual::SoranetPrivacy::DEFAULT_EVENT_BUFFER_CAPACITY")]
    pub event_buffer_capacity: usize,
}

impl Default for SoranetPrivacy {
    fn default() -> Self {
        Self {
            bucket_secs: actual::SoranetPrivacy::DEFAULT_BUCKET_SECS,
            min_handshakes: actual::SoranetPrivacy::DEFAULT_MIN_HANDSHAKES,
            flush_delay_buckets: actual::SoranetPrivacy::DEFAULT_FLUSH_DELAY_BUCKETS,
            force_flush_buckets: actual::SoranetPrivacy::DEFAULT_FORCE_FLUSH_BUCKETS,
            max_completed_buckets: actual::SoranetPrivacy::DEFAULT_MAX_COMPLETED_BUCKETS,
            max_share_lag_buckets: actual::SoranetPrivacy::DEFAULT_MAX_SHARE_LAG_BUCKETS,
            expected_shares: actual::SoranetPrivacy::DEFAULT_EXPECTED_SHARES,
            event_buffer_capacity: actual::SoranetPrivacy::DEFAULT_EVENT_BUFFER_CAPACITY,
        }
    }
}

impl SoranetPrivacy {
    fn parse(self) -> actual::SoranetPrivacy {
        let bucket_secs = if self.bucket_secs == 0 {
            actual::SoranetPrivacy::DEFAULT_BUCKET_SECS
        } else {
            self.bucket_secs
        };
        let min_handshakes = if self.min_handshakes == 0 {
            actual::SoranetPrivacy::DEFAULT_MIN_HANDSHAKES
        } else {
            self.min_handshakes
        };
        let flush_delay_buckets = if self.flush_delay_buckets == 0 {
            actual::SoranetPrivacy::DEFAULT_FLUSH_DELAY_BUCKETS
        } else {
            self.flush_delay_buckets
        };
        let force_flush_buckets = if self.force_flush_buckets == 0 {
            actual::SoranetPrivacy::DEFAULT_FORCE_FLUSH_BUCKETS
        } else {
            self.force_flush_buckets
        };
        let max_share_lag_buckets = if self.max_share_lag_buckets == 0 {
            actual::SoranetPrivacy::DEFAULT_MAX_SHARE_LAG_BUCKETS
        } else {
            self.max_share_lag_buckets
        };
        let max_completed_buckets = if self.max_completed_buckets == 0 {
            actual::SoranetPrivacy::DEFAULT_MAX_COMPLETED_BUCKETS
        } else {
            self.max_completed_buckets
        };
        let expected_shares = if self.expected_shares == 0 {
            actual::SoranetPrivacy::DEFAULT_EXPECTED_SHARES
        } else {
            self.expected_shares
        };
        let event_buffer_capacity = if self.event_buffer_capacity == 0 {
            actual::SoranetPrivacy::DEFAULT_EVENT_BUFFER_CAPACITY
        } else {
            self.event_buffer_capacity
        };

        actual::SoranetPrivacy {
            bucket_secs,
            min_handshakes,
            flush_delay_buckets,
            force_flush_buckets: force_flush_buckets.max(flush_delay_buckets),
            max_completed_buckets,
            max_share_lag_buckets,
            expected_shares,
            event_buffer_capacity,
        }
    }
}

/// User-level configuration container for `Network`.
#[derive(Debug, Clone, ReadConfig)]
pub struct SoranetVpn {
    /// Enable the native SoraNet VPN tunnel.
    #[config(default = "defaults::soranet::vpn::ENABLED")]
    pub enabled: bool,
    /// Fixed cell size (bytes).
    #[config(default = "defaults::soranet::vpn::CELL_SIZE_BYTES")]
    pub cell_size_bytes: u16,
    /// Flow label width (bits).
    #[config(default = "defaults::soranet::vpn::FLOW_LABEL_BITS")]
    pub flow_label_bits: u8,
    /// Cover-to-data ratio (permille).
    #[config(default = "defaults::soranet::vpn::COVER_TO_DATA_PER_MILLE")]
    pub cover_to_data_per_mille: u16,
    /// Maximum burst of cover cells.
    #[config(default = "defaults::soranet::vpn::MAX_COVER_BURST")]
    pub max_cover_burst: u16,
    /// Heartbeat cadence for keepalive cells (milliseconds).
    #[config(default = "defaults::soranet::vpn::HEARTBEAT_MS")]
    pub heartbeat_ms: u16,
    /// Maximum jitter applied to scheduled slots (milliseconds).
    #[config(default = "defaults::soranet::vpn::JITTER_MS")]
    pub jitter_ms: u16,
    /// Padding budget to advertise in cell headers (milliseconds).
    #[config(default = "defaults::soranet::vpn::PADDING_BUDGET_MS")]
    pub padding_budget_ms: u16,
    /// Guard/exit refresh cadence (seconds).
    #[config(default = "defaults::soranet::vpn::guard_refresh_secs_u64()")]
    pub guard_refresh_secs: u64,
    /// Control-plane lease duration (seconds).
    #[config(default = "defaults::soranet::vpn::lease_secs_u64()")]
    pub lease_secs: u64,
    /// DNS push interval (seconds).
    #[config(default = "defaults::soranet::vpn::dns_push_interval_secs_u64()")]
    pub dns_push_interval_secs: u64,
    /// Exit class label for billing/telemetry.
    #[config(default = "defaults::soranet::vpn::EXIT_CLASS.to_string()")]
    pub exit_class: String,
    /// Meter family identifier.
    #[config(default = "defaults::soranet::vpn::METER_FAMILY.to_string()")]
    pub meter_family: String,
}

impl Default for SoranetVpn {
    fn default() -> Self {
        Self {
            enabled: defaults::soranet::vpn::ENABLED,
            cell_size_bytes: defaults::soranet::vpn::CELL_SIZE_BYTES,
            flow_label_bits: defaults::soranet::vpn::FLOW_LABEL_BITS,
            cover_to_data_per_mille: defaults::soranet::vpn::COVER_TO_DATA_PER_MILLE,
            max_cover_burst: defaults::soranet::vpn::MAX_COVER_BURST,
            heartbeat_ms: defaults::soranet::vpn::HEARTBEAT_MS,
            jitter_ms: defaults::soranet::vpn::JITTER_MS,
            padding_budget_ms: defaults::soranet::vpn::PADDING_BUDGET_MS,
            guard_refresh_secs: defaults::soranet::vpn::guard_refresh_secs_u64(),
            lease_secs: defaults::soranet::vpn::lease_secs_u64(),
            dns_push_interval_secs: defaults::soranet::vpn::dns_push_interval_secs_u64(),
            exit_class: defaults::soranet::vpn::EXIT_CLASS.to_string(),
            meter_family: defaults::soranet::vpn::METER_FAMILY.to_string(),
        }
    }
}

impl SoranetVpn {
    fn parse(self) -> actual::SoranetVpn {
        let Self {
            enabled,
            cell_size_bytes,
            flow_label_bits,
            cover_to_data_per_mille,
            max_cover_burst,
            heartbeat_ms,
            jitter_ms,
            padding_budget_ms,
            guard_refresh_secs,
            lease_secs,
            dns_push_interval_secs,
            exit_class,
            meter_family,
        } = self;

        let default_cell_size = defaults::soranet::vpn::CELL_SIZE_BYTES;
        let cell_size_bytes = match cell_size_bytes {
            0 => default_cell_size,
            value if value == default_cell_size => value,
            _ => panic!("network.soranet_vpn.cell_size_bytes must equal {default_cell_size}"),
        };
        VpnFlowLabelV1::max_value_for_bits(flow_label_bits)
            .expect("flow_label_bits must be between 1 and 24");
        let cover_to_data_per_mille = cover_to_data_per_mille.min(1_000);
        let max_cover_burst = max_cover_burst.max(1);
        let heartbeat_ms = heartbeat_ms.max(1);
        let padding_budget_ms = padding_budget_ms.max(1);
        let jitter_ms = jitter_ms.min(heartbeat_ms);
        let guard_refresh = Duration::from_secs(guard_refresh_secs.max(1));
        let lease_secs =
            u32::try_from(lease_secs.max(1)).expect("network.soranet_vpn.lease_secs exceeds u32");
        let lease = Duration::from_secs(u64::from(lease_secs));
        let dns_push_interval = Duration::from_secs(dns_push_interval_secs.max(30));
        let exit_class = VpnExitClassV1::try_from_label(&exit_class)
            .expect("network.soranet_vpn.exit_class must be standard|low-latency|high-security")
            .as_label()
            .to_string();

        actual::SoranetVpn {
            enabled,
            cell_size_bytes,
            flow_label_bits,
            cover_to_data_per_mille,
            max_cover_burst,
            heartbeat_ms,
            jitter_ms,
            padding_budget_ms,
            guard_refresh,
            lease,
            dns_push_interval,
            exit_class,
            meter_family,
        }
    }
}

#[cfg(test)]
mod soranet_vpn_tests {
    use super::*;

    #[test]
    #[should_panic(expected = "flow_label_bits must be between 1 and 24")]
    fn soranet_vpn_rejects_invalid_flow_label_bits() {
        let cfg = SoranetVpn {
            flow_label_bits: 0,
            ..SoranetVpn::default()
        };
        let _ = cfg.parse();
    }

    #[test]
    #[should_panic(expected = "network.soranet_vpn.cell_size_bytes must equal")]
    fn soranet_vpn_rejects_mismatched_cell_size_bytes() {
        let cfg = SoranetVpn {
            cell_size_bytes: defaults::soranet::vpn::CELL_SIZE_BYTES.saturating_add(1),
            ..SoranetVpn::default()
        };
        let _ = cfg.parse();
    }

    #[test]
    fn soranet_vpn_allows_zero_cover_ratio() {
        let cfg = SoranetVpn {
            cover_to_data_per_mille: 0,
            ..SoranetVpn::default()
        };
        let parsed = cfg.parse();
        assert_eq!(0, parsed.cover_to_data_per_mille);
    }

    #[test]
    fn soranet_vpn_accepts_maximum_lease_secs() {
        let cfg = SoranetVpn {
            lease_secs: u64::from(u32::MAX),
            exit_class: "HIGH_SECURITY".to_string(),
            ..SoranetVpn::default()
        };
        let parsed = cfg.parse();
        assert_eq!(u64::from(u32::MAX), parsed.lease.as_secs());
        assert_eq!("high-security", parsed.exit_class);
    }

    #[test]
    #[should_panic(expected = "lease_secs exceeds u32")]
    fn soranet_vpn_rejects_overflowing_lease_secs() {
        let cfg = SoranetVpn {
            lease_secs: u64::from(u32::MAX) + 1,
            ..SoranetVpn::default()
        };
        let _ = cfg.parse();
    }

    #[test]
    #[should_panic(expected = "exit_class must be standard|low-latency|high-security")]
    fn soranet_vpn_rejects_unknown_exit_class() {
        let cfg = SoranetVpn {
            exit_class: "ultra".to_string(),
            ..SoranetVpn::default()
        };
        let _ = cfg.parse();
    }
}

/// User-level configuration container for `Network`.
#[derive(Debug, Clone, ReadConfig)]
#[allow(clippy::struct_excessive_bools)]
pub struct Network {
    /// Peer-to-peer address (internal, will be used only to bind to it).
    #[config(env = "P2P_ADDRESS")]
    pub address: WithOrigin<SocketAddr>,
    /// Peer-to-peer address (external, as seen by other peers).
    /// Will be gossiped to connected peers so that they can gossip it to other peers.
    #[config(env = "P2P_PUBLIC_ADDRESS")]
    pub public_address: WithOrigin<SocketAddr>,
    /// P2P relay role (disabled/hub/spoke/assist) for constrained topologies.
    #[config(default = "RelayMode::Disabled")]
    pub relay_mode: RelayMode,
    /// Relay hub addresses to dial when in `spoke` or `assist` mode (priority order).
    ///
    /// When set, values are attempted in order and the node may fall back to
    /// subsequent entries if the preferred hub is unreachable.
    #[config(default)]
    pub relay_hub_addresses: Vec<WithOrigin<SocketAddr>>,
    /// Hop limit for relayed frames.
    #[config(default = "defaults::network::RELAY_TTL")]
    pub relay_ttl: u8,
    /// SoraNet handshake capabilities advertised by this node.
    #[config(nested)]
    pub soranet_handshake: SoranetHandshake,
    /// Privacy telemetry configuration that relays advertise to collectors.
    #[config(nested)]
    pub soranet_privacy: SoranetPrivacy,
    /// VPN control-plane and scheduler configuration.
    #[config(nested)]
    pub soranet_vpn: SoranetVpn,
    /// Lane profile preset applied to derive networking caps (`core` or `home`).
    #[config(default = "defaults::network::lane_profile::default_label()")]
    pub lane_profile: String,
    /// Require peers to advertise matching SM helper availability during handshake.
    #[config(default = "defaults::network::REQUIRE_SM_HANDSHAKE_MATCH")]
    pub require_sm_handshake_match: bool,
    /// Require peers to match the OpenSSL preview toggle during handshake.
    #[config(default = "defaults::network::REQUIRE_SM_OPENSSL_PREVIEW_MATCH")]
    pub require_sm_openssl_preview_match: bool,
    /// Fanout cap for block-sync gossip (peer samples, block sync updates, availability votes, and NEW_VIEW gossip).
    #[config(default = "defaults::network::BLOCK_GOSSIP_SIZE")]
    pub block_gossip_size: NonZeroU32,
    /// Interval between block gossip batches in milliseconds (clamped to >= 100ms).
    #[config(default = "defaults::network::BLOCK_GOSSIP_PERIOD.into()")]
    pub block_gossip_period_ms: DurationMs,
    /// Maximum interval between block gossip batches in milliseconds (clamped to >= block_gossip_period_ms).
    #[config(default = "defaults::network::BLOCK_GOSSIP_MAX_PERIOD.into()")]
    pub block_gossip_max_period_ms: DurationMs,
    /// Interval between peer gossip batches in milliseconds (clamped to >= 100ms).
    #[config(default = "defaults::network::PEER_GOSSIP_PERIOD.into()")]
    pub peer_gossip_period_ms: DurationMs,
    /// Maximum interval between peer gossip batches in milliseconds (clamped to >= peer_gossip_period_ms).
    #[config(default = "defaults::network::PEER_GOSSIP_MAX_PERIOD.into()")]
    pub peer_gossip_max_period_ms: DurationMs,
    /// Advertise and accept signed trust gossip frames.
    #[config(default = "defaults::network::TRUST_GOSSIP")]
    pub trust_gossip: bool,
    /// Half-life for peer trust decay toward zero (milliseconds).
    #[config(default = "defaults::network::TRUST_DECAY_HALF_LIFE.into()")]
    pub trust_decay_half_life_ms: DurationMs,
    /// Penalty applied for invalid/bad trust gossip.
    #[config(default = "defaults::network::TRUST_PENALTY_BAD_GOSSIP")]
    pub trust_penalty_bad_gossip: i32,
    /// Penalty applied when gossip mentions peers outside the current topology.
    #[config(default = "defaults::network::TRUST_PENALTY_UNKNOWN_PEER")]
    pub trust_penalty_unknown_peer: i32,
    /// Minimum trust score before gossip is ignored.
    #[config(default = "defaults::network::TRUST_MIN_SCORE")]
    pub trust_min_score: i32,
    /// Maximum number of transactions gossiped per batch.
    #[config(default = "defaults::network::TRANSACTION_GOSSIP_SIZE")]
    pub transaction_gossip_size: NonZeroU32,
    /// Interval between transaction gossip batches in milliseconds (clamped to >= 100ms).
    #[config(default = "defaults::network::TRANSACTION_GOSSIP_PERIOD.into()")]
    pub transaction_gossip_period_ms: DurationMs,
    /// Number of gossip periods to wait before re-sending the same transactions.
    #[config(default = "defaults::network::TRANSACTION_GOSSIP_RESEND_TICKS")]
    pub transaction_gossip_resend_ticks: NonZeroU32,
    /// Drop transaction gossip for dataspaces missing from the lane catalog instead of falling
    /// back to restricted routing.
    #[config(default = "defaults::network::TX_GOSSIP_DROP_UNKNOWN_DATASPACE")]
    pub transaction_gossip_drop_unknown_dataspace: bool,
    /// Optional cap on the number of peers targeted for restricted transaction gossip (None = all
    /// peers in the commit topology).
    pub transaction_gossip_restricted_target_cap: Option<NonZeroUsize>,
    /// Optional cap on the number of peers targeted for public transaction gossip (None = broadcast).
    pub transaction_gossip_public_target_cap: Option<NonZeroUsize>,
    /// Interval between public transaction gossip target reshuffles in milliseconds (clamped to >= 100ms).
    #[config(default = "defaults::network::TX_GOSSIP_PUBLIC_TARGET_RESHUFFLE.into()")]
    pub transaction_gossip_public_target_reshuffle_ms: DurationMs,
    /// Interval between restricted transaction gossip target reshuffles in milliseconds (clamped to >= 100ms).
    #[config(default = "defaults::network::TX_GOSSIP_RESTRICTED_TARGET_RESHUFFLE.into()")]
    pub transaction_gossip_restricted_target_reshuffle_ms: DurationMs,
    /// Fallback behaviour when restricted gossip has no available targets (`drop` or `public_overlay`).
    #[config(default = "defaults::network::TX_GOSSIP_RESTRICTED_FALLBACK.to_string()")]
    pub transaction_gossip_restricted_fallback: String,
    /// Policy for restricted payloads when only the public overlay is available (`refuse` or `forward`).
    #[config(default = "defaults::network::TX_GOSSIP_RESTRICTED_PUBLIC_PAYLOAD.to_string()")]
    pub transaction_gossip_restricted_public_payload: String,
    /// Duration of time after which connection with peer is terminated if peer is idle
    /// (clamped to >= 100ms).
    #[config(default = "defaults::network::IDLE_TIMEOUT.into()")]
    pub idle_timeout_ms: DurationMs,
    /// Delay outbound peer dials after startup (milliseconds).
    #[config(default = "defaults::network::CONNECT_STARTUP_DELAY.into()")]
    pub connect_startup_delay_ms: DurationMs,
    /// Timeout applied to an individual outbound dial attempt (milliseconds, clamped to >= 100ms).
    #[config(default = "defaults::network::DIAL_TIMEOUT.into()")]
    pub dial_timeout_ms: DurationMs,
    /// Maximum age for deferred outbound frames while the peer session is missing (milliseconds).
    #[config(
        default = "DurationMs(std::time::Duration::from_millis(defaults::network::DEFERRED_SEND_TTL_MS))"
    )]
    pub deferred_send_ttl_ms: DurationMs,
    /// Maximum deferred outbound frames retained per peer while session is missing.
    #[config(default = "defaults::network::DEFERRED_SEND_MAX_PER_PEER")]
    pub deferred_send_max_per_peer: usize,
    /// Enable QUIC transport (feature-gated).
    #[config(env = "P2P_QUIC", default)]
    pub quic_enabled: bool,
    /// Enable QUIC DATAGRAM support for best-effort topics (feature-gated by QUIC).
    ///
    /// When enabled and QUIC is negotiated, small best-effort frames (gossip/health)
    /// may be sent over QUIC datagrams instead of streams to avoid retransmission and
    /// head-of-line blocking.
    #[config(default = "defaults::network::QUIC_DATAGRAMS_ENABLED")]
    pub quic_datagrams_enabled: bool,
    /// Upper bound (bytes) for QUIC datagram payloads.
    ///
    /// Applied as a conservative cap even if the QUIC path MTU supports larger datagrams.
    #[config(default = "defaults::network::QUIC_DATAGRAM_MAX_PAYLOAD_BYTES")]
    pub quic_datagram_max_payload_bytes: NonZeroUsize,
    /// Total receive buffer reserved for QUIC datagrams (bytes).
    #[config(default = "defaults::network::QUIC_DATAGRAM_RECEIVE_BUFFER_BYTES")]
    pub quic_datagram_receive_buffer_bytes: NonZeroUsize,
    /// Total send buffer reserved for QUIC datagrams (bytes).
    #[config(default = "defaults::network::QUIC_DATAGRAM_SEND_BUFFER_BYTES")]
    pub quic_datagram_send_buffer_bytes: NonZeroUsize,
    /// Enable SCION-guided outbound dialing when peer routes are configured.
    #[config(default = "defaults::network::SCION_ENABLED")]
    pub scion_enabled: bool,
    /// Allow fallback to legacy dialing when SCION route dialing is unavailable or fails.
    #[config(default = "defaults::network::SCION_FALLBACK_TO_LEGACY")]
    pub scion_fallback_to_legacy: bool,
    /// Optional SCION listener endpoint hint (reserved for future inbound support).
    pub scion_listen_endpoint: Option<String>,
    /// Optional per-peer SCION route map (`peer_id/public_key` -> `host:port`).
    #[config(default)]
    pub scion_routes: BTreeMap<String, String>,
    /// Optional outbound proxy URL for TCP-based dials (e.g., `http://user:pass@host:port`,
    /// `https://host:port`, `socks5://user:pass@host:port`, or `socks5h://host:port`).
    ///
    /// Note: `https://` proxies require a build with `iroha_p2p/p2p_tls` to wrap the proxy hop in TLS.
    pub p2p_proxy: Option<String>,
    /// Require that outbound TCP-based dials use `p2p_proxy`.
    ///
    /// This is a safety knob for constrained/censored environments where direct peer dials should
    /// be avoided to reduce IP leakage.
    ///
    /// Note: QUIC bypasses the proxy; set `quic_enabled=false` when `p2p_proxy_required=true`.
    #[config(default)]
    pub p2p_proxy_required: bool,
    /// Proxy bypass list (suffix match, similar to `NO_PROXY` semantics).
    ///
    /// Note: this list must be empty when `p2p_proxy_required=true`.
    #[config(default)]
    pub p2p_no_proxy: Vec<String>,
    /// Verify an `https://` proxy hop (default: true).
    ///
    /// When enabled, the proxy connection uses certificate pinning via
    /// `p2p_proxy_tls_pinned_cert_der_base64`. If no pin is configured, dialing an `https://`
    /// proxy fails. When disabled, the proxy hop is encrypted but susceptible to MITM (which can
    /// leak proxy credentials).
    #[config(default = "true")]
    pub p2p_proxy_tls_verify: bool,
    /// Optional pinned end-entity certificate for `https://` proxies (DER, base64).
    ///
    /// When `p2p_proxy_tls_verify` is enabled, the dialer pins the proxy certificate to this value.
    pub p2p_proxy_tls_pinned_cert_der_base64: Option<String>,
    /// Enable TLS-over-TCP transport (feature-gated).
    /// When enabled and built with the `iroha_p2p/p2p_tls` feature, the dialer
    /// wraps the TCP stream in TLS 1.3. Peer identity remains authenticated by
    /// the signed application handshake.
    #[config(env = "P2P_TLS", default)]
    pub tls_enabled: bool,
    /// When TLS-over-TCP is enabled, fall back to plain TCP if the TLS dial fails.
    ///
    /// Set to `false` to enforce TLS-only outbound P2P dials when `tls_enabled=true`.
    #[config(default = "true")]
    pub tls_fallback_to_plain: bool,
    /// Optional P2P TLS listener address (host:port). If set and TLS is enabled,
    /// node will accept inbound TLS connections on this address.
    /// Plain TCP listener remains active on `address` unless `tls_inbound_only=true`.
    #[config(env = "P2P_TLS_LISTEN_ADDRESS")]
    pub tls_listen_address: Option<WithOrigin<SocketAddr>>,
    /// Disable the plain TCP listener and accept inbound P2P connections only via TLS-over-TCP.
    ///
    /// Requires `tls_enabled=true` and a build with the `iroha_p2p/p2p_tls` feature.
    ///
    /// When enabled, the node binds a TLS listener on `tls_listen_address` when set, otherwise on
    /// `address`.
    #[config(default)]
    pub tls_inbound_only: bool,
    /// Optional interval to refresh DNS hostnames (when `public_address` is a hostname).
    /// Disabled if not set. Useful to catch IP changes faster.
    pub dns_refresh_interval_ms: Option<DurationMs>,
    /// Optional TTL-based refresh: when set, hostname-based peers are selectively refreshed
    /// after this TTL elapses since last refresh. If both interval and TTL are set, interval
    /// takes precedence.
    pub dns_refresh_ttl_ms: Option<DurationMs>,
    /// Prefer WebSocket fallback for outbound P2P connections (feature `p2p_ws`).
    /// If enabled, the dialer will attempt WSS/WS to Torii `/p2p` before other transports
    /// for Host addresses. This is primarily for constrained envs and tests.
    #[config(default)]
    pub prefer_ws_fallback: bool,
    /// Capacity for the high-priority network message queue and inbound peer dispatch buffer
    /// (bounded mode only).
    #[config(default = "defaults::network::P2P_QUEUE_CAP_HIGH")]
    pub p2p_queue_cap_high: NonZeroUsize,
    /// Capacity for the low-priority network message queue and inbound peer dispatch buffer
    /// (bounded mode only).
    #[config(default = "defaults::network::P2P_QUEUE_CAP_LOW")]
    pub p2p_queue_cap_low: NonZeroUsize,
    /// Capacity for the per-peer post queue (bounded mode only).
    #[config(default = "defaults::network::P2P_POST_QUEUE_CAP")]
    pub p2p_post_queue_cap: NonZeroUsize,
    /// Capacity for the inbound P2P subscriber queue feeding the node relay.
    #[config(default = "defaults::network::P2P_SUBSCRIBER_QUEUE_CAP")]
    pub p2p_subscriber_queue_cap: NonZeroUsize,
    /// Per-peer consensus ingress rate limit (msgs/sec). If unset, defaults apply.
    pub consensus_ingress_rate_per_sec: Option<NonZeroU32>,
    /// Per-peer consensus ingress burst (msgs). If unset, defaults apply.
    pub consensus_ingress_burst: Option<NonZeroU32>,
    /// Per-peer consensus ingress bytes/sec limit. If unset, defaults apply.
    pub consensus_ingress_bytes_per_sec: Option<NonZeroU32>,
    /// Per-peer consensus ingress bytes burst. If unset, defaults apply.
    pub consensus_ingress_bytes_burst: Option<NonZeroU32>,
    /// Per-peer critical consensus ingress rate limit (msgs/sec). If unset, defaults apply.
    pub consensus_ingress_critical_rate_per_sec: Option<NonZeroU32>,
    /// Per-peer critical consensus ingress burst (msgs). If unset, defaults apply.
    pub consensus_ingress_critical_burst: Option<NonZeroU32>,
    /// Per-peer critical consensus ingress bytes/sec limit. If unset, defaults apply.
    pub consensus_ingress_critical_bytes_per_sec: Option<NonZeroU32>,
    /// Per-peer critical consensus ingress bytes burst. If unset, defaults apply.
    pub consensus_ingress_critical_bytes_burst: Option<NonZeroU32>,
    /// Maximum concurrent RBC sessions accepted per peer before throttling (0 disables).
    #[config(default = "defaults::network::CONSENSUS_INGRESS_RBC_SESSION_LIMIT")]
    pub consensus_ingress_rbc_session_limit: usize,
    /// Drop threshold (per window) before temporarily suppressing consensus ingress (0 disables).
    #[config(default = "defaults::network::CONSENSUS_INGRESS_PENALTY_THRESHOLD")]
    pub consensus_ingress_penalty_threshold: u32,
    /// Window size (ms) for consensus ingress penalty tracking.
    #[config(default = "defaults::network::CONSENSUS_INGRESS_PENALTY_WINDOW_MS")]
    pub consensus_ingress_penalty_window_ms: u64,
    /// Cooldown (ms) applied after consensus ingress penalties trigger.
    #[config(default = "defaults::network::CONSENSUS_INGRESS_PENALTY_COOLDOWN_MS")]
    pub consensus_ingress_penalty_cooldown_ms: u64,
    /// Stagger between parallel address dial attempts (Happy Eyeballs)
    #[config(default = "defaults::network::HAPPY_EYEBALLS_STAGGER.into()")]
    pub happy_eyeballs_stagger_ms: DurationMs,
    /// Prefer IPv6 addresses first when multiple are available
    #[config(default)]
    pub addr_ipv6_first: bool,
    /// Maximum number of simultaneously accepted incoming connections.
    /// If unset, no explicit cap is enforced.
    pub max_incoming: Option<NonZeroUsize>,
    /// Maximum total number of connections (incoming + outgoing + in-flight accepts).
    /// If unset, no explicit cap is enforced.
    pub max_total_connections: Option<NonZeroUsize>,
    /// Per-IP(/24 for IPv4, /64 for IPv6) accept rate, in accepts per second.
    /// If unset, per-IP accept throttling is disabled.
    pub accept_rate_per_ip_per_sec: Option<NonZeroU32>,
    /// Token bucket burst for per-IP accept limiter. If unset, defaults internally.
    pub accept_burst_per_ip: Option<NonZeroU32>,
    /// Maximum number of accept throttle buckets retained (prefix + per-IP).
    #[config(default = "defaults::network::MAX_ACCEPT_BUCKETS")]
    pub max_accept_buckets: NonZeroUsize,
    /// Idle timeout for accept throttle buckets (milliseconds).
    #[config(default = "defaults::network::ACCEPT_BUCKET_IDLE.into()")]
    pub accept_bucket_idle_ms: DurationMs,
    /// Prefix length applied to IPv4 accept prefix buckets.
    #[config(default = "defaults::network::ACCEPT_PREFIX_V4_BITS")]
    pub accept_prefix_v4_bits: u8,
    /// Prefix length applied to IPv6 accept prefix buckets.
    #[config(default = "defaults::network::ACCEPT_PREFIX_V6_BITS")]
    pub accept_prefix_v6_bits: u8,
    /// Optional prefix-level accept rate (accepts per second) applied before per-IP buckets.
    pub accept_rate_per_prefix_per_sec: Option<NonZeroU32>,
    /// Optional burst for prefix-level accept limiter.
    pub accept_burst_per_prefix: Option<NonZeroU32>,
    /// Low-priority (gossip/sync) per-peer rate limit (msgs/sec). If unset, no rate limiting.
    pub low_priority_rate_per_sec: Option<NonZeroU32>,
    /// Low-priority token-bucket burst (msgs). If unset, defaults to `rate`.
    pub low_priority_burst: Option<NonZeroU32>,
    /// Low-priority per-peer bytes/sec budget. If unset, no byte rate limiting.
    pub low_priority_bytes_per_sec: Option<NonZeroU32>,
    /// Low-priority per-peer bytes burst.
    pub low_priority_bytes_burst: Option<NonZeroU32>,
    /// Only allow peers explicitly listed in `allow_keys` and addresses in `allow_cidrs`.
    #[config(default)]
    pub allowlist_only: bool,
    /// Allowlist of peer public keys.
    #[config(default)]
    pub allow_keys: Vec<iroha_crypto::PublicKey>,
    /// Denylist of peer public keys.
    #[config(default)]
    pub deny_keys: Vec<iroha_crypto::PublicKey>,
    /// CIDR allowlist (IPv4/IPv6) entries, e.g., "192.168.1.0/24", "`2001:db8::/32`".
    #[config(default)]
    pub allow_cidrs: Vec<String>,
    /// CIDR denylist entries.
    #[config(default)]
    pub deny_cidrs: Vec<String>,
    /// Disconnect on per-peer post overflow (bounded per-topic channels).
    #[config(
        env = "P2P_DISCONNECT_ON_POST_OVERFLOW",
        default = "defaults::network::DISCONNECT_ON_POST_OVERFLOW"
    )]
    pub disconnect_on_post_overflow: bool,
    /// Maximum allowed frame size (bytes) for P2P messages
    #[config(
        env = "P2P_MAX_FRAME_BYTES",
        default = "defaults::network::MAX_FRAME_BYTES"
    )]
    pub max_frame_bytes: NonZeroUsize,
    /// `TCP_NODELAY` setting for TCP sockets
    #[config(env = "P2P_TCP_NODELAY", default = "defaults::network::TCP_NODELAY")]
    pub tcp_nodelay: bool,
    /// TCP keepalive time (ms), None disables
    pub tcp_keepalive_ms: Option<DurationMs>,
    /// Per-topic maximum frame sizes (bytes). If unset, sensible defaults are used.
    #[config(default = "defaults::network::MAX_FRAME_BYTES_CONSENSUS")]
    pub max_frame_bytes_consensus: NonZeroUsize,
    /// Maximum frame size for control channel traffic.
    #[config(default = "defaults::network::MAX_FRAME_BYTES_CONTROL")]
    pub max_frame_bytes_control: NonZeroUsize,
    /// Maximum frame size for block synchronization traffic.
    #[config(default = "defaults::network::MAX_FRAME_BYTES_BLOCK_SYNC")]
    pub max_frame_bytes_block_sync: NonZeroUsize,
    /// Maximum frame size for transaction gossip traffic.
    #[config(default = "defaults::network::MAX_FRAME_BYTES_TX_GOSSIP")]
    pub max_frame_bytes_tx_gossip: NonZeroUsize,
    /// Maximum frame size for peer gossip traffic.
    #[config(default = "defaults::network::MAX_FRAME_BYTES_PEER_GOSSIP")]
    pub max_frame_bytes_peer_gossip: NonZeroUsize,
    /// Maximum frame size for health monitoring traffic.
    #[config(default = "defaults::network::MAX_FRAME_BYTES_HEALTH")]
    pub max_frame_bytes_health: NonZeroUsize,
    /// Maximum frame size for miscellaneous topics.
    #[config(default = "defaults::network::MAX_FRAME_BYTES_OTHER")]
    pub max_frame_bytes_other: NonZeroUsize,
    /// TLS policy: restrict to TLS 1.3 only (default: true).
    #[config(default)]
    pub tls_only_v1_3: bool,
    /// QUIC max idle timeout (ms); if unset, quic idle timeout default applies.
    pub quic_max_idle_timeout_ms: Option<DurationMs>,
}

impl Network {
    fn parse_scion_routes(routes: BTreeMap<String, String>) -> BTreeMap<PeerId, SocketAddr> {
        routes
            .into_iter()
            .map(|(raw_peer_id, raw_route)| {
                let peer_label = raw_peer_id.trim();
                let route_label = raw_route.trim();
                let peer_id = peer_label.parse::<PeerId>().unwrap_or_else(|err| {
                    panic!(
                        "network.scion_routes key `{peer_label}` must be a peer public key: {err}"
                    )
                });
                let route = route_label.parse::<SocketAddr>().unwrap_or_else(|err| {
                    panic!(
                        "network.scion_routes[{peer_label}] value `{route_label}` must be a socket address: {err}"
                    )
                });
                (peer_id, route)
            })
            .collect()
    }

    #[allow(clippy::too_many_lines)]
    fn parse(
        self,
    ) -> (
        actual::Network,
        actual::BlockSync,
        actual::TransactionGossiper,
    ) {
        let Self {
            address,
            public_address,
            relay_mode,
            relay_hub_addresses: user_relay_hub_addresses,
            relay_ttl,
            soranet_handshake,
            soranet_privacy: user_soranet_privacy,
            soranet_vpn,
            lane_profile,
            require_sm_handshake_match,
            require_sm_openssl_preview_match,
            block_gossip_size,
            block_gossip_period_ms: block_gossip_period,
            block_gossip_max_period_ms: block_gossip_max_period,
            peer_gossip_period_ms: peer_gossip_period,
            peer_gossip_max_period_ms: peer_gossip_max_period,
            trust_gossip,
            trust_decay_half_life_ms,
            trust_penalty_bad_gossip,
            trust_penalty_unknown_peer,
            trust_min_score,
            transaction_gossip_size,
            transaction_gossip_period_ms: transaction_gossip_period,
            transaction_gossip_resend_ticks,
            transaction_gossip_drop_unknown_dataspace,
            transaction_gossip_restricted_target_cap,
            transaction_gossip_public_target_cap,
            transaction_gossip_public_target_reshuffle_ms,
            transaction_gossip_restricted_target_reshuffle_ms,
            transaction_gossip_restricted_fallback,
            transaction_gossip_restricted_public_payload,
            idle_timeout_ms: idle_timeout,
            connect_startup_delay_ms: connect_startup_delay,
            dial_timeout_ms: dial_timeout,
            deferred_send_ttl_ms,
            deferred_send_max_per_peer,
            dns_refresh_interval_ms: dns_refresh_interval,
            dns_refresh_ttl_ms: dns_refresh_ttl,
            quic_enabled,
            quic_datagrams_enabled,
            quic_datagram_max_payload_bytes,
            quic_datagram_receive_buffer_bytes,
            quic_datagram_send_buffer_bytes,
            scion_enabled,
            scion_fallback_to_legacy,
            scion_listen_endpoint,
            scion_routes,
            p2p_proxy,
            p2p_proxy_required,
            p2p_no_proxy,
            p2p_proxy_tls_verify,
            p2p_proxy_tls_pinned_cert_der_base64,
            tls_enabled,
            tls_fallback_to_plain,
            tls_listen_address,
            tls_inbound_only,
            p2p_queue_cap_high,
            p2p_queue_cap_low,
            p2p_post_queue_cap,
            p2p_subscriber_queue_cap,
            consensus_ingress_rate_per_sec,
            consensus_ingress_burst,
            consensus_ingress_bytes_per_sec,
            consensus_ingress_bytes_burst,
            consensus_ingress_critical_rate_per_sec,
            consensus_ingress_critical_burst,
            consensus_ingress_critical_bytes_per_sec,
            consensus_ingress_critical_bytes_burst,
            consensus_ingress_rbc_session_limit,
            consensus_ingress_penalty_threshold,
            consensus_ingress_penalty_window_ms,
            consensus_ingress_penalty_cooldown_ms,
            happy_eyeballs_stagger_ms,
            addr_ipv6_first,
            max_incoming,
            max_total_connections,
            accept_rate_per_ip_per_sec,
            accept_burst_per_ip,
            max_accept_buckets,
            accept_bucket_idle_ms,
            accept_prefix_v4_bits,
            accept_prefix_v6_bits,
            accept_rate_per_prefix_per_sec,
            accept_burst_per_prefix,
            low_priority_rate_per_sec,
            low_priority_burst,
            low_priority_bytes_per_sec,
            low_priority_bytes_burst,
            allowlist_only,
            allow_keys,
            deny_keys,
            allow_cidrs,
            deny_cidrs,
            prefer_ws_fallback,
            disconnect_on_post_overflow,
            max_frame_bytes,
            tcp_nodelay,
            tcp_keepalive_ms,
            max_frame_bytes_consensus,
            max_frame_bytes_control,
            max_frame_bytes_block_sync,
            max_frame_bytes_tx_gossip,
            max_frame_bytes_peer_gossip,
            max_frame_bytes_health,
            max_frame_bytes_other,
            tls_only_v1_3,
            quic_max_idle_timeout_ms,
            ..
        } = self;

        let mut relay_hub_addresses: Vec<SocketAddr> = user_relay_hub_addresses
            .into_iter()
            .map(WithOrigin::into_value)
            .collect();
        // Deduplicate while preserving operator-supplied order.
        if relay_hub_addresses.len() > 1 {
            use std::collections::HashSet;
            let mut seen: HashSet<String> = HashSet::new();
            relay_hub_addresses.retain(|addr| seen.insert(addr.to_string()));
        }

        if matches!(relay_mode, RelayMode::Spoke | RelayMode::Assist)
            && relay_hub_addresses.is_empty()
        {
            panic!(
                "network.relay_hub_addresses must be set when network.relay_mode is spoke or assist"
            );
        }

        let transaction_gossip_restricted_target_cap = transaction_gossip_restricted_target_cap
            .or(defaults::network::TX_GOSSIP_RESTRICTED_TARGET_CAP);
        let transaction_gossip_public_target_cap =
            transaction_gossip_public_target_cap.or(defaults::network::TX_GOSSIP_PUBLIC_TARGET_CAP);
        let consensus_ingress_rate_per_sec =
            consensus_ingress_rate_per_sec.or(defaults::network::CONSENSUS_INGRESS_RATE_PER_SEC);
        let consensus_ingress_burst = if consensus_ingress_rate_per_sec.is_some() {
            consensus_ingress_burst.or(consensus_ingress_rate_per_sec)
        } else {
            None
        };
        let consensus_ingress_bytes_per_sec =
            consensus_ingress_bytes_per_sec.or(defaults::network::CONSENSUS_INGRESS_BYTES_PER_SEC);
        let consensus_ingress_bytes_burst = if consensus_ingress_bytes_per_sec.is_some() {
            consensus_ingress_bytes_burst.or(consensus_ingress_bytes_per_sec)
        } else {
            None
        };
        let consensus_ingress_critical_rate_per_sec = consensus_ingress_critical_rate_per_sec
            .or(defaults::network::CONSENSUS_INGRESS_CRITICAL_RATE_PER_SEC);
        let consensus_ingress_critical_burst = if consensus_ingress_critical_rate_per_sec.is_some()
        {
            consensus_ingress_critical_burst.or(consensus_ingress_critical_rate_per_sec)
        } else {
            None
        };
        let consensus_ingress_critical_bytes_per_sec = consensus_ingress_critical_bytes_per_sec
            .or(defaults::network::CONSENSUS_INGRESS_CRITICAL_BYTES_PER_SEC);
        let consensus_ingress_critical_bytes_burst =
            if consensus_ingress_critical_bytes_per_sec.is_some() {
                consensus_ingress_critical_bytes_burst.or(consensus_ingress_critical_bytes_per_sec)
            } else {
                None
            };

        let soranet_handshake = soranet_handshake.parse();
        let soranet_privacy = user_soranet_privacy.parse();
        let soranet_vpn = soranet_vpn.parse();
        let scion_listen_endpoint = scion_listen_endpoint.and_then(|endpoint| {
            let trimmed = endpoint.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_owned())
            }
        });
        let scion_routes = Self::parse_scion_routes(scion_routes);
        let lane_profile = actual::LaneProfile::from_label(&lane_profile);
        let limits = lane_profile.derived_limits();
        let max_incoming = max_incoming.or(limits.max_incoming);
        let max_total_connections = max_total_connections.or(limits.max_total_connections);
        let low_priority_rate_per_sec =
            low_priority_rate_per_sec.or(limits.low_priority_rate_per_sec);
        let low_priority_bytes_per_sec =
            low_priority_bytes_per_sec.or(limits.low_priority_bytes_per_sec);
        let restricted_fallback = match transaction_gossip_restricted_fallback
            .to_ascii_lowercase()
            .as_str()
        {
            "drop" => actual::DataspaceGossipFallback::Drop,
            "public_overlay" => actual::DataspaceGossipFallback::UsePublicOverlay,
            other => panic!(
                "transaction_gossip_restricted_fallback must be drop|public_overlay, got {other}"
            ),
        };
        let _restricted_public_payload = match transaction_gossip_restricted_public_payload
            .to_ascii_lowercase()
            .as_str()
        {
            "refuse" => actual::RestrictedPublicPayload::Refuse,
            "forward" => actual::RestrictedPublicPayload::Forward,
            other => panic!(
                "transaction_gossip_restricted_public_payload must be refuse|forward, got {other}"
            ),
        };
        let restricted_public_payload = match transaction_gossip_restricted_public_payload
            .to_ascii_lowercase()
            .as_str()
        {
            "refuse" => actual::RestrictedPublicPayload::Refuse,
            "forward" => actual::RestrictedPublicPayload::Forward,
            other => panic!(
                "transaction_gossip_restricted_public_payload must be refuse|forward, got {other}"
            ),
        };
        let min_interval = MIN_TIMER_INTERVAL;
        let idle_timeout = idle_timeout.get().max(min_interval);
        let dial_timeout = dial_timeout.get().max(min_interval);
        let peer_gossip_period = peer_gossip_period.get().max(min_interval);
        let peer_gossip_max_period = peer_gossip_max_period.get().max(peer_gossip_period);
        let block_gossip_period = block_gossip_period.get().max(min_interval);
        let block_gossip_max_period = block_gossip_max_period.get().max(block_gossip_period);
        let transaction_gossip_period = transaction_gossip_period.get().max(min_interval);
        let transaction_gossip_public_target_reshuffle =
            transaction_gossip_public_target_reshuffle_ms
                .get()
                .max(min_interval);
        let transaction_gossip_restricted_target_reshuffle =
            transaction_gossip_restricted_target_reshuffle_ms
                .get()
                .max(min_interval);
        (
            actual::Network {
                address,
                public_address,
                soranet_handshake,
                soranet_privacy,
                soranet_vpn,
                lane_profile,
                require_sm_handshake_match,
                require_sm_openssl_preview_match,
                relay_mode: match relay_mode {
                    RelayMode::Disabled => actual::RelayMode::Disabled,
                    RelayMode::Hub => actual::RelayMode::Hub,
                    RelayMode::Spoke => actual::RelayMode::Spoke,
                    RelayMode::Assist => actual::RelayMode::Assist,
                },
                relay_hub_addresses,
                relay_ttl,
                idle_timeout,
                connect_startup_delay: connect_startup_delay.get(),
                dial_timeout,
                deferred_send_ttl: deferred_send_ttl_ms.get().max(min_interval),
                deferred_send_max_per_peer: deferred_send_max_per_peer.max(1),
                peer_gossip_period,
                peer_gossip_max_period,
                trust_gossip,
                trust_decay_half_life: trust_decay_half_life_ms.get(),
                trust_penalty_bad_gossip,
                trust_penalty_unknown_peer,
                trust_min_score,
                dns_refresh_interval: dns_refresh_interval
                    .map(iroha_config_base::util::DurationMs::get),
                dns_refresh_ttl: dns_refresh_ttl.map(iroha_config_base::util::DurationMs::get),
                p2p_proxy,
                p2p_proxy_required,
                p2p_no_proxy,
                p2p_proxy_tls_verify,
                p2p_proxy_tls_pinned_cert_der_base64,
                quic_enabled,
                quic_datagrams_enabled,
                quic_datagram_max_payload_bytes: quic_datagram_max_payload_bytes.get(),
                quic_datagram_receive_buffer_bytes: quic_datagram_receive_buffer_bytes.get(),
                quic_datagram_send_buffer_bytes: quic_datagram_send_buffer_bytes.get(),
                scion: actual::ScionConfig {
                    enabled: scion_enabled,
                    fallback_to_legacy: scion_fallback_to_legacy,
                    listen_endpoint: scion_listen_endpoint,
                    routes: scion_routes,
                },
                tls_enabled,
                tls_fallback_to_plain,
                tls_listen_address,
                tls_inbound_only,
                prefer_ws_fallback,
                p2p_queue_cap_high,
                p2p_queue_cap_low,
                p2p_post_queue_cap,
                p2p_subscriber_queue_cap,
                consensus_ingress_rate_per_sec,
                consensus_ingress_burst,
                consensus_ingress_bytes_per_sec,
                consensus_ingress_bytes_burst,
                consensus_ingress_critical_rate_per_sec,
                consensus_ingress_critical_burst,
                consensus_ingress_critical_bytes_per_sec,
                consensus_ingress_critical_bytes_burst,
                consensus_ingress_rbc_session_limit,
                consensus_ingress_penalty_threshold,
                consensus_ingress_penalty_window: std::time::Duration::from_millis(
                    consensus_ingress_penalty_window_ms,
                ),
                consensus_ingress_penalty_cooldown: std::time::Duration::from_millis(
                    consensus_ingress_penalty_cooldown_ms,
                ),
                happy_eyeballs_stagger: happy_eyeballs_stagger_ms.get(),
                addr_ipv6_first,
                max_incoming,
                max_total_connections,
                accept_rate_per_ip_per_sec,
                accept_burst_per_ip,
                max_accept_buckets,
                accept_bucket_idle: accept_bucket_idle_ms.get(),
                accept_prefix_v4_bits,
                accept_prefix_v6_bits,
                accept_rate_per_prefix_per_sec,
                accept_burst_per_prefix,
                low_priority_rate_per_sec,
                low_priority_burst,
                low_priority_bytes_per_sec,
                low_priority_bytes_burst,
                allowlist_only,
                allow_keys,
                deny_keys,
                allow_cidrs,
                deny_cidrs,
                disconnect_on_post_overflow,
                max_frame_bytes: max_frame_bytes.get(),
                tcp_nodelay,
                tcp_keepalive: Some(tcp_keepalive_ms.map_or(
                    defaults::network::TCP_KEEPALIVE,
                    iroha_config_base::util::DurationMs::get,
                )),
                max_frame_bytes_consensus: max_frame_bytes_consensus.get(),
                max_frame_bytes_control: max_frame_bytes_control.get(),
                max_frame_bytes_block_sync: max_frame_bytes_block_sync.get(),
                max_frame_bytes_tx_gossip: max_frame_bytes_tx_gossip.get(),
                max_frame_bytes_peer_gossip: max_frame_bytes_peer_gossip.get(),
                max_frame_bytes_health: max_frame_bytes_health.get(),
                max_frame_bytes_other: max_frame_bytes_other.get(),
                tls_only_v1_3,
                quic_max_idle_timeout: quic_max_idle_timeout_ms
                    .map(iroha_config_base::util::DurationMs::get),
            },
            actual::BlockSync {
                gossip_period: block_gossip_period,
                gossip_max_period: block_gossip_max_period,
                gossip_size: block_gossip_size,
            },
            actual::TransactionGossiper {
                gossip_period: transaction_gossip_period,
                gossip_size: transaction_gossip_size,
                gossip_resend_ticks: transaction_gossip_resend_ticks,
                dataspace: actual::DataspaceGossip {
                    drop_unknown_dataspace: transaction_gossip_drop_unknown_dataspace,
                    restricted_target_cap: transaction_gossip_restricted_target_cap,
                    public_target_cap: transaction_gossip_public_target_cap,
                    public_target_reshuffle: transaction_gossip_public_target_reshuffle,
                    restricted_target_reshuffle: transaction_gossip_restricted_target_reshuffle,
                    restricted_fallback,
                    restricted_public_payload,
                },
            },
        )
    }
}

#[cfg(test)]
mod network_scion_route_tests {
    use super::*;

    #[test]
    fn parse_scion_routes_accepts_peer_key_map() {
        let peer_key = KeyPair::random().public_key().to_string();
        let mut routes = BTreeMap::new();
        routes.insert(
            peer_key.clone(),
            "scion-gateway.example.com:30257".to_owned(),
        );

        let parsed = Network::parse_scion_routes(routes);
        let peer_id = peer_key.parse::<PeerId>().expect("peer id should parse");
        let route = parsed.get(&peer_id).expect("route should exist");
        assert_eq!("scion-gateway.example.com:30257", route.to_string());
    }

    #[test]
    #[should_panic(expected = "network.scion_routes key")]
    fn parse_scion_routes_rejects_invalid_peer_key() {
        let mut routes = BTreeMap::new();
        routes.insert("not-a-peer-key".to_owned(), "127.0.0.1:30257".to_owned());
        let _ = Network::parse_scion_routes(routes);
    }

    #[test]
    #[should_panic(expected = "network.scion_routes[")]
    fn parse_scion_routes_rejects_invalid_route() {
        let peer_key = KeyPair::random().public_key().to_string();
        let mut routes = BTreeMap::new();
        routes.insert(peer_key, "not-a-socket-address".to_owned());
        let _ = Network::parse_scion_routes(routes);
    }
}

/// User-level configuration container for `Queue`.
#[derive(Debug, Clone, Copy, ReadConfig)]
pub struct Queue {
    /// The upper limit of the number of transactions waiting in the queue.
    #[config(default = "defaults::queue::CAPACITY")]
    pub capacity: NonZeroUsize,
    /// The upper limit of the number of transactions waiting in the queue for a single user.
    /// Use this option to apply throttling.
    #[config(default = "defaults::queue::CAPACITY_PER_USER")]
    pub capacity_per_user: NonZeroUsize,
    /// The transaction will be dropped after this time if it is still in the queue.
    #[config(default = "defaults::queue::TRANSACTION_TIME_TO_LIVE.into()")]
    pub transaction_time_to_live_ms: DurationMs,
    /// Minimum interval between expired-transaction sweeps.
    #[config(default = "defaults::queue::EXPIRED_CULL_INTERVAL.into()")]
    pub expired_cull_interval_ms: DurationMs,
    /// Maximum number of entries scanned per expired-transaction sweep.
    #[config(default = "defaults::queue::EXPIRED_CULL_BATCH")]
    pub expired_cull_batch: NonZeroUsize,
}

impl Queue {
    /// Convert this user configuration into the runtime representation.
    pub fn parse(self) -> actual::Queue {
        let Self {
            capacity,
            capacity_per_user,
            transaction_time_to_live_ms: transaction_time_to_live,
            expired_cull_interval_ms: expired_cull_interval,
            expired_cull_batch,
        } = self;
        actual::Queue {
            capacity,
            capacity_per_user,
            transaction_time_to_live: transaction_time_to_live.0,
            expired_cull_interval: expired_cull_interval.0,
            expired_cull_batch,
        }
    }
}

/// Confidential asset and verifier configuration.
/// User-level configuration container for `Settlement`.
#[derive(Debug, ReadConfig, Clone, Default)]
pub struct Settlement {
    /// Reverse-repurchase configuration details.
    #[config(nested)]
    pub repo: Repo,
    /// Offline settlement configuration.
    #[config(nested)]
    pub offline: Offline,
    /// Router configuration (shadow price, buffers).
    #[config(nested)]
    pub router: Router,
}

/// User-level configuration container for `Repo`.
#[derive(Debug, ReadConfig, Clone)]
pub struct Repo {
    /// Default haircut, expressed in basis points.
    #[config(default = "defaults::settlement::repo::DEFAULT_HAIRCUT_BPS")]
    pub default_haircut_bps: u16,
    /// Margin call frequency in seconds.
    #[config(default = "defaults::settlement::repo::DEFAULT_MARGIN_FREQUENCY_SECS")]
    pub margin_frequency_secs: u64,
    /// Eligible collateral asset definitions.
    #[config(default)]
    pub eligible_collateral: Vec<AssetDefinitionId>,
    /// Matrix describing which collateral definitions may substitute for another.
    #[config(default)]
    pub collateral_substitution_matrix: BTreeMap<AssetDefinitionId, Vec<AssetDefinitionId>>,
}

/// Offline aggregate-proof enforcement modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, strum::EnumString, strum::Display)]
#[strum(serialize_all = "snake_case")]
pub enum OfflineProofMode {
    /// Accept bundles without aggregate proofs (verify when present).
    Optional,
    /// Require bundles to carry aggregate proofs.
    Required,
}

impl OfflineProofMode {
    fn into_actual(self) -> actual::OfflineProofMode {
        match self {
            Self::Optional => actual::OfflineProofMode::Optional,
            Self::Required => actual::OfflineProofMode::Required,
        }
    }
}

impl json::JsonSerialize for OfflineProofMode {
    fn json_serialize(&self, out: &mut String) {
        json::write_json_string(&self.to_string(), out);
    }
}

impl json::JsonDeserialize for OfflineProofMode {
    fn json_deserialize(
        parser: &mut json::Parser<'_>,
    ) -> ::core::result::Result<Self, json::Error> {
        let text = parser.parse_string()?;
        Self::from_str(&text).map_err(|err| json::Error::InvalidField {
            field: "settlement.offline.proof_mode".into(),
            message: err.to_string(),
        })
    }
}

/// User-level configuration for offline settlement retention.
#[derive(Debug, ReadConfig, Clone)]
pub struct Offline {
    /// Minimum number of blocks to keep settlement bundles in hot storage.
    #[config(default = "defaults::settlement::offline::HOT_RETENTION_BLOCKS")]
    pub hot_retention_blocks: u64,
    /// Maximum number of bundles to archive per retention pass.
    #[config(default = "defaults::settlement::offline::ARCHIVE_BATCH_SIZE")]
    pub archive_batch_size: usize,
    /// Minimum number of blocks archived bundles remain available before pruning (0 disables pruning).
    #[config(default = "defaults::settlement::offline::COLD_RETENTION_BLOCKS")]
    pub cold_retention_blocks: u64,
    /// Maximum number of archived bundles pruned per pass.
    #[config(default = "defaults::settlement::offline::PRUNE_BATCH_SIZE")]
    pub prune_batch_size: usize,
    /// Aggregate-proof enforcement mode for offline bundles.
    #[config(default = "defaults::settlement::offline::PROOF_MODE.parse().unwrap()")]
    pub proof_mode: OfflineProofMode,
    /// Maximum age for offline receipts in milliseconds (0 disables age checks).
    #[config(
        default = "DurationMs(std::time::Duration::from_millis(defaults::settlement::offline::MAX_RECEIPT_AGE_MS))"
    )]
    pub max_receipt_age_ms: DurationMs,
    /// Require offline allowances to be escrow-backed.
    #[config(default = "false")]
    pub escrow_required: bool,
    /// Escrow account bindings keyed by asset definition id.
    #[config(default = "BTreeMap::new()")]
    pub escrow_accounts: BTreeMap<String, String>,
    /// Optional list of DER-encoded Android trust anchor files used to supplement the built-in roots.
    #[config(default)]
    pub android_trust_anchor_files: Vec<PathBuf>,
    /// Skip platform attestation verification (for local testing only).
    #[config(default = "defaults::settlement::offline::SKIP_PLATFORM_ATTESTATION")]
    pub skip_platform_attestation: bool,
    /// Skip build claim verification (for local testing only).
    #[config(default = "defaults::settlement::offline::SKIP_BUILD_CLAIM_VERIFICATION")]
    pub skip_build_claim_verification: bool,
    /// Enforce strict iOS App Attest signature verification (disable compatibility fallback).
    #[config(default = "defaults::settlement::offline::APPLE_APP_ATTEST_STRICT_SIGNATURE")]
    pub apple_app_attest_strict_signature: bool,
}

impl Default for Offline {
    fn default() -> Self {
        Self {
            hot_retention_blocks: defaults::settlement::offline::HOT_RETENTION_BLOCKS,
            archive_batch_size: defaults::settlement::offline::ARCHIVE_BATCH_SIZE,
            cold_retention_blocks: defaults::settlement::offline::COLD_RETENTION_BLOCKS,
            prune_batch_size: defaults::settlement::offline::PRUNE_BATCH_SIZE,
            proof_mode: defaults::settlement::offline::PROOF_MODE.parse().unwrap(),
            max_receipt_age_ms: DurationMs(std::time::Duration::from_millis(
                defaults::settlement::offline::MAX_RECEIPT_AGE_MS,
            )),
            escrow_required: false,
            escrow_accounts: BTreeMap::new(),
            android_trust_anchor_files: Vec::new(),
            skip_platform_attestation: defaults::settlement::offline::SKIP_PLATFORM_ATTESTATION,
            skip_build_claim_verification:
                defaults::settlement::offline::SKIP_BUILD_CLAIM_VERIFICATION,
            apple_app_attest_strict_signature:
                defaults::settlement::offline::APPLE_APP_ATTEST_STRICT_SIGNATURE,
        }
    }
}

/// User-level configuration for the settlement router.
#[derive(Debug, ReadConfig, Clone, Copy)]
pub struct Router {
    /// TWAP window used for settlement quotes (seconds).
    #[config(default = "defaults::settlement::router::TWAP_WINDOW_SECS")]
    pub twap_window_seconds: u64,
    /// Base epsilon margin applied to every quote (basis points).
    #[config(default = "defaults::settlement::router::EPSILON_BPS")]
    pub epsilon_bps: u16,
    /// Buffer alert threshold percentage.
    #[config(default = "defaults::settlement::router::ALERT_PCT")]
    pub buffer_alert_pct: u8,
    /// Buffer throttle threshold percentage.
    #[config(default = "defaults::settlement::router::THROTTLE_PCT")]
    pub buffer_throttle_pct: u8,
    /// Buffer XOR-only threshold percentage.
    #[config(default = "defaults::settlement::router::XOR_ONLY_PCT")]
    pub buffer_xor_only_pct: u8,
    /// Buffer halt threshold percentage.
    #[config(default = "defaults::settlement::router::HALT_PCT")]
    pub buffer_halt_pct: u8,
    /// Buffer horizon (hours).
    #[config(default = "defaults::settlement::router::BUFFER_HORIZON_HOURS")]
    pub buffer_horizon_hours: u16,
}

impl Default for Router {
    fn default() -> Self {
        Self {
            twap_window_seconds: defaults::settlement::router::TWAP_WINDOW_SECS,
            epsilon_bps: defaults::settlement::router::EPSILON_BPS,
            buffer_alert_pct: defaults::settlement::router::ALERT_PCT,
            buffer_throttle_pct: defaults::settlement::router::THROTTLE_PCT,
            buffer_xor_only_pct: defaults::settlement::router::XOR_ONLY_PCT,
            buffer_halt_pct: defaults::settlement::router::HALT_PCT,
            buffer_horizon_hours: defaults::settlement::router::BUFFER_HORIZON_HOURS,
        }
    }
}

impl Default for Repo {
    fn default() -> Self {
        Self {
            default_haircut_bps: defaults::settlement::repo::DEFAULT_HAIRCUT_BPS,
            margin_frequency_secs: defaults::settlement::repo::DEFAULT_MARGIN_FREQUENCY_SECS,
            eligible_collateral: Vec::new(),
            collateral_substitution_matrix: BTreeMap::new(),
        }
    }
}

/// User-level configuration container for `Confidential`.
#[derive(Debug, ReadConfig, Clone)]
pub struct Confidential {
    /// Enables confidential asset features for this node.
    #[config(default = "defaults::confidential::ENABLED")]
    pub enabled: bool,
    /// Allows observers to accept confidential blocks without verification (validators must keep false).
    #[config(default = "defaults::confidential::ASSUME_VALID")]
    pub assume_valid: bool,
    /// Preferred verifier backend identifier.
    #[config(default = "defaults::confidential::VERIFIER_BACKEND.to_string()")]
    pub verifier_backend: String,
    /// Maximum proof size accepted from a single confidential operation.
    #[config(default = "defaults::confidential::MAX_PROOF_SIZE_BYTES")]
    pub max_proof_size_bytes: u32,
    /// Maximum number of nullifiers a transaction may consume.
    #[config(default = "defaults::confidential::MAX_NULLIFIERS_PER_TX")]
    pub max_nullifiers_per_tx: u32,
    /// Maximum number of commitments a transaction may create.
    #[config(default = "defaults::confidential::MAX_COMMITMENTS_PER_TX")]
    pub max_commitments_per_tx: u32,
    /// Maximum confidential operations allowed in a block.
    #[config(default = "defaults::confidential::MAX_CONFIDENTIAL_OPS_PER_BLOCK")]
    pub max_confidential_ops_per_block: u32,
    /// Verifier timeout for confidential proofs.
    #[config(default = "defaults::confidential::VERIFY_TIMEOUT.into()")]
    pub verify_timeout_ms: DurationMs,
    /// Maximum age (in blocks) for anchors referenced by confidential proofs.
    #[config(default = "defaults::confidential::MAX_ANCHOR_AGE_BLOCKS")]
    pub max_anchor_age_blocks: u64,
    /// Aggregate proof bytes allowed per block.
    #[config(default = "defaults::confidential::MAX_PROOF_BYTES_BLOCK")]
    pub max_proof_bytes_block: u64,
    /// Maximum verification calls allowed per transaction.
    #[config(default = "defaults::confidential::MAX_VERIFY_CALLS_PER_TX")]
    pub max_verify_calls_per_tx: u32,
    /// Maximum verification calls allowed per block.
    #[config(default = "defaults::confidential::MAX_VERIFY_CALLS_PER_BLOCK")]
    pub max_verify_calls_per_block: u32,
    /// Maximum public inputs accepted per proof.
    #[config(default = "defaults::confidential::MAX_PUBLIC_INPUTS")]
    pub max_public_inputs: u32,
    /// Configured reorg depth bound for retaining commitment tree checkpoints.
    #[config(default = "defaults::confidential::REORG_DEPTH_BOUND")]
    pub reorg_depth_bound: u64,
    /// Minimum delay (in blocks) between policy change request and activation.
    #[config(default = "defaults::confidential::POLICY_TRANSITION_DELAY_BLOCKS")]
    pub policy_transition_delay_blocks: u64,
    /// Grace window (in blocks) around policy activation for conversions.
    #[config(default = "defaults::confidential::POLICY_TRANSITION_WINDOW_BLOCKS")]
    pub policy_transition_window_blocks: u64,
    /// Commitment tree root history length to retain.
    #[config(default = "defaults::confidential::TREE_ROOTS_HISTORY_LEN")]
    pub tree_roots_history_len: u64,
    /// Interval (in blocks) between frontier checkpoints.
    #[config(default = "defaults::confidential::TREE_FRONTIER_CHECKPOINT_INTERVAL")]
    pub tree_frontier_checkpoint_interval: u64,
    /// Maximum active verifier entries allowed in registry.
    #[config(default = "defaults::confidential::REGISTRY_MAX_VK_ENTRIES")]
    pub registry_max_vk_entries: u32,
    /// Maximum active parameter sets allowed in registry.
    #[config(default = "defaults::confidential::REGISTRY_MAX_PARAMS_ENTRIES")]
    pub registry_max_params_entries: u32,
    /// Maximum number of registry mutations allowed per block.
    #[config(default = "defaults::confidential::REGISTRY_MAX_DELTA_PER_BLOCK")]
    pub registry_max_delta_per_block: u32,
    /// Confidential verification gas schedule.
    #[config(nested)]
    pub gas: ConfidentialGas,
}

/// User-level configuration container for confidential verification gas schedule.
#[derive(Debug, ReadConfig, Clone, Copy)]
pub struct ConfidentialGas {
    /// Base verify cost for a confidential proof.
    #[config(default = "defaults::confidential::gas::PROOF_BASE")]
    pub proof_base: u64,
    /// Cost per public input field element.
    #[config(default = "defaults::confidential::gas::PER_PUBLIC_INPUT")]
    pub per_public_input: u64,
    /// Cost per proof byte.
    #[config(default = "defaults::confidential::gas::PER_PROOF_BYTE")]
    pub per_proof_byte: u64,
    /// Cost per nullifier.
    #[config(default = "defaults::confidential::gas::PER_NULLIFIER")]
    pub per_nullifier: u64,
    /// Cost per commitment.
    #[config(default = "defaults::confidential::gas::PER_COMMITMENT")]
    pub per_commitment: u64,
}

impl Default for ConfidentialGas {
    fn default() -> Self {
        Self {
            proof_base: defaults::confidential::gas::PROOF_BASE,
            per_public_input: defaults::confidential::gas::PER_PUBLIC_INPUT,
            per_proof_byte: defaults::confidential::gas::PER_PROOF_BYTE,
            per_nullifier: defaults::confidential::gas::PER_NULLIFIER,
            per_commitment: defaults::confidential::gas::PER_COMMITMENT,
        }
    }
}

impl ConfidentialGas {
    fn parse(self) -> actual::ConfidentialGas {
        actual::ConfidentialGas {
            proof_base: self.proof_base,
            per_public_input: self.per_public_input,
            per_proof_byte: self.per_proof_byte,
            per_nullifier: self.per_nullifier,
            per_commitment: self.per_commitment,
        }
    }
}

impl Settlement {
    /// Convert this user configuration into the runtime representation.
    pub fn parse(self, emitter: &mut Emitter<ParseError>) -> actual::Settlement {
        actual::Settlement {
            repo: self.repo.parse(emitter),
            offline: self.offline.parse(emitter),
            router: self.router.parse(emitter),
        }
    }
}

impl Repo {
    /// Convert this user configuration into the runtime representation.
    pub fn parse(self, emitter: &mut Emitter<ParseError>) -> actual::Repo {
        const MAX_HAIRCUT_BPS: u16 = 10_000;
        let Repo {
            default_haircut_bps,
            margin_frequency_secs,
            eligible_collateral,
            collateral_substitution_matrix,
        } = self;

        let haircut = if default_haircut_bps > MAX_HAIRCUT_BPS {
            emitter.emit(ParseError::InvalidSettlementConfig.into());
            MAX_HAIRCUT_BPS
        } else {
            default_haircut_bps
        };

        let margin = if margin_frequency_secs == 0 {
            emitter.emit(ParseError::InvalidSettlementConfig.into());
            defaults::settlement::repo::DEFAULT_MARGIN_FREQUENCY_SECS
        } else {
            margin_frequency_secs
        };

        actual::Repo {
            default_haircut_bps: haircut,
            margin_frequency_secs: margin,
            eligible_collateral,
            collateral_substitution_matrix,
        }
    }
}

impl Offline {
    /// Convert the offline retention policy into runtime parameters.
    pub fn parse(self, emitter: &mut Emitter<ParseError>) -> actual::Offline {
        let Offline {
            hot_retention_blocks,
            archive_batch_size,
            cold_retention_blocks,
            prune_batch_size,
            proof_mode,
            max_receipt_age_ms,
            escrow_required,
            escrow_accounts,
            android_trust_anchor_files,
            skip_platform_attestation,
            skip_build_claim_verification,
            apple_app_attest_strict_signature,
        } = self;
        if hot_retention_blocks == 0 {
            emitter.emit(ParseError::InvalidSettlementConfig.into());
        }
        if cold_retention_blocks != 0 && cold_retention_blocks <= hot_retention_blocks {
            emitter.emit(Report::new(ParseError::InvalidSettlementConfig).attach(
                "cold_retention_blocks must exceed hot_retention_blocks when pruning is enabled",
            ));
        }
        if cold_retention_blocks != 0 && prune_batch_size == 0 {
            emitter.emit(
                Report::new(ParseError::InvalidSettlementConfig)
                    .attach("prune_batch_size must be > 0 when cold retention is enabled"),
            );
        }
        let mut anchors = Vec::new();
        for path in android_trust_anchor_files {
            match fs::read(&path) {
                Ok(bytes) if !bytes.is_empty() => anchors.push(bytes),
                Ok(bytes) => {
                    drop(bytes);
                    emitter.emit(
                        Report::new(ParseError::InvalidSettlementConfig).attach(format!(
                            "android_trust_anchor file `{}` was empty",
                            path.display()
                        )),
                    );
                }
                Err(err) => {
                    emitter.emit(
                        Report::new(ParseError::InvalidSettlementConfig).attach(format!(
                            "failed to read android_trust_anchor file `{}`: {err}",
                            path.display()
                        )),
                    );
                }
            }
        }
        let mut escrow_bindings = BTreeMap::new();
        for (definition, account) in escrow_accounts {
            let definition_id = match definition.parse() {
                Ok(id) => id,
                Err(err) => {
                    emitter.emit(
                        Report::new(ParseError::InvalidSettlementConfig).attach(format!(
                            "invalid offline escrow asset definition `{definition}`: {err}"
                        )),
                    );
                    continue;
                }
            };
            let account_id = match AccountId::parse_encoded(&account) {
                Ok(parsed) => parsed.into_account_id(),
                Err(err) => {
                    emitter.emit(
                        Report::new(ParseError::InvalidSettlementConfig)
                            .attach(format!("invalid offline escrow account `{account}`: {err}")),
                    );
                    continue;
                }
            };
            if escrow_bindings.insert(definition_id, account_id).is_some() {
                emitter.emit(
                    Report::new(ParseError::InvalidSettlementConfig).attach(format!(
                        "duplicate offline escrow binding for `{definition}`"
                    )),
                );
            }
        }
        actual::Offline {
            hot_retention_blocks,
            archive_batch_size,
            cold_retention_blocks,
            prune_batch_size,
            proof_mode: proof_mode.into_actual(),
            max_receipt_age: max_receipt_age_ms.get(),
            escrow_required,
            escrow_accounts: escrow_bindings,
            android_trust_anchors: anchors,
            skip_platform_attestation,
            skip_build_claim_verification,
            apple_app_attest_strict_signature,
        }
    }
}

impl Router {
    /// Convert router knobs into runtime parameters, validating guard rails.
    pub fn parse(self, emitter: &mut Emitter<ParseError>) -> actual::Router {
        const MAX_EPSILON_BPS: u16 = 10_000;

        let Router {
            mut twap_window_seconds,
            mut epsilon_bps,
            buffer_alert_pct,
            buffer_throttle_pct,
            buffer_xor_only_pct,
            buffer_halt_pct,
            mut buffer_horizon_hours,
        } = self;

        if twap_window_seconds == 0 {
            emitter.emit(ParseError::InvalidSettlementConfig.into());
            twap_window_seconds = defaults::settlement::router::TWAP_WINDOW_SECS;
        }

        if epsilon_bps > MAX_EPSILON_BPS {
            emitter.emit(ParseError::InvalidSettlementConfig.into());
            epsilon_bps = defaults::settlement::router::EPSILON_BPS;
        }

        if buffer_horizon_hours == 0 {
            emitter.emit(ParseError::InvalidSettlementConfig.into());
            buffer_horizon_hours = defaults::settlement::router::BUFFER_HORIZON_HOURS;
        }

        let thresholds = (
            buffer_alert_pct,
            buffer_throttle_pct,
            buffer_xor_only_pct,
            buffer_halt_pct,
        );

        let resolved_thresholds = if Self::thresholds_valid(thresholds) {
            thresholds
        } else {
            emitter.emit(
                Report::new(ParseError::InvalidSettlementConfig)
                    .attach("buffer thresholds must satisfy alert > throttle > xor_only > halt"),
            );
            (
                defaults::settlement::router::ALERT_PCT,
                defaults::settlement::router::THROTTLE_PCT,
                defaults::settlement::router::XOR_ONLY_PCT,
                defaults::settlement::router::HALT_PCT,
            )
        };

        actual::Router {
            twap_window: Duration::from_secs(twap_window_seconds),
            epsilon_bps,
            buffer_alert_pct: resolved_thresholds.0,
            buffer_throttle_pct: resolved_thresholds.1,
            buffer_xor_only_pct: resolved_thresholds.2,
            buffer_halt_pct: resolved_thresholds.3,
            buffer_horizon_hours,
        }
    }

    fn thresholds_valid(thresholds: (u8, u8, u8, u8)) -> bool {
        let (alert, throttle, xor_only, halt) = thresholds;
        alert <= 100 && halt < xor_only && xor_only < throttle && throttle < alert && halt < 100
    }
}

impl Default for Confidential {
    fn default() -> Self {
        Self {
            enabled: defaults::confidential::ENABLED,
            assume_valid: defaults::confidential::ASSUME_VALID,
            verifier_backend: defaults::confidential::VERIFIER_BACKEND.to_string(),
            max_proof_size_bytes: defaults::confidential::MAX_PROOF_SIZE_BYTES,
            max_nullifiers_per_tx: defaults::confidential::MAX_NULLIFIERS_PER_TX,
            max_commitments_per_tx: defaults::confidential::MAX_COMMITMENTS_PER_TX,
            max_confidential_ops_per_block: defaults::confidential::MAX_CONFIDENTIAL_OPS_PER_BLOCK,
            verify_timeout_ms: defaults::confidential::VERIFY_TIMEOUT.into(),
            max_anchor_age_blocks: defaults::confidential::MAX_ANCHOR_AGE_BLOCKS,
            max_proof_bytes_block: defaults::confidential::MAX_PROOF_BYTES_BLOCK,
            max_verify_calls_per_tx: defaults::confidential::MAX_VERIFY_CALLS_PER_TX,
            max_verify_calls_per_block: defaults::confidential::MAX_VERIFY_CALLS_PER_BLOCK,
            max_public_inputs: defaults::confidential::MAX_PUBLIC_INPUTS,
            reorg_depth_bound: defaults::confidential::REORG_DEPTH_BOUND,
            policy_transition_delay_blocks: defaults::confidential::POLICY_TRANSITION_DELAY_BLOCKS,
            policy_transition_window_blocks:
                defaults::confidential::POLICY_TRANSITION_WINDOW_BLOCKS,
            tree_roots_history_len: defaults::confidential::TREE_ROOTS_HISTORY_LEN,
            tree_frontier_checkpoint_interval:
                defaults::confidential::TREE_FRONTIER_CHECKPOINT_INTERVAL,
            registry_max_vk_entries: defaults::confidential::REGISTRY_MAX_VK_ENTRIES,
            registry_max_params_entries: defaults::confidential::REGISTRY_MAX_PARAMS_ENTRIES,
            registry_max_delta_per_block: defaults::confidential::REGISTRY_MAX_DELTA_PER_BLOCK,
            gas: ConfidentialGas::default(),
        }
    }
}

impl Confidential {
    /// Convert this user configuration into the runtime representation.
    pub fn parse(self) -> actual::Confidential {
        actual::Confidential {
            enabled: self.enabled,
            assume_valid: self.assume_valid,
            verifier_backend: self.verifier_backend,
            max_proof_size_bytes: self.max_proof_size_bytes,
            max_nullifiers_per_tx: self.max_nullifiers_per_tx,
            max_commitments_per_tx: self.max_commitments_per_tx,
            max_confidential_ops_per_block: self.max_confidential_ops_per_block,
            verify_timeout: self.verify_timeout_ms.0,
            max_anchor_age_blocks: self.max_anchor_age_blocks,
            max_proof_bytes_block: self.max_proof_bytes_block,
            max_verify_calls_per_tx: self.max_verify_calls_per_tx,
            max_verify_calls_per_block: self.max_verify_calls_per_block,
            max_public_inputs: self.max_public_inputs,
            reorg_depth_bound: self.reorg_depth_bound,
            policy_transition_delay_blocks: self.policy_transition_delay_blocks,
            policy_transition_window_blocks: self.policy_transition_window_blocks,
            tree_roots_history_len: self.tree_roots_history_len,
            tree_frontier_checkpoint_interval: self.tree_frontier_checkpoint_interval,
            registry_max_vk_entries: self.registry_max_vk_entries,
            registry_max_params_entries: self.registry_max_params_entries,
            registry_max_delta_per_block: self.registry_max_delta_per_block,
            gas: self.gas.parse(),
        }
    }
}

/// Norito streaming control-plane configuration.
/// User-level configuration container for `Streaming`.
#[derive(Debug, Clone)]
pub struct Streaming {
    /// Hex-encoded Kyber public key used to encapsulate session material for viewers.
    pub kyber_public_key: Option<WithOrigin<String>>,
    /// Hex-encoded Kyber secret key used to decapsulate viewer payloads.
    pub kyber_secret_key: Option<WithOrigin<String>>,
    /// Named ML-KEM suite controlling HPKE key lengths (e.g., `mlkem768`).
    pub kyber_suite: Option<WithOrigin<String>>,
    /// Optional override for the streaming identity public key (must be Ed25519).
    pub identity_public_key: Option<WithOrigin<PublicKey>>,
    /// Optional override for the streaming identity private key (must be Ed25519).
    pub identity_private_key: Option<WithOrigin<PrivateKey>>,
    /// Directory for persisted streaming session snapshots.
    pub session_store_dir: WithOrigin<PathBuf>,
    /// Feature bitmask advertised during streaming capability negotiation.
    pub feature_bits: WithOrigin<u32>,
    /// Optional overrides for SoraNet circuit integration defaults.
    pub soranet: Option<WithOrigin<StreamingSoranet>>,
    /// Optional overrides for SoraVPN provisioning spools.
    pub soravpn: Option<WithOrigin<StreamingSoravpn>>,
    /// Optional overrides for audio/video sync enforcement.
    pub sync: Option<WithOrigin<StreamingSync>>,
    /// Optional codec overrides (CABAC gating, trellis scopes, rANS tables).
    pub codec: Option<WithOrigin<StreamingCodec>>,
}

impl ReadConfigTrait for Streaming {
    fn read(reader: &mut ConfigReader) -> FinalWrap<Self>
    where
        Self: Sized,
    {
        let kyber_public_key = reader
            .read_parameter::<String>(["kyber_public_key"])
            .value_optional()
            .finish_with_origin();

        let kyber_secret_key = reader
            .read_parameter::<String>(["kyber_secret_key"])
            .value_optional()
            .finish_with_origin();

        let kyber_suite = reader
            .read_parameter::<String>(["kyber_suite"])
            .value_optional()
            .finish_with_origin();

        let identity_public_key = reader
            .read_parameter::<PublicKey>(["identity_public_key"])
            .env("STREAMING_IDENTITY_PUBLIC_KEY")
            .value_optional()
            .finish_with_origin();

        let identity_private_key = reader
            .read_parameter::<PrivateKey>(["identity_private_key"])
            .env("STREAMING_IDENTITY_PRIVATE_KEY")
            .value_optional()
            .finish_with_origin();

        let session_store_dir = reader
            .read_parameter::<PathBuf>(["session_store_dir"])
            .value_or_else(|| PathBuf::from(defaults::streaming::SESSION_STORE_DIR))
            .finish_with_origin();

        let feature_bits = reader
            .read_parameter::<u32>(["feature_bits"])
            .value_or_else(|| 0)
            .finish_with_origin();

        let soranet = reader
            .read_parameter::<StreamingSoranet>(["soranet"])
            .value_optional()
            .finish_with_origin();

        let soravpn = reader
            .read_parameter::<StreamingSoravpn>(["soravpn"])
            .value_optional()
            .finish_with_origin();

        let sync = reader
            .read_parameter::<StreamingSync>(["sync"])
            .value_optional()
            .finish_with_origin();

        let codec = reader
            .read_parameter::<StreamingCodec>(["codec"])
            .value_optional()
            .finish_with_origin();

        FinalWrap::value_fn(move || Self {
            kyber_public_key: kyber_public_key.unwrap(),
            kyber_secret_key: kyber_secret_key.unwrap(),
            kyber_suite: kyber_suite.unwrap(),
            identity_public_key: identity_public_key.unwrap(),
            identity_private_key: identity_private_key.unwrap(),
            session_store_dir: session_store_dir.unwrap(),
            feature_bits: feature_bits.unwrap(),
            soranet: soranet.unwrap(),
            soravpn: soravpn.unwrap(),
            sync: sync.unwrap(),
            codec: codec.unwrap(),
        })
    }
}

impl Streaming {
    /// Convert this user configuration into the runtime representation.
    pub fn parse(
        self,
        identity: &KeyPair,
        emitter: &mut Emitter<ParseError>,
    ) -> Option<actual::Streaming> {
        let soranet_overrides = self.soranet.clone().map(WithOrigin::into_tuple);
        let soravpn_overrides = self.soravpn.clone().map(WithOrigin::into_tuple);
        let sync_overrides = self.sync.clone().map(WithOrigin::into_tuple);
        let codec_overrides = self.codec.clone().map(WithOrigin::into_tuple);
        let streaming_identity = self.resolve_identity(identity, emitter)?;
        let mut key_material = Self::init_key_material(streaming_identity, emitter)?;
        let kem_suite = self.resolve_kyber_suite(emitter)?;
        key_material.set_kem_suite(kem_suite);

        match self.collect_kyber_inputs(emitter)? {
            KyberKeyConfig::Absent => {}
            KyberKeyConfig::Present((public, public_origin, secret, secret_origin)) => {
                if let Err(err) = key_material.set_kyber_keys(&public, &secret) {
                    emitter.emit(
                        Report::new(ParseError::InvalidStreamingConfig)
                            .attach(format!("streaming Kyber key validation failed: {err}"))
                            .attach(ConfigValueAndOrigin::new("[REDACTED]", public_origin))
                            .attach(ConfigValueAndOrigin::new("[REDACTED]", secret_origin)),
                    );
                    return None;
                }
            }
        }

        let store_dir = self.session_store_dir.into_value();
        let feature_bits = self.feature_bits.into_value();
        let soranet = match soranet_overrides {
            Some((overrides, _origin)) => overrides.parse(emitter)?,
            None => actual::StreamingSoranet::from_defaults(),
        };
        let soravpn = match soravpn_overrides {
            Some((overrides, _origin)) => overrides.parse(emitter)?,
            None => actual::StreamingSoravpn::from_defaults(),
        };

        Some(actual::Streaming {
            key_material,
            session_store_dir: store_dir,
            feature_bits,
            soranet,
            soravpn,
            sync: match sync_overrides {
                Some((sync_cfg, _origin)) => sync_cfg.parse(emitter)?,
                None => actual::StreamingSync::from_defaults(),
            },
            codec: match codec_overrides {
                Some((codec_cfg, _origin)) => codec_cfg.parse(emitter)?,
                None => actual::StreamingCodec::from_defaults(),
            },
        })
    }

    fn resolve_identity(
        &self,
        identity: &KeyPair,
        emitter: &mut Emitter<ParseError>,
    ) -> Option<KeyPair> {
        match (
            self.identity_public_key.clone().map(WithOrigin::into_tuple),
            self.identity_private_key
                .clone()
                .map(WithOrigin::into_tuple),
        ) {
            (Some((pub_key, pub_origin)), Some((priv_key, priv_origin))) => {
                if pub_key.algorithm() != iroha_crypto::Algorithm::Ed25519 {
                    emitter.emit(
                        Report::new(ParseError::InvalidStreamingConfig)
                            .attach("streaming.identity_public_key must be Ed25519")
                            .attach(ConfigValueAndOrigin::new("[REDACTED]", pub_origin)),
                    );
                    return None;
                }
                if priv_key.algorithm() != iroha_crypto::Algorithm::Ed25519 {
                    emitter.emit(
                        Report::new(ParseError::InvalidStreamingConfig)
                            .attach("streaming.identity_private_key must be Ed25519")
                            .attach(ConfigValueAndOrigin::new("[REDACTED]", priv_origin)),
                    );
                    return None;
                }
                match iroha_crypto::KeyPair::new(pub_key, priv_key) {
                    Ok(pair) => Some(pair),
                    Err(err) => {
                        emitter.emit(
                            Report::new(ParseError::InvalidStreamingConfig)
                                .attach(format!("failed to construct streaming key pair: {err}"))
                                .attach(ConfigValueAndOrigin::new("[REDACTED]", pub_origin))
                                .attach(ConfigValueAndOrigin::new("[REDACTED]", priv_origin)),
                        );
                        None
                    }
                }
            }
            (Some((_, origin)), None) => {
                emitter.emit(
                    Report::new(ParseError::InvalidStreamingConfig)
                        .attach(
                            "streaming.identity_private_key must be provided when identity_public_key is set",
                        )
                        .attach(ConfigValueAndOrigin::new("[REDACTED]", origin)),
                );
                None
            }
            (None, Some((_, origin))) => {
                emitter.emit(
                    Report::new(ParseError::InvalidStreamingConfig)
                        .attach(
                            "streaming.identity_public_key must be provided when identity_private_key is set",
                        )
                        .attach(ConfigValueAndOrigin::new("[REDACTED]", origin)),
                );
                None
            }
            (None, None) => Some(identity.clone()),
        }
    }

    fn resolve_kyber_suite(&self, emitter: &mut Emitter<ParseError>) -> Option<MlKemSuite> {
        match self.kyber_suite.clone().map(WithOrigin::into_tuple) {
            Some((suite_raw, origin)) => match suite_raw.parse::<MlKemSuite>() {
                Ok(suite) => Some(suite),
                Err(SuiteParseError(invalid)) => {
                    let allowed = MlKemSuite::ALL
                        .iter()
                        .map(std::string::ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(", ");
                    emitter.emit(
                        Report::new(ParseError::InvalidStreamingConfig)
                            .attach(format!("streaming.kyber_suite must be one of {allowed}"))
                            .attach(ConfigValueAndOrigin::new(invalid, origin)),
                    );
                    None
                }
            },
            None => Some(STREAMING_DEFAULT_KEM_SUITE),
        }
    }

    fn init_key_material(
        identity: KeyPair,
        emitter: &mut Emitter<ParseError>,
    ) -> Option<StreamingKeyMaterial> {
        match StreamingKeyMaterial::new(identity) {
            Ok(material) => Some(material),
            Err(KeyMaterialError::UnsupportedIdentityAlgorithm(algorithm)) => {
                emitter.emit(
                    Report::new(ParseError::InvalidStreamingConfig).attach(format!(
                        "streaming requires an Ed25519 identity key, found {algorithm:?}"
                    )),
                );
                None
            }
            Err(error) => {
                emitter.emit(
                    Report::new(ParseError::InvalidStreamingConfig)
                        .attach(format!("streaming key material error: {error}")),
                );
                None
            }
        }
    }

    fn collect_kyber_inputs(&self, emitter: &mut Emitter<ParseError>) -> Option<KyberKeyConfig> {
        let public = self.kyber_public_key.clone().map(WithOrigin::into_tuple);
        let secret = self.kyber_secret_key.clone().map(WithOrigin::into_tuple);

        match (public, secret) {
            (Some((pub_raw, pub_origin)), Some((sec_raw, sec_origin))) => {
                let (public_bytes, public_origin) = Self::decode_hex_field(
                    pub_raw.as_str(),
                    pub_origin,
                    "kyber_public_key",
                    emitter,
                )?;
                let (secret_bytes, secret_origin) = Self::decode_hex_field(
                    sec_raw.as_str(),
                    sec_origin,
                    "kyber_secret_key",
                    emitter,
                )?;
                Some(KyberKeyConfig::Present((
                    public_bytes,
                    public_origin,
                    secret_bytes,
                    secret_origin,
                )))
            }
            (None, None) => Some(KyberKeyConfig::Absent),
            (Some((_, origin)), None) => {
                emitter.emit(
                    Report::new(ParseError::InvalidStreamingConfig)
                        .attach(
                            "streaming.kyber_secret_key must be provided when kyber_public_key is set",
                        )
                        .attach(ConfigValueAndOrigin::new("[REDACTED]", origin)),
                );
                None
            }
            (None, Some((_, origin))) => {
                emitter.emit(
                    Report::new(ParseError::InvalidStreamingConfig)
                        .attach(
                            "streaming.kyber_public_key must be provided when kyber_secret_key is set",
                        )
                        .attach(ConfigValueAndOrigin::new("[REDACTED]", origin)),
                );
                None
            }
        }
    }

    fn decode_hex_field(
        raw: &str,
        origin: ParameterOrigin,
        field: &'static str,
        emitter: &mut Emitter<ParseError>,
    ) -> Option<(Vec<u8>, ParameterOrigin)> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            emitter.emit(
                Report::new(ParseError::InvalidStreamingConfig)
                    .attach(format!("streaming.{field} must not be empty"))
                    .attach(ConfigValueAndOrigin::new("[REDACTED]", origin)),
            );
            return None;
        }
        let without_prefix = trimmed
            .strip_prefix("0x")
            .or_else(|| trimmed.strip_prefix("0X"))
            .unwrap_or(trimmed);
        match hex::decode(without_prefix) {
            Ok(bytes) => Some((bytes, origin)),
            Err(err) => {
                emitter.emit(
                    Report::new(ParseError::InvalidStreamingConfig)
                        .attach(format!("streaming.{field} failed to decode as hex: {err}"))
                        .attach(ConfigValueAndOrigin::new("[REDACTED]", origin)),
                );
                None
            }
        }
    }
}

/// Audio/video sync enforcement configuration surfaced in user config.
#[derive(Debug, Clone, ReadConfig, norito::JsonDeserialize)]
pub struct StreamingSync {
    #[config(default = "defaults::streaming::sync::ENABLED")]
    /// Toggles the sync enforcement gate.
    pub enabled: bool,
    #[config(default = "defaults::streaming::sync::OBSERVE_ONLY")]
    /// When `true`, violations are logged but not rejected.
    pub observe_only: bool,
    #[config(default = "defaults::streaming::sync::MIN_WINDOW_MS")]
    /// Minimum telemetry window (milliseconds) required before enforcement.
    pub min_window_ms: WithOrigin<u16>,
    #[config(default = "defaults::streaming::sync::EWMA_THRESHOLD_MS")]
    /// Sustained EWMA drift threshold (milliseconds).
    pub ewma_threshold_ms: WithOrigin<u16>,
    #[config(default = "defaults::streaming::sync::HARD_CAP_MS")]
    /// Hard cap for any single frame drift (milliseconds).
    pub hard_cap_ms: WithOrigin<u16>,
}

impl StreamingSync {
    /// Convert the user-facing representation into the runtime sync policy.
    pub fn parse(self, emitter: &mut Emitter<ParseError>) -> Option<actual::StreamingSync> {
        let mut config = actual::StreamingSync::from_defaults();
        config.enabled = self.enabled;
        config.observe_only = self.observe_only;

        let (min_window_ms, min_origin) = self.min_window_ms.into_tuple();
        if min_window_ms == 0 {
            emitter.emit(
                Report::new(ParseError::InvalidStreamingConfig)
                    .attach("streaming.sync.min_window_ms must be greater than zero")
                    .attach(ConfigValueAndOrigin::new(
                        min_window_ms.to_string(),
                        min_origin,
                    )),
            );
            return None;
        }
        config.min_window_ms = min_window_ms;

        let (ewma_threshold_ms, ewma_origin) = self.ewma_threshold_ms.into_tuple();
        if ewma_threshold_ms == 0 {
            emitter.emit(
                Report::new(ParseError::InvalidStreamingConfig)
                    .attach("streaming.sync.ewma_threshold_ms must be greater than zero")
                    .attach(ConfigValueAndOrigin::new(
                        ewma_threshold_ms.to_string(),
                        ewma_origin,
                    )),
            );
            return None;
        }
        config.ewma_threshold_ms = ewma_threshold_ms;

        let (hard_cap_ms, hard_origin) = self.hard_cap_ms.into_tuple();
        if hard_cap_ms == 0 {
            emitter.emit(
                Report::new(ParseError::InvalidStreamingConfig)
                    .attach("streaming.sync.hard_cap_ms must be greater than zero")
                    .attach(ConfigValueAndOrigin::new(
                        hard_cap_ms.to_string(),
                        hard_origin,
                    )),
            );
            return None;
        }
        if hard_cap_ms < ewma_threshold_ms {
            emitter.emit(
                Report::new(ParseError::InvalidStreamingConfig)
                    .attach(
                        "streaming.sync.hard_cap_ms must be greater than or equal to streaming.sync.ewma_threshold_ms",
                    )
                    .attach(ConfigValueAndOrigin::new(
                        hard_cap_ms.to_string(),
                        hard_origin,
                    ))
                    .attach(ConfigValueAndOrigin::new(
                        ewma_threshold_ms.to_string(),
                        ewma_origin,
                    )),
            );
            return None;
        }
        config.hard_cap_ms = hard_cap_ms;

        Some(config)
    }
}

/// SoraNet integration defaults surfaced in user configuration.
#[derive(Debug, Clone, ReadConfig, norito::JsonDeserialize)]
pub struct StreamingSoranet {
    #[config(default = "defaults::streaming::soranet::ENABLED")]
    /// Toggles SoraNet integration for streaming routes.
    pub enabled: bool,
    #[config(default = "defaults::streaming::soranet::EXIT_MULTIADDR.to_string()")]
    /// Default exit relay multi-address.
    pub exit_multiaddr: WithOrigin<String>,
    #[config(default = "defaults::streaming::soranet::padding_budget_ms()")]
    /// Optional padding jitter budget expressed in milliseconds.
    pub padding_budget_ms: WithOrigin<Option<u16>>,
    #[config(default = "defaults::streaming::soranet::ACCESS_KIND.to_string()")]
    /// Access policy applied when establishing SoraNet circuits.
    pub access_kind: WithOrigin<String>,
    /// Optional override hashed into blinded channel identifiers.
    pub channel_salt: Option<WithOrigin<String>>,
    #[config(default = "PathBuf::from(defaults::streaming::soranet::PROVISION_SPOOL_DIR)")]
    /// Directory where privacy-route updates are spooled for SoraNet exits.
    pub provision_spool_dir: WithOrigin<PathBuf>,
    /// Maximum on-disk footprint for the SoraNet provision spool (0 = unlimited).
    #[config(default = "defaults::streaming::soranet::PROVISION_SPOOL_MAX_BYTES")]
    pub provision_spool_max_bytes: WithOrigin<Bytes<u64>>,
    /// Segment window (inclusive) used when provisioning privacy routes.
    #[config(default = "defaults::streaming::soranet::PROVISION_WINDOW_SEGMENTS")]
    pub provision_window_segments: WithOrigin<u64>,
    /// Maximum number of queued privacy-route provisioning jobs.
    #[config(default = "defaults::streaming::soranet::PROVISION_QUEUE_CAPACITY")]
    pub provision_queue_capacity: WithOrigin<u64>,
}

impl StreamingSoranet {
    /// Convert user-supplied overrides into runtime defaults.
    pub fn parse(self, emitter: &mut Emitter<ParseError>) -> Option<actual::StreamingSoranet> {
        let mut config = actual::StreamingSoranet::from_defaults();
        config.enabled = self.enabled;

        let (exit_multiaddr, exit_origin) = self.exit_multiaddr.into_tuple();
        if exit_multiaddr.trim().is_empty() {
            emitter.emit(
                Report::new(ParseError::InvalidStreamingConfig)
                    .attach("streaming.soranet.exit_multiaddr must not be empty")
                    .attach(ConfigValueAndOrigin::new(exit_multiaddr, exit_origin)),
            );
            return None;
        }
        config.exit_multiaddr = exit_multiaddr;

        let (padding_budget_ms, _padding_origin) = self.padding_budget_ms.into_tuple();
        config.padding_budget_ms = padding_budget_ms;

        let (access_label, access_origin) = self.access_kind.into_tuple();
        if let Ok(kind) = access_label.parse::<actual::StreamingSoranetAccessKind>() {
            config.access_kind = kind;
        } else {
            emitter.emit(
                Report::new(ParseError::InvalidStreamingConfig)
                    .attach(format!(
                        "unknown streaming.soranet.access_kind `{access_label}`; expected `authenticated` or `read-only`"
                    ))
                    .attach(ConfigValueAndOrigin::new(access_label, access_origin)),
            );
            return None;
        }

        if let Some(channel_salt) = self.channel_salt {
            let (label, origin) = channel_salt.into_tuple();
            if label.trim().is_empty() {
                emitter.emit(
                    Report::new(ParseError::InvalidStreamingConfig)
                        .attach("streaming.soranet.channel_salt must not be empty when provided")
                        .attach(ConfigValueAndOrigin::new(label, origin)),
                );
                return None;
            }
            config.channel_salt = label;
        }

        let (spool_dir, spool_origin) = self.provision_spool_dir.into_tuple();
        if spool_dir.as_os_str().is_empty() {
            emitter.emit(
                Report::new(ParseError::InvalidStreamingConfig)
                    .attach("streaming.soranet.provision_spool_dir must not be empty")
                    .attach(ConfigValueAndOrigin::new(
                        spool_dir.to_string_lossy().into_owned(),
                        spool_origin,
                    )),
            );
            return None;
        }
        config.provision_spool_dir = spool_dir;
        let (spool_max_bytes, _spool_max_origin) = self.provision_spool_max_bytes.into_tuple();
        config.provision_spool_max_bytes = spool_max_bytes;
        let (window_segments, window_origin) = self.provision_window_segments.into_tuple();
        if window_segments == 0 {
            emitter.emit(
                Report::new(ParseError::InvalidStreamingConfig)
                    .attach("streaming.soranet.provision_window_segments must be greater than zero")
                    .attach(ConfigValueAndOrigin::new(
                        window_segments.to_string(),
                        window_origin,
                    )),
            );
            return None;
        }
        config.provision_window_segments = window_segments;
        let (queue_capacity, queue_origin) = self.provision_queue_capacity.into_tuple();
        if queue_capacity == 0 {
            emitter.emit(
                Report::new(ParseError::InvalidStreamingConfig)
                    .attach("streaming.soranet.provision_queue_capacity must be greater than zero")
                    .attach(ConfigValueAndOrigin::new(
                        queue_capacity.to_string(),
                        queue_origin,
                    )),
            );
            return None;
        }
        if usize::try_from(queue_capacity).is_err() {
            emitter.emit(
                Report::new(ParseError::InvalidStreamingConfig)
                    .attach(
                        "streaming.soranet.provision_queue_capacity must fit the platform usize",
                    )
                    .attach(ConfigValueAndOrigin::new(
                        queue_capacity.to_string(),
                        queue_origin,
                    )),
            );
            return None;
        }
        config.provision_queue_capacity = queue_capacity;

        Some(config)
    }
}

/// SoraVPN provisioning spools surfaced in user configuration.
#[derive(Debug, Clone, ReadConfig, norito::JsonDeserialize)]
pub struct StreamingSoravpn {
    #[config(default = "PathBuf::from(defaults::streaming::soravpn::PROVISION_SPOOL_DIR)")]
    /// Directory where SoraVPN route updates are spooled for local VPN nodes.
    pub provision_spool_dir: WithOrigin<PathBuf>,
    /// Maximum on-disk footprint for the SoraVPN provision spool (0 = unlimited).
    #[config(default = "defaults::streaming::soravpn::PROVISION_SPOOL_MAX_BYTES")]
    pub provision_spool_max_bytes: WithOrigin<Bytes<u64>>,
}

impl StreamingSoravpn {
    /// Convert user-supplied overrides into runtime defaults.
    pub fn parse(self, emitter: &mut Emitter<ParseError>) -> Option<actual::StreamingSoravpn> {
        let mut config = actual::StreamingSoravpn::from_defaults();
        let (spool_dir, spool_origin) = self.provision_spool_dir.into_tuple();
        if spool_dir.as_os_str().is_empty() {
            emitter.emit(
                Report::new(ParseError::InvalidStreamingConfig)
                    .attach("streaming.soravpn.provision_spool_dir must not be empty")
                    .attach(ConfigValueAndOrigin::new(
                        spool_dir.to_string_lossy().into_owned(),
                        spool_origin,
                    )),
            );
            return None;
        }
        config.provision_spool_dir = spool_dir;
        let (spool_max_bytes, _spool_max_origin) = self.provision_spool_max_bytes.into_tuple();
        config.provision_spool_max_bytes = spool_max_bytes;

        Some(config)
    }
}

#[cfg(test)]
mod streaming_soranet_tests {
    use super::*;

    #[test]
    fn streaming_soranet_rejects_zero_window_segments() {
        let mut emitter = Emitter::<ParseError>::new();
        let config = StreamingSoranet {
            enabled: true,
            exit_multiaddr: WithOrigin::inline(
                defaults::streaming::soranet::EXIT_MULTIADDR.to_string(),
            ),
            padding_budget_ms: WithOrigin::inline(defaults::streaming::soranet::padding_budget_ms()),
            access_kind: WithOrigin::inline(defaults::streaming::soranet::ACCESS_KIND.to_string()),
            channel_salt: None,
            provision_spool_dir: WithOrigin::inline(PathBuf::from(
                defaults::streaming::soranet::PROVISION_SPOOL_DIR,
            )),
            provision_spool_max_bytes: WithOrigin::inline(
                defaults::streaming::soranet::PROVISION_SPOOL_MAX_BYTES,
            ),
            provision_window_segments: WithOrigin::inline(0),
            provision_queue_capacity: WithOrigin::inline(
                defaults::streaming::soranet::PROVISION_QUEUE_CAPACITY,
            ),
        };

        assert!(config.parse(&mut emitter).is_none());
        let err = emitter
            .into_result()
            .expect_err("zero window segments must be rejected");
        let debug = format!("{err:?}");
        assert!(
            debug.contains("streaming.soranet.provision_window_segments"),
            "unexpected error payload: {debug}"
        );
    }

    #[test]
    fn streaming_soranet_rejects_zero_queue_capacity() {
        let mut emitter = Emitter::<ParseError>::new();
        let config = StreamingSoranet {
            enabled: true,
            exit_multiaddr: WithOrigin::inline(
                defaults::streaming::soranet::EXIT_MULTIADDR.to_string(),
            ),
            padding_budget_ms: WithOrigin::inline(defaults::streaming::soranet::padding_budget_ms()),
            access_kind: WithOrigin::inline(defaults::streaming::soranet::ACCESS_KIND.to_string()),
            channel_salt: None,
            provision_spool_dir: WithOrigin::inline(PathBuf::from(
                defaults::streaming::soranet::PROVISION_SPOOL_DIR,
            )),
            provision_spool_max_bytes: WithOrigin::inline(
                defaults::streaming::soranet::PROVISION_SPOOL_MAX_BYTES,
            ),
            provision_window_segments: WithOrigin::inline(
                defaults::streaming::soranet::PROVISION_WINDOW_SEGMENTS,
            ),
            provision_queue_capacity: WithOrigin::inline(0),
        };

        assert!(config.parse(&mut emitter).is_none());
        let err = emitter
            .into_result()
            .expect_err("zero queue capacity must be rejected");
        let debug = format!("{err:?}");
        assert!(
            debug.contains("streaming.soranet.provision_queue_capacity"),
            "unexpected error payload: {debug}"
        );
    }
}

#[cfg(test)]
mod streaming_soravpn_tests {
    use super::*;

    #[test]
    fn streaming_soravpn_rejects_empty_spool_dir() {
        let mut emitter = Emitter::<ParseError>::new();
        let config = StreamingSoravpn {
            provision_spool_dir: WithOrigin::inline(PathBuf::new()),
            provision_spool_max_bytes: WithOrigin::inline(Bytes(0)),
        };

        assert!(config.parse(&mut emitter).is_none());
        let err = emitter
            .into_result()
            .expect_err("empty spool dir must be rejected");
        let debug = format!("{err:?}");
        assert!(
            debug.contains("streaming.soravpn.provision_spool_dir"),
            "unexpected error payload: {debug}"
        );
    }
}

/// Cryptography configuration (user view).
#[derive(Debug, ReadConfig, Clone)]
pub struct Crypto {
    /// Whether the OpenSSL-backed SM preview helpers are enabled.
    #[config(
        env = "CRYPTO_SM_OPENSSL_PREVIEW",
        default = "defaults::crypto::ENABLE_SM_OPENSSL_PREVIEW"
    )]
    pub enable_sm_openssl_preview: bool,
    /// SM intrinsic dispatch policy (`auto`, `force-enable`, `force-disable`).
    #[config(
        env = "CRYPTO_SM_INTRINSICS",
        default = "SmIntrinsicsPolicyConfig::from(defaults::crypto::SM_INTRINSICS_POLICY)"
    )]
    pub sm_intrinsics: SmIntrinsicsPolicyConfig,
    /// Default hash algorithm identifier (e.g., `blake2b-256`, `sm3-256`).
    #[config(
        env = "CRYPTO_DEFAULT_HASH",
        default = "defaults::crypto::DEFAULT_HASH.to_owned()"
    )]
    pub default_hash: String,
    /// Signing algorithms allowed for transaction admission (string list).
    #[config(
        env = "CRYPTO_ALLOWED_SIGNING",
        default = "AlgorithmListConfig::from(defaults::crypto::allowed_signing_env())"
    )]
    pub allowed_signing: AlgorithmListConfig,
    /// Default distinguishing identifier applied for SM2 signatures when callers omit one.
    #[config(
        env = "CRYPTO_SM2_DISTID_DEFAULT",
        default = "defaults::crypto::SM2_DISTID_DEFAULT.to_owned()"
    )]
    pub sm2_distid_default: String,
    /// Curve capability overrides (optional).
    #[config(nested)]
    pub curves: CryptoCurves,
}

/// Curve capability configuration (user view).
#[derive(Debug, ReadConfig, Clone)]
pub struct CryptoCurves {
    /// Allowed curve identifiers per the account curve registry.
    #[config(
        env = "CRYPTO_CURVES_ALLOWED_IDS",
        default = "CurveIdListConfig::default_allowed()"
    )]
    pub allowed_curve_ids: CurveIdListConfig,
}

impl Crypto {
    /// Convert this user configuration into the runtime representation.
    #[allow(clippy::too_many_lines)]
    pub fn parse(self, emitter: &mut Emitter<ParseError>) -> actual::Crypto {
        let mut allowed = Vec::new();
        for (idx, value) in self.allowed_signing.into_vec().into_iter().enumerate() {
            match Algorithm::from_str(value.trim()) {
                Ok(algo) => allowed.push(algo),
                Err(_) => emitter.emit(Report::new(ParseError::InvalidCryptoConfig).attach(
                    format!("allowed_signing[{idx}] is not a supported algorithm: {value}"),
                )),
            }
        }

        if allowed.is_empty() {
            emitter.emit(ParseError::InvalidCryptoConfig.into());
            allowed = defaults::crypto::allowed_signing();
        }

        if !allowed.contains(&iroha_crypto::Algorithm::Ed25519) {
            emitter.emit(
                Report::new(ParseError::InvalidCryptoConfig)
                    .attach("allowed_signing must include ed25519 for control-plane operations"),
            );
            allowed.push(iroha_crypto::Algorithm::Ed25519);
        }

        allowed.sort();
        allowed.dedup();

        let has_sm2 = allowed
            .iter()
            .any(|algo| algo.as_static_str().eq_ignore_ascii_case("sm2"));

        if has_sm2 && !cfg!(feature = "sm") {
            emitter.emit(
                Report::new(ParseError::InvalidCryptoConfig)
                    .attach("allowed_signing includes `sm2`, but this build lacks SM support"),
            );
        }

        let default_hash = if self.default_hash.trim().is_empty() {
            emitter.emit(ParseError::InvalidCryptoConfig.into());
            defaults::crypto::default_hash()
        } else {
            self.default_hash
        };

        if has_sm2 {
            if !default_hash.eq_ignore_ascii_case("sm3-256") {
                emitter.emit(
                    Report::new(ParseError::InvalidCryptoConfig)
                        .attach("allowed_signing includes `sm2`, default_hash must be `sm3-256`"),
                );
            }
        } else if default_hash.eq_ignore_ascii_case("sm3-256") {
            emitter.emit(
                Report::new(ParseError::InvalidCryptoConfig)
                    .attach("default_hash `sm3-256` requires allowed_signing to include `sm2`"),
            );
        }

        let sm2_distid_default = if self.sm2_distid_default.trim().is_empty() {
            emitter.emit(ParseError::InvalidCryptoConfig.into());
            defaults::crypto::sm2_distid_default()
        } else {
            self.sm2_distid_default
        };

        if has_sm2 && sm2_distid_default.trim().is_empty() {
            emitter.emit(
                Report::new(ParseError::InvalidCryptoConfig)
                    .attach("sm2_distid_default must be non-empty when `sm2` signing is enabled"),
            );
        }

        #[allow(clippy::let_and_return)]
        let enable_sm_openssl_preview = if self.enable_sm_openssl_preview {
            if cfg!(feature = "sm-ffi-openssl") {
                true
            } else {
                emitter.emit(Report::new(ParseError::InvalidCryptoConfig).attach(
                    "`crypto.enable_sm_openssl_preview` requires building with the \
                         `sm-ffi-openssl` feature",
                ));
                false
            }
        } else {
            false
        };

        let mut allowed_curve_ids = self.curves.allowed_curve_ids.into_vec();
        if allowed_curve_ids.is_empty() {
            allowed_curve_ids = defaults::crypto::derive_curve_ids_from_algorithms(&allowed);
        }
        if allowed_curve_ids.is_empty() {
            emitter.emit(
                Report::new(ParseError::InvalidCryptoConfig)
                    .attach("crypto.curves.allowed_curve_ids resolved to an empty set; include ed25519 (curve id 0x01)"),
            );
            allowed_curve_ids = defaults::crypto::allowed_curve_ids();
        }
        allowed_curve_ids.sort_unstable();
        allowed_curve_ids.dedup();
        for id in &allowed_curve_ids {
            match CurveId::try_from(*id) {
                Ok(curve) => {
                    let algo = curve.algorithm();
                    if !allowed.contains(&algo) {
                        emitter.emit(Report::new(ParseError::InvalidCryptoConfig).attach(format!(
                            "crypto.curves.allowed_curve_ids includes {id:#04X} ({}) \
                                     but allowed_signing omits the matching algorithm",
                            algo.as_static_str()
                        )));
                    }
                }
                Err(err) => emitter
                    .emit(Report::new(ParseError::InvalidCryptoConfig).attach(format!(
                    "crypto.curves.allowed_curve_ids contains unknown identifier {id:#04X}: {err}"
                ))),
            }
        }

        let sm_intrinsics = self.sm_intrinsics;
        if matches!(sm_intrinsics, SmIntrinsicsPolicyConfig::ForceEnable) && !cfg!(feature = "sm") {
            emitter.emit(Report::new(ParseError::InvalidCryptoConfig).attach(
                "`crypto.sm_intrinsics = force-enable` requires building with the `sm` feature",
            ));
        }

        actual::Crypto {
            enable_sm_openssl_preview,
            sm_intrinsics: sm_intrinsics.into(),
            default_hash,
            allowed_signing: allowed,
            sm2_distid_default,
            allowed_curve_ids,
        }
    }
}

/// User-level configuration container for `Nexus`.
#[derive(Debug, Clone, ReadConfig, norito::JsonDeserialize)]
pub struct Nexus {
    /// Enable multilane (Nexus/Iroha3) consensus features.
    #[config(default = "defaults::nexus::ENABLED")]
    pub enabled: bool,
    /// Storage budget controls for Nexus-enabled nodes.
    #[config(nested)]
    pub storage: NexusStorage,
    /// Total number of lanes configured for the runtime.
    #[config(default = "defaults::nexus::LANE_COUNT")]
    pub lane_count: NonZeroU32,
    /// Optional explicit lane catalog entries.
    #[config(default)]
    pub lane_catalog: Vec<LaneDescriptor>,
    /// Optional data-space catalog entries.
    #[config(default)]
    pub dataspace_catalog: Vec<DataSpaceDescriptor>,
    /// Public-lane staking guardrails.
    #[config(nested)]
    pub staking: NexusStaking,
    /// Universal Nexus fee schedule.
    #[config(nested)]
    pub fees: NexusFees,
    /// Shared Hugging Face lease policy.
    #[config(nested)]
    pub hf_shared_leases: NexusHfSharedLeases,
    /// Uploaded private-model quota policy.
    #[config(nested)]
    pub uploaded_models: NexusUploadedModels,
    /// Domain endorsement controls.
    #[config(nested)]
    pub endorsement: NexusEndorsement,
    /// AXT execution and expiry policy configuration.
    #[config(nested)]
    pub axt: NexusAxt,
    /// Lane-relay emergency override configuration.
    #[config(nested)]
    pub lane_relay_emergency: LaneRelayEmergency,
    /// Lane routing policy configuration.
    #[config(default)]
    pub routing_policy: RoutingPolicy,
    /// Lane manifest registry configuration.
    #[config(nested)]
    pub registry: LaneRegistryConfig,
    /// Governance module catalog.
    #[config(nested)]
    pub governance: GovernanceCatalogConfig,
    /// Lane compliance policy configuration.
    #[config(nested)]
    pub compliance: LaneCompliance,
    /// Nexus lane-fusion tuning.
    #[config(nested)]
    pub fusion: Fusion,
    /// Deterministic lane autoscaling tuning.
    #[config(nested)]
    pub autoscale: Autoscale,
    /// Proof/commit deadline configuration.
    #[config(nested)]
    pub commit: Commit,
    /// Data-availability sampling configuration.
    #[config(nested)]
    pub da: Da,
}

impl Default for Nexus {
    fn default() -> Self {
        Self {
            enabled: defaults::nexus::ENABLED,
            storage: NexusStorage::default(),
            lane_count: defaults::nexus::LANE_COUNT,
            lane_catalog: Vec::new(),
            dataspace_catalog: Vec::new(),
            staking: NexusStaking::default(),
            fees: NexusFees::default(),
            hf_shared_leases: NexusHfSharedLeases::default(),
            uploaded_models: NexusUploadedModels::default(),
            endorsement: NexusEndorsement::default(),
            axt: NexusAxt::default(),
            lane_relay_emergency: LaneRelayEmergency::default(),
            routing_policy: RoutingPolicy::default(),
            registry: LaneRegistryConfig::default(),
            governance: GovernanceCatalogConfig::default(),
            compliance: LaneCompliance::default(),
            fusion: Fusion::default(),
            autoscale: Autoscale::default(),
            commit: Commit::default(),
            da: Da::default(),
        }
    }
}

/// User-level configuration container for Nexus storage budgets.
#[derive(Debug, Clone, Copy, ReadConfig, norito::JsonDeserialize)]
pub struct NexusStorage {
    /// Aggregate on-disk storage budget for Nexus-enabled nodes (bytes).
    #[config(default = "defaults::nexus::storage::MAX_DISK_USAGE_BYTES")]
    pub max_disk_usage_bytes: Bytes<u64>,
    /// Block interval between disk budget enforcement scans (0 = every block).
    #[config(default = "defaults::nexus::storage::BUDGET_ENFORCE_INTERVAL_BLOCKS")]
    pub budget_enforce_interval_blocks: u64,
    /// WSV hot-tier deterministic payload size budget (bytes).
    #[config(default = "defaults::nexus::storage::MAX_WSV_MEMORY_BYTES")]
    pub max_wsv_memory_bytes: Bytes<u64>,
    /// Budget weights for dividing the disk cap across subsystems.
    #[config(nested)]
    pub disk_budget_weights: NexusStorageWeights,
}

impl Default for NexusStorage {
    fn default() -> Self {
        Self {
            max_disk_usage_bytes: defaults::nexus::storage::MAX_DISK_USAGE_BYTES,
            budget_enforce_interval_blocks:
                defaults::nexus::storage::BUDGET_ENFORCE_INTERVAL_BLOCKS,
            max_wsv_memory_bytes: defaults::nexus::storage::MAX_WSV_MEMORY_BYTES,
            disk_budget_weights: NexusStorageWeights::default(),
        }
    }
}

impl NexusStorage {
    fn parse(self, emitter: &mut Emitter<ParseError>) -> Option<actual::NexusStorage> {
        let weights = self.disk_budget_weights.parse(emitter)?;
        Some(actual::NexusStorage {
            max_disk_usage_bytes: self.max_disk_usage_bytes,
            budget_enforce_interval_blocks: self.budget_enforce_interval_blocks,
            max_wsv_memory_bytes: self.max_wsv_memory_bytes,
            disk_budget_weights: weights,
        })
    }
}

/// User-level configuration container for Nexus storage budget weights.
#[derive(Debug, Clone, Copy, ReadConfig, norito::JsonDeserialize)]
#[allow(clippy::struct_field_names)]
pub struct NexusStorageWeights {
    /// Budget share for Kura block storage (basis points).
    #[config(default = "defaults::nexus::storage::KURA_BLOCKS_BPS")]
    pub kura_blocks_bps: u16,
    /// Budget share for tiered-state cold snapshots (basis points).
    #[config(default = "defaults::nexus::storage::WSV_SNAPSHOTS_BPS")]
    pub wsv_snapshots_bps: u16,
    /// Budget share for SoraFS storage (basis points).
    #[config(default = "defaults::nexus::storage::SORAFS_BPS")]
    pub sorafs_bps: u16,
    /// Budget share for SoraNet route spools (basis points).
    #[config(default = "defaults::nexus::storage::SORANET_SPOOL_BPS")]
    pub soranet_spool_bps: u16,
    /// Budget share reserved for future SoraVPN storage (basis points).
    #[config(default = "defaults::nexus::storage::SORAVPN_SPOOL_BPS")]
    pub soravpn_spool_bps: u16,
}

impl Default for NexusStorageWeights {
    fn default() -> Self {
        Self {
            kura_blocks_bps: defaults::nexus::storage::KURA_BLOCKS_BPS,
            wsv_snapshots_bps: defaults::nexus::storage::WSV_SNAPSHOTS_BPS,
            sorafs_bps: defaults::nexus::storage::SORAFS_BPS,
            soranet_spool_bps: defaults::nexus::storage::SORANET_SPOOL_BPS,
            soravpn_spool_bps: defaults::nexus::storage::SORAVPN_SPOOL_BPS,
        }
    }
}

impl NexusStorageWeights {
    fn total_bps(&self) -> u32 {
        u32::from(self.kura_blocks_bps)
            + u32::from(self.wsv_snapshots_bps)
            + u32::from(self.sorafs_bps)
            + u32::from(self.soranet_spool_bps)
            + u32::from(self.soravpn_spool_bps)
    }

    fn parse(self, emitter: &mut Emitter<ParseError>) -> Option<actual::NexusStorageWeights> {
        let total = self.total_bps();
        if total != u32::from(defaults::nexus::storage::BPS_TOTAL) {
            emitter.emit(Report::new(ParseError::InvalidNexusConfig).attach(format!(
                "nexus.storage.disk_budget_weights must sum to {} bps (got {total})",
                defaults::nexus::storage::BPS_TOTAL
            )));
            return None;
        }
        Some(actual::NexusStorageWeights {
            kura_blocks_bps: self.kura_blocks_bps,
            wsv_snapshots_bps: self.wsv_snapshots_bps,
            sorafs_bps: self.sorafs_bps,
            soranet_spool_bps: self.soranet_spool_bps,
            soravpn_spool_bps: self.soravpn_spool_bps,
        })
    }
}

/// User-level configuration container for `LaneDescriptor`.
#[derive(Debug, Clone, ReadConfig, Default, norito::JsonDeserialize)]
pub struct LaneDescriptor {
    /// Zero-based lane index within the configured lane count.
    pub index: Option<u32>,
    /// Human-readable alias.
    pub alias: Option<String>,
    /// Optional description for documentation and dashboards.
    pub description: Option<String>,
    /// Dataspace alias this lane belongs to.
    pub dataspace: Option<String>,
    /// Storage profile identifier (`full_replica`, `commitment_only`, `split_replica`).
    pub storage: Option<String>,
    /// Declarative visibility (e.g., `public`, `private`).
    pub visibility: Option<String>,
    /// Optional shard identifier used for DA cursor tracking (defaults to lane id).
    pub shard_id: Option<u32>,
    /// Proof scheme for DA commitments (`merkle_sha256`, `kzg_bls12_381`).
    pub proof_scheme: Option<String>,
    /// Lane profile/type identifier.
    pub lane_type: Option<String>,
    /// Governance policy identifier.
    pub governance: Option<String>,
    /// Settlement/fee policy identifier.
    pub settlement: Option<String>,
    /// Arbitrary metadata key-value pairs for instrumentation.
    #[config(default)]
    pub metadata: BTreeMap<String, String>,
}

/// User-level configuration container for `DataSpaceDescriptor`.
#[derive(Debug, Clone, ReadConfig, Default, norito::JsonDeserialize)]
pub struct DataSpaceDescriptor {
    /// Human-readable alias.
    pub alias: Option<String>,
    /// Explicit numerical identifier override.
    pub id: Option<u64>,
    /// Optional 32-byte hex hash feeding deterministic ID derivation.
    pub manifest_hash: Option<String>,
    /// Optional description for documentation and dashboards.
    pub description: Option<String>,
    /// Fault tolerance value (f) used to size per-dataspace committees (3f + 1).
    /// Must be at least 1.
    pub fault_tolerance: Option<u32>,
}

/// User-level configuration container for lane manifest registry.
#[derive(Debug, Clone, ReadConfig, norito::JsonDeserialize)]
pub struct LaneRegistryConfig {
    /// Optional directory containing lane manifest files.
    pub manifest_directory: Option<PathBuf>,
    /// Optional directory for caching downloaded manifests.
    pub cache_directory: Option<PathBuf>,
    /// Poll interval for refreshing manifests and governance bundles.
    #[config(default = "defaults::nexus::registry::POLL_INTERVAL.into()")]
    pub poll_interval_ms: DurationMs,
}

impl Default for LaneRegistryConfig {
    fn default() -> Self {
        Self {
            manifest_directory: None,
            cache_directory: None,
            poll_interval_ms: defaults::nexus::registry::POLL_INTERVAL.into(),
        }
    }
}

/// User-level configuration container for lane compliance policies.
#[derive(Debug, Clone, ReadConfig, norito::JsonDeserialize)]
pub struct LaneCompliance {
    /// Enable lane-level compliance evaluation.
    #[config(default = "defaults::nexus::compliance::ENABLED")]
    pub enabled: bool,
    /// When true, decisions are logged but not enforced.
    #[config(default = "defaults::nexus::compliance::AUDIT_ONLY")]
    pub audit_only: bool,
    /// Directory holding Norito-encoded policy bundles.
    pub policy_dir: Option<PathBuf>,
}

impl Default for LaneCompliance {
    fn default() -> Self {
        Self {
            enabled: defaults::nexus::compliance::ENABLED,
            audit_only: defaults::nexus::compliance::AUDIT_ONLY,
            policy_dir: None,
        }
    }
}

/// Validator activation policy for a lane (user-level view).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LaneValidatorModeConfig {
    /// Validators are elected through staking.
    #[default]
    StakeElected,
    /// Validators are administered directly (no staking path).
    AdminManaged,
}

#[derive(Debug, Error)]
#[error("invalid lane validator mode `{0}` (expected stake-elected or admin-managed)")]
/// Error returned when parsing a validator mode fails.
pub struct LaneValidatorModeParseError(String);

impl FromStr for LaneValidatorModeConfig {
    type Err = LaneValidatorModeParseError;

    fn from_str(raw: &str) -> std::result::Result<Self, Self::Err> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "stake" | "stake-elected" | "stake_elected" | "staking" => Ok(Self::StakeElected),
            "admin" | "admin-managed" | "admin_managed" | "peer-admin" | "permissioned" => {
                Ok(Self::AdminManaged)
            }
            other => Err(LaneValidatorModeParseError(other.to_owned())),
        }
    }
}

impl json::JsonDeserialize for LaneValidatorModeConfig {
    fn json_deserialize(parser: &mut json::Parser<'_>) -> std::result::Result<Self, json::Error> {
        let value = parser.parse_string()?;
        value.parse().map_err(|_| json::Error::InvalidField {
            field: "nexus.staking.validator_mode".into(),
            message: format!("expected stake-elected or admin-managed, got {value}"),
        })
    }
}

impl json::JsonSerialize for LaneValidatorModeConfig {
    fn json_serialize(&self, out: &mut String) {
        out.push_str(match self {
            Self::StakeElected => "stake_elected",
            Self::AdminManaged => "admin_managed",
        });
    }
}

impl From<LaneValidatorModeConfig> for actual::LaneValidatorMode {
    fn from(mode: LaneValidatorModeConfig) -> Self {
        match mode {
            LaneValidatorModeConfig::StakeElected => Self::StakeElected,
            LaneValidatorModeConfig::AdminManaged => Self::AdminManaged,
        }
    }
}

/// User-level configuration container for public-lane staking.
#[derive(Debug, Clone, ReadConfig, norito::JsonDeserialize)]
pub struct NexusStaking {
    /// Validator activation policy for public lanes.
    #[config(default = "LaneValidatorModeConfig::StakeElected")]
    pub public_validator_mode: LaneValidatorModeConfig,
    /// Validator activation policy for restricted/permissioned lanes.
    #[config(default = "LaneValidatorModeConfig::AdminManaged")]
    pub restricted_validator_mode: LaneValidatorModeConfig,
    /// Minimum bonded stake required to register a validator (asset base units).
    #[config(default = "defaults::nexus::staking::MIN_VALIDATOR_STAKE")]
    pub min_validator_stake: u64,
    /// Maximum number of validators allowed per lane.
    #[config(default = "defaults::nexus::staking::MAX_VALIDATORS")]
    pub max_validators: NonZeroU32,
    /// Minimum delay between scheduling and finalising an unbond (milliseconds).
    #[config(default = "defaults::nexus::staking::UNBONDING_DELAY.into()")]
    pub unbonding_delay_ms: DurationMs,
    /// Grace window after `release_at_ms` during which withdrawals must be finalised (milliseconds).
    #[config(default = "defaults::nexus::staking::WITHDRAW_GRACE.into()")]
    pub withdraw_grace_ms: DurationMs,
    /// Maximum slash ratio allowed (basis points, 10_000 = 100%).
    #[config(default = "defaults::nexus::staking::MAX_SLASH_BPS")]
    pub max_slash_bps: u16,
    /// Minimum reward amount (base units) paid out; smaller amounts are skipped as dust.
    #[config(default = "defaults::nexus::staking::REWARD_DUST_THRESHOLD")]
    pub reward_dust_threshold: u64,
    /// Asset definition used for staking bonds (string form).
    #[config(default = "defaults::nexus::staking::stake_asset_id()")]
    pub stake_asset_id: String,
    /// Escrow account that custodies bonded stake (string form).
    #[config(default = "defaults::nexus::staking::stake_escrow_account_id()")]
    pub stake_escrow_account_id: String,
    /// Account that receives slashed stake (string form).
    #[config(default = "defaults::nexus::staking::slash_sink_account_id()")]
    pub slash_sink_account_id: String,
}

impl Default for NexusStaking {
    fn default() -> Self {
        Self {
            public_validator_mode: LaneValidatorModeConfig::StakeElected,
            restricted_validator_mode: LaneValidatorModeConfig::AdminManaged,
            min_validator_stake: defaults::nexus::staking::MIN_VALIDATOR_STAKE,
            max_validators: defaults::nexus::staking::MAX_VALIDATORS,
            unbonding_delay_ms: defaults::nexus::staking::UNBONDING_DELAY.into(),
            withdraw_grace_ms: defaults::nexus::staking::WITHDRAW_GRACE.into(),
            max_slash_bps: defaults::nexus::staking::MAX_SLASH_BPS,
            reward_dust_threshold: defaults::nexus::staking::REWARD_DUST_THRESHOLD,
            stake_asset_id: defaults::nexus::staking::stake_asset_id(),
            stake_escrow_account_id: defaults::nexus::staking::stake_escrow_account_id(),
            slash_sink_account_id: defaults::nexus::staking::slash_sink_account_id(),
        }
    }
}

impl NexusStaking {
    fn parse(self, emitter: &mut Emitter<ParseError>) -> Option<actual::NexusStaking> {
        if self.max_slash_bps > 10_000 {
            emitter.emit(Report::new(ParseError::InvalidNexusConfig).attach(format!(
                "nexus.staking.max_slash_bps must be <= 10000 (found {})",
                self.max_slash_bps
            )));
            return None;
        }
        Some(actual::NexusStaking {
            public_validator_mode: self.public_validator_mode.into(),
            restricted_validator_mode: self.restricted_validator_mode.into(),
            min_validator_stake: self.min_validator_stake,
            max_validators: self.max_validators,
            unbonding_delay: self.unbonding_delay_ms.get(),
            withdraw_grace: self.withdraw_grace_ms.get(),
            max_slash_bps: self.max_slash_bps,
            reward_dust_threshold: self.reward_dust_threshold,
            stake_asset_id: self.stake_asset_id,
            stake_escrow_account_id: self.stake_escrow_account_id,
            slash_sink_account_id: self.slash_sink_account_id,
        })
    }
}

/// User-level configuration container for Nexus fee schedule.
#[derive(Debug, Clone, ReadConfig, norito::JsonDeserialize)]
pub struct NexusFees {
    /// Fee asset definition identifier (string form).
    #[config(
        env = "NEXUS_FEE_ASSET_ID",
        default = "defaults::nexus::fees::fee_asset_id()"
    )]
    pub fee_asset_id: String,
    /// Account that receives collected fees.
    #[config(
        env = "NEXUS_FEE_SINK_ACCOUNT_ID",
        default = "defaults::nexus::fees::FEE_SINK_ACCOUNT_ID.to_string()"
    )]
    pub fee_sink_account_id: String,
    /// Base fee charged per transaction (asset base units).
    #[config(default = "defaults::nexus::fees::BASE_FEE")]
    pub base_fee: u64,
    /// Per-byte fee charged over the signed transaction payload (asset base units).
    #[config(default = "defaults::nexus::fees::PER_BYTE_FEE")]
    pub per_byte_fee: u64,
    /// Per-instruction fee charged for native ISI batches (asset base units).
    #[config(default = "defaults::nexus::fees::PER_INSTRUCTION_FEE")]
    pub per_instruction_fee: u64,
    /// Per-gas-unit fee multiplier applied to measured gas usage (asset base units).
    #[config(default = "defaults::nexus::fees::PER_GAS_UNIT_FEE")]
    pub per_gas_unit_fee: u64,
    /// Whether fee sponsorship is permitted.
    #[config(default = "defaults::nexus::fees::SPONSORSHIP_ENABLED")]
    pub sponsorship_enabled: bool,
    /// Maximum fee a sponsor can cover per transaction (asset base units, 0 = unlimited).
    #[config(default = "defaults::nexus::fees::SPONSOR_MAX_FEE")]
    pub sponsor_max_fee: u64,
}

/// User-level configuration container for shared Hugging Face lease policy.
#[derive(Debug, Clone, Copy, ReadConfig, norito::JsonDeserialize)]
pub struct NexusHfSharedLeases {
    /// Drain grace window after the last member leaves a shared HF lease pool (milliseconds).
    #[config(
        default = "DurationMs(std::time::Duration::from_millis(defaults::nexus::hf_shared_leases::DRAIN_GRACE_MS))"
    )]
    pub drain_grace_ms: DurationMs,
    /// Slash ratio applied when an assigned host never finishes warmup before expiry.
    #[config(default = "defaults::nexus::hf_shared_leases::WARMUP_NO_SHOW_SLASH_BPS")]
    pub warmup_no_show_slash_bps: u16,
    /// Slash ratio applied when repeated assigned-host heartbeat misses cross the threshold.
    #[config(default = "defaults::nexus::hf_shared_leases::ASSIGNED_HEARTBEAT_MISS_SLASH_BPS")]
    pub assigned_heartbeat_miss_slash_bps: u16,
    /// Strike threshold for assigned-host heartbeat misses within one reservation window.
    #[config(
        default = "defaults::nexus::hf_shared_leases::ASSIGNED_HEARTBEAT_MISS_STRIKE_THRESHOLD"
    )]
    pub assigned_heartbeat_miss_strike_threshold: u32,
    /// Slash ratio applied when a host advert is provably self-contradictory.
    #[config(default = "defaults::nexus::hf_shared_leases::ADVERT_CONTRADICTION_SLASH_BPS")]
    pub advert_contradiction_slash_bps: u16,
}

impl Default for NexusHfSharedLeases {
    fn default() -> Self {
        Self {
            drain_grace_ms: DurationMs(std::time::Duration::from_millis(
                defaults::nexus::hf_shared_leases::DRAIN_GRACE_MS,
            )),
            warmup_no_show_slash_bps: defaults::nexus::hf_shared_leases::WARMUP_NO_SHOW_SLASH_BPS,
            assigned_heartbeat_miss_slash_bps:
                defaults::nexus::hf_shared_leases::ASSIGNED_HEARTBEAT_MISS_SLASH_BPS,
            assigned_heartbeat_miss_strike_threshold:
                defaults::nexus::hf_shared_leases::ASSIGNED_HEARTBEAT_MISS_STRIKE_THRESHOLD,
            advert_contradiction_slash_bps:
                defaults::nexus::hf_shared_leases::ADVERT_CONTRADICTION_SLASH_BPS,
        }
    }
}

impl NexusHfSharedLeases {
    fn parse(self, emitter: &mut Emitter<ParseError>) -> Option<actual::NexusHfSharedLeases> {
        for (field, value) in [
            ("warmup_no_show_slash_bps", self.warmup_no_show_slash_bps),
            (
                "assigned_heartbeat_miss_slash_bps",
                self.assigned_heartbeat_miss_slash_bps,
            ),
            (
                "advert_contradiction_slash_bps",
                self.advert_contradiction_slash_bps,
            ),
        ] {
            if value > 10_000 {
                emitter.emit(Report::new(ParseError::InvalidNexusConfig).attach(format!(
                    "nexus.hf_shared_leases.{field} must be <= 10000 (found {value})"
                )));
                return None;
            }
        }
        if self.assigned_heartbeat_miss_strike_threshold == 0 {
            emitter.emit(
                Report::new(ParseError::InvalidNexusConfig).attach(
                    "nexus.hf_shared_leases.assigned_heartbeat_miss_strike_threshold must be greater than zero"
                        .to_string(),
                ),
            );
            return None;
        }
        Some(actual::NexusHfSharedLeases {
            drain_grace: self.drain_grace_ms.get(),
            warmup_no_show_slash_bps: self.warmup_no_show_slash_bps,
            assigned_heartbeat_miss_slash_bps: self.assigned_heartbeat_miss_slash_bps,
            assigned_heartbeat_miss_strike_threshold: self.assigned_heartbeat_miss_strike_threshold,
            advert_contradiction_slash_bps: self.advert_contradiction_slash_bps,
        })
    }
}

/// User-level configuration container for uploaded private-model quotas.
#[derive(Debug, Clone, Copy, ReadConfig, norito::JsonDeserialize)]
pub struct NexusUploadedModels {
    /// Plaintext chunk size admitted before envelope encryption.
    #[config(default = "defaults::nexus::uploaded_models::CHUNK_PLAINTEXT_BYTES")]
    pub chunk_plaintext_bytes: u64,
    /// Maximum plaintext bytes admitted for one uploaded model.
    #[config(default = "defaults::nexus::uploaded_models::MAX_PLAINTEXT_BYTES_PER_MODEL")]
    pub max_plaintext_bytes_per_model: u64,
    /// Maximum encrypted chunk count admitted for one uploaded model.
    #[config(default = "defaults::nexus::uploaded_models::MAX_CHUNK_COUNT_PER_MODEL")]
    pub max_chunk_count_per_model: u32,
    /// Maximum concurrent private sessions admitted for one apartment.
    #[config(
        default = "defaults::nexus::uploaded_models::MAX_ACTIVE_PRIVATE_SESSIONS_PER_APARTMENT"
    )]
    pub max_active_private_sessions_per_apartment: u32,
    /// Maximum token budget admitted for one private session.
    #[config(default = "defaults::nexus::uploaded_models::MAX_SESSION_TOKEN_BUDGET")]
    pub max_session_token_budget: u32,
    /// Maximum image budget admitted for one private session.
    #[config(default = "defaults::nexus::uploaded_models::MAX_SESSION_IMAGE_BUDGET")]
    pub max_session_image_budget: u16,
}

impl Default for NexusUploadedModels {
    fn default() -> Self {
        Self {
            chunk_plaintext_bytes: defaults::nexus::uploaded_models::CHUNK_PLAINTEXT_BYTES,
            max_plaintext_bytes_per_model:
                defaults::nexus::uploaded_models::MAX_PLAINTEXT_BYTES_PER_MODEL,
            max_chunk_count_per_model: defaults::nexus::uploaded_models::MAX_CHUNK_COUNT_PER_MODEL,
            max_active_private_sessions_per_apartment:
                defaults::nexus::uploaded_models::MAX_ACTIVE_PRIVATE_SESSIONS_PER_APARTMENT,
            max_session_token_budget: defaults::nexus::uploaded_models::MAX_SESSION_TOKEN_BUDGET,
            max_session_image_budget: defaults::nexus::uploaded_models::MAX_SESSION_IMAGE_BUDGET,
        }
    }
}

impl NexusUploadedModels {
    fn parse(self, _emitter: &mut Emitter<ParseError>) -> Option<actual::NexusUploadedModels> {
        Some(actual::NexusUploadedModels {
            chunk_plaintext_bytes: self.chunk_plaintext_bytes,
            max_plaintext_bytes_per_model: self.max_plaintext_bytes_per_model,
            max_chunk_count_per_model: self.max_chunk_count_per_model,
            max_active_private_sessions_per_apartment: self
                .max_active_private_sessions_per_apartment,
            max_session_token_budget: self.max_session_token_budget,
            max_session_image_budget: self.max_session_image_budget,
        })
    }
}

/// User-level configuration container for domain endorsements.
#[derive(Debug, Clone, ReadConfig, norito::JsonDeserialize)]
pub struct NexusEndorsement {
    /// Committee member public keys allowed to sign endorsements (string form).
    #[config(default = "defaults::nexus::endorsement::committee_keys()")]
    pub committee_keys: Vec<String>,
    /// Quorum required to accept an endorsement (0 disables enforcement).
    #[config(default = "defaults::nexus::endorsement::QUORUM")]
    pub quorum: u16,
}

impl Default for NexusEndorsement {
    fn default() -> Self {
        Self {
            committee_keys: defaults::nexus::endorsement::committee_keys(),
            quorum: defaults::nexus::endorsement::QUORUM,
        }
    }
}

impl NexusEndorsement {
    fn parse(self, emitter: &mut Emitter<ParseError>) -> Option<actual::NexusEndorsement> {
        if self.quorum > u16::MAX {
            emitter.emit(Report::new(ParseError::InvalidNexusConfig).attach(format!(
                "nexus.endorsement.quorum exceeds u16 bounds: {}",
                self.quorum
            )));
            return None;
        }

        Some(actual::NexusEndorsement {
            committee_keys: self.committee_keys,
            quorum: self.quorum,
        })
    }
}

impl Default for NexusFees {
    fn default() -> Self {
        Self {
            fee_asset_id: defaults::nexus::fees::fee_asset_id(),
            fee_sink_account_id: defaults::nexus::fees::FEE_SINK_ACCOUNT_ID.to_string(),
            base_fee: defaults::nexus::fees::BASE_FEE,
            per_byte_fee: defaults::nexus::fees::PER_BYTE_FEE,
            per_instruction_fee: defaults::nexus::fees::PER_INSTRUCTION_FEE,
            per_gas_unit_fee: defaults::nexus::fees::PER_GAS_UNIT_FEE,
            sponsorship_enabled: defaults::nexus::fees::SPONSORSHIP_ENABLED,
            sponsor_max_fee: defaults::nexus::fees::SPONSOR_MAX_FEE,
        }
    }
}

impl NexusFees {
    fn parse(self, emitter: &mut Emitter<ParseError>) -> Option<actual::NexusFees> {
        if self.fee_asset_id.trim().is_empty() {
            emitter.emit(
                Report::new(ParseError::InvalidNexusConfig)
                    .attach("nexus.fees.fee_asset_id must not be empty".to_string()),
            );
            return None;
        }
        if self.fee_sink_account_id.trim().is_empty() {
            emitter.emit(
                Report::new(ParseError::InvalidNexusConfig)
                    .attach("nexus.fees.fee_sink_account_id must not be empty".to_string()),
            );
            return None;
        }

        Some(actual::NexusFees {
            fee_asset_id: self.fee_asset_id,
            fee_sink_account_id: self.fee_sink_account_id,
            base_fee: self.base_fee,
            per_byte_fee: self.per_byte_fee,
            per_instruction_fee: self.per_instruction_fee,
            per_gas_unit_fee: self.per_gas_unit_fee,
            sponsorship_enabled: self.sponsorship_enabled,
            sponsor_max_fee: self.sponsor_max_fee,
        })
    }
}

/// User-level configuration container for governance catalog.
#[derive(Debug, Clone, ReadConfig, Default, norito::JsonDeserialize)]
pub struct GovernanceCatalogConfig {
    /// Default governance module identifier applied when lanes omit an override.
    pub default_module: Option<String>,
    /// Registered governance modules keyed by name.
    #[config(default)]
    pub modules: BTreeMap<String, GovernanceModule>,
}

/// User-level configuration for a single governance module.
#[derive(Debug, Clone, ReadConfig, Default, norito::JsonDeserialize)]
pub struct GovernanceModule {
    /// Module type (e.g., `parliament`, `stake_weighted`).
    pub module_type: Option<String>,
    /// Additional parameters defined by the module.
    #[config(default)]
    pub params: BTreeMap<String, String>,
}

/// User-level configuration container for `RoutingPolicy`.
#[derive(Debug, Clone, ReadConfig, Default, norito::JsonDeserialize)]
pub struct RoutingPolicy {
    /// Default lane index used when no rule matches.
    pub default_lane: Option<u32>,
    /// Default dataspace alias used when a rule omits an override.
    pub default_dataspace: Option<String>,
    /// Declarative routing rules inspected in order.
    #[config(default)]
    pub rules: Vec<RoutingRule>,
}

/// User-level configuration container for `RoutingRule`.
#[derive(Debug, Clone, ReadConfig, Default, norito::JsonDeserialize)]
pub struct RoutingRule {
    /// Target lane index for matching transactions.
    pub lane: Option<u32>,
    /// Optional dataspace alias override applied when the rule matches.
    pub dataspace: Option<String>,
    /// Matcher criteria for the rule.
    #[config(default)]
    pub matcher: RouteMatcher,
}

/// User-level configuration container for `RouteMatcher`.
#[derive(Debug, Clone, ReadConfig, Default, norito::JsonDeserialize)]
pub struct RouteMatcher {
    /// Optional authority/account match (exact string).
    pub account: Option<String>,
    /// Optional instruction/path match (string prefix or identifier).
    pub instruction: Option<String>,
    /// Optional descriptive text for dashboards.
    pub description: Option<String>,
}

/// User-level configuration container for `Fusion`.
#[derive(Debug, Clone, Copy, ReadConfig, norito::JsonDeserialize)]
pub struct Fusion {
    /// Sustained TEU floor (per lane) that triggers fusion when demand stays below it.
    #[config(default = "defaults::nexus::fusion::FLOOR_TEU")]
    pub floor_teu: u32,
    /// TEU threshold at which fused lanes must split back to independent operation.
    #[config(default = "defaults::nexus::fusion::EXIT_TEU")]
    pub exit_teu: u32,
    /// Number of consecutive slots that must satisfy the floor condition before fusing.
    #[config(default = "defaults::nexus::fusion::OBSERVATION_SLOTS")]
    pub observation_slots: u16,
    /// Maximum number of slots a fused window can persist without re-evaluating load.
    #[config(default = "defaults::nexus::fusion::MAX_WINDOW_SLOTS")]
    pub max_window_slots: u16,
}

impl Default for Fusion {
    fn default() -> Self {
        Self {
            floor_teu: defaults::nexus::fusion::FLOOR_TEU,
            exit_teu: defaults::nexus::fusion::EXIT_TEU,
            observation_slots: defaults::nexus::fusion::OBSERVATION_SLOTS,
            max_window_slots: defaults::nexus::fusion::MAX_WINDOW_SLOTS,
        }
    }
}

/// User-level configuration container for deterministic lane autoscaling.
#[derive(Debug, Clone, Copy, ReadConfig, norito::JsonDeserialize)]
pub struct Autoscale {
    /// Whether consensus-driven lane autoscaling is enabled.
    #[config(default = "defaults::nexus::autoscale::ENABLED")]
    pub enabled: bool,
    /// Minimum active lane count.
    #[config(default = "defaults::nexus::autoscale::MIN_LANES")]
    pub min_lanes: u32,
    /// Maximum active lane count.
    #[config(default = "defaults::nexus::autoscale::MAX_LANES")]
    pub max_lanes: u32,
    /// Target block interval used by the autoscaler (milliseconds).
    #[config(default = "defaults::nexus::autoscale::TARGET_BLOCK_MS")]
    pub target_block_ms: u64,
    /// Scale-out latency ratio threshold versus target block interval.
    #[config(default = "defaults::nexus::autoscale::SCALE_OUT_LATENCY_RATIO")]
    pub scale_out_latency_ratio: f64,
    /// Scale-in latency ratio threshold versus target block interval.
    #[config(default = "defaults::nexus::autoscale::SCALE_IN_LATENCY_RATIO")]
    pub scale_in_latency_ratio: f64,
    /// Scale-out utilization ratio threshold.
    #[config(default = "defaults::nexus::autoscale::SCALE_OUT_UTILIZATION_RATIO")]
    pub scale_out_utilization_ratio: f64,
    /// Scale-in utilization ratio threshold.
    #[config(default = "defaults::nexus::autoscale::SCALE_IN_UTILIZATION_RATIO")]
    pub scale_in_utilization_ratio: f64,
    /// Number of recent blocks used for scale-out decisions.
    #[config(default = "defaults::nexus::autoscale::SCALE_OUT_WINDOW_BLOCKS")]
    pub scale_out_window_blocks: u16,
    /// Number of recent blocks used for scale-in decisions.
    #[config(default = "defaults::nexus::autoscale::SCALE_IN_WINDOW_BLOCKS")]
    pub scale_in_window_blocks: u16,
    /// Cooldown period in blocks after each transition.
    #[config(default = "defaults::nexus::autoscale::COOLDOWN_BLOCKS")]
    pub cooldown_blocks: u16,
    /// Per-lane throughput target used to compute utilization (tx/s).
    #[config(default = "defaults::nexus::autoscale::PER_LANE_TARGET_TPS")]
    pub per_lane_target_tps: u32,
}

impl Default for Autoscale {
    fn default() -> Self {
        Self {
            enabled: defaults::nexus::autoscale::ENABLED,
            min_lanes: defaults::nexus::autoscale::MIN_LANES,
            max_lanes: defaults::nexus::autoscale::MAX_LANES,
            target_block_ms: defaults::nexus::autoscale::TARGET_BLOCK_MS,
            scale_out_latency_ratio: defaults::nexus::autoscale::SCALE_OUT_LATENCY_RATIO,
            scale_in_latency_ratio: defaults::nexus::autoscale::SCALE_IN_LATENCY_RATIO,
            scale_out_utilization_ratio: defaults::nexus::autoscale::SCALE_OUT_UTILIZATION_RATIO,
            scale_in_utilization_ratio: defaults::nexus::autoscale::SCALE_IN_UTILIZATION_RATIO,
            scale_out_window_blocks: defaults::nexus::autoscale::SCALE_OUT_WINDOW_BLOCKS,
            scale_in_window_blocks: defaults::nexus::autoscale::SCALE_IN_WINDOW_BLOCKS,
            cooldown_blocks: defaults::nexus::autoscale::COOLDOWN_BLOCKS,
            per_lane_target_tps: defaults::nexus::autoscale::PER_LANE_TARGET_TPS,
        }
    }
}

/// User-level configuration container for `Commit`.
#[derive(Debug, Clone, Copy, ReadConfig, norito::JsonDeserialize)]
pub struct Commit {
    /// Proof/commit deadline (Δ) expressed in slots.
    #[config(default = "defaults::nexus::commit::WINDOW_SLOTS")]
    pub window_slots: u16,
}

impl Default for Commit {
    fn default() -> Self {
        Self {
            window_slots: defaults::nexus::commit::WINDOW_SLOTS,
        }
    }
}

/// User-level configuration container for `Da`.
#[derive(Debug, Clone, ReadConfig, norito::JsonDeserialize)]
pub struct Da {
    /// Total in-slot DA signatures budget per lane.
    #[config(default = "defaults::nexus::da::Q_IN_SLOT_TOTAL")]
    pub q_in_slot_total: u32,
    /// Minimum in-slot DA signatures per dataspace.
    #[config(default = "defaults::nexus::da::Q_IN_SLOT_PER_DS_MIN")]
    pub q_in_slot_per_ds_min: u16,
    /// Baseline attester sample size (S).
    #[config(default = "defaults::nexus::da::SAMPLE_SIZE_BASE")]
    pub sample_size_base: u16,
    /// Maximum attester sample size when scaling up coverage.
    #[config(default = "defaults::nexus::da::SAMPLE_SIZE_MAX")]
    pub sample_size_max: u16,
    /// Threshold `T` applied off-path for DA certificates.
    #[config(default = "defaults::nexus::da::THRESHOLD_BASE")]
    pub threshold_base: u16,
    /// Number of shards each attester must verify per slot.
    #[config(default = "defaults::nexus::da::PER_ATTESTER_SHARDS")]
    pub per_attester_shards: u16,
    /// Rolling audit configuration.
    #[config(nested)]
    pub audit: DaAudit,
    /// Recovery deadline configuration.
    #[config(nested)]
    pub recovery: DaRecovery,
    /// Temporal diversity / attester rotation configuration.
    #[config(nested)]
    pub rotation: DaRotation,
}

impl Default for Da {
    fn default() -> Self {
        Self {
            q_in_slot_total: defaults::nexus::da::Q_IN_SLOT_TOTAL,
            q_in_slot_per_ds_min: defaults::nexus::da::Q_IN_SLOT_PER_DS_MIN,
            sample_size_base: defaults::nexus::da::SAMPLE_SIZE_BASE,
            sample_size_max: defaults::nexus::da::SAMPLE_SIZE_MAX,
            threshold_base: defaults::nexus::da::THRESHOLD_BASE,
            per_attester_shards: defaults::nexus::da::PER_ATTESTER_SHARDS,
            audit: DaAudit::default(),
            recovery: DaRecovery::default(),
            rotation: DaRotation::default(),
        }
    }
}

/// User-level configuration container for `DaAudit`.
#[derive(Debug, Clone, Copy, ReadConfig, norito::JsonDeserialize)]
pub struct DaAudit {
    /// Signatures verified per audit window.
    #[config(default = "defaults::nexus::da::audit::SAMPLE_SIZE")]
    pub sample_size: u16,
    /// Number of audit windows retained before slashing for insufficient coverage.
    #[config(default = "defaults::nexus::da::audit::WINDOW_COUNT")]
    pub window_count: u16,
    /// Duration of an audit window.
    #[config(default = "defaults::nexus::da::audit::INTERVAL.into()")]
    pub interval_ms: DurationMs,
}

impl Default for DaAudit {
    fn default() -> Self {
        Self {
            sample_size: defaults::nexus::da::audit::SAMPLE_SIZE,
            window_count: defaults::nexus::da::audit::WINDOW_COUNT,
            interval_ms: defaults::nexus::da::audit::INTERVAL.into(),
        }
    }
}

/// User-level configuration container for `DaRecovery`.
#[derive(Debug, Clone, Copy, ReadConfig, norito::JsonDeserialize)]
pub struct DaRecovery {
    /// Deadline for providing recovery proofs once requested.
    #[config(default = "defaults::nexus::da::recovery::REQUEST_TIMEOUT.into()")]
    pub request_timeout_ms: DurationMs,
}

impl Default for DaRecovery {
    fn default() -> Self {
        Self {
            request_timeout_ms: defaults::nexus::da::recovery::REQUEST_TIMEOUT.into(),
        }
    }
}

/// User-level configuration container for `DaRotation`.
#[derive(Debug, Clone, ReadConfig, norito::JsonDeserialize)]
pub struct DaRotation {
    /// Maximum appearances of an attester inside the rolling window.
    #[config(default = "defaults::nexus::da::rotation::MAX_HITS_PER_WINDOW")]
    pub max_hits_per_window: u16,
    /// Rolling window length (slots) for temporal diversity enforcement.
    #[config(default = "defaults::nexus::da::rotation::WINDOW_SLOTS")]
    pub window_slots: u16,
    /// Domain-separation tag for deterministic rotation seed derivation.
    #[config(default = "defaults::nexus::da::rotation::SEED_TAG.to_string()")]
    pub seed_tag: String,
    /// Latency-bias decay factor applied to attester weights.
    #[config(default = "defaults::nexus::da::rotation::LATENCY_DECAY")]
    pub latency_decay: f64,
}

impl Default for DaRotation {
    fn default() -> Self {
        Self {
            max_hits_per_window: defaults::nexus::da::rotation::MAX_HITS_PER_WINDOW,
            window_slots: defaults::nexus::da::rotation::WINDOW_SLOTS,
            seed_tag: defaults::nexus::da::rotation::SEED_TAG.to_string(),
            latency_decay: defaults::nexus::da::rotation::LATENCY_DECAY,
        }
    }
}

impl Fusion {
    fn parse(self, emitter: &mut Emitter<ParseError>) -> Option<actual::Fusion> {
        let Fusion {
            floor_teu,
            exit_teu,
            observation_slots,
            max_window_slots,
        } = self;

        let mut invalid = false;

        if floor_teu == 0 {
            invalid = true;
            emitter.emit(
                Report::new(ParseError::InvalidNexusConfig)
                    .attach("nexus.fusion.floor_teu must be > 0"),
            );
        }

        if floor_teu >= exit_teu {
            invalid = true;
            emitter.emit(
                Report::new(ParseError::InvalidNexusConfig)
                    .attach("nexus.fusion.exit_teu must be greater than fusion.floor_teu"),
            );
        }

        let observation_slots = NonZeroU16::new(observation_slots).map_or_else(
            || {
                invalid = true;
                emitter.emit(
                    Report::new(ParseError::InvalidNexusConfig)
                        .attach("nexus.fusion.observation_slots must be > 0"),
                );
                None
            },
            Some,
        );

        let max_window_slots = NonZeroU16::new(max_window_slots).map_or_else(
            || {
                invalid = true;
                emitter.emit(
                    Report::new(ParseError::InvalidNexusConfig)
                        .attach("nexus.fusion.max_window_slots must be > 0"),
                );
                None
            },
            Some,
        );

        if let (Some(obs), Some(max)) = (observation_slots, max_window_slots)
            && max < obs
        {
            invalid = true;
            emitter.emit(
                Report::new(ParseError::InvalidNexusConfig)
                    .attach("nexus.fusion.max_window_slots must be >= fusion.observation_slots"),
            );
        }

        if invalid {
            return None;
        }

        Some(actual::Fusion {
            floor_teu,
            exit_teu,
            observation_slots: observation_slots.expect("validated"),
            max_window_slots: max_window_slots.expect("validated"),
        })
    }
}

impl Autoscale {
    fn parse(self, emitter: &mut Emitter<ParseError>) -> Option<actual::Autoscale> {
        let Autoscale {
            enabled,
            min_lanes,
            max_lanes,
            target_block_ms,
            scale_out_latency_ratio,
            scale_in_latency_ratio,
            scale_out_utilization_ratio,
            scale_in_utilization_ratio,
            scale_out_window_blocks,
            scale_in_window_blocks,
            cooldown_blocks,
            per_lane_target_tps,
        } = self;

        let mut invalid = false;

        let min_lanes = NonZeroU32::new(min_lanes).or_else(|| {
            invalid = true;
            emitter.emit(
                Report::new(ParseError::InvalidNexusConfig)
                    .attach("nexus.autoscale.min_lanes must be > 0"),
            );
            None
        });
        let max_lanes = NonZeroU32::new(max_lanes).or_else(|| {
            invalid = true;
            emitter.emit(
                Report::new(ParseError::InvalidNexusConfig)
                    .attach("nexus.autoscale.max_lanes must be > 0"),
            );
            None
        });
        let target_block_ms = NonZeroU64::new(target_block_ms).or_else(|| {
            invalid = true;
            emitter.emit(
                Report::new(ParseError::InvalidNexusConfig)
                    .attach("nexus.autoscale.target_block_ms must be > 0"),
            );
            None
        });
        let scale_out_window_blocks = NonZeroU16::new(scale_out_window_blocks).or_else(|| {
            invalid = true;
            emitter.emit(
                Report::new(ParseError::InvalidNexusConfig)
                    .attach("nexus.autoscale.scale_out_window_blocks must be > 0"),
            );
            None
        });
        let scale_in_window_blocks = NonZeroU16::new(scale_in_window_blocks).or_else(|| {
            invalid = true;
            emitter.emit(
                Report::new(ParseError::InvalidNexusConfig)
                    .attach("nexus.autoscale.scale_in_window_blocks must be > 0"),
            );
            None
        });
        let cooldown_blocks = NonZeroU16::new(cooldown_blocks).or_else(|| {
            invalid = true;
            emitter.emit(
                Report::new(ParseError::InvalidNexusConfig)
                    .attach("nexus.autoscale.cooldown_blocks must be > 0"),
            );
            None
        });
        let per_lane_target_tps = NonZeroU32::new(per_lane_target_tps).or_else(|| {
            invalid = true;
            emitter.emit(
                Report::new(ParseError::InvalidNexusConfig)
                    .attach("nexus.autoscale.per_lane_target_tps must be > 0"),
            );
            None
        });

        let ratios_in_range = |value: f64| value.is_finite() && value > 0.0;
        if !ratios_in_range(scale_out_latency_ratio) {
            invalid = true;
            emitter.emit(
                Report::new(ParseError::InvalidNexusConfig)
                    .attach("nexus.autoscale.scale_out_latency_ratio must be finite and > 0"),
            );
        }
        if !ratios_in_range(scale_in_latency_ratio) {
            invalid = true;
            emitter.emit(
                Report::new(ParseError::InvalidNexusConfig)
                    .attach("nexus.autoscale.scale_in_latency_ratio must be finite and > 0"),
            );
        }
        if !ratios_in_range(scale_out_utilization_ratio) {
            invalid = true;
            emitter.emit(
                Report::new(ParseError::InvalidNexusConfig)
                    .attach("nexus.autoscale.scale_out_utilization_ratio must be finite and > 0"),
            );
        }
        if !ratios_in_range(scale_in_utilization_ratio) {
            invalid = true;
            emitter.emit(
                Report::new(ParseError::InvalidNexusConfig)
                    .attach("nexus.autoscale.scale_in_utilization_ratio must be finite and > 0"),
            );
        }

        if let (Some(min), Some(max)) = (min_lanes, max_lanes) {
            if min > max {
                invalid = true;
                emitter.emit(
                    Report::new(ParseError::InvalidNexusConfig)
                        .attach("nexus.autoscale.min_lanes must be <= max_lanes"),
                );
            }
            if max.get() > defaults::nexus::autoscale::MAX_LANES {
                invalid = true;
                emitter.emit(Report::new(ParseError::InvalidNexusConfig).attach(format!(
                    "nexus.autoscale.max_lanes must be <= {}",
                    defaults::nexus::autoscale::MAX_LANES
                )));
            }
        }

        if scale_in_latency_ratio >= scale_out_latency_ratio {
            invalid = true;
            emitter.emit(Report::new(ParseError::InvalidNexusConfig).attach(
                "nexus.autoscale.scale_in_latency_ratio must be < scale_out_latency_ratio",
            ));
        }
        if scale_in_utilization_ratio >= scale_out_utilization_ratio {
            invalid = true;
            emitter.emit(Report::new(ParseError::InvalidNexusConfig).attach(
                "nexus.autoscale.scale_in_utilization_ratio must be < scale_out_utilization_ratio",
            ));
        }

        if invalid {
            return None;
        }

        Some(actual::Autoscale {
            enabled,
            min_lanes: min_lanes.expect("validated"),
            max_lanes: max_lanes.expect("validated"),
            target_block_ms: target_block_ms.expect("validated"),
            scale_out_latency_ratio,
            scale_in_latency_ratio,
            scale_out_utilization_ratio,
            scale_in_utilization_ratio,
            scale_out_window_blocks: scale_out_window_blocks.expect("validated"),
            scale_in_window_blocks: scale_in_window_blocks.expect("validated"),
            cooldown_blocks: cooldown_blocks.expect("validated"),
            per_lane_target_tps: per_lane_target_tps.expect("validated"),
            last_transition_height: 0,
        })
    }
}

impl Commit {
    fn parse(self, emitter: &mut Emitter<ParseError>) -> Option<actual::Commit> {
        let Some(window_slots) = NonZeroU16::new(self.window_slots) else {
            emitter.emit(
                Report::new(ParseError::InvalidNexusConfig)
                    .attach("nexus.commit.window_slots must be > 0"),
            );
            return None;
        };

        Some(actual::Commit { window_slots })
    }
}

impl Da {
    #[allow(clippy::too_many_lines)]
    fn parse(self, emitter: &mut Emitter<ParseError>) -> Option<actual::Da> {
        let mut invalid = false;

        let q_in_slot_total = NonZeroU32::new(self.q_in_slot_total).or_else(|| {
            invalid = true;
            emitter.emit(
                Report::new(ParseError::InvalidNexusConfig)
                    .attach("nexus.da.q_in_slot_total must be > 0"),
            );
            None
        });
        let q_in_slot_per_ds_min = NonZeroU16::new(self.q_in_slot_per_ds_min).or_else(|| {
            invalid = true;
            emitter.emit(
                Report::new(ParseError::InvalidNexusConfig)
                    .attach("nexus.da.q_in_slot_per_ds_min must be > 0"),
            );
            None
        });
        let sample_size_base = NonZeroU16::new(self.sample_size_base).or_else(|| {
            invalid = true;
            emitter.emit(
                Report::new(ParseError::InvalidNexusConfig)
                    .attach("nexus.da.sample_size_base must be > 0"),
            );
            None
        });
        let sample_size_max = NonZeroU16::new(self.sample_size_max).or_else(|| {
            invalid = true;
            emitter.emit(
                Report::new(ParseError::InvalidNexusConfig)
                    .attach("nexus.da.sample_size_max must be > 0"),
            );
            None
        });
        let threshold_base = NonZeroU16::new(self.threshold_base).or_else(|| {
            invalid = true;
            emitter.emit(
                Report::new(ParseError::InvalidNexusConfig)
                    .attach("nexus.da.threshold_base must be > 0"),
            );
            None
        });
        let per_attester_shards = NonZeroU16::new(self.per_attester_shards).or_else(|| {
            invalid = true;
            emitter.emit(
                Report::new(ParseError::InvalidNexusConfig)
                    .attach("nexus.da.per_attester_shards must be > 0"),
            );
            None
        });

        if let (Some(base), Some(max)) = (&sample_size_base, &sample_size_max)
            && base > max
        {
            invalid = true;
            emitter.emit(
                Report::new(ParseError::InvalidNexusConfig)
                    .attach("nexus.da.sample_size_base must be <= nexus.da.sample_size_max"),
            );
        }

        if let (Some(threshold), Some(base)) = (&threshold_base, &sample_size_base)
            && threshold > base
        {
            invalid = true;
            emitter.emit(
                Report::new(ParseError::InvalidNexusConfig)
                    .attach("nexus.da.threshold_base must be <= nexus.da.sample_size_base"),
            );
        }

        if let (Some(total), Some(min_per_ds)) = (&q_in_slot_total, &q_in_slot_per_ds_min)
            && total.get() < u32::from(min_per_ds.get())
        {
            invalid = true;
            emitter.emit(
                Report::new(ParseError::InvalidNexusConfig)
                    .attach("nexus.da.q_in_slot_total must be >= nexus.da.q_in_slot_per_ds_min"),
            );
        }

        let audit = self.audit.parse(emitter);
        let recovery = self.recovery.parse(emitter);
        let rotation = self.rotation.parse(emitter);

        if let (Some(audit), Some(max)) = (&audit, &sample_size_max)
            && audit.sample_size > *max
        {
            invalid = true;
            emitter.emit(
                Report::new(ParseError::InvalidNexusConfig)
                    .attach("nexus.da.audit.sample_size must be <= nexus.da.sample_size_max"),
            );
        }

        if invalid
            || q_in_slot_total.is_none()
            || q_in_slot_per_ds_min.is_none()
            || sample_size_base.is_none()
            || sample_size_max.is_none()
            || threshold_base.is_none()
            || per_attester_shards.is_none()
            || audit.is_none()
            || recovery.is_none()
            || rotation.is_none()
        {
            return None;
        }

        Some(actual::Da {
            q_in_slot_total: q_in_slot_total.expect("validated"),
            q_in_slot_per_ds_min: q_in_slot_per_ds_min.expect("validated"),
            sample_size_base: sample_size_base.expect("validated"),
            sample_size_max: sample_size_max.expect("validated"),
            threshold_base: threshold_base.expect("validated"),
            per_attester_shards: per_attester_shards.expect("validated"),
            audit: audit.expect("validated"),
            recovery: recovery.expect("validated"),
            rotation: rotation.expect("validated"),
        })
    }
}

impl DaAudit {
    fn parse(self, emitter: &mut Emitter<ParseError>) -> Option<actual::DaAudit> {
        let mut invalid = false;

        let sample_size = NonZeroU16::new(self.sample_size).or_else(|| {
            invalid = true;
            emitter.emit(
                Report::new(ParseError::InvalidNexusConfig)
                    .attach("nexus.da.audit.sample_size must be > 0"),
            );
            None
        });
        let window_count = NonZeroU16::new(self.window_count).or_else(|| {
            invalid = true;
            emitter.emit(
                Report::new(ParseError::InvalidNexusConfig)
                    .attach("nexus.da.audit.window_count must be > 0"),
            );
            None
        });

        let interval = self.interval_ms.get();
        if interval.is_zero() {
            invalid = true;
            emitter.emit(
                Report::new(ParseError::InvalidNexusConfig)
                    .attach("nexus.da.audit.interval_ms must be > 0"),
            );
        }

        if invalid || sample_size.is_none() || window_count.is_none() {
            return None;
        }

        Some(actual::DaAudit {
            sample_size: sample_size.expect("validated"),
            window_count: window_count.expect("validated"),
            interval,
        })
    }
}

impl DaRecovery {
    fn parse(self, emitter: &mut Emitter<ParseError>) -> Option<actual::DaRecovery> {
        let timeout = self.request_timeout_ms.get();
        if timeout.is_zero() {
            emitter.emit(
                Report::new(ParseError::InvalidNexusConfig)
                    .attach("nexus.da.recovery.request_timeout_ms must be > 0"),
            );
            return None;
        }

        Some(actual::DaRecovery {
            request_timeout: timeout,
        })
    }
}

impl DaRotation {
    fn parse(self, emitter: &mut Emitter<ParseError>) -> Option<actual::DaRotation> {
        let mut invalid = false;

        let max_hits_per_window = NonZeroU16::new(self.max_hits_per_window).or_else(|| {
            invalid = true;
            emitter.emit(
                Report::new(ParseError::InvalidNexusConfig)
                    .attach("nexus.da.rotation.max_hits_per_window must be > 0"),
            );
            None
        });
        let window_slots = NonZeroU16::new(self.window_slots).or_else(|| {
            invalid = true;
            emitter.emit(
                Report::new(ParseError::InvalidNexusConfig)
                    .attach("nexus.da.rotation.window_slots must be > 0"),
            );
            None
        });

        if self.seed_tag.trim().is_empty() {
            invalid = true;
            emitter.emit(
                Report::new(ParseError::InvalidNexusConfig)
                    .attach("nexus.da.rotation.seed_tag must be non-empty"),
            );
        }

        if !self.latency_decay.is_finite() || !(0.0..=1.0).contains(&self.latency_decay) {
            invalid = true;
            emitter.emit(
                Report::new(ParseError::InvalidNexusConfig)
                    .attach("nexus.da.rotation.latency_decay must be finite and within [0,1]"),
            );
        }

        if let (Some(max_hits), Some(window)) = (&max_hits_per_window, &window_slots)
            && max_hits.get() > window.get()
        {
            invalid = true;
            emitter.emit(
                Report::new(ParseError::InvalidNexusConfig).attach(
                    "nexus.da.rotation.max_hits_per_window must be <= rotation.window_slots",
                ),
            );
        }

        if invalid || max_hits_per_window.is_none() || window_slots.is_none() {
            return None;
        }

        Some(actual::DaRotation {
            max_hits_per_window: max_hits_per_window.expect("validated"),
            window_slots: window_slots.expect("validated"),
            seed_tag: self.seed_tag,
            latency_decay: self.latency_decay,
        })
    }
}

impl LaneRegistryConfig {
    fn parse(self, emitter: &mut Emitter<ParseError>) -> Option<actual::LaneRegistry> {
        let poll_interval = self.poll_interval_ms.get();
        if poll_interval.is_zero() {
            emitter.emit(
                Report::new(ParseError::InvalidNexusConfig)
                    .attach("nexus.registry.poll_interval_ms must be > 0"),
            );
            return None;
        }

        Some(actual::LaneRegistry {
            manifest_directory: self.manifest_directory,
            cache_directory: self.cache_directory,
            poll_interval,
        })
    }
}

impl LaneCompliance {
    fn parse(self) -> actual::LaneCompliance {
        actual::LaneCompliance {
            enabled: self.enabled,
            audit_only: self.audit_only,
            policy_dir: self.policy_dir,
        }
    }
}

impl GovernanceCatalogConfig {
    fn parse(self, emitter: &mut Emitter<ParseError>) -> Option<actual::GovernanceCatalog> {
        let mut invalid = false;
        let default_module = Nexus::normalize_opt(self.default_module);
        let mut modules = BTreeMap::new();

        for (raw_name, module_cfg) in self.modules {
            let name_trimmed = raw_name.trim();
            if name_trimmed.is_empty() {
                invalid = true;
                emitter.emit(
                    Report::new(ParseError::InvalidNexusConfig)
                        .attach("nexus.governance.modules entry name must not be empty"),
                );
                continue;
            }
            let name = name_trimmed.to_string();
            if modules.contains_key(&name) {
                invalid = true;
                emitter.emit(Report::new(ParseError::InvalidNexusConfig).attach(format!(
                    "nexus.governance.modules contains duplicate module \"{name}\""
                )));
                continue;
            }

            let module_type = Nexus::normalize_opt(module_cfg.module_type);
            let mut params = BTreeMap::new();
            for (raw_key, raw_value) in module_cfg.params {
                let key_trimmed = raw_key.trim();
                if key_trimmed.is_empty() {
                    invalid = true;
                    emitter.emit(Report::new(ParseError::InvalidNexusConfig).attach(format!(
                        "nexus.governance.modules[{name}] parameter name must not be empty"
                    )));
                    continue;
                }
                let key = key_trimmed.to_string();
                if params.contains_key(&key) {
                    invalid = true;
                    emitter.emit(Report::new(ParseError::InvalidNexusConfig).attach(format!(
                        "nexus.governance.modules[{name}] duplicates parameter \"{key}\""
                    )));
                    continue;
                }
                params.insert(key, raw_value.trim().to_string());
            }

            modules.insert(
                name.clone(),
                actual::GovernanceModule {
                    module_type,
                    params,
                },
            );
        }

        if let Some(default) = default_module.as_ref()
            && !modules.contains_key(default)
        {
            invalid = true;
            emitter.emit(Report::new(ParseError::InvalidNexusConfig).attach(format!(
                "nexus.governance.default_module \"{default}\" must reference a defined module"
            )));
        }

        if invalid {
            return None;
        }

        Some(actual::GovernanceCatalog {
            default_module,
            modules,
        })
    }
}

/// User-level configuration container for AXT execution and expiry policy.
#[derive(Debug, Clone, Copy, ReadConfig, norito::JsonDeserialize)]
pub struct NexusAxt {
    /// Slot length (milliseconds) used when deriving AXT expiry slots from block timestamps.
    #[config(default = "defaults::nexus::axt::SLOT_LENGTH_MS")]
    pub slot_length_ms: u64,
    /// Maximum tolerated wall-clock skew (milliseconds) applied to AXT expiry checks.
    #[config(default = "defaults::nexus::axt::CLOCK_SKEW_MS_DEFAULT")]
    pub max_clock_skew_ms: u64,
    /// Number of slots to retain cached AXT proofs for reuse and replay rejection.
    #[config(default = "defaults::nexus::axt::PROOF_CACHE_TTL_SLOTS")]
    pub proof_cache_ttl_slots: u64,
    /// Number of slots to retain handle usage for replay protection across restarts/peers.
    #[config(default = "defaults::nexus::axt::REPLAY_RETENTION_SLOTS")]
    pub replay_retention_slots: u64,
}

impl Default for NexusAxt {
    fn default() -> Self {
        Self {
            slot_length_ms: defaults::nexus::axt::SLOT_LENGTH_MS,
            max_clock_skew_ms: defaults::nexus::axt::CLOCK_SKEW_MS_DEFAULT,
            proof_cache_ttl_slots: defaults::nexus::axt::PROOF_CACHE_TTL_SLOTS,
            replay_retention_slots: defaults::nexus::axt::REPLAY_RETENTION_SLOTS,
        }
    }
}

/// Lane-relay emergency override configuration.
#[derive(Debug, Clone, Copy, ReadConfig, norito::JsonDeserialize)]
pub struct LaneRelayEmergency {
    /// Whether emergency validator overrides are enabled.
    #[config(default = "defaults::nexus::lane_relay_emergency::ENABLED")]
    pub enabled: bool,
    /// Minimum multisig threshold required for override transactions.
    #[config(default = "defaults::nexus::lane_relay_emergency::MULTISIG_THRESHOLD")]
    pub multisig_threshold: u16,
    /// Minimum multisig member count required for override transactions.
    #[config(default = "defaults::nexus::lane_relay_emergency::MULTISIG_MEMBERS")]
    pub multisig_members: u16,
}

impl Default for LaneRelayEmergency {
    fn default() -> Self {
        Self {
            enabled: defaults::nexus::lane_relay_emergency::ENABLED,
            multisig_threshold: defaults::nexus::lane_relay_emergency::MULTISIG_THRESHOLD,
            multisig_members: defaults::nexus::lane_relay_emergency::MULTISIG_MEMBERS,
        }
    }
}

impl LaneRelayEmergency {
    fn parse(self, emitter: &mut Emitter<ParseError>) -> Option<actual::LaneRelayEmergency> {
        let mut invalid = false;
        let threshold = NonZeroU16::new(self.multisig_threshold).or_else(|| {
            invalid = true;
            emitter.emit(
                Report::new(ParseError::InvalidNexusConfig)
                    .attach("nexus.lane_relay_emergency.multisig_threshold must be > 0"),
            );
            None
        });
        let members = NonZeroU16::new(self.multisig_members).or_else(|| {
            invalid = true;
            emitter.emit(
                Report::new(ParseError::InvalidNexusConfig)
                    .attach("nexus.lane_relay_emergency.multisig_members must be > 0"),
            );
            None
        });

        if let (Some(threshold), Some(members)) = (threshold, members)
            && threshold.get() > members.get()
        {
            invalid = true;
            emitter.emit(Report::new(ParseError::InvalidNexusConfig).attach(format!(
                "nexus.lane_relay_emergency.multisig_threshold {threshold} must be <= multisig_members {members}"
            )));
        }

        if invalid || threshold.is_none() || members.is_none() {
            return None;
        }

        Some(actual::LaneRelayEmergency {
            enabled: self.enabled,
            multisig_threshold: threshold.expect("validated"),
            multisig_members: members.expect("validated"),
        })
    }
}

impl NexusAxt {
    fn parse(self, emitter: &mut Emitter<ParseError>) -> Option<actual::NexusAxt> {
        let mut invalid = false;
        if self.slot_length_ms < defaults::nexus::axt::MIN_SLOT_LENGTH_MS
            || self.slot_length_ms > defaults::nexus::axt::MAX_SLOT_LENGTH_MS
        {
            invalid = true;
            emitter.emit(Report::new(ParseError::InvalidNexusConfig).attach(format!(
                "nexus.axt.slot_length_ms must be between {} and {} milliseconds",
                defaults::nexus::axt::MIN_SLOT_LENGTH_MS,
                defaults::nexus::axt::MAX_SLOT_LENGTH_MS
            )));
        }
        let slot_length_ms = NonZeroU64::new(self.slot_length_ms).unwrap_or_else(|| {
            invalid = true;
            emitter.emit(
                Report::new(ParseError::InvalidNexusConfig)
                    .attach("nexus.axt.slot_length_ms must be non-zero"),
            );
            NonZeroU64::new(1).expect("slot length placeholder is always non-zero")
        });

        if self.max_clock_skew_ms > defaults::nexus::axt::CLOCK_SKEW_MS_MAX {
            invalid = true;
            emitter.emit(Report::new(ParseError::InvalidNexusConfig).attach(format!(
                "nexus.axt.max_clock_skew_ms must be at most {} milliseconds",
                defaults::nexus::axt::CLOCK_SKEW_MS_MAX
            )));
        }
        if self.max_clock_skew_ms > slot_length_ms.get() {
            invalid = true;
            emitter
                .emit(Report::new(ParseError::InvalidNexusConfig).attach(
                    "nexus.axt.max_clock_skew_ms must not exceed nexus.axt.slot_length_ms",
                ));
        }

        if self.proof_cache_ttl_slots == 0
            || self.proof_cache_ttl_slots > defaults::nexus::axt::PROOF_CACHE_TTL_SLOTS_MAX
        {
            invalid = true;
            emitter.emit(Report::new(ParseError::InvalidNexusConfig).attach(format!(
                "nexus.axt.proof_cache_ttl_slots must be between 1 and {}",
                defaults::nexus::axt::PROOF_CACHE_TTL_SLOTS_MAX
            )));
        }
        let proof_cache_ttl_slots =
            NonZeroU64::new(self.proof_cache_ttl_slots).unwrap_or_else(|| {
                invalid = true;
                emitter.emit(
                    Report::new(ParseError::InvalidNexusConfig)
                        .attach("nexus.axt.proof_cache_ttl_slots must be non-zero"),
                );
                NonZeroU64::new(1).expect("ttl placeholder is always non-zero")
            });

        if self.replay_retention_slots == 0
            || self.replay_retention_slots > defaults::nexus::axt::REPLAY_RETENTION_SLOTS_MAX
        {
            invalid = true;
            emitter.emit(Report::new(ParseError::InvalidNexusConfig).attach(format!(
                "nexus.axt.replay_retention_slots must be between 1 and {}",
                defaults::nexus::axt::REPLAY_RETENTION_SLOTS_MAX
            )));
        }
        let replay_retention_slots =
            NonZeroU64::new(self.replay_retention_slots).unwrap_or_else(|| {
                invalid = true;
                emitter.emit(
                    Report::new(ParseError::InvalidNexusConfig)
                        .attach("nexus.axt.replay_retention_slots must be non-zero"),
                );
                NonZeroU64::new(1).expect("replay retention placeholder is always non-zero")
            });

        if invalid {
            return None;
        }

        Some(actual::NexusAxt {
            slot_length_ms,
            max_clock_skew_ms: self.max_clock_skew_ms,
            proof_cache_ttl_slots,
            replay_retention_slots,
        })
    }
}

impl Nexus {
    /// Convert this user configuration into the runtime representation.
    pub fn parse(self, emitter: &mut Emitter<ParseError>) -> Option<actual::Nexus> {
        let Nexus {
            enabled,
            storage,
            lane_count,
            lane_catalog,
            dataspace_catalog,
            staking,
            fees,
            hf_shared_leases,
            uploaded_models,
            endorsement: endorsement_cfg,
            axt,
            lane_relay_emergency,
            routing_policy,
            registry,
            governance,
            compliance,
            fusion,
            autoscale,
            commit,
            da,
            ..
        } = self;

        let dataspace_catalog = Self::build_dataspace_catalog(dataspace_catalog, emitter)?;
        let lane_catalog =
            Self::build_lane_catalog(lane_count, lane_catalog, &dataspace_catalog, emitter)?;
        let routing_policy =
            Self::build_routing_policy(routing_policy, &lane_catalog, &dataspace_catalog, emitter)?;
        let registry = registry.parse(emitter)?;
        let governance = governance.parse(emitter)?;
        let compliance = compliance.parse();
        let fusion = fusion.parse(emitter)?;
        let autoscale = autoscale.parse(emitter)?;
        let commit = commit.parse(emitter)?;
        let da = da.parse(emitter)?;
        let storage = storage.parse(emitter)?;
        let axt_cfg = axt.parse(emitter)?;
        let lane_relay_emergency = lane_relay_emergency.parse(emitter)?;
        let staking = staking.parse(emitter)?;
        let fees = fees.parse(emitter)?;
        let hf_shared_leases = hf_shared_leases.parse(emitter)?;
        let uploaded_models = uploaded_models.parse(emitter)?;
        let endorsement = endorsement_cfg.parse(emitter)?;
        let lane_config = actual::LaneConfig::from_catalog(&lane_catalog);
        let has_multilane = lane_catalog.lane_count().get() > 1
            || dataspace_catalog.entries().len() > 1
            || routing_policy.default_lane != LaneId::SINGLE
            || routing_policy.default_dataspace != DataSpaceId::GLOBAL
            || !routing_policy.rules.is_empty();
        if has_multilane && !enabled {
            emitter.emit(
                Report::new(ParseError::InvalidNexusConfig).attach(
                    "multi-lane catalogs or routing policies require `nexus.enabled = true` (set the flag or use `--sora`)",
                ),
            );
            return None;
        }
        if lane_relay_emergency.enabled && !enabled {
            emitter.emit(
                Report::new(ParseError::InvalidNexusConfig)
                    .attach("nexus.lane_relay_emergency.enabled requires nexus.enabled = true"),
            );
            return None;
        }
        let has_lane_overrides = !enabled
            && (lane_catalog != LaneCatalog::default()
                || dataspace_catalog != DataSpaceCatalog::default()
                || routing_policy != actual::LaneRoutingPolicy::default());
        if has_lane_overrides {
            emitter.emit(
                Report::new(ParseError::InvalidNexusConfig).attach(
                    "nexus.enabled=false requires the default single-lane catalog and routing policy; remove Nexus lane/dataspace overrides or set `nexus.enabled = true`",
                ),
            );
            return None;
        }

        Some(actual::Nexus {
            enabled,
            storage,
            staking,
            fees,
            hf_shared_leases,
            uploaded_models,
            endorsement,
            lane_catalog,
            lane_config,
            dataspace_catalog,
            routing_policy,
            registry,
            governance,
            compliance,
            fusion,
            autoscale,
            commit,
            da,
            axt: axt_cfg,
            lane_relay_emergency,
        })
    }

    fn normalize_opt(value: Option<String>) -> Option<String> {
        value.and_then(|raw| {
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        })
    }

    #[allow(clippy::too_many_lines)]
    fn build_lane_catalog(
        lane_count: NonZeroU32,
        descriptors: Vec<LaneDescriptor>,
        dataspace_catalog: &DataSpaceCatalog,
        emitter: &mut Emitter<ParseError>,
    ) -> Option<LaneCatalog> {
        let mut lane_entries = Vec::new();
        let mut lane_errors = false;

        if descriptors.is_empty() {
            lane_entries.push(LaneConfig {
                id: LaneId::SINGLE,
                alias: defaults::nexus::DEFAULT_LANE_ALIAS.to_string(),
                description: None,
                ..LaneConfig::default()
            });
        } else {
            for (idx, descriptor) in descriptors.into_iter().enumerate() {
                let Some(alias) = Self::normalize_opt(descriptor.alias) else {
                    lane_errors = true;
                    emitter.emit(
                        Report::new(ParseError::InvalidNexusConfig)
                            .attach(format!("lane[{idx}] is missing an alias")),
                    );
                    continue;
                };

                let index = if let Some(explicit) = descriptor.index {
                    explicit
                } else if let Ok(value) = u32::try_from(idx) {
                    value
                } else {
                    lane_errors = true;
                    emitter.emit(Report::new(ParseError::InvalidNexusConfig).attach(format!(
                        "lane[{idx}] implicit index {idx} exceeds the u32 range"
                    )));
                    continue;
                };

                let id = match LaneId::from_lane_index(index, lane_count) {
                    Ok(id) => id,
                    Err(err) => {
                        lane_errors = true;
                        emitter.emit(
                            Report::new(ParseError::InvalidNexusConfig)
                                .attach(format!("lane[{idx}] index {index} is invalid: {err}")),
                        );
                        continue;
                    }
                };

                let description = Self::normalize_opt(descriptor.description);
                let mut lane_metadata = LaneConfig {
                    id,
                    alias,
                    description,
                    ..LaneConfig::default()
                };
                if let Some(dataspace_alias) = Self::normalize_opt(descriptor.dataspace) {
                    if let Some(entry) = dataspace_catalog.by_alias(&dataspace_alias) {
                        lane_metadata.dataspace_id = entry.id;
                    } else {
                        lane_errors = true;
                        emitter.emit(Report::new(ParseError::InvalidNexusConfig).attach(format!(
                            "lane[{idx}] references unknown dataspace alias \"{dataspace_alias}\""
                        )));
                        continue;
                    }
                }

                if let Some(raw_visibility) = Self::normalize_opt(descriptor.visibility) {
                    match raw_visibility.parse::<LaneVisibility>() {
                        Ok(visibility) => lane_metadata.visibility = visibility,
                        Err(err) => {
                            lane_errors = true;
                            let invalid = err.to_string();
                            let value = invalid
                                .strip_prefix("invalid lane visibility `")
                                .and_then(|s| s.strip_suffix('`'))
                                .unwrap_or(&invalid);
                            emitter.emit(Report::new(ParseError::InvalidNexusConfig).attach(
                                format!(
                                    "lane[{idx}] visibility \"{value}\" is invalid (expected `public` or `restricted`)"
                                ),
                            ));
                            continue;
                        }
                    }
                }

                if let Some(raw_storage) = Self::normalize_opt(descriptor.storage) {
                    match raw_storage.parse::<LaneStorageProfile>() {
                        Ok(storage) => lane_metadata.storage = storage,
                        Err(err) => {
                            lane_errors = true;
                            let invalid = err.to_string();
                            let value = invalid
                                .strip_prefix("invalid lane storage profile `")
                                .and_then(|s| s.strip_suffix('`'))
                                .unwrap_or(&invalid);
                            emitter.emit(Report::new(ParseError::InvalidNexusConfig).attach(
                                format!(
                                    "lane[{idx}] storage profile \"{value}\" is invalid (expected `full_replica`, `commitment_only`, or `split_replica`)"
                                ),
                            ));
                            continue;
                        }
                    }
                }

                if let Some(shard_id) = descriptor.shard_id {
                    lane_metadata
                        .metadata
                        .insert("shard_id".to_string(), shard_id.to_string());
                }

                if let Some(raw_scheme) = Self::normalize_opt(descriptor.proof_scheme) {
                    match raw_scheme.parse::<DaProofScheme>() {
                        Ok(scheme) => lane_metadata.proof_scheme = scheme,
                        Err(err) => {
                            lane_errors = true;
                            let invalid = err.to_string();
                            let value = invalid
                                .strip_prefix("invalid DA proof scheme `")
                                .and_then(|s| s.strip_suffix('`'))
                                .unwrap_or(&invalid);
                            emitter.emit(Report::new(ParseError::InvalidNexusConfig).attach(
                                format!(
                                    "lane[{idx}] proof scheme \"{value}\" is invalid (expected `merkle_sha256` or `kzg_bls12_381`)"
                                ),
                            ));
                            continue;
                        }
                    }
                }

                lane_metadata.lane_type = Self::normalize_opt(descriptor.lane_type);
                lane_metadata.governance = Self::normalize_opt(descriptor.governance);
                lane_metadata.settlement = Self::normalize_opt(descriptor.settlement);
                lane_metadata.metadata = descriptor
                    .metadata
                    .into_iter()
                    .filter_map(|(key, value)| {
                        let key = key.trim();
                        if key.is_empty() {
                            emitter.emit(
                                Report::new(ParseError::InvalidNexusConfig)
                                    .attach(format!("lane[{idx}] metadata key must not be empty")),
                            );
                            lane_errors = true;
                            None
                        } else {
                            Some((key.to_string(), value.trim().to_string()))
                        }
                    })
                    .collect();

                lane_entries.push(lane_metadata);
            }
        }

        if lane_errors {
            return None;
        }

        if lane_entries.is_empty() {
            emitter.emit(
                Report::new(ParseError::InvalidNexusConfig)
                    .attach("lane catalog must contain at least one lane"),
            );
            return None;
        }

        match LaneCatalog::new(lane_count, lane_entries) {
            Ok(catalog) => Some(catalog),
            Err(error) => {
                emitter.emit(
                    Report::new(ParseError::InvalidNexusConfig)
                        .attach(format!("lane catalog validation failed: {error}")),
                );
                None
            }
        }
    }

    fn build_dataspace_catalog(
        descriptors: Vec<DataSpaceDescriptor>,
        emitter: &mut Emitter<ParseError>,
    ) -> Option<DataSpaceCatalog> {
        let mut dataspace_entries = Vec::new();
        let mut dataspace_errors = false;

        if descriptors.is_empty() {
            dataspace_entries.push(DataSpaceMetadata::default());
        } else {
            for (idx, descriptor) in descriptors.into_iter().enumerate() {
                let Some(alias) = Self::normalize_opt(descriptor.alias) else {
                    dataspace_errors = true;
                    emitter.emit(
                        Report::new(ParseError::InvalidNexusConfig)
                            .attach(format!("dataspace[{idx}] is missing an alias")),
                    );
                    continue;
                };

                let description = Self::normalize_opt(descriptor.description);
                let fault_tolerance = descriptor
                    .fault_tolerance
                    .unwrap_or(defaults::nexus::dataspace::FAULT_TOLERANCE);
                if fault_tolerance == 0 {
                    dataspace_errors = true;
                    emitter.emit(
                        Report::new(ParseError::InvalidNexusConfig)
                            .attach(format!("dataspace[{idx}] fault_tolerance must be >= 1")),
                    );
                    continue;
                }
                if fault_tolerance > (u32::MAX.saturating_sub(1)) / 3 {
                    dataspace_errors = true;
                    emitter.emit(Report::new(ParseError::InvalidNexusConfig).attach(format!(
                        "dataspace[{idx}] fault_tolerance {fault_tolerance} overflows 3f+1 sizing"
                    )));
                    continue;
                }

                let id = if let Some(raw) = descriptor.id {
                    DataSpaceId::new(raw)
                } else if let Some(hash) = Self::normalize_opt(descriptor.manifest_hash) {
                    let trimmed = hash.trim_start_matches("0x");
                    match hex::decode(trimmed) {
                        Ok(bytes) if bytes.len() == 32 => {
                            let mut array = [0u8; 32];
                            array.copy_from_slice(&bytes);
                            DataSpaceId::from_hash(&array)
                        }
                        Ok(bytes) => {
                            dataspace_errors = true;
                            emitter.emit(Report::new(ParseError::InvalidNexusConfig).attach(
                                format!(
                                    "dataspace[{idx}] hash expected 32 bytes, got {}",
                                    bytes.len()
                                ),
                            ));
                            continue;
                        }
                        Err(err) => {
                            dataspace_errors = true;
                            emitter.emit(
                                Report::new(ParseError::InvalidNexusConfig)
                                    .attach(format!("dataspace[{idx}] hash decode failed: {err}")),
                            );
                            continue;
                        }
                    }
                } else {
                    dataspace_errors = true;
                    emitter.emit(Report::new(ParseError::InvalidNexusConfig).attach(format!(
                        "dataspace[{idx}] must specify either `id` or `manifest_hash`"
                    )));
                    continue;
                };

                if alias == defaults::nexus::DEFAULT_DATASPACE_ALIAS && id != DataSpaceId::GLOBAL {
                    dataspace_errors = true;
                    emitter.emit(Report::new(ParseError::InvalidNexusConfig).attach(format!(
                        "dataspace[{idx}] alias \"{}\" must map to id {}",
                        defaults::nexus::DEFAULT_DATASPACE_ALIAS,
                        DataSpaceId::GLOBAL.as_u64()
                    )));
                    continue;
                }

                dataspace_entries.push(DataSpaceMetadata {
                    id,
                    alias,
                    description,
                    fault_tolerance,
                });
            }
        }

        if dataspace_errors {
            return None;
        }

        let has_global = dataspace_entries
            .iter()
            .any(|entry| entry.id == DataSpaceId::GLOBAL);
        let has_global_alias = dataspace_entries
            .iter()
            .any(|entry| entry.alias == defaults::nexus::DEFAULT_DATASPACE_ALIAS);
        if !has_global && !has_global_alias {
            dataspace_entries.push(DataSpaceMetadata::default());
        }

        match DataSpaceCatalog::new(dataspace_entries) {
            Ok(catalog) => Some(catalog),
            Err(error) => {
                emitter.emit(
                    Report::new(ParseError::InvalidNexusConfig)
                        .attach(format!("dataspace catalog validation failed: {error}")),
                );
                None
            }
        }
    }

    fn build_routing_policy(
        policy: RoutingPolicy,
        lane_catalog: &LaneCatalog,
        dataspace_catalog: &DataSpaceCatalog,
        emitter: &mut Emitter<ParseError>,
    ) -> Option<actual::LaneRoutingPolicy> {
        let effective_lane_count = lane_catalog.lane_count();
        let default_lane_index = policy
            .default_lane
            .unwrap_or(defaults::nexus::DEFAULT_ROUTING_LANE_INDEX);
        let default_lane = match LaneId::from_lane_index(default_lane_index, effective_lane_count) {
            Ok(id) => id,
            Err(err) => {
                emitter.emit(Report::new(ParseError::InvalidNexusConfig).attach(format!(
                    "routing default lane index {default_lane_index} is invalid: {err}"
                )));
                return None;
            }
        };

        let default_dataspace = if let Some(alias) = Self::normalize_opt(policy.default_dataspace) {
            if let Some(id) = Self::resolve_dataspace(dataspace_catalog, &alias) {
                id
            } else {
                emitter.emit(Report::new(ParseError::InvalidNexusConfig).attach(format!(
                    "routing default dataspace alias {alias} is not present in dataspace_catalog"
                )));
                return None;
            }
        } else {
            DataSpaceId::GLOBAL
        };
        let lane_dataspaces: BTreeMap<LaneId, DataSpaceId> = lane_catalog
            .lanes()
            .iter()
            .map(|lane| (lane.id, lane.dataspace_id))
            .collect();
        let default_lane_dataspace = if let Some(id) = lane_dataspaces.get(&default_lane) {
            *id
        } else {
            emitter.emit(Report::new(ParseError::InvalidNexusConfig).attach(format!(
                "routing default lane {} is missing from lane_catalog",
                default_lane.as_u32()
            )));
            return None;
        };
        if default_lane_dataspace != default_dataspace {
            emitter.emit(Report::new(ParseError::InvalidNexusConfig).attach(format!(
                "routing default dataspace {} does not match lane {} dataspace {}",
                default_dataspace.as_u64(),
                default_lane.as_u32(),
                default_lane_dataspace.as_u64()
            )));
            return None;
        }

        let mut routing_errors = false;
        let mut rules = Vec::new();
        for (idx, rule) in policy.rules.into_iter().enumerate() {
            let Some(lane_index) = rule.lane else {
                routing_errors = true;
                emitter.emit(
                    Report::new(ParseError::InvalidNexusConfig)
                        .attach(format!("routing rule[{idx}] is missing a lane index")),
                );
                continue;
            };

            let lane = match LaneId::from_lane_index(lane_index, effective_lane_count) {
                Ok(id) => id,
                Err(err) => {
                    routing_errors = true;
                    emitter.emit(Report::new(ParseError::InvalidNexusConfig).attach(format!(
                        "routing rule[{idx}] lane index {lane_index} is invalid: {err}"
                    )));
                    continue;
                }
            };

            let matcher = actual::LaneRoutingMatcher {
                account: Self::normalize_opt(rule.matcher.account),
                instruction: Self::normalize_opt(rule.matcher.instruction),
                description: Self::normalize_opt(rule.matcher.description),
            };

            let dataspace = if let Some(alias) = Self::normalize_opt(rule.dataspace) {
                if let Some(id) = Self::resolve_dataspace(dataspace_catalog, &alias) {
                    Some(id)
                } else {
                    routing_errors = true;
                    emitter.emit(Report::new(ParseError::InvalidNexusConfig).attach(
                        format!(
                            "routing rule[{idx}] dataspace alias {alias} is not present in dataspace_catalog"
                        ),
                    ));
                    continue;
                }
            } else {
                None
            };
            let lane_dataspace = if let Some(id) = lane_dataspaces.get(&lane) {
                *id
            } else {
                routing_errors = true;
                emitter.emit(Report::new(ParseError::InvalidNexusConfig).attach(format!(
                    "routing rule[{idx}] references lane {} not present in lane_catalog",
                    lane.as_u32()
                )));
                continue;
            };
            let effective_dataspace = dataspace.unwrap_or(default_dataspace);
            if lane_dataspace != effective_dataspace {
                routing_errors = true;
                emitter.emit(Report::new(ParseError::InvalidNexusConfig).attach(format!(
                    "routing rule[{idx}] dataspace {} does not match lane {} dataspace {}",
                    effective_dataspace.as_u64(),
                    lane.as_u32(),
                    lane_dataspace.as_u64()
                )));
                continue;
            }

            rules.push(actual::LaneRoutingRule {
                lane,
                dataspace,
                matcher,
            });
        }

        if routing_errors {
            return None;
        }

        Some(actual::LaneRoutingPolicy {
            default_lane,
            default_dataspace,
            rules,
        })
    }

    fn resolve_dataspace(catalog: &DataSpaceCatalog, alias: &str) -> Option<DataSpaceId> {
        catalog.by_alias(alias).map(|meta| meta.id)
    }
}

/// User-level configuration container for `Logger`.
#[derive(Debug, Clone, Default, ReadConfig)]
pub struct Logger {
    /// Level of logging verbosity
    #[config(env = "LOG_LEVEL", default)]
    pub level: Level,
    /// Refined directives
    #[config(env = "LOG_FILTER")]
    pub filter: Option<Directives>,
    /// Output format
    #[config(env = "LOG_FORMAT", default)]
    pub format: LoggerFormat,
    /// Whether to use ANSI colors in terminal output
    #[config(env = "LOG_TERMINAL_COLORS", default)]
    pub terminal_colors: bool,
}

impl Logger {
    /// Combine `level` and `filter` into a single directives list.
    ///
    /// ```
    /// use iroha_config::{logger::Level, parameters::user::Logger};
    ///
    /// let value = Logger {
    ///     level: Level::DEBUG,
    ///     filter: Some("iroha_core=trace".parse().unwrap()),
    ///     format: <_>::default(),
    ///     terminal_colors: false,
    /// };
    ///
    /// assert_eq!(
    ///     format!("{}", value.resolve_filter()),
    ///     "debug,iroha_core=trace"
    /// );
    /// ```
    pub fn resolve_filter(&self) -> Directives {
        let mut dirs: Directives = self.level.into();
        if let Some(more) = &self.filter {
            dirs.extend(more.clone());
        }
        dirs
    }
}

/// User-level configuration container for `Telemetry`.
#[derive(Debug)]
pub struct Telemetry {
    // Fields here are Options so that it is possible to warn the user if e.g. they provided `min_retry_period`, but haven't
    // provided `name` and `url`
    name: String,
    url: Url,
    min_retry_period_ms: TelemetryMinRetryPeriod,
    max_retry_delay_exponent: TelemetryMaxRetryDelayExponent,
    /// Optional Telegram bot key for alerts
    telegram_bot_key: Option<String>,
    /// Optional Telegram chat ID for alerts
    telegram_chat_id: Option<String>,
    /// Optional minimum level for Telegram alerts (e.g., "WARN", "ERROR").
    telegram_min_level: Option<String>,
    /// Optional list of target prefixes to include (e.g., `p2p`, `network`). If empty or None, include all.
    telegram_targets: Option<Vec<String>>,
    /// Optional rate limit for Telegram alerts (messages per minute)
    telegram_rate_per_minute: Option<NonZeroU32>,
    /// Include a metrics snapshot in alerts.
    telegram_include_metrics: Option<bool>,
    /// Optional metrics URL to poll (e.g., <http://127.0.0.1:8080/metrics>).
    telegram_metrics_url: Option<Url>,
    /// Optional metrics poll period (clamped to >= 100ms).
    telegram_metrics_period_ms: Option<DurationMs>,
    /// Optional allow-list of `msg` kinds to send.
    telegram_allow_kinds: Option<Vec<String>>,
    /// Optional deny-list of `msg` kinds to suppress.
    telegram_deny_kinds: Option<Vec<String>>,
}

#[derive(Debug, Copy, Clone)]
struct TelemetryMinRetryPeriod(DurationMs);

impl Default for TelemetryMinRetryPeriod {
    fn default() -> Self {
        Self(DurationMs(defaults::telemetry::MIN_RETRY_PERIOD))
    }
}

#[derive(Debug, Copy, Clone)]
struct TelemetryMaxRetryDelayExponent(u8);

impl Default for TelemetryMaxRetryDelayExponent {
    fn default() -> Self {
        Self(defaults::telemetry::MAX_RETRY_DELAY_EXPONENT)
    }
}

struct FieldState<T> {
    seen: bool,
    value: Option<T>,
}

impl<T> Default for FieldState<T> {
    fn default() -> Self {
        Self {
            seen: false,
            value: None,
        }
    }
}

impl<T> FieldState<T> {
    fn set_optional<F>(
        &mut self,
        field: &'static str,
        parser: &mut json::Parser<'_>,
        parse: F,
    ) -> ::core::result::Result<(), json::Error>
    where
        F: FnOnce(&mut json::Parser<'_>) -> ::core::result::Result<Option<T>, json::Error>,
    {
        if self.seen {
            return Err(json::Error::duplicate_field(field));
        }
        self.value = parse(parser)?;
        self.seen = true;
        Ok(())
    }

    fn into_option(self) -> Option<T> {
        self.value
    }
}

#[derive(Default)]
struct TelemetryFields {
    name: Option<String>,
    url: Option<Url>,
    min_retry_period_ms: Option<TelemetryMinRetryPeriod>,
    max_retry_delay_exponent: Option<TelemetryMaxRetryDelayExponent>,
    telegram_bot_key: FieldState<String>,
    telegram_chat_id: FieldState<String>,
    telegram_min_level: FieldState<String>,
    telegram_targets: FieldState<Vec<String>>,
    telegram_rate_per_minute: FieldState<NonZeroU32>,
    telegram_include_metrics: FieldState<bool>,
    telegram_metrics_url: FieldState<Url>,
    telegram_metrics_period_ms: FieldState<DurationMs>,
    telegram_allow_kinds: FieldState<Vec<String>>,
    telegram_deny_kinds: FieldState<Vec<String>>,
}

impl TelemetryFields {
    fn set_unique<T, F>(
        slot: &mut Option<T>,
        field: &'static str,
        parser: &mut json::Parser<'_>,
        parse: F,
    ) -> ::core::result::Result<(), json::Error>
    where
        F: FnOnce(&mut json::Parser<'_>) -> ::core::result::Result<T, json::Error>,
    {
        if slot.is_some() {
            return Err(json::Error::duplicate_field(field));
        }
        let value = parse(parser)?;
        *slot = Some(value);
        Ok(())
    }

    fn parse_field(
        &mut self,
        parser: &mut json::Parser<'_>,
        key: &str,
    ) -> ::core::result::Result<(), json::Error> {
        match key {
            "name" => Self::set_unique(&mut self.name, "name", parser, |p| {
                <String as json::JsonDeserialize>::json_deserialize(p)
            }),
            "url" => Self::set_unique(&mut self.url, "url", parser, |p| {
                let raw = <String as json::JsonDeserialize>::json_deserialize(p)?;
                Url::parse(&raw).map_err(|err| json::Error::InvalidField {
                    field: "url".into(),
                    message: err.to_string(),
                })
            }),
            "min_retry_period_ms" => Self::set_unique(
                &mut self.min_retry_period_ms,
                "min_retry_period_ms",
                parser,
                TelemetryMinRetryPeriod::json_deserialize,
            ),
            "max_retry_delay_exponent" => Self::set_unique(
                &mut self.max_retry_delay_exponent,
                "max_retry_delay_exponent",
                parser,
                TelemetryMaxRetryDelayExponent::json_deserialize,
            ),
            "telegram_bot_key" => self.telegram_bot_key.set_optional(
                "telegram_bot_key",
                parser,
                Option::<String>::json_deserialize,
            ),
            "telegram_chat_id" => self.telegram_chat_id.set_optional(
                "telegram_chat_id",
                parser,
                Option::<String>::json_deserialize,
            ),
            "telegram_min_level" => self.telegram_min_level.set_optional(
                "telegram_min_level",
                parser,
                Option::<String>::json_deserialize,
            ),
            "telegram_targets" => self.telegram_targets.set_optional(
                "telegram_targets",
                parser,
                Option::<Vec<String>>::json_deserialize,
            ),
            "telegram_rate_per_minute" => self.telegram_rate_per_minute.set_optional(
                "telegram_rate_per_minute",
                parser,
                Option::<NonZeroU32>::json_deserialize,
            ),
            "telegram_include_metrics" => self.telegram_include_metrics.set_optional(
                "telegram_include_metrics",
                parser,
                Option::<bool>::json_deserialize,
            ),
            "telegram_metrics_url" => {
                self.telegram_metrics_url
                    .set_optional("telegram_metrics_url", parser, |p| {
                        let raw = Option::<String>::json_deserialize(p)?;
                        raw.map_or(Ok(None), |value| {
                            Url::parse(&value)
                                .map(Some)
                                .map_err(|err| json::Error::InvalidField {
                                    field: "telegram_metrics_url".into(),
                                    message: err.to_string(),
                                })
                        })
                    })
            }
            "telegram_metrics_period_ms" => self.telegram_metrics_period_ms.set_optional(
                "telegram_metrics_period_ms",
                parser,
                Option::<DurationMs>::json_deserialize,
            ),
            "telegram_allow_kinds" => self.telegram_allow_kinds.set_optional(
                "telegram_allow_kinds",
                parser,
                Option::<Vec<String>>::json_deserialize,
            ),
            "telegram_deny_kinds" => self.telegram_deny_kinds.set_optional(
                "telegram_deny_kinds",
                parser,
                Option::<Vec<String>>::json_deserialize,
            ),
            other => Err(json::Error::Message(format!("unknown field {other}"))),
        }
    }

    fn finish(self) -> ::core::result::Result<Telemetry, json::Error> {
        Ok(Telemetry {
            name: self
                .name
                .ok_or_else(|| json::Error::missing_field("name"))?,
            url: self.url.ok_or_else(|| json::Error::missing_field("url"))?,
            min_retry_period_ms: self.min_retry_period_ms.unwrap_or_default(),
            max_retry_delay_exponent: self.max_retry_delay_exponent.unwrap_or_default(),
            telegram_bot_key: self.telegram_bot_key.into_option(),
            telegram_chat_id: self.telegram_chat_id.into_option(),
            telegram_min_level: self.telegram_min_level.into_option(),
            telegram_targets: self.telegram_targets.into_option(),
            telegram_rate_per_minute: self.telegram_rate_per_minute.into_option(),
            telegram_include_metrics: self.telegram_include_metrics.into_option(),
            telegram_metrics_url: self.telegram_metrics_url.into_option(),
            telegram_metrics_period_ms: self.telegram_metrics_period_ms.into_option(),
            telegram_allow_kinds: self.telegram_allow_kinds.into_option(),
            telegram_deny_kinds: self.telegram_deny_kinds.into_option(),
        })
    }
}

impl JsonDeserialize for Telemetry {
    fn json_deserialize(
        parser: &mut json::Parser<'_>,
    ) -> ::core::result::Result<Self, json::Error> {
        parser.skip_ws();
        parser.expect(b'{')?;

        let mut fields = TelemetryFields::default();

        loop {
            parser.skip_ws();
            if parser.try_consume_char(b'}')? {
                break;
            }

            let key = parser.parse_key()?;
            let key_str = key.as_str();
            fields.parse_field(parser, key_str)?;

            parser.skip_ws();
            if parser.try_consume_char(b',')? {
                continue;
            }
            if parser.try_consume_char(b'}')? {
                break;
            }
            return Err(json::Error::Message("expected , or }".into()));
        }

        fields.finish()
    }
}

impl JsonDeserialize for TelemetryMinRetryPeriod {
    fn json_deserialize(
        parser: &mut json::Parser<'_>,
    ) -> ::core::result::Result<Self, json::Error> {
        let duration = <DurationMs as JsonDeserialize>::json_deserialize(parser)?;
        Ok(Self(duration))
    }
}

impl JsonDeserialize for TelemetryMaxRetryDelayExponent {
    fn json_deserialize(
        parser: &mut json::Parser<'_>,
    ) -> ::core::result::Result<Self, json::Error> {
        let exponent = <u8 as JsonDeserialize>::json_deserialize(parser)?;
        Ok(Self(exponent))
    }
}

#[cfg(test)]
#[test]
fn telemetry_json_defaults() {
    let json = r#"{"name":"ops","url":"http://localhost:8180"}"#;
    let telemetry = norito::json::from_json::<Telemetry>(json).expect("parse telemetry defaults");

    assert_eq!(telemetry.name, "ops");
    assert_eq!(telemetry.url.scheme(), "http");
    assert_eq!(telemetry.url.host_str(), Some("localhost"));
    assert_eq!(telemetry.url.port_or_known_default(), Some(8180));
    assert!(telemetry.url.path().is_empty() || telemetry.url.path() == "/");
    let TelemetryMinRetryPeriod(duration) = telemetry.min_retry_period_ms;
    assert_eq!(duration.get(), defaults::telemetry::MIN_RETRY_PERIOD);
    let TelemetryMaxRetryDelayExponent(exponent) = telemetry.max_retry_delay_exponent;
    assert_eq!(exponent, defaults::telemetry::MAX_RETRY_DELAY_EXPONENT);
    assert!(telemetry.telegram_bot_key.is_none());
    assert!(telemetry.telegram_chat_id.is_none());
    assert!(telemetry.telegram_targets.is_none());
}

#[cfg(test)]
#[test]
fn telemetry_json_optional_fields() {
    let json = r#"{
        "name": "ops",
        "url": "https://example.com/metrics",
        "telegram_bot_key": "secret",
        "telegram_metrics_url": "https://example.com/metrics",
        "telegram_metrics_period_ms": 5000,
        "telegram_rate_per_minute": 3,
        "telegram_include_metrics": true,
        "telegram_allow_kinds": ["warn"],
        "telegram_deny_kinds": ["debug"]
    }"#;

    let telemetry =
        norito::json::from_json::<Telemetry>(json).expect("parse telemetry with optional fields");

    assert_eq!(telemetry.telegram_bot_key.as_deref(), Some("secret"));
    assert_eq!(
        telemetry
            .telegram_metrics_url
            .as_ref()
            .expect("metrics url")
            .as_str(),
        "https://example.com/metrics"
    );
    assert_eq!(telemetry.telegram_include_metrics, Some(true));
    assert_eq!(
        telemetry
            .telegram_rate_per_minute
            .map(std::num::NonZeroU32::get),
        Some(3)
    );
    let TelemetryMinRetryPeriod(period_default) = telemetry.min_retry_period_ms;
    assert_eq!(period_default.get(), defaults::telemetry::MIN_RETRY_PERIOD);
    let TelemetryMaxRetryDelayExponent(exponent_default) = telemetry.max_retry_delay_exponent;
    assert_eq!(
        exponent_default,
        defaults::telemetry::MAX_RETRY_DELAY_EXPONENT
    );
    let metrics_period = telemetry
        .telegram_metrics_period_ms
        .expect("metrics period")
        .get();
    assert_eq!(metrics_period, std::time::Duration::from_millis(5000));
    assert_eq!(
        telemetry.telegram_allow_kinds,
        Some(vec![String::from("warn")])
    );
    assert_eq!(
        telemetry.telegram_deny_kinds,
        Some(vec![String::from("debug")])
    );
}

#[cfg(test)]
#[test]
fn telemetry_json_invalid_url() {
    let json = r#"{"name":"ops","url":"not a url"}"#;
    let err = norito::json::from_json::<Telemetry>(json).expect_err("reject invalid url");
    assert!(err.to_string().contains("invalid field `url`"));
}

#[cfg(test)]
#[test]
fn telemetry_redaction_mode_round_trips() {
    let json = r#""allowlist""#;
    let mode =
        norito::json::from_json::<TelemetryRedactionMode>(json).expect("parse redaction mode");
    assert_eq!(mode, TelemetryRedactionMode::Allowlist);

    let mut out = String::new();
    mode.json_serialize(&mut out);
    assert_eq!(out, "\"allowlist\"");
}

#[cfg(test)]
#[test]
fn telemetry_signing_key_parses_hex() {
    let key = "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f";
    let parsed = parse_telemetry_signing_key(key).expect("parse key");
    assert_eq!(parsed[0], 0);
    assert_eq!(parsed[31], 0x1f);
}

#[cfg(test)]
#[test]
fn telemetry_signing_key_rejects_wrong_length() {
    let err = parse_telemetry_signing_key("deadbeef").expect_err("reject short key");
    assert!(err.contains("must be 32 bytes"));
}

#[cfg(test)]
#[test]
fn telemetry_integrity_state_dir_resolves_relative_path() {
    use std::path::Path;

    let origin = ParameterOrigin::file(
        ParameterId::from(["telemetry_integrity", "state_dir"]),
        PathBuf::from("/config/root/config.toml"),
    );
    let state_dir = WithOrigin::new(PathBuf::from("telemetry-state"), origin);
    let integrity = TelemetryIntegrity {
        enabled: true,
        state_dir: Some(state_dir),
        signing_key_hex: None,
        signing_key_id: None,
    };

    let mut emitter = Emitter::new();
    let parsed = integrity.parse(&mut emitter);
    emitter.into_result().expect("state dir parse error");

    assert_eq!(
        parsed.state_dir.as_deref(),
        Some(Path::new("/config/root/telemetry-state"))
    );
}

impl From<Telemetry> for actual::Telemetry {
    fn from(
        Telemetry {
            name,
            url,
            min_retry_period_ms: TelemetryMinRetryPeriod(DurationMs(min_retry_period)),
            max_retry_delay_exponent: TelemetryMaxRetryDelayExponent(max_retry_delay_exponent),
            telegram_bot_key,
            telegram_chat_id,
            telegram_min_level,
            telegram_targets,
            telegram_rate_per_minute,
            telegram_include_metrics,
            telegram_metrics_url,
            telegram_metrics_period_ms,
            telegram_allow_kinds,
            telegram_deny_kinds,
        }: Telemetry,
    ) -> Self {
        Self {
            name,
            url,
            min_retry_period,
            max_retry_delay_exponent,
            telegram_bot_key,
            telegram_chat_id,
            telegram_min_level,
            telegram_targets,
            telegram_rate_per_minute,
            telegram_include_metrics: telegram_include_metrics.unwrap_or(false),
            telegram_metrics_url,
            telegram_metrics_period: telegram_metrics_period_ms
                .map(iroha_config_base::util::DurationMs::get)
                .map(|duration| duration.max(MIN_TIMER_INTERVAL)),
            telegram_allow_kinds,
            telegram_deny_kinds,
        }
    }
}

/// User-level configuration container for `DevTelemetry`.
#[derive(Debug, Clone, ReadConfig)]
pub struct DevTelemetry {
    /// Optional file that receives development telemetry output.
    pub out_file: Option<WithOrigin<PathBuf>>,
    /// Panic when duplicate metrics are registered (developer diagnostics only).
    #[config(default = "defaults::telemetry::PANIC_ON_DUPLICATE_METRICS")]
    pub panic_on_duplicate_metrics: bool,
}

/// User-level configuration container for `Snapshot`.
#[derive(Debug, Clone, ReadConfig)]
pub struct Snapshot {
    /// Snapshot operating mode.
    #[config(default, env = "SNAPSHOT_MODE")]
    pub mode: SnapshotMode,
    /// Interval between automatic snapshot creation events.
    #[config(default = "defaults::snapshot::CREATE_EVERY.into()")]
    pub create_every_ms: DurationMs,
    /// Directory where snapshot artifacts are stored.
    #[config(
        default = "PathBuf::from(defaults::snapshot::STORE_DIR)",
        env = "SNAPSHOT_STORE_DIR"
    )]
    pub store_dir: WithOrigin<PathBuf>,
    /// Chunk size (bytes) used to derive Merkle proofs for snapshots.
    #[config(default = "defaults::snapshot::MERKLE_CHUNK_SIZE_BYTES")]
    pub merkle_chunk_size_bytes: NonZeroUsize,
    /// Optional public key used to verify snapshot signatures (defaults to node identity key).
    pub verification_public_key: Option<PublicKey>,
    /// Optional private key used to sign snapshots (defaults to node identity key).
    pub signing_private_key: Option<ExposedPrivateKey>,
}

/// User-level configuration container for the embedded Soracloud runtime manager.
#[derive(Debug, Clone, ReadConfig, norito::JsonDeserialize)]
pub struct SoracloudRuntime {
    /// Root directory for node-local Soracloud runtime state.
    #[config(default = "PathBuf::from(defaults::soracloud_runtime::STATE_DIR)")]
    pub state_dir: WithOrigin<PathBuf>,
    /// Reconciliation cadence against authoritative world state (milliseconds).
    #[config(
        default = "DurationMs(std::time::Duration::from_millis(defaults::soracloud_runtime::RECONCILE_INTERVAL_MS))"
    )]
    pub reconcile_interval_ms: DurationMs,
    /// Maximum concurrent artifact hydration workers.
    #[config(default = "defaults::soracloud_runtime::HYDRATION_CONCURRENCY")]
    pub hydration_concurrency: NonZeroUsize,
    /// Cache budgets for hydrated Soracloud artifacts.
    #[config(nested)]
    pub cache_budgets: SoracloudRuntimeCacheBudgets,
    /// Deterministic `NativeProcess` hosting limits.
    #[config(nested)]
    pub native_process: SoracloudRuntimeNativeProcess,
    /// Outbound egress policy for embedded runtimes.
    #[config(nested)]
    pub egress: SoracloudRuntimeEgress,
    /// Hugging Face importer and inference-bridge settings.
    #[config(nested)]
    pub hf: SoracloudRuntimeHuggingFace,
}

impl SoracloudRuntime {
    fn parse(self) -> actual::SoracloudRuntime {
        actual::SoracloudRuntime {
            state_dir: self.state_dir.resolve_relative_path(),
            reconcile_interval: self.reconcile_interval_ms.get().max(MIN_TIMER_INTERVAL),
            hydration_concurrency: self.hydration_concurrency,
            cache_budgets: self.cache_budgets.parse(),
            native_process: self.native_process.parse(),
            egress: self.egress.parse(),
            hf: self.hf.parse(),
        }
    }
}

/// User-level cache budget settings for hydrated Soracloud artifacts.
#[derive(Debug, Clone, Copy, ReadConfig, norito::JsonDeserialize)]
#[allow(clippy::struct_field_names)]
pub struct SoracloudRuntimeCacheBudgets {
    /// Cache budget for executable service bundles.
    #[config(default = "defaults::soracloud_runtime::BUNDLE_CACHE_BUDGET_BYTES")]
    pub bundle_bytes: NonZeroU64,
    /// Cache budget for hydrated static assets.
    #[config(default = "defaults::soracloud_runtime::STATIC_ASSET_CACHE_BUDGET_BYTES")]
    pub static_asset_bytes: NonZeroU64,
    /// Cache budget for runtime journals.
    #[config(default = "defaults::soracloud_runtime::JOURNAL_CACHE_BUDGET_BYTES")]
    pub journal_bytes: NonZeroU64,
    /// Cache budget for runtime checkpoints.
    #[config(default = "defaults::soracloud_runtime::CHECKPOINT_CACHE_BUDGET_BYTES")]
    pub checkpoint_bytes: NonZeroU64,
    /// Cache budget for model artifacts.
    #[config(default = "defaults::soracloud_runtime::MODEL_ARTIFACT_CACHE_BUDGET_BYTES")]
    pub model_artifact_bytes: NonZeroU64,
    /// Cache budget for model weights.
    #[config(default = "defaults::soracloud_runtime::MODEL_WEIGHT_CACHE_BUDGET_BYTES")]
    pub model_weight_bytes: NonZeroU64,
}

impl SoracloudRuntimeCacheBudgets {
    fn parse(self) -> actual::SoracloudRuntimeCacheBudgets {
        actual::SoracloudRuntimeCacheBudgets {
            bundle_bytes: self.bundle_bytes,
            static_asset_bytes: self.static_asset_bytes,
            journal_bytes: self.journal_bytes,
            checkpoint_bytes: self.checkpoint_bytes,
            model_artifact_bytes: self.model_artifact_bytes,
            model_weight_bytes: self.model_weight_bytes,
        }
    }
}

/// User-level deterministic `NativeProcess` resource ceilings.
#[derive(Debug, Clone, Copy, ReadConfig, norito::JsonDeserialize)]
pub struct SoracloudRuntimeNativeProcess {
    /// Maximum number of deterministic native processes hosted concurrently.
    #[config(default = "defaults::soracloud_runtime::NATIVE_PROCESS_MAX_CONCURRENT_PROCESSES")]
    pub max_concurrent_processes: NonZeroUsize,
    /// CPU budget in millicores per hosted process.
    #[config(default = "defaults::soracloud_runtime::NATIVE_PROCESS_CPU_MILLIS")]
    pub cpu_millis: NonZeroU32,
    /// Memory budget in bytes per hosted process.
    #[config(default = "defaults::soracloud_runtime::NATIVE_PROCESS_MEMORY_BYTES")]
    pub memory_bytes: NonZeroU64,
    /// Ephemeral filesystem budget in bytes per hosted process.
    #[config(default = "defaults::soracloud_runtime::NATIVE_PROCESS_EPHEMERAL_STORAGE_BYTES")]
    pub ephemeral_storage_bytes: NonZeroU64,
    /// Open-file ceiling per hosted process.
    #[config(default = "defaults::soracloud_runtime::NATIVE_PROCESS_MAX_OPEN_FILES")]
    pub max_open_files: NonZeroU32,
    /// Task/thread ceiling per hosted process.
    #[config(default = "defaults::soracloud_runtime::NATIVE_PROCESS_MAX_TASKS")]
    pub max_tasks: NonZeroU16,
    /// Startup grace window in milliseconds.
    #[config(
        default = "DurationMs(std::time::Duration::from_millis(defaults::soracloud_runtime::NATIVE_PROCESS_START_GRACE_MS))"
    )]
    pub start_grace_ms: DurationMs,
    /// Shutdown grace window in milliseconds.
    #[config(
        default = "DurationMs(std::time::Duration::from_millis(defaults::soracloud_runtime::NATIVE_PROCESS_STOP_GRACE_MS))"
    )]
    pub stop_grace_ms: DurationMs,
}

impl SoracloudRuntimeNativeProcess {
    fn parse(self) -> actual::SoracloudRuntimeNativeProcess {
        actual::SoracloudRuntimeNativeProcess {
            max_concurrent_processes: self.max_concurrent_processes,
            cpu_millis: self.cpu_millis,
            memory_bytes: self.memory_bytes,
            ephemeral_storage_bytes: self.ephemeral_storage_bytes,
            max_open_files: self.max_open_files,
            max_tasks: self.max_tasks,
            start_grace: self.start_grace_ms.get().max(MIN_TIMER_INTERVAL),
            stop_grace: self.stop_grace_ms.get().max(MIN_TIMER_INTERVAL),
        }
    }
}

/// User-level outbound egress policy for embedded Soracloud runtimes.
#[derive(Debug, Clone, ReadConfig, norito::JsonDeserialize)]
pub struct SoracloudRuntimeEgress {
    /// Whether egress is allowed by default when a destination is not explicitly listed.
    #[config(default = "defaults::soracloud_runtime::EGRESS_DEFAULT_ALLOW")]
    pub default_allow: bool,
    /// Explicit destination allowlist for outbound requests.
    #[config(default = "defaults::soracloud_runtime::egress_allowed_hosts()")]
    pub allowed_hosts: Vec<String>,
    /// Optional outbound request-rate cap per service/minute.
    pub rate_per_minute: Option<u32>,
    /// Optional outbound byte budget per service/minute.
    pub max_bytes_per_minute: Option<u64>,
}

impl SoracloudRuntimeEgress {
    fn parse(self) -> actual::SoracloudRuntimeEgress {
        let mut allowed_hosts: Vec<String> = self
            .allowed_hosts
            .into_iter()
            .map(|host| host.trim().to_string())
            .filter(|host| !host.is_empty())
            .collect();
        allowed_hosts.sort();
        allowed_hosts.dedup();

        actual::SoracloudRuntimeEgress {
            default_allow: self.default_allow,
            allowed_hosts,
            rate_per_minute: self.rate_per_minute.and_then(NonZeroU32::new),
            max_bytes_per_minute: self.max_bytes_per_minute.and_then(NonZeroU64::new),
        }
    }
}

/// User-level Hugging Face importer/inference settings for the embedded Soracloud runtime manager.
#[derive(Debug, Clone, ReadConfig, norito::JsonDeserialize)]
pub struct SoracloudRuntimeHuggingFace {
    /// Base URL used to resolve repo files from the Hub.
    #[config(default = "defaults::soracloud_runtime::hf::HUB_BASE_URL.to_string()")]
    pub hub_base_url: String,
    /// Base URL used to fetch model metadata from the Hub API.
    #[config(default = "defaults::soracloud_runtime::hf::API_BASE_URL.to_string()")]
    pub api_base_url: String,
    /// Base URL used to forward `/infer` calls to HF Inference.
    #[config(default = "defaults::soracloud_runtime::hf::INFERENCE_BASE_URL.to_string()")]
    pub inference_base_url: String,
    /// Timeout applied to Hugging Face API and file requests (milliseconds).
    #[config(
        default = "DurationMs(std::time::Duration::from_millis(defaults::soracloud_runtime::hf::REQUEST_TIMEOUT_MS))"
    )]
    pub request_timeout_ms: DurationMs,
    /// Whether generated HF services should prefer the on-node local runner path.
    #[config(default = "defaults::soracloud_runtime::hf::LOCAL_EXECUTION_ENABLED")]
    pub local_execution_enabled: bool,
    /// Program used to invoke the embedded local HF runner script.
    #[config(default = "defaults::soracloud_runtime::hf::LOCAL_RUNNER_PROGRAM.to_string()")]
    pub local_runner_program: String,
    /// Timeout applied to one local runner invocation (milliseconds).
    #[config(
        default = "DurationMs(std::time::Duration::from_millis(defaults::soracloud_runtime::hf::LOCAL_RUNNER_TIMEOUT_MS))"
    )]
    pub local_runner_timeout_ms: DurationMs,
    /// TTL applied when the runtime emits authoritative model-host heartbeats after a successful local probe (milliseconds).
    #[config(
        default = "DurationMs(std::time::Duration::from_millis(defaults::soracloud_runtime::hf::MODEL_HOST_HEARTBEAT_TTL_MS))"
    )]
    pub model_host_heartbeat_ttl_ms: DurationMs,
    /// Whether the runtime may fall back to HF Inference when local execution fails.
    #[config(default = "defaults::soracloud_runtime::hf::ALLOW_INFERENCE_BRIDGE_FALLBACK")]
    pub allow_inference_bridge_fallback: bool,
    /// Maximum number of imported Hub files retained per shared source.
    #[config(default = "defaults::soracloud_runtime::hf::IMPORT_MAX_FILES")]
    pub import_max_files: u32,
    /// Maximum size of one imported Hub file.
    #[config(default = "defaults::soracloud_runtime::hf::IMPORT_MAX_FILE_BYTES")]
    pub import_max_file_bytes: u64,
    /// Maximum aggregate size imported per shared source.
    #[config(default = "defaults::soracloud_runtime::hf::IMPORT_MAX_TOTAL_BYTES")]
    pub import_max_total_bytes: u64,
    /// File-selection allowlist used by the importer.
    #[config(default = "defaults::soracloud_runtime::hf::import_file_allowlist()")]
    pub import_file_allowlist: Vec<String>,
    /// Optional bearer token used for HF Inference requests.
    pub inference_token: Option<String>,
}

impl Default for SoracloudRuntimeHuggingFace {
    fn default() -> Self {
        Self {
            hub_base_url: defaults::soracloud_runtime::hf::HUB_BASE_URL.to_string(),
            api_base_url: defaults::soracloud_runtime::hf::API_BASE_URL.to_string(),
            inference_base_url: defaults::soracloud_runtime::hf::INFERENCE_BASE_URL.to_string(),
            request_timeout_ms: DurationMs(std::time::Duration::from_millis(
                defaults::soracloud_runtime::hf::REQUEST_TIMEOUT_MS,
            )),
            local_execution_enabled: defaults::soracloud_runtime::hf::LOCAL_EXECUTION_ENABLED,
            local_runner_program: defaults::soracloud_runtime::hf::LOCAL_RUNNER_PROGRAM.to_string(),
            local_runner_timeout_ms: DurationMs(std::time::Duration::from_millis(
                defaults::soracloud_runtime::hf::LOCAL_RUNNER_TIMEOUT_MS,
            )),
            model_host_heartbeat_ttl_ms: DurationMs(std::time::Duration::from_millis(
                defaults::soracloud_runtime::hf::MODEL_HOST_HEARTBEAT_TTL_MS,
            )),
            allow_inference_bridge_fallback:
                defaults::soracloud_runtime::hf::ALLOW_INFERENCE_BRIDGE_FALLBACK,
            import_max_files: defaults::soracloud_runtime::hf::IMPORT_MAX_FILES,
            import_max_file_bytes: defaults::soracloud_runtime::hf::IMPORT_MAX_FILE_BYTES,
            import_max_total_bytes: defaults::soracloud_runtime::hf::IMPORT_MAX_TOTAL_BYTES,
            import_file_allowlist: defaults::soracloud_runtime::hf::import_file_allowlist(),
            inference_token: None,
        }
    }
}

impl SoracloudRuntimeHuggingFace {
    fn parse(self) -> actual::SoracloudRuntimeHuggingFace {
        let mut import_file_allowlist = self
            .import_file_allowlist
            .into_iter()
            .map(|value| value.trim().to_ascii_lowercase())
            .filter(|value| !value.is_empty())
            .collect::<Vec<_>>();
        import_file_allowlist.sort();
        import_file_allowlist.dedup();
        let local_runner_program = {
            let trimmed = self.local_runner_program.trim();
            if trimmed.is_empty() {
                defaults::soracloud_runtime::hf::LOCAL_RUNNER_PROGRAM.to_owned()
            } else {
                trimmed.to_owned()
            }
        };

        actual::SoracloudRuntimeHuggingFace {
            hub_base_url: self.hub_base_url.trim().trim_end_matches('/').to_owned(),
            api_base_url: self.api_base_url.trim().trim_end_matches('/').to_owned(),
            inference_base_url: self
                .inference_base_url
                .trim()
                .trim_end_matches('/')
                .to_owned(),
            request_timeout: self.request_timeout_ms.get().max(MIN_TIMER_INTERVAL),
            local_execution_enabled: self.local_execution_enabled,
            local_runner_program,
            local_runner_timeout: self.local_runner_timeout_ms.get().max(MIN_TIMER_INTERVAL),
            model_host_heartbeat_ttl: self
                .model_host_heartbeat_ttl_ms
                .get()
                .max(MIN_TIMER_INTERVAL),
            allow_inference_bridge_fallback: self.allow_inference_bridge_fallback,
            import_max_files: self.import_max_files.max(1),
            import_max_file_bytes: self.import_max_file_bytes.max(1),
            import_max_total_bytes: self.import_max_total_bytes.max(1),
            import_file_allowlist,
            inference_token: self
                .inference_token
                .map(|value| value.trim().to_owned())
                .filter(|value| !value.is_empty()),
        }
    }
}

/// User-level configuration container for `Torii`.
#[derive(Debug, ReadConfig)]
pub struct Torii {
    /// Listening address for the public Torii API.
    #[config(env = "API_ADDRESS")]
    pub address: WithOrigin<SocketAddr>,
    /// Supported Torii API versions (semantic `major.minor`, oldest → newest).
    #[config(default = "defaults::torii::api_supported_versions()")]
    pub api_versions: Vec<String>,
    /// Default Torii API version assumed when the header is omitted.
    #[config(default = "defaults::torii::api_default_version()")]
    pub api_version_default: String,
    /// Minimum Torii API version required for proof/staking/fee endpoints.
    #[config(default = "defaults::torii::api_min_proof_version()")]
    pub api_min_proof_version: String,
    /// Optional unix timestamp when the oldest supported API version sunsets.
    pub api_version_sunset_unix: Option<u64>,
    /// Maximum HTTP payload size accepted by the API.
    #[config(default = "defaults::torii::MAX_CONTENT_LEN")]
    pub max_content_len: Bytes<u64>,
    /// Base directory for Torii persistence (attachments, webhooks, DA queues).
    #[config(default = "defaults::torii::data_dir()")]
    pub data_dir: PathBuf,
    /// Optional public key used to sign transaction submission receipts (must be paired).
    #[config(env = "TORII_RECEIPT_PUBLIC_KEY")]
    pub receipt_public_key: Option<PublicKey>,
    /// Optional private key used to sign transaction submission receipts (must be paired).
    #[config(env = "TORII_RECEIPT_PRIVATE_KEY")]
    pub receipt_private_key: Option<ExposedPrivateKey>,
    /// Maximum idle duration for long-running queries.
    #[config(default = "defaults::torii::QUERY_IDLE_TIME.into()")]
    pub query_idle_time_ms: DurationMs,
    /// The upper limit of the number of live queries.
    #[config(default = "defaults::torii::QUERY_STORE_CAPACITY")]
    pub query_store_capacity: NonZeroUsize,
    /// The upper limit of the number of live queries for a single user.
    #[config(default = "defaults::torii::QUERY_STORE_CAPACITY_PER_USER")]
    pub query_store_capacity_per_user: NonZeroUsize,
    /// Capacity of the broadcast channel used for Torii events/SSE/webhooks.
    #[config(default = "default_events_buffer_capacity()")]
    pub events_buffer_capacity: NonZeroUsize,
    /// WebSocket message timeout for Torii event/block streams (milliseconds).
    #[config(
        default = "DurationMs(std::time::Duration::from_millis(defaults::torii::WS_MESSAGE_TIMEOUT_MS))"
    )]
    pub ws_message_timeout_ms: DurationMs,
    /// Default page size for app-facing list/query endpoints.
    #[config(default = "defaults::torii::APP_API_DEFAULT_LIST_LIMIT")]
    pub app_api_default_list_limit: u32,
    /// Maximum page size accepted by app-facing list/query endpoints.
    #[config(default = "defaults::torii::APP_API_MAX_LIST_LIMIT")]
    pub app_api_max_list_limit: u32,
    /// Maximum fetch size accepted by app-facing iterable queries.
    #[config(default = "defaults::torii::APP_API_MAX_FETCH_SIZE")]
    pub app_api_max_fetch_size: u32,
    /// Rate-limiter cost applied per requested row on app-facing endpoints.
    #[config(default = "defaults::torii::APP_API_RATE_LIMIT_COST_PER_ROW")]
    pub app_api_rate_limit_cost_per_row: u32,
    /// Maximum allowed clock skew for signed app-facing canonical requests (seconds).
    #[config(default = "defaults::torii::app_auth::MAX_CLOCK_SKEW_SECS")]
    pub app_auth_max_clock_skew_secs: u64,
    /// TTL for signed app-facing request nonces retained for replay detection (seconds).
    #[config(default = "defaults::torii::app_auth::NONCE_TTL_SECS")]
    pub app_auth_nonce_ttl_secs: u64,
    /// Maximum number of app-facing request nonces held in memory for replay detection.
    #[config(default = "default_app_auth_replay_cache_capacity()")]
    pub app_auth_replay_cache_capacity: NonZeroUsize,
    /// Per-authority query rate (tokens/sec). None disables.
    pub query_rate_per_authority_per_sec: Option<u32>,
    /// Per-authority burst capacity (tokens). None disables.
    pub query_burst_per_authority: Option<u32>,
    /// Per-authority transaction submission rate (tokens/sec). None disables.
    pub tx_rate_per_authority_per_sec: Option<u32>,
    /// Per-authority transaction submission burst (tokens). None disables.
    pub tx_burst_per_authority: Option<u32>,
    /// Per-origin deploy rate (tokens/sec). None disables.
    pub deploy_rate_per_origin_per_sec: Option<u32>,
    /// Per-origin deploy burst tokens. None disables.
    pub deploy_burst_per_origin: Option<u32>,
    /// Public Soracloud local-read rate per remote IP (tokens/sec). None disables.
    pub soracloud_public_rate_per_ip_per_sec: Option<u32>,
    /// Public Soracloud local-read burst per remote IP (tokens). None disables.
    pub soracloud_public_burst_per_ip: Option<u32>,
    /// Maximum concurrent public Soracloud local-read executions.
    #[config(default = "defaults::torii::SORACLOUD_PUBLIC_MAX_INFLIGHT")]
    pub soracloud_public_max_inflight: NonZeroUsize,
    /// Proof endpoint steady-state rate (requests per minute). None disables.
    pub proof_rate_per_minute: Option<u32>,
    /// Proof endpoint burst tokens (requests).
    pub proof_burst: Option<u32>,
    /// Maximum proof request payload size (bytes).
    #[config(default = "defaults::torii::PROOF_MAX_BODY_BYTES")]
    pub proof_max_body_bytes: Bytes<u64>,
    /// Optional proof egress steady-state budget (bytes/sec). None disables.
    pub proof_egress_bytes_per_sec: Option<u64>,
    /// Optional proof egress burst budget (bytes). None disables.
    pub proof_egress_burst_bytes: Option<u64>,
    /// Maximum page size accepted by proof listings.
    #[config(default = "defaults::torii::PROOF_MAX_LIST_LIMIT")]
    pub proof_max_list_limit: u32,
    /// Wall-clock timeout applied to proof list/count handlers (milliseconds).
    #[config(
        default = "DurationMs(std::time::Duration::from_millis(defaults::torii::PROOF_REQUEST_TIMEOUT_MS))"
    )]
    pub proof_request_timeout_ms: DurationMs,
    /// Cache lifetime advertised for proof lookups (seconds).
    #[config(default = "defaults::torii::PROOF_CACHE_MAX_AGE_SECS")]
    pub proof_cache_max_age_secs: u64,
    /// Retry hint surfaced on throttling responses (seconds).
    #[config(default = "defaults::torii::PROOF_RETRY_AFTER_SECS")]
    pub proof_retry_after_secs: u64,
    /// Require a valid API token for app-facing endpoints.
    #[config(default = "defaults::torii::REQUIRE_API_TOKEN")]
    pub require_api_token: bool,
    /// Allowed API tokens (opaque strings). Empty means no tokens defined.
    pub api_tokens: Option<Vec<String>>,
    /// Optional fee policy: asset id used for operator fee.
    pub api_fee_asset_id: Option<String>,
    /// Optional fee policy: amount charged per endpoint use.
    pub api_fee_amount: Option<u64>,
    /// Optional fee policy: receiver account id for fees.
    pub api_fee_receiver: Option<String>,
    /// CIDR allowlist for bypassing API rate limits (IPv4/IPv6), e.g. `127.0.0.0/8`.
    pub api_allow_cidrs: Option<Vec<String>>,
    /// Optional Torii base URLs used to fetch peer telemetry metadata.
    #[config(default)]
    pub peer_telemetry_urls: Vec<Url>,
    /// Peer telemetry geo lookup configuration (disabled by default).
    #[config(nested)]
    pub peer_geo: ToriiPeerGeo,
    /// SoraNet privacy ingestion guard rails (auth/rate/namespace).
    #[config(nested)]
    pub soranet_privacy_ingest: crate::parameters::user::ToriiSoranetPrivacyIngest,
    /// Emit filter-match debug traces (developer diagnostics only).
    #[config(default = "defaults::torii::DEBUG_MATCH_FILTERS")]
    pub debug_match_filters: bool,
    /// Operator authentication policy for operator-facing endpoints.
    #[config(nested)]
    pub operator_auth: ToriiOperatorAuth,
    /// Operator request-signature authentication policy for operator-facing endpoints.
    #[config(nested)]
    pub operator_signatures: ToriiOperatorSignatures,
    /// Maximum concurrent pre-auth connections across all clients.
    pub preauth_max_connections: Option<NonZeroUsize>,
    /// Maximum concurrent pre-auth connections per IP.
    pub preauth_max_connections_per_ip: Option<NonZeroUsize>,
    /// Pre-auth handshake rate per IP (tokens/sec). None disables.
    pub preauth_rate_per_ip_per_sec: Option<u32>,
    /// Pre-auth handshake burst per IP (tokens). None disables.
    pub preauth_burst_per_ip: Option<u32>,
    /// Temporary ban duration applied after rate/limit violations (milliseconds).
    pub preauth_ban_duration_ms: Option<DurationMs>,
    /// CIDR allowlist for bypassing pre-auth gating (IPv4/IPv6).
    pub preauth_allow_cidrs: Option<Vec<String>>,
    /// Optional per-scheme concurrency caps for the pre-auth gate.
    ///
    /// Each entry enforces a dedicated connection bucket using the provided
    /// scheme label (e.g., `http`, `ws`, `norito_rpc`). Limits only apply to the
    /// listed schemes; others inherit the global cap.
    pub preauth_scheme_limits: Option<Vec<PreauthSchemeLimit>>,
    /// Optional high-load threshold (# queued txs) to enable rate limiting.
    pub api_high_load_tx_threshold: Option<usize>,
    /// Optional high-load threshold for streaming endpoints (queued txs).
    pub api_high_load_stream_threshold: Option<usize>,
    /// Optional high-load threshold for subscription WS endpoint.
    pub api_high_load_subscription_threshold: Option<usize>,
    /// Attachments TTL (seconds) for ZK attachments (app API).
    #[config(
        env = "TORII_ATTACHMENTS_TTL_SECS",
        default = "defaults::torii::ATTACHMENTS_TTL_SECS"
    )]
    pub attachments_ttl_secs: u64,
    /// Maximum size per ZK attachment (bytes) for app API.
    #[config(
        env = "TORII_ATTACHMENTS_MAX_BYTES",
        default = "defaults::torii::ATTACHMENTS_MAX_BYTES"
    )]
    pub attachments_max_bytes: u64,
    /// Maximum number of attachments retained per tenant (0 = unlimited).
    #[config(
        env = "TORII_ATTACHMENTS_PER_TENANT_MAX_COUNT",
        default = "defaults::torii::ATTACHMENTS_PER_TENANT_MAX_COUNT"
    )]
    pub attachments_per_tenant_max_count: u64,
    /// Maximum aggregate attachment bytes retained per tenant (0 = unlimited).
    #[config(
        env = "TORII_ATTACHMENTS_PER_TENANT_MAX_BYTES",
        default = "defaults::torii::ATTACHMENTS_PER_TENANT_MAX_BYTES"
    )]
    pub attachments_per_tenant_max_bytes: u64,
    /// Allowed MIME types for attachment payloads (post-sniff).
    #[config(default = "defaults::torii::attachments_allowed_mime_types()")]
    pub attachments_allowed_mime_types: Vec<String>,
    /// Maximum expanded bytes allowed when decompressing attachments.
    #[config(default = "defaults::torii::ATTACHMENTS_MAX_EXPANDED_BYTES")]
    pub attachments_max_expanded_bytes: u64,
    /// Maximum nested archive depth allowed when decompressing attachments.
    #[config(default = "defaults::torii::ATTACHMENTS_MAX_ARCHIVE_DEPTH")]
    pub attachments_max_archive_depth: u32,
    /// Attachment sanitization execution mode (`subprocess` or `in_process`).
    #[config(default = "defaults::torii::ATTACHMENTS_SANITIZER_MODE.to_string()")]
    pub attachments_sanitizer_mode: String,
    /// Attachment sanitization timeout (milliseconds).
    #[config(default = "defaults::torii::ATTACHMENTS_SANITIZE_TIMEOUT_MS")]
    pub attachments_sanitize_timeout_ms: u64,
    /// Enable background ZK prover worker (non-consensus, app API).
    #[config(
        env = "TORII_ZK_PROVER_ENABLED",
        default = "defaults::torii::ZK_PROVER_ENABLED"
    )]
    pub zk_prover_enabled: bool,
    /// Background ZK prover scan period (seconds).
    #[config(
        env = "TORII_ZK_PROVER_SCAN_PERIOD_SECS",
        default = "defaults::torii::ZK_PROVER_SCAN_PERIOD_SECS"
    )]
    pub zk_prover_scan_period_secs: u64,
    /// Background ZK prover reports retention TTL (seconds).
    #[config(
        env = "TORII_ZK_PROVER_REPORTS_TTL_SECS",
        default = "defaults::torii::ZK_PROVER_REPORTS_TTL_SECS"
    )]
    pub zk_prover_reports_ttl_secs: u64,
    /// Maximum number of attachments processed concurrently by the prover worker.
    #[config(
        env = "TORII_ZK_PROVER_MAX_INFLIGHT",
        default = "defaults::torii::ZK_PROVER_MAX_INFLIGHT"
    )]
    pub zk_prover_max_inflight: usize,
    /// Maximum aggregate attachment bytes processed per scan cycle.
    #[config(
        env = "TORII_ZK_PROVER_MAX_SCAN_BYTES",
        default = "defaults::torii::ZK_PROVER_MAX_SCAN_BYTES"
    )]
    pub zk_prover_max_scan_bytes: u64,
    /// Maximum wall-clock time (milliseconds) spent in a single scan cycle.
    #[config(
        env = "TORII_ZK_PROVER_MAX_SCAN_MILLIS",
        default = "defaults::torii::ZK_PROVER_MAX_SCAN_MILLIS"
    )]
    pub zk_prover_max_scan_millis: u64,
    /// Directory containing verifying key bytes for the background prover.
    #[config(default = "defaults::torii::zk_prover_keys_dir()")]
    pub zk_prover_keys_dir: PathBuf,
    /// Allowlisted backend prefixes for the background prover (empty = allow all).
    #[config(default = "defaults::torii::zk_prover_allowed_backends()")]
    pub zk_prover_allowed_backends: Vec<String>,
    /// Allowlisted circuit identifiers for the background prover (empty = allow all).
    #[config(default = "defaults::torii::zk_prover_allowed_circuits()")]
    pub zk_prover_allowed_circuits: Vec<String>,
    /// Maximum number of concurrent ZK IVM prove jobs handled by Torii.
    ///
    /// Applies to the non-consensus helper endpoint `POST /v1/zk/ivm/prove`.
    #[config(
        env = "TORII_ZK_IVM_PROVE_MAX_INFLIGHT",
        default = "defaults::torii::ZK_IVM_PROVE_MAX_INFLIGHT"
    )]
    pub zk_ivm_prove_max_inflight: usize,
    /// Maximum number of queued ZK IVM prove jobs accepted while inflight is saturated.
    ///
    /// Applies to the non-consensus helper endpoint `POST /v1/zk/ivm/prove`.
    #[config(
        env = "TORII_ZK_IVM_PROVE_MAX_QUEUE",
        default = "defaults::torii::ZK_IVM_PROVE_MAX_QUEUE"
    )]
    pub zk_ivm_prove_max_queue: usize,
    /// TTL (seconds) for `/v1/zk/ivm/prove` job status entries.
    #[config(
        env = "TORII_ZK_IVM_PROVE_JOB_TTL_SECS",
        default = "defaults::torii::ZK_IVM_PROVE_JOB_TTL_SECS"
    )]
    pub zk_ivm_prove_job_ttl_secs: u64,
    /// Maximum number of `/v1/zk/ivm/prove` job status entries retained in memory.
    ///
    /// Set to 0 to disable the cap (not recommended).
    #[config(
        env = "TORII_ZK_IVM_PROVE_JOB_MAX_ENTRIES",
        default = "defaults::torii::ZK_IVM_PROVE_JOB_MAX_ENTRIES"
    )]
    pub zk_ivm_prove_job_max_entries: usize,
    /// Push notification configuration (feature-gated in runtime).
    #[config(nested)]
    pub push: ToriiPush,
    /// Iroha Connect configuration.
    #[config(nested)]
    pub connect: Connect,
    /// ISO 20022 bridge configuration.
    #[config(nested)]
    pub iso_bridge: IsoBridge,
    /// RBC sampling endpoint configuration.
    #[config(nested)]
    pub rbc_sampling: RbcSampling,
    /// Data-availability ingest configuration.
    #[config(nested)]
    pub da_ingest: DaIngest,
    /// SoraFS configuration (discovery + storage + repair/GC).
    #[config(nested)]
    pub sorafs: Sorafs,
    /// Transport-specific configuration (Norito-RPC rollout, streaming knobs).
    #[config(nested)]
    pub transport: ToriiTransport,
    /// Native MCP endpoint configuration.
    #[config(nested)]
    pub mcp: ToriiMcp,
    /// Webhook delivery/backpressure configuration.
    #[config(nested)]
    pub webhook: Webhook,
    /// Webhook destination security configuration (SSRF guard rails).
    #[config(nested)]
    pub webhook_security: WebhookSecurity,
    /// Optional UAID onboarding authority wiring for app API endpoints.
    pub onboarding: Option<ToriiOnboarding>,
    /// Optional faucet configuration for app API endpoints.
    pub faucet: Option<ToriiFaucet>,
    /// Optional offline certificate issuer configuration for app API endpoints.
    pub offline_issuer: Option<ToriiOfflineIssuer>,
    /// Optional RAM-LFE runtime configuration for app API endpoints.
    pub ram_lfe: Option<ToriiRamLfe>,
    /// Optional transaction-history visibility/auth configuration for direct wallet reads.
    pub tx_history: Option<ToriiTxHistory>,
}

/// Geo lookup configuration for peer telemetry.
#[derive(Debug, ReadConfig, Clone, norito::JsonDeserialize)]
pub struct ToriiPeerGeo {
    /// Enable geo lookups for peer telemetry.
    #[config(default = "defaults::torii::peer_geo::ENABLED")]
    pub enabled: bool,
    /// Optional geo endpoint (ip-api compatible).
    pub endpoint: Option<Url>,
}

impl Default for ToriiPeerGeo {
    fn default() -> Self {
        Self {
            enabled: defaults::torii::peer_geo::ENABLED,
            endpoint: defaults::torii::peer_geo::endpoint(),
        }
    }
}

impl ToriiPeerGeo {
    fn parse(self) -> actual::ToriiPeerGeo {
        actual::ToriiPeerGeo {
            enabled: self.enabled,
            endpoint: self.endpoint.or(defaults::torii::peer_geo::endpoint()),
        }
    }
}

#[cfg(test)]
mod torii_peer_geo_tests {
    use super::*;

    #[test]
    fn torii_peer_geo_parse_copies_enabled_and_endpoint() {
        let endpoint = Url::parse("https://geo.example").expect("valid endpoint");
        let parsed = ToriiPeerGeo {
            enabled: true,
            endpoint: Some(endpoint.clone()),
        }
        .parse();
        assert!(parsed.enabled);
        assert_eq!(
            parsed.endpoint.as_ref().map(Url::as_str),
            Some(endpoint.as_str())
        );
    }

    #[test]
    fn torii_peer_geo_parse_uses_default_endpoint_when_missing() {
        let parsed = ToriiPeerGeo {
            enabled: true,
            endpoint: None,
        }
        .parse();
        assert_eq!(parsed.endpoint, defaults::torii::peer_geo::endpoint());
    }
}

/// Guard rails for SoraNet privacy ingestion endpoints.
#[derive(Debug, ReadConfig, Clone, norito::JsonDeserialize)]
pub struct ToriiSoranetPrivacyIngest {
    /// Master enable switch for the `/v1/soranet/privacy/*` endpoints.
    #[config(default = "defaults::torii::soranet_privacy_ingest::ENABLED")]
    pub enabled: bool,
    /// Require a token header before accepting telemetry.
    #[config(default = "defaults::torii::soranet_privacy_ingest::REQUIRE_TOKEN")]
    pub require_token: bool,
    /// Accepted token values.
    #[config(default = "defaults::torii::soranet_privacy_ingest::tokens()")]
    pub tokens: Vec<String>,
    /// Requests-per-second budget (None disables limiting).
    pub rate_per_sec: Option<u32>,
    /// Burst capacity for the ingest limiter.
    pub burst: Option<u32>,
    /// CIDR allow-list for trusted submitters; empty -> deny.
    #[config(default = "defaults::torii::soranet_privacy_ingest::allow_cidrs()")]
    pub allow_cidrs: Vec<String>,
}

impl Default for ToriiSoranetPrivacyIngest {
    fn default() -> Self {
        Self {
            enabled: defaults::torii::soranet_privacy_ingest::ENABLED,
            require_token: defaults::torii::soranet_privacy_ingest::REQUIRE_TOKEN,
            tokens: defaults::torii::soranet_privacy_ingest::tokens(),
            rate_per_sec: defaults::torii::soranet_privacy_ingest::RATE_PER_SEC,
            burst: defaults::torii::soranet_privacy_ingest::BURST,
            allow_cidrs: defaults::torii::soranet_privacy_ingest::allow_cidrs(),
        }
    }
}

impl ToriiSoranetPrivacyIngest {
    fn parse(self) -> actual::SoranetPrivacyIngest {
        actual::SoranetPrivacyIngest {
            enabled: self.enabled,
            require_token: self.require_token,
            tokens: self.tokens,
            rate_per_sec: self
                .rate_per_sec
                .or(defaults::torii::soranet_privacy_ingest::RATE_PER_SEC)
                .and_then(std::num::NonZeroU32::new),
            burst: self
                .burst
                .or(defaults::torii::soranet_privacy_ingest::BURST)
                .and_then(std::num::NonZeroU32::new),
            allow_cidrs: self.allow_cidrs,
        }
    }
}

/// Push-notification configuration (FCM/APNS bridge).
#[derive(Debug, ReadConfig, Clone, norito::JsonDeserialize)]
pub struct ToriiPush {
    /// Master enable switch for the push bridge.
    #[config(default = "defaults::torii::PUSH_ENABLED")]
    pub enabled: bool,
    /// Optional steady-state rate (requests per minute). None disables.
    pub rate_per_minute: Option<u32>,
    /// Optional burst tokens for push notifications.
    pub burst: Option<u32>,
    /// HTTP connect timeout for push delivery.
    #[config(
        default = "DurationMs(std::time::Duration::from_millis(defaults::torii::PUSH_CONNECT_TIMEOUT_MS))"
    )]
    pub connect_timeout_ms: DurationMs,
    /// HTTP request timeout for push delivery.
    #[config(
        default = "DurationMs(std::time::Duration::from_millis(defaults::torii::PUSH_REQUEST_TIMEOUT_MS))"
    )]
    pub request_timeout_ms: DurationMs,
    /// Maximum topics recorded per registered device.
    #[config(default = "defaults::torii::PUSH_MAX_TOPICS_PER_DEVICE")]
    pub max_topics_per_device: usize,
    /// Optional FCM API key for push delivery.
    pub fcm_api_key: Option<String>,
    /// Optional APNS endpoint base URL.
    pub apns_endpoint: Option<String>,
    /// Optional APNS auth token (e.g., JWT).
    pub apns_auth_token: Option<String>,
}

impl Default for ToriiPush {
    fn default() -> Self {
        Self {
            enabled: defaults::torii::PUSH_ENABLED,
            rate_per_minute: defaults::torii::PUSH_RATE_PER_MINUTE,
            burst: defaults::torii::PUSH_BURST,
            connect_timeout_ms: DurationMs(std::time::Duration::from_millis(
                defaults::torii::PUSH_CONNECT_TIMEOUT_MS,
            )),
            request_timeout_ms: DurationMs(std::time::Duration::from_millis(
                defaults::torii::PUSH_REQUEST_TIMEOUT_MS,
            )),
            max_topics_per_device: defaults::torii::PUSH_MAX_TOPICS_PER_DEVICE,
            fcm_api_key: None,
            apns_endpoint: None,
            apns_auth_token: None,
        }
    }
}

impl ToriiPush {
    fn parse(self) -> actual::Push {
        actual::Push {
            enabled: self.enabled,
            rate_per_minute: self
                .rate_per_minute
                .or(defaults::torii::PUSH_RATE_PER_MINUTE)
                .and_then(std::num::NonZeroU32::new),
            burst: self
                .burst
                .or(defaults::torii::PUSH_BURST)
                .and_then(std::num::NonZeroU32::new),
            connect_timeout: self.connect_timeout_ms.get(),
            request_timeout: self.request_timeout_ms.get(),
            max_topics_per_device: std::num::NonZeroUsize::new(self.max_topics_per_device.max(1))
                .unwrap_or(nonzero!(1_usize)),
            fcm_api_key: self.fcm_api_key,
            apns_endpoint: self.apns_endpoint,
            apns_auth_token: self.apns_auth_token,
        }
    }
}

impl Torii {
    fn parse_receipt_signer(
        receipt_public_key: Option<&PublicKey>,
        receipt_private_key: Option<&ExposedPrivateKey>,
    ) -> Option<KeyPair> {
        match (receipt_public_key, receipt_private_key) {
            (None, None) => None,
            (Some(_), None) => {
                panic!(
                    "torii.receipt_private_key must be set when torii.receipt_public_key is set"
                );
            }
            (None, Some(_)) => {
                panic!(
                    "torii.receipt_public_key must be set when torii.receipt_private_key is set"
                );
            }
            (Some(public_key), Some(private_key)) => {
                let key_pair = KeyPair::new(public_key.clone(), private_key.0.clone())
                    .unwrap_or_else(|err| panic!("invalid torii receipt key pair: {err}"));
                if matches!(
                    key_pair.public_key().algorithm(),
                    Algorithm::BlsNormal | Algorithm::BlsSmall
                ) {
                    panic!("torii.receipt_* must not use BLS keys; use ed25519 or secp256k1");
                }
                Some(key_pair)
            }
        }
    }

    fn parse(self) -> (actual::Torii, actual::LiveQueryStore) {
        let configured_versions = if self.api_versions.is_empty() {
            super::defaults::torii::api_supported_versions()
        } else {
            self.api_versions.clone()
        };
        let supported_versions = normalize_version_list(configured_versions, "torii.api_versions");
        let default_version = ApiVersionLabel::parse(&self.api_version_default).unwrap_or_else(|| {
            panic!(
                "invalid `torii.api_version_default` `{}`; expected a semantic version like `1.0`",
                self.api_version_default
            )
        });
        let min_proof_version =
            ApiVersionLabel::parse(&self.api_min_proof_version).unwrap_or_else(|| {
                panic!(
                    "invalid `torii.api_min_proof_version` `{}`; expected a semantic version like `1.0`",
                    self.api_min_proof_version
                )
            });
        let newest_supported = *supported_versions
            .last()
            .expect("torii.api_versions contains at least one entry");
        if !supported_versions.contains(&default_version) {
            panic!(
                "`torii.api_version_default` (`{}`) must be listed in `torii.api_versions`",
                self.api_version_default
            );
        }
        if !supported_versions.contains(&min_proof_version) {
            panic!(
                "`torii.api_min_proof_version` (`{}`) must be listed in `torii.api_versions`",
                self.api_min_proof_version
            );
        }
        if min_proof_version > newest_supported {
            panic!(
                "`torii.api_min_proof_version` (`{}`) exceeds the newest supported version `{}`",
                self.api_min_proof_version,
                newest_supported.render()
            );
        }
        let api_versions: Vec<String> = supported_versions
            .iter()
            .map(ApiVersionLabel::render)
            .collect();
        let api_version_default = default_version.render();
        let api_min_proof_version = min_proof_version.render();
        let api_version_sunset_unix = self
            .api_version_sunset_unix
            .or(super::defaults::torii::API_SUNSET_UNIX);
        let rbc_sampling = self.build_rbc_sampling();
        let default_list_limit = std::num::NonZeroU32::new(self.app_api_default_list_limit.max(1))
            .unwrap_or(nonzero!(1_u32));
        let max_list_limit = std::num::NonZeroU32::new(
            self.app_api_max_list_limit
                .max(default_list_limit.get())
                .max(1),
        )
        .unwrap_or(default_list_limit);
        let max_fetch_size = std::num::NonZeroU32::new(self.app_api_max_fetch_size.max(1))
            .unwrap_or(nonzero!(1_u32));
        let rate_limit_cost_per_row =
            std::num::NonZeroU32::new(self.app_api_rate_limit_cost_per_row.max(1))
                .unwrap_or(nonzero!(1_u32));
        let webhook = self.webhook.parse();
        let webhook_security = self.webhook_security.parse();
        let push = self.push.parse();
        let (
            sorafs_storage,
            sorafs_discovery,
            sorafs_repair,
            sorafs_gc,
            sorafs_quota,
            sorafs_alias_cache,
            sorafs_gateway,
            sorafs_por,
        ) = self.sorafs.parse();
        let receipt_signer = Self::parse_receipt_signer(
            self.receipt_public_key.as_ref(),
            self.receipt_private_key.as_ref(),
        );
        let torii = actual::Torii {
            address: self.address,
            api_versions,
            api_version_default,
            api_min_proof_version,
            api_version_sunset_unix,
            max_content_len: self.max_content_len,
            data_dir: self.data_dir,
            receipt_signer,
            events_buffer_capacity: self.events_buffer_capacity,
            ws_message_timeout: self.ws_message_timeout_ms.get(),
            query_rate_per_authority_per_sec: self
                .query_rate_per_authority_per_sec
                .or(super::defaults::torii::QUERY_RATE_PER_AUTHORITY_PER_SEC)
                .and_then(std::num::NonZeroU32::new),
            query_burst_per_authority: self
                .query_burst_per_authority
                .or(super::defaults::torii::QUERY_BURST_PER_AUTHORITY)
                .and_then(std::num::NonZeroU32::new),
            tx_rate_per_authority_per_sec: self
                .tx_rate_per_authority_per_sec
                .or(super::defaults::torii::TX_RATE_PER_AUTHORITY_PER_SEC)
                .and_then(std::num::NonZeroU32::new),
            tx_burst_per_authority: self
                .tx_burst_per_authority
                .or(super::defaults::torii::TX_BURST_PER_AUTHORITY)
                .and_then(std::num::NonZeroU32::new),
            deploy_rate_per_origin_per_sec: self
                .deploy_rate_per_origin_per_sec
                .or(super::defaults::torii::DEPLOY_RATE_PER_ORIGIN_PER_SEC)
                .and_then(std::num::NonZeroU32::new),
            deploy_burst_per_origin: self
                .deploy_burst_per_origin
                .or(super::defaults::torii::DEPLOY_BURST_PER_ORIGIN)
                .and_then(std::num::NonZeroU32::new),
            soracloud_public_rate_per_ip_per_sec: self
                .soracloud_public_rate_per_ip_per_sec
                .or(super::defaults::torii::SORACLOUD_PUBLIC_RATE_PER_IP_PER_SEC)
                .and_then(std::num::NonZeroU32::new),
            soracloud_public_burst_per_ip: self
                .soracloud_public_burst_per_ip
                .or(super::defaults::torii::SORACLOUD_PUBLIC_BURST_PER_IP)
                .and_then(std::num::NonZeroU32::new),
            soracloud_public_max_inflight: self.soracloud_public_max_inflight,
            proof_api: actual::ProofApi {
                rate_per_minute: self
                    .proof_rate_per_minute
                    .or(super::defaults::torii::PROOF_RATE_PER_MIN)
                    .and_then(std::num::NonZeroU32::new),
                burst: self
                    .proof_burst
                    .or(super::defaults::torii::PROOF_BURST)
                    .and_then(std::num::NonZeroU32::new),
                max_body_bytes: self.proof_max_body_bytes,
                egress_bytes_per_sec: self
                    .proof_egress_bytes_per_sec
                    .or(super::defaults::torii::PROOF_EGRESS_BYTES_PER_SEC)
                    .and_then(std::num::NonZeroU64::new),
                egress_burst_bytes: self
                    .proof_egress_burst_bytes
                    .or(super::defaults::torii::PROOF_EGRESS_BURST_BYTES)
                    .and_then(std::num::NonZeroU64::new),
                max_list_limit: std::num::NonZeroU32::new(self.proof_max_list_limit.max(1))
                    .expect("proof_max_list_limit must be non-zero"),
                request_timeout: self.proof_request_timeout_ms.get(),
                cache_max_age: Duration::from_secs(self.proof_cache_max_age_secs.max(1)),
                retry_after: Duration::from_secs(self.proof_retry_after_secs.max(1)),
            },
            require_api_token: self.require_api_token,
            api_tokens: self.api_tokens.unwrap_or_default(),
            api_fee_asset_id: self.api_fee_asset_id,
            api_fee_amount: self.api_fee_amount,
            api_fee_receiver: self.api_fee_receiver,
            api_allow_cidrs: self.api_allow_cidrs.unwrap_or_default(),
            peer_telemetry_urls: self.peer_telemetry_urls,
            peer_geo: self.peer_geo.parse(),
            soranet_privacy_ingest: self.soranet_privacy_ingest.parse(),
            debug_match_filters: self.debug_match_filters,
            operator_auth: self.operator_auth.parse(),
            operator_signatures: self.operator_signatures.parse(),
            preauth_max_connections: self
                .preauth_max_connections
                .or(super::defaults::torii::PREAUTH_MAX_CONNECTIONS),
            preauth_max_connections_per_ip: self
                .preauth_max_connections_per_ip
                .or(super::defaults::torii::PREAUTH_MAX_CONNECTIONS_PER_IP),
            preauth_rate_per_ip_per_sec: self
                .preauth_rate_per_ip_per_sec
                .or(super::defaults::torii::PREAUTH_RATE_PER_IP_PER_SEC)
                .and_then(std::num::NonZeroU32::new),
            preauth_burst_per_ip: self
                .preauth_burst_per_ip
                .or(super::defaults::torii::PREAUTH_BURST_PER_IP)
                .and_then(std::num::NonZeroU32::new),
            preauth_temp_ban: Some(
                self.preauth_ban_duration_ms
                    .unwrap_or_else(|| super::defaults::torii::PREAUTH_BAN_DURATION.into())
                    .get(),
            ),
            preauth_allow_cidrs: self.preauth_allow_cidrs.unwrap_or_default(),
            preauth_scheme_limits: self
                .preauth_scheme_limits
                .unwrap_or_default()
                .into_iter()
                .map(|limit| actual::PreauthSchemeLimit {
                    scheme: limit.scheme,
                    max_connections: limit.max_connections,
                })
                .collect(),
            api_high_load_tx_threshold: self.api_high_load_tx_threshold,
            api_high_load_stream_threshold: self.api_high_load_stream_threshold,
            api_high_load_subscription_threshold: self.api_high_load_subscription_threshold,
            attachments_ttl_secs: self.attachments_ttl_secs,
            attachments_max_bytes: self.attachments_max_bytes,
            attachments_per_tenant_max_count: self.attachments_per_tenant_max_count,
            attachments_per_tenant_max_bytes: self.attachments_per_tenant_max_bytes,
            attachments_allowed_mime_types: self.attachments_allowed_mime_types,
            attachments_max_expanded_bytes: self.attachments_max_expanded_bytes,
            attachments_max_archive_depth: self.attachments_max_archive_depth,
            attachments_sanitizer_mode: parse_attachment_sanitizer_mode(
                &self.attachments_sanitizer_mode,
            ),
            attachments_sanitize_timeout_ms: self.attachments_sanitize_timeout_ms,
            zk_prover_enabled: self.zk_prover_enabled,
            zk_prover_scan_period_secs: self.zk_prover_scan_period_secs,
            zk_prover_reports_ttl_secs: self.zk_prover_reports_ttl_secs,
            zk_prover_max_inflight: self.zk_prover_max_inflight,
            zk_prover_max_scan_bytes: self.zk_prover_max_scan_bytes,
            zk_prover_max_scan_millis: self.zk_prover_max_scan_millis,
            zk_prover_keys_dir: self.zk_prover_keys_dir,
            zk_prover_allowed_backends: self.zk_prover_allowed_backends,
            zk_prover_allowed_circuits: self.zk_prover_allowed_circuits,
            zk_ivm_prove_max_inflight: self.zk_ivm_prove_max_inflight,
            zk_ivm_prove_max_queue: self.zk_ivm_prove_max_queue,
            zk_ivm_prove_job_ttl_secs: self.zk_ivm_prove_job_ttl_secs,
            zk_ivm_prove_job_max_entries: self.zk_ivm_prove_job_max_entries,
            connect: self.connect.parse(),
            iso_bridge: self.iso_bridge.parse(),
            rbc_sampling,
            da_ingest: self.da_ingest.parse(),
            sorafs_discovery,
            sorafs_storage,
            sorafs_repair,
            sorafs_gc,
            sorafs_quota,
            sorafs_alias_cache,
            sorafs_gateway,
            sorafs_por,
            transport: self.transport.into(),
            mcp: self.mcp.into(),
            webhook,
            webhook_security,
            push,
            onboarding: self.onboarding.and_then(ToriiOnboarding::parse),
            faucet: self.faucet.and_then(ToriiFaucet::parse),
            offline_issuer: self.offline_issuer.and_then(ToriiOfflineIssuer::parse),
            ram_lfe: self.ram_lfe.and_then(ToriiRamLfe::parse),
            tx_history: self.tx_history.map(ToriiTxHistory::parse),
            app_api: actual::AppApi {
                default_list_limit,
                max_list_limit,
                max_fetch_size,
                rate_limit_cost_per_row,
                request_signature_max_clock_skew: Duration::from_secs(
                    self.app_auth_max_clock_skew_secs,
                ),
                request_signature_nonce_ttl: Duration::from_secs(self.app_auth_nonce_ttl_secs),
                request_signature_replay_cache_capacity: self.app_auth_replay_cache_capacity,
            },
        };

        let query = actual::LiveQueryStore {
            idle_time: self.query_idle_time_ms.get(),
            capacity: self.query_store_capacity,
            capacity_per_user: self.query_store_capacity_per_user,
        };

        (torii, query)
    }

    fn build_rbc_sampling(&self) -> actual::RbcSampling {
        actual::RbcSampling {
            enabled: self.rbc_sampling.enabled,
            max_samples_per_request: self.rbc_sampling.max_samples_per_request,
            max_bytes_per_request: self.rbc_sampling.max_bytes_per_request,
            daily_byte_budget: self.rbc_sampling.daily_byte_budget,
            rate_per_minute: self
                .rbc_sampling
                .rate_per_minute
                .or(super::defaults::torii::RBC_SAMPLING_RATE_PER_MIN)
                .and_then(std::num::NonZeroU32::new),
        }
    }
}

/// Transaction-history visibility/auth configuration for Torii app API endpoints.
#[derive(Debug, ReadConfig, Clone, norito::JsonDeserialize)]
pub struct ToriiTxHistory {
    /// Optional dataspace-keyed mandatory-alias policy file.
    pub mandatory_aliases_path: Option<PathBuf>,
    /// Optional asset-definition restriction applied to visible-history endpoints.
    pub allowed_asset_definition_id: Option<String>,
    /// Optional JWT bearer verification configuration.
    pub jwt: Option<ToriiTxHistoryJwt>,
}

impl ToriiTxHistory {
    fn parse(self) -> actual::ToriiTxHistory {
        let allowed_asset_definition_id = self.allowed_asset_definition_id.map(|value| {
            AssetDefinitionId::parse_address_literal(&value).unwrap_or_else(|err| {
                panic!("invalid torii.tx_history.allowed_asset_definition_id `{value}`: {err}")
            })
        });
        actual::ToriiTxHistory {
            mandatory_aliases_path: self.mandatory_aliases_path,
            allowed_asset_definition_id,
            jwt: self.jwt.map(ToriiTxHistoryJwt::parse),
        }
    }
}

/// JWT bearer verification inputs for transaction-history endpoints.
#[derive(Debug, ReadConfig, Clone, norito::JsonDeserialize)]
pub struct ToriiTxHistoryJwt {
    /// Expected JWT algorithm label (for example `RS256` or `HS256`).
    pub algorithm: String,
    /// Shared-secret material used for HMAC JWT algorithms.
    pub secret: Option<String>,
    /// PEM-encoded public key used for asymmetric JWT algorithms.
    pub public_key_pem: Option<String>,
    /// Optional issuer constraint.
    pub issuer: Option<String>,
    /// Optional audience constraint.
    pub audience: Option<String>,
}

impl ToriiTxHistoryJwt {
    fn parse(self) -> actual::ToriiTxHistoryJwt {
        let algorithm = self.algorithm.trim().to_ascii_uppercase();
        if algorithm.is_empty() {
            panic!("torii.tx_history.jwt.algorithm must not be empty");
        }
        match algorithm.as_str() {
            "HS256" | "HS384" | "HS512" => {
                let secret = self.secret.filter(|value| !value.trim().is_empty());
                if secret.is_none() {
                    panic!("torii.tx_history.jwt.secret must be set for HMAC JWT algorithms");
                }
                actual::ToriiTxHistoryJwt {
                    algorithm,
                    secret,
                    public_key_pem: None,
                    issuer: self.issuer.filter(|value| !value.trim().is_empty()),
                    audience: self.audience.filter(|value| !value.trim().is_empty()),
                }
            }
            "RS256" | "RS384" | "RS512" | "PS256" | "PS384" | "PS512" | "ES256" | "ES384"
            | "EDDSA" => {
                let public_key_pem = self.public_key_pem.filter(|value| !value.trim().is_empty());
                if public_key_pem.is_none() {
                    panic!(
                        "torii.tx_history.jwt.public_key_pem must be set for asymmetric JWT algorithms"
                    );
                }
                actual::ToriiTxHistoryJwt {
                    algorithm,
                    secret: None,
                    public_key_pem,
                    issuer: self.issuer.filter(|value| !value.trim().is_empty()),
                    audience: self.audience.filter(|value| !value.trim().is_empty()),
                }
            }
            other => panic!(
                "invalid torii.tx_history.jwt.algorithm `{other}`; expected HS256/384/512, RS256/384/512, PS256/384/512, ES256/384, or EdDSA"
            ),
        }
    }
}

/// Operator request-signature authentication configuration for Torii operator endpoints.
#[derive(Debug, ReadConfig, Clone, norito::JsonDeserialize)]
pub struct ToriiOperatorSignatures {
    /// Master enable switch for operator signature authentication.
    #[config(default = "defaults::torii::operator_signatures::ENABLED")]
    pub enabled: bool,
    /// Allow the node identity key (from `[common]`) to authenticate operator endpoints.
    #[config(default = "defaults::torii::operator_signatures::ALLOW_NODE_KEY")]
    pub allow_node_key: bool,
    /// Additional allow-listed operator public keys.
    #[config(default = "defaults::torii::operator_signatures::allowed_public_keys()")]
    pub allowed_public_keys: Vec<PublicKey>,
    /// Maximum allowed clock skew for signed operator requests (seconds).
    #[config(default = "defaults::torii::operator_signatures::MAX_CLOCK_SKEW_SECS")]
    pub max_clock_skew_secs: u64,
    /// TTL for operator nonces retained for replay detection (seconds).
    #[config(default = "defaults::torii::operator_signatures::NONCE_TTL_SECS")]
    pub nonce_ttl_secs: u64,
    /// Maximum number of nonces retained for replay detection.
    #[config(default = "defaults::torii::operator_signatures::REPLAY_CACHE_CAPACITY")]
    pub replay_cache_capacity: usize,
}

impl ToriiOperatorSignatures {
    fn parse(self) -> actual::ToriiOperatorSignatures {
        actual::ToriiOperatorSignatures {
            enabled: self.enabled,
            allow_node_key: self.allow_node_key,
            allowed_public_keys: self.allowed_public_keys,
            max_clock_skew: Duration::from_secs(self.max_clock_skew_secs),
            nonce_ttl: Duration::from_secs(self.nonce_ttl_secs.max(1)),
            replay_cache_capacity: std::num::NonZeroUsize::new(self.replay_cache_capacity.max(1))
                .expect("operator signatures replay cache must be non-zero"),
        }
    }
}

/// Operator authentication configuration for Torii operator endpoints.
#[derive(Debug, ReadConfig, Clone, norito::JsonDeserialize)]
pub struct ToriiOperatorAuth {
    /// Master enable switch for operator authentication.
    #[config(default = "defaults::torii::operator_auth::ENABLED")]
    pub enabled: bool,
    /// Require mTLS at ingress before allowing operator endpoints.
    #[config(default = "defaults::torii::operator_auth::REQUIRE_MTLS")]
    pub require_mtls: bool,
    /// Trusted proxy CIDRs allowed to assert forwarded client certificates.
    #[config(default = "defaults::torii::operator_auth::mtls_trusted_proxy_cidrs()")]
    pub mtls_trusted_proxy_cidrs: Vec<String>,
    /// Token fallback mode (`disabled`, `bootstrap`, `always`).
    #[config(default = "defaults::torii::operator_auth::TOKEN_FALLBACK.to_string()")]
    pub token_fallback: String,
    /// Token source (`operator`, `api`, `both`).
    #[config(default = "defaults::torii::operator_auth::TOKEN_SOURCE.to_string()")]
    pub token_source: String,
    /// Token allow-list used for operator fallback (if enabled).
    #[config(default = "defaults::torii::operator_auth::tokens()")]
    pub tokens: Vec<String>,
    /// Auth attempt rate (per minute). None disables.
    pub rate_per_minute: Option<u32>,
    /// Auth attempt burst tokens. None disables.
    pub burst: Option<u32>,
    /// Failures before triggering a temporary lockout (0 disables).
    #[config(default = "defaults::torii::operator_auth::LOCKOUT_FAILURES")]
    pub lockout_failures: u32,
    /// Window for counting failures before lockout (seconds).
    #[config(default = "defaults::torii::operator_auth::LOCKOUT_WINDOW_SECS")]
    pub lockout_window_secs: u64,
    /// Lockout duration once triggered (seconds).
    #[config(default = "defaults::torii::operator_auth::LOCKOUT_DURATION_SECS")]
    pub lockout_duration_secs: u64,
    /// WebAuthn configuration block.
    #[config(nested)]
    pub webauthn: ToriiOperatorWebAuthn,
}

/// WebAuthn configuration for operator authentication.
#[derive(Debug, ReadConfig, Clone, norito::JsonDeserialize)]
pub struct ToriiOperatorWebAuthn {
    /// Master enable switch for WebAuthn.
    #[config(default = "defaults::torii::operator_auth::webauthn::ENABLED")]
    pub enabled: bool,
    /// RP ID expected by WebAuthn clients (must be set when enabled).
    pub rp_id: Option<String>,
    /// RP display name for WebAuthn options.
    #[config(default = "defaults::torii::operator_auth::webauthn::rp_name()")]
    pub rp_name: String,
    /// Allowed WebAuthn origins.
    #[config(default = "defaults::torii::operator_auth::webauthn::origins()")]
    pub origins: Vec<String>,
    /// User id inserted into WebAuthn registration options.
    #[config(default = "defaults::torii::operator_auth::webauthn::user_id()")]
    pub user_id: String,
    /// User name inserted into WebAuthn registration options.
    #[config(default = "defaults::torii::operator_auth::webauthn::user_name()")]
    pub user_name: String,
    /// User display name inserted into WebAuthn registration options.
    #[config(default = "defaults::torii::operator_auth::webauthn::user_display_name()")]
    pub user_display_name: String,
    /// Challenge TTL for registration/assertion options (seconds).
    #[config(default = "defaults::torii::operator_auth::webauthn::CHALLENGE_TTL_SECS")]
    pub challenge_ttl_secs: u64,
    /// Session token TTL after successful assertion (seconds).
    #[config(default = "defaults::torii::operator_auth::webauthn::SESSION_TTL_SECS")]
    pub session_ttl_secs: u64,
    /// Require user verification during assertions.
    #[config(default = "defaults::torii::operator_auth::webauthn::REQUIRE_USER_VERIFICATION")]
    pub require_user_verification: bool,
    /// Allowed WebAuthn algorithms (COSE labels).
    #[config(default = "defaults::torii::operator_auth::webauthn::allowed_algorithms()")]
    pub allowed_algorithms: Vec<String>,
}

impl ToriiOperatorAuth {
    fn parse(self) -> actual::ToriiOperatorAuth {
        let token_fallback = parse_operator_token_fallback(&self.token_fallback);
        let token_source = parse_operator_token_source(&self.token_source);
        let rate_per_minute = self
            .rate_per_minute
            .or(super::defaults::torii::operator_auth::RATE_PER_MIN)
            .and_then(std::num::NonZeroU32::new);
        let burst = self
            .burst
            .or(super::defaults::torii::operator_auth::BURST)
            .and_then(std::num::NonZeroU32::new);
        let lockout_failures = std::num::NonZeroU32::new(self.lockout_failures);
        let lockout_window = Duration::from_secs(self.lockout_window_secs.max(1));
        let lockout_duration = Duration::from_secs(self.lockout_duration_secs.max(1));
        let webauthn = if self.enabled {
            self.webauthn.parse()
        } else {
            None
        };
        if self.enabled && webauthn.is_none() {
            panic!(
                "torii.operator_auth.webauthn.enabled must be true when operator auth is enabled"
            );
        }
        actual::ToriiOperatorAuth {
            enabled: self.enabled,
            require_mtls: self.require_mtls,
            mtls_trusted_proxy_cidrs: self.mtls_trusted_proxy_cidrs,
            token_fallback,
            token_source,
            tokens: self.tokens,
            rate_per_minute,
            burst,
            lockout: actual::OperatorAuthLockout {
                failures: lockout_failures,
                window: lockout_window,
                duration: lockout_duration,
            },
            webauthn,
        }
    }
}

impl ToriiOperatorWebAuthn {
    fn parse(self) -> Option<actual::OperatorWebAuthnConfig> {
        if !self.enabled {
            return None;
        }
        let rp_id = self.rp_id.unwrap_or_else(|| {
            panic!("torii.operator_auth.webauthn.rp_id must be set when WebAuthn is enabled");
        });
        let rp_id = rp_id.trim();
        if rp_id.is_empty() {
            panic!("torii.operator_auth.webauthn.rp_id must not be empty");
        }
        if self.origins.is_empty() {
            panic!("torii.operator_auth.webauthn.origins must not be empty");
        }
        let origins = self
            .origins
            .into_iter()
            .map(|origin| {
                url::Url::parse(&origin).unwrap_or_else(|err| {
                    panic!("invalid torii.operator_auth.webauthn.origins entry `{origin}`: {err}")
                })
            })
            .collect();
        let user_id = self.user_id.into_bytes();
        if user_id.is_empty() || user_id.len() > 64 {
            panic!("torii.operator_auth.webauthn.user_id must be 1..=64 bytes");
        }
        let algorithms = self
            .allowed_algorithms
            .into_iter()
            .map(|label| parse_operator_webauthn_algorithm(&label))
            .collect();
        Some(actual::OperatorWebAuthnConfig {
            rp_id: rp_id.to_string(),
            rp_name: self.rp_name,
            origins,
            user_id,
            user_name: self.user_name,
            user_display_name: self.user_display_name,
            challenge_ttl: Duration::from_secs(self.challenge_ttl_secs.max(1)),
            session_ttl: Duration::from_secs(self.session_ttl_secs.max(1)),
            require_user_verification: self.require_user_verification,
            allowed_algorithms: algorithms,
        })
    }
}

fn parse_operator_token_fallback(value: &str) -> actual::OperatorTokenFallback {
    match value.trim().to_ascii_lowercase().as_str() {
        "disabled" => actual::OperatorTokenFallback::Disabled,
        "bootstrap" => actual::OperatorTokenFallback::Bootstrap,
        "always" => actual::OperatorTokenFallback::Always,
        other => panic!(
            "invalid torii.operator_auth.token_fallback `{other}`; expected `disabled`, `bootstrap`, or `always`"
        ),
    }
}

fn parse_operator_token_source(value: &str) -> actual::OperatorTokenSource {
    match value.trim().to_ascii_lowercase().as_str() {
        "operator" => actual::OperatorTokenSource::OperatorTokens,
        "api" => actual::OperatorTokenSource::ApiTokens,
        "both" => actual::OperatorTokenSource::Both,
        other => panic!(
            "invalid torii.operator_auth.token_source `{other}`; expected `operator`, `api`, or `both`"
        ),
    }
}

fn parse_attachment_sanitizer_mode(value: &str) -> actual::AttachmentSanitizerMode {
    match value.trim().to_ascii_lowercase().as_str() {
        "in_process" | "inprocess" | "inline" => actual::AttachmentSanitizerMode::InProcess,
        "subprocess" | "external" | "process" => actual::AttachmentSanitizerMode::Subprocess,
        other => panic!(
            "invalid torii.attachments_sanitizer_mode `{other}`; expected `subprocess` or `in_process`"
        ),
    }
}

fn parse_operator_webauthn_algorithm(value: &str) -> actual::OperatorWebAuthnAlgorithm {
    match value.trim().to_ascii_lowercase().as_str() {
        "es256" | "p256" => actual::OperatorWebAuthnAlgorithm::Es256,
        "ed25519" | "eddsa" => actual::OperatorWebAuthnAlgorithm::Ed25519,
        other => panic!(
            "invalid torii.operator_auth.webauthn.allowed_algorithms entry `{other}`; expected `es256` or `ed25519`"
        ),
    }
}
/// Transport-specific Torii configuration (Norito-RPC rollout, streaming knobs).
#[derive(Debug, ReadConfig, Clone, norito::JsonDeserialize, Default)]
pub struct ToriiTransport {
    /// Norito-RPC transport rollout settings.
    #[config(nested)]
    pub norito_rpc: ToriiNoritoRpcTransport,
}

/// Norito-RPC transport configuration parameters.
#[derive(Debug, ReadConfig, Clone, norito::JsonDeserialize)]
pub struct ToriiNoritoRpcTransport {
    /// Master enable switch for Norito-RPC decoding.
    #[config(default = "defaults::torii::transport::norito_rpc::ENABLED")]
    pub enabled: bool,
    /// Require mTLS at the ingress tier before allowing Norito-RPC (surfaced for operators).
    #[config(default = "defaults::torii::transport::norito_rpc::REQUIRE_MTLS")]
    pub require_mtls: bool,
    /// Trusted proxy CIDRs allowed to assert forwarded client certificates.
    #[config(default = "defaults::torii::transport::norito_rpc::mtls_trusted_proxy_cidrs()")]
    pub mtls_trusted_proxy_cidrs: Vec<String>,
    /// Explicit list of client tokens permitted during the `canary` stage.
    #[config(default = "defaults::torii::transport::norito_rpc::allowed_clients()")]
    pub allowed_clients: Vec<String>,
    /// Rollout stage label (`disabled`, `canary`, `ga`).
    #[config(default = "defaults::torii::transport::norito_rpc::STAGE.to_string()")]
    pub stage: String,
}

impl Default for ToriiNoritoRpcTransport {
    fn default() -> Self {
        Self {
            enabled: defaults::torii::transport::norito_rpc::ENABLED,
            require_mtls: defaults::torii::transport::norito_rpc::REQUIRE_MTLS,
            mtls_trusted_proxy_cidrs:
                defaults::torii::transport::norito_rpc::mtls_trusted_proxy_cidrs(),
            allowed_clients: defaults::torii::transport::norito_rpc::allowed_clients(),
            stage: defaults::torii::transport::norito_rpc::STAGE.to_string(),
        }
    }
}

/// Native MCP endpoint configuration parameters.
#[derive(Debug, ReadConfig, Clone, norito::JsonDeserialize)]
pub struct ToriiMcp {
    /// Master enable switch for native `/v1/mcp`.
    #[config(default = "defaults::torii::mcp::ENABLED")]
    pub enabled: bool,
    /// Maximum accepted request payload size in bytes.
    #[config(default = "defaults::torii::mcp::MAX_REQUEST_BYTES")]
    pub max_request_bytes: usize,
    /// Maximum number of tools emitted in one `tools/list` response page.
    #[config(default = "defaults::torii::mcp::MAX_TOOLS_PER_LIST")]
    pub max_tools_per_list: usize,
    /// MCP tool profile (`read_only`, `writer`, `operator`).
    #[config(default = "defaults::torii::mcp::PROFILE.to_string()")]
    pub profile: String,
    /// Expose operator-only routes in MCP tool discovery.
    #[config(default = "defaults::torii::mcp::EXPOSE_OPERATOR_ROUTES")]
    pub expose_operator_routes: bool,
    /// Additional allow-list prefixes for tool names (empty => profile-only).
    #[config(default = "defaults::torii::mcp::allow_tool_prefixes()")]
    pub allow_tool_prefixes: Vec<String>,
    /// Additional deny-list prefixes for tool names.
    #[config(default = "defaults::torii::mcp::deny_tool_prefixes()")]
    pub deny_tool_prefixes: Vec<String>,
    /// Optional steady-state MCP request budget (requests/minute).
    pub rate_per_minute: Option<u32>,
    /// Optional MCP burst budget.
    pub burst: Option<u32>,
    /// Retention window in seconds for asynchronous MCP jobs.
    #[config(default = "defaults::torii::mcp::ASYNC_JOB_TTL_SECS")]
    pub async_job_ttl_secs: u64,
    /// Maximum asynchronous MCP jobs retained in memory.
    #[config(default = "defaults::torii::mcp::ASYNC_JOB_MAX_ENTRIES")]
    pub async_job_max_entries: usize,
}

impl Default for ToriiMcp {
    fn default() -> Self {
        Self {
            enabled: defaults::torii::mcp::ENABLED,
            max_request_bytes: defaults::torii::mcp::MAX_REQUEST_BYTES,
            max_tools_per_list: defaults::torii::mcp::MAX_TOOLS_PER_LIST,
            profile: defaults::torii::mcp::PROFILE.to_string(),
            expose_operator_routes: defaults::torii::mcp::EXPOSE_OPERATOR_ROUTES,
            allow_tool_prefixes: defaults::torii::mcp::allow_tool_prefixes(),
            deny_tool_prefixes: defaults::torii::mcp::deny_tool_prefixes(),
            rate_per_minute: defaults::torii::mcp::RATE_PER_MINUTE,
            burst: defaults::torii::mcp::BURST,
            async_job_ttl_secs: defaults::torii::mcp::ASYNC_JOB_TTL_SECS,
            async_job_max_entries: defaults::torii::mcp::ASYNC_JOB_MAX_ENTRIES,
        }
    }
}

/// App onboarding authority wiring for UAID registration helpers.
#[derive(Debug, ReadConfig, Clone, norito::JsonDeserialize)]
pub struct ToriiOnboarding {
    /// Master enable switch (defaults to enabled).
    #[config(default = "true")]
    pub enabled: bool,
    /// Account identifier that signs onboarding transactions.
    pub authority: String,
    /// Private key corresponding to the onboarding authority.
    pub private_key: ExposedPrivateKey,
    /// Permission names that onboarding is allowed to grant to new accounts.
    #[config(default)]
    pub allowed_permissions: Vec<String>,
    /// Optional sponsor account granted via `CanUseFeeSponsor`.
    pub fee_sponsor_account: Option<String>,
}

impl ToriiOnboarding {
    fn parse(self) -> Option<actual::ToriiOnboarding> {
        if !self.enabled {
            return None;
        }
        let authority = AccountId::parse_encoded(&self.authority).map_or_else(
            |err| {
                panic!(
                    "invalid torii.onboarding.authority `{}`: {err}",
                    self.authority
                )
            },
            iroha_data_model::account::ParsedAccountId::into_account_id,
        );
        let allowed_permissions = self
            .allowed_permissions
            .into_iter()
            .map(|permission| permission.trim().to_owned())
            .filter(|permission| !permission.is_empty())
            .collect();
        let fee_sponsor_account = self.fee_sponsor_account.map(|account| {
            AccountId::parse_encoded(&account).map_or_else(
                |err| panic!("invalid torii.onboarding.fee_sponsor_account `{account}`: {err}"),
                iroha_data_model::account::ParsedAccountId::into_account_id,
            )
        });
        Some(actual::ToriiOnboarding {
            authority,
            private_key: self.private_key,
            allowed_permissions,
            fee_sponsor_account,
        })
    }
}

/// Faucet configuration for app-facing onboarding helpers.
#[derive(Debug, ReadConfig, Clone, norito::JsonDeserialize)]
pub struct ToriiFaucet {
    /// Master enable switch (defaults to enabled).
    #[config(default = "true")]
    pub enabled: bool,
    /// Account identifier that signs faucet transfers.
    pub authority: String,
    /// Private key corresponding to the faucet authority.
    pub private_key: ExposedPrivateKey,
    /// Asset definition distributed by the faucet.
    pub asset_definition_id: String,
    /// Fixed quantity transferred to each eligible account.
    pub amount: String,
    /// Leading-zero-bit difficulty for faucet proof-of-work (0 disables PoW).
    #[config(default = "defaults::torii::faucet::POW_DIFFICULTY_BITS")]
    pub pow_difficulty_bits: u8,
    /// Scrypt `log2(N)` cost parameter for faucet proof-of-work.
    #[config(default = "defaults::torii::faucet::POW_SCRYPT_LOG_N")]
    pub pow_scrypt_log_n: u8,
    /// Scrypt block size parameter for faucet proof-of-work.
    #[config(default = "defaults::torii::faucet::POW_SCRYPT_R")]
    pub pow_scrypt_r: u32,
    /// Scrypt parallelization parameter for faucet proof-of-work.
    #[config(default = "defaults::torii::faucet::POW_SCRYPT_P")]
    pub pow_scrypt_p: u32,
    /// Maximum committed-block age for accepted faucet PoW anchors.
    #[config(default = "defaults::torii::faucet::POW_MAX_ANCHOR_AGE_BLOCKS.get()")]
    pub pow_max_anchor_age_blocks: u64,
    /// Number of recent committed blocks to scan for prior faucet claims when adapting difficulty.
    #[config(default = "defaults::torii::faucet::POW_ADAPTIVE_LOOKBACK_BLOCKS")]
    pub pow_adaptive_lookback_blocks: u64,
    /// Number of recent faucet claims required to add one extra difficulty bit.
    #[config(default = "defaults::torii::faucet::POW_ADAPTIVE_CLAIMS_PER_EXTRA_BIT")]
    pub pow_adaptive_claims_per_extra_bit: u64,
    /// Maximum number of adaptive difficulty bits added on top of the base difficulty.
    #[config(default = "defaults::torii::faucet::POW_ADAPTIVE_MAX_EXTRA_BITS")]
    pub pow_adaptive_max_extra_bits: u8,
    /// Whether finalized Sumeragi VRF epoch seeds are mixed into faucet challenges when available.
    #[config(default = "defaults::torii::faucet::POW_VRF_SEED_ENABLED")]
    pub pow_vrf_seed_enabled: bool,
}

impl ToriiFaucet {
    fn parse(self) -> Option<actual::ToriiFaucet> {
        if !self.enabled {
            return None;
        }
        let authority = AccountId::parse_encoded(&self.authority).map_or_else(
            |err| panic!("invalid torii.faucet.authority `{}`: {err}", self.authority),
            iroha_data_model::account::ParsedAccountId::into_account_id,
        );
        let asset_definition_id = self.asset_definition_id.split_once('#').map_or_else(
            || {
                AssetDefinitionId::parse_address_literal(&self.asset_definition_id).unwrap_or_else(
                    |err| {
                        panic!(
                            "invalid torii.faucet.asset_definition_id `{}`: {err}",
                            self.asset_definition_id
                        )
                    },
                )
            },
            |(name_literal, domain_literal)| {
                let name = name_literal.trim().parse().unwrap_or_else(|err| {
                    panic!("invalid torii.faucet.asset_definition_id name `{name_literal}`: {err}")
                });
                let domain = domain_literal.trim().parse().unwrap_or_else(|err| {
                    panic!(
                        "invalid torii.faucet.asset_definition_id domain `{domain_literal}`: {err}"
                    )
                });
                AssetDefinitionId::new(domain, name)
            },
        );
        let amount = Numeric::from_str(self.amount.trim())
            .unwrap_or_else(|err| panic!("invalid torii.faucet.amount `{}`: {err}", self.amount));
        if amount <= Numeric::zero() {
            panic!("torii.faucet.amount must be greater than zero");
        }
        if self.pow_scrypt_log_n == 0 {
            panic!("torii.faucet.pow_scrypt_log_n must be greater than zero");
        }
        if self.pow_scrypt_r == 0 {
            panic!("torii.faucet.pow_scrypt_r must be greater than zero");
        }
        if self.pow_scrypt_p == 0 {
            panic!("torii.faucet.pow_scrypt_p must be greater than zero");
        }
        let pow_max_anchor_age_blocks = NonZeroU64::new(self.pow_max_anchor_age_blocks)
            .unwrap_or_else(|| {
                panic!("torii.faucet.pow_max_anchor_age_blocks must be greater than zero")
            });
        Some(actual::ToriiFaucet {
            authority,
            private_key: self.private_key,
            asset_definition_id,
            amount,
            pow_difficulty_bits: self.pow_difficulty_bits,
            pow_scrypt_log_n: self.pow_scrypt_log_n,
            pow_scrypt_r: self.pow_scrypt_r,
            pow_scrypt_p: self.pow_scrypt_p,
            pow_max_anchor_age_blocks,
            pow_adaptive_lookback_blocks: self.pow_adaptive_lookback_blocks,
            pow_adaptive_claims_per_extra_bit: self.pow_adaptive_claims_per_extra_bit,
            pow_adaptive_max_extra_bits: self.pow_adaptive_max_extra_bits,
            pow_vrf_seed_enabled: self.pow_vrf_seed_enabled,
        })
    }
}

#[cfg(test)]
mod torii_faucet_tests {
    use super::*;
    use iroha_crypto::PublicKey;

    fn sample_faucet() -> ToriiFaucet {
        let public_key: PublicKey =
            "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03"
                .parse()
                .expect("public key");
        ToriiFaucet {
            enabled: true,
            authority: AccountId::new(public_key).to_string(),
            private_key: "802620CCF31D85E3B32A4BEA59987CE0C78E3B8E2DB93881468AB2435FE45D5C9DCD53"
                .parse()
                .expect("private key"),
            asset_definition_id: "xor#sora".to_owned(),
            amount: "25000".to_owned(),
            pow_difficulty_bits: 18,
            pow_scrypt_log_n: 13,
            pow_scrypt_r: 8,
            pow_scrypt_p: 1,
            pow_max_anchor_age_blocks: 4,
            pow_adaptive_lookback_blocks: 32,
            pow_adaptive_claims_per_extra_bit: 3,
            pow_adaptive_max_extra_bits: 5,
            pow_vrf_seed_enabled: true,
        }
    }

    #[test]
    fn torii_faucet_parse_maps_enabled_config() {
        let parsed = sample_faucet().parse().expect("enabled faucet");
        assert_eq!(parsed.authority.to_string(), sample_faucet().authority);
        assert_eq!(
            parsed.asset_definition_id,
            AssetDefinitionId::new(
                "sora".parse().expect("domain"),
                "xor".parse().expect("name")
            )
        );
        assert_eq!(parsed.amount.to_string(), "25000");
        assert_eq!(parsed.pow_difficulty_bits, 18);
        assert_eq!(parsed.pow_scrypt_log_n, 13);
        assert_eq!(parsed.pow_scrypt_r, 8);
        assert_eq!(parsed.pow_scrypt_p, 1);
        assert_eq!(parsed.pow_max_anchor_age_blocks.get(), 4);
        assert_eq!(parsed.pow_adaptive_lookback_blocks, 32);
        assert_eq!(parsed.pow_adaptive_claims_per_extra_bit, 3);
        assert_eq!(parsed.pow_adaptive_max_extra_bits, 5);
        assert!(parsed.pow_vrf_seed_enabled);
    }

    #[test]
    fn torii_faucet_parse_returns_none_when_disabled() {
        let mut faucet = sample_faucet();
        faucet.enabled = false;
        assert!(faucet.parse().is_none());
    }

    #[test]
    fn torii_faucet_parse_rejects_non_positive_amount() {
        let mut faucet = sample_faucet();
        faucet.amount = "0".to_owned();
        let panic = std::panic::catch_unwind(|| faucet.parse());
        assert!(panic.is_err(), "expected zero amount to panic");
    }

    #[test]
    fn torii_faucet_parse_rejects_non_positive_pow_anchor_age() {
        let mut faucet = sample_faucet();
        faucet.pow_max_anchor_age_blocks = 0;
        let panic = std::panic::catch_unwind(|| faucet.parse());
        assert!(panic.is_err(), "expected zero pow anchor age to panic");
    }

    #[test]
    fn torii_faucet_parse_rejects_non_positive_scrypt_log_n() {
        let mut faucet = sample_faucet();
        faucet.pow_scrypt_log_n = 0;
        let panic = std::panic::catch_unwind(|| faucet.parse());
        assert!(panic.is_err(), "expected zero scrypt log_n to panic");
    }
}

/// Offline certificate issuer configuration (operator signing).
#[derive(Debug, ReadConfig, Clone, norito::JsonDeserialize)]
pub struct ToriiOfflineIssuer {
    /// Master enable switch (defaults to enabled).
    #[config(default = "defaults::torii::offline_issuer::ENABLED")]
    pub enabled: bool,
    /// Optional on-chain operator account used for reserve and revocation transactions.
    pub operator_authority: Option<String>,
    /// Private key used to sign offline wallet certificates.
    pub operator_private_key: ExposedPrivateKey,
    /// Additional legacy private keys accepted for build-claim signatures.
    #[config(default = "defaults::torii::offline_issuer::legacy_operator_private_keys()")]
    pub legacy_operator_private_keys: Vec<ExposedPrivateKey>,
    /// Optional allow-list of controllers eligible for issuance.
    #[config(default = "defaults::torii::offline_issuer::allowed_controllers()")]
    pub allowed_controllers: Vec<String>,
    /// Device-bound offline reserve policy.
    #[config(default)]
    pub reserve_policy: ToriiOfflineReservePolicy,
}

impl ToriiOfflineIssuer {
    fn parse(self) -> Option<actual::ToriiOfflineIssuer> {
        if !self.enabled {
            return None;
        }
        let allowed_controllers = self
            .allowed_controllers
            .into_iter()
            .map(|controller| {
                AccountId::parse_encoded(&controller).map_or_else(
                    |err| {
                        panic!("invalid torii.offline_issuer.allowed_controllers entry `{controller}`: {err}")
                    },
                    iroha_data_model::account::ParsedAccountId::into_account_id,
                )
            })
            .collect();
        let operator_authority = self.operator_authority.map(|authority| {
            AccountId::parse_encoded(&authority).map_or_else(
                |err| {
                    panic!("invalid torii.offline_issuer.operator_authority `{authority}`: {err}")
                },
                iroha_data_model::account::ParsedAccountId::into_account_id,
            )
        });
        Some(actual::ToriiOfflineIssuer {
            operator_authority,
            operator_private_key: self.operator_private_key,
            legacy_operator_private_keys: self.legacy_operator_private_keys,
            allowed_controllers,
            reserve_policy: self.reserve_policy.parse(),
        })
    }
}

/// Device-bound offline reserve policy values.
#[derive(Debug, ReadConfig, Clone, norito::JsonDeserialize)]
pub struct ToriiOfflineReservePolicy {
    /// Maximum total spendable offline balance per reserve.
    #[config(default = "defaults::torii::offline_issuer::RESERVE_MAX_BALANCE.to_owned()")]
    pub max_balance: String,
    /// Maximum single offline transfer value.
    #[config(default = "defaults::torii::offline_issuer::RESERVE_MAX_TX_VALUE.to_owned()")]
    pub max_tx_value: String,
    /// Authorization lifetime in milliseconds.
    #[config(default = "defaults::torii::offline_issuer::RESERVE_AUTHORIZATION_TTL_MS")]
    pub authorization_ttl_ms: u64,
    /// Authorization refresh deadline in milliseconds.
    #[config(default = "defaults::torii::offline_issuer::RESERVE_AUTHORIZATION_REFRESH_MS")]
    pub authorization_refresh_ms: u64,
    /// Revocation bundle lifetime in milliseconds.
    #[config(default = "defaults::torii::offline_issuer::RESERVE_REVOCATION_TTL_MS")]
    pub revocation_ttl_ms: u64,
}

impl Default for ToriiOfflineReservePolicy {
    fn default() -> Self {
        Self {
            max_balance: defaults::torii::offline_issuer::RESERVE_MAX_BALANCE.to_owned(),
            max_tx_value: defaults::torii::offline_issuer::RESERVE_MAX_TX_VALUE.to_owned(),
            authorization_ttl_ms: defaults::torii::offline_issuer::RESERVE_AUTHORIZATION_TTL_MS,
            authorization_refresh_ms:
                defaults::torii::offline_issuer::RESERVE_AUTHORIZATION_REFRESH_MS,
            revocation_ttl_ms: defaults::torii::offline_issuer::RESERVE_REVOCATION_TTL_MS,
        }
    }
}

impl ToriiOfflineReservePolicy {
    fn parse(self) -> actual::ToriiOfflineReservePolicy {
        actual::ToriiOfflineReservePolicy {
            max_balance: self.max_balance,
            max_tx_value: self.max_tx_value,
            authorization_ttl: Duration::from_millis(self.authorization_ttl_ms),
            authorization_refresh: Duration::from_millis(self.authorization_refresh_ms),
            revocation_ttl: Duration::from_millis(self.revocation_ttl_ms),
        }
    }
}

/// RAM-LFE runtime configuration.
#[derive(Debug, ReadConfig, Clone, norito::JsonDeserialize)]
pub struct ToriiRamLfe {
    /// Master enable switch for Torii's in-process RAM-LFE runtime.
    #[config(default = "defaults::torii::ram_lfe::ENABLED")]
    pub enabled: bool,
    /// Per-program runtime entries.
    #[config(default)]
    pub programs: Vec<ToriiRamLfeProgram>,
}

impl ToriiRamLfe {
    fn parse(self) -> Option<actual::ToriiRamLfe> {
        if !self.enabled {
            return None;
        }
        Some(actual::ToriiRamLfe {
            programs: self
                .programs
                .into_iter()
                .enumerate()
                .map(|(index, program)| program.parse(index))
                .collect(),
        })
    }
}

/// Per-program runtime material for Torii's RAM-LFE runtime.
#[derive(Debug, ReadConfig, Clone, norito::JsonDeserialize)]
pub struct ToriiRamLfeProgram {
    /// On-chain RAM-LFE program identifier.
    pub program_id: String,
    /// Hidden derivation secret encoded as hex.
    pub secret_hex: String,
    /// Private key used to sign receipts for this program.
    pub signer_private_key: ExposedPrivateKey,
    /// Optional receipt TTL expressed in milliseconds.
    pub receipt_ttl_ms: Option<DurationMs>,
}

impl ToriiRamLfeProgram {
    fn parse(self, index: usize) -> actual::ToriiRamLfeProgram {
        let program_id: iroha_data_model::ram_lfe::RamLfeProgramId =
            self.program_id.parse().unwrap_or_else(|err| {
                panic!(
                    "invalid torii.ram_lfe.programs[{index}].program_id `{}`: {err}",
                    self.program_id
                )
            });
        let secret_literal = self.secret_hex.trim().trim_start_matches("0x");
        let secret = Vec::from_hex(secret_literal).unwrap_or_else(|err| {
            panic!("invalid torii.ram_lfe.programs[{index}].secret_hex: {err}")
        });
        if secret.is_empty() {
            panic!("torii.ram_lfe.programs[{index}].secret_hex must not be empty");
        }
        actual::ToriiRamLfeProgram {
            program_id,
            secret,
            signer_private_key: self.signer_private_key,
            receipt_ttl: self.receipt_ttl_ms.map(DurationMs::get),
        }
    }
}

fn default_events_buffer_capacity() -> NonZeroUsize {
    std::num::NonZeroUsize::new(defaults::torii::EVENTS_BUFFER_CAPACITY)
        .expect("events buffer capacity must be non-zero")
}

fn default_app_auth_replay_cache_capacity() -> NonZeroUsize {
    std::num::NonZeroUsize::new(defaults::torii::app_auth::REPLAY_CACHE_CAPACITY)
        .expect("app auth replay cache capacity must be non-zero")
}

fn default_webhook_queue_capacity() -> NonZeroUsize {
    std::num::NonZeroUsize::new(defaults::torii::WEBHOOK_QUEUE_CAPACITY)
        .expect("webhook queue capacity must be non-zero")
}

fn default_webhook_max_attempts() -> NonZeroU32 {
    std::num::NonZeroU32::new(defaults::torii::WEBHOOK_MAX_ATTEMPTS)
        .expect("webhook max attempts must be non-zero")
}

/// Webhook delivery/backpressure configuration.
#[derive(Debug, ReadConfig, Clone, Copy, norito::JsonDeserialize)]
pub struct Webhook {
    /// Maximum pending webhook deliveries persisted on disk.
    #[config(default = "default_webhook_queue_capacity()")]
    pub queue_capacity: NonZeroUsize,
    /// Maximum delivery attempts before a payload is dropped.
    #[config(default = "default_webhook_max_attempts()")]
    pub max_attempts: NonZeroU32,
    /// Initial backoff delay (milliseconds) applied to webhook retries.
    #[config(
        default = "DurationMs(std::time::Duration::from_millis(defaults::torii::WEBHOOK_BACKOFF_INITIAL_MS))"
    )]
    pub backoff_initial_ms: DurationMs,
    /// Maximum backoff delay (milliseconds) applied to webhook retries.
    #[config(
        default = "DurationMs(std::time::Duration::from_millis(defaults::torii::WEBHOOK_BACKOFF_MAX_MS))"
    )]
    pub backoff_max_ms: DurationMs,
    /// HTTP connect timeout (milliseconds) for webhook delivery.
    #[config(
        default = "DurationMs(std::time::Duration::from_millis(defaults::torii::WEBHOOK_CONNECT_TIMEOUT_MS))"
    )]
    pub connect_timeout_ms: DurationMs,
    /// HTTP write timeout (milliseconds) for webhook delivery.
    #[config(
        default = "DurationMs(std::time::Duration::from_millis(defaults::torii::WEBHOOK_WRITE_TIMEOUT_MS))"
    )]
    pub write_timeout_ms: DurationMs,
    /// HTTP read timeout (milliseconds) for webhook delivery.
    #[config(
        default = "DurationMs(std::time::Duration::from_millis(defaults::torii::WEBHOOK_READ_TIMEOUT_MS))"
    )]
    pub read_timeout_ms: DurationMs,
}

impl Webhook {
    fn parse(self) -> actual::Webhook {
        actual::Webhook {
            queue_capacity: self.queue_capacity,
            max_attempts: self.max_attempts,
            backoff_initial: self.backoff_initial_ms.get(),
            backoff_max: self.backoff_max_ms.get(),
            connect_timeout: self.connect_timeout_ms.get(),
            write_timeout: self.write_timeout_ms.get(),
            read_timeout: self.read_timeout_ms.get(),
        }
    }
}

/// Webhook destination security configuration (SSRF guard rails).
#[derive(Debug, ReadConfig, Clone, norito::JsonDeserialize)]
pub struct WebhookSecurity {
    /// Master enable switch for webhook destination guard rails.
    #[config(default = "defaults::torii::webhook_security::ENABLED")]
    pub enabled: bool,
    /// CIDR allow-list for webhook destinations (empty => only public IPs are allowed).
    #[config(default = "defaults::torii::webhook_security::allow_cidrs()")]
    pub allow_cidrs: Vec<String>,
}

impl WebhookSecurity {
    fn parse(self) -> actual::WebhookSecurity {
        actual::WebhookSecurity {
            enabled: self.enabled,
            allow_cidrs: self.allow_cidrs,
        }
    }
}

/// User-level configuration container for `Connect`.
#[derive(Debug, ReadConfig, Clone, norito::JsonDeserialize)]
pub struct Connect {
    /// Enable Iroha Connect WS + P2P relay.
    #[config(env = "CONNECT_ENABLED", default = "defaults::connect::ENABLED")]
    pub enabled: bool,
    /// Max concurrent WS sessions across roles.
    #[config(
        env = "CONNECT_WS_MAX_SESSIONS",
        default = "defaults::connect::WS_MAX_SESSIONS"
    )]
    pub ws_max_sessions: usize,
    /// Max concurrent WS sessions per remote IP (0 disables the per-IP cap).
    #[config(
        env = "CONNECT_WS_PER_IP_MAX_SESSIONS",
        default = "defaults::connect::WS_PER_IP_MAX_SESSIONS"
    )]
    pub ws_per_ip_max_sessions: usize,
    /// Per-IP WS handshake rate (requests per minute, 0 disables rate limiting).
    #[config(
        env = "CONNECT_WS_RATE_PER_IP_PER_MIN",
        default = "defaults::connect::WS_RATE_PER_IP_PER_MIN"
    )]
    pub ws_rate_per_ip_per_min: u32,
    /// Session inactivity TTL (ms).
    #[config(default = "defaults::connect::SESSION_TTL.into()")]
    pub session_ttl_ms: DurationMs,
    /// Maximum WS frame size accepted for Connect frames (bytes).
    #[config(
        env = "CONNECT_FRAME_MAX_BYTES",
        default = "defaults::connect::FRAME_MAX_BYTES"
    )]
    pub frame_max_bytes: usize,
    /// Maximum buffered payload per session (bytes) for pending delivery.
    #[config(
        env = "CONNECT_SESSION_BUFFER_MAX_BYTES",
        default = "defaults::connect::SESSION_BUFFER_MAX_BYTES"
    )]
    pub session_buffer_max_bytes: usize,
    /// Heartbeat ping interval (ms).
    #[config(
        env = "CONNECT_PING_INTERVAL_MS",
        default = "defaults::connect::PING_INTERVAL.into()"
    )]
    pub ping_interval_ms: DurationMs,
    /// Number of tolerated consecutive missed pongs before disconnect.
    #[config(
        env = "CONNECT_PING_MISS_TOLERANCE",
        default = "defaults::connect::PING_MISS_TOLERANCE"
    )]
    pub ping_miss_tolerance: u32,
    /// Minimum heartbeat interval allowed (ms) for browser transports.
    #[config(
        env = "CONNECT_PING_MIN_INTERVAL_MS",
        default = "defaults::connect::PING_MIN_INTERVAL.into()"
    )]
    pub ping_min_interval_ms: DurationMs,
    /// Dedupe cache TTL (ms).
    #[config(default = "defaults::connect::DEDUPE_TTL.into()")]
    pub dedupe_ttl_ms: DurationMs,
    /// Dedupe cache capacity (entries).
    #[config(env = "CONNECT_DEDUPE_CAP", default = "defaults::connect::DEDUPE_CAP")]
    pub dedupe_cap: usize,
    /// Enable P2P re-broadcast relay.
    #[config(
        env = "CONNECT_RELAY_ENABLED",
        default = "defaults::connect::RELAY_ENABLED"
    )]
    pub relay_enabled: bool,
    /// Relay strategy string.
    #[config(
        env = "CONNECT_RELAY_STRATEGY",
        default = "defaults::connect::RELAY_STRATEGY.to_string()"
    )]
    pub relay_strategy: String,
    /// Optional hop TTL for relay (0 disables; not enforced in v0 flood).
    #[config(
        env = "CONNECT_P2P_TTL_HOPS",
        default = "defaults::connect::P2P_TTL_HOPS"
    )]
    pub p2p_ttl_hops: u8,
}

/// Per-scheme concurrency limit for the Torii pre-auth gate.
#[derive(Debug, Clone, ReadConfig, norito::JsonDeserialize)]
pub struct PreauthSchemeLimit {
    /// Label of the connection scheme (matches `ConnScheme::label()` in Torii).
    pub scheme: String,
    /// Maximum concurrent pre-auth connections allowed for this scheme.
    pub max_connections: NonZeroUsize,
}

impl Connect {
    fn parse(self) -> actual::Connect {
        actual::Connect {
            enabled: self.enabled,
            ws_max_sessions: self.ws_max_sessions,
            ws_per_ip_max_sessions: self.ws_per_ip_max_sessions,
            ws_rate_per_ip_per_min: self.ws_rate_per_ip_per_min,
            session_ttl: self.session_ttl_ms.get(),
            frame_max_bytes: self.frame_max_bytes,
            session_buffer_max_bytes: self.session_buffer_max_bytes,
            ping_interval: self.ping_interval_ms.get(),
            ping_miss_tolerance: self.ping_miss_tolerance,
            ping_min_interval: self.ping_min_interval_ms.get(),
            dedupe_ttl: self.dedupe_ttl_ms.get(),
            dedupe_cap: self.dedupe_cap,
            relay_enabled: self.relay_enabled,
            relay_strategy: Box::leak(self.relay_strategy.into_boxed_str()),
            p2p_ttl_hops: self.p2p_ttl_hops,
        }
    }
}

/// User-level configuration container for `IsoBridge`.
#[derive(Debug, ReadConfig, Clone, norito::JsonDeserialize)]
pub struct IsoBridge {
    #[config(
        env = "ISO_BRIDGE_ENABLED",
        default = "defaults::torii::ISO_BRIDGE_ENABLED"
    )]
    /// Enables or disables the ISO bridge surface.
    pub enabled: bool,
    #[config(
        env = "ISO_BRIDGE_DEDUPE_TTL_SECS",
        default = "defaults::torii::ISO_BRIDGE_DEDUPE_TTL_SECS"
    )]
    /// Deduplication time-to-live for inbound ISO bridge requests.
    pub dedupe_ttl_secs: u64,
    /// Signing credentials used for bridge operations.
    pub signer: Option<IsoBridgeSigner>,
    #[config(default = "Vec::new()")]
    /// Mapping of external identifiers (e.g., IBAN) to local accounts.
    pub account_aliases: Vec<IsoAccountAlias>,
    #[config(default = "Vec::new()")]
    /// Mapping of ISO currency codes to on-chain asset definitions.
    pub currency_assets: Vec<IsoCurrencyAsset>,
    #[config(default = "IsoReferenceData::default()")]
    /// Reference-data ingestion configuration.
    pub reference_data: IsoReferenceData,
}

/// User-level configuration container for `RbcSampling`.
#[derive(Debug, Copy, Clone, ReadConfig)]
pub struct RbcSampling {
    /// Enables the RBC sampling endpoint.
    #[config(default = "defaults::torii::RBC_SAMPLING_ENABLED")]
    pub enabled: bool,
    /// Maximum number of samples allowed per request.
    #[config(default = "defaults::torii::RBC_SAMPLING_MAX_SAMPLES_PER_REQUEST")]
    pub max_samples_per_request: u32,
    /// Maximum byte size of samples returned per request.
    #[config(default = "defaults::torii::RBC_SAMPLING_MAX_BYTES_PER_REQUEST")]
    pub max_bytes_per_request: u64,
    /// Daily byte budget allocated for sampling responses.
    #[config(default = "defaults::torii::RBC_SAMPLING_DAILY_BYTE_BUDGET")]
    pub daily_byte_budget: u64,
    /// Optional per-minute token bucket override.
    pub rate_per_minute: Option<u32>,
}

/// User-level configuration for DA ingest replay cache behaviour.
#[derive(Debug, ReadConfig, Clone)]
#[allow(clippy::struct_field_names)]
pub struct DaIngest {
    /// Replay cache capacity per `(lane, epoch)` window.
    #[config(default = "defaults::torii::DA_REPLAY_CACHE_CAPACITY")]
    pub replay_cache_capacity: NonZeroUsize,
    /// Replay cache TTL (seconds).
    #[config(default = "defaults::torii::DA_REPLAY_CACHE_TTL_SECS")]
    pub replay_cache_ttl_secs: u64,
    /// Maximum tolerated sequence lag before rejecting manifests.
    #[config(default = "defaults::torii::DA_REPLAY_CACHE_MAX_SEQUENCE_LAG")]
    pub replay_cache_max_sequence_lag: u64,
    /// Directory where replay cursors are persisted.
    #[config(default = "defaults::torii::da_replay_cache_store_dir()")]
    pub replay_cache_store_dir: PathBuf,
    /// Directory where canonical DA manifests are queued for SoraFS orchestration.
    #[config(default = "defaults::torii::da_manifest_store_dir()")]
    pub manifest_store_dir: PathBuf,
    /// Optional hex-encoded ChaCha20Poly1305 key for governance-only metadata encryption.
    pub governance_metadata_key_hex: Option<String>,
    /// Optional label recorded alongside governance metadata ciphertexts.
    pub governance_metadata_key_label: Option<String>,
    /// Optional Taikai envelope anchoring configuration.
    pub taikai_anchor: Option<DaTaikaiAnchor>,
    /// Replication policy overrides applied per blob class.
    #[config(default = "da_replication_policy_default()")]
    pub replication_policy: DaReplicationPolicy,
    /// Rent policy configuration applied to DA submissions.
    #[config(default = "da_rent_policy_default()")]
    pub rent_policy: DaRentPolicy,
    /// Optional telemetry cluster label for ingest metrics.
    pub telemetry_cluster_label: Option<String>,
}

impl Default for DaIngest {
    fn default() -> Self {
        Self {
            replay_cache_capacity: defaults::torii::DA_REPLAY_CACHE_CAPACITY,
            replay_cache_ttl_secs: defaults::torii::DA_REPLAY_CACHE_TTL_SECS,
            replay_cache_max_sequence_lag: defaults::torii::DA_REPLAY_CACHE_MAX_SEQUENCE_LAG,
            replay_cache_store_dir: defaults::torii::da_replay_cache_store_dir(),
            manifest_store_dir: defaults::torii::da_manifest_store_dir(),
            governance_metadata_key_hex: None,
            governance_metadata_key_label: defaults::torii::da_governance_metadata_key_label(),
            taikai_anchor: None,
            replication_policy: DaReplicationPolicy::default(),
            rent_policy: DaRentPolicy::default(),
            telemetry_cluster_label: None,
        }
    }
}

impl DaIngest {
    fn parse(self) -> actual::DaIngest {
        let governance_metadata_key = self.governance_metadata_key_hex.as_ref().map(|hex_key| {
            let bytes = hex::decode(hex_key).unwrap_or_else(|err| {
                panic!("invalid governance_metadata_key_hex value `{hex_key}`: {err}")
            });
            <[u8; 32]>::try_from(bytes.as_slice()).unwrap_or_else(|_| {
                panic!(
                    "invalid governance_metadata_key_hex value `{hex_key}`: expected 32-byte key"
                )
            })
        });
        actual::DaIngest {
            replay_cache_capacity: self.replay_cache_capacity,
            replay_cache_ttl: Duration::from_secs(self.replay_cache_ttl_secs),
            replay_cache_max_sequence_lag: self.replay_cache_max_sequence_lag,
            replay_cache_store_dir: self.replay_cache_store_dir,
            manifest_store_dir: self.manifest_store_dir,
            governance_metadata_key,
            governance_metadata_key_label: self.governance_metadata_key_label,
            taikai_anchor: self.taikai_anchor.map(DaTaikaiAnchor::parse),
            replication_policy: self.replication_policy.parse(),
            rent_policy: self.rent_policy.into_policy(),
            telemetry_cluster_label: self.telemetry_cluster_label,
        }
    }
}

#[derive(Debug, ReadConfig, Clone, norito::JsonDeserialize)]
#[doc = "User-facing configuration for the DA replication policy."]
pub struct DaReplicationPolicy {
    /// Default retention template applied whenever no class override matches.
    #[config(default = "da_retention_template_default()")]
    pub default_retention: DaRetentionTemplate,
    /// Per-class overrides keyed by blob class string.
    #[config(default = "Vec::new()")]
    pub overrides: Vec<DaReplicationOverride>,
    /// Optional availability-class overrides for Taikai segments.
    #[config(default = "Vec::new()")]
    pub taikai_availability: Vec<DaTaikaiAvailabilityOverride>,
}

#[derive(Debug, ReadConfig, Clone, norito::JsonDeserialize)]
#[doc = "Override entry for a specific blob class."]
pub struct DaReplicationOverride {
    /// Blob class identifier (e.g., `taikai_segment`, `custom:42`).
    pub class: String,
    /// Retention template applied when the override matches.
    #[config(default = "da_retention_template_default()")]
    pub retention: DaRetentionTemplate,
}

#[derive(Debug, ReadConfig, Clone, norito::JsonDeserialize)]
#[doc = "Override entry for a Taikai availability class."]
pub struct DaTaikaiAvailabilityOverride {
    /// Availability class identifier (`hot`, `warm`, `cold`).
    pub availability_class: String,
    /// Retention template applied when the override matches.
    #[config(default = "da_retention_template_default()")]
    pub retention: DaRetentionTemplate,
}

#[derive(Debug, ReadConfig, Clone, norito::JsonDeserialize)]
#[doc = "Retention template describing hot/cold windows and replica targets."]
pub struct DaRetentionTemplate {
    /// Duration (seconds) the blob must remain in the hot tier.
    pub hot_retention_secs: u64,
    /// Duration (seconds) the blob must remain in the cold tier.
    pub cold_retention_secs: u64,
    /// Minimum replica count for the blob class.
    pub required_replicas: u16,
    /// Storage class identifier (`hot`, `warm`, `cold`).
    pub storage_class: String,
    /// Governance tag recorded alongside the manifest.
    pub governance_tag: String,
}

impl DaReplicationPolicy {
    fn parse(self) -> actual::DaReplicationPolicy {
        let default = self.default_retention.into_policy();
        let mut overrides = BTreeMap::new();
        for DaReplicationOverride { class, retention } in self.overrides {
            let parsed_class = parse_blob_class(&class);
            let policy = retention.into_policy();
            assert!(
                overrides.insert(parsed_class, policy).is_none(),
                "duplicate DA replication override for class `{class}`",
            );
        }
        let mut taikai_availability = BTreeMap::new();
        for DaTaikaiAvailabilityOverride {
            availability_class,
            retention,
        } in self.taikai_availability
        {
            let parsed_class = parse_taikai_availability_class(&availability_class);
            let policy = retention.into_policy();
            assert!(
                taikai_availability.insert(parsed_class, policy).is_none(),
                "duplicate Taikai availability override `{availability_class}`",
            );
        }
        actual::DaReplicationPolicy::new(default, overrides, taikai_availability)
    }
}

impl Default for DaReplicationPolicy {
    fn default() -> Self {
        let default_policy = defaults::torii::da_replication_default_policy();
        let default_retention = DaRetentionTemplate::from_policy(&default_policy);
        let overrides = defaults::torii::da_replication_overrides()
            .into_iter()
            .map(|(class, policy)| DaReplicationOverride {
                class: format_blob_class(class),
                retention: DaRetentionTemplate::from_policy(&policy),
            })
            .collect();
        let taikai_availability = defaults::torii::taikai_availability_overrides()
            .into_iter()
            .map(
                |(availability_class, policy)| DaTaikaiAvailabilityOverride {
                    availability_class: format_taikai_availability_class(availability_class),
                    retention: DaRetentionTemplate::from_policy(&policy),
                },
            )
            .collect();
        Self {
            default_retention,
            overrides,
            taikai_availability,
        }
    }
}

impl DaRetentionTemplate {
    fn from_policy(policy: &RetentionPolicy) -> Self {
        Self {
            hot_retention_secs: policy.hot_retention_secs,
            cold_retention_secs: policy.cold_retention_secs,
            required_replicas: policy.required_replicas,
            storage_class: storage_class_to_str(policy.storage_class).to_string(),
            governance_tag: policy.governance_tag.0.clone(),
        }
    }

    fn into_policy(self) -> RetentionPolicy {
        RetentionPolicy {
            hot_retention_secs: self.hot_retention_secs,
            cold_retention_secs: self.cold_retention_secs,
            required_replicas: self.required_replicas,
            storage_class: parse_storage_class(&self.storage_class),
            governance_tag: GovernanceTag::new(self.governance_tag),
        }
    }
}

impl Default for DaRetentionTemplate {
    fn default() -> Self {
        DaRetentionTemplate::from_policy(&defaults::torii::da_replication_default_policy())
    }
}

fn da_retention_template_default() -> DaRetentionTemplate {
    DaRetentionTemplate::default()
}

fn da_replication_policy_default() -> DaReplicationPolicy {
    DaReplicationPolicy::default()
}

#[derive(Debug, ReadConfig, Clone, Copy, norito::JsonDeserialize)]
#[doc = "User-facing configuration for DA rent and incentive policy."]
pub struct DaRentPolicy {
    /// Base XOR charged per GiB-month (micro units).
    #[config(default = "defaults::torii::DA_RENT_BASE_RATE_PER_GIB_MONTH_MICRO")]
    pub base_rate_per_gib_month_micro: u128,
    /// Basis points routed to the protocol reserve.
    #[config(default = "defaults::torii::DA_RENT_PROTOCOL_RESERVE_BPS")]
    pub protocol_reserve_bps: u16,
    /// PDP bonus basis points.
    #[config(default = "defaults::torii::DA_RENT_PDP_BONUS_BPS")]
    pub pdp_bonus_bps: u16,
    /// PoTR bonus basis points.
    #[config(default = "defaults::torii::DA_RENT_POTR_BONUS_BPS")]
    pub potr_bonus_bps: u16,
    /// XOR credit per GiB of egress (micro units).
    #[config(default = "defaults::torii::DA_RENT_EGRESS_CREDIT_PER_GIB_MICRO")]
    pub egress_credit_per_gib_micro: u128,
}

impl DaRentPolicy {
    fn from_policy(policy: &DaRentPolicyV1) -> Self {
        Self {
            base_rate_per_gib_month_micro: policy.base_rate_per_gib_month.as_micro(),
            protocol_reserve_bps: policy.protocol_reserve_bps,
            pdp_bonus_bps: policy.pdp_bonus_bps,
            potr_bonus_bps: policy.potr_bonus_bps,
            egress_credit_per_gib_micro: policy.egress_credit_per_gib.as_micro(),
        }
    }

    fn into_policy(self) -> DaRentPolicyV1 {
        DaRentPolicyV1::from_components(
            self.base_rate_per_gib_month_micro,
            self.protocol_reserve_bps,
            self.pdp_bonus_bps,
            self.potr_bonus_bps,
            self.egress_credit_per_gib_micro,
        )
    }
}

impl Default for DaRentPolicy {
    fn default() -> Self {
        Self::from_policy(&DaRentPolicyV1::default())
    }
}

fn da_rent_policy_default() -> DaRentPolicy {
    DaRentPolicy::default()
}

fn parse_blob_class(value: &str) -> BlobClass {
    let normalized = value.trim().to_ascii_lowercase().replace('-', "_");
    match normalized.as_str() {
        "taikai_segment" | "taikai" => BlobClass::TaikaiSegment,
        "nexus_lane_sidecar" | "lane_sidecar" | "sidecar" => BlobClass::NexusLaneSidecar,
        "governance_artifact" | "governance" => BlobClass::GovernanceArtifact,
        _ => normalized.strip_prefix("custom:").map_or_else(
            || {
                panic!("unsupported blob class `{value}`");
            },
            |rest| {
                let code = rest.parse::<u16>().unwrap_or_else(|err| {
                    panic!("invalid custom blob class `{value}`: {err}");
                });
                BlobClass::Custom(code)
            },
        ),
    }
}

fn format_blob_class(class: BlobClass) -> String {
    match class {
        BlobClass::TaikaiSegment => "taikai_segment".to_string(),
        BlobClass::NexusLaneSidecar => "nexus_lane_sidecar".to_string(),
        BlobClass::GovernanceArtifact => "governance_artifact".to_string(),
        BlobClass::Custom(code) => format!("custom:{code}"),
    }
}

fn parse_taikai_availability_class(value: &str) -> TaikaiAvailabilityClass {
    match value.trim().to_ascii_lowercase().as_str() {
        "hot" => TaikaiAvailabilityClass::Hot,
        "warm" => TaikaiAvailabilityClass::Warm,
        "cold" => TaikaiAvailabilityClass::Cold,
        other => panic!("unsupported Taikai availability_class `{other}`"),
    }
}

fn format_taikai_availability_class(class: TaikaiAvailabilityClass) -> String {
    match class {
        TaikaiAvailabilityClass::Hot => "hot",
        TaikaiAvailabilityClass::Warm => "warm",
        TaikaiAvailabilityClass::Cold => "cold",
    }
    .to_string()
}

fn parse_storage_class(value: &str) -> SorafsStorageClass {
    match value.trim().to_ascii_lowercase().as_str() {
        "hot" => SorafsStorageClass::Hot,
        "warm" => SorafsStorageClass::Warm,
        "cold" => SorafsStorageClass::Cold,
        other => panic!("unsupported storage_class `{other}`"),
    }
}

fn storage_class_to_str(class: SorafsStorageClass) -> &'static str {
    match class {
        SorafsStorageClass::Hot => "hot",
        SorafsStorageClass::Warm => "warm",
        SorafsStorageClass::Cold => "cold",
    }
}

/// User-facing Taikai anchoring configuration.
#[derive(Debug, ReadConfig, Clone)]
pub struct DaTaikaiAnchor {
    /// HTTP(S) endpoint that receives Taikai artefacts.
    pub endpoint: String,
    /// Optional bearer token for the remote service.
    pub api_token: Option<String>,
    #[config(default = "defaults::torii::DA_TAIKAI_ANCHOR_POLL_INTERVAL_SECS")]
    /// Poll interval in seconds between spool scans.
    pub poll_interval_secs: u64,
}

impl DaTaikaiAnchor {
    fn parse(self) -> actual::DaTaikaiAnchor {
        let endpoint = url::Url::parse(&self.endpoint).unwrap_or_else(|err| {
            panic!("invalid Taikai anchor endpoint `{}`: {err}", self.endpoint)
        });
        actual::DaTaikaiAnchor {
            endpoint,
            api_token: self.api_token,
            poll_interval: Duration::from_secs(self.poll_interval_secs),
        }
    }
}

impl json::JsonSerialize for DaTaikaiAnchor {
    fn json_serialize(&self, out: &mut String) {
        let mut map = Map::new();
        map.insert("endpoint".to_string(), Value::from(self.endpoint.clone()));
        map.insert(
            "api_token".to_string(),
            self.api_token
                .as_ref()
                .map_or(Value::Null, |token| Value::from(token.clone())),
        );
        map.insert(
            "poll_interval_secs".to_string(),
            Value::from(self.poll_interval_secs),
        );
        Value::Object(map).json_serialize(out);
    }
}

impl json::JsonDeserialize for DaTaikaiAnchor {
    fn json_deserialize(parser: &mut json::Parser<'_>) -> core::result::Result<Self, json::Error> {
        let value = Value::json_deserialize(parser)?;
        let mut map = match value {
            Value::Object(map) => map,
            other => {
                return Err(json::Error::Message(format!(
                    "expected Taikai anchor object, found {other:?}"
                )));
            }
        };

        let endpoint = map
            .remove("endpoint")
            .ok_or_else(|| json::Error::Message("missing field `endpoint`".into()))
            .and_then(json::from_value)?;

        let api_token = match map.remove("api_token") {
            Some(Value::Null) | None => None,
            Some(value) => Some(json::from_value(value)?),
        };

        let poll_interval_secs = map
            .remove("poll_interval_secs")
            .ok_or_else(|| json::Error::Message("missing field `poll_interval_secs`".into()))
            .and_then(json::from_value)?;

        Ok(Self {
            endpoint,
            api_token,
            poll_interval_secs,
        })
    }
}

/// User-level configuration container for SoraFS discovery, storage, repair, and GC subsystems.
#[derive(Debug, Default, ReadConfig, Clone, norito::JsonDeserialize)]
pub struct Sorafs {
    /// Configuration for Torii's discovery API/cache.
    #[config(nested)]
    pub discovery: SorafsDiscovery,
    /// Embedded storage worker configuration.
    #[config(nested)]
    pub storage: SorafsStorage,
    /// Repair scheduler configuration.
    #[config(nested)]
    pub repair: SorafsRepair,
    /// GC scheduler configuration.
    #[config(nested)]
    pub gc: SorafsGc,
    /// Quota configuration for SoraFS control-plane endpoints.
    #[config(nested)]
    pub quota: SorafsQuota,
    /// Alias cache policy propagated to gateways and SDKs.
    #[config(nested)]
    pub alias_cache: SorafsAliasCache,
    /// Gateway policy and automation configuration.
    #[config(nested)]
    pub gateway: crate::parameters::user::SorafsGateway,
    /// Proof-of-Retrievability coordinator configuration.
    #[config(nested)]
    pub por: SorafsPor,
}

impl Sorafs {
    fn parse(
        self,
    ) -> (
        actual::SorafsStorage,
        actual::SorafsDiscovery,
        actual::SorafsRepair,
        actual::SorafsGc,
        actual::SorafsQuota,
        actual::SorafsAliasCachePolicy,
        actual::SorafsGateway,
        actual::SorafsPor,
    ) {
        (
            self.storage.parse(),
            self.discovery.parse(),
            self.repair.parse(),
            self.gc.parse(),
            self.quota.parse(),
            self.alias_cache.parse(),
            self.gateway.parse(),
            self.por.parse(),
        )
    }
}

/// User-level configuration container for the embedded SoraFS storage worker.
#[derive(Debug, ReadConfig, Clone, norito::JsonDeserialize)]
pub struct SorafsStorage {
    /// Enables the embedded storage worker.
    #[config(
        env = "SORAFS_STORAGE_ENABLED",
        default = "defaults::sorafs::storage::ENABLED"
    )]
    pub enabled: bool,
    /// Root directory used to persist chunk data and manifests.
    #[config(
        env = "SORAFS_STORAGE_DATA_DIR",
        default = "defaults::sorafs::storage::data_dir()"
    )]
    pub data_dir: PathBuf,
    /// Maximum on-disk capacity allotted to stored chunks (bytes).
    #[config(default = "defaults::sorafs::storage::MAX_CAPACITY_BYTES")]
    pub max_capacity_bytes: Bytes<u64>,
    /// Maximum number of fetch streams served concurrently.
    #[config(default = "defaults::sorafs::storage::MAX_PARALLEL_FETCHES")]
    pub max_parallel_fetches: usize,
    /// Maximum number of manifests pinned before back-pressure engages.
    #[config(default = "defaults::sorafs::storage::MAX_PINS")]
    pub max_pins: usize,
    /// Interval between Proof-of-Retrievability sampling rounds (seconds).
    #[config(default = "defaults::sorafs::storage::POR_SAMPLE_INTERVAL_SECS")]
    pub por_sample_interval_secs: u64,
    /// Optional alias advertised in telemetry for this storage worker.
    pub alias: Option<String>,
    /// Overrides applied when advertising provider capabilities.
    #[config(nested)]
    pub adverts: SorafsAdvertOverrides,
    /// Optional smoothing configuration applied to metering outputs.
    #[config(nested)]
    pub metering_smoothing: SorafsMeteringSmoothing,
    /// Stream-token issuance configuration for chunk-range gateways.
    #[config(nested)]
    pub stream_tokens: SorafsStreamTokenConfig,
    /// Authentication and rate limits for manifest pin submissions.
    #[config(nested)]
    pub pin: SorafsStoragePin,
    /// Optional filesystem directory used to publish governance artefacts.
    pub governance_dag_dir: Option<PathBuf>,
}

impl Default for SorafsStorage {
    fn default() -> Self {
        Self {
            enabled: defaults::sorafs::storage::ENABLED,
            data_dir: defaults::sorafs::storage::data_dir(),
            max_capacity_bytes: defaults::sorafs::storage::MAX_CAPACITY_BYTES,
            max_parallel_fetches: defaults::sorafs::storage::MAX_PARALLEL_FETCHES,
            max_pins: defaults::sorafs::storage::MAX_PINS,
            por_sample_interval_secs: defaults::sorafs::storage::POR_SAMPLE_INTERVAL_SECS,
            alias: defaults::sorafs::storage::alias(),
            adverts: SorafsAdvertOverrides::default(),
            metering_smoothing: SorafsMeteringSmoothing::default(),
            stream_tokens: SorafsStreamTokenConfig::default(),
            pin: SorafsStoragePin::default(),
            governance_dag_dir: defaults::sorafs::storage::governance_dir(),
        }
    }
}

impl SorafsStorage {
    fn parse(self) -> actual::SorafsStorage {
        actual::SorafsStorage {
            enabled: self.enabled,
            data_dir: self.data_dir,
            max_capacity_bytes: self.max_capacity_bytes,
            max_parallel_fetches: self.max_parallel_fetches,
            max_pins: self.max_pins,
            por_sample_interval_secs: self.por_sample_interval_secs,
            alias: self.alias.or_else(super::defaults::sorafs::storage::alias),
            adverts: self.adverts.parse(),
            metering_smoothing: self.metering_smoothing.parse(),
            stream_tokens: self.stream_tokens.parse(),
            pin: self.pin.parse(),
            governance_dag_dir: self.governance_dag_dir,
        }
    }
}

/// Authentication and abuse controls for `/v1/sorafs/storage/pin`.
#[derive(Debug, ReadConfig, Clone, norito::JsonDeserialize, Default)]
pub struct SorafsStoragePin {
    /// Whether a bearer token is required to submit a pin request.
    #[config(env = "SORAFS_STORAGE_PIN_REQUIRE_TOKEN", default = "false")]
    pub require_token: bool,
    /// Static allow-list of bearer tokens accepted by the gateway.
    #[config(default = "Vec::new()")]
    pub tokens: Vec<String>,
    /// Optional CIDR allow-list limiting clients that may submit pin requests.
    #[config(default = "Vec::new()")]
    pub allow_cidrs: Vec<String>,
    /// Per-client rate limits applied to pin submissions.
    #[config(nested)]
    pub rate_limit: SorafsPinRateLimit,
}

impl SorafsStoragePin {
    fn parse(self) -> actual::SorafsStoragePin {
        let tokens: BTreeSet<String> = self
            .tokens
            .into_iter()
            .map(|token| token.trim().to_string())
            .filter(|token| !token.is_empty())
            .collect();
        actual::SorafsStoragePin {
            require_token: self.require_token,
            tokens,
            allow_cidrs: self.allow_cidrs,
            rate_limit: self.rate_limit.parse(),
        }
    }
}

/// Per-client rate limits for manifest pin submissions.
#[derive(Debug, ReadConfig, Clone, Copy, norito::JsonDeserialize)]
pub struct SorafsPinRateLimit {
    /// Maximum requests permitted within the window. `None` disables rate limiting.
    #[config(env = "SORAFS_STORAGE_PIN_RATE_LIMIT_MAX_REQUESTS")]
    pub max_requests: Option<u32>,
    /// Duration of the rolling window (seconds).
    #[config(env = "SORAFS_STORAGE_PIN_RATE_LIMIT_WINDOW_SECS", default = "1u64")]
    pub window_secs: u64,
    /// Optional ban duration applied after exceeding the limit (seconds).
    #[config(env = "SORAFS_STORAGE_PIN_RATE_LIMIT_BAN_SECS")]
    pub ban_secs: Option<u64>,
}

impl Default for SorafsPinRateLimit {
    fn default() -> Self {
        Self {
            max_requests: None,
            window_secs: 1,
            ban_secs: None,
        }
    }
}

impl SorafsPinRateLimit {
    fn parse(self) -> actual::SorafsGatewayRateLimit {
        actual::SorafsGatewayRateLimit {
            max_requests: self.max_requests.and_then(NonZeroU32::new),
            window: Duration::from_secs(self.window_secs.max(1)),
            ban: self.ban_secs.map(Duration::from_secs),
        }
    }
}

/// User-level configuration for the repair scheduler.
#[derive(Debug, ReadConfig, Clone, norito::JsonDeserialize)]
pub struct SorafsRepair {
    /// Enable the repair scheduler.
    #[config(default = "defaults::sorafs::repair::ENABLED")]
    pub enabled: bool,
    /// Optional directory for durable repair state.
    pub state_dir: Option<PathBuf>,
    /// Claim TTL for repair tickets (seconds).
    #[config(default = "defaults::sorafs::repair::CLAIM_TTL_SECS")]
    pub claim_ttl_secs: u64,
    /// Heartbeat interval/TTL for active claims (seconds).
    #[config(default = "defaults::sorafs::repair::HEARTBEAT_INTERVAL_SECS")]
    pub heartbeat_interval_secs: u64,
    /// Maximum number of attempts before escalation.
    #[config(default = "defaults::sorafs::repair::MAX_ATTEMPTS")]
    pub max_attempts: u32,
    /// Concurrent repair workers per node.
    #[config(default = "defaults::sorafs::repair::WORKER_CONCURRENCY")]
    pub worker_concurrency: usize,
    /// Initial retry backoff for failed repairs (seconds).
    #[config(default = "defaults::sorafs::repair::BACKOFF_INITIAL_SECS")]
    pub backoff_initial_secs: u64,
    /// Maximum retry backoff for failed repairs (seconds).
    #[config(default = "defaults::sorafs::repair::BACKOFF_MAX_SECS")]
    pub backoff_max_secs: u64,
    /// Default penalty used for scheduler-generated repair slash proposals (nano-XOR).
    #[config(default = "defaults::sorafs::repair::DEFAULT_SLASH_PENALTY_NANO")]
    pub default_slash_penalty_nano: u128,
}

impl Default for SorafsRepair {
    fn default() -> Self {
        Self {
            enabled: defaults::sorafs::repair::ENABLED,
            state_dir: defaults::sorafs::repair::state_dir(),
            claim_ttl_secs: defaults::sorafs::repair::CLAIM_TTL_SECS,
            heartbeat_interval_secs: defaults::sorafs::repair::HEARTBEAT_INTERVAL_SECS,
            max_attempts: defaults::sorafs::repair::MAX_ATTEMPTS,
            worker_concurrency: defaults::sorafs::repair::WORKER_CONCURRENCY,
            backoff_initial_secs: defaults::sorafs::repair::BACKOFF_INITIAL_SECS,
            backoff_max_secs: defaults::sorafs::repair::BACKOFF_MAX_SECS,
            default_slash_penalty_nano: defaults::sorafs::repair::DEFAULT_SLASH_PENALTY_NANO,
        }
    }
}

impl SorafsRepair {
    fn parse(self) -> actual::SorafsRepair {
        actual::SorafsRepair {
            enabled: self.enabled,
            state_dir: self.state_dir,
            claim_ttl_secs: self.claim_ttl_secs.max(1),
            heartbeat_interval_secs: self.heartbeat_interval_secs.max(1),
            max_attempts: self.max_attempts.max(1),
            worker_concurrency: self.worker_concurrency.max(1),
            backoff_initial_secs: self.backoff_initial_secs.max(1),
            backoff_max_secs: self.backoff_max_secs.max(self.backoff_initial_secs.max(1)),
            default_slash_penalty_nano: self.default_slash_penalty_nano.max(1),
        }
    }
}

/// User-level configuration for the GC scheduler.
#[derive(Debug, ReadConfig, Clone, norito::JsonDeserialize)]
pub struct SorafsGc {
    /// Enable the GC worker.
    #[config(default = "defaults::sorafs::gc::ENABLED")]
    pub enabled: bool,
    /// Optional directory for durable GC state.
    pub state_dir: Option<PathBuf>,
    /// GC cadence (seconds).
    #[config(default = "defaults::sorafs::gc::INTERVAL_SECS")]
    pub interval_secs: u64,
    /// Maximum deletions per GC run.
    #[config(default = "defaults::sorafs::gc::MAX_DELETIONS_PER_RUN")]
    pub max_deletions_per_run: u32,
    /// Grace window for retention expiry (seconds).
    #[config(default = "defaults::sorafs::gc::RETENTION_GRACE_SECS")]
    pub retention_grace_secs: u64,
    /// Attempt a GC sweep before rejecting new pins when storage is full.
    #[config(default = "defaults::sorafs::gc::PRE_ADMISSION_SWEEP")]
    pub pre_admission_sweep: bool,
}

impl Default for SorafsGc {
    fn default() -> Self {
        Self {
            enabled: defaults::sorafs::gc::ENABLED,
            state_dir: defaults::sorafs::gc::state_dir(),
            interval_secs: defaults::sorafs::gc::INTERVAL_SECS,
            max_deletions_per_run: defaults::sorafs::gc::MAX_DELETIONS_PER_RUN,
            retention_grace_secs: defaults::sorafs::gc::RETENTION_GRACE_SECS,
            pre_admission_sweep: defaults::sorafs::gc::PRE_ADMISSION_SWEEP,
        }
    }
}

impl SorafsGc {
    fn parse(self) -> actual::SorafsGc {
        actual::SorafsGc {
            enabled: self.enabled,
            state_dir: self.state_dir,
            interval_secs: self.interval_secs.max(1),
            max_deletions_per_run: self.max_deletions_per_run.max(1),
            retention_grace_secs: self.retention_grace_secs,
            pre_admission_sweep: self.pre_admission_sweep,
        }
    }
}

#[cfg(test)]
mod sorafs_repair_gc_tests {
    use super::*;

    #[test]
    fn sorafs_repair_and_gc_parse_clamps_values() {
        let repair = SorafsRepair {
            enabled: true,
            state_dir: None,
            claim_ttl_secs: 0,
            heartbeat_interval_secs: 0,
            max_attempts: 0,
            worker_concurrency: 0,
            backoff_initial_secs: 0,
            backoff_max_secs: 0,
            default_slash_penalty_nano: 0,
        };
        let actual = repair.parse();
        assert_eq!(actual.claim_ttl_secs, 1);
        assert_eq!(actual.heartbeat_interval_secs, 1);
        assert_eq!(actual.max_attempts, 1);
        assert_eq!(actual.worker_concurrency, 1);
        assert_eq!(actual.backoff_initial_secs, 1);
        assert_eq!(actual.backoff_max_secs, 1);
        assert_eq!(actual.default_slash_penalty_nano, 1);

        let gc = SorafsGc {
            enabled: true,
            state_dir: None,
            interval_secs: 0,
            max_deletions_per_run: 0,
            retention_grace_secs: 42,
            pre_admission_sweep: false,
        };
        let actual_gc = gc.parse();
        assert_eq!(actual_gc.interval_secs, 1);
        assert_eq!(actual_gc.max_deletions_per_run, 1);
        assert_eq!(actual_gc.retention_grace_secs, 42);
        assert!(!actual_gc.pre_admission_sweep);
    }
}

/// User-level configuration for the PoR coordinator runtime.
#[derive(Debug, ReadConfig, Clone, norito::JsonDeserialize)]
pub struct SorafsPor {
    /// Enable the PoR coordinator runtime.
    #[config(env = "SORAFS_POR_ENABLED", default = "defaults::sorafs::por::ENABLED")]
    pub enabled: bool,
    /// Epoch interval (seconds) controlling challenge generation cadence.
    #[config(
        env = "SORAFS_POR_EPOCH_INTERVAL_SECS",
        default = "defaults::sorafs::por::EPOCH_INTERVAL_SECS"
    )]
    pub epoch_interval_secs: u64,
    /// Response window (seconds) granted to providers for PoR proofs.
    #[config(
        env = "SORAFS_POR_RESPONSE_WINDOW_SECS",
        default = "defaults::sorafs::por::RESPONSE_WINDOW_SECS"
    )]
    pub response_window_secs: u64,
    /// Filesystem directory used for governance DAG outputs.
    #[config(
        env = "SORAFS_POR_GOVERNANCE_DIR",
        default = "defaults::sorafs::por::governance_dir()"
    )]
    pub governance_dir: PathBuf,
    /// Optional deterministic randomness seed represented as hex-encoded bytes.
    pub randomness_seed_hex: Option<String>,
}

impl Default for SorafsPor {
    fn default() -> Self {
        Self {
            enabled: defaults::sorafs::por::ENABLED,
            epoch_interval_secs: defaults::sorafs::por::EPOCH_INTERVAL_SECS,
            response_window_secs: defaults::sorafs::por::RESPONSE_WINDOW_SECS,
            governance_dir: defaults::sorafs::por::governance_dir(),
            randomness_seed_hex: defaults::sorafs::por::randomness_seed_hex(),
        }
    }
}

impl SorafsPor {
    fn parse(self) -> actual::SorafsPor {
        let randomness_seed = self.randomness_seed_hex.as_ref().map(|hex| {
            let bytes =
                hex::decode(hex).expect("failed to decode SORAFS_POR_RANDOMNESS_SEED hex value");
            let array: [u8; 32] = bytes
                .try_into()
                .expect("SORAFS_POR randomness seed must be 32 bytes");
            array
        });
        actual::SorafsPor {
            enabled: self.enabled,
            epoch_interval_secs: self.epoch_interval_secs,
            response_window_secs: self.response_window_secs,
            governance_dag_dir: self.governance_dir,
            randomness_seed,
        }
    }
}

/// User-level configuration for stream-token issuance.
#[derive(Debug, ReadConfig, Clone, norito::JsonDeserialize)]
pub struct SorafsStreamTokenConfig {
    /// Enable stream-token issuance.
    #[config(
        env = "SORAFS_STREAM_TOKENS_ENABLED",
        default = "defaults::sorafs::storage::tokens::ENABLED"
    )]
    pub enabled: bool,
    /// Filesystem path to the Ed25519 signing key.
    pub signing_key_path: Option<PathBuf>,
    /// Public-key version advertised in tokens.
    #[config(default = "defaults::sorafs::storage::tokens::KEY_VERSION")]
    pub key_version: u32,
    /// Default TTL applied to issued tokens (seconds).
    #[config(default = "defaults::sorafs::storage::tokens::DEFAULT_TTL_SECS")]
    pub default_ttl_secs: u64,
    /// Default maximum concurrent streams per token.
    #[config(default = "defaults::sorafs::storage::tokens::DEFAULT_MAX_STREAMS")]
    pub default_max_streams: u16,
    /// Default sustained throughput per token (bytes per second).
    #[config(default = "defaults::sorafs::storage::tokens::DEFAULT_RATE_LIMIT_BYTES")]
    pub default_rate_limit_bytes: u64,
    /// Default refresh allowance (requests per minute).
    #[config(default = "defaults::sorafs::storage::tokens::DEFAULT_REQUESTS_PER_MINUTE")]
    pub default_requests_per_minute: u32,
}

impl Default for SorafsStreamTokenConfig {
    fn default() -> Self {
        Self {
            enabled: defaults::sorafs::storage::tokens::ENABLED,
            signing_key_path: defaults::sorafs::storage::tokens::signing_key_path(),
            key_version: defaults::sorafs::storage::tokens::KEY_VERSION,
            default_ttl_secs: defaults::sorafs::storage::tokens::DEFAULT_TTL_SECS,
            default_max_streams: defaults::sorafs::storage::tokens::DEFAULT_MAX_STREAMS,
            default_rate_limit_bytes: defaults::sorafs::storage::tokens::DEFAULT_RATE_LIMIT_BYTES,
            default_requests_per_minute:
                defaults::sorafs::storage::tokens::DEFAULT_REQUESTS_PER_MINUTE,
        }
    }
}

impl SorafsStreamTokenConfig {
    fn parse(self) -> actual::SorafsTokenConfig {
        actual::SorafsTokenConfig {
            enabled: self.enabled,
            signing_key_path: self.signing_key_path,
            key_version: self.key_version,
            default_ttl_secs: self.default_ttl_secs,
            default_max_streams: self.default_max_streams,
            default_rate_limit_bytes: self.default_rate_limit_bytes,
            default_requests_per_minute: self.default_requests_per_minute,
        }
    }
}

/// User-level quota configuration for SoraFS control-plane endpoints.
#[derive(Debug, ReadConfig, Clone, Copy, norito::JsonDeserialize)]
pub struct SorafsQuota {
    /// Optional override for the number of capacity declarations allowed per window.
    pub capacity_declaration_max_events: Option<u32>,
    /// Rolling window (seconds) applied to capacity declaration quotas.
    #[config(default = "defaults::torii::SORAFS_QUOTA_DECLARATION_WINDOW_SECS")]
    pub capacity_declaration_window_secs: u64,
    /// Optional override for the number of capacity telemetry reports allowed per window.
    pub capacity_telemetry_max_events: Option<u32>,
    /// Rolling window (seconds) applied to capacity telemetry quotas.
    #[config(default = "defaults::torii::SORAFS_QUOTA_TELEMETRY_WINDOW_SECS")]
    pub capacity_telemetry_window_secs: u64,
    /// Optional override for the number of deal telemetry submissions allowed per window.
    pub deal_telemetry_max_events: Option<u32>,
    /// Rolling window (seconds) applied to deal telemetry quotas.
    #[config(default = "defaults::torii::SORAFS_QUOTA_DEAL_TELEMETRY_WINDOW_SECS")]
    pub deal_telemetry_window_secs: u64,
    /// Optional override for the number of capacity disputes allowed per window.
    pub capacity_dispute_max_events: Option<u32>,
    /// Rolling window (seconds) applied to capacity dispute quotas.
    #[config(default = "defaults::torii::SORAFS_QUOTA_DISPUTE_WINDOW_SECS")]
    pub capacity_dispute_window_secs: u64,
    /// Optional override for the number of manifest pin submissions allowed per window.
    pub storage_pin_max_events: Option<u32>,
    /// Rolling window (seconds) applied to storage pin quotas.
    #[config(default = "defaults::torii::SORAFS_QUOTA_STORAGE_PIN_WINDOW_SECS")]
    pub storage_pin_window_secs: u64,
    /// Optional override for the number of PoR submissions allowed per window.
    pub por_submission_max_events: Option<u32>,
    /// Rolling window (seconds) applied to PoR submission quotas.
    #[config(default = "defaults::torii::SORAFS_QUOTA_POR_WINDOW_SECS")]
    pub por_submission_window_secs: u64,
}

impl Default for SorafsQuota {
    fn default() -> Self {
        Self {
            capacity_declaration_max_events: defaults::torii::SORAFS_QUOTA_DECLARATION_MAX_EVENTS,
            capacity_declaration_window_secs: defaults::torii::SORAFS_QUOTA_DECLARATION_WINDOW_SECS,
            capacity_telemetry_max_events: defaults::torii::SORAFS_QUOTA_TELEMETRY_MAX_EVENTS,
            capacity_telemetry_window_secs: defaults::torii::SORAFS_QUOTA_TELEMETRY_WINDOW_SECS,
            deal_telemetry_max_events: defaults::torii::SORAFS_QUOTA_DEAL_TELEMETRY_MAX_EVENTS,
            deal_telemetry_window_secs: defaults::torii::SORAFS_QUOTA_DEAL_TELEMETRY_WINDOW_SECS,
            capacity_dispute_max_events: defaults::torii::SORAFS_QUOTA_DISPUTE_MAX_EVENTS,
            capacity_dispute_window_secs: defaults::torii::SORAFS_QUOTA_DISPUTE_WINDOW_SECS,
            storage_pin_max_events: defaults::torii::SORAFS_QUOTA_STORAGE_PIN_MAX_EVENTS,
            storage_pin_window_secs: defaults::torii::SORAFS_QUOTA_STORAGE_PIN_WINDOW_SECS,
            por_submission_max_events: defaults::torii::SORAFS_QUOTA_POR_MAX_EVENTS,
            por_submission_window_secs: defaults::torii::SORAFS_QUOTA_POR_WINDOW_SECS,
        }
    }
}

impl SorafsQuota {
    fn parse(self) -> actual::SorafsQuota {
        fn window(max: Option<u32>, secs: u64) -> actual::SorafsQuotaWindow {
            actual::SorafsQuotaWindow {
                max_events: max.and_then(std::num::NonZeroU32::new),
                window: Duration::from_secs(secs),
            }
        }

        actual::SorafsQuota {
            capacity_declaration: window(
                self.capacity_declaration_max_events
                    .or(super::defaults::torii::SORAFS_QUOTA_DECLARATION_MAX_EVENTS),
                self.capacity_declaration_window_secs,
            ),
            capacity_telemetry: window(
                self.capacity_telemetry_max_events
                    .or(super::defaults::torii::SORAFS_QUOTA_TELEMETRY_MAX_EVENTS),
                self.capacity_telemetry_window_secs,
            ),
            deal_telemetry: window(
                self.deal_telemetry_max_events
                    .or(super::defaults::torii::SORAFS_QUOTA_DEAL_TELEMETRY_MAX_EVENTS),
                self.deal_telemetry_window_secs,
            ),
            capacity_dispute: window(
                self.capacity_dispute_max_events
                    .or(super::defaults::torii::SORAFS_QUOTA_DISPUTE_MAX_EVENTS),
                self.capacity_dispute_window_secs,
            ),
            storage_pin: window(
                self.storage_pin_max_events
                    .or(super::defaults::torii::SORAFS_QUOTA_STORAGE_PIN_MAX_EVENTS),
                self.storage_pin_window_secs,
            ),
            por_submission: window(
                self.por_submission_max_events
                    .or(super::defaults::torii::SORAFS_QUOTA_POR_MAX_EVENTS),
                self.por_submission_window_secs,
            ),
        }
    }
}

/// User-level configuration for the SoraFS alias cache policy.
#[derive(Debug, ReadConfig, Clone, Copy, norito::JsonDeserialize)]
pub struct SorafsAliasCache {
    /// Positive TTL in seconds applied to cached alias proofs.
    #[config(default = "defaults::torii::SORAFS_ALIAS_POSITIVE_TTL_SECS")]
    pub positive_ttl: u64,
    /// Refresh window in seconds before the positive TTL elapses.
    #[config(default = "defaults::torii::SORAFS_ALIAS_REFRESH_WINDOW_SECS")]
    pub refresh_window: u64,
    /// Hard expiry in seconds after which stale proofs are rejected.
    #[config(default = "defaults::torii::SORAFS_ALIAS_HARD_EXPIRY_SECS")]
    pub hard_expiry: u64,
    /// Negative cache TTL in seconds for missing aliases.
    #[config(default = "defaults::torii::SORAFS_ALIAS_NEGATIVE_TTL_SECS")]
    pub negative_ttl: u64,
    /// TTL in seconds for revoked aliases (`410 Gone` responses).
    #[config(default = "defaults::torii::SORAFS_ALIAS_REVOCATION_TTL_SECS")]
    pub revocation_ttl: u64,
    /// Maximum age in seconds tolerated before alias proof bundles must rotate.
    #[config(default = "defaults::torii::SORAFS_ALIAS_ROTATION_MAX_AGE_SECS")]
    pub rotation_max_age: u64,
    /// Grace period in seconds applied after an approved successor before refusing predecessor proofs.
    #[config(default = "defaults::torii::SORAFS_ALIAS_SUCCESSOR_GRACE_SECS")]
    pub successor_grace: u64,
    /// Grace period in seconds applied to governance rotation events.
    #[config(default = "defaults::torii::SORAFS_ALIAS_GOVERNANCE_GRACE_SECS")]
    pub governance_grace: u64,
}

impl Default for SorafsAliasCache {
    fn default() -> Self {
        Self {
            positive_ttl: defaults::torii::SORAFS_ALIAS_POSITIVE_TTL_SECS,
            refresh_window: defaults::torii::SORAFS_ALIAS_REFRESH_WINDOW_SECS,
            hard_expiry: defaults::torii::SORAFS_ALIAS_HARD_EXPIRY_SECS,
            negative_ttl: defaults::torii::SORAFS_ALIAS_NEGATIVE_TTL_SECS,
            revocation_ttl: defaults::torii::SORAFS_ALIAS_REVOCATION_TTL_SECS,
            rotation_max_age: defaults::torii::SORAFS_ALIAS_ROTATION_MAX_AGE_SECS,
            successor_grace: defaults::torii::SORAFS_ALIAS_SUCCESSOR_GRACE_SECS,
            governance_grace: defaults::torii::SORAFS_ALIAS_GOVERNANCE_GRACE_SECS,
        }
    }
}

impl SorafsAliasCache {
    fn parse(self) -> actual::SorafsAliasCachePolicy {
        actual::SorafsAliasCachePolicy {
            positive_ttl: Duration::from_secs(self.positive_ttl),
            refresh_window: Duration::from_secs(self.refresh_window),
            hard_expiry: Duration::from_secs(self.hard_expiry),
            negative_ttl: Duration::from_secs(self.negative_ttl),
            revocation_ttl: Duration::from_secs(self.revocation_ttl),
            rotation_max_age: Duration::from_secs(self.rotation_max_age),
            successor_grace: Duration::from_secs(self.successor_grace),
            governance_grace: Duration::from_secs(self.governance_grace),
        }
    }
}

/// User-level configuration for the SoraFS gateway policy and automation surface.
#[derive(Debug, ReadConfig, Clone, norito::JsonDeserialize)]
pub struct SorafsGateway {
    /// Require manifest envelopes on incoming requests.
    #[config(default = "defaults::sorafs::gateway::REQUIRE_MANIFEST_ENVELOPE")]
    pub require_manifest_envelope: bool,
    /// Enforce admission registry membership checks.
    #[config(default = "defaults::sorafs::gateway::ENFORCE_ADMISSION")]
    pub enforce_admission: bool,
    /// Enforce advertised capabilities before serving data.
    #[config(default = "defaults::sorafs::gateway::ENFORCE_CAPABILITIES")]
    pub enforce_capabilities: bool,
    /// Directory containing SoraNet salt announcements (Norito JSON files).
    pub salt_schedule_dir: Option<PathBuf>,
    /// Rolling-window rate limiting configuration.
    #[config(nested)]
    pub rate_limit: SorafsGatewayRateLimit,
    /// Denylist bootstrap configuration.
    #[config(nested)]
    pub denylist: SorafsGatewayDenylist,
    /// ACME automation configuration.
    #[config(nested)]
    pub acme: SorafsGatewayAcme,
    /// High-level rollout phase controlling the default anonymity policy.
    #[config(default = "defaults::sorafs::gateway::rollout_phase()")]
    pub rollout_phase: String,
    /// Optional staged anonymity policy override for SoraNet transports.
    pub anonymity_policy: Option<String>,
    /// Optional direct-mode override configuration.
    pub direct_mode: Option<SorafsGatewayDirectMode>,
}

impl Default for SorafsGateway {
    fn default() -> Self {
        Self {
            require_manifest_envelope: defaults::sorafs::gateway::REQUIRE_MANIFEST_ENVELOPE,
            enforce_admission: defaults::sorafs::gateway::ENFORCE_ADMISSION,
            enforce_capabilities: defaults::sorafs::gateway::ENFORCE_CAPABILITIES,
            salt_schedule_dir: None,
            rate_limit: SorafsGatewayRateLimit::default(),
            denylist: SorafsGatewayDenylist::default(),
            acme: SorafsGatewayAcme::default(),
            rollout_phase: defaults::sorafs::gateway::rollout_phase(),
            anonymity_policy: defaults::sorafs::gateway::anonymity_policy(),
            direct_mode: None,
        }
    }
}

impl SorafsGateway {
    fn parse(self) -> actual::SorafsGateway {
        let Self {
            require_manifest_envelope,
            enforce_admission,
            enforce_capabilities,
            salt_schedule_dir,
            rate_limit,
            denylist,
            acme,
            rollout_phase,
            anonymity_policy,
            direct_mode,
        } = self;

        let rollout_phase = actual::SorafsRolloutPhase::parse(&rollout_phase).unwrap_or_else(|| {
            panic!(
                "invalid `sorafs.gateway.rollout_phase` value `{rollout_phase}`; expected canary|ramp|default or stage_a|stage_b|stage_c aliases"
            )
        });

        let explicit_stage = anonymity_policy.map(|label| {
            actual::SorafsAnonymityStage::parse(&label).unwrap_or_else(|| {
                panic!(
                    "invalid `sorafs.gateway.anonymity_policy` value `{label}`; expected one of anon-guard-pq|anon-majority-pq|anon-strict-pq or stage_a|stage_b|stage_c aliases"
                )
            })
        });

        actual::SorafsGateway {
            require_manifest_envelope,
            enforce_admission,
            enforce_capabilities,
            salt_schedule_dir,
            cdn_policy_path: None,
            rate_limit: rate_limit.parse(),
            denylist: denylist.parse(),
            acme: acme.parse(),
            rollout_phase,
            anonymity_policy: Some(
                explicit_stage.unwrap_or_else(|| rollout_phase.default_anonymity_policy()),
            ),
            direct_mode: direct_mode.map(SorafsGatewayDirectMode::parse),
        }
    }
}

/// User-level rolling-window rate limit configuration.
#[derive(Debug, ReadConfig, Clone, Copy, norito::JsonDeserialize)]
pub struct SorafsGatewayRateLimit {
    /// Maximum requests permitted within the rolling window.
    pub max_requests: Option<u32>,
    /// Duration of the rolling window.
    #[config(default = "defaults::sorafs::gateway::rate_limit::WINDOW")]
    pub window: Duration,
    /// Optional temporary ban duration applied after repeated violations.
    pub ban: Option<Duration>,
}

impl Default for SorafsGatewayRateLimit {
    fn default() -> Self {
        Self {
            max_requests: defaults::sorafs::gateway::rate_limit::MAX_REQUESTS,
            window: defaults::sorafs::gateway::rate_limit::WINDOW,
            ban: defaults::sorafs::gateway::rate_limit::BAN,
        }
    }
}

impl SorafsGatewayRateLimit {
    fn parse(self) -> actual::SorafsGatewayRateLimit {
        actual::SorafsGatewayRateLimit {
            max_requests: self
                .max_requests
                .or(defaults::sorafs::gateway::rate_limit::MAX_REQUESTS)
                .and_then(NonZeroU32::new),
            window: self.window,
            ban: self.ban.or(defaults::sorafs::gateway::rate_limit::BAN),
        }
    }
}

/// Denylist bootstrap configuration for the SoraFS gateway.
#[derive(Debug, ReadConfig, Clone, norito::JsonDeserialize)]
pub struct SorafsGatewayDenylist {
    /// Optional filesystem path to a JSON denylist definition.
    pub path: Option<PathBuf>,
    /// Maximum TTL applied to standard entries when `expires_at` is omitted.
    #[config(default = "defaults::sorafs::gateway::denylist::STANDARD_TTL")]
    pub standard_ttl: Duration,
    /// Maximum TTL applied to emergency entries.
    #[config(default = "defaults::sorafs::gateway::denylist::EMERGENCY_TTL")]
    pub emergency_ttl: Duration,
    /// Review window enforced for emergency canons.
    #[config(default = "defaults::sorafs::gateway::denylist::EMERGENCY_REVIEW_WINDOW")]
    pub emergency_review_window: Duration,
    /// Require governance references for permanent entries.
    #[config(default = "defaults::sorafs::gateway::denylist::REQUIRE_GOVERNANCE_REFERENCE")]
    pub require_governance_reference: bool,
}

impl Default for SorafsGatewayDenylist {
    fn default() -> Self {
        Self {
            path: defaults::sorafs::gateway::denylist::path(),
            standard_ttl: defaults::sorafs::gateway::denylist::STANDARD_TTL,
            emergency_ttl: defaults::sorafs::gateway::denylist::EMERGENCY_TTL,
            emergency_review_window: defaults::sorafs::gateway::denylist::EMERGENCY_REVIEW_WINDOW,
            require_governance_reference:
                defaults::sorafs::gateway::denylist::REQUIRE_GOVERNANCE_REFERENCE,
        }
    }
}

impl SorafsGatewayDenylist {
    fn parse(self) -> actual::SorafsGatewayDenylist {
        actual::SorafsGatewayDenylist {
            path: self.path,
            standard_ttl: self.standard_ttl,
            emergency_ttl: self.emergency_ttl,
            emergency_review_window: self.emergency_review_window,
            require_governance_reference: self.require_governance_reference,
        }
    }
}

/// Challenge toggles for ACME automation.
#[derive(Debug, ReadConfig, Clone, Copy, norito::JsonDeserialize)]
pub struct SorafsGatewayAcmeChallenges {
    /// Solve DNS-01 challenges.
    #[config(default = "defaults::sorafs::gateway::acme::DNS01")]
    pub dns01: bool,
    /// Solve TLS-ALPN-01 challenges.
    #[config(default = "defaults::sorafs::gateway::acme::TLS_ALPN_01")]
    pub tls_alpn_01: bool,
}

impl Default for SorafsGatewayAcmeChallenges {
    fn default() -> Self {
        Self {
            dns01: defaults::sorafs::gateway::acme::DNS01,
            tls_alpn_01: defaults::sorafs::gateway::acme::TLS_ALPN_01,
        }
    }
}

impl SorafsGatewayAcmeChallenges {
    fn parse(self) -> actual::SorafsGatewayAcmeChallenges {
        actual::SorafsGatewayAcmeChallenges {
            dns01: self.dns01,
            tls_alpn_01: self.tls_alpn_01,
        }
    }
}

/// ACME automation configuration for SoraFS gateway certificates.
#[derive(Debug, ReadConfig, Clone, norito::JsonDeserialize)]
pub struct SorafsGatewayAcme {
    /// Enable ACME automation.
    #[config(default = "defaults::sorafs::gateway::acme::ENABLED")]
    pub enabled: bool,
    /// Account email used for ACME registration.
    pub account_email: Option<String>,
    /// ACME directory URL.
    #[config(default = "defaults::sorafs::gateway::acme::directory_url()")]
    pub directory_url: String,
    /// Hostnames covered by the certificate order.
    #[config(default = "defaults::sorafs::gateway::acme::hostnames()")]
    pub hostnames: Vec<String>,
    /// Identifier of the DNS provider automation plug-in.
    pub dns_provider_id: Option<String>,
    /// Renewal window applied before expiry.
    #[config(default = "defaults::sorafs::gateway::acme::RENEWAL_WINDOW")]
    pub renewal_window: Duration,
    /// Backoff duration applied after failures.
    #[config(default = "defaults::sorafs::gateway::acme::RETRY_BACKOFF")]
    pub retry_backoff: Duration,
    /// Maximum jitter applied to retry scheduling.
    #[config(default = "defaults::sorafs::gateway::acme::RETRY_JITTER")]
    pub retry_jitter: Duration,
    /// Challenge toggles to exercise.
    #[config(nested)]
    pub challenges: SorafsGatewayAcmeChallenges,
    /// Initial ECH enabled state advertised via telemetry.
    #[config(default = "defaults::sorafs::gateway::acme::ECH_ENABLED")]
    pub ech_enabled: bool,
}

impl Default for SorafsGatewayAcme {
    fn default() -> Self {
        Self {
            enabled: defaults::sorafs::gateway::acme::ENABLED,
            account_email: defaults::sorafs::gateway::acme::account_email(),
            directory_url: defaults::sorafs::gateway::acme::directory_url(),
            hostnames: defaults::sorafs::gateway::acme::hostnames(),
            dns_provider_id: defaults::sorafs::gateway::acme::dns_provider_id(),
            renewal_window: defaults::sorafs::gateway::acme::RENEWAL_WINDOW,
            retry_backoff: defaults::sorafs::gateway::acme::RETRY_BACKOFF,
            retry_jitter: defaults::sorafs::gateway::acme::RETRY_JITTER,
            challenges: SorafsGatewayAcmeChallenges::default(),
            ech_enabled: defaults::sorafs::gateway::acme::ECH_ENABLED,
        }
    }
}

impl SorafsGatewayAcme {
    fn parse(self) -> actual::SorafsGatewayAcme {
        actual::SorafsGatewayAcme {
            enabled: self.enabled,
            account_email: self.account_email,
            directory_url: self.directory_url,
            hostnames: self.hostnames,
            dns_provider_id: self.dns_provider_id,
            renewal_window: self.renewal_window,
            retry_backoff: self.retry_backoff,
            retry_jitter: self.retry_jitter,
            challenges: self.challenges.parse(),
            ech_enabled: self.ech_enabled,
        }
    }
}

/// Direct-mode override configuration for gateway operation.
#[derive(Debug, ReadConfig, Clone, norito::JsonDeserialize)]
pub struct SorafsGatewayDirectMode {
    /// Provider identifier (hex-encoded) bound to the override.
    pub provider_id_hex: String,
    /// Chain identifier associated with the override.
    pub chain_id: String,
    /// Canonical hostname published for the provider.
    pub canonical_host: String,
    /// Vanity hostname exposed for operator tooling.
    pub vanity_host: String,
    /// Direct-CAR endpoint bound to the canonical host.
    pub direct_car_canonical: String,
    /// Direct-CAR endpoint bound to the vanity host.
    pub direct_car_vanity: String,
    /// Manifest digest associated with the override.
    pub manifest_digest_hex: String,
}

impl SorafsGatewayDirectMode {
    fn parse(self) -> actual::SorafsGatewayDirectMode {
        actual::SorafsGatewayDirectMode {
            provider_id_hex: self.provider_id_hex,
            chain_id: self.chain_id,
            canonical_host: self.canonical_host,
            vanity_host: self.vanity_host,
            direct_car_canonical: self.direct_car_canonical,
            direct_car_vanity: self.direct_car_vanity,
            manifest_digest_hex: self.manifest_digest_hex,
        }
    }
}

/// User-level advert override configuration for SoraFS provider telemetry.
#[derive(Debug, ReadConfig, Clone, norito::JsonDeserialize)]
pub struct SorafsAdvertOverrides {
    /// Optional stake pointer override applied to generated adverts.
    pub stake_pointer: Option<String>,
    /// Availability tier advertised for this node.
    #[config(default = "defaults::sorafs::storage::advert_availability()")]
    pub availability: String,
    /// Maximum retrieval latency advertised (milliseconds).
    #[config(default = "defaults::sorafs::storage::ADVERT_MAX_LATENCY_MS")]
    pub max_latency_ms: u32,
    /// Rendezvous topics broadcast to the discovery mesh.
    #[config(default = "defaults::sorafs::storage::advert_topics()")]
    pub topics: Vec<String>,
}

impl Default for SorafsAdvertOverrides {
    fn default() -> Self {
        Self {
            stake_pointer: None,
            availability: defaults::sorafs::storage::advert_availability(),
            max_latency_ms: defaults::sorafs::storage::ADVERT_MAX_LATENCY_MS,
            topics: defaults::sorafs::storage::advert_topics(),
        }
    }
}

impl SorafsAdvertOverrides {
    fn parse(self) -> actual::SorafsAdvertOverrides {
        actual::SorafsAdvertOverrides {
            stake_pointer: self.stake_pointer,
            availability: self.availability,
            max_latency_ms: self.max_latency_ms,
            topics: self.topics,
        }
    }
}

/// User-level smoothing configuration for metering outputs.
#[derive(Debug, ReadConfig, Clone, Copy, norito::JsonDeserialize)]
pub struct SorafsMeteringSmoothing {
    /// Enable EMA smoothing for GiB·hour counters.
    #[config(default = "false")]
    pub gib_hours_enabled: bool,
    /// Alpha used for GiB·hour EMA (values <= 0 disable smoothing).
    #[config(default = "0.2")]
    pub gib_hours_alpha: f64,
    /// Enable EMA smoothing for PoR success counters.
    #[config(default = "false")]
    pub por_success_enabled: bool,
    /// Alpha used for PoR success EMA (values <= 0 disable smoothing).
    #[config(default = "0.2")]
    pub por_success_alpha: f64,
}

impl Default for SorafsMeteringSmoothing {
    fn default() -> Self {
        Self {
            gib_hours_enabled: false,
            gib_hours_alpha: 0.2,
            por_success_enabled: false,
            por_success_alpha: 0.2,
        }
    }
}

impl SorafsMeteringSmoothing {
    fn parse(self) -> actual::SorafsMeteringSmoothing {
        actual::SorafsMeteringSmoothing {
            gib_hours_alpha: if self.gib_hours_enabled {
                Some(self.gib_hours_alpha)
            } else {
                None
            },
            por_success_alpha: if self.por_success_enabled {
                Some(self.por_success_alpha)
            } else {
                None
            },
        }
    }
}
/// Torii discovery cache configuration for SoraFS provider adverts.
#[derive(Debug, ReadConfig, Clone, norito::JsonDeserialize)]
pub struct SorafsDiscovery {
    /// Enables the SoraFS discovery ingress/egress API.
    #[config(
        env = "TORII_SORAFS_DISCOVERY_ENABLED",
        default = "defaults::torii::SORAFS_DISCOVERY_ENABLED"
    )]
    pub discovery_enabled: bool,
    /// Capability names accepted when validating provider adverts.
    #[config(default = "defaults::torii::sorafs_known_capabilities()")]
    pub known_capabilities: Vec<String>,
    /// Optional governance admission configuration.
    #[config(nested)]
    pub admission: SorafsAdmissionConfig,
}

impl Default for SorafsDiscovery {
    fn default() -> Self {
        Self {
            discovery_enabled: super::defaults::torii::SORAFS_DISCOVERY_ENABLED,
            known_capabilities: super::defaults::torii::sorafs_known_capabilities(),
            admission: SorafsAdmissionConfig::default(),
        }
    }
}

impl SorafsDiscovery {
    fn parse(self) -> actual::SorafsDiscovery {
        actual::SorafsDiscovery {
            discovery_enabled: self.discovery_enabled,
            known_capabilities: self.known_capabilities,
            admission: self.admission.into_actual(),
        }
    }
}

/// Governance admission configuration for SoraFS discovery ingress.
#[derive(Debug, ReadConfig, Clone, Default, norito::JsonDeserialize)]
pub struct SorafsAdmissionConfig {
    /// Directory containing governance envelopes for approved providers.
    #[config(env = "TORII_SORAFS_ADMISSION_DIR")]
    pub envelopes_dir: Option<PathBuf>,
}

impl SorafsAdmissionConfig {
    fn into_actual(self) -> Option<actual::SorafsAdmission> {
        self.envelopes_dir.map(|path| actual::SorafsAdmission {
            envelopes_dir: path,
        })
    }
}
impl IsoBridge {
    fn parse(self) -> actual::IsoBridge {
        actual::IsoBridge {
            enabled: self.enabled,
            dedupe_ttl_secs: self.dedupe_ttl_secs,
            signer: self.signer.map(IsoBridgeSigner::parse),
            account_aliases: self
                .account_aliases
                .into_iter()
                .map(IsoAccountAlias::parse)
                .collect(),
            currency_assets: self
                .currency_assets
                .into_iter()
                .map(IsoCurrencyAsset::parse)
                .collect(),
            reference_data: self.reference_data.parse(),
        }
    }
}

/// User-level configuration for ISO bridge reference data.
#[derive(Debug, ReadConfig, Clone, norito::JsonDeserialize)]
pub struct IsoReferenceData {
    #[config(
        env = "ISO_BRIDGE_REFERENCE_REFRESH_SECS",
        default = "defaults::torii::ISO_BRIDGE_REFERENCE_REFRESH_SECS"
    )]
    /// Refresh cadence for ISO reference datasets (seconds).
    pub refresh_interval_secs: u64,
    /// Optional path to an ANNA/CUSIP crosswalk snapshot.
    pub isin_crosswalk_path: Option<PathBuf>,
    /// Optional path to a BIC↔LEI mapping snapshot.
    pub bic_lei_path: Option<PathBuf>,
    /// Optional path to a MIC directory snapshot.
    pub mic_directory_path: Option<PathBuf>,
    #[config(env = "ISO_BRIDGE_REFERENCE_CACHE_DIR")]
    /// Directory where cached snapshots and provenance metadata are stored.
    pub cache_dir: Option<PathBuf>,
}

impl Default for IsoReferenceData {
    fn default() -> Self {
        Self {
            refresh_interval_secs: defaults::torii::ISO_BRIDGE_REFERENCE_REFRESH_SECS,
            isin_crosswalk_path: None,
            bic_lei_path: None,
            mic_directory_path: None,
            cache_dir: None,
        }
    }
}

impl IsoReferenceData {
    fn parse(self) -> actual::IsoReferenceData {
        actual::IsoReferenceData {
            refresh_interval: Duration::from_secs(self.refresh_interval_secs),
            isin_crosswalk_path: self.isin_crosswalk_path,
            bic_lei_path: self.bic_lei_path,
            mic_directory_path: self.mic_directory_path,
            cache_dir: self.cache_dir,
        }
    }
}

/// User-level configuration container for `IsoBridgeSigner`.
#[derive(Debug, ReadConfig, Clone)]
pub struct IsoBridgeSigner {
    /// Account identifier that authorises bridge actions.
    pub account_id: String,
    /// Private key (with provenance metadata) used for signing.
    pub private_key: WithOrigin<PrivateKey>,
}

impl IsoBridgeSigner {
    fn parse(self) -> actual::IsoBridgeSigner {
        actual::IsoBridgeSigner {
            account_id: self.account_id,
            private_key: self.private_key.into_value(),
        }
    }
}

impl json::JsonDeserialize for IsoBridgeSigner {
    fn json_deserialize(
        parser: &mut json::Parser<'_>,
    ) -> ::core::result::Result<Self, json::Error> {
        let mut visitor = json::MapVisitor::new(parser)?;
        let mut account_id: Option<String> = None;
        let mut private_key: Option<WithOrigin<PrivateKey>> = None;

        while let Some(key) = visitor.next_key()? {
            match key.as_str() {
                "account_id" => {
                    let value = visitor.parse_value::<String>()?;
                    account_id = Some(value);
                }
                "private_key" => {
                    let encoded = visitor.parse_value::<String>()?;
                    let parsed = PrivateKey::from_str(&encoded).map_err(|err| {
                        json::Error::InvalidField {
                            field: "private_key".into(),
                            message: err.to_string(),
                        }
                    })?;
                    private_key = Some(WithOrigin::inline(parsed));
                }
                other => {
                    return Err(json::Error::unknown_field(other));
                }
            }
        }

        visitor.finish()?;

        let account_id = account_id.ok_or_else(|| json::Error::missing_field("account_id"))?;
        let private_key = private_key.ok_or_else(|| json::Error::missing_field("private_key"))?;

        Ok(Self {
            account_id,
            private_key,
        })
    }
}

/// User-level configuration container for `IsoAccountAlias`.
#[derive(Debug, ReadConfig, Clone, norito::JsonDeserialize)]
pub struct IsoAccountAlias {
    /// IBAN (or external identifier) mapped to a local account.
    pub iban: String,
    /// Corresponding Iroha account ID.
    pub account_id: String,
}

impl IsoAccountAlias {
    fn parse(self) -> actual::IsoAccountAlias {
        actual::IsoAccountAlias {
            iban: self.iban,
            account_id: self.account_id,
        }
    }
}

/// User-level configuration container for `IsoCurrencyAsset`.
#[derive(Debug, ReadConfig, Clone, norito::JsonDeserialize)]
pub struct IsoCurrencyAsset {
    /// ISO currency code handled by the bridge.
    pub currency: String,
    /// Asset definition tied to the currency on-chain.
    pub asset_definition: String,
}

impl IsoCurrencyAsset {
    fn parse(self) -> actual::IsoCurrencyAsset {
        actual::IsoCurrencyAsset {
            currency: self.currency,
            asset_definition: self.asset_definition,
        }
    }
}

#[cfg(test)]
mod offline_cfg_tests {
    use core::str::FromStr;

    use super::*;

    fn bundled_tables_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../codec/rans/tables/rans_seed0.toml")
            .canonicalize()
            .expect("deterministic bundled rANS tables must exist")
    }

    #[test]
    fn resolve_logger_filters() {
        let mut cfg = Logger {
            level: Level::INFO,
            filter: None,
            format: <_>::default(),
            terminal_colors: false,
        };
        assert_eq!(format!("{}", cfg.resolve_filter()), "info");

        cfg.filter = Some("iroha_core=debug".parse().unwrap());
        assert_eq!(format!("{}", cfg.resolve_filter()), "info,iroha_core=debug");

        cfg.level = Level::WARN;
        cfg.filter = Some("info".parse().unwrap());
        assert_eq!(format!("{}", cfg.resolve_filter()), "warn,info");
    }
    #[test]
    fn repo_parse_enforces_defaults() {
        let repo = Repo {
            default_haircut_bps: 12_500,
            margin_frequency_secs: 0,
            eligible_collateral: vec![iroha_data_model::asset::AssetDefinitionId::new(
                "wonderland".parse().unwrap(),
                "bond".parse().unwrap(),
            )],
            collateral_substitution_matrix: BTreeMap::new(),
        };

        let mut emitter = Emitter::new();
        let parsed = repo.parse(&mut emitter);

        assert_eq!(parsed.default_haircut_bps, 10_000);
        assert_eq!(
            parsed.margin_frequency_secs,
            defaults::settlement::repo::DEFAULT_MARGIN_FREQUENCY_SECS
        );
        assert_eq!(parsed.eligible_collateral.len(), 1);
        assert!(emitter.into_result().is_err());
    }

    #[test]
    fn offline_parse_maps_proof_mode() {
        let mut offline = Offline::default();
        offline.proof_mode = OfflineProofMode::Required;

        let mut emitter = Emitter::new();
        let parsed = offline.parse(&mut emitter);

        assert_eq!(parsed.proof_mode, actual::OfflineProofMode::Required);
        assert!(emitter.into_result().is_ok());
    }

    #[test]
    fn codec_parse_requires_cabac_build_flag() {
        if norito::streaming::CABAC_BUILD_AVAILABLE {
            // Nothing to assert when CABAC was compiled in for this test binary.
            return;
        }
        let codec = StreamingCodec {
            cabac_mode: WithOrigin::inline(CabacMode::Forced),
            trellis_blocks: WithOrigin::inline(Vec::new()),
            rans_tables_path: WithOrigin::inline(bundled_tables_path()),
            entropy_mode: WithOrigin::inline(defaults::streaming::codec::entropy_mode()),
            bundle_width: WithOrigin::inline(defaults::streaming::codec::bundle_width()),
            bundle_accel: WithOrigin::inline(defaults::streaming::codec::bundle_accel()),
        };
        let mut emitter = Emitter::new();
        assert!(codec.parse(&mut emitter).is_none());
        assert!(emitter.into_result().is_err());
    }

    #[test]
    fn codec_parse_rejects_missing_rans_tables() {
        let codec = StreamingCodec {
            cabac_mode: WithOrigin::inline(CabacMode::Disabled),
            trellis_blocks: WithOrigin::inline(Vec::new()),
            rans_tables_path: WithOrigin::inline(PathBuf::from(
                "codec/rans/tables/does_not_exist.toml",
            )),
            entropy_mode: WithOrigin::inline(defaults::streaming::codec::entropy_mode()),
            bundle_width: WithOrigin::inline(defaults::streaming::codec::bundle_width()),
            bundle_accel: WithOrigin::inline(defaults::streaming::codec::bundle_accel()),
        };
        let mut emitter = Emitter::new();
        assert!(codec.parse(&mut emitter).is_none());
        assert!(emitter.into_result().is_err());
    }

    #[test]
    fn codec_parse_rejects_unknown_entropy_mode() {
        let codec = StreamingCodec {
            cabac_mode: WithOrigin::inline(CabacMode::Disabled),
            trellis_blocks: WithOrigin::inline(Vec::new()),
            rans_tables_path: WithOrigin::inline(bundled_tables_path()),
            entropy_mode: WithOrigin::inline("invalid".to_string()),
            bundle_width: WithOrigin::inline(2),
            bundle_accel: WithOrigin::inline(defaults::streaming::codec::bundle_accel()),
        };
        let mut emitter = Emitter::new();
        assert!(codec.parse(&mut emitter).is_none());
        assert!(emitter.into_result().is_err());
    }

    #[test]
    fn codec_parse_enforces_bundled_build_support() {
        assert!(
            norito::streaming::BUNDLED_RANS_BUILD_AVAILABLE,
            "Bundled rANS must be compiled in for the first release; rebuild with ENABLE_RANS_BUNDLES=1"
        );
        let codec = StreamingCodec {
            cabac_mode: WithOrigin::inline(CabacMode::Disabled),
            trellis_blocks: WithOrigin::inline(Vec::new()),
            rans_tables_path: WithOrigin::inline(bundled_tables_path()),
            entropy_mode: WithOrigin::inline(EntropyMode::RansBundled.to_string()),
            bundle_width: WithOrigin::inline(2),
            bundle_accel: WithOrigin::inline(defaults::streaming::codec::bundle_accel()),
        };
        let mut emitter = Emitter::new();
        let parsed = codec
            .parse(&mut emitter)
            .expect("bundled builds should allow rans_bundled");
        assert_eq!(parsed.entropy_mode, EntropyMode::RansBundled);
        assert_eq!(parsed.bundle_width, 2);
        assert!(emitter.into_result().is_ok());
    }

    #[test]
    fn codec_parse_allows_bundle_accel_when_bundled_mode_is_available() {
        assert!(
            norito::streaming::BUNDLED_RANS_BUILD_AVAILABLE,
            "Bundled rANS must be compiled in for the first release; rebuild with ENABLE_RANS_BUNDLES=1"
        );
        let codec = StreamingCodec {
            cabac_mode: WithOrigin::inline(CabacMode::Disabled),
            trellis_blocks: WithOrigin::inline(Vec::new()),
            rans_tables_path: WithOrigin::inline(bundled_tables_path()),
            entropy_mode: WithOrigin::inline(EntropyMode::RansBundled.to_string()),
            bundle_width: WithOrigin::inline(2),
            bundle_accel: WithOrigin::inline("cpu_simd".to_string()),
        };
        let mut emitter = Emitter::new();
        let parsed = codec
            .parse(&mut emitter)
            .expect("bundled mode should parse");
        assert_eq!(parsed.entropy_mode, EntropyMode::RansBundled);
        assert_eq!(parsed.bundle_accel, actual::BundleAcceleration::CpuSimd);
        assert!(emitter.into_result().is_ok());
    }

    #[test]
    fn codec_parse_rejects_gpu_acceleration_on_cpu_only_builds() {
        if norito::streaming::BUNDLED_RANS_GPU_BUILD_AVAILABLE {
            // Acceptance is covered by runtime gating when GPU support is compiled in.
            return;
        }
        let codec = StreamingCodec {
            cabac_mode: WithOrigin::inline(CabacMode::Disabled),
            trellis_blocks: WithOrigin::inline(Vec::new()),
            rans_tables_path: WithOrigin::inline(bundled_tables_path()),
            entropy_mode: WithOrigin::inline(EntropyMode::RansBundled.to_string()),
            bundle_width: WithOrigin::inline(2),
            bundle_accel: WithOrigin::inline("gpu".to_string()),
        };
        let mut emitter = Emitter::new();
        assert!(
            codec.parse(&mut emitter).is_none(),
            "CPU-only builds must reject bundle_accel = \"gpu\" during parsing"
        );
        assert!(
            emitter.into_result().is_err(),
            "emitter should record the GPU gating failure"
        );
    }

    #[test]
    fn iso_bridge_json_deserializes() {
        let json = r#"{
            "enabled": true,
            "dedupe_ttl_secs": 120,
            "signer": {
                "account_id": "6cmzPVPX944pj7vVyADRpma2DCcBUsG1mhz8VrXArhXaGsjvRUcnbVn",
                "private_key": "802620282ED9F3CF92811C3818DBC4AE594ED59DC1A2F78E4241E31924E101D6B1FB83"
            },
            "account_aliases": [
                {"iban": "DE137017", "account_id": "6cmzPVPX4Vs6C1nbbQ7UD7Q6AWKJFC12abs4kZtXEE9SsFf6QRpp8rU"}
            ],
            "currency_assets": [
                {"currency": "USD", "asset_definition": "usd#fin"}
            ],
            "reference_data": {
                "refresh_interval_secs": 3600,
                "isin_crosswalk_path": null,
                "bic_lei_path": null,
                "mic_directory_path": null
            }
        }"#;

        let parsed: IsoBridge = norito::json::from_json(json).expect("valid iso bridge JSON");

        assert!(parsed.enabled);
        assert_eq!(parsed.dedupe_ttl_secs, 120);
        let signer = parsed.signer.expect("signer present");
        assert_eq!(
            signer.account_id,
            "6cmzPVPX944pj7vVyADRpma2DCcBUsG1mhz8VrXArhXaGsjvRUcnbVn"
        );
        assert_eq!(parsed.account_aliases[0].iban, "DE137017");
        assert_eq!(parsed.currency_assets[0].currency, "USD");
        assert_eq!(parsed.reference_data.refresh_interval_secs, 3600);
        assert!(parsed.reference_data.isin_crosswalk_path.is_none());
    }

    #[test]
    fn governance_parliament_fields_roundtrip() {
        let cfg = Governance {
            vk_ballot: None,
            vk_tally: None,
            voting_asset_id: defaults::governance::voting_asset_id(),
            citizenship_asset_id: defaults::governance::citizenship_asset_id(),
            citizenship_bond_amount: 99,
            citizenship_escrow_account: defaults::governance::citizenship_escrow_account(),
            min_bond_amount: 42,
            bond_escrow_account: defaults::governance::bond_escrow_account(),
            sorafs_pin_policy: SorafsPinPolicyConstraints::default(),
            sorafs_penalty: SorafsPenaltyPolicy::default(),
            conviction_step_blocks: 42,
            max_conviction: 7,
            min_enactment_delay: 10,
            window_span: 99,
            plain_voting_enabled: true,
            approval_threshold_q_num: 2,
            approval_threshold_q_den: 3,
            min_turnout: 123,
            parliament_committee_size: 11,
            parliament_term_blocks: 12_345,
            parliament_min_stake: 456,
            parliament_eligibility_asset_id: defaults::governance::parliament_eligibility_asset_id(
            ),
            parliament_alternate_size: Some(13),
            ..Governance::default()
        };

        let parsed = cfg.parse();

        assert_eq!(parsed.conviction_step_blocks, 42);
        assert_eq!(parsed.max_conviction, 7);
        assert_eq!(parsed.min_enactment_delay, 10);
        assert_eq!(parsed.window_span, 99);
        assert!(parsed.plain_voting_enabled);
        assert_eq!(
            parsed.voting_asset_id,
            iroha_data_model::asset::prelude::AssetDefinitionId::new(
                "sora".parse().unwrap(),
                "xor".parse().unwrap()
            )
        );
        assert_eq!(
            parsed.citizenship_asset_id,
            iroha_data_model::asset::prelude::AssetDefinitionId::new(
                "sora".parse().unwrap(),
                "xor".parse().unwrap()
            )
        );
        assert_eq!(parsed.citizenship_bond_amount, 99);
        assert_eq!(
            parsed.citizenship_escrow_account,
            iroha_data_model::account::AccountId::parse_encoded(
                &defaults::governance::citizenship_escrow_account()
            )
            .map(iroha_data_model::account::ParsedAccountId::into_account_id)
            .unwrap()
        );
        assert_eq!(parsed.min_bond_amount, 42);
        assert_eq!(
            parsed.bond_escrow_account,
            iroha_data_model::account::AccountId::parse_encoded(
                &defaults::governance::bond_escrow_account()
            )
            .map(iroha_data_model::account::ParsedAccountId::into_account_id)
            .unwrap()
        );
        assert_eq!(parsed.approval_threshold_q_num, 2);
        assert_eq!(parsed.approval_threshold_q_den, 3);
        assert_eq!(parsed.min_turnout, 123);
        assert_eq!(parsed.parliament_committee_size, 11);
        assert_eq!(parsed.parliament_term_blocks, 12_345);
        assert_eq!(parsed.parliament_min_stake, 456);
        assert_eq!(
            parsed.parliament_eligibility_asset_id,
            iroha_data_model::asset::prelude::AssetDefinitionId::new(
                "stake".parse().unwrap(),
                "SORA".parse().unwrap()
            )
        );
        assert_eq!(parsed.parliament_alternate_size, Some(13));
    }

    #[test]
    fn halo2_defaults_parse() {
        let backend = ZkHalo2Backend::from_str(defaults::zk::halo2::BACKEND)
            .expect("default backend string should parse");
        assert_eq!(backend, ZkHalo2Backend::Ipa);

        let curve = ZkCurve::from_str(defaults::zk::halo2::CURVE)
            .expect("default curve string should parse");
        assert_eq!(curve, ZkCurve::Pallas);
    }

    #[test]
    fn telemetry_profile_round_trips_via_strings() {
        for variant in [
            TelemetryProfile::Disabled,
            TelemetryProfile::Operator,
            TelemetryProfile::Extended,
            TelemetryProfile::Developer,
            TelemetryProfile::Full,
        ] {
            let text = variant.to_string();
            let parsed = TelemetryProfile::from_str(&text).expect("profile string should parse");
            assert_eq!(parsed, variant);
        }
    }
}

#[cfg(test)]
mod duration_clamp_tests {
    use std::{path::PathBuf, time::Duration as StdDuration};

    use iroha_config_base::toml::TomlSource;
    use toml::{Table, Value};

    use crate::parameters::{actual, defaults};

    const MINIMAL_CONFIG: &str = r#"
chain = "00000000-0000-0000-0000-000000000000"
public_key = "ea01309060D021340617E9554CCBC2CF3CC3DB922A9BA323ABDF7C271FCC6EF69BE7A8DEBCA7D9E96C0F0089ABA22CDAADE4A2"
private_key = "8926201CA347641228C3B79AA43839DEDC85FA51C0E8B9B6A00F6B0D6B0423E902973F"
trusted_peers_pop = [
  { public_key = "ea01309060D021340617E9554CCBC2CF3CC3DB922A9BA323ABDF7C271FCC6EF69BE7A8DEBCA7D9E96C0F0089ABA22CDAADE4A2", pop_hex = "8515da750f81182aaba5c22fc9f03a01e81ed85e4495a2ca6b29a71c0c8549537e31e79cddf6ff285b9e22d0d9dc17ce0f46e7d0cf78b2ef9feab50c849a1ea8e1e4f07e966f6113faa8a999317545d9f111b8e08a7273913710b43a20b19c08" }
]

[network]
address = "addr:127.0.0.1:1337#8F78"
public_address = "addr:127.0.0.1:1337#8F78"

[torii]
address = "addr:127.0.0.1:8080#8942"

[genesis]
public_key = "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03"

[streaming]
identity_public_key = "ed01208BA62848CF767D72E7F7F4B9D2D7BA07FEE33760F79ABE5597A51520E292A0CB"
identity_private_key = "8026208F4C15E5D664DA3F13778801D23D4E89B76E94C1B94B389544168B6CB894F84F"
"#;

    fn base_table() -> Table {
        toml::from_str(MINIMAL_CONFIG).expect("parse minimal config")
    }

    fn load_root(table: Table) -> actual::Root {
        actual::Root::from_toml_source(TomlSource::inline(table)).expect("load minimal config")
    }

    #[test]
    fn network_parse_clamps_zero_periods() {
        let mut table = base_table();
        let network = table
            .get_mut("network")
            .and_then(Value::as_table_mut)
            .expect("network table");
        network.insert("block_gossip_period_ms".into(), Value::Integer(0));
        network.insert("block_gossip_max_period_ms".into(), Value::Integer(0));
        network.insert("peer_gossip_period_ms".into(), Value::Integer(0));
        network.insert("peer_gossip_max_period_ms".into(), Value::Integer(0));
        network.insert("transaction_gossip_period_ms".into(), Value::Integer(0));
        network.insert(
            "transaction_gossip_public_target_reshuffle_ms".into(),
            Value::Integer(0),
        );
        network.insert(
            "transaction_gossip_restricted_target_reshuffle_ms".into(),
            Value::Integer(0),
        );
        network.insert("idle_timeout_ms".into(), Value::Integer(0));
        let actual = load_root(table);
        let min = StdDuration::from_millis(100);
        assert_eq!(actual.block_sync.gossip_period, min);
        assert_eq!(actual.block_sync.gossip_max_period, min);
        assert_eq!(actual.transaction_gossiper.gossip_period, min);
        assert_eq!(
            actual
                .transaction_gossiper
                .dataspace
                .public_target_reshuffle,
            min
        );
        assert_eq!(
            actual
                .transaction_gossiper
                .dataspace
                .restricted_target_reshuffle,
            min
        );
        assert_eq!(actual.network.peer_gossip_period, min);
        assert_eq!(actual.network.peer_gossip_max_period, min);
        assert_eq!(actual.network.idle_timeout, min);
    }

    #[test]
    fn network_defaults_apply_transaction_gossip_target_caps() {
        let actual = load_root(base_table());
        assert_eq!(
            actual.transaction_gossiper.dataspace.public_target_cap,
            defaults::network::TX_GOSSIP_PUBLIC_TARGET_CAP
        );
        assert_eq!(
            actual.transaction_gossiper.dataspace.restricted_target_cap,
            defaults::network::TX_GOSSIP_RESTRICTED_TARGET_CAP
        );
    }

    #[test]
    fn network_defaults_apply_transaction_gossip_resend_ticks() {
        let actual = load_root(base_table());
        assert_eq!(
            actual.transaction_gossiper.gossip_resend_ticks,
            defaults::network::TRANSACTION_GOSSIP_RESEND_TICKS
        );
    }

    #[test]
    fn storage_budget_applies_after_parse() {
        let mut table = base_table();
        let nexus = table
            .entry("nexus")
            .or_insert_with(|| Value::Table(Table::new()))
            .as_table_mut()
            .expect("nexus table");
        let mut storage = Table::new();
        storage.insert("max_disk_usage_bytes".into(), Value::Integer(1_000));
        storage.insert("max_wsv_memory_bytes".into(), Value::Integer(256));
        let mut weights = Table::new();
        weights.insert("kura_blocks_bps".into(), Value::Integer(10_000));
        weights.insert("wsv_snapshots_bps".into(), Value::Integer(0));
        weights.insert("sorafs_bps".into(), Value::Integer(0));
        weights.insert("soranet_spool_bps".into(), Value::Integer(0));
        weights.insert("soravpn_spool_bps".into(), Value::Integer(0));
        storage.insert("disk_budget_weights".into(), Value::Table(weights));
        nexus.insert("storage".into(), Value::Table(storage));

        let actual = load_root(table);
        assert_eq!(actual.kura.max_disk_usage_bytes.get(), 1_000);
        assert!(actual.tiered_state.enabled);
        assert_eq!(actual.tiered_state.hot_retained_bytes.get(), 256);
    }

    #[test]
    fn soracloud_runtime_defaults_apply() {
        let actual = load_root(base_table());
        assert_eq!(
            actual.soracloud_runtime.state_dir,
            defaults::soracloud_runtime::state_dir()
        );
        assert_eq!(
            actual.soracloud_runtime.reconcile_interval,
            StdDuration::from_millis(defaults::soracloud_runtime::RECONCILE_INTERVAL_MS)
        );
        assert_eq!(
            actual.soracloud_runtime.hydration_concurrency,
            defaults::soracloud_runtime::HYDRATION_CONCURRENCY
        );
        assert_eq!(
            actual.soracloud_runtime.cache_budgets.bundle_bytes,
            defaults::soracloud_runtime::BUNDLE_CACHE_BUDGET_BYTES
        );
        assert_eq!(
            actual
                .soracloud_runtime
                .native_process
                .max_concurrent_processes,
            defaults::soracloud_runtime::NATIVE_PROCESS_MAX_CONCURRENT_PROCESSES
        );
        assert_eq!(
            actual.soracloud_runtime.egress.default_allow,
            defaults::soracloud_runtime::EGRESS_DEFAULT_ALLOW
        );
        assert_eq!(
            actual.soracloud_runtime.hf.hub_base_url,
            defaults::soracloud_runtime::hf::HUB_BASE_URL
        );
        assert_eq!(
            actual.soracloud_runtime.hf.local_execution_enabled,
            defaults::soracloud_runtime::hf::LOCAL_EXECUTION_ENABLED
        );
        assert_eq!(
            actual.soracloud_runtime.hf.local_runner_program,
            defaults::soracloud_runtime::hf::LOCAL_RUNNER_PROGRAM
        );
        assert_eq!(
            actual.soracloud_runtime.hf.model_host_heartbeat_ttl,
            StdDuration::from_millis(defaults::soracloud_runtime::hf::MODEL_HOST_HEARTBEAT_TTL_MS)
        );
        assert_eq!(
            actual.soracloud_runtime.hf.import_max_files,
            defaults::soracloud_runtime::hf::IMPORT_MAX_FILES
        );
        assert!(actual.soracloud_runtime.hf.inference_token.is_none());
    }

    #[test]
    fn soracloud_runtime_parse_applies_explicit_overrides() {
        let mut table = base_table();
        let runtime = table
            .entry("soracloud_runtime")
            .or_insert_with(|| Value::Table(Table::new()))
            .as_table_mut()
            .expect("soracloud_runtime table");
        runtime.insert(
            "state_dir".into(),
            Value::String("./runtime/custom".to_string()),
        );
        runtime.insert("reconcile_interval_ms".into(), Value::Integer(2_500));
        runtime.insert("hydration_concurrency".into(), Value::Integer(7));

        let mut cache_budgets = Table::new();
        cache_budgets.insert("bundle_bytes".into(), Value::Integer(1_024));
        cache_budgets.insert("static_asset_bytes".into(), Value::Integer(2_048));
        cache_budgets.insert("journal_bytes".into(), Value::Integer(3_072));
        cache_budgets.insert("checkpoint_bytes".into(), Value::Integer(4_096));
        cache_budgets.insert("model_artifact_bytes".into(), Value::Integer(5_120));
        cache_budgets.insert("model_weight_bytes".into(), Value::Integer(6_144));
        runtime.insert("cache_budgets".into(), Value::Table(cache_budgets));

        let mut native_process = Table::new();
        native_process.insert("max_concurrent_processes".into(), Value::Integer(3));
        native_process.insert("cpu_millis".into(), Value::Integer(1_500));
        native_process.insert("memory_bytes".into(), Value::Integer(65_536));
        native_process.insert("ephemeral_storage_bytes".into(), Value::Integer(131_072));
        native_process.insert("max_open_files".into(), Value::Integer(64));
        native_process.insert("max_tasks".into(), Value::Integer(32));
        native_process.insert("start_grace_ms".into(), Value::Integer(1_500));
        native_process.insert("stop_grace_ms".into(), Value::Integer(2_500));
        runtime.insert("native_process".into(), Value::Table(native_process));

        let mut egress = Table::new();
        egress.insert("default_allow".into(), Value::Boolean(true));
        egress.insert(
            "allowed_hosts".into(),
            Value::Array(vec![
                Value::String("cdn.sora.test".to_string()),
                Value::String(" api.sora.test ".to_string()),
                Value::String("cdn.sora.test".to_string()),
            ]),
        );
        egress.insert("rate_per_minute".into(), Value::Integer(120));
        egress.insert("max_bytes_per_minute".into(), Value::Integer(262_144));
        runtime.insert("egress".into(), Value::Table(egress));

        let mut hf = Table::new();
        hf.insert(
            "hub_base_url".into(),
            Value::String(" https://mirror.hf.test/ ".to_string()),
        );
        hf.insert(
            "api_base_url".into(),
            Value::String("https://mirror.hf.test/api/".to_string()),
        );
        hf.insert(
            "inference_base_url".into(),
            Value::String("https://router.hf.test/hf-inference/models/".to_string()),
        );
        hf.insert("request_timeout_ms".into(), Value::Integer(21_000));
        hf.insert("local_execution_enabled".into(), Value::Boolean(false));
        hf.insert(
            "local_runner_program".into(),
            Value::String(" python3.12 ".to_string()),
        );
        hf.insert("local_runner_timeout_ms".into(), Value::Integer(45_000));
        hf.insert("model_host_heartbeat_ttl_ms".into(), Value::Integer(18_000));
        hf.insert(
            "allow_inference_bridge_fallback".into(),
            Value::Boolean(false),
        );
        hf.insert("import_max_files".into(), Value::Integer(48));
        hf.insert("import_max_file_bytes".into(), Value::Integer(777_777));
        hf.insert("import_max_total_bytes".into(), Value::Integer(9_999_999));
        hf.insert(
            "import_file_allowlist".into(),
            Value::Array(vec![
                Value::String(" config.json ".to_string()),
                Value::String("*.safetensors".to_string()),
                Value::String("CONFIG.JSON".to_string()),
            ]),
        );
        hf.insert(
            "inference_token".into(),
            Value::String("  secret-token  ".to_string()),
        );
        runtime.insert("hf".into(), Value::Table(hf));

        let actual = load_root(table);
        assert!(
            actual
                .soracloud_runtime
                .state_dir
                .to_string_lossy()
                .ends_with("runtime/custom"),
            "resolved path should retain configured suffix: {}",
            actual.soracloud_runtime.state_dir.display()
        );
        assert_eq!(
            actual.soracloud_runtime.reconcile_interval,
            StdDuration::from_millis(2_500)
        );
        assert_eq!(actual.soracloud_runtime.hydration_concurrency.get(), 7);
        assert_eq!(
            actual.soracloud_runtime.cache_budgets.bundle_bytes.get(),
            1_024
        );
        assert_eq!(
            actual
                .soracloud_runtime
                .cache_budgets
                .model_weight_bytes
                .get(),
            6_144
        );
        assert_eq!(
            actual
                .soracloud_runtime
                .native_process
                .max_concurrent_processes
                .get(),
            3
        );
        assert_eq!(
            actual.soracloud_runtime.native_process.start_grace,
            StdDuration::from_millis(1_500)
        );
        assert!(actual.soracloud_runtime.egress.default_allow);
        assert_eq!(
            actual.soracloud_runtime.egress.allowed_hosts,
            vec!["api.sora.test".to_string(), "cdn.sora.test".to_string()]
        );
        assert_eq!(
            actual
                .soracloud_runtime
                .egress
                .rate_per_minute
                .expect("rate cap")
                .get(),
            120
        );
        assert_eq!(
            actual
                .soracloud_runtime
                .egress
                .max_bytes_per_minute
                .expect("byte cap")
                .get(),
            262_144
        );
        assert_eq!(
            actual.soracloud_runtime.hf.hub_base_url,
            "https://mirror.hf.test"
        );
        assert_eq!(
            actual.soracloud_runtime.hf.api_base_url,
            "https://mirror.hf.test/api"
        );
        assert_eq!(
            actual.soracloud_runtime.hf.inference_base_url,
            "https://router.hf.test/hf-inference/models"
        );
        assert_eq!(
            actual.soracloud_runtime.hf.request_timeout,
            StdDuration::from_millis(21_000)
        );
        assert!(!actual.soracloud_runtime.hf.local_execution_enabled);
        assert_eq!(
            actual.soracloud_runtime.hf.local_runner_program,
            "python3.12"
        );
        assert_eq!(
            actual.soracloud_runtime.hf.local_runner_timeout,
            StdDuration::from_millis(45_000)
        );
        assert_eq!(
            actual.soracloud_runtime.hf.model_host_heartbeat_ttl,
            StdDuration::from_millis(18_000)
        );
        assert!(!actual.soracloud_runtime.hf.allow_inference_bridge_fallback);
        assert_eq!(actual.soracloud_runtime.hf.import_max_files, 48);
        assert_eq!(actual.soracloud_runtime.hf.import_max_file_bytes, 777_777);
        assert_eq!(
            actual.soracloud_runtime.hf.import_max_total_bytes,
            9_999_999
        );
        assert_eq!(
            actual.soracloud_runtime.hf.import_file_allowlist,
            vec!["*.safetensors".to_string(), "config.json".to_string()]
        );
        assert_eq!(
            actual.soracloud_runtime.hf.inference_token.as_deref(),
            Some("secret-token")
        );
    }

    #[test]
    fn nexus_hf_shared_leases_defaults_apply() {
        let actual = load_root(base_table());
        assert_eq!(
            actual.nexus.hf_shared_leases.drain_grace,
            StdDuration::from_millis(defaults::nexus::hf_shared_leases::DRAIN_GRACE_MS)
        );
        assert_eq!(
            actual.nexus.hf_shared_leases.warmup_no_show_slash_bps,
            defaults::nexus::hf_shared_leases::WARMUP_NO_SHOW_SLASH_BPS
        );
        assert_eq!(
            actual
                .nexus
                .hf_shared_leases
                .assigned_heartbeat_miss_slash_bps,
            defaults::nexus::hf_shared_leases::ASSIGNED_HEARTBEAT_MISS_SLASH_BPS
        );
        assert_eq!(
            actual
                .nexus
                .hf_shared_leases
                .assigned_heartbeat_miss_strike_threshold,
            defaults::nexus::hf_shared_leases::ASSIGNED_HEARTBEAT_MISS_STRIKE_THRESHOLD
        );
        assert_eq!(
            actual.nexus.hf_shared_leases.advert_contradiction_slash_bps,
            defaults::nexus::hf_shared_leases::ADVERT_CONTRADICTION_SLASH_BPS
        );
    }

    #[test]
    fn nexus_hf_shared_leases_parse_applies_explicit_overrides() {
        let mut table = base_table();
        let nexus = table
            .entry("nexus")
            .or_insert_with(|| Value::Table(Table::new()))
            .as_table_mut()
            .expect("nexus table");
        let mut hf_shared_leases = Table::new();
        hf_shared_leases.insert("drain_grace_ms".into(), Value::Integer(12_345));
        hf_shared_leases.insert("warmup_no_show_slash_bps".into(), Value::Integer(777));
        hf_shared_leases.insert(
            "assigned_heartbeat_miss_slash_bps".into(),
            Value::Integer(222),
        );
        hf_shared_leases.insert(
            "assigned_heartbeat_miss_strike_threshold".into(),
            Value::Integer(4),
        );
        hf_shared_leases.insert("advert_contradiction_slash_bps".into(), Value::Integer(999));
        nexus.insert("hf_shared_leases".into(), Value::Table(hf_shared_leases));

        let actual = load_root(table);
        assert_eq!(
            actual.nexus.hf_shared_leases.drain_grace,
            StdDuration::from_millis(12_345)
        );
        assert_eq!(actual.nexus.hf_shared_leases.warmup_no_show_slash_bps, 777);
        assert_eq!(
            actual
                .nexus
                .hf_shared_leases
                .assigned_heartbeat_miss_slash_bps,
            222
        );
        assert_eq!(
            actual
                .nexus
                .hf_shared_leases
                .assigned_heartbeat_miss_strike_threshold,
            4
        );
        assert_eq!(
            actual.nexus.hf_shared_leases.advert_contradiction_slash_bps,
            999
        );
    }

    #[test]
    fn nexus_uploaded_models_defaults_apply() {
        let actual = load_root(base_table());
        assert_eq!(
            actual.nexus.uploaded_models.chunk_plaintext_bytes,
            defaults::nexus::uploaded_models::CHUNK_PLAINTEXT_BYTES
        );
        assert_eq!(
            actual.nexus.uploaded_models.max_session_token_budget,
            defaults::nexus::uploaded_models::MAX_SESSION_TOKEN_BUDGET
        );
    }

    #[test]
    fn nexus_uploaded_models_parse_applies_explicit_overrides() {
        let mut table = base_table();
        let nexus = table
            .entry("nexus")
            .or_insert_with(|| Value::Table(Table::new()))
            .as_table_mut()
            .expect("nexus table");
        let mut uploaded_models = Table::new();
        uploaded_models.insert("chunk_plaintext_bytes".into(), Value::Integer(2_097_152));
        uploaded_models.insert(
            "max_active_private_sessions_per_apartment".into(),
            Value::Integer(6),
        );
        uploaded_models.insert("max_session_token_budget".into(), Value::Integer(4_096));
        nexus.insert("uploaded_models".into(), Value::Table(uploaded_models));

        let actual = load_root(table);
        assert_eq!(
            actual.nexus.uploaded_models.chunk_plaintext_bytes,
            2_097_152
        );
        assert_eq!(
            actual
                .nexus
                .uploaded_models
                .max_active_private_sessions_per_apartment,
            6
        );
        assert_eq!(actual.nexus.uploaded_models.max_session_token_budget, 4_096);
    }

    #[test]
    fn tiered_state_parse_accepts_da_store_root() {
        let mut table = base_table();
        let tiered = table
            .entry("tiered_state")
            .or_insert_with(|| Value::Table(Table::new()))
            .as_table_mut()
            .expect("tiered_state table");
        tiered.insert(
            "da_store_root".into(),
            Value::String("./storage/da_wsv_custom".to_string()),
        );

        let actual = load_root(table);
        assert_eq!(
            actual
                .tiered_state
                .da_store_root
                .as_ref()
                .expect("da_store_root must parse"),
            &PathBuf::from("./storage/da_wsv_custom")
        );
    }

    #[test]
    fn sumeragi_rejects_zero_membership_mismatch_threshold() {
        let mut table = base_table();
        let sumeragi = table
            .entry("sumeragi")
            .or_insert_with(|| Value::Table(Table::new()))
            .as_table_mut()
            .expect("sumeragi table");
        let gating = sumeragi
            .entry("gating")
            .or_insert_with(|| Value::Table(Table::new()))
            .as_table_mut()
            .expect("sumeragi.gating table");
        gating.insert(
            "membership_mismatch_alert_threshold".into(),
            Value::Integer(0),
        );
        assert!(actual::Root::from_toml_source(TomlSource::inline(table)).is_err());
    }

    #[test]
    fn sumeragi_rejects_zero_worker_vote_burst_cap_with_payload_backlog() {
        let mut table = base_table();
        let sumeragi = table
            .entry("sumeragi")
            .or_insert_with(|| Value::Table(Table::new()))
            .as_table_mut()
            .expect("sumeragi table");
        let advanced = sumeragi
            .entry("advanced")
            .or_insert_with(|| Value::Table(Table::new()))
            .as_table_mut()
            .expect("sumeragi.advanced table");
        let worker = advanced
            .entry("worker")
            .or_insert_with(|| Value::Table(Table::new()))
            .as_table_mut()
            .expect("sumeragi.advanced.worker table");
        worker.insert(
            "vote_burst_cap_with_payload_backlog".into(),
            Value::Integer(0),
        );
        assert!(actual::Root::from_toml_source(TomlSource::inline(table)).is_err());
    }

    #[test]
    fn sumeragi_rejects_zero_worker_validation_queue_full_inline_cutover_divisor() {
        let mut table = base_table();
        let sumeragi = table
            .entry("sumeragi")
            .or_insert_with(|| Value::Table(Table::new()))
            .as_table_mut()
            .expect("sumeragi table");
        let advanced = sumeragi
            .entry("advanced")
            .or_insert_with(|| Value::Table(Table::new()))
            .as_table_mut()
            .expect("sumeragi.advanced table");
        let worker = advanced
            .entry("worker")
            .or_insert_with(|| Value::Table(Table::new()))
            .as_table_mut()
            .expect("sumeragi.advanced.worker table");
        worker.insert(
            "validation_queue_full_inline_cutover_divisor".into(),
            Value::Integer(0),
        );
        assert!(actual::Root::from_toml_source(TomlSource::inline(table)).is_err());
    }

    #[test]
    fn sumeragi_rejects_zero_worker_max_urgent_before_da_critical() {
        let mut table = base_table();
        let sumeragi = table
            .entry("sumeragi")
            .or_insert_with(|| Value::Table(Table::new()))
            .as_table_mut()
            .expect("sumeragi table");
        let advanced = sumeragi
            .entry("advanced")
            .or_insert_with(|| Value::Table(Table::new()))
            .as_table_mut()
            .expect("sumeragi.advanced table");
        let worker = advanced
            .entry("worker")
            .or_insert_with(|| Value::Table(Table::new()))
            .as_table_mut()
            .expect("sumeragi.advanced.worker table");
        worker.insert("max_urgent_before_da_critical".into(), Value::Integer(0));
        assert!(actual::Root::from_toml_source(TomlSource::inline(table)).is_err());
    }

    #[test]
    fn root_rejects_non_bls_consensus_keys() {
        let mut table = base_table();
        table.insert(
            "public_key".into(),
            Value::String(
                "ed01208BA62848CF767D72E7F7F4B9D2D7BA07FEE33760F79ABE5597A51520E292A0CB"
                    .to_string(),
            ),
        );
        table.insert(
            "private_key".into(),
            Value::String(
                "8026208F4C15E5D664DA3F13778801D23D4E89B76E94C1B94B389544168B6CB894F84F"
                    .to_string(),
            ),
        );
        assert!(actual::Root::from_toml_source(TomlSource::inline(table)).is_err());
    }

    #[test]
    fn sumeragi_requires_bls_allowed_algorithms() {
        let mut table = base_table();
        let sumeragi = table
            .entry("sumeragi")
            .or_insert_with(|| Value::Table(Table::new()))
            .as_table_mut()
            .expect("sumeragi table");
        sumeragi.insert(
            "key_allowed_algorithms".into(),
            Value::Array(vec![Value::String("ed25519".to_string())]),
        );
        assert!(actual::Root::from_toml_source(TomlSource::inline(table)).is_err());
    }

    #[test]
    fn kura_roster_retention_must_cover_evidence_horizon() {
        let mut table = base_table();
        let kura = table
            .entry("kura")
            .or_insert_with(|| Value::Table(Table::new()))
            .as_table_mut()
            .expect("kura table");
        kura.insert("block_sync_roster_retention".into(), Value::Integer(10));

        let sumeragi = table
            .entry("sumeragi")
            .or_insert_with(|| Value::Table(Table::new()))
            .as_table_mut()
            .expect("sumeragi table");
        let npos = sumeragi
            .entry("npos")
            .or_insert_with(|| Value::Table(Table::new()))
            .as_table_mut()
            .expect("npos table");
        let reconfig = npos
            .entry("reconfig")
            .or_insert_with(|| Value::Table(Table::new()))
            .as_table_mut()
            .expect("reconfig table");
        reconfig.insert("evidence_horizon_blocks".into(), Value::Integer(20));

        assert!(actual::Root::from_toml_source(TomlSource::inline(table)).is_err());
    }

    #[test]
    fn nts_parse_clamps_zero_sample_interval() {
        let mut table = base_table();
        let mut nts = Table::new();
        nts.insert("sample_interval_ms".into(), Value::Integer(0));
        table.insert("nts".into(), Value::Table(nts));
        let actual = load_root(table);
        assert_eq!(actual.nts.sample_interval, StdDuration::from_millis(100));
    }

    #[test]
    fn telemetry_clamps_zero_telegram_metrics_period() {
        let mut table = base_table();
        let mut telemetry = Table::new();
        telemetry.insert("name".into(), Value::String("ops".to_string()));
        telemetry.insert(
            "url".into(),
            Value::String("http://localhost:8180".to_string()),
        );
        telemetry.insert(
            "telegram_metrics_url".into(),
            Value::String("http://localhost:8180/metrics".to_string()),
        );
        telemetry.insert("telegram_metrics_period_ms".into(), Value::Integer(0));
        table.insert("telemetry".into(), Value::Table(telemetry));
        let actual = load_root(table);
        let telemetry = actual.telemetry.expect("telemetry configured");
        assert_eq!(
            telemetry.telegram_metrics_period,
            Some(StdDuration::from_millis(100))
        );
    }
}

#[cfg(test)]
mod settlement_router_tests {
    use std::time::Duration as StdDuration;

    use super::*;

    #[test]
    fn router_parse_clamps_invalid_values() {
        let router = Router {
            twap_window_seconds: 0,
            epsilon_bps: 20_000,
            buffer_alert_pct: 10,
            buffer_throttle_pct: 50,
            buffer_xor_only_pct: 40,
            buffer_halt_pct: 35,
            buffer_horizon_hours: 0,
        };
        let mut emitter = Emitter::new();
        let actual = router.parse(&mut emitter);
        assert_eq!(
            actual.twap_window,
            StdDuration::from_secs(defaults::settlement::router::TWAP_WINDOW_SECS)
        );
        assert_eq!(
            actual.epsilon_bps,
            defaults::settlement::router::EPSILON_BPS
        );
        assert_eq!(
            actual.buffer_alert_pct,
            defaults::settlement::router::ALERT_PCT
        );
        assert_eq!(
            actual.buffer_throttle_pct,
            defaults::settlement::router::THROTTLE_PCT
        );
        assert_eq!(
            actual.buffer_xor_only_pct,
            defaults::settlement::router::XOR_ONLY_PCT
        );
        assert_eq!(
            actual.buffer_halt_pct,
            defaults::settlement::router::HALT_PCT
        );
        assert_eq!(
            actual.buffer_horizon_hours,
            defaults::settlement::router::BUFFER_HORIZON_HOURS
        );
        assert!(emitter.into_result().is_err());
    }
}

#[cfg(test)]
mod adaptive_observability_tests {
    use super::*;

    #[test]
    fn adaptive_observability_rejects_zero_thresholds() {
        let user = AdaptiveObservability {
            enabled: true,
            qc_latency_alert_ms: 0,
            da_reschedule_burst: 0,
            pacemaker_extra_ms: 10,
            collector_redundant_r: 0,
            cooldown_ms: 0,
        };
        let mut emitter = Emitter::new();
        assert!(user.parse(&mut emitter).is_none());
        assert!(emitter.into_result().is_err());
    }
}

#[cfg(test)]
mod pacing_governor_tests {
    use super::*;

    #[test]
    fn pacing_governor_rejects_invalid_bounds() {
        let user = SumeragiPacingGovernor {
            window_blocks: 1,
            view_change_pressure_permille: 0,
            view_change_clear_permille: 10,
            commit_spacing_pressure_permille: 0,
            commit_spacing_clear_permille: 500,
            step_up_bps: 0,
            step_down_bps: 0,
            min_factor_bps: 9_000,
            max_factor_bps: 8_000,
        };
        let mut emitter = Emitter::new();
        assert!(user.parse(&mut emitter).is_none());
        assert!(emitter.into_result().is_err());
    }
}
