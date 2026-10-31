//! Maintenance observations over explicit custody/proof mocks, without phone qualification.

use super::*;
use std::{ops::ControlFlow, sync::mpsc, thread};

#[test]
fn authentic_enrollment_parks_without_granting_fresh_work_or_sampling_archive_age() {
    let mut w = wallet();
    let source = w.custody.status.clone();
    let records = w.archive.records.lock().unwrap().clone();
    w.proofs.reject = true;
    assert_eq!(
        w.maintenance_view().unwrap(),
        MaintenanceViewV1::BeforeBootstrap
    );
    assert!(matches!(w.require_fresh_work(), Err(Error::NoHead)));
    assert_eq!(
        w.deferred_archive_wake_hint().unwrap(),
        DeferredArchiveWakeHintV1::NoAgeWake
    );
    let turn = w.scheduler().prepare_archive_turn().unwrap();
    assert_eq!(
        w.drain_deferred_archive_with_test_clock(turn, || panic!(
            "Enrollment has no age candidate"
        ))
        .unwrap(),
        DeferredArchiveDrainV1::NoWork
    );
    assert_eq!(w.custody.status, source);
    assert!(w.custody.checkpoint.is_none());
    assert_eq!(*w.archive.records.lock().unwrap(), records);
    assert_eq!(w.custody.publication_calls, 0);
    assert_eq!(w.custody.signatures, 0);
    assert_eq!(w.proofs.preparations.load(Ordering::SeqCst), 0);
    w.proofs.reject = false;
    assert!(matches!(
        w.commit(bootstrap()).unwrap(),
        Completion::Complete(_)
    ));
    assert!(matches!(
        w.maintenance_view().unwrap(),
        MaintenanceViewV1::Released(_)
    ));
}

#[test]
fn enrollment_never_hides_selected_manifest_loss_or_nonempty_checkpoint() {
    let mut w = wallet();
    let SlotStatus::Enrollment(record) = &w.custody.status else {
        panic!("Enrollment fixture")
    };
    let mut file: TestMarkerFile = archive::decode(record.file_bytes()).unwrap();
    file.archive_checkpoint = [0x93; 32];
    let invalid = MarkerRecord::decode(
        &archive::encode(&file).unwrap(),
        record.slot(),
        &w.scheme_id,
    )
    .unwrap_err();
    w.custody.status_error = Some(invalid);
    assert!(matches!(w.maintenance_view(), Err(Error::Provider(actual)) if actual == invalid));
    assert!(
        w.custody.checkpoint.is_none(),
        "missing selected checkpoint is not pristine Enrollment"
    );

    let mut w = wallet();
    let empty = archive::encode(&w.manifest().unwrap().1).unwrap();
    let digest =
        crate::kagemusha_wallet_advance_v1::kagemusha_wallet_archive_checkpoint_digest_v1(&empty);
    w.custody.checkpoint = Some((digest, empty));
    assert!(
        matches!(w.maintenance_view(), Err(Error::WitnessLost(_))),
        "persisted unindexed manifest is invalid"
    );
    w.custody.checkpoint = Some(([0x94; 32], b"corrupt manifest".to_vec()));
    assert!(matches!(w.maintenance_view(), Err(Error::WitnessLost(_))));

    let mut released = wallet();
    released.commit(bootstrap()).unwrap();
    w.custody.checkpoint = released.custody.checkpoint;
    assert!(
        w.maintenance_view().is_err(),
        "Enrollment cannot hide an indexed selected source"
    );
    assert_eq!(w.custody.signatures, 0);
    assert_eq!(w.custody.publication_calls, 0);
}

#[test]
fn enrollment_observation_rechecks_source_and_propagates_provider_failure() {
    let mut released = wallet();
    released.commit(bootstrap()).unwrap();
    let mut w = wallet();
    w.custody.status_replacement = Some((w.custody.status_calls + 3, released.custody.status));
    assert!(matches!(w.maintenance_view(), Err(Error::WitnessLost(_))));
    w.custody.status_replacement = None;
    w.custody.checkpoint_replacement =
        Some((w.custody.checkpoint_calls + 2, ([0x95; 32], vec![1])));
    assert!(matches!(w.maintenance_view(), Err(Error::WitnessLost(_))));
    w.custody.checkpoint_replacement = None;
    let error = ProviderError::Unavailable(
        crate::kagemusha_wallet_advance_v1::KagemushaWalletUnavailableV1::Locked,
    );
    w.custody.status_error = Some(error);
    assert!(matches!(w.maintenance_view(), Err(Error::Provider(actual)) if actual == error));
    w.custody.status_error = None;
    assert_eq!(
        w.maintenance_view().unwrap(),
        MaintenanceViewV1::BeforeBootstrap
    );
    assert_eq!(w.custody.publication_calls, 0);
    assert_eq!(w.custody.signatures, 0);
}

#[test]
fn selected_pending_view_precedes_snapshot_and_does_not_execute_recovery() {
    let mut w = wallet();
    w.custody.pause = true;
    assert_eq!(w.commit(bootstrap()).unwrap(), Completion::Pending);
    let checkpoint = w.custody.checkpoint.clone();
    let records = w.archive.records.lock().unwrap().clone();
    let reads = w.archive.reads.load(Ordering::SeqCst);
    w.proofs.reject = true;
    assert_eq!(
        w.maintenance_view().unwrap(),
        MaintenanceViewV1::SelectedPending
    );
    assert_eq!(w.archive.reads.load(Ordering::SeqCst), reads);
    assert_eq!(w.custody.checkpoint, checkpoint);
    assert_eq!(*w.archive.records.lock().unwrap(), records);
    assert_eq!(w.custody.signatures, 0);
    // Observation grants no recovery permission or verdict; the existing resume rechecks it.
    w.proofs.reject = false;
    w.custody.pause = false;
    assert!(matches!(w.resume().unwrap(), Some(Completion::Complete(_))));
    assert!(matches!(
        w.maintenance_view().unwrap(),
        MaintenanceViewV1::Released(_)
    ));
    assert_eq!(w.custody.signatures, 1);
}

#[test]
fn unfolded_archive_view_survives_exact_dequeue_and_reopen_without_queue_membership() {
    for core_pending in [false, true] {
        let (mut w, send, original) =
            credit_projection::payer_projection_wallet(core_pending, None);
        let MaintenanceViewV1::AwaitingArchiveFold(snapshot) = w.maintenance_view().unwrap() else {
            panic!("current ArchiveSent needs its covering fold");
        };
        assert!(snapshot.fold_backlog > 0);
        let receipt = w.retain_deferred_credited(&send, &original).unwrap();
        w.verify_deferred_credited(&receipt).unwrap();
        let entry = w.deferred_archive_summary().unwrap().oldest.unwrap();
        assert!(matches!(
            w.reconcile_deferred_archive(&entry).unwrap(),
            RequestStatusV1::Outcome(Completion::Complete(_))
        ));
        assert_eq!(w.deferred_archive_summary().unwrap().count, 0);
        let mut w =
            Coordinator::new(w.custody, w.archive, w.proofs, w.scheme_id, w.wallet_id).unwrap();
        assert_eq!(w.deferred_archive_summary().unwrap().count, 0);
        assert_eq!(
            w.maintenance_view().unwrap(),
            MaintenanceViewV1::AwaitingArchiveFold(snapshot)
        );
        assert_eq!(w.custody.signatures, 0);
    }
}

#[test]
fn covered_archive_and_ordinary_unfolded_head_are_distinct_from_archive_wait() {
    let (mut archive, _, _) = credit_projection::payer_projection_wallet(false, Some(false));
    let MaintenanceViewV1::Released(snapshot) = archive.maintenance_view().unwrap() else {
        panic!("covered ArchiveSent");
    };
    assert_eq!(snapshot.fold_backlog, 0);
    assert_eq!(snapshot.verified_fold.unwrap().head, snapshot.head);
    let mut w = wallet();
    w.commit(bootstrap()).unwrap();
    let MaintenanceViewV1::Released(snapshot) = w.maintenance_view().unwrap() else {
        panic!("unfolded Bootstrap is ordinary backlog");
    };
    assert_eq!(snapshot.fold_backlog, 1, "Released is not idle admission");
    assert!(snapshot.verified_fold.is_none());
    snapshot_test_fold(&mut w);
    let MaintenanceViewV1::Released(snapshot) = w.maintenance_view().unwrap() else {
        panic!("covered Bootstrap");
    };
    assert_eq!(snapshot.fold_backlog, 0);
}

#[test]
fn unavailable_or_missing_selected_fold_never_becomes_released_readiness() {
    let mut empty = wallet();
    assert_eq!(
        empty.maintenance_view().unwrap(),
        MaintenanceViewV1::BeforeBootstrap
    );
    assert!(matches!(empty.require_fresh_work(), Err(Error::NoHead)));
    let (mut w, _, _) = credit_projection::payer_projection_wallet(false, Some(false));
    let expected = w.maintenance_view().unwrap();
    let failure = ProviderError::Unavailable(
        crate::kagemusha_wallet_advance_v1::KagemushaWalletUnavailableV1::Locked,
    );
    w.custody.status_error = Some(failure);
    assert!(matches!(w.maintenance_view(), Err(Error::Provider(error)) if error == failure));
    w.custody.status_error = None;
    let sequence = w.snapshot().unwrap().sequence;
    let fold = w
        .archive
        .records
        .lock()
        .unwrap()
        .remove(&ArchiveKey::Fold(sequence))
        .unwrap();
    assert!(matches!(w.maintenance_view(), Err(Error::WitnessLost(_))));
    w.archive
        .records
        .lock()
        .unwrap()
        .insert(ArchiveKey::Fold(sequence), fold);
    assert_eq!(w.maintenance_view().unwrap(), expected);
    assert_eq!(w.custody.signatures, 0);
}

#[test]
fn pending_source_change_during_released_observation_is_never_released() {
    let mut pending = wallet();
    pending.custody.pause = true;
    assert_eq!(pending.commit(bootstrap()).unwrap(), Completion::Pending);
    let mut w = wallet();
    w.commit(bootstrap()).unwrap();
    let signatures = w.custody.signatures;
    w.custody.status_replacement = Some((w.custody.status_calls + 2, pending.custody.status));
    assert!(matches!(w.maintenance_view(), Err(Error::Pending)));
    assert_eq!(w.custody.signatures, signatures);
}

#[test]
fn maintenance_observation_preserves_running_and_parked_fold_work() {
    let mut w = wallet();
    w.proofs.checkpoint_stages = 3;
    w.commit(bootstrap()).unwrap();
    w.scheduler().set_activity(true, false);
    let job = match w.fold_prepare().unwrap() {
        ControlFlow::Continue(job) => job,
        ControlFlow::Break(ended) => panic!("admitted fold job: {ended:?}"),
    };
    let (finished_tx, finished) = mpsc::channel();
    let reader = thread::spawn(move || {
        let result = w.maintenance_view();
        finished_tx.send(()).unwrap();
        (w, result)
    });
    let read_without_join = finished.recv_timeout(Duration::from_secs(5)).is_ok();
    // Always release the job before asserting, even against an implementation that joins it.
    let proved = job.prove();
    let (mut w, result) = reader.join().unwrap();
    assert!(
        read_without_join,
        "maintenance read must not join proof work"
    );
    assert!(matches!(result.unwrap(), MaintenanceViewV1::Released(_)));
    assert_eq!(
        w.fold_publish(proved.expect("maintenance must not cancel proof work"))
            .unwrap()
            .status,
        FoldStatus::Checkpoint {
            sequence: 0,
            ordinal: 0
        }
    );
    let retained = w.proofs.last_left();
    let checkpoint = w.custody.checkpoint.clone();
    let records = w.archive.records.lock().unwrap().clone();
    assert!(w.scheduler().holds_retained_for_test());
    assert!(matches!(
        w.maintenance_view().unwrap(),
        MaintenanceViewV1::Released(_)
    ));
    assert!(w.scheduler().holds_retained_for_test());
    assert_eq!(w.custody.checkpoint, checkpoint);
    assert_eq!(*w.archive.records.lock().unwrap(), records);
    assert_eq!(
        w.fold_once().unwrap(),
        FoldStatus::Checkpoint {
            sequence: 0,
            ordinal: 1
        }
    );
    assert_eq!(w.proofs.last_handed(), Some(retained));
    assert_eq!(w.custody.signatures, 1);
}
