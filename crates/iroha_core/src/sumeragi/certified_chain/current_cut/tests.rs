//! Original current-cut verdict, logical accounting and physical retirement regressions.

use super::*;
use crate::{
    state::World,
    sumeragi::test_chain::{CertifiedTestChain, Signers, TestChainConfig},
};
use iroha_data_model::sumeragi::finality::NativeFinalityLimits;

fn limits() -> NativeFinalityLimits {
    NativeFinalityLimits {
        block_bytes: iroha_data_model::sumeragi::finality::NATIVE_FINALITY_MAX_BLOCK_BYTES,
        journal_bytes: iroha_data_model::sumeragi::finality::NATIVE_FINALITY_MAX_JOURNAL_BYTES,
        block_count: 3,
        allocated_bytes: 240_000_000,
    }
}
fn source_limits() -> CurrentCutSourceLimits {
    let value = limits();
    CurrentCutSourceLimits::new(value.block_bytes, value.journal_bytes, value.block_count).unwrap()
}
fn unlimited_observation() -> norito::core::DecodeBudgetContext {
    norito::core::DecodeBudgetContext::new(norito::DecodeLimits::new(
        usize::MAX,
        usize::MAX,
        usize::MAX,
        usize::MAX,
        128,
    ))
}
fn read(
    chain: &CertifiedTestChain,
) -> Result<CurrentCertifiedCut, ExecutionAttemptError<QueryExecutionFail>> {
    let limits = limits();
    let frames = AllocationBudget::new(limits.journal_bytes);
    let graphs = AllocationBudget::new(limits.allocated_bytes);
    let result = read_current_certified_cut(
        &chain.state().view(),
        chain.height(),
        chain.root_scope(),
        source_limits(),
        &frames,
        &graphs,
    );
    assert_eq!(frames.reserved_bytes(), 0);
    assert_eq!(graphs.reserved_bytes(), 0);
    result
}
fn original(
    chain: &CertifiedTestChain,
) -> Result<CommittedBlock, ExecutionAttemptError<QueryExecutionFail>> {
    let view = chain.state().view();
    let chain = CertifiedChain::new_with_source_admission(&view, |_, _| Ok(()))?;
    chain
        .certified_from_execution(NonZeroUsize::new(view.height()).unwrap(), |_, _| Ok(()))
        .map(CertifiedBlock::into_committed)
}

#[test]
fn current_cut_retirement_preserves_original_logical_totals_including_genesis_parent() {
    let mut chain = CertifiedTestChain::start(TestChainConfig::new(World::new(), 1_000)).unwrap();
    for height in 2..=3 {
        chain.commit(Vec::new());
        let old_counter = unlimited_observation();
        let old = old_counter.with(|| original(&chain)).unwrap();
        let new_counter = unlimited_observation();
        let cut = new_counter.with(|| read(&chain)).unwrap();
        assert_eq!(cut.height(), height);
        assert_eq!(cut.id(), old.id());
        assert_eq!(
            cut.world_state_root(),
            old.commitment().execution.world_state_root
        );
        assert_eq!(
            new_counter.consumed_allocated_bytes(),
            old_counter.consumed_allocated_bytes()
        );
        let bytes = old_counter.consumed_allocated_bytes() as usize;
        for cap in [bytes - 1, bytes] {
            let outer = norito::core::DecodeBudgetContext::new(norito::DecodeLimits::new(
                usize::MAX,
                usize::MAX,
                usize::MAX,
                cap,
                128,
            ));
            let result = outer.with(|| read(&chain));
            if cap < bytes {
                assert!(matches!(result, Err(ExecutionAttemptError::Deferred(_))));
            } else {
                assert_eq!(result.unwrap().id(), cut.id());
            }
        }
    }
}

#[test]
fn current_cut_rejects_foreign_stale_and_missing_original_sources() {
    let mut chain = CertifiedTestChain::start(TestChainConfig::new(World::new(), 1_000)).unwrap();
    chain.commit(Vec::new());
    chain.commit(Vec::new());
    let view = chain.state().view();
    let frames = AllocationBudget::new(limits().journal_bytes);
    let graphs = AllocationBudget::new(limits().allocated_bytes);
    for height in [1, 2, 4] {
        assert!(matches!(
            read_current_certified_cut(
                &view,
                height,
                chain.root_scope(),
                source_limits(),
                &frames,
                &graphs
            ),
            Err(ExecutionAttemptError::Rejected(_))
        ));
    }
    drop(view);
    let (private, _, _) = crate::sumeragi::test_chain::native_amx_approval_route_fixture();
    assert!(matches!(
        read_current_certified_cut(
            &private.state().view(),
            private.height(),
            SumeragiRootScope::Global,
            source_limits(),
            &frames,
            &graphs
        ),
        Err(ExecutionAttemptError::Rejected(_))
    ));
    chain
        .kura()
        .corrupt_canonical_body_for_testing(NonZeroUsize::new(2).unwrap())
        .unwrap();
    assert!(matches!(
        read(&chain),
        Err(ExecutionAttemptError::Rejected(_))
    ));
    assert!(matches!(
        original(&chain),
        Err(ExecutionAttemptError::Rejected(_))
    ));
    assert_eq!(frames.reserved_bytes(), 0);
    assert_eq!(graphs.reserved_bytes(), 0);
}

#[test]
fn current_cut_keeps_invalid_quorums_invalid_and_local_decode_refusal_local() {
    for signers in [Signers::BelowQuorum, Signers::All] {
        let mut chain =
            CertifiedTestChain::start(TestChainConfig::new(World::new(), 1_000)).unwrap();
        chain.commit(Vec::new());
        chain.corrupt_local_quorum_for_test(2, signers);
        assert!(matches!(
            original(&chain),
            Err(ExecutionAttemptError::Rejected(_))
        ));
        assert!(matches!(
            read(&chain),
            Err(ExecutionAttemptError::Rejected(_))
        ));
        let outer = norito::core::DecodeBudgetContext::new(norito::DecodeLimits::new(
            usize::MAX,
            usize::MAX,
            usize::MAX,
            1,
            128,
        ));
        assert!(matches!(
            outer.with(|| read(&chain)),
            Err(ExecutionAttemptError::Deferred(_))
        ));
    }
}

#[test]
fn current_cut_consecutive_maximum_payments_fit_live_custody_without_logical_relaxation() {
    use crate::sumeragi::test_chain::native_payment::{PaymentKind, maximum_payment_chain};
    use crate::test_allocations::live_allocations_during;
    let chain = maximum_payment_chain(PaymentKind::Applied);
    let old_counter = unlimited_observation();
    let old = old_counter.with(|| original(&chain)).unwrap();
    assert!(old_counter.consumed_allocated_bytes() > limits().allocated_bytes as u64);
    let expected = old.id();
    drop(old);
    for _ in 0..2 {
        let counter = unlimited_observation();
        let (result, live) = live_allocations_during(65_536, || counter.with(|| read(&chain)));
        assert_eq!(result.unwrap().id(), expected);
        assert_eq!(
            counter.consumed_allocated_bytes(),
            old_counter.consumed_allocated_bytes()
        );
        assert!(!live.incomplete);
        assert!(
            live.peak_with_realloc_overlap_bytes
                <= limits().journal_bytes + limits().allocated_bytes,
            "actual decoder working set exceeds retained physical custody: {live:?}"
        );
        eprintln!("current-cut actual allocator census: {live:?}");
    }
}

#[test]
fn current_cut_snapshot_matches_original_and_refuses_tampered_or_later_cuts() {
    use iroha_data_model::{domain::Domain, isi::Register, sns::lease::SnsLeaseProofRefV1};
    use iroha_model_base::domain::DomainId;
    let domain = DomainId::try_new("cutproof", "universal").unwrap();
    let selector = crate::sns::selector_for_domain(&domain).unwrap();
    let mut config = TestChainConfig::new(World::new(), 1_000);
    config.genesis_instructions = vec![Register::domain(Domain::new(domain)).into()];
    let mut chain = CertifiedTestChain::start(config).unwrap();
    chain.commit(Vec::new());
    let original = original(&chain).unwrap();
    let cut = read(&chain).unwrap();
    let budget = AllocationBudget::new(48_000_000);
    let expected = chain
        .state()
        .with_sns_lease_snapshot_reference_for_test(
            &original,
            &selector,
            &budget,
            |world, bytes| {
                norito::encode_canonical(&SnsLeaseProofRefV1::new(world, bytes))
                    .map_err(|error| error.to_string())
            },
        )
        .unwrap();
    let actual = chain
        .state()
        .with_native_sns_lease_cut_v1(&cut, &selector, &budget, |world, bytes| {
            norito::encode_canonical(&SnsLeaseProofRefV1::new(world, bytes))
                .map_err(|error| error.to_string())
        })
        .unwrap();
    assert_eq!(actual, expected);
    let assert_error_parity = |state: &crate::state::State,
                               selector: &iroha_data_model::sns::NameSelectorV1,
                               budget: &AllocationBudget| {
        let original_error = state
            .with_sns_lease_snapshot_reference_for_test(
                &original,
                selector,
                budget,
                |_, _| -> Result<(), String> {
                    panic!("refused original cut cannot reach publication")
                },
            )
            .unwrap_err();
        let projected_error = state
            .with_native_sns_lease_cut_v1(&cut, selector, budget, |_, _| -> Result<(), String> {
                panic!("refused projected cut cannot reach publication")
            })
            .unwrap_err();
        assert_eq!(projected_error.to_string(), original_error.to_string());
    };
    assert_error_parity(chain.state(), &selector, &AllocationBudget::new(0));
    let missing = iroha_data_model::sns::NameSelectorV1::new(
        iroha_data_model::sns::DATASPACE_ALIAS_SUFFIX_ID,
        "missing",
    )
    .unwrap();
    assert_error_parity(chain.state(), &missing, &budget);
    let mut changed = [cut; 4];
    changed[0].core_hash = Hash32([99; 32]);
    changed[1].result = Hash32([99; 32]);
    changed[2].world_state_root = Hash::new(b"changed certified World");
    changed[3].block_time_ms += 1;
    for changed in changed {
        assert!(
            chain
                .state()
                .with_native_sns_lease_cut_v1(
                    &changed,
                    &selector,
                    &budget,
                    |_, _| -> Result<(), String> {
                        panic!("a changed scalar cut cannot reach publication")
                    }
                )
                .is_err()
        );
    }
    chain.commit(Vec::new());
    assert_error_parity(chain.state(), &selector, &budget);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn current_cut_corrupt_native_frame_keeps_original_codec_verdict_even_with_warm_state() {
    let mut chain = CertifiedTestChain::start(TestChainConfig::new(World::new(), 1_000)).unwrap();
    chain.commit(Vec::new());
    chain.commit(Vec::new());
    let expected = read(&chain).unwrap().id();
    let height = NonZeroUsize::new(3).unwrap();
    chain.kura().corrupt_native_frame_for_test(height);
    let old = original(&chain).unwrap_err();
    let new = read(&chain).unwrap_err();
    assert!(matches!(old, ExecutionAttemptError::Rejected(_)));
    assert!(matches!(new, ExecutionAttemptError::Rejected(_)));
    assert_eq!(new.to_string(), old.to_string());
    chain.kura().corrupt_native_frame_for_test(height);
    assert_eq!(read(&chain).unwrap().id(), expected);
}

#[test]
fn current_cut_source_policy_rejects_unbounded_and_contradictory_geometry() {
    use iroha_data_model::sumeragi::finality::{
        NATIVE_FINALITY_MAX_BLOCK_BYTES, NATIVE_FINALITY_MAX_BLOCK_COUNT,
        NATIVE_FINALITY_MAX_JOURNAL_BYTES,
    };
    for (block, journal, count) in [
        (0, 1, 1),
        (
            NATIVE_FINALITY_MAX_BLOCK_BYTES + 1,
            NATIVE_FINALITY_MAX_JOURNAL_BYTES,
            3,
        ),
        (2, 1, 3),
        (1, NATIVE_FINALITY_MAX_JOURNAL_BYTES + 1, 3),
        (1, 1, 0),
        (1, 1, NATIVE_FINALITY_MAX_BLOCK_COUNT + 1),
    ] {
        assert!(CurrentCutSourceLimits::new(block, journal, count).is_err());
    }
    assert!(CurrentCutSourceLimits::new(1, 1, 1).is_ok());
}

#[test]
fn current_cut_projected_payload_preserves_original_admission_and_rejection_order() {
    fn assert_verdict_parity(
        original: &Result<(), VerificationReadError>,
        projected: &Result<(), VerificationReadError>,
    ) {
        match (original, projected) {
            (Ok(()), Ok(())) => {}
            (Err(VerificationReadError::Source(old)), Err(VerificationReadError::Source(new))) => {
                assert_eq!(new, old);
            }
            (
                Err(VerificationReadError::Deferred(old)),
                Err(VerificationReadError::Deferred(new)),
            ) => {
                assert_eq!(
                    new.decode_refusal().unwrap().resource_error(),
                    old.decode_refusal().unwrap().resource_error()
                );
            }
            verdicts => panic!("original/projected verifier classification changed: {verdicts:?}"),
        }
    }

    let mut chain = CertifiedTestChain::start(TestChainConfig::new(World::new(), 1_000)).unwrap();
    chain.commit(Vec::new());
    let pool = AllocationBudget::new(64 * 1024 * 1024);
    let block = read_durable_pinned_block_bounded(
        chain.kura(),
        NonZeroUsize::new(2).unwrap(),
        chain.committed(2).block_hash(),
        &pool,
        limits().block_bytes,
    )
    .unwrap();
    let original = read_frame(block, 2, chain.root_scope()).unwrap();
    let parent = chain.committed(1);
    let schedule::ScheduledSlot::Ready(scheduled) = &parent.commitment().schedule.next else {
        panic!("signed genesis authorizes height 2");
    };
    let mut validation = EpochValidationScope::new();
    let config = scheduled
        .height_config_with_validation(&mut validation)
        .unwrap();
    let authority = VerifiedAuthority::new(scheduled.epoch.clone(), 2, &mut validation).unwrap();
    let payload = query_reader::capture_payload(
        &original.block,
        original.header().unwrap().payload_len as usize,
        limits().block_bytes,
        &pool,
        2,
    )
    .unwrap();
    let wire = original.block.canonical_wire_identity().unwrap();
    for case in 0..4 {
        let mut current = original.clone();
        let mut header = current.header().unwrap().clone();
        // Private verifier inputs isolate the shared predicate after C1. Public
        // readers still reject these header substitutions before projection.
        match case {
            0 => {}
            1 => header.payload_hash = Hash32([99; 32]),
            2 => header.payload_len += 1,
            3 => header.payload_len = u32::try_from(MAX_PROPOSAL_BYTES + 1).unwrap(),
            _ => unreachable!(),
        }
        current.header = Some(PrefixHeader::Decoded(header));
        let (projection, header) =
            query_projection::CheckedFrameProjection::from_current_original(current.clone(), &pool)
                .unwrap();
        let source = ProjectedCertificateSource {
            frame: &projection,
            header: header.as_ref(),
            payload: Some(&payload),
            wire,
            admit_payload: true,
        };
        let check = |source, cap| {
            let context = norito::core::DecodeBudgetContext::new(norito::DecodeLimits::new(
                usize::MAX,
                usize::MAX,
                usize::MAX,
                cap,
                128,
            ));
            let result = context.with(|| {
                verify_availability_source(
                    source,
                    &config,
                    &authority.crypto,
                    None,
                    &mut state_certificate::query_scratch_admission,
                    None,
                )
            });
            (result, context.consumed_allocated_bytes())
        };
        let (old, charges) = check(CertificateSource::Original(&current), usize::MAX);
        let (new, projected_charges) = check(CertificateSource::Projected(&source), usize::MAX);
        assert_verdict_parity(&old, &new);
        assert_eq!(projected_charges, charges);
        assert_eq!(old.is_ok(), case == 0);
        if case == 3 {
            assert_eq!(
                charges, 0,
                "oversized payloads reject before scratch admission"
            );
        }
        let charges = usize::try_from(charges).unwrap();
        for cap in [0, charges.saturating_sub(1), charges] {
            let (old, old_charges) = check(CertificateSource::Original(&current), cap);
            let (new, new_charges) = check(CertificateSource::Projected(&source), cap);
            assert_verdict_parity(&old, &new);
            assert_eq!(new_charges, old_charges);
            assert_eq!(
                matches!(old, Err(VerificationReadError::Deferred(_))),
                cap < charges,
                "a local refusal before completed payload validation stays local"
            );
        }
    }
    drop((payload, original));
    assert_eq!(pool.reserved_bytes(), 0);
}
