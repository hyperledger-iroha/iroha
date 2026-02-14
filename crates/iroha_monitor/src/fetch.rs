//! HTTP polling utilities for `iroha_monitor`.
//!
//! The monitor keeps one task per peer that periodically pulls `/status` and
//! `/metrics` endpoints.  This module hides the blocking HTTP client behind
//! `tokio::spawn_blocking` and normalises the responses into lightweight
//! snapshots used by the TUI.

use std::{
    io::Read,
    time::{Duration, Instant},
};

use eyre::{Result, eyre};
use norito::{derive::JsonDeserialize, json};
use tokio::{sync::mpsc, task::JoinHandle};

pub const STATUS_HTTP_TIMEOUT: Duration = Duration::from_secs(2);
pub const METRICS_HTTP_TIMEOUT: Duration = Duration::from_secs(2);
pub const STATUS_BODY_LIMIT: usize = 128 * 1024;
pub const METRICS_BODY_LIMIT: usize = 512 * 1024;

#[derive(Debug, Clone, JsonDeserialize, Default)]
pub struct CryptoStatusPayload {
    pub sm_helpers_available: Option<bool>,
    pub sm_openssl_preview_enabled: Option<bool>,
}

#[derive(Debug, Clone, JsonDeserialize, Default)]
pub struct StatusPayload {
    pub alias: Option<String>,
    pub peers: Option<u64>,
    pub blocks: Option<u64>,
    pub blocks_non_empty: Option<u64>,
    pub commit_time_ms: Option<u64>,
    pub txs_approved: Option<u64>,
    pub txs_rejected: Option<u64>,
    pub queue_size: Option<u64>,
    pub uptime: Option<u64>,
    pub view_changes: Option<u64>,
    #[allow(dead_code)]
    pub governance: Option<json::Value>,
    pub crypto: Option<CryptoStatusPayload>,
}

impl StatusPayload {
    pub fn from_i23_status(v: &json::Value) -> Self {
        // i23: peers.total
        let peers = v
            .get("peers")
            .and_then(|p| p.get("total"))
            .and_then(|v| v.as_u64());

        // i23: blocks.height
        let blocks = v
            .get("blocks")
            .and_then(|b| b.get("height"))
            .and_then(|v| v.as_u64());

        // i23: blocks.non_empty
        let blocks_non_empty = v
            .get("blocks")
            .and_then(|b| b.get("non_empty"))
            .and_then(|v| v.as_u64());

        // i23: queue.size
        let queue_size = v
            .get("queue")
            .and_then(|q| q.get("size"))
            .and_then(|v| v.as_u64());

        // i23: uptime (単位はそのまま扱う)
        let uptime = v.get("uptime").and_then(|v| v.as_u64());

        // i23: txs.approved / rejected
        let txs_approved = v
            .get("txs")
            .and_then(|t| t.get("approved"))
            .and_then(|v| v.as_u64());

        let txs_rejected = v
            .get("txs")
            .and_then(|t| t.get("rejected"))
            .and_then(|v| v.as_u64());

        // i23: commit.time_ms
        let commit_time_ms = v
            .get("commit")
            .and_then(|c| c.get("time_ms"))
            .and_then(|v| v.as_u64());

        StatusPayload {
            alias: None,
            peers,
            blocks,
            blocks_non_empty,
            commit_time_ms,
            txs_approved,
            txs_rejected,
            queue_size,
            uptime,
            view_changes: None,
            governance: v.get("governance").cloned(),
            crypto: None,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct MetricsSnapshot {
    pub block_height: Option<u64>,
    pub block_height_non_empty: Option<u64>,
    pub tx_accepted: Option<u64>,
    pub tx_rejected: Option<u64>,
    pub tx_queue_depth: Option<u64>,
    pub gas_used: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct PeerSnapshot {
    pub status: Option<StatusPayload>,
    pub metrics: MetricsSnapshot,
    pub latency: Option<Duration>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct PeerUpdate {
    pub index: usize,
    pub snapshot: PeerSnapshot,
}

pub struct PeerFetcher {
    rx: mpsc::Receiver<PeerUpdate>,
    handles: Vec<JoinHandle<()>>,
}

impl PeerFetcher {
    pub fn new(endpoints: Vec<String>, interval: Duration) -> Self {
        let (tx, rx) = mpsc::channel::<PeerUpdate>(endpoints.len().max(1) * 2);
        let mut handles = Vec::with_capacity(endpoints.len());
        for (index, endpoint) in endpoints.into_iter().enumerate() {
            let tx = tx.clone();
            handles.push(tokio::spawn(async move {
                let mut ticker = tokio::time::interval(interval);
                ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
                loop {
                    ticker.tick().await;
                    let endpoint_owned = endpoint.clone();
                    let tx_cloned = tx.clone();
                    let result =
                        tokio::task::spawn_blocking(move || fetch_once(index, &endpoint_owned))
                            .await;
                    match result {
                        Ok(snapshot) => {
                            if tx_cloned.send(snapshot).await.is_err() {
                                break;
                            }
                        }
                        Err(err) => {
                            let msg = format!("fetch task join error: {err}");
                            let snapshot = PeerSnapshot {
                                status: None,
                                metrics: MetricsSnapshot::default(),
                                latency: None,
                                warnings: vec![msg],
                            };
                            if tx_cloned
                                .send(PeerUpdate { index, snapshot })
                                .await
                                .is_err()
                            {
                                break;
                            }
                        }
                    }
                }
            }));
        }
        Self { rx, handles }
    }

    pub async fn recv(&mut self) -> Option<PeerUpdate> {
        self.rx.recv().await
    }
}

impl Drop for PeerFetcher {
    fn drop(&mut self) {
        for handle in &self.handles {
            handle.abort();
        }
    }
}

fn fetch_once(index: usize, endpoint: &str) -> PeerUpdate {
    let mut warnings = Vec::new();

    let trimmed = endpoint.trim_end_matches('/');
    let status_url = format!("{trimmed}/status");
    let metrics_url = format!("{trimmed}/metrics");

    // ---- status ----
    let status_result = fetch_status(&status_url);
    let latency = status_result.as_ref().ok().and_then(|info| info.latency);

    if let Err(err) = &status_result {
        warnings.push(format!("status: {err}"));
    }

    let mut status = status_result.ok().and_then(|info| info.payload);

    if let Some(crypto) = status.as_ref().and_then(|payload| payload.crypto.as_ref()) {
        if crypto.sm_helpers_available.is_some_and(|v| v) {
            warnings.push("info: peer advertises SM helpers".to_owned());
        }
        if crypto.sm_openssl_preview_enabled.is_some_and(|v| v) {
            warnings.push("info: peer enables SM OpenSSL preview".to_owned());
        }
    }

    // ---- metrics ----
    let metrics = match fetch_metrics(&metrics_url) {
        Ok(metrics) => metrics,
        Err(err) => {
            warnings.push(format!("metrics: {err}"));
            MetricsSnapshot::default()
        }
    };

    // ---- status ← metrics synthesis (i23) ----
    match status {
        Some(ref mut payload) => {
            payload.blocks =
                payload.blocks.or(metrics.block_height);
    
            payload.blocks_non_empty =
                payload.blocks_non_empty.or(metrics.block_height_non_empty);
    
            // tx accepted → txs_approved
            payload.txs_approved =
                payload.txs_approved.or(metrics.tx_accepted);
    
            // queue
            payload.queue_size =
                payload.queue_size.or(metrics.tx_queue_depth);
    
            // gas は commit_time_ms とは別なので無視（monitor仕様）
        }
        None => {
            status = Some(StatusPayload {
                blocks: metrics.block_height,
                blocks_non_empty: metrics.block_height_non_empty,
                txs_approved: metrics.tx_accepted,
                queue_size: metrics.tx_queue_depth,
                ..Default::default()
            });
        }
    }

    PeerUpdate {
        index,
        snapshot: PeerSnapshot {
            status,
            metrics,
            latency,
            warnings,
        },
    }
}

struct StatusFetch {
    payload: Option<StatusPayload>,
    latency: Option<Duration>,
}

fn fetch_status(url: &str) -> Result<StatusFetch> {
    if let Some(descriptor) = parse_stub_descriptor(url, "/status") {
        let payload = stub_status_payload(descriptor.peer_index, descriptor.total_peers);
        return Ok(StatusFetch {
            payload: Some(payload),
            latency: Some(Duration::from_millis(5)),
        });
    }
    let start = Instant::now();
    let response = attohttpc::get(url)
        .timeout(STATUS_HTTP_TIMEOUT)
        .send()
        .map_err(|err| eyre!("GET {url} failed: {err}"))?;
    let latency = Some(start.elapsed());
    if !response.is_success() {
        return Err(eyre!("GET {url} returned {}", response.status()));
    }
    let (body, truncated) = read_body_with_limit(response, STATUS_BODY_LIMIT)
        .map_err(|err| eyre!("failed to read status body from {url}: {err}"))?;
    if truncated {
        return Err(eyre!("GET {url} body exceeds {} bytes", STATUS_BODY_LIMIT));
    }
    if body.is_empty() {
        return Ok(StatusFetch {
            payload: None,
            latency,
        });
    }
    let raw: json::Value = json::from_slice(&body)
        .map_err(|e| eyre!("invalid JSON from {url}: {e}"))?;

    let payload = StatusPayload::from_i23_status(&raw);
    Ok(StatusFetch {
        payload: Some(payload),
        latency,
    })
}

fn fetch_metrics(url: &str) -> Result<MetricsSnapshot> {
    if let Some(descriptor) = parse_stub_descriptor(url, "/metrics") {
        return Ok(stub_metrics_snapshot(descriptor.peer_index));
    }
    let response = attohttpc::get(url)
        .timeout(METRICS_HTTP_TIMEOUT)
        .send()
        .map_err(|err| eyre!("GET {url} failed: {err}"))?;
    if !response.is_success() {
        return Err(eyre!("GET {url} returned {}", response.status()));
    }
    let (body, truncated) = read_body_with_limit(response, METRICS_BODY_LIMIT)
        .map_err(|err| eyre!("failed to read metrics body from {url}: {err}"))?;
    if truncated {
        return Err(eyre!("GET {url} body exceeds {} bytes", METRICS_BODY_LIMIT));
    }
    let text = String::from_utf8_lossy(&body);
    Ok(parse_prometheus_metrics(&text))
}

fn read_body_with_limit<R: Read>(mut reader: R, limit: usize) -> Result<(Vec<u8>, bool)> {
    let mut buf = Vec::new();
    {
        let mut limited = (&mut reader).take((limit + 1) as u64);
        limited.read_to_end(&mut buf)?;
    }
    if buf.len() > limit {
        buf.truncate(limit);
        Ok((buf, true))
    } else {
        Ok((buf, false))
    }
}

fn parse_prometheus_metrics(text: &str) -> MetricsSnapshot {
    let mut snap = MetricsSnapshot::default();

    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        // ラベル付き txs
        if let Some(v) = line.strip_prefix("txs{type=\"accepted\"} ") {
            snap.tx_accepted = v.parse().ok();
            continue;
        }
        if let Some(v) = line.strip_prefix("txs{type=\"rejected\"} ") {
            snap.tx_rejected = v.parse().ok();
            continue;
        }

        let Some((key, value)) = line.split_once(' ') else { continue };
        let Ok(v) = value.parse::<u64>() else { continue };

        match key {
            "block_height" => snap.block_height = Some(v),
            "block_height_non_empty" => snap.block_height_non_empty = Some(v),
            "block_gas_used" => snap.gas_used = Some(v),
            "sumeragi_tx_queue_depth" => snap.tx_queue_depth = Some(v),
            _ => {}
        }
    }

    snap
}

pub struct StubCluster {
    urls: Vec<String>,
}

impl StubCluster {
    pub fn urls(&self) -> &[String] {
        &self.urls
    }
}

pub async fn spawn_stub_cluster(count: usize) -> Result<StubCluster> {
    let mut urls = Vec::with_capacity(count);
    for idx in 0..count {
        urls.push(format!("stub://peer/{}/{count}", idx + 1));
    }
    Ok(StubCluster { urls })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct StubDescriptor {
    peer_index: usize,
    total_peers: usize,
}

fn parse_stub_descriptor(url: &str, suffix: &str) -> Option<StubDescriptor> {
    let without_scheme = url.strip_prefix("stub://")?;
    let without_suffix = without_scheme.strip_suffix(suffix)?;
    let mut parts = without_suffix.split('/');
    if parts.next()? != "peer" {
        return None;
    }
    let peer_index = parts.next()?.parse().ok()?;
    let total_peers = parts.next()?.parse().ok()?;
    if parts.next().is_some() {
        return None;
    }
    if peer_index == 0 || total_peers == 0 {
        return None;
    }
    Some(StubDescriptor {
        peer_index,
        total_peers,
    })
}

fn stub_status_payload(peer_index: usize, total_peers: usize) -> StatusPayload {
    StatusPayload {
        alias: Some("祭りノード".to_owned()),
        peers: Some(total_peers as u64),
        blocks: Some(42 + peer_index as u64),
        blocks_non_empty: Some(40 + peer_index as u64),
        commit_time_ms: Some(95),
        txs_approved: Some(320 + peer_index as u64 * 4),
        txs_rejected: Some(peer_index as u64),
        queue_size: Some((peer_index as u64) % 3),
        uptime: Some(1),
        view_changes: Some(0),
        governance: None,
        crypto: None,
    }
}

fn stub_metrics_snapshot(_peer_index: usize) -> MetricsSnapshot {
    MetricsSnapshot {
        block_height: Some(1),
        tx_accepted: Some(10),
        tx_rejected: Some(0),
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_prometheus_extracts_expected_metrics() {
        let text = "# HELP\nblock_gas_used 123\nblock_fee_total_units 456\nother_metric 1";
        let metrics = parse_prometheus_metrics(text);
        assert_eq!(metrics.gas_used, Some(123));
        assert_eq!(metrics.fee_units, Some(456));
    }

    #[test]
    fn read_body_respects_limit() {
        let data = vec![b'x'; 16];
        let (body, truncated) = read_body_with_limit(std::io::Cursor::new(data), 32).unwrap();
        assert!(!truncated);
        assert_eq!(body.len(), 16);

        let data = vec![b'y'; 64];
        let (body, truncated) = read_body_with_limit(std::io::Cursor::new(data), 32).unwrap();
        assert!(truncated);
        assert_eq!(body.len(), 32);
    }

    #[test]
    fn parse_prometheus_skips_non_integer_samples() {
        let text = "block_gas_used 12.5\nblock_gas_used 42\nblock_fee_total_units NaN\nblock_fee_total_units 24";
        let metrics = parse_prometheus_metrics(text);
        assert_eq!(metrics.gas_used, Some(42));
        assert_eq!(metrics.fee_units, Some(24));
    }

    #[test]
    fn stub_descriptor_parses_expected_format() {
        let descriptor = parse_stub_descriptor("stub://peer/2/5/status", "/status").unwrap();
        assert_eq!(descriptor.peer_index, 2);
        assert_eq!(descriptor.total_peers, 5);
        assert!(parse_stub_descriptor("stub://peer/0/5/status", "/status").is_none());
        assert!(parse_stub_descriptor("stub://other/1/2/status", "/status").is_none());
        assert!(parse_stub_descriptor("stub://peer/1/status", "/status").is_none());
    }

    #[test]
    fn stub_fetch_status_returns_payload() {
        let fetch = fetch_status("stub://peer/3/4/status").expect("stub status should succeed");
        let payload = fetch.payload.expect("payload expected");
        assert_eq!(payload.alias.as_deref(), Some("祭りノード"));
        assert_eq!(payload.peers, Some(4));
        assert_eq!(payload.blocks, Some(45));
        assert!(fetch.latency.is_some());
    }

    #[test]
    fn stub_fetch_metrics_returns_synthetic_values() {
        let metrics =
            fetch_metrics("stub://peer/1/4/metrics").expect("stub metrics should succeed");
        assert_eq!(metrics.gas_used, Some(117));
        assert_eq!(metrics.fee_units, Some(57));
    }
}
