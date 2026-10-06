//! Original validation decoder refusal retains typed worker ownership before local diagnostics.

use super::*;

/// Observe only the actual payload and merge prefix; no production admission is added.
fn original_validation_prefix(worker: &Worker<'_>, block: &AvailableBody) -> usize {
    const CEILING: usize = 1 << 28;
    norito::with_decode_limits_scope(
        norito::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, CEILING, 64),
        || {
            let proposal = payload::decode(block.payload().as_slice()).unwrap();
            let schedule = worker.scheduled(block.header().height).unwrap();
            let expansion = lanes::merge::expand(
                worker.state,
                &proposal,
                &*worker.context.lane_blocks,
                Duration::from_millis(schedule.params.exec_budget_ms),
            )
            .unwrap();
            drop(expansion);
            let norito::Error::TotalAllocationExceeded { attempted, limit } =
                norito::core::reserve_decode_allocation(CEILING + 1).unwrap_err()
            else {
                panic!("original validation prefix allocation observation changed");
            };
            assert_eq!(limit, CEILING as u64);
            let prefix = usize::try_from(attempted).unwrap() - CEILING - 1;
            assert!(prefix > 0);
            prefix
        },
    )
}

#[test]
fn original_prepared_certificate_read_refusal_retains_worker_owner_and_funded_execution() {
    publication_tests::with_worker(|chain, worker, _blocks, events| {
        let block = publication_tests::proposal(chain, worker);
        let hash = block.hash(&**worker.context.crypto.as_ref().unwrap());
        let Some(ExecOutcome::Valid(result)) = worker.execute(&block, hash) else {
            panic!("the actual available body must execute before certification refusal");
        };
        let qc = chain.commit_qc(
            block.header().height,
            hash,
            result,
            block.header().attest,
            crate::sumeragi::test_chain::Signers::Quorum,
        );
        let original_source = std::ptr::from_ref(block.source());
        let original_bytes = block.payload().as_slice().as_ptr();
        let original = worker.live.as_ref().unwrap();
        let original_commitment = std::ptr::from_ref(original.commitment.get());
        let original_overlay = std::ptr::from_ref(original.overlay.as_ref().unwrap());
        let original_witness = iroha_crypto::HashOf::new(original.witness.as_ref().unwrap().wire());
        let PublicationPhase::Executed { preimage, .. } = &original.phase else {
            panic!("the same original execution must await publication");
        };
        let original_preimage = preimage.as_slice().as_ptr();
        let budget = worker.state.ivm_execution_budget();
        let epoch = crossbeam_epoch::pin();
        let occupied_bytes = budget.reserved_bytes();
        let height = worker.state.view().height();
        let limits = norito::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, 0, 64);
        let error = norito::with_decode_limits_scope(limits, || {
            crate::sumeragi::certified_chain::committed_block(&worker.state.view(), 1)
        })
        .unwrap_err();
        let crate::execution_attempt::ExecutionAttemptError::Deferred(original_refusal) = error
        else {
            panic!("original certificate authority read must retain its typed refusal: {error:?}");
        };
        assert!(original_refusal.allocation_refusal().is_none());
        assert!(matches!(
            norito::with_decode_limits_scope(limits, || worker.prepare(&block, &qc)),
            Err(PublicationError::Deferred(ref source)) if source.execution() == Some(&original_refusal)
        ));
        assert_eq!(
            worker.routing_refusal.as_ref(),
            Some(&original_refusal),
            "the prepared-certificate diagnostic retains the original typed reader owner"
        );
        let retained = worker.live.as_ref().unwrap();
        assert_eq!(
            std::ptr::from_ref(retained.commitment.get()),
            original_commitment
        );
        assert_eq!(
            std::ptr::from_ref(retained.overlay.as_ref().unwrap()),
            original_overlay
        );
        assert_eq!(
            iroha_crypto::HashOf::new(retained.witness.as_ref().unwrap().wire()),
            original_witness
        );
        let PublicationPhase::Executed { preimage, .. } = &retained.phase else {
            panic!("an unfinished authority read cannot stage the original publication");
        };
        assert_eq!(preimage.as_slice().as_ptr(), original_preimage);
        assert_eq!(budget.reserved_bytes(), occupied_bytes);
        assert_eq!(worker.state.view().height(), height);
        assert!(worker.context.staging.get(&hash).is_none());
        assert!(worker.pending_commit.is_none());
        assert!(worker.finishing.is_none());
        assert!(worker.quarantine_context.is_none());
        assert!(worker.recovery.is_none());
        assert!(events.try_recv().is_err());
        assert_eq!(std::ptr::from_ref(block.source()), original_source);
        assert_eq!(block.payload().as_slice().as_ptr(), original_bytes);
        drop(epoch);
        assert_eq!(worker.prepare(&block, &qc).unwrap(), Some(result));
        assert!(worker.routing_refusal.is_none());
        let retained = worker.live.as_ref().unwrap();
        assert_eq!(
            std::ptr::from_ref(retained.commitment.get()),
            original_commitment
        );
        assert_eq!(
            std::ptr::from_ref(retained.overlay.as_ref().unwrap()),
            original_overlay
        );
        assert_eq!(
            iroha_crypto::HashOf::new(retained.witness.as_ref().unwrap().wire()),
            original_witness
        );
        assert!(matches!(retained.phase, PublicationPhase::Prepared { .. }));
        assert_eq!(worker.state.view().height(), height);
    });
}

#[test]
fn original_post_merge_validation_refusal_retains_worker_owner_and_exact_available_retry() {
    publication_tests::with_worker(|chain, worker, _blocks, events| {
        let block = publication_tests::proposal(chain, worker);
        let hash = block.hash(&**worker.context.crypto.as_ref().unwrap());
        let original_source = std::ptr::from_ref(block.source());
        let original_bytes = block.payload().as_slice().as_ptr();
        let height = worker.state.view().height();
        let prefix = original_validation_prefix(worker, &block);
        let limits = norito::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, prefix, 64);
        let error = norito::with_decode_limits_scope(limits, || {
            let proposal = payload::decode(block.payload().as_slice())
                .expect("original payload prefix completes");
            let schedule = worker.scheduled(block.header().height).unwrap();
            let committee = schedule
                .epoch
                .committee
                .iter()
                .map(|member| member.validator.clone())
                .collect::<Vec<_>>();
            let topology = Topology::new(committee);
            let expansion = lanes::merge::expand(
                worker.state,
                &proposal,
                &*worker.context.lane_blocks,
                Duration::from_millis(schedule.params.exec_budget_ms),
            )
            .expect("original merge prefix completes before validation refusal");
            ValidBlock::validate_sumeragi_block(
                proposal,
                &topology,
                &worker.context.genesis_account,
                Duration::from_millis(schedule.params.block_time_ms),
                worker.context.consensus_mode,
                expansion,
                block.header(),
                block.payload().as_slice(),
                worker.state,
            )
            .unpack(|_| {})
            .map(|_| ())
            .unwrap_err()
            .1
        });
        let BlockValidationError::ExecutionDeferred(original) = *error else {
            panic!("actual post-merge validator must preserve its local reason: {error:?}");
        };
        assert_eq!(
            original.reason(),
            ivm::error::ExecutionDeferral::ActiveMemoryCapacity
        );
        assert!(matches!(
            norito::with_decode_limits_scope(limits, || worker.execute(&block, hash)),
            Some(ExecOutcome::Failed(_))
        ));
        assert_eq!(
            worker.routing_refusal.as_ref(),
            Some(&original),
            "local diagnostic must retain the actual validation owner"
        );
        assert!(!worker.results.contains_key(&hash));
        assert!(worker.quarantine_context.is_none());
        assert!(worker.live.is_none());
        assert!(worker.finishing.is_none());
        assert!(worker.recovery.is_none());
        assert!(worker.context.staging.get(&hash).is_none());
        assert!(events.try_recv().is_err());
        assert_eq!(worker.state.view().height(), height);
        assert_eq!(std::ptr::from_ref(block.source()), original_source);
        assert_eq!(block.payload().as_slice().as_ptr(), original_bytes);
        let returned = worker
            .signature_decode
            .as_ref()
            .and_then(|attempt| attempt.decoded.as_ref())
            .expect("actual local validation returns the original decoded graph");
        let original_entries = returned.external_entrypoints_slice().as_ptr();
        assert!(returned.signatures_admitted_to(&worker.state.ivm_execution_budget()));
        assert!(
            returned
                .matches_resultless_proposal_wire(block.payload().as_slice())
                .unwrap()
        );
        assert!(matches!(
            worker.execute(&block, hash),
            Some(ExecOutcome::Valid(_))
        ));
        let PublicationPhase::Executed { valid, .. } = &worker.live.as_ref().unwrap().phase else {
            panic!("same original decoded graph completes production validation");
        };
        assert_eq!(
            valid.as_ref().external_entrypoints_slice().as_ptr(),
            original_entries
        );
        assert!(worker.signature_decode.is_none());
        assert!(worker.routing_refusal.is_none());
        assert_eq!(
            worker.state.view().height(),
            height,
            "retry executes without publishing"
        );
        assert_eq!(std::ptr::from_ref(block.source()), original_source);
        assert_eq!(block.payload().as_slice().as_ptr(), original_bytes);
    });
}

#[test]
fn prepared_certificate_busy_retries_same_execution_after_original_reader_release() {
    use std::{
        sync::mpsc,
        task::{Context, Poll, Waker},
    };
    publication_tests::with_worker(|chain, worker, _blocks, events| {
        let block = publication_tests::proposal(chain, worker);
        let hash = block.hash(&**worker.context.crypto.as_ref().unwrap());
        let Some(ExecOutcome::Valid(result)) = worker.execute(&block, hash) else {
            panic!("the actual signed body must execute before reader contention");
        };
        let qc = chain.commit_qc(
            block.header().height,
            hash,
            result,
            block.header().attest,
            crate::sumeragi::test_chain::Signers::Quorum,
        );
        let state = worker.state;
        let original_source = std::ptr::from_ref(block.source());
        let original_bytes = block.payload().as_slice().as_ptr();
        let original = worker.live.as_ref().unwrap();
        let original_commitment = std::ptr::from_ref(original.commitment.get());
        let original_overlay = std::ptr::from_ref(original.overlay.as_ref().unwrap());
        let original_witness = iroha_crypto::HashOf::new(original.witness.as_ref().unwrap().wire());
        let PublicationPhase::Executed { preimage, .. } = &original.phase else {
            panic!("the original execution must await publication");
        };
        let original_preimage = preimage.as_slice().as_ptr();
        let budget = state.ivm_execution_budget();
        let mut registration = crate::unit_test_support::release_registration(&budget);
        let epoch = crossbeam_epoch::pin();
        let occupied = budget.reserved_bytes();
        let height = state.view().height();
        std::thread::scope(|scope| {
            let (ready_tx, ready_rx) = mpsc::channel();
            let (release_tx, release_rx) = mpsc::channel();
            let holder = scope.spawn(move || {
                state.with_held_header_for_reader_test(|wait| {
                    ready_tx.send(wait).unwrap();
                    // A blocking regression releases the real guard on a finite deadline.
                    release_rx.recv_timeout(Duration::from_secs(20)).is_ok()
                })
            });
            let original_wait = ready_rx.recv_timeout(Duration::from_secs(20)).unwrap();
            let outcome = worker.prepare(&block, &qc);
            // Release before any assertion can unwind and strand the foreign holder.
            release_tx.send(()).ok();
            assert!(
                holder.join().unwrap(),
                "reader probe blocked on the actual writer"
            );
            let Err(PublicationError::Deferred(reason)) = outcome else {
                panic!("original reader Busy must remain typed: {outcome:?}");
            };
            assert!(
                matches!(&reason, crate::sumeragi::driver::traits::PublicationDeferral::StateViewBusy(wait) if wait == &original_wait)
            );
            let wait = original_wait;
            assert_eq!(
                registration.poll_wait(&wait, &mut Context::from_waker(Waker::noop())),
                Poll::Ready(()),
                "release before first poll must not be lost"
            );
        });
        let retained = worker.live.as_ref().unwrap();
        assert_eq!(
            std::ptr::from_ref(retained.commitment.get()),
            original_commitment
        );
        assert_eq!(
            std::ptr::from_ref(retained.overlay.as_ref().unwrap()),
            original_overlay
        );
        assert_eq!(
            iroha_crypto::HashOf::new(retained.witness.as_ref().unwrap().wire()),
            original_witness
        );
        let PublicationPhase::Executed { preimage, .. } = &retained.phase else {
            panic!("unfinished reader cannot stage the original publication");
        };
        assert_eq!(preimage.as_slice().as_ptr(), original_preimage);
        assert_eq!(budget.reserved_bytes(), occupied);
        assert_eq!(state.view().height(), height);
        assert!(worker.context.staging.get(&hash).is_none());
        assert!(worker.pending_commit.is_none());
        assert!(worker.finishing.is_none());
        assert!(worker.recovery.is_none());
        assert!(events.try_recv().is_err());
        assert_eq!(std::ptr::from_ref(block.source()), original_source);
        assert_eq!(block.payload().as_slice().as_ptr(), original_bytes);
        drop(epoch);
        assert_eq!(worker.prepare(&block, &qc).unwrap(), Some(result));
        let retained = worker.live.as_ref().unwrap();
        assert_eq!(
            std::ptr::from_ref(retained.commitment.get()),
            original_commitment
        );
        assert_eq!(
            std::ptr::from_ref(retained.overlay.as_ref().unwrap()),
            original_overlay
        );
        assert_eq!(
            iroha_crypto::HashOf::new(retained.witness.as_ref().unwrap().wire()),
            original_witness
        );
        assert!(matches!(retained.phase, PublicationPhase::Prepared { .. }));
        assert_eq!(state.view().height(), height);
    });
}

/// Reach the native finalizer using only exact original-pool capacity refusals. This changes
/// the test's local allocation ceiling, never signed policy, protocol behavior or an allocator.
#[test]
fn original_lane_finalizer_refusal_returns_same_graph_before_seal_and_publishes_after_retry() {
    use crate::sumeragi::driver::traits::BlockStore as _;
    use iroha_allocation::AllocationRefusal;
    use iroha_data_model::{
        parameter::{Parameter, system::SumeragiParameters},
        sumeragi_lanes::{SumeragiLaneAutoscale, SumeragiLanePolicy},
    };
    use iroha_model_base::topology::{DataSpaceId, LaneId};

    publication_tests::with_worker_from(
        || {
            let mut config = crate::sumeragi::test_chain::TestChainConfig::new(
                crate::state::World::new(),
                1_000,
            );
            let mut policy = SumeragiLanePolicy::for_chain(
                SumeragiParameters::default(),
                iroha_sumeragi::availability::recommended_data_availability_layout(),
            );
            policy.autoscale = Some(SumeragiLaneAutoscale {
                min_lane: LaneId::new(16),
                max_lane_exclusive: LaneId::new(20),
                dataspace: DataSpaceId::UNIVERSAL,
                committee_size: 4,
                per_lane_target_tps: 10,
                window: 8,
                scale_out_permille: 800,
                scale_in_permille: 200,
                cooldown: 3,
            });
            config
                .genesis_parameters
                .push(Parameter::Custom(policy.into_custom_parameter()));
            crate::sumeragi::test_chain::CertifiedTestChain::start(config)
                .expect("actual signed autoscale genesis")
        },
        ConsensusMode::Permissioned,
        |chain, worker, blocks, events| {
            let block = publication_tests::proposal(chain, worker);
            let hash = block.hash(&**worker.context.crypto.as_ref().unwrap());
            let budget = worker.state.ivm_execution_budget();
            let original_limit = budget.limit_bytes();
            let height = worker.state.view().height();
            let sample_ptr = worker
                .state
                .view()
                .world()
                .sumeragi_lanes()
                .samples
                .as_ptr();
            let mut attempt = SignatureDecodeAttempt {
                block_hash: hash,
                source: block.clone(),
                decoder: iroha_data_model::block::PreparedSignedBlockSignaturesDecode::new(&budget)
                    .expect("same State-pool decoder"),
                decoded: None,
                returned_refusal: None,
            };
            let entries = attempt
                .original_decoded(&budget)
                .unwrap()
                .external_entrypoints_slice()
                .as_ptr();
            worker.signature_decode = Some(attempt);
            // Keep MV reclamation out of this operation's exact capacity observations.
            let epoch = crossbeam_epoch::pin();
            let mut ceiling = budget.reserved_bytes();
            let mut reached_lane_finalizer = false;
            for _ in 0..128 {
                budget.set_limit_bytes(ceiling);
                let outcome = worker.run_execution(&block, hash);
                let next_ceiling = match &outcome {
                    Err(PublicationError::Deferred(source)) => match source.allocation_refusal() {
                        Some(AllocationRefusal::Capacity {
                            requested_bytes,
                            reserved_bytes,
                            ..
                        }) => Some(reserved_bytes.checked_add(*requested_bytes).unwrap()),
                        Some(AllocationRefusal::ExceedsLimit {
                            requested_bytes, ..
                        }) => Some(
                            budget
                                .reserved_bytes()
                                .checked_add(*requested_bytes)
                                .unwrap(),
                        ),
                        Some(AllocationRefusal::DemandOverflow) => {
                            panic!("actual native demand overflowed")
                        }
                        None if source.execution().is_some_and(|original| {
                            original.reason()
                                == ivm::error::ExecutionDeferral::AllocationUnavailable
                        }) =>
                        {
                            None
                        }
                        None => {
                            panic!("capacity probe encountered another local owner: {source:?}")
                        }
                    },
                    // The HC133 counterfactual reaches the executor's late lane take. That
                    // path must fail the ownership assertion below, not re-decode a new graph.
                    Err(PublicationError::Retryable(reason))
                        if reason == "local lane custody allocation refused" =>
                    {
                        None
                    }
                    other => panic!("capacity probe did not reach a retained refusal: {other:?}"),
                };
                let returned = worker
                    .signature_decode
                    .as_ref()
                    .and_then(|original| original.decoded.as_ref())
                    .expect("late lane refusal must return the sole original graph before sealing");
                assert_eq!(returned.external_entrypoints_slice().as_ptr(), entries);
                assert!(returned.signatures_admitted_to(&budget));
                assert!(
                    returned
                        .matches_resultless_proposal_wire(block.payload().as_slice())
                        .unwrap()
                );
                assert_eq!(worker.state.view().height(), height);
                assert_eq!(
                    worker
                        .state
                        .view()
                        .world()
                        .sumeragi_lanes()
                        .samples
                        .as_ptr(),
                    sample_ptr
                );
                assert!(worker.live.is_none());
                assert!(worker.finishing.is_none());
                assert!(worker.results.is_empty());
                assert!(worker.pending_commit.is_none());
                assert!(worker.quarantine_context.is_none());
                assert!(worker.recovery.is_none());
                assert!(worker.context.staging.get(&hash).is_none());
                assert!(events.try_recv().is_err());
                if let Some(next) = next_ceiling {
                    assert!(
                        next > ceiling,
                        "one original capacity admission makes strict progress"
                    );
                    assert!(
                        next <= original_limit,
                        "source still fits its unchanged full pool"
                    );
                    ceiling = next;
                } else {
                    reached_lane_finalizer = true;
                    break;
                }
            }
            budget.set_limit_bytes(original_limit);
            drop(epoch);
            assert!(
                reached_lane_finalizer,
                "native capacity probes reached the real late lane step"
            );
            let Some(ExecOutcome::Valid(result)) = worker.execute(&block, hash) else {
                panic!("the same original proposal retries after local capacity is restored");
            };
            let PublicationPhase::Executed { valid, .. } = &worker.live.as_ref().unwrap().phase
            else {
                panic!("same original graph finishes execution");
            };
            assert_eq!(
                valid.as_ref().external_entrypoints_slice().as_ptr(),
                entries
            );
            assert!(worker.signature_decode.is_none());
            let qc = chain.commit_qc(
                block.header().height,
                hash,
                result,
                block.header().attest,
                crate::sumeragi::test_chain::Signers::Quorum,
            );
            assert_eq!(worker.prepare(&block, &qc).unwrap(), Some(result));
            let original = worker.context.staging.get(&hash).unwrap();
            assert_eq!(
                original.executed.external_entrypoints_slice().as_ptr(),
                entries
            );
            blocks.append(&block, &qc).unwrap();
            worker.commit(&block, &qc).unwrap();
            let published = worker.state.view().latest_block().unwrap().unwrap();
            assert!(iroha_data_model::block::SharedSignedBlock::ptr_eq(
                &published,
                &original.executed
            ));
            assert_eq!(worker.state.view().height(), height + 1);
        },
    );
}

/// Exercise the actual validator-to-Worker handoff, including the source guard's refusal.
/// Faults use the existing one-shot accessor and transaction application API; they do not
/// mutate the funded witness, fabricate a source or add a production injection hook.
#[test]
fn validated_witness_guard_failure_requires_recovery_without_reexecuting_original_source() {
    use crate::sumeragi::driver::traits::BlockStore as _;

    // Zero is the ordinary publication control. One is prior one-shot extraction;
    // two is a genuine empty application after the original output seal completed.
    for fault in 0..3 {
        publication_tests::with_worker(move |chain, worker, blocks, events| {
            let block = publication_tests::proposal(chain, worker);
            let hash = block.hash(&**worker.context.crypto.as_ref().unwrap());
            let budget = worker.state.ivm_execution_budget();
            let height = worker.state.view().height();
            let original_available = block.payload().as_slice().as_ptr();
            let original_source = std::ptr::from_ref(block.source());
            let mut attempt = SignatureDecodeAttempt {
                block_hash: hash,
                source: block.clone(),
                decoder: iroha_data_model::block::PreparedSignedBlockSignaturesDecode::new(&budget)
                    .expect("actual original-pool signature decoder"),
                decoded: None,
                returned_refusal: None,
            };
            let entries = attempt
                .original_decoded(&budget)
                .unwrap()
                .external_entrypoints_slice()
                .as_ptr();
            let proposal = attempt.decoded.take().unwrap();
            worker.signature_decode = Some(attempt);
            let scheduled = worker.scheduled(block.header().height).unwrap();
            let committee = scheduled
                .epoch
                .committee
                .iter()
                .map(|member| member.validator.clone())
                .collect::<Vec<_>>();
            let topology = Topology::new(committee.clone());
            let expansion = lanes::merge::expand(
                worker.state,
                &proposal,
                &*worker.context.lane_blocks,
                Duration::from_millis(scheduled.params.exec_budget_ms),
            )
            .unwrap();
            let mut recorded_events = Vec::new();
            let (valid, mut overlay) = ValidBlock::validate_sumeragi_block(
                proposal,
                &topology,
                &worker.context.genesis_account,
                Duration::from_millis(scheduled.params.block_time_ms),
                worker.context.consensus_mode,
                expansion,
                block.header(),
                block.payload().as_slice(),
                worker.state,
            )
            .unpack(|event| recorded_events.push(event.into()))
            .expect("actual available proposal seals its real execution and witness");
            assert_eq!(
                valid.as_ref().external_entrypoints_slice().as_ptr(),
                entries
            );
            assert!(valid.as_ref().signatures_admitted_to(&budget));
            let original_fragments = overlay.committed_fragment_count();
            let detached = if fault == 1 {
                let witness = overlay
                    .take_exec_witness()
                    .expect("first actual witness owner");
                witness.verify_source_binding().unwrap();
                Some(witness)
            } else {
                None
            };
            if fault == 2 {
                // This normal State API rejects any application after the output seal.
                // An empty transaction cannot change the original transaction or wire graph.
                overlay.transaction().apply();
                assert_eq!(overlay.committed_fragment_count(), original_fragments);
                assert!(
                    overlay
                        .verify_execution_output_seal(valid.as_ref())
                        .is_err()
                );
            }
            let outcome = worker.retain_validated_execution(
                &block,
                hash,
                valid,
                overlay,
                committee,
                recorded_events,
            );
            if fault == 0 {
                assert!(outcome.unwrap().is_some());
                let original = worker.finishing.as_ref().unwrap();
                assert_eq!(
                    original
                        .valid
                        .as_ref()
                        .external_entrypoints_slice()
                        .as_ptr(),
                    entries
                );
                original.witness.verify_source_binding().unwrap();
                assert!(worker.recovery.is_none());
                assert!(worker.signature_decode.is_none());
                let Some(ExecOutcome::Valid(result)) = worker.execute(&block, hash) else {
                    panic!("ordinary witness handoff completes the same actual execution");
                };
                let qc = chain.commit_qc(
                    block.header().height,
                    hash,
                    result,
                    block.header().attest,
                    crate::sumeragi::test_chain::Signers::Quorum,
                );
                assert_eq!(worker.prepare(&block, &qc).unwrap(), Some(result));
                let staged = worker.context.staging.get(&hash).unwrap();
                assert_eq!(
                    staged.executed.external_entrypoints_slice().as_ptr(),
                    entries
                );
                blocks.append(&block, &qc).unwrap();
                worker.commit(&block, &qc).unwrap();
                let published = worker.state.view().latest_block().unwrap().unwrap();
                assert!(iroha_data_model::block::SharedSignedBlock::ptr_eq(
                    &published,
                    &staged.executed
                ));
                assert_eq!(worker.state.view().height(), height + 1);
                return;
            }
            let Err(PublicationError::RecoveryRequired(reason)) = outcome else {
                panic!(
                    "an absent or source-invalid original witness cannot promise retry: {outcome:?}"
                );
            };
            assert_eq!(worker.recovery.as_deref(), Some(reason.as_str()));
            assert_eq!(
                reason,
                "the original execution witness is absent or no longer source-valid"
            );
            assert!(worker.finishing.is_none());
            assert!(worker.live.is_none());
            assert!(worker.pending_commit.is_none());
            assert!(worker.results.is_empty());
            assert!(worker.quarantine_context.is_none());
            assert!(worker.context.staging.get(&hash).is_none());
            assert!(events.try_recv().is_err());
            assert_eq!(worker.state.view().height(), height);
            let original_decode = worker.signature_decode.as_ref().unwrap();
            assert!(original_decode.decoded.is_none());
            let decoder = std::ptr::from_ref(&original_decode.decoder);
            let expected = PublicationError::RecoveryRequired(reason.clone()).to_string();
            assert!(matches!(
                worker.execute(&block, hash),
                Some(ExecOutcome::Failed(report)) if report == expected
            ));
            assert_eq!(worker.recovery.as_deref(), Some(reason.as_str()));
            assert_eq!(
                std::ptr::from_ref(&worker.signature_decode.as_ref().unwrap().decoder),
                decoder,
                "execute observes latched recovery before another decode or State execution"
            );
            assert!(worker.signature_decode.as_ref().unwrap().decoded.is_none());
            assert!(worker.results.is_empty());
            assert!(worker.finishing.is_none());
            assert!(worker.live.is_none());
            assert!(events.try_recv().is_err());
            assert_eq!(worker.state.view().height(), height);
            assert_eq!(block.payload().as_slice().as_ptr(), original_available);
            assert_eq!(std::ptr::from_ref(block.source()), original_source);
            if let Some(original_witness) = detached {
                // The actual first extracted owner remains separate and immutable;
                // recovery never reimports it or fabricates a replacement witness.
                original_witness.verify_source_binding().unwrap();
            }
        });
    }
}
