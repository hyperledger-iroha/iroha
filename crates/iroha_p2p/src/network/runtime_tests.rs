use super::*;
use iroha_config::parameters::actual::{
    SoranetPow as ActualSoranetPow, SoranetPuzzle as ConfigPuzzle,
};
use std::{
    fs,
    num::{NonZeroU32, NonZeroUsize},
    time::Duration,
};
use tempfile::tempdir;
#[test]
fn runtime_from_handshake_preserves_puzzle_parameters() {
    let mut handshake = ActualSoranetHandshake::default();
    handshake.pow.required = true;
    handshake.pow.difficulty = 6;
    handshake.pow.max_future_skew = Duration::from_secs(300);
    handshake.pow.min_ticket_ttl = Duration::from_secs(60);
    handshake.pow.ticket_ttl = Duration::from_secs(240);
    handshake.pow.puzzle = Some(ConfigPuzzle {
        memory_kib: NonZeroU32::new(64 * 1024).expect("memory"),
        time_cost: NonZeroU32::new(3).expect("time_cost"),
        lanes: NonZeroU32::new(2).expect("lanes"),
    });
    let dir = tempdir().expect("tempdir");
    handshake.pow.revocation_store_path = dir
        .path()
        .join("revocations.norito")
        .to_string_lossy()
        .into_owned()
        .into();
    let runtime = runtime_from_handshake(handshake).expect("runtime");
    assert!(
        runtime.pow_required(),
        "puzzle-enabled handshake must require PoW"
    );
    let pow = runtime.pow_parameters();
    assert_eq!(pow.difficulty(), 6);
    let puzzle = runtime
        .puzzle_parameters()
        .expect("puzzle parameters should be present");
    assert_eq!(puzzle.memory_kib().get(), 64 * 1024);
    assert_eq!(puzzle.time_cost().get(), 3);
    assert_eq!(puzzle.lanes().get(), 2);
    assert_eq!(
        runtime.puzzle_work_capacities(),
        (NonZeroUsize::new(1).unwrap(), NonZeroUsize::new(1).unwrap())
    );
}
#[test]
fn runtime_reload_rejects_puzzle_capacity_change_without_replacing_the_gate() {
    let mut handshake = ActualSoranetHandshake::default();
    let dir = tempdir().expect("tempdir");
    handshake.pow.revocation_store_path = dir
        .path()
        .join("revocations.norito")
        .to_string_lossy()
        .into_owned()
        .into();
    let runtime = runtime_from_handshake(handshake.clone()).expect("initial runtime");
    handshake.pow.outbound_mint_capacity = NonZeroUsize::new(2).unwrap();
    let error = runtime_from_handshake(handshake).expect_err("capacity reload requires restart");
    assert!(
        matches!(error, Error::HandshakeSoranet(message) if message.contains("restart required"))
    );
    assert_eq!(
        runtime.puzzle_work_capacities(),
        (NonZeroUsize::new(1).unwrap(), NonZeroUsize::new(1).unwrap())
    );
}

#[test]
fn runtime_from_handshake_rejects_oversized_actual_puzzle_capacity() {
    let mut handshake = ActualSoranetHandshake::default();
    handshake.pow.outbound_mint_capacity = NonZeroUsize::new(usize::MAX).unwrap();
    let error = runtime_from_handshake(handshake)
        .expect_err("programmatic actual capacity must respect the production bound");
    assert!(matches!(error, Error::HandshakeSoranet(message)
            if message.contains("outbound_mint_capacity")
                && message.contains("exceeds the per-direction maximum")));

    let mut handshake = ActualSoranetHandshake::default();
    handshake.pow.inbound_verify_capacity =
        NonZeroUsize::new(ActualSoranetPow::MAX_PUZZLE_WORK_CAPACITY_PER_DIRECTION + 1).unwrap();
    let error = runtime_from_handshake(handshake)
        .expect_err("inbound actual capacity must respect the production bound");
    assert!(matches!(error, Error::HandshakeSoranet(message)
            if message.contains("inbound_verify_capacity")
                && message.contains("exceeds the per-direction maximum")));
}

#[test]
fn runtime_from_handshake_rejects_invalid_pow_bounds() {
    let mut handshake = ActualSoranetHandshake::default();
    handshake.pow.required = true;
    handshake.pow.max_future_skew = Duration::from_secs(30);
    handshake.pow.min_ticket_ttl = Duration::from_secs(60);
    let err = runtime_from_handshake(handshake).expect_err("invalid PoW bounds must fail");
    match err {
        Error::HandshakeSoranet(message) => {
            assert!(
                message.contains("PoW")
                    && message.contains("max_future_skew")
                    && message.contains("min_ttl"),
                "expected PoW bounds validation failure, got {message}"
            );
        }
        other => panic!("unexpected error type: {other:?}"),
    }
}
#[test]
fn runtime_from_handshake_rejects_puzzle_ticket_ttl_without_solution_window() {
    let mut handshake = ActualSoranetHandshake::default();
    handshake.pow.required = true;
    handshake.pow.max_future_skew = Duration::from_secs(300);
    handshake.pow.min_ticket_ttl = Duration::from_secs(60);
    handshake.pow.ticket_ttl = Duration::from_secs(60);
    let err = runtime_from_handshake(handshake)
        .expect_err("puzzle target ttl equal to the required remainder must fail startup");
    match err {
        Error::HandshakeSoranet(message) => assert!(
            message.contains("ticket_ttl")
                && message.contains("must exceed")
                && message.contains("min_ticket_ttl"),
            "expected puzzle solution-window validation failure, got {message}"
        ),
        other => panic!("unexpected error type: {other:?}"),
    }
}
#[test]
fn runtime_from_handshake_rejects_invalid_revocation_limits() {
    let mut handshake = ActualSoranetHandshake::default();
    handshake.pow.required = true;
    handshake.pow.revocation_store_capacity = 0;
    let err = runtime_from_handshake(handshake).expect_err("should fail");
    match err {
        Error::HandshakeSoranet(message) => {
            assert!(
                message.contains("revocation"),
                "expected revocation validation failure, got {message}"
            );
        }
        other => panic!("unexpected error type: {other:?}"),
    }
}
#[test]
fn runtime_from_handshake_fails_closed_on_corrupt_revocation_snapshot() {
    let mut handshake = ActualSoranetHandshake::default();
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("revocations.norito");
    fs::write(&path, b"corrupt snapshot").expect("write corrupt revocation file");
    handshake.pow.required = true;
    handshake.pow.difficulty = 1;
    handshake.pow.revocation_store_path = path.to_string_lossy().into_owned().into();
    let err = runtime_from_handshake(handshake)
        .expect_err("corrupt persistent replay state must fail startup");
    assert!(
        matches!(
            err,
            Error::HandshakeSoranet(ref message)
                if message.contains("failed to load soranet revocation store")
        ),
        "unexpected error: {err:?}"
    );
}
#[test]
fn disabled_test_admission_uses_only_in_memory_replay_state() {
    let mut handshake = ActualSoranetHandshake::default();
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("intentionally-unused-revocations.norito");
    fs::write(&path, b"corrupt snapshot").expect("write corrupt revocation file");
    handshake.pow.required = false;
    handshake.pow.revocation_store_path = path.to_string_lossy().into_owned().into();
    let runtime =
        runtime_from_handshake(handshake).expect("test-local disabled admission is in-memory");
    assert!(!runtime.pow_required());
    assert_eq!(runtime.active_revocations(), 0);
}
