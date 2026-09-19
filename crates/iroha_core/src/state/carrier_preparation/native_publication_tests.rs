//! Actual single and atomic Native journals reach the sole terminal consumer.

use super::*;
use crate::state::{
    StateReadOnly, StateReadOnlyWithTransactions, WorldReadOnly,
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

struct Reservation(Arc<AtomicUsize>);

impl Drop for Reservation {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
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
    let fixture = native_publication_fixture(false);
    assert_native_publication(false, fixture);
}

#[test]
fn native_atomic_publishes_original_sources_and_exact_checkpoint_once() {
    let fixture = native_publication_fixture(true);
    assert_native_publication(true, fixture);
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
    let binding_released = Arc::new(AtomicUsize::new(0));
    let installation_released = Arc::new(AtomicUsize::new(0));
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
        .prepare_journals(None, None, |_| {
            Ok::<_, Infallible>(Reservation(Arc::clone(&capture_released)))
        })
        .unwrap();
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
    assert!(journals.effects.pending_autoscale_lifecycle.is_none());
    let checkpoint = journals.checkpoint;
    let decision = journals
        .bind_decision(finality, |_| {
            Ok::<_, Infallible>(Reservation(Arc::clone(&binding_released)))
        })
        .unwrap_or_else(|refusal| panic!("real Native decision binding: {:?}", refusal.error));
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
        Reservation,
        DetachedCarrierComponents,
        KuraWsvCheckpointReceipt,
    >| {
        decision
            .try_prepare_physical(state, |_, _| {
                Ok::<_, Infallible>(Reservation(Arc::clone(&installation_released)))
            })
            .unwrap_or_else(|(_, error)| panic!("actual Native physical acquisition: {error:?}"))
    };
    // A real abort releases every acquired writer, preserving exact source and
    // checkpoint custody for the sole later successful consuming publication.
    let decision = acquire(decision).abort();
    assert_eq!(installation_released.load(Ordering::SeqCst), 1);
    assert_eq!(capture_released.load(Ordering::SeqCst), 0);
    assert_eq!(binding_released.load(Ordering::SeqCst), 0);
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
    let generation = state.state_view_generation();
    let published = acquire(decision)
        .publish()
        .unwrap_or_else(|(_, error)| panic!("complete genuine Native publication: {error:?}"));
    assert_eq!(state.state_view_generation(), generation + 2);
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
    assert_eq!(binding_released.load(Ordering::SeqCst), 0);
    assert_eq!(installation_released.load(Ordering::SeqCst), 1);
    drop(published);
    assert_eq!(capture_released.load(Ordering::SeqCst), 1);
    assert_eq!(binding_released.load(Ordering::SeqCst), 1);
    assert_eq!(installation_released.load(Ordering::SeqCst), 2);
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
