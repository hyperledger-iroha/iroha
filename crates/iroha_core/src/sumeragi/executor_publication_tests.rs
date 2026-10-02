//! Original executor ownership through durable-append refusal, success, and consuming failure.
//!
//! The fixture executes actual signed genesis and the real State validator. Its four BLS keys
//! sign the real result; no synthetic genesis QC or substitute overlay is used.

use super::*;
use crate::{
    state::World,
    sumeragi::{
        block_store::KuraBlockStore,
        crypto::BlsCrypto,
        driver::traits::BlockStore as _,
        startup,
        test_chain::{CertifiedTestChain, Signers, TestChainConfig},
    },
};
use iroha_sumeragi::{crypto::NoAttestation, preimage::payload_hash};

pub(super) fn with_worker(
    test: impl FnOnce(
        &CertifiedTestChain,
        &mut Worker<'_>,
        &KuraBlockStore,
        &mut tokio::sync::broadcast::Receiver<EventBox>,
    ) + Send
    + 'static,
) {
    with_worker_from(
        || {
            CertifiedTestChain::start(TestChainConfig::new(World::new(), 1_000))
                .expect("actual signed genesis")
        },
        ConsensusMode::Permissioned,
        test,
    );
}

fn with_worker_from(
    make_chain: impl FnOnce() -> CertifiedTestChain + Send + 'static,
    consensus_mode: ConsensusMode,
    test: impl FnOnce(
        &CertifiedTestChain,
        &mut Worker<'_>,
        &KuraBlockStore,
        &mut tokio::sync::broadcast::Receiver<EventBox>,
    ) + Send
    + 'static,
) {
    crate::sumeragi::threads::sumeragi_thread_builder("sumeragi-publication-test")
        .spawn(move || {
            let chain = make_chain();
            assert_eq!(chain.validators().len(), 4);
            let (_, certificate, _) = startup::stored_genesis(chain.state()).unwrap().unwrap();
            assert!(certificate.consensus_header().is_empty());
            assert!(
                certificate.commit_qc().is_empty(),
                "genesis carries its executed result, never a fabricated quorum"
            );
            let crypto = Arc::new(BlsCrypto::new());
            crypto
                .admit_committee(
                    chain
                        .validators()
                        .iter()
                        .map(|(peer, pop)| (peer.public_key(), pop.as_slice())),
                )
                .unwrap();
            let (events, mut receiver) = tokio::sync::broadcast::channel(1024);
            let context = ExecutorContext {
                state: Arc::clone(chain.state()),
                native_context_archive: Arc::new(
                    crate::query::native_context_archive::NativeContextArchive::open(
                        chain.state().kura(),
                        chain.state().ivm_execution_budget(),
                        chain.state().kura().native_context_archive_max_bytes(),
                    )
                    .expect("original-pool native context archive"),
                ),
                queue: None,
                staging: Staging::new(),
                events,
                genesis_account: chain.genesis_account().clone(),
                consensus_mode,
                applied: (chain.height(), chain.committed(chain.height()).core_hash()),
                crypto: Some(Arc::clone(&crypto)),
                applied_watch: Arc::new(crate::sumeragi::lanes::global::AppliedWatch::new(1, None)),
                lane_blocks: Arc::new(crate::sumeragi::lanes::merge::NoLanes),
            };
            let schedule = Arc::new(
                crate::sumeragi::runtime_availability::NativeGlobalAvailability::new(
                    Arc::clone(chain.state()),
                    chain.instance(),
                    Arc::clone(&crypto),
                )
                .unwrap(),
            );
            let verifier = Arc::new(crate::sumeragi::attestation::NativePastaVerifier::new(
                chain.instance(),
                chain.network_id(),
            ));
            let blocks = KuraBlockStore::new(
                Arc::clone(chain.kura()),
                crypto,
                1,
                context.staging.clone(),
                context.state.ivm_execution_budget(),
                schedule,
                verifier,
            );
            let mut worker = Worker {
                payload_build: None,
                routing_refusal: None,
                payload_refusal: None,
                context: &context,
                state: &context.state,
                applied: context.applied,
                live: None,
                finishing: None,
                results: BTreeMap::new(),
                last_built: None,
                queue: None,
                recovery: None,
                beacon: None,
                archives: None,
                pending_commit: None,
                attestation: None,
                quarantine_context: None,
            };
            test(&chain, &mut worker, &blocks, &mut receiver);
        })
        .expect("spawn publication worker fixture")
        .join()
        .expect("publication worker fixture");
}

pub(super) fn proposal(chain: &CertifiedTestChain, worker: &Worker<'_>) -> AvailableBody {
    proposal_with_transaction(chain, worker, CertifiedTestChain::tick)
}

#[test]
fn payload_decode_refusal_retains_available_owner_without_negative_cache() {
    with_worker(|chain, worker, _blocks, events| {
        let block = proposal(chain, worker);
        let hash = block.hash(&**worker.context.crypto.as_ref().unwrap());
        let source = std::ptr::from_ref(block.source());
        let payload = block.payload().as_slice().as_ptr();
        let budget = worker.state.ivm_execution_budget();
        let state_height = worker.state.view().height();
        assert!(block.admitted_to(&budget));
        // Pin only the refusal observations: unrelated MV reclamation must not change the
        // baseline while this test measures the request's immediately refunded owners.
        let epoch = crossbeam_epoch::pin();
        let retained = budget.reserved_bytes();
        for limits in [
            norito::DecodeLimits::new(usize::MAX, 1, usize::MAX, usize::MAX, 64),
            norito::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, 1, 64),
            norito::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, usize::MAX, 0),
        ] {
            let error = norito::with_decode_limits_scope(limits, || {
                iroha_data_model::block::decode_versioned_signed_block(block.payload().as_slice())
            })
            .unwrap_err();
            assert!(error.is_decode_resource_limit(), "{error:?}");
            assert_eq!(
                norito::with_decode_limits_scope(limits, || payload::decode(
                    block.payload().as_slice()
                ))
                .unwrap_err(),
                payload::PayloadError::DecodeResource(
                    ivm::error::ExecutionDeferral::ActiveMemoryCapacity.into()
                )
            );
            let outcome = norito::with_decode_limits_scope(limits, || worker.execute(&block, hash));
            assert!(
                matches!(outcome, Some(ExecOutcome::Failed(_))),
                "local payload decode refusal must defer, got {outcome:?}; negatively cached: {}",
                worker.results.contains_key(&hash)
            );
            assert!(!worker.results.contains_key(&hash));
            assert!(worker.live.is_none());
            assert!(worker.finishing.is_none());
            assert!(worker.recovery.is_none());
            assert!(worker.context.staging.get(&hash).is_none());
            assert!(events.try_recv().is_err());
            assert_eq!(worker.state.view().height(), state_height);
            assert_eq!(std::ptr::from_ref(block.source()), source);
            assert_eq!(block.payload().as_slice().as_ptr(), payload);
            assert!(block.admitted_to(&budget));
            assert_eq!(budget.reserved_bytes(), retained);
        }
        drop(epoch);
        assert!(matches!(
            worker.execute(&block, hash),
            Some(ExecOutcome::Valid(_))
        ));
        assert!(!worker.results.contains_key(&hash));
        assert_eq!(std::ptr::from_ref(block.source()), source);
        assert_eq!(block.payload().as_slice().as_ptr(), payload);
        assert!(block.admitted_to(&budget));
        assert_eq!(
            worker.state.view().height(),
            state_height,
            "execution has not published"
        );
    });
}

#[test]
fn original_staking_payload_worker_retains_pool_refusal_and_exact_queued_retry() {
    with_worker_from(
        || {
            use iroha_data_model::parameter::{
                Parameter,
                system::{SumeragiConsensusMode, SumeragiNposParameters},
            };
            let mut config = TestChainConfig::new(World::new(), 1_000);
            config.consensus_mode = SumeragiConsensusMode::Npos;
            config.genesis_parameters.push(Parameter::Custom(
                SumeragiNposParameters {
                    slashing_delay_blocks: 1,
                    ..SumeragiNposParameters::default()
                }
                .into_custom_parameter(),
            ));
            CertifiedTestChain::start(config).expect("actual four-validator NPoS genesis")
        },
        ConsensusMode::Npos,
        |chain, worker, _blocks, events| {
            use crate::state::EvidencePreparationError;
            use std::{future::Future, pin::pin, task::Context};
            let transaction = chain.tick(2_000);
            let original_hash = transaction.hash_as_entrypoint();
            let (_, time) =
                iroha_primitives::time::TimeSource::new_mock(Duration::from_millis(2_001));
            let queue = Arc::new(Queue::test(
                iroha_config::parameters::actual::Queue::default(),
                &time,
            ));
            let accepted = crate::tx::AcceptedTransaction::accept_with_time_source(
                transaction,
                &chain.network_id(),
                Duration::from_secs(1),
                chain.state().view().world().parameters().transaction(),
                &iroha_config::parameters::actual::Crypto::default(),
                &time,
            )
            .unwrap();
            queue.push(accepted, chain.state().view()).unwrap();
            worker.queue = Some(Arc::clone(&queue));
            let (due, requested_bytes) =
                super::super::penalties::pending_payload_penalty_fixture(worker.state);
            let original = worker
                .state
                .world
                .consensus_evidence
                .view()
                .get(&due)
                .cloned()
                .unwrap();
            let budget = worker.state.evidence_preparation_budget();
            let occupied_bytes = budget.limit_bytes() - requested_bytes + 1;
            let blocking_owner = budget.try_reserve_bytes(occupied_bytes).unwrap();
            let header = iroha_data_model::block::BlockHeader::new(
                std::num::NonZeroU64::new(2).unwrap(),
                None,
                None,
                2_001,
                0,
            );
            let original_error = super::super::penalties::PenaltyApplier::new(worker.state, None)
                .derive_npos_consensus_effects(&header)
                .unwrap_err();
            let EvidencePreparationError::Admission(original_allocation) = original_error
                .downcast_ref::<EvidencePreparationError>()
                .unwrap()
            else {
                panic!("the original evidence pool must refuse its actual due-entry backing");
            };
            let original_refusal: crate::execution_attempt::ExecutionDeferred =
                original_allocation.clone().into();
            assert!(matches!(
                worker.build(2, 0, 1 << 20),
                Err(PublicationError::Retryable(_))
            ));
            assert_eq!(
                worker.routing_refusal.as_ref(),
                Some(&original_refusal),
                "the payload worker retains the same evidence-pool release owner"
            );
            assert!(matches!(
                worker.payload_refusal.as_ref(),
                Some(payload::PayloadError::StakingPreparation(refusal))
                    if Some(refusal) == original_error.downcast_ref::<EvidencePreparationError>()
            ));
            let iroha_allocation::AllocationRefusal::Capacity { release, .. } = worker
                .routing_refusal
                .as_ref()
                .unwrap()
                .allocation_refusal()
                .unwrap()
            else {
                panic!("the original occupied pool remains retryable");
            };
            let mut release = pin!(release.clone().wait_for_release());
            let mut context = Context::from_waker(std::task::Waker::noop());
            assert!(release.as_mut().poll(&mut context).is_pending());
            assert_eq!(budget.reserved_bytes(), occupied_bytes);
            assert_eq!(queue.queued_len(), 1);
            assert!(queue.contains_entrypoint_hash(original_hash));
            assert_eq!(
                worker.state.world.consensus_evidence.view().get(&due),
                Some(&original)
            );
            assert_eq!(worker.state.view().height(), 1);
            assert!(worker.payload_build.is_none());
            assert!(worker.last_built.is_none());
            assert!(worker.live.is_none());
            assert!(worker.finishing.is_none());
            assert!(worker.pending_commit.is_none());
            assert!(worker.recovery.is_none());
            assert!(events.try_recv().is_err());
            drop(blocking_owner);
            assert!(release.as_mut().poll(&mut context).is_ready());
            let (Some(bytes), false) = worker.build(2, 0, 1 << 20).unwrap() else {
                panic!("the exact original queued work retries after the original release");
            };
            assert!(worker.routing_refusal.is_none());
            assert!(worker.payload_refusal.is_none());
            let proposal = payload::decode(bytes.as_slice()).unwrap();
            assert_eq!(
                proposal.external_entrypoints_slice()[0].hash(),
                original_hash
            );
            assert!(
                matches!(proposal.npos_consensus_effects().unwrap().penalty_actions.as_slice(),
                [iroha_data_model::consensus::NposPenaltyAction::MarkConsensusEvidenceApplied(mark)] if mark.evidence_key == due && mark.height == 2)
            );
            assert_eq!(queue.queued_len(), 1);
            assert!(queue.contains_entrypoint_hash(original_hash));
            assert_eq!(
                worker.state.world.consensus_evidence.view().get(&due),
                Some(&original)
            );
            assert_eq!(worker.state.view().height(), 1);
            assert_eq!(budget.reserved_bytes(), 0);
            assert!(events.try_recv().is_err());
        },
    );
}

#[test]
fn malformed_available_payload_remains_invalid_and_negatively_cached() {
    with_worker(|chain, worker, _blocks, events| {
        let original = proposal(chain, worker);
        let mut malformed = original.payload().as_slice().to_vec();
        malformed.pop().unwrap();
        let mut header = original.header().clone();
        header.payload_hash = payload_hash(&**worker.context.crypto.as_ref().unwrap(), &malformed);
        header.payload_len = u32::try_from(malformed.len()).unwrap();
        // Availability is genuine for these malformed application bytes. Its signatures
        // authenticate custody, not the nested block's validity.
        let block = chain.author_payload(header, malformed);
        let hash = block.hash(&**worker.context.crypto.as_ref().unwrap());
        let state_height = worker.state.view().height();
        assert!(matches!(
            payload::decode(block.payload().as_slice()),
            Err(payload::PayloadError::NotCanonical(_))
        ));
        assert!(matches!(
            worker.execute(&block, hash),
            Some(ExecOutcome::Invalid)
        ));
        let (source, outcome) = worker
            .results
            .get(&hash)
            .expect("deterministic negative verdict");
        assert_eq!(source, block.source());
        assert!(matches!(outcome, ExecOutcome::Invalid));
        let limits = norito::DecodeLimits::new(usize::MAX, 1, usize::MAX, usize::MAX, 64);
        assert!(matches!(
            norito::with_decode_limits_scope(limits, || worker.execute(&block, hash)),
            Some(ExecOutcome::Invalid)
        ));
        assert_eq!(worker.results.len(), 1);
        assert!(worker.live.is_none());
        assert!(worker.finishing.is_none());
        assert!(worker.context.staging.get(&hash).is_none());
        assert!(events.try_recv().is_err());
        assert_eq!(worker.state.view().height(), state_height);
    });
}

fn proposal_with_transaction(
    chain: &CertifiedTestChain,
    worker: &Worker<'_>,
    transaction: impl FnOnce(
        &CertifiedTestChain,
        u64,
    ) -> iroha_data_model::transaction::SignedTransaction,
) -> AvailableBody {
    let height = worker.applied.0 + 1;
    let certified_parent = chain.committed(worker.applied.0);
    let scheduled = worker.scheduled(height).unwrap().height_config().unwrap();
    let crypto = worker.context.crypto.as_ref().unwrap();
    let view = chain.state().view();
    let parent = view.latest_block().unwrap();
    let cadence = Duration::from_millis(scheduled.params.block_time);
    let block_time = parent.header().creation_time() + cadence;
    let tx = transaction(chain, u64::try_from(block_time.as_millis()).unwrap() - 1);
    let (_, time) = iroha_primitives::time::TimeSource::new_mock(block_time);
    let accepted = crate::tx::AcceptedTransaction::accept_with_time_source(
        tx,
        &chain.network_id(),
        Duration::from_secs(1),
        view.world().parameters().transaction(),
        &iroha_config::parameters::actual::Crypto::default(),
        &time,
    )
    .unwrap();
    drop(view);
    let proposal = payload::assemble(
        chain.state(),
        Assembly {
            parent: &parent,
            view: 0,
            cadence,
        },
        &[accepted],
    )
    .unwrap();
    let payload = payload::encode(&proposal).unwrap();
    chain.author_payload(
        iroha_sumeragi::message::BlockHeader {
            instance: chain.instance(),
            epoch: scheduled.epoch.id,
            height,
            origin_view: 0,
            parent_hash: certified_parent.core_hash(),
            parent_result: certified_parent.result(),
            payload_hash: payload_hash(&**crypto, &payload),
            payload_len: u32::try_from(payload.len()).unwrap(),
            availability_digest: Hash32::ZERO,
            proposer: 0,
            skipped_leaders: Vec::new(),
            control_witness: iroha_sumeragi::types::ControlWitness::empty(),
            attest: height == scheduled.epoch.last_height,
        },
        payload,
    )
}

pub(super) fn executed(chain: &CertifiedTestChain, worker: &mut Worker<'_>) -> (AvailableBody, Qc) {
    let block = proposal(chain, worker);
    execute_proposal(chain, worker, block)
}

fn execute_proposal(
    chain: &CertifiedTestChain,
    worker: &mut Worker<'_>,
    block: AvailableBody,
) -> (AvailableBody, Qc) {
    let crypto = worker.context.crypto.as_ref().unwrap();
    let block_hash = block.hash(&**crypto);
    let Some(ExecOutcome::Valid(result)) = worker.execute(&block, block_hash) else {
        panic!("original native validation must succeed")
    };
    let qc = chain.commit_qc(2, block_hash, result, false, Signers::Quorum);
    let committee = worker
        .scheduled(2)
        .unwrap()
        .height_config()
        .unwrap()
        .committee;
    iroha_sumeragi::crypto::Verifier::new(
        &**worker.context.crypto.as_ref().unwrap(),
        &chain.instance(),
        &block.header().epoch,
        &committee,
    )
    .verify_qc(&NoAttestation, &qc)
    .unwrap();
    (block, qc)
}

fn original_overlay(worker: &Worker<'_>) -> usize {
    std::ptr::from_ref(worker.live.as_ref().unwrap().overlay.as_deref().unwrap()) as usize
}

#[cfg(feature = "telemetry")]
#[test]
fn canonical_replay_origin_retains_transition_idempotence_through_publication_retry() {
    use iroha_data_model::{
        governance::types::{
            AbiVersion, ContractAbiHash, ContractCodeHash, DeployContractProposal, ProposalKind,
        },
        isi::governance::{
            CreateParliamentGovernanceAttemptV1, ParliamentLifecycleTransitionV1,
            SubmitParliamentLifecycleTransitionV1,
        },
        permission::Permission,
        prelude::InstructionBox,
    };
    use iroha_executor_data_model::permission::governance::CanManageParliament;
    use mv::storage::StorageReadOnly as _;
    use std::collections::BTreeSet;

    for origin in [
        CommitTelemetryOrigin::Forward,
        CommitTelemetryOrigin::HistoricalReplay,
    ] {
        let manager =
            iroha_crypto::KeyPair::from_seed(vec![0xCE; 32], iroha_crypto::Algorithm::Ed25519);
        let manager_account =
            iroha_data_model::account::AccountId::new(manager.public_key().clone());
        let create = CreateParliamentGovernanceAttemptV1 {
            proposal: ProposalKind::DeployContract(DeployContractProposal {
                proposal_operator: manager_account.clone(),
                contract_address: "irohac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4lv4vp9gg4yxgjw"
                    .parse()
                    .unwrap(),
                code_hash: ContractCodeHash::new([0x31; 32]),
                abi_hash: ContractAbiHash::new([0x41; 32]),
                abi_version: AbiVersion::new(1),
                manifest_provenance: None,
            }),
            attempt_sequence: 0,
        };
        let seeded_proposal = create.proposal.clone();
        with_worker_from(
            move || {
                // Fix the initial proposal and manager in World before the actual
                // signed genesis is executed. No post-genesis source is replaced.
                let mut world = World::new();
                world.account_permissions.insert(
                    manager_account.clone(),
                    BTreeSet::from([Permission::from(CanManageParliament)]),
                );
                world.governance_proposals.insert(
                    seeded_proposal.fingerprint(),
                    crate::state::GovernanceProposalRecord {
                        proposer: manager_account,
                        kind: seeded_proposal,
                        created_height: 1,
                        status: crate::state::GovernanceProposalStatus::Proposed,
                    },
                );
                CertifiedTestChain::start(TestChainConfig::new(world, 1_000))
                    .expect("actual signed genesis with initial governance World")
            },
            ConsensusMode::Permissioned,
            move |chain, worker, blocks, _| {
                // Signed instructions create the real reducer state and event before
                // the original execution witness and publication surface are captured.
                let attempt_id = create.governance_attempt_id();
                let transition = SubmitParliamentLifecycleTransitionV1 {
                    governance_attempt_id: attempt_id,
                    transition: ParliamentLifecycleTransitionV1::CompleteQualification,
                };
                let body = proposal_with_transaction(chain, worker, |chain, created_ms| {
                    chain.sign(
                        &manager,
                        [
                            InstructionBox::from(create),
                            InstructionBox::from(transition),
                        ],
                        created_ms,
                    )
                });
                let (block, qc) = execute_proposal(chain, worker, body);
                let stage = worker
                    .live
                    .as_ref()
                    .unwrap()
                    .overlay
                    .as_ref()
                    .unwrap()
                    .world
                    .parliament_attempts
                    .get(&attempt_id)
                    .expect("signed Parliament instructions executed")
                    .attempt()
                    .stage;
                assert_ne!(
                    stage,
                    iroha_data_model::governance::types::GovernanceStageV1::Qualification,
                    "the actual reducer completed qualification before witness capture"
                );
                worker.prepare_with_origin(&block, &qc, origin).unwrap();
                let original = original_overlay(worker);
                let other = match origin {
                    CommitTelemetryOrigin::Forward => CommitTelemetryOrigin::HistoricalReplay,
                    CommitTelemetryOrigin::HistoricalReplay => CommitTelemetryOrigin::Forward,
                };
                assert!(worker.prepare_with_origin(&block, &qc, other).is_err());
                assert_eq!(original_overlay(worker), original);
                blocks.append(&block, &qc).unwrap();
                let metric = chain
                    .state()
                    .telemetry
                    .governance_parliament_transitions_total
                    .with_label_values(&["complete_qualification"]);
                assert_eq!(metric.get(), 0);
                for _ in 0..2 {
                    chain.state().with_publication_blocked_for_test(|| {
                        assert!(matches!(
                            worker.commit(&block, &qc),
                            Err(PublicationError::Retryable(_))
                        ));
                    });
                    assert_eq!(
                        metric.get(),
                        0,
                        "refusal cannot publish transition observations"
                    );
                    assert_eq!(original_overlay(worker), original);
                    assert_eq!(
                        worker.prepare_with_origin(&block, &qc, origin).unwrap(),
                        Some(qc.result)
                    );
                    assert!(worker.prepare_with_origin(&block, &qc, other).is_err());
                }
                worker.commit(&block, &qc).unwrap();
                assert_eq!(
                    chain
                        .state()
                        .view()
                        .world()
                        .parliament_attempts()
                        .get(&attempt_id)
                        .unwrap()
                        .attempt()
                        .stage,
                    stage,
                    "both origins publish the same authenticated transition state"
                );
                let expected = u64::from(origin == CommitTelemetryOrigin::Forward);
                assert_eq!(metric.get(), expected);
                worker.commit(&block, &qc).unwrap();
                assert_eq!(
                    metric.get(),
                    expected,
                    "the original publication completes once"
                );
            },
        );
    }
}

#[test]
fn reversible_publication_refusal_retains_original_overlay_capture_and_certified_frame() {
    with_worker(|chain, worker, blocks, events| {
        let (block, qc) = executed(chain, worker);
        let overlay = original_overlay(worker);
        let witness = iroha_crypto::HashOf::new(&worker.live.as_ref().unwrap().witness);
        let context = worker
            .live
            .as_ref()
            .unwrap()
            .commitment
            .get()
            .schedule
            .current
            .committee
            .as_ptr();
        let preimage = match &worker.live.as_ref().unwrap().phase {
            PublicationPhase::Executed { preimage, .. } => preimage.as_slice().as_ptr(),
            _ => panic!("new original execution"),
        };
        assert_eq!(worker.prepare(&block, &qc).unwrap(), Some(qc.result));
        let original = worker.context.staging.get(&qc.block_hash).unwrap();
        let certificate = original.executed.commit_certificate().unwrap();
        assert_eq!(
            certificate.result_preimage().as_ptr(),
            preimage,
            "the original captured R allocation moved into its sole certificate"
        );
        let bytes = original.executed.encode_wire().unwrap();
        let pending_events = worker.live.as_ref().unwrap().events.len();
        for _ in 0..3 {
            let error = worker.commit(&block, &qc).unwrap_err();
            assert!(matches!(&error, PublicationError::Retryable(_)));
            assert!(error.to_string().contains("durably stored"));
            assert!(worker.recovery.is_none());
            assert_eq!(original_overlay(worker), overlay);
            assert_eq!(
                worker
                    .live
                    .as_ref()
                    .unwrap()
                    .commitment
                    .get()
                    .schedule
                    .current
                    .committee
                    .as_ptr(),
                context,
                "publication retry retains the original canonical authority allocation"
            );
            assert_eq!(
                iroha_crypto::HashOf::new(&worker.live.as_ref().unwrap().witness),
                witness
            );
            assert_eq!(worker.live.as_ref().unwrap().events.len(), pending_events);
            assert!(
                events.try_recv().is_err(),
                "authorization refusal publishes no events"
            );
            assert_eq!(worker.prepare(&block, &qc).unwrap(), Some(qc.result));
            let retry = worker.context.staging.get(&qc.block_hash).unwrap();
            assert!(Arc::ptr_eq(&retry, &original));
            assert!(Arc::ptr_eq(&retry.executed, &original.executed));
            assert!(std::ptr::eq(
                retry.executed.commit_certificate().unwrap(),
                certificate
            ));
        }
        blocks.append(&block, &qc).unwrap();
        blocks.append(&block, &qc).unwrap();
        assert_eq!(original.executed.encode_wire().unwrap(), bytes);
        let next = worker.commit(&block, &qc).unwrap();
        assert_eq!(worker.state.view().height(), 2);
        assert_eq!(worker.applied, (2, qc.block_hash));
        assert!(worker.live.as_ref().unwrap().overlay.is_none());
        let mut emitted = 0;
        while events.try_recv().is_ok() {
            emitted += 1;
        }
        assert!(
            emitted > 0,
            "actual validation/application emits its events once"
        );
        assert_eq!(worker.commit(&block, &qc).unwrap(), next);
        assert_eq!(worker.prepare(&block, &qc).unwrap(), Some(qc.result));
        assert_eq!(worker.commit(&block, &qc).unwrap(), next);
        assert!(
            events.try_recv().is_err(),
            "completed retry cannot duplicate side effects"
        );
        assert_eq!(blocks.entry(2).unwrap().unwrap().commit_qc, qc);
    });
}

#[test]
fn prepared_certificate_cannot_be_rebound_to_another_epoch_context() {
    with_worker(|chain, worker, blocks, _| {
        let (block, qc) = executed(chain, worker);
        let overlay = original_overlay(worker);
        worker.prepare(&block, &qc).unwrap();
        let staged = worker.context.staging.get(&qc.block_hash).unwrap();
        let mut changed = qc.clone();
        changed.epoch.context.0[0] ^= 1;
        assert!(worker.prepare(&block, &changed).is_err());
        assert!(worker.commit(&block, &changed).is_err());
        assert_eq!(original_overlay(worker), overlay);
        assert!(Arc::ptr_eq(
            &staged,
            &worker.context.staging.get(&qc.block_hash).unwrap()
        ));
        assert_eq!(worker.state.committed_height(), 1);
        blocks.append(&block, &qc).unwrap();
        worker.commit(&block, &qc).unwrap();
        assert_eq!(worker.state.committed_height(), 2);
    });
}

#[test]
fn preparation_pins_original_even_against_discard_replacement_and_another_valid_quorum() {
    with_worker(|chain, worker, blocks, _| {
        let (block, qc) = executed(chain, worker);
        let overlay = original_overlay(worker);
        worker.prepare(&block, &qc).unwrap();
        let original = worker.context.staging.get(&qc.block_hash).unwrap();
        worker.discard(2, &[]);
        assert_eq!(original_overlay(worker), overlay);
        let mut competing_header = block.header().clone();
        competing_header.origin_view += 1;
        let competing = chain.author_payload(competing_header, block.payload().as_slice().to_vec());
        let hash = competing.hash(&**worker.context.crypto.as_ref().unwrap());
        assert!(matches!(
            worker.execute(&competing, hash),
            Some(ExecOutcome::Failed(_))
        ));
        assert_eq!(original_overlay(worker), overlay);
        let alternate = chain.commit_qc(2, qc.block_hash, qc.result, false, Signers::LastThree);
        let committee = worker
            .scheduled(2)
            .unwrap()
            .height_config()
            .unwrap()
            .committee;
        iroha_sumeragi::crypto::Verifier::new(
            &**worker.context.crypto.as_ref().unwrap(),
            &chain.instance(),
            &block.header().epoch,
            &committee,
        )
        .verify_qc(&NoAttestation, &alternate)
        .unwrap();
        assert!(worker.prepare(&block, &alternate).is_err());
        assert!(worker.commit(&block, &alternate).is_err());
        assert!(Arc::ptr_eq(
            &worker.context.staging.get(&qc.block_hash).unwrap(),
            &original
        ));
        blocks.append(&block, &qc).unwrap();
        let original_wire = chain
            .kura()
            .get_block(std::num::NonZeroUsize::new(2).unwrap())
            .unwrap()
            .encode_wire()
            .unwrap();
        assert!(
            blocks.append(&block, &alternate).is_err(),
            "durable retry cannot replace the prepared certificate"
        );
        blocks.append(&block, &qc).unwrap();
        assert_eq!(blocks.committed_body(2).unwrap().unwrap().1, qc);
        assert_eq!(
            chain
                .kura()
                .get_block(std::num::NonZeroUsize::new(2).unwrap())
                .unwrap()
                .encode_wire()
                .unwrap(),
            original_wire,
            "refusal and exact retry retain the complete original durable frame"
        );
        worker.commit(&block, &qc).unwrap();
    });
}

#[test]
fn mismatching_certified_result_reports_original_without_reexecuting_or_pinning_it() {
    with_worker(|chain, worker, _, _| {
        let (block, qc) = executed(chain, worker);
        let overlay = original_overlay(worker);
        let other_result =
            chain.commit_qc(2, qc.block_hash, Hash32([0x71; 32]), false, Signers::Quorum);
        assert_eq!(
            worker.prepare(&block, &other_result).unwrap(),
            Some(qc.result)
        );
        assert_eq!(original_overlay(worker), overlay);
        assert!(worker.context.staging.get(&qc.block_hash).is_none());
        assert!(matches!(
            worker.live.as_ref().unwrap().phase,
            PublicationPhase::Executed { .. }
        ));
        worker.prepare(&block, &qc).unwrap();
        assert_eq!(original_overlay(worker), overlay);
    });
}

#[test]
fn consuming_publication_failure_blocks_every_reexecution_path() {
    with_worker(|chain, worker, blocks, events| {
        let (block, qc) = executed(chain, worker);
        worker.prepare(&block, &qc).unwrap();
        blocks.append(&block, &qc).unwrap();
        let original = worker.context.staging.get(&qc.block_hash).unwrap();
        let mut consumed = 0;
        let failure = worker
            .commit_with(&block, &qc, |overlay| {
                consumed += 1;
                StateBlock::fail_publication_after_preparation_for_test();
                overlay.try_publish()
            })
            .unwrap_err();
        assert_eq!(consumed, 1);
        assert!(matches!(failure, PublicationError::RecoveryRequired(_)));
        assert!(
            worker.live.as_ref().unwrap().overlay.is_some(),
            "terminal recovery retains its original allocation"
        );
        assert_eq!(worker.state.view().height(), 1);
        assert!(worker.prepare(&block, &qc).is_err());
        assert!(worker.commit(&block, &qc).is_err());
        assert!(matches!(
            worker.execute(&block, qc.block_hash),
            Some(ExecOutcome::Failed(_))
        ));
        worker.discard(2, &[]);
        worker.reject(2, 0, qc.block_hash);
        assert!(matches!(
            worker.build(2, 0, 1024),
            Err(PublicationError::RecoveryRequired(_))
        ));
        assert!(Arc::ptr_eq(
            &worker.context.staging.get(&qc.block_hash).unwrap(),
            &original
        ));
        assert!(events.try_recv().is_err());
    });
}

#[test]
fn panic_after_actual_visibility_is_local_recovery_never_success_or_reexecution() {
    with_worker(|chain, worker, blocks, events| {
        let (block, qc) = executed(chain, worker);
        worker.prepare(&block, &qc).unwrap();
        blocks.append(&block, &qc).unwrap();
        let result = worker.commit_with(&block, &qc, |overlay| {
            assert!(
                matches!(
                    overlay.try_publish(),
                    crate::state::StatePublicationOutcome::Published
                ),
                "actual State publication succeeds"
            );
            panic!("injected failure after State visibility");
        });
        assert!(matches!(result, Err(PublicationError::RecoveryRequired(_))));
        assert_eq!(
            worker.state.view().height(),
            2,
            "the failure happened after real visibility"
        );
        assert_eq!(
            worker.applied.0, 1,
            "no successful completion was advertised"
        );
        assert!(worker.prepare(&block, &qc).is_err());
        assert!(worker.commit(&block, &qc).is_err());
        assert!(matches!(
            worker.execute(&block, qc.block_hash),
            Some(ExecOutcome::Failed(_))
        ));
        let (reply, response) = mpsc::sync_channel(1);
        worker.serve(Request::Commit(block, qc, reply));
        assert!(
            response.recv().unwrap().is_err(),
            "actual request dispatcher retains recovery"
        );
        assert!(events.try_recv().is_err());
    });
}

/// The actual original State owner reports its consuming failure through the production kernel.
#[test]
fn original_worker_consuming_failure_halts_driver_status_without_reexecution() {
    use crate::sumeragi::driver::{
        Completion, DriverConfig, Kernel, KernelStart, Op,
        exec::{ExecDone, ExecOp},
        ingress::{Ingress, IngressLimits},
    };
    use iroha_sumeragi::{
        api::{Action, CommittedTip, Event, HaltReason, Init, LocalParams},
        crypto::Attestation,
    };
    use parking_lot::Mutex;

    with_worker(|chain, worker, blocks, _| {
        let (block, qc) = executed(chain, worker);
        let original = original_overlay(worker);
        let (_, _, genesis) = startup::stored_genesis(chain.state()).unwrap().unwrap();
        let make_crypto = || {
            let crypto = BlsCrypto::new();
            crypto
                .admit_committee(
                    chain
                        .validators()
                        .iter()
                        .map(|(peer, pop)| (peer.public_key(), pop.as_slice())),
                )
                .unwrap();
            Box::new(crypto)
        };
        let (mut kernel, _) = Kernel::start(KernelStart {
            allocation_budget: chain.state().ivm_execution_budget(),
            local: LocalParams::default(),
            init: Init {
                instance: chain.instance(),
                records: Vec::new(),
                genesis_height: 1,
                demotion_window: 128,
                nonce: 1,
                tip: CommittedTip {
                    height: 1,
                    block_hash: genesis.block_hash,
                    result: genesis.result,
                    header: None,
                    commit_qc: None,
                },
                configs: vec![
                    (
                        2,
                        iroha_sumeragi::types::ConfigSlot::Ready(
                            worker.scheduled(2).unwrap().height_config().unwrap(),
                        ),
                    ),
                    (
                        3,
                        iroha_sumeragi::types::ConfigSlot::Ready(
                            worker.scheduled(3).unwrap().height_config().unwrap(),
                        ),
                    ),
                ],
                recent_headers: Vec::new(),
            },
            signers: Vec::new(),
            crypto: make_crypto(),
            hasher: make_crypto(),
            attestation: Attestation::none(),
            now: 0,
            ingress: Arc::new(Mutex::new(Ingress::new(IngressLimits::default()))),
            config: DriverConfig::default(),
        })
        .unwrap();
        kernel.route(vec![Action::CommitBlock {
            block: block.clone(),
            commit_qc: qc.clone(),
        }]);
        assert!(
            kernel
                .poll(0)
                .iter()
                .any(|op| matches!(op, Op::Exec(ExecOp::Prepare(_))))
        );
        kernel.complete(
            0,
            Completion::Exec(ExecDone::Prepared(worker.prepare(&block, &qc))),
        );
        assert_eq!(original_overlay(worker), original);
        assert!(
            kernel
                .poll(0)
                .iter()
                .any(|op| matches!(op, Op::Exec(ExecOp::Append(_))))
        );
        blocks.append(&block, &qc).unwrap();
        kernel.complete(
            0,
            Completion::Exec(ExecDone::Appended {
                durable: true,
                deferred: None,
            }),
        );
        assert!(
            kernel
                .poll(0)
                .iter()
                .any(|op| matches!(op, Op::Exec(ExecOp::Commit(_))))
        );
        let result = worker.commit_with(&block, &qc, |overlay| {
            StateBlock::fail_publication_after_preparation_for_test();
            overlay.try_publish()
        });
        assert!(matches!(result, Err(PublicationError::RecoveryRequired(_))));
        kernel.complete(
            0,
            Completion::Exec(ExecDone::Committed(result.map(Box::new))),
        );
        let halt = HaltReason::PublicationRecoveryRequired { height: 2 };
        assert_eq!(kernel.core().status().halted, Some(halt));
        assert_eq!(kernel.exec().halted(), Some(halt));
        assert_eq!(kernel.exec().applied(), 1);
        assert_eq!(worker.state.view().height(), 1);
        assert!(
            worker.live.as_ref().unwrap().overlay.is_some(),
            "terminal recovery retains its original allocation"
        );
        kernel.route(vec![Action::Execute {
            block: block.clone(),
            req: 88,
        }]);
        for now in [1, 1000, 10_000] {
            kernel.handle(now, Event::Tick);
            assert!(
                !kernel
                    .poll(now)
                    .iter()
                    .any(|op| matches!(op, Op::Exec(_) | Op::Send { .. }))
            );
        }
        assert!(matches!(
            worker.prepare(&block, &qc),
            Err(PublicationError::RecoveryRequired(_))
        ));
        assert_eq!(
            original_overlay(worker),
            original,
            "no reconstructed execution owner"
        );
    });
}

#[test]
fn state_busy_retry_retains_original_metadata_snapshot_certificate_and_pool_custody() {
    with_worker(|chain, worker, blocks, events| {
        let (block, qc) = executed(chain, worker);
        let overlay = original_overlay(worker);
        let witness = iroha_crypto::HashOf::new(&worker.live.as_ref().unwrap().witness);
        worker.prepare(&block, &qc).unwrap();
        blocks.append(&block, &qc).unwrap();
        let staged = worker.context.staging.get(&qc.block_hash).unwrap();
        let cert = staged.executed.commit_certificate().unwrap();
        let certificate_bytes = staged.executed.encode_wire().unwrap();
        let dir = tempfile::tempdir().unwrap();
        chain
            .state()
            .enable_publication_snapshot_for_test(dir.path());
        State::observe_publication_captures_for_test(|captures| {
            let mut phase_identity = None;
            let mut occupancy = None;
            let mut metadata_identity = None;
            for attempt in 0..4 {
                let mut attempt_commit = || {
                    let result = worker.commit(&block, &qc);
                    assert!(
                        matches!(result, Err(PublicationError::Retryable(_))),
                        "{result:?}"
                    );
                };
                if attempt % 2 == 0 {
                    chain
                        .state()
                        .with_publication_blocked_for_test(&mut attempt_commit);
                } else {
                    chain
                        .state()
                        .with_hash_publication_blocked_for_test(&mut attempt_commit);
                }
                assert!(worker.recovery.is_none());
                assert_eq!(original_overlay(worker), overlay);
                assert_eq!(worker.applied.0, 1);
                assert_eq!(chain.state().committed_height(), 1);
                assert_eq!(
                    captures.counts(),
                    (1, 0),
                    "capture exactly once and retain on refusal"
                );
                assert!(events.try_recv().is_err());
                let live = worker.live.as_ref().unwrap();
                assert_eq!(iroha_crypto::HashOf::new(&live.witness), witness);
                let PublicationPhase::Prepared {
                    state_events: Some(tail),
                    ..
                } = &live.phase
                else {
                    panic!("the original metadata finalization survives State refusal")
                };
                let metadata = (tail.as_ptr() as usize, tail.len(), tail.capacity());
                assert_eq!(*metadata_identity.get_or_insert(metadata), metadata);
                let identity = live
                    .overlay
                    .as_ref()
                    .unwrap()
                    .publication_identity_for_test();
                assert_ne!(identity.0, 0);
                assert_ne!(identity.1, 0);
                assert!(identity.2);
                assert_eq!(*phase_identity.get_or_insert(identity), identity);
                let used = chain.state().publication_pool_usage_for_test();
                assert_eq!(
                    *occupancy.get_or_insert(used),
                    used,
                    "same original pool allocations across retry"
                );
                let retry = worker.context.staging.get(&qc.block_hash).unwrap();
                assert!(Arc::ptr_eq(&retry, &staged));
                assert!(std::ptr::eq(
                    retry.executed.commit_certificate().unwrap(),
                    cert
                ));
                assert_eq!(retry.executed.encode_wire().unwrap(), certificate_bytes);
                assert_eq!(worker.prepare(&block, &qc).unwrap(), Some(qc.result));
            }
            worker.commit(&block, &qc).unwrap();
            assert_eq!(worker.applied.0, 2);
            assert_eq!(chain.state().committed_height(), 2);
            assert_eq!(captures.counts(), (1, 1));
            let mut delivered = 0;
            while events.try_recv().is_ok() {
                delivered += 1;
            }
            assert!(delivered > 0);
            worker.commit(&block, &qc).unwrap();
            assert!(
                events.try_recv().is_err(),
                "successful repeat emits no second effect"
            );
            assert_eq!(captures.counts(), (1, 1));
            assert!(
                Arc::ptr_eq(
                    &worker.context.staging.get(&qc.block_hash).unwrap().executed,
                    &staged.executed
                ),
                "Kura handoff still uses the same retained certified source"
            );
        });
    });
}

#[test]
fn context_proof_capacity_retry_retains_original_witness_inputs_and_execution() {
    with_worker(|chain, worker, blocks, events| {
        let block = proposal(chain, worker);
        let block_hash = block.hash(&**worker.context.crypto.as_ref().unwrap());
        let mut occupied = None;
        let outcome = worker.run_execution_with_finisher(&block, block_hash, |worker| {
            assert!(matches!(
                worker.finishing.as_ref().unwrap().phase,
                FinishingPhase::ContextProof { .. }
            ));
            let budget = worker.state.ivm_execution_budget();
            occupied = Some(
                budget
                    .try_reserve_bytes(budget.limit_bytes() - budget.reserved_bytes())
                    .unwrap(),
            );
            worker.finish_execution_with_encoder(encode_result_preimage)
        });
        assert!(matches!(outcome, ExecOutcome::Failed(_)));
        assert!(worker.recovery.is_none());
        assert!(worker.live.is_none());
        assert!(!worker.results.contains_key(&block_hash));
        let pending = worker
            .finishing
            .as_ref()
            .expect("same original completed execution");
        let FinishingPhase::ContextProof { inputs, refusal } = &pending.phase else {
            panic!("proof scratch refused before consuming original inputs")
        };
        assert!(matches!(
            refusal,
            Some(NativeLaneStateProofError::Scratch(
                iroha_allocation::ChargedBufferError::Admission(
                    iroha_allocation::AllocationRefusal::Capacity { .. }
                )
            ))
        ));
        let overlay = std::ptr::from_ref(pending.overlay.as_ref());
        let authority = inputs.get().schedule.current.committee.as_ptr();
        let writes = pending.witness.writes.as_ptr();
        let witness = iroha_crypto::HashOf::new(&pending.witness);
        let event_count = pending.events.len();
        let budget = worker.state.ivm_execution_budget();
        let retained_bytes = budget.reserved_bytes();
        for _ in 0..2 {
            assert!(matches!(
                worker.execute(&block, block_hash),
                Some(ExecOutcome::Failed(_))
            ));
            let pending = worker.finishing.as_ref().unwrap();
            let FinishingPhase::ContextProof { inputs, refusal } = &pending.phase else {
                panic!("original proof frontier retained")
            };
            assert!(refusal.as_ref().unwrap().is_local_refusal());
            assert_eq!(std::ptr::from_ref(pending.overlay.as_ref()), overlay);
            assert_eq!(inputs.get().schedule.current.committee.as_ptr(), authority);
            assert_eq!(pending.witness.writes.as_ptr(), writes);
            assert_eq!(iroha_crypto::HashOf::new(&pending.witness), witness);
            assert_eq!(pending.events.len(), event_count);
            assert_eq!(budget.reserved_bytes(), retained_bytes);
            assert!(!worker.results.contains_key(&block_hash));
        }
        assert!(events.try_recv().is_err());
        drop(occupied);
        let Some(ExecOutcome::Valid(result)) = worker.execute(&block, block_hash) else {
            panic!("same original execution completes after real pool release")
        };
        assert!(worker.finishing.is_none());
        let live = worker.live.as_ref().unwrap();
        assert_eq!(
            std::ptr::from_ref(live.overlay.as_deref().unwrap()),
            overlay
        );
        assert_eq!(
            live.commitment.get().schedule.current.committee.as_ptr(),
            authority
        );
        assert_eq!(live.witness.writes.as_ptr(), writes);
        assert_eq!(iroha_crypto::HashOf::new(&live.witness), witness);
        assert_eq!(live.events.len(), event_count);
        let qc = chain.commit_qc(2, block_hash, result, false, Signers::Quorum);
        assert_eq!(worker.prepare(&block, &qc).unwrap(), Some(result));
        blocks.append(&block, &qc).unwrap();
        worker.commit(&block, &qc).unwrap();
        assert_eq!(worker.state.view().height(), 2);
    });
}

#[test]
fn result_encoding_capacity_retry_keeps_original_execution_and_allocation_custody() {
    with_worker(|chain, worker, blocks, events| {
        let block = proposal(chain, worker);
        let block_hash = block.hash(&**worker.context.crypto.as_ref().unwrap());
        let mut occupied = None;
        let outcome = worker.run_execution_with_encoder(&block, block_hash, |original, budget| {
            let remaining = budget
                .limit_bytes()
                .checked_sub(budget.reserved_bytes())
                .unwrap();
            occupied = Some(
                budget
                    .try_reserve_bytes(remaining)
                    .expect("occupy real original pool"),
            );
            encode_result_preimage(original, budget)
        });
        assert!(matches!(outcome, ExecOutcome::Failed(_)));
        assert!(worker.recovery.is_none());
        assert!(worker.live.is_none());
        assert!(!worker.results.contains_key(&block_hash));
        let pending = worker
            .finishing
            .as_ref()
            .expect("same completed execution is retained");
        assert!(matches!(
            pending.encoding_refusal,
            Some(
                crate::sumeragi::commitment::ResultPreimageError::Allocation(
                    iroha_allocation::ChargedBufferError::Admission(
                        iroha_allocation::AllocationRefusal::Capacity { .. }
                    )
                )
            )
        ));
        let overlay = std::ptr::from_ref(pending.overlay.as_ref());
        let authority = pending
            .phase
            .ready()
            .unwrap()
            .get()
            .schedule
            .current
            .committee
            .as_ptr();
        let witness = iroha_crypto::HashOf::new(&pending.witness);
        let event_count = pending.events.len();
        for _ in 0..2 {
            assert!(matches!(
                worker.execute(&block, block_hash),
                Some(ExecOutcome::Failed(_))
            ));
            let pending = worker.finishing.as_ref().unwrap();
            assert_eq!(std::ptr::from_ref(pending.overlay.as_ref()), overlay);
            assert_eq!(
                pending
                    .phase
                    .ready()
                    .unwrap()
                    .get()
                    .schedule
                    .current
                    .committee
                    .as_ptr(),
                authority
            );
            assert_eq!(iroha_crypto::HashOf::new(&pending.witness), witness);
            assert_eq!(pending.events.len(), event_count);
            assert!(!worker.results.contains_key(&block_hash));
        }
        assert!(events.try_recv().is_err());
        drop(occupied);
        let Some(ExecOutcome::Valid(result)) = worker.execute(&block, block_hash) else {
            panic!("release permits encoding of the same original execution");
        };
        assert!(worker.finishing.is_none());
        let live = worker.live.as_ref().unwrap();
        assert_eq!(
            std::ptr::from_ref(live.overlay.as_deref().unwrap()),
            overlay
        );
        assert_eq!(
            live.commitment.get().schedule.current.committee.as_ptr(),
            authority
        );
        assert_eq!(iroha_crypto::HashOf::new(&live.witness), witness);
        assert_eq!(live.events.len(), event_count);
        let qc = chain.commit_qc(2, block_hash, result, false, Signers::Quorum);
        assert_eq!(worker.prepare(&block, &qc).unwrap(), Some(result));
        blocks.append(&block, &qc).unwrap();
        worker.commit(&block, &qc).unwrap();
        assert_eq!(worker.state.view().height(), 2);
    });
}

#[test]
fn result_encoding_foreign_pool_requires_recovery_and_retains_original_execution() {
    with_worker(|chain, worker, _, _| {
        let block = proposal(chain, worker);
        let block_hash = block.hash(&**worker.context.crypto.as_ref().unwrap());
        let outcome = worker.run_execution_with_encoder(&block, block_hash, |original, budget| {
            let foreign = iroha_allocation::AllocationBudget::new(budget.limit_bytes());
            encode_result_preimage(original, &foreign)
        });
        assert!(matches!(outcome, ExecOutcome::Failed(_)));
        assert!(worker.recovery.is_some());
        let pending = worker
            .finishing
            .as_ref()
            .expect("original retained for recovery");
        assert!(matches!(
            pending.encoding_refusal,
            Some(crate::sumeragi::commitment::ResultPreimageError::ForeignBudget)
        ));
        let overlay = std::ptr::from_ref(pending.overlay.as_ref());
        assert!(matches!(
            worker.execute(&block, block_hash),
            Some(ExecOutcome::Failed(_))
        ));
        assert_eq!(
            std::ptr::from_ref(worker.finishing.as_ref().unwrap().overlay.as_ref()),
            overlay
        );
        assert!(worker.live.is_none());
        assert!(!worker.results.contains_key(&block_hash));
    });
}

fn assert_certificate_allocation_retry(occupy_after_part: usize) {
    with_worker(move |chain, worker, blocks, events| {
        let (block, qc) = executed(chain, worker);
        let overlay = original_overlay(worker);
        let live = worker.live.as_ref().unwrap();
        let witness = iroha_crypto::HashOf::new(&live.witness);
        let authority = live.commitment.get().schedule.current.committee.as_ptr();
        let preimage = match &live.phase {
            PublicationPhase::Executed { preimage, .. } => preimage.as_slice().as_ptr(),
            _ => panic!("original executed phase"),
        };
        let event_count = live.events.len();
        let mut occupied = None;
        let mut encoded = 0;
        let mut header_pointer = None;
        let mut qc_pointer = None;
        let mut availability_pointer = None;
        let outcome = worker.prepare_with_encoder(
            &block,
            &qc,
            CommitTelemetryOrigin::Forward,
            |part, budget| {
                let bytes = crate::sumeragi::commitment::encode_certificate_part(
                    part,
                    budget,
                    crate::sumeragi::commitment::MAX_RESULT_PREIMAGE_BYTES,
                )?;
                encoded += 1;
                if encoded == 1 {
                    header_pointer = Some(bytes.as_slice().as_ptr());
                }
                if encoded == 2 {
                    qc_pointer = Some(bytes.as_slice().as_ptr());
                }
                if encoded == 3 {
                    availability_pointer = Some(bytes.as_slice().as_ptr());
                }
                if encoded == occupy_after_part {
                    occupied = Some(
                        budget
                            .try_reserve_bytes(
                                budget
                                    .limit_bytes()
                                    .checked_sub(budget.reserved_bytes())
                                    .unwrap(),
                            )
                            .expect("occupy actual remaining original capacity"),
                    );
                }
                Ok(bytes)
            },
        );
        assert!(matches!(outcome, Err(PublicationError::Retryable(_))));
        assert_eq!(encoded, occupy_after_part);
        for _ in 0..2 {
            assert!(worker.recovery.is_none());
            assert_eq!(original_overlay(worker), overlay);
            let live = worker.live.as_ref().unwrap();
            assert_eq!(iroha_crypto::HashOf::new(&live.witness), witness);
            assert_eq!(
                live.commitment.get().schedule.current.committee.as_ptr(),
                authority
            );
            assert_eq!(live.events.len(), event_count);
            match &live.phase {
                PublicationPhase::EncodingCertificate {
                    preimage: original,
                    header_wire: Some(header),
                    qc_wire: None,
                    refusal:
                        Some(crate::sumeragi::commitment::ResultPreimageError::Allocation(
                            iroha_allocation::ChargedBufferError::Admission(
                                iroha_allocation::AllocationRefusal::Capacity { .. },
                            ),
                        )),
                    ..
                } if occupy_after_part == 1 => {
                    assert_eq!(original.as_slice().as_ptr(), preimage);
                    assert_eq!(Some(header.as_slice().as_ptr()), header_pointer);
                }
                PublicationPhase::EncodingCertificate {
                    preimage: original,
                    header_wire: Some(header),
                    qc_wire: Some(original_qc),
                    availability_wire: None,
                    refusal:
                        Some(crate::sumeragi::commitment::ResultPreimageError::Allocation(
                            iroha_allocation::ChargedBufferError::Admission(
                                iroha_allocation::AllocationRefusal::Capacity { .. },
                            ),
                        )),
                    ..
                } if occupy_after_part == 2 => {
                    assert_eq!(original.as_slice().as_ptr(), preimage);
                    assert_eq!(Some(header.as_slice().as_ptr()), header_pointer);
                    assert_eq!(Some(original_qc.as_slice().as_ptr()), qc_pointer);
                }
                PublicationPhase::Certifying {
                    parts: Some(parts),
                    refusal:
                        Some(iroha_data_model::block::CertificateAdmissionError::ControlAdmission(
                            iroha_allocation::AllocationRefusal::Capacity { .. },
                        )),
                    ..
                } if occupy_after_part == 3 => {
                    assert_eq!(
                        Some(parts.availability.as_slice().as_ptr()),
                        availability_pointer
                    );
                    assert_eq!(parts.result_preimage.as_slice().as_ptr(), preimage);
                    assert_eq!(
                        Some(parts.consensus_header.as_slice().as_ptr()),
                        header_pointer
                    );
                    assert_eq!(Some(parts.commit_qc.as_slice().as_ptr()), qc_pointer);
                }
                _ => panic!("exact original allocation refusal must survive"),
            }
            assert!(worker.context.staging.get(&qc.block_hash).is_none());
            assert!(events.try_recv().is_err());
            assert!(matches!(
                worker.prepare(&block, &qc),
                Err(PublicationError::Retryable(_))
            ));
        }
        let alternate = chain.commit_qc(2, qc.block_hash, qc.result, false, Signers::LastThree);
        assert!(
            worker.prepare(&block, &alternate).is_err(),
            "frozen exact QC cannot be replaced"
        );
        drop(occupied);
        assert_eq!(worker.prepare(&block, &qc).unwrap(), Some(qc.result));
        let staged = worker.context.staging.get(&qc.block_hash).unwrap();
        let certificate = staged.executed.commit_certificate().unwrap();
        assert!(certificate.admitted_to(&worker.state.ivm_execution_budget()));
        assert_eq!(certificate.result_preimage().as_ptr(), preimage);
        assert_eq!(
            Some(certificate.consensus_header().as_ptr()),
            header_pointer
        );
        if let Some(original_qc) = qc_pointer {
            assert_eq!(certificate.commit_qc().as_ptr(), original_qc);
        }
        if let Some(original_availability) = availability_pointer {
            assert_eq!(certificate.availability().as_ptr(), original_availability);
        }
        let decoded_frame: iroha_sumeragi::availability::AvailabilityFrame =
            norito::decode_canonical(certificate.availability()).unwrap();
        assert_eq!(&decoded_frame, block.availability());
        assert_eq!(original_overlay(worker), overlay);
        blocks.append(&block, &qc).unwrap();
        worker.commit(&block, &qc).unwrap();
        assert_eq!(worker.state.view().height(), 2);
    });
}

#[test]
fn qc_encoding_refusal_retains_original_header_result_and_execution_until_capacity_returns() {
    assert_certificate_allocation_retry(1);
}

#[test]
fn certificate_control_refusal_retains_every_original_part_through_publication_retry() {
    assert_certificate_allocation_retry(3);
}

/// Mandatory availability encoding retains both earlier encodings and the execution owner.
#[test]
fn availability_encoding_refusal_retains_original_header_qc_and_execution() {
    assert_certificate_allocation_retry(2);
}

#[test]
fn native_control_is_attached_once_and_remote_refusal_preserves_the_original_owner() {
    with_worker(|chain, worker, _, _| {
        let block = proposal(chain, worker);
        let source = ApplicationControlContext {
            instance: block.header().instance,
            epoch: block.header().epoch,
            height: block.header().height,
            parent_hash: block.header().parent_hash,
            parent_result: block.header().parent_result,
        };
        let build = ControlWitnessContext {
            height: source.height,
            view: block.header().origin_view,
            epoch: source.epoch,
            parent_hash: source.parent_hash,
            parent_result: source.parent_result,
        };
        assert!(matches!(
            worker.drive_control(&source),
            Err(PublicationError::RecoveryRequired(_))
        ));
        worker.attach_beacon(chain.instance(), None, None).unwrap();
        assert!(matches!(
            worker.build_control_witness(&build),
            Err(PublicationError::Retryable(_))
        ));
        assert!(worker.drive_control(&source).unwrap().is_none());
        let original = worker.build_control_witness(&build).unwrap();
        assert!(original.0.is_empty());
        assert!(!original.1);
        assert!(matches!(
            worker.attach_beacon(Hash32([99; 32]), None, None),
            Err(PublicationError::RecoveryRequired(_))
        ));
        // A failed replacement leaves the old actual instance functional.
        assert!(worker.drive_control(&source).unwrap().is_none());
        let sender = schedule::consensus_key(&chain.validators()[0].0).unwrap();
        worker
            .receive_application_control(
                &sender,
                &ApplicationControl {
                    context: source,
                    bytes: ControlWitness::try_from_slice(&[1, 2, 3]).unwrap(),
                },
            )
            .unwrap();
        assert!(worker.recovery.is_none());
        assert_eq!(worker.build_control_witness(&build).unwrap(), original);
        let mut stale = source;
        stale.parent_result = Hash32([98; 32]);
        worker
            .receive_application_control(
                &sender,
                &ApplicationControl {
                    context: stale,
                    bytes: ControlWitness::empty(),
                },
            )
            .unwrap();
        assert_eq!(worker.build_control_witness(&build).unwrap(), original);
    });
}

#[test]
fn native_control_never_displaces_the_original_prepared_publication() {
    with_worker(|chain, worker, _, _| {
        worker.attach_beacon(chain.instance(), None, None).unwrap();
        let (block, qc) = executed(chain, worker);
        let original = original_overlay(worker);
        let source = ApplicationControlContext {
            instance: block.header().instance,
            epoch: block.header().epoch,
            height: block.header().height,
            parent_hash: block.header().parent_hash,
            parent_result: block.header().parent_result,
        };
        assert!(
            worker.drive_control(&source).unwrap().is_none(),
            "committed reads remain available while the original speculative overlay is retained"
        );
        assert_eq!(original_overlay(worker), original);
        assert_eq!(worker.prepare(&block, &qc).unwrap(), Some(qc.result));

        assert!(matches!(
            worker.drive_control(&source),
            Err(PublicationError::Retryable(_))
        ));
        assert_eq!(original_overlay(worker), original);
        assert_eq!(worker.prepare(&block, &qc).unwrap(), Some(qc.result));
    });
}

#[test]
fn quarantine_requires_the_exact_control_free_transaction_rejection_hash() {
    with_worker(|chain, worker, _, _| {
        let block = proposal(chain, worker);
        let crypto = worker.context.crypto.as_ref().unwrap();
        let hash = block.hash(&**crypto);
        worker.last_built = Some((2, 0, Vec::new()));
        worker.reject(2, 0, hash);
        assert!(
            worker.last_built.is_some(),
            "no transaction verdict authorizes queue isolation"
        );
        worker.quarantine_context = Some(QuarantineContext {
            height: 2,
            view: 0,
            block_hash: hash,
            pulse_context: control::pulse_context(block.header()),
        });
        worker.reject(2, 0, Hash32([201; 32]));
        assert!(
            worker.last_built.is_some(),
            "another proposal cannot consume the original queue selection"
        );
        let mut controlled = block.header().clone();
        controlled.control_witness = ControlWitness::try_from_slice(&[1, 2, 3]).unwrap();
        let block = chain.author_payload(controlled, block.payload().as_slice().to_vec());
        let hash = block.hash(&**worker.context.crypto.as_ref().unwrap());
        assert!(matches!(
            worker.execute(&block, hash),
            Some(ExecOutcome::Invalid)
        ));
        assert!(
            worker.quarantine_context.is_none(),
            "malformed control never authorizes transaction blame"
        );
        worker.reject(2, 0, hash);
        assert!(worker.last_built.is_some());
    });
}

#[test]
fn native_header_source_is_checked_against_the_pristine_committed_parent() {
    with_worker(|chain, worker, _, _| {
        for changed in 0..3 {
            let block = proposal(chain, worker);
            let mut header = block.header().clone();
            let mut foreign_context = block.source().config().clone();
            match changed {
                0 => header.instance = Hash32([201; 32]),
                1 => header.parent_result = Hash32([202; 32]),
                _ => {
                    header.epoch.context = Hash32([203; 32]);
                    foreign_context.epoch.id = header.epoch;
                }
            }
            let changed_body = chain
                .author_payload_under_test_context(
                    header,
                    block.payload().as_slice().to_vec(),
                    &foreign_context,
                )
                .unwrap_or_else(|(_, error)| panic!("genuinely signed altered context: {error:?}"));
            assert_ne!(changed_body.header(), block.header());
            if changed != 1 {
                assert_ne!(changed_body.source(), block.source());
            }
            let block = changed_body;
            let hash = block.hash(&**worker.context.crypto.as_ref().unwrap());
            assert!(matches!(
                worker.execute(&block, hash),
                Some(ExecOutcome::Invalid)
            ));
            assert!(worker.live.is_none());
            assert!(worker.finishing.is_none());
            assert_eq!(worker.applied.0, 1);
        }
        let (block, _) = executed(chain, worker);
        let expected = control::pulse_context(block.header());
        assert_eq!(expected.instance, chain.instance().0);
        assert_eq!(expected.parent_consensus_hash, block.header().parent_hash.0);
        assert_eq!(expected.parent_result, block.header().parent_result.0);
        assert_eq!(expected.epoch, block.header().epoch.epoch);
        assert_eq!(expected.epoch_context_id, block.header().epoch.context.0);
    });
}

#[test]
fn state_executor_serializes_real_control_requests_and_one_time_attachment() {
    with_worker(|chain, worker, _, _| {
        let block = proposal(chain, worker);
        let mut executor = StateExecutor::spawn(worker.context.clone()).unwrap();
        let source = ApplicationControlContext {
            instance: block.header().instance,
            epoch: block.header().epoch,
            height: block.header().height,
            parent_hash: block.header().parent_hash,
            parent_result: block.header().parent_result,
        };
        let build = ControlWitnessContext {
            height: source.height,
            view: block.header().origin_view,
            epoch: source.epoch,
            parent_hash: source.parent_hash,
            parent_result: source.parent_result,
        };
        executor
            .attach_beacon(chain.instance(), None, None)
            .unwrap();
        assert!(executor.drive_control(&source).unwrap().is_none());
        assert_eq!(
            executor.build_control_witness(&build).unwrap(),
            (ControlWitness::empty(), false)
        );
        let sender = schedule::consensus_key(&chain.validators()[0].0).unwrap();
        executor
            .receive_application_control(
                &sender,
                &ApplicationControl {
                    context: source,
                    bytes: ControlWitness::try_from_slice(&[1]).unwrap(),
                },
            )
            .unwrap();
        assert!(
            executor
                .attach_beacon(Hash32([211; 32]), None, None)
                .is_err()
        );
        assert!(executor.drive_control(&source).unwrap().is_none());
        assert_eq!(
            executor.build_control_witness(&build).unwrap(),
            (ControlWitness::empty(), false)
        );
    });
}

#[test]
fn native_attestation_attachment_retains_the_first_original_pool_mailbox() {
    with_worker(|chain, worker, _, _| {
        use crate::sumeragi::attestation::{NativePastaVerifier, channel};
        let budget = worker.state.ivm_execution_budget();
        let key = crate::sumeragi::crypto::core_key(chain.validators()[0].0.public_key()).unwrap();
        let (attestor, publisher) = channel(chain.instance(), &key, false, &budget).unwrap();
        let original_key = std::ptr::from_ref(publisher.key());
        let verifier = NativePastaVerifier::new(chain.instance(), *worker.state.network_id_ref());
        worker
            .attach_attestation(verifier, None, publisher)
            .unwrap();
        assert_eq!(
            std::ptr::from_ref(worker.attestation.as_ref().unwrap().publisher.key()),
            original_key
        );
        let occupied = budget.reserved_bytes();
        let (other_attestor, other_publisher) =
            channel(chain.instance(), &key, false, &budget).unwrap();
        assert!(matches!(
            worker.attach_attestation(verifier, None, other_publisher),
            Err(PublicationError::RecoveryRequired(_))
        ));
        drop(other_attestor);
        assert_eq!(budget.reserved_bytes(), occupied);
        assert_eq!(
            std::ptr::from_ref(worker.attestation.as_ref().unwrap().publisher.key()),
            original_key
        );
        assert!(worker.recovery.is_none());
        drop(attestor);
    });
}

#[test]
fn native_prepare_checks_real_quorum_before_changing_original_publication_owners() {
    with_worker(|chain, worker, _, _| {
        let (block, qc) = executed(chain, worker);
        let original = original_overlay(worker);
        let commitment = std::ptr::from_ref(worker.live.as_ref().unwrap().commitment.get());
        let mut changed = qc.clone();
        changed.result = Hash32([0xA9; 32]);
        // The unchanged genuine signature cannot authenticate a different execution result.
        assert!(worker.prepare(&block, &changed).is_err());
        assert_eq!(original_overlay(worker), original);
        assert_eq!(
            std::ptr::from_ref(worker.live.as_ref().unwrap().commitment.get()),
            commitment
        );
        assert!(matches!(
            worker.live.as_ref().unwrap().phase,
            PublicationPhase::Executed { .. }
        ));
        assert!(worker.context.staging.get(&qc.block_hash).is_none());
        assert!(worker.recovery.is_none());
        assert_eq!(worker.prepare(&block, &qc).unwrap(), Some(qc.result));
        assert_eq!(original_overlay(worker), original);
    });
}

#[test]
fn native_pasta_refusals_retain_original_execution_until_actual_receipt_publication() {
    with_worker_from(
        CertifiedTestChain::npos_boundary_fixture,
        ConsensusMode::Npos,
        |chain, worker, blocks, _| {
            use crate::sumeragi::attestation::{NativePastaVerifier, channel};
            use iroha_sumeragi::crypto::{AttestOutcome, Attestor as _};
            use local_attestation::Progress;
            let budget = worker.state.ivm_execution_budget();
            let original_limit = budget.limit_bytes();
            let key =
                crate::sumeragi::crypto::core_key(chain.validators()[0].0.public_key()).unwrap();
            let (attestor, publisher) = channel(chain.instance(), &key, true, &budget).unwrap();
            worker
                .attach_attestation(
                    NativePastaVerifier::new(chain.instance(), chain.network_id()),
                    Some(Arc::new(chain.pasta_custody(0))),
                    publisher,
                )
                .unwrap();
            let block = proposal(chain, worker);
            assert_eq!(block.header().height, 10);
            assert!(block.header().attest);
            let block_hash = block.hash(&**worker.context.crypto.as_ref().unwrap());
            let outcome =
                worker.run_execution_with_encoder(&block, block_hash, |original, source| {
                    let bytes = encode_result_preimage(original, source).unwrap();
                    // Refuse the actual next witness backing after the original R/preimage exist.
                    source.set_limit_bytes(source.reserved_bytes());
                    Ok(bytes)
                });
            assert!(matches!(outcome, ExecOutcome::Failed(_)));
            let live = worker.live.as_ref().unwrap();
            assert!(matches!(
                live.attestation,
                Progress::WaitingBacking(Some(_))
            ));
            let original_overlay_pointer = original_overlay(worker);
            let original_result_pointer = std::ptr::from_ref(live.commitment.get());
            let PublicationPhase::Executed { preimage, .. } = &live.phase else {
                panic!("same original execution must remain before certificate preparation")
            };
            let original_preimage_pointer = preimage.as_slice().as_ptr();
            let witness_len = preimage.as_slice().len();
            let result = live.result;
            let statement = iroha_sumeragi::preimage::att_preimage(
                &chain.instance(),
                &block.header().epoch,
                10,
                &block_hash,
                &result,
            );
            assert!(matches!(
                attestor.attest(10, &key, &statement),
                AttestOutcome::Pending
            ));
            assert!(worker.context.staging.get(&block_hash).is_none());

            // Admit exactly the backing bytes and refuse the genuine shared-control layout.
            budget.set_limit_bytes(budget.reserved_bytes().checked_add(witness_len).unwrap());
            assert!(matches!(
                worker.execute(&block, block_hash),
                Some(ExecOutcome::Failed(_))
            ));
            let Progress::WaitingControl {
                bytes,
                refusal: Some(_),
            } = &worker.live.as_ref().unwrap().attestation
            else {
                panic!("same original copied witness must await actual control admission")
            };
            let witness_pointer = bytes.as_slice().as_ptr();
            for _ in 0..2 {
                assert!(matches!(
                    worker.execute(&block, block_hash),
                    Some(ExecOutcome::Failed(_))
                ));
                let Progress::WaitingControl {
                    bytes,
                    refusal: Some(_),
                } = &worker.live.as_ref().unwrap().attestation
                else {
                    panic!("retained backing")
                };
                assert_eq!(bytes.as_slice().as_ptr(), witness_pointer);
                assert_eq!(original_overlay(worker), original_overlay_pointer);
            }

            budget.set_limit_bytes(original_limit);
            let qc = attestor.with_locked_receipt_for_test(|| {
                assert!(matches!(
                    worker.execute(&block, block_hash),
                    Some(ExecOutcome::Failed(_))
                ));
                let Progress::Publishing(receipt) = &worker.live.as_ref().unwrap().attestation
                else {
                    panic!("actual signature must survive real mailbox contention")
                };
                assert_eq!(
                    receipt.witness_for_test().as_slice().as_ptr(),
                    witness_pointer
                );
                let qc = chain.commit_qc_with_witness(
                    10,
                    block_hash,
                    result,
                    true,
                    Signers::LastThree,
                    Some(receipt.witness_for_test().clone()),
                );
                // An independently valid remote quorum cannot bypass this local publication cut.
                assert!(matches!(
                    worker.prepare(&block, &qc),
                    Err(PublicationError::Retryable(_))
                ));
                assert!(worker.commit(&block, &qc).is_err());
                assert!(worker.context.staging.get(&block_hash).is_none());
                let Progress::Publishing(receipt) = &worker.live.as_ref().unwrap().attestation
                else {
                    panic!("same actual receipt retained on every refused handoff")
                };
                assert_eq!(
                    receipt.witness_for_test().as_slice().as_ptr(),
                    witness_pointer
                );
                assert_eq!(original_overlay(worker), original_overlay_pointer);
                assert!(!matches!(
                    worker.live.as_ref().unwrap().phase,
                    PublicationPhase::Prepared { .. }
                ));
                qc
            });
            assert!(
                matches!(worker.execute(&block, block_hash), Some(ExecOutcome::Valid(value)) if value == result)
            );
            let AttestOutcome::Attested(share) = attestor.attest(10, &key, &statement) else {
                panic!("Valid requires the actual original receipt to be visible")
            };
            assert_eq!(share.witness.as_slice().as_ptr(), witness_pointer);
            assert!(share.witness.admitted_to(&budget));
            assert_eq!(
                std::ptr::from_ref(worker.live.as_ref().unwrap().commitment.get()),
                original_result_pointer
            );
            assert_eq!(original_overlay(worker), original_overlay_pointer);
            let PublicationPhase::Executed { preimage, .. } = &worker.live.as_ref().unwrap().phase
            else {
                panic!("same original preimage awaits certificate publication")
            };
            assert_eq!(preimage.as_slice().as_ptr(), original_preimage_pointer);
            let encoded = norito::encode_canonical(&qc).unwrap();
            let decoded: Qc = norito::decode_canonical(&encoded).unwrap();
            assert_eq!(
                decoded, qc,
                "genuine signatures and source survive decoding"
            );
            assert!(
                !decoded
                    .attestation_witness
                    .as_ref()
                    .unwrap()
                    .admitted_to(&budget)
            );
            assert!(require_qc_witness_admission(&decoded, &budget).is_err());
            assert!(worker.prepare(&block, &decoded).is_err());
            assert_eq!(original_overlay(worker), original_overlay_pointer);
            assert!(worker.context.staging.get(&block_hash).is_none());
            let foreign = iroha_allocation::AllocationBudget::new(1024 * 1024);
            let mut foreign_qc = decoded;
            foreign_qc.admit_attestation_witness(&foreign).unwrap();
            assert!(require_qc_witness_admission(&foreign_qc, &budget).is_err());
            assert!(worker.prepare(&block, &foreign_qc).is_err());
            assert_eq!(original_overlay(worker), original_overlay_pointer);
            assert_eq!(worker.prepare(&block, &qc).unwrap(), Some(result));
            blocks.append(&block, &qc).unwrap();
            // Measure the actual immutable table control separately; admission below still
            // uses only the original State pool and retains its own real backing and control.
            let measure = iroha_allocation::AllocationBudget::new(original_limit);
            let mut measured = iroha_allocation::ChargedBuffer::new(
                block.availability().as_slice().len(),
                &measure,
            )
            .unwrap();
            measured.append(block.availability().as_slice()).unwrap();
            let measured =
                iroha_sumeragi::availability::AvailabilityFrame::from_charged(measured, &measure)
                    .unwrap_or_else(|(_, error)| panic!("measure table custody: {error}"));
            let table_total = measure.reserved_bytes();
            drop(measured);
            assert_eq!(measure.reserved_bytes(), 0);
            let stored_source = chain
                .kura()
                .get_block(std::num::NonZeroUsize::new(10).unwrap())
                .unwrap();
            let before_read = budget.reserved_bytes();
            let read_capacity = before_read + table_total + witness_len;
            budget.set_limit_bytes(read_capacity);
            assert_eq!(
                blocks.certified(10).unwrap_err().io_kind(),
                std::io::ErrorKind::WouldBlock
            );
            let observation = blocks.pending_certificate_read_for_test().unwrap();
            assert_eq!(observation.0, Arc::as_ptr(&stored_source));
            assert!(observation.1.is_some(), "original admitted table retained");
            let read_pointer = observation
                .2
                .expect("original witness backing awaits control");
            assert_eq!(budget.reserved_bytes(), read_capacity);
            assert_eq!(
                blocks.certified(10).unwrap_err().io_kind(),
                std::io::ErrorKind::WouldBlock
            );
            assert_eq!(
                blocks.pending_certificate_read_for_test(),
                Some(observation)
            );
            assert_eq!(budget.reserved_bytes(), read_capacity);
            // Another request cannot discard the original source or either retained buffer.
            assert_eq!(
                blocks.certified(9).unwrap_err().io_kind(),
                std::io::ErrorKind::WouldBlock
            );
            assert_eq!(
                blocks.pending_certificate_read_for_test(),
                Some(observation)
            );
            assert_eq!(budget.reserved_bytes(), read_capacity);
            budget.set_limit_bytes(original_limit);
            let (_, recovered) = blocks.certified(10).unwrap().unwrap();
            assert_eq!(
                recovered
                    .attestation_witness
                    .as_ref()
                    .unwrap()
                    .as_slice()
                    .as_ptr(),
                read_pointer
            );
            assert!(
                recovered
                    .attestation_witness
                    .as_ref()
                    .unwrap()
                    .admitted_to(&budget)
            );
            assert_eq!(recovered, qc);
            assert!(blocks.pending_certificate_read_for_test().is_none());
            drop(recovered);
            let stored = blocks
                .entry(10)
                .expect("actual stored boundary is readable")
                .expect("actual stored boundary exists");
            assert_eq!(stored.commit_qc, qc);
            assert!(
                stored
                    .commit_qc
                    .attestation_witness
                    .as_ref()
                    .unwrap()
                    .admitted_to(&budget)
            );
            let (_, certified) = blocks.certified(10).unwrap().unwrap();
            assert!(
                certified
                    .attestation_witness
                    .as_ref()
                    .unwrap()
                    .admitted_to(&budget)
            );
            assert!(worker.commit(&block, &foreign_qc).is_err());
            assert_eq!(original_overlay(worker), original_overlay_pointer);
            worker.commit(&block, &qc).unwrap();
            assert_eq!(worker.applied, (10, block_hash));
            assert!(matches!(
                attestor.attest(10, &key, &statement),
                AttestOutcome::Pending
            ));
            assert!(worker.live.as_ref().unwrap().overlay.is_none());
        },
    );
}

#[test]
fn native_pasta_discard_invalidates_the_receipt_before_releasing_its_original_execution() {
    with_worker_from(
        CertifiedTestChain::npos_boundary_fixture,
        ConsensusMode::Npos,
        |chain, worker, _, _| {
            use crate::sumeragi::attestation::{NativePastaVerifier, channel};
            use iroha_sumeragi::crypto::{AttestOutcome, Attestor as _};
            let budget = worker.state.ivm_execution_budget();
            let key =
                crate::sumeragi::crypto::core_key(chain.validators()[0].0.public_key()).unwrap();
            let (attestor, publisher) = channel(chain.instance(), &key, true, &budget).unwrap();
            worker
                .attach_attestation(
                    NativePastaVerifier::new(chain.instance(), chain.network_id()),
                    Some(Arc::new(chain.pasta_custody(0))),
                    publisher,
                )
                .unwrap();
            let block = proposal(chain, worker);
            let hash = block.hash(&**worker.context.crypto.as_ref().unwrap());
            let Some(ExecOutcome::Valid(result)) = worker.execute(&block, hash) else {
                panic!("genuine boundary execution and Pasta publication")
            };
            let statement = iroha_sumeragi::preimage::att_preimage(
                &chain.instance(),
                &block.header().epoch,
                10,
                &hash,
                &result,
            );
            assert!(matches!(
                attestor.attest(10, &key, &statement),
                AttestOutcome::Attested(_)
            ));
            worker.discard(10, &[hash]);
            assert!(worker.live.as_ref().unwrap().overlay.is_some());
            assert!(matches!(
                attestor.attest(10, &key, &statement),
                AttestOutcome::Attested(_)
            ));
            worker.discard(10, &[]);
            assert!(worker.live.is_none());
            assert!(matches!(
                attestor.attest(10, &key, &statement),
                AttestOutcome::Pending
            ));
            assert!(worker.recovery.is_none());
        },
    );
}

#[test]
fn native_context_archive_capacity_retry_retains_original_overlay_and_result() {
    with_worker(|chain, worker, blocks, events| {
        let block = proposal(chain, worker);
        let block_hash = block.hash(&**worker.context.crypto.as_ref().unwrap());
        let mut occupied = None;
        let outcome = worker.run_execution_with_finisher(&block, block_hash, |worker| {
            worker.prepare_original_result().unwrap();
            let budget = worker.state.ivm_execution_budget();
            occupied = Some(
                budget
                    .try_reserve_bytes(budget.limit_bytes() - budget.reserved_bytes())
                    .unwrap(),
            );
            worker.finish_execution_with_encoder(encode_result_preimage)
        });
        assert!(matches!(outcome, ExecOutcome::Failed(_)));
        assert!(worker.recovery.is_none());
        assert!(worker.live.is_none());
        let original = worker.finishing.as_ref().unwrap();
        assert!(original.native_contexts.is_none());
        assert!(
            original
                .archive_refusal
                .as_ref()
                .unwrap()
                .is_local_refusal()
        );
        let overlay = std::ptr::from_ref(original.overlay.as_ref());
        let commitment = original.phase.ready().unwrap().get();
        let authority = commitment.schedule.current.committee.as_ptr();
        let proofs: Vec<_> = commitment
            .schedule
            .current
            .committee
            .iter()
            .map(|member| member.proof_of_possession.as_ptr())
            .collect();
        let canonical_result = norito::encode_canonical(commitment).unwrap();
        let witness = iroha_crypto::HashOf::new(&original.witness);
        let budget = worker.state.ivm_execution_budget();
        assert!(original.phase.ready().unwrap().belongs_to(&budget));
        let held = budget.reserved_bytes();
        for _ in 0..2 {
            assert!(matches!(
                worker.execute(&block, block_hash),
                Some(ExecOutcome::Failed(_))
            ));
            let retained = worker.finishing.as_ref().unwrap();
            assert_eq!(std::ptr::from_ref(retained.overlay.as_ref()), overlay);
            assert_eq!(
                retained
                    .phase
                    .ready()
                    .unwrap()
                    .get()
                    .schedule
                    .current
                    .committee
                    .as_ptr(),
                authority
            );
            let commitment = retained.phase.ready().unwrap().get();
            assert_eq!(
                commitment
                    .schedule
                    .current
                    .committee
                    .iter()
                    .map(|member| member.proof_of_possession.as_ptr())
                    .collect::<Vec<_>>(),
                proofs
            );
            assert_eq!(
                norito::encode_canonical(commitment).unwrap(),
                canonical_result
            );
            assert_eq!(iroha_crypto::HashOf::new(&retained.witness), witness);
            assert!(
                retained
                    .archive_refusal
                    .as_ref()
                    .unwrap()
                    .is_local_refusal()
            );
            assert!(retained.native_contexts.is_none());
            assert_eq!(budget.reserved_bytes(), held);
            assert!(retained.phase.ready().unwrap().belongs_to(&budget));
            assert!(!worker.results.contains_key(&block_hash));
            assert!(worker.recovery.is_none());
        }
        assert!(events.try_recv().is_err());
        drop(occupied);
        let Some(ExecOutcome::Valid(digest)) = worker.execute(&block, block_hash) else {
            panic!("original execution finishes after actual pool capacity returns")
        };
        let live = worker.live.as_ref().unwrap();
        assert_eq!(
            std::ptr::from_ref(live.overlay.as_deref().unwrap()),
            overlay
        );
        // RetainedPayload's inline value moves into Live. Its original heap allocations,
        // complete canonical value and allocation-pool custody must survive that move.
        assert_eq!(
            live.commitment.get().schedule.current.committee.as_ptr(),
            authority
        );
        assert_eq!(
            live.commitment
                .get()
                .schedule
                .current
                .committee
                .iter()
                .map(|member| member.proof_of_possession.as_ptr())
                .collect::<Vec<_>>(),
            proofs
        );
        assert_eq!(
            norito::encode_canonical(live.commitment.get()).unwrap(),
            canonical_result
        );
        assert!(live.commitment.belongs_to(&budget));
        assert_eq!(iroha_crypto::HashOf::new(&live.witness), witness);
        let source = live.native_contexts.as_ref().unwrap();
        let bytes = source.canonical_bytes().to_vec();
        let carrier_hash = source.carrier_hash();
        let projection: crate::state::NativeExecutionProjectionV1 =
            norito::decode_canonical_with_limits(
                &bytes,
                norito::canonical_decode_limits(bytes.len()),
            )
            .unwrap();
        assert_eq!(projection.carrier_hash, carrier_hash);
        assert_eq!(projection.carrier_height, 2);
        assert_eq!(
            &projection.casting_bindings,
            live.overlay
                .as_ref()
                .unwrap()
                .captured_parliament_casting_bindings()
                .unwrap()
        );
        assert_eq!(
            projection.ordinary_writes, live.witness.writes,
            "archive retains the exact original execution write order"
        );
        assert!(
            live.commitment
                .get()
                .native_lanes
                .matches_state_encoding(chain.network_id(), 2, &projection.lanes)
                .unwrap()
        );
        let qc = chain.commit_qc(2, block_hash, digest, false, Signers::Quorum);
        worker.prepare(&block, &qc).unwrap();
        blocks.append(&block, &qc).unwrap();
        worker.commit(&block, &qc).unwrap();
        let stored = worker
            .context
            .native_context_archive
            .read_exact(2, carrier_hash)
            .unwrap();
        assert_eq!(stored.as_slice(), bytes);
        assert_eq!(worker.state.view().height(), 2);
    });
}

#[test]
fn native_context_archive_failure_preserves_original_bytes_until_durable_acknowledgement() {
    with_worker(|chain, worker, blocks, events| {
        let (block, qc) = executed(chain, worker);
        let original = worker
            .live
            .as_ref()
            .unwrap()
            .native_contexts
            .as_ref()
            .unwrap();
        let pointer = original.canonical_bytes().as_ptr();
        let bytes = original.canonical_bytes().to_vec();
        let hash = original.carrier_hash();
        worker.prepare(&block, &qc).unwrap();
        blocks.append(&block, &qc).unwrap();
        let directory = chain.kura().store_root().join("native-contexts");
        let hidden = directory.with_extension("temporarily-unavailable");
        std::fs::rename(&directory, &hidden).unwrap();
        assert!(
            worker
                .commit(&block, &qc)
                .unwrap_err()
                .to_string()
                .contains("native context archive publication")
        );
        assert_eq!(worker.state.view().height(), 2);
        assert_eq!(worker.applied.0, 1);
        assert!(worker.live.as_ref().unwrap().overlay.is_none());
        let pending = worker.pending_commit.as_ref().unwrap();
        let event_count = pending.events.len();
        assert!(event_count > 0);
        assert_eq!(pending.native_contexts.canonical_bytes().as_ptr(), pointer);
        assert_eq!(pending.native_contexts.canonical_bytes(), bytes);
        for _ in 0..2 {
            assert!(worker.commit(&block, &qc).is_err());
            assert_eq!(
                worker
                    .pending_commit
                    .as_ref()
                    .unwrap()
                    .native_contexts
                    .canonical_bytes()
                    .as_ptr(),
                pointer
            );
            assert!(worker.execute(&block, qc.block_hash).is_none());
            assert_eq!(worker.build(3, 0, 1 << 20).unwrap(), (None, false));
            assert_eq!(worker.applied.0, 1);
            assert!(events.try_recv().is_err());
        }
        std::fs::rename(&hidden, &directory).unwrap();
        let next = worker.commit(&block, &qc).unwrap();
        assert!(worker.pending_commit.is_none());
        assert_eq!(worker.applied, (2, qc.block_hash));
        assert_eq!(
            worker
                .context
                .native_context_archive
                .read_exact(2, hash)
                .unwrap()
                .as_slice(),
            bytes
        );
        let mut emitted = 0;
        while events.try_recv().is_ok() {
            emitted += 1;
        }
        assert_eq!(emitted, event_count);
        assert_eq!(worker.commit(&block, &qc).unwrap(), next);
        assert!(events.try_recv().is_err());
    });
}

#[test]
fn native_context_archive_preparation_refuses_foreign_pool_without_reexecuting() {
    with_worker(|chain, worker, _blocks, events| {
        let archive_files = || {
            std::fs::read_dir(chain.kura().store_root().join("native-contexts"))
                .unwrap()
                .map(|entry| {
                    let entry = entry.unwrap();
                    (entry.file_name(), std::fs::read(entry.path()).unwrap())
                })
                .collect::<BTreeMap<_, _>>()
        };
        let original_archives = archive_files();
        assert_eq!(
            original_archives.len(),
            1,
            "original genesis context is durable"
        );
        let genesis_context = worker
            .context
            .native_context_archive
            .read_exact(1, chain.genesis().hash())
            .unwrap();
        assert_eq!(
            original_archives.values().next().unwrap().as_slice(),
            genesis_context.as_slice()
        );
        drop(genesis_context);
        let block = proposal(chain, worker);
        let block_hash = block.hash(&**worker.context.crypto.as_ref().unwrap());
        let foreign_budget = iroha_allocation::AllocationBudget::new(1 << 20);
        let foreign = NativeContextArchive::open(
            chain.kura(),
            foreign_budget.clone(),
            chain.kura().native_context_archive_max_bytes(),
        )
        .unwrap();
        let mut original_overlay = None;
        let mut original_result = None;
        let outcome = worker.run_execution_with_finisher(&block, block_hash, |worker| {
            worker.prepare_original_result().unwrap();
            let original = worker.finishing.as_ref().unwrap();
            original_overlay = Some(std::ptr::from_ref(original.overlay.as_ref()));
            let result = original.phase.ready().unwrap();
            assert!(result.belongs_to(&worker.state.ivm_execution_budget()));
            original_result = Some((
                result.get().schedule.current.committee.as_ptr(),
                result
                    .get()
                    .schedule
                    .current
                    .committee
                    .iter()
                    .map(|member| member.proof_of_possession.as_ptr())
                    .collect::<Vec<_>>(),
                norito::encode_canonical(result.get()).unwrap(),
            ));
            assert!(matches!(
                foreign.prepare(
                    &original.overlay,
                    original.valid.as_ref(),
                    original.phase.ready().unwrap(),
                    &original.witness
                ),
                Err(NativeContextArchiveError::Source(_)),
            ));
            assert_eq!(foreign_budget.reserved_bytes(), 0);
            assert!(original.native_contexts.is_none());
            assert_eq!(archive_files(), original_archives);
            worker.finish_execution_with_encoder(encode_result_preimage)
        });
        assert!(matches!(outcome, ExecOutcome::Valid(_)));
        let retained = worker.live.as_ref().unwrap();
        assert_eq!(
            Some(std::ptr::from_ref(retained.overlay.as_deref().unwrap())),
            original_overlay
        );
        assert_eq!(
            Some((
                retained
                    .commitment
                    .get()
                    .schedule
                    .current
                    .committee
                    .as_ptr(),
                retained
                    .commitment
                    .get()
                    .schedule
                    .current
                    .committee
                    .iter()
                    .map(|member| member.proof_of_possession.as_ptr())
                    .collect::<Vec<_>>(),
                norito::encode_canonical(retained.commitment.get()).unwrap(),
            )),
            original_result
        );
        assert!(
            retained
                .commitment
                .belongs_to(&worker.state.ivm_execution_budget())
        );
        assert!(retained.native_contexts.is_some());
        assert_eq!(
            archive_files(),
            original_archives,
            "preparation publishes no H2 archive"
        );
        assert_eq!(foreign_budget.reserved_bytes(), 0);
        assert_eq!(worker.state.view().height(), 1);
        assert!(events.try_recv().is_err());
    });
}

#[test]
fn original_lane_policy_proposal_refusal_retains_worker_owner_and_exact_queued_retry() {
    with_worker_from(
        || {
            let mut config = TestChainConfig::new(World::new(), 1_000);
            config
                .genesis_parameters
                .push(iroha_data_model::parameter::Parameter::Custom(
                    iroha_data_model::sumeragi_lanes::SumeragiLanePolicy::for_chain(
                        iroha_data_model::parameter::system::SumeragiParameters::default(),
                        iroha_sumeragi::availability::recommended_data_availability_layout(),
                    )
                    .into_custom_parameter(),
                ));
            CertifiedTestChain::start(config).expect("original signed lane policy")
        },
        ConsensusMode::Permissioned,
        |chain, worker, _blocks, events| {
            let transaction = chain.tick(2_000);
            let hash = transaction.hash_as_entrypoint();
            let (_, time) =
                iroha_primitives::time::TimeSource::new_mock(Duration::from_millis(2_001));
            let queue = Arc::new(Queue::test(
                iroha_config::parameters::actual::Queue::default(),
                &time,
            ));
            let accepted = crate::tx::AcceptedTransaction::accept_with_time_source(
                transaction,
                &chain.network_id(),
                Duration::from_secs(1),
                chain.state().view().world().parameters().transaction(),
                &iroha_config::parameters::actual::Crypto::default(),
                &time,
            )
            .unwrap();
            queue.push(accepted, chain.state().view()).unwrap();
            worker.queue = Some(Arc::clone(&queue));
            let limits = norito::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, 0, 64);
            let original = norito::with_decode_limits_scope(limits, || {
                super::super::lanes::lane_policy(worker.state.view().world()).unwrap_err()
            });
            assert_eq!(
                original.reason(),
                ivm::error::ExecutionDeferral::ActiveMemoryCapacity
            );
            assert!(original.allocation_refusal().is_none());
            let policy_id = iroha_data_model::sumeragi_lanes::SumeragiLanePolicy::parameter_id();
            let original_policy = worker
                .state
                .view()
                .world()
                .parameters()
                .custom()
                .get(&policy_id)
                .unwrap()
                .payload()
                .get()
                .to_owned();
            let budget = worker.state.ivm_execution_budget();
            let charged = budget.reserved_bytes();
            assert!(matches!(
                norito::with_decode_limits_scope(limits, || worker.build(2, 0, 1 << 20)),
                Err(PublicationError::Retryable(_))
            ));
            assert_eq!(
                worker.routing_refusal.as_ref(),
                Some(&original),
                "the actual merge proposal must retain the original policy owner before diagnostics"
            );
            assert!(worker.payload_build.is_none());
            assert!(worker.last_built.is_none());
            assert!(worker.live.is_none());
            assert!(worker.recovery.is_none());
            assert!(worker.results.is_empty());
            assert!(worker.pending_commit.is_none());
            assert!(events.try_recv().is_err());
            assert_eq!(queue.queued_len(), 1);
            assert_eq!(budget.reserved_bytes(), charged);
            assert_eq!(
                worker
                    .state
                    .view()
                    .world()
                    .parameters()
                    .custom()
                    .get(&policy_id)
                    .unwrap()
                    .payload()
                    .get(),
                &original_policy
            );
            let (payload, _) = worker.build(2, 0, 1 << 20).unwrap();
            let payload = payload.expect("same queued original retries after caller scope removal");
            let block = super::super::payload::decode(payload.as_slice()).unwrap();
            assert!(
                block
                    .external_entrypoints_slice()
                    .iter()
                    .any(|entry| entry.hash() == hash)
            );
            assert!(worker.routing_refusal.is_none());
            assert_eq!(queue.queued_len(), 1);
            assert!(events.try_recv().is_err());
            assert_eq!(worker.state.view().height(), 1);
        },
    );
}
