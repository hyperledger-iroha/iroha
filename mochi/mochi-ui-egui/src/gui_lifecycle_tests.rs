//! Lifecycle aggregation tests for the desktop supervisor shell.
use std::collections::HashSet;
use mochi_core::SupervisorError;
use super::MochiApp;
#[test]
fn alias_operation_aggregation_attempts_every_requested_peer() {
    let aliases = vec!["peer0".to_owned(), "peer1".to_owned(), "peer2".to_owned()];
    let mut attempted = Vec::new();
    let failures = MochiApp::collect_alias_operation_failures(&aliases, |alias| {
        attempted.push(alias.to_owned());
        if alias == "peer0" || alias == "peer2" {
            Err("injected start failure")
        } else {
            Ok(())
        }
    });
    assert_eq!(attempted, aliases, "every alias must be attempted");
    assert_eq!(failures.len(), 2);
    assert!(failures[0].contains("peer0"));
    assert!(failures[1].contains("peer2"));
}
#[test]
fn requested_peer_start_skips_running_and_aggregates_all_failures() {
    let aliases = vec!["peer0".to_owned(), "peer1".to_owned(), "peer2".to_owned()];
    let already_running = HashSet::from(["peer1".to_owned()]);
    let mut attempted = Vec::new();
    let result = MochiApp::start_requested_peer_aliases_with(
        &aliases,
        &already_running,
        |alias| -> Result<(), &'static str> {
            attempted.push(alias.to_owned());
            Err("injected start failure")
        },
    );
    assert_eq!(attempted, ["peer0", "peer2"]);
    let error = result.expect_err("both attempted starts should be aggregated");
    assert!(matches!(error, SupervisorError::PeerSetStart { .. }));
    let message = error.to_string();
    assert!(message.contains("peer0"));
    assert!(message.contains("peer2"));
    assert!(!message.contains("peer1"));
}
#[test]
fn partial_stop_restore_failure_preserves_both_errors() {
    let combined = MochiApp::combine_with_running_set_restore(
        SupervisorError::Config("injected partial stop failure".to_owned()),
        Err(SupervisorError::PeerSetStart {
            details: "peer1: injected restore failure".to_owned(),
        }),
    );
    assert!(matches!(
        combined,
        SupervisorError::OperationAndRunningSetRestore { .. }
    ));
    let message = combined.to_string();
    assert!(message.contains("partial stop failure"));
    assert!(message.contains("peer1"));
    assert!(message.contains("restore failure"));
}
