// Restart timing coverage for the four-validator contention scenario.
#[test]
fn restart_scenario_uses_a_contention_tolerant_view_zero_deadline() {
    let cadence_ms = u64::try_from(RESTART_BLOCK_CADENCE.as_millis())
        .expect("restart cadence fits the canonical millisecond width");
    let (base_round_timeout_ms, _) =
        iroha_config::parameters::actual::sumeragi_v2_timing_ms(cadence_ms)
            .expect("restart cadence derives valid v2 timing");
    assert_eq!(base_round_timeout_ms, 20_000);
}
