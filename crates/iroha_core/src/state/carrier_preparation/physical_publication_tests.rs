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
    bind_and_persist(state, context, journals, binding)
}

fn bind_and_persist<A, B>(
    state: &State,
    context: &HeightContext,
    journals: super::super::super::PreparedCarrierJournals<A>,
    binding: B,
) -> CheckpointDecision<A, B> {
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

type ProviderArchive = crate::query::provider_ingest_finalized::ProviderIngestFinalizedArchiveV1;
type ReputationArchive = crate::query::reputation_finalized::ReputationFinalizedArchive;

fn fixture_archive_decision() -> (
    tempfile::TempDir,
    Box<State>,
    CheckpointDecision<(), ()>,
    Arc<ProviderArchive>,
    Arc<ReputationArchive>,
) {
    use crate::query::{
        provider_ingest_finalized::{
            ProviderIngestFinalizedArchiveBoundsV1, ProviderIngestFinalizedArchiveKeyV1,
        },
        reputation_finalized::{ReputationFinalizedArchiveBounds, ReputationFinalizedArchiveKeyV1},
    };
    let (state, proposal, topology, context) = super::super::super::tests::archive_fixture();
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().canonicalize().unwrap();
    let provider = Arc::new(
        ProviderArchive::try_open(
            root.join("provider"),
            ProviderIngestFinalizedArchiveBoundsV1::try_new(1 << 20, 16, 16 << 20, 16, 16, 256, 16)
                .unwrap(),
        )
        .unwrap(),
    );
    let reputation = Arc::new(
        ReputationArchive::try_open(
            root.join("reputation"),
            ReputationFinalizedArchiveBounds::try_new(1 << 20, 16, 16 << 20).unwrap(),
        )
        .unwrap(),
    );
    let provider_candidate = provider
        .try_reserve_candidate(
            ProviderIngestFinalizedArchiveKeyV1::try_new(
                context.network_id,
                proposal.header().height().get(),
                *proposal.hash().as_ref(),
                proposal.header().creation_time_ms,
            )
            .unwrap(),
            &state.kura,
        )
        .unwrap();
    let reputation_candidate = reputation
        .try_reserve_candidate(
            ReputationFinalizedArchiveKeyV1::try_new(
                context.network_id,
                proposal.header().height().get(),
                *proposal.hash().as_ref(),
            )
            .unwrap(),
            proposal.header().creation_time_ms,
            &state.kura,
        )
        .unwrap();
    let journals = prepare(&state, proposal, &topology, &context)
        .unwrap_or_else(|(_, error)| panic!("real archive execution: {error}"))
        .prepare_journals(Some(provider_candidate), Some(reputation_candidate), |_| {
            Ok::<_, Infallible>(())
        })
        .unwrap();
    let decision = bind_and_persist(&state, &context, journals, ());
    (directory, state, decision, provider, reputation)
}

#[test]
fn joint_publication_persists_both_original_archives_without_state_effects_or_relocking() {
    let (_directory, state, mut decision, provider, reputation) = fixture_archive_decision();
    let before = crate::snapshot::canonical_state_snapshot_hash(&state).unwrap();
    let generation = state.state_view_generation();
    let wire = decision.block().encode_wire().unwrap();
    let hashes = decision
        .journals
        .components
        .block_hashes
        .as_slice()
        .as_ptr();
    for _ in 0..2 {
        let physical = acquire(decision, &state);
        assert!(!provider.is_empty().unwrap());
        assert!(!reputation.is_empty().unwrap());
        decision = physical.abort();
        assert_eq!(decision.block().encode_wire().unwrap(), wire);
        assert_eq!(
            decision
                .journals
                .components
                .block_hashes
                .as_slice()
                .as_ptr(),
            hashes
        );
        assert!(decision.journals.provider_capture.is_some());
        assert!(decision.journals.reputation_capture.is_some());
        assert_fences_free_except(&state, "");
        drop(state.kura.try_publication_lease().unwrap());
    }
    assert_eq!(state.committed_height(), 0);
    assert_eq!(state.state_view_generation(), generation);
    assert_eq!(
        crate::snapshot::canonical_state_snapshot_hash(&state).unwrap(),
        before
    );
}

#[test]
fn foreign_archive_refusal_precedes_state_acquisition_and_returns_complete_retry() {
    let (_directory, state, mut decision, provider, reputation) = fixture_archive_decision();
    let (_foreign_directory, _foreign_state, mut foreign, foreign_provider, foreign_reputation) =
        fixture_archive_decision();
    let before = crate::snapshot::canonical_state_snapshot_hash(&state).unwrap();
    let wire = decision.block().encode_wire().unwrap();
    let hashes = decision
        .journals
        .components
        .block_hashes
        .as_slice()
        .as_ptr();
    for provider_case in [true, false] {
        let provider_was_empty = provider.is_empty().unwrap();
        let reputation_was_empty = reputation.is_empty().unwrap();
        // Only adversarial tests can exchange private capture fields. The
        // actual original Kura seal must still reject equal projection bytes.
        if provider_case {
            std::mem::swap(
                &mut decision.journals.provider_capture,
                &mut foreign.journals.provider_capture,
            );
        } else {
            std::mem::swap(
                &mut decision.journals.reputation_capture,
                &mut foreign.journals.reputation_capture,
            );
        }
        let held = state.state_commit_lock.lock();
        let (mut retry, error) =
            match decision.try_prepare_physical(&state, |_, _| Ok::<_, Infallible>(())) {
                Ok(_) => panic!("foreign archive may not substitute for the captured original"),
                Err(refusal) => refusal,
            };
        if provider_case {
            assert!(matches!(
                error,
                CarrierPhysicalPreparationError::Provider(_)
            ));
            std::mem::swap(
                &mut retry.journals.provider_capture,
                &mut foreign.journals.provider_capture,
            );
        } else {
            assert!(matches!(
                error,
                CarrierPhysicalPreparationError::Reputation(_)
            ));
            std::mem::swap(
                &mut retry.journals.reputation_capture,
                &mut foreign.journals.reputation_capture,
            );
        }
        assert_fences_free_except(&state, "state_commit_lock");
        drop(state.kura.try_publication_lease().unwrap());
        assert!(state.block_hashes.inner.try_write().is_some());
        assert_eq!(retry.block().encode_wire().unwrap(), wire);
        assert_eq!(
            retry.journals.components.block_hashes.as_slice().as_ptr(),
            hashes
        );
        assert_eq!(provider.is_empty().unwrap(), provider_was_empty);
        assert_eq!(reputation.is_empty().unwrap(), reputation_was_empty);
        assert!(foreign_provider.is_empty().unwrap());
        assert!(foreign_reputation.is_empty().unwrap());
        drop(held);
        decision = acquire(retry, &state).abort();
    }
    assert_eq!(state.committed_height(), 0);
    assert_eq!(
        crate::snapshot::canonical_state_snapshot_hash(&state).unwrap(),
        before
    );
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

#[test]
fn source_substitution_refuses_before_state_acquisition_and_retains_original_retry() {
    let (state, mut decision) = fixture_decision();
    let (foreign_state, proposal, topology, context) =
        crate::state::carrier_preparation::tests::fixture_with_instructions(&[
            iroha_data_model::isi::Log::new(
                iroha_data_model::level::Level::INFO,
                "distinct retained source".to_owned(),
            )
            .into(),
        ]);
    let mut foreign = prepare(&foreign_state, proposal, &topology, &context)
        .unwrap_or_else(|(_, error)| panic!("foreign real execution: {error}"))
        .prepare_journals(None, None, |_| Ok::<_, Infallible>(()))
        .unwrap();
    let before = crate::snapshot::canonical_state_snapshot_hash(&state).unwrap();
    let generation = state.state_view_generation();
    let wire = decision.block().encode_wire().unwrap();
    let inventory = Arc::clone(decision.journals.source_prefix.inventory());
    let witness = decision.journals.source_prefix.witness().writes.as_ptr();
    let sources = decision.journals.source_prefix.sources().entries().as_ptr();
    let hashes = decision
        .journals
        .components
        .block_hashes
        .as_slice()
        .as_ptr();
    std::mem::swap(
        &mut decision.journals.source_prefix,
        &mut foreign.source_prefix,
    );
    let held = state.state_commit_lock.lock();
    let (mut retry, error) =
        match decision.try_prepare_physical(&state, |_, _| Ok::<_, Infallible>(())) {
            Ok(_) => panic!("foreign executed prefix must refuse before State writers"),
            Err(refusal) => refusal,
        };
    assert!(
        matches!(error, CarrierPhysicalPreparationError::Source(_)),
        "wrong refusal: {error:?}"
    );
    assert!(
        !state
            .kura
            .store_root()
            .join("blocks/canonical/kagemusha_v1_finality")
            .join(format!("{:020}.norito", retry.finality().height))
            .exists(),
        "foreign execution custody must refuse before witness publication"
    );
    assert_fences_free_except(&state, "state_commit_lock");
    drop(
        state
            .kura
            .try_publication_lease()
            .expect("source refusal released Kura"),
    );
    assert_eq!(retry.block().encode_wire().unwrap(), wire);
    assert_eq!(
        retry.journals.components.block_hashes.as_slice().as_ptr(),
        hashes
    );
    std::mem::swap(
        &mut retry.journals.source_prefix,
        &mut foreign.source_prefix,
    );
    assert!(Arc::ptr_eq(
        retry.journals.source_prefix.inventory(),
        &inventory
    ));
    assert_eq!(
        retry.journals.source_prefix.witness().writes.as_ptr(),
        witness
    );
    assert_eq!(
        retry.journals.source_prefix.sources().entries().as_ptr(),
        sources
    );
    drop(held);
    for _ in 0..2 {
        retry = acquire(retry, &state).abort();
        assert!(Arc::ptr_eq(
            retry.journals.source_prefix.inventory(),
            &inventory
        ));
        assert_eq!(
            retry.journals.source_prefix.witness().writes.as_ptr(),
            witness
        );
        assert_eq!(
            retry.journals.source_prefix.sources().entries().as_ptr(),
            sources
        );
        assert_fences_free_except(&state, "");
        drop(state.kura.try_publication_lease().unwrap());
    }
    assert_eq!(state.state_view_generation(), generation);
    assert_eq!(
        crate::snapshot::canonical_state_snapshot_hash(&state).unwrap(),
        before
    );
}

#[test]
fn changed_carrier_wire_refuses_source_join_and_restored_owner_reauthenticates() {
    let (state, mut decision) = fixture_decision();
    let before = crate::snapshot::canonical_state_snapshot_hash(&state).unwrap();
    let wire = decision.block().encode_wire().unwrap();
    let original_signatures = decision.block().signatures().cloned().collect();
    let inventory = Arc::clone(decision.journals.source_prefix.inventory());
    let witness = decision.journals.source_prefix.witness().writes.as_ptr();
    let extra =
        iroha_crypto::KeyPair::try_from_seed(vec![0xFD; 32], iroha_crypto::Algorithm::BlsNormal)
            .unwrap();
    decision
        .journals
        .valid
        .as_mut()
        .sign(extra.private_key(), 99);
    let substituted_wire = decision.block().encode_wire().unwrap();
    assert_ne!(substituted_wire, wire);
    let held = state.state_commit_lock.lock();
    let (mut retry, error) =
        match decision.try_prepare_physical(&state, |_, _| Ok::<_, Infallible>(())) {
            Ok(_) => panic!("changed retained wire must refuse source authentication"),
            Err(refusal) => refusal,
        };
    assert!(
        matches!(error, CarrierPhysicalPreparationError::Source(_)),
        "wrong refusal: {error:?}"
    );
    assert_eq!(retry.block().encode_wire().unwrap(), substituted_wire);
    assert!(Arc::ptr_eq(
        retry.journals.source_prefix.inventory(),
        &inventory
    ));
    assert_eq!(
        retry.journals.source_prefix.witness().writes.as_ptr(),
        witness
    );
    assert_fences_free_except(&state, "state_commit_lock");
    drop(state.kura.try_publication_lease().unwrap());
    retry
        .journals
        .valid
        .as_mut()
        .replace_signatures(original_signatures)
        .unwrap();
    drop(held);
    let retry = acquire(retry, &state).abort();
    assert_eq!(retry.block().encode_wire().unwrap(), wire);
    assert!(Arc::ptr_eq(
        retry.journals.source_prefix.inventory(),
        &inventory
    ));
    assert_eq!(
        retry.journals.source_prefix.witness().writes.as_ptr(),
        witness
    );
    assert_eq!(
        crate::snapshot::canonical_state_snapshot_hash(&state).unwrap(),
        before
    );
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
    assert!(std::ptr::eq(prepared.target, &*state));
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
fn geometry_refusal_returns_original_decision_and_releases_every_physical_writer() {
    let (state, mut decision) = fixture_decision();
    let (foreign, _, _, _) = fixture();
    let foreign_geometry = foreign
        .merge_preexecution_block(decision.block().header())
        .prepare_carrier_geometry()
        .unwrap();
    // Only this adversarial test can substitute the private geometry owner.
    // The carrier must still use its captured target and original held lease.
    let original_geometry = std::mem::replace(&mut decision.journals.geometry, foreign_geometry);
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
    let physical = acquire(decision, &state);
    let (mut retry, _) = match physical.try_resume_geometry() {
        Ok(_) => panic!("foreign geometry must refuse before effects"),
        Err(refusal) => refusal,
    };
    assert_fences_free_except(&state, "");
    drop(state.kura.try_publication_lease().unwrap());
    assert!(state.block_hashes.inner.try_write().is_some());
    assert!(
        retry
            .journals
            .components
            .world
            .matches_current(&state.world)
    );
    assert!(retry.journals.components.runtime.matches_current(&state));
    assert_eq!(retry.block().encode_wire().unwrap(), wire);
    assert_eq!(
        retry.journals.components.block_hashes.as_slice().as_ptr(),
        hashes
    );
    assert_eq!(
        std::ptr::from_ref(retry.journals.components.transactions.staged_membership().1),
        membership
    );
    retry.journals.geometry = original_geometry;
    let physical = acquire(retry, &state)
        .try_resume_geometry()
        .unwrap_or_else(|(_, error)| panic!("exact geometry owner retry: {error}"));
    assert!(std::ptr::eq(physical.target, &*state));
    drop(physical.abort());
    assert_eq!(state.state_view_generation(), generation);
    assert_eq!(
        crate::snapshot::canonical_state_snapshot_hash(&state).unwrap(),
        before
    );
}

#[test]
fn geometry_backend_contention_releases_writers_and_waits_for_actual_backend_release() {
    let (state, decision) = fixture_decision();
    let before = crate::snapshot::canonical_state_snapshot_hash(&state).unwrap();
    let wire = decision.block().encode_wire().unwrap();
    let hashes = decision
        .journals
        .components
        .block_hashes
        .as_slice()
        .as_ptr();
    let held = state.tiered_backend.lock();
    let (retry, error) = match acquire(decision, &state).try_resume_geometry() {
        Ok(_) => panic!("the actual backend owner must release first"),
        Err(refusal) => refusal,
    };
    let crate::state::LaneLifecycleError::PublicationBusy { field, wait } = error else {
        panic!("expected the backend's actual release observation");
    };
    assert_eq!(field, "tiered_backend");
    assert_fences_free_except(&state, "");
    drop(state.kura.try_publication_lease().unwrap());
    assert_eq!(retry.block().encode_wire().unwrap(), wire);
    assert_eq!(
        retry.journals.components.block_hashes.as_slice().as_ptr(),
        hashes
    );
    let wakes = Arc::new(WakeCount::default());
    let mut wait = wait.wait_for_release();
    assert!(poll(&mut wait, &wakes).is_pending());
    drop(held);
    assert_eq!(wakes.0.load(Ordering::SeqCst), 1);
    assert!(poll(&mut wait, &wakes).is_ready());
    let physical = acquire(retry, &state)
        .try_resume_geometry()
        .unwrap_or_else(|(_, error)| panic!("original owner after backend release: {error}"));
    drop(physical.abort());
    assert_eq!(
        crate::snapshot::canonical_state_snapshot_hash(&state).unwrap(),
        before
    );
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
