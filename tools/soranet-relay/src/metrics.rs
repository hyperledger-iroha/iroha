//! Minimal metrics tracking for the relay daemon.

use std::{
    collections::BTreeMap,
    fmt::Write as _,
    sync::{
        Mutex,
        atomic::{AtomicBool, AtomicU8, AtomicU64, Ordering},
    },
    time::Duration,
};

use iroha_crypto::soranet::handshake::HandshakeSuite;
use iroha_data_model::soranet::vpn::VpnSessionReceiptV1;

use crate::{config::RelayMode, scheduler::CellClass};

fn store_f64(atom: &AtomicU64, value: f64) {
    atom.store(value.to_bits(), Ordering::Relaxed);
}

fn load_f64(atom: &AtomicU64) -> f64 {
    f64::from_bits(atom.load(Ordering::Relaxed))
}

/// Key used for per-issuer token verification outcomes.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct TokenOutcomeKey {
    /// Token issuer fingerprint or label.
    pub issuer: String,
    /// Relay identifier involved in the outcome.
    pub relay: String,
    /// Outcome label (e.g., ok, expired, replayed).
    pub outcome: String,
}

/// Runtime status for the VPN tunnel overlay.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VpnRuntimeState {
    Disabled = 0,
    Active = 1,
    Stubbed = 2,
}

impl VpnRuntimeState {
    /// Return a human-readable label for the runtime state.
    pub fn as_label(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::Active => "active",
            Self::Stubbed => "stubbed",
        }
    }

    /// Decode a runtime state from a stored u8 sentinel.
    fn from_u8(value: u8) -> Self {
        match value {
            1 => Self::Active,
            2 => Self::Stubbed,
            _ => Self::Disabled,
        }
    }
}

/// Mutable counters shared across async tasks.
#[derive(Debug)]
pub struct Metrics {
    success: AtomicU64,
    failure: AtomicU64,
    throttled: AtomicU64,
    capacity_reject: AtomicU64,
    pow_difficulty: AtomicU64,
    throttled_remote_quota: AtomicU64,
    throttled_descriptor_quota: AtomicU64,
    throttled_descriptor_replay: AtomicU64,
    throttled_cooldown: AtomicU64,
    throttled_emergency: AtomicU64,
    active_remote_cooldowns: AtomicU64,
    active_descriptor_cooldowns: AtomicU64,
    handshake_mode_counts: [AtomicU64; 3],
    handshake_bytes: AtomicU64,
    puzzle_solve_count: AtomicU64,
    puzzle_solve_micros: AtomicU64,
    padding_cells: AtomicU64,
    padding_bytes: AtomicU64,
    padding_throttled: AtomicU64,
    token_outcomes: Mutex<BTreeMap<TokenOutcomeKey, u64>>,
    descriptor_commit: Mutex<Option<String>>,
    ml_kem_public: Mutex<Option<String>>,
    downgrade_counts: Mutex<BTreeMap<String, u64>>,
    constant_rate_profile: Mutex<Option<String>>,
    constant_rate_neighbors: AtomicU64,
    constant_rate_active_neighbors: AtomicU64,
    constant_rate_queue_depth: AtomicU64,
    constant_rate_queue_control: AtomicU64,
    constant_rate_queue_interactive: AtomicU64,
    constant_rate_queue_bulk: AtomicU64,
    constant_rate_saturation_percent: AtomicU64,
    constant_rate_dummy_lanes: AtomicU64,
    constant_rate_dummy_ratio: AtomicU64,
    constant_rate_slot_rate_hz: AtomicU64,
    constant_rate_ceiling_hits: AtomicU64,
    constant_rate_low_dummy_events: AtomicU64,
    constant_rate_congestion_events: AtomicU64,
    constant_rate_congestion_drops: AtomicU64,
    constant_rate_degraded: AtomicBool,
    vpn_runtime_state: AtomicU8,
    vpn_session_meter_label: Mutex<Option<String>>,
    vpn_byte_meter_label: Mutex<Option<String>>,
    vpn_sessions: AtomicU64,
    vpn_bytes: AtomicU64,
    vpn_ingress_bytes: AtomicU64,
    vpn_egress_bytes: AtomicU64,
    vpn_data_bytes: AtomicU64,
    vpn_data_ingress_bytes: AtomicU64,
    vpn_data_egress_bytes: AtomicU64,
    vpn_cover_bytes: AtomicU64,
    vpn_cover_ingress_bytes: AtomicU64,
    vpn_cover_egress_bytes: AtomicU64,
    vpn_frames: AtomicU64,
    vpn_ingress_frames: AtomicU64,
    vpn_egress_frames: AtomicU64,
    vpn_data_frames: AtomicU64,
    vpn_data_ingress_frames: AtomicU64,
    vpn_data_egress_frames: AtomicU64,
    vpn_cover_frames: AtomicU64,
    vpn_cover_ingress_frames: AtomicU64,
    vpn_cover_egress_frames: AtomicU64,
    vpn_session_receipts: AtomicU64,
    vpn_receipt_ingress_bytes: AtomicU64,
    vpn_receipt_egress_bytes: AtomicU64,
    vpn_receipt_cover_bytes: AtomicU64,
    vpn_receipt_uptime_secs: AtomicU64,
}

impl Default for Metrics {
    fn default() -> Self {
        Self {
            success: AtomicU64::new(0),
            failure: AtomicU64::new(0),
            throttled: AtomicU64::new(0),
            capacity_reject: AtomicU64::new(0),
            pow_difficulty: AtomicU64::new(0),
            throttled_remote_quota: AtomicU64::new(0),
            throttled_descriptor_quota: AtomicU64::new(0),
            throttled_descriptor_replay: AtomicU64::new(0),
            throttled_cooldown: AtomicU64::new(0),
            throttled_emergency: AtomicU64::new(0),
            active_remote_cooldowns: AtomicU64::new(0),
            active_descriptor_cooldowns: AtomicU64::new(0),
            handshake_mode_counts: std::array::from_fn(|_| AtomicU64::new(0)),
            handshake_bytes: AtomicU64::new(0),
            puzzle_solve_count: AtomicU64::new(0),
            puzzle_solve_micros: AtomicU64::new(0),
            padding_cells: AtomicU64::new(0),
            padding_bytes: AtomicU64::new(0),
            padding_throttled: AtomicU64::new(0),
            token_outcomes: Mutex::new(BTreeMap::new()),
            descriptor_commit: Mutex::new(None),
            ml_kem_public: Mutex::new(None),
            downgrade_counts: Mutex::new(BTreeMap::new()),
            constant_rate_profile: Mutex::new(None),
            constant_rate_neighbors: AtomicU64::new(0),
            constant_rate_active_neighbors: AtomicU64::new(0),
            constant_rate_queue_depth: AtomicU64::new(0),
            constant_rate_queue_control: AtomicU64::new(0),
            constant_rate_queue_interactive: AtomicU64::new(0),
            constant_rate_queue_bulk: AtomicU64::new(0),
            constant_rate_saturation_percent: AtomicU64::new(0),
            constant_rate_dummy_lanes: AtomicU64::new(0),
            constant_rate_dummy_ratio: AtomicU64::new(0.0f64.to_bits()),
            constant_rate_slot_rate_hz: AtomicU64::new(0.0f64.to_bits()),
            constant_rate_ceiling_hits: AtomicU64::new(0),
            constant_rate_low_dummy_events: AtomicU64::new(0),
            constant_rate_congestion_events: AtomicU64::new(0),
            constant_rate_congestion_drops: AtomicU64::new(0),
            constant_rate_degraded: AtomicBool::new(false),
            vpn_runtime_state: AtomicU8::new(VpnRuntimeState::Disabled as u8),
            vpn_session_meter_label: Mutex::new(None),
            vpn_byte_meter_label: Mutex::new(None),
            vpn_sessions: AtomicU64::new(0),
            vpn_bytes: AtomicU64::new(0),
            vpn_ingress_bytes: AtomicU64::new(0),
            vpn_egress_bytes: AtomicU64::new(0),
            vpn_data_bytes: AtomicU64::new(0),
            vpn_data_ingress_bytes: AtomicU64::new(0),
            vpn_data_egress_bytes: AtomicU64::new(0),
            vpn_cover_bytes: AtomicU64::new(0),
            vpn_cover_ingress_bytes: AtomicU64::new(0),
            vpn_cover_egress_bytes: AtomicU64::new(0),
            vpn_frames: AtomicU64::new(0),
            vpn_ingress_frames: AtomicU64::new(0),
            vpn_egress_frames: AtomicU64::new(0),
            vpn_data_frames: AtomicU64::new(0),
            vpn_data_ingress_frames: AtomicU64::new(0),
            vpn_data_egress_frames: AtomicU64::new(0),
            vpn_cover_frames: AtomicU64::new(0),
            vpn_cover_ingress_frames: AtomicU64::new(0),
            vpn_cover_egress_frames: AtomicU64::new(0),
            vpn_session_receipts: AtomicU64::new(0),
            vpn_receipt_ingress_bytes: AtomicU64::new(0),
            vpn_receipt_egress_bytes: AtomicU64::new(0),
            vpn_receipt_cover_bytes: AtomicU64::new(0),
            vpn_receipt_uptime_secs: AtomicU64::new(0),
        }
    }
}

impl Metrics {
    /// Construct a metrics registry with zeroed counters.
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record_success(&self) {
        self.success.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_failure(&self) {
        self.failure.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_throttled(&self) {
        self.throttled.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_capacity_reject(&self) {
        self.capacity_reject.fetch_add(1, Ordering::Relaxed);
    }

    pub fn set_pow_difficulty(&self, difficulty: u8) {
        self.pow_difficulty
            .store(difficulty as u64, Ordering::Relaxed);
    }

    pub fn record_remote_quota_throttle(&self) {
        self.throttled_remote_quota.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_descriptor_quota_throttle(&self) {
        self.throttled_descriptor_quota
            .fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_descriptor_replay_throttle(&self) {
        self.throttled_descriptor_replay
            .fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_handshake_cooldown_throttle(&self) {
        self.throttled_cooldown.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_emergency_throttle(&self) {
        self.throttled_emergency.fetch_add(1, Ordering::Relaxed);
    }

    pub fn set_active_remote_cooldowns(&self, count: u64) {
        self.active_remote_cooldowns.store(count, Ordering::Relaxed);
    }

    pub fn set_active_descriptor_cooldowns(&self, count: u64) {
        self.active_descriptor_cooldowns
            .store(count, Ordering::Relaxed);
    }

    pub fn set_descriptor_commit_hex(&self, commit: Option<String>) {
        let mut guard = self
            .descriptor_commit
            .lock()
            .expect("metrics descriptor commit mutex poisoned");
        *guard = commit;
    }

    pub fn set_ml_kem_public_hex(&self, key: Option<String>) {
        let mut guard = self
            .ml_kem_public
            .lock()
            .expect("metrics ml-kem public mutex poisoned");
        *guard = key;
    }

    pub fn set_constant_rate_profile(
        &self,
        profile: &str,
        neighbor_cap: u64,
        tick_millis: f64,
        dummy_floor: u64,
    ) {
        {
            let mut guard = self
                .constant_rate_profile
                .lock()
                .expect("metrics constant-rate profile mutex poisoned");
            *guard = Some(profile.to_string());
        }
        self.constant_rate_neighbors
            .store(neighbor_cap, Ordering::Relaxed);
        let slot_rate_hz = if tick_millis <= f64::EPSILON {
            0.0
        } else {
            1000.0 / tick_millis
        };
        store_f64(&self.constant_rate_slot_rate_hz, slot_rate_hz);
        self.constant_rate_dummy_lanes
            .store(dummy_floor, Ordering::Relaxed);
        let denom = neighbor_cap.max(1) as f64;
        store_f64(
            &self.constant_rate_dummy_ratio,
            (dummy_floor as f64 / denom).min(1.0),
        );
        self.constant_rate_degraded.store(false, Ordering::Relaxed);
    }

    pub fn set_constant_rate_active_neighbors(&self, count: u64) {
        self.constant_rate_active_neighbors
            .store(count, Ordering::Relaxed);
    }

    pub fn set_constant_rate_queue_depth(&self, depth: u64) {
        self.constant_rate_queue_depth
            .store(depth, Ordering::Relaxed);
    }

    pub fn set_constant_rate_queue_depths(&self, control: u64, interactive: u64, bulk: u64) {
        self.constant_rate_queue_control
            .store(control, Ordering::Relaxed);
        self.constant_rate_queue_interactive
            .store(interactive, Ordering::Relaxed);
        self.constant_rate_queue_bulk.store(bulk, Ordering::Relaxed);
    }

    pub fn set_constant_rate_saturation_percent(&self, percent: u64) {
        self.constant_rate_saturation_percent
            .store(percent, Ordering::Relaxed);
    }

    pub fn set_constant_rate_dummy_lanes(&self, lanes: u64) {
        self.constant_rate_dummy_lanes
            .store(lanes, Ordering::Relaxed);
    }

    pub fn set_constant_rate_dummy_ratio(&self, ratio: f64) {
        store_f64(&self.constant_rate_dummy_ratio, ratio.clamp(0.0, 1.0));
    }

    pub fn record_constant_rate_low_dummy(&self) {
        self.constant_rate_low_dummy_events
            .fetch_add(1, Ordering::Relaxed);
    }

    pub fn set_constant_rate_degraded(&self, degraded: bool) {
        self.constant_rate_degraded
            .store(degraded, Ordering::Relaxed);
    }

    pub fn record_constant_rate_ceiling_hit(&self) {
        self.constant_rate_ceiling_hits
            .fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_constant_rate_congestion_event(&self, _buffer_space_bytes: u64) {
        self.constant_rate_congestion_events
            .fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_constant_rate_congestion_drop(&self, _class: CellClass) {
        self.constant_rate_congestion_drops
            .fetch_add(1, Ordering::Relaxed);
    }

    pub fn set_vpn_runtime_state(&self, state: VpnRuntimeState) {
        self.vpn_runtime_state.store(state as u8, Ordering::Relaxed);
    }

    pub fn vpn_runtime_state(&self) -> VpnRuntimeState {
        VpnRuntimeState::from_u8(self.vpn_runtime_state.load(Ordering::Relaxed))
    }

    pub fn record_handshake_mode(&self, suite: HandshakeSuite) {
        let idx = match suite {
            HandshakeSuite::Nk1NoiseXx => 0,
            HandshakeSuite::Nk2Hybrid => 1,
            HandshakeSuite::Nk3PqForwardSecure => 2,
        };
        self.handshake_mode_counts[idx].fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_downgrade(&self, reason: &str) {
        let normalized = normalize_downgrade_reason(reason);
        let mut guard = self
            .downgrade_counts
            .lock()
            .expect("metrics downgrade mutex poisoned");
        *guard.entry(normalized).or_insert(0) += 1;
    }

    pub fn record_handshake_bytes(&self, bytes: u64) {
        self.handshake_bytes.fetch_add(bytes, Ordering::Relaxed);
    }

    pub fn record_puzzle_verify(&self, duration: Duration) {
        let micros = duration.as_micros().min(u128::from(u64::MAX)) as u64;
        self.puzzle_solve_count.fetch_add(1, Ordering::Relaxed);
        self.puzzle_solve_micros
            .fetch_add(micros, Ordering::Relaxed);
    }

    pub fn record_padding_cell_sent(&self, bytes: u64) {
        self.padding_cells.fetch_add(1, Ordering::Relaxed);
        self.padding_bytes.fetch_add(bytes, Ordering::Relaxed);
    }

    pub fn record_padding_cell_throttled(&self) {
        self.padding_throttled.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_token_outcome(&self, issuer_hex: &str, relay_hex: &str, outcome: &str) {
        let mut guard = self
            .token_outcomes
            .lock()
            .expect("token outcomes mutex poisoned");
        let key = TokenOutcomeKey {
            issuer: issuer_hex.trim().to_owned(),
            relay: relay_hex.trim().to_owned(),
            outcome: outcome.trim().to_owned(),
        };
        *guard.entry(key).or_insert(0) += 1;
    }

    pub fn set_vpn_meter_labels(&self, session_label: &str, byte_label: &str) {
        let mut session = self
            .vpn_session_meter_label
            .lock()
            .expect("vpn session meter mutex poisoned");
        let mut bytes = self
            .vpn_byte_meter_label
            .lock()
            .expect("vpn byte meter mutex poisoned");
        *session = Some(session_label.trim().to_owned());
        *bytes = Some(byte_label.trim().to_owned());
    }

    pub fn record_vpn_session(&self) {
        self.vpn_sessions.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_vpn_frame_ingress(&self, is_cover: bool) {
        self.record_vpn_frame_ingress_count(1, is_cover);
    }

    pub fn record_vpn_frame_egress(&self, is_cover: bool) {
        self.record_vpn_frame_egress_count(1, is_cover);
    }

    pub fn record_vpn_frame_ingress_count(&self, frames: u64, is_cover: bool) {
        if frames == 0 {
            return;
        }
        self.vpn_frames.fetch_add(frames, Ordering::Relaxed);
        self.vpn_ingress_frames.fetch_add(frames, Ordering::Relaxed);
        if is_cover {
            self.vpn_cover_frames.fetch_add(frames, Ordering::Relaxed);
            self.vpn_cover_ingress_frames
                .fetch_add(frames, Ordering::Relaxed);
        } else {
            self.vpn_data_frames.fetch_add(frames, Ordering::Relaxed);
            self.vpn_data_ingress_frames
                .fetch_add(frames, Ordering::Relaxed);
        }
    }

    pub fn record_vpn_frame_egress_count(&self, frames: u64, is_cover: bool) {
        if frames == 0 {
            return;
        }
        self.vpn_frames.fetch_add(frames, Ordering::Relaxed);
        self.vpn_egress_frames.fetch_add(frames, Ordering::Relaxed);
        if is_cover {
            self.vpn_cover_frames.fetch_add(frames, Ordering::Relaxed);
            self.vpn_cover_egress_frames
                .fetch_add(frames, Ordering::Relaxed);
        } else {
            self.vpn_data_frames.fetch_add(frames, Ordering::Relaxed);
            self.vpn_data_egress_frames
                .fetch_add(frames, Ordering::Relaxed);
        }
    }

    pub fn record_vpn_bytes(&self, bytes: u64) {
        self.vpn_bytes.fetch_add(bytes, Ordering::Relaxed);
    }

    pub fn record_vpn_ingress(&self, bytes: u64, is_cover: bool) {
        self.vpn_bytes.fetch_add(bytes, Ordering::Relaxed);
        self.vpn_ingress_bytes.fetch_add(bytes, Ordering::Relaxed);
        if is_cover {
            self.vpn_cover_bytes.fetch_add(bytes, Ordering::Relaxed);
            self.vpn_cover_ingress_bytes
                .fetch_add(bytes, Ordering::Relaxed);
        } else {
            self.vpn_data_bytes.fetch_add(bytes, Ordering::Relaxed);
            self.vpn_data_ingress_bytes
                .fetch_add(bytes, Ordering::Relaxed);
        }
    }

    pub fn record_vpn_egress(&self, bytes: u64, is_cover: bool) {
        self.vpn_bytes.fetch_add(bytes, Ordering::Relaxed);
        self.vpn_egress_bytes.fetch_add(bytes, Ordering::Relaxed);
        if is_cover {
            self.vpn_cover_bytes.fetch_add(bytes, Ordering::Relaxed);
            self.vpn_cover_egress_bytes
                .fetch_add(bytes, Ordering::Relaxed);
        } else {
            self.vpn_data_bytes.fetch_add(bytes, Ordering::Relaxed);
            self.vpn_data_egress_bytes
                .fetch_add(bytes, Ordering::Relaxed);
        }
    }

    pub fn record_vpn_receipt(&self, receipt: &VpnSessionReceiptV1) {
        self.vpn_session_receipts.fetch_add(1, Ordering::Relaxed);
        self.vpn_receipt_ingress_bytes
            .fetch_add(receipt.ingress_bytes, Ordering::Relaxed);
        self.vpn_receipt_egress_bytes
            .fetch_add(receipt.egress_bytes, Ordering::Relaxed);
        self.vpn_receipt_cover_bytes
            .fetch_add(receipt.cover_bytes, Ordering::Relaxed);
        self.vpn_receipt_uptime_secs
            .fetch_add(u64::from(receipt.uptime_secs), Ordering::Relaxed);
    }

    pub fn snapshot(&self) -> MetricsSnapshot {
        MetricsSnapshot {
            success: self.success.load(Ordering::Relaxed),
            failure: self.failure.load(Ordering::Relaxed),
            throttled: self.throttled.load(Ordering::Relaxed),
            capacity_reject: self.capacity_reject.load(Ordering::Relaxed),
            pow_difficulty: self.pow_difficulty.load(Ordering::Relaxed),
            throttled_remote_quota: self.throttled_remote_quota.load(Ordering::Relaxed),
            throttled_descriptor_quota: self.throttled_descriptor_quota.load(Ordering::Relaxed),
            throttled_descriptor_replay: self.throttled_descriptor_replay.load(Ordering::Relaxed),
            throttled_cooldown: self.throttled_cooldown.load(Ordering::Relaxed),
            throttled_emergency: self.throttled_emergency.load(Ordering::Relaxed),
            active_remote_cooldowns: self.active_remote_cooldowns.load(Ordering::Relaxed),
            active_descriptor_cooldowns: self.active_descriptor_cooldowns.load(Ordering::Relaxed),
            handshake_mode_counts: [
                self.handshake_mode_counts[0].load(Ordering::Relaxed),
                self.handshake_mode_counts[1].load(Ordering::Relaxed),
                self.handshake_mode_counts[2].load(Ordering::Relaxed),
            ],
            handshake_bytes: self.handshake_bytes.load(Ordering::Relaxed),
            puzzle_solve_count: self.puzzle_solve_count.load(Ordering::Relaxed),
            puzzle_solve_micros: self.puzzle_solve_micros.load(Ordering::Relaxed),
            padding_cells: self.padding_cells.load(Ordering::Relaxed),
            padding_bytes: self.padding_bytes.load(Ordering::Relaxed),
            padding_throttled: self.padding_throttled.load(Ordering::Relaxed),
            token_outcomes: self
                .token_outcomes
                .lock()
                .expect("token outcomes mutex poisoned")
                .clone(),
            downgrade_counts: self
                .downgrade_counts
                .lock()
                .expect("metrics downgrade mutex poisoned")
                .clone(),
            vpn_runtime_state: self.vpn_runtime_state(),
            descriptor_commit: self
                .descriptor_commit
                .lock()
                .expect("metrics descriptor commit mutex poisoned")
                .clone(),
            ml_kem_public: self
                .ml_kem_public
                .lock()
                .expect("metrics ml-kem public mutex poisoned")
                .clone(),
            constant_rate_profile: self
                .constant_rate_profile
                .lock()
                .expect("metrics constant-rate profile mutex poisoned")
                .clone(),
            constant_rate_neighbors: self.constant_rate_neighbors.load(Ordering::Relaxed),
            constant_rate_active_neighbors: self
                .constant_rate_active_neighbors
                .load(Ordering::Relaxed),
            constant_rate_queue_depth: self.constant_rate_queue_depth.load(Ordering::Relaxed),
            constant_rate_queue_control: self.constant_rate_queue_control.load(Ordering::Relaxed),
            constant_rate_queue_interactive: self
                .constant_rate_queue_interactive
                .load(Ordering::Relaxed),
            constant_rate_queue_bulk: self.constant_rate_queue_bulk.load(Ordering::Relaxed),
            constant_rate_saturation_percent: self
                .constant_rate_saturation_percent
                .load(Ordering::Relaxed),
            constant_rate_dummy_lanes: self.constant_rate_dummy_lanes.load(Ordering::Relaxed),
            constant_rate_dummy_ratio: load_f64(&self.constant_rate_dummy_ratio),
            constant_rate_slot_rate_hz: load_f64(&self.constant_rate_slot_rate_hz),
            constant_rate_ceiling_hits: self.constant_rate_ceiling_hits.load(Ordering::Relaxed),
            constant_rate_low_dummy_events: self
                .constant_rate_low_dummy_events
                .load(Ordering::Relaxed),
            constant_rate_congestion_events: self
                .constant_rate_congestion_events
                .load(Ordering::Relaxed),
            constant_rate_congestion_drops: self
                .constant_rate_congestion_drops
                .load(Ordering::Relaxed),
            constant_rate_degraded: self.constant_rate_degraded.load(Ordering::Relaxed),
            vpn_session_meter_label: self
                .vpn_session_meter_label
                .lock()
                .expect("vpn session meter mutex poisoned")
                .clone(),
            vpn_byte_meter_label: self
                .vpn_byte_meter_label
                .lock()
                .expect("vpn byte meter mutex poisoned")
                .clone(),
            vpn_sessions: self.vpn_sessions.load(Ordering::Relaxed),
            vpn_bytes: self.vpn_bytes.load(Ordering::Relaxed),
            vpn_ingress_bytes: self.vpn_ingress_bytes.load(Ordering::Relaxed),
            vpn_egress_bytes: self.vpn_egress_bytes.load(Ordering::Relaxed),
            vpn_data_bytes: self.vpn_data_bytes.load(Ordering::Relaxed),
            vpn_data_ingress_bytes: self.vpn_data_ingress_bytes.load(Ordering::Relaxed),
            vpn_data_egress_bytes: self.vpn_data_egress_bytes.load(Ordering::Relaxed),
            vpn_cover_bytes: self.vpn_cover_bytes.load(Ordering::Relaxed),
            vpn_cover_ingress_bytes: self.vpn_cover_ingress_bytes.load(Ordering::Relaxed),
            vpn_cover_egress_bytes: self.vpn_cover_egress_bytes.load(Ordering::Relaxed),
            vpn_frames: self.vpn_frames.load(Ordering::Relaxed),
            vpn_ingress_frames: self.vpn_ingress_frames.load(Ordering::Relaxed),
            vpn_egress_frames: self.vpn_egress_frames.load(Ordering::Relaxed),
            vpn_data_frames: self.vpn_data_frames.load(Ordering::Relaxed),
            vpn_data_ingress_frames: self.vpn_data_ingress_frames.load(Ordering::Relaxed),
            vpn_data_egress_frames: self.vpn_data_egress_frames.load(Ordering::Relaxed),
            vpn_cover_frames: self.vpn_cover_frames.load(Ordering::Relaxed),
            vpn_cover_ingress_frames: self.vpn_cover_ingress_frames.load(Ordering::Relaxed),
            vpn_cover_egress_frames: self.vpn_cover_egress_frames.load(Ordering::Relaxed),
            vpn_session_receipts: self.vpn_session_receipts.load(Ordering::Relaxed),
            vpn_receipt_ingress_bytes: self.vpn_receipt_ingress_bytes.load(Ordering::Relaxed),
            vpn_receipt_egress_bytes: self.vpn_receipt_egress_bytes.load(Ordering::Relaxed),
            vpn_receipt_cover_bytes: self.vpn_receipt_cover_bytes.load(Ordering::Relaxed),
            vpn_receipt_uptime_secs: self.vpn_receipt_uptime_secs.load(Ordering::Relaxed),
        }
    }

    pub fn render_prometheus(&self, mode: RelayMode, proxy_queue_depth: u64) -> String {
        let snapshot = self.snapshot();
        let profile_label = snapshot
            .constant_rate_profile
            .as_deref()
            .unwrap_or("unknown");
        let neighbor_label = snapshot.constant_rate_neighbors;
        let base_labels = format!(
            "mode=\"{mode}\",constant_rate_profile=\"{profile}\",constant_rate_neighbors=\"{neighbors}\"",
            mode = mode.as_label(),
            profile = profile_label,
            neighbors = neighbor_label,
        );
        let labels = base_labels.as_str();
        let mut output = String::new();

        macro_rules! help_and_type {
            ($help:expr, $typ:expr) => {{
                let _ = writeln!(output, $help);
                let _ = writeln!(output, $typ);
            }};
        }

        macro_rules! metric_line {
            ($fmt:expr, $($arg:tt)*) => {{
                let _ = writeln!(output, $fmt, $($arg)*);
            }};
        }

        help_and_type!(
            "# HELP soranet_handshake_success_total Total successful SoraNet relay handshakes.",
            "# TYPE soranet_handshake_success_total counter"
        );
        metric_line!(
            "soranet_handshake_success_total{{{labels}}} {value}",
            labels = labels,
            value = snapshot.success
        );

        help_and_type!(
            "# HELP soranet_handshake_failure_total Total failed SoraNet relay handshakes.",
            "# TYPE soranet_handshake_failure_total counter"
        );
        metric_line!(
            "soranet_handshake_failure_total{{{labels}}} {value}",
            labels = labels,
            value = snapshot.failure
        );

        help_and_type!(
            "# HELP soranet_handshake_throttled_total Handshake attempts throttled by cooldown.",
            "# TYPE soranet_handshake_throttled_total counter"
        );
        metric_line!(
            "soranet_handshake_throttled_total{{{labels}}} {value}",
            labels = labels,
            value = snapshot.throttled
        );

        help_and_type!(
            "# HELP soranet_handshake_capacity_reject_total Handshake attempts rejected due to circuit limits.",
            "# TYPE soranet_handshake_capacity_reject_total counter"
        );
        metric_line!(
            "soranet_handshake_capacity_reject_total{{{labels}}} {value}",
            labels = labels,
            value = snapshot.capacity_reject
        );

        help_and_type!(
            "# HELP soranet_constant_rate_active_neighbors Active constant-rate circuits permitted by the relay.",
            "# TYPE soranet_constant_rate_active_neighbors gauge"
        );
        metric_line!(
            "soranet_constant_rate_active_neighbors{{{labels}}} {value}",
            labels = labels,
            value = snapshot.constant_rate_active_neighbors
        );
        help_and_type!(
            "# HELP soranet_constant_rate_queue_depth Tracked constant-rate handshake queue depth (active neighbors).",
            "# TYPE soranet_constant_rate_queue_depth gauge"
        );
        metric_line!(
            "soranet_constant_rate_queue_depth{{{labels}}} {value}",
            labels = labels,
            value = snapshot.constant_rate_queue_depth
        );
        help_and_type!(
            "# HELP soranet_constant_rate_queue_depth_class Constant-rate queue depth per traffic class.",
            "# TYPE soranet_constant_rate_queue_depth_class gauge"
        );
        metric_line!(
            "soranet_constant_rate_queue_depth_class{{{labels},class=\"control\"}} {value}",
            labels = labels,
            value = snapshot.constant_rate_queue_control
        );
        metric_line!(
            "soranet_constant_rate_queue_depth_class{{{labels},class=\"interactive\"}} {value}",
            labels = labels,
            value = snapshot.constant_rate_queue_interactive
        );
        metric_line!(
            "soranet_constant_rate_queue_depth_class{{{labels},class=\"bulk\"}} {value}",
            labels = labels,
            value = snapshot.constant_rate_queue_bulk
        );
        help_and_type!(
            "# HELP soranet_constant_rate_saturation_percent Saturation percentage relative to the constant-rate neighbor cap.",
            "# TYPE soranet_constant_rate_saturation_percent gauge"
        );
        metric_line!(
            "soranet_constant_rate_saturation_percent{{{labels}}} {value}",
            labels = labels,
            value = snapshot.constant_rate_saturation_percent
        );
        help_and_type!(
            "# HELP soranet_constant_rate_dummy_lanes Dummy constant-rate lanes maintained by the relay.",
            "# TYPE soranet_constant_rate_dummy_lanes gauge"
        );
        metric_line!(
            "soranet_constant_rate_dummy_lanes{{{labels}}} {value}",
            labels = labels,
            value = snapshot.constant_rate_dummy_lanes
        );
        help_and_type!(
            "# HELP soranet_constant_rate_dummy_ratio Ratio of dummy lanes relative to the neighbor cap.",
            "# TYPE soranet_constant_rate_dummy_ratio gauge"
        );
        metric_line!(
            "soranet_constant_rate_dummy_ratio{{{labels}}} {value}",
            labels = labels,
            value = snapshot.constant_rate_dummy_ratio
        );
        help_and_type!(
            "# HELP soranet_constant_rate_slot_rate_hz Slot rate derived from the constant-rate tick interval.",
            "# TYPE soranet_constant_rate_slot_rate_hz gauge"
        );
        metric_line!(
            "soranet_constant_rate_slot_rate_hz{{{labels}}} {value}",
            labels = labels,
            value = snapshot.constant_rate_slot_rate_hz
        );
        help_and_type!(
            "# HELP soranet_constant_rate_ceiling_hits_total Times the constant-rate neighbor cap auto-disabled due to saturation.",
            "# TYPE soranet_constant_rate_ceiling_hits_total counter"
        );
        metric_line!(
            "soranet_constant_rate_ceiling_hits_total{{{labels}}} {value}",
            labels = labels,
            value = snapshot.constant_rate_ceiling_hits
        );
        help_and_type!(
            "# HELP soranet_constant_rate_low_dummy_events_total Times the constant-rate dummy ratio fell below the configured minimum.",
            "# TYPE soranet_constant_rate_low_dummy_events_total counter"
        );
        metric_line!(
            "soranet_constant_rate_low_dummy_events_total{{{labels}}} {value}",
            labels = labels,
            value = snapshot.constant_rate_low_dummy_events
        );
        help_and_type!(
            "# HELP soranet_constant_rate_degraded Whether the constant-rate lane manager is running in degraded mode.",
            "# TYPE soranet_constant_rate_degraded gauge"
        );
        metric_line!(
            "soranet_constant_rate_degraded{{{labels}}} {value}",
            labels = labels,
            value = u64::from(snapshot.constant_rate_degraded)
        );
        help_and_type!(
            "# HELP soranet_constant_rate_congestion_events_total Times the datagram buffer reported congestion before emitting a constant-rate cell.",
            "# TYPE soranet_constant_rate_congestion_events_total counter"
        );
        metric_line!(
            "soranet_constant_rate_congestion_events_total{{{labels}}} {value}",
            labels = labels,
            value = snapshot.constant_rate_congestion_events
        );
        help_and_type!(
            "# HELP soranet_constant_rate_congestion_drops_total Queued constant-rate cells dropped after congestion signals.",
            "# TYPE soranet_constant_rate_congestion_drops_total counter"
        );
        metric_line!(
            "soranet_constant_rate_congestion_drops_total{{{labels}}} {value}",
            labels = labels,
            value = snapshot.constant_rate_congestion_drops
        );

        let vpn_session_label = snapshot
            .vpn_session_meter_label
            .as_deref()
            .unwrap_or("unknown");
        let vpn_byte_label = snapshot
            .vpn_byte_meter_label
            .as_deref()
            .unwrap_or("unknown");
        let vpn_labels = format!(
            "{labels},vpn_session_meter=\"{vpn_session_label}\",vpn_byte_meter=\"{vpn_byte_label}\"",
        );
        help_and_type!(
            "# HELP soranet_vpn_runtime_status VPN runtime state (1 for the active state label).",
            "# TYPE soranet_vpn_runtime_status gauge"
        );
        for state in [
            VpnRuntimeState::Disabled,
            VpnRuntimeState::Active,
            VpnRuntimeState::Stubbed,
        ] {
            metric_line!(
                "soranet_vpn_runtime_status{{{labels},state=\"{state}\"}} {value}",
                labels = vpn_labels,
                state = state.as_label(),
                value = u64::from(snapshot.vpn_runtime_state == state)
            );
        }
        if snapshot.vpn_session_meter_label.is_some() && snapshot.vpn_byte_meter_label.is_some() {
            help_and_type!(
                "# HELP soranet_vpn_sessions_total VPN sessions observed by the relay.",
                "# TYPE soranet_vpn_sessions_total counter"
            );
            metric_line!(
                "soranet_vpn_sessions_total{{{labels}}} {value}",
                labels = vpn_labels,
                value = snapshot.vpn_sessions
            );

            help_and_type!(
                "# HELP soranet_vpn_bytes_total VPN bytes observed by the relay.",
                "# TYPE soranet_vpn_bytes_total counter"
            );
            metric_line!(
                "soranet_vpn_bytes_total{{{labels}}} {value}",
                labels = vpn_labels,
                value = snapshot.vpn_bytes
            );

            help_and_type!(
                "# HELP soranet_vpn_ingress_bytes_total VPN ingress bytes observed by the relay.",
                "# TYPE soranet_vpn_ingress_bytes_total counter"
            );
            metric_line!(
                "soranet_vpn_ingress_bytes_total{{{labels}}} {value}",
                labels = vpn_labels,
                value = snapshot.vpn_ingress_bytes
            );

            help_and_type!(
                "# HELP soranet_vpn_egress_bytes_total VPN egress bytes observed by the relay.",
                "# TYPE soranet_vpn_egress_bytes_total counter"
            );
            metric_line!(
                "soranet_vpn_egress_bytes_total{{{labels}}} {value}",
                labels = vpn_labels,
                value = snapshot.vpn_egress_bytes
            );

            help_and_type!(
                "# HELP soranet_vpn_data_bytes_total VPN data bytes observed by the relay.",
                "# TYPE soranet_vpn_data_bytes_total counter"
            );
            metric_line!(
                "soranet_vpn_data_bytes_total{{{labels}}} {value}",
                labels = vpn_labels,
                value = snapshot.vpn_data_bytes
            );

            help_and_type!(
                "# HELP soranet_vpn_data_ingress_bytes_total VPN data ingress bytes observed by the relay.",
                "# TYPE soranet_vpn_data_ingress_bytes_total counter"
            );
            metric_line!(
                "soranet_vpn_data_ingress_bytes_total{{{labels}}} {value}",
                labels = vpn_labels,
                value = snapshot.vpn_data_ingress_bytes
            );

            help_and_type!(
                "# HELP soranet_vpn_data_egress_bytes_total VPN data egress bytes observed by the relay.",
                "# TYPE soranet_vpn_data_egress_bytes_total counter"
            );
            metric_line!(
                "soranet_vpn_data_egress_bytes_total{{{labels}}} {value}",
                labels = vpn_labels,
                value = snapshot.vpn_data_egress_bytes
            );

            help_and_type!(
                "# HELP soranet_vpn_cover_bytes_total VPN cover bytes observed by the relay.",
                "# TYPE soranet_vpn_cover_bytes_total counter"
            );
            metric_line!(
                "soranet_vpn_cover_bytes_total{{{labels}}} {value}",
                labels = vpn_labels,
                value = snapshot.vpn_cover_bytes
            );

            help_and_type!(
                "# HELP soranet_vpn_cover_ingress_bytes_total VPN cover ingress bytes observed by the relay.",
                "# TYPE soranet_vpn_cover_ingress_bytes_total counter"
            );
            metric_line!(
                "soranet_vpn_cover_ingress_bytes_total{{{labels}}} {value}",
                labels = vpn_labels,
                value = snapshot.vpn_cover_ingress_bytes
            );

            help_and_type!(
                "# HELP soranet_vpn_cover_egress_bytes_total VPN cover egress bytes observed by the relay.",
                "# TYPE soranet_vpn_cover_egress_bytes_total counter"
            );
            metric_line!(
                "soranet_vpn_cover_egress_bytes_total{{{labels}}} {value}",
                labels = vpn_labels,
                value = snapshot.vpn_cover_egress_bytes
            );

            help_and_type!(
                "# HELP soranet_vpn_frames_total VPN frames observed by the relay.",
                "# TYPE soranet_vpn_frames_total counter"
            );
            metric_line!(
                "soranet_vpn_frames_total{{{labels}}} {value}",
                labels = vpn_labels,
                value = snapshot.vpn_frames
            );

            help_and_type!(
                "# HELP soranet_vpn_ingress_frames_total VPN ingress frames observed by the relay.",
                "# TYPE soranet_vpn_ingress_frames_total counter"
            );
            metric_line!(
                "soranet_vpn_ingress_frames_total{{{labels}}} {value}",
                labels = vpn_labels,
                value = snapshot.vpn_ingress_frames
            );

            help_and_type!(
                "# HELP soranet_vpn_egress_frames_total VPN egress frames observed by the relay.",
                "# TYPE soranet_vpn_egress_frames_total counter"
            );
            metric_line!(
                "soranet_vpn_egress_frames_total{{{labels}}} {value}",
                labels = vpn_labels,
                value = snapshot.vpn_egress_frames
            );

            help_and_type!(
                "# HELP soranet_vpn_data_frames_total VPN data frames observed by the relay.",
                "# TYPE soranet_vpn_data_frames_total counter"
            );
            metric_line!(
                "soranet_vpn_data_frames_total{{{labels}}} {value}",
                labels = vpn_labels,
                value = snapshot.vpn_data_frames
            );

            help_and_type!(
                "# HELP soranet_vpn_data_ingress_frames_total VPN data ingress frames observed by the relay.",
                "# TYPE soranet_vpn_data_ingress_frames_total counter"
            );
            metric_line!(
                "soranet_vpn_data_ingress_frames_total{{{labels}}} {value}",
                labels = vpn_labels,
                value = snapshot.vpn_data_ingress_frames
            );

            help_and_type!(
                "# HELP soranet_vpn_data_egress_frames_total VPN data egress frames observed by the relay.",
                "# TYPE soranet_vpn_data_egress_frames_total counter"
            );
            metric_line!(
                "soranet_vpn_data_egress_frames_total{{{labels}}} {value}",
                labels = vpn_labels,
                value = snapshot.vpn_data_egress_frames
            );

            help_and_type!(
                "# HELP soranet_vpn_cover_frames_total VPN cover frames observed by the relay.",
                "# TYPE soranet_vpn_cover_frames_total counter"
            );
            metric_line!(
                "soranet_vpn_cover_frames_total{{{labels}}} {value}",
                labels = vpn_labels,
                value = snapshot.vpn_cover_frames
            );

            help_and_type!(
                "# HELP soranet_vpn_cover_ingress_frames_total VPN cover ingress frames observed by the relay.",
                "# TYPE soranet_vpn_cover_ingress_frames_total counter"
            );
            metric_line!(
                "soranet_vpn_cover_ingress_frames_total{{{labels}}} {value}",
                labels = vpn_labels,
                value = snapshot.vpn_cover_ingress_frames
            );

            help_and_type!(
                "# HELP soranet_vpn_cover_egress_frames_total VPN cover egress frames observed by the relay.",
                "# TYPE soranet_vpn_cover_egress_frames_total counter"
            );
            metric_line!(
                "soranet_vpn_cover_egress_frames_total{{{labels}}} {value}",
                labels = vpn_labels,
                value = snapshot.vpn_cover_egress_frames
            );

            help_and_type!(
                "# HELP soranet_vpn_session_receipts_total VPN session receipts emitted by the relay.",
                "# TYPE soranet_vpn_session_receipts_total counter"
            );
            metric_line!(
                "soranet_vpn_session_receipts_total{{{labels}}} {value}",
                labels = vpn_labels,
                value = snapshot.vpn_session_receipts
            );

            help_and_type!(
                "# HELP soranet_vpn_receipt_ingress_bytes_total VPN ingress bytes captured in receipts.",
                "# TYPE soranet_vpn_receipt_ingress_bytes_total counter"
            );
            metric_line!(
                "soranet_vpn_receipt_ingress_bytes_total{{{labels}}} {value}",
                labels = vpn_labels,
                value = snapshot.vpn_receipt_ingress_bytes
            );

            help_and_type!(
                "# HELP soranet_vpn_receipt_egress_bytes_total VPN egress bytes captured in receipts.",
                "# TYPE soranet_vpn_receipt_egress_bytes_total counter"
            );
            metric_line!(
                "soranet_vpn_receipt_egress_bytes_total{{{labels}}} {value}",
                labels = vpn_labels,
                value = snapshot.vpn_receipt_egress_bytes
            );

            help_and_type!(
                "# HELP soranet_vpn_receipt_cover_bytes_total VPN cover bytes captured in receipts.",
                "# TYPE soranet_vpn_receipt_cover_bytes_total counter"
            );
            metric_line!(
                "soranet_vpn_receipt_cover_bytes_total{{{labels}}} {value}",
                labels = vpn_labels,
                value = snapshot.vpn_receipt_cover_bytes
            );

            help_and_type!(
                "# HELP soranet_vpn_receipt_uptime_seconds_total VPN session uptime captured in receipts (seconds).",
                "# TYPE soranet_vpn_receipt_uptime_seconds_total counter"
            );
            metric_line!(
                "soranet_vpn_receipt_uptime_seconds_total{{{labels}}} {value}",
                labels = vpn_labels,
                value = snapshot.vpn_receipt_uptime_secs
            );
        }

        help_and_type!(
            "# HELP soranet_padding_cells_sent_total Padding cells emitted by the relay.",
            "# TYPE soranet_padding_cells_sent_total counter"
        );
        metric_line!(
            "soranet_padding_cells_sent_total{{{labels}}} {value}",
            labels = labels,
            value = snapshot.padding_cells
        );

        help_and_type!(
            "# HELP soranet_padding_bytes_sent_total Padding bytes emitted by the relay.",
            "# TYPE soranet_padding_bytes_sent_total counter"
        );
        metric_line!(
            "soranet_padding_bytes_sent_total{{{labels}}} {value}",
            labels = labels,
            value = snapshot.padding_bytes
        );

        help_and_type!(
            "# HELP soranet_padding_cells_throttled_total Padding cells skipped due to the global padding budget.",
            "# TYPE soranet_padding_cells_throttled_total counter"
        );
        metric_line!(
            "soranet_padding_cells_throttled_total{{{labels}}} {value}",
            labels = labels,
            value = snapshot.padding_throttled
        );

        if !snapshot.token_outcomes.is_empty() {
            help_and_type!(
                "# HELP soranet_token_verify_total Admission token verification outcomes.",
                "# TYPE soranet_token_verify_total counter"
            );
            for (key, count) in snapshot.token_outcomes.iter() {
                let outcome_labels = format!(
                    "{labels},issuer=\"{issuer}\",relay=\"{relay}\",outcome=\"{outcome}\"",
                    labels = labels,
                    issuer = key.issuer,
                    relay = key.relay,
                    outcome = key.outcome
                );
                metric_line!(
                    "soranet_token_verify_total{{{labels}}} {value}",
                    labels = outcome_labels,
                    value = count
                );
            }
        }

        help_and_type!(
            "# HELP soranet_handshake_pow_difficulty Current PoW difficulty required by the relay.",
            "# TYPE soranet_handshake_pow_difficulty gauge"
        );
        metric_line!(
            "soranet_handshake_pow_difficulty{{{labels}}} {value}",
            labels = labels,
            value = snapshot.pow_difficulty
        );

        help_and_type!(
            "# HELP soranet_handshake_throttled_remote_quota_total Handshake attempts throttled by per-remote quotas.",
            "# TYPE soranet_handshake_throttled_remote_quota_total counter"
        );
        metric_line!(
            "soranet_handshake_throttled_remote_quota_total{{{labels}}} {value}",
            labels = labels,
            value = snapshot.throttled_remote_quota
        );

        help_and_type!(
            "# HELP soranet_handshake_throttled_descriptor_quota_total Handshake attempts throttled by per-descriptor quotas.",
            "# TYPE soranet_handshake_throttled_descriptor_quota_total counter"
        );
        metric_line!(
            "soranet_handshake_throttled_descriptor_quota_total{{{labels}}} {value}",
            labels = labels,
            value = snapshot.throttled_descriptor_quota
        );

        help_and_type!(
            "# HELP soranet_handshake_throttled_descriptor_replay_total Handshake attempts throttled by descriptor replay filter.",
            "# TYPE soranet_handshake_throttled_descriptor_replay_total counter"
        );
        metric_line!(
            "soranet_handshake_throttled_descriptor_replay_total{{{labels}}} {value}",
            labels = labels,
            value = snapshot.throttled_descriptor_replay
        );

        help_and_type!(
            "# HELP soranet_handshake_throttled_cooldown_total Handshake attempts throttled by the congestion cooldown.",
            "# TYPE soranet_handshake_throttled_cooldown_total counter"
        );
        metric_line!(
            "soranet_handshake_throttled_cooldown_total{{{labels}}} {value}",
            labels = labels,
            value = snapshot.throttled_cooldown
        );

        help_and_type!(
            "# HELP soranet_handshake_throttled_emergency_total Handshake attempts throttled by consensus emergency policy.",
            "# TYPE soranet_handshake_throttled_emergency_total counter"
        );
        metric_line!(
            "soranet_handshake_throttled_emergency_total{{{labels}}} {value}",
            labels = labels,
            value = snapshot.throttled_emergency
        );

        help_and_type!(
            "# HELP soranet_abuse_remote_cooldowns Active remote cooldown entries enforced by the relay.",
            "# TYPE soranet_abuse_remote_cooldowns gauge"
        );
        metric_line!(
            "soranet_abuse_remote_cooldowns{{{labels}}} {value}",
            labels = labels,
            value = snapshot.active_remote_cooldowns
        );

        help_and_type!(
            "# HELP soranet_abuse_descriptor_cooldowns Active descriptor cooldown entries enforced by the relay.",
            "# TYPE soranet_abuse_descriptor_cooldowns gauge"
        );
        metric_line!(
            "soranet_abuse_descriptor_cooldowns{{{labels}}} {value}",
            labels = labels,
            value = snapshot.active_descriptor_cooldowns
        );

        help_and_type!(
            "# HELP soranet_proxy_policy_queue_depth Downgrade events queued for proxy remediation.",
            "# TYPE soranet_proxy_policy_queue_depth gauge"
        );
        metric_line!(
            "soranet_proxy_policy_queue_depth{{{labels}}} {value}",
            labels = labels,
            value = proxy_queue_depth
        );

        const HANDSHAKE_LABELS: [&str; 3] = ["nk1", "nk2", "nk3"];
        help_and_type!(
            "# HELP sn16_handshake_mode_total Count of negotiated SoraNet handshake suites.",
            "# TYPE sn16_handshake_mode_total counter"
        );
        for (idx, &count) in snapshot.handshake_mode_counts.iter().enumerate() {
            let handshake = HANDSHAKE_LABELS[idx];
            metric_line!(
                "sn16_handshake_mode_total{{{labels},handshake=\"{handshake}\"}} {count}",
                labels = labels,
                handshake = handshake,
                count = count
            );
        }

        help_and_type!(
            "# HELP sn16_handshake_bytes_total Total bytes transferred during SoraNet handshakes.",
            "# TYPE sn16_handshake_bytes_total counter"
        );
        metric_line!(
            "sn16_handshake_bytes_total{{{labels}}} {bytes}",
            labels = labels,
            bytes = snapshot.handshake_bytes
        );

        help_and_type!(
            "# HELP sn16_puzzle_verify_seconds Duration of Argon2 ticket verification at the relay.",
            "# TYPE sn16_puzzle_verify_seconds summary"
        );
        let sum_seconds = snapshot.puzzle_solve_micros as f64 / 1_000_000.0;
        metric_line!(
            "sn16_puzzle_verify_seconds_count{{{labels}}} {count}",
            labels = labels,
            count = snapshot.puzzle_solve_count
        );
        metric_line!(
            "sn16_puzzle_verify_seconds_sum{{{labels}}} {sum:.6}",
            labels = labels,
            sum = sum_seconds
        );

        if !snapshot.downgrade_counts.is_empty() {
            help_and_type!(
                "# HELP sn16_handshake_downgrade_total Downgrade detections grouped by reason.",
                "# TYPE sn16_handshake_downgrade_total counter"
            );
            for (reason, count) in snapshot.downgrade_counts.iter() {
                metric_line!(
                    "sn16_handshake_downgrade_total{{{labels},reason=\"{reason}\"}} {count}",
                    labels = labels,
                    reason = reason,
                    count = count
                );
            }
        }

        if let Some(commit) = snapshot.descriptor_commit {
            help_and_type!(
                "# HELP soranet_guard_descriptor_commit Relay descriptor commitment for guard pinning.",
                "# TYPE soranet_guard_descriptor_commit gauge"
            );
            metric_line!(
                "soranet_guard_descriptor_commit{{{labels},commit=\"{commit}\"}} 1",
                labels = labels,
                commit = commit,
            );
        }
        if let Some(kem) = snapshot.ml_kem_public {
            help_and_type!(
                "# HELP soranet_guard_mlkem_public Relay ML-KEM key advertised for guard pinning.",
                "# TYPE soranet_guard_mlkem_public gauge"
            );
            metric_line!(
                "soranet_guard_mlkem_public{{{labels},key=\"{kem}\"}} 1",
                labels = labels,
                kem = kem,
            );
        }

        output
    }
}

pub(crate) fn normalize_downgrade_reason(input: &str) -> String {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return "unknown".to_string();
    }
    let lower = trimmed.to_ascii_lowercase();
    if lower.contains("no overlapping handshake suite") {
        return "suite_no_overlap".to_string();
    }
    if lower.contains("preferred") && lower.contains("negotiated") {
        return "suite_preference_mismatch".to_string();
    }
    if lower.contains("relay omitted suite_list") {
        return "relay_suite_list_missing".to_string();
    }
    if lower.contains("client omitted suite_list") {
        return "client_suite_list_missing".to_string();
    }
    if lower.contains("omitted suite_list") {
        return "suite_list_missing".to_string();
    }
    if lower.contains("ml-kem") && lower.contains("missing") {
        return "mlkem_missing".to_string();
    }
    if lower.contains("admission token") && lower.contains("revoked") {
        return "admission_token_revoked".to_string();
    }
    if lower.contains("puzzle") && lower.contains("expired") {
        return "puzzle_ticket_expired".to_string();
    }
    sanitize_reason(trimmed)
}

fn sanitize_reason(input: &str) -> String {
    let mut slug = String::with_capacity(input.len().min(64));
    let mut last_was_sep = false;
    for ch in input.chars() {
        let mapped = match ch {
            _ if ch.is_ascii_alphanumeric() => ch.to_ascii_lowercase(),
            _ if ch.is_whitespace() || matches!(ch, '-' | '_' | '/' | '.' | ':') => '_',
            _ => continue,
        };
        if mapped == '_' {
            if !slug.is_empty() && !last_was_sep {
                slug.push('_');
            }
            last_was_sep = true;
        } else {
            if slug.len() < 64 {
                slug.push(mapped);
            }
            last_was_sep = false;
        }
        if slug.len() >= 64 {
            break;
        }
    }
    if slug.ends_with('_') {
        slug.pop();
    }
    if slug.is_empty() {
        "unknown".to_string()
    } else {
        slug
    }
}

/// Snapshot of counters for logging or telemetry export.
#[derive(Debug, Clone, PartialEq)]
pub struct MetricsSnapshot {
    /// Successful circuit admissions.
    pub success: u64,
    /// Failed circuit admissions.
    pub failure: u64,
    /// Requests throttled due to policy limits.
    pub throttled: u64,
    /// Requests rejected due to capacity.
    pub capacity_reject: u64,
    /// Current PoW difficulty advertised to clients.
    pub pow_difficulty: u64,
    /// Requests throttled by remote quota.
    pub throttled_remote_quota: u64,
    /// Requests throttled by descriptor quota.
    pub throttled_descriptor_quota: u64,
    /// Requests throttled by descriptor replay filter.
    pub throttled_descriptor_replay: u64,
    /// Requests throttled due to handshake cooldown.
    pub throttled_cooldown: u64,
    /// Requests throttled due to emergency limits.
    pub throttled_emergency: u64,
    /// Active cooldowns keyed by remote.
    pub active_remote_cooldowns: u64,
    /// Active cooldowns keyed by descriptor.
    pub active_descriptor_cooldowns: u64,
    /// Handshake mode counts (entry/middle/exit).
    pub handshake_mode_counts: [u64; 3],
    /// Bytes consumed during handshakes.
    pub handshake_bytes: u64,
    /// Completed puzzle solves.
    pub puzzle_solve_count: u64,
    /// Aggregate time spent solving puzzles in microseconds.
    pub puzzle_solve_micros: u64,
    /// Padding cells sent.
    pub padding_cells: u64,
    /// Padding bytes sent.
    pub padding_bytes: u64,
    /// Padding sends skipped due to throttling.
    pub padding_throttled: u64,
    /// Token verification outcomes by issuer/relay/outcome.
    pub token_outcomes: BTreeMap<TokenOutcomeKey, u64>,
    /// Downgrade reasons tallied for telemetry.
    pub downgrade_counts: BTreeMap<String, u64>,
    /// Optional descriptor commit advertised to clients.
    pub descriptor_commit: Option<String>,
    /// Optional ML-KEM public key advertised to clients.
    pub ml_kem_public: Option<String>,
    /// Current constant-rate profile name.
    pub constant_rate_profile: Option<String>,
    /// Neighbors detected for constant-rate lanes.
    pub constant_rate_neighbors: u64,
    /// Active neighbors contributing to constant-rate lanes.
    pub constant_rate_active_neighbors: u64,
    /// Aggregate constant-rate queue depth.
    pub constant_rate_queue_depth: u64,
    /// Control lane queue depth.
    pub constant_rate_queue_control: u64,
    /// Interactive lane queue depth.
    pub constant_rate_queue_interactive: u64,
    /// Bulk lane queue depth.
    pub constant_rate_queue_bulk: u64,
    /// Saturation percent for constant-rate queues.
    pub constant_rate_saturation_percent: u64,
    /// Dummy lanes currently configured.
    pub constant_rate_dummy_lanes: u64,
    /// Dummy ratio observed across lanes.
    pub constant_rate_dummy_ratio: f64,
    /// Slot rate observed in Hz.
    pub constant_rate_slot_rate_hz: f64,
    /// Times the constant-rate ceiling was hit.
    pub constant_rate_ceiling_hits: u64,
    /// Events where dummy ratio fell below threshold.
    pub constant_rate_low_dummy_events: u64,
    /// Constant-rate congestion events.
    pub constant_rate_congestion_events: u64,
    /// Constant-rate congestion drops.
    pub constant_rate_congestion_drops: u64,
    /// Whether constant-rate is operating in degraded mode.
    pub constant_rate_degraded: bool,
    /// Runtime state of the VPN overlay.
    pub vpn_runtime_state: VpnRuntimeState,
    /// Optional session meter label for VPN billing.
    pub vpn_session_meter_label: Option<String>,
    /// Optional byte meter label for VPN billing.
    pub vpn_byte_meter_label: Option<String>,
    /// VPN sessions recorded.
    pub vpn_sessions: u64,
    /// Aggregate VPN bytes.
    pub vpn_bytes: u64,
    /// Aggregate VPN ingress bytes.
    pub vpn_ingress_bytes: u64,
    /// Aggregate VPN egress bytes.
    pub vpn_egress_bytes: u64,
    /// Aggregate VPN data bytes.
    pub vpn_data_bytes: u64,
    /// Aggregate VPN data ingress bytes.
    pub vpn_data_ingress_bytes: u64,
    /// Aggregate VPN data egress bytes.
    pub vpn_data_egress_bytes: u64,
    /// Aggregate VPN cover bytes.
    pub vpn_cover_bytes: u64,
    /// Aggregate VPN cover ingress bytes.
    pub vpn_cover_ingress_bytes: u64,
    /// Aggregate VPN cover egress bytes.
    pub vpn_cover_egress_bytes: u64,
    /// Total VPN frames observed.
    pub vpn_frames: u64,
    /// Ingress VPN frames observed.
    pub vpn_ingress_frames: u64,
    /// Egress VPN frames observed.
    pub vpn_egress_frames: u64,
    /// Total VPN data frames observed.
    pub vpn_data_frames: u64,
    /// Ingress VPN data frames observed.
    pub vpn_data_ingress_frames: u64,
    /// Egress VPN data frames observed.
    pub vpn_data_egress_frames: u64,
    /// Total VPN cover frames observed.
    pub vpn_cover_frames: u64,
    /// Ingress VPN cover frames observed.
    pub vpn_cover_ingress_frames: u64,
    /// Egress VPN cover frames observed.
    pub vpn_cover_egress_frames: u64,
    /// VPN session receipts emitted.
    pub vpn_session_receipts: u64,
    /// Total ingress bytes accounted in receipts.
    pub vpn_receipt_ingress_bytes: u64,
    /// Total egress bytes accounted in receipts.
    pub vpn_receipt_egress_bytes: u64,
    /// Cover bytes accounted in receipts.
    pub vpn_receipt_cover_bytes: u64,
    /// Session uptime recorded in receipts (seconds).
    pub vpn_receipt_uptime_secs: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_maps_known_patterns() {
        assert_eq!(
            normalize_downgrade_reason("No overlapping handshake suite between client and relay"),
            "suite_no_overlap"
        );
        assert_eq!(
            normalize_downgrade_reason(
                "Client preferred NK3 but negotiated NK1 due to relay requirement"
            ),
            "suite_preference_mismatch"
        );
        assert_eq!(
            normalize_downgrade_reason(
                "Relay omitted suite_list capability despite client marking it required"
            ),
            "relay_suite_list_missing"
        );
        assert_eq!(
            normalize_downgrade_reason(
                "Client omitted suite_list capability despite relay marking it required"
            ),
            "client_suite_list_missing"
        );
        assert_eq!(
            normalize_downgrade_reason("Argon2 puzzle ticket expired before verification"),
            "puzzle_ticket_expired"
        );
        assert_eq!(
            normalize_downgrade_reason("Constant-rate capability missing on hop"),
            "constant_rate_capability_missing_on_hop"
        );
    }

    #[test]
    fn normalize_falls_back_to_sanitized_slug() {
        assert_eq!(
            normalize_downgrade_reason("Unexpected reason: Foo Bar"),
            "unexpected_reason_foo_bar"
        );
    }

    #[test]
    fn render_prometheus_output() {
        let metrics = Metrics::new();
        metrics.record_success();
        metrics.record_success();
        metrics.record_failure();
        metrics.record_throttled();
        metrics.record_capacity_reject();
        metrics.set_descriptor_commit_hex(Some("deadbeef".to_string()));
        metrics.set_ml_kem_public_hex(Some("cafebabe".to_string()));
        metrics.set_pow_difficulty(12);
        metrics.record_remote_quota_throttle();
        metrics.record_descriptor_quota_throttle();
        metrics.record_descriptor_replay_throttle();
        metrics.record_handshake_cooldown_throttle();
        metrics.record_emergency_throttle();
        metrics.set_active_remote_cooldowns(3);
        metrics.set_active_descriptor_cooldowns(2);
        metrics.record_padding_cell_sent(512);
        metrics.record_padding_cell_sent(256);
        metrics.record_padding_cell_throttled();
        metrics.set_constant_rate_profile("core", 8, 5.0, 4);
        metrics.set_constant_rate_active_neighbors(2);
        metrics.set_constant_rate_queue_depth(2);
        metrics.set_constant_rate_queue_depths(1, 2, 3);
        metrics.set_constant_rate_saturation_percent(50);
        metrics.set_constant_rate_dummy_lanes(3);
        metrics.set_constant_rate_dummy_ratio(0.5);
        metrics.record_constant_rate_ceiling_hit();
        metrics.record_constant_rate_low_dummy();
        metrics.set_constant_rate_degraded(true);
        metrics.record_constant_rate_congestion_event(0);
        metrics.record_constant_rate_congestion_drop(CellClass::Bulk);
        metrics.set_vpn_runtime_state(VpnRuntimeState::Stubbed);
        metrics.set_vpn_meter_labels("vpn.session", "vpn.egress.bytes");
        metrics.record_vpn_session();
        metrics.record_vpn_bytes(1_500);
        metrics.record_vpn_receipt(&iroha_data_model::soranet::vpn::VpnSessionReceiptV1 {
            session_id: [0xAA; 16],
            ingress_bytes: 10,
            egress_bytes: 20,
            cover_bytes: 5,
            uptime_secs: 3,
            exit_class: iroha_data_model::soranet::vpn::VpnExitClassV1::Standard,
            meter_hash: [0x11; 32],
        });
        metrics.record_token_outcome("deadbeef", "cafebabe", "accepted");

        let rendered = metrics.render_prometheus(RelayMode::Middle, 2);
        let label_block =
            "mode=\"middle\",constant_rate_profile=\"core\",constant_rate_neighbors=\"8\"";
        assert!(rendered.contains(&format!(
            "soranet_handshake_success_total{{{label_block}}} 2"
        )));
        assert!(rendered.contains(&format!(
            "soranet_handshake_failure_total{{{label_block}}} 1"
        )));
        assert!(rendered.contains(&format!(
            "soranet_handshake_throttled_total{{{label_block}}} 1"
        )));
        assert!(rendered.contains(&format!(
            "soranet_handshake_capacity_reject_total{{{label_block}}} 1"
        )));
        assert!(rendered.contains(&format!(
            "soranet_constant_rate_active_neighbors{{{label_block}}} 2"
        )));
        assert!(rendered.contains(&format!(
            "soranet_constant_rate_queue_depth{{{label_block}}} 2"
        )));
        assert!(rendered.contains(&format!(
            "soranet_constant_rate_queue_depth_class{{{label_block},class=\"control\"}} 1"
        )));
        assert!(rendered.contains(&format!(
            "soranet_constant_rate_queue_depth_class{{{label_block},class=\"interactive\"}} 2"
        )));
        assert!(rendered.contains(&format!(
            "soranet_constant_rate_queue_depth_class{{{label_block},class=\"bulk\"}} 3"
        )));
        assert!(rendered.contains(&format!(
            "soranet_constant_rate_saturation_percent{{{label_block}}} 50"
        )));
        assert!(rendered.contains(&format!(
            "soranet_constant_rate_dummy_lanes{{{label_block}}} 3"
        )));
        assert!(rendered.contains(&format!(
            "soranet_constant_rate_dummy_ratio{{{label_block}}} 0.5"
        )));
        assert!(rendered.contains(&format!(
            "soranet_constant_rate_slot_rate_hz{{{label_block}}} 200"
        )));
        assert!(rendered.contains(&format!(
            "soranet_constant_rate_ceiling_hits_total{{{label_block}}} 1"
        )));
        assert!(rendered.contains(&format!(
            "soranet_constant_rate_low_dummy_events_total{{{label_block}}} 1"
        )));
        assert!(rendered.contains(&format!(
            "soranet_constant_rate_degraded{{{label_block}}} 1"
        )));
        assert!(rendered.contains(&format!(
            "soranet_constant_rate_congestion_events_total{{{label_block}}} 1"
        )));
        assert!(rendered.contains(&format!(
            "soranet_constant_rate_congestion_drops_total{{{label_block}}} 1"
        )));
        assert!(rendered.contains(&format!(
            "soranet_handshake_pow_difficulty{{{label_block}}} 12"
        )));
        assert!(rendered.contains(&format!(
            "soranet_handshake_throttled_remote_quota_total{{{label_block}}} 1"
        )));
        assert!(rendered.contains(&format!(
            "soranet_handshake_throttled_descriptor_quota_total{{{label_block}}} 1"
        )));
        assert!(rendered.contains(&format!(
            "soranet_handshake_throttled_descriptor_replay_total{{{label_block}}} 1"
        )));
        assert!(rendered.contains(&format!(
            "soranet_handshake_throttled_cooldown_total{{{label_block}}} 1"
        )));
        assert!(rendered.contains(&format!(
            "soranet_handshake_throttled_emergency_total{{{label_block}}} 1"
        )));
        assert!(rendered.contains(&format!(
            "soranet_padding_cells_sent_total{{{label_block}}} 2"
        )));
        assert!(rendered.contains(&format!(
            "soranet_padding_bytes_sent_total{{{label_block}}} 768"
        )));
        assert!(rendered.contains(&format!(
            "soranet_padding_cells_throttled_total{{{label_block}}} 1"
        )));
        assert!(rendered.contains(&format!(
            "soranet_abuse_remote_cooldowns{{{label_block}}} 3"
        )));
        assert!(rendered.contains(&format!(
            "soranet_abuse_descriptor_cooldowns{{{label_block}}} 2"
        )));
        assert!(rendered.contains(&format!(
            "soranet_proxy_policy_queue_depth{{{label_block}}} 2"
        )));
        assert!(rendered.contains(&format!(
            "soranet_guard_descriptor_commit{{{label_block},commit=\"deadbeef\"}} 1"
        )));
        assert!(rendered.contains(&format!(
            "soranet_guard_mlkem_public{{{label_block},key=\"cafebabe\"}} 1"
        )));
        assert!(rendered.contains("soranet_vpn_sessions_total"));
        assert!(rendered.contains("vpn_session_meter=\"vpn.session\""));
        assert!(rendered.contains("soranet_vpn_bytes_total"));
        assert!(rendered.contains("soranet_vpn_data_bytes_total"));
        assert!(rendered.contains("soranet_vpn_cover_bytes_total"));
        assert!(rendered.contains("soranet_vpn_frames_total"));
        assert!(rendered.contains("soranet_vpn_ingress_frames_total"));
        assert!(rendered.contains("soranet_vpn_egress_frames_total"));
        assert!(rendered.contains("soranet_vpn_data_frames_total"));
        assert!(rendered.contains("soranet_vpn_cover_frames_total"));
        assert!(rendered.contains("soranet_vpn_session_receipts_total"));
        assert!(rendered.contains("soranet_vpn_receipt_ingress_bytes_total"));
        assert!(rendered.contains("soranet_vpn_receipt_uptime_seconds_total"));
        let vpn_label_block = format!(
            "{label_block},vpn_session_meter=\"vpn.session\",vpn_byte_meter=\"vpn.egress.bytes\""
        );
        assert!(rendered.contains(&format!(
            "soranet_vpn_runtime_status{{{labels},state=\"stubbed\"}} 1",
            labels = vpn_label_block
        )));
        assert!(rendered.contains(&format!(
            "soranet_vpn_runtime_status{{{labels},state=\"disabled\"}} 0",
            labels = vpn_label_block
        )));
        assert!(rendered.contains(&format!(
            "soranet_vpn_session_receipts_total{{{labels}}} 1",
            labels = vpn_label_block
        )));
        assert!(rendered.contains(&format!(
            "soranet_vpn_receipt_ingress_bytes_total{{{labels}}} 10",
            labels = vpn_label_block
        )));
        assert!(rendered.contains(&format!(
            "soranet_vpn_receipt_egress_bytes_total{{{labels}}} 20",
            labels = vpn_label_block
        )));
        assert!(rendered.contains(&format!(
            "soranet_vpn_bytes_total{{{labels}}} 2070",
            labels = vpn_label_block
        )));
        assert!(rendered.contains(&format!(
            "soranet_vpn_ingress_bytes_total{{{labels}}} 350",
            labels = vpn_label_block
        )));
        assert!(rendered.contains(&format!(
            "soranet_vpn_egress_bytes_total{{{labels}}} 220",
            labels = vpn_label_block
        )));
        assert!(rendered.contains(&format!(
            "soranet_vpn_data_bytes_total{{{labels}}} 500",
            labels = vpn_label_block
        )));
        assert!(rendered.contains(&format!(
            "soranet_vpn_cover_bytes_total{{{labels}}} 70",
            labels = vpn_label_block
        )));
        assert!(rendered.contains(&format!(
            "soranet_vpn_frames_total{{{labels}}} 5",
            labels = vpn_label_block
        )));
        assert!(rendered.contains(&format!(
            "soranet_vpn_ingress_frames_total{{{labels}}} 2",
            labels = vpn_label_block
        )));
        assert!(rendered.contains(&format!(
            "soranet_vpn_egress_frames_total{{{labels}}} 3",
            labels = vpn_label_block
        )));
        assert!(rendered.contains(&format!(
            "soranet_vpn_data_frames_total{{{labels}}} 2",
            labels = vpn_label_block
        )));
        assert!(rendered.contains(&format!(
            "soranet_vpn_cover_frames_total{{{labels}}} 3",
            labels = vpn_label_block
        )));
        assert!(rendered.contains(&format!(
            "soranet_token_verify_total{{{label_block},issuer=\"deadbeef\",relay=\"cafebabe\",outcome=\"accepted\"}} 1"
        )));
    }

    #[test]
    fn token_outcome_metrics_accumulate() {
        let metrics = Metrics::new();
        metrics.record_token_outcome("issuer_hex", "relay_hex", "accepted");
        metrics.record_token_outcome("issuer_hex", "relay_hex", "accepted");
        metrics.record_token_outcome("issuer_hex", "relay_hex", "replay");

        let snapshot = metrics.snapshot();
        let accepted_key = TokenOutcomeKey {
            issuer: "issuer_hex".to_string(),
            relay: "relay_hex".to_string(),
            outcome: "accepted".to_string(),
        };
        let replay_key = TokenOutcomeKey {
            issuer: "issuer_hex".to_string(),
            relay: "relay_hex".to_string(),
            outcome: "replay".to_string(),
        };
        assert_eq!(snapshot.token_outcomes.get(&accepted_key), Some(&2));
        assert_eq!(snapshot.token_outcomes.get(&replay_key), Some(&1));
    }

    #[test]
    fn sanitize_reason_normalizes_labels() {
        assert_eq!(sanitize_reason("Hello World!"), "hello_world");
        assert_eq!(sanitize_reason("   "), "unknown");
        assert_eq!(
            sanitize_reason("Downgrade: TLS/Fallback-Flow"),
            "downgrade_tls_fallback_flow"
        );
        assert_eq!(
            sanitize_reason("Repeated___Separators---Are::Merged"),
            "repeated_separators_are_merged"
        );
        assert_eq!(
            sanitize_reason(&"A".repeat(128)),
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
        );
    }
}
