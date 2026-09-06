//! Trace-digest queue regressions.

use super::*;

#[test]
fn queue_and_collect_trace_digests() {
    reset_trace_digest_state_for_tests();
    let code_hash = [0x11; 32];
    let digest = [0xAA; 32];
    let artifact = make_trace_digest_artifact(code_hash, None, digest);
    queue_trace_digest(7, artifact.clone());
    let collected = collect_trace_digests_for_height(7);
    assert_eq!(collected.len(), 1);
    assert_eq!(collected[0].backend, TRACE_DIGEST_BACKEND);
    assert_eq!(collected[0].proof, digest.to_vec());
    assert_eq!(collected[0].code_hash, code_hash);
    assert!(collected[0].tx_hash.is_none());
    // Subsequent collection should be empty once drained.
    assert!(collect_trace_digests_for_height(7).is_empty());
}
