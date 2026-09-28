//! UI tests for `iroha_telemetry_derive`.
#![cfg(all(feature = "trybuild-tests", not(coverage)))]
use trybuild::TestCases;
/// `trybuild-tests` implies `metric-instrumentation` and `telemetry`, so every
/// case is compiled against the fully instrumented expansion. The
/// telemetry-disabled expansion is covered by the crate's unit tests.
#[test]
fn ui() {
    let test_cases = TestCases::new();
    test_cases.pass("tests/ui_pass/basic.rs");
    test_cases.pass("tests/ui_pass/timing.rs");
    test_cases.pass("tests/ui_pass/labels.rs");
    test_cases.compile_fail("tests/ui_fail/*.rs");
}
