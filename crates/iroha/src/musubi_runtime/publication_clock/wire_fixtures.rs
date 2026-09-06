//! Pre-extraction durable clock frame and digest evidence.

use super::*;
use crate::musubi_runtime::tests::wire_fixtures::record;

pub(in crate::musubi_runtime) fn check_identities(fixtures: &[norito::json::Value]) {
    use crate::musubi_runtime::tests::wire_fixtures::check_identity;
    check_identity::<DurableClockStateV1>(fixtures);
    check_identity::<DurableClockEnvelopeV1>(fixtures);
}

pub(in crate::musubi_runtime) fn records() -> Vec<norito::json::Value> {
    let mut state = DurableClockStateV1::new(1_735_689_600_123);
    state.revision = 7;
    state.validate().unwrap();
    let envelope = DurableClockEnvelopeV1::new(state.clone()).unwrap();
    envelope.validate().unwrap();
    vec![
        record("clock.state", &state),
        record("clock.envelope", &envelope),
        norito::json!({
            "specimen": "clock.digest",
            "state_digest": (hex::encode(state.digest().unwrap())),
        }),
    ]
}
