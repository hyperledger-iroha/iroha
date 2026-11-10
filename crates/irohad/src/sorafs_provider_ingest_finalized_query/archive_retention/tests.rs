//! Deterministic archive retention, restart and admission-lease regression tests.

use std::sync::atomic::{AtomicUsize, Ordering};

use iroha_core::{
    query::provider_ingest_finalized::{
        ProviderIngestFinalizedArchiveRetentionApprovalRecordV1,
        ProviderIngestFinalizedArchiveRetentionAuthorityExternalErrorV1,
        ProviderIngestFinalizedArchiveRetentionAuthorityQualificationV1,
        ProviderIngestFinalizedArchiveRetentionAuthorityV1,
    },
    state::World,
    sumeragi::test_chain::{CertifiedTestChain, TestChainConfig},
};
use iroha_data_model::{NetworkId, sorafs::capacity::ProviderId};

use super::{
    super::{
        ActiveArchiveScanV1, PreparedProviderIngestFinalizedArchiveV1,
        ProviderIngestFinalizedArchiveCursorV1, ProviderIngestFinalizedArchiveKeyV1,
        ProviderIngestFinalizedCursorV1, ProviderIngestFinalizedLedgerErrorV1, ReplicationOrderId,
        prepare_provider_ingest_finalized_archive_v1,
    },
    *,
};

const HANDLE: &str = "sealed://sorafs/provider-ingest/archive-retention-primary";
const REVISION: u64 = 7;
const POLICY_DIGEST: [u8; 32] = [0xA7; 32];

#[derive(Debug, Default)]
struct SealedAuthority {
    latest: Mutex<Option<ProviderIngestFinalizedArchiveRetentionApprovalRecordV1>>,
    obstruct_publication_in: Mutex<Option<std::path::PathBuf>>,
    obstruction_path: Mutex<Option<std::path::PathBuf>>,
}

impl ProviderIngestFinalizedArchiveRetentionAuthorityV1 for SealedAuthority {
    fn handle(&self) -> &str {
        HANDLE
    }
    fn qualification(
        &self,
    ) -> std::result::Result<
        ProviderIngestFinalizedArchiveRetentionAuthorityQualificationV1,
        ProviderIngestFinalizedArchiveRetentionAuthorityExternalErrorV1,
    > {
        Ok(
            ProviderIngestFinalizedArchiveRetentionAuthorityQualificationV1::new(
                REVISION,
                POLICY_DIGEST,
            ),
        )
    }
    fn load_latest(
        &self,
        _network_id: &NetworkId,
    ) -> std::result::Result<
        Option<ProviderIngestFinalizedArchiveRetentionApprovalRecordV1>,
        ProviderIngestFinalizedArchiveRetentionAuthorityExternalErrorV1,
    > {
        Ok(self.latest.lock().unwrap().clone())
    }
    fn compare_and_swap_latest(
        &self,
        _network_id: &NetworkId,
        _expected_revision: Option<[u8; 32]>,
        next: &ProviderIngestFinalizedArchiveRetentionApprovalRecordV1,
    ) -> std::result::Result<(), ProviderIngestFinalizedArchiveRetentionAuthorityExternalErrorV1>
    {
        *self.latest.lock().unwrap() = Some(next.clone());
        if let Some(directory) = self.obstruct_publication_in.lock().unwrap().take() {
            let path = directory.join(format!(
                "{}.provider-ingest-base.to",
                hex::encode(next.proposal().checkpoint_digest())
            ));
            std::fs::write(&path, [0_u8]).unwrap();
            *self.obstruction_path.lock().unwrap() = Some(path);
        }
        Ok(())
    }
}

fn config(
    retention: bool,
) -> iroha_config::parameters::actual::SorafsProviderIngestFinalizedArchive {
    iroha_config::parameters::actual::SorafsProviderIngestFinalizedArchive {
        relative_root: std::path::PathBuf::from("provider-ingest-finalized-v1"),
        max_record_bytes: 2 * 1024 * 1024,
        max_archive_entries: 8,
        max_total_bytes: 16 * 1024 * 1024,
        max_providers_per_anchor: 8,
        max_orders_per_provider: 8,
        max_total_orders_per_anchor: 16,
        max_page_rows: 2,
        max_kura_tip_lag_blocks: 0,
        retention_authority: retention.then(|| {
            iroha_config::parameters::actual::SorafsProviderIngestFinalizedArchiveRetentionAuthority {
                handle: HANDLE.to_owned(),
                revision: REVISION,
                policy_digest: POLICY_DIGEST,
            }
        }),
    }
}

fn physical_tempdir() -> tempfile::TempDir {
    tempfile::Builder::new()
        .prefix("irohad-provider-archive-retention-")
        .tempdir_in(std::env::temp_dir().canonicalize().unwrap())
        .unwrap()
}

fn chain() -> CertifiedTestChain {
    CertifiedTestChain::start(TestChainConfig::new(World::new(), 1_000)).unwrap()
}

fn prepare(
    chain: &CertifiedTestChain,
    root: &tempfile::TempDir,
    authority: Option<Arc<SealedAuthority>>,
) -> PreparedProviderIngestFinalizedArchiveV1 {
    let retention = authority.is_some();
    prepare_provider_ingest_finalized_archive_v1(
        &config(retention),
        chain.network_id(),
        ProviderId::new([0x51; 32]),
        root.path(),
        chain.state(),
        chain.kura(),
        &[],
        authority.map(|authority| {
            let authority: Arc<dyn ProviderIngestFinalizedArchiveRetentionAuthorityV1> = authority;
            authority
        }),
    )
    .unwrap()
}

fn worker(
    prepared: &PreparedProviderIngestFinalizedArchiveV1,
    chain: &CertifiedTestChain,
    pins: ProviderIngestArchivePinsV1,
) -> ProviderIngestArchiveRetentionWorkerV1 {
    ProviderIngestArchiveRetentionWorkerV1::new(
        Arc::clone(prepared.archive()),
        Arc::clone(chain.state()),
        Arc::clone(chain.kura()),
        prepared.retention_authority().cloned(),
        pins,
        Arc::clone(&prepared.query.anchor_leases),
    )
}

fn no_pins() -> ProviderIngestArchivePinsV1 {
    Arc::new(|| Ok(ProviderIngestArchivePinsSnapshotV1::default()))
}

fn commit_and_capture(
    prepared: &PreparedProviderIngestFinalizedArchiveV1,
    chain: &mut CertifiedTestChain,
    blocks: usize,
) {
    for _ in 0..blocks {
        chain.commit(Vec::new());
        prepared
            .archive()
            .capture_certified_view(&chain.state().view(), chain.kura())
            .unwrap();
    }
}

/// The daemon's worker step, over an archive from the daemon's own preparation, carries it
/// past its entry bound and leaves it reopening through the daemon's own startup.
#[test]
fn the_daemon_worker_keeps_a_configured_archive_below_its_bound() {
    let root = physical_tempdir();
    let mut chain = chain();
    let authority = Arc::new(SealedAuthority::default());
    let prepared = prepare(&chain, &root, Some(Arc::clone(&authority)));
    let worker = worker(&prepared, &chain, no_pins());
    let mut compactions = 0;
    for _ in 0..20 {
        commit_and_capture(&prepared, &mut chain, 1);
        if matches!(
            worker.tick().unwrap(),
            RetentionTickV1::Step(ProviderIngestFinalizedArchiveRetentionStepV1::Compacted(_))
        ) {
            compactions += 1;
        }
        assert!(prepared.archive().capacity().unwrap().retained_entries() < 8);
    }
    assert!(compactions > 1, "twenty blocks cross a bound of eight");
    let qualification = prepared.query().qualify_live().unwrap();
    assert_eq!(qualification.archive_tip().height, 21);
    assert!(qualification.activation_floor().height > 1);
    assert!(prepared.runtime_query().activation_ready().unwrap());
    drop(worker);
    drop(prepared);
    let reopened = prepare(&chain, &root, Some(authority));
    assert!(reopened.runtime_query().activation_ready().unwrap());
}

/// A scan can stay quiet past its pin's lifetime and still be running. If retention then
/// passes its anchor, both readers say so, and the runtime's reader lets a fresh scan start.
#[test]
fn a_scan_retention_passed_is_told_so_and_can_start_again() {
    let root = physical_tempdir();
    let mut chain = chain();
    let prepared = prepare(&chain, &root, Some(Arc::new(SealedAuthority::default())));
    let worker = worker(&prepared, &chain, no_pins());
    commit_and_capture(&prepared, &mut chain, 5);
    assert!(matches!(
        worker.tick().unwrap(),
        RetentionTickV1::Step(ProviderIngestFinalizedArchiveRetentionStepV1::Compacted(_))
    ));
    let floor = prepared
        .query()
        .qualify_live()
        .unwrap()
        .activation_floor()
        .height;
    let retired = chain.committed(floor - 1);
    let public_cursor = ProviderIngestFinalizedCursorV1 {
        height: floor - 1,
        block_hash: *retired.block_hash().as_ref(),
    };
    let key = ProviderIngestFinalizedArchiveKeyV1::try_new(
        chain.network_id(),
        public_cursor.height,
        public_cursor.block_hash,
        retired.block_time_ms(),
    )
    .unwrap();
    let after_order_id = [0x54; 32];
    let runtime = prepared.runtime_query();
    *runtime.active.lock().unwrap() = Some(ActiveArchiveScanV1 {
        key,
        cursor: ProviderIngestFinalizedArchiveCursorV1 {
            key,
            provider_id: ProviderId::new([0x51; 32]),
            provider_state_root: [0x53; 32],
            after_order_id: ReplicationOrderId::new(after_order_id),
        },
    });
    assert_eq!(
        runtime.read_page_without_musubi_claims_for_test(None, None, 1),
        Err(ProviderIngestFinalizedLedgerErrorV1::Rejected),
        "a scan still under way keeps its cursor until it finds out"
    );
    assert_eq!(
        runtime.read_page_without_musubi_claims_for_test(
            Some(public_cursor),
            Some(after_order_id),
            1
        ),
        Err(ProviderIngestFinalizedLedgerErrorV1::ScanAnchorRetired)
    );
    assert!(runtime.active.lock().unwrap().is_none());
    runtime
        .read_page_without_musubi_claims_for_test(None, None, 1)
        .expect("a fresh scan reads the current head");
    let capture = prepared
        .signed_capture_reader
        .as_ref()
        .expect("prepared signed capture reader");
    assert_eq!(
        capture.read_replay_safe_capture_source_page(Some(public_cursor), Some(after_order_id), 1),
        Err(ProviderIngestFinalizedLedgerErrorV1::ScanAnchorRetired)
    );
}

#[test]
fn a_pinned_height_holds_the_daemon_worker_back() {
    let root = physical_tempdir();
    let mut chain = chain();
    let prepared = prepare(&chain, &root, Some(Arc::new(SealedAuthority::default())));
    let pinned = Arc::new(Mutex::new(vec![2_u64]));
    let pins: ProviderIngestArchivePinsV1 = {
        let pinned = Arc::clone(&pinned);
        Arc::new(move || Ok(pinned.lock().unwrap().clone().into()))
    };
    let worker = worker(&prepared, &chain, pins);
    commit_and_capture(&prepared, &mut chain, 5);
    assert!(matches!(
        worker.tick().unwrap(),
        RetentionTickV1::Step(ProviderIngestFinalizedArchiveRetentionStepV1::Compacted(_))
    ));
    assert_eq!(
        prepared
            .query()
            .qualify_live()
            .unwrap()
            .activation_floor()
            .height,
        2
    );
    assert!(matches!(
        worker.tick().unwrap(),
        RetentionTickV1::Step(ProviderIngestFinalizedArchiveRetentionStepV1::Pinned {
            pinned_height: 2,
            ..
        })
    ));
    pinned.lock().unwrap().clear();
    assert!(matches!(
        worker.tick().unwrap(),
        RetentionTickV1::Step(ProviderIngestFinalizedArchiveRetentionStepV1::Compacted(_))
    ));
}

/// Not knowing what is still read must stop deletion, not be read as "nothing".
#[test]
fn an_unreadable_pin_source_deletes_nothing() {
    let root = physical_tempdir();
    let mut chain = chain();
    let authority = Arc::new(SealedAuthority::default());
    let prepared = prepare(&chain, &root, Some(Arc::clone(&authority)));
    let worker = worker(
        &prepared,
        &chain,
        Arc::new(|| Err(eyre!("outbox unavailable"))),
    );
    commit_and_capture(&prepared, &mut chain, 5);
    assert!(worker.tick().is_err());
    assert!(authority.latest.lock().unwrap().is_none());
    assert_eq!(
        prepared
            .query()
            .qualify_live()
            .unwrap()
            .activation_floor()
            .height,
        1
    );
}

/// Manual retention stays manual: the worker reports and never touches the archive.
#[test]
fn without_an_authority_the_daemon_worker_only_reports_capacity() {
    let root = physical_tempdir();
    let mut chain = chain();
    let prepared = prepare(&chain, &root, None);
    let worker = worker(&prepared, &chain, no_pins());
    commit_and_capture(&prepared, &mut chain, 5);
    let RetentionTickV1::Manual(capacity) = worker.tick().unwrap() else {
        panic!("no authority is configured");
    };
    assert_eq!(capacity.retained_entries(), 6);
    assert_eq!(capacity.maximum_entries(), 8);
    assert!(capacity.at_high_watermark());
    assert_eq!(
        prepared
            .query()
            .qualify_live()
            .unwrap()
            .activation_floor()
            .height,
        1
    );
}

fn build_to_the_bound_and_hold_one_commit(
    prepared: &PreparedProviderIngestFinalizedArchiveV1,
    chain: &mut CertifiedTestChain,
) {
    commit_and_capture(prepared, chain, 7);
    assert_eq!(prepared.archive().capacity().unwrap().retained_entries(), 8);
    chain.commit(Vec::new());
}

/// The node holds a commit at its bound and restarts. Startup frees room itself, because this
/// worker only starts after that capture.
#[test]
fn a_restart_at_the_bound_starts_with_an_authority() {
    let root = physical_tempdir();
    let mut chain = chain();
    let authority = Arc::new(SealedAuthority::default());
    let prepared = prepare(&chain, &root, Some(Arc::clone(&authority)));
    build_to_the_bound_and_hold_one_commit(&prepared, &mut chain);
    drop(prepared);
    let reopened = prepare(&chain, &root, Some(authority));
    let qualification = reopened.query().qualify_live().unwrap();
    assert_eq!(qualification.archive_tip().height, 9);
    assert!(qualification.activation_floor().height > 1);
}

/// Manual retention keeps its documented refusal: the operator's remedy is the namespace.
#[test]
fn a_restart_at_the_bound_refuses_without_an_authority() {
    let root = physical_tempdir();
    let mut chain = chain();
    let prepared = prepare(&chain, &root, None);
    build_to_the_bound_and_hold_one_commit(&prepared, &mut chain);
    drop(prepared);
    let refused = prepare_provider_ingest_finalized_archive_v1(
        &config(false),
        chain.network_id(),
        ProviderId::new([0x51; 32]),
        root.path(),
        chain.state(),
        chain.kura(),
        &[],
        None,
    );
    assert!(refused.is_err());
}

/// Each reader's latest page holds the floor, and an abandoned scan's hold expires, so a
/// wedged scan cannot pin history until the bound stops this node committing.
#[test]
fn a_scan_pins_its_latest_page_until_it_goes_quiet() {
    let root = physical_tempdir();
    let chain = chain();
    let prepared = prepare(&chain, &root, None);
    let slots = [
        Arc::clone(&prepared.query.scan_pin),
        Arc::clone(&prepared.runtime_query.scan_pin),
        Arc::clone(&prepared.capture_scan_pin),
    ];
    let pins = running_node_pins(|| Ok(vec![3, 9]), slots.clone());
    assert_eq!(
        pins().unwrap().into_heights(true),
        [3, 9],
        "no scan has read a page yet"
    );
    prepared
        .runtime_query
        .read_page_without_musubi_claims_for_test(None, None, 1)
        .unwrap();
    assert_eq!(
        pins().unwrap().into_heights(true),
        [3, 9, 1],
        "the runtime's page at the tip"
    );
    note_scan_page(&slots[2], 7);
    assert_eq!(
        pins().unwrap().into_heights(true),
        [3, 9, 1, 7],
        "the capture reader's page"
    );
    // Later pages replace an earlier one, and an old one stops counting.
    let later = Instant::now() + SCAN_PIN_LIFETIME;
    assert_eq!(recent_scan_height(&slots[2], later).unwrap(), None);
    assert_eq!(
        recent_scan_height(&slots[2], Instant::now()).unwrap(),
        Some(7)
    );
    note_scan_page(&slots[2], 8);
    assert_eq!(
        recent_scan_height(&slots[2], Instant::now()).unwrap(),
        Some(8)
    );
}

#[test]
fn approved_but_unpublished_floor_blocks_admission_until_exact_recovery() {
    let root = physical_tempdir();
    let mut chain = chain();
    let authority = Arc::new(SealedAuthority::default());
    let prepared = prepare(&chain, &root, Some(Arc::clone(&authority)));
    commit_and_capture(&prepared, &mut chain, 5);
    let first = chain.committed(1);
    let cursor = ProviderIngestFinalizedCursorV1 {
        height: 1,
        block_hash: *first.block_hash().as_ref(),
    };
    *authority.obstruct_publication_in.lock().unwrap() =
        Some(root.path().join("provider-ingest-finalized-v1/checkpoints"));
    let worker = worker(
        &prepared,
        &chain,
        running_node_pins(
            || Ok(Vec::new()),
            [
                Arc::clone(&prepared.query.scan_pin),
                Arc::clone(&prepared.runtime_query.scan_pin),
                Arc::clone(&prepared.capture_scan_pin),
            ],
        ),
    );
    assert!(worker.tick().is_err());
    assert!(authority.latest.lock().unwrap().is_some());
    // Local bytes still resolve, but the authority has already committed to retiring them.
    prepared
        .archive
        .resolve_exact_key(&chain.network_id(), cursor.height, cursor.block_hash)
        .unwrap();
    assert!(matches!(
        prepared.query.anchor_leases.retain_exact_cursor(
            prepared.archive.as_ref(),
            &chain.network_id(),
            cursor
        ),
        Err(ProviderIngestFinalizedLedgerErrorV1::Unavailable)
    ));
    note_scan_page(&prepared.query.scan_pin, 1);
    std::fs::remove_file(authority.obstruction_path.lock().unwrap().take().unwrap()).unwrap();
    assert!(matches!(
        worker.tick().unwrap(),
        RetentionTickV1::Step(ProviderIngestFinalizedArchiveRetentionStepV1::Compacted(_))
    ));
    assert!(matches!(
        prepared.query.anchor_leases.retain_exact_cursor(
            prepared.archive.as_ref(),
            &chain.network_id(),
            cursor
        ),
        Err(ProviderIngestFinalizedLedgerErrorV1::ScanAnchorRetired)
    ));
    let tip = chain.committed(6);
    let current = ProviderIngestFinalizedCursorV1 {
        height: 6,
        block_hash: *tip.block_hash().as_ref(),
    };
    drop(
        prepared
            .query
            .anchor_leases
            .retain_exact_cursor(prepared.archive.as_ref(), &chain.network_id(), current)
            .unwrap(),
    );
}

#[test]
fn pruning_wins_before_lease_acquisition_and_rejects_the_retired_page() {
    use std::sync::mpsc;
    let root = physical_tempdir();
    let mut chain = chain();
    let prepared = prepare(&chain, &root, Some(Arc::new(SealedAuthority::default())));
    commit_and_capture(&prepared, &mut chain, 5);
    let first = chain.committed(1);
    let cursor = ProviderIngestFinalizedCursorV1 {
        height: 1,
        block_hash: *first.block_hash().as_ref(),
    };
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let release_rx = Mutex::new(release_rx);
    let worker = worker(
        &prepared,
        &chain,
        Arc::new(move || {
            entered_tx.send(()).unwrap();
            release_rx
                .lock()
                .unwrap()
                .recv_timeout(Duration::from_secs(5))
                .unwrap();
            Ok(ProviderIngestArchivePinsSnapshotV1::default())
        }),
    );
    let pruning = std::thread::spawn(move || worker.tick());
    entered_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    assert!(
        prepared.query.anchor_leases.active.try_lock().is_err(),
        "retention must hold the lease fence before sampling the outbox"
    );
    let reader = Arc::clone(prepared.runtime_query());
    let (started_tx, started_rx) = mpsc::channel();
    let acquisition = std::thread::spawn(move || {
        started_tx.send(()).unwrap();
        reader.anchor_leases.retain_exact_cursor(
            reader.archive.as_ref(),
            &reader.network_id,
            cursor,
        )
    });
    started_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    release_tx.send(()).unwrap();
    assert!(matches!(
        pruning.join().unwrap().unwrap(),
        RetentionTickV1::Step(ProviderIngestFinalizedArchiveRetentionStepV1::Compacted(_))
    ));
    assert!(matches!(
        acquisition.join().unwrap(),
        Err(ProviderIngestFinalizedLedgerErrorV1::ScanAnchorRetired)
    ));
}

#[test]
fn lease_to_active_job_handoff_has_no_outbox_snapshot_gap() {
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        mpsc,
    };
    let root = physical_tempdir();
    let mut chain = chain();
    let prepared = prepare(&chain, &root, Some(Arc::new(SealedAuthority::default())));
    commit_and_capture(&prepared, &mut chain, 5);
    let first = chain.committed(1);
    let cursor = ProviderIngestFinalizedCursorV1 {
        height: 1,
        block_hash: *first.block_hash().as_ref(),
    };
    let lease = prepared
        .runtime_query
        .anchor_leases
        .retain_exact_cursor(prepared.archive.as_ref(), &chain.network_id(), cursor)
        .unwrap();
    // Model the worker's sealed active-job reader. The runtime test separately exercises
    // a real outbox enqueue while its source callback is held at a deterministic barrier.
    let active_jobs = Arc::new(Mutex::new(Vec::new()));
    let active_reader = Arc::clone(&active_jobs);
    let (sampled_tx, sampled_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let release_rx = Mutex::new(release_rx);
    let first_sample = AtomicBool::new(true);
    let worker = Arc::new(worker(
        &prepared,
        &chain,
        Arc::new(move || {
            let snapshot = active_reader.lock().unwrap().clone();
            if first_sample.swap(false, Ordering::SeqCst) {
                sampled_tx.send(()).unwrap();
                release_rx
                    .lock()
                    .unwrap()
                    .recv_timeout(Duration::from_secs(5))
                    .unwrap();
            }
            Ok(snapshot.into())
        }),
    ));
    let pruning_worker = Arc::clone(&worker);
    let pruning = std::thread::spawn(move || pruning_worker.tick());
    sampled_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    active_jobs.lock().unwrap().push(1);
    let (dropping_tx, dropping_rx) = mpsc::channel();
    let release_lease = std::thread::spawn(move || {
        dropping_tx.send(()).unwrap();
        drop(lease);
    });
    dropping_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    assert!(prepared.query.anchor_leases.active.try_lock().is_err());
    release_tx.send(()).unwrap();
    pruning.join().unwrap().unwrap();
    release_lease.join().unwrap();
    assert!(
        prepared
            .query
            .anchor_leases
            .active
            .lock()
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        prepared
            .archive
            .retention_floor(&chain.network_id())
            .unwrap()
            .unwrap()
            .height,
        1
    );
    assert!(matches!(
        worker.tick().unwrap(),
        RetentionTickV1::Step(ProviderIngestFinalizedArchiveRetentionStepV1::Pinned {
            pinned_height: 1,
            ..
        })
    ));
    active_jobs.lock().unwrap().clear();
    assert!(matches!(
        worker.tick().unwrap(),
        RetentionTickV1::Step(ProviderIngestFinalizedArchiveRetentionStepV1::Compacted(_))
    ));
}

#[test]
fn all_three_readers_share_nonexpiring_anchor_leases() {
    let root = physical_tempdir();
    let mut chain = chain();
    let prepared = prepare(&chain, &root, Some(Arc::new(SealedAuthority::default())));
    commit_and_capture(&prepared, &mut chain, 5);
    let capture = prepared.signed_capture_reader.as_ref().unwrap();
    assert!(Arc::ptr_eq(
        &prepared.query.anchor_leases,
        &prepared.runtime_query.anchor_leases
    ));
    assert!(Arc::ptr_eq(
        &prepared.query.anchor_leases,
        &capture.anchor_leases
    ));
    let first = chain.committed(1);
    let cursor = ProviderIngestFinalizedCursorV1 {
        height: 1,
        block_hash: *first.block_hash().as_ref(),
    };
    let mut leases = Vec::new();
    for reader in [
        prepared.query.as_ref(),
        prepared.runtime_query.as_ref(),
        capture,
    ] {
        *reader.scan_pin.lock().unwrap() = Some(ArchiveScanPinV1 {
            height: 1,
            read_at: Instant::now()
                .checked_sub(SCAN_PIN_LIFETIME + Duration::from_secs(1))
                .expect("test clock can represent an expired scan pin"),
        });
        leases.push(
            reader
                .anchor_leases
                .retain_exact_cursor(reader.archive.as_ref(), &reader.network_id, cursor)
                .unwrap(),
        );
    }
    assert_eq!(
        prepared.query.anchor_leases.active.lock().unwrap().get(&1),
        Some(&3)
    );
    let worker = worker(&prepared, &chain, no_pins());
    worker.tick().unwrap();
    assert_eq!(
        prepared
            .archive
            .retention_floor(&chain.network_id())
            .unwrap()
            .unwrap()
            .height,
        1
    );
    drop(leases);
    assert!(matches!(
        worker.tick().unwrap(),
        RetentionTickV1::Step(ProviderIngestFinalizedArchiveRetentionStepV1::Compacted(_))
    ));
}

#[test]
fn restored_exact_pending_anchor_survives_startup_capacity_recovery() {
    let root = physical_tempdir();
    let mut chain = chain();
    let authority = Arc::new(SealedAuthority::default());
    let prepared = prepare(&chain, &root, Some(Arc::clone(&authority)));
    commit_and_capture(&prepared, &mut chain, 7);
    let pending = chain.committed(2);
    let cursor = ProviderIngestFinalizedCursorV1 {
        height: 2,
        block_hash: *pending.block_hash().as_ref(),
    };
    chain.commit(Vec::new());
    drop(prepared);
    let runtime_authority: Arc<dyn ProviderIngestFinalizedArchiveRetentionAuthorityV1> = authority;
    let reopened = prepare_provider_ingest_finalized_archive_v1(
        &config(true),
        chain.network_id(),
        ProviderId::new([0x51; 32]),
        root.path(),
        chain.state(),
        chain.kura(),
        &[cursor],
        Some(runtime_authority),
    )
    .unwrap();
    assert_eq!(
        reopened.query.qualify_live().unwrap().archive_tip().height,
        9
    );
    assert_eq!(
        reopened
            .archive
            .resolve_exact_key(&chain.network_id(), cursor.height, cursor.block_hash)
            .unwrap()
            .height,
        2
    );
}

#[test]
fn substituted_or_already_retired_pending_anchor_refuses_before_startup_retention() {
    let root = physical_tempdir();
    let mut chain = chain();
    let authority = Arc::new(SealedAuthority::default());
    let prepared = prepare(&chain, &root, Some(Arc::clone(&authority)));
    commit_and_capture(&prepared, &mut chain, 5);
    let first = chain.committed(1);
    let retired = ProviderIngestFinalizedCursorV1 {
        height: 1,
        block_hash: *first.block_hash().as_ref(),
    };
    worker(&prepared, &chain, no_pins()).tick().unwrap();
    let head = authority.latest.lock().unwrap().clone();
    drop(prepared);
    for cursor in [
        retired,
        ProviderIngestFinalizedCursorV1 {
            height: 6,
            block_hash: [0xDA; 32],
        },
    ] {
        let runtime_authority: Arc<dyn ProviderIngestFinalizedArchiveRetentionAuthorityV1> =
            authority.clone();
        assert!(
            prepare_provider_ingest_finalized_archive_v1(
                &config(true),
                chain.network_id(),
                ProviderId::new([0x51; 32]),
                root.path(),
                chain.state(),
                chain.kura(),
                &[cursor],
                Some(runtime_authority),
            )
            .is_err()
        );
        assert_eq!(*authority.latest.lock().unwrap(), head);
    }
}

/// A failing or panicking step must leave the supervised task running: a child that exits
/// before shutdown takes the whole node down, for a condition that only matters at the bound.
#[tokio::test]
async fn the_supervised_worker_survives_failures_and_stops_on_shutdown() {
    let root = physical_tempdir();
    let chain = chain();
    let prepared = prepare(&chain, &root, Some(Arc::new(SealedAuthority::default())));
    let calls = Arc::new(AtomicUsize::new(0));
    let pins: ProviderIngestArchivePinsV1 = {
        let calls = Arc::clone(&calls);
        Arc::new(move || {
            assert_ne!(calls.fetch_add(1, Ordering::SeqCst), 0, "a panicking step");
            Err(eyre!("a failing step"))
        })
    };
    let shutdown = ShutdownSignal::new();
    let worker = Arc::new(worker(&prepared, &chain, pins));
    let task = tokio::spawn(run(Arc::clone(&worker), shutdown.clone()));
    tokio::time::timeout(Duration::from_secs(120), async {
        while !worker.anchor_leases.active.is_poisoned() {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .expect("the recovered panic poisons the retained admission fence");
    assert!(
        worker.tick().is_err(),
        "subsequent ticks fail closed on the poisoned pin registry"
    );
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert!(!task.is_finished());
    shutdown.send();
    tokio::time::timeout(Duration::from_secs(120), task)
        .await
        .expect("the worker stops on shutdown")
        .unwrap();
}

#[test]
fn a_lasting_warning_repeats_only_occasionally() {
    let mut reporter = RetentionReporterV1::default();
    let start = Instant::now();
    assert!(reporter.warning_due(start));
    assert!(!reporter.warning_due(start + Duration::from_secs(1)));
    assert!(reporter.warning_due(start + RETENTION_WARNING_INTERVAL));
}
