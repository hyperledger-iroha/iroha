//! Checkpoint images come from original publication even when persistence falls behind.

use super::*;
use crate::{
    runtime::catalog::additions::RUNTIME_LIFECYCLE,
    snapshot::{CapturedStateSnapshot, SNAPSHOT_CURRENT_FILE_NAME},
    state::World,
    sumeragi::test_chain::{CertifiedTestChain, TestChainConfig},
};
use iroha_data_model::parameter::{Parameter, RuntimeCheckpointPolicyV1};
use sha2::Digest as _;

pub(in crate::snapshot) const HOUR_MS: u64 = 60 * 60 * 1000;

fn committed_event(at_ms: u64) -> iroha_data_model::events::EventBox {
    use iroha_data_model::events::{
        EventBox,
        time::{TimeEvent, TimeInterval},
    };
    EventBox::Time(TimeEvent {
        interval: TimeInterval::new(
            std::time::Duration::from_millis(at_ms),
            std::time::Duration::from_millis(1),
        ),
    })
}

pub(in crate::snapshot) fn chain() -> CertifiedTestChain {
    let mut world = World::new();
    crate::runtime::keyed_state::install_for_test(&mut world);
    crate::runtime::activate_additions_for_testing(&world, RUNTIME_LIFECYCLE.module).unwrap();
    world.smart_contract_state.insert(
        "checkpoint-populated-state".parse().unwrap(),
        vec![0, 7, 255],
    );
    world.seed_external_events_for_snapshot_test(
        vec![committed_event(10)],
        vec![committed_event(20), committed_event(30)],
    );
    let mut config = TestChainConfig::new(world, 1_000);
    config.genesis_parameters.push(Parameter::Custom(
        RuntimeCheckpointPolicyV1 {
            cadence_ms: HOUR_MS,
            retain_all_ms: 3 * HOUR_MS,
            retain_spacing_ms: 24 * HOUR_MS,
            recovery_window_ms: 24 * HOUR_MS,
            floor_notice_ms: HOUR_MS,
        }
        .to_custom_parameter()
        .unwrap(),
    ));
    CertifiedTestChain::start(config).unwrap()
}

pub(in crate::snapshot) fn maker(
    state: std::sync::Arc<crate::state::State>,
    directory: &std::path::Path,
) -> crate::snapshot::SnapshotMaker {
    crate::snapshot::SnapshotMaker {
        state,
        create_every: std::time::Duration::from_secs(3600),
        store_dir: directory.to_path_buf(),
        latest_block_hash: None,
        signing_key: iroha_crypto::KeyPair::random(),
        merkle_chunk_size: NonZeroUsize::new(4096).unwrap(),
        max_payload_bytes: NonZeroUsize::new(64 * 1024 * 1024).unwrap(),
        resource_policy: Default::default(),
        read_buffer_budget: iroha_allocation::AllocationBudget::new(64 * 1024 * 1024),
        commit_evidence_refusal: None,
        checkpoints: Default::default(),
    }
}

pub(in crate::snapshot) fn commit_checkpoint(
    chain: &mut CertifiedTestChain,
    at_ms: u64,
    receiver: &mpsc::Receiver<Arc<CheckpointJob>>,
    maker: &crate::snapshot::SnapshotMaker,
) -> Arc<CheckpointJob> {
    use crate::sumeragi::test_chain::Signers;
    let proposal = chain.proposal(Some(at_ms), Vec::new());
    let mut pending = chain.begin_proposal(proposal, Default::default()).unwrap();
    let before = maker.state.committed_height();
    let refusal = pending
        .publish(Signers::Quorum)
        .expect_err("original waits for durable acknowledgement");
    assert!(refusal.contains("durable acknowledgement"), "{refusal}");
    assert_eq!(maker.state.committed_height(), before);
    let job = receiver
        .try_recv()
        .expect("exact original queued outside publication fences");
    job.acknowledge(maker.persist_pending_checkpoint(&job.capture));
    pending
        .publish(Signers::Quorum)
        .expect("same original publishes after durable acknowledgement");
    assert!(job.visible());
    job
}

fn assert_snapshot_equal(actual: &str, expected: &str) {
    if actual == expected {
        return;
    }
    let offset = actual
        .bytes()
        .zip(expected.bytes())
        .position(|(a, b)| a != b)
        .unwrap_or_else(|| actual.len().min(expected.len()));
    let excerpt = |text: &str| {
        String::from_utf8_lossy(
            &text.as_bytes()[offset.saturating_sub(100)..text.len().min(offset + 160)],
        )
        .into_owned()
    };
    fn differing_leaves(actual: &str, expected: &str, path: &str, output: &mut Vec<String>) {
        if actual == expected || output.len() == 12 {
            return;
        }
        if let (Ok(left), Ok(right)) = (
            crate::state::deserialize::json_object_members(actual),
            crate::state::deserialize::json_object_members(expected),
        ) {
            for (key, value) in &left {
                if let Some((_, other)) = right.iter().find(|(name, _)| name == key) {
                    differing_leaves(value, other, &format!("{path}.{key}"), output);
                } else if output.len() < 12 {
                    output.push(format!("{path}.{key}: absent from expected"));
                }
            }
            for (key, _) in &right {
                if !left.iter().any(|(name, _)| name == key) && output.len() < 12 {
                    output.push(format!("{path}.{key}: absent from actual"));
                }
            }
        } else {
            output.push(format!(
                "{path}: lengths {} vs {}, actual {:?}, expected {:?}",
                actual.len(),
                expected.len(),
                actual.chars().take(120).collect::<String>(),
                expected.chars().take(120).collect::<String>(),
            ));
        }
    }
    let mut differences = Vec::new();
    differing_leaves(actual, expected, "state", &mut differences);
    panic!(
        "checkpoint differs from published State at byte {offset} (lengths {} and {}): actual {:?}; expected {:?}; differing leaves: {differences:?}",
        actual.len(),
        expected.len(),
        excerpt(actual),
        excerpt(expected)
    );
}

#[test]
fn checkpoint_fixture_reports_original_serialized_extent() {
    let mut chain = chain();
    chain.commit_at(HOUR_MS, Vec::new());
    let original = CapturedStateSnapshot::capture(chain.state()).unwrap();
    let members = crate::state::deserialize::json_object_members(&original.json).unwrap();
    let world = members.iter().find(|(name, _)| *name == "world").unwrap().1;
    let mut fields: Vec<_> = crate::state::deserialize::json_object_members(world)
        .unwrap()
        .into_iter()
        .map(|(name, bytes)| (name, bytes.len()))
        .collect();
    fields.sort_by_key(|(_, bytes)| std::cmp::Reverse(*bytes));
    assert!(
        original.json.len() <= 64 * 1024 * 1024,
        "original complete fixture snapshot is {} bytes, exceeding its 64 MiB checkpoint policy; largest fields: {:?}",
        original.json.len(),
        &fields[..fields.len().min(5)]
    );
}

#[test]
fn checkpoint_integrity_failure_halts_with_original_cause_and_image_retained() {
    use crate::sumeragi::{
        driver::traits::PublicationError, executor::publication::original_refusal,
        test_chain::Signers,
    };
    let mut chain = chain();
    let state = Arc::clone(chain.state());
    let receiver = state
        .runtime_checkpoint_capture
        .start(NonZeroUsize::new(64 * 1024 * 1024).unwrap());
    let directory = tempfile::tempdir().unwrap();
    let writer = maker(Arc::clone(&state), directory.path());
    let proposal = chain.proposal(Some(HOUR_MS), Vec::new());
    let mut original = chain.begin_proposal(proposal, Default::default()).unwrap();
    let result = original.result();
    original.publish(Signers::Quorum).unwrap_err();
    let job = receiver.try_recv().unwrap();
    let image = job.capture.snapshot.json.as_ptr();
    writer.persist_pending_checkpoint(&job.capture).unwrap();
    let pending = super::super::checkpoint_pending::entries(directory.path())
        .unwrap()
        .remove(0);
    let signature = pending
        .path
        .join(crate::snapshot::SNAPSHOT_SIGNATURE_FILE_NAME);
    let authentic = std::fs::read(&signature).unwrap();
    std::fs::write(&signature, b"00").unwrap();
    let failure = writer.reconcile_pending_checkpoints(None).unwrap_err();
    assert!(
        matches!(&failure, CheckpointWriteError::Read(crate::snapshot::TryReadError::SignatureMalformed(value)) if value == "00")
    );
    job.acknowledge(Err(failure));
    assert!(!job.needs_write());
    let mut terminal = None;
    assert_eq!(
        crate::test_allocations::allocations_during(|| {
            terminal = Some(original_refusal(job.take_refusal().unwrap()));
        }),
        0
    );
    assert!(
        matches!(terminal, Some(PublicationError::RecoveryRequired(message)) if message.contains("00"))
    );
    assert!(matches!(&*job.phase.lock(), CheckpointPhase::Corrupt {
        _original: CheckpointWriteError::Read(crate::snapshot::TryReadError::SignatureMalformed(value)),
        diagnostic: None,
    } if value == "00"));
    // Restoring local bytes cannot authorize reusing a terminal publication owner.
    std::fs::write(signature, authentic).unwrap();
    let failure = original.publish(Signers::Quorum).unwrap_err();
    assert!(failure.contains("recovery required"), "{failure}");
    assert!(!job.needs_write());
    assert!(!job.visible());
    assert_eq!(job.capture.snapshot.json.as_ptr(), image);
    assert_eq!(original.result(), result);
    assert_eq!(state.committed_height(), 1);
    assert_eq!(state.exact_durable_block_count().unwrap(), 2);
    assert!(
        super::super::checkpoint_store::entries(directory.path())
            .unwrap()
            .is_empty()
    );
    state.runtime_checkpoint_capture.stop();
}

#[test]
fn later_publications_cannot_replace_any_queued_checkpoint_image() {
    let mut chain = chain();
    let receiver = chain
        .state()
        .runtime_checkpoint_capture
        .start(NonZeroUsize::new(64 * 1024 * 1024).unwrap());
    let pending_directory = tempfile::tempdir().unwrap();
    let maker = maker(Arc::clone(chain.state()), pending_directory.path());
    let first_job = commit_checkpoint(&mut chain, HOUR_MS, &receiver, &maker);
    let first = &first_job.capture;
    let first_height = first.height;
    assert_snapshot_equal(
        &first.snapshot.json,
        &CapturedStateSnapshot::capture(chain.state()).unwrap().json,
    );
    let fields = crate::state::deserialize::json_object_members(&first.snapshot.json).unwrap();
    let world = fields.iter().find(|(key, _)| *key == "world").unwrap().1;
    let members = crate::state::deserialize::json_object_members(world).unwrap();
    let events = members
        .iter()
        .find(|(key, _)| *key == "external_event_buf")
        .unwrap()
        .1;
    let previous = norito::json::to_json(&vec![committed_event(10)]).unwrap();
    let current = norito::json::to_json(&vec![committed_event(20), committed_event(30)]).unwrap();
    assert_eq!(
        events,
        format!("{{\"revert\":{{\"value\":{previous}}},\"blocks\":{current}}}")
    );

    // The writer still owns the first image; several more blocks commit before it writes.
    for time in [HOUR_MS + 1, HOUR_MS + 2, HOUR_MS + 3] {
        chain.commit_at(time, Vec::new());
    }
    assert!(
        receiver.try_recv().is_err(),
        "ordinary heights are not checkpoints"
    );
    let second_job = commit_checkpoint(&mut chain, 2 * HOUR_MS, &receiver, &maker);
    let second_expected = CapturedStateSnapshot::capture(chain.state()).unwrap().json;
    chain.commit_at(2 * HOUR_MS + 1, Vec::new());
    let second = &second_job.capture;
    assert_snapshot_equal(&second.snapshot.json, &second_expected);
    assert!(second.height > first_height);
    assert_eq!(first.snapshot.identity.height as u64, first_height);
    assert_ne!(first.snapshot.json, second.snapshot.json);
    assert_eq!(first.committed_ms, HOUR_MS);
    assert_eq!(second.committed_ms, 2 * HOUR_MS);

    // Restore the populated original after its certified successor already exists.
    // This checks the complete published State root, not only JSON equality.
    let source = chain.state();
    let mut restored = crate::state::deserialize::KuraSeed {
        execution_budget: iroha_allocation::AllocationBudget::new(
            iroha_config::parameters::defaults::pipeline::IVM_EXECUTION_MAX_BYTES,
        ),
        lane_manifests: source.lane_manifests.read().clone(),
        kura: std::sync::Arc::clone(chain.kura()),
        query_handle: crate::query::store::LiveQueryStore::start_test(),
        #[cfg(feature = "telemetry")]
        telemetry: crate::telemetry::StateTelemetry::default(),
    }
    .into_state_from_json_str_with_configured_nexus(&first.snapshot.json, source.nexus_snapshot())
    .expect("original checkpoint has the current canonical restore layout");
    restored.set_gov(source.gov.clone());
    restored.set_pipeline(iroha_config::parameters::actual::Pipeline::default());
    restored.install_node_fraud_monitoring(iroha_config::parameters::actual::FraudMonitoring::default());
    assert!(restored.authenticate_restored_keyed_state().unwrap());
    assert_eq!(restored.committed_height() as u64, first_height);

    // Delayed durable publication authenticates the original older image, and must not
    // regress the rolling snapshot pointer to that older height.
    let directory = tempfile::tempdir().unwrap();
    let signing_key = iroha_crypto::KeyPair::random();
    crate::snapshot::try_write_snapshot_payload_with_limit_locked(
        chain.state(),
        directory.path(),
        &signing_key,
        (
            NonZeroUsize::new(4096).unwrap(),
            NonZeroUsize::new(64 * 1024 * 1024).unwrap(),
            iroha_config::parameters::actual::SnapshotResourcePolicy::default(),
        ),
        first.snapshot.json.as_bytes(),
        first.snapshot.identity.clone(),
        &iroha_allocation::AllocationBudget::new(64 * 1024 * 1024),
        false,
    )
    .expect("an exact retained checkpoint remains authentic after later commits");
    assert!(!directory.path().join(SNAPSHOT_CURRENT_FILE_NAME).exists());
    let generation = directory
        .path()
        .join(crate::snapshot::SNAPSHOT_GENERATIONS_DIR_NAME)
        .join(hex::encode(sha2::Sha256::digest(
            first.snapshot.json.as_bytes(),
        )));
    crate::snapshot::checkpoint_store::keep(
        directory.path(),
        &generation,
        first_height,
        first.committed_ms,
        NonZeroUsize::new(64 * 1024 * 1024).unwrap(),
        NonZeroUsize::new(4096).unwrap(),
    )
    .unwrap();
    // Retaining the checkpoint must not turn its immutable source into an unsafe
    // hard-linked file, and restart must discover it before any rolling pointer exists.
    let mut recovered = crate::snapshot::try_read_snapshot(
        &source.ivm_execution_budget(),
        directory.path(),
        chain.kura(),
        &source.lane_manifests.read().clone(),
        &source.nexus_snapshot(),
        crate::query::store::LiveQueryStore::start_test,
        crate::kura::BlockCount(source.committed_height()),
        NonZeroUsize::new(4096).unwrap(),
        signing_key.public_key(),
        source.chain_id_ref(),
        source.network_id_ref(),
        &source.zk_snapshot(),
        #[cfg(feature = "telemetry")]
        crate::telemetry::StateTelemetry::default(),
        &iroha_allocation::AllocationBudget::new(64 * 1024 * 1024),
    )
    .expect("strict startup discovers the original signed checkpoint without a rolling pointer");
    recovered.set_gov(source.gov.clone());
    recovered.set_pipeline(iroha_config::parameters::actual::Pipeline::default());
    recovered.install_node_fraud_monitoring(iroha_config::parameters::actual::FraudMonitoring::default());
    assert!(recovered.authenticate_restored_keyed_state().unwrap());
    assert_eq!(recovered.committed_height() as u64, first_height);
    assert!(!directory.path().join(SNAPSHOT_CURRENT_FILE_NAME).exists());
    chain.state().runtime_checkpoint_capture.stop();
}

#[test]
fn capture_is_inert_without_a_writer_or_without_the_selected_policy() {
    let mut chain = chain();
    chain.commit_at(HOUR_MS, Vec::new());
    let receiver = chain
        .state()
        .runtime_checkpoint_capture
        .start(NonZeroUsize::new(64 * 1024 * 1024).unwrap());
    // Starting the consumer must not manufacture captures from older live State.
    assert!(receiver.try_recv().is_err());
    chain.commit_at(HOUR_MS + 1, Vec::new());
    assert!(receiver.try_recv().is_err());
    chain.state().runtime_checkpoint_capture.stop();
    chain.commit_at(2 * HOUR_MS, Vec::new());
    assert!(receiver.try_recv().is_err());

    let mut baseline =
        CertifiedTestChain::start(TestChainConfig::new(World::new(), 1_000)).unwrap();
    let receiver = baseline
        .state()
        .runtime_checkpoint_capture
        .start(NonZeroUsize::new(64 * 1024 * 1024).unwrap());
    baseline.commit_at(HOUR_MS, Vec::new());
    assert!(
        receiver.try_recv().is_err(),
        "the baseline captures no governed checkpoints"
    );
    baseline.state().runtime_checkpoint_capture.stop();
}

#[test]
fn local_byte_limit_defers_the_same_original_publication_without_changing_state() {
    use crate::sumeragi::test_chain::Signers;
    let mut chain = chain();
    let state = std::sync::Arc::clone(chain.state());
    let receiver = state
        .runtime_checkpoint_capture
        .start(NonZeroUsize::new(1).unwrap());
    let before = CapturedStateSnapshot::capture(&state).unwrap().json;
    let proposal = chain.proposal(Some(HOUR_MS), Vec::new());
    let mut pending = chain.begin_proposal(proposal, Default::default()).unwrap();
    let result = pending.result();
    let mut original = None;
    for _ in 0..2 {
        let error = pending
            .publish(Signers::Quorum)
            .expect_err("node-local byte cap defers visibility");
        assert!(error.contains("capacity"), "{error}");
        let owner = pending
            .inspect_prepared(|view| std::ptr::from_ref(view.state) as usize)
            .unwrap();
        assert_eq!(
            *original.get_or_insert(owner),
            owner,
            "same original overlay retained"
        );
        assert_eq!(pending.result(), result);
        assert_eq!(CapturedStateSnapshot::capture(&state).unwrap().json, before);
        assert!(receiver.try_recv().is_err());
    }
    state
        .runtime_checkpoint_capture
        .0
        .lock()
        .as_mut()
        .unwrap()
        .max_payload_bytes = NonZeroUsize::new(64 * 1024 * 1024).unwrap();
    let refusal = pending
        .publish(Signers::Quorum)
        .expect_err("original now waits for durability");
    assert!(refusal.contains("durable acknowledgement"), "{refusal}");
    let job = receiver.try_recv().unwrap();
    let directory = tempfile::tempdir().unwrap();
    let maker = maker(Arc::clone(&state), directory.path());
    job.acknowledge(maker.persist_pending_checkpoint(&job.capture));
    pending
        .publish(Signers::Quorum)
        .expect("original publication retries after durable acknowledgement");
    drop(pending);
    let checkpoint = &job.capture;
    assert_snapshot_equal(
        &checkpoint.snapshot.json,
        &CapturedStateSnapshot::capture(&state).unwrap().json,
    );
    assert_ne!(checkpoint.snapshot.json, before);
    state.runtime_checkpoint_capture.stop();
}

#[test]
fn disconnected_checkpoint_consumer_defers_the_original_before_visibility() {
    use crate::sumeragi::test_chain::Signers;
    let mut chain = chain();
    let state = std::sync::Arc::clone(chain.state());
    let receiver = state
        .runtime_checkpoint_capture
        .start(NonZeroUsize::new(64 * 1024 * 1024).unwrap());
    drop(receiver);
    let proposal = chain.proposal(Some(HOUR_MS), Vec::new());
    let mut pending = chain.begin_proposal(proposal, Default::default()).unwrap();
    let error = pending
        .publish(Signers::Quorum)
        .expect_err("consumer loss cannot silently discard checkpoint custody");
    assert!(error.contains("no available writer"), "{error}");
    assert!(
        pending.publish(Signers::Quorum).is_err(),
        "disconnected writer remains a local retry refusal"
    );
    state.runtime_checkpoint_capture.stop();
}

#[test]
fn armed_capture_skips_replay_and_keeps_first_live_checkpoint_before_worker_runs() {
    use crate::sumeragi::test_chain::Signers;
    let mut chain = chain();
    let state = Arc::clone(chain.state());
    let receiver = state
        .runtime_checkpoint_capture
        .arm(NonZeroUsize::new(64 * 1024 * 1024).unwrap());
    chain.commit_at(HOUR_MS, Vec::new());
    assert!(
        receiver.try_recv().is_err(),
        "armed replay does not capture"
    );
    state.runtime_checkpoint_capture.activate();
    let before = CapturedStateSnapshot::capture(&state).unwrap().json;
    let proposal = chain.proposal(Some(2 * HOUR_MS), Vec::new());
    let mut pending = chain.begin_proposal(proposal, Default::default()).unwrap();
    let error = pending
        .publish(Signers::Quorum)
        .expect_err("startup worker is not ready");
    assert!(error.contains("writer startup"), "{error}");
    assert_eq!(CapturedStateSnapshot::capture(&state).unwrap().json, before);
    assert!(receiver.try_recv().is_err());
    state.runtime_checkpoint_capture.writer_ready();
    let error = pending
        .publish(Signers::Quorum)
        .expect_err("same original awaits durability");
    assert!(error.contains("durable acknowledgement"), "{error}");
    let job = receiver.try_recv().unwrap();
    let directory = tempfile::tempdir().unwrap();
    let maker = maker(Arc::clone(&state), directory.path());
    job.acknowledge(maker.persist_pending_checkpoint(&job.capture));
    pending.publish(Signers::Quorum).unwrap();
    drop(pending);
    assert_snapshot_equal(
        &job.capture.snapshot.json,
        &CapturedStateSnapshot::capture(&state).unwrap().json,
    );
    chain.commit_at(2 * HOUR_MS + 1, Vec::new());
    assert_eq!(job.capture.committed_ms, 2 * HOUR_MS);
    state.runtime_checkpoint_capture.stop();
}

#[test]
fn checkpoint_acknowledgement_keeps_disk_and_rolling_writers_live() {
    use crate::sumeragi::test_chain::Signers;
    use std::{thread, time::Duration};
    // The deadline bounds only the concurrent phases under test: the disk writer and the
    // rolling capture must finish while the original publisher stays frozen. Fixture
    // construction and the first execution run on the producer alone, before any other
    // thread of this test exists, so they cannot deadlock against it and are not timed.
    let timeout = Duration::from_secs(20);
    let (setup, ready) = mpsc::channel();
    let (refused, refusal) = mpsc::channel();
    let (advance, advanced) = mpsc::channel();
    let (published, publication) = mpsc::channel();
    let producer = thread::spawn(move || {
        let mut chain = chain();
        let state = Arc::clone(chain.state());
        let receiver = state
            .runtime_checkpoint_capture
            .start(NonZeroUsize::new(64 * 1024 * 1024).unwrap());
        setup.send((state, receiver)).unwrap();
        let proposal = chain.proposal(Some(HOUR_MS), Vec::new());
        let mut pending = chain.begin_proposal(proposal, Default::default()).unwrap();
        assert!(
            pending
                .publish(Signers::Quorum)
                .unwrap_err()
                .contains("durable acknowledgement")
        );
        refused.send(()).unwrap();
        advanced.recv().unwrap();
        let _ = published.send(pending.publish(Signers::Quorum).map(|_| ()));
    });
    // A producer panic drops both senders, so these waits fail instead of hanging.
    let (state, receiver) = ready.recv().unwrap();
    refusal.recv().unwrap();
    let job = receiver
        .try_recv()
        .expect("the refused publication queued its exact original image before returning");
    let original_height = state.committed_height();
    assert_eq!(job.capture.height, original_height as u64 + 1);
    let directory = tempfile::tempdir().unwrap();
    let store = directory.path().to_path_buf();
    let worker_state = Arc::clone(&state);
    let worker_job = Arc::clone(&job);
    let (written, disk_result) = mpsc::channel();
    let writer = thread::spawn(move || {
        let mut maker = maker(worker_state, &store);
        let result = maker.persist_pending_checkpoint(&worker_job.capture);
        let written_ok = result.is_ok();
        worker_job.acknowledge(result);
        // The original publisher is still frozen and waiting; rolling capture must run.
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async {
            maker.create_snapshot();
        });
        written.send((written_ok, maker)).unwrap();
    });
    let (written_ok, maker) = disk_result.recv_timeout(timeout).unwrap();
    assert!(written_ok);
    assert_eq!(state.committed_height(), original_height);
    assert!(
        crate::snapshot::checkpoint_store::entries(directory.path())
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        crate::snapshot::checkpoint_pending::entries(directory.path())
            .unwrap()
            .len(),
        1
    );
    advance.send(()).unwrap();
    assert_eq!(publication.recv_timeout(timeout).unwrap(), Ok(()));
    producer.join().unwrap();
    writer.join().unwrap();
    assert!(job.visible());
    maker.promote_visible_checkpoint(&job.capture).unwrap();
    assert!(
        crate::snapshot::checkpoint_pending::entries(directory.path())
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        crate::snapshot::checkpoint_store::entries(directory.path()).unwrap()[0].0,
        job.capture.height
    );
    state.runtime_checkpoint_capture.stop();
}
