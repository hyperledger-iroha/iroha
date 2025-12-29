//! Trybuild UI tests for `iroha_derive` (config base).
#![cfg(all(feature = "trybuild-tests", not(coverage)))]
use trybuild::TestCases;

#[test]
fn ui() {
    let test_cases = TestCases::new();
    test_cases.pass("tests/config_base_ui_pass/*.rs");
    test_cases.compile_fail("tests/config_base_ui_fail/*.rs");
}
