//! Compiler diagnostics for required generated-builder identity declarations.
#![cfg(all(feature = "trybuild-tests", not(coverage)))]

#[test]
fn registrable_builder_identity_metadata() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/ui_fail/registrable_builder_identity_missing.rs");
    cases.compile_fail("tests/ui_fail/registrable_builder_identity_duplicate.rs");
    cases.compile_fail("tests/ui_fail/registrable_builder_identity_invalid.rs");
}
