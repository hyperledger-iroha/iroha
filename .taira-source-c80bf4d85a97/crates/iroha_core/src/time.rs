//! Network Time Service (NTS)
//!
//! A lightweight time synchronization service that computes a network time
//! offset using NTP-style pings to peers and a trimmed-median aggregator.
//! - Periodically samples online peers with `TimePing` messages and collects
//!   `TimePong` replies.
//! - Computes per-sample offset and RTT using t1..t4 timestamps and filters
//!   high-RTT outliers.
//! - Aggregates offsets via trimmed median; exposes `now()` for Torii and
//!   timers.

use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    sync::{OnceLock, RwLock},
    time::Instant,
};

use iroha_config::parameters::actual::NtsEnforcementMode;
use iroha_data_model::peer::Peer;
use norito::codec::{Decode, Encode};
use tokio::sync::watch;

use crate::IrohaNetwork;

/// Outbound time probe message (peer → peer).
#[derive(Clone, Copy, Debug, Encode, Decode)]
pub struct TimePing {
    /// Monotonic probe identifier.
    pub id: u64,
    /// Local send timestamp (ms since UNIX epoch).
    pub t1_ms: u64,
}

/// Inbound time probe response (peer → peer).
#[derive(Clone, Copy, Debug, Encode, Decode)]
pub struct TimePong {
    /// Echoed probe identifier.
    pub id: u64,
    /// Receiver timestamp at arrival.
    pub t2_ms: u64,
    /// Receiver timestamp at response send.
    pub t3_ms: u64,
}

/// Snapshot of the current network time estimation.
#[derive(Clone, Copy, Debug)]
pub struct NetworkTimeStatus {
    /// Adjusted current time based on the estimated offset.
    pub now: std::time::SystemTime,
    /// Estimated offset from local clock in milliseconds.
    pub offset_ms: i64,
    /// Robust dispersion estimate in milliseconds (median absolute deviation).
    pub confidence_ms: u64,
    /// Number of peer samples used in the current aggregation (post-filter).
    pub sample_count: usize,
    /// Number of peers with at least one recent sample (pre-filter).
    pub peer_count: usize,
    /// Whether NTS fell back to local time due to missing/invalid samples.
    pub fallback: bool,
    /// Health evaluation flags for the current status snapshot.
    pub health: NtsHealth,
}

/// Health evaluation flags for the current NTS snapshot.
#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Copy, Debug)]
pub struct NtsHealth {
    /// Whether the minimum sample threshold has been met.
    pub min_samples_ok: bool,
    /// Whether the absolute offset is within configured bounds.
    pub offset_ok: bool,
    /// Whether the confidence (MAD) is within configured bounds.
    pub confidence_ok: bool,
    /// Overall health status (true only when all checks pass and no fallback).
    pub healthy: bool,
}

/// Health policy thresholds for evaluating NTS snapshots.
#[derive(Debug, Clone, Copy)]
pub struct NtsHealthPolicy {
    /// Minimum number of peer samples required before NTS is considered healthy.
    pub min_samples: usize,
    /// Maximum absolute offset (ms) permitted before NTS is considered unhealthy (0 disables).
    pub max_offset_ms: u64,
    /// Maximum confidence (MAD) in ms permitted before NTS is considered unhealthy (0 disables).
    pub max_confidence_ms: u64,
}

impl NtsHealthPolicy {
    fn evaluate(
        self,
        sample_count: usize,
        offset_ms: i64,
        confidence_ms: u64,
        fallback: bool,
    ) -> NtsHealth {
        let min_samples_ok = sample_count >= self.min_samples;
        let offset_ok = self.max_offset_ms == 0 || offset_ms.unsigned_abs() <= self.max_offset_ms;
        let confidence_ok = self.max_confidence_ms == 0 || confidence_ms <= self.max_confidence_ms;
        let healthy = !fallback && min_samples_ok && offset_ok && confidence_ok;
        NtsHealth {
            min_samples_ok,
            offset_ok,
            confidence_ok,
            healthy,
        }
    }
}

impl Default for NtsHealthPolicy {
    fn default() -> Self {
        Self {
            min_samples: iroha_config::parameters::defaults::time::NTS_MIN_SAMPLES,
            max_offset_ms: iroha_config::parameters::defaults::time::NTS_MAX_OFFSET_MS,
            max_confidence_ms: iroha_config::parameters::defaults::time::NTS_MAX_CONFIDENCE_MS,
        }
    }
}

#[derive(Default, Clone, Copy)]
struct Sample {
    offset_ms: i64,
    rtt_ms: u64,
}

/// Runtime parameters for the Network Time Service.
#[derive(Debug, Clone, Copy)]
pub struct Params {
    /// Sampling interval for peer time probes.
    pub sample_interval: std::time::Duration,
    /// Maximum peers to sample per round.
    pub sample_cap_per_round: usize,
    /// Maximum acceptable round-trip time (milliseconds) for samples.
    pub max_rtt_ms: u64,
    /// Trim percent for median aggregation (0–45 allowed; 10 typical).
    pub trim_percent: u8,
    /// Per-peer ring buffer capacity for samples.
    pub per_peer_buffer: usize,
    /// Enable EMA smoothing of network offset.
    pub smoothing_enabled: bool,
    /// EMA alpha in [0,1]; higher means more responsive.
    pub smoothing_alpha: f64,
    /// Maximum allowed adjustment per minute (ms) when smoothing.
    pub max_adjust_ms_per_min: u64,
    /// Health policy thresholds for NTS status evaluation.
    pub health_policy: NtsHealthPolicy,
    /// Enforcement mode for unhealthy NTS during admission.
    pub enforcement_mode: NtsEnforcementMode,
}

impl Default for Params {
    fn default() -> Self {
        Self {
            sample_interval: std::time::Duration::from_secs(5),
            sample_cap_per_round: 8,
            max_rtt_ms: 500,
            trim_percent: 10,
            per_peer_buffer: 16,
            smoothing_enabled: false,
            smoothing_alpha: 0.2,
            max_adjust_ms_per_min: 50,
            health_policy: NtsHealthPolicy::default(),
            enforcement_mode: NtsEnforcementMode::Warn,
        }
    }
}

impl From<&iroha_config::parameters::actual::Nts> for Params {
    fn from(x: &iroha_config::parameters::actual::Nts) -> Self {
        Self {
            sample_interval: x.sample_interval,
            sample_cap_per_round: x.sample_cap_per_round,
            max_rtt_ms: x.max_rtt_ms,
            trim_percent: x.trim_percent,
            per_peer_buffer: x.per_peer_buffer,
            smoothing_enabled: x.smoothing_enabled,
            smoothing_alpha: x.smoothing_alpha,
            max_adjust_ms_per_min: x.max_adjust_ms_per_min,
            health_policy: NtsHealthPolicy {
                min_samples: x.min_samples,
                max_offset_ms: x.max_offset_ms,
                max_confidence_ms: x.max_confidence_ms,
            },
            enforcement_mode: x.enforcement_mode,
        }
    }
}

struct Service {
    outstanding: BTreeMap<(iroha_data_model::peer::PeerId, u64), u64>, // (peer,id) -> t1_ms
    per_peer: BTreeMap<iroha_data_model::peer::PeerId, VecDeque<Sample>>, // ring buffer
    id_counter: u64,
    params: Params,
    // Smoothing state
    smoothed_offset_ms: i64,
    has_smoothed: bool,
    last_smooth_update: Instant,
    // RTT histogram aggregates
    rtt_bounds_ms: &'static [u64],
    rtt_bucket_counts: Vec<u64>,
    rtt_ms_sum: u64,
    rtt_ms_count: u64,
}

static SERVICE: OnceLock<tokio::sync::Mutex<Service>> = OnceLock::new();
static PARAMS_SNAPSHOT: OnceLock<RwLock<ParamsSnapshot>> = OnceLock::new();

#[derive(Clone, Copy, Debug)]
struct ParamsSnapshot {
    enforcement_mode: NtsEnforcementMode,
    health_policy: NtsHealthPolicy,
}

fn params_snapshot_store() -> &'static RwLock<ParamsSnapshot> {
    PARAMS_SNAPSHOT.get_or_init(|| {
        RwLock::new(ParamsSnapshot {
            enforcement_mode: NtsEnforcementMode::Warn,
            health_policy: NtsHealthPolicy::default(),
        })
    })
}

fn params_snapshot() -> ParamsSnapshot {
    params_snapshot_store()
        .read()
        .map_or_else(|err| *err.into_inner(), |guard| *guard)
}

/// Configure the NTS admission policy snapshot used before the service starts.
pub fn configure(params: Params) {
    let snapshot = ParamsSnapshot {
        enforcement_mode: params.enforcement_mode,
        health_policy: params.health_policy,
    };
    if let Ok(mut guard) = params_snapshot_store().write() {
        *guard = snapshot;
    }
}

fn lock_service(mutex: &tokio::sync::Mutex<Service>) -> tokio::sync::MutexGuard<'_, Service> {
    if tokio::runtime::Handle::try_current().is_ok() {
        tokio::task::block_in_place(|| mutex.blocking_lock())
    } else {
        mutex.blocking_lock()
    }
}

fn now_ms() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(0)
}

/// Start the NTS background sampler with explicit parameters. Idempotent.
pub fn start_with_params(
    network: IrohaNetwork,
    _peers_rx: watch::Receiver<BTreeSet<Peer>>,
    params: Params,
) {
    configure(params);
    let guard = SERVICE.get_or_init(|| {
        tokio::sync::Mutex::new(Service {
            outstanding: BTreeMap::new(),
            per_peer: BTreeMap::new(),
            id_counter: 1,
            params,
            smoothed_offset_ms: 0,
            has_smoothed: false,
            last_smooth_update: Instant::now(),
            rtt_bounds_ms: &[1, 2, 4, 8, 16, 32, 64, 128, 256, 512, 1024, 2048],
            rtt_bucket_counts: vec![0; 12],
            rtt_ms_sum: 0,
            rtt_ms_count: 0,
        })
    });
    // Spawn sampler loop once
    tokio::task::spawn(async move {
        let mut ticker = tokio::time::interval(params.sample_interval);
        loop {
            ticker.tick().await;
            // Snapshot peers and send pings
            let peers = network.online_peers(Clone::clone);
            // Limit per-interval probes to avoid flooding
            let max_per_round = params.sample_cap_per_round;
            for peer in peers.iter().take(max_per_round) {
                let pid = peer.id().clone();
                let t1 = now_ms();
                let id = {
                    let mut svc = guard.lock().await;
                    let id = svc.id_counter;
                    svc.id_counter = svc.id_counter.wrapping_add(1).max(1);
                    svc.outstanding.insert((pid.clone(), id), t1);
                    id
                };
                let ping = crate::NetworkMessage::TimePing(Box::new(TimePing { id, t1_ms: t1 }));
                network.post(iroha_p2p::Post {
                    data: ping,
                    peer_id: pid,
                    priority: iroha_p2p::Priority::Low,
                });
            }
        }
    });
}

/// Start the NTS background sampler with default parameters. Idempotent.
pub fn start(network: IrohaNetwork, peers_rx: watch::Receiver<BTreeSet<Peer>>) {
    start_with_params(network, peers_rx, Params::default())
}

/// Handle incoming time messages from the network relay.
pub async fn handle_message(peer: Peer, msg: crate::NetworkMessage, network: &IrohaNetwork) {
    match msg {
        crate::NetworkMessage::TimePing(p) => {
            let t2 = now_ms();
            let pong = TimePong {
                id: p.id,
                t2_ms: t2,
                t3_ms: now_ms(),
            };
            network.post(iroha_p2p::Post {
                data: crate::NetworkMessage::TimePong(Box::new(pong)),
                peer_id: peer.id().clone(),
                priority: iroha_p2p::Priority::Low,
            });
        }
        crate::NetworkMessage::TimePong(p) => {
            let t4 = now_ms();
            let pid = peer.id().clone();
            let mut svc = SERVICE.get().expect("time service").lock().await;
            if let Some(t1) = svc.outstanding.remove(&(pid.clone(), p.id)) {
                let t1_i = i128::from(t1);
                let t2_i = i128::from(p.t2_ms);
                let t3_i = i128::from(p.t3_ms);
                let t4_i = i128::from(t4);
                // NTP-style offset and RTT
                let offset = i128::midpoint(t2_i - t1_i, t3_i - t4_i);
                let rtt = u64::try_from(((t4_i - t1_i) - (t3_i - t2_i)).max(0)).unwrap_or(0);
                let sample = Sample {
                    offset_ms: i64::try_from(offset).unwrap_or(0),
                    rtt_ms: rtt,
                };
                let cap = svc.params.per_peer_buffer;
                let buf = svc
                    .per_peer
                    .entry(pid)
                    .or_insert_with(|| VecDeque::with_capacity(cap));
                if buf.len() == buf.capacity() {
                    let _ = buf.pop_front();
                }
                buf.push_back(sample);
                // Update RTT histogram aggregates
                let mut idx = 0usize;
                while idx < svc.rtt_bounds_ms.len() && rtt > svc.rtt_bounds_ms[idx] {
                    idx += 1;
                }
                if idx >= svc.rtt_bucket_counts.len() {
                    idx = svc.rtt_bucket_counts.len() - 1;
                }
                if let Some(slot) = svc.rtt_bucket_counts.get_mut(idx) {
                    *slot = slot.saturating_add(1);
                }
                svc.rtt_ms_sum = svc.rtt_ms_sum.saturating_add(rtt);
                svc.rtt_ms_count = svc.rtt_ms_count.saturating_add(1);
            }
        }
        _ => {}
    }
}

/// Compute current network time status using trimmed median.
/// Compute current network time status using a trimmed-median aggregator over
/// per-peer NTP-style samples. Falls back to local time if no samples exist.
#[allow(
    clippy::cast_precision_loss,
    clippy::suboptimal_flops,
    clippy::cast_possible_truncation
)]
pub fn now() -> NetworkTimeStatus {
    use std::time::{Duration, SystemTime};
    let svc_opt = SERVICE.get();
    if svc_opt.is_none() {
        let policy = params_snapshot().health_policy;
        let fallback = true;
        return NetworkTimeStatus {
            now: SystemTime::now(),
            offset_ms: 0,
            confidence_ms: 0,
            sample_count: 0,
            peer_count: 0,
            fallback,
            health: policy.evaluate(0, 0, 0, fallback),
        };
    }
    let svc_lock = svc_opt.unwrap();
    let mut svc = lock_service(svc_lock);
    let peer_count = svc.per_peer.len();
    // Collect latest sample per peer with RTT filter
    let mut offsets: Vec<i64> = Vec::new();
    for buf in svc.per_peer.values() {
        if let Some(s) = buf.back() {
            if s.rtt_ms <= svc.params.max_rtt_ms {
                offsets.push(s.offset_ms);
            }
        }
    }
    let sample_count = offsets.len();
    if offsets.is_empty() {
        let fallback = true;
        return NetworkTimeStatus {
            now: SystemTime::now(),
            offset_ms: 0,
            confidence_ms: 0,
            sample_count,
            peer_count,
            fallback,
            health: svc
                .params
                .health_policy
                .evaluate(sample_count, 0, 0, fallback),
        };
    }
    let (median, mad) = trimmed_median_and_mad(&mut offsets, svc.params.trim_percent);
    // Optional smoothing with EMA and slew cap
    let offset = if svc.params.smoothing_enabled {
        if svc.has_smoothed {
            let prev = svc.smoothed_offset_ms as f64;
            let ema_next = svc
                .params
                .smoothing_alpha
                .mul_add(median as f64, (1.0 - svc.params.smoothing_alpha) * prev);
            let elapsed_min = svc.last_smooth_update.elapsed().as_secs_f64() / 60.0;
            let max_delta = (svc.params.max_adjust_ms_per_min as f64) * elapsed_min;
            let delta = ema_next - prev;
            let delta_clamped = delta.clamp(-max_delta, max_delta);
            let next = prev + delta_clamped;
            svc.smoothed_offset_ms = next.round() as i64;
            svc.last_smooth_update = Instant::now();
        } else {
            svc.smoothed_offset_ms = median;
            svc.has_smoothed = true;
            svc.last_smooth_update = Instant::now();
        }
        svc.smoothed_offset_ms
    } else {
        median
    };
    let adjusted_now = if offset >= 0 {
        SystemTime::now() + Duration::from_millis(offset.unsigned_abs())
    } else {
        SystemTime::now() - Duration::from_millis(offset.unsigned_abs())
    };
    let fallback = false;
    NetworkTimeStatus {
        now: adjusted_now,
        offset_ms: offset,
        confidence_ms: mad,
        sample_count,
        peer_count,
        fallback,
        health: svc
            .params
            .health_policy
            .evaluate(sample_count, offset, mad, fallback),
    }
}

/// Compute a trimmed median and MAD (median absolute deviation) from a list of offsets.
/// Mutates the input vector by sorting it.
fn trimmed_median_and_mad(offsets: &mut [i64], trim_percent: u8) -> (i64, u64) {
    if offsets.is_empty() {
        return (0, 0);
    }
    offsets.sort_unstable();
    let n = offsets.len();
    let tp = usize::from(trim_percent.min(45));
    let trim = (n * tp) / 100; // symmetric trim, integer math
    let hi = (n - trim).max(trim + 1);
    let slice = &offsets[trim..hi];
    let median = slice[slice.len() / 2];
    let mut devs: Vec<u64> = slice.iter().map(|&x| (x - median).unsigned_abs()).collect();
    devs.sort_unstable();
    let mad = devs[devs.len() / 2];
    (median, mad)
}

/// Debug snapshot of per-peer samples for diagnostics endpoints.
pub fn debug_snapshot() -> Vec<(String, i64, u64, usize)> {
    let svc_opt = SERVICE.get();
    if svc_opt.is_none() {
        return Vec::new();
    }
    let svc = lock_service(svc_opt.unwrap());
    let mut out = Vec::new();
    for (pid, buf) in &svc.per_peer {
        if let Some(last) = buf.back() {
            out.push((pid.to_string(), last.offset_ms, last.rtt_ms, buf.len()));
        }
    }
    out
}

/// RTT histogram helpers for telemetry (bucket bounds in ms).
pub fn rtt_bucket_bounds_ms() -> &'static [u64] {
    if let Some(lock) = SERVICE.get() {
        let svc = lock_service(lock);
        return svc.rtt_bounds_ms;
    }
    &[]
}

/// RTT histogram counts per bucket.
pub fn rtt_bucket_counts() -> Vec<u64> {
    if let Some(lock) = SERVICE.get() {
        let svc = lock_service(lock);
        return svc.rtt_bucket_counts.clone();
    }
    Vec::new()
}

/// RTT histogram sum of observed RTTs in ms.
pub fn rtt_ms_sum() -> u64 {
    if let Some(lock) = SERVICE.get() {
        let svc = lock_service(lock);
        return svc.rtt_ms_sum;
    }
    0
}

/// RTT histogram count of observations.
pub fn rtt_ms_count() -> u64 {
    if let Some(lock) = SERVICE.get() {
        let svc = lock_service(lock);
        return svc.rtt_ms_count;
    }
    0
}

/// Current NTS enforcement mode for time-sensitive admission.
pub fn enforcement_mode() -> NtsEnforcementMode {
    if let Some(lock) = SERVICE.get() {
        let svc = lock_service(lock);
        return svc.params.enforcement_mode;
    }
    params_snapshot().enforcement_mode
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trimmed_median_and_mad_basics() {
        let mut v = vec![10, 12, 13, 1000, 11, 9, 8, 10, 12, -1000];
        let (median, mad) = trimmed_median_and_mad(&mut v, 10);
        // Extreme outliers should be trimmed; median around 11
        assert!(
            (10..=12).contains(&median),
            "median {median} not in expected range"
        );
        // MAD should be small given tight cluster around ~11
        assert!(mad <= 2, "mad {mad} too large");
    }

    #[test]
    fn now_fallback_without_service() {
        // SERVICE is unset in this test process; ensure fallback path returns without panicking
        let s = now();
        // Confidence is 0 when no samples; offset 0
        assert_eq!(s.offset_ms, 0);
        assert_eq!(s.confidence_ms, 0);
        assert_eq!(s.sample_count, 0);
        assert_eq!(s.peer_count, 0);
        assert!(s.fallback);
        assert!(!s.health.healthy);
    }

    #[test]
    fn enforcement_mode_uses_configured_params_without_service() {
        configure(Params {
            enforcement_mode: NtsEnforcementMode::Reject,
            ..Params::default()
        });
        assert_eq!(enforcement_mode(), NtsEnforcementMode::Reject);
        configure(Params::default());
    }

    #[test]
    fn nts_health_policy_evaluates_thresholds() {
        let policy = NtsHealthPolicy {
            min_samples: 2,
            max_offset_ms: 100,
            max_confidence_ms: 50,
        };
        let ok = policy.evaluate(2, 10, 20, false);
        assert!(ok.min_samples_ok);
        assert!(ok.offset_ok);
        assert!(ok.confidence_ok);
        assert!(ok.healthy);

        let offset_bad = policy.evaluate(2, 250, 20, false);
        assert!(offset_bad.min_samples_ok);
        assert!(!offset_bad.offset_ok);
        assert!(offset_bad.confidence_ok);
        assert!(!offset_bad.healthy);

        let samples_bad = policy.evaluate(1, 10, 20, false);
        assert!(!samples_bad.min_samples_ok);
        assert!(samples_bad.offset_ok);
        assert!(samples_bad.confidence_ok);
        assert!(!samples_bad.healthy);

        let fallback = policy.evaluate(2, 10, 20, true);
        assert!(!fallback.healthy);
    }
}
