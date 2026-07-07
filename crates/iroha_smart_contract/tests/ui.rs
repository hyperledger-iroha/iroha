//! Trybuild UI tests for `iroha_smart_contract`.

#[cfg(feature = "trybuild-tests")]
#[test]
fn ui_suite() {
    let cases = trybuild::TestCases::new();
    cases.pass("tests/ui/pass/*.rs");
    cases.compile_fail("tests/ui/fail/*.rs");
}
