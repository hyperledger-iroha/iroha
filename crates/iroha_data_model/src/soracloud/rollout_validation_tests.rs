//! Rollout traffic, health, and first-error boundaries.
use super::*;

fn baseline(stage: SoraRolloutStageV1) -> SoraServiceRolloutStateV1 {
    SoraServiceRolloutStateV1 {
        schema_version: SORA_SERVICE_ROLLOUT_STATE_VERSION_V1,
        rollout_handle: "portal:rollout:1".to_owned(),
        baseline_version: "1.0.0".to_owned(),
        candidate_version: "1.1.0".to_owned(),
        canary_percent: 10,
        traffic_percent: match stage {
            SoraRolloutStageV1::Canary => 10,
            SoraRolloutStageV1::Promoted => 100,
            SoraRolloutStageV1::RolledBack => 0,
        },
        stage,
        health_failures: if stage == SoraRolloutStageV1::RolledBack {
            2
        } else {
            0
        },
        max_health_failures: 2,
        health_window_secs: 30,
        created_sequence: 1,
        updated_sequence: 1,
    }
}

#[test]
fn rollout_stages_enforce_traffic_and_health_boundaries() {
    for stage in [
        SoraRolloutStageV1::Canary,
        SoraRolloutStageV1::Promoted,
        SoraRolloutStageV1::RolledBack,
    ] {
        for (percent, expected) in [
            (0, stage == SoraRolloutStageV1::RolledBack),
            (9, false),
            (10, stage == SoraRolloutStageV1::Canary),
            (99, stage == SoraRolloutStageV1::Canary),
            (100, stage == SoraRolloutStageV1::Promoted),
            (101, false),
        ] {
            let mut changed = baseline(stage);
            changed.traffic_percent = percent;
            assert_eq!(
                changed.validate().is_ok(),
                expected,
                "{stage:?}: traffic {percent}"
            );
        }
        for failures in [0, 1, 2, 3, u32::MAX] {
            let mut changed = baseline(stage);
            changed.health_failures = failures;
            let expected = match stage {
                SoraRolloutStageV1::Canary => failures < 2,
                SoraRolloutStageV1::Promoted => failures == 0,
                SoraRolloutStageV1::RolledBack => failures >= 2,
            };
            assert_eq!(
                changed.validate().is_ok(),
                expected,
                "{stage:?}: failures {failures}"
            );
        }
    }
}

fn rejects_field(state: &SoraServiceRolloutStateV1, expected: &str) {
    match state.validate().unwrap_err() {
        SoracloudManifestError::InvalidField { field, .. } => assert_eq!(field, expected),
        other => panic!("expected invalid {expected}, got {other:?}"),
    }
}

#[test]
fn rollout_validation_preserves_identity_traffic_health_sequence_order() {
    let mut state = baseline(SoraRolloutStageV1::Canary);
    state.baseline_version.clone_from(&state.candidate_version);
    state.canary_percent = 101;
    state.traffic_percent = 101;
    state.max_health_failures = 0;
    state.health_failures = 2;
    state.health_window_secs = 0;
    state.updated_sequence = 0;
    rejects_field(&state, "baseline_version");
    state.baseline_version = "1.0.0".to_owned();
    rejects_field(&state, "canary_percent");
    state.canary_percent = 10;
    rejects_field(&state, "traffic_percent");
    state.traffic_percent = 10;
    rejects_field(&state, "max_health_failures");
    state.max_health_failures = 2;
    rejects_field(&state, "health_failures");
    state.health_failures = 1;
    rejects_field(&state, "health_window_secs");
    state.health_window_secs = 30;
    rejects_field(&state, "updated_sequence");
    state.updated_sequence = 1;
    assert!(state.validate().is_ok());
}
