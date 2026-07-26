//! UI tests for `iroha_telemetry_derive`.
#![cfg(all(feature = "trybuild-tests", not(coverage)))]
use trybuild::TestCases;

#[test]
fn ui() {
    let test_cases = TestCases::new();
    test_cases.pass("tests/ui_pass/basic.rs");
    test_cases.pass("tests/ui_pass/timing_no_feature.rs");
    #[cfg(feature = "metric-instrumentation")]
    test_cases.pass("tests/ui_pass/timing.rs");
    test_cases.compile_fail("tests/ui_fail/*.rs");
}
