//! Joint physical ownership using actual executed four-validator genesis decisions.

use super::super::tests::{signed_finality, subject};
use super::*;
use crate::queue::Queue;
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

struct PhaseReservation(Arc<AtomicUsize>);

impl Drop for PhaseReservation {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

type RetainedPhase = crate::state::RetainedCarrier<PhaseReservation, PhaseReservation>;

struct ActualPhaseValidator {
    state: Arc<State>,
    queue: Arc<Queue>,
    topology: Topology,
    calls: Arc<AtomicUsize>,
    releases: Arc<AtomicUsize>,
    provider: Option<crate::query::provider_ingest_finalized::ProviderCandidateCapture>,
    reputation: Option<crate::query::reputation_finalized::ReputationCandidateCapture>,
    wake: Waker,
}

fn phase_queue() -> Arc<Queue> {
    let (events, _receiver) = tokio::sync::broadcast::channel(32);
    Arc::new(Queue::from_config(
        iroha_config::parameters::actual::Queue::default(),
        events,
    ))
}

impl crate::sumeragi::v2_apply::validation_custody::CarrierValidator for ActualPhaseValidator {
    type Owner = RetainedPhase;
    type Error = String;

    fn prepare(
        &mut self,
        context: &HeightContext,
        body: &SignedBlock,
    ) -> Result<Self::Owner, Self::Error> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let prepared = prepare(&self.state, body.clone(), &self.topology, context)
            .map_err(|(_, error)| error.to_string())?;
        match prepared.prepare_journals(self.provider.take(), self.reputation.take(), |_| {
            Ok::<_, Infallible>(PhaseReservation(Arc::clone(&self.releases)))
        }) {
            Ok(journals) => Ok(RetainedPhase::Validated(journals)),
            Err(super::super::super::CarrierJournalPreparationError::ArchivePreparation {
                carrier,
                ..
            }) => Ok(RetainedPhase::Capturing(carrier)),
            Err(error) => Err(error.to_string()),
        }
    }

    fn resume(
        &mut self,
        owner: Self::Owner,
    ) -> Result<
        Self::Owner,
        (
            Self::Owner,
            crate::sumeragi::v2_body_store::LocalValidationRefusal,
        ),
    > {
        use crate::query::{
            provider_ingest_finalized::ProviderIngestFinalizedArchiveErrorV1,
            reputation_finalized::ReputationFinalizedArchiveError,
        };
        use crate::sumeragi::v2_body_store::{BodyValidationBusy, LocalValidationRefusal};
        owner.resume_capture().map_err(|(owner, error)| {
            let dependency = match &error {
                super::super::super::CarrierArchivePreparationError::Provider(error) => {
                    match error.as_ref() {
                        ProviderIngestFinalizedArchiveErrorV1::IndexBusy { wait } => {
                            Some(("provider archive index", wait))
                        }
                        _ => None,
                    }
                }
                super::super::super::CarrierArchivePreparationError::Reputation(error) => {
                    match error.as_ref() {
                        ReputationFinalizedArchiveError::IndexBusy { wait } => {
                            Some(("reputation archive index", wait))
                        }
                        _ => None,
                    }
                }
            };
            let refusal = match dependency {
                Some((resource, wait)) => LocalValidationRefusal::PhysicalBusy(
                    BodyValidationBusy::new(resource, wait.clone(), self.wake.clone()),
                ),
                None => LocalValidationRefusal::RecoveryRequired(error.to_string()),
            };
            (owner, refusal)
        })
    }
}

fn phase_allocations(phase: &RetainedPhase) -> [*const (); 6] {
    fn allocations<B>(
        journals: &super::super::super::PreparedCarrierJournals<PhaseReservation, B>,
    ) -> [*const (); 6] {
        [
            journals
                .components
                .block_hashes
                .get(0)
                .map_or(std::ptr::null(), std::ptr::from_ref)
                .cast(),
            std::ptr::from_ref(journals.components.transactions.staged_membership().1).cast(),
            journals.source_prefix.witness().writes.as_ptr().cast(),
            journals.source_prefix.sources().entries().as_ptr().cast(),
            Arc::as_ptr(journals.source_prefix.inventory()).cast(),
            journals.publication_events.as_ptr().cast(),
        ]
    }
    match phase {
        RetainedPhase::Capturing(capture) => allocations(&capture.journals),
        RetainedPhase::Validated(journals) => allocations(journals),
        RetainedPhase::Decided(decision) => allocations(&decision.journals),
        RetainedPhase::Checkpointed(decision) => allocations(&decision.journals),
    }
}

// The foreign fixture's construction scratch is released before the retained
// service begins; the returned Arc remains a strong identity witness throughout.
#[inline(never)]
fn phase_foreign_state(state: &Arc<State>) -> Arc<State> {
    let (mut foreign, _, _, _) = fixture();
    foreign.kura = Arc::clone(&state.kura);
    foreign.into()
}

// Complete the Queue-only selection attempt before later source decoding. Its
// affine selection/refusal temporaries do not belong on the publication stack.
#[inline(never)]
fn assert_original_queue_refusal(
    service: &mut crate::sumeragi::v2_apply::validation_custody::RetainedBodyValidationService<
        ActualPhaseValidator,
    >,
    receipt: &crate::sumeragi::v2_body_store::ValidatedBodyReceipt,
    state: &Arc<State>,
    queue: &Arc<Queue>,
    foreign: &Arc<State>,
    allocations: [*const (); 6],
) {
    let decoy_queue = phase_queue();
    assert!(!Arc::ptr_eq(queue, &decoy_queue));
    let generation = state.state_view_generation();
    let before = crate::snapshot::canonical_state_snapshot_hash(state).unwrap();
    // A lockable empty Queue cannot replace the original producer's dependency.
    // This nonretiring carrier does not gain retirement authority from an outer
    // observer: the terminal publisher's retirement refusal remains unchanged.
    let queue_held = queue.lock_lane_retirement_observer();
    let refused = service
        .select(receipt)
        .unwrap()
        .try_consume(|producer, phase| {
            assert!(Arc::ptr_eq(&producer.state, state));
            assert!(Arc::ptr_eq(&producer.queue, queue));
            assert!(!Arc::ptr_eq(&producer.state, foreign));
            assert!(!Arc::ptr_eq(&producer.queue, &decoy_queue));
            assert!(matches!(phase, RetainedPhase::Checkpointed(_)));
            match producer.queue.try_lock_lane_retirement_observer() {
                Ok(_) => panic!("the original Queue must defer this selected owner"),
                Err(wait) => Err::<(), _>((phase, wait)),
            }
        });
    let mut queue_wait = refused.unwrap_err().wait_for_release();
    let queue_wakes = Arc::new(WakeCount::default());
    assert!(poll(&mut queue_wait, &queue_wakes).is_pending());
    drop(decoy_queue.try_lock_lane_retirement_observer().unwrap());
    assert_eq!(queue_wakes.0.load(Ordering::SeqCst), 0);
    assert!(poll(&mut queue_wait, &queue_wakes).is_pending());
    let restored = service.owner_for_test(receipt.durable().subject()).unwrap();
    assert!(matches!(restored, RetainedPhase::Checkpointed(_)));
    assert_eq!(phase_allocations(restored), allocations);
    assert_fences_free_except(state, "");
    assert_eq!(state.state_view_generation(), generation);
    assert_eq!(
        crate::snapshot::canonical_state_snapshot_hash(state).unwrap(),
        before
    );
    drop(queue_held);
    assert_eq!(queue_wakes.0.load(Ordering::SeqCst), 1);
    assert!(poll(&mut queue_wait, &queue_wakes).is_ready());
}

#[test]
fn retained_execution_phases_survive_marker_reproposal_and_publication_refusals() {
    use crate::sumeragi::{
        v2_apply::validation_custody::{CarrierCustodyError, RetainedValidationOwner},
        v2_body_store::{
            BlockSignaturePolicy, V2BodyStore, V2BodyStoreError, fail_next_marker_directory_sync,
            fail_next_marker_file_sync,
        },
        v2_chunks::encode_payload,
    };
    use iroha_data_model::block::consensus_v2 as wire;

    let (state, proposal, topology, context) = fixture();
    let state: Arc<State> = state.into();
    let queue = phase_queue();
    let foreign = phase_foreign_state(&state);
    assert!(!Arc::ptr_eq(&state, &foreign));
    let generation = state.state_view_generation();
    let before = crate::snapshot::canonical_state_snapshot_hash(&state).unwrap();
    assert_eq!(
        crate::snapshot::canonical_state_snapshot_hash(&foreign).unwrap(),
        before
    );
    let calls = Arc::new(AtomicUsize::new(0));
    let releases = Arc::new(AtomicUsize::new(0));
    let directory = tempfile::tempdir().unwrap();
    let mut store = V2BodyStore::open_with_policy(
        directory.path(),
        context.clone(),
        BlockSignaturePolicy::GenesisAuthority(
            iroha_test_samples::SAMPLE_GENESIS_ACCOUNT_KEYPAIR
                .public_key()
                .clone(),
        ),
    )
    .unwrap();
    let bytes = proposal
        .canonical_resultless_proposal()
        .encode_wire()
        .unwrap();
    let round = wire::ConsensusRound {
        context_id: context.id(),
        height: context.height,
        view: 0,
    };
    let manifest = encode_payload(&context, round, subject(&proposal), &bytes)
        .unwrap()
        .manifest()
        .clone();
    let durable = store.store(manifest, bytes.clone()).unwrap();
    let descriptor_budget = mv::allocation::AllocationBudget::new(
        store
            .retained_validation_descriptor_bytes::<ActualPhaseValidator>()
            .unwrap(),
    );
    let mut service = store
        .retained_validation_service(
            ActualPhaseValidator {
                state: Arc::clone(&state),
                queue: Arc::clone(&queue),
                topology,
                calls: Arc::clone(&calls),
                releases: Arc::clone(&releases),
                provider: None,
                reputation: None,
                wake: Waker::noop().clone(),
            },
            &descriptor_budget,
        )
        .unwrap();
    fail_next_marker_file_sync();
    assert!(matches!(
        store.execute_retained_durable_validation(
            durable.clone(),
            durable.manifest_hash(),
            &mut service
        ),
        Err(V2BodyStoreError::Io { .. })
    ));
    let original = service.owner_for_test(durable.subject()).unwrap();
    let allocations = phase_allocations(original);
    let commitment = original.ready_commitment().unwrap();
    let mut wrong_context = context.clone();
    wrong_context.height += 1;
    assert!(matches!(original, RetainedPhase::Validated(_)));
    assert!(original.matches_candidate(&context, &proposal));
    assert!(!original.matches_candidate(&wrong_context, &proposal));
    assert_eq!(service.marker_counts_for_test(), (1, 0));
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(releases.load(Ordering::SeqCst), 0);
    let receipt = store
        .execute_retained_durable_validation(durable.clone(), durable.manifest_hash(), &mut service)
        .unwrap()
        .into_validated_receipt()
        .unwrap();
    let finality = signed_finality(context.clone(), subject(&proposal), commitment, 0);
    let paused = service
        .select(&receipt)
        .unwrap()
        .try_consume(|producer, phase| {
            assert!(Arc::ptr_eq(&producer.state, &state));
            assert!(Arc::ptr_eq(&producer.queue, &queue));
            let RetainedPhase::Validated(journals) = phase else {
                panic!("first selection owns the original validation");
            };
            let decision = journals
                .bind_decision(finality, |_| {
                    Ok::<_, Infallible>(PhaseReservation(Arc::clone(&producer.releases)))
                })
                .unwrap_or_else(|refusal| panic!("real signed decision: {:?}", refusal.error));
            Err::<(), _>((RetainedPhase::Decided(decision), "await exact durability"))
        });
    assert_eq!(paused, Err("await exact durability"));
    let decided = service.owner_for_test(durable.subject()).unwrap();
    assert!(matches!(decided, RetainedPhase::Decided(_)));
    assert_eq!(phase_allocations(decided), allocations);
    assert_eq!(decided.ready_commitment(), Some(commitment));
    assert!(decided.matches_candidate(&context, &proposal));
    assert!(!decided.matches_candidate(&wrong_context, &proposal));

    // A later occurrence must reuse the decided execution even if its marker
    // rename succeeds and directory sync refuses. The old receipt stays usable.
    let later_manifest = encode_payload(
        &context,
        wire::ConsensusRound { view: 7, ..round },
        subject(&proposal),
        &bytes,
    )
    .unwrap()
    .manifest()
    .clone();
    let later = store.store(later_manifest, bytes).unwrap();
    fail_next_marker_directory_sync();
    assert!(matches!(
        store.execute_retained_durable_validation(
            later.clone(),
            later.manifest_hash(),
            &mut service
        ),
        Err(V2BodyStoreError::Io { .. })
    ));
    assert_eq!(service.marker_counts_for_test(), (1, 1));
    drop(service.select(&receipt).unwrap());
    assert_eq!(
        phase_allocations(service.owner_for_test(durable.subject()).unwrap()),
        allocations
    );
    let later_receipt = store
        .execute_retained_durable_validation(later.clone(), later.manifest_hash(), &mut service)
        .unwrap()
        .into_validated_receipt()
        .unwrap();
    assert_eq!(service.marker_counts_for_test(), (0, 2));
    assert_eq!(calls.load(Ordering::SeqCst), 1);

    let paused = service
        .select(&later_receipt)
        .unwrap()
        .try_consume(|producer, phase| {
            let RetainedPhase::Decided(decision) = phase else {
                panic!("reproposal retains the current decided phase");
            };
            producer
                .state
                .kura
                .store_block(decision.block().clone())
                .unwrap();
            let durable_finality = producer
                .state
                .kura
                .store_v2_finality_artifact(decision.finality())
                .unwrap();
            let checkpoint = producer
                .state
                .kura
                .persist_wsv_checkpoint_for_v2_commit(
                    &durable_finality,
                    decision.journals.checkpoint,
                )
                .unwrap();
            Err::<(), _>((
                RetainedPhase::Checkpointed(decision.attach_checkpoint(checkpoint)),
                "await publication",
            ))
        });
    assert_eq!(paused, Err("await publication"));
    let checkpointed = service.owner_for_test(durable.subject()).unwrap();
    assert!(matches!(checkpointed, RetainedPhase::Checkpointed(_)));
    assert_eq!(phase_allocations(checkpointed), allocations);
    assert_eq!(checkpointed.ready_commitment(), Some(commitment));
    assert!(checkpointed.matches_candidate(&context, &proposal));
    assert!(!checkpointed.matches_candidate(&wrong_context, &proposal));

    assert_original_queue_refusal(
        &mut service,
        &receipt,
        &state,
        &queue,
        &foreign,
        allocations,
    );
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(releases.load(Ordering::SeqCst), 0);

    let held = hold(&state, "world.accounts");
    let refusal = service
        .select(&receipt)
        .unwrap()
        .try_consume(|producer, phase| {
            let _queue = producer.queue.try_lock_lane_retirement_observer().unwrap();
            let RetainedPhase::Checkpointed(decision) = phase else {
                panic!("physical acquisition must receive the original checkpoint");
            };
            match decision
                .try_prepare_physical(&producer.state, None, |_, _| Ok::<_, Infallible>(()))
            {
                Ok(_) => panic!("original account writer must defer publication"),
                Err((decision, error)) => {
                    Err::<(), _>((RetainedPhase::Checkpointed(decision), error))
                }
            }
        });
    let mut wait = busy_wait(refusal.unwrap_err()).wait_for_release();
    let wakes = Arc::new(WakeCount::default());
    assert!(poll(&mut wait, &wakes).is_pending());
    assert_fences_free_except(&state, "world.accounts");
    drop(queue.try_lock_lane_retirement_observer().unwrap());
    drop(state.kura.try_publication_lease().unwrap());
    assert!(state.block_hashes.writer_available());
    assert_eq!(
        phase_allocations(service.owner_for_test(durable.subject()).unwrap()),
        allocations
    );
    assert_eq!(releases.load(Ordering::SeqCst), 0);
    drop(held);
    assert!(poll(&mut wait, &wakes).is_ready());
    assert_eq!(state.state_view_generation(), generation);
    assert_eq!(
        crate::snapshot::canonical_state_snapshot_hash(&state).unwrap(),
        before
    );

    // An explicit physical abort returns the same checkpointed phase too.
    let aborted = service
        .select(&later_receipt)
        .unwrap()
        .try_consume(|producer, phase| {
            let _queue = producer.queue.try_lock_lane_retirement_observer().unwrap();
            let RetainedPhase::Checkpointed(decision) = phase else {
                panic!("retry cannot reconstruct a validation phase");
            };
            let original = acquire(decision, &producer.state).abort();
            Err::<(), _>((
                RetainedPhase::Checkpointed(original),
                "abort physical attempt",
            ))
        });
    assert_eq!(aborted, Err("abort physical attempt"));
    assert_eq!(
        phase_allocations(service.owner_for_test(durable.subject()).unwrap()),
        allocations
    );
    assert_fences_free_except(&state, "");
    drop(queue.try_lock_lane_retirement_observer().unwrap());
    drop(state.kura.try_publication_lease().unwrap());
    store
        .execute_retained_durable_validation(later.clone(), later.manifest_hash(), &mut service)
        .unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 1);

    let published = service
        .select(&receipt)
        .unwrap()
        .try_consume(|producer, phase| {
            let _queue = producer.queue.try_lock_lane_retirement_observer().unwrap();
            let RetainedPhase::Checkpointed(decision) = phase else {
                panic!("the publisher consumes the retained checkpointed execution");
            };
            acquire(decision, &producer.state)
                .publish()
                .map_err(|(decision, error)| (RetainedPhase::Checkpointed(decision), error))
        })
        .unwrap();
    assert_eq!(published.block().hash(), proposal.hash());
    assert_eq!(state.committed_height(), 1);
    assert_eq!(foreign.committed_height(), 0);
    assert_eq!(
        crate::snapshot::canonical_state_snapshot_hash(&foreign).unwrap(),
        before
    );
    drop(queue.try_lock_lane_retirement_observer().unwrap());
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(releases.load(Ordering::SeqCst), 0);
    assert_eq!(service.marker_counts_for_test(), (0, 0));
    assert!(matches!(
        service.select(&later_receipt),
        Err(CarrierCustodyError::Unconfirmed)
    ));
    assert!(matches!(
        store.execute_retained_durable_validation(
            durable.clone(),
            durable.manifest_hash(),
            &mut service
        ),
        Err(V2BodyStoreError::CarrierCustody(
            CarrierCustodyError::MissingOwner
        ))
    ));
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    drop(published);
    assert_eq!(releases.load(Ordering::SeqCst), 2);
}

#[test]
fn retained_capture_refusal_resumes_original_archives_before_any_validation_marker() {
    use crate::query::{
        provider_ingest_finalized::{
            ProviderIngestFinalizedArchiveBoundsV1, ProviderIngestFinalizedArchiveKeyV1,
        },
        reputation_finalized::{ReputationFinalizedArchiveBounds, ReputationFinalizedArchiveKeyV1},
    };
    use crate::sumeragi::{
        v2_apply::validation_custody::{CarrierCustodyError, RetainedValidationOwner},
        v2_body_store::{
            BlockSignaturePolicy, LocalValidationRefusal, V2BodyStore, V2BodyStoreError,
            fail_next_marker_file_sync,
        },
        v2_chunks::encode_payload,
    };
    use iroha_data_model::block::consensus_v2 as wire;

    let (state, proposal, topology, context) = super::super::super::tests::archive_fixture();
    let state: Arc<State> = state.into();
    let generation = state.state_view_generation();
    let before = crate::snapshot::canonical_state_snapshot_hash(&state).unwrap();
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
    // Reserve the original archive predecessors before execution. The index
    // reader below blocks only insertion preparation after State detachment.
    let provider_owner = provider
        .try_reserve_candidate(
            ProviderIngestFinalizedArchiveKeyV1::try_new(
                context.network_id,
                context.height,
                *proposal.hash().as_ref(),
                proposal.header().creation_time_ms,
            )
            .unwrap(),
            &state.kura,
        )
        .unwrap();
    let reputation_owner = reputation
        .try_reserve_candidate(
            ReputationFinalizedArchiveKeyV1::try_new(
                context.network_id,
                context.height,
                *proposal.hash().as_ref(),
            )
            .unwrap(),
            proposal.header().creation_time_ms,
            &state.kura,
        )
        .unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let releases = Arc::new(AtomicUsize::new(0));
    let wakes = Arc::new(WakeCount::default());
    let wake = Waker::from(Arc::clone(&wakes));
    let mut store = V2BodyStore::open_with_policy(
        root.join("bodies"),
        context.clone(),
        BlockSignaturePolicy::GenesisAuthority(
            iroha_test_samples::SAMPLE_GENESIS_ACCOUNT_KEYPAIR
                .public_key()
                .clone(),
        ),
    )
    .unwrap();
    let bytes = proposal
        .canonical_resultless_proposal()
        .encode_wire()
        .unwrap();
    let round = wire::ConsensusRound {
        context_id: context.id(),
        height: context.height,
        view: 0,
    };
    let first = store
        .store(
            encode_payload(&context, round, subject(&proposal), &bytes)
                .unwrap()
                .manifest()
                .clone(),
            bytes.clone(),
        )
        .unwrap();
    let later = store
        .store(
            encode_payload(
                &context,
                wire::ConsensusRound { view: 7, ..round },
                subject(&proposal),
                &bytes,
            )
            .unwrap()
            .manifest()
            .clone(),
            bytes,
        )
        .unwrap();
    let descriptor_budget = mv::allocation::AllocationBudget::new(
        store
            .retained_validation_descriptor_bytes::<ActualPhaseValidator>()
            .unwrap(),
    );
    let mut service = store
        .retained_validation_service(
            ActualPhaseValidator {
                state: Arc::clone(&state),
                queue: phase_queue(),
                topology,
                calls: Arc::clone(&calls),
                releases: Arc::clone(&releases),
                provider: Some(provider_owner),
                reputation: Some(reputation_owner),
                wake: wake.clone(),
            },
            &descriptor_budget,
        )
        .unwrap();
    let mut allocations = None;
    let mut capture_allocation = None;
    let mut provider_plan = None;
    for durable in [&first, &later] {
        let wake_count = wakes.0.load(Ordering::SeqCst);
        let mut wait = reputation.with_index_reader_for_test(|| {
            let error = store
                .execute_retained_durable_validation(
                    durable.clone(),
                    durable.manifest_hash(),
                    &mut service,
                )
                .unwrap_err();
            let V2BodyStoreError::LocalValidation(LocalValidationRefusal::PhysicalBusy(busy)) =
                error
            else {
                panic!("actual archive contention must retain a local dependency: {error:?}");
            };
            assert_eq!(busy.resource, "reputation archive index");
            assert!(busy.waker().will_wake(&wake));
            let mut wait = busy.wait.wait_for_release();
            assert!(poll(&mut wait, &wakes).is_pending());
            let owner = service.owner_for_test(first.subject()).unwrap();
            let RetainedPhase::Capturing(capture) = owner else {
                panic!("the original execution remains in the candidate slot before validation");
            };
            let original_capture = std::ptr::from_ref(capture.as_ref());
            assert_eq!(
                *capture_allocation.get_or_insert(original_capture),
                original_capture
            );
            let actual = phase_allocations(owner);
            assert_eq!(*allocations.get_or_insert(actual), actual);
            let actual_plan = capture
                .provider
                .as_ref()
                .unwrap()
                .prepared_bytes_identity_for_test()
                .unwrap();
            assert_eq!(*provider_plan.get_or_insert(actual_plan), actual_plan);
            assert!(owner.matches_candidate(&context, &proposal));
            assert_eq!(owner.ready_commitment(), None);
            assert_eq!(service.marker_counts_for_test(), (0, 0));
            assert!(store.validated_recovery_catalog().is_empty());
            assert!(store.rejected_recovery_catalog().is_empty());
            assert_eq!(calls.load(Ordering::SeqCst), 1);
            assert_eq!(releases.load(Ordering::SeqCst), 0);
            assert!(state.block_hashes.writer_available());
            drop(state.world.accounts.block());
            drop(state.transactions.block());
            assert_fences_free_except(&state, "");
            wait
        });
        assert!(poll(&mut wait, &wakes).is_ready());
        assert_eq!(wakes.0.load(Ordering::SeqCst), wake_count + 1);
    }
    assert!(provider.is_empty().unwrap());
    assert!(reputation.is_empty().unwrap());
    assert_eq!(state.state_view_generation(), generation);
    assert_eq!(
        crate::snapshot::canonical_state_snapshot_hash(&state).unwrap(),
        before
    );

    // Archive completion advances the same owner before marker durability.
    fail_next_marker_file_sync();
    assert!(matches!(
        store.execute_retained_durable_validation(
            later.clone(),
            later.manifest_hash(),
            &mut service
        ),
        Err(V2BodyStoreError::Io { .. })
    ));
    let original = service.owner_for_test(first.subject()).unwrap();
    assert!(matches!(original, RetainedPhase::Validated(_)));
    assert_eq!(phase_allocations(original), allocations.unwrap());
    assert!(original.ready_commitment().is_some());
    assert_eq!(service.marker_counts_for_test(), (1, 0));
    assert!(store.rejected_recovery_catalog().is_empty());
    let receipt = store
        .execute_retained_durable_validation(later.clone(), later.manifest_hash(), &mut service)
        .unwrap()
        .into_validated_receipt()
        .unwrap();
    let published = service
        .select(&receipt)
        .unwrap()
        .try_consume(|producer, owner| {
            let _queue = producer.queue.try_lock_lane_retirement_observer().unwrap();
            let RetainedPhase::Validated(journals) = owner else {
                panic!("only completed capture may receive a success marker");
            };
            let decision = bind_and_persist(
                &producer.state,
                &context,
                journals,
                PhaseReservation(Arc::clone(&producer.releases)),
            );
            acquire(decision, &producer.state)
                .publish()
                .map_err(|(owner, error)| (RetainedPhase::Checkpointed(owner), error))
        })
        .unwrap();
    assert_eq!(published.block().hash(), proposal.hash());
    assert_eq!(state.committed_height(), 1);
    assert_eq!(state.state_view_generation(), generation + 2);
    assert!(!provider.is_empty().unwrap());
    assert!(!reputation.is_empty().unwrap());
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(releases.load(Ordering::SeqCst), 0);
    assert_eq!(service.marker_counts_for_test(), (0, 0));
    assert!(matches!(
        store.execute_retained_durable_validation(
            first.clone(),
            first.manifest_hash(),
            &mut service
        ),
        Err(V2BodyStoreError::CarrierCustody(
            CarrierCustodyError::MissingOwner
        ))
    ));
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    drop(published);
    assert_eq!(releases.load(Ordering::SeqCst), 2);
}

#[test]
fn physical_preparation_diagnostics_retain_storage_cause_and_busy_owner() {
    use crate::{
        query::{
            provider_ingest_finalized::ProviderIngestFinalizedArchiveErrorV1,
            reputation_finalized::ReputationFinalizedArchiveError,
        },
        state::carrier_preparation::execution_prefix::CarrierSourceAuthenticationError,
    };

    for error in [
        CarrierPhysicalPreparationError::<Infallible>::Checkpoint(
            crate::kura::Error::CanonicalStoragePoisoned,
        ),
        CarrierPhysicalPreparationError::ExecutionWitness(
            crate::kura::Error::CanonicalStoragePoisoned,
        ),
        CarrierPhysicalPreparationError::Archive(
            super::super::archive_publication::CarrierArchivePublicationError::Checkpoint(
                crate::kura::Error::CanonicalStoragePoisoned,
            ),
        ),
    ] {
        assert!(format!("{error:?}").contains("CanonicalStoragePoisoned"));
    }

    for (error, expected) in [
        (
            CarrierPhysicalPreparationError::<Infallible>::Source(
                CarrierSourceAuthenticationError::Storage(
                    crate::kura::Error::CanonicalStoragePoisoned,
                ),
            ),
            "Source(Storage(CanonicalStoragePoisoned))",
        ),
        (
            CarrierPhysicalPreparationError::Provider(
                ProviderIngestFinalizedArchiveErrorV1::CaptureOwnerMismatch,
            ),
            "Provider(CaptureOwnerMismatch)",
        ),
        (
            CarrierPhysicalPreparationError::Reputation(
                ReputationFinalizedArchiveError::CaptureOwnerMismatch,
            ),
            "Reputation(CaptureOwnerMismatch)",
        ),
    ] {
        assert_eq!(format!("{error:?}"), expected);
    }

    let lock = crate::publication_lock::PublicationMutex::<()>::default();
    let held = lock.lock();
    let wait = lock
        .try_lock_or_wait()
        .err()
        .expect("original owner is held");
    let error = CarrierPhysicalPreparationError::<Infallible>::Fence {
        field: "state_write_lock",
        wait,
    };
    let diagnostic = format!("{error:?}");
    assert!(diagnostic.contains("state_write_lock"));
    assert!(diagnostic.contains("wait"));
    drop(held);
}

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
fn original_state_and_header_are_required_before_witness_or_archive_writes() {
    for wrong_header in [false, true] {
        let (directory, state, mut decision, provider, reputation) = fixture_archive_decision();
        let (mut foreign, _, _, _) = fixture();
        foreign.kura = Arc::clone(&state.kura);
        let original_geometry = if wrong_header {
            let mut header = decision.block().header();
            header.creation_time_ms += 1;
            let substituted = state
                .merge_preexecution_block(header)
                .prepare_carrier_geometry()
                .unwrap();
            Some(std::mem::replace(
                &mut decision.journals.geometry,
                substituted,
            ))
        } else {
            None
        };
        let target = if wrong_header {
            state.as_ref()
        } else {
            foreign.as_ref()
        };
        let before = crate::snapshot::canonical_state_snapshot_hash(&state).unwrap();
        let generation = state.state_view_generation();
        let wire = decision.block().encode_wire().unwrap();
        let hashes = decision
            .journals
            .components
            .block_hashes
            .get(0)
            .map_or(std::ptr::null(), std::ptr::from_ref);
        let witness = decision.journals.source_prefix.witness().writes.as_ptr();
        let inventory = Arc::clone(decision.journals.source_prefix.inventory());
        let releases = Arc::new(AtomicUsize::new(0));
        for occupied in [true, false] {
            // Identity refusal precedes the first Kura and State probes. Repeat
            // without contention to prove that no derived artifact is written.
            let kura = occupied.then(|| state.kura.canonical_publication_lease());
            let held = occupied.then(|| target.state_commit_lock.lock());
            let (retry, error) = match decision.try_prepare_physical(target, None, |_, _| {
                Ok::<_, Infallible>(PhaseReservation(Arc::clone(&releases)))
            }) {
                Ok(_) => panic!("foreign State/header must refuse before publication I/O"),
                Err(refusal) => refusal,
            };
            assert!(matches!(
                error,
                CarrierPhysicalPreparationError::ForeignTarget
            ));
            drop(held);
            drop(kura);
            assert_fences_free_except(target, "");
            assert_eq!(retry.block().encode_wire().unwrap(), wire);
            assert_eq!(
                retry
                    .journals
                    .components
                    .block_hashes
                    .get(0)
                    .map_or(std::ptr::null(), std::ptr::from_ref),
                hashes
            );
            assert_eq!(
                retry.journals.source_prefix.witness().writes.as_ptr(),
                witness
            );
            assert!(Arc::ptr_eq(
                retry.journals.source_prefix.inventory(),
                &inventory
            ));
            assert!(provider.is_empty().unwrap());
            assert!(reputation.is_empty().unwrap());
            for relative in [
                "provider/records",
                "reputation/anchors",
                "reputation/policies",
            ] {
                assert_eq!(
                    std::fs::read_dir(directory.path().join(relative))
                        .unwrap()
                        .count(),
                    0
                );
            }
            for name in ["kagemusha_v1_finality", "kagemusha_v1_finality_staging"] {
                let entries = match std::fs::read_dir(
                    state.kura.store_root().join("blocks/canonical").join(name),
                ) {
                    Ok(entries) => entries.count(),
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => 0,
                    Err(error) => panic!("inspect original witness namespace: {error}"),
                };
                assert_eq!(entries, 0, "target refusal must not write {name}");
            }
            decision = retry;
        }
        assert_eq!(releases.load(Ordering::SeqCst), 2);
        assert_eq!(state.state_view_generation(), generation);
        assert_eq!(
            crate::snapshot::canonical_state_snapshot_hash(&state).unwrap(),
            before
        );
        assert_eq!(foreign.committed_height(), 0);
        if let Some(original_geometry) = original_geometry {
            decision.journals.geometry = original_geometry;
        }
        let checkpoint = decision.journals.checkpoint;
        drop(
            acquire(decision, &state)
                .publish()
                .unwrap_or_else(|(_, error)| {
                    panic!("publish the same owner through its original target: {error:?}")
                }),
        );
        assert!(!provider.is_empty().unwrap());
        assert!(!reputation.is_empty().unwrap());
        assert_eq!(state.state_view_generation(), generation + 2);
        assert_eq!(
            crate::snapshot::canonical_state_snapshot_hash(&state).unwrap(),
            checkpoint
        );
    }
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
        .get(0)
        .map_or(std::ptr::null(), std::ptr::from_ref);
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
                .get(0)
                .map_or(std::ptr::null(), std::ptr::from_ref),
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
        .get(0)
        .map_or(std::ptr::null(), std::ptr::from_ref);
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
            match decision.try_prepare_physical(&state, None, |_, _| Ok::<_, Infallible>(())) {
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
        assert!(state.block_hashes.writer_available());
        assert_eq!(retry.block().encode_wire().unwrap(), wire);
        assert_eq!(
            retry
                .journals
                .components
                .block_hashes
                .get(0)
                .map_or(std::ptr::null(), std::ptr::from_ref),
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

/// Execute a signed lane addition after publishing its actual genesis predecessor.
fn fixture_lifecycle_decision() -> (Box<State>, CheckpointDecision<(), ()>) {
    let (state, decision, _queue) = fixture_lifecycle_decision_with_retirement(None);
    (state, decision)
}

/// Execute real signed lifecycle instructions against the configured original lanes.
fn fixture_lifecycle_decision_with_retirement(
    retirement: Option<bool>,
) -> (Box<State>, CheckpointDecision<(), ()>, Arc<Queue>) {
    use crate::queue::{Queue, execution_context_for_routing_plan};
    use crate::tx::AcceptedTransaction;
    use iroha_crypto::{Algorithm, Hash, KeyPair};
    use iroha_data_model::{
        block::{BlockExecutionContextBundle, BlockHeader},
        nexus::{LaneConfig, LaneLifecycleParameterV1, LaneLifecyclePlan},
        prelude::{Parameter, SetParameter, TransactionBuilder},
    };
    use iroha_model_base::topology::LaneId;
    use iroha_test_samples::{SAMPLE_GENESIS_ACCOUNT_ID, SAMPLE_GENESIS_ACCOUNT_KEYPAIR};
    use std::{borrow::Cow, num::NonZeroU64, time::Duration};

    let mut nexus = iroha_config::parameters::actual::Nexus::default();
    if retirement.is_some() {
        nexus.lane_catalog = iroha_data_model::nexus::LaneCatalog::new(
            std::num::NonZeroU32::new(2).unwrap(),
            vec![
                LaneConfig::default(),
                LaneConfig {
                    id: LaneId::new(1),
                    alias: "retiring-lifecycle".to_owned(),
                    ..LaneConfig::default()
                },
            ],
        )
        .unwrap();
    }
    nexus.fees.base_fee = iroha_primitives::numeric::Quantity::zero();
    nexus.fees.per_byte_fee = iroha_primitives::numeric::Quantity::zero();
    nexus.fees.per_instruction_fee = iroha_primitives::numeric::Quantity::zero();
    nexus.fees.per_gas_unit_fee = iroha_primitives::numeric::Quantity::zero();
    let permission: iroha_data_model::permission::Permission =
        iroha_executor_data_model::permission::parameter::CanSetParameters.into();
    let (state, proposal, topology, mut context) =
        crate::state::carrier_preparation::tests::fixture_with_instructions_and_nexus(
            &[iroha_data_model::isi::Grant::account_permission(
                permission,
                SAMPLE_GENESIS_ACCOUNT_ID.clone(),
            )
            .into()],
            &nexus,
        );
    // This successor uses an actual configured lane committee, independently of
    // the global roster. Bind its explicit manifest to the signed genesis keys
    // before executing the predecessor, rather than bypassing lane planning.
    let lane = state.nexus_snapshot().lane_catalog.lanes()[0].clone();
    let validators = context
        .roster
        .iter()
        .map(|member| {
            iroha_data_model::account::AccountId::new(member.validator.public_key().clone())
        })
        .collect::<Vec<_>>();
    let validator_bindings = validators
        .iter()
        .zip(&context.roster)
        .map(
            |(account, member)| crate::governance::manifest::ManifestValidatorBinding {
                validator: account.clone(),
                peer_id: member.validator.clone(),
                torii_url: None,
            },
        )
        .collect();
    state.install_lane_manifests(&Arc::new(
        crate::governance::manifest::LaneManifestRegistry::from_statuses(
            std::collections::BTreeMap::from([(
                lane.id,
                crate::governance::manifest::LaneManifestStatus {
                    lane: lane.id,
                    alias: lane.alias,
                    dataspace: lane.dataspace_id,
                    visibility: lane.visibility,
                    storage: lane.storage,
                    governance: lane.governance,
                    manifest_path: Some(std::path::PathBuf::from(
                        "fixtures/lifecycle-manifest.json",
                    )),
                    governance_rules: Some(crate::governance::manifest::GovernanceRules {
                        validators,
                        validator_bindings,
                        ..crate::governance::manifest::GovernanceRules::default()
                    }),
                    privacy_commitments: Vec::new(),
                },
            )]),
        ),
    ));
    let genesis = decided(&state, proposal, &topology, &context, (), ());
    let parent = genesis.block().clone();
    context.height = 2;
    context.parent_commit_qc = Some(genesis.finality().commit_qc.clone());
    drop(
        acquire(genesis, &state)
            .publish()
            .unwrap_or_else(|(_, error)| panic!("actual lifecycle predecessor: {error:?}")),
    );
    context.validate().unwrap();
    let nexus = state.nexus_snapshot();
    let incarnations = LaneLifecycleParameterV1::canonical_incarnations(
        &nexus.lane_catalog,
        &state.lane_incarnations_snapshot(),
    )
    .unwrap();
    let parameter = LaneLifecycleParameterV1::new(
        &nexus.lane_catalog,
        &incarnations,
        LaneLifecyclePlan {
            additions: if retirement == Some(false) {
                Vec::new()
            } else {
                vec![LaneConfig {
                    id: LaneId::new(1),
                    alias: "published-lifecycle".to_owned(),
                    ..LaneConfig::default()
                }]
            },
            retire: if retirement.is_some() {
                vec![LaneId::new(1)]
            } else {
                Vec::new()
            },
        },
    )
    .unwrap()
    .into_custom_parameter();
    let mut transaction = TransactionBuilder::new(
        context.network_id,
        SAMPLE_GENESIS_ACCOUNT_ID.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    );
    transaction.set_creation_time(parent.header().creation_time() + Duration::from_millis(1));
    let transaction = transaction
        .with_instructions([SetParameter::new(Parameter::Custom(parameter))])
        .sign(SAMPLE_GENESIS_ACCOUNT_KEYPAIR.private_key());
    let (events, _receiver) = tokio::sync::broadcast::channel(32);
    let queue = Arc::new(Queue::from_config(
        iroha_config::parameters::actual::Queue::default(),
        events,
    ));
    let accepted = AcceptedTransaction::new_unchecked(Cow::Owned(transaction.clone()));
    let route = queue.route_plan_with_state(&accepted, &state).unwrap();
    let leader = context.leader(0);
    let plan = crate::sumeragi::lane_planner::prepare_v2_lane_payload_plan(
        &state,
        &state.kura,
        &context,
        0,
        &context.roster[leader as usize].validator,
        &[route.coordinator_route()],
        &[Hash::from(accepted.hash_as_entrypoint())],
    )
    .unwrap();
    assert!(plan.unavailable_indices.is_empty());
    let execution_context =
        BlockExecutionContextBundle::new(vec![execution_context_for_routing_plan(
            transaction.hash_as_entrypoint(),
            &route,
        )])
        .with_lane_payload_ownerships(plan.ownerships);
    let creation_time = (parent.header().creation_time() + state.sumeragi_block_cadence())
        .max(transaction.creation_time() + Duration::from_millis(1));
    let mut header = BlockHeader::new(
        NonZeroU64::new(2).unwrap(),
        Some(parent.hash()),
        None,
        creation_time.as_millis().try_into().unwrap(),
        0,
    );
    let features = {
        let view = state.view();
        crate::state::compute_confidential_feature_digest(
            view.world(),
            &view.zk,
            view.sccp_registry.as_ref(),
            2,
        )
    };
    header.set_confidential_features((!features.is_empty()).then_some(features));
    let signer = (0_u8..4)
        .map(|index| KeyPair::try_from_seed(vec![0xB0 + index; 32], Algorithm::BlsNormal).unwrap())
        .find(|key| key.public_key() == context.roster[leader as usize].validator.public_key())
        .unwrap();
    let mut builder = iroha_data_model::block::builder::BlockBuilder::new(header);
    builder.push_transaction(transaction);
    builder.set_da_proof_policies(Some(crate::da::active_proof_policy_bundle_at_height(
        &nexus, 2,
    )));
    builder.set_execution_context(Some(execution_context));
    let proposal = builder
        .try_build_with_signature(u64::from(leader), signer.private_key())
        .unwrap()
        .canonical_resultless_proposal();
    let decision = decided(&state, proposal, &topology, &context, (), ());
    assert!(
        decision.block().output_error(0).is_none(),
        "signed lifecycle execution failed: {:?}",
        decision.block().output_error(0)
    );
    assert!(decision.journals.geometry.has_pending_lifecycle());
    assert!(decision.journals.geometry.requires_storage_transition());
    assert_eq!(
        decision.journals.geometry.requires_queue_custody(),
        retirement.is_some()
    );
    (state, decision, queue)
}

pub(super) fn acquire<'target, A, B>(
    decision: CheckpointDecision<A, B>,
    state: &'target State,
) -> PhysicallyPreparedCarrier<'target, A, B, ()> {
    decision
        .try_prepare_physical(state, None, |_, _| Ok::<_, Infallible>(()))
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
        .get(0)
        .map_or(std::ptr::null(), std::ptr::from_ref);
    std::mem::swap(
        &mut decision.journals.source_prefix,
        &mut foreign.source_prefix,
    );
    let held = state.state_commit_lock.lock();
    let (mut retry, error) =
        match decision.try_prepare_physical(&state, None, |_, _| Ok::<_, Infallible>(())) {
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
        retry
            .journals
            .components
            .block_hashes
            .get(0)
            .map_or(std::ptr::null(), std::ptr::from_ref),
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
        match decision.try_prepare_physical(&state, None, |_, _| Ok::<_, Infallible>(())) {
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
        "block_hashes" => Box::new(
            state
                .block_hashes
                .block()
                .detach()
                .try_prepare_publication(&state.block_hashes, |_, _| Ok::<_, ()>(()))
                .unwrap_or_else(|_| panic!("hold hash publisher")),
        ),
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
        .get(0)
        .map_or(std::ptr::null(), std::ptr::from_ref);
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
            match decision.try_prepare_physical(&state, None, |_, _| Ok::<_, Infallible>(())) {
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
            assert!(state.block_hashes.writer_available());
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
            retry
                .journals
                .components
                .block_hashes
                .get(0)
                .map_or(std::ptr::null(), std::ptr::from_ref),
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
    assert!(state.block_hashes.try_view().is_err());
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
    let (state, decision) = fixture_decision();
    let (foreign, _, _, _) = fixture();
    let foreign_geometry = foreign
        .merge_preexecution_block(decision.block().header())
        .prepare_carrier_geometry()
        .unwrap();
    let before = crate::snapshot::canonical_state_snapshot_hash(&state).unwrap();
    let generation = state.state_view_generation();
    let wire = decision.block().encode_wire().unwrap();
    let hashes = decision
        .journals
        .components
        .block_hashes
        .get(0)
        .map_or(std::ptr::null(), std::ptr::from_ref);
    let membership = std::ptr::from_ref(
        decision
            .journals
            .components
            .transactions
            .staged_membership()
            .1,
    );
    let mut physical = acquire(decision, &state);
    // Only this adversarial test can replace geometry after the early check.
    // The terminal consumer must recheck it under the original held lease.
    let original_geometry =
        std::mem::replace(&mut physical.decision.journals.geometry, foreign_geometry);
    let (mut retry, error) = match physical.publish() {
        Ok(_) => panic!("foreign geometry must refuse before effects"),
        Err(refusal) => refusal,
    };
    assert!(matches!(
        error,
        publication::CarrierPublicationError::Geometry
    ));
    assert_fences_free_except(&state, "");
    drop(state.kura.try_publication_lease().unwrap());
    assert!(state.block_hashes.writer_available());
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
        retry
            .journals
            .components
            .block_hashes
            .get(0)
            .map_or(std::ptr::null(), std::ptr::from_ref),
        hashes
    );
    assert_eq!(
        std::ptr::from_ref(retry.journals.components.transactions.staged_membership().1),
        membership
    );
    assert_eq!(state.state_view_generation(), generation);
    assert_eq!(
        crate::snapshot::canonical_state_snapshot_hash(&state).unwrap(),
        before
    );
    retry.journals.geometry = original_geometry;
    let checkpoint = retry.journals.checkpoint;
    drop(
        acquire(retry, &state)
            .publish()
            .unwrap_or_else(|(_, error)| panic!("exact geometry owner retry: {error:?}")),
    );
    assert_eq!(state.state_view_generation(), generation + 2);
    assert_eq!(
        crate::snapshot::canonical_state_snapshot_hash(&state).unwrap(),
        checkpoint
    );
}

#[test]
fn geometry_backend_contention_releases_writers_and_waits_for_actual_backend_release() {
    let (state, decision) = fixture_lifecycle_decision();
    // Only an actual storage transition exercises backend contention during publication.
    assert!(decision.journals.geometry.requires_storage_transition());
    let checkpoint = decision.journals.checkpoint;
    let generation = state.state_view_generation();
    let before = crate::snapshot::canonical_state_snapshot_hash(&state).unwrap();
    let wire = decision.block().encode_wire().unwrap();
    let hashes = decision
        .journals
        .components
        .block_hashes
        .get(0)
        .map_or(std::ptr::null(), std::ptr::from_ref);
    let held = state.tiered_backend.lock();
    let (retry, error) = match acquire(decision, &state).publish() {
        Ok(_) => panic!("the actual backend owner must release first"),
        Err(refusal) => refusal,
    };
    let publication::CarrierPublicationError::GeometryStorage(
        crate::state::LaneLifecycleError::PublicationBusy { field, wait },
    ) = error
    else {
        panic!("expected the backend's actual release observation");
    };
    assert_eq!(field, "tiered_backend");
    assert_fences_free_except(&state, "");
    drop(state.kura.try_publication_lease().unwrap());
    assert_eq!(retry.block().encode_wire().unwrap(), wire);
    assert_eq!(
        retry
            .journals
            .components
            .block_hashes
            .get(0)
            .map_or(std::ptr::null(), std::ptr::from_ref),
        hashes
    );
    let wakes = Arc::new(WakeCount::default());
    let mut wait = wait.wait_for_release();
    assert!(poll(&mut wait, &wakes).is_pending());
    drop(held);
    assert_eq!(wakes.0.load(Ordering::SeqCst), 1);
    assert!(poll(&mut wait, &wakes).is_ready());
    assert_eq!(state.state_view_generation(), generation);
    assert_eq!(
        crate::snapshot::canonical_state_snapshot_hash(&state).unwrap(),
        before
    );
    drop(
        acquire(retry, &state)
            .publish()
            .unwrap_or_else(|(_, error)| panic!("original owner after backend release: {error:?}")),
    );
    assert_eq!(state.state_view_generation(), generation + 2);
    assert_eq!(
        crate::snapshot::canonical_state_snapshot_hash(&state).unwrap(),
        checkpoint
    );
    assert!(
        state
            .nexus_snapshot()
            .lane_catalog
            .lanes()
            .iter()
            .any(|lane| lane.id == iroha_model_base::topology::LaneId::new(1))
    );
    assert_eq!(
        state
            .da_shard_cursors
            .read()
            .canonical_reset_height_for_lane(iroha_model_base::topology::LaneId::new(1)),
        Some(2)
    );
    assert!(
        state
            .lane_manifests
            .read()
            .status(iroha_model_base::topology::LaneId::new(1))
            .is_some()
    );
    assert_fences_free_except(&state, "");
    drop(state.kura.try_publication_lease().unwrap());
}

#[test]
fn lifecycle_effect_refusal_precedes_storage_and_preserves_exact_retry() {
    let (state, mut decision) = fixture_lifecycle_decision();
    let checkpoint = decision.journals.checkpoint;
    let lifecycle = decision.journals.effects.lifecycle.take().unwrap();
    let before = crate::snapshot::canonical_state_snapshot_hash(&state).unwrap();
    let generation = state.state_view_generation();
    let wire = decision.block().encode_wire().unwrap();
    let held = state.tiered_backend.lock();
    let (mut retry, error) = acquire(decision, &state)
        .publish()
        .err()
        .expect("missing accepted lifecycle effects cannot reach geometry storage");
    assert!(matches!(
        error,
        publication::CarrierPublicationError::Geometry
    ));
    assert_fences_free_except(&state, "");
    drop(state.kura.try_publication_lease().unwrap());
    assert_eq!(state.state_view_generation(), generation);
    assert_eq!(retry.block().encode_wire().unwrap(), wire);
    assert_eq!(
        crate::snapshot::canonical_state_snapshot_hash(&state).unwrap(),
        before
    );
    drop(held);
    retry.journals.effects.lifecycle = Some(lifecycle);
    drop(
        acquire(retry, &state)
            .publish()
            .unwrap_or_else(|(_, error)| panic!("exact lifecycle retry: {error:?}")),
    );
    assert_eq!(state.state_view_generation(), generation + 2);
    assert_eq!(
        crate::snapshot::canonical_state_snapshot_hash(&state).unwrap(),
        checkpoint
    );
}

#[test]
fn installation_refusal_precedes_all_fences_and_returns_the_decided_carrier() {
    let (state, decision) = fixture_decision();
    let original = decision.block().encode_wire().unwrap();
    let held = state.state_commit_lock.lock();
    let canonical = state.kura.canonical_publication_lease();
    let (retry, error) =
        match decision.try_prepare_physical(&state, None, |_, _| Err::<(), _>("capacity")) {
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
    let (retry, error) =
        match decision.try_prepare_physical(&state, None, |_, _| Ok::<_, Infallible>(())) {
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
    assert!(state.block_hashes.writer_available());
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
fn actual_validation_overlay_releases_hash_before_retaining_membership_writers() {
    let (state, decision) = fixture_decision();
    let before = crate::snapshot::canonical_state_snapshot_hash(&state).unwrap();
    let validating = state.block(decision.block().header());
    let (retry, error) =
        match decision.try_prepare_physical(&state, None, |_, _| Ok::<_, Infallible>(())) {
            Ok(_) => panic!("the real validation overlay owns the original cut"),
            Err(refusal) => refusal,
        };
    assert!(matches!(
        &error,
        CarrierPhysicalPreparationError::Component {
            field: "transactions",
            cause: mv::PublicationPreparationError::Busy(_),
        }
    ));
    assert!(state.block_hashes.writer_available());
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
    let (decision, error) = match decision.try_prepare_physical(&foreign, None, |_, _| {
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
    let (decision, error) = match decision.try_prepare_physical(&foreign, None, |_, _| {
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
    let canonical = state.kura.canonical_publication_lease();
    let held = foreign.state_commit_lock.lock();
    let (retry, error) =
        match decision.try_prepare_physical(&foreign, None, |_, _| Ok::<_, Infallible>(())) {
            Ok(_) => panic!("equal State bytes cannot replace original journals"),
            Err(refusal) => refusal,
        };
    assert!(matches!(
        error,
        CarrierPhysicalPreparationError::ForeignTarget
    ));
    drop(held);
    drop(canonical);
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
        assert!(self.state.block_hashes.writer_available());
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
            .try_prepare_physical(&state, None, |_, _| {
                Ok::<_, Infallible>(guard("installation"))
            })
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
        .get(0)
        .map_or(std::ptr::null(), std::ptr::from_ref);
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
            match decision.try_prepare_physical(&state, None, |_, _| Ok::<_, Infallible>(())) {
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
        assert!(state.block_hashes.writer_available());
        assert_eq!(retry.block().encode_wire().unwrap(), wire);
        assert_eq!(
            retry
                .journals
                .components
                .block_hashes
                .get(0)
                .map_or(std::ptr::null(), std::ptr::from_ref),
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
        .get(0)
        .map_or(std::ptr::null(), std::ptr::from_ref);
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
    let (retry, error) =
        match decision.try_prepare_physical(&state, None, |_, _| Ok::<_, Infallible>(())) {
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
    assert!(state.block_hashes.writer_available());
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
        retry
            .journals
            .components
            .block_hashes
            .get(0)
            .map_or(std::ptr::null(), std::ptr::from_ref),
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
        .get(0)
        .map_or(std::ptr::null(), std::ptr::from_ref);
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
            match decision.try_prepare_physical(&state, None, |_, _| Ok::<_, Infallible>(())) {
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
        assert!(state.block_hashes.writer_available());
        assert_eq!(retry.block().encode_wire().unwrap(), wire);
        assert_eq!(
            retry
                .journals
                .components
                .block_hashes
                .get(0)
                .map_or(std::ptr::null(), std::ptr::from_ref),
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
        match decision.try_prepare_physical(&state, None, |_, _| Ok::<_, Infallible>(())) {
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

#[path = "queue_publication_tests.rs"]
mod queue_publication_tests;
