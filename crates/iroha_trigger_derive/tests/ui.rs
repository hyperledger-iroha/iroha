//! UI tests for `iroha_trigger_derive`.
#![cfg(all(feature = "trybuild-tests", not(coverage)))]

use trybuild::TestCases;

#[test]
fn ui() {
    let t = TestCases::new();
    t.pass("tests/ui/pass/*.rs");
    t.compile_fail("tests/ui/fail/*.rs");
}
