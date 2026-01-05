//! Local QUIC proxy helpers for browser/SDK integrations.
//!
//! The proxy terminates QUIC connections on the operator workstation and
//! relays metadata (certificate, manifest) to browser extensions so they can
//! connect to SoraNet-enabled Torii gateways without handling certificates or
//! guard cache keys directly.

#![allow(unexpected_cfgs)]

use std::{
    net::SocketAddr,
    path::{Component, Path, PathBuf},
    pin::Pin,
    str::FromStr,
    sync::Arc,
    task::{Context, Poll},
};

use hex::ToHex;
use iroha_logger::{info, warn};
use iroha_telemetry::metrics::{global_or_default, global_sorafs_fetch_otel};
use norito::{
    NoritoDeserialize, NoritoSerialize, core::DecodeFromSlice, decode_from_bytes, to_bytes,
};
use quinn::{
    ConnectionError, Endpoint, ServerConfig, VarInt,
    rustls::pki_types::{CertificateDer, PrivateKeyDer},
};
use rand::{rand_core::TryRngCore, rngs::OsRng};
use rcgen::generate_simple_self_signed;
use sha2::{Digest, Sha256};
use thiserror::Error;
use tokio::{
    fs,
    io::{self, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadBuf},
    net::TcpStream,
    sync::{Mutex, watch},
    task::JoinHandle,
};

use crate::soranet::{GuardCacheKey, GuardCacheKeyError};

const PROXY_HANDSHAKE_VERSION: u8 = 1;
const PROXY_ALPN_LABEL: &str = "sorafs-proxy/1";
pub(crate) const PROXY_PROTOCOL_LABEL: &str = "local_quic_proxy";
pub(crate) const PROXY_MANIFEST_ID: &str = "local_quic_proxy";
const PROXY_CAPABILITIES: &[&str] = &[
    "raw-stream",
    "car",
    "tcp-connect",
    "telemetry-v2",
    "cache-tag-v1",
];
const PROXY_SESSION_ID_LEN: usize = 16;
const PROXY_CACHE_TAG_SALT_LEN: usize = 16;
const PROXY_STREAM_VERSION: u8 = 1;
const PROXY_MAX_FRAME_BYTES: usize = 1024 * 1024;
const STREAM_ACK_OK: u8 = 0;
const STREAM_ACK_UNSUPPORTED_SERVICE: u8 = 1;
const STREAM_ACK_MODE_DISABLED: u8 = 2;
const STREAM_ACK_BAD_REQUEST: u8 = 3;
const STREAM_ACK_INTERNAL_ERROR: u8 = 4;
const STREAM_ACK_UNSUPPORTED_VERSION: u8 = 5;
const STREAM_ACK_NOT_IMPLEMENTED: u8 = 7;
const CACHE_TAG_FIELD_SEPARATOR: u8 = 0x1f;

/// Configuration for the local QUIC proxy.
#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, PartialEq, Eq, Default)]
pub enum ProxyMode {
    /// Proxy terminates QUIC locally and bridges traffic to SoraNet paths.
    #[norito(rename = "bridge")]
    #[default]
    Bridge,
    /// Proxy only advertises manifest metadata; clients establish SoraNet circuits.
    #[norito(rename = "metadata-only")]
    MetadataOnly,
}

impl ProxyMode {
    /// Returns the canonical label for this proxy mode.
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Bridge => "bridge",
            Self::MetadataOnly => "metadata-only",
        }
    }

    /// Parses a textual proxy mode label.
    pub fn parse(label: &str) -> Option<Self> {
        match label.trim().to_ascii_lowercase().as_str() {
            "bridge" => Some(Self::Bridge),
            "metadata-only" | "metadata_only" | "metadata" => Some(Self::MetadataOnly),
            _ => None,
        }
    }
}

impl norito::json::FastJsonWrite for ProxyMode {
    fn write_json(&self, out: &mut String) {
        norito::json::write_json_string(self.as_str(), out);
    }
}

impl norito::json::JsonDeserialize for ProxyMode {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        let value = parser.parse_string()?;
        ProxyMode::parse(&value)
            .ok_or_else(|| norito::json::Error::Message(format!("invalid proxy_mode `{value}`")))
    }

    fn json_from_value(value: &norito::json::Value) -> Result<Self, norito::json::Error> {
        if let Some(label) = value.as_str() {
            return ProxyMode::parse(label).ok_or_else(|| {
                norito::json::Error::Message(format!("invalid proxy_mode `{label}`"))
            });
        }
        Err(norito::json::Error::Message(
            "proxy_mode expects a string".to_owned(),
        ))
    }

    fn json_from_map_key(key: &str) -> Result<Self, norito::json::Error> {
        ProxyMode::parse(key)
            .ok_or_else(|| norito::json::Error::Message(format!("invalid proxy_mode `{key}`")))
    }
}

#[derive(
    Debug,
    Clone,
    NoritoSerialize,
    NoritoDeserialize,
    norito::derive::JsonSerialize,
    norito::derive::JsonDeserialize,
    PartialEq,
    Eq,
)]
pub struct LocalQuicProxyConfig {
    /// Address to bind for incoming browser/SDK traffic.
    pub bind_addr: String,
    /// Optional telemetry label used in metrics (defaults to `proxy`).
    #[norito(default)]
    pub telemetry_label: Option<String>,
    /// Optional guard cache key advertised to browser integrations.
    #[norito(default)]
    pub guard_cache_key_hex: Option<String>,
    /// Whether to emit a browser manifest payload in the handshake.
    #[norito(default)]
    pub emit_browser_manifest: bool,
    /// Proxy runtime mode (bridge vs metadata-only).
    #[norito(default)]
    pub proxy_mode: ProxyMode,
    /// Prefer eager circuit creation when a session starts.
    #[norito(default)]
    pub prewarm_circuits: bool,
    /// Maximum number of client streams attached to a single circuit.
    #[norito(default)]
    pub max_streams_per_circuit: Option<u32>,
    /// Optional configured circuit TTL (seconds) surfaced to clients.
    #[norito(default)]
    pub circuit_ttl_hint_secs: Option<u32>,
    /// Optional Norito bridge configuration (filesystem spool).
    #[norito(default)]
    pub norito_bridge: Option<ProxyNoritoBridgeConfig>,
    /// Optional CAR bridge configuration (local cache archives).
    #[norito(default)]
    pub car_bridge: Option<ProxyCarBridgeConfig>,
    /// Optional Kaigi bridge configuration (filesystem spool + policy).
    #[norito(default)]
    pub kaigi_bridge: Option<ProxyKaigiBridgeConfig>,
}

impl Default for LocalQuicProxyConfig {
    fn default() -> Self {
        Self {
            bind_addr: "127.0.0.1:0".into(),
            telemetry_label: None,
            guard_cache_key_hex: None,
            emit_browser_manifest: true,
            proxy_mode: ProxyMode::Bridge,
            prewarm_circuits: true,
            max_streams_per_circuit: Some(64),
            circuit_ttl_hint_secs: Some(300),
            norito_bridge: None,
            car_bridge: None,
            kaigi_bridge: None,
        }
    }
}

#[derive(
    Debug,
    Clone,
    NoritoSerialize,
    NoritoDeserialize,
    norito::derive::JsonSerialize,
    norito::derive::JsonDeserialize,
    PartialEq,
    Eq,
)]
pub struct ProxyNoritoBridgeConfig {
    pub spool_dir: String,
    #[norito(default = "default_norito_bridge_extension")]
    pub extension: Option<String>,
}

fn default_norito_bridge_extension() -> Option<String> {
    Some("norito".to_string())
}

#[derive(
    Debug,
    Clone,
    NoritoSerialize,
    NoritoDeserialize,
    norito::derive::JsonSerialize,
    norito::derive::JsonDeserialize,
    PartialEq,
    Eq,
)]
pub struct ProxyCarBridgeConfig {
    pub cache_dir: String,
    #[norito(default = "default_car_bridge_extension")]
    pub extension: Option<String>,
    #[norito(default)]
    pub allow_zst: bool,
}

fn default_car_bridge_extension() -> Option<String> {
    Some("car".to_string())
}

#[derive(
    Debug,
    Clone,
    NoritoSerialize,
    NoritoDeserialize,
    norito::derive::JsonSerialize,
    norito::derive::JsonDeserialize,
    PartialEq,
    Eq,
)]
pub struct ProxyKaigiBridgeConfig {
    pub spool_dir: String,
    #[norito(default = "default_kaigi_bridge_extension")]
    pub extension: Option<String>,
    #[norito(default)]
    pub room_policy: Option<String>,
}

fn default_kaigi_bridge_extension() -> Option<String> {
    Some("norito".to_string())
}

#[derive(Clone, Default)]
struct ProxyBridgeConfig {
    norito: Option<ProxyNoritoBridge>,
    car: Option<ProxyCarBridge>,
    kaigi: Option<ProxyKaigiBridge>,
}

impl ProxyBridgeConfig {
    fn from_config(config: &LocalQuicProxyConfig) -> Self {
        let norito = config
            .norito_bridge
            .as_ref()
            .and_then(ProxyNoritoBridge::from_config);
        let car = config
            .car_bridge
            .as_ref()
            .and_then(ProxyCarBridge::from_config);
        let kaigi = config
            .kaigi_bridge
            .as_ref()
            .and_then(ProxyKaigiBridge::from_config);
        Self { norito, car, kaigi }
    }

    fn norito(&self) -> Option<&ProxyNoritoBridge> {
        self.norito.as_ref()
    }

    fn car(&self) -> Option<&ProxyCarBridge> {
        self.car.as_ref()
    }

    fn kaigi(&self) -> Option<&ProxyKaigiBridge> {
        self.kaigi.as_ref()
    }
}

#[derive(Clone)]
struct ProxyNoritoBridge {
    spool_dir: PathBuf,
    extension: Option<String>,
}

impl ProxyNoritoBridge {
    fn from_config(cfg: &ProxyNoritoBridgeConfig) -> Option<Self> {
        let spool_dir = cfg.spool_dir.trim();
        if spool_dir.is_empty() {
            return None;
        }
        let extension = cfg
            .extension
            .as_ref()
            .and_then(|ext| sanitize_extension(ext));
        Some(Self {
            spool_dir: PathBuf::from(spool_dir),
            extension,
        })
    }

    fn resolve_target(&self, target: &str) -> Result<PathBuf, String> {
        let sanitized = sanitize_relative_target(target)?;
        let mut path = self.spool_dir.join(sanitized);
        if path.extension().is_none()
            && let Some(ext) = &self.extension
        {
            path.set_extension(ext);
        }
        Ok(path)
    }
}

#[derive(Clone)]
struct ProxyCarBridge {
    cache_dir: PathBuf,
    extension: Option<String>,
    allow_zst: bool,
}

impl ProxyCarBridge {
    fn from_config(cfg: &ProxyCarBridgeConfig) -> Option<Self> {
        let cache_dir = cfg.cache_dir.trim();
        if cache_dir.is_empty() {
            return None;
        }
        let extension = cfg
            .extension
            .as_ref()
            .and_then(|ext| sanitize_extension(ext));
        Some(Self {
            cache_dir: PathBuf::from(cache_dir),
            extension,
            allow_zst: cfg.allow_zst,
        })
    }

    fn resolve_target(&self, target: &str) -> Result<PathBuf, String> {
        let sanitized = sanitize_relative_target(target)?;
        let mut path = self.cache_dir.join(&sanitized);
        if path.extension().is_none()
            && let Some(ext) = &self.extension
        {
            path.set_extension(ext);
        } else if let Some(ext) = path.extension().and_then(|e| e.to_str())
            && ext.eq_ignore_ascii_case("zst")
            && !self.allow_zst
        {
            return Err("compressed car payloads are disabled".to_string());
        }
        Ok(path)
    }
}

#[derive(Clone)]
struct ProxyKaigiBridge {
    spool_dir: PathBuf,
    extension: Option<String>,
    room_policy: ProxyKaigiRoomPolicy,
}

impl ProxyKaigiBridge {
    fn from_config(cfg: &ProxyKaigiBridgeConfig) -> Option<Self> {
        let spool_dir = cfg.spool_dir.trim();
        if spool_dir.is_empty() {
            return None;
        }
        let extension = cfg
            .extension
            .as_ref()
            .and_then(|ext| sanitize_extension(ext));
        let room_policy = match cfg.room_policy.as_deref() {
            Some(label) => match ProxyKaigiRoomPolicy::from_str(label) {
                Ok(policy) => policy,
                Err(err) => {
                    warn!(
                        target: "soranet.proxy",
                        %err,
                        "ignoring kaigi bridge configuration due to invalid room policy"
                    );
                    return None;
                }
            },
            None => ProxyKaigiRoomPolicy::Public,
        };
        Some(Self {
            spool_dir: PathBuf::from(spool_dir),
            extension,
            room_policy,
        })
    }

    fn resolve_target(&self, target: &str) -> Result<PathBuf, String> {
        let path = sanitize_relative_target(target)?;
        let mut resolved = self.spool_dir.clone();
        resolved.push(&path);
        if resolved.extension().is_none()
            && let Some(ext) = &self.extension
        {
            resolved.set_extension(ext);
        }
        Ok(resolved)
    }

    fn room_policy(&self) -> ProxyKaigiRoomPolicy {
        self.room_policy
    }
}

fn sanitize_extension(ext: &str) -> Option<String> {
    let trimmed = ext.trim().trim_start_matches('.');
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_ascii_lowercase())
    }
}

fn sanitize_relative_target(target: &str) -> Result<PathBuf, String> {
    if target.trim().is_empty() {
        return Err("target must not be empty".to_string());
    }
    let path = Path::new(target);
    if path.is_absolute() {
        return Err("absolute paths are not permitted".to_string());
    }
    let mut sanitized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(segment) => sanitized.push(segment),
            Component::CurDir => continue,
            _ => return Err("path traversal is not permitted".to_string()),
        }
    }
    if sanitized.as_os_str().is_empty() {
        return Err("target must not be empty".to_string());
    }
    Ok(sanitized)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ProxyKaigiRoomPolicy {
    Public,
    Authenticated,
}

impl ProxyKaigiRoomPolicy {
    fn from_str(value: &str) -> Result<Self, String> {
        match value.trim().to_ascii_lowercase().as_str() {
            "" => Ok(Self::Public),
            "public" => Ok(Self::Public),
            "authenticated" => Ok(Self::Authenticated),
            other => Err(format!(
                "kaigi room policy must be `public` or `authenticated` (got `{other}`)"
            )),
        }
    }

    fn as_label(&self) -> &'static str {
        match self {
            Self::Public => "public",
            Self::Authenticated => "authenticated",
        }
    }
}

impl LocalQuicProxyConfig {
    fn parsed_bind_addr(&self) -> Result<SocketAddr, ProxyError> {
        SocketAddr::from_str(self.bind_addr.trim())
            .map_err(|err| ProxyError::BindAddressInvalid(self.bind_addr.clone(), err.to_string()))
    }

    fn guard_cache_key(&self) -> Result<Option<GuardCacheKey>, ProxyError> {
        match self.guard_cache_key_hex.as_deref() {
            Some(hex) => Ok(Some(
                GuardCacheKey::from_hex(hex).map_err(ProxyError::GuardCacheKey)?,
            )),
            None => Ok(None),
        }
    }
}

/// Errors surfaced while starting or running the proxy.
#[derive(Debug, Error)]
pub enum ProxyError {
    /// Bind address failed to parse.
    #[error("invalid bind address `{0}`: {1}")]
    BindAddressInvalid(String, String),
    /// Guard cache key could not be parsed.
    #[error(transparent)]
    GuardCacheKey(#[from] GuardCacheKeyError),
    /// Generating the self-signed certificate failed.
    #[error("failed to generate proxy certificate: {0}")]
    Certificate(rcgen::Error),
    /// Preparing the proxy private key failed.
    #[error("failed to prepare proxy private key: {0}")]
    PrivateKey(String),
    /// Configuring the QUIC endpoint failed.
    #[error("failed to configure QUIC endpoint: {0}")]
    QuinnConfig(String),
    /// Starting the endpoint failed.
    #[error("failed to start QUIC endpoint: {0}")]
    QuinnEndpoint(String),
    /// Handshake payload failed to serialise/deserialise.
    #[error("handshake error: {0}")]
    Handshake(String),
    /// Application stream handling failed.
    #[error("application stream error: {0}")]
    Stream(String),
}

/// Runtime handle for a spawned local QUIC proxy.
#[derive(Clone)]
pub struct LocalQuicProxyHandle {
    inner: Arc<LocalQuicProxyInner>,
}

impl LocalQuicProxyHandle {
    /// Address that the proxy listens on.
    #[must_use]
    pub fn local_addr(&self) -> SocketAddr {
        self.inner.local_addr
    }

    /// PEM-encoded certificate accepted by the proxy.
    #[must_use]
    pub fn certificate_pem(&self) -> &str {
        &self.inner.certificate_pem
    }

    /// Optional manifest payload for browser extensions.
    #[must_use]
    pub fn browser_manifest(&self) -> Option<BrowserExtensionManifest> {
        self.inner
            .browser_manifest
            .as_ref()
            .map(BrowserManifestTemplate::preview)
    }

    /// Runtime mode advertised by the proxy.
    #[must_use]
    pub fn mode(&self) -> ProxyMode {
        self.inner.mode.clone()
    }

    /// Guard cache key associated with the proxy (if configured).
    #[must_use]
    pub fn guard_cache_key(&self) -> Option<GuardCacheKey> {
        self.inner.guard_cache_key.clone()
    }

    /// Gracefully shuts the proxy down.
    pub async fn shutdown(&self) {
        if self.inner.shutdown_tx.send(true).is_ok()
            && let Some(handle) = self.inner.join_handle.lock().await.take()
        {
            let _ = handle.await;
        }
    }
}

struct LocalQuicProxyInner {
    #[allow(unused)]
    endpoint: Endpoint,
    local_addr: SocketAddr,
    certificate_pem: String,
    browser_manifest: Option<BrowserManifestTemplate>,
    mode: ProxyMode,
    guard_cache_key: Option<GuardCacheKey>,
    shutdown_tx: watch::Sender<bool>,
    join_handle: Mutex<Option<JoinHandle<()>>>,
}

#[derive(Clone)]
struct BrowserManifestTemplate {
    base: BrowserExtensionManifest,
}

impl BrowserManifestTemplate {
    fn new(base: BrowserExtensionManifest) -> Self {
        Self { base }
    }

    fn instantiate(&self) -> BrowserExtensionManifest {
        let mut manifest = self.base.clone();
        manifest.session_id = Some(generate_session_id());
        if let Some(cache) = manifest.cache_tagging.as_mut() {
            cache.salt_hex = Some(generate_cache_salt());
        }
        manifest
    }

    fn preview(&self) -> BrowserExtensionManifest {
        self.instantiate()
    }
}

fn build_manifest_template(
    config: &LocalQuicProxyConfig,
    local_addr: SocketAddr,
    certificate_pem: &str,
    certificate_der: &[u8],
    guard_cache_key: Option<&GuardCacheKey>,
) -> Option<BrowserManifestTemplate> {
    if !config.emit_browser_manifest {
        return None;
    }

    let mut hasher = Sha256::new();
    hasher.update(certificate_der);
    let fingerprint_hex = hex::encode(hasher.finalize());
    let guard_cache_key_hex = guard_cache_key.map(GuardCacheKey::to_hex);
    let mut capabilities = Vec::with_capacity(PROXY_CAPABILITIES.len());
    capabilities.extend(PROXY_CAPABILITIES.iter().map(|cap| cap.to_string()));

    let circuit = ProxyCircuitHints {
        ttl_sec: config.circuit_ttl_hint_secs,
        max_streams_per_circuit: config.max_streams_per_circuit,
        prewarm: config.prewarm_circuits,
        reuse_policy: Some("young-first".into()),
    };
    let guard_selection = ProxyGuardSelection {
        guard_set_id: None,
        stickiness: Some("per-origin".into()),
        min_guards: Some(1),
        max_guards: Some(3),
    };
    let cache_tagging = ProxyCacheTagging::default();
    let telemetry_labels = ProxyTelemetryLabels {
        telemetry_label: config.telemetry_label.clone(),
        authority: Some(local_addr.to_string()),
    };
    let telemetry = ProxyTelemetryHints {
        labels: Some(telemetry_labels),
        sampling: Some(ProxyTelemetrySampling {
            traces: Some(0.05),
            metrics: Some(1.0),
        }),
        privacy: Some(ProxyTelemetryPrivacy::new_default()),
    };

    let mut route_hints = Vec::new();
    if let Some(kaigi_bridge) = config.kaigi_bridge.as_ref() {
        let policy_label = kaigi_bridge
            .room_policy
            .as_deref()
            .unwrap_or("public")
            .to_string();
        route_hints.push(ProxyRouteHint {
            policy_id: Some("kaigi-room-policy".into()),
            exit_country: None,
            categories: vec!["kaigi".into(), format!("kaigi.room_policy.{policy_label}")],
        });
    }

    let manifest = BrowserExtensionManifest {
        version: 2,
        authority: local_addr.to_string(),
        certificate_pem: certificate_pem.to_string(),
        cert_fingerprint_hex: Some(fingerprint_hex),
        alpn: Some(PROXY_ALPN_LABEL.to_string()),
        capabilities,
        proxy_mode: config.proxy_mode.clone(),
        session_id: None,
        telemetry_label: config.telemetry_label.clone(),
        guard_cache_key_hex,
        circuit: Some(circuit),
        guard_selection: Some(guard_selection),
        route_hints,
        cache_tagging: Some(cache_tagging),
        telemetry_v2: Some(telemetry),
    };

    Some(BrowserManifestTemplate::new(manifest))
}

/// Manifest returned to browser extensions establishing trust with the proxy.
#[derive(
    Debug,
    Clone,
    NoritoSerialize,
    NoritoDeserialize,
    norito::derive::JsonSerialize,
    norito::derive::JsonDeserialize,
    PartialEq,
)]
pub struct BrowserExtensionManifest {
    /// Manifest schema version.
    pub version: u8,
    /// Authority (host:port) exposed by the proxy.
    pub authority: String,
    /// PEM-encoded certificate clients should trust.
    pub certificate_pem: String,
    /// Optional certificate fingerprint (SHA-256 hex).
    #[norito(default)]
    #[norito(rename = "cert_sha256")]
    pub cert_fingerprint_hex: Option<String>,
    /// ALPN label advertised by the proxy.
    #[norito(default)]
    pub alpn: Option<String>,
    /// Proxy capabilities supported within this release.
    #[norito(default)]
    pub capabilities: Vec<String>,
    /// Operational mode advertised to clients.
    #[norito(default)]
    pub proxy_mode: ProxyMode,
    /// Random session identifier unique per handshake.
    #[norito(default)]
    pub session_id: Option<String>,
    /// Optional telemetry label describing the proxy.
    #[norito(default)]
    pub telemetry_label: Option<String>,
    /// Optional guard cache key delivered to the extension.
    #[norito(default)]
    pub guard_cache_key_hex: Option<String>,
    /// Circuit tuning hints surfaced to clients.
    #[norito(default)]
    pub circuit: Option<ProxyCircuitHints>,
    /// Guard stickiness parameters.
    #[norito(default)]
    pub guard_selection: Option<ProxyGuardSelection>,
    /// Preferred route hints.
    #[norito(default)]
    pub route_hints: Vec<ProxyRouteHint>,
    /// Cache tagging parameters applied by the proxy.
    #[norito(default)]
    pub cache_tagging: Option<ProxyCacheTagging>,
    /// Telemetry configuration hints for clients.
    #[norito(default)]
    pub telemetry_v2: Option<ProxyTelemetryHints>,
}

/// Per-session circuit hints surfaced to clients.
#[derive(
    Debug,
    Clone,
    Default,
    NoritoSerialize,
    NoritoDeserialize,
    norito::derive::JsonSerialize,
    norito::derive::JsonDeserialize,
    PartialEq,
    Eq,
)]
pub struct ProxyCircuitHints {
    /// Suggested TTL for circuits in seconds.
    #[norito(default)]
    pub ttl_sec: Option<u32>,
    /// Maximum number of simultaneous streams per circuit.
    #[norito(default)]
    pub max_streams_per_circuit: Option<u32>,
    /// Whether the proxy prewarms circuits.
    #[norito(default)]
    pub prewarm: bool,
    /// Optional reuse policy label (e.g. `young-first`).
    #[norito(default)]
    pub reuse_policy: Option<String>,
}

/// Guard selection preferences exposed to callers.
#[derive(
    Debug,
    Clone,
    Default,
    NoritoSerialize,
    NoritoDeserialize,
    norito::derive::JsonSerialize,
    norito::derive::JsonDeserialize,
    PartialEq,
    Eq,
)]
pub struct ProxyGuardSelection {
    /// Identifier describing the guard set in use.
    #[norito(default)]
    pub guard_set_id: Option<String>,
    /// Stickiness policy label (e.g. `per-origin`).
    #[norito(default)]
    pub stickiness: Option<String>,
    /// Minimum number of guards used when bridging sessions.
    #[norito(default)]
    pub min_guards: Option<u32>,
    /// Maximum number of guards used when bridging sessions.
    #[norito(default)]
    pub max_guards: Option<u32>,
}

/// Lightweight route hints surfaced to browser clients.
#[derive(
    Debug,
    Clone,
    Default,
    NoritoSerialize,
    NoritoDeserialize,
    norito::derive::JsonSerialize,
    norito::derive::JsonDeserialize,
    PartialEq,
    Eq,
)]
pub struct ProxyRouteHint {
    /// Optional policy identifier.
    #[norito(default)]
    pub policy_id: Option<String>,
    /// Preferred exit country (ISO code).
    #[norito(default)]
    pub exit_country: Option<String>,
    /// Free-form categories describing the route.
    #[norito(default)]
    pub categories: Vec<String>,
}

/// Cache tagging parameters advertised by the proxy.
#[derive(
    Debug,
    Clone,
    NoritoSerialize,
    NoritoDeserialize,
    norito::derive::JsonSerialize,
    norito::derive::JsonDeserialize,
    PartialEq,
    Eq,
)]
pub struct ProxyCacheTagging {
    /// Version of the tagging format.
    pub version: u8,
    /// Algorithm label used when deriving cache tags.
    pub alg: String,
    /// Header name carrying cache tags for HTTP/TCP streams.
    pub tag_header: String,
    /// Salt used when deriving HMAC tags for this session.
    #[norito(default)]
    pub salt_hex: Option<String>,
    /// Services where cache tags are attached.
    #[norito(default)]
    pub attach_on: Vec<String>,
}

impl Default for ProxyCacheTagging {
    fn default() -> Self {
        Self {
            version: 1,
            alg: "HMAC-SHA256-128".into(),
            tag_header: "x-sorafs-cache-tag".into(),
            salt_hex: None,
            attach_on: vec!["car".into(), "norito".into(), "kaigi".into(), "tcp".into()],
        }
    }
}

/// Telemetry hints for browser integrations.
#[derive(
    Debug,
    Clone,
    Default,
    NoritoSerialize,
    NoritoDeserialize,
    norito::derive::JsonSerialize,
    norito::derive::JsonDeserialize,
    PartialEq,
)]
pub struct ProxyTelemetryHints {
    /// Static labels clients may include when exporting telemetry.
    #[norito(default)]
    pub labels: Option<ProxyTelemetryLabels>,
    /// Sampling configuration for traces and metrics.
    #[norito(default)]
    pub sampling: Option<ProxyTelemetrySampling>,
    /// Privacy-preserving truncation hints.
    #[norito(default)]
    pub privacy: Option<ProxyTelemetryPrivacy>,
}

/// Static telemetry labels.
#[derive(
    Debug,
    Clone,
    Default,
    NoritoSerialize,
    NoritoDeserialize,
    norito::derive::JsonSerialize,
    norito::derive::JsonDeserialize,
    PartialEq,
)]
pub struct ProxyTelemetryLabels {
    #[norito(default)]
    pub telemetry_label: Option<String>,
    #[norito(default)]
    pub authority: Option<String>,
}

/// Telemetry sampling hints.
#[derive(
    Debug,
    Clone,
    Default,
    NoritoSerialize,
    NoritoDeserialize,
    norito::derive::JsonSerialize,
    norito::derive::JsonDeserialize,
    PartialEq,
)]
pub struct ProxyTelemetrySampling {
    #[norito(default)]
    pub traces: Option<f64>,
    #[norito(default)]
    pub metrics: Option<f64>,
}

/// Telemetry privacy hints (low-cardinality fingerprints, etc.).
#[derive(
    Debug,
    Clone,
    Default,
    NoritoSerialize,
    NoritoDeserialize,
    norito::derive::JsonSerialize,
    norito::derive::JsonDeserialize,
    PartialEq,
)]
pub struct ProxyTelemetryPrivacy {
    #[norito(default)]
    pub fingerprint_prefix_bits: Option<u8>,
    #[norito(default)]
    pub redact_authority: bool,
}

impl ProxyTelemetryPrivacy {
    fn new_default() -> Self {
        Self {
            fingerprint_prefix_bits: Some(20),
            redact_authority: true,
        }
    }
}

impl<'a> DecodeFromSlice<'a> for ProxyTelemetryHints {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        norito::core::decode_field_canonical(bytes)
    }
}

impl<'a> DecodeFromSlice<'a> for ProxyTelemetryLabels {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        norito::core::decode_field_canonical(bytes)
    }
}

impl<'a> DecodeFromSlice<'a> for ProxyTelemetrySampling {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        norito::core::decode_field_canonical(bytes)
    }
}

impl<'a> DecodeFromSlice<'a> for ProxyTelemetryPrivacy {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        norito::core::decode_field_canonical(bytes)
    }
}

impl<'a> DecodeFromSlice<'a> for BrowserExtensionManifest {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        norito::core::decode_field_canonical(bytes)
    }
}

impl<'a> DecodeFromSlice<'a> for ProxyCircuitHints {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        norito::core::decode_field_canonical(bytes)
    }
}

impl<'a> DecodeFromSlice<'a> for ProxyGuardSelection {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        norito::core::decode_field_canonical(bytes)
    }
}

impl<'a> DecodeFromSlice<'a> for ProxyRouteHint {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        norito::core::decode_field_canonical(bytes)
    }
}

impl<'a> DecodeFromSlice<'a> for ProxyCacheTagging {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        norito::core::decode_field_canonical(bytes)
    }
}

#[derive(Clone)]
struct ProxySession {
    telemetry_label: String,
    mode: ProxyMode,
    guard_cache_key: Option<GuardCacheKey>,
    session_id: Option<String>,
    cache_tags: Option<CacheTagContext>,
    bridge: Arc<ProxyBridgeConfig>,
}

#[derive(Clone)]
struct CacheTagContext {
    alg: String,
    attach_on: Vec<String>,
    salt: Vec<u8>,
}

impl CacheTagContext {
    fn from_manifest(manifest: &BrowserExtensionManifest) -> Option<Self> {
        let tagging = manifest.cache_tagging.as_ref()?;
        let salt_hex = tagging.salt_hex.as_deref()?;
        let salt = match hex::decode(salt_hex) {
            Ok(bytes) => bytes,
            Err(err) => {
                warn!(
                    target: "soranet.proxy",
                    ?err,
                    "failed to decode cache tag salt"
                );
                return None;
            }
        };
        let attach_on = tagging
            .attach_on
            .iter()
            .map(|label| label.trim().to_ascii_lowercase())
            .collect();
        Some(Self {
            alg: tagging.alg.clone(),
            attach_on,
            salt,
        })
    }

    fn supports(&self, service: &str) -> bool {
        if self.attach_on.is_empty() {
            return false;
        }
        self.attach_on.iter().any(|label| label == service)
    }
}

impl ProxySession {
    fn cache_tag_for(
        &self,
        service: ProxyStreamService,
        open_frame: &ProxyStreamOpenV1,
    ) -> Option<String> {
        let context = self.cache_tags.as_ref()?;
        if !context.alg.trim().eq_ignore_ascii_case("HMAC-SHA256-128") {
            return None;
        }
        let guard_key = self.guard_cache_key.as_ref()?;
        let service_label = service.as_label();
        if !context.supports(service_label) {
            return None;
        }

        let mut message = Vec::with_capacity(context.salt.len() + 64);
        message.extend_from_slice(&context.salt);
        append_tag_component(&mut message, Some(service_label));
        append_tag_component(&mut message, open_frame.authority.as_deref());
        append_tag_component(&mut message, open_frame.target.as_deref());
        append_tag_component(&mut message, open_frame.client_id.as_deref());
        append_tag_component(&mut message, open_frame.route_policy_id.as_deref());
        append_tag_component(&mut message, open_frame.exit_country.as_deref());
        append_tag_component(&mut message, self.session_id.as_deref());

        let mac = hmac_sha256_128(guard_key.as_bytes(), &message);
        Some(hex::encode_upper(mac))
    }
}

struct QuicBidirectionalStream {
    send: quinn::SendStream,
    recv: quinn::RecvStream,
}

impl QuicBidirectionalStream {
    fn new(send: quinn::SendStream, recv: quinn::RecvStream) -> Self {
        Self { send, recv }
    }

    async fn shutdown(&mut self) {
        let _ = self.send.finish();
        let _ = self.recv.stop(VarInt::from_u32(0));
    }
}

impl AsyncRead for QuicBidirectionalStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        Pin::new(&mut self.recv).poll_read(cx, buf)
    }
}

impl AsyncWrite for QuicBidirectionalStream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        match Pin::new(&mut self.send).poll_write(cx, buf) {
            Poll::Ready(Ok(len)) => Poll::Ready(Ok(len)),
            Poll::Ready(Err(err)) => Poll::Ready(Err(io::Error::other(err))),
            Poll::Pending => Poll::Pending,
        }
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        match Pin::new(&mut self.send).poll_flush(cx) {
            Poll::Ready(Ok(())) => Poll::Ready(Ok(())),
            Poll::Ready(Err(err)) => Poll::Ready(Err(io::Error::other(err))),
            Poll::Pending => Poll::Pending,
        }
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        match Pin::new(&mut self.send).poll_shutdown(cx) {
            Poll::Ready(Ok(())) => Poll::Ready(Ok(())),
            Poll::Ready(Err(err)) => Poll::Ready(Err(io::Error::other(err))),
            Poll::Pending => Poll::Pending,
        }
    }
}

fn append_tag_component(buffer: &mut Vec<u8>, value: Option<&str>) {
    buffer.push(CACHE_TAG_FIELD_SEPARATOR);
    if let Some(raw) = value {
        let trimmed = raw.trim();
        if !trimmed.is_empty() {
            buffer.extend_from_slice(trimmed.as_bytes());
        }
    }
}

fn hmac_sha256_128(key: &[u8], message: &[u8]) -> [u8; 16] {
    const BLOCK: usize = 64;
    let mut key_block = [0u8; BLOCK];
    if key.len() > BLOCK {
        let digest = Sha256::digest(key);
        key_block[..digest.len()].copy_from_slice(&digest);
    } else {
        key_block[..key.len()].copy_from_slice(key);
    }

    let mut o_key_pad = [0u8; BLOCK];
    let mut i_key_pad = [0u8; BLOCK];
    for i in 0..BLOCK {
        o_key_pad[i] = key_block[i] ^ 0x5c;
        i_key_pad[i] = key_block[i] ^ 0x36;
    }

    let mut inner = Sha256::new();
    inner.update(i_key_pad);
    inner.update(message);
    let inner_sum = inner.finalize();

    let mut outer = Sha256::new();
    outer.update(o_key_pad);
    outer.update(inner_sum);
    let mac = outer.finalize();

    let mut truncated = [0u8; 16];
    truncated.copy_from_slice(&mac[..16]);
    truncated
}

/// Spawn a QUIC proxy bound to the configured address.
pub fn spawn_local_quic_proxy(
    config: LocalQuicProxyConfig,
) -> Result<LocalQuicProxyHandle, ProxyError> {
    let bind_addr = config.parsed_bind_addr()?;
    let guard_cache_key = config.guard_cache_key()?;

    let sans = vec!["localhost".to_string(), bind_addr.ip().to_string()];
    let rcgen::CertifiedKey { cert, signing_key } =
        generate_simple_self_signed(sans).map_err(ProxyError::Certificate)?;
    let cert_der: CertificateDer<'static> = cert.der().clone();
    let private_key = PrivateKeyDer::try_from(signing_key.serialize_der())
        .map_err(|err| ProxyError::PrivateKey(err.to_string()))?;

    let mut server_config = ServerConfig::with_single_cert(vec![cert_der.clone()], private_key)
        .map_err(|err| ProxyError::QuinnConfig(err.to_string()))?;
    server_config.transport = Arc::new(quinn::TransportConfig::default());

    let endpoint = Endpoint::server(server_config, bind_addr)
        .map_err(|err| ProxyError::QuinnEndpoint(err.to_string()))?;
    let local_addr = endpoint
        .local_addr()
        .map_err(|err| ProxyError::QuinnEndpoint(err.to_string()))?;

    let certificate_pem = cert.pem().to_string();
    let browser_manifest = build_manifest_template(
        &config,
        local_addr,
        &certificate_pem,
        cert_der.as_ref(),
        guard_cache_key.as_ref(),
    );

    let telemetry_label = config
        .telemetry_label
        .clone()
        .unwrap_or_else(|| "proxy".to_string());

    let bridge_config = Arc::new(ProxyBridgeConfig::from_config(&config));
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let join_handle = tokio::spawn({
        let guard_cache_key = guard_cache_key.clone();
        let bridge_config = bridge_config.clone();
        run_accept_loop(
            endpoint.clone(),
            telemetry_label.clone(),
            browser_manifest.clone(),
            config.proxy_mode.clone(),
            guard_cache_key,
            bridge_config,
            shutdown_rx,
        )
    });

    Ok(LocalQuicProxyHandle {
        inner: Arc::new(LocalQuicProxyInner {
            endpoint,
            local_addr,
            certificate_pem,
            browser_manifest,
            mode: config.proxy_mode.clone(),
            guard_cache_key,
            shutdown_tx,
            join_handle: Mutex::new(Some(join_handle)),
        }),
    })
}

async fn run_accept_loop(
    endpoint: Endpoint,
    telemetry_label: String,
    manifest_template: Option<BrowserManifestTemplate>,
    mode: ProxyMode,
    guard_cache_key: Option<GuardCacheKey>,
    bridge_config: Arc<ProxyBridgeConfig>,
    mut shutdown_rx: watch::Receiver<bool>,
) {
    loop {
        let connecting_opt = tokio::select! {
            biased;
            changed = shutdown_rx.changed() => {
                if changed.is_ok() {
                    info!(target: "soranet.proxy", label = telemetry_label, "shutting down local QUIC proxy");
                }
                break;
            }
            incoming = endpoint.accept() => incoming,
        };

        let Some(incoming) = connecting_opt else {
            break;
        };

        match incoming.accept() {
            Ok(connecting) => {
                let manifest_template = manifest_template.clone();
                let mode = mode.clone();
                let guard_cache_key = guard_cache_key.clone();
                let label = telemetry_label.clone();
                let bridge_config = bridge_config.clone();
                tokio::spawn(async move {
                    match connecting.await {
                        Ok(new_conn) => {
                            record_transport_event(&label, "connection_open", "accepted");
                            if let Err(err) = handle_connection(
                                new_conn,
                                manifest_template,
                                mode,
                                guard_cache_key,
                                bridge_config,
                                &label,
                            )
                            .await
                            {
                                warn!(target: "soranet.proxy", ?err, label = label, "proxy handshake failed");
                                record_transport_event(&label, "handshake_error", &err.to_string());
                            }
                        }
                        Err(err) => {
                            warn!(target: "soranet.proxy", ?err, label = label, "incoming connection failed");
                            record_transport_event(&label, "connection_error", &err.to_string());
                        }
                    }
                });
            }
            Err(err) => {
                warn!(target: "soranet.proxy", ?err, label = telemetry_label, "incoming connection failed");
                record_transport_event(&telemetry_label, "connection_error", &err.to_string());
            }
        }
    }
}

async fn handle_connection(
    connection: quinn::Connection,
    manifest_template: Option<BrowserManifestTemplate>,
    mode: ProxyMode,
    guard_cache_key: Option<GuardCacheKey>,
    bridge_config: Arc<ProxyBridgeConfig>,
    telemetry_label: &str,
) -> Result<(), ProxyError> {
    let (mut send_stream, mut recv_stream) = connection
        .accept_bi()
        .await
        .map_err(|err| ProxyError::Handshake(err.to_string()))?;

    let handshake_bytes = read_frame(&mut recv_stream)
        .await
        .map_err(|err| ProxyError::Handshake(err.to_string()))?;
    let handshake: ProxyHandshakeV1 = decode_from_bytes(&handshake_bytes)
        .map_err(|err| ProxyError::Handshake(err.to_string()))?;

    if handshake.version != PROXY_HANDSHAKE_VERSION {
        let ack = ProxyHandshakeAckV1 {
            version: PROXY_HANDSHAKE_VERSION,
            accepted: false,
            message: Some("unsupported version".to_string()),
            manifest: None,
        };
        write_frame(&mut send_stream, &ack)
            .await
            .map_err(|err| ProxyError::Handshake(err.to_string()))?;
        let _ = send_stream.finish();
        let _ = recv_stream.stop(VarInt::from_u32(0));
        record_transport_event(telemetry_label, "handshake_reject", "unsupported_version");
        return Ok(());
    }

    record_transport_event(telemetry_label, "handshake_accept", "ok");
    let manifest = manifest_template
        .as_ref()
        .map(BrowserManifestTemplate::instantiate);
    let session = Arc::new(ProxySession {
        telemetry_label: telemetry_label.to_string(),
        mode: mode.clone(),
        guard_cache_key,
        session_id: manifest.as_ref().and_then(|m| m.session_id.clone()),
        cache_tags: manifest.as_ref().and_then(CacheTagContext::from_manifest),
        bridge: bridge_config,
    });
    let ack = ProxyHandshakeAckV1 {
        version: PROXY_HANDSHAKE_VERSION,
        accepted: true,
        message: None,
        manifest,
    };
    write_frame(&mut send_stream, &ack)
        .await
        .map_err(ProxyError::Handshake)?;
    if let Err(err) = send_stream.finish() {
        record_transport_event(telemetry_label, "handshake_close_error", &err.to_string());
        return Err(ProxyError::Handshake(err.to_string()));
    }

    if matches!(session.mode, ProxyMode::MetadataOnly) {
        record_transport_event(telemetry_label, "bridge_disabled", "metadata_only");
        return Ok(());
    }

    loop {
        match connection.accept_bi().await {
            Ok((stream_send, stream_recv)) => {
                record_transport_event(telemetry_label, "stream_open", "accepted");
                let session = session.clone();
                let label = session.telemetry_label.clone();
                tokio::spawn(async move {
                    if let Err(err) =
                        handle_application_stream(stream_send, stream_recv, session.clone()).await
                    {
                        warn!(target: "soranet.proxy", ?err, label = label, "application stream failed");
                        record_transport_event(&label, "stream_error", &err.to_string());
                    }
                });
            }
            Err(ConnectionError::LocallyClosed) => break,
            Err(ConnectionError::ApplicationClosed { .. }) => break,
            Err(err) => {
                warn!(target: "soranet.proxy", ?err, label = telemetry_label, "stream accept loop terminated");
                record_transport_event(telemetry_label, "stream_accept_error", &err.to_string());
                break;
            }
        }
    }

    Ok(())
}

#[derive(Debug, Error)]
enum FrameError {
    #[error("frame length {len} exceeds max {max} bytes")]
    FrameTooLarge { len: usize, max: usize },
    #[error("frame read error: {0}")]
    Read(#[from] quinn::ReadExactError),
}

async fn read_frame(stream: &mut quinn::RecvStream) -> Result<Vec<u8>, FrameError> {
    let mut len_buf = [0u8; 4];
    stream.read_exact(&mut len_buf).await?;
    let len = u32::from_be_bytes(len_buf) as usize;
    if len > PROXY_MAX_FRAME_BYTES {
        return Err(FrameError::FrameTooLarge {
            len,
            max: PROXY_MAX_FRAME_BYTES,
        });
    }
    let mut buf = vec![0u8; len];
    stream.read_exact(&mut buf).await?;
    Ok(buf)
}

async fn write_frame<T: NoritoSerialize>(
    stream: &mut quinn::SendStream,
    payload: &T,
) -> Result<(), String> {
    let bytes = to_bytes(payload).map_err(|err| err.to_string())?;
    let len = bytes.len() as u32;
    stream
        .write_all(&len.to_be_bytes())
        .await
        .map_err(|err| err.to_string())?;
    stream
        .write_all(&bytes)
        .await
        .map_err(|err| err.to_string())?;
    stream.flush().await.map_err(|err| err.to_string())?;
    Ok(())
}

async fn handle_application_stream(
    mut send_stream: quinn::SendStream,
    mut recv_stream: quinn::RecvStream,
    session: Arc<ProxySession>,
) -> Result<(), ProxyError> {
    let telemetry_label = session.telemetry_label.as_str();
    let frame_bytes = read_frame(&mut recv_stream)
        .await
        .map_err(|err| ProxyError::Stream(err.to_string()))?;
    let open_frame: ProxyStreamOpenV1 =
        decode_from_bytes(&frame_bytes).map_err(|err| ProxyError::Stream(err.to_string()))?;

    if open_frame.version != PROXY_STREAM_VERSION {
        record_transport_event(telemetry_label, "stream_reject", "unsupported_version");
        let ack = ProxyStreamAckV1 {
            version: PROXY_STREAM_VERSION,
            code: STREAM_ACK_UNSUPPORTED_VERSION,
            accepted: false,
            message: Some("unsupported version".into()),
            cache_tag_hex: None,
        };
        write_frame(&mut send_stream, &ack)
            .await
            .map_err(ProxyError::Stream)?;
        let _ = send_stream.finish();
        let _ = recv_stream.stop(VarInt::from_u32(0));
        return Ok(());
    }

    let service = ProxyStreamService::from_label(&open_frame.service);

    if matches!(session.mode, ProxyMode::MetadataOnly) {
        record_transport_event(telemetry_label, "stream_reject", "mode_metadata_only");
        let ack = ProxyStreamAckV1 {
            version: PROXY_STREAM_VERSION,
            code: STREAM_ACK_MODE_DISABLED,
            accepted: false,
            message: Some("proxy running in metadata-only mode".into()),
            cache_tag_hex: None,
        };
        write_frame(&mut send_stream, &ack)
            .await
            .map_err(ProxyError::Stream)?;
        let _ = send_stream.finish();
        let _ = recv_stream.stop(VarInt::from_u32(0));
        return Ok(());
    }

    if service == ProxyStreamService::Unknown {
        record_transport_event(telemetry_label, "stream_reject", "unsupported_service");
        let ack = ProxyStreamAckV1 {
            version: PROXY_STREAM_VERSION,
            code: STREAM_ACK_UNSUPPORTED_SERVICE,
            accepted: false,
            message: Some(format!("unsupported service `{}`", open_frame.service)),
            cache_tag_hex: None,
        };
        write_frame(&mut send_stream, &ack)
            .await
            .map_err(ProxyError::Stream)?;
        let _ = send_stream.finish();
        let _ = recv_stream.stop(VarInt::from_u32(0));
        return Ok(());
    }

    match service {
        ProxyStreamService::Tcp => {
            handle_tcp_stream(send_stream, recv_stream, session, open_frame).await
        }
        ProxyStreamService::Norito => {
            handle_norito_stream(send_stream, recv_stream, session, open_frame).await
        }
        ProxyStreamService::Car => {
            handle_car_stream(send_stream, recv_stream, session, open_frame).await
        }
        ProxyStreamService::Kaigi => {
            handle_kaigi_stream(send_stream, recv_stream, session, open_frame).await
        }
        ProxyStreamService::Unknown => unreachable!("unknown service handled earlier"),
    }
}

fn record_transport_event(label: &str, event: &str, reason: &str) {
    let metrics = global_or_default();
    metrics.inc_sorafs_orchestrator_transport_event(label, PROXY_PROTOCOL_LABEL, event, reason);
    let otel = global_sorafs_fetch_otel();
    otel.record_transport_event(
        PROXY_MANIFEST_ID,
        label,
        None::<&str>,
        PROXY_PROTOCOL_LABEL,
        event,
        reason,
    );
}

async fn send_failure_ack(
    mut send_stream: quinn::SendStream,
    mut recv_stream: quinn::RecvStream,
    ack: ProxyStreamAckV1,
) -> Result<(), ProxyError> {
    write_frame(&mut send_stream, &ack)
        .await
        .map_err(ProxyError::Stream)?;
    let _ = send_stream.finish();
    let _ = recv_stream.stop(VarInt::from_u32(0));
    Ok(())
}

async fn handle_tcp_stream(
    mut send_stream: quinn::SendStream,
    recv_stream: quinn::RecvStream,
    session: Arc<ProxySession>,
    open_frame: ProxyStreamOpenV1,
) -> Result<(), ProxyError> {
    let telemetry_label = session.telemetry_label.as_str();
    let Some(authority) = open_frame
        .authority
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string())
    else {
        record_transport_event(telemetry_label, "stream_reject", "tcp_missing_authority");
        return send_failure_ack(
            send_stream,
            recv_stream,
            ProxyStreamAckV1 {
                version: PROXY_STREAM_VERSION,
                code: STREAM_ACK_BAD_REQUEST,
                accepted: false,
                message: Some("tcp stream requires authority".into()),
                cache_tag_hex: None,
            },
        )
        .await;
    };

    let mut tcp_stream = match TcpStream::connect(&authority).await {
        Ok(stream) => stream,
        Err(err) => {
            record_transport_event(telemetry_label, "stream_reject", "tcp_connect_failed");
            warn!(
                target: "soranet.proxy",
                ?err,
                label = telemetry_label,
                %authority,
                "failed to connect tcp authority"
            );
            return send_failure_ack(
                send_stream,
                recv_stream,
                ProxyStreamAckV1 {
                    version: PROXY_STREAM_VERSION,
                    code: STREAM_ACK_INTERNAL_ERROR,
                    accepted: false,
                    message: Some(format!("failed to connect `{authority}`: {err}")),
                    cache_tag_hex: None,
                },
            )
            .await;
        }
    };

    let cache_tag = session.cache_tag_for(ProxyStreamService::Tcp, &open_frame);
    let ack = ProxyStreamAckV1 {
        version: PROXY_STREAM_VERSION,
        code: STREAM_ACK_OK,
        accepted: true,
        message: None,
        cache_tag_hex: cache_tag,
    };
    write_frame(&mut send_stream, &ack)
        .await
        .map_err(|err| ProxyError::Stream(err.to_string()))?;

    let mut quic_stream = QuicBidirectionalStream::new(send_stream, recv_stream);
    match tokio::io::copy_bidirectional(&mut quic_stream, &mut tcp_stream).await {
        Ok((client_bytes, remote_bytes)) => {
            record_transport_event(telemetry_label, "stream_complete", "tcp_ok");
            info!(
                target: "soranet.proxy",
                label = telemetry_label,
                %authority,
                client_bytes,
                remote_bytes,
                "proxied tcp stream"
            );
        }
        Err(err) => {
            record_transport_event(telemetry_label, "stream_error", "tcp_proxy_failed");
            quic_stream.shutdown().await;
            let _ = tcp_stream.shutdown().await;
            return Err(ProxyError::Stream(err.to_string()));
        }
    }

    quic_stream.shutdown().await;
    let _ = tcp_stream.shutdown().await;
    Ok(())
}

async fn handle_norito_stream(
    mut send_stream: quinn::SendStream,
    mut recv_stream: quinn::RecvStream,
    session: Arc<ProxySession>,
    open_frame: ProxyStreamOpenV1,
) -> Result<(), ProxyError> {
    let telemetry_label = session.telemetry_label.as_str();
    let Some(bridge) = session.bridge.norito() else {
        record_transport_event(telemetry_label, "stream_reject", "norito_bridge_disabled");
        return send_failure_ack(
            send_stream,
            recv_stream,
            ProxyStreamAckV1 {
                version: PROXY_STREAM_VERSION,
                code: STREAM_ACK_NOT_IMPLEMENTED,
                accepted: false,
                message: Some("norito bridge not configured".into()),
                cache_tag_hex: None,
            },
        )
        .await;
    };

    let Some(target) = extract_stream_target(&open_frame) else {
        record_transport_event(telemetry_label, "stream_reject", "norito_missing_target");
        return send_failure_ack(
            send_stream,
            recv_stream,
            ProxyStreamAckV1 {
                version: PROXY_STREAM_VERSION,
                code: STREAM_ACK_BAD_REQUEST,
                accepted: false,
                message: Some("norito stream requires target".into()),
                cache_tag_hex: None,
            },
        )
        .await;
    };

    let path = match bridge.resolve_target(target) {
        Ok(path) => path,
        Err(reason) => {
            record_transport_event(telemetry_label, "stream_reject", "norito_invalid_target");
            return send_failure_ack(
                send_stream,
                recv_stream,
                ProxyStreamAckV1 {
                    version: PROXY_STREAM_VERSION,
                    code: STREAM_ACK_BAD_REQUEST,
                    accepted: false,
                    message: Some(format!("invalid norito target: {reason}")),
                    cache_tag_hex: None,
                },
            )
            .await;
        }
    };

    let mut file = match fs::File::open(&path).await {
        Ok(file) => file,
        Err(err) if err.kind() == io::ErrorKind::NotFound => {
            record_transport_event(telemetry_label, "stream_reject", "norito_not_found");
            return send_failure_ack(
                send_stream,
                recv_stream,
                ProxyStreamAckV1 {
                    version: PROXY_STREAM_VERSION,
                    code: STREAM_ACK_BAD_REQUEST,
                    accepted: false,
                    message: Some(format!("norito payload `{}` not found", path.display())),
                    cache_tag_hex: None,
                },
            )
            .await;
        }
        Err(err) => {
            record_transport_event(telemetry_label, "stream_reject", "norito_open_failed");
            return send_failure_ack(
                send_stream,
                recv_stream,
                ProxyStreamAckV1 {
                    version: PROXY_STREAM_VERSION,
                    code: STREAM_ACK_INTERNAL_ERROR,
                    accepted: false,
                    message: Some(format!(
                        "failed to open norito payload `{}`: {err}",
                        path.display()
                    )),
                    cache_tag_hex: None,
                },
            )
            .await;
        }
    };

    let metadata = match file.metadata().await {
        Ok(meta) => meta,
        Err(err) => {
            record_transport_event(telemetry_label, "stream_reject", "norito_metadata_failed");
            return send_failure_ack(
                send_stream,
                recv_stream,
                ProxyStreamAckV1 {
                    version: PROXY_STREAM_VERSION,
                    code: STREAM_ACK_INTERNAL_ERROR,
                    accepted: false,
                    message: Some(format!(
                        "failed to fetch norito metadata `{}`: {err}",
                        path.display()
                    )),
                    cache_tag_hex: None,
                },
            )
            .await;
        }
    };

    if !metadata.is_file() {
        record_transport_event(telemetry_label, "stream_reject", "norito_not_file");
        return send_failure_ack(
            send_stream,
            recv_stream,
            ProxyStreamAckV1 {
                version: PROXY_STREAM_VERSION,
                code: STREAM_ACK_BAD_REQUEST,
                accepted: false,
                message: Some(format!("norito target `{}` is not a file", path.display())),
                cache_tag_hex: None,
            },
        )
        .await;
    }

    let cache_tag = session.cache_tag_for(ProxyStreamService::Norito, &open_frame);
    let ack = ProxyStreamAckV1 {
        version: PROXY_STREAM_VERSION,
        code: STREAM_ACK_OK,
        accepted: true,
        message: None,
        cache_tag_hex: cache_tag,
    };
    write_frame(&mut send_stream, &ack)
        .await
        .map_err(|err| ProxyError::Stream(err.to_string()))?;

    match tokio::io::copy(&mut file, &mut send_stream).await {
        Ok(bytes) => {
            record_transport_event(telemetry_label, "stream_complete", "norito_ok");
            info!(
                target: "soranet.proxy",
                label = telemetry_label,
                target = %path.display(),
                bytes,
                "streamed norito payload"
            );
            let _ = send_stream.finish();
            let _ = recv_stream.stop(VarInt::from_u32(0));
            Ok(())
        }
        Err(err) => {
            record_transport_event(telemetry_label, "stream_error", "norito_stream_failed");
            let _ = send_stream.finish();
            let _ = recv_stream.stop(VarInt::from_u32(0));
            Err(ProxyError::Stream(err.to_string()))
        }
    }
}

async fn handle_kaigi_stream(
    mut send_stream: quinn::SendStream,
    mut recv_stream: quinn::RecvStream,
    session: Arc<ProxySession>,
    open_frame: ProxyStreamOpenV1,
) -> Result<(), ProxyError> {
    let telemetry_label = session.telemetry_label.as_str();
    let Some(bridge) = session.bridge.kaigi() else {
        record_transport_event(telemetry_label, "stream_reject", "kaigi_bridge_disabled");
        return send_failure_ack(
            send_stream,
            recv_stream,
            ProxyStreamAckV1 {
                version: PROXY_STREAM_VERSION,
                code: STREAM_ACK_NOT_IMPLEMENTED,
                accepted: false,
                message: Some("kaigi bridge not configured".into()),
                cache_tag_hex: None,
            },
        )
        .await;
    };

    let Some(target) = extract_stream_target(&open_frame) else {
        record_transport_event(telemetry_label, "stream_reject", "kaigi_missing_target");
        return send_failure_ack(
            send_stream,
            recv_stream,
            ProxyStreamAckV1 {
                version: PROXY_STREAM_VERSION,
                code: STREAM_ACK_BAD_REQUEST,
                accepted: false,
                message: Some("kaigi stream requires target".into()),
                cache_tag_hex: None,
            },
        )
        .await;
    };

    let path = match bridge.resolve_target(target) {
        Ok(path) => path,
        Err(reason) => {
            record_transport_event(telemetry_label, "stream_reject", "kaigi_invalid_target");
            return send_failure_ack(
                send_stream,
                recv_stream,
                ProxyStreamAckV1 {
                    version: PROXY_STREAM_VERSION,
                    code: STREAM_ACK_BAD_REQUEST,
                    accepted: false,
                    message: Some(format!("invalid kaigi target: {reason}")),
                    cache_tag_hex: None,
                },
            )
            .await;
        }
    };

    let mut file = match fs::File::open(&path).await {
        Ok(file) => file,
        Err(err) if err.kind() == io::ErrorKind::NotFound => {
            record_transport_event(telemetry_label, "stream_reject", "kaigi_not_found");
            return send_failure_ack(
                send_stream,
                recv_stream,
                ProxyStreamAckV1 {
                    version: PROXY_STREAM_VERSION,
                    code: STREAM_ACK_BAD_REQUEST,
                    accepted: false,
                    message: Some(format!("kaigi payload `{}` not found", path.display())),
                    cache_tag_hex: None,
                },
            )
            .await;
        }
        Err(err) => {
            record_transport_event(telemetry_label, "stream_reject", "kaigi_open_failed");
            return send_failure_ack(
                send_stream,
                recv_stream,
                ProxyStreamAckV1 {
                    version: PROXY_STREAM_VERSION,
                    code: STREAM_ACK_INTERNAL_ERROR,
                    accepted: false,
                    message: Some(format!("failed to open kaigi payload: {err}")),
                    cache_tag_hex: None,
                },
            )
            .await;
        }
    };

    let mut payload = Vec::new();
    file.read_to_end(&mut payload).await.map_err(|err| {
        record_transport_event(telemetry_label, "stream_reject", "kaigi_read_failed");
        ProxyError::Stream(err.to_string())
    })?;

    let room_policy = bridge.room_policy().as_label().to_string();
    let cache_tag = session.cache_tag_for(ProxyStreamService::Kaigi, &open_frame);
    let ack = ProxyStreamAckV1 {
        version: PROXY_STREAM_VERSION,
        code: STREAM_ACK_OK,
        accepted: true,
        message: Some(format!("room-policy={room_policy}")),
        cache_tag_hex: cache_tag,
    };
    write_frame(&mut send_stream, &ack)
        .await
        .map_err(ProxyError::Stream)?;
    send_stream
        .write_all(&payload)
        .await
        .map_err(|err| ProxyError::Stream(err.to_string()))?;
    send_stream
        .finish()
        .map_err(|err| ProxyError::Stream(err.to_string()))?;
    recv_stream
        .stop(VarInt::from_u32(0))
        .map_err(|err| ProxyError::Stream(err.to_string()))?;

    record_transport_event(telemetry_label, "stream_complete", "kaigi_ok");
    Ok(())
}

async fn handle_car_stream(
    mut send_stream: quinn::SendStream,
    mut recv_stream: quinn::RecvStream,
    session: Arc<ProxySession>,
    open_frame: ProxyStreamOpenV1,
) -> Result<(), ProxyError> {
    let telemetry_label = session.telemetry_label.as_str();
    let Some(bridge) = session.bridge.car() else {
        record_transport_event(telemetry_label, "stream_reject", "car_bridge_disabled");
        return send_failure_ack(
            send_stream,
            recv_stream,
            ProxyStreamAckV1 {
                version: PROXY_STREAM_VERSION,
                code: STREAM_ACK_NOT_IMPLEMENTED,
                accepted: false,
                message: Some("car bridge not configured".into()),
                cache_tag_hex: None,
            },
        )
        .await;
    };

    let Some(target) = extract_stream_target(&open_frame) else {
        record_transport_event(telemetry_label, "stream_reject", "car_missing_target");
        return send_failure_ack(
            send_stream,
            recv_stream,
            ProxyStreamAckV1 {
                version: PROXY_STREAM_VERSION,
                code: STREAM_ACK_BAD_REQUEST,
                accepted: false,
                message: Some("car stream requires target".into()),
                cache_tag_hex: None,
            },
        )
        .await;
    };

    let path = match bridge.resolve_target(target) {
        Ok(path) => path,
        Err(reason) => {
            record_transport_event(telemetry_label, "stream_reject", "car_invalid_target");
            return send_failure_ack(
                send_stream,
                recv_stream,
                ProxyStreamAckV1 {
                    version: PROXY_STREAM_VERSION,
                    code: STREAM_ACK_BAD_REQUEST,
                    accepted: false,
                    message: Some(format!("invalid car target: {reason}")),
                    cache_tag_hex: None,
                },
            )
            .await;
        }
    };

    let mut file = match fs::File::open(&path).await {
        Ok(file) => file,
        Err(err) if err.kind() == io::ErrorKind::NotFound => {
            record_transport_event(telemetry_label, "stream_reject", "car_not_found");
            return send_failure_ack(
                send_stream,
                recv_stream,
                ProxyStreamAckV1 {
                    version: PROXY_STREAM_VERSION,
                    code: STREAM_ACK_BAD_REQUEST,
                    accepted: false,
                    message: Some(format!("car archive `{}` not found", path.display())),
                    cache_tag_hex: None,
                },
            )
            .await;
        }
        Err(err) => {
            record_transport_event(telemetry_label, "stream_reject", "car_open_failed");
            return send_failure_ack(
                send_stream,
                recv_stream,
                ProxyStreamAckV1 {
                    version: PROXY_STREAM_VERSION,
                    code: STREAM_ACK_INTERNAL_ERROR,
                    accepted: false,
                    message: Some(format!(
                        "failed to open car archive `{}`: {err}",
                        path.display()
                    )),
                    cache_tag_hex: None,
                },
            )
            .await;
        }
    };

    let metadata = match file.metadata().await {
        Ok(meta) => meta,
        Err(err) => {
            record_transport_event(telemetry_label, "stream_reject", "car_metadata_failed");
            return send_failure_ack(
                send_stream,
                recv_stream,
                ProxyStreamAckV1 {
                    version: PROXY_STREAM_VERSION,
                    code: STREAM_ACK_INTERNAL_ERROR,
                    accepted: false,
                    message: Some(format!(
                        "failed to fetch car metadata `{}`: {err}",
                        path.display()
                    )),
                    cache_tag_hex: None,
                },
            )
            .await;
        }
    };

    if !metadata.is_file() {
        record_transport_event(telemetry_label, "stream_reject", "car_not_file");
        return send_failure_ack(
            send_stream,
            recv_stream,
            ProxyStreamAckV1 {
                version: PROXY_STREAM_VERSION,
                code: STREAM_ACK_BAD_REQUEST,
                accepted: false,
                message: Some(format!("car target `{}` is not a file", path.display())),
                cache_tag_hex: None,
            },
        )
        .await;
    }

    let cache_tag = session.cache_tag_for(ProxyStreamService::Car, &open_frame);
    let ack = ProxyStreamAckV1 {
        version: PROXY_STREAM_VERSION,
        code: STREAM_ACK_OK,
        accepted: true,
        message: None,
        cache_tag_hex: cache_tag,
    };
    write_frame(&mut send_stream, &ack)
        .await
        .map_err(|err| ProxyError::Stream(err.to_string()))?;

    match tokio::io::copy(&mut file, &mut send_stream).await {
        Ok(bytes) => {
            record_transport_event(telemetry_label, "stream_complete", "car_ok");
            info!(
                target: "soranet.proxy",
                label = telemetry_label,
                target = %path.display(),
                bytes,
                "streamed car archive"
            );
            let _ = send_stream.finish();
            let _ = recv_stream.stop(VarInt::from_u32(0));
            Ok(())
        }
        Err(err) => {
            record_transport_event(telemetry_label, "stream_error", "car_stream_failed");
            let _ = send_stream.finish();
            let _ = recv_stream.stop(VarInt::from_u32(0));
            Err(ProxyError::Stream(err.to_string()))
        }
    }
}

fn extract_stream_target(open_frame: &ProxyStreamOpenV1) -> Option<&str> {
    open_frame
        .target
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn generate_session_id() -> String {
    let mut bytes = [0u8; PROXY_SESSION_ID_LEN];
    let mut rng = OsRng;
    rng.try_fill_bytes(&mut bytes)
        .expect("os rng available for session id");
    bytes.encode_hex::<String>()
}

fn generate_cache_salt() -> String {
    let mut bytes = [0u8; PROXY_CACHE_TAG_SALT_LEN];
    let mut rng = OsRng;
    rng.try_fill_bytes(&mut bytes)
        .expect("os rng available for cache salt");
    bytes.encode_hex::<String>()
}

/// Proxy handshake payload dispatched by clients.
#[derive(Debug, NoritoSerialize, NoritoDeserialize)]
struct ProxyHandshakeV1 {
    version: u8,
    #[norito(default)]
    client: Option<String>,
    #[norito(default)]
    user_agent: Option<String>,
}

/// Acknowledgement returned to clients after the handshake completes.
#[derive(Debug, NoritoSerialize)]
struct ProxyHandshakeAckV1 {
    version: u8,
    accepted: bool,
    #[norito(default)]
    message: Option<String>,
    #[norito(default)]
    manifest: Option<BrowserExtensionManifest>,
}

/// Stream open frame dispatched per application channel.
#[derive(Debug, NoritoSerialize, NoritoDeserialize)]
struct ProxyStreamOpenV1 {
    version: u8,
    #[norito(default)]
    service: String,
    #[norito(default)]
    authority: Option<String>,
    #[norito(default)]
    target: Option<String>,
    #[norito(default)]
    route_policy_id: Option<String>,
    #[norito(default)]
    exit_country: Option<String>,
    #[norito(default)]
    client_id: Option<String>,
}

/// Stream acknowledgement returned once the proxy routes a request.
#[derive(Debug, NoritoSerialize)]
struct ProxyStreamAckV1 {
    version: u8,
    code: u8,
    accepted: bool,
    #[norito(default)]
    message: Option<String>,
    #[norito(default)]
    cache_tag_hex: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProxyStreamService {
    Tcp,
    Norito,
    Car,
    Kaigi,
    Unknown,
}

impl ProxyStreamService {
    fn from_label(label: &str) -> Self {
        match label.trim().to_ascii_lowercase().as_str() {
            "tcp" => Self::Tcp,
            "norito" => Self::Norito,
            "car" => Self::Car,
            "kaigi" => Self::Kaigi,
            _ => Self::Unknown,
        }
    }

    fn as_label(&self) -> &'static str {
        match self {
            Self::Tcp => "tcp",
            Self::Norito => "norito",
            Self::Car => "car",
            Self::Kaigi => "kaigi",
            Self::Unknown => "unknown",
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{io, net::SocketAddr, sync::Arc, time::Duration};

    use tempfile::TempDir;
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
        task::JoinHandle,
    };

    use super::*;

    const TEST_GUARD_KEY: &str = "0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF";

    use quinn::{
        ClientConfig, Endpoint, ServerConfig, VarInt,
        crypto::rustls::QuicClientConfig as QuinnRustlsClientConfig,
    };
    use rustls::{
        DigitallySignedStruct, Error as RustlsError, SignatureScheme,
        client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier},
        pki_types::{CertificateDer, PrivateKeyDer, ServerName, UnixTime},
    };

    fn should_skip_socket_permission(message: &str) -> bool {
        message.contains("Operation not permitted") || message.contains("Permission denied")
    }

    fn spawn_proxy_or_skip(config: LocalQuicProxyConfig) -> Option<LocalQuicProxyHandle> {
        match spawn_local_quic_proxy(config) {
            Ok(handle) => Some(handle),
            Err(ProxyError::QuinnEndpoint(message)) if should_skip_socket_permission(&message) => {
                eprintln!("skipping proxy test (loopback socket unavailable): {message}");
                None
            }
            Err(err) => panic!("spawn proxy: {err}"),
        }
    }

    fn sample_certificate() -> (String, Vec<u8>) {
        let rcgen::CertifiedKey { cert, .. } =
            generate_simple_self_signed(vec!["localhost".into()]).expect("generate certificate");
        let cert_pem = cert.pem().to_string();
        let cert_der = cert.der().to_vec();
        (cert_pem, cert_der)
    }

    fn spawn_frame_server_or_skip() -> Option<(
        Endpoint,
        SocketAddr,
        JoinHandle<Result<Vec<u8>, FrameError>>,
    )> {
        let rcgen::CertifiedKey { cert, signing_key } =
            generate_simple_self_signed(vec!["localhost".into()]).expect("generate certificate");
        let cert_der: CertificateDer<'static> = cert.der().clone();
        let private_key =
            PrivateKeyDer::try_from(signing_key.serialize_der()).expect("private key");
        let mut server_config =
            ServerConfig::with_single_cert(vec![cert_der], private_key).expect("server config");
        server_config.transport = Arc::new(quinn::TransportConfig::default());

        let endpoint = match Endpoint::server(server_config, "127.0.0.1:0".parse().expect("addr")) {
            Ok(endpoint) => endpoint,
            Err(err) if should_skip_socket_permission(&err.to_string()) => {
                eprintln!("skipping frame test (loopback socket unavailable): {err}");
                return None;
            }
            Err(err) => panic!("spawn frame server: {err}"),
        };
        let local_addr = endpoint.local_addr().expect("local addr");
        let endpoint_handle = endpoint.clone();
        let join_handle = tokio::spawn(async move {
            let incoming = endpoint_handle.accept().await.expect("accept incoming");
            let connecting = incoming.accept().expect("accept connection");
            let connection = connecting.await.expect("connect");
            let (_send, mut recv) = connection.accept_bi().await.expect("accept stream");
            read_frame(&mut recv).await
        });

        Some((endpoint, local_addr, join_handle))
    }

    #[test]
    fn manifest_template_populates_expected_fields() {
        let config = LocalQuicProxyConfig {
            telemetry_label: Some("dev-proxy".into()),
            proxy_mode: ProxyMode::Bridge,
            ..LocalQuicProxyConfig::default()
        };
        let guard_key = GuardCacheKey::from_bytes([0xAB; GuardCacheKey::LENGTH]);
        let (cert_pem, cert_der) = sample_certificate();
        let addr: SocketAddr = "127.0.0.1:4433".parse().expect("addr");

        let template =
            build_manifest_template(&config, addr, &cert_pem, &cert_der, Some(&guard_key))
                .expect("manifest template");
        let manifest = template.preview();

        let expected_fingerprint = {
            let mut hasher = Sha256::new();
            hasher.update(&cert_der);
            hex::encode(hasher.finalize())
        };

        let expected_caps: Vec<String> = PROXY_CAPABILITIES
            .iter()
            .map(|cap| cap.to_string())
            .collect();

        assert_eq!(manifest.version, 2);
        assert_eq!(manifest.alpn.as_deref(), Some(PROXY_ALPN_LABEL));
        assert_eq!(manifest.capabilities, expected_caps);
        assert_eq!(manifest.proxy_mode, ProxyMode::Bridge);
        assert_eq!(
            manifest.guard_cache_key_hex.as_deref(),
            Some(guard_key.to_hex().as_str())
        );
        assert_eq!(
            manifest.cert_fingerprint_hex.as_deref(),
            Some(expected_fingerprint.as_str())
        );
        assert_eq!(expected_fingerprint.len(), 64);
        let session = manifest.session_id.expect("session id");
        assert_eq!(session.len(), PROXY_SESSION_ID_LEN * 2);
        let cache = manifest.cache_tagging.expect("cache tagging");
        let salt = cache.salt_hex.expect("salt");
        assert_eq!(salt.len(), PROXY_CACHE_TAG_SALT_LEN * 2);
    }

    #[test]
    fn cache_tag_generation_matches_expected() {
        let config = LocalQuicProxyConfig {
            telemetry_label: Some("dev-proxy".into()),
            proxy_mode: ProxyMode::Bridge,
            ..LocalQuicProxyConfig::default()
        };
        let guard_key = GuardCacheKey::from_bytes([0x11; GuardCacheKey::LENGTH]);
        let (cert_pem, cert_der) = sample_certificate();
        let addr: SocketAddr = "127.0.0.1:9443".parse().expect("addr");

        let template =
            build_manifest_template(&config, addr, &cert_pem, &cert_der, Some(&guard_key))
                .expect("manifest template");
        let manifest = template.preview();
        let cache_context =
            CacheTagContext::from_manifest(&manifest).expect("cache context present");
        let expected_context = cache_context.clone();
        let bridge_config = Arc::new(ProxyBridgeConfig::from_config(&config));

        let session = ProxySession {
            telemetry_label: "test".into(),
            mode: ProxyMode::Bridge,
            guard_cache_key: Some(guard_key.clone()),
            session_id: manifest.session_id.clone(),
            cache_tags: Some(cache_context),
            bridge: bridge_config,
        };

        let open_frame = ProxyStreamOpenV1 {
            version: PROXY_STREAM_VERSION,
            service: "car".into(),
            authority: Some("torii.dev:8080".into()),
            target: Some("payload.car".into()),
            route_policy_id: Some("policy-alpha".into()),
            exit_country: Some("JP".into()),
            client_id: Some("client-1".into()),
        };

        let actual = session
            .cache_tag_for(ProxyStreamService::Car, &open_frame)
            .expect("cache tag");

        let mut expected_message = Vec::new();
        expected_message.extend_from_slice(&expected_context.salt);
        append_tag_component(&mut expected_message, Some("car"));
        append_tag_component(&mut expected_message, open_frame.authority.as_deref());
        append_tag_component(&mut expected_message, open_frame.target.as_deref());
        append_tag_component(&mut expected_message, open_frame.client_id.as_deref());
        append_tag_component(&mut expected_message, open_frame.route_policy_id.as_deref());
        append_tag_component(&mut expected_message, open_frame.exit_country.as_deref());
        append_tag_component(&mut expected_message, session.session_id.as_deref());

        let expected = hex::encode_upper(hmac_sha256_128(guard_key.as_bytes(), &expected_message));
        assert_eq!(actual, expected);

        let tcp_tag = session
            .cache_tag_for(ProxyStreamService::Tcp, &open_frame)
            .expect("tcp cache tag");
        let mut tcp_message = Vec::new();
        tcp_message.extend_from_slice(&expected_context.salt);
        append_tag_component(&mut tcp_message, Some("tcp"));
        append_tag_component(&mut tcp_message, open_frame.authority.as_deref());
        append_tag_component(&mut tcp_message, open_frame.target.as_deref());
        append_tag_component(&mut tcp_message, open_frame.client_id.as_deref());
        append_tag_component(&mut tcp_message, open_frame.route_policy_id.as_deref());
        append_tag_component(&mut tcp_message, open_frame.exit_country.as_deref());
        append_tag_component(&mut tcp_message, session.session_id.as_deref());
        let expected_tcp = hex::encode_upper(hmac_sha256_128(guard_key.as_bytes(), &tcp_message));
        assert_eq!(tcp_tag, expected_tcp);
    }

    #[test]
    fn proxy_stream_service_lookup_handles_unknown() {
        assert_eq!(
            ProxyStreamService::from_label("tcp"),
            ProxyStreamService::Tcp
        );
        assert_eq!(
            ProxyStreamService::from_label("Norito"),
            ProxyStreamService::Norito
        );
        assert_eq!(
            ProxyStreamService::from_label("car"),
            ProxyStreamService::Car
        );
        assert_eq!(
            ProxyStreamService::from_label("something"),
            ProxyStreamService::Unknown
        );
    }

    #[test]
    fn extract_stream_target_filters_blanks() {
        let frame = ProxyStreamOpenV1 {
            version: PROXY_STREAM_VERSION,
            service: "norito".into(),
            authority: None,
            target: Some("  routes/route-alpha  ".into()),
            route_policy_id: None,
            exit_country: None,
            client_id: None,
        };
        assert_eq!(extract_stream_target(&frame), Some("routes/route-alpha"));

        let blank = ProxyStreamOpenV1 {
            version: PROXY_STREAM_VERSION,
            service: "norito".into(),
            authority: None,
            target: Some("   ".into()),
            route_policy_id: None,
            exit_country: None,
            client_id: None,
        };
        assert!(extract_stream_target(&blank).is_none());

        let none_target = ProxyStreamOpenV1 {
            version: PROXY_STREAM_VERSION,
            service: "norito".into(),
            authority: None,
            target: None,
            route_policy_id: None,
            exit_country: None,
            client_id: None,
        };
        assert!(extract_stream_target(&none_target).is_none());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn read_frame_rejects_oversized_payload() {
        let Some((server_endpoint, server_addr, server_task)) = spawn_frame_server_or_skip() else {
            return;
        };
        let (client_endpoint, connection) = connect_proxy_client(server_addr).await;
        let (mut send_stream, _recv_stream) = connection.open_bi().await.expect("open stream");
        let oversized_len =
            u32::try_from(PROXY_MAX_FRAME_BYTES + 1).expect("frame length fits u32");
        send_stream
            .write_all(&oversized_len.to_be_bytes())
            .await
            .expect("write frame length");
        send_stream.flush().await.expect("flush frame length");
        send_stream.finish().expect("finish stream");

        let result = tokio::time::timeout(Duration::from_secs(2), server_task)
            .await
            .expect("server task timeout")
            .expect("server task join");
        match result {
            Err(FrameError::FrameTooLarge { len, max }) => {
                assert_eq!(len, PROXY_MAX_FRAME_BYTES + 1);
                assert_eq!(max, PROXY_MAX_FRAME_BYTES);
            }
            other => panic!("expected FrameTooLarge error, got {other:?}"),
        }

        connection.close(VarInt::from_u32(0), b"done");
        client_endpoint.wait_idle().await;
        server_endpoint.wait_idle().await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn handshake_manifest_includes_cache_tagging() {
        let config = LocalQuicProxyConfig {
            bind_addr: "127.0.0.1:0".into(),
            telemetry_label: Some("dev-proxy".into()),
            guard_cache_key_hex: Some(TEST_GUARD_KEY.into()),
            ..LocalQuicProxyConfig::default()
        };
        let Some(proxy) = spawn_proxy_or_skip(config.clone()) else {
            return;
        };
        let proxy_addr = proxy.local_addr();

        let (endpoint, connection) = connect_proxy_client(proxy_addr).await;
        let handshake_ack = perform_proxy_handshake(&connection).await;
        assert!(handshake_ack.accepted);

        let manifest = handshake_ack
            .manifest
            .expect("handshake should include manifest");
        assert_eq!(manifest.telemetry_label, config.telemetry_label);
        assert_eq!(manifest.proxy_mode, config.proxy_mode);
        let cache = manifest.cache_tagging.expect("cache tagging present");
        let salt = cache.salt_hex.expect("cache salt present");
        assert_eq!(salt.len(), PROXY_CACHE_TAG_SALT_LEN * 2);
        assert!(
            cache.attach_on.iter().any(|label| label == "norito")
                && cache.attach_on.iter().any(|label| label == "car"),
            "expected cache tagging to advertise both norito and car services"
        );

        connection.close(VarInt::from_u32(0), b"done");
        endpoint.wait_idle().await;
        proxy.shutdown().await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn stream_rejects_version_mismatch() {
        let config = LocalQuicProxyConfig {
            bind_addr: "127.0.0.1:0".into(),
            proxy_mode: ProxyMode::Bridge,
            emit_browser_manifest: false,
            ..LocalQuicProxyConfig::default()
        };
        let Some(proxy) = spawn_proxy_or_skip(config) else {
            return;
        };
        let proxy_addr = proxy.local_addr();

        let (endpoint, connection) = connect_proxy_client(proxy_addr).await;
        let handshake_ack = perform_proxy_handshake(&connection).await;
        assert!(handshake_ack.accepted);

        let open_frame = ProxyStreamOpenV1 {
            version: PROXY_STREAM_VERSION.saturating_add(1),
            service: "norito".into(),
            authority: Some("torii.dev:9443".into()),
            target: Some("routes/route-alpha".into()),
            route_policy_id: None,
            exit_country: None,
            client_id: Some("test-client".into()),
        };
        let (mut stream_send, mut stream_recv, stream_ack) =
            open_proxy_stream(&connection, &open_frame).await;
        assert!(!stream_ack.accepted);
        assert_eq!(stream_ack.code, STREAM_ACK_UNSUPPORTED_VERSION);
        let _ = stream_send.finish();
        let _ = stream_recv.stop(VarInt::from_u32(0));

        connection.close(VarInt::from_u32(0), b"done");
        endpoint.wait_idle().await;
        proxy.shutdown().await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn handshake_rejects_version_mismatch() {
        let config = LocalQuicProxyConfig {
            bind_addr: "127.0.0.1:0".into(),
            emit_browser_manifest: false,
            ..LocalQuicProxyConfig::default()
        };
        let Some(proxy) = spawn_proxy_or_skip(config) else {
            return;
        };
        let proxy_addr = proxy.local_addr();

        let (endpoint, connection) = connect_proxy_client(proxy_addr).await;
        let (mut handshake_send, mut handshake_recv) =
            connection.open_bi().await.expect("open handshake stream");
        let handshake = ProxyHandshakeV1 {
            version: PROXY_HANDSHAKE_VERSION.saturating_add(1),
            client: Some("test-client".into()),
            user_agent: Some("orchestrator-test".into()),
        };
        write_frame(&mut handshake_send, &handshake)
            .await
            .expect("write handshake");
        handshake_send.finish().expect("finish handshake send");
        let ack_bytes = read_frame(&mut handshake_recv)
            .await
            .expect("read handshake ack");
        let ack: TestHandshakeAck = decode_from_bytes(&ack_bytes).expect("decode handshake ack");
        assert_eq!(ack.version, PROXY_HANDSHAKE_VERSION);
        assert!(!ack.accepted);
        assert_eq!(ack.message.as_deref(), Some("unsupported version"));

        connection.close(VarInt::from_u32(0), b"done");
        endpoint.wait_idle().await;
        proxy.shutdown().await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn norito_bridge_streams_spool_payload() {
        let spool_dir = TempDir::new().expect("tempdir");
        let spool_root = spool_dir.path().join("routes");
        tokio::fs::create_dir_all(&spool_root)
            .await
            .expect("create norito spool");
        let spool_path = spool_root.join("route-alpha.norito");
        tokio::fs::write(&spool_path, b"norito-bytes")
            .await
            .expect("write norito payload");

        let mut config = LocalQuicProxyConfig {
            bind_addr: "127.0.0.1:0".into(),
            proxy_mode: ProxyMode::Bridge,
            emit_browser_manifest: false,
            ..LocalQuicProxyConfig::default()
        };
        config.norito_bridge = Some(ProxyNoritoBridgeConfig {
            spool_dir: spool_dir.path().to_string_lossy().into_owned(),
            extension: Some("norito".into()),
        });
        let Some(proxy) = spawn_proxy_or_skip(config) else {
            return;
        };
        let proxy_addr = proxy.local_addr();

        let (endpoint, connection) = connect_proxy_client(proxy_addr).await;
        let handshake_ack = perform_proxy_handshake(&connection).await;
        assert!(handshake_ack.accepted);

        let open_frame = ProxyStreamOpenV1 {
            version: PROXY_STREAM_VERSION,
            service: "norito".into(),
            authority: Some("torii.dev:9443".into()),
            target: Some("routes/route-alpha".into()),
            route_policy_id: Some("policy-alpha".into()),
            exit_country: Some("JP".into()),
            client_id: Some("test-client".into()),
        };
        let (mut stream_send, mut stream_recv, stream_ack) =
            open_proxy_stream(&connection, &open_frame).await;
        assert!(stream_ack.accepted);
        assert_eq!(stream_ack.code, STREAM_ACK_OK);
        stream_send.finish().expect("finish stream open");

        let payload = stream_recv
            .read_to_end(16 * 1024 * 1024)
            .await
            .expect("read norito payload");
        assert_eq!(payload, b"norito-bytes");

        connection.close(VarInt::from_u32(0), b"done");
        endpoint.wait_idle().await;
        proxy.shutdown().await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn car_bridge_streams_archive() {
        let cache_dir = TempDir::new().expect("tempdir");
        let cache_root = cache_dir.path().join("archives");
        tokio::fs::create_dir_all(&cache_root)
            .await
            .expect("create car cache");
        let car_path = cache_root.join("ledger.car");
        tokio::fs::write(&car_path, b"car-bytes")
            .await
            .expect("write car payload");

        let mut config = LocalQuicProxyConfig {
            bind_addr: "127.0.0.1:0".into(),
            proxy_mode: ProxyMode::Bridge,
            emit_browser_manifest: false,
            ..LocalQuicProxyConfig::default()
        };
        config.car_bridge = Some(ProxyCarBridgeConfig {
            cache_dir: cache_dir.path().to_string_lossy().into_owned(),
            extension: Some("car".into()),
            allow_zst: false,
        });
        let Some(proxy) = spawn_proxy_or_skip(config) else {
            return;
        };
        let proxy_addr = proxy.local_addr();

        let (endpoint, connection) = connect_proxy_client(proxy_addr).await;
        let handshake_ack = perform_proxy_handshake(&connection).await;
        assert!(handshake_ack.accepted);

        let open_frame = ProxyStreamOpenV1 {
            version: PROXY_STREAM_VERSION,
            service: "car".into(),
            authority: Some("torii.dev:9443".into()),
            target: Some("archives/ledger".into()),
            route_policy_id: Some("policy-alpha".into()),
            exit_country: Some("JP".into()),
            client_id: Some("test-client".into()),
        };
        let (mut stream_send, mut stream_recv, stream_ack) =
            open_proxy_stream(&connection, &open_frame).await;
        assert!(stream_ack.accepted);
        assert_eq!(stream_ack.code, STREAM_ACK_OK);
        stream_send.finish().expect("finish stream open");

        let payload = stream_recv
            .read_to_end(16 * 1024 * 1024)
            .await
            .expect("read car payload");
        assert_eq!(payload, b"car-bytes");

        connection.close(VarInt::from_u32(0), b"done");
        endpoint.wait_idle().await;
        proxy.shutdown().await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn kaigi_bridge_streams_spool_payload_with_policy() {
        let spool_dir = TempDir::new().expect("tempdir");
        let spool_root = spool_dir.path().join("kaigi");
        tokio::fs::create_dir_all(&spool_root)
            .await
            .expect("create kaigi spool");
        let spool_path = spool_root.join("room-alpha.norito");
        tokio::fs::write(&spool_path, b"kaigi-bytes")
            .await
            .expect("write kaigi payload");

        let mut config = LocalQuicProxyConfig {
            bind_addr: "127.0.0.1:0".into(),
            proxy_mode: ProxyMode::Bridge,
            emit_browser_manifest: false,
            guard_cache_key_hex: Some(TEST_GUARD_KEY.into()),
            ..LocalQuicProxyConfig::default()
        };
        config.kaigi_bridge = Some(ProxyKaigiBridgeConfig {
            spool_dir: spool_dir.path().to_string_lossy().into_owned(),
            extension: Some("norito".into()),
            room_policy: Some("authenticated".into()),
        });
        let Some(proxy) = spawn_proxy_or_skip(config) else {
            return;
        };
        let proxy_addr = proxy.local_addr();

        let (endpoint, connection) = connect_proxy_client(proxy_addr).await;
        let handshake_ack = perform_proxy_handshake(&connection).await;
        assert!(handshake_ack.accepted);

        let open_frame = ProxyStreamOpenV1 {
            version: PROXY_STREAM_VERSION,
            service: "kaigi".into(),
            authority: Some("kaigi.example:9443".into()),
            target: Some("kaigi/room-alpha".into()),
            route_policy_id: Some("policy-kaigi".into()),
            exit_country: Some("SG".into()),
            client_id: Some("kaigi-client".into()),
        };
        let (mut stream_send, mut stream_recv, stream_ack) =
            open_proxy_stream(&connection, &open_frame).await;
        assert!(stream_ack.accepted);
        assert_eq!(stream_ack.code, STREAM_ACK_OK);
        assert_eq!(
            stream_ack.message.as_deref(),
            Some("room-policy=authenticated")
        );
        let cache_tag = stream_ack
            .cache_tag_hex
            .as_ref()
            .expect("kaigi ack should include cache tag");
        assert_eq!(cache_tag.len(), 32);

        stream_send.finish().expect("finish stream open");

        let payload = stream_recv
            .read_to_end(16 * 1024 * 1024)
            .await
            .expect("read kaigi payload");
        assert_eq!(payload, b"kaigi-bytes");

        connection.close(VarInt::from_u32(0), b"done");
        endpoint.wait_idle().await;
        proxy.shutdown().await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn tcp_stream_bridge_transfers_payload() {
        let listener = match TcpListener::bind("127.0.0.1:0").await {
            Ok(listener) => listener,
            Err(err) if err.kind() == io::ErrorKind::PermissionDenied => {
                eprintln!("skipping proxy TCP bridge test (loopback denied): {err}");
                return;
            }
            Err(err) => panic!("bind backend listener: {err}"),
        };
        let backend_addr = listener.local_addr().expect("backend addr");
        let backend_task = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.expect("accept backend");
            let mut buf = [0u8; 4];
            socket.read_exact(&mut buf).await.expect("read request");
            assert_eq!(&buf, b"ping");
            socket.write_all(b"pong").await.expect("write response");
        });

        let config = LocalQuicProxyConfig {
            bind_addr: "127.0.0.1:0".into(),
            proxy_mode: ProxyMode::Bridge,
            emit_browser_manifest: false,
            guard_cache_key_hex: Some(TEST_GUARD_KEY.into()),
            ..LocalQuicProxyConfig::default()
        };
        let Some(proxy) = spawn_proxy_or_skip(config) else {
            backend_task.abort();
            return;
        };
        let proxy_addr = proxy.local_addr();

        let (endpoint, connection) = connect_proxy_client(proxy_addr).await;
        let handshake_ack = perform_proxy_handshake(&connection).await;
        assert!(handshake_ack.accepted);

        let open_frame = ProxyStreamOpenV1 {
            version: PROXY_STREAM_VERSION,
            service: "tcp".into(),
            authority: Some(backend_addr.to_string()),
            target: None,
            route_policy_id: None,
            exit_country: None,
            client_id: Some("browser".into()),
        };
        let (mut stream_send, mut stream_recv, stream_ack) =
            open_proxy_stream(&connection, &open_frame).await;
        assert!(stream_ack.accepted);
        assert_eq!(stream_ack.code, STREAM_ACK_OK);
        let cache_tag = stream_ack
            .cache_tag_hex
            .as_ref()
            .expect("tcp ack should include cache tag");
        assert_eq!(cache_tag.len(), 32);

        stream_send.write_all(b"ping").await.expect("write payload");
        stream_send.flush().await.expect("flush payload");
        let mut response = [0u8; 4];
        stream_recv
            .read_exact(&mut response)
            .await
            .expect("read response");
        assert_eq!(&response, b"pong");
        stream_send.finish().expect("finish stream send");

        connection.close(VarInt::from_u32(0), b"done");
        endpoint.wait_idle().await;
        proxy.shutdown().await;
        backend_task.await.expect("backend task");
    }

    async fn connect_proxy_client(proxy_addr: SocketAddr) -> (Endpoint, quinn::Connection) {
        let mut endpoint = Endpoint::client("0.0.0.0:0".parse().unwrap()).expect("endpoint");
        let verifier: Arc<dyn ServerCertVerifier> = Arc::new(NoCertificateVerification);
        let mut tls_config = rustls::ClientConfig::builder()
            .dangerous()
            .with_custom_certificate_verifier(verifier)
            .with_no_client_auth();
        tls_config.alpn_protocols = vec![PROXY_ALPN_LABEL.as_bytes().to_vec()];
        let tls_config = Arc::new(tls_config);
        let crypto =
            QuinnRustlsClientConfig::try_from(Arc::clone(&tls_config)).expect("client crypto");
        let client_config = ClientConfig::new(Arc::new(crypto));
        endpoint.set_default_client_config(client_config);

        let connecting = endpoint
            .connect(proxy_addr, "localhost")
            .expect("connect request");
        let connection = connecting.await.expect("proxy handshake");
        (endpoint, connection)
    }

    async fn perform_proxy_handshake(connection: &quinn::Connection) -> TestHandshakeAck {
        let (mut handshake_send, mut handshake_recv) =
            connection.open_bi().await.expect("open handshake stream");
        let handshake = ProxyHandshakeV1 {
            version: PROXY_HANDSHAKE_VERSION,
            client: Some("test-client".into()),
            user_agent: Some("orchestrator-test".into()),
        };
        write_frame(&mut handshake_send, &handshake)
            .await
            .expect("write handshake");
        handshake_send.finish().expect("finish handshake send");
        let ack_bytes = read_frame(&mut handshake_recv)
            .await
            .expect("read handshake ack");
        let ack: TestHandshakeAck = decode_from_bytes(&ack_bytes).expect("decode handshake ack");
        assert_eq!(ack.version, PROXY_HANDSHAKE_VERSION);
        let _ = ack.message.as_ref();
        ack
    }

    async fn open_proxy_stream(
        connection: &quinn::Connection,
        open_frame: &ProxyStreamOpenV1,
    ) -> (quinn::SendStream, quinn::RecvStream, TestStreamAck) {
        let (mut stream_send, mut stream_recv) =
            connection.open_bi().await.expect("open application stream");
        write_frame(&mut stream_send, open_frame)
            .await
            .expect("write stream open");
        let ack_bytes = read_frame(&mut stream_recv).await.expect("read stream ack");
        let ack: TestStreamAck = decode_from_bytes(&ack_bytes).expect("decode stream ack");
        assert_eq!(ack.version, PROXY_STREAM_VERSION);
        let _ = ack.message.as_ref();
        (stream_send, stream_recv, ack)
    }

    #[derive(Debug, NoritoDeserialize)]
    struct TestHandshakeAck {
        version: u8,
        accepted: bool,
        #[norito(default)]
        message: Option<String>,
        #[norito(default)]
        manifest: Option<BrowserExtensionManifest>,
    }

    #[derive(Debug, NoritoDeserialize)]
    struct TestStreamAck {
        version: u8,
        code: u8,
        accepted: bool,
        #[norito(default)]
        message: Option<String>,
        #[norito(default)]
        cache_tag_hex: Option<String>,
    }

    #[derive(Debug)]
    struct NoCertificateVerification;

    impl ServerCertVerifier for NoCertificateVerification {
        fn verify_server_cert(
            &self,
            _end_entity: &CertificateDer<'_>,
            _intermediates: &[CertificateDer<'_>],
            _server_name: &ServerName<'_>,
            _ocsp_response: &[u8],
            _now: UnixTime,
        ) -> Result<ServerCertVerified, RustlsError> {
            Ok(ServerCertVerified::assertion())
        }

        fn verify_tls12_signature(
            &self,
            _message: &[u8],
            _cert: &CertificateDer<'_>,
            _dss: &DigitallySignedStruct,
        ) -> Result<HandshakeSignatureValid, RustlsError> {
            Ok(HandshakeSignatureValid::assertion())
        }

        fn verify_tls13_signature(
            &self,
            _message: &[u8],
            _cert: &CertificateDer<'_>,
            _dss: &DigitallySignedStruct,
        ) -> Result<HandshakeSignatureValid, RustlsError> {
            Ok(HandshakeSignatureValid::assertion())
        }

        fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
            vec![
                SignatureScheme::ECDSA_NISTP256_SHA256,
                SignatureScheme::ED25519,
                SignatureScheme::RSA_PSS_SHA256,
            ]
        }
    }
}
