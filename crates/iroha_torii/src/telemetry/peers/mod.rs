mod monitor;

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    net::SocketAddr,
    str::FromStr,
    sync::Arc,
};

use iroha_config::client_api::ConfigGetDTO;
use iroha_crypto::PublicKey;
use iroha_logger::prelude::*;
pub use monitor::Update;
use tokio::sync::RwLock;
use url::Url;

use crate::{
    explorer::ExplorerDurationDto,
    json_macros::{JsonDeserialize, JsonSerialize},
};

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
pub(super) struct PeerConfigSnapshot {
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
    geo_config: GeoLookupConfig,
}

impl PeerTelemetryService {
    pub fn new(peer_urls: Vec<ToriiUrl>, geo_config: GeoLookupConfig) -> Arc<Self> {
        let service = Arc::new(Self {
            peers: RwLock::new(BTreeMap::new()),
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
            Update::Metrics(_) => {
                // Peer metrics are used by `/v1/telemetry/live`; explorer only needs metadata.
            }
        }
    }

    pub async fn peers_info(&self) -> Vec<PeerInfoDto> {
        let guard = self.peers.read().await;
        guard.values().map(PeerState::info).collect()
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
}
