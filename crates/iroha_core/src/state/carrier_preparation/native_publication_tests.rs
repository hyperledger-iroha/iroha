//! Actual single and atomic Native journals reach the sole terminal consumer.

use super::*;
use crate::state::{
    StateReadOnlyWithTransactions, WorldReadOnly,
    lane_decision_batch::NativeExecutionCustody,
    storage_transactions::TransactionsReadOnly,
    tests::{NativePublicationFixture, native_publication_fixture},
};
use iroha_data_model::{
    block::{BlockHeader, SignedBlock, consensus_v2::PayloadEncoding},
    events::pipeline::{BlockStatus, PipelineEventBox},
};
use iroha_primitives::numeric::Quantity;
use mv::storage::StorageReadOnly;
use std::{
    convert::Infallible,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

struct Reservation {
    released: Arc<AtomicUsize>,
    _allocation: mv::allocation::AllocationReservation,
}

impl Drop for Reservation {
    fn drop(&mut self) {
        self.released.fetch_add(1, Ordering::SeqCst);
    }
}

struct OriginalNativeCustody {
    groups: *const (),
    body: *const u8,
    decisions: *const (),
    contexts: *const (),
    global_context: *const (),
}

impl OriginalNativeCustody {
    fn capture(custody: &NativeExecutionCustody) -> Self {
        let groups = custody.sources_for_test();
        assert_eq!(groups.len(), 1);
        Self {
            groups: groups.as_ptr().cast(),
            body: groups[0].body().canonical_bytes().as_ptr(),
            decisions: groups[0].decisions().as_ptr().cast(),
            contexts: groups[0].contexts().as_ptr().cast(),
            global_context: std::ptr::from_ref(custody.context().context()).cast(),
        }
    }

    fn assert_retained(&self, custody: &NativeExecutionCustody) {
        let current = Self::capture(custody);
        assert_eq!(current.groups, self.groups);
        assert_eq!(current.body, self.body);
        assert_eq!(current.decisions, self.decisions);
        assert_eq!(current.contexts, self.contexts);
        assert_eq!(current.global_context, self.global_context);
    }
}

// End the sizeable original State/World constructor frame before native
// preparation, and end the publication test frame before writer probing.
#[inline(never)]
fn assert_original_writers_released(state: &State, header: BlockHeader) {
    assert!(state.state_commit_lock.try_lock_or_wait().is_ok());
    assert!(state.state_write_lock.try_lock_or_wait().is_ok());
    assert!(state.lane_lifecycle_lock.try_lock_or_wait().is_ok());
    drop(state.kura.try_publication_lease().unwrap());
    drop(state.block(header));
}

#[test]
fn native_single_publishes_original_sources_and_exact_checkpoint_once() {
    assert_native_publication_on_bounded_stack(false);
}

#[test]
fn native_atomic_publishes_original_sources_and_exact_checkpoint_once() {
    assert_native_publication_on_bounded_stack(true);
}

fn assert_native_publication_on_bounded_stack(atomic: bool) {
    // Pin the ordinary Rust worker budget so RUST_MIN_STACK cannot hide large
    // carrier moves during durable-source decoding, abort, or publication.
    std::thread::Builder::new()
        .name(format!("native-publication-atomic-{atomic}"))
        .stack_size(2 * 1024 * 1024)
        .spawn(move || {
            let fixture = native_publication_fixture(atomic);
            assert_native_publication(atomic, fixture);
        })
        .unwrap()
        .join()
        .unwrap();
}

#[inline(never)]
fn assert_native_publication(atomic: bool, fixture: Box<NativePublicationFixture>) {
    let state = fixture.state();
    let before = crate::snapshot::canonical_state_snapshot_hash(state).unwrap();
    let before_height = state.committed_height();
    let header = fixture.carrier().header();
    let height = header.height().get();
    assert_eq!(usize::try_from(height).unwrap(), before_height + 1);
    let capture_released = Arc::new(AtomicUsize::new(0));
    let capture_budget = mv::allocation::AllocationBudget::new(64 << 20);
    let mut unready = fixture.local_unready_owner();
    assert!(unready.native_records().is_empty());
    assert!(unready.held_effects().next().is_none());
    let mut foreign_owner = fixture.foreign_apply_owner();
    let foreign_decision = foreign_owner.native_decision().unwrap().unwrap();
    let foreign_effects = foreign_owner.held_effects().cloned().collect::<Vec<_>>();
    let foreign_records = foreign_owner.native_records().to_vec();
    let mut local = fixture.local_apply_owners();
    let original_local = local
        .iter()
        .map(|(_, owner)| {
            (
                owner.native_decision().unwrap().unwrap(),
                owner.held_effects().cloned().collect::<Vec<_>>(),
                owner.native_records().to_vec(),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        foreign_decision, original_local[0].0,
        "the genuine foreign owner differs only in original State family, not Decision authority"
    );
    let prepared = fixture.prepare();
    let custody = prepared.native_source_for_test().unwrap();
    assert_eq!(
        custody.sources_for_test()[0].contexts().len(),
        if atomic { 2 } else { 1 }
    );
    assert_eq!(
        custody.sources_for_test()[0].decisions().len(),
        if atomic { 2 } else { 1 }
    );
    assert!(custody.retains_carrier(prepared.block(), fixture.context()));
    let mut foreign_context = fixture.context().clone();
    foreign_context.height += 1;
    assert!(!custody.retains_carrier(prepared.block(), &foreign_context));
    let mut foreign_block = prepared.block().clone();
    let mut foreign_header = foreign_block.header();
    foreign_header.creation_time_ms += 1;
    foreign_block.replace_header_for_testing(foreign_header);
    assert!(!custody.retains_carrier(&foreign_block, fixture.context()));
    let original = OriginalNativeCustody::capture(custody);
    assert!(
        prepared.native_amx_manifest().entries().is_empty(),
        "Native source custody cannot fabricate legacy participant authorization"
    );
    let execution = prepared.execution_prefix_commitment();
    let finality = fixture.finality(prepared.block(), execution);
    let journals = prepared
        .prepare_journals(None, None, |inputs| {
            let bytes = inputs
                .world_journal_shell_bytes()?
                .checked_add(inputs.retained_effects_layout.size())
                .ok_or(mv::allocation::AllocationRefusal::DemandOverflow)?;
            Ok::<_, mv::allocation::AllocationRefusal>(Reservation {
                released: Arc::clone(&capture_released),
                _allocation: capture_budget.try_reserve_bytes(bytes)?,
            })
        })
        .unwrap();
    let retained_shell_bytes = capture_budget.reserved_bytes();
    assert!(retained_shell_bytes > 0);
    let original_effects = std::ptr::from_ref(journals.effects.as_ref());
    original.assert_retained(journals.native_source_for_test().unwrap());
    assert!(
        journals
            .source_prefix
            .retains_carrier(journals.valid.as_ref(), fixture.context())
    );
    assert!(
        journals.geometry.is_identity_transition(header),
        "this real fixture has no geometry transition to fabricate permission for"
    );
    assert!(!journals.geometry.has_pending_lifecycle());
    let checkpoint = journals.checkpoint;
    let decision = journals
        .bind_decision(finality)
        .unwrap_or_else(|refusal| panic!("real Native decision binding: {:?}", refusal.error));
    assert_eq!(
        std::ptr::from_ref(decision.journals.effects.as_ref()),
        original_effects
    );
    let finality = decision.finality().clone();
    let wire = decision.block().encode_wire().unwrap();
    assert_eq!(finality.commit_qc.execution_commitment, execution);
    state.kura.store_block(decision.block().clone()).unwrap();
    let receipt = state.kura.store_v2_finality_artifact(&finality).unwrap();
    let checkpoint_receipt = state
        .kura
        .persist_wsv_checkpoint_for_v2_commit(&receipt, checkpoint)
        .unwrap();
    let decision = decision.attach_checkpoint(checkpoint_receipt);
    assert_eq!(
        crate::snapshot::canonical_state_snapshot_hash(state).unwrap(),
        before
    );
    assert_eq!(state.committed_height(), before_height);
    assert_original_writers_released(state, header);

    let acquire = |decision: DecisionBoundCarrierJournals<
        Reservation,
        DetachedCarrierComponents,
        KuraWsvCheckpointReceipt,
    >| {
        assert_eq!(
            std::ptr::from_ref(decision.journals.effects.as_ref()),
            original_effects
        );
        decision
            .try_prepare_physical(state, None)
            .unwrap_or_else(|(_, error)| panic!("actual Native physical acquisition: {error:?}"))
    };
    // A real abort releases every acquired writer, preserving exact source and
    // checkpoint custody for the sole later successful consuming publication.
    let decision = acquire(decision).abort();
    assert_eq!(
        std::ptr::from_ref(decision.journals.effects.as_ref()),
        original_effects
    );
    assert_eq!(capture_released.load(Ordering::SeqCst), 0);
    assert_eq!(capture_budget.reserved_bytes(), retained_shell_bytes);
    assert_eq!(decision.block().encode_wire().unwrap(), wire);
    original.assert_retained(decision.journals.source_prefix.native_for_test().unwrap());
    assert!(
        decision
            .journals
            .source_prefix
            .retains_carrier(decision.block(), fixture.context())
    );
    assert_original_writers_released(state, header);
    assert_eq!(
        crate::snapshot::canonical_state_snapshot_hash(state).unwrap(),
        before
    );
    // Candidate execution, durable global QC/checkpoint and physical abort have
    // acknowledged none of the original local reducer Apply obligations.
    for ((_, owner), (decision, effects, records)) in local.iter().zip(&original_local) {
        assert_eq!(owner.native_decision().unwrap().as_ref(), Some(decision));
        assert_eq!(owner.held_effects().cloned().collect::<Vec<_>>(), *effects);
        assert_eq!(owner.native_records(), records);
    }
    let generation = state.state_view_generation();
    let published = acquire(decision)
        .publish()
        .unwrap_or_else(|(_, error)| panic!("complete genuine Native publication: {error:?}"));
    assert_eq!(state.state_view_generation(), generation + 2);
    // The production completion boundary moves the whole published owner. Only
    // after the worker handoff may the original driver borrow Apply authority.
    let source_allocation = std::ptr::from_ref(published.source.native_for_test().unwrap()).addr();
    let events_allocation = published.events().as_ptr().addr();
    let published = handoff_published_carrier(published);
    assert_eq!(
        std::ptr::from_ref(published.source.native_for_test().unwrap()).addr(),
        source_allocation
    );
    assert_eq!(published.events().as_ptr().addr(), events_allocation);
    assert!(published.matches_state(state));
    assert_eq!(published.artifact(), &finality);
    assert_eq!(published.receipt().artifact_hash(), receipt.artifact_hash());
    assert_eq!(published.receipt().context_id(), receipt.context_id());
    assert_eq!(published.receipt().certificate(), receipt.certificate());
    assert_eq!(published.receipt().subject(), receipt.subject());
    assert_eq!(published.receipt().block_hash(), receipt.block_hash());
    assert_eq!(published.receipt().height(), receipt.height());
    assert_eq!(published.committed_event().header, header);
    assert_eq!(published.committed_event().status, BlockStatus::Committed);
    assert_eq!(capture_released.load(Ordering::SeqCst), 0);
    assert_eq!(capture_budget.reserved_bytes(), retained_shell_bytes);
    assert_eq!(published.block().encode_wire().unwrap(), wire);
    assert_eq!(published.block().header(), header);
    assert_eq!(published.committed_event.header, header);
    assert_eq!(published.committed_event.status, BlockStatus::Committed);
    assert_eq!(
        published
            .events()
            .iter()
            .filter(|event| matches!(event,
        EventBox::Pipeline(PipelineEventBox::Block(event))
            if event.header == header && event.status == BlockStatus::Applied))
            .count(),
        1
    );
    original.assert_retained(published.source.native_for_test().unwrap());
    let application = published
        .native_apply()
        .expect("actual Native publication proof");
    assert!(matches!(
        unready.settle_published_apply(&application).unwrap(),
        crate::sumeragi::v2_lane_instance::LaneApplySettlement::NotReady
    ));
    assert!(unready.native_records().is_empty());
    assert!(unready.held_effects().next().is_none());
    let next = state.verified_lane_consensus_contexts().unwrap().unwrap();
    for ((lane, owner), (decision, effects, records)) in local.iter_mut().zip(&original_local) {
        assert!(
            !next
                .contexts()
                .iter()
                .any(|context| context.instance_id() == lane.instance_id()),
            "publication genuinely closes the old slot before local completion"
        );
        application
            .authorizes(owner.state_owner_for_test(), lane, decision)
            .unwrap();
        assert!(
            application
                .authorizes(foreign_owner.state_owner_for_test(), lane, decision)
                .is_err()
        );
        let mut other_value = decision.clone();
        other_value.manifest.value.payload_hash =
            iroha_crypto::Hash::new(b"different immutable input");
        fixture.resign_local_decision(lane, &mut other_value);
        assert!(
            application
                .authorizes(owner.state_owner_for_test(), lane, &other_value)
                .is_err()
        );
        let mut other_manifest = decision.clone();
        other_manifest.manifest.byte_len += 1;
        assert!(
            application
                .authorizes(owner.state_owner_for_test(), lane, &other_manifest)
                .is_err()
        );
        assert!(
            application
                .authorizes(owner.state_owner_for_test(), &next.contexts()[0], decision)
                .is_err()
        );
        let mut other_group = decision.clone();
        other_group.manifest.value.admitted_binding_hash =
            iroha_crypto::Hash::new(b"different admitted group");
        other_group.commit_qc.statement.value = other_group.manifest.value;
        assert!(
            application
                .authorizes(owner.state_owner_for_test(), lane, &other_group)
                .is_err()
        );
        // A genuinely opened foreign State instance retains the same immutable
        // Decision, yet cannot consume its original Apply using this publication.
        assert!(foreign_owner.settle_published_apply(&application).is_err());
        assert_eq!(
            foreign_owner.native_decision().unwrap().as_ref(),
            Some(&foreign_decision)
        );
        assert_eq!(
            foreign_owner.held_effects().cloned().collect::<Vec<_>>(),
            foreign_effects
        );
        assert_eq!(foreign_owner.native_records(), foreign_records);
        assert_eq!(owner.held_effects().cloned().collect::<Vec<_>>(), *effects);
        use crate::sumeragi::{v2_core as core, v2_lane_instance::LaneApplySettlement};
        owner.restrict_effect_capacity_to_retained_for_test();
        let LaneApplySettlement::Applied(receipt) =
            owner.settle_published_apply(&application).unwrap()
        else {
            panic!("genuine publication must settle the original local Apply");
        };
        assert_eq!(receipt.disposition, core::StepDisposition::Applied);
        assert_eq!(owner.retirement_count(), 0);
        let remaining = effects
            .iter()
            .filter(|effect| !matches!(effect, core::Effect::Apply { .. }))
            .cloned()
            .collect::<Vec<_>>();
        assert_eq!(owner.held_effects().cloned().collect::<Vec<_>>(), remaining);
        assert_eq!(owner.native_decision().unwrap().as_ref(), Some(decision));
        assert_eq!(
            owner.native_records(),
            records,
            "no original local QC/WAL was replaced"
        );
        assert!(matches!(
            owner.settle_published_apply(&application).unwrap(),
            LaneApplySettlement::AlreadyApplied
        ));
        assert_eq!(owner.held_effects().cloned().collect::<Vec<_>>(), remaining);
    }
    assert!(
        published
            .source
            .retains_carrier(published.block(), fixture.context())
    );
    assert_eq!(state.committed_height(), usize::try_from(height).unwrap());
    assert_eq!(state.latest_block_hash_fast(), Some(header.hash()));
    assert_eq!(
        crate::snapshot::canonical_state_snapshot_hash(state).unwrap(),
        checkpoint
    );
    assert_ne!(checkpoint, before);
    assert_native_effects(&fixture, published.block(), atomic);
    let lease = state.kura.try_publication_lease().unwrap();
    lease
        .reauthenticate_checkpoint(&published.checkpoint, &finality, checkpoint)
        .unwrap();
    drop(lease);
    assert!(
        crate::block::ValidBlock::validate_inactive_native_carrier_for_test(published.block())
            .unwrap_err()
            .to_string()
            .contains("not active")
    );
    assert_original_writers_released(state, header);
    assert_eq!(state.state_view_generation(), generation + 2);
    assert_eq!(
        crate::snapshot::canonical_state_snapshot_hash(state).unwrap(),
        checkpoint
    );
    assert_eq!(capture_released.load(Ordering::SeqCst), 0);
    assert_eq!(capture_budget.reserved_bytes(), retained_shell_bytes);
    drop(published);
    assert_eq!(capture_released.load(Ordering::SeqCst), 1);
    assert_eq!(capture_budget.reserved_bytes(), 0);
    assert_eq!(state.state_view_generation(), generation + 2);
    assert_native_effects(&fixture, fixture.carrier(), atomic);
}

fn assert_native_effects(fixture: &NativePublicationFixture, block: &SignedBlock, atomic: bool) {
    let state = fixture.state();
    let view = state.view();
    let (source, destination) = fixture.assets();
    assert_eq!(
        view.world().assets().get(source).unwrap().0,
        Quantity::from(75u32)
    );
    assert_eq!(
        view.world().assets().get(destination).unwrap().0,
        Quantity::from(25u32)
    );
    assert_eq!(
        view.world()
            .global_beacon_pulses()
            .get(&fixture.pulse().pulse_id),
        Some(fixture.pulse())
    );
    let height = block.header().height().get();
    for pending in fixture.pending_inputs() {
        let binding = State::pending_queue_plan_binding_for_execution(
            &view,
            &pending.entrypoint,
            &pending.routing_plan().unwrap(),
            height,
        )
        .unwrap()
        .expect("genuine later work remains pending after Native Apply");
        assert_eq!(
            binding.canonical_hash(),
            pending.certificate.binding.canonical_hash()
        );
        assert!(
            view.transactions()
                .get(&pending.entrypoint.hash())
                .is_none()
        );
    }
    assert_eq!(block.network_entrypoint_count(), 1);
    let transaction = block
        .network_input_hashes()
        .next()
        .expect("actual Native input");
    assert_eq!(
        view.transactions()
            .get(&transaction)
            .map(std::num::NonZeroUsize::get),
        Some(usize::try_from(height).unwrap())
    );
    drop(view);
    let next = state.verified_lane_consensus_contexts().unwrap().unwrap();
    assert_eq!(next.contexts().len(), if atomic { 2 } else { 1 });
    for context in next.contexts() {
        let context = context.frozen();
        assert_eq!(context.opening_global_height, height);
        assert_eq!(context.opening_global_context_id, fixture.context().id());
        assert_eq!(
            context.admitted_binding_hash,
            fixture.pending_inputs()[0]
                .certificate
                .binding
                .canonical_hash()
        );
        assert_eq!(context.committee.len(), 4);
        assert_eq!(context.da_layout.encoding, PayloadEncoding::ReedSolomon16);
    }
}

#[cfg(all(unix, not(target_os = "espidf")))]
#[test]
fn native_driver_settles_original_closed_apply_only_after_real_publication() {
    let fixture = native_publication_fixture(true);
    assert_native_driver_publication(fixture, false);
}

#[cfg(all(unix, not(target_os = "espidf")))]
#[test]
fn native_driver_settles_complete_published_carrier_after_owned_worker_handoff() {
    assert_native_driver_publication(native_publication_fixture(true), true);
}

#[inline(never)]
fn assert_native_driver_publication(fixture: Box<NativePublicationFixture>, whole_carrier: bool) {
    use crate::sumeragi::{
        v2_core as core,
        v2_lane_driver::{NativeLaneAdmission, NativeLaneInput},
        v2_lane_instance::LaneApplySettlement,
    };
    use std::time::{Duration, Instant};

    let observed = fixture
        .state()
        .verified_lane_consensus_contexts()
        .unwrap()
        .unwrap();
    let prepared = fixture.prepare();
    let original = prepared
        .native_source_for_test()
        .unwrap()
        .sources_for_test()
        .iter()
        .flat_map(|group| group.contexts().iter().zip(group.decisions()))
        .map(|(lane, decision)| {
            let mut local = decision.clone();
            fixture.resign_local_decision(lane, &mut local);
            (lane.instance_id(), local)
        })
        .collect::<Vec<_>>();
    assert_eq!(original.len(), 2, "all actual atomic legs are present");
    let finality = fixture.finality(prepared.block(), prepared.execution_prefix_commitment());
    let journals = prepared
        .prepare_journals(None, None, |_| Ok::<_, Infallible>(()))
        .unwrap();
    let checkpoint = journals.checkpoint;
    let decided = journals
        .bind_decision(finality)
        .unwrap_or_else(|refusal| panic!("exact global binding: {:?}", refusal.error));
    // The State moves once after detachment; every original storage identity is
    // retained. No replacement State or synthetic publication acknowledgement.
    let (state, mut driver) = fixture.into_shared_driver();
    assert!(observed.is_current(&state));
    for (_, local) in &original {
        assert!(matches!(
            driver.admit(&observed, NativeLaneInput::Decision(local.clone())),
            NativeLaneAdmission::Accepted
        ));
    }
    let now = Instant::now();
    let deadline = now + Duration::from_secs(30);
    loop {
        driver.poll(&observed, now).unwrap();
        if original.iter().all(|(id, decision)| {
            driver.process().instance(*id).is_some_and(|owner| {
                owner.native_decision().unwrap().as_ref() == Some(decision)
                    && owner
                        .held_effects()
                        .any(|effect| matches!(effect, core::Effect::Apply { .. }))
            })
        }) {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "real driver WAL/body work must reach original Apply"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
    let custody = original
        .iter()
        .map(|(id, _)| {
            let owner = driver.process().instance(*id).unwrap();
            (
                std::ptr::from_ref(owner),
                owner.native_records().to_vec(),
                owner.held_effects().cloned().collect::<Vec<_>>(),
            )
        })
        .collect::<Vec<_>>();
    let before = crate::snapshot::canonical_state_snapshot_hash(&state).unwrap();
    let finality = decided.finality().clone();
    state.kura.store_block(decided.block().clone()).unwrap();
    let receipt = state.kura.store_v2_finality_artifact(&finality).unwrap();
    let checkpoint = state
        .kura
        .persist_wsv_checkpoint_for_v2_commit(&receipt, checkpoint)
        .unwrap();
    let decided = decided.attach_checkpoint(checkpoint);
    assert_eq!(
        crate::snapshot::canonical_state_snapshot_hash(&state).unwrap(),
        before
    );
    let physical = decided
        .try_prepare_physical(&state, None)
        .unwrap_or_else(|(_, error)| panic!("exact physical acquisition: {error:?}"));
    let published = physical
        .publish()
        .unwrap_or_else(|(_, error)| panic!("actual global publication: {error:?}"));
    let published = handoff_published_carrier(published);
    let application = published.native_apply().unwrap();
    let current = state.verified_lane_consensus_contexts().unwrap().unwrap();
    assert!(original.iter().all(|(id, _)| {
        current
            .contexts()
            .iter()
            .all(|lane| lane.instance_id() != *id)
    }));
    if whole_carrier {
        assert!(
            !driver.settle_published_carrier(&application).unwrap(),
            "actual Apply settles while original physical custody still needs drain"
        );
        for ((id, decision), (owner_ptr, records, effects)) in original.iter().zip(&custody) {
            let owner = driver.process().instance(*id).unwrap();
            assert_eq!(std::ptr::from_ref(owner), *owner_ptr);
            assert_eq!(owner.native_decision().unwrap().as_ref(), Some(decision));
            assert_eq!(owner.native_records(), records);
            let expected = effects
                .iter()
                .filter(|effect| !matches!(effect, core::Effect::Apply { .. }))
                .cloned()
                .collect::<Vec<_>>();
            assert_eq!(owner.held_effects().cloned().collect::<Vec<_>>(), expected);
        }
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            driver
                .poll(&current, now)
                .expect("publication completion must not fault the original driver guard");
            if driver.settle_published_carrier(&application).unwrap() {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "original publication custody must complete physical drain"
            );
            std::thread::sleep(Duration::from_millis(1));
        }
        assert!(
            original
                .iter()
                .all(|(id, _)| driver.process().instance(*id).is_none()),
            "every original published instance has retired; successor openings remain independent"
        );
        driver
            .poll(&current, now)
            .expect("no output guard fault after actual original retirement");
        assert!(
            driver.settle_published_carrier(&application).unwrap(),
            "complete original retirement is idempotent"
        );
        assert_eq!(
            state.committed_height(),
            usize::try_from(published.block().header().height().get()).unwrap()
        );
        driver.shutdown().join().unwrap();
        return;
    }
    let close_deadline = Instant::now() + Duration::from_secs(30);
    while driver.process().occupancy().closed < original.len() {
        driver.poll(&current, now).unwrap();
        assert!(
            Instant::now() < close_deadline,
            "actual original handles must finish their closed drain"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
    for ((id, decision), (owner_ptr, records, effects)) in original.iter().zip(&custody) {
        let owner = driver.process().instance(*id).unwrap();
        assert_eq!(std::ptr::from_ref(owner), *owner_ptr);
        assert!(!driver.process().is_productive(*id));
        assert_eq!(owner.native_decision().unwrap().as_ref(), Some(decision));
        assert_eq!(owner.native_records(), records);
        assert_eq!(
            owner.held_effects().cloned().collect::<Vec<_>>(),
            *effects,
            "authenticated absence and physical drain do not acknowledge Apply"
        );
    }
    let first = original[0].0;
    assert!(matches!(
        driver.settle_published_apply(first, &application).unwrap(),
        Some(LaneApplySettlement::Applied(_))
    ));
    assert!(matches!(
        driver.settle_published_apply(first, &application).unwrap(),
        Some(LaneApplySettlement::AlreadyApplied)
    ));
    assert!(
        !driver
            .process()
            .instance(first)
            .unwrap()
            .held_effects()
            .any(|effect| matches!(effect, core::Effect::Apply { .. }))
    );
    assert_eq!(
        driver.process().instance(first).unwrap().native_records(),
        custody[0].1
    );
    let second = original[1].0;
    let mut closed = driver
        .take_closed(second)
        .expect("exact drained owner transfer");
    assert!(
        driver
            .settle_published_apply(second, &application)
            .unwrap()
            .is_none()
    );
    let original_closed = std::ptr::from_ref(closed.instance());
    let (returned, error) = match closed.retire_published(&application) {
        Err(refusal) => refusal,
        Ok(()) => panic!("held Apply must use the actual reducer settlement"),
    };
    closed = returned;
    assert!(error.to_string().contains("original Apply"));
    assert_eq!(std::ptr::from_ref(closed.instance()), original_closed);
    assert_eq!(closed.instance().native_records(), custody[1].1);
    assert_eq!(
        closed
            .instance()
            .held_effects()
            .cloned()
            .collect::<Vec<_>>(),
        custody[1].2
    );
    assert!(matches!(
        closed.settle_published_apply(&application).unwrap(),
        LaneApplySettlement::Applied(_)
    ));
    assert!(matches!(
        closed.settle_published_apply(&application).unwrap(),
        LaneApplySettlement::AlreadyApplied
    ));
    assert_eq!(closed.instance().native_records(), custody[1].1);
    assert!(
        !closed
            .instance()
            .held_effects()
            .any(|effect| matches!(effect, core::Effect::Apply { .. }))
    );
    closed
        .retire_published(&application)
        .unwrap_or_else(|(_, error)| panic!("settled original owner can retire: {error}"));
    driver
        .take_closed(first)
        .unwrap()
        .retire_published(&application)
        .unwrap_or_else(|(_, error)| panic!("first original settled owner can retire: {error}"));
    driver.shutdown().join().unwrap();
}

// These cases use the actual publisher plus original physical opening/body/WAL
// work. No local Ready/Decision/Apply is fabricated to authorize cleanup.
#[derive(Clone, Copy)]
enum TerminalDecisionCase {
    None,
    DurableMatching,
    DurableConflicting,
    HeldMatching,
    HeldConflicting,
}

#[cfg(all(unix, not(target_os = "espidf")))]
#[test]
fn native_published_terminal_retires_closed_body_without_local_qc() {
    // Finish constructing/publishing the independent State before entering the
    // original process test frame. Both actual owners remain live on the heap.
    let foreign = unrelated_native_terminal_publication();
    let fixture = native_publication_fixture(false);
    assert_native_terminal_publication(fixture, TerminalDecisionCase::None, Some(foreign));
}

#[cfg(all(unix, not(target_os = "espidf")))]
#[test]
fn native_published_terminal_checks_unacknowledged_durable_decision() {
    let fixture = native_publication_fixture(false);
    assert_native_terminal_publication(fixture, TerminalDecisionCase::DurableMatching, None);
}

#[cfg(all(unix, not(target_os = "espidf")))]
#[test]
fn native_published_terminal_refuses_conflicting_durable_decision() {
    let fixture = native_publication_fixture(false);
    assert_native_terminal_publication(fixture, TerminalDecisionCase::DurableConflicting, None);
}

#[cfg(all(unix, not(target_os = "espidf")))]
#[test]
fn native_published_terminal_checks_unlaunched_decision() {
    let fixture = native_publication_fixture(false);
    assert_native_terminal_publication(fixture, TerminalDecisionCase::HeldMatching, None);
}

#[cfg(all(unix, not(target_os = "espidf")))]
#[test]
fn native_published_terminal_refuses_conflicting_unlaunched_decision() {
    let fixture = native_publication_fixture(false);
    assert_native_terminal_publication(fixture, TerminalDecisionCase::HeldConflicting, None);
}

#[inline(never)]
fn receive_native_terminal_work(
    pool: &crate::sumeragi::v2_lane_instance::LanePhysicalPool,
) -> crate::sumeragi::v2_lane_instance::LanePhysicalCompletion {
    let until = std::time::Instant::now() + std::time::Duration::from_secs(30);
    loop {
        if let Some(completion) = pool.try_completion().unwrap() {
            return completion;
        }
        assert!(
            std::time::Instant::now() < until,
            "original physical work returns"
        );
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
}

#[inline(never)]
fn unrelated_native_terminal_publication() -> Box<PublishedCarrier<()>> {
    let fixture = native_publication_fixture(false);
    publish_unrelated_native_terminal_fixture(fixture)
}

#[inline(never)]
fn publish_unrelated_native_terminal_fixture(
    fixture: Box<NativePublicationFixture>,
) -> Box<PublishedCarrier<()>> {
    let prepared = fixture.prepare();
    let finality = fixture.finality(prepared.block(), prepared.execution_prefix_commitment());
    let journals = prepared
        .prepare_journals(None, None, |_| Ok::<_, Infallible>(()))
        .unwrap();
    let checkpoint = journals.checkpoint;
    let decided = journals
        .bind_decision(finality)
        .unwrap_or_else(|refusal| panic!("actual unrelated decision: {:?}", refusal.error));
    let state = fixture.into_shared_state();
    let finality = decided.finality().clone();
    state.kura.store_block(decided.block().clone()).unwrap();
    let receipt = state.kura.store_v2_finality_artifact(&finality).unwrap();
    let checkpoint = state
        .kura
        .persist_wsv_checkpoint_for_v2_commit(&receipt, checkpoint)
        .unwrap();
    let physical = decided
        .attach_checkpoint(checkpoint)
        .try_prepare_physical(&state, None)
        .unwrap_or_else(|(_, error)| panic!("actual unrelated physical owner: {error:?}"));
    Box::new(
        physical
            .publish()
            .unwrap_or_else(|(_, error)| panic!("actual unrelated publication: {error:?}")),
    )
}

#[inline(never)]
fn assert_native_terminal_publication(
    fixture: Box<NativePublicationFixture>,
    case: TerminalDecisionCase,
    foreign: Option<Box<PublishedCarrier<()>>>,
) {
    use crate::sumeragi::{
        output_guard::ConsensusOutputGuard,
        v2_core as core,
        v2_lane_instance::{
            LaneApplySettlement, LaneBodyProgress, LaneCurrentGate, LaneInputOutcome,
            LanePhysicalPool, LaneProcessOwner, LaneProcessProgress, LaneService, LaneWorkerClass,
        },
        v2_lane_wire::LaneWalRecordV1,
    };
    use iroha_data_model::block::lane_consensus::LaneMessageV1;
    use std::{
        sync::mpsc,
        time::{Duration, Instant},
    };

    let observed = fixture
        .state()
        .verified_lane_consensus_contexts()
        .unwrap()
        .unwrap();
    let lane = observed.contexts()[0].clone();
    let id = lane.instance_id();
    let leader = lane
        .reducer_context()
        .roster()
        .iter()
        .position(|validator| validator.id() == lane.reducer_context().leader(0))
        .unwrap();
    let key = fixture.key_for(&lane, leader);
    let prepared = fixture.prepare();
    let source = prepared.native_source_for_test().unwrap();
    let expected_body = source.sources_for_test()[0]
        .body()
        .canonical_bytes()
        .to_vec();
    let published_decision = source.sources_for_test()[0].decisions()[0].clone();
    let mut local_decision = published_decision.clone();
    let conflict = matches!(
        case,
        TerminalDecisionCase::DurableConflicting | TerminalDecisionCase::HeldConflicting
    );
    let durable = matches!(
        case,
        TerminalDecisionCase::DurableMatching | TerminalDecisionCase::DurableConflicting
    );
    if conflict {
        // A genuinely signed, independently valid conflicting CommitQC is an
        // invariant contradiction, not a malformed unsigned test placeholder.
        local_decision.manifest.value.payload_hash =
            iroha_crypto::Hash::new(b"other committed input");
    }
    fixture.resign_local_decision(&lane, &mut local_decision);
    assert_ne!(
        local_decision.commit_qc.shares, published_decision.commit_qc.shares,
        "the positive deliberately uses another valid exact quorum subset"
    );
    let finality = fixture.finality(prepared.block(), prepared.execution_prefix_commitment());
    let journals = prepared
        .prepare_journals(None, None, |_| Ok::<_, Infallible>(()))
        .unwrap();
    let checkpoint = journals.checkpoint;
    let decided = journals
        .bind_decision(finality)
        .unwrap_or_else(|refusal| panic!("actual original decision binding: {:?}", refusal.error));
    let state = fixture.into_shared_state();
    let guard = ConsensusOutputGuard::isolated();
    let limits = NativePublicationFixture::process_limits();
    let mut process =
        LaneProcessOwner::new(Arc::clone(&state), Arc::clone(&guard), limits).unwrap();
    let pool = LanePhysicalPool::new(Arc::clone(&state), Arc::clone(&guard), limits).unwrap();
    let now = Instant::now();
    process.reserve_opening(&observed, &lane, key, now).unwrap();
    // A weak witness prevents allocator address reuse without supplying another
    // strong owner that could hide loss of original physical context custody.
    let original_context_custody = Arc::downgrade(
        process
            .queued_context_for_test(id, LaneWorkerClass::Opening)
            .unwrap(),
    );
    let original_context = original_context_custody.as_ptr();
    process
        .dispatch_one(&pool, LaneWorkerClass::Opening)
        .unwrap();
    process
        .accept_completion(receive_native_terminal_work(&pool), &observed)
        .unwrap();
    assert!(matches!(
        process.settle_opening(id, &observed).unwrap(),
        LaneProcessProgress::OpeningAdopted
    ));
    assert_eq!(
        std::ptr::from_ref(process.instance(id).unwrap().context_for_test()),
        original_context,
        "physical opening and adoption move the original immutable context"
    );
    process.prepare_body(id, &observed).unwrap();
    assert_eq!(
        Arc::as_ptr(
            process
                .queued_context_for_test(id, LaneWorkerClass::Body)
                .unwrap()
        ),
        original_context,
        "the actual body job shares its original instance's immutable allocation"
    );
    let (entered, entered_rx) = mpsc::channel();
    let (release, release_rx) = mpsc::channel();
    process
        .hold_next_completion_for_test(id, LaneWorkerClass::Body, move || {
            entered.send(()).unwrap();
            let _ = release_rx.recv();
        })
        .unwrap();
    process.dispatch_one(&pool, LaneWorkerClass::Body).unwrap();
    entered_rx.recv_timeout(Duration::from_secs(30)).unwrap();
    // Actual body allocation now exists; it is held before completion delivery.
    // The local reducer has never learned Ready and has no local body manifest.
    assert!(
        process
            .instance(id)
            .unwrap()
            .native_decision()
            .unwrap()
            .is_none()
    );
    if !matches!(case, TerminalDecisionCase::None) {
        assert!(matches!(
            process
                .offer(
                    id,
                    &observed,
                    &LaneMessageV1::QuorumCertificate(local_decision.commit_qc.clone())
                )
                .unwrap(),
            LaneInputOutcome::Stepped(_)
        ));
        if durable {
            process.prepare_persistence(id).unwrap();
            process.dispatch_one(&pool, LaneWorkerClass::Wal).unwrap();
            assert!(matches!(
                process
                    .accept_completion(receive_native_terminal_work(&pool), &observed)
                    .unwrap(),
                LaneProcessProgress::Persistence(LaneService::PersistedAwaitingAck)
            ));
            assert_eq!(
                process.instance(id).unwrap().durable_decision_certificate(),
                Some(&local_decision.commit_qc)
            );
        } else {
            assert!(process.instance(id).unwrap().native_records().is_empty());
            assert!(process.instance(id).unwrap().held_effects().any(|effect| matches!(effect,
                core::Effect::Persist { entry, .. } if matches!(entry.record(), core::WalRecord::Decision(_)))));
        }
    }
    assert!(
        process
            .instance(id)
            .unwrap()
            .native_decision()
            .unwrap()
            .is_none(),
        "a durable Decision without its local body manifest is still not local Apply"
    );
    let original_owner = std::ptr::from_ref(process.instance(id).unwrap());
    let original_tag = process.instance(id).unwrap().tag();
    let original_records = process.instance(id).unwrap().native_records().to_vec();
    let original_effects = process
        .instance(id)
        .unwrap()
        .held_effects()
        .cloned()
        .collect::<Vec<_>>();
    assert!(
        !original_effects
            .iter()
            .any(|effect| matches!(effect, core::Effect::Apply { .. }))
    );
    if !durable {
        process.restrict_effect_capacity_to_retained_for_test(id);
    }
    // A returned WAL completion still owns its reserved successor capacity.
    // Keep that reservation intact while independently retiring the body result.

    let finality = decided.finality().clone();
    state.kura.store_block(decided.block().clone()).unwrap();
    let receipt = state.kura.store_v2_finality_artifact(&finality).unwrap();
    let checkpoint = state
        .kura
        .persist_wsv_checkpoint_for_v2_commit(&receipt, checkpoint)
        .unwrap();
    let before = crate::snapshot::canonical_state_snapshot_hash(&state).unwrap();
    let physical = decided
        .attach_checkpoint(checkpoint)
        .try_prepare_physical(&state, None)
        .unwrap_or_else(|(_, error)| panic!("actual original physical publication: {error:?}"));
    let published = physical
        .publish()
        .unwrap_or_else(|(_, error)| panic!("actual original publication: {error:?}"));
    assert_ne!(
        crate::snapshot::canonical_state_snapshot_hash(&state).unwrap(),
        before
    );
    let proof = published.native_apply().unwrap();
    let current = state.verified_lane_consensus_contexts().unwrap().unwrap();
    assert!(
        current
            .contexts()
            .iter()
            .all(|context| context.instance_id() != id)
    );
    assert_eq!(process.reconcile(&current), LaneCurrentGate::Current);
    release.send(()).unwrap();
    assert!(matches!(
        process
            .accept_completion(receive_native_terminal_work(&pool), &current)
            .unwrap(),
        LaneProcessProgress::Body(LaneBodyProgress::Retired)
    ));
    process.prepare_closed_drain(id).unwrap();
    process.dispatch_one(&pool, LaneWorkerClass::Body).unwrap();
    assert!(matches!(
        process
            .accept_completion(receive_native_terminal_work(&pool), &current)
            .unwrap(),
        LaneProcessProgress::ClosedDrained
    ));
    let mut closed = process.take_closed(id).unwrap();
    assert!(process.take_closed(id).is_none());
    assert_eq!(std::ptr::from_ref(closed.instance()), original_owner);
    assert_eq!(
        std::ptr::from_ref(closed.instance().context_for_test()),
        original_context,
        "publication and physical drain do not rebuild the closed instance context"
    );
    assert_eq!(closed.instance().tag(), original_tag);
    assert_eq!(closed.instance().native_records(), original_records);
    assert_eq!(
        closed
            .instance()
            .held_effects()
            .cloned()
            .collect::<Vec<_>>(),
        original_effects
    );
    assert_eq!(closed.instance().retirement_count(), 1);
    assert_eq!(closed.unacknowledged_control().is_some(), durable);
    assert!(matches!(
        closed.settle_published_apply(&proof).unwrap(),
        LaneApplySettlement::NotReady
    ));
    assert_eq!(
        closed.instance().tag(),
        original_tag,
        "publication did not synthesize Ready/Apply"
    );
    assert_eq!(closed.instance().native_records(), original_records);
    assert_eq!(
        closed
            .instance()
            .held_effects()
            .cloned()
            .collect::<Vec<_>>(),
        original_effects
    );
    assert_eq!(
        original_records
            .iter()
            .filter(|record| matches!(record.record, LaneWalRecordV1::Decision(_)))
            .count(),
        usize::from(durable)
    );
    assert!(!guard.restart_required());

    // This same State's genuine next opening is not a member of the published
    // group. The original State identity alone cannot grant terminal authority.
    assert!(!current.contexts().is_empty());
    assert!(
        proof
            .authorizes_terminal(
                closed.instance().state_owner_for_test(),
                &current.contexts()[0],
                std::iter::empty()
            )
            .is_err()
    );

    if conflict {
        let (returned, _) = match closed.retire_published(&proof) {
            Err(refusal) => refusal,
            Ok(()) => panic!("known conflicting Decision must retain its exact closed owner"),
        };
        closed = returned;
        assert_eq!(std::ptr::from_ref(closed.instance()), original_owner);
        assert_eq!(closed.instance().native_records(), original_records);
        assert_eq!(
            closed
                .instance()
                .held_effects()
                .cloned()
                .collect::<Vec<_>>(),
            original_effects
        );
        assert_eq!(closed.instance().retirement_count(), 1);
        assert_eq!(closed.unacknowledged_control().is_some(), durable);
        assert!(
            !guard.restart_required(),
            "proof refusal retains ownership without dropping it"
        );
        drop(closed);
        assert!(
            guard.restart_required(),
            "discarding refused custody still fences output"
        );
    } else if matches!(case, TerminalDecisionCase::None) {
        let foreign = foreign
            .as_ref()
            .expect("original unrelated publication retained");
        let foreign_proof = foreign.native_apply().unwrap();
        let (returned, _) = match closed.retire_published(&foreign_proof) {
            Err(refusal) => refusal,
            Ok(()) => panic!("another actual State publication is not this original owner"),
        };
        closed = returned;
        assert_eq!(std::ptr::from_ref(closed.instance()), original_owner);
        assert_eq!(closed.instance().tag(), original_tag);
        assert_eq!(closed.instance().retirement_count(), 1);
        assert!(!guard.restart_required());
        let retired = closed.take_retirement().unwrap();
        assert_eq!(
            std::ptr::from_ref(retired.context_for_test()),
            original_context,
            "taking closure custody shares the context without a cleanup allocation"
        );
        assert_eq!(retired.body_bytes(), Some(expected_body.as_slice()));
        assert_eq!(retired.instance(), id);
        let body_pointer = retired.body_bytes().unwrap().as_ptr();
        let (retired, _) = match retired.retire_published(&foreign_proof) {
            Err(refusal) => refusal,
            Ok(()) => panic!("foreign publication cannot consume the taken original body"),
        };
        assert_eq!(retired.body_bytes().unwrap().as_ptr(), body_pointer);
        assert_eq!(
            std::ptr::from_ref(retired.context_for_test()),
            original_context
        );
        assert!(retired.requires_recovery());
        assert!(!guard.restart_required());
        closed
            .retire_published(&proof)
            .unwrap_or_else(|(_, error)| panic!("actual no-QC terminal owner: {error}"));
        assert!(
            retired.requires_recovery(),
            "closed consumption did not disarm separately transferred custody"
        );
        assert_eq!(
            std::ptr::from_ref(retired.context_for_test()),
            original_context,
            "taken retirement retains the original context after its instance is consumed"
        );
        assert_eq!(retired.context_for_test().frozen(), lane.frozen());
        assert_eq!(original_context_custody.strong_count(), 1);
        assert!(!guard.restart_required());
        retired
            .retire_published(&proof)
            .unwrap_or_else(|(_, error)| panic!("actual taken body terminal proof: {error}"));
        assert_eq!(original_context_custody.strong_count(), 0);
        assert!(
            !guard.restart_required(),
            "authorized consuming retirement releases without false fail-stop"
        );
    } else {
        closed
            .retire_published(&proof)
            .unwrap_or_else(|(_, error)| panic!("exact authenticated local Decision: {error}"));
        assert!(
            !guard.restart_required(),
            "actual original unacknowledged Decision was checked, never acknowledged"
        );
    }
    assert_eq!(process.occupancy().instances, 0);
    drop(process);
    pool.shutdown().join().unwrap();
}

/// Compile and exercise the crate-facing owned handoff on a different worker.
fn handoff_published_carrier<A>(
    published: crate::state::PublishedCarrier<A>,
) -> crate::state::PublishedCarrier<A>
where
    A: Send + 'static,
{
    let (send, receive) = std::sync::mpsc::sync_channel(1);
    let worker = std::thread::spawn(move || {
        assert!(published.native_apply().is_some());
        assert!(send.send(published).is_ok());
    });
    let published = receive.recv().expect("exact owned publication completion");
    worker.join().expect("publication completion worker");
    published
}
