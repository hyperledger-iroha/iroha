//! Trybuild UI tests for `iroha_derive`.
#![cfg(all(feature = "trybuild-tests", not(coverage)))]
#[path = "config_base_ui.rs"]
mod config_base_ui;
use trybuild::TestCases;
fn serial_guard() -> std::sync::MutexGuard<'static, ()> {
    static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());
    SERIAL
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}
#[test]
fn ui() {
    let _serial = crate::serial_guard();
    let test_cases = TestCases::new();
    test_cases.pass("tests/ui_pass/*.rs");
    test_cases.compile_fail("tests/ui_fail/*.rs");
}
