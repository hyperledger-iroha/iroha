//! Configuration handling for the SoraNet relay daemon.

use std::{
    fmt, fs,
    net::SocketAddr,
    num::NonZeroU32,
    path::{Path, PathBuf},
    str::FromStr,
    sync::{Arc, Mutex},
    time::{Duration, SystemTime},
};

use ed25519_dalek::VerifyingKey;
use hex::FromHexError;
use iroha_crypto::soranet::{
    certificate::{CertificateValidationPhase, RelayCertificateBundleV2},
    pow, puzzle,
    token::{
        AdmissionTokenVerifier, InMemoryTokenStore, PersistentTokenStore, TokenStore,
        TokenStoreLimits,
    },
};
use iroha_data_model::soranet::{
    privacy_metrics::SoranetPrivacyModeV1,
    vpn::{VPN_CELL_LEN, VpnExitClassV1, VpnRouteV1},
};
use norito::{
    derive::{JsonDeserialize, JsonSerialize},
    json,
};
use soranet_pq::MlDsaSuite;
use thiserror::Error;

use crate::{
    capability::{self, ConstantRateMode, GreaseEntry},
    constant_rate::{CONSTANT_RATE_CELL_BYTES, ConstantRateProfileName},
    incentive_log::IncentiveLogConfig,
    token_tool::read_revocation_file,
};

const DEFAULT_SELF_SIGNED_SUBJECT: &str = "soranet-relay.local";
const ML_KEM_768_PUBLIC_LEN: usize = 1_184;
const ML_KEM_768_SECRET_LEN: usize = 2_400;

const DEFAULT_PRIVACY_BUCKET_SECS: u64 = 60;
const DEFAULT_PRIVACY_MIN_HANDSHAKES: u64 = 12;
const DEFAULT_PRIVACY_FLUSH_DELAY_BUCKETS: u64 = 1;
const DEFAULT_PRIVACY_FORCE_FLUSH_BUCKETS: u64 = 6;
const DEFAULT_PRIVACY_MAX_COMPLETED_BUCKETS: usize = 60;
const DEFAULT_PRIVACY_EXPECTED_SHARES: u16 = 2;
const DEFAULT_PRIVACY_EVENT_BUFFER_CAPACITY: usize = 4_096;
const DEFAULT_VPN_CELL_SIZE_BYTES: u16 = 1_024;
const DEFAULT_VPN_PACING_MILLIS: u64 = 25;
const DEFAULT_VPN_PADDING_BUDGET_MS: u16 = 15;
const DEFAULT_VPN_COVER_TO_DATA_PER_MILLE: u16 = 250;
const DEFAULT_VPN_HEARTBEAT_MILLIS: u16 = 500;
const DEFAULT_VPN_COVER_BURST_CELLS: u16 = 3;
const DEFAULT_VPN_COVER_JITTER_MILLIS: u16 = 10;
const DEFAULT_VPN_EXIT_CLASS: &str = "standard";
const DEFAULT_VPN_LEASE_SECS: u32 = 10 * 60;
const DEFAULT_VPN_DNS_PUSH_INTERVAL_SECS: u32 = 90;
const DEFAULT_VPN_METER_HASH_HEX: &str =
    "0000000000000000000000000000000000000000000000000000000000000000";

/// Operating mode for the relay.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelayMode {
    Entry,
    Middle,
    Exit,
}

impl RelayMode {
    /// Returns the lowercase label used in logs/metrics.
    pub fn as_label(self) -> &'static str {
        match self {
            RelayMode::Entry => "entry",
            RelayMode::Middle => "middle",
            RelayMode::Exit => "exit",
        }
    }
}

impl fmt::Display for RelayMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_label())
    }
}

impl From<RelayMode> for SoranetPrivacyModeV1 {
    fn from(mode: RelayMode) -> Self {
        match mode {
            RelayMode::Entry => SoranetPrivacyModeV1::Entry,
            RelayMode::Middle => SoranetPrivacyModeV1::Middle,
            RelayMode::Exit => SoranetPrivacyModeV1::Exit,
        }
    }
}

impl FromStr for RelayMode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "entry" => Ok(Self::Entry),
            "middle" => Ok(Self::Middle),
            "exit" => Ok(Self::Exit),
            other => Err(format!("unknown relay mode `{other}`")),
        }
    }
}

impl json::JsonSerialize for RelayMode {
    fn json_serialize(&self, out: &mut String) {
        json::write_json_string(self.as_label(), out);
    }
}

impl json::JsonDeserialize for RelayMode {
    fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
        let value = parser.parse_string()?;
        RelayMode::from_str(&value).map_err(json::Error::Message)
    }
}

/// TLS configuration for the QUIC endpoint.
#[derive(Debug, Clone, PartialEq, Eq, JsonDeserialize, JsonSerialize, Default)]
pub struct TlsConfig {
    /// Path to the PEM-encoded certificate chain served by the relay. When
    /// omitted the relay generates a self-signed leaf certificate at startup.
    pub certificate_path: Option<PathBuf>,
    /// Path to the PEM-encoded private key that matches `certificate_path`.
    pub private_key_path: Option<PathBuf>,
    /// Subject to use when the relay generates a self-signed certificate.
    pub self_signed_subject: Option<String>,
}

impl TlsConfig {
    pub fn subject_or_default(&self) -> &str {
        self.self_signed_subject
            .as_deref()
            .unwrap_or(DEFAULT_SELF_SIGNED_SUBJECT)
    }

    pub fn validate(&self) -> Result<(), ConfigError> {
        match (&self.certificate_path, &self.private_key_path) {
            (Some(_), Some(_)) | (None, None) => Ok(()),
            (Some(_), None) => Err(ConfigError::TlsPaths(
                "private key path missing while certificate path set".to_string(),
            )),
            (None, Some(_)) => Err(ConfigError::TlsPaths(
                "certificate path missing while private key path set".to_string(),
            )),
        }
    }
}

fn default_privacy_bucket_secs() -> u64 {
    DEFAULT_PRIVACY_BUCKET_SECS
}

fn default_privacy_min_handshakes() -> u64 {
    DEFAULT_PRIVACY_MIN_HANDSHAKES
}

fn default_privacy_flush_delay_buckets() -> u64 {
    DEFAULT_PRIVACY_FLUSH_DELAY_BUCKETS
}

fn default_privacy_force_flush_buckets() -> u64 {
    DEFAULT_PRIVACY_FORCE_FLUSH_BUCKETS
}

fn default_privacy_max_completed_buckets() -> usize {
    DEFAULT_PRIVACY_MAX_COMPLETED_BUCKETS
}

fn default_privacy_expected_shares() -> u16 {
    DEFAULT_PRIVACY_EXPECTED_SHARES
}

fn default_privacy_event_buffer_capacity() -> usize {
    DEFAULT_PRIVACY_EVENT_BUFFER_CAPACITY
}

/// Configuration for the privacy-preserving telemetry buckets.
#[derive(Debug, Clone, PartialEq, Eq, JsonDeserialize, JsonSerialize)]
pub struct PrivacyTelemetryConfig {
    /// Duration of each privacy bucket in seconds.
    #[norito(default = "default_privacy_bucket_secs")]
    pub bucket_secs: u64,
    /// Minimum completed handshakes required before a bucket is flushed.
    #[norito(default = "default_privacy_min_handshakes")]
    pub min_handshakes: u64,
    /// Buckets to delay before flushing partially-complete buckets.
    #[norito(default = "default_privacy_flush_delay_buckets")]
    pub flush_delay_buckets: u64,
    /// Maximum buckets to wait before forcing a flush even when incomplete.
    #[norito(default = "default_privacy_force_flush_buckets")]
    pub force_flush_buckets: u64,
    /// Maximum completed buckets retained in memory at once.
    #[norito(default = "default_privacy_max_completed_buckets")]
    pub max_completed_buckets: usize,
    /// Expected number of shares contributed by relays for aggregation.
    #[norito(default = "default_privacy_expected_shares")]
    pub expected_shares: u16,
    /// Capacity for the in-memory privacy event buffer.
    #[norito(default = "default_privacy_event_buffer_capacity")]
    pub event_buffer_capacity: usize,
}

impl Default for PrivacyTelemetryConfig {
    fn default() -> Self {
        Self {
            bucket_secs: DEFAULT_PRIVACY_BUCKET_SECS,
            min_handshakes: DEFAULT_PRIVACY_MIN_HANDSHAKES,
            flush_delay_buckets: DEFAULT_PRIVACY_FLUSH_DELAY_BUCKETS,
            force_flush_buckets: DEFAULT_PRIVACY_FORCE_FLUSH_BUCKETS,
            max_completed_buckets: DEFAULT_PRIVACY_MAX_COMPLETED_BUCKETS,
            expected_shares: DEFAULT_PRIVACY_EXPECTED_SHARES,
            event_buffer_capacity: DEFAULT_PRIVACY_EVENT_BUFFER_CAPACITY,
        }
    }
}

impl PrivacyTelemetryConfig {
    pub fn apply_defaults(&mut self) -> Result<(), ConfigError> {
        if self.bucket_secs == 0 {
            return Err(ConfigError::Privacy(
                "privacy.bucket_secs must be greater than zero".to_string(),
            ));
        }
        if self.min_handshakes == 0 {
            return Err(ConfigError::Privacy(
                "privacy.min_handshakes must be greater than zero".to_string(),
            ));
        }
        if self.max_completed_buckets == 0 {
            return Err(ConfigError::Privacy(
                "privacy.max_completed_buckets must be greater than zero".to_string(),
            ));
        }
        if self.expected_shares == 0 {
            return Err(ConfigError::Privacy(
                "privacy.expected_shares must be greater than zero".to_string(),
            ));
        }
        if self.event_buffer_capacity == 0 {
            self.event_buffer_capacity = DEFAULT_PRIVACY_EVENT_BUFFER_CAPACITY;
        }
        if self.force_flush_buckets < self.flush_delay_buckets {
            return Err(ConfigError::Privacy(format!(
                "privacy.force_flush_buckets ({}) must be >= privacy.flush_delay_buckets ({})",
                self.force_flush_buckets, self.flush_delay_buckets
            )));
        }
        Ok(())
    }
}

/// Parsed admission token parameters produced from configuration.
#[derive(Debug, Clone)]
pub struct TokenPolicySource {
    /// Verifier used to authenticate admission tokens.
    pub verifier: AdmissionTokenVerifier,
    /// Revoked token identifiers (32-byte digests).
    pub revocations: Vec<[u8; 32]>,
}

const DEFAULT_NORITO_CONNECT_TIMEOUT_MILLIS: u64 = 5_000;
const DEFAULT_NORITO_PADDING_TARGET_MILLIS: u64 = 35;
const DEFAULT_NORITO_ROUTE_REFRESH_SECS: u64 = 5;

fn default_norito_connect_timeout_millis() -> u64 {
    DEFAULT_NORITO_CONNECT_TIMEOUT_MILLIS
}

fn default_norito_padding_target_millis() -> u64 {
    DEFAULT_NORITO_PADDING_TARGET_MILLIS
}

fn default_norito_route_refresh_secs() -> u64 {
    DEFAULT_NORITO_ROUTE_REFRESH_SECS
}

const DEFAULT_KAIGI_CONNECT_TIMEOUT_MILLIS: u64 = 5_000;
const DEFAULT_KAIGI_ROUTE_REFRESH_SECS: u64 = 5;

fn default_kaigi_connect_timeout_millis() -> u64 {
    DEFAULT_KAIGI_CONNECT_TIMEOUT_MILLIS
}

fn default_kaigi_route_refresh_secs() -> u64 {
    DEFAULT_KAIGI_ROUTE_REFRESH_SECS
}

fn default_vpn_cell_size_bytes() -> u16 {
    DEFAULT_VPN_CELL_SIZE_BYTES
}

fn default_vpn_flow_label_bits() -> u8 {
    iroha_data_model::soranet::vpn::VpnFlowLabelV1::MAX_BITS
}

fn default_vpn_pacing_millis() -> u64 {
    DEFAULT_VPN_PACING_MILLIS
}

fn default_vpn_padding_budget_ms() -> u16 {
    DEFAULT_VPN_PADDING_BUDGET_MS
}

fn default_vpn_exit_class() -> String {
    DEFAULT_VPN_EXIT_CLASS.to_string()
}

fn default_vpn_lease_secs() -> u32 {
    DEFAULT_VPN_LEASE_SECS
}

fn default_vpn_dns_push_interval_secs() -> u32 {
    DEFAULT_VPN_DNS_PUSH_INTERVAL_SECS
}

fn default_vpn_cover_to_data_per_mille() -> u16 {
    DEFAULT_VPN_COVER_TO_DATA_PER_MILLE
}

fn default_vpn_heartbeat_millis() -> u16 {
    DEFAULT_VPN_HEARTBEAT_MILLIS
}

fn default_vpn_cover_burst_cells() -> u16 {
    DEFAULT_VPN_COVER_BURST_CELLS
}

fn default_vpn_cover_jitter_millis() -> u16 {
    DEFAULT_VPN_COVER_JITTER_MILLIS
}

fn default_vpn_session_meter_label() -> String {
    "vpn.session".to_string()
}

fn default_vpn_byte_meter_label() -> String {
    "vpn.egress.bytes".to_string()
}

fn default_vpn_meter_hash_hex() -> String {
    DEFAULT_VPN_METER_HASH_HEX.to_string()
}

/// Exit routing configuration for the relay's streaming services.
#[derive(Debug, Clone, PartialEq, Eq, JsonDeserialize, JsonSerialize, Default)]
pub struct ExitRoutingConfig {
    /// Norito streaming configuration exposed to Torii.
    #[norito(default)]
    pub norito_stream: Option<NoritoStreamRoutingConfig>,
    /// Kaigi streaming configuration exposed to Torii.
    #[norito(default)]
    pub kaigi_stream: Option<KaigiStreamRoutingConfig>,
}

impl ExitRoutingConfig {
    pub fn validate(&mut self) -> Result<(), ConfigError> {
        if let Some(route) = self.norito_stream.as_mut() {
            route.apply_defaults();
            let trimmed = route.torii_ws_url.trim();
            if trimmed.is_empty() {
                return Err(ConfigError::Routing(
                    "norito_stream.torii_ws_url must not be empty".to_string(),
                ));
            }
            if !trimmed.starts_with("ws://") && !trimmed.starts_with("wss://") {
                return Err(ConfigError::Routing(format!(
                    "norito_stream.torii_ws_url must start with ws:// or wss:// (got `{trimmed}`)"
                )));
            }
            route.torii_ws_url = trimmed.to_owned();
        }
        if let Some(route) = self.kaigi_stream.as_mut() {
            route.apply_defaults();
            let trimmed = route.hub_ws_url.trim();
            if trimmed.is_empty() {
                return Err(ConfigError::Routing(
                    "kaigi_stream.hub_ws_url must not be empty".to_string(),
                ));
            }
            if !trimmed.starts_with("ws://") && !trimmed.starts_with("wss://") {
                return Err(ConfigError::Routing(format!(
                    "kaigi_stream.hub_ws_url must start with ws:// or wss:// (got `{trimmed}`)"
                )));
            }
            route.hub_ws_url = trimmed.to_owned();
        }
        Ok(())
    }
}

/// Connection settings for the Norito streaming endpoint.
#[derive(Debug, Clone, PartialEq, Eq, JsonDeserialize, JsonSerialize)]
pub struct NoritoStreamRoutingConfig {
    /// Torii WebSocket endpoint (ws:// or wss://) exposed to clients.
    pub torii_ws_url: String,
    /// Connection timeout when opening a Norito route.
    #[norito(default = "default_norito_connect_timeout_millis")]
    pub connect_timeout_millis: u64,
    /// Target padding delay used when smoothing Norito traffic.
    #[norito(default = "default_norito_padding_target_millis")]
    pub padding_target_millis: u64,
    /// GAR category recorded for read-only access failures.
    #[norito(default)]
    pub gar_category_read_only: Option<String>,
    /// GAR category recorded for authenticated access failures.
    #[norito(default)]
    pub gar_category_authenticated: Option<String>,
    /// Optional on-disk spool directory for deframed Norito payloads.
    #[norito(default)]
    pub spool_dir: Option<PathBuf>,
    /// How often the relay refreshes Norito routes from Torii.
    #[norito(default = "default_norito_route_refresh_secs")]
    pub route_refresh_secs: u64,
}

impl NoritoStreamRoutingConfig {
    fn apply_defaults(&mut self) {
        if self.connect_timeout_millis == 0 {
            self.connect_timeout_millis = default_norito_connect_timeout_millis();
        }
        if self.padding_target_millis == 0 {
            self.padding_target_millis = default_norito_padding_target_millis();
        }
        if self.route_refresh_secs == 0 {
            self.route_refresh_secs = default_norito_route_refresh_secs();
        }
    }
}

/// Connection settings for the Kaigi streaming endpoint.
#[derive(Debug, Clone, PartialEq, Eq, JsonDeserialize, JsonSerialize)]
pub struct KaigiStreamRoutingConfig {
    /// Hub WebSocket endpoint (ws:// or wss://) used for Kaigi streams.
    pub hub_ws_url: String,
    /// Connection timeout when opening a Kaigi route.
    #[norito(default = "default_kaigi_connect_timeout_millis")]
    pub connect_timeout_millis: u64,
    /// GAR category recorded for unauthenticated access failures.
    #[norito(default)]
    pub gar_category_public: Option<String>,
    /// GAR category recorded for authenticated access failures.
    #[norito(default)]
    pub gar_category_authenticated: Option<String>,
    /// Optional on-disk spool directory for Kaigi payloads.
    #[norito(default)]
    pub spool_dir: Option<PathBuf>,
    /// How often the relay refreshes Kaigi routes from Torii.
    #[norito(default = "default_kaigi_route_refresh_secs")]
    pub route_refresh_secs: u64,
}

impl KaigiStreamRoutingConfig {
    fn apply_defaults(&mut self) {
        if self.connect_timeout_millis == 0 {
            self.connect_timeout_millis = default_kaigi_connect_timeout_millis();
        }
        if self.route_refresh_secs == 0 {
            self.route_refresh_secs = default_kaigi_route_refresh_secs();
        }
    }
}

/// VPN overlay configuration (IP-over-SoraNet tunnel).
#[derive(Debug, Clone, PartialEq, Eq, JsonDeserialize, JsonSerialize)]
pub struct VpnConfig {
    /// Whether the VPN overlay is enabled.
    #[norito(default)]
    pub enabled: bool,
    /// Fixed VPN cell size in bytes.
    #[norito(default = "default_vpn_cell_size_bytes")]
    pub cell_size_bytes: u16,
    /// Number of bits allocated for VPN flow labels.
    #[norito(default = "default_vpn_flow_label_bits")]
    pub flow_label_bits: u8,
    /// Target pacing interval between VPN cells in milliseconds.
    #[norito(default = "default_vpn_pacing_millis")]
    pub pacing_millis: u64,
    /// Padding budget for each VPN cell in milliseconds.
    #[norito(default = "default_vpn_padding_budget_ms")]
    pub padding_budget_ms: u16,
    /// Exit-class label advertised to upstream relays.
    #[norito(default = "default_vpn_exit_class")]
    pub exit_class: String,
    /// Control-plane lease duration (seconds).
    #[norito(default = "default_vpn_lease_secs")]
    pub lease_secs: u32,
    /// Interval for pushing DNS updates to clients (seconds).
    #[norito(default = "default_vpn_dns_push_interval_secs")]
    pub dns_push_interval_secs: u32,
    /// Routes pushed into the VPN client on connect.
    #[norito(default)]
    pub route_push: Vec<String>,
    /// DNS resolver overrides pushed into the VPN client on connect.
    #[norito(default)]
    pub dns_overrides: Vec<String>,
    /// Cover traffic generation parameters.
    #[norito(default)]
    pub cover: VpnCoverTrafficConfig,
    /// Billing/telemetry parameters for VPN sessions and bytes.
    #[norito(default)]
    pub billing: VpnBillingConfig,
}

impl Default for VpnConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            cell_size_bytes: default_vpn_cell_size_bytes(),
            flow_label_bits: default_vpn_flow_label_bits(),
            pacing_millis: default_vpn_pacing_millis(),
            padding_budget_ms: default_vpn_padding_budget_ms(),
            exit_class: default_vpn_exit_class(),
            lease_secs: default_vpn_lease_secs(),
            dns_push_interval_secs: default_vpn_dns_push_interval_secs(),
            route_push: Vec::new(),
            dns_overrides: Vec::new(),
            cover: VpnCoverTrafficConfig::default(),
            billing: VpnBillingConfig::default(),
        }
    }
}

impl VpnConfig {
    fn apply_defaults(&mut self) {
        if self.cell_size_bytes == 0 {
            self.cell_size_bytes = default_vpn_cell_size_bytes();
        }
        if self.pacing_millis == 0 {
            self.pacing_millis = default_vpn_pacing_millis();
        }
        if self.padding_budget_ms == 0 {
            self.padding_budget_ms = default_vpn_padding_budget_ms();
        }
        if self.lease_secs == 0 {
            self.lease_secs = default_vpn_lease_secs();
        }
        if self.dns_push_interval_secs == 0 {
            self.dns_push_interval_secs = default_vpn_dns_push_interval_secs();
        }
        self.cover.apply_defaults();
        self.billing.apply_defaults();
    }

    pub fn validate(&mut self) -> Result<(), ConfigError> {
        self.apply_defaults();
        self.cover.validate()?;
        self.billing.validate()?;
        if self.flow_label_bits == 0 || self.flow_label_bits > 24 {
            return Err(ConfigError::Vpn(
                "vpn.flow_label_bits must be between 1 and 24".to_string(),
            ));
        }
        if !self.enabled {
            return Ok(());
        }
        if self.cell_size_bytes == 0 || usize::from(self.cell_size_bytes) != VPN_CELL_LEN {
            return Err(ConfigError::Vpn(format!(
                "vpn.cell_size_bytes must equal the pinned cell length ({VPN_CELL_LEN} bytes)"
            )));
        }
        if self.pacing_millis == 0 {
            return Err(ConfigError::Vpn(
                "vpn.pacing_millis must be non-zero".to_string(),
            ));
        }
        if self.pacing_millis > u64::from(u16::MAX) {
            return Err(ConfigError::Vpn(
                "vpn.pacing_millis must fit into a u16".to_string(),
            ));
        }
        if self.padding_budget_ms == 0 {
            return Err(ConfigError::Vpn(
                "vpn.padding_budget_ms must be non-zero".to_string(),
            ));
        }
        let routes = self.parse_route_push()?;
        let dns_overrides = self.parse_dns_overrides()?;
        self.exit_class = self.exit_class.trim().to_owned();
        if let Err(error) = VpnExitClassV1::try_from_label(&self.exit_class) {
            return Err(ConfigError::Vpn(format!(
                "vpn.exit_class must be standard|low-latency|high-security: {error}"
            )));
        }
        self.route_push = routes.into_iter().map(|route| route.cidr.clone()).collect();
        self.dns_overrides = dns_overrides;
        Ok(())
    }

    /// Return the configured meter hash as raw bytes. Only safe to call after `validate`.
    pub fn meter_hash_bytes(&self) -> [u8; 32] {
        let mut meter_hash = [0u8; 32];
        let decoded =
            hex::decode(&self.billing.meter_hash_hex).expect("validated meter hash to decode");
        meter_hash.copy_from_slice(&decoded);
        meter_hash
    }

    /// Guard ensuring the VPN overlay is compiled in when enabled.
    pub fn require_runtime_available(&self) -> Result<(), ConfigError> {
        if self.enabled && !vpn_runtime_available() {
            return Err(ConfigError::Vpn(
                "vpn.enabled requires a tunnel runtime; rebuild with the VPN data-plane enabled"
                    .to_string(),
            ));
        }
        Ok(())
    }

    pub(crate) fn parse_route_push(&self) -> Result<Vec<VpnRouteV1>, ConfigError> {
        let mut parsed = Vec::with_capacity(self.route_push.len());
        for route in &self.route_push {
            let trimmed = route.trim();
            if trimmed.is_empty() {
                return Err(ConfigError::Vpn(
                    "vpn.route_push entries must not be empty".to_string(),
                ));
            }
            let (addr_str, prefix_str) = trimmed.split_once('/').ok_or_else(|| {
                ConfigError::Vpn(
                    "vpn.route_push entries must be CIDR strings (e.g. 10.0.0.0/24)".to_string(),
                )
            })?;
            let addr = addr_str.parse::<std::net::IpAddr>().map_err(|err| {
                ConfigError::Vpn(format!("vpn.route_push CIDR address is invalid: {err}"))
            })?;
            let prefix: u8 = prefix_str.parse().map_err(|err| {
                ConfigError::Vpn(format!(
                    "vpn.route_push CIDR prefix `{prefix_str}` is invalid: {err}"
                ))
            })?;
            let max_prefix = match addr {
                std::net::IpAddr::V4(_) => 32,
                std::net::IpAddr::V6(_) => 128,
            };
            if prefix > max_prefix {
                return Err(ConfigError::Vpn(format!(
                    "vpn.route_push CIDR prefix `{prefix}` exceeds maximum {max_prefix} for `{addr}`"
                )));
            }
            parsed.push(VpnRouteV1 {
                cidr: format!("{addr}/{prefix}"),
                via: None,
                metric: None,
            });
        }
        Ok(parsed)
    }

    pub(crate) fn parse_dns_overrides(&self) -> Result<Vec<String>, ConfigError> {
        let mut parsed = Vec::with_capacity(self.dns_overrides.len());
        for dns in &self.dns_overrides {
            let trimmed = dns.trim();
            if trimmed.is_empty() {
                return Err(ConfigError::Vpn(
                    "vpn.dns_overrides entries must not be empty".to_string(),
                ));
            }
            let addr = trimmed.parse::<std::net::IpAddr>().map_err(|err| {
                ConfigError::Vpn(format!(
                    "vpn.dns_overrides entry `{trimmed}` is not a valid IP address: {err}"
                ))
            })?;
            parsed.push(addr.to_string());
        }
        Ok(parsed)
    }
}

/// Billing/telemetry settings for VPN sessions.
#[derive(Debug, Clone, PartialEq, Eq, JsonDeserialize, JsonSerialize)]
pub struct VpnBillingConfig {
    /// Whether billing/telemetry emission is enabled.
    #[norito(default)]
    pub enabled: bool,
    /// Meter label used for per-session accounting.
    #[norito(default = "default_vpn_session_meter_label")]
    pub session_meter_label: String,
    /// Meter label used for byte-level accounting.
    #[norito(default = "default_vpn_byte_meter_label")]
    pub byte_meter_label: String,
    /// Hex-encoded 32-byte meter hash expected by the accounting backend.
    #[norito(default = "default_vpn_meter_hash_hex")]
    pub meter_hash_hex: String,
}

impl VpnBillingConfig {
    fn apply_defaults(&mut self) {
        if self.session_meter_label.trim().is_empty() {
            self.session_meter_label = default_vpn_session_meter_label();
        } else {
            self.session_meter_label = self.session_meter_label.trim().to_owned();
        }
        if self.byte_meter_label.trim().is_empty() {
            self.byte_meter_label = default_vpn_byte_meter_label();
        } else {
            self.byte_meter_label = self.byte_meter_label.trim().to_owned();
        }
        if self.meter_hash_hex.trim().is_empty() {
            self.meter_hash_hex = default_vpn_meter_hash_hex();
        } else {
            self.meter_hash_hex = self.meter_hash_hex.trim().to_owned();
        }
    }

    fn validate(&mut self) -> Result<(), ConfigError> {
        self.apply_defaults();
        if self.meter_hash_hex.len() != 64
            || hex::decode(&self.meter_hash_hex)
                .map(|bytes| bytes.len())
                .unwrap_or(0)
                != 32
        {
            return Err(ConfigError::Vpn(
                "vpn.billing.meter_hash_hex must be a 64-character hex-encoded 32-byte hash"
                    .to_string(),
            ));
        }
        Ok(())
    }
}

impl Default for VpnBillingConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            session_meter_label: default_vpn_session_meter_label(),
            byte_meter_label: default_vpn_byte_meter_label(),
            meter_hash_hex: default_vpn_meter_hash_hex(),
        }
    }
}

/// Cover-traffic configuration for the VPN overlay.
#[derive(Debug, Clone, PartialEq, Eq, JsonDeserialize, JsonSerialize, Default)]
pub struct VpnCoverTrafficConfig {
    /// Whether to emit cover traffic alongside user data.
    #[norito(default)]
    pub enabled: bool,
    /// Ratio of cover cells to data cells, expressed in permille.
    #[norito(default = "default_vpn_cover_to_data_per_mille")]
    pub cover_to_data_per_mille: u16,
    /// Heartbeat cadence for cover streams in milliseconds.
    #[norito(default = "default_vpn_heartbeat_millis")]
    pub heartbeat_ms: u16,
    /// Maximum burst of consecutive cover cells permitted.
    #[norito(default = "default_vpn_cover_burst_cells")]
    pub max_cover_burst: u16,
    /// Maximum jitter applied to cover heartbeats in milliseconds.
    #[norito(default = "default_vpn_cover_jitter_millis")]
    pub max_jitter_millis: u16,
}

impl VpnCoverTrafficConfig {
    fn apply_defaults(&mut self) {
        if self.cover_to_data_per_mille == 0 && !self.enabled {
            self.cover_to_data_per_mille = default_vpn_cover_to_data_per_mille();
        }
        if self.heartbeat_ms == 0 {
            self.heartbeat_ms = default_vpn_heartbeat_millis();
        }
        if self.max_cover_burst == 0 {
            self.max_cover_burst = default_vpn_cover_burst_cells();
        }
        if self.max_jitter_millis == 0 {
            self.max_jitter_millis = default_vpn_cover_jitter_millis();
        }
    }

    fn validate(&self) -> Result<(), ConfigError> {
        if self.heartbeat_ms == 0 || self.max_cover_burst == 0 {
            return Err(ConfigError::Vpn(
                "vpn.cover.heartbeat_ms and vpn.cover.max_cover_burst must be non-zero".to_string(),
            ));
        }
        if self.cover_to_data_per_mille > 1_000 {
            return Err(ConfigError::Vpn(
                "vpn.cover.cover_to_data_per_mille must be between 0 and 1000".to_string(),
            ));
        }
        if u64::from(self.max_jitter_millis) > u64::from(self.heartbeat_ms) {
            return Err(ConfigError::Vpn(
                "vpn.cover.max_jitter_millis must not exceed vpn.cover.heartbeat_ms".to_string(),
            ));
        }
        Ok(())
    }
}

/// Returns whether the VPN tunnel runtime is available.
pub const fn vpn_runtime_available() -> bool {
    true
}

/// Proof-of-work policy.
#[derive(Debug, Clone, PartialEq, Eq, JsonDeserialize, JsonSerialize)]
pub struct PowConfig {
    /// Whether PoW is required for admission tokens.
    pub required: bool,
    /// Baseline difficulty (shifted into the PoW challenge).
    pub difficulty: u32,
    /// Maximum clock skew accepted when verifying PoW tickets.
    #[norito(default)]
    pub max_future_skew_secs: u64,
    /// Minimum time-to-live accepted for PoW tickets.
    #[norito(default)]
    pub min_ticket_ttl_secs: u64,
    /// Capacity of the in-memory revocation store for spent tickets.
    #[norito(default = "PowConfig::default_revocation_store_capacity")]
    pub revocation_store_capacity: u64,
    /// TTL (seconds) for PoW ticket revocations.
    #[norito(default = "PowConfig::default_revocation_store_ttl")]
    pub revocation_store_ttl_secs: u64,
    /// On-disk path where revocations are persisted.
    #[norito(default = "PowConfig::default_revocation_store_path")]
    pub revocation_store_path: PathBuf,
    /// Optional verifying key used for signed PoW tickets.
    #[norito(default)]
    pub signed_ticket_public_key_hex: Option<String>,
    /// Adaptive difficulty tuning parameters.
    #[norito(default)]
    pub adaptive: AdaptiveDifficultyConfig,
    /// Global rate limits applied to handshake traffic.
    #[norito(default)]
    pub quotas: QuotaConfig,
    /// Mode-specific quota overrides (entry/middle/exit).
    #[norito(default)]
    pub quotas_per_mode: Option<HopQuotaOverrides>,
    /// Slowloris mitigation thresholds applied to client hellos.
    #[norito(default)]
    pub slowloris: SlowlorisConfig,
    /// Optional Argon2 puzzle applied to inbound connections.
    #[norito(default)]
    pub puzzle: Option<PuzzleConfig>,
    /// Optional signed token authentication layer.
    #[norito(default)]
    pub token: Option<TokenConfig>,
    /// Replay filter enforcing nonce uniqueness for admission.
    #[norito(default)]
    pub replay_filter: ReplayFilterConfig,
    /// Emergency throttle applied when the relay is degraded.
    #[norito(default)]
    pub emergency: Option<EmergencyThrottleConfig>,
}

impl Default for PowConfig {
    fn default() -> Self {
        Self {
            required: false,
            difficulty: 0,
            max_future_skew_secs: 300,
            min_ticket_ttl_secs: 30,
            revocation_store_capacity: Self::default_revocation_store_capacity(),
            revocation_store_ttl_secs: Self::default_revocation_store_ttl(),
            revocation_store_path: Self::default_revocation_store_path(),
            signed_ticket_public_key_hex: None,
            adaptive: AdaptiveDifficultyConfig::default(),
            quotas: QuotaConfig::default(),
            quotas_per_mode: None,
            slowloris: SlowlorisConfig::default(),
            puzzle: Some(PuzzleConfig {
                enabled: true,
                memory_kib: PuzzleConfig::default_memory_kib(),
                time_cost: PuzzleConfig::default_time_cost(),
                lanes: PuzzleConfig::default_lanes(),
            }),
            token: None,
            replay_filter: ReplayFilterConfig::default(),
            emergency: None,
        }
    }
}

impl PowConfig {
    const fn default_revocation_store_capacity() -> u64 {
        8_192
    }

    const fn default_revocation_store_ttl() -> u64 {
        900
    }

    fn default_revocation_store_path() -> PathBuf {
        PathBuf::from("./storage/soranet/ticket_revocations.norito")
    }

    pub fn apply_defaults(&mut self) -> Result<(), ConfigError> {
        if self.max_future_skew_secs == 0 {
            self.max_future_skew_secs = 300;
        }
        if self.min_ticket_ttl_secs == 0 {
            self.min_ticket_ttl_secs = 30;
        }
        if self.revocation_store_capacity == 0 {
            self.revocation_store_capacity = Self::default_revocation_store_capacity();
        }
        if self.revocation_store_ttl_secs == 0 {
            self.revocation_store_ttl_secs = Self::default_revocation_store_ttl();
        }
        if self.revocation_store_path.as_os_str().is_empty() {
            self.revocation_store_path = Self::default_revocation_store_path();
        }
        self.adaptive.apply_defaults();
        self.quotas.apply_defaults();
        if let Some(overrides) = self.quotas_per_mode.as_mut() {
            overrides.apply_defaults();
        }
        self.slowloris.apply_defaults();
        if let Some(puzzle) = self.puzzle.as_mut() {
            puzzle.apply_defaults();
            puzzle.validate()?;
        }
        if let Some(token) = self.token.as_mut() {
            token.apply_defaults();
            token.validate()?;
        }
        self.replay_filter.apply_defaults()?;
        if let Some(emergency) = self.emergency.as_mut() {
            emergency.apply_defaults()?;
        }
        Ok(())
    }

    /// Build the admission token verifier if configured.
    pub fn token_policy(&self) -> Result<Option<TokenPolicySource>, ConfigError> {
        match &self.token {
            Some(cfg) => cfg.build_policy(),
            None => Ok(None),
        }
    }

    pub fn quotas_for_mode(&self, mode: RelayMode) -> QuotaConfig {
        let mut effective = if let Some(overrides) = &self.quotas_per_mode {
            overrides
                .for_mode(mode)
                .unwrap_or_else(|| self.quotas.clone())
        } else {
            self.quotas.clone()
        };
        effective.apply_defaults();
        effective
    }

    pub fn puzzle_parameters(
        &self,
        base: &pow::Parameters,
    ) -> Result<Option<puzzle::Parameters>, ConfigError> {
        match &self.puzzle {
            Some(cfg) => cfg.parameters(base),
            None => Ok(None),
        }
    }

    pub fn replay_filter(&self) -> &ReplayFilterConfig {
        &self.replay_filter
    }

    pub fn emergency_throttle(&self) -> Option<&EmergencyThrottleConfig> {
        self.emergency.as_ref()
    }
}

/// Adaptive difficulty tuning knobs for the relay PoW verifier.
#[derive(Debug, Clone, PartialEq, Eq, JsonDeserialize, JsonSerialize)]
pub struct AdaptiveDifficultyConfig {
    /// Whether adaptive difficulty tuning is enabled.
    #[norito(default = "AdaptiveDifficultyConfig::default_enabled")]
    pub enabled: bool,
    /// Minimum difficulty the tuner may select.
    #[norito(default = "AdaptiveDifficultyConfig::default_min_difficulty")]
    pub min_difficulty: u8,
    /// Maximum difficulty the tuner may select.
    #[norito(default = "AdaptiveDifficultyConfig::default_max_difficulty")]
    pub max_difficulty: u8,
    /// Consecutive failures required before increasing difficulty.
    #[norito(default = "AdaptiveDifficultyConfig::default_pow_failure_threshold")]
    pub pow_failure_threshold: u32,
    /// Consecutive successes required before decreasing difficulty.
    #[norito(default = "AdaptiveDifficultyConfig::default_success_threshold")]
    pub success_threshold: u32,
    /// Sliding window used when counting successes/failures.
    #[norito(default = "AdaptiveDifficultyConfig::default_window_secs")]
    pub window_secs: u64,
    /// Step applied when increasing difficulty.
    #[norito(default = "AdaptiveDifficultyConfig::default_increase_step")]
    pub increase_step: u8,
    /// Step applied when decreasing difficulty.
    #[norito(default = "AdaptiveDifficultyConfig::default_decrease_step")]
    pub decrease_step: u8,
}

impl Default for AdaptiveDifficultyConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            min_difficulty: Self::default_min_difficulty(),
            max_difficulty: Self::default_max_difficulty(),
            pow_failure_threshold: Self::default_pow_failure_threshold(),
            success_threshold: Self::default_success_threshold(),
            window_secs: Self::default_window_secs(),
            increase_step: Self::default_increase_step(),
            decrease_step: Self::default_decrease_step(),
        }
    }
}

impl AdaptiveDifficultyConfig {
    const fn default_enabled() -> bool {
        true
    }

    const fn default_min_difficulty() -> u8 {
        6
    }

    const fn default_max_difficulty() -> u8 {
        28
    }

    const fn default_pow_failure_threshold() -> u32 {
        24
    }

    const fn default_success_threshold() -> u32 {
        480
    }

    const fn default_window_secs() -> u64 {
        60
    }

    const fn default_increase_step() -> u8 {
        1
    }

    const fn default_decrease_step() -> u8 {
        1
    }

    pub fn apply_defaults(&mut self) {
        if self.max_difficulty < self.min_difficulty {
            self.max_difficulty = self.min_difficulty;
        }
        if self.pow_failure_threshold == 0 {
            self.pow_failure_threshold = Self::default_pow_failure_threshold();
        }
        if self.success_threshold == 0 {
            self.success_threshold = Self::default_success_threshold();
        }
        if self.window_secs == 0 {
            self.window_secs = Self::default_window_secs();
        }
        if self.increase_step == 0 {
            self.increase_step = Self::default_increase_step();
        }
        if self.decrease_step == 0 {
            self.decrease_step = Self::default_decrease_step();
        }
    }
}

/// Argon2 puzzle configuration applied to inbound handshakes.
#[derive(Debug, Clone, PartialEq, Eq, JsonDeserialize, JsonSerialize)]
pub struct PuzzleConfig {
    /// Whether the Argon2 puzzle is enforced.
    #[norito(default)]
    pub enabled: bool,
    /// Memory cost (KiB) for the Argon2 puzzle.
    #[norito(default = "PuzzleConfig::default_memory_kib")]
    pub memory_kib: u32,
    /// Time cost for the Argon2 puzzle.
    #[norito(default = "PuzzleConfig::default_time_cost")]
    pub time_cost: u32,
    /// Number of Argon2 lanes to use when verifying.
    #[norito(default = "PuzzleConfig::default_lanes")]
    pub lanes: u32,
}

impl PuzzleConfig {
    const fn default_memory_kib() -> u32 {
        64 * 1024
    }

    const fn default_time_cost() -> u32 {
        2
    }

    const fn default_lanes() -> u32 {
        1
    }

    fn apply_defaults(&mut self) {
        if self.memory_kib == 0 {
            self.memory_kib = Self::default_memory_kib();
        }
        if self.time_cost == 0 {
            self.time_cost = Self::default_time_cost();
        }
        if self.lanes == 0 {
            self.lanes = Self::default_lanes();
        }
    }

    fn validate(&self) -> Result<(), ConfigError> {
        if !self.enabled {
            return Ok(());
        }
        if self.memory_kib < 4096 {
            return Err(ConfigError::Puzzle(
                "pow.puzzle.memory_kib must be at least 4096".to_string(),
            ));
        }
        if self.time_cost == 0 {
            return Err(ConfigError::Puzzle(
                "pow.puzzle.time_cost must be greater than zero".to_string(),
            ));
        }
        if !(1..=16).contains(&self.lanes) {
            return Err(ConfigError::Puzzle(
                "pow.puzzle.lanes must be between 1 and 16".to_string(),
            ));
        }
        Ok(())
    }

    fn parameters(
        &self,
        base: &pow::Parameters,
    ) -> Result<Option<puzzle::Parameters>, ConfigError> {
        if !self.enabled {
            return Ok(None);
        }
        let memory = NonZeroU32::new(self.memory_kib).ok_or_else(|| {
            ConfigError::Puzzle("pow.puzzle.memory_kib must be non-zero".to_string())
        })?;
        let time = NonZeroU32::new(self.time_cost).ok_or_else(|| {
            ConfigError::Puzzle("pow.puzzle.time_cost must be non-zero".to_string())
        })?;
        let lanes = NonZeroU32::new(self.lanes)
            .ok_or_else(|| ConfigError::Puzzle("pow.puzzle.lanes must be non-zero".to_string()))?;
        Ok(Some(puzzle::Parameters::new(
            memory,
            time,
            lanes,
            base.difficulty(),
            base.max_future_skew(),
            base.min_ticket_ttl(),
        )))
    }
}

/// Admission token configuration for bypassing puzzles.
#[derive(Debug, Clone, Default, PartialEq, Eq, JsonDeserialize, JsonSerialize)]
pub struct TokenConfig {
    /// Whether signed admission tokens are accepted.
    #[norito(default)]
    pub enabled: bool,
    /// Hex-encoded verifier key for validating admission tokens.
    #[norito(default)]
    pub issuer_public_key_hex: Option<String>,
    /// Hex-encoded issuer secret key used to mint admission tokens (test only).
    #[norito(default)]
    pub issuer_secret_key_hex: Option<String>,
    /// Path to a file containing the issuer secret key (test only).
    #[norito(default)]
    pub issuer_secret_key_path: Option<PathBuf>,
    /// Maximum TTL for admission tokens accepted by the relay.
    #[norito(default = "TokenConfig::default_max_ttl_secs")]
    pub max_ttl_secs: u64,
    /// Allowed clock skew when validating token timestamps.
    #[norito(default = "TokenConfig::default_clock_skew_secs")]
    pub clock_skew_secs: u64,
    /// Capacity of the replay filter used to reject reused tokens.
    #[norito(default = "TokenConfig::default_replay_store_capacity")]
    pub replay_store_capacity: usize,
    /// TTL (seconds) for entries in the token replay filter.
    #[norito(default = "TokenConfig::default_replay_store_ttl_secs")]
    pub replay_store_ttl_secs: u64,
    /// On-disk path for persisting replay filter state.
    #[norito(default)]
    pub replay_store_path: Option<PathBuf>,
    /// Hex-encoded list of revoked token identifiers.
    #[norito(default)]
    pub revocation_list_hex: Vec<String>,
    /// Path to a file containing revoked token identifiers.
    #[norito(default)]
    pub revocation_list_path: Option<PathBuf>,
    /// Optional refresh cadence for reloading revocation files.
    #[norito(default)]
    pub revocation_refresh_secs: Option<u64>,
}

impl TokenConfig {
    const fn default_max_ttl_secs() -> u64 {
        900
    }

    const fn default_clock_skew_secs() -> u64 {
        5
    }

    const fn default_replay_store_capacity() -> usize {
        8_192
    }

    const fn default_replay_store_ttl_secs() -> u64 {
        900
    }

    fn apply_defaults(&mut self) {
        if self.max_ttl_secs == 0 {
            self.max_ttl_secs = Self::default_max_ttl_secs();
        }
        if self.clock_skew_secs == 0 {
            self.clock_skew_secs = Self::default_clock_skew_secs();
        }
        if self.replay_store_capacity == 0 {
            self.replay_store_capacity = Self::default_replay_store_capacity();
        }
        if self.replay_store_ttl_secs == 0 {
            self.replay_store_ttl_secs = Self::default_replay_store_ttl_secs();
        }
        if let Some(refresh) = self.revocation_refresh_secs.as_mut()
            && *refresh == 0
        {
            *refresh = 30;
        }
    }

    fn validate(&self) -> Result<(), ConfigError> {
        if !self.enabled {
            return Ok(());
        }
        let Some(public_hex) = self.issuer_public_key_hex.as_ref() else {
            return Err(ConfigError::Token(
                "pow.token.issuer_public_key_hex must be set when pow.token.enabled = true"
                    .to_string(),
            ));
        };
        if public_hex.is_empty() {
            return Err(ConfigError::Token(
                "pow.token.issuer_public_key_hex must not be empty".to_string(),
            ));
        }
        for (idx, value) in self.revocation_list_hex.iter().enumerate() {
            match hex::decode(value) {
                Ok(bytes) if bytes.len() == 32 => {}
                Ok(_) => {
                    return Err(ConfigError::Token(format!(
                        "pow.token.revocation_list_hex[{idx}] must decode to 32 bytes"
                    )));
                }
                Err(err) => {
                    return Err(ConfigError::Hex {
                        field: format!("pow.token.revocation_list_hex[{idx}]"),
                        kind: err,
                    });
                }
            }
        }
        if let Some(path) = &self.revocation_list_path
            && path.as_os_str().is_empty()
        {
            return Err(ConfigError::Token(
                "pow.token.revocation_list_path must not be empty".to_string(),
            ));
        }
        if let Some(path) = &self.replay_store_path
            && path.as_os_str().is_empty()
        {
            return Err(ConfigError::Token(
                "pow.token.replay_store_path must not be empty".to_string(),
            ));
        }
        if self.replay_store_capacity == 0 {
            return Err(ConfigError::Token(
                "pow.token.replay_store_capacity must be greater than zero".to_string(),
            ));
        }
        if self.replay_store_ttl_secs == 0 {
            return Err(ConfigError::Token(
                "pow.token.replay_store_ttl_secs must be greater than zero".to_string(),
            ));
        }
        if let Some(path) = &self.replay_store_path
            && path.as_os_str().is_empty()
        {
            return Err(ConfigError::Token(
                "pow.token.replay_store_path must not be empty when set".to_string(),
            ));
        }
        Ok(())
    }

    fn build_policy(&self) -> Result<Option<TokenPolicySource>, ConfigError> {
        if !self.enabled {
            return Ok(None);
        }
        let public_hex = self
            .issuer_public_key_hex
            .as_ref()
            .expect("validated issuer public key");
        let public_key = hex::decode(public_hex).map_err(|kind| ConfigError::Hex {
            field: "pow.token.issuer_public_key_hex".to_string(),
            kind,
        })?;

        let store_limits = TokenStoreLimits::new(
            self.replay_store_capacity,
            Duration::from_secs(self.replay_store_ttl_secs.min(self.max_ttl_secs)),
        )
        .map_err(|err| {
            ConfigError::Token(format!("invalid pow.token replay store settings: {err}"))
        })?;
        let store: Arc<Mutex<dyn TokenStore + Send>> = if let Some(path) = &self.replay_store_path {
            let persistent = PersistentTokenStore::load(path, store_limits, SystemTime::now())
                .map_err(|err| {
                    ConfigError::Token(format!(
                        "failed to load pow.token replay store ({}): {err}",
                        path.display()
                    ))
                })?;
            Arc::new(Mutex::new(persistent))
        } else {
            Arc::new(Mutex::new(InMemoryTokenStore::new(store_limits)))
        };

        let verifier = AdmissionTokenVerifier::new(
            MlDsaSuite::MlDsa44,
            public_key,
            Duration::from_secs(self.max_ttl_secs),
            Duration::from_secs(self.clock_skew_secs),
        )
        .with_replay_store(store);

        let mut revocations = Vec::with_capacity(self.revocation_list_hex.len());
        for (idx, value) in self.revocation_list_hex.iter().enumerate() {
            let bytes = hex::decode(value).map_err(|kind| ConfigError::Hex {
                field: format!("pow.token.revocation_list_hex[{idx}]"),
                kind,
            })?;
            if bytes.len() != 32 {
                return Err(ConfigError::Token(format!(
                    "pow.token.revocation_list_hex[{idx}] must decode to 32 bytes"
                )));
            }
            let mut id = [0u8; 32];
            id.copy_from_slice(&bytes);
            revocations.push(id);
        }

        if let Some(path) = &self.revocation_list_path {
            let ids = read_revocation_file(path).map_err(|err| {
                ConfigError::Token(format!(
                    "failed to load pow.token.revocation_list_path ({}): {err}",
                    path.display()
                ))
            })?;
            revocations.extend(ids);
        }

        Ok(Some(TokenPolicySource {
            verifier,
            revocations,
        }))
    }
}

/// Emergency throttle configuration sourced from directory consensus.
#[derive(Debug, Clone, PartialEq, Eq, JsonDeserialize, JsonSerialize)]
pub struct EmergencyThrottleConfig {
    /// Expected directory commit hashes that activate the throttle.
    #[norito(default)]
    pub descriptor_commit_hex: Vec<String>,
    /// Optional path to an on-disk throttle descriptor.
    #[norito(default)]
    pub file_path: Option<PathBuf>,
    /// Cooldown before resuming normal operation once throttled.
    #[norito(default = "EmergencyThrottleConfig::default_cooldown_secs")]
    pub cooldown_secs: u64,
    /// Interval for reloading throttle descriptors.
    #[norito(default = "EmergencyThrottleConfig::default_refresh_secs")]
    pub refresh_secs: u64,
}

impl EmergencyThrottleConfig {
    const fn default_cooldown_secs() -> u64 {
        300
    }

    const fn default_refresh_secs() -> u64 {
        30
    }

    pub fn apply_defaults(&mut self) -> Result<(), ConfigError> {
        if self.cooldown_secs == 0 {
            self.cooldown_secs = Self::default_cooldown_secs();
        }
        if self.refresh_secs == 0 {
            self.refresh_secs = Self::default_refresh_secs();
        }
        for (idx, hex_value) in self.descriptor_commit_hex.iter().enumerate() {
            capability::parse_descriptor_commit_hex(hex_value).map_err(|_| {
                ConfigError::EmergencyThrottle(format!(
                    "descriptor_commit_hex[{idx}] must decode to 32 bytes"
                ))
            })?;
        }
        Ok(())
    }
}

/// Guard directory snapshot validation configuration.
#[derive(Debug, Clone, PartialEq, Eq, JsonDeserialize, JsonSerialize)]
pub struct GuardDirectoryConfig {
    /// Path to the guard directory snapshot.
    pub snapshot_path: PathBuf,
    /// Optional expected hash of the snapshot file for integrity checks.
    #[norito(default)]
    pub expected_directory_hash_hex: Option<String>,
    /// Whether to tolerate missing entries when verifying a snapshot.
    #[norito(default)]
    pub allow_missing_entry: bool,
    /// Optional path to a pinning proof associated with the snapshot.
    #[norito(default)]
    pub pinning_proof_path: Option<PathBuf>,
}

impl GuardDirectoryConfig {
    pub fn apply_defaults(&mut self) -> Result<(), ConfigError> {
        if let Some(hex_value) = &self.expected_directory_hash_hex {
            if hex_value.len() != 64 {
                return Err(ConfigError::GuardDirectory(
                    "expected_directory_hash_hex must be 64 hex characters".to_string(),
                ));
            }
            hex::decode(hex_value).map_err(|_| {
                ConfigError::GuardDirectory(
                    "expected_directory_hash_hex must decode to 32 bytes".to_string(),
                )
            })?;
        }
        Ok(())
    }

    #[must_use]
    pub fn snapshot_path(&self) -> &Path {
        &self.snapshot_path
    }

    #[must_use]
    pub fn expected_directory_hash(&self) -> Option<[u8; 32]> {
        self.expected_directory_hash_hex.as_ref().map(|hex_value| {
            let raw = hex::decode(hex_value).expect("hex validated in apply_defaults");
            let mut bytes = [0u8; 32];
            bytes.copy_from_slice(&raw);
            bytes
        })
    }

    #[must_use]
    pub fn allow_missing_entry(&self) -> bool {
        self.allow_missing_entry
    }

    #[must_use]
    pub fn pinning_proof_path(&self) -> Option<&Path> {
        self.pinning_proof_path.as_deref()
    }
}

/// Configuration for the blinded descriptor replay filter.
#[derive(Debug, Clone, PartialEq, Eq, JsonDeserialize, JsonSerialize)]
pub struct ReplayFilterConfig {
    /// Whether the replay filter is enforced during handshakes.
    #[norito(default)]
    pub enabled: bool,
    /// Number of counters in the bloom filter (rounded up to the next power of two).
    #[norito(default = "ReplayFilterConfig::default_bits")]
    pub bits: u32,
    /// Number of independent hash probes used when inserting into the filter.
    #[norito(default = "ReplayFilterConfig::default_hash_functions")]
    pub hash_functions: u8,
    /// Time-to-live for entries retained inside the filter, in seconds.
    #[norito(default = "ReplayFilterConfig::default_ttl_secs")]
    pub ttl_secs: u64,
}

impl Default for ReplayFilterConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            bits: Self::default_bits(),
            hash_functions: Self::default_hash_functions(),
            ttl_secs: Self::default_ttl_secs(),
        }
    }
}

impl ReplayFilterConfig {
    const fn default_bits() -> u32 {
        1 << 18 // 262,144 counters
    }

    const fn default_hash_functions() -> u8 {
        4
    }

    const fn default_ttl_secs() -> u64 {
        30
    }

    pub fn apply_defaults(&mut self) -> Result<(), ConfigError> {
        const MAX_BITS: u32 = 1 << 24; // 16,777,216 counters

        if self.hash_functions == 0 {
            self.hash_functions = Self::default_hash_functions();
        }
        if self.hash_functions > 16 {
            return Err(ConfigError::ReplayFilter(
                "replay_filter.hash_functions must be between 1 and 16".to_string(),
            ));
        }

        if self.bits == 0 {
            self.bits = Self::default_bits();
        }
        let clamped = self.bits.max(64);
        let next_power = clamped.next_power_of_two();
        if next_power > MAX_BITS {
            return Err(ConfigError::ReplayFilter(
                "replay_filter.bits must not exceed 16,777,216".to_string(),
            ));
        }
        self.bits = next_power;

        if self.ttl_secs == 0 {
            self.ttl_secs = Self::default_ttl_secs();
        }
        Ok(())
    }

    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    #[must_use]
    pub fn ttl(&self) -> Duration {
        Duration::from_secs(self.ttl_secs.max(1))
    }

    #[must_use]
    pub fn bits_usize(&self) -> usize {
        self.bits as usize
    }

    #[must_use]
    pub fn hash_count(&self) -> u8 {
        self.hash_functions
    }
}

/// Admission quotas applied to inbound circuits.
#[derive(Debug, Clone, PartialEq, Eq, JsonDeserialize, JsonSerialize)]
pub struct QuotaConfig {
    /// Burst of circuits permitted from a single remote before throttling.
    #[norito(default = "QuotaConfig::default_per_remote_burst")]
    pub per_remote_burst: u32,
    /// Window (seconds) over which `per_remote_burst` is measured.
    #[norito(default = "QuotaConfig::default_per_remote_window_secs")]
    pub per_remote_window_secs: u64,
    /// Burst of circuits permitted per blinded descriptor.
    #[norito(default = "QuotaConfig::default_per_descriptor_burst")]
    pub per_descriptor_burst: u32,
    /// Window (seconds) over which `per_descriptor_burst` is measured.
    #[norito(default = "QuotaConfig::default_per_descriptor_window_secs")]
    pub per_descriptor_window_secs: u64,
    /// Cooldown applied after exceeding a quota window.
    #[norito(default = "QuotaConfig::default_cooldown_secs")]
    pub cooldown_secs: u64,
    /// Maximum entries retained in the quota tracker.
    #[norito(default = "QuotaConfig::default_max_entries")]
    pub max_entries: usize,
}

impl Default for QuotaConfig {
    fn default() -> Self {
        Self {
            per_remote_burst: Self::default_per_remote_burst(),
            per_remote_window_secs: Self::default_per_remote_window_secs(),
            per_descriptor_burst: Self::default_per_descriptor_burst(),
            per_descriptor_window_secs: Self::default_per_descriptor_window_secs(),
            cooldown_secs: Self::default_cooldown_secs(),
            max_entries: Self::default_max_entries(),
        }
    }
}

impl QuotaConfig {
    const fn default_per_remote_burst() -> u32 {
        40
    }

    const fn default_per_remote_window_secs() -> u64 {
        60
    }

    const fn default_per_descriptor_burst() -> u32 {
        160
    }

    const fn default_per_descriptor_window_secs() -> u64 {
        60
    }

    const fn default_cooldown_secs() -> u64 {
        20
    }

    const fn default_max_entries() -> usize {
        4_096
    }

    pub fn apply_defaults(&mut self) {
        if self.per_remote_window_secs == 0 {
            self.per_remote_window_secs = Self::default_per_remote_window_secs();
        }
        if self.per_descriptor_window_secs == 0 {
            self.per_descriptor_window_secs = Self::default_per_descriptor_window_secs();
        }
        if self.cooldown_secs == 0 {
            self.cooldown_secs = Self::default_cooldown_secs();
        }
        if self.max_entries == 0 {
            self.max_entries = Self::default_max_entries();
        }
    }
}

/// Optional overrides for quota configuration per relay hop.
#[derive(Debug, Clone, PartialEq, Eq, JsonDeserialize, JsonSerialize, Default)]
pub struct HopQuotaOverrides {
    /// Entry-relay quota overrides.
    #[norito(default)]
    pub entry: Option<QuotaConfig>,
    /// Middle-relay quota overrides.
    #[norito(default)]
    pub middle: Option<QuotaConfig>,
    /// Exit-relay quota overrides.
    #[norito(default)]
    pub exit: Option<QuotaConfig>,
}

impl HopQuotaOverrides {
    fn apply_defaults(&mut self) {
        if let Some(entry) = self.entry.as_mut() {
            entry.apply_defaults();
        }
        if let Some(middle) = self.middle.as_mut() {
            middle.apply_defaults();
        }
        if let Some(exit) = self.exit.as_mut() {
            exit.apply_defaults();
        }
    }

    fn for_mode(&self, mode: RelayMode) -> Option<QuotaConfig> {
        match mode {
            RelayMode::Entry => self.entry.clone(),
            RelayMode::Middle => self.middle.clone(),
            RelayMode::Exit => self.exit.clone(),
        }
    }
}

/// Slowloris-style connection detection thresholds.
#[derive(Debug, Clone, PartialEq, Eq, JsonDeserialize, JsonSerialize)]
pub struct SlowlorisConfig {
    /// Whether slowloris detection is enabled.
    #[norito(default = "SlowlorisConfig::default_enabled")]
    pub enabled: bool,
    /// Maximum time allowed to complete a handshake.
    #[norito(default = "SlowlorisConfig::default_max_handshake_millis")]
    pub max_handshake_millis: u64,
    /// Number of timeouts tolerated within a window before penalising.
    #[norito(default = "SlowlorisConfig::default_timeout_threshold")]
    pub timeout_threshold: u32,
    /// Observation window for counting timeouts.
    #[norito(default = "SlowlorisConfig::default_window_secs")]
    pub window_secs: u64,
    /// Penalty duration applied once the threshold is exceeded.
    #[norito(default = "SlowlorisConfig::default_penalty_secs")]
    pub penalty_secs: u64,
}

impl Default for SlowlorisConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_handshake_millis: Self::default_max_handshake_millis(),
            timeout_threshold: Self::default_timeout_threshold(),
            window_secs: Self::default_window_secs(),
            penalty_secs: Self::default_penalty_secs(),
        }
    }
}

impl SlowlorisConfig {
    const fn default_enabled() -> bool {
        true
    }

    const fn default_max_handshake_millis() -> u64 {
        1500
    }

    const fn default_timeout_threshold() -> u32 {
        3
    }

    const fn default_window_secs() -> u64 {
        60
    }

    const fn default_penalty_secs() -> u64 {
        45
    }

    pub fn apply_defaults(&mut self) {
        if self.max_handshake_millis == 0 {
            self.max_handshake_millis = Self::default_max_handshake_millis();
        }
        if self.timeout_threshold == 0 {
            self.timeout_threshold = Self::default_timeout_threshold();
        }
        if self.window_secs == 0 {
            self.window_secs = Self::default_window_secs();
        }
        if self.penalty_secs == 0 {
            self.penalty_secs = Self::default_penalty_secs();
        }
    }
}

/// Stream padding parameters.
#[derive(Debug, Clone, PartialEq, Eq, JsonDeserialize, JsonSerialize)]
pub struct PaddingConfig {
    /// Size of each padding cell in bytes.
    pub cell_size: u16,
    /// Maximum idle time before injecting cover padding.
    pub max_idle_millis: u64,
    /// Optional global egress rate limit for padding bytes per second.
    #[norito(default = "PaddingConfig::default_global_rate_limit_bytes_per_sec")]
    pub global_rate_limit_bytes_per_sec: u64,
    /// Token bucket burst allowance for padding traffic.
    #[norito(default = "PaddingConfig::default_burst_bytes")]
    pub burst_bytes: u64,
}

impl PaddingConfig {
    const IPV6_MIN_MTU_BYTES: u16 = 1_280;
    const UDP_IPV6_OVERHEAD_BYTES: u16 = 48;
    const MIN_NOISE_FRAMING_BYTES: u16 = 96;

    const fn default_global_rate_limit_bytes_per_sec() -> u64 {
        0
    }

    const fn default_burst_bytes() -> u64 {
        0
    }

    const fn mtu_guard_bytes() -> u16 {
        Self::UDP_IPV6_OVERHEAD_BYTES + Self::MIN_NOISE_FRAMING_BYTES
    }

    /// Maximum padding cell payload that still fits below the IPv6 minimum MTU once UDP/Noise framing is applied.
    pub const fn max_cell_size_bytes() -> u16 {
        Self::IPV6_MIN_MTU_BYTES - Self::mtu_guard_bytes()
    }

    /// Clamp a padding cell size to the MTU-safe envelope.
    pub const fn clamp_cell_size(cell_size: u16) -> u16 {
        if cell_size <= Self::max_cell_size_bytes() {
            cell_size
        } else {
            Self::max_cell_size_bytes()
        }
    }

    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.cell_size == 0 {
            return Err(ConfigError::Padding(
                "padding.cell_size must be non-zero".to_string(),
            ));
        }
        let max = Self::max_cell_size_bytes();
        if self.cell_size > max {
            return Err(ConfigError::Padding(format!(
                "padding.cell_size ({}) exceeds MTU-safe limit of {max} bytes (IPv6 MTU {} minus {} bytes reserved for UDP + Norito/Noise framing)",
                self.cell_size,
                Self::IPV6_MIN_MTU_BYTES,
                Self::mtu_guard_bytes(),
            )));
        }
        Ok(())
    }
}

impl Default for PaddingConfig {
    fn default() -> Self {
        Self {
            cell_size: 1024,
            max_idle_millis: 250,
            global_rate_limit_bytes_per_sec: Self::default_global_rate_limit_bytes_per_sec(),
            burst_bytes: Self::default_burst_bytes(),
        }
    }
}

/// Capability advertisement for constant-rate transport lanes.
#[derive(Debug, Clone, PartialEq, Eq, JsonDeserialize, JsonSerialize)]
pub struct ConstantRateCapabilityConfig {
    /// Whether constant-rate support is advertised.
    #[norito(default = "ConstantRateCapabilityConfig::default_enabled")]
    pub enabled: bool,
    /// Protocol version for the capability handshake.
    #[norito(default = "ConstantRateCapabilityConfig::default_version")]
    pub version: u8,
    /// Whether to require peers to honour constant-rate strictly.
    #[norito(default = "ConstantRateCapabilityConfig::default_strict")]
    pub strict: bool,
}

impl ConstantRateCapabilityConfig {
    const fn default_enabled() -> bool {
        false
    }

    const fn default_version() -> u8 {
        1
    }

    const fn default_strict() -> bool {
        false
    }

    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.enabled && self.version != 1 {
            return Err(ConfigError::ConstantRateCapability(format!(
                "constant-rate version {} is not supported",
                self.version
            )));
        }
        Ok(())
    }

    pub fn capability(&self) -> capability::ConstantRateCapability {
        capability::ConstantRateCapability {
            version: self.version,
            mode: if self.strict {
                ConstantRateMode::Strict
            } else {
                ConstantRateMode::BestEffort
            },
            cell_bytes: CONSTANT_RATE_CELL_BYTES as u16,
        }
    }
}

/// Per-client congestion control parameters.
#[derive(Debug, Clone, PartialEq, Eq, JsonDeserialize, JsonSerialize)]
pub struct CongestionConfig {
    /// Maximum concurrent circuits permitted from a single client.
    #[norito(default = "CongestionConfig::default_max_circuits")]
    pub max_circuits_per_client: u32,
    /// Cooldown applied between circuit creations to avoid bursts.
    #[norito(default = "CongestionConfig::default_handshake_cooldown_millis")]
    pub handshake_cooldown_millis: u64,
}

impl Default for CongestionConfig {
    fn default() -> Self {
        Self {
            max_circuits_per_client: Self::default_max_circuits(),
            handshake_cooldown_millis: Self::default_handshake_cooldown_millis(),
        }
    }
}

impl CongestionConfig {
    const fn default_max_circuits() -> u32 {
        8
    }

    const fn default_handshake_cooldown_millis() -> u64 {
        200
    }

    pub fn apply_defaults(&mut self) {
        if self.max_circuits_per_client == 0 {
            self.max_circuits_per_client = Self::default_max_circuits();
        }
        if self.handshake_cooldown_millis == 0 {
            self.handshake_cooldown_millis = Self::default_handshake_cooldown_millis();
        }
    }
}

/// Compliance logging settings.
#[derive(Debug, Clone, PartialEq, Eq, JsonDeserialize, JsonSerialize)]
pub struct ComplianceConfig {
    /// Whether compliance logging is enabled.
    #[norito(default)]
    pub enable: bool,
    /// Path to the compliance log file.
    #[norito(default)]
    pub log_path: Option<PathBuf>,
    /// Optional hex-encoded salt mixed into anonymised hashes.
    #[norito(default)]
    pub hash_salt_hex: Option<String>,
    /// Maximum size of the compliance log before rotation.
    #[norito(default = "ComplianceConfig::default_max_log_bytes")]
    pub max_log_bytes: u64,
    /// Maximum number of rotated compliance log files to keep.
    #[norito(default = "ComplianceConfig::default_max_backup_files")]
    pub max_backup_files: u8,
    /// Optional spool directory for compliance pipeline staging.
    #[norito(default)]
    pub pipeline_spool_dir: Option<PathBuf>,
}

impl Default for ComplianceConfig {
    fn default() -> Self {
        Self {
            enable: false,
            log_path: None,
            hash_salt_hex: None,
            max_log_bytes: Self::default_max_log_bytes(),
            max_backup_files: Self::default_max_backup_files(),
            pipeline_spool_dir: None,
        }
    }
}

impl ComplianceConfig {
    const fn default_max_log_bytes() -> u64 {
        64 * 1024 * 1024
    }

    const fn default_max_backup_files() -> u8 {
        5
    }

    pub fn apply_defaults(&mut self) -> Result<(), ConfigError> {
        if self.enable && self.log_path.is_none() {
            return Err(ConfigError::Compliance(
                "compliance logging enabled but `log_path` not provided".to_string(),
            ));
        }
        if self.max_log_bytes == 0 {
            self.max_log_bytes = Self::default_max_log_bytes();
        }
        if self.max_backup_files == 0 {
            self.max_backup_files = Self::default_max_backup_files();
        }
        Ok(())
    }

    pub fn log_path(&self) -> Option<&Path> {
        self.log_path.as_deref()
    }

    pub fn hash_salt_bytes(&self) -> Result<Option<[u8; 32]>, ConfigError> {
        self.hash_salt_hex
            .as_ref()
            .map(|hex_value| {
                let decoded = hex::decode(hex_value).map_err(|err| ConfigError::Hex {
                    field: "compliance.hash_salt_hex".to_string(),
                    kind: err,
                })?;
                if decoded.len() != 32 {
                    return Err(ConfigError::Compliance(
                        "compliance.hash_salt_hex must decode to 32 bytes".to_string(),
                    ));
                }
                let mut salt = [0u8; 32];
                salt.copy_from_slice(&decoded);
                Ok(salt)
            })
            .transpose()
    }

    pub fn pipeline_spool_dir(&self) -> Option<&Path> {
        self.pipeline_spool_dir.as_deref()
    }
}

/// Advertised KEM capability in the handshake policy.
#[derive(Debug, Clone, JsonDeserialize, JsonSerialize)]
pub struct KemPolicyEntry {
    /// Identifier of the KEM suite (e.g., `ml-kem-768`).
    pub id: String,
    /// Whether this KEM must be supported by peers.
    #[norito(default)]
    pub required: bool,
}

/// Advertised signature capability in the handshake policy.
#[derive(Debug, Clone, JsonDeserialize, JsonSerialize)]
pub struct SignaturePolicyEntry {
    /// Identifier of the signature suite (e.g., `dilithium3`).
    pub id: String,
    /// Whether this signature suite must be supported by peers.
    #[norito(default)]
    pub required: bool,
}

/// GREASE TLV entry that the relay appends to handshake responses.
#[derive(Debug, Clone, JsonDeserialize, JsonSerialize)]
pub struct GreasePolicyEntry {
    /// TLV type reserved for GREASE.
    pub typ: u16,
    /// Hex-encoded GREASE payload value.
    pub value_hex: String,
}

/// Handshake policy describing relay capabilities.
#[derive(Debug, Clone, JsonDeserialize, JsonSerialize)]
pub struct HandshakePolicy {
    /// KEM suites advertised during capability negotiation.
    #[norito(default)]
    pub kem: Vec<KemPolicyEntry>,
    /// Signature suites advertised during capability negotiation.
    #[norito(default)]
    pub signatures: Vec<SignaturePolicyEntry>,
    /// Descriptor commit hash expected in peer advertisements.
    #[norito(default)]
    pub descriptor_commit_hex: Option<String>,
    /// Extra GREASE TLVs appended to handshake responses.
    #[norito(default)]
    pub grease: Vec<GreasePolicyEntry>,
    /// Optional Ed25519 private key to override the manifest identity.
    #[norito(default)]
    pub identity_private_key_hex: Option<String>,
    /// Optional path to a descriptor manifest for key extraction.
    #[norito(default)]
    pub descriptor_manifest_path: Option<PathBuf>,
    /// Optional certificate validation parameters.
    #[norito(default)]
    pub certificate: Option<CertificateConfig>,
}

/// Validation phase setting accepted from configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CertificateValidationPhaseSetting {
    Phase1AllowSingle,
    Phase2PreferDual,
    Phase3RequireDual,
}

impl CertificateValidationPhaseSetting {
    const fn default_phase() -> Self {
        Self::Phase2PreferDual
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Phase1AllowSingle => "phase1_allow_single",
            Self::Phase2PreferDual => "phase2_prefer_dual",
            Self::Phase3RequireDual => "phase3_require_dual",
        }
    }

    fn into_runtime(self) -> CertificateValidationPhase {
        match self {
            Self::Phase1AllowSingle => CertificateValidationPhase::Phase1AllowSingle,
            Self::Phase2PreferDual => CertificateValidationPhase::Phase2PreferDual,
            Self::Phase3RequireDual => CertificateValidationPhase::Phase3RequireDual,
        }
    }
}

impl Default for CertificateValidationPhaseSetting {
    fn default() -> Self {
        Self::default_phase()
    }
}

impl FromStr for CertificateValidationPhaseSetting {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "phase1_allow_single" | "phase1" | "allow_single" => Ok(Self::Phase1AllowSingle),
            "phase2_prefer_dual" | "phase2" | "prefer_dual" => Ok(Self::Phase2PreferDual),
            "phase3_require_dual" | "phase3" | "require_dual" => Ok(Self::Phase3RequireDual),
            other => Err(format!(
                "unknown certificate validation phase `{other}` (expected phase1_allow_single, phase2_prefer_dual, or phase3_require_dual)"
            )),
        }
    }
}

impl json::JsonSerialize for CertificateValidationPhaseSetting {
    fn json_serialize(&self, out: &mut String) {
        json::write_json_string(self.as_str(), out);
    }
}

impl json::JsonDeserialize for CertificateValidationPhaseSetting {
    fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
        let value = parser.parse_string()?;
        CertificateValidationPhaseSetting::from_str(&value).map_err(json::Error::Message)
    }
}

/// Certificate verification configuration.
#[derive(Debug, Clone, JsonDeserialize, JsonSerialize)]
pub struct CertificateConfig {
    /// Path to the Norito-encoded certificate bundle.
    pub bundle_path: PathBuf,
    /// Hex-encoded Ed25519 issuer public key expected in the bundle.
    pub issuer_ed25519_hex: String,
    /// Optional hex-encoded ML-DSA issuer public key expected in the bundle.
    #[norito(default)]
    pub issuer_mldsa_hex: Option<String>,
    /// Certificate validation phase enforced by the relay.
    #[norito(default = "CertificateValidationPhaseSetting::default_phase")]
    pub validation_phase: CertificateValidationPhaseSetting,
}

impl CertificateConfig {
    fn validate(&self) -> Result<(), ConfigError> {
        let _ = hex::decode(&self.issuer_ed25519_hex).map_err(|kind| ConfigError::Hex {
            field: "handshake.certificate.issuer_ed25519_hex".to_string(),
            kind,
        })?;
        if let Some(hex) = &self.issuer_mldsa_hex {
            let _ = hex::decode(hex).map_err(|kind| ConfigError::Hex {
                field: "handshake.certificate.issuer_mldsa_hex".to_string(),
                kind,
            })?;
        }
        Ok(())
    }

    fn load_bundle(&self) -> Result<RelayCertificateBundleV2, ConfigError> {
        let bytes = fs::read(&self.bundle_path).map_err(|err| ConfigError::Certificate {
            path: self.bundle_path.clone(),
            message: format!("failed to read certificate bundle: {err}"),
        })?;
        RelayCertificateBundleV2::from_cbor(&bytes).map_err(|err| ConfigError::Certificate {
            path: self.bundle_path.clone(),
            message: format!("failed to parse certificate bundle: {err}"),
        })
    }

    fn parse_issuer_ed25519(&self) -> Result<VerifyingKey, ConfigError> {
        let bytes = hex::decode(&self.issuer_ed25519_hex).map_err(|kind| ConfigError::Hex {
            field: "handshake.certificate.issuer_ed25519_hex".to_string(),
            kind,
        })?;
        let array: [u8; 32] =
            bytes
                .as_slice()
                .try_into()
                .map_err(|_| ConfigError::Certificate {
                    path: self.bundle_path.clone(),
                    message: "issuer_ed25519_hex must decode to 32 bytes".to_string(),
                })?;
        VerifyingKey::from_bytes(&array).map_err(|err| ConfigError::Certificate {
            path: self.bundle_path.clone(),
            message: format!("failed to parse issuer Ed25519 key: {err}"),
        })
    }

    fn parse_issuer_mldsa(&self) -> Result<Option<Vec<u8>>, ConfigError> {
        match &self.issuer_mldsa_hex {
            Some(hex) => {
                let bytes = hex::decode(hex).map_err(|kind| ConfigError::Hex {
                    field: "handshake.certificate.issuer_mldsa_hex".to_string(),
                    kind,
                })?;
                Ok(Some(bytes))
            }
            None => Ok(None),
        }
    }
}

/// ML-KEM keypair decoded from the descriptor manifest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MlKemKeys {
    /// ML-KEM-768 public key bytes.
    pub public: Vec<u8>,
    /// ML-KEM-768 secret key bytes.
    pub private: Vec<u8>,
}

#[derive(Debug, Clone)]
pub(crate) struct ManifestSecrets {
    pub(crate) identity_private_key: Option<[u8; 32]>,
    pub(crate) ml_kem_private_key: Option<Vec<u8>>,
    pub(crate) ml_kem_public_key: Option<Vec<u8>>,
}

impl Default for HandshakePolicy {
    fn default() -> Self {
        Self {
            kem: vec![KemPolicyEntry {
                id: "ml-kem-768".to_string(),
                required: true,
            }],
            signatures: vec![SignaturePolicyEntry {
                id: "dilithium3".to_string(),
                required: true,
            }],
            descriptor_commit_hex: None,
            grease: vec![
                GreasePolicyEntry {
                    typ: 0x7F10,
                    value_hex: "deadbeef".to_string(),
                },
                GreasePolicyEntry {
                    typ: 0x7F11,
                    value_hex: "cafebabe".to_string(),
                },
            ],
            identity_private_key_hex: None,
            descriptor_manifest_path: None,
            certificate: None,
        }
    }
}

impl HandshakePolicy {
    pub fn apply_defaults(&mut self) {
        if self.kem.is_empty() {
            self.kem = HandshakePolicy::default().kem;
        }
        if self.signatures.is_empty() {
            self.signatures = HandshakePolicy::default().signatures;
        }
        if self.grease.is_empty() {
            self.grease = HandshakePolicy::default().grease;
        }
    }

    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.kem.is_empty() {
            return Err(ConfigError::Handshake(
                "handshake.kem list cannot be empty".to_string(),
            ));
        }
        if self.signatures.is_empty() {
            return Err(ConfigError::Handshake(
                "handshake.signatures list cannot be empty".to_string(),
            ));
        }
        for entry in &self.kem {
            if parse_kem_id(&entry.id).is_none() {
                return Err(ConfigError::Handshake(format!(
                    "unknown KEM identifier `{}`",
                    entry.id
                )));
            }
        }
        for entry in &self.signatures {
            if parse_signature_id(&entry.id).is_none() {
                return Err(ConfigError::Handshake(format!(
                    "unknown signature identifier `{}`",
                    entry.id
                )));
            }
        }
        if let Some(hex) = &self.descriptor_commit_hex {
            let _ = capability::parse_descriptor_commit_hex(hex).map_err(|_| {
                ConfigError::Handshake("descriptor_commit_hex must decode to 32 bytes".to_string())
            })?;
        }
        for grease in &self.grease {
            if !(0x7F00..=0x7FFF).contains(&grease.typ) {
                return Err(ConfigError::Handshake(format!(
                    "GREASE type {:04x} outside reserved range 0x7F00-0x7FFF",
                    grease.typ
                )));
            }
            hex::decode(&grease.value_hex).map_err(|err| ConfigError::Hex {
                field: format!("handshake.grease[{:#06x}]", grease.typ),
                kind: err,
            })?;
        }
        if let Some(identity_hex) = &self.identity_private_key_hex {
            let decoded = hex::decode(identity_hex).map_err(|err| ConfigError::Hex {
                field: "handshake.identity_private_key_hex".to_string(),
                kind: err,
            })?;
            if decoded.len() != 32 {
                return Err(ConfigError::Handshake(
                    "identity_private_key_hex must decode to 32 bytes".to_string(),
                ));
            }
        }
        if let Some(certificate) = &self.certificate {
            certificate.validate()?;
        }
        Ok(())
    }

    pub fn descriptor_commit_bytes(&self) -> Result<Option<[u8; 32]>, ConfigError> {
        self.descriptor_commit_hex
            .as_ref()
            .map(|hex| {
                capability::parse_descriptor_commit_hex(hex).map_err(|_| {
                    ConfigError::Handshake(
                        "descriptor_commit_hex must decode to 32 bytes".to_string(),
                    )
                })
            })
            .transpose()
    }

    pub fn grease_entries(&self) -> Result<Vec<GreaseEntry>, ConfigError> {
        let mut entries = Vec::with_capacity(self.grease.len());
        for g in &self.grease {
            let value = hex::decode(&g.value_hex).map_err(|err| ConfigError::Hex {
                field: format!("handshake.grease[{:#06x}]", g.typ),
                kind: err,
            })?;
            entries.push(GreaseEntry { ty: g.typ, value });
        }
        Ok(entries)
    }

    pub fn identity_private_key_bytes(&self) -> Result<Option<[u8; 32]>, ConfigError> {
        self.identity_private_key_hex
            .as_ref()
            .map(|hex| {
                let decoded = hex::decode(hex).map_err(|err| ConfigError::Hex {
                    field: "handshake.identity_private_key_hex".to_string(),
                    kind: err,
                })?;
                if decoded.len() != 32 {
                    return Err(ConfigError::Handshake(
                        "identity_private_key_hex must decode to 32 bytes".to_string(),
                    ));
                }
                let mut key = [0u8; 32];
                key.copy_from_slice(&decoded);
                Ok(key)
            })
            .transpose()
    }

    pub fn load_certificate_bundle(&self) -> Result<Option<RelayCertificateBundleV2>, ConfigError> {
        let Some(certificate) = &self.certificate else {
            return Ok(None);
        };

        let bundle = certificate.load_bundle()?;
        let issuer_ed25519 = certificate.parse_issuer_ed25519()?;
        let issuer_mldsa = certificate.parse_issuer_mldsa()?;
        let validation_phase = certificate.validation_phase.into_runtime();

        let mldsa_bytes = match (issuer_mldsa, validation_phase) {
            (Some(bytes), _) => bytes,
            (None, CertificateValidationPhase::Phase1AllowSingle) => Vec::new(),
            (None, CertificateValidationPhase::Phase2PreferDual)
            | (None, CertificateValidationPhase::Phase3RequireDual) => {
                return Err(ConfigError::Handshake(
                    "handshake.certificate.issuer_mldsa_hex must be provided for dual-signature validation phases"
                        .to_string(),
                ));
            }
        };

        bundle
            .verify(&issuer_ed25519, mldsa_bytes.as_slice(), validation_phase)
            .map_err(|err| ConfigError::Certificate {
                path: certificate.bundle_path.clone(),
                message: format!("certificate verification failed: {err}"),
            })?;

        Ok(Some(bundle))
    }

    pub fn descriptor_manifest_path(&self) -> Option<&Path> {
        self.descriptor_manifest_path.as_deref()
    }

    pub(crate) fn manifest_secrets(&self) -> Result<Option<ManifestSecrets>, ConfigError> {
        let Some(path) = self.descriptor_manifest_path() else {
            return Ok(None);
        };

        let bytes = fs::read(path).map_err(|err| {
            manifest_error(path, format!("failed to read descriptor manifest: {err}"))
        })?;

        let value: norito::json::Value = norito::json::from_slice(&bytes).map_err(|err| {
            manifest_error(
                path,
                format!("failed to parse descriptor manifest JSON: {err}"),
            )
        })?;

        let identity_private_key = extract_manifest_identity_private_key(&value)
            .map(|hex| {
                decode_manifest_identity_seed(hex).map_err(|message| manifest_error(path, message))
            })
            .transpose()?;

        let (private_hex, public_hex) = extract_manifest_ml_kem_hex(&value);
        let private_hex = private_hex.map(str::trim).filter(|value| !value.is_empty());
        let public_hex = public_hex.map(str::trim).filter(|value| !value.is_empty());

        let (ml_kem_private_key, ml_kem_public_key) = match (private_hex, public_hex) {
            (None, None) => (None, None),
            (Some(private), Some(public)) => {
                let private = decode_manifest_ml_kem_key(
                    private,
                    ML_KEM_768_SECRET_LEN,
                    "ml_kem_private_key_hex",
                )
                .map_err(|message| manifest_error(path, message))?;
                let public =
                    decode_manifest_ml_kem_key(public, ML_KEM_768_PUBLIC_LEN, "ml_kem_public_hex")
                        .map_err(|message| manifest_error(path, message))?;
                (Some(private), Some(public))
            }
            _ => {
                return Err(manifest_error(
                    path,
                    "descriptor manifest must provide both ml_kem_private_key_hex and ml_kem_public_hex when configuring ML-KEM identity material",
                ));
            }
        };

        Ok(Some(ManifestSecrets {
            identity_private_key,
            ml_kem_private_key,
            ml_kem_public_key,
        }))
    }

    pub fn ml_kem_keys_from_manifest(&self) -> Result<Option<MlKemKeys>, ConfigError> {
        Ok(self.manifest_secrets()?.and_then(|secrets| {
            match (secrets.ml_kem_public_key, secrets.ml_kem_private_key) {
                (Some(public), Some(private)) => Some(MlKemKeys { public, private }),
                _ => None,
            }
        }))
    }

    pub fn identity_private_key_from_manifest(&self) -> Result<Option<[u8; 32]>, ConfigError> {
        match self.manifest_secrets()? {
            Some(ManifestSecrets {
                identity_private_key: Some(seed),
                ..
            }) => Ok(Some(seed)),
            Some(_) => {
                let path = self
                    .descriptor_manifest_path()
                    .expect("descriptor manifest path available when secrets loaded");
                Err(manifest_error(
                    path,
                    "descriptor manifest missing identity private key",
                ))
            }
            None => Ok(None),
        }
    }
}

fn manifest_error(path: &Path, message: impl Into<String>) -> ConfigError {
    ConfigError::DescriptorManifest {
        path: path.to_path_buf(),
        message: message.into(),
    }
}

fn decode_manifest_identity_seed(hex_value: &str) -> Result<[u8; 32], String> {
    let decoded =
        hex::decode(hex_value).map_err(|err| format!("identity private key hex invalid: {err}"))?;
    if decoded.len() != 32 {
        return Err(format!(
            "identity private key hex must decode to 32 bytes (got {})",
            decoded.len()
        ));
    }
    let mut seed = [0u8; 32];
    seed.copy_from_slice(&decoded);
    Ok(seed)
}

fn decode_manifest_ml_kem_key(
    hex_value: &str,
    expected_len: usize,
    field: &str,
) -> Result<Vec<u8>, String> {
    let decoded = hex::decode(hex_value).map_err(|err| format!("{field} invalid: {err}"))?;
    if decoded.len() != expected_len {
        return Err(format!(
            "{field} must decode to {expected_len} bytes (got {})",
            decoded.len()
        ));
    }
    Ok(decoded)
}

fn extract_manifest_identity_private_key(value: &norito::json::Value) -> Option<&str> {
    use norito::json::Value;

    match value {
        Value::Object(map) => {
            if let Some(hex) = map.get("identity_private_key_hex").and_then(Value::as_str) {
                return Some(hex);
            }
            if let Some(identity) = map.get("identity").and_then(Value::as_object) {
                if let Some(hex) = identity
                    .get("ed25519_private_key_hex")
                    .and_then(Value::as_str)
                {
                    return Some(hex);
                }
                if let Some(hex) = identity.get("private_key_hex").and_then(Value::as_str) {
                    return Some(hex);
                }
            }
            if let Some(relay) = map.get("relay").and_then(Value::as_object)
                && let Some(identity) = relay.get("identity").and_then(Value::as_object)
            {
                if let Some(hex) = identity
                    .get("ed25519_private_key_hex")
                    .and_then(Value::as_str)
                {
                    return Some(hex);
                }
                if let Some(hex) = identity.get("private_key_hex").and_then(Value::as_str) {
                    return Some(hex);
                }
            }
            for child in map.values() {
                if let Some(hex) = extract_manifest_identity_private_key(child) {
                    return Some(hex);
                }
            }
            None
        }
        Value::Array(array) => array
            .iter()
            .find_map(|item| extract_manifest_identity_private_key(item)),
        _ => None,
    }
}

fn extract_manifest_ml_kem_hex(value: &norito::json::Value) -> (Option<&str>, Option<&str>) {
    use norito::json::Value;

    match value {
        Value::Object(map) => {
            let mut private = map.get("ml_kem_private_key_hex").and_then(Value::as_str);
            let mut public = map.get("ml_kem_public_hex").and_then(Value::as_str);
            if let Some(identity) = map.get("identity").and_then(Value::as_object) {
                if private.is_none() {
                    private = identity
                        .get("ml_kem_private_key_hex")
                        .and_then(Value::as_str);
                }
                if public.is_none() {
                    public = identity.get("ml_kem_public_hex").and_then(Value::as_str);
                }
            }
            if private.is_some() || public.is_some() {
                return (private, public);
            }
            for child in map.values() {
                let (child_private, child_public) = extract_manifest_ml_kem_hex(child);
                if child_private.is_some() || child_public.is_some() {
                    return (child_private, child_public);
                }
            }
            (None, None)
        }
        Value::Array(array) => {
            for item in array {
                let (child_private, child_public) = extract_manifest_ml_kem_hex(item);
                if child_private.is_some() || child_public.is_some() {
                    return (child_private, child_public);
                }
            }
            (None, None)
        }
        _ => (None, None),
    }
}

/// Relay configuration structure loaded from a JSON file.
#[derive(Debug, Clone, JsonDeserialize, JsonSerialize)]
pub struct RelayConfig {
    /// Operating mode for this relay (entry, middle, or exit).
    pub mode: RelayMode,
    /// QUIC listen address (host:port).
    pub listen: String,
    /// Optional admin HTTP interface address.
    pub admin_listen: Option<String>,
    /// TLS settings for the QUIC endpoint.
    pub tls: Option<TlsConfig>,
    /// Proof-of-work admission configuration.
    pub pow: Option<PowConfig>,
    /// Stream padding defaults.
    pub padding: Option<PaddingConfig>,
    /// Handshake policy describing advertised capabilities.
    pub handshake: Option<HandshakePolicy>,
    /// Per-client congestion control settings.
    pub congestion: Option<CongestionConfig>,
    /// Compliance logging configuration.
    pub compliance: Option<ComplianceConfig>,
    /// Incentive logging configuration.
    pub incentives: Option<IncentiveLogConfig>,
    /// Exit routing configuration for Norito/Kaigi.
    #[norito(default)]
    pub exit_routing: ExitRoutingConfig,
    /// Optional VPN overlay configuration.
    #[norito(default)]
    pub vpn: Option<VpnConfig>,
    /// Privacy telemetry configuration.
    pub privacy: Option<PrivacyTelemetryConfig>,
    /// Guard directory validation configuration.
    #[norito(default)]
    pub guard_directory: Option<GuardDirectoryConfig>,
    /// Constant-rate capability advertisement settings.
    #[norito(default)]
    pub constant_rate_capability: Option<ConstantRateCapabilityConfig>,
    /// Constant-rate profile name to advertise.
    #[norito(default)]
    pub constant_rate_profile: ConstantRateProfileName,
}

impl RelayConfig {
    pub fn load(path: impl AsRef<Path>) -> Result<Self, ConfigError> {
        let data = fs::read(path.as_ref())?;
        let mut config: RelayConfig = json::from_slice(&data)?;
        config.validate()?;
        Ok(config)
    }

    pub fn validate(&mut self) -> Result<(), ConfigError> {
        let tls = self.tls.get_or_insert_with(TlsConfig::default);
        tls.validate()?;

        self.listen_addr()?;
        if self.admin_listen.is_some() {
            self.admin_addr()?;
        }

        if self.pow.is_none() {
            self.pow = Some(PowConfig::default());
        }
        if let Some(pow) = self.pow.as_mut() {
            pow.apply_defaults()?;
        }
        if self.padding.is_none() {
            self.padding = Some(PaddingConfig::default());
        }
        if let Some(padding) = self.padding.as_ref() {
            padding.validate()?;
        }
        if self.congestion.is_none() {
            self.congestion = Some(CongestionConfig::default());
        }
        if let Some(congestion) = self.congestion.as_mut() {
            congestion.apply_defaults();
        }
        if self.compliance.is_none() {
            self.compliance = Some(ComplianceConfig::default());
        }
        if let Some(compliance) = self.compliance.as_mut() {
            compliance.apply_defaults()?;
        }
        if self.incentives.is_none() {
            self.incentives = Some(IncentiveLogConfig::default());
        }
        if let Some(incentives) = self.incentives.as_mut() {
            incentives.apply_defaults();
        }
        if self.handshake.is_none() {
            self.handshake = Some(HandshakePolicy::default());
        }
        if let Some(policy) = self.handshake.as_mut() {
            policy.apply_defaults();
            policy.validate()?;
        }
        if self.privacy.is_none() {
            self.privacy = Some(PrivacyTelemetryConfig::default());
        }
        if let Some(privacy) = self.privacy.as_mut() {
            privacy.apply_defaults()?;
        }
        if let Some(guard) = self.guard_directory.as_mut() {
            guard.apply_defaults()?;
        }
        if let Some(constant_rate) = self.constant_rate_capability.as_ref() {
            constant_rate.validate()?;
        }
        self.exit_routing.validate()?;
        if let Some(vpn) = self.vpn.as_mut() {
            vpn.validate()?;
        }

        Ok(())
    }

    pub fn listen_addr(&self) -> Result<SocketAddr, ConfigError> {
        SocketAddr::from_str(&self.listen)
            .map_err(|_| ConfigError::InvalidAddress("listen".to_string(), self.listen.clone()))
    }

    pub fn admin_addr(&self) -> Result<Option<SocketAddr>, ConfigError> {
        match &self.admin_listen {
            Some(value) => SocketAddr::from_str(value).map(Some).map_err(|_| {
                ConfigError::InvalidAddress("admin_listen".to_string(), value.clone())
            }),
            None => Ok(None),
        }
    }

    pub fn certificate_path(&self) -> Option<&Path> {
        self.tls
            .as_ref()
            .and_then(|tls| tls.certificate_path.as_deref())
    }

    pub fn private_key_path(&self) -> Option<&Path> {
        self.tls
            .as_ref()
            .and_then(|tls| tls.private_key_path.as_deref())
    }

    pub fn guard_directory_config(&self) -> Option<&GuardDirectoryConfig> {
        self.guard_directory.as_ref()
    }

    pub fn self_signed_subject(&self) -> &str {
        self.tls
            .as_ref()
            .map(TlsConfig::subject_or_default)
            .unwrap_or(DEFAULT_SELF_SIGNED_SUBJECT)
    }

    pub fn pow_config(&self) -> &PowConfig {
        self.pow
            .as_ref()
            .expect("pow defaults applied during validation")
    }

    pub fn padding_config(&self) -> &PaddingConfig {
        self.padding
            .as_ref()
            .expect("padding defaults applied during validation")
    }

    #[allow(dead_code)]
    pub fn congestion_config(&self) -> &CongestionConfig {
        self.congestion
            .as_ref()
            .expect("congestion defaults applied during validation")
    }

    pub fn compliance_config(&self) -> &ComplianceConfig {
        self.compliance
            .as_ref()
            .expect("compliance defaults applied during validation")
    }

    pub fn handshake_policy(&self) -> &HandshakePolicy {
        self.handshake
            .as_ref()
            .expect("handshake defaults applied during validation")
    }

    pub fn incentive_log_config(&self) -> &IncentiveLogConfig {
        self.incentives
            .as_ref()
            .expect("incentive defaults applied during validation")
    }

    pub fn exit_routing_config(&self) -> &ExitRoutingConfig {
        &self.exit_routing
    }

    pub fn vpn_config(&self) -> Option<&VpnConfig> {
        self.vpn.as_ref()
    }

    pub fn privacy_config(&self) -> &PrivacyTelemetryConfig {
        self.privacy
            .as_ref()
            .expect("privacy defaults applied during validation")
    }

    pub fn constant_rate_profile(&self) -> ConstantRateProfileName {
        self.constant_rate_profile
    }

    pub fn constant_rate_capability(&self) -> Option<capability::ConstantRateCapability> {
        self.constant_rate_capability
            .as_ref()
            .filter(|cfg| cfg.enabled)
            .map(ConstantRateCapabilityConfig::capability)
    }
}

/// Errors surfaced while parsing or validating configuration.
#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("failed to parse configuration JSON: {0}")]
    Json(#[from] json::Error),
    #[error("invalid socket address for `{0}`: `{1}`")]
    InvalidAddress(String, String),
    #[error("invalid TLS configuration: {0}")]
    TlsPaths(String),
    #[error("invalid hex value for `{field}`: {kind}")]
    Hex { field: String, kind: FromHexError },
    #[error("handshake configuration error: {0}")]
    Handshake(String),
    #[error("descriptor manifest error at `{path}`: {message}")]
    DescriptorManifest { path: PathBuf, message: String },
    #[error("certificate error at `{path}`: {message}")]
    Certificate { path: PathBuf, message: String },
    #[error("compliance configuration error: {0}")]
    Compliance(String),
    #[error("puzzle configuration error: {0}")]
    Puzzle(String),
    #[error("token configuration error: {0}")]
    Token(String),
    #[error("exit routing configuration error: {0}")]
    Routing(String),
    #[error("replay filter configuration error: {0}")]
    ReplayFilter(String),
    #[error("emergency throttle configuration error: {0}")]
    EmergencyThrottle(String),
    #[error("privacy telemetry configuration error: {0}")]
    Privacy(String),
    #[error("padding configuration error: {0}")]
    Padding(String),
    #[error("constant-rate capability configuration error: {0}")]
    ConstantRateCapability(String),
    #[error("guard directory configuration error: {0}")]
    GuardDirectory(String),
    #[error("vpn configuration error: {0}")]
    Vpn(String),
}

pub(crate) fn parse_kem_id(id: &str) -> Option<capability::KemId> {
    match id {
        "classic" => Some(capability::KemId::Classic),
        "ml-kem-768" => Some(capability::KemId::MlKem768),
        "ml-kem-1024" => Some(capability::KemId::MlKem1024),
        _ => None,
    }
}

pub(crate) fn parse_signature_id(id: &str) -> Option<capability::SignatureId> {
    match id {
        "ed25519" => Some(capability::SignatureId::Ed25519),
        "dilithium3" => Some(capability::SignatureId::Dilithium3),
        "falcon512" => Some(capability::SignatureId::Falcon512),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use hex::FromHex;
    use tempfile::NamedTempFile;

    use super::*;
    use crate::vpn::VpnOverlay;

    fn write_config(json: &str) -> PathBuf {
        let file = NamedTempFile::new().expect("create temp file");
        std::fs::write(file.path(), json).expect("write config");
        file.into_temp_path().keep().expect("persist temp file")
    }

    fn write_manifest(json: &str) -> NamedTempFile {
        let file = NamedTempFile::new().expect("create manifest file");
        std::fs::write(file.path(), json).expect("write manifest");
        file
    }

    #[test]
    fn manifest_with_ml_kem_keys_loads() {
        let private_hex = "aa".repeat(ML_KEM_768_SECRET_LEN);
        let public_hex = "bb".repeat(ML_KEM_768_PUBLIC_LEN);
        let manifest_json = format!(
            r#"{{
                "version": 1,
                "identity": {{
                    "ed25519_private_key_hex": "{seed}",
                    "ml_kem_private_key_hex": "{private_hex}",
                    "ml_kem_public_hex": "{public_hex}"
                }}
            }}"#,
            seed = "11".repeat(32),
            private_hex = private_hex,
            public_hex = public_hex,
        );
        let manifest = write_manifest(&manifest_json);
        let manifest_path = manifest.path().display().to_string();
        let manifest_json_path = format!("{manifest_path:?}");
        let config_json = format!(
            r#"{{
                "mode": "Entry",
                "listen": "127.0.0.1:0",
                "handshake": {{ "descriptor_manifest_path": {manifest_json_path} }}
            }}"#
        );
        let config_path = write_config(&config_json);
        let cfg = RelayConfig::load(config_path).expect("load config");
        let policy = cfg.handshake_policy();

        let secrets = policy
            .manifest_secrets()
            .expect("manifest secrets")
            .expect("secrets expected");
        assert!(secrets.identity_private_key.is_some());
        assert_eq!(
            secrets.ml_kem_private_key.as_ref().map(Vec::len),
            Some(ML_KEM_768_SECRET_LEN)
        );
        assert_eq!(
            secrets.ml_kem_public_key.as_ref().map(Vec::len),
            Some(ML_KEM_768_PUBLIC_LEN)
        );

        let ml_kem = policy
            .ml_kem_keys_from_manifest()
            .expect("ml-kem keys")
            .expect("ml-kem keypair expected");
        assert_eq!(ml_kem.private.len(), ML_KEM_768_SECRET_LEN);
        assert_eq!(ml_kem.public.len(), ML_KEM_768_PUBLIC_LEN);
    }

    #[test]
    fn manifest_requires_complete_ml_kem_pair() {
        let manifest_json = format!(
            r#"{{
                "version": 1,
                "identity": {{
                    "ed25519_private_key_hex": "{seed}",
                    "ml_kem_public_hex": "{public_hex}"
                }}
            }}"#,
            seed = "22".repeat(32),
            public_hex = "cc".repeat(ML_KEM_768_PUBLIC_LEN),
        );
        let manifest = write_manifest(&manifest_json);
        let manifest_path = manifest.path().display().to_string();
        let manifest_json_path = format!("{manifest_path:?}");
        let config_json = format!(
            r#"{{
                "mode": "Entry",
                "listen": "127.0.0.1:0",
                "handshake": {{ "descriptor_manifest_path": {manifest_json_path} }}
            }}"#
        );
        let config_path = write_config(&config_json);
        let cfg = RelayConfig::load(config_path).expect("load config");
        let policy = cfg.handshake_policy();

        let err = policy
            .ml_kem_keys_from_manifest()
            .expect_err("expected manifest error");
        match err {
            ConfigError::DescriptorManifest { message, .. } => {
                assert!(
                    message.contains("ml_kem_private_key_hex")
                        && message.contains("ml_kem_public_hex"),
                    "unexpected error message: {message}"
                );
            }
            other => panic!("unexpected error {other:?}"),
        }
    }

    #[test]
    fn load_self_signed_config() {
        let json = r#"
            {
                "mode": "Entry",
                "listen": "127.0.0.1:0",
                "admin_listen": "127.0.0.1:0",
                "tls": {},
                "pow": { "required": true, "difficulty": 18 },
                "padding": { "cell_size": 1024, "max_idle_millis": 150 }
            }
        "#;
        let path = write_config(json);
        let cfg = RelayConfig::load(path).expect("load config");

        assert_eq!(cfg.mode, RelayMode::Entry);
        assert_eq!(cfg.listen_addr().unwrap().port(), 0);
        assert!(cfg.pow_config().required);
        assert_eq!(cfg.pow_config().difficulty, 18);
        assert_eq!(cfg.pow_config().max_future_skew_secs, 300);
        assert_eq!(cfg.pow_config().min_ticket_ttl_secs, 30);
        assert_eq!(cfg.self_signed_subject(), DEFAULT_SELF_SIGNED_SUBJECT);
        assert_eq!(cfg.padding_config().cell_size, 1024);
        assert_eq!(cfg.padding_config().max_idle_millis, 150);
        assert_eq!(
            cfg.padding_config().global_rate_limit_bytes_per_sec,
            PaddingConfig::default_global_rate_limit_bytes_per_sec()
        );
        assert_eq!(
            cfg.padding_config().burst_bytes,
            PaddingConfig::default_burst_bytes()
        );
        assert_eq!(cfg.congestion_config().max_circuits_per_client, 8);
        assert_eq!(cfg.congestion_config().handshake_cooldown_millis, 200);
        assert!(!cfg.compliance_config().enable);
        assert_eq!(cfg.compliance_config().max_log_bytes, 64 * 1024 * 1024);
        assert_eq!(cfg.compliance_config().max_backup_files, 5);
        assert!(cfg.compliance_config().pipeline_spool_dir().is_none());
        assert!(!cfg.incentive_log_config().enable);
        assert!(cfg.incentive_log_config().spool_dir.is_none());
    }

    #[test]
    fn padding_cell_size_must_be_non_zero() {
        let json = r#"
            {
                "mode": "Middle",
                "listen": "127.0.0.1:0",
                "pow": { "required": true, "difficulty": 18 },
                "padding": { "cell_size": 0, "max_idle_millis": 200 }
            }
        "#;
        let path = write_config(json);
        let err = RelayConfig::load(path).expect_err("validation must fail");
        match err {
            ConfigError::Padding(message) => {
                assert!(
                    message.contains("non-zero"),
                    "unexpected padding error: {message}"
                );
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn padding_cell_size_must_fit_ipv6_mtu() {
        let max = PaddingConfig::max_cell_size_bytes();
        let invalid = u32::from(max) + 1;
        let json = format!(
            r#"{{
                "mode": "Middle",
                "listen": "127.0.0.1:0",
                "pow": {{ "required": true, "difficulty": 18 }},
                "padding": {{ "cell_size": {invalid}, "max_idle_millis": 200 }}
            }}"#
        );
        let path = write_config(&json);
        let err = RelayConfig::load(path).expect_err("validation must fail");
        match err {
            ConfigError::Padding(message) => {
                assert!(
                    message.contains("MTU-safe"),
                    "unexpected padding error: {message}"
                );
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn padding_cell_size_clamps_to_limit() {
        let max = PaddingConfig::max_cell_size_bytes();
        assert_eq!(PaddingConfig::clamp_cell_size(max), max);
        let invalid = max.saturating_add(1);
        assert_eq!(PaddingConfig::clamp_cell_size(invalid), max);
        assert_eq!(PaddingConfig::clamp_cell_size(0), 0);
    }

    #[test]
    fn constant_rate_capability_rejects_future_versions() {
        let json = r#"
            {
                "mode": "Entry",
                "listen": "127.0.0.1:0",
                "pow": { "required": true, "difficulty": 18 },
                "constant_rate_capability": { "enabled": true, "version": 2, "strict": true }
            }
        "#;
        let path = write_config(json);
        let err = RelayConfig::load(path).expect_err("validation must fail");
        match err {
            ConfigError::ConstantRateCapability(message) => {
                assert!(
                    message.contains("version"),
                    "unexpected constant-rate capability error: {message}"
                );
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn constant_rate_capability_returns_none_when_disabled() {
        let json = r#"
            {
                "mode": "Entry",
                "listen": "127.0.0.1:0",
                "pow": { "required": true, "difficulty": 18 },
                "constant_rate_capability": { "enabled": false, "strict": true }
            }
        "#;
        let path = write_config(json);
        let cfg = RelayConfig::load(path).expect("load config");
        assert!(cfg.constant_rate_capability().is_none());
    }

    #[test]
    fn incentive_log_defaults_when_enabled() {
        let mut cfg = RelayConfig {
            mode: RelayMode::Entry,
            listen: "127.0.0.1:0".to_owned(),
            admin_listen: None,
            tls: None,
            pow: None,
            padding: None,
            handshake: None,
            congestion: None,
            compliance: None,
            incentives: Some(IncentiveLogConfig {
                enable: true,
                spool_dir: None,
            }),
            exit_routing: ExitRoutingConfig::default(),
            vpn: None,
            privacy: None,
            guard_directory: None,
            constant_rate_capability: None,
            constant_rate_profile: ConstantRateProfileName::Core,
        };
        cfg.validate().expect("validate config");
        let incentives = cfg.incentive_log_config();
        assert!(incentives.enable);
        assert!(incentives.spool_dir.is_some());
    }

    #[test]
    fn exit_routing_validation_rejects_plain_http() {
        let mut routing = ExitRoutingConfig {
            norito_stream: Some(NoritoStreamRoutingConfig {
                torii_ws_url: "http://localhost:8080/ws".into(),
                connect_timeout_millis: 0,
                padding_target_millis: 0,
                gar_category_read_only: None,
                gar_category_authenticated: None,
                spool_dir: None,
                route_refresh_secs: 0,
            }),
            ..ExitRoutingConfig::default()
        };
        let err = routing.validate().expect_err("validation must fail");
        match err {
            ConfigError::Routing(message) => {
                assert!(
                    message.contains("ws://"),
                    "unexpected routing error message: {message}"
                );
            }
            other => panic!("unexpected error {other:?}"),
        }
    }

    #[test]
    fn exit_routing_validation_rejects_kaigi_plain_http() {
        let mut routing = ExitRoutingConfig {
            kaigi_stream: Some(KaigiStreamRoutingConfig {
                hub_ws_url: "http://localhost:9090/ws".into(),
                connect_timeout_millis: 0,
                gar_category_public: None,
                gar_category_authenticated: None,
                spool_dir: None,
                route_refresh_secs: 0,
            }),
            ..ExitRoutingConfig::default()
        };
        let err = routing.validate().expect_err("validation must fail");
        match err {
            ConfigError::Routing(message) => {
                assert!(
                    message.contains("ws://"),
                    "unexpected routing error message: {message}"
                );
            }
            other => panic!("unexpected error {other:?}"),
        }
    }

    #[test]
    fn puzzle_config_disabled_returns_none() {
        let mut pow = PowConfig {
            puzzle: Some(PuzzleConfig {
                enabled: false,
                memory_kib: 0,
                time_cost: 0,
                lanes: 0,
            }),
            ..PowConfig::default()
        };
        pow.apply_defaults().expect("defaults");
        let base = pow::Parameters::new(16, Duration::from_secs(60), Duration::from_secs(10));
        assert!(pow.puzzle_parameters(&base).expect("parameters").is_none());
    }

    #[test]
    fn quotas_for_mode_honours_overrides() {
        let mut pow = PowConfig {
            quotas: QuotaConfig {
                per_remote_burst: 100,
                per_remote_window_secs: 45,
                per_descriptor_burst: 80,
                per_descriptor_window_secs: 35,
                cooldown_secs: 15,
                max_entries: 2048,
            },
            quotas_per_mode: Some(HopQuotaOverrides {
                entry: Some(QuotaConfig {
                    per_remote_burst: 5,
                    per_remote_window_secs: 30,
                    per_descriptor_burst: 0,
                    per_descriptor_window_secs: 0,
                    cooldown_secs: 9,
                    max_entries: 1024,
                }),
                middle: None,
                exit: Some(QuotaConfig {
                    per_remote_burst: 70,
                    per_remote_window_secs: 0,
                    per_descriptor_burst: 20,
                    per_descriptor_window_secs: 0,
                    cooldown_secs: 0,
                    max_entries: 0,
                }),
            }),
            ..PowConfig::default()
        };
        pow.apply_defaults().expect("pow defaults");

        let entry = pow.quotas_for_mode(RelayMode::Entry);
        assert_eq!(entry.per_remote_burst, 5);
        assert_eq!(entry.per_remote_window_secs, 30);
        assert_eq!(entry.per_descriptor_burst, 0);
        assert_eq!(entry.cooldown_secs, 9);
        assert_eq!(entry.max_entries, 1024);

        let middle = pow.quotas_for_mode(RelayMode::Middle);
        assert_eq!(middle.per_remote_burst, 100);
        assert_eq!(middle.per_remote_window_secs, 45);
        assert_eq!(middle.cooldown_secs, 15);
        assert_eq!(middle.max_entries, 2048);

        let exit = pow.quotas_for_mode(RelayMode::Exit);
        assert_eq!(exit.per_remote_burst, 70);
        assert_eq!(
            exit.per_remote_window_secs,
            QuotaConfig::default_per_remote_window_secs()
        );
        assert_eq!(
            exit.per_descriptor_window_secs,
            QuotaConfig::default_per_descriptor_window_secs()
        );
        assert_eq!(exit.cooldown_secs, QuotaConfig::default_cooldown_secs());
        assert_eq!(exit.max_entries, QuotaConfig::default_max_entries());
        assert_eq!(exit.per_descriptor_burst, 20);
    }

    #[test]
    fn puzzle_config_rejects_invalid_values() {
        let mut pow = PowConfig {
            puzzle: Some(PuzzleConfig {
                enabled: true,
                memory_kib: 1024,
                time_cost: 0,
                lanes: 0,
            }),
            ..PowConfig::default()
        };
        let err = pow.apply_defaults().expect_err("invalid puzzle config");
        match err {
            ConfigError::Puzzle(message) => {
                assert!(message.contains("memory_kib"));
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn puzzle_config_builds_parameters() {
        let mut pow = PowConfig {
            difficulty: 12,
            max_future_skew_secs: 45,
            min_ticket_ttl_secs: 15,
            puzzle: Some(PuzzleConfig {
                enabled: true,
                memory_kib: 32 * 1024,
                time_cost: 3,
                lanes: 2,
            }),
            ..PowConfig::default()
        };
        pow.apply_defaults().expect("defaults");
        let base = pow::Parameters::new(12, Duration::from_secs(45), Duration::from_secs(15));
        let params = pow
            .puzzle_parameters(&base)
            .expect("parameters")
            .expect("enabled puzzle");
        assert_eq!(params.memory_kib().get(), 32 * 1024);
        assert_eq!(params.time_cost().get(), 3);
        assert_eq!(params.lanes().get(), 2);
        assert_eq!(params.difficulty(), 12);
    }

    #[test]
    fn replay_filter_defaults_and_rounds_parameters() {
        let mut cfg = ReplayFilterConfig {
            enabled: true,
            bits: 1_000,
            hash_functions: 0,
            ttl_secs: 0,
        };
        cfg.apply_defaults().expect("defaults");
        assert_eq!(cfg.bits, 1_024);
        assert_eq!(
            cfg.hash_functions,
            ReplayFilterConfig::default_hash_functions()
        );
        assert_eq!(cfg.ttl_secs, ReplayFilterConfig::default_ttl_secs());
    }

    #[test]
    fn replay_filter_rejects_invalid_parameters() {
        let mut too_many_bits = ReplayFilterConfig {
            enabled: true,
            bits: (1 << 24) + 1,
            hash_functions: 4,
            ttl_secs: 10,
        };
        let err = too_many_bits
            .apply_defaults()
            .expect_err("bits exceeding limit should fail");
        assert!(
            matches!(err, ConfigError::ReplayFilter(ref message) if message.contains("bits")),
            "unexpected error: {err:?}"
        );

        let mut too_many_hashes = ReplayFilterConfig {
            enabled: true,
            bits: 256,
            hash_functions: 17,
            ttl_secs: 10,
        };
        let err = too_many_hashes
            .apply_defaults()
            .expect_err("hash functions exceeding limit should fail");
        assert!(
            matches!(err, ConfigError::ReplayFilter(ref message) if message.contains("hash_functions")),
            "unexpected error: {err:?}"
        );
    }

    #[test]
    fn relay_config_loads_replay_filter_settings() {
        let json = r#"
            {
                "mode": "Entry",
                "listen": "127.0.0.1:0",
                "pow": {
                    "required": true,
                    "difficulty": 14,
                    "replay_filter": {
                        "enabled": true,
                        "bits": 4096,
                        "hash_functions": 3,
                        "ttl_secs": 45
                    }
                }
            }
        "#;
        let path = write_config(json);
        let cfg = RelayConfig::load(path).expect("load config");
        let filter = cfg.pow_config().replay_filter();
        assert!(filter.is_enabled());
        assert_eq!(filter.bits_usize(), 4_096);
        assert_eq!(filter.hash_count(), 3);
        assert_eq!(filter.ttl().as_secs(), 45);
    }

    #[test]
    fn rejects_partial_tls_paths() {
        let json = r#"
            {
                "mode": "Exit",
                "listen": "127.0.0.1:0",
                "tls": { "certificate_path": "/tmp/server.crt" }
            }
        "#;
        let path = write_config(json);
        let err = RelayConfig::load(path).expect_err("config should fail");
        match err {
            ConfigError::TlsPaths(message) => assert!(message.contains("private key path")),
            other => panic!("unexpected error variant: {other:?}"),
        }
    }

    #[test]
    fn rejects_bad_address() {
        let json = r#"
            {
                "mode": "Middle",
                "listen": "invalid:address",
                "tls": {}
            }
        "#;
        let path = write_config(json);
        let err = RelayConfig::load(path).expect_err("config should fail");
        match err {
            ConfigError::InvalidAddress(field, value) => {
                assert_eq!(field, "listen");
                assert_eq!(value, "invalid:address");
            }
            other => panic!("unexpected error variant: {other:?}"),
        }
    }

    #[test]
    fn handshake_descriptor_commit_is_decoded() {
        let json = r#"
            {
                "mode": "Entry",
                "listen": "127.0.0.1:0",
                "handshake": {
                    "descriptor_commit_hex": "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f"
                }
            }
        "#;
        let path = write_config(json);
        let cfg = RelayConfig::load(path).expect("load config");
        let commit = cfg
            .handshake_policy()
            .descriptor_commit_bytes()
            .expect("decode commit")
            .expect("commit present");
        assert_eq!(
            commit,
            [
                0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D,
                0x0E, 0x0F, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1A, 0x1B,
                0x1C, 0x1D, 0x1E, 0x1F
            ]
        );
    }

    #[test]
    fn rejects_unknown_kem_identifier() {
        let json = r#"
            {
                "mode": "Entry",
                "listen": "127.0.0.1:0",
                "handshake": {
                    "kem": [
                        { "id": "unknown-kem", "required": true }
                    ],
                    "signatures": [
                        { "id": "dilithium3", "required": true }
                    ],
                    "grease": []
                }
            }
        "#;
        let path = write_config(json);
        let err = RelayConfig::load(path).expect_err("config should fail");
        match err {
            ConfigError::Handshake(message) => {
                assert!(message.contains("unknown KEM identifier"));
            }
            other => panic!("unexpected error variant: {other:?}"),
        }
    }

    #[test]
    fn pow_defaults_populate_missing_fields() {
        let json = r#"
            {
                "mode": "Entry",
                "listen": "127.0.0.1:0",
                "pow": {
                    "required": true,
                    "difficulty": 5,
                    "max_future_skew_secs": 0,
                    "min_ticket_ttl_secs": 0
                }
            }
        "#;
        let path = write_config(json);
        let cfg = RelayConfig::load(path).expect("load config");
        assert_eq!(cfg.pow_config().max_future_skew_secs, 300);
        assert_eq!(cfg.pow_config().min_ticket_ttl_secs, 30);
    }

    #[test]
    fn rejects_identity_private_key_with_wrong_length() {
        let json = r#"
            {
                "mode": "Entry",
                "listen": "127.0.0.1:0",
                "handshake": {
                    "identity_private_key_hex": "0011"
                }
            }
        "#;
        let path = write_config(json);
        let err = RelayConfig::load(path).expect_err("config should fail");
        match err {
            ConfigError::Handshake(message) => {
                assert!(message.contains("identity_private_key_hex"));
            }
            other => panic!("unexpected error variant: {other:?}"),
        }
    }

    #[test]
    fn custom_congestion_config_validates() {
        let json = r#"
            {
                "mode": "Entry",
                "listen": "127.0.0.1:0",
                "congestion": {
                    "max_circuits_per_client": 4,
                    "handshake_cooldown_millis": 750
                }
            }
        "#;
        let path = write_config(json);
        let cfg = RelayConfig::load(path).expect("load config");
        assert_eq!(cfg.congestion_config().max_circuits_per_client, 4);
        assert_eq!(cfg.congestion_config().handshake_cooldown_millis, 750);
    }

    #[test]
    fn compliance_requires_log_path_when_enabled() {
        let json = r#"
            {
                "mode": "Entry",
                "listen": "127.0.0.1:0",
                "compliance": {
                    "enable": true
                }
            }
        "#;
        let path = write_config(json);
        let err = RelayConfig::load(path).expect_err("config should fail");
        match err {
            ConfigError::Compliance(message) => {
                assert!(message.contains("log_path"));
            }
            other => panic!("expected compliance error, got {other:?}"),
        }
    }

    #[test]
    fn compliance_salt_decodes() {
        let salt_hex = "0102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f20";
        let json = format!(
            r#"{{
                "mode": "Entry",
                "listen": "127.0.0.1:0",
                "compliance": {{
                    "enable": true,
                    "log_path": "/tmp/logger.jsonl",
                    "hash_salt_hex": "{salt_hex}",
                    "max_log_bytes": 123456,
                    "max_backup_files": 3,
                    "pipeline_spool_dir": "/tmp/spool"
                }}
            }}"#
        );
        let path = write_config(&json);
        let cfg = RelayConfig::load(path).expect("load config");
        let salt = cfg
            .compliance_config()
            .hash_salt_bytes()
            .expect("salt decode")
            .expect("salt present");
        assert_eq!(salt, <[u8; 32]>::from_hex(salt_hex).expect("hex to bytes"));
        assert_eq!(cfg.compliance_config().max_log_bytes, 123456);
        assert_eq!(cfg.compliance_config().max_backup_files, 3);
        assert_eq!(
            cfg.compliance_config()
                .pipeline_spool_dir()
                .expect("spool dir")
                .display()
                .to_string(),
            "/tmp/spool"
        );
    }

    #[test]
    fn identity_manifest_is_loaded() {
        let seed_hex = "abf17b54402f71fbb8ce1b716e2fdd9e7e1825cfb64fe0d4a1cfae3d6458f207";
        let manifest = write_manifest(&format!(
            r#"{{
                "version": 1,
                "identity": {{
                    "ed25519_private_key_hex": "{seed_hex}"
                }}
            }}"#
        ));
        let manifest_path = manifest.path().to_str().expect("manifest path utf-8");

        let json = format!(
            r#"{{
                "mode": "Entry",
                "listen": "127.0.0.1:0",
                "handshake": {{
                    "descriptor_manifest_path": "{manifest_path}"
                }}
            }}"#
        );
        let config_path = write_config(&json);
        let config = RelayConfig::load(config_path).expect("load config");
        let seed = config
            .handshake_policy()
            .identity_private_key_from_manifest()
            .expect("manifest parsing")
            .expect("seed present");
        let expected_bytes = hex::decode(seed_hex).expect("valid hex");
        let mut expected = [0u8; 32];
        expected.copy_from_slice(&expected_bytes);
        assert_eq!(seed, expected);
    }

    #[test]
    fn identity_manifest_missing_key_errors() {
        let manifest =
            write_manifest(r#"{ "version": 1, "identity": { "metadata": "placeholder" } }"#);
        let manifest_path = manifest.path().to_str().expect("manifest path utf-8");

        let json = format!(
            r#"{{
                "mode": "Entry",
                "listen": "127.0.0.1:0",
                "handshake": {{
                    "descriptor_manifest_path": "{manifest_path}"
                }}
            }}"#
        );
        let config_path = write_config(&json);
        let config = RelayConfig::load(config_path).expect("load config");
        match config
            .handshake_policy()
            .identity_private_key_from_manifest()
        {
            Err(ConfigError::DescriptorManifest { message, .. }) => {
                assert!(message.contains("missing"), "unexpected message: {message}");
            }
            other => panic!("expected manifest error, got {other:?}"),
        }
    }

    #[test]
    fn deploy_sample_config_validates() {
        let json = include_str!("../deploy/config/relay.entry.json");
        let mut cfg: RelayConfig = norito::json::from_str(json).expect("parse sample config");
        cfg.validate().expect("sample config validates");
        assert_eq!(cfg.mode, RelayMode::Entry);
        assert_eq!(
            cfg.handshake_policy()
                .descriptor_manifest_path()
                .expect("manifest path set")
                .display()
                .to_string(),
            "/etc/soranet/relay/secrets/relay-descriptor-manifest.json"
        );
        assert!(cfg.compliance_config().enable);
        assert_eq!(
            cfg.compliance_config()
                .log_path()
                .expect("log path")
                .display()
                .to_string(),
            "/var/log/soranet/relay_compliance.jsonl"
        );
        assert!(
            cfg.compliance_config()
                .hash_salt_bytes()
                .expect("salt bytes")
                .is_some()
        );
        assert_eq!(cfg.compliance_config().max_log_bytes, 67_108_864);
        assert_eq!(cfg.compliance_config().max_backup_files, 7);
        assert_eq!(
            cfg.compliance_config()
                .pipeline_spool_dir()
                .expect("spool dir")
                .display()
                .to_string(),
            "/var/spool/soranet/audit"
        );
        assert_eq!(cfg.constant_rate_profile(), ConstantRateProfileName::Core);
        assert_eq!(
            cfg.privacy_config().bucket_secs,
            DEFAULT_PRIVACY_BUCKET_SECS
        );
        assert_eq!(
            cfg.privacy_config().min_handshakes,
            DEFAULT_PRIVACY_MIN_HANDSHAKES
        );
        assert_eq!(
            cfg.privacy_config().flush_delay_buckets,
            DEFAULT_PRIVACY_FLUSH_DELAY_BUCKETS
        );
        assert_eq!(
            cfg.privacy_config().force_flush_buckets,
            DEFAULT_PRIVACY_FORCE_FLUSH_BUCKETS
        );
        assert_eq!(
            cfg.privacy_config().max_completed_buckets,
            DEFAULT_PRIVACY_MAX_COMPLETED_BUCKETS
        );
    }

    #[test]
    fn token_config_merges_inline_and_file_revocations() {
        use iroha_crypto::soranet::token::compute_issuer_fingerprint;
        use soranet_pq::{MlDsaSuite, generate_mldsa_keypair};

        let keypair = generate_mldsa_keypair(MlDsaSuite::MlDsa44).expect("generate keypair");
        let issuer_hex = hex::encode(keypair.public_key());

        let file_ids = [hex::encode([0x11; 32]), hex::encode([0x22; 32])];
        let json_value = norito::json::Value::Array(
            file_ids
                .iter()
                .cloned()
                .map(norito::json::Value::String)
                .collect(),
        );
        let json = norito::json::to_string_pretty(&json_value).expect("serialise revocations");
        let file = NamedTempFile::new().expect("revocation file");
        std::fs::write(file.path(), format!("{json}\n")).expect("write revocation file");

        let mut cfg = TokenConfig {
            enabled: true,
            issuer_public_key_hex: Some(issuer_hex),
            max_ttl_secs: 300,
            clock_skew_secs: 5,
            revocation_list_hex: vec![hex::encode([0x33; 32])],
            revocation_list_path: Some(file.path().to_path_buf()),
            ..TokenConfig::default()
        };

        cfg.apply_defaults();
        cfg.validate().expect("token config validates");
        let policy = cfg
            .build_policy()
            .expect("build policy")
            .expect("policy enabled");
        assert_eq!(policy.revocations.len(), 3);
        let expected_fp = compute_issuer_fingerprint(keypair.public_key());
        assert_eq!(policy.verifier.issuer_fingerprint(), &expected_fp);
    }

    #[test]
    fn privacy_config_overrides_are_applied() {
        let json = r#"
            {
                "mode": "Middle",
                "listen": "127.0.0.1:0",
                "privacy": {
                    "bucket_secs": 90,
                    "min_handshakes": 8,
                    "flush_delay_buckets": 2,
                    "force_flush_buckets": 4,
                    "max_completed_buckets": 12,
                    "expected_shares": 3,
                    "event_buffer_capacity": 2048
                }
            }
        "#;
        let path = write_config(json);
        let cfg = RelayConfig::load(path).expect("load config");
        let privacy = cfg.privacy_config();
        assert_eq!(privacy.bucket_secs, 90);
        assert_eq!(privacy.min_handshakes, 8);
        assert_eq!(privacy.flush_delay_buckets, 2);
        assert_eq!(privacy.force_flush_buckets, 4);
        assert_eq!(privacy.max_completed_buckets, 12);
        assert_eq!(privacy.expected_shares, 3);
        assert_eq!(privacy.event_buffer_capacity, 2_048);
    }

    #[test]
    fn privacy_config_validates_force_flush_ordering() {
        let json = r#"
            {
                "mode": "Exit",
                "listen": "127.0.0.1:0",
                "privacy": {
                    "flush_delay_buckets": 3,
                    "force_flush_buckets": 2
                }
            }
        "#;
        let path = write_config(json);
        let err = RelayConfig::load(path).expect_err("expected privacy validation error");
        match err {
            ConfigError::Privacy(message) => {
                assert!(
                    message.contains("force_flush_buckets"),
                    "unexpected privacy error: {message}"
                );
            }
            other => panic!("unexpected error {other:?}"),
        }
    }

    #[test]
    fn vpn_route_push_rejects_invalid_cidr() {
        let mut cfg = VpnConfig {
            enabled: true,
            route_push: vec!["not-a-cidr".to_string()],
            ..VpnConfig::default()
        };
        let err = cfg
            .validate()
            .expect_err("expected CIDR validation failure");
        match err {
            ConfigError::Vpn(message) => {
                assert!(
                    message.contains("CIDR"),
                    "unexpected vpn route error: {message}"
                );
            }
            other => panic!("unexpected error {other:?}"),
        }
    }

    #[test]
    fn vpn_dns_override_rejects_non_ip() {
        let mut cfg = VpnConfig {
            enabled: true,
            dns_overrides: vec!["example.com".to_string()],
            ..VpnConfig::default()
        };
        let err = cfg.validate().expect_err("expected dns validation failure");
        match err {
            ConfigError::Vpn(message) => {
                assert!(
                    message.contains("dns_overrides"),
                    "unexpected vpn dns error: {message}"
                );
            }
            other => panic!("unexpected error {other:?}"),
        }
    }

    #[test]
    fn vpn_cover_ratio_allows_zero_when_enabled() {
        let mut cfg = VpnConfig {
            enabled: true,
            cover: VpnCoverTrafficConfig {
                enabled: true,
                cover_to_data_per_mille: 0,
                heartbeat_ms: 10,
                max_cover_burst: 1,
                max_jitter_millis: 1,
            },
            ..VpnConfig::default()
        };

        cfg.validate().expect("vpn config should validate");
        assert_eq!(cfg.cover.cover_to_data_per_mille, 0);
    }

    #[test]
    fn vpn_control_plane_threads_routes_and_dns() {
        let mut cfg = VpnConfig {
            enabled: true,
            lease_secs: 45,
            route_push: vec!["10.0.0.0/24".to_string()],
            dns_overrides: vec!["1.1.1.1".to_string()],
            ..VpnConfig::default()
        };
        cfg.validate().expect("vpn config validates");
        let overlay = VpnOverlay::from_config(cfg);
        let entry_guard = [0xAA; 32];
        let exit_guard = [0xBB; 32];
        let envelope = overlay.control_plane_envelope(entry_guard, exit_guard);

        assert_eq!(envelope.entry_guard, entry_guard);
        assert_eq!(envelope.exit_guard, exit_guard);
        assert_eq!(envelope.lease_seconds, 45);
        assert_eq!(envelope.exit_class, VpnExitClassV1::Standard);
        assert_eq!(envelope.dns_servers, vec!["1.1.1.1".to_string()]);
        assert_eq!(envelope.routes.len(), 1);
        assert_eq!(envelope.routes[0].cidr, "10.0.0.0/24");
    }
}
