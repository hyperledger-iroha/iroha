use std::{
    collections::{BTreeSet, VecDeque},
    future::Future,
    str::FromStr,
    sync::Arc,
    time::{Duration, Instant},
};

use eyre::{Report, eyre};
use http::StatusCode;
use iroha_config::client_api::ConfigGetDTO;
use iroha_crypto::PublicKey;
use iroha_logger::prelude::*;
use iroha_telemetry::metrics::Status;
use norito::json::{self, Value};
use reqwest::Client;
use tokio::{
    sync::{mpsc, oneshot},
    task::JoinSet,
    time::MissedTickBehavior,
};
use tracing::{Instrument, info_span};
use url::Url;

use super::{GeoLocation, ToriiUrl};

const GET_STATUS_INTERVAL: Duration = Duration::from_secs(5);
const GET_STATUS_DISCONNECT_TIMEOUT: Duration = Duration::from_mins(1);
const GET_PEERS_INTERVAL: Duration = Duration::from_mins(1);
const TELEMETRY_UNSUPPORTED_CHECK_INTERVAL: Duration = Duration::from_mins(5);
const GET_GEO_RETRY_INTERVAL: Duration = Duration::from_mins(1);
const GET_CONFIG_INIT_INTERVAL: Duration = Duration::from_secs(15);
const GET_CONFIG_MAX_INTERVAL: Duration = Duration::from_mins(2);
const GET_CONFIG_INTERVAL_MULTIPLIER: f64 = 1.67;

#[derive(Clone, Copy, Debug)]
pub struct Metrics {
    pub block: u32,
    pub block_commit_time: Duration,
    pub avg_commit_time: Duration,
    pub queue_size: u32,
    pub uptime: Duration,
}

#[derive(Clone, Debug)]
pub enum Update {
    Connected(Box<ConfigGetDTO>),
    Disconnected,
    TelemetryUnsupported,
    Metrics(Metrics),
    Geo(GeoLocation),
    Peers(BTreeSet<PublicKey>),
}

pub fn run(torii_url: ToriiUrl) -> (mpsc::Receiver<Update>, impl Future<Output = ()> + Sized) {
    let (tx, rx) = mpsc::channel(128);
    let url = Arc::new(torii_url);

    let fut = {
        let tx = tx.clone();
        let url = Arc::clone(&url);
        async move {
            let mut set = JoinSet::new();

            let geo_span_url = Arc::clone(&url);
            set.spawn({
                let tx = tx.clone();
                let url = Arc::clone(&geo_span_url);
                async move {
                    let geo = match collect_geo(&url).await {
                        Ok(geo) => geo,
                        Err(err) => {
                            iroha_logger::error!(?err, "failed to collect geo data");
                            return;
                        }
                    };
                    let _: Result<_, _> = tx.send(Update::Geo(geo)).await;
                }
                .instrument(info_span!("peer_geo", torii_url = %geo_span_url.as_ref()))
            });

            let monitor_span_url = Arc::clone(&url);
            set.spawn(
                {
                    let url = Arc::clone(&monitor_span_url);
                    async move {
                        loop {
                            let cfg = get_config_with_retry(&url).await;
                            iroha_logger::debug!(?cfg, "peer connected");
                            let _ = tx.send(Update::Connected(Box::new(cfg))).await;

                            let (status_fin_tx, status_fin_rx) = oneshot::channel();
                            let mut workers = JoinSet::new();
                            workers.spawn({
                                let tx = tx.clone();
                                let url = Arc::clone(&url);
                                async move {
                                    get_peers_periodic(&url, tx).await;
                                }
                            });
                            workers.spawn({
                                let tx = tx.clone();
                                let url = Arc::clone(&url);
                                async move {
                                    get_metrics_periodic_timeout(&url, tx).await;
                                    let _ = status_fin_tx.send(());
                                }
                            });
                            let _ = status_fin_rx.await;
                            iroha_logger::warn!(
                                "peer stopped responding to /status; marking as disconnected"
                            );
                            let _ = tx.send(Update::Disconnected).await;
                        }
                    }
                }
                .instrument(info_span!("peer_monitor", torii_url = %monitor_span_url.as_ref())),
            );

            while set.join_next().await.is_some() {}
        }
    };

    (rx, fut)
}

#[derive(Debug)]
enum IpApiComResponse {
    Success(GeoLocation),
    Fail { message: String },
}

#[derive(thiserror::Error, Debug)]
enum RequestError {
    #[error("request to ip-api.com failed: {0:?}")]
    Http(#[from] reqwest::Error),
    #[error("request to ip-api.com failed with message: {message}")]
    FailResponse { message: String },
    #[error("request to ip-api.com returned invalid payload: {0}")]
    InvalidResponse(String),
}

fn decode_ip_api_response(bytes: &[u8]) -> Result<IpApiComResponse, RequestError> {
    let value: Value =
        json::from_slice(bytes).map_err(|err| RequestError::InvalidResponse(err.to_string()))?;

    let object = match value {
        Value::Object(object) => object,
        _ => {
            return Err(RequestError::InvalidResponse(
                "expected object payload".to_owned(),
            ));
        }
    };

    let status = object
        .get("status")
        .and_then(Value::as_str)
        .ok_or_else(|| RequestError::InvalidResponse("missing status field".to_owned()))?;

    match status {
        "success" => {
            let lat = object
                .get("lat")
                .and_then(Value::as_f64)
                .ok_or_else(|| RequestError::InvalidResponse("missing lat field".to_owned()))?;
            let lon = object
                .get("lon")
                .and_then(Value::as_f64)
                .ok_or_else(|| RequestError::InvalidResponse("missing lon field".to_owned()))?;
            let country = object
                .get("country")
                .and_then(Value::as_str)
                .ok_or_else(|| RequestError::InvalidResponse("missing country field".to_owned()))?
                .to_owned();
            let city = object
                .get("city")
                .and_then(Value::as_str)
                .ok_or_else(|| RequestError::InvalidResponse("missing city field".to_owned()))?
                .to_owned();

            Ok(IpApiComResponse::Success(GeoLocation {
                lat,
                lon,
                country,
                city,
            }))
        }
        "fail" => {
            let message = object
                .get("message")
                .and_then(Value::as_str)
                .ok_or_else(|| RequestError::InvalidResponse("missing message field".to_owned()))?
                .to_owned();
            Ok(IpApiComResponse::Fail { message })
        }
        other => Err(RequestError::InvalidResponse(format!(
            "unexpected status value: {other}"
        ))),
    }
}

async fn collect_geo(torii_url: &ToriiUrl) -> eyre::Result<GeoLocation> {
    let client = Client::new();
    let url = construct_ip_api_com_query(torii_url)?;

    let do_request = || async {
        let bytes = client.get(url.clone()).send().await?.bytes().await?;
        let response = decode_ip_api_response(&bytes)?;
        match response {
            IpApiComResponse::Success(data) => Ok(data),
            IpApiComResponse::Fail { message } => Err(RequestError::FailResponse { message }),
        }
    };

    loop {
        match do_request().await {
            Ok(value) => return Ok(value),
            Err(RequestError::Http(err)) => {
                iroha_logger::warn!(?err, "failed to fetch geo (http error)");
                tokio::time::sleep(GET_GEO_RETRY_INTERVAL).await;
            }
            Err(RequestError::FailResponse { message }) => {
                iroha_logger::error!(%message, "failed to fetch geo (service error)");
                return Err(eyre!(message));
            }
            Err(RequestError::InvalidResponse(message)) => {
                iroha_logger::error!(%message, "failed to parse geo response");
                return Err(eyre!(message));
            }
        }
    }
}

fn construct_ip_api_com_query(torii_url: &ToriiUrl) -> Result<Url, eyre::Error> {
    let Some(host) = torii_url.host_str() else {
        return Err(eyre!("Torii URL does not have host"));
    };
    let mut url = Url::parse("http://ip-api.com/json").expect("valid base URL");
    url.path_segments_mut().expect("base path").push(host);
    url.query_pairs_mut()
        .append_pair("fields", "status,message,lat,lon,country,city");
    Ok(url)
}

async fn get_config_with_retry(torii_url: &ToriiUrl) -> ConfigGetDTO {
    let client = Client::new();
    let url = torii_url.0.join("/configuration").expect("valid url");

    let do_request = || async {
        let response = client.get(url.clone()).send().await?;
        let bytes = response.bytes().await?;
        let config = json::from_slice(&bytes)
            .map_err(|err| eyre!("failed to decode /configuration payload: {err}"))?;
        Ok::<_, Report>(config)
    };

    let mut interval = GET_CONFIG_INIT_INTERVAL;
    loop {
        match do_request().await {
            Ok(value) => return value,
            Err(err) => {
                iroha_logger::warn!(?err, "failed to fetch configuration");
                tokio::time::sleep(interval).await;
                let next = (interval.as_secs_f64() * GET_CONFIG_INTERVAL_MULTIPLIER)
                    .min(GET_CONFIG_MAX_INTERVAL.as_secs_f64());
                interval = Duration::from_secs_f64(next);
            }
        }
    }
}

async fn get_peers_periodic(torii_url: &ToriiUrl, tx: mpsc::Sender<Update>) -> ! {
    let client = Client::new();
    let url = torii_url.0.join("/peers").expect("valid url");

    let get = || async {
        let response = client.get(url.clone()).send().await?;
        let bytes = response.bytes().await?;
        let peers: Vec<String> = json::from_slice(&bytes)
            .map_err(|err| eyre!("failed to decode /peers payload: {err}"))?;
        Ok::<_, Report>(peers)
    };

    let mut interval = tokio::time::interval(GET_PEERS_INTERVAL);
    interval.set_missed_tick_behavior(MissedTickBehavior::Skip);

    loop {
        match get().await {
            Ok(peers) => {
                let mut set = BTreeSet::new();
                for peer_repr in peers {
                    match peer_public_key(&peer_repr) {
                        Ok(pk) => {
                            set.insert(pk);
                        }
                        Err(err) => {
                            iroha_logger::warn!(
                                peer = %peer_repr,
                                ?err,
                                "failed to parse peer public key from /peers payload"
                            );
                        }
                    }
                }
                let _ = tx.send(Update::Peers(set)).await;
            }
            Err(err) => {
                iroha_logger::warn!(?err, "failed to fetch peer list");
            }
        }
        interval.tick().await;
    }
}

fn peer_public_key(peer_repr: &str) -> eyre::Result<PublicKey> {
    let (public_key, _) = peer_repr
        .split_once('@')
        .ok_or_else(|| eyre!("peer value missing '@' separator"))?;
    PublicKey::from_str(public_key).map_err(|err| eyre!(err))
}

async fn get_metrics_periodic_timeout(torii_url: &ToriiUrl, tx: mpsc::Sender<Update>) {
    #[derive(thiserror::Error, Debug)]
    enum GetError {
        #[error("http error: {0}")]
        Http(#[from] reqwest::Error),
        #[error("failed to decode telemetry status payload: {0}")]
        Decode(#[from] norito::json::Error),
        #[error("telemetry is not available")]
        NotImplemented,
    }

    let mut avg_commit_time = AverageCommitTime::<AVG_COMMIT_BLOCK_TIME_WINDOW>::new();
    let client = Client::new();
    let url = torii_url.0.join("/status").expect("valid url");

    let get_status = || async {
        let resp = client.get(url.clone()).send().await?;
        if resp.status() == StatusCode::NOT_IMPLEMENTED {
            return Err(GetError::NotImplemented);
        }
        let bytes = resp.bytes().await?;
        let status: Status = json::from_slice(&bytes)?;
        Ok::<_, GetError>(status)
    };

    let mut telemetry_unsupported_checked = Instant::now();
    loop {
        match tokio::time::timeout(GET_STATUS_INTERVAL, get_status()).await {
            Ok(Ok(status)) => {
                let block_height = u32::try_from(status.blocks).unwrap_or(u32::MAX);
                let queue_depth = u32::try_from(status.queue_size).unwrap_or(u32::MAX);
                avg_commit_time
                    .observe(status.blocks, Duration::from_millis(status.commit_time_ms));
                let metrics = Metrics {
                    block: block_height,
                    block_commit_time: Duration::from_millis(status.commit_time_ms),
                    avg_commit_time: avg_commit_time
                        .calculate()
                        .unwrap_or_else(|| Duration::from_millis(status.commit_time_ms)),
                    queue_size: queue_depth,
                    uptime: Duration::from_millis(status.uptime.0.as_millis() as u64),
                };
                let _ = tx.send(Update::Metrics(metrics)).await;
            }
            Ok(Err(GetError::NotImplemented)) => {
                if telemetry_unsupported_checked.elapsed() >= TELEMETRY_UNSUPPORTED_CHECK_INTERVAL {
                    telemetry_unsupported_checked = Instant::now();
                    let _ = tx.send(Update::TelemetryUnsupported).await;
                }
                tokio::time::sleep(TELEMETRY_UNSUPPORTED_CHECK_INTERVAL).await;
            }
            Ok(Err(GetError::Http(err))) => {
                iroha_logger::warn!(?err, "failed to fetch peer status");
            }
            Ok(Err(GetError::Decode(err))) => {
                iroha_logger::warn!(?err, "failed to decode peer status payload");
            }
            Err(_) => {
                iroha_logger::warn!(
                    timeout = ?GET_STATUS_DISCONNECT_TIMEOUT,
                    "peer status request timed out"
                );
                let _ = tx.send(Update::Disconnected).await;
                tokio::time::sleep(GET_STATUS_DISCONNECT_TIMEOUT).await;
            }
        }
    }
}

#[derive(Default)]
struct AverageCommitTime<const N: usize> {
    buff: CircularBuffer<N>,
    last_height: Option<u64>,
}

const AVG_COMMIT_BLOCK_TIME_WINDOW: usize = 16;

impl<const N: usize> AverageCommitTime<N> {
    fn new() -> Self {
        Self::default()
    }

    fn observe(&mut self, height: u64, block_time: Duration) {
        if self.last_height.map(|x| x == height).unwrap_or(false) {
            return;
        }
        self.last_height = Some(height);
        self.buff.push_back(block_time);
    }

    fn calculate(&self) -> Option<Duration> {
        let sum = self
            .buff
            .iter()
            .fold(None, |acc, x| Some(acc.unwrap_or(Duration::ZERO) + *x));
        sum.map(|sum| {
            sum.checked_div(self.buff.len() as u32)
                .expect("non-zero if sum exists")
        })
    }
}

#[derive(Default)]
struct CircularBuffer<const N: usize> {
    data: VecDeque<Duration>,
}

impl<const N: usize> CircularBuffer<N> {
    fn push_back(&mut self, value: Duration) {
        if self.data.len() == N {
            self.data.pop_front();
        }
        self.data.push_back(value);
    }

    fn iter(&self) -> impl Iterator<Item = &Duration> {
        self.data.iter()
    }

    fn len(&self) -> usize {
        self.data.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_ip_api_com_success() {
        let payload = br#"{
            "status":"success",
            "lat":35.0,
            "lon":139.0,
            "country":"Japan",
            "city":"Tokyo"
        }"#;

        let response = decode_ip_api_response(payload).expect("payload should decode");

        match response {
            IpApiComResponse::Success(geo) => {
                assert!((geo.lat - 35.0).abs() < f64::EPSILON);
                assert!((geo.lon - 139.0).abs() < f64::EPSILON);
                assert_eq!(geo.country, "Japan");
                assert_eq!(geo.city, "Tokyo");
            }
            other => panic!("unexpected response: {other:?}"),
        }
    }

    #[test]
    fn decode_ip_api_com_failure_response() {
        let payload = br#"{
            "status":"fail",
            "message":"invalid query"
        }"#;

        let response = decode_ip_api_response(payload).expect("payload should decode");

        match response {
            IpApiComResponse::Fail { message } => {
                assert_eq!(message, "invalid query");
            }
            other => panic!("unexpected response: {other:?}"),
        }
    }
}
