//! Production retention over genuine commit certificates: bounded, off the commit path,
//! restart-safe, and never below history that something still reads.

use super::*;
use crate::{
    state::World,
    sumeragi::test_chain::{CertifiedTestChain, TestChainConfig},
};
use std::{
    sync::atomic::{AtomicUsize, Ordering},
    sync::mpsc,
    time::Duration,
};

/// Lets one step finish its prefix, unless a test is studying how the work is divided.
const ONE_CALL: RetentionLimitsV1 = RetentionLimitsV1 {
    max_prefix_entries: 1_024,
    max_prefix_bytes: u64::MAX,
};

fn chain() -> CertifiedTestChain {
    CertifiedTestChain::start(TestChainConfig::new(World::new(), 1_000)).expect("signed genesis")
}

fn small_bounds(max_archive_entries: usize) -> ProviderIngestFinalizedArchiveBoundsV1 {
    ProviderIngestFinalizedArchiveBoundsV1::try_new(
        2 * 1024 * 1024,
        max_archive_entries,
        32 * 1024 * 1024,
        16,
        16,
        64,
        1,
    )
    .unwrap()
}

fn capture_tip(archive: &ProviderIngestFinalizedArchiveV1, chain: &CertifiedTestChain) {
    archive
        .capture_certified_view(&chain.state().view(), chain.kura())
        .unwrap();
}

fn commit_and_capture(
    archive: &ProviderIngestFinalizedArchiveV1,
    chain: &mut CertifiedTestChain,
    blocks: usize,
) {
    for _ in 0..blocks {
        chain.commit(Vec::new());
        capture_tip(archive, chain);
    }
}

/// What qualification does on every worker tick: walk from signed genesis once, then stand proved.
fn prove(archive: &ProviderIngestFinalizedArchiveV1, chain: &CertifiedTestChain) {
    let _ = archive
        .qualify_against_certified_tip(&chain.state().view(), chain.kura(), u64::MAX)
        .unwrap();
}

fn step(
    archive: &ProviderIngestFinalizedArchiveV1,
    chain: &CertifiedTestChain,
    authority: &TestRetentionAuthority,
    pinned_heights: &[u64],
) -> Result<ProviderIngestFinalizedArchiveRetentionStepV1, ProviderIngestFinalizedArchiveErrorV1> {
    prove(archive, chain);
    archive.reconcile_retention_with_limits(
        &chain.state().query_view(),
        chain.kura(),
        &authority.binding(),
        authority,
        pinned_heights,
        ONE_CALL,
    )
}

fn plan(
    archive: &ProviderIngestFinalizedArchiveV1,
    network_id: &NetworkId,
    continuing: bool,
    pinned_heights: &[u64],
    limits: RetentionLimitsV1,
) -> RetentionPlanV1 {
    plan_retention(
        &archive.read_index().unwrap(),
        network_id,
        archive.bounds,
        continuing,
        pinned_heights,
        limits,
    )
}

fn target_height(plan: RetentionPlanV1) -> u64 {
    match plan {
        RetentionPlanV1::Compact { target, .. } => target.height,
        other => panic!("expected a compaction, found {other:?}"),
    }
}

fn floor_height(archive: &ProviderIngestFinalizedArchiveV1, network_id: &NetworkId) -> u64 {
    archive
        .activation_floor(network_id)
        .unwrap()
        .unwrap()
        .height
}

#[test]
fn plan_starts_at_the_high_watermark_and_stops_at_the_low_one() {
    let directory = physical_tempdir().unwrap();
    let archive =
        ProviderIngestFinalizedArchiveV1::try_open(archive_root(&directory), small_bounds(8))
            .unwrap();
    let mut current = projection(7);
    let network_id = current.key.network_id;
    archive.insert(current.clone()).unwrap();
    for height in 8..12 {
        current = advance_projection(&current, height);
        archive.insert(current.clone()).unwrap();
    }
    // Five of eight: below three quarters, so nothing starts.
    assert_eq!(
        plan(&archive, &network_id, false, &[], ONE_CALL),
        RetentionPlanV1::Idle
    );
    // A running cycle continues down to half. The new base counts: 5 + 1 - 2 = 4.
    assert_eq!(
        target_height(plan(&archive, &network_id, true, &[], ONE_CALL)),
        8
    );
    current = advance_projection(&current, 12);
    archive.insert(current.clone()).unwrap();
    // Six of eight starts a cycle: 6 + 1 - 3 = 4.
    assert_eq!(
        target_height(plan(&archive, &network_id, false, &[], ONE_CALL)),
        9
    );
    // One step never takes more than its limit, and then the cycle has more to do.
    let limited = RetentionLimitsV1 {
        max_prefix_entries: 2,
        ..ONE_CALL
    };
    assert!(matches!(
        plan(&archive, &network_id, false, &[], limited),
        RetentionPlanV1::Compact {
            target,
            completes_cycle: false,
        } if target.height == 8
    ));
    assert!(matches!(
        plan(&archive, &network_id, false, &[], ONE_CALL),
        RetentionPlanV1::Compact {
            target,
            completes_cycle: true,
        } if target.height == 9
    ));
}

#[test]
fn plan_keeps_every_pin_resolvable_and_ignores_history_already_gone() {
    let directory = physical_tempdir().unwrap();
    let archive =
        ProviderIngestFinalizedArchiveV1::try_open(archive_root(&directory), small_bounds(8))
            .unwrap();
    let mut current = projection(7);
    let network_id = current.key.network_id;
    archive.insert(current.clone()).unwrap();
    for height in 8..13 {
        current = advance_projection(&current, height);
        archive.insert(current.clone()).unwrap();
    }
    let compact = |pins: &[u64]| target_height(plan(&archive, &network_id, false, pins, ONE_CALL));
    assert_eq!(
        compact(&[8]),
        8,
        "the floor may stop on a pin, never pass it"
    );
    assert_eq!(
        compact(&[7]),
        7,
        "a floor at the activation record is allowed"
    );
    assert_eq!(compact(&[8, 11]), 8, "the oldest pin decides");
    assert_eq!(compact(&[6]), 9, "history below the floor is already gone");
    assert_eq!(compact(&[6, 8]), 8, "a dead pin does not hide a live one");
    assert_eq!(compact(&[12]), 9, "a pin above the target changes nothing");
}

#[test]
fn plan_never_takes_the_tip_and_refuses_bounds_too_small_to_hold_a_base_and_a_tip() {
    let directory = physical_tempdir().unwrap();
    let archive =
        ProviderIngestFinalizedArchiveV1::try_open(archive_root(&directory), small_bounds(4))
            .unwrap();
    let mut current = projection(7);
    let network_id = current.key.network_id;
    archive.insert(current.clone()).unwrap();
    for height in 8..10 {
        current = advance_projection(&current, height);
        archive.insert(current.clone()).unwrap();
    }
    assert_eq!(
        target_height(plan(&archive, &network_id, false, &[], ONE_CALL)),
        8,
        "a base and the tip record remain"
    );
    let directory = physical_tempdir().unwrap();
    let tiny =
        ProviderIngestFinalizedArchiveV1::try_open(archive_root(&directory), small_bounds(3))
            .unwrap();
    let mut current = projection(7);
    tiny.insert(current.clone()).unwrap();
    for height in 8..10 {
        current = advance_projection(&current, height);
        tiny.insert(current.clone()).unwrap();
    }
    assert_eq!(
        plan(&tiny, &network_id, false, &[], ONE_CALL),
        RetentionPlanV1::Idle
    );
}

#[test]
fn plan_watches_the_byte_bound_when_entries_are_far_from_theirs() {
    let probe_directory = physical_tempdir().unwrap();
    let probe =
        ProviderIngestFinalizedArchiveV1::try_open(archive_root(&probe_directory), bounds())
            .unwrap();
    let mut records = vec![projection(7)];
    for height in 8..13 {
        records.push(advance_projection(records.last().unwrap(), height));
    }
    for record in &records {
        probe.insert(record.clone()).unwrap();
    }
    let total = probe.capacity().unwrap().total_bytes();
    drop(probe);

    let directory = physical_tempdir().unwrap();
    // Three quarters of this bound is exactly the six records, however their sizes vary.
    let byte_bound = total * 4 / 3;
    let archive = ProviderIngestFinalizedArchiveV1::try_open(
        archive_root(&directory),
        ProviderIngestFinalizedArchiveBoundsV1::try_new(total, 1_000, byte_bound, 16, 16, 64, 1)
            .unwrap(),
    )
    .unwrap();
    let network_id = records[0].key.network_id;
    for record in &records[..records.len() - 1] {
        archive.insert(record.clone()).unwrap();
    }
    assert!(!archive.capacity().unwrap().at_high_watermark());
    assert_eq!(
        plan(&archive, &network_id, false, &[], ONE_CALL),
        RetentionPlanV1::Idle
    );
    archive.insert(records.last().unwrap().clone()).unwrap();
    let capacity = archive.capacity().unwrap();
    assert_eq!(capacity.retained_entries(), 6);
    assert!(capacity.at_high_watermark(), "{capacity:?}");
    assert!(matches!(
        plan(&archive, &network_id, false, &[], ONE_CALL),
        RetentionPlanV1::Compact { .. }
    ));
}

/// A sealed authority that counts every external call.
#[derive(Debug)]
struct CountingAuthority {
    inner: TestRetentionAuthority,
    calls: AtomicUsize,
}

impl ProviderIngestFinalizedArchiveRetentionAuthorityV1 for CountingAuthority {
    fn handle(&self) -> &str {
        self.inner.handle()
    }
    fn qualification(
        &self,
    ) -> Result<
        ProviderIngestFinalizedArchiveRetentionAuthorityQualificationV1,
        ProviderIngestFinalizedArchiveRetentionAuthorityExternalErrorV1,
    > {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.inner.qualification()
    }
    fn load_latest(
        &self,
        network_id: &NetworkId,
    ) -> Result<
        Option<ProviderIngestFinalizedArchiveRetentionApprovalRecordV1>,
        ProviderIngestFinalizedArchiveRetentionAuthorityExternalErrorV1,
    > {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.inner.load_latest(network_id)
    }
    fn compare_and_swap_latest(
        &self,
        network_id: &NetworkId,
        expected_revision: Option<[u8; 32]>,
        next: &ProviderIngestFinalizedArchiveRetentionApprovalRecordV1,
    ) -> Result<(), ProviderIngestFinalizedArchiveRetentionAuthorityExternalErrorV1> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.inner
            .compare_and_swap_latest(network_id, expected_revision, next)
    }
}

#[test]
fn idle_retention_contacts_no_authority_and_changes_nothing() {
    let directory = physical_tempdir().unwrap();
    let mut chain = chain();
    let archive =
        ProviderIngestFinalizedArchiveV1::try_open(archive_root(&directory), bounds()).unwrap();
    capture_tip(&archive, &chain);
    commit_and_capture(&archive, &mut chain, 2);
    let authority = CountingAuthority {
        inner: TestRetentionAuthority::new(),
        calls: AtomicUsize::new(0),
    };
    let records = archive_namespace_snapshot(&archive.records);
    assert_eq!(
        archive
            .reconcile_retention_once(
                &chain.state().query_view(),
                chain.kura(),
                &authority.inner.binding(),
                &authority,
                &[],
            )
            .unwrap(),
        ProviderIngestFinalizedArchiveRetentionStepV1::Idle
    );
    assert_eq!(authority.calls.load(Ordering::SeqCst), 0);
    assert_eq!(archive_namespace_snapshot(&archive.records), records);
    assert_eq!(fs::read_dir(&archive.checkpoints).unwrap().count(), 0);
}

/// Three bound crossings over real commits: every refused capture recovers through the worker's
/// step, nothing above the floor changes, and the result survives a reopen.
#[test]
fn retention_carries_a_chain_past_the_entry_bound_repeatedly() {
    let directory = physical_tempdir().unwrap();
    let root = archive_root(&directory);
    let reference_directory = physical_tempdir().unwrap();
    let mut chain = chain();
    let bound = small_bounds(8);
    let archive = ProviderIngestFinalizedArchiveV1::try_open(&root, bound).unwrap();
    let reference =
        ProviderIngestFinalizedArchiveV1::try_open(archive_root(&reference_directory), bounds())
            .unwrap();
    capture_tip(&archive, &chain);
    capture_tip(&reference, &chain);
    let authority = TestRetentionAuthority::new();
    let mut refused = 0;
    for _ in 0..24 {
        chain.commit(Vec::new());
        capture_tip(&reference, &chain);
        loop {
            match archive.capture_certified_view(&chain.state().view(), chain.kura()) {
                Ok(_) => break,
                Err(ProviderIngestFinalizedArchiveErrorV1::ArchiveCapacityExceeded {
                    observed,
                    maximum,
                }) => {
                    assert_eq!((observed, maximum), (9, 8));
                    refused += 1;
                    assert!(
                        refused <= 12,
                        "a refused capture must recover through the retention step"
                    );
                    let compacted = step(&archive, &chain, &authority, &[]).unwrap();
                    assert!(
                        matches!(
                            compacted,
                            ProviderIngestFinalizedArchiveRetentionStepV1::Compacted(_)
                        ),
                        "the bound was reached, so the step must free capacity: {compacted:?}"
                    );
                }
                Err(error) => panic!("unexpected capture refusal: {error}"),
            }
        }
    }
    assert!(refused >= 3, "the bound was crossed {refused} times");
    let network_id = chain.network_id();
    let floor = floor_height(&archive, &network_id);
    assert!(floor > 1);
    assert!(archive.capacity().unwrap().retained_entries() <= 8);
    let view = chain.state().view();
    let qualification = archive
        .qualify_against_certified_tip(&view, chain.kura(), 0)
        .unwrap();
    assert_eq!(qualification.archive_tip().height, 25);
    assert_eq!(qualification.activation_floor().height, floor);
    for height in floor + 1..=25 {
        let key = reference
            .read_index()
            .unwrap()
            .by_height
            .get(&(network_id, height))
            .unwrap()
            .record
            .material
            .key;
        assert_eq!(
            fs::read(archive.record_path(&key).unwrap()).unwrap(),
            fs::read(reference.record_path(&key).unwrap()).unwrap(),
            "compaction must not change a later record's bytes"
        );
    }
    drop(archive);
    let reopened = ProviderIngestFinalizedArchiveV1::try_open_with_retention_authority(
        &root,
        bound,
        &network_id,
        &view,
        chain.kura(),
        &authority.binding(),
        &authority,
        &[],
    )
    .unwrap();
    assert_eq!(floor_height(&reopened, &network_id), floor);
    assert_eq!(
        reopened
            .qualify_against_certified_tip(&view, chain.kura(), 0)
            .unwrap()
            .archive_tip()
            .height,
        25
    );
}

/// A sealed authority that holds its compare-and-swap until the test releases it.
#[derive(Debug)]
struct GatedAuthority {
    inner: TestRetentionAuthority,
    entered: Mutex<mpsc::Sender<()>>,
    release: Mutex<mpsc::Receiver<()>>,
}

impl ProviderIngestFinalizedArchiveRetentionAuthorityV1 for GatedAuthority {
    fn handle(&self) -> &str {
        self.inner.handle()
    }
    fn qualification(
        &self,
    ) -> Result<
        ProviderIngestFinalizedArchiveRetentionAuthorityQualificationV1,
        ProviderIngestFinalizedArchiveRetentionAuthorityExternalErrorV1,
    > {
        self.inner.qualification()
    }
    fn load_latest(
        &self,
        network_id: &NetworkId,
    ) -> Result<
        Option<ProviderIngestFinalizedArchiveRetentionApprovalRecordV1>,
        ProviderIngestFinalizedArchiveRetentionAuthorityExternalErrorV1,
    > {
        self.inner.load_latest(network_id)
    }
    fn compare_and_swap_latest(
        &self,
        network_id: &NetworkId,
        expected_revision: Option<[u8; 32]>,
        next: &ProviderIngestFinalizedArchiveRetentionApprovalRecordV1,
    ) -> Result<(), ProviderIngestFinalizedArchiveRetentionAuthorityExternalErrorV1> {
        let _ = self.entered.lock().unwrap().send(());
        // A dropped sender also releases, so a failing test cannot hang this worker.
        let _ = self.release.lock().unwrap().recv();
        self.inner
            .compare_and_swap_latest(network_id, expected_revision, next)
    }
}

/// An external authority call must never hold back a commit's capture.
#[test]
fn commit_capture_proceeds_while_the_authority_decides() {
    let directory = physical_tempdir().unwrap();
    let mut chain = chain();
    let archive = Arc::new(
        ProviderIngestFinalizedArchiveV1::try_open(archive_root(&directory), bounds()).unwrap(),
    );
    capture_tip(&archive, &chain);
    commit_and_capture(&archive, &mut chain, 3);
    let network_id = chain.network_id();
    let view = chain.state().view();
    let key = archive
        .read_index()
        .unwrap()
        .by_height
        .get(&(network_id, 2))
        .unwrap()
        .record
        .material
        .key;
    let fence = ProviderIngestFinalizedArchiveRetentionFenceV1::try_new(
        key,
        *chain.committed(2).id().0.as_ref(),
        archive.health_generation().unwrap(),
    )
    .unwrap();
    let proposal = archive
        .prepare_certified_compaction(&fence, &view, chain.kura())
        .unwrap();
    drop(view);
    let (entered_sender, entered) = mpsc::channel();
    let (release, release_receiver) = mpsc::channel();
    let authority = Arc::new(GatedAuthority {
        inner: TestRetentionAuthority::new(),
        entered: Mutex::new(entered_sender),
        release: Mutex::new(release_receiver),
    });
    let binding = authority.inner.binding();
    let worker = {
        let archive = Arc::clone(&archive);
        let authority = Arc::clone(&authority);
        let state = Arc::clone(chain.state());
        crate::sumeragi::threads::sumeragi_thread_builder("retention-test-worker")
            .spawn(move || {
                let view = state.query_view();
                archive
                    .approve_and_install_certified_compaction(
                        &proposal,
                        &view,
                        state.kura(),
                        &binding,
                        authority.as_ref(),
                    )
                    .map_err(|error| error.to_string())
            })
            .unwrap()
    };
    entered.recv_timeout(Duration::from_secs(120)).unwrap();
    chain.commit(Vec::new());
    let captured = archive.capture_certified_view(&chain.state().view(), chain.kura());
    assert!(
        matches!(
            captured,
            Ok(ProviderIngestFinalizedArchiveInsertOutcomeV1::Inserted)
        ),
        "a commit must not wait for the external authority: {captured:?}"
    );
    release.send(()).unwrap();
    let outcome = worker.join().unwrap().unwrap();
    assert_eq!(outcome.retention_floor(), &key);
    assert_eq!(floor_height(&archive, &network_id), 2);
    assert_eq!(
        archive
            .qualify_against_certified_tip(&chain.state().view(), chain.kura(), 0)
            .unwrap()
            .archive_tip()
            .height,
        5
    );
}

/// The commit's constant-cost insertion must survive a compaction, or every compaction would
/// cost the next commit a complete walk under its State view and the archive writer.
#[test]
fn compaction_keeps_the_commit_on_its_retained_walk() {
    let directory = physical_tempdir().unwrap();
    let mut chain = chain();
    let archive =
        ProviderIngestFinalizedArchiveV1::try_open(archive_root(&directory), bounds()).unwrap();
    capture_tip(&archive, &chain);
    commit_and_capture(&archive, &mut chain, 3);
    let network_id = chain.network_id();
    let view = chain.state().view();
    let key = archive
        .read_index()
        .unwrap()
        .by_height
        .get(&(network_id, 2))
        .unwrap()
        .record
        .material
        .key;
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
    {
        let index = archive.read_index().unwrap();
        let tip = index.by_height.get(&(network_id, 4)).unwrap();
        let retained = index
            .tip_walk(tip)
            .expect("compaction keeps the walk at the tip");
        let mut complete =
            ProviderCoverageWalkV1::start(index.virtual_bases.get(&network_id)).unwrap();
        for (_, entry) in &index.by_height {
            complete
                .step(&index, network_id, entry, false, bounds())
                .unwrap();
        }
        assert_eq!(
            format!("{retained:?}"),
            format!("{complete:?}"),
            "the walk over the new base equals replaying every retained record"
        );
    }
    // The next capture reads only its own tip: a damaged record below the tip goes unnoticed
    // here, and stays the periodic audit's and the next open's to find.
    let below_tip = archive
        .read_index()
        .unwrap()
        .by_height
        .get(&(network_id, 3))
        .unwrap()
        .path
        .clone();
    let original = fs::read(&below_tip).unwrap();
    fs::write(&below_tip, [0_u8]).unwrap();
    chain.commit(Vec::new());
    assert_eq!(
        archive
            .capture_certified_view(&chain.state().view(), chain.kura())
            .unwrap(),
        ProviderIngestFinalizedArchiveInsertOutcomeV1::Inserted
    );
    fs::write(&below_tip, original).unwrap();
    // Nor does it reread the checkpoint, whose size grows with every order ever seen. Same length,
    // different bytes: qualification reads the file and finds it, and the commit does not.
    let checkpoint = archive
        .read_index()
        .unwrap()
        .virtual_bases
        .get(&network_id)
        .unwrap()
        .path
        .clone();
    let intact = fs::read(&checkpoint).unwrap();
    let mut damaged = intact.clone();
    let last = damaged.len() - 1;
    damaged[last] ^= 0xFF;
    fs::write(&checkpoint, damaged).unwrap();
    chain.commit(Vec::new());
    assert_eq!(
        archive
            .capture_certified_view(&chain.state().view(), chain.kura())
            .unwrap(),
        ProviderIngestFinalizedArchiveInsertOutcomeV1::Inserted
    );
    assert!(
        archive
            .qualify_against_certified_tip(&chain.state().view(), chain.kura(), 0)
            .is_err()
    );
    fs::write(&checkpoint, intact).unwrap();
}

/// A prefix approved by the authority must still install after later captures advanced the
/// archive past the approved generation, which a publish failure followed by a restart allows.
#[test]
fn reopen_finishes_an_approved_compaction_after_captures_outran_it() {
    let directory = physical_tempdir().unwrap();
    let root = archive_root(&directory);
    let mut chain = chain();
    let archive = ProviderIngestFinalizedArchiveV1::try_open(&root, bounds()).unwrap();
    capture_tip(&archive, &chain);
    commit_and_capture(&archive, &mut chain, 3);
    let network_id = chain.network_id();
    let view = chain.state().view();
    let key = archive
        .read_index()
        .unwrap()
        .by_height
        .get(&(network_id, 2))
        .unwrap()
        .record
        .material
        .key;
    let fence = ProviderIngestFinalizedArchiveRetentionFenceV1::try_new(
        key,
        *chain.committed(2).id().0.as_ref(),
        archive.health_generation().unwrap(),
    )
    .unwrap();
    let proposal = archive
        .prepare_certified_compaction(&fence, &view, chain.kura())
        .unwrap();
    drop(view);
    // The CAS committed and the process died before publishing the checkpoint, and captures
    // landed in between.
    let authority = TestRetentionAuthority::new();
    let binding = authority.binding();
    *authority.latest.lock().unwrap() = Some(
        ProviderIngestFinalizedArchiveRetentionApprovalRecordV1::try_new(
            1,
            binding.qualification(),
            proposal,
            None,
            None,
        )
        .unwrap(),
    );
    commit_and_capture(&archive, &mut chain, 2);
    drop(archive);
    let view = chain.state().view();
    let records_before = archive_namespace_snapshot(&root.join(RECORDS_DIRECTORY));
    let checkpoints_before = archive_namespace_snapshot(&root.join(CHECKPOINTS_DIRECTORY));
    let approval_before = authority.latest.lock().unwrap().clone();
    let first = chain.committed(1);
    let restored_pending = ProviderIngestFinalizedAnchorV1 {
        height: 1,
        block_hash: *first.block_hash().as_ref(),
    };
    assert!(
        matches!(
            ProviderIngestFinalizedArchiveV1::try_open_with_retention_authority(
                &root,
                bounds(),
                &network_id,
                &view,
                chain.kura(),
                &binding,
                &authority,
                &[restored_pending],
            ),
            Err(ProviderIngestFinalizedArchiveErrorV1::InvalidRetentionFence { .. })
        ),
        "startup must check restored jobs before recovering or unlinking an approved prefix"
    );
    assert!(matches!(
        ProviderIngestFinalizedArchiveV1::try_open_with_retention_authority(
            &root,
            bounds(),
            &network_id,
            &view,
            chain.kura(),
            &binding,
            &authority,
            &[ProviderIngestFinalizedAnchorV1 {
                height: key.height,
                block_hash: [0xA4; 32]
            }],
        ),
        Err(ProviderIngestFinalizedArchiveErrorV1::FinalizedFork { .. })
    ));
    assert_eq!(
        archive_namespace_snapshot(&root.join(RECORDS_DIRECTORY)),
        records_before
    );
    assert_eq!(
        archive_namespace_snapshot(&root.join(CHECKPOINTS_DIRECTORY)),
        checkpoints_before
    );
    assert_eq!(*authority.latest.lock().unwrap(), approval_before);
    let reopened = ProviderIngestFinalizedArchiveV1::try_open_with_retention_authority(
        &root,
        bounds(),
        &network_id,
        &view,
        chain.kura(),
        &binding,
        &authority,
        &[key.finalized_anchor()],
    )
    .unwrap();
    assert_eq!(reopened.retention_floor(&network_id).unwrap(), Some(key));
    assert_eq!(
        fs::read_dir(&reopened.records).unwrap().count(),
        4,
        "exactly the approved prefix was removed"
    );
    assert_eq!(
        reopened
            .qualify_against_certified_tip(&view, chain.kura(), 0)
            .unwrap()
            .archive_tip()
            .height,
        6
    );
    drop(view);
    commit_and_capture(&reopened, &mut chain, 1);
}

/// A failed checkpoint publication leaves the authority ahead of this archive. The next step
/// must finish that exact approval, not plan a different floor the authority would refuse.
#[test]
fn a_step_finishes_an_approval_it_could_not_publish() {
    let directory = physical_tempdir().unwrap();
    let mut chain = chain();
    let archive =
        ProviderIngestFinalizedArchiveV1::try_open(archive_root(&directory), small_bounds(8))
            .unwrap();
    capture_tip(&archive, &chain);
    commit_and_capture(&archive, &mut chain, 5);
    let network_id = chain.network_id();
    let authority = TestRetentionAuthority::new();
    // The same content-addressed name, already occupied by other bytes.
    let view = chain.state().view();
    let index = archive.read_index().unwrap();
    let RetentionPlanV1::Compact { target, .. } =
        plan_retention(&index, &network_id, archive.bounds, false, &[], ONE_CALL)
    else {
        panic!("six of eight entries must start a cycle");
    };
    drop(index);
    let fence = ProviderIngestFinalizedArchiveRetentionFenceV1::try_new(
        target,
        *chain.committed(target.height).id().0.as_ref(),
        archive.health_generation().unwrap(),
    )
    .unwrap();
    let proposal = archive
        .prepare_certified_compaction(&fence, &view, chain.kura())
        .unwrap();
    drop(view);
    let occupied = archive
        .checkpoints
        .join(checkpoint_file_name(proposal.checkpoint_digest()));
    fs::write(&occupied, [0_u8]).unwrap();
    assert!(step(&archive, &chain, &authority, &[]).is_err());
    assert!(
        authority.latest.lock().unwrap().is_some(),
        "the approval landed before the failed publication"
    );
    assert_eq!(archive.retention_floor(&network_id).unwrap(), None);
    fs::remove_file(&occupied).unwrap();
    assert!(
        matches!(
            step(&archive, &chain, &authority, &[1]),
            Err(ProviderIngestFinalizedArchiveErrorV1::InvalidRetentionFence { .. })
        ),
        "an interrupted approval cannot ignore a newly conflicting hard obligation"
    );
    assert_eq!(archive.retention_floor(&network_id).unwrap(), None);
    // Traffic moves the plan on, which must not change the floor already approved.
    commit_and_capture(&archive, &mut chain, 1);
    let finished = step(&archive, &chain, &authority, &[]).unwrap();
    let ProviderIngestFinalizedArchiveRetentionStepV1::Compacted(outcome) = finished else {
        panic!("the approved compaction must finish: {finished:?}");
    };
    assert_eq!(outcome.retention_floor(), &target);
    assert_eq!(outcome.checkpoint_digest(), proposal.checkpoint_digest());
    assert_eq!(archive.retention_floor(&network_id).unwrap(), Some(target));
}

/// An authority behind the installed floor has lost its lineage. That must stop retention with
/// the real diagnosis, and it deletes nothing.
#[test]
fn an_authority_behind_the_floor_is_a_rollback() {
    let directory = physical_tempdir().unwrap();
    let mut chain = chain();
    let archive =
        ProviderIngestFinalizedArchiveV1::try_open(archive_root(&directory), small_bounds(8))
            .unwrap();
    capture_tip(&archive, &chain);
    commit_and_capture(&archive, &mut chain, 5);
    prove(&archive, &chain);
    let authority = TestRetentionAuthority::new();
    let limits = RetentionLimitsV1 {
        max_prefix_entries: 1,
        ..ONE_CALL
    };
    let run = |chain: &CertifiedTestChain| {
        archive.reconcile_retention_with_limits(
            &chain.state().query_view(),
            chain.kura(),
            &authority.binding(),
            &authority,
            &[],
            limits,
        )
    };
    let older = run(&chain).unwrap();
    let approved = authority.latest.lock().unwrap().clone();
    let newer = run(&chain).unwrap();
    assert!(
        matches!(
            (older, newer),
            (
                ProviderIngestFinalizedArchiveRetentionStepV1::Compacted(_),
                ProviderIngestFinalizedArchiveRetentionStepV1::Compacted(_)
            )
        ),
        "{older:?} {newer:?}"
    );
    let records = archive_namespace_snapshot(&archive.records);
    *authority.latest.lock().unwrap() = approved;
    assert!(matches!(
        run(&chain),
        Err(ProviderIngestFinalizedArchiveErrorV1::RetentionAuthorityRollback)
    ));
    assert_eq!(archive_namespace_snapshot(&archive.records), records);
}

#[test]
fn retention_stops_at_a_pin_and_resumes_when_it_clears() {
    let directory = physical_tempdir().unwrap();
    let mut chain = chain();
    let archive =
        ProviderIngestFinalizedArchiveV1::try_open(archive_root(&directory), small_bounds(8))
            .unwrap();
    capture_tip(&archive, &chain);
    commit_and_capture(&archive, &mut chain, 5);
    let network_id = chain.network_id();
    let authority = TestRetentionAuthority::new();
    let pinned = archive
        .read_index()
        .unwrap()
        .by_height
        .get(&(network_id, 2))
        .unwrap()
        .record
        .material
        .key;

    let first = step(&archive, &chain, &authority, &[2]).unwrap();
    assert!(
        matches!(
            first,
            ProviderIngestFinalizedArchiveRetentionStepV1::Compacted(_)
        ),
        "{first:?}"
    );
    assert_eq!(floor_height(&archive, &network_id), 2);
    // The job's admission anchor is still the exact retained floor.
    assert_eq!(
        archive
            .resolve_exact_key(&network_id, 2, pinned.block_hash)
            .unwrap(),
        pinned
    );
    let second = step(&archive, &chain, &authority, &[2]).unwrap();
    assert!(
        matches!(
            second,
            ProviderIngestFinalizedArchiveRetentionStepV1::Pinned {
                pinned_height: 2,
                ..
            }
        ),
        "the cycle has more to do and the pin holds it: {second:?}"
    );
    assert_eq!(floor_height(&archive, &network_id), 2);

    let third = step(&archive, &chain, &authority, &[]).unwrap();
    assert!(
        matches!(
            third,
            ProviderIngestFinalizedArchiveRetentionStepV1::Compacted(_)
        ),
        "{third:?}"
    );
    assert!(floor_height(&archive, &network_id) > 2);
    assert!(matches!(
        archive.resolve_exact_key(&network_id, 2, pinned.block_hash),
        Err(ProviderIngestFinalizedArchiveErrorV1::BelowRetentionFloor { .. })
    ));
}

/// A cycle runs from the high watermark to the low one, then stops contacting the authority.
#[test]
fn a_cycle_ends_at_the_low_watermark() {
    let directory = physical_tempdir().unwrap();
    let mut chain = chain();
    let archive =
        ProviderIngestFinalizedArchiveV1::try_open(archive_root(&directory), small_bounds(8))
            .unwrap();
    capture_tip(&archive, &chain);
    commit_and_capture(&archive, &mut chain, 6);
    prove(&archive, &chain);
    let authority = TestRetentionAuthority::new();
    let limits = RetentionLimitsV1 {
        max_prefix_entries: 1,
        ..ONE_CALL
    };
    let mut compactions = 0;
    loop {
        match archive
            .reconcile_retention_with_limits(
                &chain.state().query_view(),
                chain.kura(),
                &authority.binding(),
                &authority,
                &[],
                limits,
            )
            .unwrap()
        {
            ProviderIngestFinalizedArchiveRetentionStepV1::Compacted(_) => {
                compactions += 1;
                assert!(compactions <= 8, "a cycle must end");
            }
            ProviderIngestFinalizedArchiveRetentionStepV1::Idle => break,
            other => panic!("unexpected step {other:?}"),
        }
    }
    // Seven records, plus the base, down to four entries.
    assert_eq!(compactions, 4);
    assert_eq!(archive.capacity().unwrap().retained_entries(), 4);
}

/// Under steady commits a block always lands between steps. A cycle must still end at the
/// compaction that reaches the low watermark, or every step would compact and call the authority.
#[test]
fn steady_commits_do_not_keep_a_finished_cycle_running() {
    let directory = physical_tempdir().unwrap();
    let mut chain = chain();
    let archive =
        ProviderIngestFinalizedArchiveV1::try_open(archive_root(&directory), small_bounds(8))
            .unwrap();
    capture_tip(&archive, &chain);
    commit_and_capture(&archive, &mut chain, 5);
    let authority = TestRetentionAuthority::new();
    assert!(matches!(
        step(&archive, &chain, &authority, &[]).unwrap(),
        ProviderIngestFinalizedArchiveRetentionStepV1::Compacted(_)
    ));
    assert_eq!(archive.capacity().unwrap().retained_entries(), 4);
    // Above the low watermark again, below the high one.
    commit_and_capture(&archive, &mut chain, 1);
    assert_eq!(archive.capacity().unwrap().retained_entries(), 5);
    assert_eq!(
        step(&archive, &chain, &authority, &[]).unwrap(),
        ProviderIngestFinalizedArchiveRetentionStepV1::Idle
    );
    assert_eq!(
        authority
            .latest
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .sequence(),
        1,
        "no further approval until the next cycle is due"
    );
}

/// A step only prunes what this process already proved, so until qualification has walked it
/// waits, and it never contacts the authority first.
#[test]
fn a_step_waits_for_proved_coverage() {
    let directory = physical_tempdir().unwrap();
    let mut chain = chain();
    let archive =
        ProviderIngestFinalizedArchiveV1::try_open(archive_root(&directory), small_bounds(8))
            .unwrap();
    capture_tip(&archive, &chain);
    commit_and_capture(&archive, &mut chain, 5);
    let authority = CountingAuthority {
        inner: TestRetentionAuthority::new(),
        calls: AtomicUsize::new(0),
    };
    let binding = authority.inner.binding();
    let once = |chain: &CertifiedTestChain| {
        archive
            .reconcile_retention_once(
                &chain.state().query_view(),
                chain.kura(),
                &binding,
                &authority,
                &[],
            )
            .unwrap()
    };
    assert_eq!(
        once(&chain),
        ProviderIngestFinalizedArchiveRetentionStepV1::AwaitingProof
    );
    assert_eq!(authority.calls.load(Ordering::SeqCst), 0);
    prove(&archive, &chain);
    assert!(matches!(
        once(&chain),
        ProviderIngestFinalizedArchiveRetentionStepV1::Compacted(_)
    ));
    // A fault the audit finds takes the proof back, and retention waits again.
    archive.authentication.forget_coverage();
    commit_and_capture(&archive, &mut chain, 2);
    assert_eq!(
        once(&chain),
        ProviderIngestFinalizedArchiveRetentionStepV1::AwaitingProof
    );
}

/// A step checks no commit certificates, because it prunes only what is already proved. A
/// certificate gone bad below the floor stays the audit's and the next open's to find, as for any
/// proved anchor.
#[test]
fn a_step_checks_no_commit_certificates_and_a_reopen_still_does() {
    let directory = physical_tempdir().unwrap();
    let root = archive_root(&directory);
    let mut chain = chain();
    let bound = small_bounds(8);
    let archive = ProviderIngestFinalizedArchiveV1::try_open(&root, bound).unwrap();
    capture_tip(&archive, &chain);
    commit_and_capture(&archive, &mut chain, 5);
    prove(&archive, &chain);
    chain
        .kura()
        .corrupt_commit_certificate_for_testing(std::num::NonZeroUsize::new(2).unwrap(), None)
        .unwrap();
    let authority = TestRetentionAuthority::new();
    let compacted = archive
        .reconcile_retention_with_limits(
            &chain.state().query_view(),
            chain.kura(),
            &authority.binding(),
            &authority,
            &[],
            ONE_CALL,
        )
        .unwrap();
    assert!(
        matches!(
            compacted,
            ProviderIngestFinalizedArchiveRetentionStepV1::Compacted(_)
        ),
        "{compacted:?}"
    );
    drop(archive);
    let view = chain.state().view();
    assert!(
        ProviderIngestFinalizedArchiveV1::try_open_with_retention_authority(
            &root,
            bound,
            &chain.network_id(),
            &view,
            chain.kura(),
            &authority.binding(),
            &authority,
            &[],
        )
        .is_err(),
        "the next process's walk from signed genesis finds it"
    );
}

/// Capacity stays held until the pruned files are gone. A reopen counts files before recovery can
/// remove any, so capture must never add a record while their total could pass the bound.
#[test]
fn capture_waits_for_the_unlinks_so_no_restart_finds_too_many_files() {
    let directory = physical_tempdir().unwrap();
    let root = archive_root(&directory);
    let mut chain = chain();
    let bound = small_bounds(8);
    let archive = ProviderIngestFinalizedArchiveV1::try_open(&root, bound).unwrap();
    capture_tip(&archive, &chain);
    commit_and_capture(&archive, &mut chain, 7);
    let network_id = chain.network_id();
    let authority = TestRetentionAuthority::new();
    let binding = authority.binding();
    let key = archive
        .read_index()
        .unwrap()
        .by_height
        .get(&(network_id, 5))
        .unwrap()
        .record
        .material
        .key;
    let fence = ProviderIngestFinalizedArchiveRetentionFenceV1::try_new(
        key,
        *chain.committed(5).id().0.as_ref(),
        archive.health_generation().unwrap(),
    )
    .unwrap();
    let snapshot = archive.retention_snapshot(&key).unwrap();
    let prepared = prepare_retention_from_snapshot(&snapshot, &fence, bound).unwrap();
    *authority.latest.lock().unwrap() = Some(
        ProviderIngestFinalizedArchiveRetentionApprovalRecordV1::try_new(
            1,
            binding.qualification(),
            compaction_proposal(&prepared, &fence).unwrap(),
            None,
            None,
        )
        .unwrap(),
    );
    let path = archive
        .checkpoints
        .join(checkpoint_file_name(prepared.checkpoint.checkpoint_digest));
    publish_immutable_bytes(
        #[cfg(windows)]
        &archive.native,
        &archive.checkpoints,
        archive.checkpoints_identity,
        &path,
        &prepared.canonical_bytes,
    )
    .unwrap();
    let _ = archive
        .install_published_compaction(&prepared, path)
        .unwrap();
    // Installed, with all eight record files still on disk: a capture now would make nine.
    chain.commit(Vec::new());
    assert!(matches!(
        archive.capture_certified_view(&chain.state().view(), chain.kura()),
        Err(ProviderIngestFinalizedArchiveErrorV1::ArchiveCapacityExceeded { .. })
    ));
    assert_eq!(fs::read_dir(&archive.records).unwrap().count(), 8);
    {
        let mut obsolete = prepared.obsolete;
        obsolete.reverse();
        let mut progress = archive.retention.lock();
        progress.obsolete_records = obsolete;
        archive
            .finish_pending_retention_cleanup(&mut progress)
            .unwrap();
    }
    assert_eq!(
        archive
            .capture_certified_view(&chain.state().view(), chain.kura())
            .unwrap(),
        ProviderIngestFinalizedArchiveInsertOutcomeV1::Inserted,
        "the same held commit goes through once the files are gone"
    );
    drop(archive);
    let view = chain.state().view();
    let reopened = ProviderIngestFinalizedArchiveV1::try_open_with_retention_authority(
        &root,
        bound,
        &network_id,
        &view,
        chain.kura(),
        &binding,
        &authority,
        &[],
    )
    .unwrap();
    assert_eq!(reopened.retention_floor(&network_id).unwrap(), Some(key));
}

/// Startup has not opened the job outbox, so a capacity refusal must preserve every
/// possible admission anchor without advancing the retention authority.
#[test]
fn a_restart_at_the_bound_preserves_unopened_job_admission_anchors() {
    let directory = physical_tempdir().unwrap();
    let root = archive_root(&directory);
    let mut chain = chain();
    let bound = small_bounds(8);
    let archive = ProviderIngestFinalizedArchiveV1::try_open(&root, bound).unwrap();
    capture_tip(&archive, &chain);
    commit_and_capture(&archive, &mut chain, 7);
    chain.commit(Vec::new());
    let network_id = chain.network_id();
    let view = chain.state().view();
    assert!(matches!(
        archive.capture_certified_view(&view, chain.kura()),
        Err(ProviderIngestFinalizedArchiveErrorV1::ArchiveCapacityExceeded { .. })
    ));
    drop(archive);

    let authority = TestRetentionAuthority::new();
    let binding = authority.binding();
    let reopened = ProviderIngestFinalizedArchiveV1::try_open_with_retention_authority(
        &root,
        bound,
        &network_id,
        &view,
        chain.kura(),
        &binding,
        &authority,
        &[],
    )
    .unwrap();
    assert!(matches!(
        reopened.reconcile_certified_state_tip(&view, chain.kura()),
        Err(ProviderIngestFinalizedArchiveErrorV1::ArchiveCapacityExceeded { .. })
    ));
    let admission = reopened.activation_floor(&network_id).unwrap().unwrap();
    let records = archive_namespace_snapshot(&reopened.records);
    assert_eq!(admission.height, 1);
    for _ in 0..2 {
        assert!(matches!(
            reopened.reconcile_certified_state_tip(&view, chain.kura()),
            Err(ProviderIngestFinalizedArchiveErrorV1::ArchiveCapacityExceeded { .. })
        ));
        assert_eq!(archive_namespace_snapshot(&reopened.records), records);
        assert_eq!(
            reopened
                .resolve_exact_key(&network_id, admission.height, admission.block_hash)
                .unwrap(),
            admission
        );
        assert!(authority.latest.lock().unwrap().is_none());
    }
}

/// Retaining the exact pending-job pins lets startup recover capacity without
/// retiring an admission anchor; a pin at the first anchor still refuses safely.
#[test]
fn startup_retention_recovers_with_pins_and_refuses_when_all_history_is_needed() {
    let directory = physical_tempdir().unwrap();
    let mut chain = chain();
    let archive =
        ProviderIngestFinalizedArchiveV1::try_open(archive_root(&directory), small_bounds(8))
            .unwrap();
    capture_tip(&archive, &chain);
    commit_and_capture(&archive, &mut chain, 7);
    chain.commit(Vec::new());
    let network_id = chain.network_id();
    let authority = TestRetentionAuthority::new();
    let binding = authority.binding();
    let view = chain.state().view();
    let first = chain.committed(1);
    let first_key = archive
        .resolve_exact_key(&network_id, 1, *first.block_hash().as_ref())
        .unwrap();
    assert!(matches!(
        archive.reconcile_certified_state_tip_with_retention(
            &view,
            chain.kura(),
            &binding,
            &authority,
            &[first_key],
        ),
        Err(ProviderIngestFinalizedArchiveErrorV1::ArchiveCapacityExceeded { .. })
    ));
    assert_eq!(
        archive
            .resolve_exact_key(&network_id, 1, first_key.block_hash)
            .unwrap(),
        first_key
    );
    assert_eq!(archive.capacity().unwrap().retained_entries(), 8);
    let pending = chain.committed(2);
    let pending_key = archive
        .resolve_exact_key(&network_id, 2, *pending.block_hash().as_ref())
        .unwrap();
    let result = archive
        .reconcile_certified_state_tip_with_retention(
            &view,
            chain.kura(),
            &binding,
            &authority,
            &[pending_key],
        )
        .unwrap();
    assert_eq!(result.qualification().archive_tip().height, 9);
    assert_eq!(
        archive.retention_floor(&network_id).unwrap(),
        Some(pending_key)
    );
    assert_eq!(
        archive
            .resolve_exact_key(&network_id, 2, pending_key.block_hash)
            .unwrap(),
        pending_key
    );
}

/// Below every limit, the same call is plain reconciliation and contacts nobody.
#[test]
fn a_restart_below_the_bound_is_plain_reconciliation() {
    let directory = physical_tempdir().unwrap();
    let mut chain = chain();
    let archive =
        ProviderIngestFinalizedArchiveV1::try_open(archive_root(&directory), bounds()).unwrap();
    capture_tip(&archive, &chain);
    chain.commit(Vec::new());
    let authority = CountingAuthority {
        inner: TestRetentionAuthority::new(),
        calls: AtomicUsize::new(0),
    };
    let outcome = archive
        .reconcile_certified_state_tip(&chain.state().view(), chain.kura())
        .unwrap();
    assert_eq!(outcome.qualification().archive_tip().height, 2);
    assert_eq!(authority.calls.load(Ordering::SeqCst), 0);
}

/// A step's view can lag Kura's tip, because commits keep landing while the step runs.
#[test]
fn a_view_older_than_the_tip_can_still_compact() {
    let directory = physical_tempdir().unwrap();
    let mut chain = chain();
    let archive =
        ProviderIngestFinalizedArchiveV1::try_open(archive_root(&directory), small_bounds(8))
            .unwrap();
    capture_tip(&archive, &chain);
    commit_and_capture(&archive, &mut chain, 5);
    prove(&archive, &chain);
    let authority = TestRetentionAuthority::new();
    let state = Arc::clone(chain.state());
    let stale = state.query_view();
    commit_and_capture(&archive, &mut chain, 1);
    let outcome = archive
        .reconcile_retention_with_limits(
            &stale,
            state.kura(),
            &authority.binding(),
            &authority,
            &[],
            ONE_CALL,
        )
        .unwrap();
    assert!(
        matches!(
            outcome,
            ProviderIngestFinalizedArchiveRetentionStepV1::Compacted(_)
        ),
        "{outcome:?}"
    );
}

/// Identical inputs produce identical checkpoint bytes on any node, so a restart's reconstruction
/// matches the approval.
#[test]
fn identical_prefixes_propose_identical_checkpoints() {
    let mut chain = chain();
    let directories = [physical_tempdir().unwrap(), physical_tempdir().unwrap()];
    let archives = directories.each_ref().map(|directory| {
        ProviderIngestFinalizedArchiveV1::try_open(archive_root(directory), bounds()).unwrap()
    });
    for archive in &archives {
        capture_tip(archive, &chain);
    }
    for _ in 0..3 {
        chain.commit(Vec::new());
        for archive in &archives {
            capture_tip(archive, &chain);
        }
    }
    let view = chain.state().view();
    let [first, second] = archives.each_ref().map(|archive| {
        let key = archive
            .read_index()
            .unwrap()
            .by_height
            .get(&(chain.network_id(), 2))
            .unwrap()
            .record
            .material
            .key;
        let fence = ProviderIngestFinalizedArchiveRetentionFenceV1::try_new(
            key,
            *chain.committed(2).id().0.as_ref(),
            archive.health_generation().unwrap(),
        )
        .unwrap();
        archive
            .prepare_certified_compaction(&fence, &view, chain.kura())
            .unwrap()
    });
    assert_eq!(first, second);
}

/// Installed, and the process dies before it unlinks anything: the next open finishes the job, and
/// the next commit proceeds. `reopen_finishes_an_approved_compaction_after_captures_outran_it`
/// covers the window before publication.
#[test]
fn a_crash_between_the_swap_and_the_unlinks_finishes_on_reopen() {
    let directory = physical_tempdir().unwrap();
    let root = archive_root(&directory);
    let mut chain = chain();
    let archive = ProviderIngestFinalizedArchiveV1::try_open(&root, bounds()).unwrap();
    capture_tip(&archive, &chain);
    commit_and_capture(&archive, &mut chain, 3);
    let network_id = chain.network_id();
    let authority = TestRetentionAuthority::new();
    let binding = authority.binding();
    let key = archive
        .read_index()
        .unwrap()
        .by_height
        .get(&(network_id, 2))
        .unwrap()
        .record
        .material
        .key;
    let fence = ProviderIngestFinalizedArchiveRetentionFenceV1::try_new(
        key,
        *chain.committed(2).id().0.as_ref(),
        archive.health_generation().unwrap(),
    )
    .unwrap();
    let snapshot = archive.retention_snapshot(&key).unwrap();
    let prepared = prepare_retention_from_snapshot(&snapshot, &fence, bounds()).unwrap();
    let proposal = compaction_proposal(&prepared, &fence).unwrap();
    *authority.latest.lock().unwrap() = Some(
        ProviderIngestFinalizedArchiveRetentionApprovalRecordV1::try_new(
            1,
            binding.qualification(),
            proposal,
            None,
            None,
        )
        .unwrap(),
    );
    let path = archive
        .checkpoints
        .join(checkpoint_file_name(prepared.checkpoint.checkpoint_digest));
    publish_immutable_bytes(
        #[cfg(windows)]
        &archive.native,
        &archive.checkpoints,
        archive.checkpoints_identity,
        &path,
        &prepared.canonical_bytes,
    )
    .unwrap();
    // Installed, and the process dies before it unlinks anything.
    let _ = archive
        .install_published_compaction(&prepared, path)
        .unwrap();
    assert_eq!(fs::read_dir(&archive.records).unwrap().count(), 4);
    commit_and_capture(&archive, &mut chain, 1);
    drop(archive);

    let view = chain.state().view();
    let reopened = ProviderIngestFinalizedArchiveV1::try_open_with_retention_authority(
        &root,
        bounds(),
        &network_id,
        &view,
        chain.kura(),
        &binding,
        &authority,
        &[],
    )
    .unwrap();
    assert_eq!(reopened.retention_floor(&network_id).unwrap(), Some(key));
    assert_eq!(
        fs::read_dir(&reopened.records).unwrap().count(),
        3,
        "startup finished the unlinks"
    );
    assert_eq!(
        reopened
            .qualify_against_certified_tip(&view, chain.kura(), 0)
            .unwrap()
            .archive_tip()
            .height,
        5
    );
    drop(view);
    commit_and_capture(&reopened, &mut chain, 1);
}

/// A checkpoint with an unfinished predecessor's files left behind could not reopen, so a failed
/// cleanup holds back the next compaction and keeps retrying until it finishes.
#[cfg(unix)]
#[test]
fn a_failed_cleanup_holds_the_next_compaction_until_it_finishes() {
    let directory = physical_tempdir().unwrap();
    let mut chain = chain();
    let archive =
        ProviderIngestFinalizedArchiveV1::try_open(archive_root(&directory), small_bounds(8))
            .unwrap();
    capture_tip(&archive, &chain);
    commit_and_capture(&archive, &mut chain, 5);
    let network_id = chain.network_id();
    let authority = TestRetentionAuthority::new();
    // A pending file with a second link refuses its verified unlink until that link is gone.
    let (subject, mut stuck) = archive
        .read_index()
        .unwrap()
        .by_height
        .iter()
        .next()
        .map(|(subject, entry)| (*subject, entry.clone()))
        .unwrap();
    stuck.path = archive.records.join("unfinished-cleanup");
    fs::write(&stuck.path, [0_u8]).unwrap();
    let second_link = directory.path().join("second-link");
    fs::hard_link(&stuck.path, &second_link).unwrap();
    archive.retention.lock().obsolete_records = vec![(subject, stuck.clone())];

    assert!(step(&archive, &chain, &authority, &[]).is_err());
    assert!(
        authority.latest.lock().unwrap().is_none(),
        "nothing new is approved while cleanup is unfinished"
    );
    assert_eq!(archive.retention_floor(&network_id).unwrap(), None);
    fs::remove_file(&second_link).unwrap();
    let next = step(&archive, &chain, &authority, &[]).unwrap();
    assert!(
        matches!(
            next,
            ProviderIngestFinalizedArchiveRetentionStepV1::Compacted(_)
        ),
        "{next:?}"
    );
    assert!(!stuck.path.exists(), "the retried cleanup finished");
}

/// The authority refusing or failing leaves this node exactly where it was, ready to retry.
#[test]
fn an_authority_failure_changes_nothing_and_a_later_step_retries() {
    let directory = physical_tempdir().unwrap();
    let mut chain = chain();
    let archive =
        ProviderIngestFinalizedArchiveV1::try_open(archive_root(&directory), small_bounds(8))
            .unwrap();
    capture_tip(&archive, &chain);
    commit_and_capture(&archive, &mut chain, 5);
    let network_id = chain.network_id();
    let authority = TestRetentionAuthority::new();
    let records = archive_namespace_snapshot(&archive.records);
    authority.set_behavior(TestRetentionCasBehavior::LeaveUnchanged);
    assert!(matches!(
        step(&archive, &chain, &authority, &[]),
        Err(ProviderIngestFinalizedArchiveErrorV1::RetentionAuthorityCasUnchanged)
    ));
    assert_eq!(archive.retention_floor(&network_id).unwrap(), None);
    assert_eq!(fs::read_dir(&archive.checkpoints).unwrap().count(), 0);
    assert_eq!(archive_namespace_snapshot(&archive.records), records);
    // Commits go on meanwhile.
    commit_and_capture(&archive, &mut chain, 1);
    // A write that lands but reports ambiguity resolves by exact readback.
    authority.set_behavior(TestRetentionCasBehavior::ApplyAmbiguous);
    let retried = step(&archive, &chain, &authority, &[]).unwrap();
    assert!(
        matches!(
            retried,
            ProviderIngestFinalizedArchiveRetentionStepV1::Compacted(_)
        ),
        "{retried:?}"
    );
}

/// A step far below the tip must not pay for every frame above its target, or at the default
/// bound each step would read hundreds of thousands of frames to prune about a thousand records.
#[test]
fn a_step_reads_only_frames_near_its_target_once_a_walk_reached_them() {
    let directory = physical_tempdir().unwrap();
    let mut chain = chain();
    let archive =
        ProviderIngestFinalizedArchiveV1::try_open(archive_root(&directory), small_bounds(48))
            .unwrap();
    capture_tip(&archive, &chain);
    commit_and_capture(&archive, &mut chain, 40);
    prove(&archive, &chain);
    let authority = TestRetentionAuthority::new();
    let limits = RetentionLimitsV1 {
        max_prefix_entries: 2,
        max_prefix_bytes: u64::MAX,
    };
    let run_step = || {
        archive.reconcile_retention_with_limits(
            &chain.state().query_view(),
            chain.kura(),
            &authority.binding(),
            &authority,
            &[],
            limits,
        )
    };
    assert!(matches!(
        run_step().unwrap(),
        ProviderIngestFinalizedArchiveRetentionStepV1::Compacted(_)
    ));
    let (next, counts) = crate::sumeragi::certified_chain::relation_counts::measure(run_step);
    let ProviderIngestFinalizedArchiveRetentionStepV1::Compacted(outcome) = next.unwrap() else {
        panic!("the cycle has not reached its low watermark");
    };
    assert!(
        outcome.retention_floor().height + 30 < 41,
        "the target is far below the tip"
    );
    assert!(
        counts.frames.len() <= 2,
        "signed genesis and the target at most: {:?}",
        counts.frames
    );
    assert!(counts.qcs.is_empty(), "{:?}", counts.qcs);
}

/// A fresh walk can overlap a compaction while coverage still stands, when only its retained
/// point is gone. The compaction carried that coverage to its new base, and the walk's older
/// identity must not replace it, or retention would wait for yet another walk from genesis.
#[test]
fn a_walk_overtaken_by_a_compaction_leaves_the_coverage_it_carried() {
    let directory = physical_tempdir().unwrap();
    let mut chain = chain();
    let archive =
        ProviderIngestFinalizedArchiveV1::try_open(archive_root(&directory), small_bounds(8))
            .unwrap();
    capture_tip(&archive, &chain);
    commit_and_capture(&archive, &mut chain, 5);
    prove(&archive, &chain);
    archive.authentication.forget_retained_prefix_for_testing();
    let authority = TestRetentionAuthority::new();
    let walk_view = chain.state().view();
    let walked = archive.qualify_against_certified_tip_with_interleaving_for_testing(
        &walk_view,
        chain.kura(),
        u64::MAX,
        |_| {
            let step = archive
                .reconcile_retention_with_limits(
                    &chain.state().query_view(),
                    chain.kura(),
                    &authority.binding(),
                    &authority,
                    &[],
                    ONE_CALL,
                )
                .unwrap();
            assert!(
                matches!(
                    step,
                    ProviderIngestFinalizedArchiveRetentionStepV1::Compacted(_)
                ),
                "{step:?}"
            );
            Ok(())
        },
    );
    assert!(
        matches!(
            walked,
            Err(ProviderIngestFinalizedArchiveErrorV1::QualificationBoundaryChanged { .. })
        ),
        "{walked:?}"
    );
    let identity = archive
        .read_index()
        .unwrap()
        .content_identity(&chain.network_id());
    assert_eq!(archive.authentication.covered_through(&identity), Some(6));
}

/// Qualification runs under the reader capture needs, on every tick, and a checkpoint can be tens
/// of megabytes. Proved history therefore leaves the file to the audit, as it does for records.
#[test]
fn the_audit_rereads_a_checkpoint_that_proved_qualification_leaves_alone() {
    let directory = physical_tempdir().unwrap();
    let mut chain = chain();
    let archive =
        ProviderIngestFinalizedArchiveV1::try_open(archive_root(&directory), small_bounds(8))
            .unwrap();
    capture_tip(&archive, &chain);
    commit_and_capture(&archive, &mut chain, 5);
    let authority = TestRetentionAuthority::new();
    assert!(matches!(
        step(&archive, &chain, &authority, &[]).unwrap(),
        ProviderIngestFinalizedArchiveRetentionStepV1::Compacted(_)
    ));
    prove(&archive, &chain);
    let network_id = chain.network_id();
    let checkpoint = archive
        .read_index()
        .unwrap()
        .virtual_bases
        .get(&network_id)
        .unwrap()
        .path
        .clone();
    let mut damaged = fs::read(&checkpoint).unwrap();
    let last = damaged.len() - 1;
    damaged[last] ^= 0xFF;
    fs::write(&checkpoint, damaged).unwrap();
    prove(&archive, &chain);
    archive.authentication.expire_audit_for_testing();
    assert!(
        archive
            .qualify_against_certified_tip(&chain.state().query_view(), chain.kura(), u64::MAX)
            .is_err(),
        "the audit's first slice reads the file"
    );
    let identity = archive.read_index().unwrap().content_identity(&network_id);
    assert_eq!(
        archive.authentication.covered_through(&identity),
        None,
        "the fault takes the proof back"
    );
}

/// The periodic audit copies entries and then reads their files with no lock held, so retention
/// can unlink one in between. That is not damage, and it must not cost proved coverage, which a
/// reader would pay for with a complete walk.
#[test]
fn the_audit_ignores_a_file_compacted_behind_it_but_not_a_retained_one() {
    let directory = physical_tempdir().unwrap();
    let mut chain = chain();
    let archive =
        ProviderIngestFinalizedArchiveV1::try_open(archive_root(&directory), small_bounds(8))
            .unwrap();
    capture_tip(&archive, &chain);
    commit_and_capture(&archive, &mut chain, 5);
    let network_id = chain.network_id();
    let qualified = archive
        .qualify_against_certified_tip(&chain.state().view(), chain.kura(), 0)
        .unwrap();
    let entries = archive
        .read_index()
        .unwrap()
        .by_height
        .iter()
        .map(|(subject, entry)| (*subject, entry.clone()))
        .collect::<Vec<_>>();
    let authority = TestRetentionAuthority::new();
    let compacted = step(&archive, &chain, &authority, &[]).unwrap();
    assert!(
        matches!(
            compacted,
            ProviderIngestFinalizedArchiveRetentionStepV1::Compacted(_)
        ),
        "{compacted:?}"
    );
    archive.verify_audit_entries(&entries).unwrap();
    let identity = archive.read_index().unwrap().content_identity(&network_id);
    assert_eq!(
        archive.authentication.covered_through(&identity),
        Some(qualified.archive_tip().height),
        "compaction and the audit both keep proved coverage"
    );

    let retained = entries
        .iter()
        .rfind(|(subject, _)| subject.1 == qualified.archive_tip().height)
        .unwrap();
    fs::write(&retained.1.path, [0_u8]).unwrap();
    assert!(archive.verify_audit_entries(&entries).is_err());
    assert_eq!(
        archive.authentication.covered_through(&identity),
        None,
        "a retained record that changed is still damage"
    );
}
