use std::{
    convert::TryFrom,
    fs::File,
    io::Read,
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
use norito::{decode_from_bytes, json};
use norito_derive::{JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize};
use reqwest::header::HeaderName;
use tokio::fs;

use crate::{
    bundle::ProofBundleV1,
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
        let mut file = File::open(path)
            .wrap_err_with(|| format!("failed to open resolver config `{}`", path.display()))?;
        let mut buf = Vec::new();
        file.read_to_end(&mut buf)
            .wrap_err_with(|| format!("failed to read resolver config `{}`", path.display()))?;
        let raw: ResolverConfigRaw =
            json::from_slice(&buf).wrap_err("failed to parse resolver config JSON")?;
        Self::try_from(raw)
    }

    /// Validate high-level invariants.
    pub fn validate(&self) -> Result<()> {
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
        let bundle_sources = raw
            .bundle_sources
            .unwrap_or_default()
            .into_iter()
            .map(BundleSourceConfig::try_into_source)
            .collect::<Result<Vec<_>>>()?;

        let rad_sources = raw
            .rad_sources
            .unwrap_or_default()
            .into_iter()
            .map(RadSourceConfig::try_into_source)
            .collect::<Result<Vec<_>>>()?;

        let static_zones = raw
            .static_zones
            .unwrap_or_default()
            .into_iter()
            .map(StaticZoneConfig::try_into_zone)
            .collect::<Result<Vec<_>>>()?;

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

const DEFAULT_SYNC_INTERVAL_SECS: u64 = 30;

fn parse_socket_list(name: &str, list: Option<Vec<String>>) -> Result<Vec<SocketAddr>> {
    let mut result = Vec::new();
    if let Some(values) = list {
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
                let bytes = fs::read(path)
                    .await
                    .wrap_err_with(|| format!("failed to read bundle `{}`", path.display()))?;
                let bundle: ProofBundleV1 =
                    decode_from_bytes(&bytes).wrap_err("failed to decode proof bundle")?;
                Ok(vec![bundle])
            }
            Self::Torii {
                base_url,
                namehashes,
                headers,
            } => {
                let base = trim_trailing_slash(base_url);
                let mut bundles = Vec::new();
                for namehash in namehashes {
                    let url = format!("{base}/v1/soradns/proof/{namehash}");
                    let request = apply_headers(client.get(&url), headers);
                    let bytes = request
                        .send()
                        .await
                        .wrap_err_with(|| format!("failed to fetch proof bundle from `{url}`"))?
                        .bytes()
                        .await
                        .wrap_err_with(|| format!("failed to read response body from `{url}`"))?;
                    let bundle: ProofBundleV1 = decode_from_bytes(&bytes).wrap_err_with(|| {
                        format!("failed to decode proof bundle fetched from `{url}`")
                    })?;
                    bundles.push(bundle);
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
                for cid in cids {
                    let url = format!("{base}/ipfs/{cid}");
                    let request = apply_headers(client.get(&url), headers);
                    let bytes = request
                        .send()
                        .await
                        .wrap_err_with(|| format!("failed to fetch proof bundle from `{url}`"))?
                        .bytes()
                        .await
                        .wrap_err_with(|| format!("failed to read response body from `{url}`"))?;
                    let bundle: ProofBundleV1 = decode_from_bytes(&bytes).wrap_err_with(|| {
                        format!("failed to decode proof bundle fetched from `{url}`")
                    })?;
                    bundles.push(bundle);
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
                let bytes = fs::read(path).await.wrap_err_with(|| {
                    format!("failed to read RAD snapshot `{}`", path.display())
                })?;
                let entries = decode_rad_entries(&bytes).wrap_err_with(|| {
                    format!("failed to decode RAD snapshot `{}`", path.display())
                })?;
                Ok(entries)
            }
            Self::Torii { base_url, headers } => {
                let base = trim_trailing_slash(base_url);
                let url = format!("{base}/v1/soradns/resolvers");
                let request = apply_headers(client.get(&url), headers);
                let bytes = request
                    .send()
                    .await
                    .wrap_err_with(|| format!("failed to fetch resolver adverts from `{url}`"))?
                    .bytes()
                    .await
                    .wrap_err_with(|| format!("failed to read response body from `{url}`"))?;
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
                let bytes = request
                    .send()
                    .await
                    .wrap_err_with(|| format!("failed to fetch resolver adverts from `{object}`"))?
                    .bytes()
                    .await
                    .wrap_err_with(|| format!("failed to read response body from `{object}`"))?;
                let entries = decode_rad_entries(&bytes).wrap_err_with(|| {
                    format!("failed to decode resolver attestations fetched from `{object}`")
                })?;
                Ok(entries)
            }
        }
    }
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

fn convert_headers(configs: Option<Vec<HeaderConfig>>) -> Result<Vec<HeaderEntry>> {
    let mut entries = Vec::new();
    if let Some(headers) = configs {
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
    fn try_into_zone(self) -> Result<StaticZone> {
        let canonical = normalize_domain(&self.domain)?;
        let origin = Name::from_ascii(&self.domain)
            .wrap_err_with(|| format!("invalid domain name `{}`", self.domain))?;
        let mut records = Vec::new();
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
}
