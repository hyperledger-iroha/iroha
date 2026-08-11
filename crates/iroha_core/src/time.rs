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

#[derive(Clone, Copy)]
struct Sample {
    offset_ms: i64,
    rtt_ms: u64,
    expires_at: Instant,
}

#[derive(Clone, Copy)]
struct OutstandingProbe {
    t1_ms: u64,
    expires_at: Instant,
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
    outstanding: BTreeMap<(iroha_data_model::peer::PeerId, u64), OutstandingProbe>,
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

impl Service {
    fn new(params: Params) -> Self {
        Self {
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
        }
    }

    fn probe_timeout(&self) -> std::time::Duration {
        std::time::Duration::from_millis(self.params.max_rtt_ms)
    }

    fn sample_freshness_window(&self) -> std::time::Duration {
        let retained_rounds = u32::try_from(self.params.per_peer_buffer.max(1)).unwrap_or(u32::MAX);
        self.params
            .sample_interval
            .saturating_mul(retained_rounds)
            .saturating_add(self.probe_timeout())
    }

    fn deadline_after(now: Instant, duration: std::time::Duration) -> Instant {
        // An unrepresentable deadline must not turn into unbounded state retention.
        now.checked_add(duration).unwrap_or(now)
    }

    fn probe_deadline(&self, sent_at: Instant) -> Instant {
        Self::deadline_after(sent_at, self.probe_timeout())
    }

    fn sample_deadline(&self, received_at: Instant) -> Instant {
        Self::deadline_after(received_at, self.sample_freshness_window())
    }

    fn prune_expired(&mut self, now: Instant) {
        self.outstanding.retain(|_, probe| probe.expires_at > now);
        self.per_peer.retain(|_, samples| {
            samples.retain(|sample| sample.expires_at > now);
            !samples.is_empty()
        });
    }

    fn take_live_probe(
        &mut self,
        peer: &iroha_data_model::peer::PeerId,
        id: u64,
        received_at: Instant,
    ) -> Option<OutstandingProbe> {
        self.prune_expired(received_at);
        self.outstanding.remove(&(peer.clone(), id))
    }

    fn debug_snapshot(&mut self, now: Instant) -> Vec<(String, i64, u64, usize)> {
        self.prune_expired(now);
        self.per_peer
            .iter()
            .filter_map(|(pid, samples)| {
                samples
                    .back()
                    .map(|last| (pid.to_string(), last.offset_ms, last.rtt_ms, samples.len()))
            })
            .collect()
    }

    fn record_sample(&mut self, peer: iroha_data_model::peer::PeerId, sample: Sample) {
        let cap = self.params.per_peer_buffer.max(1);
        let samples = self.per_peer.entry(peer).or_insert_with(VecDeque::new);
        while samples.len() >= cap {
            let _ = samples.pop_front();
        }
        samples.push_back(sample);
    }
}

fn initialize_service_once<'a>(
    cell: &'a OnceLock<tokio::sync::Mutex<Service>>,
    params: Params,
) -> (&'a tokio::sync::Mutex<Service>, bool) {
    let mut initialized_here = false;
    let service = cell.get_or_init(|| {
        initialized_here = true;
        tokio::sync::Mutex::new(Service::new(params))
    });
    (service, initialized_here)
}

fn sampler_interval(period: std::time::Duration) -> tokio::time::Interval {
    let mut ticker = tokio::time::interval(period);
    // Replaying missed rounds creates a burst of probes whose cardinality is
    // unrelated to the configured sampling rate and RTT lifetime.
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    ticker
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

fn clone_bounded<'a, T: Clone + 'a>(values: impl Iterator<Item = &'a T>, limit: usize) -> Vec<T> {
    values.take(limit).cloned().collect()
}

/// Start the NTS background sampler with explicit parameters. Idempotent.
pub fn start_with_params(
    network: IrohaNetwork,
    _peers_rx: watch::Receiver<BTreeSet<Peer>>,
    params: Params,
) {
    configure(params);
    let (guard, initialized_here) = initialize_service_once(&SERVICE, params);
    if !initialized_here {
        return;
    }
    // Spawn sampler loop once
    tokio::task::spawn(async move {
        let mut ticker = sampler_interval(params.sample_interval);
        loop {
            ticker.tick().await;
            // Every probe has a protocol RTT deadline. Expire unanswered work even
            // when there are no online peers, otherwise rotating/disconnected peers
            // retain their identity and request record indefinitely.
            {
                let mut svc = guard.lock().await;
                svc.prune_expired(Instant::now());
            }
            // Limit per-interval probes to avoid flooding
            let max_per_round = params.sample_cap_per_round;
            // Clone only the bounded prefix while the watch snapshot is borrowed.
            // Cloning the complete online set before `take` made transient memory
            // proportional to every connected peer instead of this round's cap.
            let peers = network.online_peers(|online| clone_bounded(online.iter(), max_per_round));
            for peer in &peers {
                let pid = peer.id().clone();
                let t1 = now_ms();
                let id = {
                    let mut svc = guard.lock().await;
                    let sent_at = Instant::now();
                    svc.prune_expired(sent_at);
                    let id = svc.id_counter;
                    svc.id_counter = svc.id_counter.wrapping_add(1).max(1);
                    let expires_at = svc.probe_deadline(sent_at);
                    svc.outstanding.insert(
                        (pid.clone(), id),
                        OutstandingProbe {
                            t1_ms: t1,
                            expires_at,
                        },
                    );
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
            let received_at = Instant::now();
            let pid = peer.id().clone();
            let mut svc = SERVICE.get().expect("time service").lock().await;
            // A late pong cannot be correlated with a live probe. Pruning before
            // removal enforces the same deadline whether or not the sampler ticked.
            if let Some(probe) = svc.take_live_probe(&pid, p.id, received_at) {
                let t1_i = i128::from(probe.t1_ms);
                let t2_i = i128::from(p.t2_ms);
                let t3_i = i128::from(p.t3_ms);
                let t4_i = i128::from(t4);
                // NTP-style offset and RTT
                let offset = i128::midpoint(t2_i - t1_i, t3_i - t4_i);
                let rtt = u64::try_from(((t4_i - t1_i) - (t3_i - t2_i)).max(0)).unwrap_or(0);
                let sample = Sample {
                    offset_ms: i64::try_from(offset).unwrap_or(0),
                    rtt_ms: rtt,
                    expires_at: svc.sample_deadline(received_at),
                };
                svc.record_sample(pid, sample);
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
    svc.prune_expired(Instant::now());
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
    let mut svc = lock_service(svc_opt.unwrap());
    svc.debug_snapshot(Instant::now())
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
    use std::{cell::Cell, rc::Rc};

    use super::*;

    fn service_for_tests(params: Params) -> Service {
        Service::new(params)
    }

    #[test]
    fn peer_snapshot_clones_only_the_round_cap() {
        #[derive(Debug)]
        struct CloneCounter {
            clones: Rc<Cell<usize>>,
        }

        impl Clone for CloneCounter {
            fn clone(&self) -> Self {
                self.clones.set(self.clones.get() + 1);
                Self {
                    clones: Rc::clone(&self.clones),
                }
            }
        }

        let clones = Rc::new(Cell::new(0));
        let peers = (0..128)
            .map(|_| CloneCounter {
                clones: Rc::clone(&clones),
            })
            .collect::<Vec<_>>();

        let snapshot = clone_bounded(peers.iter(), 3);

        assert_eq!(snapshot.len(), 3);
        assert_eq!(clones.get(), 3, "uncapped peer snapshots amplify memory");
    }

    fn test_peer_id() -> iroha_data_model::peer::PeerId {
        let key_pair =
            iroha_crypto::KeyPair::try_random_with_algorithm(iroha_crypto::Algorithm::Ed25519)
                .expect("generate test peer key");
        iroha_data_model::peer::PeerId::new(key_pair.public_key().clone())
    }

    fn insert_sample(
        svc: &mut Service,
        peer: iroha_data_model::peer::PeerId,
        received_at: Instant,
    ) {
        let expires_at = svc.sample_deadline(received_at);
        svc.record_sample(
            peer,
            Sample {
                offset_ms: 0,
                rtt_ms: 1,
                expires_at,
            },
        );
    }

    #[test]
    fn service_initialization_assigns_one_sampler_owner() {
        let cell = OnceLock::new();
        let (first, first_owns_sampler) = initialize_service_once(&cell, Params::default());
        let (second, second_owns_sampler) = initialize_service_once(&cell, Params::default());

        assert!(first_owns_sampler);
        assert!(!second_owns_sampler);
        assert!(std::ptr::eq(first, second));
    }

    #[tokio::test]
    async fn sampler_interval_skips_missed_rounds() {
        let ticker = sampler_interval(std::time::Duration::from_millis(10));
        assert_eq!(
            ticker.missed_tick_behavior(),
            tokio::time::MissedTickBehavior::Skip
        );
        tokio::task::yield_now().await;
    }

    #[test]
    fn sample_ring_allocates_lazily_and_honors_its_limit() {
        let large_params = Params {
            per_peer_buffer: 1_000_000,
            ..Params::default()
        };
        let mut svc = service_for_tests(large_params);
        let peer = test_peer_id();
        let received_at = Instant::now();

        insert_sample(&mut svc, peer.clone(), received_at);
        let first_capacity = svc.per_peer[&peer].capacity();
        assert!(
            first_capacity < large_params.per_peer_buffer,
            "the first sample must not reserve the configured ring limit"
        );

        let params = Params {
            per_peer_buffer: 8,
            ..Params::default()
        };
        let mut svc = service_for_tests(params);
        let peer = test_peer_id();
        for _ in 0..(params.per_peer_buffer * 2) {
            insert_sample(&mut svc, peer.clone(), received_at);
        }
        assert_eq!(svc.per_peer[&peer].len(), params.per_peer_buffer);
    }

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
    fn unanswered_probes_are_bounded_by_the_rtt_deadline() {
        let params = Params {
            sample_interval: std::time::Duration::from_millis(10),
            sample_cap_per_round: 3,
            max_rtt_ms: 25,
            ..Params::default()
        };
        let mut svc = service_for_tests(params);
        let start = Instant::now();
        assert!(svc.probe_timeout() > params.sample_interval);
        let interval_nanos = params.sample_interval.as_nanos();
        let active_rounds =
            usize::try_from((svc.probe_timeout().as_nanos() + interval_nanos - 1) / interval_nanos)
                .expect("test round count fits");
        let maximum_live_probes = params.sample_cap_per_round * active_rounds;
        let mut saw_overlapping_rounds = false;

        for round in 0_u32..128 {
            let now = start
                .checked_add(params.sample_interval.saturating_mul(round))
                .expect("test deadline fits");
            svc.prune_expired(now);
            for probe in 0..params.sample_cap_per_round {
                let peer = test_peer_id();
                svc.outstanding.insert(
                    (
                        peer,
                        u64::from(round) << 32 | u64::try_from(probe).expect("probe fits"),
                    ),
                    OutstandingProbe {
                        t1_ms: 0,
                        expires_at: svc.probe_deadline(now),
                    },
                );
            }
            saw_overlapping_rounds |= svc.outstanding.len() > params.sample_cap_per_round;
            assert!(
                svc.outstanding.len() <= maximum_live_probes,
                "timed-out probes must not accumulate across rounds"
            );
        }
        assert!(
            saw_overlapping_rounds,
            "the test must exercise several simultaneously live probe rounds"
        );

        let after_last_deadline = start
            .checked_add(
                params
                    .sample_interval
                    .saturating_mul(127)
                    .saturating_add(svc.probe_timeout()),
            )
            .expect("test deadline fits");
        svc.prune_expired(after_last_deadline);
        assert!(svc.outstanding.is_empty());

        let peer = test_peer_id();
        svc.outstanding.insert(
            (peer.clone(), 999),
            OutstandingProbe {
                t1_ms: 0,
                expires_at: start,
            },
        );
        assert!(
            svc.take_live_probe(&peer, 999, start).is_none(),
            "a pong received at or after the RTT deadline must be ignored"
        );
    }

    #[test]
    fn rotating_peer_samples_and_diagnostics_remain_bounded_by_ring_horizon() {
        let params = Params {
            sample_interval: std::time::Duration::from_millis(10),
            sample_cap_per_round: 3,
            max_rtt_ms: 5,
            per_peer_buffer: 3,
            ..Params::default()
        };
        let mut svc = service_for_tests(params);
        let start = Instant::now();
        let maximum_live_peers = params.sample_cap_per_round * (params.per_peer_buffer + 1);

        for round in 0_u32..128 {
            let now = start
                .checked_add(params.sample_interval.saturating_mul(round))
                .expect("test deadline fits");
            svc.prune_expired(now);
            for _ in 0..params.sample_cap_per_round {
                insert_sample(&mut svc, test_peer_id(), now);
            }
            let snapshot = svc.debug_snapshot(now);
            assert_eq!(snapshot.len(), svc.per_peer.len());
            assert!(
                snapshot.len() <= maximum_live_peers,
                "diagnostics must only clone samples in the configured retention horizon"
            );
        }

        let recent = start
            .checked_add(params.sample_interval.saturating_mul(128))
            .expect("test deadline fits");
        let peer = test_peer_id();
        insert_sample(&mut svc, peer, recent);
        svc.prune_expired(
            recent
                .checked_add(std::time::Duration::from_millis(params.max_rtt_ms + 1))
                .expect("test deadline fits"),
        );
        assert!(
            !svc.per_peer.is_empty(),
            "a sample remains usable beyond its RTT deadline for the ring horizon"
        );

        let after_horizon = recent
            .checked_add(svc.sample_freshness_window())
            .expect("test deadline fits");
        svc.prune_expired(after_horizon);
        assert!(svc.per_peer.is_empty());
        assert!(svc.debug_snapshot(after_horizon).is_empty());
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
