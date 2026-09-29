//! Existing Metal helper tests qualification controls.

use super::{
    AdaptiveScheduler, MAX_QUEUE_FANOUT, QueuePolicy, STATE_WIDTH, default_queue_column_threshold,
    lde_tile_stage_limit, parse_queue_fanout_override, parse_queue_threshold_override,
    poseidon_element_range, poseidon_recommended_states_per_batch, post_tile_stage_start,
    queue_total_columns_hint, select_poseidon_batch, select_poseidon_batch_with_scheduler,
};
use crate::metal_config::{self, DeviceHints};
#[test]
fn post_tile_stage_start_only_dispatches_when_needed() {
    assert_eq!(post_tile_stage_start(10, 4), Some(4));
    assert_eq!(post_tile_stage_start(8, 16), None);
    assert_eq!(post_tile_stage_start(0, 4), None);
}
#[test]
fn lde_tile_stage_limit_scales_with_log_size() {
    let _hint_guard = metal_config::device_hints_test_guard();
    assert_eq!(lde_tile_stage_limit(5), 5);
    assert_eq!(lde_tile_stage_limit(18), 8);
    assert_eq!(lde_tile_stage_limit(64), 8);
}
#[test]
fn lde_tile_stage_limit_respects_device_hints() {
    let _hint_guard = metal_config::device_hints_test_guard();
    metal_config::set_device_hints_for_tests(Some(DeviceHints::new(
        false,
        true,
        true,
        24 * 1024 * 1024 * 1024,
    )));
    assert_eq!(lde_tile_stage_limit(18), 8);
}
#[test]
fn queue_policy_round_robins_above_threshold() {
    let policy = QueuePolicy::new(3, 8);
    let below = policy.select_index(4, 5);
    assert_eq!(below, 0, "fan-out should not engage below threshold");
    let indices: Vec<_> = (0..6).map(|idx| policy.select_index(16, idx)).collect();
    assert_eq!(indices, vec![0, 1, 2, 0, 1, 2]);
}
#[test]
fn queue_policy_clamps_requested_values() {
    let policy = QueuePolicy::new(0, 0);
    assert_eq!(policy.fanout(), 1);
    assert_eq!(policy.column_threshold(), 1);
    let capped = QueuePolicy::new(MAX_QUEUE_FANOUT + 10, 4);
    assert_eq!(capped.fanout(), MAX_QUEUE_FANOUT);
    assert_eq!(capped.column_threshold(), 4);
}
#[test]
fn queue_fanout_override_validation() {
    assert_eq!(parse_queue_fanout_override("2").unwrap(), 2);
    assert!(parse_queue_fanout_override("0").is_err());
    assert!(parse_queue_fanout_override("abc").is_err());
}
#[test]
fn queue_threshold_override_validation() {
    assert_eq!(parse_queue_threshold_override("12").unwrap(), 12);
    assert!(parse_queue_threshold_override("0").is_err());
    assert!(parse_queue_threshold_override("abc").is_err());
}
#[test]
fn default_queue_threshold_scales_with_fanout() {
    assert_eq!(default_queue_column_threshold(1), u32::MAX);
    assert_eq!(default_queue_column_threshold(2), 16);
    assert_eq!(default_queue_column_threshold(3), 24);
}
#[test]
fn inverse_fft_hint_disables_threshold_fanout() {
    let policy = QueuePolicy::new(2, 16);
    assert_eq!(queue_total_columns_hint(16, true, &policy), 15);
    assert_eq!(queue_total_columns_hint(15, true, &policy), 15);
    assert_eq!(queue_total_columns_hint(32, true, &policy), 32);
    assert_eq!(queue_total_columns_hint(16, false, &policy), 16);
}
#[test]
fn poseidon_recommended_batch_respects_caps() {
    let tuning = metal_config::PoseidonTuning {
        threadgroup_lanes: 64,
        states_per_lane: 4,
    };
    assert_eq!(poseidon_recommended_states_per_batch(0, tuning), 0);
    assert_eq!(poseidon_recommended_states_per_batch(1, tuning), 1);
    let target = tuning
        .threadgroup_lanes
        .saturating_mul(tuning.states_per_lane)
        .saturating_mul(metal_config::poseidon_batch_multiplier());
    let recommended = poseidon_recommended_states_per_batch(target * 2, tuning);
    let base = tuning
        .threadgroup_lanes
        .saturating_mul(tuning.states_per_lane);
    let max_expected = base.saturating_mul(4);
    assert!(recommended >= base);
    assert!(recommended <= max_expected);
}
#[test]
fn poseidon_batch_selection_respects_remaining_states() {
    let tuning = metal_config::PoseidonTuning {
        threadgroup_lanes: 64,
        states_per_lane: 4,
    };
    let total_states = 32;
    let selection = select_poseidon_batch(total_states, tuning);
    assert!((1..=total_states).contains(&selection.columns()));
    let sample = selection.sample_for(selection.columns());
    assert!(sample.is_some(), "adaptive sample expected");
}
#[test]
fn poseidon_batch_selection_clamps_shared_state_to_current_safe_cap() {
    let scheduler = AdaptiveScheduler::new();
    let seeded = scheduler.select_poseidon(4_096, 4_096);
    assert_eq!(seeded.columns(), 4_096);
    let tuning = metal_config::PoseidonTuning {
        threadgroup_lanes: 2,
        states_per_lane: 1,
    };
    let state_count = 4_096;
    let recommended = poseidon_recommended_states_per_batch(state_count, tuning);
    let selection = select_poseidon_batch_with_scheduler(&scheduler, state_count, tuning);
    assert_eq!(selection.max_columns, recommended);
    assert!(selection.columns() <= recommended);
}
#[test]
fn poseidon_element_range_scales_with_state_width() {
    let range = poseidon_element_range(2, 3).expect("range");
    assert_eq!(range.start, 2 * STATE_WIDTH);
    assert_eq!(range.end, 5 * STATE_WIDTH);
}
