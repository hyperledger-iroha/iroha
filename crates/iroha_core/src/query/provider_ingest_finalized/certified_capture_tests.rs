//! Archive publication and recovery from genuine current Sumeragi commit certificates.

use super::*;
use crate::{
    state::World,
    sumeragi::test_chain::{CertifiedTestChain, Signers, TestChainConfig},
};

fn chain() -> CertifiedTestChain {
    CertifiedTestChain::start(TestChainConfig::new(World::new(), 1_000)).expect("signed genesis")
}

#[test]
fn certified_capture_replays_exactly_and_recovers_one_crash_successor() {
    let directory = physical_tempdir().unwrap();
    let root = archive_root(&directory);
    let mut chain = chain();
    let archive = ProviderIngestFinalizedArchiveV1::try_open(&root, bounds()).unwrap();
    assert_eq!(
        archive
            .capture_certified_view(&chain.state().view(), chain.kura())
            .unwrap(),
        ProviderIngestFinalizedArchiveInsertOutcomeV1::Inserted
    );
    let generation = archive.health_generation().unwrap();
    assert_eq!(
        archive
            .capture_certified_view(&chain.state().view(), chain.kura())
            .unwrap(),
        ProviderIngestFinalizedArchiveInsertOutcomeV1::ExactReplay
    );
    assert_eq!(archive.health_generation().unwrap(), generation);
    chain.commit_at(2_000, Vec::new());
    drop(archive);
    let archive = ProviderIngestFinalizedArchiveV1::try_open(&root, bounds()).unwrap();
    let result = archive
        .reconcile_certified_state_tip(&chain.state().view(), chain.kura())
        .unwrap();
    assert_eq!(
        result.insertion(),
        ProviderIngestFinalizedArchiveInsertOutcomeV1::Inserted
    );
    assert!(!result.activation_floor_created());
    assert_eq!(
        result.qualification().archive_tip(),
        &authenticate_capture_view(&chain.state().view(), chain.kura()).unwrap()
    );
    assert_eq!(result.qualification().kura_tip_height(), 2);
    assert_eq!(result.qualification().lag_blocks(), 0);
    assert_eq!(archive.health_generation().unwrap(), generation + 1);
    let before = archive.health_generation().unwrap();
    let replay = archive
        .reconcile_certified_state_tip(&chain.state().view(), chain.kura())
        .unwrap();
    assert_eq!(
        replay.insertion(),
        ProviderIngestFinalizedArchiveInsertOutcomeV1::ExactReplay
    );
    assert!(!replay.activation_floor_created());
    assert_eq!(replay.qualification(), result.qualification());
    assert_eq!(archive.health_generation().unwrap(), before);
}

#[test]
fn certified_capture_never_manufactures_a_missing_committed_prefix() {
    let directory = physical_tempdir().unwrap();
    let mut chain = chain();
    let archive =
        ProviderIngestFinalizedArchiveV1::try_open(archive_root(&directory), bounds()).unwrap();
    archive
        .capture_certified_view(&chain.state().view(), chain.kura())
        .unwrap();
    let before = archive.health_generation().unwrap();
    chain.commit_at(2_000, Vec::new());
    chain.commit_at(3_000, Vec::new());
    assert!(
        archive
            .reconcile_certified_state_tip(&chain.state().view(), chain.kura())
            .is_err()
    );
    assert_eq!(archive.health_generation().unwrap(), before);
}

#[test]
fn certified_capture_refuses_bad_certificate_and_timestamp_substitution() {
    let directory = physical_tempdir().unwrap();
    let mut chain = chain();
    let archive =
        ProviderIngestFinalizedArchiveV1::try_open(archive_root(&directory), bounds()).unwrap();
    archive
        .capture_certified_view(&chain.state().view(), chain.kura())
        .unwrap();
    let before = archive.health_generation().unwrap();
    chain.commit_at(2_000, Vec::new());
    chain.corrupt_local_quorum_for_test(2, Signers::BelowQuorum);
    assert!(
        archive
            .capture_certified_view(&chain.state().view(), chain.kura())
            .is_err()
    );
    assert_eq!(archive.health_generation().unwrap(), before);

    let view = chain.state().view();
    let certified = CertifiedArchiveView::new(&view, chain.kura()).unwrap();
    let block = certified.block(1).unwrap();
    let mut key = ProviderIngestFinalizedArchiveKeyV1::try_new(
        chain.network_id(),
        1,
        *block.block_hash().as_ref(),
        block.block_time_ms(),
    )
    .unwrap();
    authenticate_archive_anchor(&key, view.network_id(), &certified).unwrap();
    let correct_hash = key.block_hash;
    key.block_hash[0] ^= 1;
    assert!(authenticate_archive_anchor(&key, view.network_id(), &certified).is_err());
    key.block_hash = correct_hash;
    key.finalized_at_unix_ms += 1;
    assert!(authenticate_archive_anchor(&key, view.network_id(), &certified).is_err());
}

#[test]
fn certified_capture_capacity_refusal_retries_without_artifact_or_state_writes() {
    let directory = physical_tempdir().unwrap();
    let chain = chain();
    let before = crate::snapshot::canonical_state_snapshot_hash(chain.state()).unwrap();
    let tiny = ProviderIngestFinalizedArchiveBoundsV1::try_new(1, 1, 1, 1, 1, 1, 1).unwrap();
    let archive =
        ProviderIngestFinalizedArchiveV1::try_open(archive_root(&directory), tiny).unwrap();
    for _ in 0..2 {
        let error = archive
            .capture_certified_view(&chain.state().view(), chain.kura())
            .unwrap_err();
        assert!(
            matches!(error, ProviderIngestFinalizedArchiveErrorV1::RecordTooLarge { observed, maximum: 1 } if observed > 1),
            "{error}"
        );
        assert!(archive.is_empty().unwrap());
        assert_eq!(fs::read_dir(&archive.records).unwrap().count(), 0);
        assert_eq!(fs::read_dir(&archive.checkpoints).unwrap().count(), 0);
        // Acquire and release a real private hash generation after refused capture.
        drop(chain.state().block_hashes.block());
        assert_eq!(
            crate::snapshot::canonical_state_snapshot_hash(chain.state()).unwrap(),
            before
        );
    }
}

#[test]
fn certified_capture_contention_preserves_exact_bytes_and_releases_for_retry() {
    use std::{
        future::Future as _,
        pin::Pin,
        task::{Context, Poll, Waker},
    };
    // Keep retired fixture cells fixed while checking the registration's
    // exact synchronous refund from the same State allocation pool.
    let _state_retirement = crossbeam_epoch::pin();
    let directory = physical_tempdir().unwrap();
    let chain = chain();
    let waiter_budget = chain.state().ivm_execution_budget();
    let mut registration = crate::unit_test_support::release_registration(&waiter_budget);
    let archive =
        ProviderIngestFinalizedArchiveV1::try_open(archive_root(&directory), bounds()).unwrap();
    let reader = archive.read_index().unwrap();
    let error = archive
        .capture_certified_view(&chain.state().view(), chain.kura())
        .unwrap_err();
    let ProviderIngestFinalizedArchiveErrorV1::IndexBusy { wait } = error else {
        panic!("held physical reader must refuse capture")
    };
    let mut release = wait.wait_for_release(&mut registration);
    assert!(matches!(
        Pin::new(&mut release).poll(&mut Context::from_waker(Waker::noop())),
        Poll::Pending
    ));
    assert_eq!(fs::read_dir(&archive.records).unwrap().count(), 0);
    drop(reader);
    let later_reader = archive.read_index().unwrap();
    assert!(matches!(
        Pin::new(&mut release).poll(&mut Context::from_waker(Waker::noop())),
        Poll::Ready(())
    ));
    assert!(
        matches!(
            archive.capture_certified_view(&chain.state().view(), chain.kura()),
            Err(ProviderIngestFinalizedArchiveErrorV1::IndexBusy { .. })
        ),
        "the released notification does not authorize a later reader's lock"
    );
    drop(later_reader);
    archive
        .capture_certified_view(&chain.state().view(), chain.kura())
        .unwrap();
    let bytes = archive_namespace_snapshot(&archive.records);
    let generation = archive.health_generation().unwrap();
    archive
        .capture_certified_view(&chain.state().view(), chain.kura())
        .unwrap();
    assert_eq!(archive_namespace_snapshot(&archive.records), bytes);
    assert_eq!(archive.health_generation().unwrap(), generation);
    drop(release);
    let with_registration = waiter_budget.reserved_bytes();
    drop(registration);
    assert_eq!(
        waiter_budget.reserved_bytes(),
        with_registration
            - iroha_allocation::release::ReleaseRegistration::allocation_layout().size()
    );
}

#[test]
fn certified_capture_moves_original_archive_to_worker_without_a_detached_producer() {
    fn assert_static_send_sync<T: Send + Sync + 'static>() {}
    assert_static_send_sync::<ProviderIngestFinalizedArchiveV1>();
    let directory = physical_tempdir().unwrap();
    let chain = chain();
    let state = Arc::clone(chain.state());
    let archive = Arc::new(
        ProviderIngestFinalizedArchiveV1::try_open(archive_root(&directory), bounds()).unwrap(),
    );
    let original = Arc::downgrade(&archive);
    let worker_owner = Arc::clone(&archive);
    drop(archive);
    assert!(original.upgrade().is_some());
    std::thread::spawn(move || {
        worker_owner
            .capture_certified_view(&state.view(), state.kura())
            .unwrap();
    })
    .join()
    .unwrap();
    assert!(original.upgrade().is_none());
    let reopened =
        ProviderIngestFinalizedArchiveV1::try_open(archive_root(&directory), bounds()).unwrap();
    let qualification = reopened
        .qualify_against_certified_tip(&chain.state().view(), chain.kura(), 0)
        .unwrap();
    assert_eq!(qualification.archive_tip().height, 1);
    assert_eq!(qualification.generation(), 1);
}

#[test]
fn certified_retention_waits_for_actual_index_owner_before_authority_or_file_mutation() {
    let directory = physical_tempdir().unwrap();
    let mut chain = chain();
    let archive = Arc::new(
        ProviderIngestFinalizedArchiveV1::try_open(archive_root(&directory), bounds()).unwrap(),
    );
    archive
        .capture_certified_view(&chain.state().view(), chain.kura())
        .unwrap();
    chain.commit_at(2_000, Vec::new());
    archive
        .capture_certified_view(&chain.state().view(), chain.kura())
        .unwrap();
    let state = Arc::clone(chain.state());
    let view = state.view();
    let key = authenticate_capture_view(&view, chain.kura()).unwrap();
    let fence = ProviderIngestFinalizedArchiveRetentionFenceV1::try_new(
        key,
        *chain.committed(2).id().0.as_ref(),
        archive.health_generation().unwrap(),
    )
    .unwrap();
    let proposal = archive
        .prepare_certified_compaction(&fence, &view, chain.kura())
        .unwrap();
    let checkpoint_digest = proposal.checkpoint_digest();
    let record_bytes: u64 = archive_namespace_snapshot(&archive.records)
        .values()
        .map(|bytes| u64::try_from(bytes.len()).unwrap())
        .sum();
    drop(view);
    let authority = Arc::new(TestRetentionAuthority::new());
    let worker_authority = Arc::clone(&authority);
    let worker_archive = Arc::clone(&archive);
    let reader = archive.read_index().unwrap();
    let (entered, started) = std::sync::mpsc::channel();
    let worker = std::thread::spawn(move || {
        entered.send(()).unwrap();
        worker_archive
            .approve_and_install_certified_compaction(
                &proposal,
                &state.view(),
                state.kura(),
                &worker_authority.binding(),
                worker_authority.as_ref(),
            )
            .unwrap()
    });
    started.recv().unwrap();
    assert!(authority.latest.lock().unwrap().is_none());
    assert_eq!(fs::read_dir(&archive.records).unwrap().count(), 2);
    assert_eq!(fs::read_dir(&archive.checkpoints).unwrap().count(), 0);
    drop(reader);
    let compacted = worker.join().unwrap();
    assert_eq!(compacted.retention_floor(), &key);
    assert_eq!(compacted.checkpoint_digest(), checkpoint_digest);
    assert_eq!(compacted.pruned_entries(), 2);
    assert_eq!(compacted.pruned_bytes(), record_bytes);
    assert_eq!(compacted.generation(), fence.expected_archive_generation());
    assert!(authority.latest.lock().unwrap().is_some());
    assert_eq!(fs::read_dir(&archive.records).unwrap().count(), 0);
    assert_eq!(fs::read_dir(&archive.checkpoints).unwrap().count(), 1);
    let qualification = archive
        .qualify_against_certified_tip(&chain.state().view(), chain.kura(), 0)
        .unwrap();
    assert_eq!(qualification.activation_floor(), &key);
    assert_eq!(qualification.archive_tip(), &key);
    assert_eq!(qualification.kura_tip_height(), key.height);
    assert_eq!(qualification.lag_blocks(), 0);
    assert_eq!(qualification.generation(), compacted.generation());
}

#[test]
fn certified_capture_index_poison_is_permanent_storage_refusal_without_writes() {
    let directory = physical_tempdir().unwrap();
    let chain = chain();
    let archive =
        ProviderIngestFinalizedArchiveV1::try_open(archive_root(&directory), bounds()).unwrap();
    assert!(
        std::panic::catch_unwind(AssertUnwindSafe(|| {
            let _writer = archive.index.write().unwrap();
            panic!("poison original archive writer");
        }))
        .is_err()
    );
    for _ in 0..2 {
        assert!(matches!(
            archive.capture_certified_view(&chain.state().view(), chain.kura()),
            Err(ProviderIngestFinalizedArchiveErrorV1::ArchiveLockPoisoned)
        ));
        assert_eq!(fs::read_dir(&archive.records).unwrap().count(), 0);
    }
}

#[test]
fn certified_retention_requires_exact_result_identity_and_survives_reopen() {
    let directory = physical_tempdir().unwrap();
    let root = archive_root(&directory);
    let mut chain = chain();
    let archive = ProviderIngestFinalizedArchiveV1::try_open(&root, bounds()).unwrap();
    archive
        .capture_certified_view(&chain.state().view(), chain.kura())
        .unwrap();
    chain.commit_at(2_000, Vec::new());
    archive
        .capture_certified_view(&chain.state().view(), chain.kura())
        .unwrap();
    let view = chain.state().view();
    let key = authenticate_capture_view(&view, chain.kura()).unwrap();
    let block = chain.committed(2);
    let mut fence = ProviderIngestFinalizedArchiveRetentionFenceV1::try_new(
        key,
        *block.id().0.as_ref(),
        archive.health_generation().unwrap(),
    )
    .unwrap();
    let valid_id = fence.certified_block_id;
    fence.certified_block_id = [0xA9; 32];
    assert!(
        archive
            .prepare_certified_compaction(&fence, &view, chain.kura())
            .is_err()
    );
    fence.certified_block_id = valid_id;
    let proposal = archive
        .prepare_certified_compaction(&fence, &view, chain.kura())
        .unwrap();
    let authority = TestRetentionAuthority::new();
    let binding = authority.binding();
    let compacted = archive
        .approve_and_install_certified_compaction(
            &proposal,
            &view,
            chain.kura(),
            &binding,
            &authority,
        )
        .unwrap();
    assert_eq!(compacted.retention_floor(), &key);
    assert_eq!(compacted.checkpoint_digest(), proposal.checkpoint_digest());
    assert_eq!(compacted.pruned_entries(), 2);
    assert!(compacted.pruned_bytes() > 0);
    assert_eq!(compacted.generation(), fence.expected_archive_generation());
    drop(archive);
    let reopened = ProviderIngestFinalizedArchiveV1::try_open_with_retention_authority(
        &root,
        bounds(),
        &chain.network_id(),
        &view,
        chain.kura(),
        &binding,
        &authority,
    )
    .unwrap();
    let qualification = reopened
        .qualify_against_certified_tip(&view, chain.kura(), 0)
        .unwrap();
    assert_eq!(qualification.activation_floor(), &key);
    assert_eq!(qualification.archive_tip(), &key);
    assert_eq!(qualification.kura_tip_height(), key.height);
    assert_eq!(qualification.lag_blocks(), 0);
    assert_eq!(qualification.generation(), compacted.generation());
}

#[test]
fn qualification_releases_only_certificate_walk_and_detects_concurrent_capture() {
    let directory = physical_tempdir().unwrap();
    let mut chain = chain();
    let archive =
        ProviderIngestFinalizedArchiveV1::try_open(archive_root(&directory), bounds()).unwrap();
    archive
        .capture_certified_view(&chain.state().view(), chain.kura())
        .unwrap();
    chain.commit_at(2_000, Vec::new());
    let state = Arc::clone(chain.state());
    let view = state.view();
    let original_generation = archive.health_generation().unwrap();
    archive
        .with_index_reader_for_testing(|| {
            assert!(matches!(
                archive.capture_certified_view(&view, state.kura()),
                Err(ProviderIngestFinalizedArchiveErrorV1::IndexBusy { .. })
            ));
        })
        .unwrap();
    let result =
        archive.qualify_against_certified_tip_observed(&view, state.kura(), 1, |released| {
            assert!(released);
            std::thread::scope(|scope| {
                scope
                    .spawn(|| archive.capture_certified_view(&state.view(), state.kura()))
                    .join()
                    .unwrap()
                    .map(|outcome| {
                        assert_eq!(
                            outcome,
                            ProviderIngestFinalizedArchiveInsertOutcomeV1::Inserted
                        );
                    })
            })
        });
    assert!(matches!(
        result,
        Err(
            ProviderIngestFinalizedArchiveErrorV1::QualificationBoundaryChanged {
                boundary: "archive"
            }
        )
    ));
    assert_eq!(
        archive.health_generation().unwrap(),
        original_generation + 1
    );
    let current = archive
        .qualify_against_certified_tip(&view, state.kura(), 0)
        .unwrap();
    assert_eq!(current.archive_tip().height, 2);
    assert_eq!(current.generation(), original_generation + 1);

    let wide = DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, usize::MAX, 128);
    let active = norito::core::with_decode_limits_scope(wide, || {
        archive.qualify_against_certified_tip_observed(&view, state.kura(), 0, |released| {
            assert!(!released, "active decode scopes keep the physical reader");
            assert!(matches!(
                archive.capture_certified_view(&view, state.kura()),
                Err(ProviderIngestFinalizedArchiveErrorV1::IndexBusy { .. })
            ));
            Ok(())
        })
    })
    .unwrap();
    assert_eq!(active, current);
    for allowance in [0, 1] {
        let limits = DecodeLimits::new(usize::MAX, usize::MAX, allowance, allowance, 128);
        assert!(
            norito::core::with_decode_limits_scope(limits, || {
                archive.qualify_against_certified_tip(&view, state.kura(), 0)
            })
            .is_err()
        );
    }
    assert_eq!(archive.health_generation().unwrap(), current.generation());
}

#[test]
fn qualification_detects_same_generation_compaction_before_returning_authority() {
    let directory = physical_tempdir().unwrap();
    let mut chain = chain();
    let archive =
        ProviderIngestFinalizedArchiveV1::try_open(archive_root(&directory), bounds()).unwrap();
    archive
        .capture_certified_view(&chain.state().view(), chain.kura())
        .unwrap();
    chain.commit_at(2_000, Vec::new());
    archive
        .capture_certified_view(&chain.state().view(), chain.kura())
        .unwrap();
    let state = Arc::clone(chain.state());
    let view = state.view();
    let key = authenticate_capture_view(&view, state.kura()).unwrap();
    let generation = archive.health_generation().unwrap();
    let fence = ProviderIngestFinalizedArchiveRetentionFenceV1::try_new(
        key,
        *chain.committed(2).id().0.as_ref(),
        generation,
    )
    .unwrap();
    let proposal = archive
        .prepare_certified_compaction(&fence, &view, state.kura())
        .unwrap();
    let authority = TestRetentionAuthority::new();
    let result =
        archive.qualify_against_certified_tip_observed(&view, state.kura(), 0, |released| {
            assert!(released);
            std::thread::scope(|scope| {
                scope
                    .spawn(|| {
                        archive.approve_and_install_certified_compaction(
                            &proposal,
                            &state.view(),
                            state.kura(),
                            &authority.binding(),
                            &authority,
                        )
                    })
                    .join()
                    .unwrap()
                    .map(|compacted| {
                        assert_eq!(
                            compacted.generation(),
                            generation,
                            "compaction preserves this counter"
                        );
                        assert_eq!(compacted.pruned_entries(), 2);
                    })
            })
        });
    assert!(matches!(
        result,
        Err(
            ProviderIngestFinalizedArchiveErrorV1::QualificationBoundaryChanged {
                boundary: "archive"
            }
        )
    ));
    assert_eq!(archive.health_generation().unwrap(), generation);
    let current = archive
        .qualify_against_certified_tip(&view, state.kura(), 0)
        .unwrap();
    assert_eq!(current.activation_floor(), &key);
    assert_eq!(current.archive_tip(), &key);

    // The handoff never replaces the original physical checkpoint observation.
    let path = archive
        .read_index()
        .unwrap()
        .virtual_bases
        .get(view.network_id())
        .unwrap()
        .path
        .clone();
    let original = fs::read(&path).unwrap();
    fs::write(&path, [0_u8]).unwrap();
    assert!(
        archive
            .qualify_against_certified_tip(&view, state.kura(), 0)
            .is_err()
    );
    fs::write(&path, original).unwrap();
    assert_eq!(
        archive
            .qualify_against_certified_tip(&view, state.kura(), 0)
            .unwrap(),
        current
    );
    drop(view);
    chain.corrupt_local_quorum_for_test(2, Signers::BelowQuorum);
    assert!(
        archive
            .qualify_against_certified_tip(&state.view(), state.kura(), 0)
            .is_err(),
        "the compact source inventory never authenticates a corrupted native certificate"
    );
}

#[test]
fn qualification_closes_original_sources_on_physical_and_ordinary_errors() {
    let directory = physical_tempdir().unwrap();
    let mut chain = chain();
    let archive =
        ProviderIngestFinalizedArchiveV1::try_open(archive_root(&directory), bounds()).unwrap();
    archive
        .capture_certified_view(&chain.state().view(), chain.kura())
        .unwrap();
    let state = Arc::clone(chain.state());
    let view = state.view();
    let key = authenticate_capture_view(&view, state.kura()).unwrap();
    let path = archive.record_path(&key).unwrap();
    let original = fs::read(&path).unwrap();
    fs::write(&path, [0_u8]).unwrap();
    assert!(matches!(
        archive.qualify_against_certified_tip(&view, state.kura(), 0),
        Err(ProviderIngestFinalizedArchiveErrorV1::InvalidStorage { .. })
    ));
    fs::write(&path, original).unwrap();
    let ordinary = || ProviderIngestFinalizedArchiveErrorV1::ArchiveUnavailable {
        reason: "ordinary observer refusal",
    };
    assert!(matches!(
        archive.qualify_against_certified_tip_observed(&view, state.kura(), 0, |_| Err(ordinary())),
        Err(ProviderIngestFinalizedArchiveErrorV1::ArchiveUnavailable {
            reason: "ordinary observer refusal"
        })
    ));

    let moved = archive.root.join("original-records");
    let result =
        archive.qualify_against_certified_tip_observed(&view, state.kura(), 0, |released| {
            assert!(released);
            fs::rename(&archive.records, &moved).unwrap();
            fs::create_dir(&archive.records).unwrap();
            Err(ordinary())
        });
    assert!(
        matches!(result, Err(ProviderIngestFinalizedArchiveErrorV1::Read { path, .. }) if path == archive.records),
        "original native source refusal must supersede ordinary error"
    );
    fs::remove_dir(&archive.records).unwrap();
    fs::rename(&moved, &archive.records).unwrap();
    let qualification = archive
        .qualify_against_certified_tip(&view, state.kura(), 0)
        .unwrap();
    assert_eq!(qualification.archive_tip(), &key);
    assert_eq!(qualification.kura_tip_height(), key.height);
    assert_eq!(qualification.lag_blocks(), 0);

    // Advance the genuine Kura source after selection; its exit also closes an error.
    let result =
        archive.qualify_against_certified_tip_observed(&view, state.kura(), 0, |released| {
            assert!(released);
            chain.commit_at(2_000, Vec::new());
            Err(ordinary())
        });
    assert!(matches!(
        result,
        Err(
            ProviderIngestFinalizedArchiveErrorV1::QualificationBoundaryChanged {
                boundary: "certified Kura"
            }
        )
    ));
}

#[test]
fn qualification_inventory_is_bounded_and_tracks_every_original_network_source() {
    let directory = physical_tempdir().unwrap();
    let mut first = chain();
    let second = CertifiedTestChain::start(TestChainConfig::new(World::new(), 3_000)).unwrap();
    assert_ne!(first.network_id(), second.network_id());
    let archive =
        ProviderIngestFinalizedArchiveV1::try_open(archive_root(&directory), bounds()).unwrap();
    archive
        .capture_certified_view(&first.state().view(), first.kura())
        .unwrap();
    archive
        .capture_certified_view(&second.state().view(), second.kura())
        .unwrap();
    let index = archive.read_index().unwrap();
    let inventory = QualificationInventory::capture(&index, bounds()).unwrap();
    assert_eq!(inventory.records.len(), 2);
    inventory.require_same(&index).unwrap();
    let mut different_path = QualificationInventory::capture(&index, bounds()).unwrap();
    different_path.records[1].path.push("foreign-record");
    assert!(different_path.require_same(&index).is_err());
    let mut different_digest = QualificationInventory::capture(&index, bounds()).unwrap();
    different_digest.records[1].digest[0] ^= 1;
    assert!(different_digest.require_same(&index).is_err());
    let tiny = ProviderIngestFinalizedArchiveBoundsV1::try_new(
        2 * 1024 * 1024,
        1,
        32 * 1024 * 1024,
        16,
        16,
        64,
        1,
    )
    .unwrap();
    assert!(matches!(
        QualificationInventory::capture(&index, tiny),
        Err(
            ProviderIngestFinalizedArchiveErrorV1::ArchiveCapacityExceeded {
                observed: 2,
                maximum: 1
            }
        )
    ));
    drop(index);
    // A genuine append for another network invalidates the same inventory too.
    first.commit_at(2_000, Vec::new());
    archive
        .capture_certified_view(&first.state().view(), first.kura())
        .unwrap();
    assert!(matches!(
        inventory.require_same(&archive.read_index().unwrap()),
        Err(
            ProviderIngestFinalizedArchiveErrorV1::QualificationBoundaryChanged {
                boundary: "archive"
            }
        )
    ));
    let qualification = archive
        .qualify_against_certified_tip(&second.state().view(), second.kura(), 0)
        .unwrap();
    assert_eq!(
        qualification.archive_tip(),
        qualification.activation_floor()
    );
    assert_eq!(qualification.archive_tip().height, second.height());
    assert_eq!(qualification.kura_tip_height(), second.height());
    assert_eq!(qualification.lag_blocks(), 0);
}

// Capture this chain's tip, commit `times` more blocks, and capture each of them.
fn capture_through(
    archive: &ProviderIngestFinalizedArchiveV1,
    chain: &mut CertifiedTestChain,
    times: &[u64],
) {
    archive
        .capture_certified_view(&chain.state().view(), chain.kura())
        .unwrap();
    for time in times {
        chain.commit_at(*time, Vec::new());
        archive
            .capture_certified_view(&chain.state().view(), chain.kura())
            .unwrap();
    }
}

#[test]
fn certified_capture_resumes_and_publishes_the_same_bytes_as_a_fresh_walk() {
    let directory = physical_tempdir().unwrap();
    let fresh_directory = physical_tempdir().unwrap();
    let fresh_root = archive_root(&fresh_directory);
    let mut chain = chain();
    let archive =
        ProviderIngestFinalizedArchiveV1::try_open(archive_root(&directory), bounds()).unwrap();
    for time in [None, Some(2_000), Some(3_000), Some(4_000), Some(5_000)] {
        if let Some(time) = time {
            chain.commit_at(time, Vec::new());
        }
        archive
            .capture_certified_view(&chain.state().view(), chain.kura())
            .unwrap();
        // Reopening discards every retained prefix, so this one walks from signed genesis.
        ProviderIngestFinalizedArchiveV1::try_open(&fresh_root, bounds())
            .unwrap()
            .capture_certified_view(&chain.state().view(), chain.kura())
            .unwrap();
    }
    assert_eq!(
        archive.authentication.retained_heights_for_testing(),
        [4, 5]
    );
    let fresh = ProviderIngestFinalizedArchiveV1::try_open(&fresh_root, bounds()).unwrap();
    assert_eq!(
        archive_namespace_snapshot(&archive.records),
        archive_namespace_snapshot(&fresh.records)
    );
}

#[test]
fn certified_capture_survives_a_failed_publication_without_repeating_the_walk() {
    let directory = physical_tempdir().unwrap();
    let mut chain = chain();
    let archive =
        ProviderIngestFinalizedArchiveV1::try_open(archive_root(&directory), bounds()).unwrap();
    capture_through(&archive, &mut chain, &[2_000, 3_000]);
    assert_eq!(
        archive.authentication.retained_heights_for_testing(),
        [2, 3]
    );
    chain.commit_at(4_000, Vec::new());
    let reader = archive.read_index().unwrap();
    assert!(matches!(
        archive.capture_certified_view(&chain.state().view(), chain.kura()),
        Err(ProviderIngestFinalizedArchiveErrorV1::IndexBusy { .. })
    ));
    drop(reader);
    assert_eq!(
        archive.authentication.retained_heights_for_testing(),
        [3, 4]
    );
    // Height 2 is now outside every retained point. A retry that walked would reject it.
    chain.corrupt_local_quorum_for_test(2, Signers::BelowQuorum);
    assert_eq!(
        archive
            .capture_certified_view(&chain.state().view(), chain.kura())
            .unwrap(),
        ProviderIngestFinalizedArchiveInsertOutcomeV1::Inserted
    );
    assert_eq!(
        archive
            .capture_certified_view(&chain.state().view(), chain.kura())
            .unwrap(),
        ProviderIngestFinalizedArchiveInsertOutcomeV1::ExactReplay,
        "a same-height retry verifies again from the retained predecessor"
    );
}

#[test]
fn qualification_audit_finds_a_changed_history_frame_that_capture_does_not_read() {
    let directory = physical_tempdir().unwrap();
    let mut chain = chain();
    let archive =
        ProviderIngestFinalizedArchiveV1::try_open(archive_root(&directory), bounds()).unwrap();
    capture_through(&archive, &mut chain, &[2_000, 3_000, 4_000]);
    let first = archive
        .qualify_against_certified_tip(&chain.state().view(), chain.kura(), 0)
        .unwrap();
    assert_eq!(first.archive_tip().height, 4);

    chain.corrupt_local_quorum_for_test(2, Signers::BelowQuorum);
    chain.commit_at(5_000, Vec::new());
    assert_eq!(
        archive
            .capture_certified_view(&chain.state().view(), chain.kura())
            .unwrap(),
        ProviderIngestFinalizedArchiveInsertOutcomeV1::Inserted
    );
    let proved = archive
        .qualify_against_certified_tip(&chain.state().view(), chain.kura(), 0)
        .unwrap();
    assert_eq!(proved.archive_tip().height, 5);

    archive.authentication.expire_audit_for_testing();
    for _ in 0..2 {
        assert!(
            archive
                .qualify_against_certified_tip(&chain.state().view(), chain.kura(), 0)
                .is_err(),
            "a due audit reaches the frame, then readers keep refusing from a fresh walk"
        );
    }
    // Readers fail closed, but commits do not stop: capture still resumes above the fault.
    chain.commit_at(6_000, Vec::new());
    assert_eq!(
        archive
            .capture_certified_view(&chain.state().view(), chain.kura())
            .unwrap(),
        ProviderIngestFinalizedArchiveInsertOutcomeV1::Inserted
    );
}

#[test]
fn qualification_on_a_stale_view_retries_without_retiring_capture_points() {
    let directory = physical_tempdir().unwrap();
    let mut chain = chain();
    let archive =
        ProviderIngestFinalizedArchiveV1::try_open(archive_root(&directory), bounds()).unwrap();
    capture_through(&archive, &mut chain, &[2_000, 3_000]);
    let _ = archive
        .qualify_against_certified_tip(&chain.state().view(), chain.kura(), 0)
        .unwrap();
    let state = Arc::clone(chain.state());
    let stale = state.view();
    chain.commit_at(4_000, Vec::new());
    archive
        .capture_certified_view(&state.view(), state.kura())
        .unwrap();
    assert_eq!(
        archive.authentication.retained_heights_for_testing(),
        [3, 4]
    );
    assert!(matches!(
        archive.qualify_against_certified_tip(&stale, state.kura(), 8),
        Err(ProviderIngestFinalizedArchiveErrorV1::QualificationBoundaryChanged { .. })
    ));
    assert_eq!(
        archive.authentication.retained_heights_for_testing(),
        [3, 4],
        "a reader that lost a race must not push the next commit back to a walk"
    );
    drop(stale);
    let current = archive
        .qualify_against_certified_tip(&state.view(), state.kura(), 0)
        .unwrap();
    assert_eq!(current.archive_tip().height, 4);
}

#[test]
fn a_full_walk_overtaken_by_captures_still_establishes_coverage() {
    let directory = physical_tempdir().unwrap();
    let mut chain = chain();
    let archive =
        ProviderIngestFinalizedArchiveV1::try_open(archive_root(&directory), bounds()).unwrap();
    capture_through(&archive, &mut chain, &[2_000, 3_000]);
    let state = Arc::clone(chain.state());
    let view = state.view();
    let result =
        archive.qualify_against_certified_tip_observed(&view, state.kura(), 8, |released| {
            assert!(released);
            // A commit and its capture land after selection, as they do every few seconds
            // while a long walk runs.
            chain.commit_at(4_000, Vec::new());
            archive
                .capture_certified_view(&state.view(), state.kura())
                .map(drop)
        });
    assert!(matches!(
        result,
        Err(ProviderIngestFinalizedArchiveErrorV1::QualificationBoundaryChanged { .. })
    ));
    drop(view);
    // Coverage reached the capture's height, so this does not walk: a frame below every
    // retained point has changed, and a walk would reject it.
    chain.corrupt_local_quorum_for_test(2, Signers::BelowQuorum);
    let current = archive
        .qualify_against_certified_tip(&state.view(), state.kura(), 0)
        .unwrap();
    assert_eq!(current.archive_tip().height, 4);
}

#[test]
fn qualification_rechecks_the_retained_frame_of_proved_coverage() {
    let directory = physical_tempdir().unwrap();
    let mut chain = chain();
    let archive =
        ProviderIngestFinalizedArchiveV1::try_open(archive_root(&directory), bounds()).unwrap();
    capture_through(&archive, &mut chain, &[2_000, 3_000]);
    let _ = archive
        .qualify_against_certified_tip(&chain.state().view(), chain.kura(), 0)
        .unwrap();
    chain.commit_at(4_000, Vec::new());
    chain.corrupt_local_quorum_for_test(3, Signers::BelowQuorum);
    assert!(
        archive
            .qualify_against_certified_tip(&chain.state().view(), chain.kura(), 1)
            .is_err(),
        "the archive tip's changed certificate is visible on the very next tick"
    );
    assert_eq!(
        archive.authentication.retained_heights_for_testing(),
        [2, 3],
        "a reader's recheck retires nothing"
    );
    assert!(
        archive
            .capture_certified_view(&chain.state().view(), chain.kura())
            .is_err(),
        "the commit path's own recheck refuses the changed frame"
    );
    assert!(
        archive
            .authentication
            .retained_heights_for_testing()
            .is_empty()
    );
}

#[test]
fn reopening_lets_the_next_commit_continue_from_the_opening_walk() {
    let directory = physical_tempdir().unwrap();
    let root = archive_root(&directory);
    let mut chain = chain();
    let archive = ProviderIngestFinalizedArchiveV1::try_open(&root, bounds()).unwrap();
    capture_through(&archive, &mut chain, &[2_000, 3_000]);
    drop(archive);
    let reopened = ProviderIngestFinalizedArchiveV1::try_open(&root, bounds()).unwrap();
    let older = {
        let index = reopened.read_index().unwrap();
        let tip = index.by_height.get(&(chain.network_id(), 3)).unwrap();
        assert!(index.tip_walk(tip).is_some());
        index
            .by_height
            .get(&(chain.network_id(), 2))
            .unwrap()
            .path
            .clone()
    };
    // A record below the tip changes after opening. Only a whole-history insertion reads it.
    fs::write(&older, [0_u8]).unwrap();
    chain.commit_at(4_000, Vec::new());
    assert_eq!(
        reopened
            .capture_certified_view(&chain.state().view(), chain.kura())
            .unwrap(),
        ProviderIngestFinalizedArchiveInsertOutcomeV1::Inserted
    );
}

#[test]
fn archive_insertion_reads_only_the_tip_record_and_the_audit_reads_the_rest() {
    let directory = physical_tempdir().unwrap();
    let root = archive_root(&directory);
    let mut chain = chain();
    let archive = ProviderIngestFinalizedArchiveV1::try_open(&root, bounds()).unwrap();
    capture_through(&archive, &mut chain, &[2_000, 3_000]);
    let _ = archive
        .qualify_against_certified_tip(&chain.state().view(), chain.kura(), 0)
        .unwrap();
    let older = archive
        .read_index()
        .unwrap()
        .by_height
        .get(&(chain.network_id(), 2))
        .unwrap()
        .path
        .clone();
    let original = fs::read(&older).unwrap();
    fs::write(&older, [0_u8]).unwrap();

    chain.commit_at(4_000, Vec::new());
    assert_eq!(
        archive
            .capture_certified_view(&chain.state().view(), chain.kura())
            .unwrap(),
        ProviderIngestFinalizedArchiveInsertOutcomeV1::Inserted
    );
    let _ = archive
        .qualify_against_certified_tip(&chain.state().view(), chain.kura(), 0)
        .unwrap();

    archive.authentication.expire_audit_for_testing();
    assert!(matches!(
        archive.qualify_against_certified_tip(&chain.state().view(), chain.kura(), 0),
        Err(ProviderIngestFinalizedArchiveErrorV1::InvalidStorage { .. })
    ));
    drop(archive);
    assert!(
        ProviderIngestFinalizedArchiveV1::try_open(&root, bounds()).is_err(),
        "reopening reads every retained record"
    );
    fs::write(&older, original).unwrap();
    let reopened = ProviderIngestFinalizedArchiveV1::try_open(&root, bounds()).unwrap();
    assert_eq!(
        reopened
            .qualify_against_certified_tip(&chain.state().view(), chain.kura(), 0)
            .unwrap()
            .archive_tip()
            .height,
        4
    );
}

#[test]
fn archive_insertion_still_refuses_a_changed_tip_record() {
    let directory = physical_tempdir().unwrap();
    let mut chain = chain();
    let archive =
        ProviderIngestFinalizedArchiveV1::try_open(archive_root(&directory), bounds()).unwrap();
    capture_through(&archive, &mut chain, &[2_000]);
    let tip = archive
        .read_index()
        .unwrap()
        .by_height
        .get(&(chain.network_id(), 2))
        .unwrap()
        .path
        .clone();
    fs::write(&tip, [0_u8]).unwrap();
    chain.commit_at(3_000, Vec::new());
    assert!(
        archive
            .capture_certified_view(&chain.state().view(), chain.kura())
            .is_err()
    );
}

#[test]
fn archive_tip_walk_matches_the_complete_walk_after_each_insertion() {
    let directory = physical_tempdir().unwrap();
    let archive =
        ProviderIngestFinalizedArchiveV1::try_open(archive_root(&directory), bounds()).unwrap();
    let mut current = projection(7);
    let network_id = current.key.network_id;
    archive.insert(current.clone()).unwrap();
    for height in 8..12 {
        current = advance_projection(&current, height);
        archive.insert(current.clone()).unwrap();
        let index = archive.read_index().unwrap();
        let mut complete = ProviderCoverageWalkV1::start(None).unwrap();
        for (_, entry) in &index.by_height {
            complete
                .step(&index, network_id, entry, false, bounds())
                .unwrap();
        }
        assert_eq!(
            format!("{:?}", index.tip_walks.get(&network_id).unwrap()),
            format!("{complete:?}"),
            "one retained step equals replaying every record"
        );
        assert_eq!(
            tip_or_reconstructed_projection(&index, &current.key, bounds()).unwrap(),
            reconstruct_projection(&index, &current.key, bounds()).unwrap()
        );
    }
}

#[test]
fn archive_tip_walk_is_retired_by_compaction_of_its_virtual_base() {
    let directory = physical_tempdir().unwrap();
    let mut chain = chain();
    let archive =
        ProviderIngestFinalizedArchiveV1::try_open(archive_root(&directory), bounds()).unwrap();
    capture_through(&archive, &mut chain, &[2_000]);
    let view = chain.state().view();
    let key = authenticate_capture_view(&view, chain.kura()).unwrap();
    let fence = ProviderIngestFinalizedArchiveRetentionFenceV1::try_new(
        key,
        *chain.committed(2).id().0.as_ref(),
        archive.health_generation().unwrap(),
    )
    .unwrap();
    let proposal = archive
        .prepare_certified_compaction(&fence, &view, chain.kura())
        .unwrap();
    let authority = TestRetentionAuthority::new();
    let _ = archive
        .approve_and_install_certified_compaction(
            &proposal,
            &view,
            chain.kura(),
            &authority.binding(),
            &authority,
        )
        .unwrap();
    drop(view);
    chain.commit_at(3_000, Vec::new());
    assert_eq!(
        archive
            .capture_certified_view(&chain.state().view(), chain.kura())
            .unwrap(),
        ProviderIngestFinalizedArchiveInsertOutcomeV1::Inserted
    );
    chain.commit_at(4_000, Vec::new());
    archive
        .capture_certified_view(&chain.state().view(), chain.kura())
        .unwrap();
    let qualification = archive
        .qualify_against_certified_tip(&chain.state().view(), chain.kura(), 0)
        .unwrap();
    assert_eq!(qualification.activation_floor(), &key);
    assert_eq!(qualification.archive_tip().height, 4);
}

#[test]
fn provider_page_at_the_tip_reads_only_the_tip_record() {
    let directory = physical_tempdir().unwrap();
    let archive =
        ProviderIngestFinalizedArchiveV1::try_open(archive_root(&directory), bounds()).unwrap();
    let first = projection(7);
    archive.insert(first.clone()).unwrap();
    let second = advance_projection(&first, 8);
    archive.insert(second.clone()).unwrap();
    let tip = advance_projection(&second, 9);
    archive.insert(tip.clone()).unwrap();
    let before = format!(
        "{:?}",
        archive
            .read_provider_page(&tip.key, PROVIDER_A, None, 1)
            .unwrap()
    );
    // The replayed answer at the same key, for comparison with the retained walk's.
    let replayed = reconstruct_provider_projection(
        &ArchiveIndexV1 {
            by_height: archive.read_index().unwrap().by_height.clone(),
            ..ArchiveIndexV1::default()
        },
        &tip.key,
        PROVIDER_A,
        bounds(),
    )
    .unwrap();
    assert_eq!(
        reconstruct_provider_projection(
            &archive.read_index().unwrap(),
            &tip.key,
            PROVIDER_A,
            bounds()
        )
        .unwrap(),
        replayed
    );
    fs::write(archive.record_path(&first.key).unwrap(), [0_u8]).unwrap();
    assert_eq!(
        format!(
            "{:?}",
            archive
                .read_provider_page(&tip.key, PROVIDER_A, None, 1)
                .unwrap()
        ),
        before,
        "a worker tick at the tip does not read older records"
    );
    assert!(
        archive
            .read_provider_page(&second.key, PROVIDER_A, None, 1)
            .is_err(),
        "an older key still replays its history and fails closed"
    );
    fs::write(archive.record_path(&tip.key).unwrap(), [0_u8]).unwrap();
    assert!(
        archive
            .read_provider_page(&tip.key, PROVIDER_A, None, 1)
            .is_err(),
        "the tip's own record is still read"
    );
}
