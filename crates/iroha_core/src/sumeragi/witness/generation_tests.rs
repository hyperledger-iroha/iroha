//! Recorder generations isolate stale overlays without changing successful witness bytes.

use super::*;
use iroha_data_model::fastpq::{TransferDeltaTranscript, TransferSmtWitness};
use iroha_test_samples::{ALICE_ID, BOB_ID};
use norito::codec::Encode;
use std::{sync::mpsc, thread};

#[derive(Clone, Copy, Debug)]
enum TranscriptMode {
    Append,
    Replace,
    ReplaceEmpty,
}

const MODES: [TranscriptMode; 3] = [
    TranscriptMode::Append,
    TranscriptMode::Replace,
    TranscriptMode::ReplaceEmpty,
];

fn transcript(seed: u8) -> TransferTranscript {
    let batch_hash = Hash::new([seed, 0x91]);
    let delta = TransferDeltaTranscript {
        from_account: (*ALICE_ID).clone(),
        to_account: (*BOB_ID).clone(),
        asset_definition: AssetDefinitionId::derive_from_components(
            DomainId::try_new("wonderland", "universal").unwrap(),
            "rose".parse().unwrap(),
        ),
        amount: Quantity::from(1_u32),
        from_balance_before: Quantity::from(10_u32),
        from_balance_after: Quantity::from(9_u32),
        to_balance_before: Quantity::zero(),
        to_balance_after: Quantity::from(1_u32),
        from_smt_witness: TransferSmtWitness::default(),
        to_smt_witness: TransferSmtWitness::default(),
    };
    let digest = crate::fastpq::poseidon_preimage_digest(&delta, &batch_hash);
    TransferTranscript {
        batch_hash,
        deltas: vec![delta],
        authority_digest: crate::fastpq::authority_digest(&ALICE_ID),
        poseidon_preimage_digest: Some(digest),
    }
}

fn transcript_map(seed: u8) -> BTreeMap<Hash, Vec<TransferTranscript>> {
    let transcript = transcript(seed);
    BTreeMap::from([(transcript.batch_hash, vec![transcript])])
}

fn record(seed: u8, mode: TranscriptMode) {
    with_active_slot(|witness| {
        witness.reads.entry(vec![0x11]).or_insert(vec![seed]);
        witness.writes.insert(vec![0x22], vec![seed]);
    });
    match mode {
        TranscriptMode::Append => record_fastpq_transcript(&transcript(seed)),
        TranscriptMode::Replace => synchronize_fastpq_transcripts(&transcript_map(seed)),
        TranscriptMode::ReplaceEmpty => synchronize_fastpq_transcripts(&BTreeMap::new()),
    }
}

fn expected(seed: u8) -> ExecWitness {
    ExecWitness {
        reads: vec![ExecKv {
            key: vec![0x11],
            value: vec![seed],
        }],
        writes: vec![ExecKv {
            key: vec![0x22],
            value: vec![seed],
        }],
        fastpq_transcripts: map_to_bundles(transcript_map(seed)),
        fastpq_batches: Vec::new(),
    }
}

fn assert_inactive_generation_removed() {
    let witness = lock_slot();
    assert!(!witness.active);
    assert!(witness.generation.is_none());
    assert!(witness.reads.is_empty());
    assert!(witness.writes.is_empty());
    assert!(witness.fastpq_transcripts.is_empty());
}

fn assert_capture(expected: ExecWitness) {
    let actual = drain_exec_witness_checked(|_| Ok(())).unwrap();
    assert_eq!(actual, expected);
    assert_inactive_generation_removed();
}

fn seed_from_joined_thread(seed: u8) {
    // The caller deliberately keeps an old TLS overlay. Seed real global records on
    // a thread without that overlay, and join before examining or draining them.
    thread::spawn(move || record(seed, TranscriptMode::Append))
        .join()
        .unwrap();
}

#[test]
fn stale_overlay_cannot_append_replace_or_clear_restarted_capture() {
    let _guard = exec_witness_guard();
    for mode in MODES {
        start_block();
        let old = begin_exec_witness_overlay();
        record(1, mode);
        assert!(drain_exec_witness_checked(|_| Ok(())).is_err());
        assert_inactive_generation_removed();
        start_block();
        seed_from_joined_thread(2);

        let mut called = false;
        with_active_slot(|_| called = true);
        assert!(
            !called,
            "a stale overlay must consume the attempted operation"
        );
        record(3, mode);
        old.commit();

        // Removing stale guards does not disable a fresh overlay in the same B capture.
        let fresh = begin_exec_witness_overlay();
        record(4, TranscriptMode::Append);
        fresh.commit();
        let mut expected = expected(2);
        expected.writes[0].value = vec![4];
        let mut map = transcript_map(2);
        map.extend(transcript_map(4));
        expected.fastpq_transcripts = map_to_bundles(map);
        assert_capture(expected);
    }
}

#[test]
fn stale_nested_guards_and_new_children_preserve_lifo_without_rebinding() {
    let _guard = exec_witness_guard();
    for commit_outer in [false, true] {
        for commit_inner in [false, true] {
            for commit_child in [false, true] {
                start_block();
                let outer = begin_exec_witness_overlay();
                record(1, TranscriptMode::Append);
                let inner = begin_exec_witness_overlay();
                record(2, TranscriptMode::ReplaceEmpty);
                assert!(drain_exec_witness_checked(|_| Ok(())).is_err());
                start_block();
                seed_from_joined_thread(3);
                let child = begin_exec_witness_overlay();
                {
                    let global = lock_slot();
                    EXEC_WITNESS_OVERLAYS.with(|overlays| {
                        let overlays = overlays.borrow();
                        assert_eq!(overlays.len(), 3);
                        assert!(same_recorder_generation(
                            &overlays[1].witness,
                            &overlays[2].witness,
                        ));
                        assert!(!same_recorder_generation(&global, &overlays[2].witness));
                    });
                }
                record(4, TranscriptMode::ReplaceEmpty);
                if commit_child {
                    child.commit();
                } else {
                    drop(child);
                }
                if commit_inner {
                    inner.commit();
                } else {
                    drop(inner);
                }
                if commit_outer {
                    outer.commit();
                } else {
                    drop(outer);
                }
                EXEC_WITNESS_OVERLAYS.with(|overlays| assert!(overlays.borrow().is_empty()));
                assert_capture(expected(3));
            }
        }
    }
}

#[test]
fn overlay_opened_inactive_and_its_child_never_attach_to_a_later_capture() {
    let _guard = exec_witness_guard();
    clear_block();
    let old = begin_exec_witness_overlay();
    record(1, TranscriptMode::Append);
    start_block();
    seed_from_joined_thread(2);
    let child = begin_exec_witness_overlay();
    EXEC_WITNESS_OVERLAYS.with(|overlays| {
        for overlay in overlays.borrow().iter() {
            assert!(overlay.witness.generation.is_none());
        }
    });
    record(3, TranscriptMode::ReplaceEmpty);
    child.commit();
    old.commit();
    assert_capture(expected(2));
}

#[derive(Clone, Copy, Debug)]
enum Invalidation {
    Restart,
    Clear,
    LegacyDrain,
    CheckedDrain,
    CheckedReject,
    CheckedPanic,
    CachedReject,
    GuardCleanup,
}

#[test]
fn every_capture_lifecycle_boundary_discards_cross_thread_stale_commits() {
    for invalidation in [
        Invalidation::Restart,
        Invalidation::Clear,
        Invalidation::LegacyDrain,
        Invalidation::CheckedDrain,
        Invalidation::CheckedReject,
        Invalidation::CheckedPanic,
        Invalidation::CachedReject,
        Invalidation::GuardCleanup,
    ] {
        for mode in MODES {
            let mut guard = Some(exec_witness_guard());
            start_block();
            record(1, TranscriptMode::Append);
            let (ready_tx, ready_rx) = mpsc::sync_channel(0);
            let (resume_tx, resume_rx) = mpsc::sync_channel(0);
            let worker = thread::spawn(move || {
                let old = begin_exec_witness_overlay();
                record(2, mode);
                ready_tx.send(()).unwrap();
                resume_rx.recv().unwrap();
                record(4, mode);
                old.commit();
            });
            ready_rx.recv().unwrap();
            // Deliberately violate the caller's join-before-drain rule to test isolation
            // of a scoped stale worker. Direct unscoped worker writes remain caller-owned.
            match invalidation {
                Invalidation::Restart => start_block(),
                Invalidation::Clear => clear_block(),
                Invalidation::LegacyDrain => assert_eq!(drain_exec_witness(), expected(1)),
                Invalidation::CheckedDrain => assert_capture(expected(1)),
                Invalidation::CheckedReject => {
                    assert_eq!(
                        drain_exec_witness_checked(|_| Err("owner rejected".to_owned())),
                        Err("owner rejected".to_owned()),
                    );
                }
                Invalidation::CheckedPanic => {
                    assert!(
                        std::panic::catch_unwind(|| {
                            let _ = drain_exec_witness_checked(|_| {
                                panic!("validator panic invalidates the capture generation");
                            });
                        })
                        .is_err()
                    );
                }
                Invalidation::CachedReject => {
                    assert!(finish_cached_exec_witness_capture().is_err());
                }
                Invalidation::GuardCleanup => {
                    drop(guard.take());
                    guard = Some(exec_witness_guard());
                }
            }
            if !matches!(invalidation, Invalidation::Restart) {
                assert_inactive_generation_removed();
            }
            start_block();
            record(3, TranscriptMode::Append);
            resume_tx.send(()).unwrap();
            worker.join().unwrap();
            assert_capture(expected(3));
            drop(guard);
        }
    }
}

#[test]
fn same_generation_nested_overlays_preserve_direct_recording_bytes() {
    let _guard = exec_witness_guard();
    for mode in MODES {
        start_block();
        record(1, TranscriptMode::Append);
        record(2, TranscriptMode::Append);
        record(3, mode);
        record(4, TranscriptMode::Append);
        let direct = drain_exec_witness_checked(|_| Ok(())).unwrap();
        start_block();
        record(1, TranscriptMode::Append);
        let outer = begin_exec_witness_overlay();
        record(2, TranscriptMode::Append);
        let inner = begin_exec_witness_overlay();
        record(3, mode);
        inner.commit();
        record(4, TranscriptMode::Append);
        outer.commit();
        let scoped = drain_exec_witness_checked(|_| Ok(())).unwrap();
        assert_eq!(scoped, direct, "same-generation mode {mode:?}");
        assert_eq!(scoped.encode(), direct.encode());
        assert_inactive_generation_removed();
    }
}

#[test]
fn generation_identity_rejects_absence_and_distinct_live_allocations() {
    let absent = BlockWitness::default();
    assert!(!same_recorder_generation(&absent, &absent));
    let token = Arc::new(RecorderGeneration);
    let first = BlockWitness {
        generation: Some(Arc::clone(&token)),
        ..BlockWitness::default()
    };
    let same = BlockWitness {
        generation: Some(token),
        ..BlockWitness::default()
    };
    let other = BlockWitness {
        generation: Some(Arc::new(RecorderGeneration)),
        ..BlockWitness::default()
    };
    assert!(same_recorder_generation(&first, &same));
    assert!(!same_recorder_generation(&first, &other));
    assert!(!same_recorder_generation(&first, &absent));
    assert!(!same_recorder_generation(&absent, &first));
}

#[test]
fn rejected_merge_does_not_propagate_empty_replacement_or_change_target_identity() {
    let token = Arc::new(RecorderGeneration);
    for source_generation in [None, Some(Arc::new(RecorderGeneration))] {
        let mut target = WitnessOverlayFrame {
            witness: BlockWitness {
                generation: Some(Arc::clone(&token)),
                fastpq_transcripts: transcript_map(1),
                ..BlockWitness::default()
            },
            replaces_fastpq_transcripts: false,
        };
        let source = WitnessOverlayFrame {
            witness: BlockWitness {
                generation: source_generation,
                ..BlockWitness::default()
            },
            replaces_fastpq_transcripts: true,
        };
        merge_overlay_frame(&mut target, source);
        assert!(!target.replaces_fastpq_transcripts);
        assert_eq!(target.witness.fastpq_transcripts, transcript_map(1));
        assert!(Arc::ptr_eq(
            target.witness.generation.as_ref().unwrap(),
            &token
        ));
    }
}

#[test]
fn stale_overlay_keeps_its_identity_alive_until_lifo_cleanup() {
    let _guard = exec_witness_guard();
    start_block();
    let old = {
        let global = lock_slot();
        Arc::downgrade(global.generation.as_ref().unwrap())
    };
    let overlay = begin_exec_witness_overlay();
    clear_block();
    assert_inactive_generation_removed();
    assert!(old.upgrade().is_some());
    start_block();
    {
        let global = lock_slot();
        assert!(!Arc::ptr_eq(
            &old.upgrade().unwrap(),
            global.generation.as_ref().unwrap(),
        ));
    }
    drop(overlay);
    assert!(old.upgrade().is_none());
    record(2, TranscriptMode::Append);
    assert_capture(expected(2));
}
