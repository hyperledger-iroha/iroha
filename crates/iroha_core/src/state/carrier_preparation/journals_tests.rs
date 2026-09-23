//! Actual candidate journal ownership, drop and resource-admission controls.

use super::*;
use crate::query::provider_ingest_finalized::ProviderIngestFinalizedArchiveBoundsV1;
use mv::storage::StorageReadOnly;

std::thread_local! {
    static EFFECTS_ALLOCATION_ATTEMPTS: std::cell::Cell<usize> = const {
        std::cell::Cell::new(0)
    };
}

// Observe only entry into the real Box allocation expression. This is not a
// memory-accounting policy or a claim about nested effects allocations.
pub(super) fn observe_effects_allocation_attempt() {
    EFFECTS_ALLOCATION_ATTEMPTS.with(|attempts| attempts.set(attempts.get() + 1));
}

fn reserve_provider_for_test(
    archive: &Arc<ProviderIngestFinalizedArchiveV1>,
    state: &State,
    proposal: &iroha_data_model::block::SignedBlock,
    context: &iroha_data_model::block::consensus_v2::HeightContext,
) -> ProviderCandidateCapture {
    archive
        .try_reserve_candidate(
            crate::query::provider_ingest_finalized::ProviderIngestFinalizedArchiveKeyV1::try_new(
                context.network_id,
                proposal.header().height().get(),
                *proposal.hash().as_ref(),
                proposal.header().creation_time_ms,
            )
            .unwrap(),
            &state.kura,
        )
        .unwrap()
}

fn reserve_reputation_for_test(
    archive: &Arc<ReputationFinalizedArchive>,
    state: &State,
    proposal: &iroha_data_model::block::SignedBlock,
    context: &iroha_data_model::block::consensus_v2::HeightContext,
) -> ReputationCandidateCapture {
    archive
        .try_reserve_candidate(
            crate::query::reputation_finalized::ReputationFinalizedArchiveKeyV1::try_new(
                context.network_id,
                proposal.header().height().get(),
                *proposal.hash().as_ref(),
            )
            .unwrap(),
            proposal.header().creation_time_ms,
            &state.kura,
        )
        .unwrap()
}

fn admit_runtime_for_test(
    inputs: RuntimeJournalInputs<'_, '_>,
) -> Result<(), std::convert::Infallible> {
    let mode = inputs.canonical_runtime().mode();
    assert_eq!(inputs.commit_topology().mode(), mode);
    assert_eq!(inputs.prev_commit_topology().mode(), mode);
    assert_eq!(inputs.lane_consensus_contexts().mode(), mode);
    Ok(())
}

fn admit_journals_for_test(
    inputs: CarrierJournalInputs<'_, '_>,
) -> Result<(), std::convert::Infallible> {
    let state = inputs.state;
    assert_eq!(inputs.prefix.sources().proposal(), state._curr_block.hash());
    assert_eq!(inputs.valid.as_ref().hash(), state._curr_block.hash());
    assert_eq!(inputs.context.height, state._curr_block.height().get());
    inputs
        .prefix
        .inventory()
        .verify_ordinary_witness_bundles(&inputs.prefix.witness().fastpq_transcripts)
        .unwrap();
    let mode = state.canonical_runtime.mode();
    assert_eq!(state.commit_topology.mode(), mode);
    assert_eq!(state.prev_commit_topology.mode(), mode);
    assert_eq!(state.lane_consensus_contexts.mode(), mode);
    Ok(())
}

fn enable_cold_tiered_capture(state: &mut State) -> tempfile::TempDir {
    let directory = tempfile::tempdir().unwrap();
    *state.tiered_backend.lock() = TieredStateBackend::new(
        true,
        0,
        0,
        0,
        Some(directory.path().to_path_buf()),
        None,
        0,
        0,
    );
    state.tiered_snapshot_worker = TieredSnapshotWorker::inert(
        Arc::clone(&state.tiered_backend),
        #[cfg(feature = "telemetry")]
        None,
    );
    directory
}

#[test]
fn retained_validation_match_binds_original_context_and_signed_proposal() {
    let (state, proposal, topology, context) = super::super::tests::fixture();
    let before = crate::snapshot::canonical_state_snapshot_hash(&state).unwrap();
    let journals = super::super::tests::prepare(&state, proposal.clone(), &topology, &context)
        .unwrap_or_else(|(_, error)| panic!("prepare original candidate: {error}"))
        .prepare_journals(None, None, admit_journals_for_test)
        .unwrap_or_else(|error| panic!("capture original candidate: {error}"));

    assert!(journals.matches_validation_candidate(&context, &proposal));
    assert!(journals.matches_validation_candidate(&context, journals.valid.as_ref()));
    let mut other_context = context.clone();
    other_context.roster[0].power += 1;
    assert!(!journals.matches_validation_candidate(&other_context, &proposal));

    let mut other_signed_proposal = proposal.clone();
    let key =
        iroha_crypto::KeyPair::try_from_seed(vec![0xFC; 32], iroha_crypto::Algorithm::BlsNormal)
            .unwrap();
    other_signed_proposal.sign(key.private_key(), 99);
    assert_eq!(proposal.hash(), other_signed_proposal.hash());
    assert!(!journals.matches_validation_candidate(&context, &other_signed_proposal));
    assert!(journals.matches_validation_candidate(&context, &proposal));
    drop(journals);
    assert_eq!(
        crate::snapshot::canonical_state_snapshot_hash(&state).unwrap(),
        before
    );
}

#[test]
fn journal_admission_refusal_prevents_cold_tiered_capture() {
    let (mut state, proposal, topology, context) = super::super::tests::fixture();
    let directory = enable_cold_tiered_capture(&mut state);
    let before = crate::snapshot::canonical_state_snapshot_hash(&state).unwrap();
    tiered_publication::capture_observer::observe(|counts| {
        let prepared = super::super::tests::prepare(&state, proposal, &topology, &context)
            .unwrap_or_else(|(_, error)| panic!("prepare cold candidate: {error}"));
        assert_eq!(counts.captured(), 0, "preparation must await admission");
        let error = prepared
            .prepare_journals(None, None, |original| {
                assert_eq!(counts.captured(), 0);
                assert_eq!(
                    original.state.world.musubi_resolver_index_checkpoints.len(),
                    1
                );
                Err::<(), _>("candidate memory exhausted")
            })
            .err()
            .expect("journal admission must refuse");
        assert!(matches!(
            error,
            CarrierJournalPreparationError::JournalAdmission {
                error: "candidate memory exhausted",
                ..
            }
        ));
        drop(error);
        assert_eq!(counts.captured(), 0, "refusal must not allocate a snapshot");
        assert_eq!(counts.released(), 0);
    });
    assert_eq!(
        crate::snapshot::canonical_state_snapshot_hash(&state).unwrap(),
        before
    );
    assert!(!state.tiered_backend.lock().snapshot_baseline_ready());
    assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 0);
    assert!(state.block_hashes.writer_available());
    drop(state.world.block());
}

#[test]
fn admitted_cold_tiered_capture_retains_exact_world_and_drops_before_reservation() {
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct Reservation {
        counts: Arc<tiered_publication::capture_observer::Counts>,
        snapshots_released_first: Arc<AtomicUsize>,
    }
    impl Drop for Reservation {
        fn drop(&mut self) {
            self.snapshots_released_first
                .store(self.counts.released(), Ordering::SeqCst);
        }
    }

    let (mut state, proposal, topology, context) = super::super::tests::fixture();
    let directory = enable_cold_tiered_capture(&mut state);
    let key: StatePath = "admitted-cold-baseline".parse().unwrap();
    state
        .world
        .smart_contract_state
        .insert(key.clone(), vec![1_u8, 2, 3]);
    let reference_directory = tempfile::tempdir().unwrap();
    let mut reference = TieredStateBackend::new(
        true,
        0,
        0,
        0,
        Some(reference_directory.path().to_path_buf()),
        None,
        0,
        0,
    );
    tiered_publication::capture_observer::observe(|counts| {
        let prepared = super::super::tests::prepare(&state, proposal.clone(), &topology, &context)
            .unwrap_or_else(|(_, error)| panic!("prepare cold candidate: {error}"));
        let events = prepared._publication_events.clone();
        let prefix = prepared.execution_prefix_commitment();
        let released_first = Arc::new(AtomicUsize::new(0));
        let journals = prepared
            .prepare_journals(None, None, |original| {
                assert_eq!(counts.captured(), 0);
                // Independent persistence of the exact immutable prepared World
                // provides the full baseline reference, including untouched keys.
                reference
                    .record_world_snapshot_with_payload(
                        &original
                            .state
                            .world
                            .tiered_snapshot_payload_with_scope(true),
                    )
                    .unwrap();
                admit_journals_for_test(original)?;
                Ok::<_, std::convert::Infallible>(Reservation {
                    counts: Arc::clone(&counts),
                    snapshots_released_first: Arc::clone(&released_first),
                })
            })
            .unwrap();
        assert_eq!(counts.captured(), 1);
        assert_eq!(counts.released(), 0);
        assert_eq!(journals.publication_events, events);
        assert_eq!(journals.execution_prefix_commitment(), prefix);
        assert_eq!(state.committed_height(), 0);
        assert_eq!(state.kura.blocks_count(), 0);
        assert!(!state.tiered_backend.lock().snapshot_baseline_ready());
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 0);
        let expected = norito::json::to_json(reference.last_manifest().unwrap()).unwrap();
        // Change the live World only after detachment: the retained snapshot
        // must contain the prepared candidate, never substitute current values.
        let mut later = state.world.block();
        later.smart_contract_state.insert(key, vec![91_u8, 92, 93]);
        later.commit();
        {
            let mut backend = state.tiered_backend.lock();
            backend
                .record_world_snapshot_with_payload(
                    journals
                        .tiered_snapshot
                        .payload_for_test()
                        .expect("cold payload"),
                )
                .unwrap();
            assert_eq!(
                norito::json::to_json(backend.last_manifest().unwrap()).unwrap(),
                expected
            );
        }
        drop(journals);
        assert_eq!(counts.released(), 1);
        assert_eq!(released_first.load(Ordering::SeqCst), 1);
    });
}

#[test]
fn journal_admission_refusal_returns_original_carrier_and_archive_predecessor() {
    #[derive(Debug, PartialEq, Eq)]
    enum Capacity {
        Exhausted,
    }
    let (state, proposal, topology, context) = super::super::tests::fixture();
    let before = crate::snapshot::canonical_state_snapshot_hash(&state).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let directory_path = directory.path().canonicalize().unwrap();
    let bounds =
        ProviderIngestFinalizedArchiveBoundsV1::try_new(1 << 20, 16, 16 << 20, 16, 16, 256, 16)
            .unwrap();
    let archive = Arc::new(
        ProviderIngestFinalizedArchiveV1::try_open(directory_path.join("archive"), bounds).unwrap(),
    );
    let original_archive = reserve_provider_for_test(&archive, &state, &proposal, &context);
    let mut prepared = super::super::tests::prepare(&state, proposal.clone(), &topology, &context)
        .unwrap_or_else(|(_, error)| panic!("prepare candidate: {error}"));
    // A retained allocation can exceed its serialized contents. Admission must
    // inspect its actual capacity without reconstructing or shrinking the owner.
    assert!(!prepared._publication_events.is_empty());
    prepared.parts_mut()._publication_events.reserve(128);
    let events_pointer = prepared._publication_events.as_ptr();
    let events_capacity = prepared._publication_events.capacity();
    assert!(events_capacity > prepared._publication_events.len());
    let context_owner = Arc::clone(&prepared.context);
    let outputs_pointer = prepared.block().execution_outputs().as_ptr();
    assert!(!prepared.block().execution_outputs().is_empty());
    let manifest_root = prepared.native_amx_manifest.root();
    let manifest_entries = prepared.native_amx_manifest.entries().as_ptr();
    let da_pins_pointer = prepared._world_effects.admission_pins().as_ptr();
    let da_pins_capacity = prepared._world_effects.admission_pins().capacity();
    let mut called = false;
    let original_prefix = prepared.execution_prefix_commitment();
    let original_state_pointer = std::ptr::from_ref(prepared.state.as_ref());
    let effects_allocation_attempts = EFFECTS_ALLOCATION_ATTEMPTS.with(std::cell::Cell::get);
    let effects_layout = std::alloc::Layout::new::<RetainedCarrierEffects>();
    let inspect_original = |original: &CarrierJournalInputs<'_, '_>| {
        assert_eq!(original.retained_effects_layout, effects_layout);
        assert_eq!(
            EFFECTS_ALLOCATION_ATTEMPTS.with(std::cell::Cell::get),
            effects_allocation_attempts,
            "effects allocation must await successful original admission"
        );
        assert_eq!(std::ptr::from_ref(original.state), original_state_pointer);
        assert_eq!(
            original.valid.as_ref().execution_outputs().as_ptr(),
            outputs_pointer
        );
        assert!(Arc::ptr_eq(original.context, &context_owner));
        assert_eq!(*original.execution_prefix, original_prefix);
        assert_eq!(original.native_amx_manifest.root(), manifest_root);
        assert_eq!(
            original.native_amx_manifest.entries().as_ptr(),
            manifest_entries
        );
        assert_eq!(original.da_pins.as_ptr(), da_pins_pointer);
        assert_eq!(original.da_pins.capacity(), da_pins_capacity);
        assert_eq!(original.publication_events.as_ptr(), events_pointer);
        assert_eq!(original.publication_events.capacity(), events_capacity);
    };
    let result = prepared.prepare_journals(Some(original_archive), None, |original| {
        inspect_original(&original);
        assert!(original.provider.is_some());
        assert!(original.reputation.is_none());
        assert!(!called);
        called = true;
        assert_eq!(
            original.state.canonical_runtime.mode(),
            mv::BlockMode::Ordinary
        );
        assert_eq!(
            original.state.world.musubi_resolver_index_checkpoints.len(),
            1
        );
        assert_eq!(
            original.state.commit_topology.get(),
            &context
                .roster
                .iter()
                .map(|entry| entry.validator.clone())
                .collect::<Vec<_>>()
        );
        Err::<(), _>(Capacity::Exhausted)
    });
    let Err(CarrierJournalPreparationError::JournalAdmission {
        carrier,
        provider,
        reputation,
        error: Capacity::Exhausted,
    }) = result
    else {
        panic!("resource refusal returns every original owner");
    };
    assert_eq!(
        std::ptr::from_ref(carrier.state.as_ref()),
        original_state_pointer
    );
    assert_eq!(carrier.execution_prefix_commitment(), original_prefix);
    assert_eq!(
        EFFECTS_ALLOCATION_ATTEMPTS.with(std::cell::Cell::get),
        effects_allocation_attempts,
        "refused admission must not allocate retained effects"
    );
    assert!(
        state.block_hashes.writer_available(),
        "private execution retains no hash writer"
    );
    let reserved = archive
        .try_reserve_candidate(
            crate::query::provider_ingest_finalized::ProviderIngestFinalizedArchiveKeyV1::try_new(
                context.network_id,
                proposal.header().height().get(),
                *proposal.hash().as_ref(),
                proposal.header().creation_time_ms,
            )
            .unwrap(),
            &state.kura,
        )
        .err()
        .expect("returned predecessor remains reserved");
    assert!(matches!(reserved,
        crate::query::provider_ingest_finalized::ProviderIngestFinalizedArchiveErrorV1::CaptureReserved { .. }));
    // Retry synchronously on the original borrowed owner; no second execution.
    let journals = carrier
        .prepare_journals(provider, reputation, |original| {
            inspect_original(&original);
            admit_journals_for_test(original)
        })
        .unwrap();
    assert_eq!(journals.execution_prefix_commitment(), original_prefix);
    assert_eq!(
        EFFECTS_ALLOCATION_ATTEMPTS.with(std::cell::Cell::get),
        effects_allocation_attempts + 1
    );
    assert_eq!(
        std::alloc::Layout::for_value(journals.effects.as_ref()),
        effects_layout,
        "admission exposes the actual effects pointee, not its Box handle"
    );
    assert_eq!(
        journals.valid.as_ref().execution_outputs().as_ptr(),
        outputs_pointer
    );
    assert!(Arc::ptr_eq(&journals.context, &context_owner));
    assert_eq!(journals.publication_events.as_ptr(), events_pointer);
    assert_eq!(journals.publication_events.capacity(), events_capacity);
    drop(journals);
    assert!(called);
    assert_eq!(
        std::fs::read_dir(directory_path.join("archive/records"))
            .unwrap()
            .count(),
        0
    );
    assert_eq!(
        crate::snapshot::canonical_state_snapshot_hash(&state).unwrap(),
        before
    );
    assert!(state.block_hashes.writer_available());
}

#[test]
fn admitted_runtime_owner_retains_guard_and_survives_static_worker_handoff() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    struct Reservation(Arc<AtomicUsize>);
    impl Drop for Reservation {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }
    fn assert_send<T: Send + 'static>() {}
    assert_send::<RuntimeJournals<Reservation>>();
    let (state, _, _, _) = super::super::tests::fixture();
    let released = Arc::new(AtomicUsize::new(0));
    let original = RuntimeJournals::capture(
        state.canonical_runtime.block(),
        state.commit_topology.block(),
        state.prev_commit_topology.block(),
        state.lane_consensus_contexts.block(),
        |inputs| {
            admit_runtime_for_test(inputs)?;
            Ok::<_, std::convert::Infallible>(Reservation(Arc::clone(&released)))
        },
    )
    .unwrap();
    assert!(original.matches_current(&state));
    assert!(Arc::ptr_eq(&original.admission().0, &released));
    drop(state.canonical_runtime.block());
    drop(state.commit_topology.block());
    drop(state.prev_commit_topology.block());
    drop(state.lane_consensus_contexts.block());
    drop(state);
    let returned = std::thread::spawn(move || {
        assert_eq!(original.canonical_runtime.mode(), mv::BlockMode::Ordinary);
        assert!(original.canonical_runtime.touched_value().is_none());
        original
    })
    .join()
    .unwrap();
    assert_eq!(released.load(Ordering::SeqCst), 0);
    drop(returned);
    assert_eq!(released.load(Ordering::SeqCst), 1);
}

#[test]
fn detached_runtime_detects_replacement_and_each_owner_change() {
    let (state, _, _, _) = super::super::tests::fixture();
    let capture = || {
        RuntimeJournals::capture(
            state.canonical_runtime.block_and_revert(),
            state.commit_topology.block_and_revert(),
            state.prev_commit_topology.block_and_revert(),
            state.lane_consensus_contexts.block_and_revert(),
            admit_runtime_for_test,
        )
        .unwrap()
    };
    for owner in 0..4 {
        let original = capture();
        assert!(original.matches_current(&state));
        assert_eq!(original.canonical_runtime.mode(), mv::BlockMode::Replace);
        // A no-op commit changes the exact current/undo pair even when its
        // observable value remains equal; every component must be checked.
        match owner {
            0 => state.canonical_runtime.block().commit(),
            1 => state.commit_topology.block().commit(),
            2 => state.prev_commit_topology.block().commit(),
            3 => state.lane_consensus_contexts.block().commit(),
            _ => unreachable!(),
        }
        assert!(!original.matches_current(&state));
    }
}

#[test]
fn prepared_journals_retain_the_original_cut_and_drop_without_publication() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    struct Reservation(Arc<AtomicUsize>);
    impl Drop for Reservation {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }
    let (state, proposal, topology, context) = super::super::tests::fixture();
    let before = crate::snapshot::canonical_state_snapshot_hash(&state).unwrap();
    let prepared = super::super::tests::prepare(&state, proposal.clone(), &topology, &context)
        .unwrap_or_else(|(_, error)| panic!("prepare candidate: {error}"));
    let prefix = prepared.execution_prefix_commitment();
    let checkpoint = crate::snapshot::canonical_staged_state_snapshot_hash(prepared.state());
    let runtime_delta = prepared
        .state
        .canonical_runtime
        .touched_value()
        .map(|values| (values.before.clone(), values.after.clone()));
    let original_events = prepared.state.world.external_event_buf.clone();
    let released = Arc::new(AtomicUsize::new(0));
    let mut admissions = 0;
    let journals = prepared
        .prepare_journals(None, None, |original| {
            admissions += 1;
            let original_state = original.state;
            admit_journals_for_test(original)?;
            assert_eq!(original_state.world.external_event_buf, original_events);
            assert_eq!(
                original_state.world.musubi_resolver_index_checkpoints.len(),
                1
            );
            Ok::<_, std::convert::Infallible>(Reservation(Arc::clone(&released)))
        })
        .unwrap();
    assert_eq!(admissions, 1);
    assert_eq!(released.load(Ordering::SeqCst), 0);
    assert!(Arc::ptr_eq(&journals.admission.0, &released));
    assert_eq!(journals.execution_prefix_commitment(), prefix);
    assert_eq!(journals.checkpoint, checkpoint);
    assert_eq!(journals.valid.as_ref().hash(), proposal.hash());
    assert_eq!(*journals.context, context);
    assert_eq!(
        journals
            .components
            .runtime
            .canonical_runtime
            .touched_value()
            .map(|values| (values.before.clone(), values.after.clone())),
        runtime_delta
    );
    assert!(journals.components.runtime.matches_current(&state));
    // All original World and runtime writers are free while their captured
    // values and archive plans remain owned by this candidate.
    drop(state.canonical_runtime.block());
    drop(state.commit_topology.block());
    drop(state.prev_commit_topology.block());
    drop(state.lane_consensus_contexts.block());
    assert_eq!(
        journals.components.block_hashes.last(),
        Some(&proposal.hash())
    );
    assert!(
        journals
            .components
            .block_hashes
            .matches_current(&state.block_hashes)
    );
    assert!(state.block_hashes.writer_available());
    assert_eq!(
        journals.components.block_hashes.mode(),
        mv::BlockMode::Ordinary
    );
    assert_eq!(
        journals.components.block_hashes.pending(),
        &[proposal.hash()]
    );
    assert_eq!(
        journals
            .components
            .world
            .field("musubi_resolver_index_checkpoints")
            .unwrap()
            .touched_values,
        1
    );
    assert_eq!(journals.components.world.field_count(), 282);
    assert_eq!(journals.components.world.mode(), mv::BlockMode::Ordinary);
    assert!(journals.components.world.matches_current(&state.world));
    assert_eq!(
        journals.components.world.external_events(),
        original_events.as_slice()
    );
    drop(state.world.block());
    assert_eq!(
        journals.components.transactions.staged_membership().0.get(),
        1
    );
    assert_eq!(
        journals
            .components
            .transactions
            .observe_predecessor(&state.transactions),
        storage_transactions::MembershipPredecessorStatus::Current
    );
    // Membership's writer is already released while the other journals live.
    drop(state.transactions.block());
    assert_eq!(journals.source_prefix.sources().proposal(), proposal.hash());
    journals
        .source_prefix
        .inventory()
        .verify_ordinary_witness_bundles(&journals.source_prefix.witness().fastpq_transcripts)
        .unwrap();
    assert!(!journals.publication_events.is_empty());
    assert_eq!(state.transactions.latest_height(), 0);
    assert_eq!(state.committed_height(), 0);
    assert_eq!(state.kura.blocks_count(), 0);
    drop(journals);
    assert_eq!(released.load(Ordering::SeqCst), 1);
    assert_eq!(
        crate::snapshot::canonical_state_snapshot_hash(&state).unwrap(),
        before
    );
    // Every original writer must be released by drop, without replacement scopes.
    let retry = super::super::tests::prepare(&state, proposal, &topology, &context)
        .unwrap_or_else(|(_, error)| panic!("prepare after dropped journals: {error}"));
    assert_eq!(retry.execution_prefix_commitment(), prefix);
    drop(retry);
}

#[cfg(feature = "telemetry")]
#[test]
fn prepared_journals_capture_dirty_telemetry_from_original_world() {
    let (state, proposal, topology, context) = super::super::tests::fixture();
    let mut prepared = super::super::tests::prepare(&state, proposal, &topology, &context)
        .unwrap_or_else(|(_, error)| panic!("prepare telemetry candidate: {error}"));
    let counts = ParliamentAttemptCountsV1 {
        status_counts: [1, 2, 3, 4, 5, 6],
        stage_counts: [13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1],
    };
    // Exercise the capture component with distinct dirty values. This fixture
    // does not publish or authorize the manually changed candidate journals.
    *prepared
        .parts_mut()
        .state
        .world
        .parliament_attempt_counts
        .get_mut() = counts;
    let citizen = iroha_test_samples::SAMPLE_GENESIS_ACCOUNT_ID.clone();
    prepared.parts_mut().state.world.citizens.insert(
        citizen.clone(),
        CitizenshipRecord::new(citizen, Quantity::from(10_u64), 1),
    );
    *prepared
        .parts_mut()
        .state
        .world
        .musubi_replication_shortfall_releases
        .get_mut() = 17;
    let journals = prepared
        .prepare_journals(None, None, |original| {
            assert!(original.state.world.parliament_attempt_counts.is_dirty());
            assert!(original.state.world.citizens.is_dirty());
            assert_eq!(
                *original.state.world.parliament_attempt_counts.get(),
                counts
            );
            assert_eq!(original.state.world.citizens.len(), 1);
            admit_journals_for_test(original)
        })
        .unwrap();
    assert_eq!(
        journals.effects.committed_parliament_attempt_counts,
        Some(counts)
    );
    assert_eq!(journals.effects.committed_citizens_total, Some(1));
    assert_eq!(
        journals
            .effects
            .committed_musubi_replication_shortfall_releases,
        17
    );
    assert!(state.world.citizens.view().is_empty());
    assert_eq!(
        *state.world.parliament_attempt_counts.view().get(),
        ParliamentAttemptCountsV1::default()
    );

    // A later live World cannot replace the exact values retained at capture.
    let mut later = state.world.block();
    *later.parliament_attempt_counts.get_mut() = ParliamentAttemptCountsV1::default();
    *later.musubi_replication_shortfall_releases.get_mut() = 99;
    later.commit();
    assert_eq!(
        journals.effects.committed_parliament_attempt_counts,
        Some(counts)
    );
    assert_eq!(journals.effects.committed_citizens_total, Some(1));
    assert_eq!(
        journals
            .effects
            .committed_musubi_replication_shortfall_releases,
        17
    );
    drop(journals);
    assert!(state.world.citizens.view().is_empty());
    assert_eq!(
        *state
            .world
            .musubi_replication_shortfall_releases
            .view()
            .get(),
        99
    );
}

#[cfg(feature = "telemetry")]
#[test]
fn prepared_journals_preserve_clean_parliament_gauge_suppression() {
    let (state, proposal, topology, context) = super::super::tests::fixture();
    let prepared = super::super::tests::prepare(&state, proposal, &topology, &context)
        .unwrap_or_else(|(_, error)| panic!("prepare clean telemetry candidate: {error}"));
    assert!(!prepared.state.world.parliament_attempt_counts.is_dirty());
    assert!(!prepared.state.world.citizens.is_dirty());
    let shortfall = *prepared
        .state
        .world
        .musubi_replication_shortfall_releases
        .get();
    let journals = prepared
        .prepare_journals(None, None, admit_journals_for_test)
        .unwrap();
    assert_eq!(journals.effects.committed_parliament_attempt_counts, None);
    assert_eq!(journals.effects.committed_citizens_total, None);
    assert_eq!(
        journals
            .effects
            .committed_musubi_replication_shortfall_releases,
        shortfall
    );
}

#[test]
fn complete_carrier_journals_move_to_a_worker_after_the_original_state_is_dropped() {
    fn assert_static_send<T: Send + 'static>() {}
    assert_static_send::<PreparedCarrierJournals<()>>();
    let (state, proposal, topology, context) = super::super::tests::fixture();
    let state: Arc<State> = Arc::from(state);
    let original = Arc::downgrade(&state);
    let kura = Arc::clone(&state.kura);
    let prepared = super::super::tests::prepare(&state, proposal.clone(), &topology, &context)
        .unwrap_or_else(|(_, error)| panic!("prepare candidate: {error}"));
    let prefix = prepared.execution_prefix_commitment();
    let journals = prepared
        .prepare_journals(None, None, admit_journals_for_test)
        .unwrap();
    drop(state);
    assert!(
        original.upgrade().is_none(),
        "the candidate owns no hidden State clone"
    );
    let returned = std::thread::spawn(move || {
        assert_eq!(journals.execution_prefix_commitment(), prefix);
        assert_eq!(journals.valid.as_ref().hash(), proposal.hash());
        assert_eq!(journals.components.world.field_count(), 282);
        assert!(!journals.publication_events.is_empty());
        journals
    })
    .join()
    .unwrap();
    assert!(Arc::ptr_eq(&returned.kura, &kura));
    assert_eq!(kura.blocks_count(), 0);
    drop(returned);
    assert_eq!(kura.blocks_count(), 0);
}

#[test]
fn archive_capacity_failure_retains_static_original_journals_without_artifact_writes() {
    use std::sync::atomic::{AtomicBool, Ordering};
    struct Reservation(Arc<AtomicBool>);
    impl Drop for Reservation {
        fn drop(&mut self) {
            self.0.store(true, Ordering::SeqCst);
        }
    }
    fn assert_static_send<T: Send + 'static>() {}
    assert_static_send::<StagedCarrierCapture<Reservation>>();
    let (state, proposal, topology, context) = super::super::tests::fixture();
    let state: Arc<State> = Arc::from(state);
    let original_state = Arc::downgrade(&state);
    let before = crate::snapshot::canonical_state_snapshot_hash(&state).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let directory_path = directory.path().canonicalize().unwrap();
    let bounds = ProviderIngestFinalizedArchiveBoundsV1::try_new(1, 1, 1, 1, 1, 1, 1).unwrap();
    let archive = Arc::new(
        ProviderIngestFinalizedArchiveV1::try_open(directory_path.join("archive"), bounds).unwrap(),
    );
    let predecessor = reserve_provider_for_test(&archive, &state, &proposal, &context);
    let prepared = super::super::tests::prepare(&state, proposal, &topology, &context)
        .unwrap_or_else(|(_, error)| panic!("prepare candidate: {error}"));
    let prefix = prepared.execution_prefix_commitment();
    let released = Arc::new(AtomicBool::new(false));
    let (carrier, error) = {
        let failure = prepared
            .prepare_journals(Some(predecessor), None, |_| {
                Ok::<_, std::convert::Infallible>(Reservation(Arc::clone(&released)))
            })
            .err()
            .expect("configured archive bound refuses");
        match failure {
            CarrierJournalPreparationError::ArchivePreparation { carrier, error } => {
                (carrier, error)
            }
            _ => panic!("archive refusal must retain static execution"),
        }
    };
    assert!(
        matches!(&error, CarrierArchivePreparationError::Provider(error)
        if matches!(error.as_ref(), crate::query::provider_ingest_finalized::ProviderIngestFinalizedArchiveErrorV1::RecordTooLarge { observed, maximum: 1 } if *observed > 1)),
        "{error}"
    );
    assert!(!released.load(Ordering::SeqCst));
    assert!(state.block_hashes.writer_available());
    drop(state.world.block());
    drop(state.transactions.block());
    assert_eq!(
        crate::snapshot::canonical_state_snapshot_hash(&state).unwrap(),
        before
    );
    assert_eq!(
        std::fs::read_dir(directory_path.join("archive/records"))
            .unwrap()
            .count(),
        0
    );
    let original_box = std::ptr::from_ref(carrier.as_ref());
    drop(state);
    assert!(original_state.upgrade().is_none());
    let carrier = std::thread::spawn(move || {
        assert_eq!(carrier.journals.execution_prefix_commitment(), prefix);
        let (carrier, _) = carrier
            .try_complete()
            .err()
            .expect("unchanged limit still refuses");
        assert_eq!(carrier.journals.execution_prefix_commitment(), prefix);
        carrier
    })
    .join()
    .unwrap();
    assert_eq!(std::ptr::from_ref(carrier.as_ref()), original_box);
    assert!(!released.load(Ordering::SeqCst));
    drop(carrier);
    assert!(released.load(Ordering::SeqCst));
}

/// Signed genesis with the governed policies required by both archive captures.
pub(in crate::state::carrier_preparation::journals) fn archive_fixture() -> (
    Box<State>,
    iroha_data_model::block::SignedBlock,
    crate::sumeragi::network_topology::Topology,
    iroha_data_model::block::consensus_v2::HeightContext,
) {
    super::super::tests::fixture_with_instructions(&archive_fixture_instructions())
}

/// Exact governed feed instructions shared by real archive capture fixtures.
pub(in crate::state) fn archive_fixture_instructions() -> Vec<iroha_data_model::isi::InstructionBox>
{
    use iroha_data_model::{
        isi::{
            Grant, Register,
            sorafs::{
                SetSorafsOrderbookPolicy, SetSorafsReputationJournalAuthorityPolicy,
                SetSorafsReservePolicy,
            },
        },
        permission::Permission,
        sorafs::{
            orderbook::{ORDERBOOK_ADMISSION_POLICY_VERSION_V1, OrderbookAdmissionPolicyV1},
            reputation::{
                REPUTATION_JOURNAL_AUTHORITY_POLICY_VERSION_V1,
                REPUTATION_JOURNAL_MAX_SOURCE_AGE_MS_V1, ReputationJournalAuthorityPolicyV1,
            },
            reserve::{
                RESERVE_AUTHORITY_POLICY_VERSION_V1, ReserveAuthorityPolicyV1, ReservePolicyV1,
            },
        },
    };
    let authority = iroha_test_samples::SAMPLE_GENESIS_ACCOUNT_ID.clone();
    let policy = ReputationJournalAuthorityPolicyV1 {
        version: REPUTATION_JOURNAL_AUTHORITY_POLICY_VERSION_V1,
        revision: 1,
        predecessor_policy_digest: None,
        por_recorder_authority: authority.clone(),
        dispute_recorder_authority: authority.clone(),
        token_recorder_authority: authority.clone(),
        max_source_age_ms: REPUTATION_JOURNAL_MAX_SOURCE_AGE_MS_V1,
    };
    let orderbook_policy = OrderbookAdmissionPolicyV1 {
        version: ORDERBOOK_ADMISSION_POLICY_VERSION_V1,
        revision: 1,
        predecessor_policy_digest: None,
        market_id: [0xA5; 32],
        matcher_authority: authority.clone(),
        settlement_authority: authority.clone(),
        paused: false,
        min_order_gib: 1,
        max_order_gib: 1024,
        price_tick_micro_xor: 10,
        max_maker_fee_bps: 100,
        max_taker_fee_bps: 200,
        max_order_lifetime_secs: 3600,
        max_receipt_age_secs: 300,
        max_clock_skew_secs: 5,
        max_receipt_bytes: 1024,
        max_receipts_per_channel: 2,
    };
    let custody = iroha_test_samples::ALICE_ID.clone();
    let asset_id = iroha_data_model::asset::AssetDefinitionId::derive_from_components(
        iroha_genesis::GENESIS_DOMAIN_ID.clone(),
        "reserve".parse().unwrap(),
    );
    let reserve_policy = ReserveAuthorityPolicyV1 {
        version: RESERVE_AUTHORITY_POLICY_VERSION_V1,
        revision: 1,
        predecessor_policy_digest: None,
        economics: ReservePolicyV1::default(),
        asset_definition: asset_id.clone(),
        custody_account: custody.clone(),
        treasury_account: authority.clone(),
        operations_authority: authority.clone(),
        decision_authority: authority.clone(),
        grace_period_days: 7,
        default_after_days: 30,
        max_provider_debt: sorafs_manifest::deal::XorQuantity::try_from_micro(1_000_000_000)
            .unwrap(),
        max_pending_movements_per_provider: 4,
        max_open_appeals_per_provider: 2,
    };
    // Preserve all permissions and policy histories through real instructions.
    vec![
        Grant::account_permission(
            Permission::new(
                "CanManageSorafsReputationJournalPolicy".to_owned(),
                iroha_primitives::json::Json::new(()),
            ),
            authority.clone(),
        )
        .into(),
        SetSorafsReputationJournalAuthorityPolicy::new(policy).into(),
        Grant::account_permission(
            Permission::new(
                "CanSetSorafsPricing".to_owned(),
                iroha_primitives::json::Json::new(()),
            ),
            authority.clone(),
        )
        .into(),
        SetSorafsOrderbookPolicy::new(orderbook_policy).into(),
        Register::account(iroha_data_model::account::Account::new(custody)).into(),
        Register::asset_definition(iroha_data_model::asset::AssetDefinition::numeric(
            asset_id,
            "Reserve".to_owned(),
            iroha_data_model::asset::AssetBalancePolicy::Global,
            None,
        ))
        .into(),
        Grant::account_permission(
            Permission::new(
                "CanSetSorafsReservePolicy".to_owned(),
                iroha_primitives::json::Json::new(()),
            ),
            authority,
        )
        .into(),
        SetSorafsReservePolicy::new(reserve_policy).into(),
    ]
}

#[test]
fn prepared_archive_projections_survive_state_journal_decomposition() {
    use crate::query::reputation_finalized::ReputationFinalizedArchiveBounds;

    let (state, proposal, topology, context) = archive_fixture();
    let before = crate::snapshot::canonical_state_snapshot_hash(&state).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let directory_path = directory.path().canonicalize().unwrap();
    let provider = Arc::new(
        ProviderIngestFinalizedArchiveV1::try_open(
            directory_path.join("provider"),
            ProviderIngestFinalizedArchiveBoundsV1::try_new(1 << 20, 16, 16 << 20, 16, 16, 256, 16)
                .unwrap(),
        )
        .unwrap(),
    );
    let reputation = Arc::new(
        ReputationFinalizedArchive::try_open(
            directory_path.join("reputation"),
            ReputationFinalizedArchiveBounds::try_new(1 << 20, 16, 16 << 20).unwrap(),
        )
        .unwrap(),
    );
    for _ in 0..2 {
        let provider_owner = reserve_provider_for_test(&provider, &state, &proposal, &context);
        let reputation_owner =
            reserve_reputation_for_test(&reputation, &state, &proposal, &context);
        let prepared = super::super::tests::prepare(&state, proposal.clone(), &topology, &context)
            .unwrap_or_else(|(_, error)| panic!("prepare candidate: {error}"));
        let journals = prepared
            .prepare_journals(Some(provider_owner), Some(reputation_owner), |original| {
                assert!(original.provider.is_some());
                assert!(original.reputation.is_some());
                admit_journals_for_test(original)
            })
            .unwrap();
        assert!(journals.provider_capture.is_some());
        assert!(journals.reputation_capture.is_some());
        assert!(provider.is_empty().unwrap());
        assert!(reputation.is_empty().unwrap());
        assert_eq!(state.committed_height(), 0);
        assert_eq!(state.kura.blocks_count(), 0);
        for relative in [
            "provider/records",
            "reputation/anchors",
            "reputation/policies",
        ] {
            assert_eq!(
                std::fs::read_dir(directory_path.join(relative))
                    .unwrap()
                    .count(),
                0
            );
        }
        // Dropping also releases both reservations, allowing the exact candidate
        // to be prepared again without inventing a receipt or replaying effects.
        drop(journals);
        assert_eq!(
            crate::snapshot::canonical_state_snapshot_hash(&state).unwrap(),
            before
        );
    }
}

#[test]
fn archive_index_refusal_retains_same_static_execution_and_completed_provider_plan() {
    use crate::query::reputation_finalized::{
        ReputationFinalizedArchiveBounds, ReputationFinalizedArchiveError,
    };
    use std::{
        future::Future,
        task::{Context, Poll, Waker},
    };

    let (state, proposal, topology, context) = archive_fixture();
    let state: Arc<State> = Arc::from(state);
    let original_state = Arc::downgrade(&state);
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().canonicalize().unwrap();
    let provider = Arc::new(
        ProviderIngestFinalizedArchiveV1::try_open(
            root.join("provider"),
            ProviderIngestFinalizedArchiveBoundsV1::try_new(1 << 20, 16, 16 << 20, 16, 16, 256, 16)
                .unwrap(),
        )
        .unwrap(),
    );
    let reputation = Arc::new(
        ReputationFinalizedArchive::try_open(
            root.join("reputation"),
            ReputationFinalizedArchiveBounds::try_new(1 << 20, 16, 16 << 20).unwrap(),
        )
        .unwrap(),
    );
    let provider_owner = reserve_provider_for_test(&provider, &state, &proposal, &context);
    let reputation_owner = reserve_reputation_for_test(&reputation, &state, &proposal, &context);
    let prepared = super::super::tests::prepare(&state, proposal, &topology, &context)
        .unwrap_or_else(|(_, error)| panic!("prepare archive candidate: {error}"));
    let prefix = prepared.execution_prefix_commitment();
    let original_inventory = Arc::clone(prepared.source_prefix.inventory());
    // Original capture must complete without reading this held index. Only the
    // later detached insertion preparation may observe its actual release wait.
    let (carrier, error) = {
        let failure = reputation
            .with_index_reader_for_test(|| {
                prepared.prepare_journals(
                    Some(provider_owner),
                    Some(reputation_owner),
                    admit_journals_for_test,
                )
            })
            .err()
            .expect("held reputation reader refuses publication preparation");
        match failure {
            CarrierJournalPreparationError::ArchivePreparation { carrier, error } => {
                (carrier, error)
            }
            _ => panic!("index refusal must retain detached execution"),
        }
    };
    let CarrierArchivePreparationError::Reputation(error) = error else {
        panic!("provider must have completed before reputation contention");
    };
    let ReputationFinalizedArchiveError::IndexBusy { wait } = error.as_ref() else {
        panic!("actual index reader must supply its release observation: {error}");
    };
    let mut released = Box::pin(wait.clone().wait_for_release());
    assert!(matches!(
        released
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop())),
        Poll::Ready(())
    ));
    let original_box = std::ptr::from_ref(carrier.as_ref());
    let provider_bytes = carrier
        .provider
        .as_ref()
        .unwrap()
        .prepared_bytes_identity_for_test()
        .expect("original provider insertion is prepared");
    assert_eq!(carrier.journals.execution_prefix_commitment(), prefix);
    assert!(Arc::ptr_eq(
        carrier.journals.source_prefix.inventory(),
        &original_inventory
    ));
    assert!(state.block_hashes.writer_available());
    drop(state.world.block());
    drop(state.transactions.block());
    drop(state.canonical_runtime.block());
    drop(state);
    assert!(
        original_state.upgrade().is_none(),
        "staged owner retains no hidden State"
    );
    let (carrier, _) = reputation
        .with_index_reader_for_test(|| carrier.try_complete())
        .err()
        .expect("another real reader still refuses the same owner");
    assert_eq!(std::ptr::from_ref(carrier.as_ref()), original_box);
    assert_eq!(
        carrier
            .provider
            .as_ref()
            .unwrap()
            .prepared_bytes_identity_for_test(),
        Some(provider_bytes)
    );
    let journals = std::thread::spawn(move || {
        carrier
            .try_complete()
            .unwrap_or_else(|(_, error)| panic!("resume exact detached execution: {error}"))
    })
    .join()
    .unwrap();
    assert_eq!(journals.execution_prefix_commitment(), prefix);
    assert!(Arc::ptr_eq(
        journals.source_prefix.inventory(),
        &original_inventory
    ));
    assert!(journals.provider_capture.is_some());
    assert!(journals.reputation_capture.is_some());
    assert!(provider.is_empty().unwrap());
    assert!(reputation.is_empty().unwrap());
    for relative in [
        "provider/records",
        "reputation/anchors",
        "reputation/policies",
    ] {
        assert_eq!(std::fs::read_dir(root.join(relative)).unwrap().count(), 0);
    }
}

#[test]
fn archive_original_capture_identity_refusal_retains_static_recovery_owner() {
    use crate::query::reputation_finalized::{
        ReputationFinalizedArchiveBounds, ReputationFinalizedArchiveError,
        ReputationFinalizedArchiveKeyV1,
    };
    let (state, proposal, topology, context) = archive_fixture();
    let directory = tempfile::tempdir().unwrap();
    let archive = Arc::new(
        ReputationFinalizedArchive::try_open(
            directory.path().canonicalize().unwrap().join("reputation"),
            ReputationFinalizedArchiveBounds::try_new(1 << 20, 16, 16 << 20).unwrap(),
        )
        .unwrap(),
    );
    for wrong_hash in [false, true] {
        let mut hash = *proposal.hash().as_ref();
        let mut time = proposal.header().creation_time_ms;
        if wrong_hash {
            hash[0] ^= 1;
        } else {
            time += 1;
        }
        let owner = archive
            .try_reserve_candidate(
                ReputationFinalizedArchiveKeyV1::try_new(
                    context.network_id,
                    proposal.header().height().get(),
                    hash,
                )
                .unwrap(),
                time,
                &state.kura,
            )
            .unwrap();
        let prepared = super::super::tests::prepare(&state, proposal.clone(), &topology, &context)
            .unwrap_or_else(|(_, error)| panic!("prepare original candidate: {error}"));
        let prefix = prepared.execution_prefix_commitment();
        let failure = prepared
            .prepare_journals(None, Some(owner), admit_journals_for_test)
            .err()
            .expect("reserved identity must match original State exactly");
        let CarrierJournalPreparationError::ArchivePreparation { carrier, error } = failure else {
            panic!("capture failure keeps original detached execution");
        };
        let CarrierArchivePreparationError::Reputation(original_error) = error else {
            panic!("exact reputation source mismatch");
        };
        assert!(matches!(
            original_error.as_ref(),
            ReputationFinalizedArchiveError::FinalityAuthentication {
                reason: "candidate reputation State differs from its reserved exact identity"
            }
        ));
        assert!(state.block_hashes.writer_available());
        drop(state.world.block());
        let original_box = std::ptr::from_ref(carrier.as_ref());
        let (carrier, repeated) = carrier
            .try_complete()
            .err()
            .expect("failed capture is recovery-required");
        assert_eq!(std::ptr::from_ref(carrier.as_ref()), original_box);
        let CarrierArchivePreparationError::Reputation(repeated) = repeated else {
            panic!("same exact refusal");
        };
        assert!(
            Arc::ptr_eq(&original_error, &repeated),
            "retry cannot reread another State"
        );
        assert_eq!(carrier.journals.execution_prefix_commitment(), prefix);
        assert!(archive.is_empty().unwrap());
        drop(carrier);
    }
}

#[test]
fn journal_resource_refusal_precedes_geometry_projection() {
    #[derive(Debug, PartialEq, Eq)]
    enum Capacity {
        Exhausted,
    }
    let (state, proposal, topology, context) = super::super::tests::fixture();
    let before = crate::snapshot::canonical_state_snapshot_hash(&state).unwrap();
    let mut prepared = super::super::tests::prepare(&state, proposal, &topology, &context)
        .unwrap_or_else(|(_, error)| panic!("actual candidate: {error}"));
    // Deliberate test-only projection drift would fail geometry capture. Whole
    // capture admission must nevertheless precede its allocating projections.
    prepared.parts_mut().state.nexus.autoscale.enabled = !prepared.state.nexus.autoscale.enabled;
    assert!(prepared.state.prepare_carrier_geometry().is_err());
    let mut called = false;
    let error = prepared
        .prepare_journals(None, None, |_| {
            called = true;
            Err::<(), _>(Capacity::Exhausted)
        })
        .err()
        .expect("local capture refusal");
    assert!(called);
    assert!(matches!(
        error,
        CarrierJournalPreparationError::JournalAdmission {
            error: Capacity::Exhausted,
            ..
        }
    ));
    drop(error);
    assert_eq!(
        crate::snapshot::canonical_state_snapshot_hash(&state).unwrap(),
        before
    );
    assert!(state.block_hashes.writer_available());
    assert_eq!(state.kura.blocks_count(), 0);
}

#[test]
fn geometry_refusal_drops_originals_before_capture_reservation() {
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    struct Reservation<'a> {
        state: &'a State,
        released: Arc<AtomicUsize>,
        originals_released_first: Arc<AtomicBool>,
    }
    impl Drop for Reservation<'_> {
        fn drop(&mut self) {
            self.originals_released_first
                .store(self.state.block_hashes.writer_available(), Ordering::SeqCst);
            self.released.fetch_add(1, Ordering::SeqCst);
        }
    }
    let (state, proposal, topology, context) = super::super::tests::fixture();
    let before = crate::snapshot::canonical_state_snapshot_hash(&state).unwrap();
    let mut prepared = super::super::tests::prepare(&state, proposal, &topology, &context)
        .unwrap_or_else(|(_, error)| panic!("actual candidate: {error}"));
    prepared.parts_mut().state.nexus.autoscale.enabled = !prepared.state.nexus.autoscale.enabled;
    assert!(prepared.state.prepare_carrier_geometry().is_err());
    let released = Arc::new(AtomicUsize::new(0));
    let originals_released_first = Arc::new(AtomicBool::new(false));
    let error = prepared
        .prepare_journals(None, None, |_| {
            Ok::<_, std::convert::Infallible>(Reservation {
                state: &state,
                released: Arc::clone(&released),
                originals_released_first: Arc::clone(&originals_released_first),
            })
        })
        .err()
        .expect("geometry drift refuses capture");
    assert!(matches!(error, CarrierJournalPreparationError::Geometry(_)));
    assert_eq!(released.load(Ordering::SeqCst), 1);
    assert!(originals_released_first.load(Ordering::SeqCst));
    assert_eq!(
        crate::snapshot::canonical_state_snapshot_hash(&state).unwrap(),
        before
    );
    assert_eq!(state.kura.blocks_count(), 0);
}

#[test]
fn carrier_journal_shell_plan_precedes_execution_and_survives_capture() {
    // This reservation is deliberately scoped to actual World shells. The
    // fixture supplies the separate execution/runtime/archive payload owners.
    let bytes = PreparedCarrier::world_journal_shell_bytes().unwrap();
    let budget = mv::allocation::AllocationBudget::new(bytes);
    let reservation = budget.try_reserve_bytes(bytes).unwrap();
    let (state, proposal, topology, context) = super::super::tests::fixture();
    let before = crate::snapshot::canonical_state_snapshot_hash(&state).unwrap();
    let prepared = super::super::tests::prepare(&state, proposal, &topology, &context)
        .unwrap_or_else(|(_, error)| panic!("prepare candidate: {error}"));
    let journals = prepared
        .prepare_journals(None, None, |inputs| {
            assert_eq!(inputs.world_journal_shell_bytes().unwrap(), bytes);
            assert_eq!(budget.reserved_bytes(), bytes);
            Ok::<_, std::convert::Infallible>(reservation)
        })
        .unwrap_or_else(|error| panic!("capture candidate: {error:?}"));
    assert_eq!(budget.reserved_bytes(), bytes);
    assert!(matches!(
        budget.try_reserve_bytes(1),
        Err(mv::allocation::AllocationRefusal::Capacity { .. })
    ));
    drop(journals);
    assert_eq!(budget.reserved_bytes(), 0);
    assert_eq!(
        crate::snapshot::canonical_state_snapshot_hash(&state).unwrap(),
        before
    );
}

#[path = "state_capture_tests.rs"]
mod state_capture_tests;
