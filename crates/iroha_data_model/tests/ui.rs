//! Trybuild UI tests harness.
#![cfg(all(feature = "trybuild-tests", not(coverage)))]
use trybuild::TestCases;

#[test]
fn ui() {
    let t = TestCases::new();
    t.case("tests/ui/instruction_registry.rs");
}
