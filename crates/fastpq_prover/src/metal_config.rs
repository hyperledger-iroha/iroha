//! Metal runtime configuration knobs shared by the FFT kernels and Poseidon GPU path.
#![cfg_attr(
    not(all(feature = "fastpq-gpu", target_os = "macos")),
    allow(dead_code)
)]

use std::{env, sync::OnceLock};

use tracing::{debug, warn};

const FFT_LANES_ENV: &str = "FASTPQ_METAL_FFT_LANES";
const FFT_TILE_ENV: &str = "FASTPQ_METAL_FFT_TILE_STAGES";
const DEFAULT_THREADGROUP_LANES: u32 = 32;
const MIN_THREADGROUP_LANES: u32 = 8;
const MAX_THREADGROUP_LANES: u32 = 256;
const DEFAULT_TILE_STAGE_LIMIT: u32 = 5;
const MIN_TILE_STAGE_LIMIT: u32 = 1;
const MAX_TILE_STAGE_LIMIT: u32 = 16;
const POSEIDON_LANES_ENV: &str = "FASTPQ_METAL_POSEIDON_LANES";
const POSEIDON_BATCH_ENV: &str = "FASTPQ_METAL_POSEIDON_BATCH";
const DEFAULT_POSEIDON_LANES: u32 = 256;
const MIN_POSEIDON_LANES: u32 = 32;
const MAX_POSEIDON_LANES: u32 = 256;
const DEFAULT_POSEIDON_BATCH: u32 = 8;
const MIN_POSEIDON_BATCH: u32 = 1;
const MAX_POSEIDON_BATCH: u32 = 32;
const GIB_BYTES: u64 = 1024 * 1024 * 1024;
const GIB_F64: f64 = 1024.0 * 1024.0 * 1024.0;
#[allow(clippy::doc_markdown)]
/// Maximum LDE tile depth allowed for Metal kernels.
pub const LDE_TILE_STAGE_LIMIT_MAX: u32 = 32;
#[allow(clippy::doc_markdown)]
/// Minimum LDE tile depth allowed for Metal kernels.
pub const LDE_TILE_STAGE_LIMIT_MIN: u32 = 1;

static FFT_LANE_OVERRIDE: OnceLock<Option<u32>> = OnceLock::new();
static FFT_TILE_OVERRIDE: OnceLock<Option<u32>> = OnceLock::new();
static POSEIDON_LANE_OVERRIDE: OnceLock<Option<u32>> = OnceLock::new();
static POSEIDON_BATCH_OVERRIDE: OnceLock<Option<u32>> = OnceLock::new();
static DEVICE_HINTS: OnceLock<DeviceHints> = OnceLock::new();
#[cfg(test)]
static TEST_DEVICE_HINTS: OnceLock<std::sync::Mutex<Option<DeviceHints>>> = OnceLock::new();

/// Tunable FFT parameters consumed by the Metal kernels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FftTuning {
    /// Active threadgroup lanes per FFT column after heuristics and overrides.
    pub threadgroup_lanes: u32,
    /// Number of FFT stages executed inside the shared-memory tile.
    pub tile_stage_limit: u32,
}

/// Tunable Poseidon parameters consumed by the Metal kernel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PoseidonTuning {
    /// Threads per threadgroup dedicated to Poseidon states.
    pub threadgroup_lanes: u32,
    /// Sequential states executed by each thread before yielding.
    pub states_per_lane: u32,
}

/// Hardware hints surfaced from `MTLDevice` so heuristics can choose better defaults.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DeviceHints {
    pub is_low_power: bool,
    pub is_headless: bool,
    pub has_discrete_link: bool,
    pub recommended_max_working_set: u64,
}

impl DeviceHints {
    #[must_use]
    pub const fn new(
        is_low_power: bool,
        is_headless: bool,
        has_discrete_link: bool,
        recommended_max_working_set: u64,
    ) -> Self {
        Self {
            is_low_power,
            is_headless,
            has_discrete_link,
            recommended_max_working_set,
        }
    }

    #[must_use]
    pub const fn is_discrete(&self) -> bool {
        self.has_discrete_link || self.is_headless || !self.is_low_power
    }
}

impl Default for DeviceHints {
    fn default() -> Self {
        Self {
            is_low_power: true,
            is_headless: false,
            has_discrete_link: false,
            recommended_max_working_set: 0,
        }
    }
}

/// Registers a best-effort snapshot of the underlying Metal device's capabilities.
pub fn register_device_hints(hints: DeviceHints) {
    let _ = DEVICE_HINTS.set(hints);
}

/// Returns the stored Metal device hints (or defaults when unset).
#[must_use]
pub fn device_hint_snapshot() -> DeviceHints {
    device_hints()
}

/// Combine heuristics, hardware limits, and env overrides to build a tuning profile.
pub fn fft_tuning(log_len: u32, exec_width: u32, max_threads: u32) -> FftTuning {
    let lane_target = fft_lane_override().unwrap_or_else(|| default_lane_target(log_len));
    let exec_requirement = ceil_power_of_two(exec_width.clamp(1, MAX_THREADGROUP_LANES));
    let hardware_ceiling =
        floor_power_of_two(max_threads.clamp(1, MAX_THREADGROUP_LANES)).max(MIN_THREADGROUP_LANES);

    let mut lanes = lane_target.clamp(MIN_THREADGROUP_LANES, MAX_THREADGROUP_LANES);
    lanes = lanes.max(exec_requirement).min(hardware_ceiling);

    let tile_stage_limit = if log_len == 0 {
        0
    } else {
        fft_tile_override()
            .unwrap_or_else(|| default_tile_stage_target(log_len))
            .clamp(MIN_TILE_STAGE_LIMIT, MAX_TILE_STAGE_LIMIT)
            .min(log_len)
    };

    FftTuning {
        threadgroup_lanes: lanes,
        tile_stage_limit,
    }
}

fn fft_lane_override() -> Option<u32> {
    *FFT_LANE_OVERRIDE.get_or_init(|| {
        env::var(FFT_LANES_ENV)
            .ok()
            .and_then(|raw| match parse_fft_lane_override(raw.trim()) {
                Ok(value) => {
                    debug!(
                        target: "fastpq::metal",
                        lanes = value,
                        "overriding Metal FFT threadgroup lanes via {FFT_LANES_ENV}"
                    );
                    Some(value)
                }
                Err(error) => {
                    warn!(
                        target: "fastpq::metal",
                        raw,
                        %error,
                        default_lanes = DEFAULT_THREADGROUP_LANES,
                        "ignoring invalid {FFT_LANES_ENV} override; keeping default lanes"
                    );
                    None
                }
            })
    })
}

fn fft_tile_override() -> Option<u32> {
    *FFT_TILE_OVERRIDE.get_or_init(|| {
        env::var(FFT_TILE_ENV)
            .ok()
            .and_then(|raw| match parse_fft_tile_override(raw.trim()) {
                Ok(value) => {
                    debug!(
                        target: "fastpq::metal",
                        stages = value,
                        "overriding Metal FFT tile stages via {FFT_TILE_ENV}"
                    );
                    Some(value)
                }
                Err(error) => {
                    warn!(
                        target: "fastpq::metal",
                        raw,
                        %error,
                        default_stages = DEFAULT_TILE_STAGE_LIMIT,
                        "ignoring invalid {FFT_TILE_ENV} override; keeping default tile depth"
                    );
                    None
                }
            })
    })
}

/// Combine heuristics, hardware limits, and env overrides for Poseidon dispatches.
pub fn poseidon_tuning(exec_width: u32, max_threads: u32) -> PoseidonTuning {
    let hints = device_hints();
    let exec_requirement = ceil_power_of_two(
        exec_width
            .clamp(1, MAX_POSEIDON_LANES)
            .max(MIN_POSEIDON_LANES),
    );
    let hardware_ceiling = floor_power_of_two(max_threads).max(MIN_POSEIDON_LANES);
    let lane_target =
        poseidon_lane_override().unwrap_or_else(|| default_poseidon_lane_target(hardware_ceiling));
    let lanes = lane_target
        .clamp(MIN_POSEIDON_LANES, MAX_POSEIDON_LANES)
        .max(exec_requirement)
        .min(hardware_ceiling);

    let default_states = default_poseidon_states_per_lane(hints, lanes);
    let states_per_lane = poseidon_batch_override()
        .unwrap_or(default_states)
        .clamp(MIN_POSEIDON_BATCH, MAX_POSEIDON_BATCH);

    PoseidonTuning {
        threadgroup_lanes: lanes,
        states_per_lane,
    }
}

fn poseidon_lane_override() -> Option<u32> {
    *POSEIDON_LANE_OVERRIDE.get_or_init(|| {
        env::var(POSEIDON_LANES_ENV).ok().and_then(|raw| {
            match parse_poseidon_lane_override(raw.trim()) {
                Ok(value) => {
                    debug!(
                        target: "fastpq::metal",
                        lanes = value,
                        "overriding Metal Poseidon lane count via {POSEIDON_LANES_ENV}"
                    );
                    Some(value)
                }
                Err(error) => {
                    warn!(
                        target: "fastpq::metal",
                        raw,
                        %error,
                        default_lanes = DEFAULT_POSEIDON_LANES,
                        "ignoring invalid {POSEIDON_LANES_ENV} override; keeping default lanes"
                    );
                    None
                }
            }
        })
    })
}

fn poseidon_batch_override() -> Option<u32> {
    *POSEIDON_BATCH_OVERRIDE.get_or_init(|| {
        env::var(POSEIDON_BATCH_ENV).ok().and_then(|raw| {
            match parse_poseidon_batch_override(raw.trim()) {
                Ok(value) => {
                    debug!(
                        target: "fastpq::metal",
                        states = value,
                        "overriding Metal Poseidon batch size via {POSEIDON_BATCH_ENV}"
                    );
                    Some(value)
                }
                Err(error) => {
                    warn!(
                        target: "fastpq::metal",
                        raw,
                        %error,
                        default_batch = DEFAULT_POSEIDON_BATCH,
                        "ignoring invalid {POSEIDON_BATCH_ENV} override; keeping default batch"
                    );
                    None
                }
            }
        })
    })
}

fn default_lane_target(log_len: u32) -> u32 {
    match log_len {
        n if n >= 18 => 256,
        n if n >= 14 => 128,
        n if n >= 10 => 64,
        n if n >= 6 => 32,
        _ => 16,
    }
}

fn default_tile_stage_target(log_len: u32) -> u32 {
    match log_len {
        0 => 0,
        n if n >= 22 => 16,
        n if n >= 20 => 14,
        n if n >= 18 => 12,
        n if n >= 12 => 4,
        _ => 5,
    }
}

fn parse_fft_lane_override(raw: &str) -> Result<u32, &'static str> {
    let value: u32 = raw.parse().map_err(|_| "not an integer")?;
    if !(MIN_THREADGROUP_LANES..=MAX_THREADGROUP_LANES).contains(&value) {
        return Err("lane count out of supported range (8–256)");
    }
    if !value.is_power_of_two() {
        return Err("lane count must be a power of two");
    }
    Ok(value)
}

fn parse_fft_tile_override(raw: &str) -> Result<u32, &'static str> {
    let value: u32 = raw.parse().map_err(|_| "not an integer")?;
    if !(MIN_TILE_STAGE_LIMIT..=MAX_TILE_STAGE_LIMIT).contains(&value) {
        return Err("tile depth out of supported range (1–16)");
    }
    Ok(value)
}

fn parse_poseidon_lane_override(raw: &str) -> Result<u32, &'static str> {
    let value: u32 = raw.parse().map_err(|_| "not an integer")?;
    if !(MIN_POSEIDON_LANES..=MAX_POSEIDON_LANES).contains(&value) {
        return Err("lane count out of supported range (32–256)");
    }
    if !value.is_power_of_two() {
        return Err("lane count must be a power of two");
    }
    Ok(value)
}

fn parse_poseidon_batch_override(raw: &str) -> Result<u32, &'static str> {
    let value: u32 = raw.parse().map_err(|_| "not an integer")?;
    if !(MIN_POSEIDON_BATCH..=MAX_POSEIDON_BATCH).contains(&value) {
        return Err("batch size out of supported range (1–32 states)");
    }
    Ok(value)
}

fn device_hints() -> DeviceHints {
    #[cfg(test)]
    if let Some(hints) = TEST_DEVICE_HINTS
        .get()
        .and_then(|store| store.lock().ok())
        .and_then(|guard| *guard)
    {
        return hints;
    }
    DEVICE_HINTS.get().copied().unwrap_or_default()
}

fn default_poseidon_lane_target(hardware_ceiling: u32) -> u32 {
    if hardware_ceiling >= 256 {
        256
    } else if hardware_ceiling >= 128 {
        128
    } else if hardware_ceiling >= 64 {
        64
    } else {
        32
    }
}

fn default_poseidon_states_per_lane(hints: DeviceHints, lanes: u32) -> u32 {
    let discrete = hints.is_discrete();
    // Force tiny per-lane batches so Poseidon dispatches cannot run for tens of seconds on large traces.
    if discrete && lanes >= 128 { 2 } else { 1 }
}

/// Multiplier applied to the per-threadgroup state budget when sizing Poseidon batches.
#[must_use]
pub fn poseidon_batch_multiplier() -> u32 {
    let hints = device_hints();
    let working_set = working_set_gib(hints);
    if hints.is_discrete() && working_set >= 48.0 {
        4
    } else if hints.is_discrete() && working_set >= 32.0 {
        3
    } else if working_set >= 24.0 {
        2
    } else {
        1
    }
}

/// Recommend an LDE tile depth before clamping to runtime bounds.
#[must_use]
pub fn lde_tile_stage_target(eval_log: u32, hints: DeviceHints) -> u32 {
    if eval_log == 0 {
        return 0;
    }
    let base = match eval_log {
        n if n >= 22 => 8,
        n if n >= 20 => 10,
        n if n >= 18 => 12,
        n if n >= 16 => 14,
        n if n >= 14 => 16,
        n if n >= 12 => 20,
        _ => eval_log,
    };
    let mut target = base.min(eval_log);
    let working_set = working_set_gib(hints);
    if hints.is_discrete() && working_set >= 24.0 && eval_log >= 18 {
        target = target.saturating_add(2).min(eval_log);
    } else if working_set >= 16.0 && eval_log >= 18 {
        target = target.saturating_add(1).min(eval_log);
    }
    target
}

fn working_set_gib(hints: DeviceHints) -> f64 {
    if hints.recommended_max_working_set == 0 {
        0.0
    } else {
        let whole_gib =
            u32::try_from(hints.recommended_max_working_set / GIB_BYTES).unwrap_or(u32::MAX);
        let remainder = u32::try_from(hints.recommended_max_working_set % GIB_BYTES)
            .unwrap_or_else(|_| {
                unreachable!("remainder is always less than one GiB");
            });
        f64::from(whole_gib) + f64::from(remainder) / GIB_F64
    }
}

#[cfg(test)]
pub fn set_device_hints_for_tests(hints: Option<DeviceHints>) {
    let store = TEST_DEVICE_HINTS.get_or_init(|| std::sync::Mutex::new(None));
    if let Ok(mut guard) = store.lock() {
        *guard = hints;
    }
}

fn ceil_power_of_two(value: u32) -> u32 {
    if value <= 1 {
        1
    } else {
        value
            .checked_next_power_of_two()
            .unwrap_or(MAX_THREADGROUP_LANES)
    }
}

fn floor_power_of_two(value: u32) -> u32 {
    if value <= 1 {
        1
    } else {
        1 << (31 - value.leading_zeros())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_lane_override_accepts_power_of_two() {
        assert_eq!(parse_fft_lane_override("64").unwrap(), 64);
    }

    #[test]
    fn parse_lane_override_rejects_non_power_of_two() {
        assert!(parse_fft_lane_override("48").is_err());
    }

    #[test]
    fn parse_lane_override_rejects_out_of_range() {
        assert!(parse_fft_lane_override("4").is_err());
        assert!(parse_fft_lane_override("512").is_err());
    }

    #[test]
    fn parse_tile_override_bounds() {
        assert_eq!(parse_fft_tile_override("1").unwrap(), 1);
        assert_eq!(parse_fft_tile_override("8").unwrap(), 8);
        assert!(parse_fft_tile_override("0").is_err());
    }

    #[test]
    fn poseidon_lane_override_accepts_power_of_two() {
        assert_eq!(parse_poseidon_lane_override("64").unwrap(), 64);
        assert!(parse_poseidon_lane_override("48").is_err());
    }

    #[test]
    fn poseidon_lane_override_rejects_out_of_range() {
        assert!(parse_poseidon_lane_override("8").is_err());
        assert!(parse_poseidon_lane_override("512").is_err());
    }

    #[test]
    fn poseidon_batch_override_bounds() {
        assert_eq!(parse_poseidon_batch_override("1").unwrap(), 1);
        assert_eq!(parse_poseidon_batch_override("16").unwrap(), 16);
        assert!(parse_poseidon_batch_override("0").is_err());
        assert!(parse_poseidon_batch_override("64").is_err());
    }

    #[test]
    fn default_lane_target_scales_with_log_len() {
        assert!(default_lane_target(18) > default_lane_target(8));
    }

    #[test]
    fn fft_tuning_respects_hardware_limits() {
        let tuning = fft_tuning(20, 32, 64);
        assert!(tuning.threadgroup_lanes <= 64);
        assert!(tuning.threadgroup_lanes >= 32);
        assert!(tuning.tile_stage_limit <= MAX_TILE_STAGE_LIMIT);
    }

    #[test]
    fn fft_tuning_scales_with_trace_size() {
        let small = fft_tuning(8, 16, 32);
        assert_eq!(small.threadgroup_lanes, 32);
        assert_eq!(small.tile_stage_limit, 5);

        let large = fft_tuning(18, 32, 512);
        assert_eq!(large.threadgroup_lanes, 256);
        assert_eq!(large.tile_stage_limit, 12);

        let huge = fft_tuning(22, 32, 512);
        assert_eq!(huge.threadgroup_lanes, 256);
        assert_eq!(huge.tile_stage_limit, 16);
    }

    #[test]
    fn poseidon_tuning_respects_limits() {
        let tuning = poseidon_tuning(40, 128);
        assert!(tuning.threadgroup_lanes >= MIN_POSEIDON_LANES);
        assert!(tuning.threadgroup_lanes <= MAX_POSEIDON_LANES);
        assert!(tuning.states_per_lane >= MIN_POSEIDON_BATCH);
        assert!(tuning.states_per_lane <= MAX_POSEIDON_BATCH);
    }

    #[test]
    fn poseidon_tuning_scales_with_device_hints() {
        set_device_hints_for_tests(Some(DeviceHints::new(false, true, true, 0)));
        let tuning = poseidon_tuning(64, 512);
        assert_eq!(tuning.threadgroup_lanes, 256);
        assert_eq!(tuning.states_per_lane, 2);
        set_device_hints_for_tests(None);
    }

    #[test]
    fn poseidon_states_increase_for_large_discrete_gpus() {
        let hints = DeviceHints::new(false, true, true, 64 * GIB_BYTES);
        assert_eq!(default_poseidon_states_per_lane(hints, 256), 2);
        let medium = DeviceHints::new(false, true, true, 32 * GIB_BYTES);
        assert_eq!(default_poseidon_states_per_lane(medium, 256), 2);
    }

    #[test]
    fn poseidon_states_respect_integrated_memory_limits() {
        let integrated = DeviceHints::new(true, false, false, 24 * GIB_BYTES);
        assert_eq!(default_poseidon_states_per_lane(integrated, 256), 1);
        let low_mem = DeviceHints::new(true, false, false, 8 * GIB_BYTES);
        assert_eq!(default_poseidon_states_per_lane(low_mem, 128), 1);
    }

    #[test]
    fn poseidon_multiplier_scales_with_hardware() {
        set_device_hints_for_tests(Some(DeviceHints::new(false, true, true, 48 * GIB_BYTES)));
        assert_eq!(poseidon_batch_multiplier(), 4);
        set_device_hints_for_tests(Some(DeviceHints::new(false, true, true, 32 * GIB_BYTES)));
        assert_eq!(poseidon_batch_multiplier(), 3);
        set_device_hints_for_tests(Some(DeviceHints::new(true, false, false, 24 * GIB_BYTES)));
        assert_eq!(poseidon_batch_multiplier(), 2);
        set_device_hints_for_tests(None);
        assert_eq!(poseidon_batch_multiplier(), 1);
    }

    #[test]
    fn working_set_gib_reports_fractional_values() {
        let half = GIB_BYTES / 2;
        let hints = DeviceHints::new(false, false, false, GIB_BYTES + half);
        let reported = working_set_gib(hints);
        assert!((reported - 1.5).abs() < f64::EPSILON);
    }

    #[test]
    fn lde_tile_stage_target_tracks_memory_tier() {
        set_device_hints_for_tests(None);
        let default_target = lde_tile_stage_target(18, device_hint_snapshot());
        assert_eq!(default_target, 12);

        set_device_hints_for_tests(Some(DeviceHints::new(false, true, true, 24 * GIB_BYTES)));
        let boosted = lde_tile_stage_target(18, device_hint_snapshot());
        assert_eq!(boosted, 14);

        set_device_hints_for_tests(None);
    }
}
