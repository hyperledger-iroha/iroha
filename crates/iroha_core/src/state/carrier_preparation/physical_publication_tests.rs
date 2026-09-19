//! Joint physical ownership using actual executed four-validator genesis decisions.

use super::super::tests::{signed_finality, subject};
use super::*;
use crate::state::carrier_preparation::tests::{fixture, prepare};
use crate::sumeragi::network_topology::Topology;
use iroha_data_model::block::{SignedBlock, consensus_v2::HeightContext};
use std::{
    future::Future,
    pin::Pin,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    task::{Context, Poll, Wake, Waker},
};

type CheckpointDecision<A, B> =
    DecisionBoundCarrierJournals<A, B, DetachedCarrierComponents, KuraWsvCheckpointReceipt>;

fn decided<A, B>(
    state: &State,
    proposal: SignedBlock,
    topology: &Topology,
    context: &HeightContext,
    admission: A,
    binding: B,
) -> CheckpointDecision<A, B> {
    let journals = prepare(state, proposal, topology, context)
        .unwrap_or_else(|(_, error)| panic!("real execution: {error}"))
        .prepare_journals(None, None, |_| Ok::<_, Infallible>(admission))
        .unwrap();
    let finality = signed_finality(
        context.clone(),
        subject(journals.valid.as_ref()),
        journals.execution_prefix,
        0,
    );
    let decision = journals
        .bind_decision(finality, |_| Ok::<_, Infallible>(binding))
        .unwrap_or_else(|refusal| panic!("exact decision: {:?}", refusal.error));
    state.kura.store_block(decision.block().clone()).unwrap();
    let finality = state
        .kura
        .store_v2_finality_artifact(decision.finality())
        .unwrap();
    let checkpoint = state
        .kura
        .persist_wsv_checkpoint_for_v2_commit(&finality, decision.journals.checkpoint)
        .unwrap();
    decision.attach_checkpoint(checkpoint)
}

pub(super) fn fixture_decision() -> (Box<State>, CheckpointDecision<(), ()>) {
    let (state, proposal, topology, context) = fixture();
    let decision = decided(&state, proposal, &topology, &context, (), ());
    (state, decision)
}

pub(super) fn acquire<'target, A, B>(
    decision: CheckpointDecision<A, B>,
    state: &'target State,
) -> PhysicallyPreparedCarrier<'target, A, B, ()> {
    decision
        .try_prepare_physical(state, |_, _| Ok::<_, Infallible>(()))
        .unwrap_or_else(|(_, error)| panic!("joint acquisition: {error:?}"))
}

#[derive(Default)]
struct WakeCount(AtomicUsize);
impl Wake for WakeCount {
    fn wake(self: Arc<Self>) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}
fn poll(wait: &mut mv::ReleaseFuture, wakes: &Arc<WakeCount>) -> Poll<()> {
    Pin::new(wait).poll(&mut Context::from_waker(&Waker::from(Arc::clone(wakes))))
}

fn busy_wait(error: CarrierPhysicalPreparationError<Infallible>) -> mv::ReleaseWait {
    match error {
        CarrierPhysicalPreparationError::Fence { wait, .. }
        | CarrierPhysicalPreparationError::Kura(KuraPublicationPreparationError::Busy {
            wait,
            ..
        }) => wait,
        CarrierPhysicalPreparationError::Component {
            cause: mv::PublicationPreparationError::Busy(wait),
            ..
        }
        | CarrierPhysicalPreparationError::Runtime(RuntimePublicationError::Component {
            cause: mv::PublicationPreparationError::Busy(wait),
            ..
        }) => wait,
        CarrierPhysicalPreparationError::World(WorldPublicationError::Field(field)) => {
            match field.cause {
                mv::PublicationPreparationError::Busy(wait) => wait,
                cause => panic!("World was not busy: {cause:?}"),
            }
        }
        error => panic!("expected exact physical Busy: {error:?}"),
    }
}

trait Held {}
impl<T> Held for T {}
fn hold<'state>(state: &'state State, name: &str) -> Box<dyn Held + 'state> {
    match name {
        "state_commit_lock" => Box::new(state.state_commit_lock.lock()),
        "lane_lifecycle_lock" => Box::new(state.lane_lifecycle_lock.lock()),
        "state_write_lock" => Box::new(state.state_write_lock.lock()),
        "block_hashes" => Box::new(state.block_hashes.view()),
        "transactions" => Box::new(state.transactions.block()),
        "canonical_runtime" => Box::new(state.canonical_runtime.block()),
        "commit_topology" => Box::new(state.commit_topology.block()),
        "prev_commit_topology" => Box::new(state.prev_commit_topology.block()),
        "lane_consensus_contexts" => Box::new(state.lane_consensus_contexts.block()),
        "world.accounts" => Box::new(state.world.accounts.block()),
        "world.triggers" => Box::new(state.world.triggers.block()),
        _ => panic!("unknown physical fixture owner"),
    }
}

fn assert_fences_free_except(state: &State, except: &str) {
    for (name, lock) in [
        ("state_commit_lock", state.state_commit_lock.as_ref()),
        ("lane_lifecycle_lock", &state.lane_lifecycle_lock),
        ("state_write_lock", &state.state_write_lock),
    ] {
        if name != except {
            assert!(lock.try_lock().is_some(), "retained {name}");
        }
    }
}

#[test]
fn every_busy_carrier_family_releases_earlier_writers_and_retains_exact_retry() {
    let (state, mut decision) = fixture_decision();
    let before = crate::snapshot::canonical_state_snapshot_hash(&state).unwrap();
    let generation = state.state_view_generation();
    let wire = decision.block().encode_wire().unwrap();
    let original_hashes = decision
        .journals
        .components
        .block_hashes
        .as_slice()
        .as_ptr();
    let original_membership = std::ptr::from_ref(
        decision
            .journals
            .components
            .transactions
            .staged_membership()
            .1,
    );
    for name in [
        "state_commit_lock",
        "lane_lifecycle_lock",
        "state_write_lock",
        "block_hashes",
        "transactions",
        "canonical_runtime",
        "commit_topology",
        "prev_commit_topology",
        "lane_consensus_contexts",
        "world.accounts",
        "world.triggers",
    ] {
        let held = hold(&state, name);
        let (retry, error) =
            match decision.try_prepare_physical(&state, |_, _| Ok::<_, Infallible>(())) {
                Ok(_) => panic!("held {name} must defer"),
                Err(refusal) => refusal,
            };
        assert_fences_free_except(&state, name);
        drop(
            state
                .kura
                .try_publication_lease()
                .expect("Kura released before State refusal"),
        );
        if name != "block_hashes" {
            assert!(state.block_hashes.inner.try_write().is_some());
        }
        if name != "transactions" {
            assert!(matches!(
                retry
                    .journals
                    .components
                    .transactions
                    .observe_predecessor(&state.transactions),
                crate::state::storage_transactions::MembershipPredecessorStatus::Current
            ));
        }
        if name.starts_with("world.") {
            // World is acquired last: all four preceding runtime writers must
            // have been aborted before this refusal is delivered.
            drop(state.canonical_runtime.block());
            drop(state.commit_topology.block());
            drop(state.prev_commit_topology.block());
            drop(state.lane_consensus_contexts.block());
        }
        assert_eq!(
            retry.journals.components.block_hashes.as_slice().as_ptr(),
            original_hashes
        );
        assert_eq!(
            std::ptr::from_ref(retry.journals.components.transactions.staged_membership().1),
            original_membership
        );
        assert_eq!(retry.block().encode_wire().unwrap(), wire);
        let mut wait = busy_wait(error).wait_for_release();
        let wakes = Arc::new(WakeCount::default());
        assert!(
            poll(&mut wait, &wakes).is_pending(),
            "failed owner is still held: {name}"
        );
        drop(held);
        assert_eq!(
            wakes.0.load(Ordering::SeqCst),
            1,
            "only actual release wakes {name}"
        );
        assert!(poll(&mut wait, &wakes).is_ready());
        decision = acquire(retry, &state).abort();
        assert_eq!(state.state_view_generation(), generation);
        assert_eq!(
            crate::snapshot::canonical_state_snapshot_hash(&state).unwrap(),
            before
        );
    }
    assert_eq!(state.kura.blocks_count(), 1);
}

#[test]
fn aggregate_acquisition_holds_every_family_without_publishing_or_losing_originals() {
    let (state, decision) = fixture_decision();
    let before = crate::snapshot::canonical_state_snapshot_hash(&state).unwrap();
    let generation = state.state_view_generation();
    let world_probe = state
        .world
        .block()
        .try_detach_journals(|_| Ok::<_, Infallible>(()))
        .unwrap();
    let runtime_probe = super::super::super::runtime_journals::RuntimeJournals::capture(
        state.canonical_runtime.block(),
        state.commit_topology.block(),
        state.prev_commit_topology.block(),
        state.lane_consensus_contexts.block(),
        |_| Ok::<_, Infallible>(()),
    )
    .unwrap();
    let prepared = acquire(decision, &state);
    assert!(matches!(
        state.kura.try_publication_lease(),
        Err(KuraPublicationPreparationError::Busy {
            field: "prune_lock",
            ..
        })
    ));
    assert!(state.state_commit_lock.try_lock().is_none());
    assert!(state.lane_lifecycle_lock.try_lock().is_none());
    assert!(state.state_write_lock.try_lock().is_none());
    assert!(state.block_hashes.inner.try_read().is_none());
    assert_eq!(state.committed_height(), 0);
    assert_eq!(state.state_view_generation(), generation);
    assert!(matches!(
        world_probe.try_prepare_publication(&state.world, |_, _| Ok::<_, Infallible>(())),
        Err((_, WorldPublicationError::Field(_)))
    ));
    assert!(matches!(
        runtime_probe.try_prepare_publication(&state, |_, _| Ok::<_, Infallible>(())),
        Err((_, RuntimePublicationError::Component { .. }))
    ));
    let retry = prepared.abort();
    assert_fences_free_except(&state, "");
    assert!(
        retry
            .journals
            .components
            .world
            .matches_current(&state.world)
    );
    assert!(retry.journals.components.runtime.matches_current(&state));
    drop(acquire(retry, &state));
    assert_eq!(
        crate::snapshot::canonical_state_snapshot_hash(&state).unwrap(),
        before
    );
    assert_eq!(state.kura.blocks_count(), 1);
}

#[test]
fn installation_refusal_precedes_all_fences_and_returns_the_decided_carrier() {
    let (state, decision) = fixture_decision();
    let original = decision.block().encode_wire().unwrap();
    let held = state.state_commit_lock.lock();
    let canonical = state.kura.canonical_publication_lease();
    let (retry, error) =
        match decision.try_prepare_physical(&state, |_, _| Err::<(), _>("capacity")) {
            Ok(_) => panic!("capacity refused"),
            Err(refusal) => refusal,
        };
    assert!(matches!(
        error,
        CarrierPhysicalPreparationError::Admission("capacity")
    ));
    drop(held);
    drop(canonical);
    assert_eq!(retry.block().encode_wire().unwrap(), original);
    drop(acquire(retry, &state));
    assert_fences_free_except(&state, "");
}

#[test]
fn changed_world_predecessor_releases_all_earlier_families_without_rebinding() {
    let (state, decision) = fixture_decision();
    let before = crate::snapshot::canonical_state_snapshot_hash(&state).unwrap();
    let mut parameters = state.world.parameters.block();
    let identical = parameters.get().clone();
    *parameters.get_mut() = identical;
    parameters.commit();
    assert_eq!(
        crate::snapshot::canonical_state_snapshot_hash(&state).unwrap(),
        before
    );
    let (retry, error) = match decision.try_prepare_physical(&state, |_, _| Ok::<_, Infallible>(()))
    {
        Ok(_) => panic!("equal bytes cannot rebind an original owner"),
        Err(refusal) => refusal,
    };
    assert!(matches!(
        error,
        CarrierPhysicalPreparationError::World(WorldPublicationError::Field(
            crate::state::world_journals::publication::FieldRefusal {
                cause: mv::PublicationPreparationError::Changed,
                ..
            }
        ))
    ));
    assert_fences_free_except(&state, "");
    assert!(state.block_hashes.inner.try_write().is_some());
    drop(state.transactions.block());
    drop(state.canonical_runtime.block());
    drop(state.commit_topology.block());
    drop(state.prev_commit_topology.block());
    drop(state.lane_consensus_contexts.block());
    assert!(
        !retry
            .journals
            .components
            .world
            .matches_current(&state.world)
    );
    assert_eq!(state.committed_height(), 0);
}

struct Reservation<'state> {
    state: &'state State,
    name: &'static str,
    released: Arc<Mutex<Vec<&'static str>>>,
}

#[test]
fn actual_validation_overlay_defers_at_hash_before_taking_its_world_writers() {
    let (state, decision) = fixture_decision();
    let before = crate::snapshot::canonical_state_snapshot_hash(&state).unwrap();
    let validating = state.block(decision.block().header());
    let (retry, error) = match decision.try_prepare_physical(&state, |_, _| Ok::<_, Infallible>(()))
    {
        Ok(_) => panic!("the real validation overlay owns the original cut"),
        Err(refusal) => refusal,
    };
    assert!(matches!(
        &error,
        CarrierPhysicalPreparationError::Component {
            field: "block_hashes",
            cause: mv::PublicationPreparationError::Busy(_),
        }
    ));
    assert_fences_free_except(&state, "");
    let mut wait = busy_wait(error).wait_for_release();
    let wakes = Arc::new(WakeCount::default());
    assert!(poll(&mut wait, &wakes).is_pending());
    drop(validating);
    assert!(poll(&mut wait, &wakes).is_ready());
    assert_eq!(wakes.0.load(Ordering::SeqCst), 1);
    drop(acquire(retry, &state));
    assert_eq!(
        crate::snapshot::canonical_state_snapshot_hash(&state).unwrap(),
        before
    );
}

#[test]
fn identical_foreign_state_cannot_replace_the_original_physical_owners() {
    let (state, decision) = fixture_decision();
    let (mut foreign, _, _, _) = fixture();
    assert_eq!(
        crate::snapshot::canonical_state_snapshot_hash(&state).unwrap(),
        crate::snapshot::canonical_state_snapshot_hash(&foreign).unwrap()
    );
    let wire = decision.block().encode_wire().unwrap();
    let calls = AtomicUsize::new(0);
    // Capacity refusal wins even when the target Kura identity is foreign.
    let (decision, error) = match decision.try_prepare_physical(&foreign, |_, _| {
        calls.fetch_add(1, Ordering::SeqCst);
        Err::<(), _>("installation capacity")
    }) {
        Ok(_) => panic!("capacity refused"),
        Err(refusal) => refusal,
    };
    assert!(matches!(
        error,
        CarrierPhysicalPreparationError::Admission("installation capacity")
    ));
    let held = foreign.state_commit_lock.lock();
    let canonical = foreign.kura.canonical_publication_lease();
    let (decision, error) = match decision.try_prepare_physical(&foreign, |_, _| {
        calls.fetch_add(1, Ordering::SeqCst);
        Ok::<_, Infallible>(())
    }) {
        Ok(_) => panic!("equal storage bytes cannot replace original Kura"),
        Err(refusal) => refusal,
    };
    assert!(matches!(
        error,
        CarrierPhysicalPreparationError::ForeignKura
    ));
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    assert_eq!(decision.block().encode_wire().unwrap(), wire);
    drop(canonical);
    drop(held);
    assert_fences_free_except(&foreign, "");
    drop(
        foreign
            .kura
            .try_publication_lease()
            .expect("foreign Kura remains free"),
    );
    // Keep the original independent State-owner regression: using the original
    // Kura still cannot rebind byte-identical hash/MV owners in another State.
    foreign.kura = Arc::clone(&state.kura);
    let (retry, error) =
        match decision.try_prepare_physical(&foreign, |_, _| Ok::<_, Infallible>(())) {
            Ok(_) => panic!("equal State bytes cannot replace original journals"),
            Err(refusal) => refusal,
        };
    assert!(matches!(
        error,
        CarrierPhysicalPreparationError::Component {
            field: "block_hashes",
            cause: mv::PublicationPreparationError::Changed,
        }
    ));
    assert_fences_free_except(&foreign, "");
    assert_eq!(retry.block().encode_wire().unwrap(), wire);
    drop(acquire(retry, &state));
    assert_eq!(state.committed_height(), 0);
    assert_eq!(foreign.committed_height(), 0);
}
impl Drop for Reservation<'_> {
    fn drop(&mut self) {
        assert_fences_free_except(self.state, "");
        drop(
            self.state
                .kura
                .try_publication_lease()
                .expect("Kura releases before reservations"),
        );
        assert!(self.state.block_hashes.inner.try_write().is_some());
        drop(self.state.transactions.block());
        drop(self.state.world.accounts.block());
        drop(self.state.lane_consensus_contexts.block());
        self.released.lock().unwrap().push(self.name);
    }
}

#[test]
fn all_reservations_outlive_component_writers_and_state_fences_on_drop_and_abort() {
    for abort in [false, true] {
        let (state, proposal, topology, context) = fixture();
        let released = Arc::new(Mutex::new(Vec::new()));
        let guard = |name| Reservation {
            state: &state,
            name,
            released: Arc::clone(&released),
        };
        let decision = decided(
            &state,
            proposal,
            &topology,
            &context,
            guard("capture"),
            guard("binding"),
        );
        let prepared = decision
            .try_prepare_physical(&state, |_, _| Ok::<_, Infallible>(guard("installation")))
            .unwrap_or_else(|(_, error)| panic!("physical preparation: {error:?}"));
        assert!(released.lock().unwrap().is_empty());
        if abort {
            let retry = prepared.abort();
            assert_eq!(*released.lock().unwrap(), ["installation"]);
            drop(retry);
            assert_eq!(
                *released.lock().unwrap(),
                ["installation", "capture", "binding"]
            );
        } else {
            drop(prepared);
            assert_eq!(
                *released.lock().unwrap(),
                ["capture", "binding", "installation"]
            );
        }
    }
}

#[test]
fn original_kura_contention_returns_exact_decided_carrier_and_release_driven_retry() {
    let (state, mut decision) = fixture_decision();
    let before = crate::snapshot::canonical_state_snapshot_hash(&state).unwrap();
    let generation = state.state_view_generation();
    let wire = decision.block().encode_wire().unwrap();
    let hashes = decision
        .journals
        .components
        .block_hashes
        .as_slice()
        .as_ptr();
    let membership = std::ptr::from_ref(
        decision
            .journals
            .components
            .transactions
            .staged_membership()
            .1,
    );
    for queue in [false, true] {
        let held: Box<dyn Held + '_> = if queue {
            Box::new(
                state
                    .kura
                    .try_queue_plan_publication_at_height(1)
                    .unwrap()
                    .unwrap(),
            )
        } else {
            Box::new(state.kura.canonical_publication_lease())
        };
        // Kura refusal must win without entering any State fence.
        let state_held = state.state_commit_lock.lock();
        let (retry, error) =
            match decision.try_prepare_physical(&state, |_, _| Ok::<_, Infallible>(())) {
                Ok(_) => panic!("original canonical owner is held"),
                Err(refusal) => refusal,
            };
        assert!(matches!(
            &error,
            CarrierPhysicalPreparationError::Kura(KuraPublicationPreparationError::Busy {
                field: "canonical_chain_lock",
                ..
            })
        ));
        assert_fences_free_except(&state, "state_commit_lock");
        assert!(state.block_hashes.inner.try_write().is_some());
        assert_eq!(retry.block().encode_wire().unwrap(), wire);
        assert_eq!(
            retry.journals.components.block_hashes.as_slice().as_ptr(),
            hashes
        );
        assert_eq!(
            std::ptr::from_ref(retry.journals.components.transactions.staged_membership().1),
            membership
        );
        let mut wait = busy_wait(error).wait_for_release();
        let wakes = Arc::new(WakeCount::default());
        assert!(poll(&mut wait, &wakes).is_pending());
        drop(state_held);
        assert!(poll(&mut wait, &wakes).is_pending());
        assert_eq!(wakes.0.load(Ordering::SeqCst), 0);
        drop(held);
        assert_eq!(wakes.0.load(Ordering::SeqCst), 1);
        assert!(poll(&mut wait, &wakes).is_ready());
        decision = acquire(retry, &state).abort();
    }
    assert_eq!(state.state_view_generation(), generation);
    assert_eq!(
        crate::snapshot::canonical_state_snapshot_hash(&state).unwrap(),
        before
    );
    assert_eq!(state.kura.exact_durable_blocks_count().unwrap(), 1);
}

#[test]
fn original_kura_storage_failure_returns_carrier_and_releases_all_acquired_owners() {
    let (state, decision) = fixture_decision();
    let wire = decision.block().encode_wire().unwrap();
    let hashes = decision
        .journals
        .components
        .block_hashes
        .as_slice()
        .as_ptr();
    let membership = std::ptr::from_ref(
        decision
            .journals
            .components
            .transactions
            .staged_membership()
            .1,
    );
    let generation = state.state_view_generation();
    state.kura.poison_canonical_storage_for_tests();
    let state_held = state.state_commit_lock.lock();
    let (retry, error) = match decision.try_prepare_physical(&state, |_, _| Ok::<_, Infallible>(()))
    {
        Ok(_) => panic!("poison requires actual storage repair"),
        Err(refusal) => refusal,
    };
    assert!(matches!(
        error,
        CarrierPhysicalPreparationError::Kura(KuraPublicationPreparationError::Storage(
            crate::kura::Error::CanonicalStoragePoisoned
        ))
    ));
    drop(state_held);
    assert_fences_free_except(&state, "");
    assert!(state.block_hashes.inner.try_write().is_some());
    // The full lease remains a typed storage error, never Busy from a leaked guard.
    assert!(matches!(
        state.kura.try_publication_lease(),
        Err(KuraPublicationPreparationError::Storage(
            crate::kura::Error::CanonicalStoragePoisoned
        ))
    ));
    drop(state.kura.canonical_publication_lease());
    assert_eq!(retry.block().encode_wire().unwrap(), wire);
    assert_eq!(
        retry.journals.components.block_hashes.as_slice().as_ptr(),
        hashes
    );
    assert_eq!(
        std::ptr::from_ref(retry.journals.components.transactions.staged_membership().1),
        membership
    );
    drop(retry);
    assert_eq!(state.committed_height(), 0);
    assert_eq!(state.state_view_generation(), generation);
}

#[test]
fn checkpoint_storage_refusal_precedes_state_and_retains_exact_originals() {
    let (state, mut decision) = fixture_decision();
    let before = crate::snapshot::canonical_state_snapshot_hash(&state).unwrap();
    let generation = state.state_view_generation();
    let wire = decision.block().encode_wire().unwrap();
    let hashes = decision
        .journals
        .components
        .block_hashes
        .as_slice()
        .as_ptr();
    let membership = std::ptr::from_ref(
        decision
            .journals
            .components
            .transactions
            .staged_membership()
            .1,
    );
    // Removal and then replacement with identical bytes must both refuse the
    // retained original receipt, before touching even a held first State fence.
    state
        .kura
        .remove_wsv_checkpoint_without_binding_for_tests(1)
        .unwrap();
    for replaced in [false, true] {
        if replaced {
            let finality = state
                .kura
                .store_v2_finality_artifact(decision.finality())
                .unwrap();
            let replacement = state
                .kura
                .persist_wsv_checkpoint_for_v2_commit(&finality, decision.journals.checkpoint)
                .unwrap();
            state
                .kura
                .reauthenticate_wsv_checkpoint_receipt(
                    &replacement,
                    decision.finality(),
                    decision.journals.checkpoint,
                )
                .unwrap();
        }
        let held = state.state_commit_lock.lock();
        let (retry, error) =
            match decision.try_prepare_physical(&state, |_, _| Ok::<_, Infallible>(())) {
                Ok(_) => panic!("missing or replaced checkpoint is not the retained original"),
                Err(refusal) => refusal,
            };
        assert!(matches!(
            error,
            CarrierPhysicalPreparationError::Checkpoint(_)
        ));
        assert_fences_free_except(&state, "state_commit_lock");
        drop(
            state
                .kura
                .try_publication_lease()
                .expect("all Kura fences released"),
        );
        assert!(state.block_hashes.inner.try_write().is_some());
        assert_eq!(retry.block().encode_wire().unwrap(), wire);
        assert_eq!(
            retry.journals.components.block_hashes.as_slice().as_ptr(),
            hashes
        );
        assert_eq!(
            std::ptr::from_ref(retry.journals.components.transactions.staged_membership().1),
            membership
        );
        drop(held);
        assert_fences_free_except(&state, "");
        decision = retry;
    }
    assert_eq!(state.committed_height(), 0);
    assert_eq!(state.state_view_generation(), generation);
    assert_eq!(
        crate::snapshot::canonical_state_snapshot_hash(&state).unwrap(),
        before
    );
    assert_eq!(state.kura.exact_durable_blocks_count().unwrap(), 1);
}

#[test]
fn exact_checkpoint_retry_preserves_receipt_across_physical_abort() {
    let (state, decision) = fixture_decision();
    let decision = acquire(decision, &state).abort();
    let finality = state
        .kura
        .store_v2_finality_artifact(decision.finality())
        .unwrap();
    let repeated = state
        .kura
        .persist_wsv_checkpoint_for_v2_commit(&finality, decision.journals.checkpoint)
        .unwrap();
    // The original receipt still belongs to the original persisted object.
    // Repeating persistence must not silently invalidate a queued publication.
    drop(repeated);
    let decision = acquire(decision, &state).abort();
    state
        .kura
        .reauthenticate_wsv_checkpoint_receipt(
            &decision.checkpoint,
            decision.finality(),
            decision.journals.checkpoint,
        )
        .unwrap();
    assert_eq!(state.committed_height(), 0);
    assert_eq!(state.kura.exact_durable_blocks_count().unwrap(), 1);
}

#[test]
fn attached_foreign_checkpoint_never_grants_state_acquisition() {
    let (state, mut decision) = fixture_decision();
    let (other, mut foreign) = fixture_decision();
    std::mem::swap(&mut decision.checkpoint, &mut foreign.checkpoint);
    // Both blocks and finality have identical bytes. Only the original Kura
    // object and its exact durable receipt may join these captured journals.
    assert_eq!(
        decision.block().encode_wire().unwrap(),
        foreign.block().encode_wire().unwrap()
    );
    let held = state.state_commit_lock.lock();
    let (decision, error) =
        match decision.try_prepare_physical(&state, |_, _| Ok::<_, Infallible>(())) {
            Ok(_) => panic!("attachment is custody, not authority"),
            Err(refusal) => refusal,
        };
    assert!(matches!(
        error,
        CarrierPhysicalPreparationError::Checkpoint(_)
    ));
    drop(
        state
            .kura
            .try_publication_lease()
            .expect("original Kura released"),
    );
    drop(held);
    assert_fences_free_except(&state, "");
    assert_eq!(state.committed_height(), 0);
    assert_eq!(other.committed_height(), 0);
    drop(decision);
    drop(foreign);
}
