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
