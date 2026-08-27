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
    handshake.pow.difficulty = 6;
    handshake.pow.max_future_skew = Duration::from_secs(300);
    handshake.pow.min_ticket_ttl = Duration::from_secs(60);
    handshake.pow.ticket_ttl = Duration::from_secs(240);
    handshake.pow.puzzle = ConfigPuzzle {
        memory_kib: NonZeroU32::new(64 * 1024).expect("memory"),
        time_cost: NonZeroU32::new(3).expect("time_cost"),
        lanes: NonZeroU32::new(2).expect("lanes"),
    };
    let dir = tempdir().expect("tempdir");
    handshake.pow.revocation_store_path = dir
        .path()
        .join("revocations.norito")
        .to_string_lossy()
        .into_owned()
        .into();
    let runtime = runtime_from_handshake(handshake).expect("runtime");
    let runtime = runtime.snapshot().expect("runtime policy");
    let puzzle = runtime.puzzle_parameters();
    assert_eq!(puzzle.difficulty(), 6);
    assert_eq!(puzzle.memory_kib().get(), 64 * 1024);
    assert_eq!(puzzle.time_cost().get(), 3);
    assert_eq!(puzzle.lanes().get(), 2);
    assert_eq!(
        runtime.puzzle_work_capacities(),
        (
            ActualSoranetPow::DEFAULT_PUZZLE_WORK_CAPACITY_PER_DIRECTION,
            ActualSoranetPow::DEFAULT_PUZZLE_WORK_CAPACITY_PER_DIRECTION,
        )
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
    let error = runtime
        .reload(handshake)
        .expect_err("capacity reload requires restart");
    assert!(
        matches!(error, Error::HandshakeSoranet(message) if message.contains("restart required"))
    );
    assert_eq!(
        runtime
            .snapshot()
            .expect("active policy")
            .puzzle_work_capacities(),
        (
            ActualSoranetPow::DEFAULT_PUZZLE_WORK_CAPACITY_PER_DIRECTION,
            ActualSoranetPow::DEFAULT_PUZZLE_WORK_CAPACITY_PER_DIRECTION,
        )
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
fn runtime_from_handshake_rejects_invalid_puzzle_bounds() {
    let mut handshake = ActualSoranetHandshake::default();
    handshake.pow.max_future_skew = Duration::from_secs(30);
    handshake.pow.min_ticket_ttl = Duration::from_secs(60);
    let err = runtime_from_handshake(handshake).expect_err("invalid puzzle bounds must fail");
    match err {
        Error::HandshakeSoranet(message) => {
            assert!(
                message.contains("puzzle")
                    && message.contains("max_future_skew")
                    && message.contains("min_ticket_ttl"),
                "expected puzzle bounds validation failure, got {message}"
            );
        }
        other => panic!("unexpected error type: {other:?}"),
    }
}
#[test]
fn runtime_from_handshake_rejects_puzzle_ticket_ttl_without_solution_window() {
    let mut handshake = ActualSoranetHandshake::default();
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
fn handshake_with_replay_path(path: &std::path::Path) -> ActualSoranetHandshake {
    let mut handshake = ActualSoranetHandshake::default();
    handshake.pow.revocation_store_path = path.to_string_lossy().into_owned().into();
    handshake
}

#[test]
fn runtime_reload_same_path_publishes_new_difficulty() {
    let dir = tempdir().expect("tempdir");
    let mut handshake = handshake_with_replay_path(&dir.path().join("revocations.norito"));
    handshake.pow.difficulty = 4;
    let runtime = runtime_from_handshake(handshake.clone()).expect("initial runtime");
    let initial = runtime.snapshot().expect("initial policy");

    handshake.pow.difficulty = 7;
    let updated = runtime.reload(handshake).expect("compatible reload");

    assert_eq!(updated.puzzle_parameters().difficulty(), 7);
    assert!(!Arc::ptr_eq(&initial, &updated));
    assert!(Arc::ptr_eq(
        &updated,
        &runtime.snapshot().expect("published policy")
    ));
}

#[test]
fn runtime_reload_preserves_replay_pending_and_admission_state() {
    let dir = tempdir().expect("tempdir");
    let mut handshake = handshake_with_replay_path(&dir.path().join("revocations.norito"));
    let runtime = runtime_from_handshake(handshake.clone()).expect("initial runtime");
    let initial = runtime.snapshot().expect("initial policy");

    handshake.pow.difficulty = handshake.pow.difficulty.saturating_add(1);
    let updated = runtime.reload(handshake).expect("compatible reload");

    assert!(
        initial.shares_security_state_with(&updated),
        "replay store, pending reservations, and puzzle gates must have one owner"
    );
}

#[test]
fn runtime_reload_rejects_owner_changes_without_replacing_policy() {
    let dir = tempdir().expect("tempdir");
    let handshake = handshake_with_replay_path(&dir.path().join("revocations.norito"));
    let runtime = runtime_from_handshake(handshake.clone()).expect("initial runtime");
    let initial = runtime.snapshot().expect("initial policy");

    let mut incompatible = Vec::new();
    let mut changed_path = handshake.clone();
    changed_path.pow.revocation_store_path = dir
        .path()
        .join("other-revocations.norito")
        .to_string_lossy()
        .into_owned()
        .into();
    incompatible.push(("pow.revocation_store_path", changed_path));
    let mut changed_store_capacity = handshake.clone();
    changed_store_capacity.pow.revocation_store_capacity += 1;
    incompatible.push(("pow.revocation_store_capacity", changed_store_capacity));
    let mut changed_store_ttl = handshake.clone();
    changed_store_ttl.pow.revocation_max_ttl += Duration::from_secs(1);
    incompatible.push(("pow.revocation_max_ttl", changed_store_ttl));
    let mut changed_work_capacity = handshake;
    changed_work_capacity.pow.outbound_mint_capacity = NonZeroUsize::new(2).unwrap();
    incompatible.push(("pow.outbound_mint_capacity", changed_work_capacity));

    for (field, requested) in incompatible {
        let error = runtime
            .reload(requested)
            .expect_err("owner-changing reload requires restart");
        assert!(
            matches!(&error, Error::HandshakeSoranet(message)
                if message.contains(field) && message.contains("restart required")),
            "unexpected error for {field}: {error:?}"
        );
        assert!(Arc::ptr_eq(
            &initial,
            &runtime.snapshot().expect("unchanged active policy")
        ));
    }
}

#[test]
fn listener_and_outbound_runtime_clones_observe_published_snapshot() {
    let dir = tempdir().expect("tempdir");
    let mut handshake = handshake_with_replay_path(&dir.path().join("revocations.norito"));
    handshake.pow.difficulty = 3;
    let runtime = runtime_from_handshake(handshake.clone()).expect("initial runtime");
    let listener_runtime = Arc::clone(&runtime);
    let outbound_runtime = Arc::clone(&runtime);

    handshake.pow.difficulty = 8;
    let updated = runtime.reload(handshake).expect("compatible reload");
    let listener_snapshot = listener_runtime.snapshot().expect("listener snapshot");
    let outbound_snapshot = outbound_runtime.snapshot().expect("outbound snapshot");

    assert!(Arc::ptr_eq(&updated, &listener_snapshot));
    assert!(Arc::ptr_eq(&updated, &outbound_snapshot));
    assert_eq!(listener_snapshot.puzzle_parameters().difficulty(), 8);
    assert_eq!(outbound_snapshot.puzzle_parameters().difficulty(), 8);
}
