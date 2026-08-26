use super::{GeoLocation, GeoLookupConfig, PeerConfigSnapshot, ToriiUrl};
use crate::operator_signatures;
use eyre::{Report, eyre};
use http::StatusCode;
use iroha_config::client_api::ConfigGetDTO;
use iroha_crypto::{KeyPair, PublicKey};
use iroha_data_model::NetworkId;
use iroha_logger::prelude::*;
use iroha_telemetry::metrics::Status;
use norito::json::{self, Value};
use reqwest::{Client, redirect::Policy};
use std::{
    collections::{BTreeSet, VecDeque},
    future::Future,
    net::{IpAddr, Ipv4Addr, Ipv6Addr},
    str::FromStr,
    sync::Arc,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tokio::{
    sync::{mpsc, oneshot},
    task::JoinSet,
    time::MissedTickBehavior,
};
use tracing::{Instrument, info_span};
use url::Url;
const GEO_QUERY_FIELDS: &str = "status,message,lat,lon,country,city";
#[cfg(test)]
const GET_STATUS_INTERVAL: Duration = Duration::from_millis(200);
#[cfg(not(test))]
const GET_STATUS_INTERVAL: Duration = Duration::from_secs(5);
const GET_PEERS_INTERVAL: Duration = Duration::from_mins(1);
#[cfg(test)]
const TELEMETRY_UNSUPPORTED_CHECK_INTERVAL: Duration = Duration::from_millis(200);
#[cfg(not(test))]
const TELEMETRY_UNSUPPORTED_CHECK_INTERVAL: Duration = Duration::from_mins(5);
const GET_GEO_RETRY_INTERVAL: Duration = Duration::from_mins(1);
const GET_CONFIG_INIT_INTERVAL: Duration = Duration::from_secs(15);
const GET_CONFIG_MAX_INTERVAL: Duration = Duration::from_mins(2);
const GET_CONFIG_INTERVAL_MULTIPLIER: f64 = 1.67;
const STATUS_RTT_WINDOW: usize = 32;
/// Fixed JSON envelope returned by the configured geo provider.
const GEO_RESPONSE_MAX_BYTES: usize = 16 * 1024;
/// Complete public/operator configuration snapshot accepted by the monitor.
const CONFIG_RESPONSE_MAX_BYTES: usize = 4 * 1024 * 1024;
/// Bounded online-peer inventory accepted from one monitored node.
const PEERS_RESPONSE_MAX_BYTES: usize = 1024 * 1024;
/// Fixed-schema `/status` response accepted from one monitored node.
const STATUS_RESPONSE_MAX_BYTES: usize = 64 * 1024;
/// Read one remote response without allowing a peer, proxy, or decompressor to
/// grow a monitor task's resident body buffer without bound.
async fn read_response_body_bounded(
    mut response: reqwest::Response,
    max_bytes: usize,
    label: &'static str,
) -> Result<Vec<u8>, Report> {
    if max_bytes == 0 {
        return Err(eyre!("{label} response byte limit must be non-zero"));
    }
    let max_bytes_u64 = u64::try_from(max_bytes)
        .map_err(|_| eyre!("{label} response byte limit does not fit u64"))?;
    if let Some(declared) = response.content_length()
        && declared > max_bytes_u64
    {
        return Err(eyre!(
            "{label} response declares {declared} bytes, exceeding the {max_bytes}-byte limit"
        ));
    }
    let initial_capacity = response
        .content_length()
        .and_then(|declared| usize::try_from(declared).ok())
        .unwrap_or(0)
        .min(max_bytes);
    let mut body = Vec::new();
    body.try_reserve_exact(initial_capacity)
        .map_err(|error| eyre!("failed to reserve {label} response buffer: {error}"))?;
    while let Some(chunk) = response.chunk().await? {
        let next_len = body
            .len()
            .checked_add(chunk.len())
            .ok_or_else(|| eyre!("{label} response length overflowed"))?;
        if next_len > max_bytes {
            return Err(eyre!(
                "{label} response exceeded the {max_bytes}-byte limit while streaming"
            ));
        }
        body.try_reserve(chunk.len())
            .map_err(|error| eyre!("failed to grow {label} response buffer: {error}"))?;
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}
#[derive(Clone, Copy, Debug)]
pub struct Metrics {
    pub block: u32,
    pub block_commit_time: Duration,
    pub avg_commit_time: Duration,
    pub queue_size: u32,
    pub uptime: Duration,
    pub status_rtt: Option<Duration>,
    pub status_rtt_avg: Option<Duration>,
    pub status_rtt_p95: Option<Duration>,
    pub observed_at_ms: Option<u64>,
}
#[derive(Clone, Debug)]
pub enum Update {
    Connected(Box<PeerConfigSnapshot>),
    Disconnected,
    TelemetryUnsupported,
    Metrics(Metrics),
    Geo(GeoLocation),
    Peers(BTreeSet<PublicKey>),
}
pub fn run(
    torii_url: ToriiUrl,
    geo_config: GeoLookupConfig,
    network_id: NetworkId,
    operator_signer: Option<KeyPair>,
) -> (mpsc::Receiver<Update>, impl Future<Output = ()> + Sized) {
    let (tx, rx) = mpsc::channel(128);
    let url = Arc::new(torii_url);
    let fut = {
        let tx = tx.clone();
        let url = Arc::clone(&url);
        async move {
            let mut set = JoinSet::new();
            if geo_config.enabled {
                let geo_span_url = Arc::clone(&url);
                let geo_config = geo_config.clone();
                set.spawn({
                    let tx = tx.clone();
                    let url = Arc::clone(&geo_span_url);
                    async move {
                        let geo = match collect_geo(&url, geo_config).await {
                            Ok(geo) => geo,
                            Err(GeoLookupError::Disabled) => {
                                iroha_logger::debug!("geo lookup disabled for peer telemetry");
                                return;
                            }
                            Err(GeoLookupError::NonPublicHost { host }) => {
                                iroha_logger::debug!(
                                    %host,
                                    "skipping geo lookup for non-public torii host"
                                );
                                return;
                            }
                            Err(GeoLookupError::MissingHost) => {
                                iroha_logger::warn!(
                                    "Torii URL does not have host; skipping geo lookup"
                                );
                                return;
                            }
                            Err(GeoLookupError::MissingEndpoint) => {
                                iroha_logger::warn!(
                                    "peer geo lookup enabled without torii.peer_geo.endpoint; skipping geo lookup"
                                );
                                return;
                            }
                            Err(GeoLookupError::InsecureEndpoint { endpoint }) => {
                                iroha_logger::warn!(
                                    %endpoint,
                                    "peer geo lookup endpoint must use HTTPS; skipping geo lookup"
                                );
                                return;
                            }
                            Err(GeoLookupError::InvalidEndpoint { endpoint }) => {
                                iroha_logger::warn!(
                                    %endpoint,
                                    "peer geo lookup endpoint is not a base URL; skipping geo lookup"
                                );
                                return;
                            }
                            Err(err) => {
                                iroha_logger::error!(?err, "failed to collect geo data");
                                return;
                            }
                        };
                        let _: Result<_, _> = tx.send(Update::Geo(geo)).await;
                    }
                    .instrument(info_span!("peer_geo", torii_url = %geo_span_url.as_ref()))
                });
            } else {
                iroha_logger::debug!("peer geo lookups disabled by configuration");
            }
            let monitor_span_url = Arc::clone(&url);
            set.spawn(
                {
                    let url = Arc::clone(&monitor_span_url);
                    async move {
                        loop {
                            let cfg =
                                get_config_with_retry(&url, &network_id, operator_signer.as_ref())
                                    .await;
                            iroha_logger::debug!(?cfg, "peer connected");
                            let _ = tx.send(Update::Connected(Box::new(cfg))).await;
                            let (status_fin_tx, status_fin_rx) = oneshot::channel();
                            let mut workers = JoinSet::new();
                            workers.spawn({
                                let tx = tx.clone();
                                let url = Arc::clone(&url);
                                let network_id = network_id;
                                let operator_signer = operator_signer.clone();
                                async move {
                                    get_peers_periodic(
                                        &url,
                                        &network_id,
                                        operator_signer.as_ref(),
                                        tx,
                                    )
                                    .await;
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
    #[error("request to geo endpoint failed: {0:?}")]
    Http(#[from] reqwest::Error),
    #[error("geo endpoint returned failure message: {message}")]
    FailResponse { message: String },
    #[error("geo endpoint returned invalid payload: {0}")]
    InvalidResponse(String),
}
#[derive(thiserror::Error, Debug)]
enum GeoLookupError {
    #[error("geo lookup disabled")]
    Disabled,
    #[error("Torii URL does not have host")]
    MissingHost,
    #[error("peer geo lookup enabled without an endpoint")]
    MissingEndpoint,
    #[error("Torii host is not public: {host}")]
    NonPublicHost { host: String },
    #[error("geo endpoint must use HTTPS: {endpoint}")]
    InsecureEndpoint { endpoint: String },
    #[error("geo endpoint is not a base URL: {endpoint}")]
    InvalidEndpoint { endpoint: String },
    #[error(transparent)]
    Request(#[from] RequestError),
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
async fn collect_geo(
    torii_url: &ToriiUrl,
    geo_config: GeoLookupConfig,
) -> Result<GeoLocation, GeoLookupError> {
    if !geo_config.enabled {
        return Err(GeoLookupError::Disabled);
    }
    let client = Client::new();
    let url = construct_geo_query(torii_url, geo_config.endpoint.as_ref())?;
    let do_request = || async {
        let response = client.get(url.clone()).send().await?;
        let bytes = read_response_body_bounded(response, GEO_RESPONSE_MAX_BYTES, "geo lookup")
            .await
            .map_err(|error| RequestError::InvalidResponse(error.to_string()))?;
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
                return Err(GeoLookupError::Request(RequestError::FailResponse {
                    message,
                }));
            }
            Err(RequestError::InvalidResponse(message)) => {
                iroha_logger::error!(%message, "failed to parse geo response");
                return Err(GeoLookupError::Request(RequestError::InvalidResponse(
                    message,
                )));
            }
        }
    }
}
fn is_public_geo_host(host: &str) -> bool {
    if host.eq_ignore_ascii_case("localhost") {
        return false;
    }
    host.parse::<IpAddr>().map_or(true, is_public_ip)
}
fn is_public_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(addr) => is_public_ipv4(addr),
        IpAddr::V6(addr) => is_public_ipv6(addr),
    }
}
fn is_public_ipv4(addr: Ipv4Addr) -> bool {
    if addr.is_private()
        || addr.is_loopback()
        || addr.is_link_local()
        || addr.is_multicast()
        || addr.is_broadcast()
        || addr.is_unspecified()
    {
        return false;
    }
    let octets = addr.octets();
    if octets[0] == 100 && (64..=127).contains(&octets[1]) {
        return false;
    }
    if octets[0] == 198 && (18..=19).contains(&octets[1]) {
        return false;
    }
    if octets[0] == 192 && octets[1] == 0 && octets[2] == 0 {
        return false;
    }
    if (octets[0] == 192 && octets[1] == 0 && octets[2] == 2)
        || (octets[0] == 198 && octets[1] == 51 && octets[2] == 100)
        || (octets[0] == 203 && octets[1] == 0 && octets[2] == 113)
    {
        return false;
    }
    if octets[0] >= 240 {
        return false;
    }
    true
}
fn is_public_ipv6(addr: Ipv6Addr) -> bool {
    if addr.is_loopback()
        || addr.is_unspecified()
        || addr.is_multicast()
        || addr.is_unique_local()
        || addr.is_unicast_link_local()
    {
        return false;
    }
    if let Some(v4) = addr.to_ipv4() {
        return is_public_ipv4(v4);
    }
    let seg0 = addr.segments()[0];
    if (seg0 & 0xffc0) == 0xfec0 {
        return false;
    }
    let segments = addr.segments();
    if segments[0] == 0x2001 && segments[1] == 0x0db8 {
        return false;
    }
    true
}
fn construct_geo_query(
    torii_url: &ToriiUrl,
    endpoint: Option<&Url>,
) -> Result<Url, GeoLookupError> {
    let Some(host) = torii_url.host_str() else {
        return Err(GeoLookupError::MissingHost);
    };
    if !is_public_geo_host(host) {
        return Err(GeoLookupError::NonPublicHost {
            host: host.to_owned(),
        });
    }
    let Some(mut url) = endpoint.cloned() else {
        return Err(GeoLookupError::MissingEndpoint);
    };
    if url.scheme() != "https" {
        return Err(GeoLookupError::InsecureEndpoint {
            endpoint: url.to_string(),
        });
    }
    let endpoint_label = url.to_string();
    {
        let mut segments =
            url.path_segments_mut()
                .map_err(|_| GeoLookupError::InvalidEndpoint {
                    endpoint: endpoint_label,
                })?;
        segments.push(host);
    }
    url.query_pairs_mut()
        .append_pair("fields", GEO_QUERY_FIELDS);
    Ok(url)
}
fn decode_peer_config_payload(bytes: &[u8]) -> eyre::Result<PeerConfigSnapshot> {
    let config = json::from_slice::<ConfigGetDTO>(bytes)
        .map_err(|error| eyre!("failed to decode canonical /v1/configuration payload: {error}"))?;
    Ok(PeerConfigSnapshot::from(&config))
}
fn configuration_request(
    client: &Client,
    url: Url,
    network_id: &NetworkId,
    operator_signer: Option<&KeyPair>,
) -> eyre::Result<reqwest::Request> {
    let config_uri: crate::Uri = iroha_torii_shared::uri::CONFIGURATION
        .parse()
        .expect("static configuration URI");
    let mut request = client
        .get(url)
        .header(http::header::ACCEPT, "application/json");
    if let Some(key_pair) = operator_signer {
        let headers = operator_signatures::signed_request_headers(
            key_pair,
            network_id,
            &crate::Method::GET,
            &config_uri,
            &[],
        )
        .map_err(|error| eyre!("failed to sign /v1/configuration operator request: {error}"))?;
        request = request.headers(headers);
    }
    request.build().map_err(Into::into)
}
fn decode_peer_config_response(
    status: StatusCode,
    bytes: &[u8],
) -> eyre::Result<PeerConfigSnapshot> {
    if !status.is_success() {
        return Err(eyre!("/v1/configuration returned HTTP {status}"));
    }
    decode_peer_config_payload(bytes)
}
async fn get_config_with_retry(
    torii_url: &ToriiUrl,
    network_id: &NetworkId,
    operator_signer: Option<&KeyPair>,
) -> PeerConfigSnapshot {
    let client = Client::new();
    let url = torii_url
        .0
        .join(iroha_torii_shared::uri::CONFIGURATION)
        .expect("valid url");
    let do_request = || async {
        let request = configuration_request(&client, url.clone(), network_id, operator_signer)?;
        let response = client.execute(request).await?;
        let status = response.status();
        let bytes =
            read_response_body_bounded(response, CONFIG_RESPONSE_MAX_BYTES, "peer configuration")
                .await?;
        let config = decode_peer_config_response(status, &bytes)?;
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
fn signed_peers_request(
    client: &Client,
    url: Url,
    network_id: &NetworkId,
    operator_signer: Option<&KeyPair>,
) -> eyre::Result<reqwest::Request> {
    let key_pair = operator_signer
        .ok_or_else(|| eyre!("/v1/peers requires an immutable operator signing context"))?;
    let peers_uri: crate::Uri = iroha_torii_shared::uri::PEERS
        .parse()
        .expect("static peers URI");
    let headers = operator_signatures::signed_request_headers(
        key_pair,
        network_id,
        &crate::Method::GET,
        &peers_uri,
        &[],
    )
    .map_err(|error| eyre!("failed to sign /v1/peers operator request: {error}"))?;
    client
        .get(url)
        .header(http::header::ACCEPT, "application/json")
        .headers(headers)
        .build()
        .map_err(Into::into)
}
async fn get_peers_periodic(
    torii_url: &ToriiUrl,
    network_id: &NetworkId,
    operator_signer: Option<&KeyPair>,
    tx: mpsc::Sender<Update>,
) -> ! {
    let client = Client::builder()
        .redirect(Policy::none())
        .build()
        .expect("peer monitor HTTP client configuration is valid");
    let url = torii_url
        .0
        .join(iroha_torii_shared::uri::PEERS)
        .expect("valid url");
    let get = || async {
        let request = signed_peers_request(&client, url.clone(), network_id, operator_signer)?;
        let response = client.execute(request).await?;
        let status = response.status();
        let bytes =
            read_response_body_bounded(response, PEERS_RESPONSE_MAX_BYTES, "peer list").await?;
        if !status.is_success() {
            return Err(eyre!("/v1/peers returned HTTP {status}"));
        }
        let peers: Vec<String> = json::from_slice(&bytes)
            .map_err(|err| eyre!("failed to decode /v1/peers payload: {err}"))?;
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
                                "failed to parse peer public key from /v1/peers payload"
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
        #[error("telemetry is not available ({0})")]
        TelemetryUnavailable(StatusCode),
        #[error("unexpected status code: {0}")]
        UnexpectedStatus(StatusCode),
        #[error("invalid bounded response: {0}")]
        Response(String),
    }
    let mut avg_commit_time = AverageCommitTime::<AVG_COMMIT_BLOCK_TIME_WINDOW>::new();
    let mut status_rtt_window = LatencyWindow::<STATUS_RTT_WINDOW>::new();
    let client = Client::new();
    let url = torii_url.0.join("/status").expect("valid url");
    let get_status = || async {
        let started_at = Instant::now();
        let resp = client
            .get(url.clone())
            .header(http::header::ACCEPT, "application/json")
            .send()
            .await?;
        let status = resp.status();
        if matches!(
            status,
            StatusCode::NOT_IMPLEMENTED | StatusCode::NOT_FOUND | StatusCode::SERVICE_UNAVAILABLE
        ) {
            return Err(GetError::TelemetryUnavailable(status));
        }
        if !status.is_success() {
            return Err(GetError::UnexpectedStatus(status));
        }
        let bytes = read_response_body_bounded(resp, STATUS_RESPONSE_MAX_BYTES, "peer status")
            .await
            .map_err(|error| GetError::Response(error.to_string()))?;
        let status: Status = json::from_slice(&bytes)?;
        let request_rtt = started_at.elapsed();
        let observed_at_ms = unix_epoch_ms();
        Ok::<_, GetError>((status, request_rtt, observed_at_ms))
    };
    let mut telemetry_unsupported_checked = Instant::now();
    let mut interval = tokio::time::interval(GET_STATUS_INTERVAL);
    interval.set_missed_tick_behavior(MissedTickBehavior::Skip);
    loop {
        match tokio::time::timeout(GET_STATUS_INTERVAL, get_status()).await {
            Ok(Ok((status, request_rtt, observed_at_ms))) => {
                let block_height = u32::try_from(status.blocks).unwrap_or(u32::MAX);
                let queue_depth = u32::try_from(status.queue_size).unwrap_or(u32::MAX);
                avg_commit_time
                    .observe(status.blocks, Duration::from_millis(status.commit_time_ms));
                status_rtt_window.observe(request_rtt);
                let metrics = Metrics {
                    block: block_height,
                    block_commit_time: Duration::from_millis(status.commit_time_ms),
                    avg_commit_time: avg_commit_time
                        .calculate()
                        .unwrap_or_else(|| Duration::from_millis(status.commit_time_ms)),
                    queue_size: queue_depth,
                    uptime: Duration::from_millis(status.uptime.0.as_millis() as u64),
                    status_rtt: Some(request_rtt),
                    status_rtt_avg: status_rtt_window.average(),
                    status_rtt_p95: status_rtt_window.percentile(95),
                    observed_at_ms,
                };
                let _ = tx.send(Update::Metrics(metrics)).await;
            }
            Ok(Err(GetError::TelemetryUnavailable(_))) => {
                if telemetry_unsupported_checked.elapsed() >= TELEMETRY_UNSUPPORTED_CHECK_INTERVAL {
                    telemetry_unsupported_checked = Instant::now();
                    let _ = tx.send(Update::TelemetryUnsupported).await;
                }
                tokio::time::sleep(TELEMETRY_UNSUPPORTED_CHECK_INTERVAL).await;
            }
            Ok(Err(GetError::UnexpectedStatus(status))) => {
                iroha_logger::warn!(status = status.as_u16(), "unexpected /status response");
            }
            Ok(Err(GetError::Http(err))) => {
                iroha_logger::warn!(?err, "failed to fetch peer status");
            }
            Ok(Err(GetError::Decode(err))) => {
                iroha_logger::warn!(?err, "failed to decode peer status payload");
            }
            Ok(Err(GetError::Response(err))) => {
                iroha_logger::warn!(%err, "rejected oversized peer status payload");
            }
            Err(_) => {
                iroha_logger::warn!(
                    timeout = ?GET_STATUS_INTERVAL,
                    "peer status request timed out; disconnecting"
                );
                return;
            }
        }
        interval.tick().await;
    }
}
fn unix_epoch_ms() -> Option<u64> {
    let elapsed = SystemTime::now().duration_since(UNIX_EPOCH).ok()?;
    u64::try_from(elapsed.as_millis()).ok()
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
struct LatencyWindow<const N: usize> {
    buff: CircularBuffer<N>,
}
impl<const N: usize> LatencyWindow<N> {
    fn new() -> Self {
        Self::default()
    }
    fn observe(&mut self, sample: Duration) {
        self.buff.push_back(sample);
    }
    fn average(&self) -> Option<Duration> {
        let sum = self
            .buff
            .iter()
            .fold(None, |acc, x| Some(acc.unwrap_or(Duration::ZERO) + *x))?;
        sum.checked_div(self.buff.len() as u32)
    }
    fn percentile(&self, percentile: u8) -> Option<Duration> {
        let mut values_ms = self
            .buff
            .iter()
            .map(|sample| sample.as_millis())
            .collect::<Vec<_>>();
        if values_ms.is_empty() {
            return None;
        }
        if values_ms.len() == 1 {
            let single = u64::try_from(values_ms[0]).ok()?;
            return Some(Duration::from_millis(single));
        }
        values_ms.sort_unstable();
        let clamped = percentile.min(100) as f64;
        let rank = (clamped / 100.0) * ((values_ms.len() - 1) as f64);
        let lower = rank.floor() as usize;
        let upper = rank.ceil() as usize;
        let selected = if lower == upper {
            values_ms[lower] as f64
        } else {
            let weight = rank - (lower as f64);
            let low = values_ms[lower] as f64;
            let high = values_ms[upper] as f64;
            low + ((high - low) * weight)
        };
        if !selected.is_finite() || selected < 0.0 {
            return None;
        }
        Some(Duration::from_millis(selected.round() as u64))
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
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;
    async fn raw_http_response(response: Vec<u8>) -> reqwest::Response {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind bounded-response listener");
        let addr = listener.local_addr().expect("bounded-response address");
        tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.expect("accept request");
            let mut request = [0_u8; 1024];
            let _ = socket.read(&mut request).await;
            socket
                .write_all(&response)
                .await
                .expect("write raw response");
        });
        Client::new()
            .get(format!("http://{addr}/bounded"))
            .send()
            .await
            .expect("receive raw response headers")
    }
    #[tokio::test]
    async fn bounded_response_reader_rejects_declared_and_streamed_overflow() {
        let declared = raw_http_response(
            b"HTTP/1.1 200 OK\r\nContent-Length: 9\r\nConnection: close\r\n\r\n123456789".to_vec(),
        )
        .await;
        let declared_error = read_response_body_bounded(declared, 8, "test")
            .await
            .expect_err("declared overflow must fail before body retention");
        assert!(declared_error.to_string().contains("declares 9 bytes"));
        let exact = raw_http_response(
            b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n8\r\n12345678\r\n0\r\n\r\n"
                .to_vec(),
        )
        .await;
        assert_eq!(
            read_response_body_bounded(exact, 8, "test")
                .await
                .expect("exact limit is accepted"),
            b"12345678"
        );
        let streamed = raw_http_response(
            b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n9\r\n123456789\r\n0\r\n\r\n"
                .to_vec(),
        )
        .await;
        let streamed_error = read_response_body_bounded(streamed, 8, "test")
            .await
            .expect_err("chunked overflow must fail at max plus one");
        assert!(streamed_error.to_string().contains("while streaming"));
    }
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
    #[test]
    fn geo_host_publicity_checks() {
        assert!(!is_public_geo_host("127.0.0.1"));
        assert!(!is_public_geo_host("10.0.0.1"));
        assert!(!is_public_geo_host("::1"));
        assert!(!is_public_geo_host("::ffff:127.0.0.1"));
        assert!(!is_public_geo_host("::ffff:10.0.0.1"));
        assert!(!is_public_geo_host("localhost"));
        assert!(!is_public_geo_host("192.0.2.1"));
        assert!(!is_public_geo_host("2001:db8::1"));
        assert!(is_public_geo_host("8.8.8.8"));
        assert!(is_public_geo_host("::ffff:8.8.8.8"));
        assert!(is_public_geo_host("example.com"));
    }
    #[test]
    fn construct_geo_query_requires_explicit_endpoint() {
        let torii_url: ToriiUrl = "http://example.com:8080".parse().expect("valid torii url");
        let err =
            construct_geo_query(&torii_url, None).expect_err("missing endpoint should fail closed");
        assert!(matches!(err, GeoLookupError::MissingEndpoint));
    }
    #[test]
    fn construct_geo_query_rejects_non_https_endpoint() {
        let torii_url: ToriiUrl = "http://example.com:8080".parse().expect("valid torii url");
        let endpoint = Url::parse("http://geo.internal/api").expect("valid endpoint");
        let err = construct_geo_query(&torii_url, Some(&endpoint))
            .expect_err("non-HTTPS endpoint should fail closed");
        match err {
            GeoLookupError::InsecureEndpoint { endpoint } => {
                assert_eq!(endpoint, "http://geo.internal/api");
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }
    #[test]
    fn construct_geo_query_uses_custom_endpoint() {
        let torii_url: ToriiUrl = "http://example.com:8080".parse().expect("valid torii url");
        let endpoint = Url::parse("https://geo.internal/api").expect("valid endpoint");
        let url = construct_geo_query(&torii_url, Some(&endpoint)).expect("geo query should build");
        assert_eq!(url.scheme(), "https");
        assert_eq!(url.host_str(), Some("geo.internal"));
        assert_eq!(url.path(), "/api/example.com");
        let fields = url
            .query_pairs()
            .find_map(|(key, value)| (key == "fields").then(|| value.into_owned()));
        assert_eq!(fields.as_deref(), Some(GEO_QUERY_FIELDS));
    }
    #[tokio::test]
    async fn collect_geo_respects_disabled_config() {
        let url: ToriiUrl = "http://example.com:8080".parse().expect("valid torii url");
        let err = collect_geo(&url, GeoLookupConfig::disabled())
            .await
            .expect_err("disabled config should short-circuit");
        assert!(matches!(err, GeoLookupError::Disabled));
    }
    #[tokio::test]
    async fn collect_geo_rejects_non_public_hosts() {
        let url: ToriiUrl = "http://127.0.0.1:8080".parse().expect("valid torii url");
        let err = collect_geo(
            &url,
            GeoLookupConfig {
                enabled: true,
                endpoint: Some(Url::parse("https://geo.internal/api").expect("valid endpoint")),
            },
        )
        .await
        .expect_err("non-public host should be rejected");
        match err {
            GeoLookupError::NonPublicHost { host } => {
                assert_eq!(host, "127.0.0.1");
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }
    #[tokio::test]
    async fn collect_geo_requires_explicit_endpoint_when_enabled() {
        let url: ToriiUrl = "http://example.com:8080".parse().expect("valid torii url");
        let err = collect_geo(
            &url,
            GeoLookupConfig {
                enabled: true,
                endpoint: None,
            },
        )
        .await
        .expect_err("missing endpoint should fail closed");
        assert!(matches!(err, GeoLookupError::MissingEndpoint));
    }
    #[tokio::test]
    async fn collect_geo_rejects_non_https_endpoint() {
        let url: ToriiUrl = "http://example.com:8080".parse().expect("valid torii url");
        let err = collect_geo(
            &url,
            GeoLookupConfig {
                enabled: true,
                endpoint: Some(Url::parse("http://geo.internal/api").expect("valid endpoint")),
            },
        )
        .await
        .expect_err("non-HTTPS endpoint should fail closed");
        match err {
            GeoLookupError::InsecureEndpoint { endpoint } => {
                assert_eq!(endpoint, "http://geo.internal/api");
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }
    #[tokio::test]
    async fn metrics_timeout_exits_poll_loop() {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind listener");
        let addr = listener.local_addr().expect("listener addr");
        let accept_task = tokio::spawn(async move {
            if let Ok((socket, _)) = listener.accept().await {
                let _socket = socket;
                tokio::time::sleep(Duration::from_secs(60)).await;
            }
        });
        let url: ToriiUrl = format!("http://{addr}").parse().expect("valid torii url");
        let (tx, _rx) = mpsc::channel(1);
        let result = tokio::time::timeout(
            GET_STATUS_INTERVAL + Duration::from_secs(2),
            get_metrics_periodic_timeout(&url, tx),
        )
        .await;
        accept_task.abort();
        assert!(result.is_ok(), "metrics loop should exit after timeout");
    }
    #[tokio::test]
    async fn metrics_marks_telemetry_unsupported_on_service_unavailable() {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind listener");
        let addr = listener.local_addr().expect("listener addr");
        let server = tokio::spawn(async move {
            loop {
                let (mut socket, _) = listener.accept().await.expect("accept socket");
                let mut buf = [0u8; 1024];
                let _ = socket.read(&mut buf).await;
                let response = b"HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
                let _ = socket.write_all(response).await;
            }
        });
        let url: ToriiUrl = format!("http://{addr}").parse().expect("valid torii url");
        let (tx, mut rx) = mpsc::channel(4);
        let metrics_task = tokio::spawn(async move {
            get_metrics_periodic_timeout(&url, tx).await;
        });
        let timeout =
            TELEMETRY_UNSUPPORTED_CHECK_INTERVAL + GET_STATUS_INTERVAL + GET_STATUS_INTERVAL;
        let update = tokio::time::timeout(timeout, rx.recv())
            .await
            .expect("telemetry update timeout")
            .expect("telemetry update");
        assert!(matches!(update, Update::TelemetryUnsupported));
        metrics_task.abort();
        server.abort();
    }
    #[test]
    fn decode_peer_config_payload_rejects_retired_partial_shape() {
        let payload = br#"{
            "queue": { "capacity": 512 },
            "network": {
                "block_gossip_size": 64,
                "block_gossip_period_ms": 1500,
                "transaction_gossip_size": 32,
                "transaction_gossip_period_ms": 2500
            }
        }"#;
        let error = decode_peer_config_payload(payload)
            .expect_err("partial pre-release configuration shape must fail closed");
        assert!(
            error
                .to_string()
                .contains("failed to decode canonical /v1/configuration payload"),
            "unexpected error: {error}"
        );
    }
    #[test]
    fn decode_peer_config_payload_rejects_unrelated_error_shape() {
        let payload = br#"{ "error": "unauthorized" }"#;
        let err = decode_peer_config_payload(payload).expect_err("invalid shape should fail");
        let message = err.to_string();
        assert!(
            message.contains("failed to decode canonical /v1/configuration payload"),
            "unexpected error: {message}"
        );
    }
    #[test]
    fn peer_monitor_configuration_request_accepts_json() {
        let cfg = crate::test_utils::mk_minimal_root_cfg();
        let network_id = crate::test_utils::signed_query_network_id();
        let client = Client::new();
        let url = Url::parse("https://peer.example/v1/configuration").expect("configuration URL");
        let request = configuration_request(&client, url, &network_id, Some(&cfg.common.key_pair))
            .expect("configuration request");
        assert_eq!(
            request.headers().get(http::header::ACCEPT),
            Some(&http::HeaderValue::from_static("application/json"))
        );
        assert!(request.headers().contains_key("x-iroha-operator-signature"));
    }
    #[test]
    fn peer_monitor_configuration_rejects_http_failure_before_decode() {
        let error = decode_peer_config_response(StatusCode::INTERNAL_SERVER_ERROR, b"not-json")
            .expect_err("HTTP failure must be reported before payload decode");
        assert_eq!(
            error.to_string(),
            "/v1/configuration returned HTTP 500 Internal Server Error"
        );

        for status in [
            StatusCode::NOT_FOUND,
            StatusCode::UNAUTHORIZED,
            StatusCode::FORBIDDEN,
        ] {
            let error = decode_peer_config_response(status, b"not-json")
                .expect_err("missing or unauthorized configuration must fail closed");
            assert_eq!(
                error.to_string(),
                format!("/v1/configuration returned HTTP {status}")
            );
        }
    }
    #[test]
    fn peer_monitor_builds_an_exact_signed_empty_body_get() {
        let cfg = crate::test_utils::mk_minimal_root_cfg();
        let network_id = crate::test_utils::signed_query_network_id();
        let client = Client::builder()
            .redirect(Policy::none())
            .build()
            .expect("test HTTP client");
        let url = Url::parse("https://peer.example/v1/peers").expect("peers URL");
        let request = signed_peers_request(&client, url, &network_id, Some(&cfg.common.key_pair))
            .expect("signed peers request");
        assert_eq!(request.method(), reqwest::Method::GET);
        assert_eq!(request.url().path(), "/v1/peers");
        assert!(request.url().query().is_none());
        assert!(request.body().is_none());
        assert_eq!(
            request.headers().get(http::header::ACCEPT),
            Some(&http::HeaderValue::from_static("application/json"))
        );
        for header in [
            "x-iroha-operator-public-key",
            "x-iroha-operator-timestamp-ms",
            "x-iroha-operator-nonce",
            "x-iroha-operator-signature",
        ] {
            assert!(request.headers().contains_key(header), "missing {header}");
        }
        assert!(!request.headers().contains_key("authorization"));
        assert!(!request.headers().contains_key("x-api-token"));
    }
    #[test]
    fn peer_monitor_fails_before_dispatch_without_an_operator_signer() {
        let client = Client::new();
        let url = Url::parse("https://peer.example/v1/peers").expect("peers URL");
        let error = signed_peers_request(
            &client,
            url,
            &crate::test_utils::signed_query_network_id(),
            None,
        )
        .expect_err("missing operator signer must fail closed");
        assert!(error.to_string().contains("operator signing context"));
    }
    #[test]
    fn latency_window_calculates_average_and_percentile() {
        let mut window = LatencyWindow::<4>::new();
        window.observe(Duration::from_millis(10));
        window.observe(Duration::from_millis(30));
        window.observe(Duration::from_millis(20));
        window.observe(Duration::from_millis(40));
        assert_eq!(
            window.average().map(|duration| duration.as_millis()),
            Some(25)
        );
        assert_eq!(
            window.percentile(95).map(|duration| duration.as_millis()),
            Some(39)
        );
    }
    #[test]
    fn latency_window_keeps_latest_samples_only() {
        let mut window = LatencyWindow::<3>::new();
        window.observe(Duration::from_millis(10));
        window.observe(Duration::from_millis(20));
        window.observe(Duration::from_millis(30));
        window.observe(Duration::from_millis(40));
        // oldest sample (10ms) is evicted
        assert_eq!(
            window.average().map(|duration| duration.as_millis()),
            Some(30)
        );
        assert_eq!(
            window.percentile(95).map(|duration| duration.as_millis()),
            Some(39)
        );
    }
}
