//! Ensure pre-decode cache statistics track successes and failures.

use ivm::ivm_cache::{IvmCache, global_stats};

#[test]
fn predecode_stats_track_success_and_failure() {
    let before = global_stats();

    let mut halt = Vec::new();
    halt.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());
    let decoded = IvmCache::decode_stream(&halt).expect("halt should decode");
    assert_eq!(decoded.len(), 1, "halt bytecode should decode to one op");

    let after_success = global_stats();
    assert!(
        after_success.decoded_streams > before.decoded_streams,
        "decoded stream count should increment"
    );
    assert!(
        after_success.decoded_ops_total >= before.decoded_ops_total + decoded.len() as u64,
        "decoded op count should increment by stream length"
    );
    assert!(
        after_success.decode_time_ns_total >= before.decode_time_ns_total,
        "decode time should be monotonic"
    );
    assert_eq!(
        after_success.decode_failures, before.decode_failures,
        "successful decode must not bump failure counter"
    );

    // Feed an intentionally invalid byte stream (truncated instruction) to trigger a failure.
    let invalid = vec![0xFF];
    let err = IvmCache::decode_stream(&invalid).expect_err("invalid stream must fail");
    assert!(
        matches!(
            err,
            ivm::VMError::DecodeError | ivm::VMError::MemoryAccessViolation { .. }
        ),
        "unexpected error variant: {err:?}"
    );

    let after_failure = global_stats();
    assert!(
        after_failure.decode_failures > after_success.decode_failures,
        "failure counter should increment"
    );
    assert!(
        after_failure.decode_time_ns_total >= after_success.decode_time_ns_total,
        "decode time remains monotonic after failure"
    );
    assert!(
        after_failure.decoded_streams >= after_success.decoded_streams,
        "failed decode must not reduce decoded stream count"
    );
}
