mod monitor;

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    net::SocketAddr,
    str::FromStr,
    sync::Arc,
    time::Duration,
};

use iroha_config::client_api::ConfigGetDTO;
use iroha_crypto::PublicKey;
use iroha_logger::prelude::*;
use monitor::Metrics as PeerMetricsSnapshot;
pub use monitor::Update;
use tokio::sync::RwLock;
use url::Url;

use crate::{
    explorer::ExplorerDurationDto,
    json_macros::{JsonDeserialize, JsonSerialize},
};

const PROPAGATION_HISTORY_LIMIT: usize = 64;
const PROPAGATION_SNAPSHOT_LIMIT: usize = 32;

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
pub struct GeoLocation {
    pub lat: f64,
    pub lon: f64,
    pub country: String,
    pub city: String,
}

#[derive(Clone, Debug, JsonSerialize)]
pub struct PeerConfigDto {
    #[norito(skip_serializing_if = "Option::is_none")]
    pub public_key: Option<String>,
    #[norito(skip_serializing_if = "Option::is_none")]
    pub queue_capacity: Option<u32>,
    #[norito(skip_serializing_if = "Option::is_none")]
    pub network_block_gossip_size: Option<u32>,
    #[norito(skip_serializing_if = "Option::is_none")]
    pub network_block_gossip_period: Option<ExplorerDurationDto>,
    #[norito(skip_serializing_if = "Option::is_none")]
    pub network_tx_gossip_size: Option<u32>,
    #[norito(skip_serializing_if = "Option::is_none")]
    pub network_tx_gossip_period: Option<ExplorerDurationDto>,
}

#[derive(Clone, Debug)]
pub(crate) struct PeerConfigSnapshot {
    pub public_key: Option<PublicKey>,
    pub queue_capacity: Option<u32>,
    pub network_block_gossip_size: Option<u32>,
    pub network_block_gossip_period_ms: Option<u64>,
    pub network_tx_gossip_size: Option<u32>,
    pub network_tx_gossip_period_ms: Option<u64>,
}

#[derive(Clone, Debug, JsonSerialize)]
pub struct PeerInfoDto {
    pub url: String,
    pub connected: bool,
    pub telemetry_unsupported: bool,
    #[norito(skip_serializing_if = "Option::is_none")]
    pub config: Option<PeerConfigDto>,
    #[norito(skip_serializing_if = "Option::is_none")]
    pub location: Option<GeoLocation>,
    #[norito(skip_serializing_if = "Option::is_none")]
    pub connected_peers: Option<Vec<String>>,
}

#[derive(Clone, Debug, JsonSerialize)]
pub struct PeerStatusDto {
    pub url: String,
    pub block: u32,
    pub commit_time: ExplorerDurationDto,
    pub avg_commit_time: ExplorerDurationDto,
    #[norito(skip_serializing_if = "Option::is_none")]
    pub status_rtt: Option<ExplorerDurationDto>,
    #[norito(skip_serializing_if = "Option::is_none")]
    pub status_rtt_avg: Option<ExplorerDurationDto>,
    #[norito(skip_serializing_if = "Option::is_none")]
    pub status_rtt_p95: Option<ExplorerDurationDto>,
    pub queue_size: u32,
    pub uptime: ExplorerDurationDto,
    #[norito(skip_serializing_if = "Option::is_none")]
    pub propagation_time: Option<ExplorerDurationDto>,
    #[norito(skip_serializing_if = "Option::is_none")]
    pub observed_at_ms: Option<u64>,
}

#[derive(Clone, Debug, JsonSerialize)]
pub struct PeerPropagationDto {
    pub block: u32,
    pub first_seen_at_ms: u64,
    pub last_seen_at_ms: u64,
    pub spread_ms: u64,
    pub peers_reported: u32,
}

#[derive(Clone, Debug)]
pub struct PeerTelemetrySnapshot {
    pub peers_info: Vec<PeerInfoDto>,
    pub peers_status: Vec<PeerStatusDto>,
    pub propagation: Vec<PeerPropagationDto>,
}

#[derive(Clone, Debug)]
pub struct GeoLookupConfig {
    pub enabled: bool,
    pub endpoint: Option<Url>,
}

impl GeoLookupConfig {
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            endpoint: None,
        }
    }
}

impl From<&iroha_config::parameters::actual::ToriiPeerGeo> for GeoLookupConfig {
    fn from(config: &iroha_config::parameters::actual::ToriiPeerGeo) -> Self {
        Self {
            enabled: config.enabled,
            endpoint: config.endpoint.clone(),
        }
    }
}

pub struct PeerTelemetryService {
    peers: RwLock<BTreeMap<ToriiUrl, PeerState>>,
    propagation: RwLock<PropagationTracker>,
    geo_config: GeoLookupConfig,
}

impl PeerTelemetryService {
    pub fn new(peer_urls: Vec<ToriiUrl>, geo_config: GeoLookupConfig) -> Arc<Self> {
        let service = Arc::new(Self {
            peers: RwLock::new(BTreeMap::new()),
            propagation: RwLock::new(PropagationTracker::default()),
            geo_config,
        });
        for url in BTreeSet::from_iter(peer_urls) {
            service.spawn_monitor(url);
        }
        service
    }

    fn spawn_monitor(self: &Arc<Self>, url: ToriiUrl) {
        let service = Arc::clone(self);
        let geo_config = service.geo_config.clone();
        tokio::spawn(async move {
            let (mut rx, fut) = monitor::run(url.clone(), geo_config);
            tokio::spawn(fut);
            while let Some(update) = rx.recv().await {
                service.apply_update(url.clone(), update).await;
            }
        });
    }

    async fn apply_update(&self, url: ToriiUrl, update: Update) {
        let peer_url = url.as_str().to_owned();
        let mut metrics_update = None;
        let mut guard = self.peers.write().await;
        let state = guard
            .entry(url.clone())
            .or_insert_with(|| PeerState::new(url.clone()));
        match update {
            Update::Connected(config) => {
                state.connected = true;
                state.telemetry_unsupported = false;
                state.config = Some(*config);
            }
            Update::Disconnected => {
                state.connected = false;
            }
            Update::TelemetryUnsupported => {
                state.telemetry_unsupported = true;
            }
            Update::Geo(geo) => {
                state.geo = Some(geo);
            }
            Update::Peers(peers) => {
                let list = peers
                    .into_iter()
                    .map(|pk| pk.to_string())
                    .collect::<Vec<_>>();
                state.connected_peers = Some(list);
            }
            Update::Metrics(metrics) => {
                state.metrics = Some(metrics);
                metrics_update = Some(metrics);
            }
        }
        drop(guard);

        if let Some(metrics) = metrics_update {
            self.observe_propagation(&peer_url, metrics).await;
        }
    }

    async fn observe_propagation(&self, peer_url: &str, metrics: PeerMetricsSnapshot) {
        let Some(observed_at_ms) = metrics.observed_at_ms else {
            return;
        };
        let mut propagation = self.propagation.write().await;
        propagation.observe(peer_url, metrics.block, observed_at_ms);
    }

    pub async fn peers_info(&self) -> Vec<PeerInfoDto> {
        let guard = self.peers.read().await;
        guard.values().map(PeerState::info).collect()
    }

    pub async fn peers_status(&self) -> Vec<PeerStatusDto> {
        let first_seen_by_block = {
            let propagation = self.propagation.read().await;
            propagation.first_seen_by_block()
        };
        let guard = self.peers.read().await;
        guard
            .values()
            .filter_map(|peer| peer.status(&first_seen_by_block))
            .collect()
    }

    pub async fn propagation(&self, limit: usize) -> Vec<PeerPropagationDto> {
        let propagation = self.propagation.read().await;
        propagation.snapshot(limit)
    }

    pub async fn snapshot(&self) -> PeerTelemetrySnapshot {
        let (first_seen_by_block, propagation) = {
            let propagation = self.propagation.read().await;
            (
                propagation.first_seen_by_block(),
                propagation.snapshot(PROPAGATION_SNAPSHOT_LIMIT),
            )
        };
        let guard = self.peers.read().await;
        let peers_info = guard.values().map(PeerState::info).collect();
        let peers_status = guard
            .values()
            .filter_map(|peer| peer.status(&first_seen_by_block))
            .collect();
        PeerTelemetrySnapshot {
            peers_info,
            peers_status,
            propagation,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct ToriiUrl(Url);

impl ToriiUrl {
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    pub fn host_str(&self) -> Option<&str> {
        self.0.host_str()
    }
}

impl fmt::Display for ToriiUrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for ToriiUrl {
    type Err = url::ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Url::parse(s).map(Self)
    }
}

impl From<Url> for ToriiUrl {
    fn from(url: Url) -> Self {
        Self(url)
    }
}

impl TryFrom<SocketAddr> for ToriiUrl {
    type Error = url::ParseError;

    fn try_from(addr: SocketAddr) -> Result<Self, Self::Error> {
        Url::parse(&format!("http://{}", addr)).map(Self)
    }
}

struct PeerState {
    url: ToriiUrl,
    connected: bool,
    telemetry_unsupported: bool,
    config: Option<PeerConfigSnapshot>,
    geo: Option<GeoLocation>,
    connected_peers: Option<Vec<String>>,
    metrics: Option<PeerMetricsSnapshot>,
}

impl PeerState {
    fn new(url: ToriiUrl) -> Self {
        Self {
            url,
            connected: false,
            telemetry_unsupported: false,
            config: None,
            geo: None,
            connected_peers: None,
            metrics: None,
        }
    }

    fn info(&self) -> PeerInfoDto {
        PeerInfoDto {
            url: self.url.as_str().to_string(),
            connected: self.connected,
            telemetry_unsupported: self.telemetry_unsupported,
            config: self.config.as_ref().map(PeerConfigDto::from_config),
            location: self.geo.clone(),
            connected_peers: self.connected_peers.clone(),
        }
    }

    fn status(&self, first_seen_by_block: &BTreeMap<u32, u64>) -> Option<PeerStatusDto> {
        let metrics = self.metrics?;
        let propagation_time = metrics.observed_at_ms.and_then(|observed_at_ms| {
            first_seen_by_block
                .get(&metrics.block)
                .copied()
                .map(|first_seen_ms| observed_at_ms.saturating_sub(first_seen_ms))
        });
        Some(PeerStatusDto {
            url: self.url.as_str().to_string(),
            block: metrics.block,
            commit_time: ExplorerDurationDto {
                ms: duration_ms_u64(metrics.block_commit_time),
            },
            avg_commit_time: ExplorerDurationDto {
                ms: duration_ms_u64(metrics.avg_commit_time),
            },
            status_rtt: metrics.status_rtt.map(|duration| ExplorerDurationDto {
                ms: duration_ms_u64(duration),
            }),
            status_rtt_avg: metrics.status_rtt_avg.map(|duration| ExplorerDurationDto {
                ms: duration_ms_u64(duration),
            }),
            status_rtt_p95: metrics.status_rtt_p95.map(|duration| ExplorerDurationDto {
                ms: duration_ms_u64(duration),
            }),
            queue_size: metrics.queue_size,
            uptime: ExplorerDurationDto {
                ms: duration_ms_u64(metrics.uptime),
            },
            propagation_time: propagation_time.map(|ms| ExplorerDurationDto { ms }),
            observed_at_ms: metrics.observed_at_ms,
        })
    }
}

fn duration_ms_u64(duration: std::time::Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

#[derive(Default)]
struct PropagationTracker {
    by_block: BTreeMap<u32, BlockPropagationEntry>,
}

impl PropagationTracker {
    fn observe(&mut self, peer_url: &str, block: u32, observed_at_ms: u64) {
        match self.by_block.entry(block) {
            std::collections::btree_map::Entry::Vacant(slot) => {
                slot.insert(BlockPropagationEntry::new(peer_url, observed_at_ms));
            }
            std::collections::btree_map::Entry::Occupied(mut slot) => {
                slot.get_mut().observe(peer_url, observed_at_ms);
            }
        }

        while self.by_block.len() > PROPAGATION_HISTORY_LIMIT {
            let Some(oldest) = self.by_block.keys().next().copied() else {
                break;
            };
            self.by_block.remove(&oldest);
        }
    }

    fn first_seen_by_block(&self) -> BTreeMap<u32, u64> {
        self.by_block
            .iter()
            .map(|(block, entry)| (*block, entry.first_seen_at_ms))
            .collect()
    }

    fn snapshot(&self, limit: usize) -> Vec<PeerPropagationDto> {
        if limit == 0 {
            return Vec::new();
        }

        let mut entries = self
            .by_block
            .iter()
            .rev()
            .take(limit)
            .map(|(block, entry)| PeerPropagationDto {
                block: *block,
                first_seen_at_ms: entry.first_seen_at_ms,
                last_seen_at_ms: entry.last_seen_at_ms,
                spread_ms: entry.spread_ms(),
                peers_reported: entry.peers_reported(),
            })
            .collect::<Vec<_>>();
        entries.reverse();
        entries
    }
}

#[derive(Clone, Debug)]
struct BlockPropagationEntry {
    first_seen_at_ms: u64,
    last_seen_at_ms: u64,
    peers: BTreeSet<String>,
}

impl BlockPropagationEntry {
    fn new(peer_url: &str, observed_at_ms: u64) -> Self {
        let mut peers = BTreeSet::new();
        peers.insert(peer_url.to_owned());
        Self {
            first_seen_at_ms: observed_at_ms,
            last_seen_at_ms: observed_at_ms,
            peers,
        }
    }

    fn observe(&mut self, peer_url: &str, observed_at_ms: u64) {
        if !self.peers.insert(peer_url.to_owned()) {
            return;
        }
        self.first_seen_at_ms = self.first_seen_at_ms.min(observed_at_ms);
        self.last_seen_at_ms = self.last_seen_at_ms.max(observed_at_ms);
    }

    fn spread_ms(&self) -> u64 {
        self.last_seen_at_ms.saturating_sub(self.first_seen_at_ms)
    }

    fn peers_reported(&self) -> u32 {
        u32::try_from(self.peers.len()).unwrap_or(u32::MAX)
    }
}

impl PeerConfigDto {
    fn from_config(cfg: &PeerConfigSnapshot) -> Self {
        Self {
            public_key: cfg.public_key.as_ref().map(ToString::to_string),
            queue_capacity: cfg.queue_capacity,
            network_block_gossip_size: cfg.network_block_gossip_size,
            network_block_gossip_period: cfg
                .network_block_gossip_period_ms
                .map(|ms| ExplorerDurationDto { ms: ms.into() }),
            network_tx_gossip_size: cfg.network_tx_gossip_size,
            network_tx_gossip_period: cfg
                .network_tx_gossip_period_ms
                .map(|ms| ExplorerDurationDto { ms: ms.into() }),
        }
    }
}

impl From<&ConfigGetDTO> for PeerConfigSnapshot {
    fn from(cfg: &ConfigGetDTO) -> Self {
        Self {
            public_key: Some(cfg.public_key.clone()),
            queue_capacity: cfg.queue.capacity.get().try_into().ok(),
            network_block_gossip_size: Some(cfg.network.block_gossip_size.get()),
            network_block_gossip_period_ms: Some(cfg.network.block_gossip_period_ms.into()),
            network_tx_gossip_size: Some(cfg.network.transaction_gossip_size.get()),
            network_tx_gossip_period_ms: Some(cfg.network.transaction_gossip_period_ms.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn geo_lookup_config_respects_disable_helper() {
        let config = GeoLookupConfig::disabled();
        assert!(!config.enabled);
        assert!(config.endpoint.is_none());
    }

    #[test]
    fn geo_lookup_config_from_actual_copies_values() {
        let endpoint = Url::parse("https://geo.example").expect("valid endpoint");
        let actual = iroha_config::parameters::actual::ToriiPeerGeo {
            enabled: true,
            endpoint: Some(endpoint.clone()),
        };
        let config = GeoLookupConfig::from(&actual);
        assert!(config.enabled);
        assert_eq!(
            config.endpoint.as_ref().map(Url::as_str),
            Some(endpoint.as_str())
        );
    }

    #[tokio::test]
    async fn peers_status_reflects_metrics_updates() {
        let service = PeerTelemetryService::new(Vec::new(), GeoLookupConfig::disabled());
        let url: ToriiUrl = "http://peer.example:8080".parse().expect("torii url");
        service
            .apply_update(
                url.clone(),
                Update::Metrics(monitor::Metrics {
                    block: 42,
                    block_commit_time: Duration::from_millis(850),
                    avg_commit_time: Duration::from_millis(700),
                    queue_size: 3,
                    uptime: Duration::from_secs(3600),
                    status_rtt: Some(Duration::from_millis(18)),
                    status_rtt_avg: Some(Duration::from_millis(21)),
                    status_rtt_p95: Some(Duration::from_millis(34)),
                    observed_at_ms: Some(100),
                }),
            )
            .await;

        let statuses = service.peers_status().await;
        assert_eq!(statuses.len(), 1);
        let status = &statuses[0];
        assert_eq!(status.url, url.as_str());
        assert_eq!(status.block, 42);
        assert_eq!(status.commit_time.ms, 850);
        assert_eq!(status.avg_commit_time.ms, 700);
        assert_eq!(
            status.status_rtt.as_ref().map(|duration| duration.ms),
            Some(18)
        );
        assert_eq!(
            status.status_rtt_avg.as_ref().map(|duration| duration.ms),
            Some(21)
        );
        assert_eq!(
            status.status_rtt_p95.as_ref().map(|duration| duration.ms),
            Some(34)
        );
        assert_eq!(status.queue_size, 3);
        assert_eq!(status.uptime.ms, 3_600_000);
        assert_eq!(
            status.propagation_time.as_ref().map(|duration| duration.ms),
            Some(0)
        );
        assert_eq!(status.observed_at_ms, Some(100));
    }

    #[tokio::test]
    async fn snapshot_returns_info_and_status_views() {
        let service = PeerTelemetryService::new(Vec::new(), GeoLookupConfig::disabled());
        let url: ToriiUrl = "http://peer.example:8080".parse().expect("torii url");
        service
            .apply_update(
                url.clone(),
                Update::Connected(Box::new(PeerConfigSnapshot {
                    public_key: None,
                    queue_capacity: Some(256),
                    network_block_gossip_size: Some(64),
                    network_block_gossip_period_ms: Some(1_500),
                    network_tx_gossip_size: Some(32),
                    network_tx_gossip_period_ms: Some(2_500),
                })),
            )
            .await;
        service
            .apply_update(
                url.clone(),
                Update::Metrics(monitor::Metrics {
                    block: 9,
                    block_commit_time: Duration::from_millis(1200),
                    avg_commit_time: Duration::from_millis(1100),
                    queue_size: 1,
                    uptime: Duration::from_secs(120),
                    status_rtt: None,
                    status_rtt_avg: None,
                    status_rtt_p95: None,
                    observed_at_ms: Some(200),
                }),
            )
            .await;

        let snapshot = service.snapshot().await;
        assert_eq!(snapshot.peers_info.len(), 1);
        assert_eq!(snapshot.peers_status.len(), 1);
        assert_eq!(snapshot.propagation.len(), 1);
        assert_eq!(snapshot.peers_info[0].url, url.as_str());
        assert_eq!(snapshot.peers_status[0].url, url.as_str());
        assert_eq!(snapshot.propagation[0].block, 9);
        assert_eq!(snapshot.propagation[0].spread_ms, 0);
        assert_eq!(snapshot.propagation[0].peers_reported, 1);
    }

    #[tokio::test]
    async fn peers_status_computes_propagation_from_first_seen_timestamp() {
        let service = PeerTelemetryService::new(Vec::new(), GeoLookupConfig::disabled());
        let url_a: ToriiUrl = "http://peer-a.example:8080".parse().expect("torii url");
        let url_b: ToriiUrl = "http://peer-b.example:8080".parse().expect("torii url");

        service
            .apply_update(
                url_a.clone(),
                Update::Metrics(monitor::Metrics {
                    block: 20,
                    block_commit_time: Duration::from_millis(400),
                    avg_commit_time: Duration::from_millis(390),
                    queue_size: 1,
                    uptime: Duration::from_secs(10),
                    status_rtt: Some(Duration::from_millis(14)),
                    status_rtt_avg: Some(Duration::from_millis(16)),
                    status_rtt_p95: Some(Duration::from_millis(20)),
                    observed_at_ms: Some(1_000),
                }),
            )
            .await;
        service
            .apply_update(
                url_b.clone(),
                Update::Metrics(monitor::Metrics {
                    block: 20,
                    block_commit_time: Duration::from_millis(410),
                    avg_commit_time: Duration::from_millis(395),
                    queue_size: 2,
                    uptime: Duration::from_secs(11),
                    status_rtt: Some(Duration::from_millis(18)),
                    status_rtt_avg: Some(Duration::from_millis(19)),
                    status_rtt_p95: Some(Duration::from_millis(27)),
                    observed_at_ms: Some(1_045),
                }),
            )
            .await;

        let statuses = service.peers_status().await;
        let status_a = statuses
            .iter()
            .find(|status| status.url == url_a.as_str())
            .expect("status for peer a");
        let status_b = statuses
            .iter()
            .find(|status| status.url == url_b.as_str())
            .expect("status for peer b");
        assert_eq!(
            status_a
                .propagation_time
                .as_ref()
                .map(|duration| duration.ms),
            Some(0)
        );
        assert_eq!(
            status_b
                .propagation_time
                .as_ref()
                .map(|duration| duration.ms),
            Some(45)
        );

        let propagation = service.propagation(10).await;
        assert_eq!(propagation.len(), 1);
        assert_eq!(propagation[0].block, 20);
        assert_eq!(propagation[0].spread_ms, 45);
        assert_eq!(propagation[0].peers_reported, 2);
    }
}
