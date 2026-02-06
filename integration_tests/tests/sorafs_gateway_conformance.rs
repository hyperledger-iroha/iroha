#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Tests for the `SoraFS` gateway conformance harness.

use integration_tests::{
    sorafs_gateway_capability_refusal,
    sorafs_gateway_conformance::{ScenarioOutcome, default_suite_report},
};
use norito::json::Value;

#[test]
fn sorafs_gateway_conformance_suite_passes() {
    let report = default_suite_report();
    assert!(
        report.all_passed(),
        "expected canned conformance suite to pass for fixture-backed gateway simulation"
    );
}

#[test]
fn sorafs_gateway_suite_report_json_contains_expected_fields() {
    let report = default_suite_report();
    let value = report.to_json_value();
    assert!(
        value.get("profile_version").is_some(),
        "profile_version field should be present"
    );
    assert!(
        value.get("load_profile").is_some(),
        "load_profile field should be present"
    );
    assert!(
        value.get("load_report").is_some(),
        "load_report field should be present"
    );
    let scenarios = value
        .get("scenarios")
        .and_then(|v| v.as_array())
        .expect("scenarios array must be present");
    assert_eq!(
        scenarios.len(),
        report.scenario_count(),
        "scenario count mismatch between report and JSON"
    );
    let capability_fixture = sorafs_gateway_capability_refusal::load_scenarios()
        .expect("load capability refusal scenarios");
    let expected_c1 = capability_fixture
        .iter()
        .find(|scenario| scenario.id == "C1")
        .expect("C1 capability scenario present in fixtures");
    let capability_entry = scenarios
        .iter()
        .find(|scenario| {
            scenario
                .get("id")
                .and_then(Value::as_str)
                .is_some_and(|id| id == "C1")
        })
        .expect("capability scenario C1 present in suite JSON");
    let refusal = capability_entry
        .get("refusal")
        .and_then(Value::as_object)
        .expect("capability scenario JSON should include refusal payload");
    assert_eq!(
        refusal.get("status"),
        Some(&Value::from(u64::from(expected_c1.status))),
        "capability refusal JSON must include status matching fixture"
    );
    assert_eq!(
        refusal.get("error"),
        Some(&Value::from(expected_c1.error.clone())),
        "capability refusal JSON must include error code"
    );
    assert_eq!(
        refusal.get("reason"),
        Some(&Value::from(expected_c1.reason.clone())),
        "capability refusal JSON must include human-readable reason"
    );
    assert_eq!(
        refusal.get("details"),
        Some(&expected_c1.details),
        "capability refusal JSON must include details map"
    );
}

#[test]
fn sorafs_gateway_provider_denylist_is_refused() {
    let report = default_suite_report();
    let scenario = report
        .scenario("B5")
        .expect("scenario B5 must be present in default harness");
    assert_eq!(scenario.expected_status, 412, "expected status mismatch");
    assert_eq!(scenario.observed_status, 412, "observed status mismatch");
    assert_eq!(
        scenario.observed_outcome,
        ScenarioOutcome::Refusal,
        "observed outcome mismatch"
    );
}

#[test]
fn sorafs_gateway_refusal_scenarios_match_expectations() {
    let report = default_suite_report();
    let expected = [
        ("A3", 416u16),
        ("B1", 406),
        ("B2", 428),
        ("B3", 422),
        ("B4", 422),
        ("B5", 412),
        ("B6", 429),
        ("C1", 406),
        ("C2", 412),
        ("C3", 412),
        ("C4", 428),
        ("C5", 428),
        ("C6", 406),
        ("C7", 422),
        ("D1", 451),
    ];

    for (id, status) in expected {
        let scenario = report
            .scenario(id)
            .unwrap_or_else(|| panic!("scenario {id} must be present in default harness"));
        assert_eq!(
            scenario.expected_outcome,
            ScenarioOutcome::Refusal,
            "expected outcome mismatch for scenario {id}"
        );
        assert_eq!(
            scenario.observed_outcome,
            ScenarioOutcome::Refusal,
            "observed outcome mismatch for scenario {id}"
        );
        assert_eq!(
            scenario.expected_status, status,
            "expected status mismatch for scenario {id}"
        );
        assert_eq!(
            scenario.observed_status, status,
            "observed status mismatch for scenario {id}"
        );
        assert!(
            scenario.passed(),
            "scenario {id} should be marked as passed after refusal validation"
        );
    }
}

#[test]
fn sorafs_gateway_capability_refusals_align_with_fixtures() {
    let report = default_suite_report();
    let scenarios = sorafs_gateway_capability_refusal::load_scenarios()
        .expect("load capability refusal scenarios");

    for scenario in scenarios {
        let entry = report.scenario(&scenario.id).unwrap_or_else(|| {
            panic!(
                "capability refusal scenario {} missing from suite",
                scenario.id
            )
        });
        assert_eq!(
            entry.expected_status, scenario.status,
            "expected status mismatch for capability scenario {}",
            scenario.id
        );
        assert_eq!(
            entry.observed_status, scenario.status,
            "observed status mismatch for capability scenario {}",
            scenario.id
        );
        assert_eq!(
            entry.expected_outcome,
            ScenarioOutcome::Refusal,
            "expected outcome should be refusal for capability scenario {}",
            scenario.id
        );
        assert_eq!(
            entry.observed_outcome,
            ScenarioOutcome::Refusal,
            "observed outcome should be refusal for capability scenario {}",
            scenario.id
        );
        let refusal = entry.refusal.as_ref().unwrap_or_else(|| {
            panic!(
                "capability scenario {} missing refusal payload",
                scenario.id
            )
        });
        assert_eq!(
            refusal.status, scenario.status,
            "refusal status mismatch for capability scenario {}",
            scenario.id
        );
        assert_eq!(
            refusal.error, scenario.error,
            "refusal error code mismatch for capability scenario {}",
            scenario.id
        );
        assert_eq!(
            refusal.reason, scenario.reason,
            "refusal reason mismatch for capability scenario {}",
            scenario.id
        );
        assert_eq!(
            refusal.details, scenario.details,
            "refusal details mismatch for capability scenario {}",
            scenario.id
        );
    }
}
