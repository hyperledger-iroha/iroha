//! Actual paid Worker deferral retains completed leg custody, not a cached execution result.

use super::*;

#[test]
fn original_paid_amx_post_decode_refusal_retains_worker_leg_and_exact_retry() {
    crate::sumeragi::threads::sumeragi_thread_builder("amx-worker-retry-test")
        .spawn(|| crate::sumeragi::amx::with_paid_prepare_retry_fixture(|chain, instruction, customer| {
            publication_tests::with_worker_chain(chain, ConsensusMode::Permissioned,
                Arc::new(crate::sumeragi::lanes::merge::NoLanes),
                |chain, worker, _blocks, events| {
                    let block = publication_tests::proposal_with_transaction(chain, worker, |chain, time| {
                        chain.sign(&customer, [instruction], time)
                    });
                    let hash = block.hash(&**worker.context.crypto.as_ref().unwrap());
                    let budget = worker.state.ivm_execution_budget();
                    let height = worker.state.view().height();
                    let wire = block.payload().as_slice().as_ptr();
                    let original = {
                        let view = worker.state.view();
                        view.world().sumeragi_amx_participant().clone()
                    };
                    let observer = crate::sumeragi::amx::NativeLegRetryObservation::occupy_after_first_completion();
                    assert!(worker.parent_applied(&block), "the actual execution parent is applied");
                    // Borrow the production Worker's typed result before the public
                    // Execute transport projects local failures into a diagnostic string.
                    let Err(PublicationError::Deferred(refusal)) = worker.run_execution(&block, hash) else {
                        panic!("the original Candidate must return its typed local deferral");
                    };
                    let first = observer.snapshot();
                    assert_eq!(first.0, 1, "actual canonical leg completed before Candidate refusal");
                    assert_eq!(first.1, 1);
                    assert!(first.2.unwrap().retained_bytes > 0);
                    assert!(matches!(&refusal, PublicationDeferral::StateStorage(
                        crate::state::StateStorageAdmissionError::NativeAmx(_))));
                    assert!(matches!(refusal.allocation_refusal(),
                        Some(iroha_allocation::AllocationRefusal::Capacity { .. })));
                    let original_candidate = observer.original_candidate_refusal()
                        .expect("the real Candidate refused after actual canonical completion");
                    assert_eq!(refusal.allocation_refusal(), Some(&original_candidate),
                        "Worker returns the exact original Candidate pool/release generation");
                    assert_eq!(worker.state.view().height(), height);
                    assert!(worker.results.is_empty());
                    assert!(worker.live.is_none()); assert!(worker.finishing.is_none());
                    assert!(worker.quarantine_context.is_none());
                    assert!(worker.context.staging.get(&hash).is_none());
                    assert!(events.try_recv().is_err());
                    assert_eq!(block.payload().as_slice().as_ptr(), wire);
                    {
                        let view = worker.state.view();
                        assert!(std::ptr::eq(view.world().sumeragi_amx_participant().canonical().unwrap(),
                            original.canonical().unwrap()), "no fee/debit/escrow publication on local refusal");
                    }
                    observer.release_original_blocker();
                    let Some(ExecOutcome::Valid(_)) = worker.execute(&block, hash) else {
                        panic!("same original paid candidate must retry after genuine capacity release");
                    };
                    let retry = observer.snapshot();
                    assert_eq!(retry.0, 1,
                        "completed canonical decoder must not allocate or consume cumulative leg work again");
                    assert_eq!(retry.1, 2, "current business/authority checks rerun on the original graph");
                    assert_eq!(retry.2, first.2);
                    assert_eq!(block.payload().as_slice().as_ptr(), wire);
                    assert_eq!(worker.state.view().height(), height);
                    assert!(worker.routing_refusal.is_none());
                    assert!(worker.live.is_some() || worker.finishing.is_some());
                    assert!(budget.reserved_bytes() > 0);
                });
        })).unwrap().join().unwrap();
}

fn with_original_paid_bank(
    duplicate: bool,
    test: impl FnOnce(
        &SignedBlock,
        &AvailableBody,
        &iroha_allocation::AllocationBudget,
        u64,
        &dyn iroha_sumeragi::crypto::Crypto,
    ) + Send
    + 'static,
) {
    crate::sumeragi::threads::sumeragi_thread_builder("amx-worker-bank-test")
        .spawn(move || {
            crate::sumeragi::amx::with_paid_prepare_retry_fixture(|chain, instruction, customer| {
                publication_tests::with_worker_chain(
                    chain,
                    ConsensusMode::Permissioned,
                    Arc::new(crate::sumeragi::lanes::merge::NoLanes),
                    |chain, worker, _blocks, _events| {
                        let block = publication_tests::proposal_with_transaction(
                            chain,
                            worker,
                            |chain, time| {
                                let instructions = if duplicate {
                                    vec![instruction.clone(), instruction]
                                } else {
                                    vec![instruction]
                                };
                                chain.sign(&customer, instructions, time)
                            },
                        );
                        let pool = worker.state.ivm_execution_budget();
                        let mut attempt = SignatureDecodeAttempt {
                            block_hash: block.hash(&**worker.context.crypto.as_ref().unwrap()),
                            source: block.clone(),
                            decoder:
                                iroha_data_model::block::PreparedSignedBlockSignaturesDecode::new(
                                    &pool,
                                )
                                .unwrap(),
                            decoded: None,
                            returned_refusal: None,
                            amx_legs: None,
                        };
                        let original = attempt.original_decoded(&pool).unwrap();
                        test(
                            original,
                            &block,
                            &pool,
                            worker.state.state_view_generation(),
                            &**worker.context.crypto.as_ref().unwrap(),
                        );
                    },
                );
            })
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn completed_amx_worker_bank_refuses_replaced_source_parent_and_foreign_pool() {
    with_original_paid_bank(false, |original, source, pool, generation, crypto| {
        let baseline = pool.reserved_bytes();
        let mut bank = None;
        let requests = crate::test_allocations::allocations_during(|| {
            bank = Some(crate::sumeragi::amx::NativeAmxLegPreparations::new(
                original, source, pool, generation,
            ));
        });
        assert_eq!(
            requests, 0,
            "bank binding clones only existing charged shared source owners"
        );
        assert_eq!(
            pool.reserved_bytes(),
            baseline,
            "source binding creates no new physical reservation"
        );
        let bank = bank.unwrap();
        assert!(bank.matches_original(original, source, source, pool, generation));
        let replacement = original.clone();
        assert!(
            replacement
                .matches_resultless_proposal_wire(source.payload().as_slice())
                .unwrap()
        );
        assert_ne!(
            replacement.external_entrypoints_slice().as_ptr(),
            original.external_entrypoints_slice().as_ptr()
        );
        assert!(
            !bank.matches_original(&replacement, source, source, pool, generation),
            "equal-content replacement is not the actual retained original SignedBlock"
        );
        assert!(!bank.matches_original(
            original,
            source,
            source,
            pool,
            generation.checked_add(2).unwrap()
        ));
        assert!(!bank.matches_original(
            original,
            source,
            source,
            pool,
            generation.checked_add(1).unwrap()
        ));
        let foreign = iroha_allocation::AllocationBudget::new(pool.limit_bytes());
        assert!(!bank.matches_original(original, source, source, &foreign, generation));
        // Each replacement is genuinely restored with the same original signed
        // header, codeword and production crypto, not an untrusted fabricated body.
        for replace_payload in [true, false] {
            let restored = restored_same_content_source(source, pool, crypto, replace_payload);
            assert!(
                restored == *source,
                "semantic equality alone cannot identify physical source custody"
            );
            let original_owner = if replace_payload {
                source.payload().charged_source(pool)
            } else {
                source.availability().charged_source(pool)
            }
            .unwrap();
            let replacement_owner = if replace_payload {
                restored.payload().charged_source(pool)
            } else {
                restored.availability().charged_source(pool)
            }
            .unwrap();
            assert!(!std::ptr::eq(original_owner, replacement_owner));
            assert!(
                !bank.matches_original(original, source, &restored, pool, generation),
                "equal signed bytes with a different actual payload or availability owner must refuse"
            );
            assert!(
                !bank.matches_original(original, &restored, &restored, pool, generation),
                "a substituted retained source cannot rebind the bank's original charged owners"
            );
        }
        assert!(bank.matches_original(original, source, source, pool, generation));
        assert_eq!(
            pool.reserved_bytes(),
            baseline,
            "restored alternatives retire their actual charged backing"
        );
        drop(bank);
        assert_eq!(
            pool.reserved_bytes(),
            baseline,
            "shared bank anchors retire without premature source refund"
        );
    });
}

fn restored_same_content_source(
    source: &AvailableBody,
    pool: &iroha_allocation::AllocationBudget,
    crypto: &dyn iroha_sumeragi::crypto::Crypto,
    replace_payload: bool,
) -> AvailableBody {
    use iroha_sumeragi::availability::{AvailabilityFrame, BodyRestoration, PayloadBytes};
    let bytes = if replace_payload {
        source.payload().as_slice()
    } else {
        source.availability().as_slice()
    };
    let mut replacement = iroha_allocation::ChargedBuffer::new(bytes.len(), pool).unwrap();
    replacement.append(bytes).unwrap();
    let (availability, payload) = if replace_payload {
        (
            source.availability().clone(),
            PayloadBytes::from_charged(replacement, pool)
                .unwrap_or_else(|(_, error)| panic!("actual payload sharing: {error}")),
        )
    } else {
        (
            AvailabilityFrame::from_charged(replacement, pool)
                .unwrap_or_else(|(_, error)| panic!("actual availability sharing: {error}")),
            source.payload().clone(),
        )
    };
    BodyRestoration::new(
        source.source().clone(),
        source.header().clone(),
        availability,
        payload,
    )
    .complete(pool, crypto)
    .unwrap_or_else(|(_, error)| panic!("genuine signed body restoration: {error:?}"))
}

#[test]
fn completed_amx_worker_bank_preserves_equal_occurrences_and_metadata_refusal() {
    with_original_paid_bank(true, |original, source, pool, generation, _crypto| {
        use iroha_data_model::{isi::sumeragi_amx::PrepareAmxV1, transaction::Executable};
        let input = &original.external_entrypoints_slice()[0];
        let iroha_data_model::transaction::TransactionEntrypoint::External(signed) = input else {
            panic!("signed input");
        };
        let Executable::Instructions(instructions) = signed.instructions() else {
            panic!("direct ISIs");
        };
        let first = instructions[0]
            .as_any()
            .downcast_ref::<PrepareAmxV1>()
            .unwrap();
        let second = instructions[1]
            .as_any()
            .downcast_ref::<PrepareAmxV1>()
            .unwrap();
        assert_eq!(first, second);
        assert_ne!(
            first
                .transaction
                .leg(first.dataspace)
                .unwrap()
                .payload
                .as_ptr(),
            second
                .transaction
                .leg(second.dataspace)
                .unwrap()
                .payload
                .as_ptr()
        );
        let baseline = pool.reserved_bytes();
        let mut bank =
            crate::sumeragi::amx::NativeAmxLegPreparations::new(original, source, pool, generation);
        let observer =
            crate::sumeragi::amx::NativeLegRetryObservation::occupy_after_first_completion();
        let failure = crate::sumeragi::amx::NativeAmxLegExecution::new(&mut bank, input, 0)
            .with_leg(first, 0, pool, |_| Ok::<_, ()>(()))
            .expect_err("occupied original pool must retain graph before metadata admission");
        assert!(matches!(
            failure,
            crate::sumeragi::amx::NativeLegExecutionError::Metadata(
                iroha_allocation::ChargedBufferError::Admission(
                    iroha_allocation::AllocationRefusal::Capacity { .. }
                )
            )
        ));
        let retained = observer.snapshot();
        assert_eq!(retained.0, 1);
        observer.release_original_blocker();
        assert_eq!(
            pool.reserved_bytes(),
            baseline + retained.2.unwrap().retained_bytes,
            "metadata refusal retains actual graph and both canonical controls"
        );
        assert!(
            crate::sumeragi::amx::NativeAmxLegExecution::new(&mut bank, input, 0)
                .with_leg(first, 0, pool, |_| Ok::<_, ()>(()))
                .is_ok()
        );
        assert_eq!(observer.snapshot().0, 1);
        assert!(
            crate::sumeragi::amx::NativeAmxLegExecution::new(&mut bank, input, 0)
                .with_leg(second, 1, pool, |_| Ok::<_, ()>(()))
                .is_ok()
        );
        assert_eq!(
            observer.snapshot().0,
            2,
            "equal values at distinct signed ordinals decode independently"
        );
        assert!(
            crate::sumeragi::amx::NativeAmxLegExecution::new(&mut bank, input, 0)
                .with_leg(first, 0, pool, |_| Ok::<_, ()>(()))
                .is_ok()
        );
        assert_eq!(
            observer.snapshot().0,
            2,
            "exact completed occurrence reuses its original work"
        );
        assert!(bank.matches_original(original, source, source, pool, generation));
        drop(bank);
        assert_eq!(
            pool.reserved_bytes(),
            baseline,
            "all actual graph/control/metadata storage retires before its refunds"
        );
    });
}
