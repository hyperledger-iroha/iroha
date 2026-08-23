use crate::{
    bundle::ProofBundleV1,
    limits::{
        MAX_CHILD_STRINGS, MAX_CONFIG_BYTES, MAX_FIELD_BYTES, MAX_IDENTIFIER_BYTES,
        MAX_LISTEN_ADDRESSES, MAX_PROOF_BUNDLE_BYTES, MAX_RAD_SNAPSHOT_BYTES, MAX_SOURCES_PER_KIND,
        MAX_STATIC_RECORDS, MAX_STATIC_ZONE_RETAINED_BYTES, MAX_STATIC_ZONES, config_decode_limits,
        preflight_json, proof_bundle_decode_limits, read_bounded_file, read_bounded_file_async,
    },
    rad::{ResolverAttestation, decode_rad_entries},
};
use eyre::{Context, Result, bail};
use hickory_proto::rr::{
    Name, RData, Record,
    rdata::{A, AAAA, CNAME, TXT},
};
use norito::{decode_from_bytes_with_limits, json};
use norito_derive::{JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize};
use std::{
    convert::TryFrom,
    net::{Ipv4Addr, Ipv6Addr, SocketAddr},
    path::{Path, PathBuf},
    time::Duration,
};
/// Resolver configuration with normalised runtime values.
#[derive(Debug, Clone)]
pub struct ResolverConfig {
    pub resolver_id: String,
    pub region: String,
    doh_listen: Vec<SocketAddr>,
    dot_listen: Vec<SocketAddr>,
    event_listen: Option<SocketAddr>,
    operations_auth_token_path: Option<PathBuf>,
    bundle_sources: Vec<BundleSource>,
    rad_sources: Vec<RadSource>,
    dot_tls: Option<DotTlsConfig>,
    static_zones: Vec<StaticZone>,
    event_log_path: Option<PathBuf>,
    sync_interval: Duration,
}
impl ResolverConfig {
    /// Load configuration from a Norito JSON file.
    pub fn load_from_path(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let buf = read_bounded_file(path, MAX_CONFIG_BYTES, "resolver config")?;
        let decode_limits = config_decode_limits();
        preflight_json(&buf, MAX_CONFIG_BYTES, decode_limits, "resolver config")?;
        let raw: ResolverConfigRaw =
            norito::with_decode_limits_scope(decode_limits, || json::from_slice(&buf))
                .wrap_err("failed to parse resolver config JSON")?;
        Self::try_from(raw)
    }
    /// Validate high-level invariants.
    pub fn validate(&self) -> Result<()> {
        check_string("resolver_id", &self.resolver_id, MAX_IDENTIFIER_BYTES)?;
        check_string("region", &self.region, MAX_IDENTIFIER_BYTES)?;
        if self.resolver_id.trim().is_empty() {
            bail!("resolver_id must not be empty");
        }
        if self.region.trim().is_empty() {
            bail!("region must not be empty");
        }
        if self.bundle_sources.is_empty() {
            bail!("at least one bundle source is required");
        }
        if self.rad_sources.is_empty() {
            bail!("at least one RAD source is required");
        }
        if self.sync_interval.is_zero() {
            bail!("sync interval must be greater than zero seconds");
        }
        Ok(())
    }
    #[must_use]
    pub(crate) fn doh_listen(&self) -> &[SocketAddr] {
        &self.doh_listen
    }
    #[must_use]
    pub(crate) fn dot_listen(&self) -> &[SocketAddr] {
        &self.dot_listen
    }
    #[must_use]
    pub(crate) fn bundle_sources(&self) -> &[BundleSource] {
        &self.bundle_sources
    }
    #[must_use]
    pub(crate) fn rad_sources(&self) -> &[RadSource] {
        &self.rad_sources
    }
    #[must_use]
    pub(crate) fn event_listen(&self) -> Option<SocketAddr> {
        self.event_listen
    }
    #[must_use]
    pub(crate) fn operations_auth_token_path(&self) -> Option<&Path> {
        self.operations_auth_token_path.as_deref()
    }
    #[must_use]
    pub(crate) fn dot_tls(&self) -> Option<&DotTlsConfig> {
        self.dot_tls.as_ref()
    }
    #[must_use]
    pub(crate) fn static_zones(&self) -> &[StaticZone] {
        &self.static_zones
    }
    #[must_use]
    pub(crate) fn event_log_path(&self) -> Option<&PathBuf> {
        self.event_log_path.as_ref()
    }
    /// Configured background refresh cadence for bundles/RAD adverts.
    #[must_use]
    pub fn sync_interval(&self) -> Duration {
        self.sync_interval
    }
    /// Override the sync interval after loading configuration.
    pub fn override_sync_interval(&mut self, interval: Duration) -> Result<()> {
        if interval.is_zero() {
            bail!("sync interval must be greater than zero seconds");
        }
        self.sync_interval = interval;
        Ok(())
    }
}
impl TryFrom<ResolverConfigRaw> for ResolverConfig {
    type Error = eyre::Error;
    fn try_from(raw: ResolverConfigRaw) -> Result<Self> {
        raw.validate_bounds()?;
        if raw.event_log_path.is_some() {
            bail!(
                "event_log_path is disabled in v1; use the bounded loopback event stream or process logging"
            );
        }
        let raw_bundle_sources = raw.bundle_sources.unwrap_or_default();
        let mut bundle_sources = Vec::new();
        bundle_sources
            .try_reserve_exact(raw_bundle_sources.len())
            .wrap_err("failed to reserve bundle source table")?;
        for source in raw_bundle_sources {
            bundle_sources.push(source.try_into_source()?);
        }
        let raw_rad_sources = raw.rad_sources.unwrap_or_default();
        let mut rad_sources = Vec::new();
        rad_sources
            .try_reserve_exact(raw_rad_sources.len())
            .wrap_err("failed to reserve RAD source table")?;
        for source in raw_rad_sources {
            rad_sources.push(source.try_into_source()?);
        }
        let raw_static_zones = raw.static_zones.unwrap_or_default();
        let mut static_zones = Vec::new();
        static_zones
            .try_reserve_exact(raw_static_zones.len())
            .wrap_err("failed to reserve static-zone table")?;
        let mut static_retained_bytes = 0usize;
        for zone in raw_static_zones {
            let zone = zone.try_into_zone()?;
            static_retained_bytes = static_retained_bytes
                .checked_add(zone.retained_bytes)
                .filter(|total| *total <= MAX_STATIC_ZONE_RETAINED_BYTES)
                .ok_or_else(|| {
                    eyre::eyre!(
                        "static zones exceed the {MAX_STATIC_ZONE_RETAINED_BYTES}-byte retained-memory limit"
                    )
                })?;
            static_zones.push(zone);
        }
        let doh_listen = parse_socket_list("doh_listen", raw.doh_listen)?;
        validate_loopback_listeners("doh_listen", &doh_listen)?;
        let dot_listen = parse_socket_list("dot_listen", raw.dot_listen)?;
        if raw.doq_listen.is_some() {
            bail!(
                "doq_listen is not part of the v1 resolver schema; authenticated QUIC transport is not implemented"
            );
        }
        let event_listen = parse_socket("event_listen", raw.event_listen)?;
        if let Some(address) = event_listen
            && !address.ip().is_loopback()
        {
            bail!(
                "event_listen must use a loopback address because its bearer-authenticated metrics and event streams use plaintext HTTP behind a local TLS proxy"
            );
        }
        let operations_auth_token_path = raw
            .operations_auth_token_path
            .map(PathBuf::from)
            .filter(|path| !path.as_os_str().is_empty());
        match (event_listen, operations_auth_token_path.as_ref()) {
            (Some(_), None) => bail!(
                "operations_auth_token_path is required when event_listen enables operational endpoints"
            ),
            (None, Some(_)) => bail!(
                "operations_auth_token_path requires event_listen so the credential is not loaded unused"
            ),
            _ => {}
        }
        let event_log_path = None;
        let dot_tls = match raw.dot_tls {
            Some(tls) => Some(tls.try_into_config()?),
            None => None,
        };
        let sync_secs = raw.sync_interval_secs.unwrap_or(DEFAULT_SYNC_INTERVAL_SECS);
        if sync_secs == 0 {
            bail!("sync_interval_secs must be greater than zero");
        }
        let sync_interval = Duration::from_secs(sync_secs);
        Ok(Self {
            resolver_id: raw.resolver_id,
            region: raw.region,
            doh_listen,
            dot_listen,
            event_listen,
            operations_auth_token_path,
            bundle_sources,
            rad_sources,
            dot_tls,
            static_zones,
            event_log_path,
            sync_interval,
        })
    }
}
#[derive(Debug, JsonSerialize, JsonDeserialize)]
struct ResolverConfigRaw {
    resolver_id: String,
    region: String,
    bundle_sources: Option<Vec<BundleSourceConfig>>,
    rad_sources: Option<Vec<RadSourceConfig>>,
    doh_listen: Option<Vec<String>>,
    dot_listen: Option<Vec<String>>,
    doq_listen: Option<Vec<String>>,
    event_listen: Option<String>,
    operations_auth_token_path: Option<String>,
    dot_tls: Option<DotTlsConfigRaw>,
    static_zones: Option<Vec<StaticZoneConfig>>,
    event_log_path: Option<String>,
    sync_interval_secs: Option<u64>,
}
impl ResolverConfigRaw {
    fn validate_bounds(&self) -> Result<()> {
        check_string("resolver_id", &self.resolver_id, MAX_IDENTIFIER_BYTES)?;
        check_string("region", &self.region, MAX_IDENTIFIER_BYTES)?;
        check_optional_string(
            "event_listen",
            self.event_listen.as_deref(),
            MAX_IDENTIFIER_BYTES,
        )?;
        check_optional_string(
            "event_log_path",
            self.event_log_path.as_deref(),
            MAX_FIELD_BYTES,
        )?;
        check_optional_string(
            "operations_auth_token_path",
            self.operations_auth_token_path.as_deref(),
            MAX_FIELD_BYTES,
        )?;
        let bundle_sources = self.bundle_sources.as_deref().unwrap_or_default();
        check_count("bundle_sources", bundle_sources.len(), MAX_SOURCES_PER_KIND)?;
        let rad_sources = self.rad_sources.as_deref().unwrap_or_default();
        check_count("rad_sources", rad_sources.len(), MAX_SOURCES_PER_KIND)?;
        check_optional_count(
            "doh_listen",
            self.doh_listen.as_deref(),
            MAX_LISTEN_ADDRESSES,
        )?;
        check_optional_count(
            "dot_listen",
            self.dot_listen.as_deref(),
            MAX_LISTEN_ADDRESSES,
        )?;
        check_optional_count(
            "doq_listen",
            self.doq_listen.as_deref(),
            MAX_LISTEN_ADDRESSES,
        )?;
        check_optional_count(
            "static_zones",
            self.static_zones.as_deref(),
            MAX_STATIC_ZONES,
        )?;
        for source in bundle_sources {
            source.validate_bounds()?;
        }
        for source in rad_sources {
            source.validate_bounds()?;
        }
        for (name, values) in [
            ("doh_listen", self.doh_listen.as_deref()),
            ("dot_listen", self.dot_listen.as_deref()),
            ("doq_listen", self.doq_listen.as_deref()),
        ] {
            for value in values.unwrap_or_default() {
                check_string(name, value, MAX_IDENTIFIER_BYTES)?;
            }
        }
        if let Some(tls) = &self.dot_tls {
            tls.validate_bounds()?;
        }
        let mut record_count = 0usize;
        for zone in self.static_zones.as_deref().unwrap_or_default() {
            record_count = record_count
                .checked_add(zone.records.len())
                .filter(|count| *count <= MAX_STATIC_RECORDS)
                .ok_or_else(|| {
                    eyre::eyre!("static zone records exceed the {MAX_STATIC_RECORDS}-record limit")
                })?;
            zone.validate_bounds()?;
        }
        Ok(())
    }
}
const DEFAULT_SYNC_INTERVAL_SECS: u64 = 30;
fn check_count(label: &str, count: usize, maximum: usize) -> Result<()> {
    if count > maximum {
        bail!("{label} contains {count} entries; the limit is {maximum}");
    }
    Ok(())
}
fn check_optional_count<T>(label: &str, values: Option<&[T]>, maximum: usize) -> Result<()> {
    check_count(label, values.map_or(0, |values| values.len()), maximum)
}
fn check_string(label: &str, value: &str, maximum: usize) -> Result<()> {
    if value.len() > maximum {
        bail!(
            "{label} contains {} UTF-8 bytes; the limit is {maximum}",
            value.len()
        );
    }
    Ok(())
}
fn check_optional_string(label: &str, value: Option<&str>, maximum: usize) -> Result<()> {
    if let Some(value) = value {
        check_string(label, value, maximum)?;
    }
    Ok(())
}
fn parse_socket_list(name: &str, list: Option<Vec<String>>) -> Result<Vec<SocketAddr>> {
    let mut result = Vec::new();
    if let Some(values) = list {
        result
            .try_reserve_exact(values.len())
            .wrap_err_with(|| format!("failed to reserve `{name}` listener list"))?;
        for value in values {
            let addr: SocketAddr = value.parse().wrap_err_with(|| {
                format!("failed to parse `{name}` entry `{value}` as socket address")
            })?;
            result.push(addr);
        }
    }
    Ok(result)
}
fn parse_socket(name: &str, value: Option<String>) -> Result<Option<SocketAddr>> {
    value
        .map(|addr| {
            addr.parse()
                .wrap_err_with(|| format!("failed to parse `{name}` socket address `{addr}`"))
        })
        .transpose()
}
fn validate_loopback_listeners(name: &str, addresses: &[SocketAddr]) -> Result<()> {
    if let Some(address) = addresses.iter().find(|address| !address.ip().is_loopback()) {
        bail!(
            "{name} address {address} must be loopback because v1 serves plaintext HTTP; terminate TLS at a local proxy"
        );
    }
    Ok(())
}
/// TLS configuration for DoT listeners.
#[derive(Debug, Clone)]
pub struct DotTlsConfig {
    pub cert_path: PathBuf,
    pub key_path: PathBuf,
}
#[derive(Debug, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize)]
struct DotTlsConfigRaw {
    cert_path: String,
    key_path: String,
}
impl DotTlsConfigRaw {
    fn validate_bounds(&self) -> Result<()> {
        check_string("dot_tls.cert_path", &self.cert_path, MAX_FIELD_BYTES)?;
        check_string("dot_tls.key_path", &self.key_path, MAX_FIELD_BYTES)
    }
    fn try_into_config(self) -> Result<DotTlsConfig> {
        if self.cert_path.trim().is_empty() {
            bail!("dot_tls.cert_path must not be empty");
        }
        if self.key_path.trim().is_empty() {
            bail!("dot_tls.key_path must not be empty");
        }
        Ok(DotTlsConfig {
            cert_path: PathBuf::from(self.cert_path),
            key_path: PathBuf::from(self.key_path),
        })
    }
}
/// Source for proof bundles. Each variant may yield one or more bundles during a fetch.
#[derive(Debug, Clone)]
pub(crate) enum BundleSource {
    File {
        path: PathBuf,
        expected_blake3: [u8; 32],
    },
}
impl BundleSource {
    pub async fn fetch(&self, _client: &reqwest::Client) -> Result<Vec<ProofBundleV1>> {
        match self {
            Self::File {
                path,
                expected_blake3,
            } => {
                let label = format!("proof bundle `{}`", path.display());
                let bytes =
                    read_bounded_file_async(path.clone(), MAX_PROOF_BUNDLE_BYTES, label.clone())
                        .await?;
                verify_pinned_snapshot(&bytes, expected_blake3, &label)?;
                let bundle = decode_proof_bundle(&bytes, &label)?;
                let mut bundles = Vec::new();
                bundles
                    .try_reserve_exact(1)
                    .wrap_err("failed to reserve local bundle result")?;
                bundles.push(bundle);
                Ok(bundles)
            }
        }
    }
}
/// Resolver Advertisement source.
#[derive(Debug, Clone)]
pub(crate) enum RadSource {
    File {
        path: PathBuf,
        expected_blake3: [u8; 32],
    },
}
impl RadSource {
    pub async fn fetch(&self, _client: &reqwest::Client) -> Result<Vec<ResolverAttestation>> {
        match self {
            Self::File {
                path,
                expected_blake3,
            } => {
                let label = format!("RAD snapshot `{}`", path.display());
                let bytes =
                    read_bounded_file_async(path.clone(), MAX_RAD_SNAPSHOT_BYTES, label.clone())
                        .await?;
                verify_pinned_snapshot(&bytes, expected_blake3, &label)?;
                let entries = decode_rad_entries(&bytes).wrap_err_with(|| {
                    format!("failed to decode RAD snapshot `{}`", path.display())
                })?;
                Ok(entries)
            }
        }
    }
}
fn verify_pinned_snapshot(bytes: &[u8], expected: &[u8; 32], label: &str) -> Result<()> {
    if blake3::hash(bytes).as_bytes() != expected {
        bail!("{label} does not match its independently provisioned BLAKE3 digest");
    }
    Ok(())
}
fn decode_proof_bundle(bytes: &[u8], label: &str) -> Result<ProofBundleV1> {
    if bytes.len() > MAX_PROOF_BUNDLE_BYTES {
        bail!("{label} exceeds the {MAX_PROOF_BUNDLE_BYTES}-byte limit");
    }
    let bundle: ProofBundleV1 = decode_from_bytes_with_limits(bytes, proof_bundle_decode_limits())
        .wrap_err_with(|| format!("failed to decode {label}"))?;
    bundle
        .validate_resource_bounds()
        .wrap_err_with(|| format!("{label} exceeds proof-bundle field limits"))?;
    Ok(bundle)
}
#[derive(Debug, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize)]
#[norito(tag = "kind", content = "value")]
enum BundleSourceConfig {
    #[norito(rename = "file")]
    File {
        path: String,
        expected_blake3_hex: String,
    },
}
impl BundleSourceConfig {
    fn validate_bounds(&self) -> Result<()> {
        match self {
            Self::File {
                path,
                expected_blake3_hex,
            } => {
                check_string("bundle source path", path, MAX_FIELD_BYTES)?;
                check_string("bundle source expected_blake3_hex", expected_blake3_hex, 64)
            }
        }
    }
    fn try_into_source(self) -> Result<BundleSource> {
        match self {
            Self::File {
                path,
                expected_blake3_hex,
            } => {
                if path.trim().is_empty() {
                    bail!("bundle source path must not be empty");
                }
                Ok(BundleSource::File {
                    path: PathBuf::from(path),
                    expected_blake3: parse_snapshot_digest(
                        &expected_blake3_hex,
                        "bundle source expected_blake3_hex",
                    )?,
                })
            }
        }
    }
}
#[derive(Debug, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize)]
#[norito(tag = "kind", content = "value")]
enum RadSourceConfig {
    #[norito(rename = "file")]
    File {
        path: String,
        expected_blake3_hex: String,
    },
}
impl RadSourceConfig {
    fn validate_bounds(&self) -> Result<()> {
        match self {
            Self::File {
                path,
                expected_blake3_hex,
            } => {
                check_string("RAD source path", path, MAX_FIELD_BYTES)?;
                check_string("RAD source expected_blake3_hex", expected_blake3_hex, 64)
            }
        }
    }
    fn try_into_source(self) -> Result<RadSource> {
        match self {
            Self::File {
                path,
                expected_blake3_hex,
            } => {
                if path.trim().is_empty() {
                    bail!("rad source path must not be empty");
                }
                Ok(RadSource::File {
                    path: PathBuf::from(path),
                    expected_blake3: parse_snapshot_digest(
                        &expected_blake3_hex,
                        "RAD source expected_blake3_hex",
                    )?,
                })
            }
        }
    }
}
fn parse_snapshot_digest(value: &str, field: &str) -> Result<[u8; 32]> {
    if value.len() != 64 {
        bail!("{field} must contain exactly 64 hexadecimal characters");
    }
    let mut digest = [0u8; 32];
    hex::decode_to_slice(value, &mut digest)
        .wrap_err_with(|| format!("{field} must be valid hexadecimal"))?;
    if digest.iter().all(|byte| *byte == 0) {
        bail!("{field} must not be the all-zero placeholder");
    }
    Ok(digest)
}
fn normalize_domain(domain: &str) -> Result<String> {
    let name =
        Name::from_ascii(domain).wrap_err_with(|| format!("invalid domain name `{domain}`"))?;
    Ok(name.to_ascii().trim_end_matches('.').to_lowercase())
}
#[derive(Debug, Clone)]
pub(crate) struct StaticZone {
    pub domain: String,
    pub records: Vec<Record>,
    pub freeze: Option<FreezeMetadata>,
    pub(crate) retained_bytes: usize,
}
#[derive(Debug, Clone)]
pub(crate) struct FreezeMetadata {
    pub state: FreezeState,
    pub ticket: Option<String>,
    pub expires_at: Option<String>,
    pub notes: Vec<String>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FreezeState {
    Soft,
    Hard,
    Thawing,
    Monitoring,
    Emergency,
}
impl FreezeState {
    fn from_str(value: &str) -> Result<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "soft" => Ok(Self::Soft),
            "hard" => Ok(Self::Hard),
            "thawing" => Ok(Self::Thawing),
            "monitoring" => Ok(Self::Monitoring),
            "emergency" => Ok(Self::Emergency),
            other => bail!(
                "freeze state `{}` is not supported (expected soft, hard, thawing, monitoring, or emergency)",
                other
            ),
        }
    }
}
#[derive(Debug, JsonSerialize, JsonDeserialize)]
struct FreezeMetadataConfig {
    state: String,
    ticket: Option<String>,
    expires_at: Option<String>,
    notes: Option<Vec<String>>,
}
impl FreezeMetadataConfig {
    fn validate_bounds(&self) -> Result<()> {
        check_string(
            "static zone freeze state",
            &self.state,
            MAX_IDENTIFIER_BYTES,
        )?;
        check_optional_string(
            "static zone freeze ticket",
            self.ticket.as_deref(),
            MAX_IDENTIFIER_BYTES,
        )?;
        check_optional_string(
            "static zone freeze expiry",
            self.expires_at.as_deref(),
            MAX_IDENTIFIER_BYTES,
        )?;
        check_optional_count(
            "static zone freeze notes",
            self.notes.as_deref(),
            MAX_CHILD_STRINGS,
        )?;
        for note in self.notes.as_deref().unwrap_or_default() {
            check_string("static zone freeze note", note, MAX_FIELD_BYTES)?;
        }
        Ok(())
    }
    fn try_into_metadata(self) -> Result<FreezeMetadata> {
        let state = FreezeState::from_str(&self.state)?;
        let notes = self.notes.unwrap_or_default();
        Ok(FreezeMetadata {
            state,
            ticket: self.ticket,
            expires_at: self.expires_at,
            notes,
        })
    }
}
#[derive(Debug, JsonSerialize, JsonDeserialize)]
struct StaticZoneConfig {
    domain: String,
    records: Vec<StaticRecordConfig>,
    freeze: Option<FreezeMetadataConfig>,
}
impl StaticZoneConfig {
    fn validate_bounds(&self) -> Result<()> {
        check_string("static zone domain", &self.domain, MAX_IDENTIFIER_BYTES)?;
        check_count(
            "static zone records",
            self.records.len(),
            MAX_STATIC_RECORDS,
        )?;
        for record in &self.records {
            record.validate_bounds()?;
        }
        if let Some(freeze) = &self.freeze {
            freeze.validate_bounds()?;
        }
        Ok(())
    }
    fn retained_bytes(&self) -> Result<usize> {
        let mut retained = std::mem::size_of::<StaticZone>()
            .saturating_mul(2)
            .checked_add(self.domain.capacity().saturating_mul(2))
            .and_then(|bytes| {
                bytes.checked_add(
                    self.records
                        .capacity()
                        .saturating_mul(std::mem::size_of::<Record>())
                        .saturating_mul(2),
                )
            })
            .ok_or_else(|| eyre::eyre!("static zone retained-byte accounting overflow"))?;
        for record in &self.records {
            retained = retained
                .checked_add(record.retained_source_bytes().saturating_mul(2))
                .ok_or_else(|| eyre::eyre!("static record retained-byte accounting overflow"))?;
        }
        if let Some(freeze) = &self.freeze {
            retained = retained
                .checked_add(std::mem::size_of::<FreezeMetadata>().saturating_mul(2))
                .and_then(|bytes| bytes.checked_add(freeze.state.capacity().saturating_mul(2)))
                .and_then(|bytes| {
                    bytes.checked_add(
                        freeze
                            .ticket
                            .as_ref()
                            .map_or(0, |value| value.capacity().saturating_mul(2)),
                    )
                })
                .and_then(|bytes| {
                    bytes.checked_add(
                        freeze
                            .expires_at
                            .as_ref()
                            .map_or(0, |value| value.capacity().saturating_mul(2)),
                    )
                })
                .and_then(|bytes| {
                    bytes.checked_add(freeze.notes.as_ref().map_or(0, |notes| {
                        notes
                            .capacity()
                            .saturating_mul(std::mem::size_of::<String>())
                            .saturating_mul(2)
                    }))
                })
                .ok_or_else(|| eyre::eyre!("static freeze retained-byte accounting overflow"))?;
            for note in freeze.notes.as_deref().unwrap_or_default() {
                retained = retained
                    .checked_add(note.capacity().saturating_mul(2))
                    .ok_or_else(|| eyre::eyre!("static note retained-byte accounting overflow"))?;
            }
        }
        Ok(retained)
    }
    fn try_into_zone(self) -> Result<StaticZone> {
        let retained_bytes = self.retained_bytes()?;
        let canonical = normalize_domain(&self.domain)?;
        let origin = Name::from_ascii(&self.domain)
            .wrap_err_with(|| format!("invalid domain name `{}`", self.domain))?;
        let mut records = Vec::new();
        records
            .try_reserve_exact(self.records.len())
            .wrap_err("failed to reserve static-zone record table")?;
        for record in self.records {
            records.push(record.into_record(&origin)?);
        }
        let freeze = match self.freeze {
            Some(metadata) => Some(metadata.try_into_metadata()?),
            None => None,
        };
        Ok(StaticZone {
            domain: canonical,
            records,
            freeze,
            retained_bytes,
        })
    }
}
#[derive(Debug, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize)]
#[norito(tag = "type", content = "value")]
enum StaticRecordConfig {
    #[norito(rename = "A")]
    A { ttl: u32, address: String },
    #[norito(rename = "AAAA")]
    Aaaa { ttl: u32, address: String },
    #[norito(rename = "CNAME")]
    Cname { ttl: u32, target: String },
    #[norito(rename = "TXT")]
    Txt { ttl: u32, text: Vec<String> },
}
impl StaticRecordConfig {
    fn validate_bounds(&self) -> Result<()> {
        match self {
            Self::A { address, .. } => {
                check_string("static A address", address, MAX_IDENTIFIER_BYTES)
            }
            Self::Aaaa { address, .. } => {
                check_string("static AAAA address", address, MAX_IDENTIFIER_BYTES)
            }
            Self::Cname { target, .. } => {
                check_string("static CNAME target", target, MAX_IDENTIFIER_BYTES)
            }
            Self::Txt { text, .. } => {
                check_count("static TXT chunks", text.len(), MAX_CHILD_STRINGS)?;
                for chunk in text {
                    check_string("static TXT chunk", chunk, MAX_IDENTIFIER_BYTES)?;
                }
                Ok(())
            }
        }
    }
    fn retained_source_bytes(&self) -> usize {
        let base = std::mem::size_of::<Self>();
        match self {
            Self::A { address, .. } | Self::Aaaa { address, .. } => {
                base.saturating_add(address.capacity())
            }
            Self::Cname { target, .. } => base.saturating_add(target.capacity()),
            Self::Txt { text, .. } => text.iter().fold(
                base.saturating_add(
                    text.capacity()
                        .saturating_mul(std::mem::size_of::<String>()),
                ),
                |bytes, chunk| bytes.saturating_add(chunk.capacity()),
            ),
        }
    }
    fn into_record(self, origin: &Name) -> Result<Record> {
        match self {
            StaticRecordConfig::A { ttl, address } => {
                let ip: Ipv4Addr = address
                    .parse()
                    .wrap_err_with(|| format!("invalid IPv4 address `{address}`"))?;
                Ok(Record::from_rdata(origin.clone(), ttl, RData::A(A(ip))))
            }
            StaticRecordConfig::Aaaa { ttl, address } => {
                let ip: Ipv6Addr = address
                    .parse()
                    .wrap_err_with(|| format!("invalid IPv6 address `{address}`"))?;
                Ok(Record::from_rdata(
                    origin.clone(),
                    ttl,
                    RData::AAAA(AAAA(ip)),
                ))
            }
            StaticRecordConfig::Cname { ttl, target } => {
                let target = Name::from_ascii(&target)
                    .wrap_err_with(|| format!("invalid CNAME target `{target}`"))?;
                Ok(Record::from_rdata(
                    origin.clone(),
                    ttl,
                    RData::CNAME(CNAME(target)),
                ))
            }
            StaticRecordConfig::Txt { ttl, text } => Ok(Record::from_rdata(
                origin.clone(),
                ttl,
                RData::TXT(TXT::new(text)),
            )),
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use expect_test::expect;
    use std::io::Write;
    use tempfile::NamedTempFile;
    fn write_config(contents: &str) -> NamedTempFile {
        let mut file = NamedTempFile::new().expect("temp file");
        file.write_all(contents.as_bytes()).expect("write config");
        file.flush().expect("flush");
        file
    }
    #[test]
    fn parses_config() {
        let temp_bundle = NamedTempFile::new().expect("bundle file");
        let temp_rad = NamedTempFile::new().expect("rad file");
        let config_json = format!(
            r#"{{
  "resolver_id": "resolver.sora.test",
  "region": "global",
  "bundle_sources": [
    {{
      "kind": "file",
      "value": {{
        "path": "{}",
        "expected_blake3_hex": "af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262"
      }}
    }}
  ],
  "rad_sources": [
    {{
      "kind": "file",
      "value": {{
        "path": "{}",
        "expected_blake3_hex": "af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262"
      }}
    }}
  ],
  "doh_listen": ["127.0.0.1:8443"],
  "dot_listen": ["127.0.0.1:853"],
  "event_listen": "127.0.0.1:9000",
  "operations_auth_token_path": "/run/secrets/soradns-operations-token",
  "static_zones": [
    {{
      "domain": "example.sora",
      "records": [
        {{
          "type": "A",
          "value": {{"ttl": 300, "address": "192.0.2.1"}}
        }}
      ],
      "freeze": {{
        "state": "soft",
        "ticket": "SNS-DF-123",
        "expires_at": "2026-03-01T00:00:00Z",
        "notes": ["guardian review"]
      }}
    }}
  ],
  "sync_interval_secs": 45
}}"#,
            temp_bundle.path().display(),
            temp_rad.path().display()
        );
        let file = write_config(&config_json);
        let config = ResolverConfig::load_from_path(file.path()).expect("config loads");
        assert_eq!(config.resolver_id, "resolver.sora.test");
        assert_eq!(config.region, "global");
        assert_eq!(config.doh_listen.len(), 1);
        assert_eq!(config.dot_listen.len(), 1);
        assert!(config.event_listen().is_some());
        assert_eq!(
            config.operations_auth_token_path(),
            Some(Path::new("/run/secrets/soradns-operations-token"))
        );
        assert_eq!(config.static_zones().len(), 1);
        let zone = &config.static_zones()[0];
        let freeze = zone.freeze.as_ref().expect("freeze metadata parsed");
        assert_eq!(freeze.state, FreezeState::Soft);
        assert_eq!(freeze.ticket.as_deref(), Some("SNS-DF-123"));
        assert_eq!(freeze.expires_at.as_deref(), Some("2026-03-01T00:00:00Z"));
        assert_eq!(freeze.notes, vec!["guardian review".to_string()]);
        assert!(config.event_log_path().is_none());
        assert_eq!(config.sync_interval(), Duration::from_secs(45));
    }
    #[test]
    fn config_rejects_unbounded_file_event_sink() {
        let temp_bundle = NamedTempFile::new().expect("bundle file");
        let temp_rad = NamedTempFile::new().expect("rad file");
        let config_json = format!(
            r#"{{
  "resolver_id": "resolver.sora.test",
  "region": "global",
  "bundle_sources": [{{"kind":"file","value":{{"path":"{}","expected_blake3_hex":"af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262"}}}}],
  "rad_sources": [{{"kind":"file","value":{{"path":"{}","expected_blake3_hex":"af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262"}}}}],
  "event_log_path": "resolver.log"
}}"#,
            temp_bundle.path().display(),
            temp_rad.path().display()
        );
        let file = write_config(&config_json);
        let error = ResolverConfig::load_from_path(file.path())
            .expect_err("file event sink must fail closed");
        assert!(error.to_string().contains("event_log_path is disabled"));
    }
    #[test]
    fn config_rejects_raw_udp_doq_listener() {
        let temp_bundle = NamedTempFile::new().expect("bundle file");
        let temp_rad = NamedTempFile::new().expect("rad file");
        let config_json = format!(
            r#"{{
  "resolver_id": "resolver.sora.test",
  "region": "global",
  "bundle_sources": [{{"kind":"file","value":{{"path":"{}","expected_blake3_hex":"af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262"}}}}],
  "rad_sources": [{{"kind":"file","value":{{"path":"{}","expected_blake3_hex":"af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262"}}}}],
  "doq_listen": ["0.0.0.0:8853"]
}}"#,
            temp_bundle.path().display(),
            temp_rad.path().display()
        );
        let file = write_config(&config_json);
        let error = ResolverConfig::load_from_path(file.path())
            .expect_err("raw UDP DoQ listener must fail closed");
        assert!(error.to_string().contains("authenticated QUIC"));
    }
    #[test]
    fn config_rejects_plaintext_non_loopback_http_listeners() {
        let temp_bundle = NamedTempFile::new().expect("bundle file");
        let temp_rad = NamedTempFile::new().expect("rad file");
        for listener in [
            r#""doh_listen":["0.0.0.0:8443"]"#,
            r#""event_listen":"0.0.0.0:9000""#,
        ] {
            let config_json = format!(
                r#"{{
  "resolver_id":"resolver.sora.test",
  "region":"global",
  "bundle_sources":[{{"kind":"file","value":{{"path":"{}","expected_blake3_hex":"af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262"}}}}],
  "rad_sources":[{{"kind":"file","value":{{"path":"{}","expected_blake3_hex":"af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262"}}}}],
  {listener}
}}"#,
                temp_bundle.path().display(),
                temp_rad.path().display(),
            );
            let file = write_config(&config_json);
            let error = ResolverConfig::load_from_path(file.path())
                .expect_err("remote plaintext listener must fail closed");
            assert!(error.to_string().contains("loopback"));
        }
    }
    #[test]
    fn operational_listener_requires_exactly_one_private_credential_path() {
        let temp_bundle = NamedTempFile::new().expect("bundle file");
        let temp_rad = NamedTempFile::new().expect("rad file");
        let base = format!(
            r#"{{
  "resolver_id":"resolver.sora.test",
  "region":"global",
  "bundle_sources":[{{"kind":"file","value":{{"path":"{}","expected_blake3_hex":"af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262"}}}}],
  "rad_sources":[{{"kind":"file","value":{{"path":"{}","expected_blake3_hex":"af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262"}}}}],
  REPLACEMENT
}}"#,
            temp_bundle.path().display(),
            temp_rad.path().display(),
        );
        for replacement in [
            r#""event_listen":"127.0.0.1:9000""#,
            r#""operations_auth_token_path":"/run/secrets/soradns-operations-token""#,
        ] {
            let file = write_config(&base.replace("REPLACEMENT", replacement));
            let error = ResolverConfig::load_from_path(file.path())
                .expect_err("listener and credential path must be configured together");
            assert!(error.to_string().contains("operations_auth_token_path"));
        }
    }
    #[test]
    fn remote_source_kinds_are_absent_from_the_v1_schema() {
        for kind in ["torii", "sorafs"] {
            let file = write_config(&format!(
                r#"{{
  "resolver_id":"resolver.sora.test",
  "region":"global",
  "bundle_sources":[{{"kind":"{kind}","value":{{"headers":[{{"name":"authorization","value":"Bearer secret"}}]}}}}]
}}"#,
            ));
            let error = ResolverConfig::load_from_path(file.path())
                .expect_err("unknown remote source kind must fail during config decoding");
            assert!(
                error
                    .to_string()
                    .contains("failed to parse resolver config JSON")
            );

            let file = write_config(&format!(
                r#"{{
  "resolver_id":"resolver.sora.test",
  "region":"global",
  "rad_sources":[{{"kind":"{kind}","value":{{"headers":[{{"name":"authorization","value":"Bearer secret"}}]}}}}]
}}"#,
            ));
            let error = ResolverConfig::load_from_path(file.path())
                .expect_err("unknown remote RAD source kind must fail during config decoding");
            assert!(
                error
                    .to_string()
                    .contains("failed to parse resolver config JSON")
            );
        }
    }
    #[test]
    fn independently_pinned_snapshot_digest_is_enforced() {
        let bytes = b"authenticated snapshot";
        let expected = *blake3::hash(bytes).as_bytes();
        verify_pinned_snapshot(bytes, &expected, "fixture").expect("matching pin");
        let error = verify_pinned_snapshot(bytes, &[0xAA; 32], "fixture")
            .expect_err("mismatched pin must fail");
        assert!(error.to_string().contains("BLAKE3 digest"));
        assert!(parse_snapshot_digest(&"00".repeat(32), "fixture pin").is_err());
    }
    #[test]
    fn static_zone_retained_bytes_accounts_for_optional_freeze_notes() {
        let config = StaticZoneConfig {
            domain: "example.sora".to_owned(),
            records: Vec::new(),
            freeze: Some(FreezeMetadataConfig {
                state: "soft".to_owned(),
                ticket: None,
                expires_at: None,
                notes: Some(vec!["guardian review".to_owned()]),
            }),
        };
        let freeze = config.freeze.as_ref().expect("freeze metadata");
        let notes = freeze.notes.as_ref().expect("freeze notes");
        let note_bytes = notes
            .capacity()
            .saturating_mul(std::mem::size_of::<String>())
            .saturating_mul(2)
            .saturating_add(
                notes
                    .iter()
                    .map(|note| note.capacity().saturating_mul(2))
                    .sum(),
            );
        let expected = std::mem::size_of::<StaticZone>()
            .saturating_mul(2)
            .saturating_add(config.domain.capacity().saturating_mul(2))
            .saturating_add(
                config
                    .records
                    .capacity()
                    .saturating_mul(std::mem::size_of::<Record>())
                    .saturating_mul(2),
            )
            .saturating_add(std::mem::size_of::<FreezeMetadata>().saturating_mul(2))
            .saturating_add(freeze.state.capacity().saturating_mul(2))
            .saturating_add(note_bytes);
        assert_eq!(config.retained_bytes().expect("retained bytes"), expected);
    }
    #[test]
    fn empty_bundle_sources_rejected() {
        let file = write_config(
            r#"{
  "resolver_id": "resolver.sora.test",
  "region": "global",
  "bundle_sources": [],
  "rad_sources": []
}"#,
        );
        let config = ResolverConfig::load_from_path(file.path()).expect("config loads");
        let err = config.validate().expect_err("validation should fail");
        expect!["at least one bundle source is required"].assert_eq(&err.to_string());
    }
    #[test]
    fn sync_interval_defaults_to_constant() {
        let temp_bundle = NamedTempFile::new().expect("bundle file");
        let temp_rad = NamedTempFile::new().expect("rad file");
        let config_json = format!(
            r#"{{
  "resolver_id": "resolver.default",
  "region": "global",
  "bundle_sources": [{{"kind":"file","value":{{"path":"{}","expected_blake3_hex":"af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262"}}}}],
  "rad_sources": [{{"kind":"file","value":{{"path":"{}","expected_blake3_hex":"af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262"}}}}]
}}"#,
            temp_bundle.path().display(),
            temp_rad.path().display()
        );
        let file = write_config(&config_json);
        let config = ResolverConfig::load_from_path(file.path()).expect("config loads");
        assert_eq!(
            config.sync_interval(),
            Duration::from_secs(DEFAULT_SYNC_INTERVAL_SECS)
        );
    }
    #[test]
    fn zero_sync_interval_rejected() {
        let temp_bundle = NamedTempFile::new().expect("bundle file");
        let temp_rad = NamedTempFile::new().expect("rad file");
        let config_json = format!(
            r#"{{
  "resolver_id": "resolver.zero",
  "region": "global",
  "bundle_sources": [{{"kind":"file","value":{{"path":"{}","expected_blake3_hex":"af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262"}}}}],
  "rad_sources": [{{"kind":"file","value":{{"path":"{}","expected_blake3_hex":"af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262"}}}}],
  "sync_interval_secs": 0
}}"#,
            temp_bundle.path().display(),
            temp_rad.path().display()
        );
        let file = write_config(&config_json);
        let err = ResolverConfig::load_from_path(file.path()).expect_err("should fail");
        expect!["sync_interval_secs must be greater than zero"].assert_eq(&err.to_string());
    }
    #[test]
    fn override_sync_interval_updates_value() {
        let temp_bundle = NamedTempFile::new().expect("bundle file");
        let temp_rad = NamedTempFile::new().expect("rad file");
        let config_json = format!(
            r#"{{
  "resolver_id": "resolver.override",
  "region": "global",
  "bundle_sources": [{{"kind":"file","value":{{"path":"{}","expected_blake3_hex":"af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262"}}}}],
  "rad_sources": [{{"kind":"file","value":{{"path":"{}","expected_blake3_hex":"af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262"}}}}]
}}"#,
            temp_bundle.path().display(),
            temp_rad.path().display()
        );
        let file = write_config(&config_json);
        let mut config = ResolverConfig::load_from_path(file.path()).expect("config loads");
        config
            .override_sync_interval(Duration::from_secs(5))
            .expect("override succeeds");
        assert_eq!(config.sync_interval(), Duration::from_secs(5));
    }
    #[test]
    fn override_sync_interval_rejects_zero_duration() {
        let temp_bundle = NamedTempFile::new().expect("bundle file");
        let temp_rad = NamedTempFile::new().expect("rad file");
        let config_json = format!(
            r#"{{
  "resolver_id": "resolver.override.zero",
  "region": "global",
  "bundle_sources": [{{"kind":"file","value":{{"path":"{}","expected_blake3_hex":"af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262"}}}}],
  "rad_sources": [{{"kind":"file","value":{{"path":"{}","expected_blake3_hex":"af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262"}}}}]
}}"#,
            temp_bundle.path().display(),
            temp_rad.path().display()
        );
        let file = write_config(&config_json);
        let mut config = ResolverConfig::load_from_path(file.path()).expect("config loads");
        let err = config
            .override_sync_interval(Duration::from_secs(0))
            .expect_err("override should fail");
        expect!["sync interval must be greater than zero seconds"].assert_eq(&err.to_string());
    }
    #[test]
    fn config_file_byte_corridor_accepts_exact_and_rejects_plus_one() {
        let prefix = r#"{"resolver_id":"r","region":"g"}"#;
        let mut exact = prefix.to_string();
        exact.extend(std::iter::repeat_n(' ', MAX_CONFIG_BYTES - prefix.len()));
        let exact_file = write_config(&exact);
        ResolverConfig::load_from_path(exact_file.path()).expect("exact byte boundary loads");
        exact.push(' ');
        let oversized_file = write_config(&exact);
        let error = ResolverConfig::load_from_path(oversized_file.path())
            .expect_err("max + 1 bytes must fail before JSON materialisation");
        assert!(
            error
                .to_string()
                .contains("exceeding the 1048576-byte limit")
        );
    }
    #[test]
    fn collection_corridors_accept_exact_and_reject_plus_one() {
        for (label, maximum) in [
            ("test sources", MAX_SOURCES_PER_KIND),
            ("test listeners", MAX_LISTEN_ADDRESSES),
            ("test static zones", MAX_STATIC_ZONES),
            ("test static records", MAX_STATIC_RECORDS),
        ] {
            check_count(label, maximum, maximum).expect("exact collection count");
            assert!(
                check_count(label, maximum + 1, maximum).is_err(),
                "{label} max + 1 must fail"
            );
        }
    }
}
