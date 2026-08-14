use super::*;
use std::time::Duration as StdDuration;
#[test]
fn router_parse_clamps_invalid_values() {
    let router = Router {
        twap_window_seconds: 0,
        epsilon_bps: 20_000,
        buffer_alert_pct: 10,
        buffer_throttle_pct: 50,
        buffer_xor_only_pct: 40,
        buffer_halt_pct: 35,
        buffer_horizon_hours: 0,
    };
    let mut emitter = Emitter::new();
    let actual = router.parse(&mut emitter);
    assert_eq!(
        actual.twap_window,
        StdDuration::from_secs(defaults::settlement::router::TWAP_WINDOW_SECS)
    );
    assert_eq!(
        actual.epsilon_bps,
        defaults::settlement::router::EPSILON_BPS
    );
    assert_eq!(
        actual.buffer_alert_pct,
        defaults::settlement::router::ALERT_PCT
    );
    assert_eq!(
        actual.buffer_throttle_pct,
        defaults::settlement::router::THROTTLE_PCT
    );
    assert_eq!(
        actual.buffer_xor_only_pct,
        defaults::settlement::router::XOR_ONLY_PCT
    );
    assert_eq!(
        actual.buffer_halt_pct,
        defaults::settlement::router::HALT_PCT
    );
    assert_eq!(
        actual.buffer_horizon_hours,
        defaults::settlement::router::BUFFER_HORIZON_HOURS
    );
    assert!(emitter.into_result().is_err());
}
