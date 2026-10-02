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
        let original_witness = iroha_crypto::HashOf::new(&original.witness);
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
            Err(PublicationError::Retryable(_))
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
            iroha_crypto::HashOf::new(&retained.witness),
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
            iroha_crypto::HashOf::new(&retained.witness),
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
        assert!(matches!(
            worker.execute(&block, hash),
            Some(ExecOutcome::Valid(_))
        ));
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
