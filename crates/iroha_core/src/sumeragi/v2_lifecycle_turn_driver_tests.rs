//! Focused tests for the lifecycle-owned Completion and Ingress turn boundary.

#[test]
fn turn_outcomes_retain_only_the_real_pass_through_cursor() {
    let source = include_str!("v2_lifecycle_turn_driver.rs");
    assert!(source.contains("PassThrough(LifecycleCurrentRunnerTurn<'cursor>)"));
    assert!(!source.contains("LifecycleRunnerRankSnapshot"));
    assert!(!source.contains("into_parts"));
    assert!(!source.contains("pub executor:"));
    assert!(!source.contains("pub services:"));
}
