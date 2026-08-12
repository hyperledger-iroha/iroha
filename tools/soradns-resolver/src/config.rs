use std::{
    convert::TryFrom,
    net::{Ipv4Addr, Ipv6Addr, SocketAddr},
    path::{Path, PathBuf},
    str::FromStr,
    time::Duration,
};

use eyre::{Context, Result, bail};
use hickory_proto::rr::{
    Name, RData, Record,
    rdata::{A, AAAA, CNAME, TXT},
};
use norito::{decode_from_bytes_with_limits, json};
use norito_derive::{JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize};
use reqwest::header::HeaderName;

use crate::{
    bundle::ProofBundleV1,
    limits::{
        MAX_CHILD_STRINGS, MAX_CONFIG_BYTES, MAX_FIELD_BYTES, MAX_HEADERS_PER_SOURCE,
        MAX_IDENTIFIER_BYTES, MAX_LISTEN_ADDRESSES, MAX_PROOF_BUNDLE_BYTES, MAX_RAD_SNAPSHOT_BYTES,
        MAX_SOURCE_BATCH_RETAINED_BYTES, MAX_SOURCE_REFERENCES, MAX_SOURCES_PER_KIND,
        MAX_STATIC_RECORDS, MAX_STATIC_ZONE_RETAINED_BYTES, MAX_STATIC_ZONES, config_decode_limits,
        preflight_json, proof_bundle_decode_limits, read_bounded_file, read_bounded_file_async,
        read_http_body_bounded,
    },
    rad::{ResolverAttestation, decode_rad_entries},
};

/// Resolver configuration with normalised runtime values.
#[derive(Debug, Clone)]
pub struct ResolverConfig {
    pub resolver_id: String,
    pub region: String,
    doh_listen: Vec<SocketAddr>,
    dot_listen: Vec<SocketAddr>,
    doq_listen: Vec<SocketAddr>,
    event_listen: Option<SocketAddr>,
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
    pub(crate) fn doq_listen(&self) -> &[SocketAddr] {
        &self.doq_listen
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
        let dot_listen = parse_socket_list("dot_listen", raw.dot_listen)?;
        let doq_listen = parse_socket_list("doq_listen", raw.doq_listen)?;
        let event_listen = parse_socket("event_listen", raw.event_listen)?;
        let event_log_path = raw.event_log_path.map(PathBuf::from);
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
            doq_listen,
            event_listen,
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

        let mut references = 0usize;
        for source in bundle_sources {
            references = references
                .checked_add(source.reference_count())
                .filter(|count| *count <= MAX_SOURCE_REFERENCES)
                .ok_or_else(|| {
                    eyre::eyre!(
                        "bundle source references exceed the {MAX_SOURCE_REFERENCES}-entry limit"
                    )
                })?;
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
    },
    Torii {
        base_url: String,
        namehashes: Vec<String>,
        headers: Vec<HeaderEntry>,
    },
    SoraFs {
        gateway: String,
        cids: Vec<String>,
        headers: Vec<HeaderEntry>,
    },
}

impl BundleSource {
    pub async fn fetch(&self, client: &reqwest::Client) -> Result<Vec<ProofBundleV1>> {
        match self {
            Self::File { path } => {
                let label = format!("proof bundle `{}`", path.display());
                let bytes =
                    read_bounded_file_async(path.clone(), MAX_PROOF_BUNDLE_BYTES, label.clone())
                        .await?;
                let bundle = decode_proof_bundle(&bytes, &label)?;
                let mut bundles = Vec::new();
                bundles
                    .try_reserve_exact(1)
                    .wrap_err("failed to reserve local bundle result")?;
                bundles.push(bundle);
                Ok(bundles)
            }
            Self::Torii {
                base_url,
                namehashes,
                headers,
            } => {
                let base = trim_trailing_slash(base_url);
                let mut bundles = Vec::new();
                let mut retained_bytes = 0usize;
                bundles
                    .try_reserve_exact(namehashes.len())
                    .wrap_err("failed to reserve Torii bundle result list")?;
                for namehash in namehashes {
                    let url = format!("{base}/v1/soradns/proof/{namehash}");
                    let request = apply_headers(client.get(&url), headers);
                    let response = request
                        .send()
                        .await
                        .wrap_err_with(|| format!("failed to fetch proof bundle from `{url}`"))?;
                    let label = format!("proof bundle response from `{url}`");
                    let bytes =
                        read_http_body_bounded(response, MAX_PROOF_BUNDLE_BYTES, &label).await?;
                    let bundle = decode_proof_bundle(&bytes, &label)?;
                    push_fetched_bundle(&mut bundles, bundle, &mut retained_bytes)?;
                }
                Ok(bundles)
            }
            Self::SoraFs {
                gateway,
                cids,
                headers,
            } => {
                let base = trim_trailing_slash(gateway);
                let mut bundles = Vec::new();
                let mut retained_bytes = 0usize;
                bundles
                    .try_reserve_exact(cids.len())
                    .wrap_err("failed to reserve SoraFS bundle result list")?;
                for cid in cids {
                    let url = format!("{base}/ipfs/{cid}");
                    let request = apply_headers(client.get(&url), headers);
                    let response = request
                        .send()
                        .await
                        .wrap_err_with(|| format!("failed to fetch proof bundle from `{url}`"))?;
                    let label = format!("proof bundle response from `{url}`");
                    let bytes =
                        read_http_body_bounded(response, MAX_PROOF_BUNDLE_BYTES, &label).await?;
                    let bundle = decode_proof_bundle(&bytes, &label)?;
                    push_fetched_bundle(&mut bundles, bundle, &mut retained_bytes)?;
                }
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
    },
    Torii {
        base_url: String,
        headers: Vec<HeaderEntry>,
    },
    SoraFs {
        gateway: String,
        path: String,
        headers: Vec<HeaderEntry>,
    },
}

impl RadSource {
    pub async fn fetch(&self, client: &reqwest::Client) -> Result<Vec<ResolverAttestation>> {
        match self {
            Self::File { path } => {
                let label = format!("RAD snapshot `{}`", path.display());
                let bytes =
                    read_bounded_file_async(path.clone(), MAX_RAD_SNAPSHOT_BYTES, label.clone())
                        .await?;
                let entries = decode_rad_entries(&bytes).wrap_err_with(|| {
                    format!("failed to decode RAD snapshot `{}`", path.display())
                })?;
                Ok(entries)
            }
            Self::Torii { base_url, headers } => {
                let base = trim_trailing_slash(base_url);
                let url = format!("{base}/v1/soradns/resolvers");
                let request = apply_headers(client.get(&url), headers);
                let response = request
                    .send()
                    .await
                    .wrap_err_with(|| format!("failed to fetch resolver adverts from `{url}`"))?;
                let label = format!("RAD response from `{url}`");
                let bytes =
                    read_http_body_bounded(response, MAX_RAD_SNAPSHOT_BYTES, &label).await?;
                let entries = decode_rad_entries(&bytes).wrap_err_with(|| {
                    format!("failed to decode resolver attestations fetched from `{url}`")
                })?;
                Ok(entries)
            }
            Self::SoraFs {
                gateway,
                path,
                headers,
            } => {
                let base = trim_trailing_slash(gateway);
                let object = format!("{base}/{}", path.trim_start_matches('/'));
                let request = apply_headers(client.get(&object), headers);
                let response = request.send().await.wrap_err_with(|| {
                    format!("failed to fetch resolver adverts from `{object}`")
                })?;
                let label = format!("RAD response from `{object}`");
                let bytes =
                    read_http_body_bounded(response, MAX_RAD_SNAPSHOT_BYTES, &label).await?;
                let entries = decode_rad_entries(&bytes).wrap_err_with(|| {
                    format!("failed to decode resolver attestations fetched from `{object}`")
                })?;
                Ok(entries)
            }
        }
    }
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

fn push_fetched_bundle(
    bundles: &mut Vec<ProofBundleV1>,
    bundle: ProofBundleV1,
    retained_bytes: &mut usize,
) -> Result<()> {
    let entry_bytes = bundle
        .retained_bytes()?
        .checked_add(std::mem::size_of::<ProofBundleV1>())
        .ok_or_else(|| eyre::eyre!("proof-bundle fetch accounting overflow"))?;
    let next = retained_bytes
        .checked_add(entry_bytes)
        .filter(|bytes| *bytes <= MAX_SOURCE_BATCH_RETAINED_BYTES)
        .ok_or_else(|| {
            eyre::eyre!(
                "one proof-bundle source exceeds the {MAX_SOURCE_BATCH_RETAINED_BYTES}-byte retained-memory limit"
            )
        })?;
    bundles.push(bundle);
    *retained_bytes = next;
    Ok(())
}

#[derive(Debug, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize)]
#[norito(tag = "kind", content = "value")]
enum BundleSourceConfig {
    #[norito(rename = "file")]
    File { path: String },
    #[norito(rename = "torii")]
    Torii {
        base_url: String,
        namehashes: Vec<String>,
        headers: Option<Vec<HeaderConfig>>,
    },
    #[norito(rename = "sorafs")]
    SoraFs {
        gateway: String,
        cids: Vec<String>,
        headers: Option<Vec<HeaderConfig>>,
    },
}

impl BundleSourceConfig {
    fn reference_count(&self) -> usize {
        match self {
            Self::File { .. } => 1,
            Self::Torii { namehashes, .. } => namehashes.len(),
            Self::SoraFs { cids, .. } => cids.len(),
        }
    }

    fn validate_bounds(&self) -> Result<()> {
        match self {
            Self::File { path } => check_string("bundle source path", path, MAX_FIELD_BYTES),
            Self::Torii {
                base_url,
                namehashes,
                headers,
            } => {
                check_string("torii bundle source base_url", base_url, MAX_FIELD_BYTES)?;
                check_count(
                    "torii bundle source namehashes",
                    namehashes.len(),
                    MAX_SOURCE_REFERENCES,
                )?;
                for namehash in namehashes {
                    check_string(
                        "torii bundle source namehash",
                        namehash,
                        MAX_IDENTIFIER_BYTES,
                    )?;
                }
                validate_headers(headers.as_deref())
            }
            Self::SoraFs {
                gateway,
                cids,
                headers,
            } => {
                check_string("sorafs bundle source gateway", gateway, MAX_FIELD_BYTES)?;
                check_count(
                    "sorafs bundle source cids",
                    cids.len(),
                    MAX_SOURCE_REFERENCES,
                )?;
                for cid in cids {
                    check_string("sorafs bundle source cid", cid, MAX_IDENTIFIER_BYTES)?;
                }
                validate_headers(headers.as_deref())
            }
        }
    }

    fn try_into_source(self) -> Result<BundleSource> {
        match self {
            Self::File { path } => {
                if path.trim().is_empty() {
                    bail!("bundle source path must not be empty");
                }
                Ok(BundleSource::File {
                    path: PathBuf::from(path),
                })
            }
            Self::Torii {
                base_url,
                namehashes,
                headers,
            } => {
                if base_url.trim().is_empty() {
                    bail!("torii bundle source base_url must not be empty");
                }
                if namehashes.is_empty() {
                    bail!("torii bundle source requires at least one namehash");
                }
                Ok(BundleSource::Torii {
                    base_url,
                    namehashes,
                    headers: convert_headers(headers)?,
                })
            }
            Self::SoraFs {
                gateway,
                cids,
                headers,
            } => {
                if gateway.trim().is_empty() {
                    bail!("sorafs bundle source gateway must not be empty");
                }
                if cids.is_empty() {
                    bail!("sorafs bundle source requires at least one cid");
                }
                Ok(BundleSource::SoraFs {
                    gateway,
                    cids,
                    headers: convert_headers(headers)?,
                })
            }
        }
    }
}

#[derive(Debug, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize)]
#[norito(tag = "kind", content = "value")]
enum RadSourceConfig {
    #[norito(rename = "file")]
    File { path: String },
    #[norito(rename = "torii")]
    Torii {
        base_url: String,
        headers: Option<Vec<HeaderConfig>>,
    },
    #[norito(rename = "sorafs")]
    SoraFs {
        gateway: String,
        path: String,
        headers: Option<Vec<HeaderConfig>>,
    },
}

impl RadSourceConfig {
    fn validate_bounds(&self) -> Result<()> {
        match self {
            Self::File { path } => check_string("RAD source path", path, MAX_FIELD_BYTES),
            Self::Torii { base_url, headers } => {
                check_string("torii RAD source base_url", base_url, MAX_FIELD_BYTES)?;
                validate_headers(headers.as_deref())
            }
            Self::SoraFs {
                gateway,
                path,
                headers,
            } => {
                check_string("sorafs RAD source gateway", gateway, MAX_FIELD_BYTES)?;
                check_string("sorafs RAD source path", path, MAX_FIELD_BYTES)?;
                validate_headers(headers.as_deref())
            }
        }
    }

    fn try_into_source(self) -> Result<RadSource> {
        match self {
            Self::File { path } => {
                if path.trim().is_empty() {
                    bail!("rad source path must not be empty");
                }
                Ok(RadSource::File {
                    path: PathBuf::from(path),
                })
            }
            Self::Torii { base_url, headers } => {
                if base_url.trim().is_empty() {
                    bail!("torii rad source base_url must not be empty");
                }
                Ok(RadSource::Torii {
                    base_url,
                    headers: convert_headers(headers)?,
                })
            }
            Self::SoraFs {
                gateway,
                path,
                headers,
            } => {
                if gateway.trim().is_empty() {
                    bail!("sorafs rad source gateway must not be empty");
                }
                if path.trim().is_empty() {
                    bail!("sorafs rad source path must not be empty");
                }
                Ok(RadSource::SoraFs {
                    gateway,
                    path,
                    headers: convert_headers(headers)?,
                })
            }
        }
    }
}

#[derive(Debug, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize)]
struct HeaderConfig {
    name: String,
    value: String,
}

#[derive(Debug, Clone)]
pub(crate) struct HeaderEntry {
    name: HeaderName,
    value: String,
}

fn validate_headers(headers: Option<&[HeaderConfig]>) -> Result<()> {
    check_optional_count("source headers", headers, MAX_HEADERS_PER_SOURCE)?;
    for header in headers.unwrap_or_default() {
        check_string("source header name", &header.name, 256)?;
        check_string("source header value", &header.value, MAX_FIELD_BYTES)?;
    }
    Ok(())
}

fn convert_headers(configs: Option<Vec<HeaderConfig>>) -> Result<Vec<HeaderEntry>> {
    let mut entries = Vec::new();
    if let Some(headers) = configs {
        entries
            .try_reserve_exact(headers.len())
            .wrap_err("failed to reserve source header table")?;
        for header in headers {
            if header.name.trim().is_empty() {
                bail!("header name must not be empty");
            }
            let name = HeaderName::from_str(&header.name)
                .wrap_err_with(|| format!("invalid header name `{}`", header.name))?;
            entries.push(HeaderEntry {
                name,
                value: header.value,
            });
        }
    }
    Ok(entries)
}

fn apply_headers(
    mut request: reqwest::RequestBuilder,
    headers: &[HeaderEntry],
) -> reqwest::RequestBuilder {
    for header in headers {
        request = request.header(header.name.clone(), header.value.clone());
    }
    request
}

fn trim_trailing_slash(input: &str) -> String {
    input.trim_end_matches('/').to_string()
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
                    bytes.checked_add(
                        freeze
                            .notes
                            .capacity()
                            .saturating_mul(std::mem::size_of::<String>())
                            .saturating_mul(2),
                    )
                })
                .ok_or_else(|| eyre::eyre!("static freeze retained-byte accounting overflow"))?;
            for note in &freeze.notes {
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
    use std::io::Write;

    use expect_test::expect;
    use tempfile::NamedTempFile;

    use super::*;

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
        "path": "{}"
      }}
    }}
  ],
  "rad_sources": [
    {{
      "kind": "file",
      "value": {{
        "path": "{}"
      }}
    }}
  ],
  "doh_listen": ["127.0.0.1:8443"],
  "dot_listen": ["127.0.0.1:853"],
  "event_listen": "127.0.0.1:9000",
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
  "event_log_path": "resolver.log",
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
        assert_eq!(config.static_zones().len(), 1);
        let zone = &config.static_zones()[0];
        let freeze = zone.freeze.as_ref().expect("freeze metadata parsed");
        assert_eq!(freeze.state, FreezeState::Soft);
        assert_eq!(freeze.ticket.as_deref(), Some("SNS-DF-123"));
        assert_eq!(freeze.expires_at.as_deref(), Some("2026-03-01T00:00:00Z"));
        assert_eq!(freeze.notes, vec!["guardian review".to_string()]);
        assert!(config.event_log_path().is_some());
        assert_eq!(config.sync_interval(), Duration::from_secs(45));
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
  "bundle_sources": [{{"kind":"file","value":{{"path":"{}"}}}}],
  "rad_sources": [{{"kind":"file","value":{{"path":"{}"}}}}]
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
  "bundle_sources": [{{"kind":"file","value":{{"path":"{}"}}}}],
  "rad_sources": [{{"kind":"file","value":{{"path":"{}"}}}}],
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
  "bundle_sources": [{{"kind":"file","value":{{"path":"{}"}}}}],
  "rad_sources": [{{"kind":"file","value":{{"path":"{}"}}}}]
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
  "bundle_sources": [{{"kind":"file","value":{{"path":"{}"}}}}],
  "rad_sources": [{{"kind":"file","value":{{"path":"{}"}}}}]
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
            ("test references", MAX_SOURCE_REFERENCES),
            ("test headers", MAX_HEADERS_PER_SOURCE),
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
