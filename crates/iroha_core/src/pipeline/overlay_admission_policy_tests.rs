#[test]
fn analysis_refusals_keep_their_original_owner_in_overlay() {
    let budget = iroha_allocation::AllocationBudget::new(1);
    let occupied = budget.try_reserve_bytes(1).unwrap();
    let refusal = budget.try_reserve_bytes(1).unwrap_err();
    for metadata in [false, true] {
        for allocation in [true, false] {
            let original = if allocation {
                IvmError::Deferred(ivm::error::VmDeferral::AllocationDeferred(refusal.clone()))
            } else {
                IvmError::Deferred(ivm::error::VmDeferral::ExecutionDeferred(
                    ivm::error::ExecutionDeferral::AllocationUnavailable,
                ))
            };
            let expected_reason = original.execution_deferral().unwrap();
            let error = if metadata {
                ProgramAnalysisError::Metadata(original)
            } else {
                ProgramAnalysisError::Decode(original)
            };
            let mapped = map_program_analysis_error(error);
            assert!(matches!(mapped, OverlayBuildError::IvmLoad(_)));
            let retained = mapped.into_execution_deferral().unwrap();
            assert_eq!(retained.reason(), expected_reason);
            assert_eq!(
                retained.allocation_refusal(),
                allocation.then_some(&refusal)
            );
        }
    }
    let malformed = map_program_analysis_error(ProgramAnalysisError::Metadata(IvmError::Trap(
        ivm::error::VmTrap::InvalidMetadata,
    )));
    assert!(matches!(malformed, OverlayBuildError::IvmHeaderParse));
    assert!(matches!(
        malformed.into_execution_deferral(),
        Err(OverlayBuildError::IvmHeaderParse)
    ));
    drop(occupied);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn generic_preparation_preserves_original_pool_refusal_in_overlay() {
    use crate::smartcontracts::ivm::cache::PreparedContractCache;
    use iroha_allocation::{AllocationBudget, AllocationRefusal};

    // The policy-less cache methods prepare under the interface (baseline) profile.
    let mut program = ivm::ProgramMetadata {
        max_cycles: 1_000,
        ..ivm::ProgramMetadata::default()
    }
    .encode_for_profile(ivm::IvmProfileId::BASELINE_ABI_V1, 0);
    program.extend_from_slice(&ivm_abi::baseline::encoding::encode_halt().to_le_bytes());
    let budget = AllocationBudget::new(64 * 1024 * 1024);
    let occupied = budget.try_reserve_bytes(budget.limit_bytes()).unwrap();
    let mut cache = IvmCache::with_prepared_contract_cache(
        0,
        PreparedContractCache::with_execution_budget(0, budget.clone()),
    );
    let original = cache.summarize_executable(&program).unwrap_err();
    let expected = match &original {
        IvmError::Deferred(ivm::error::VmDeferral::AllocationDeferred(original)) => {
            original.clone()
        }
        _ => panic!("original allocation refusal"),
    };
    assert!(matches!(expected, AllocationRefusal::Capacity { .. }));
    let mapped = map_program_summary_error(original);
    assert!(matches!(mapped, OverlayBuildError::IvmLoad(_)));
    assert_eq!(
        mapped
            .into_execution_deferral()
            .unwrap()
            .allocation_refusal(),
        Some(&expected)
    );
    drop(occupied);
    assert!(cache.summarize_executable(&program).is_ok());
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn checked_keypair_preserves_default_algorithm() {
    assert_eq!(checked_keypair().algorithm(), Algorithm::default());
}
fn zk_meta(max_cycles: u64) -> ivm::ProgramMetadata {
    ivm::ProgramMetadata {
        mode: ivm::ivm_mode::ZK,
        max_cycles,
        ..ivm::ProgramMetadata::default()
    }
}
const C10: ivm::IvmEngine = ivm::IvmEngine::C10;
const BASELINE: ivm::IvmEngine = ivm::IvmEngine::Baseline;
#[test]
fn pre_execution_trace_ceiling_accepts_exact_bound() {
    enforce_pre_execution_policy_for(
        C10,
        NonZeroU64::new(42).expect("test ceiling is non-zero"),
        &zk_meta(42),
    )
    .expect("a ZK trace bound at the configured ceiling should be admitted");
}
#[test]
fn pre_execution_policy_has_no_cycle_check_for_non_zk_c10_artifacts() {
    enforce_pre_execution_policy_for(
        C10,
        NonZeroU64::new(1).expect("test ceiling is non-zero"),
        &ivm::ProgramMetadata::default(),
    )
    .expect("gas is the only budget of a non-ZK C10 artifact");
}
#[test]
fn baseline_pre_execution_policy_bounds_every_cycle_limit() {
    let ceiling = NonZeroU64::new(42).expect("test ceiling is non-zero");
    assert!(matches!(
        enforce_pre_execution_policy_for(BASELINE, ceiling, &ivm::ProgramMetadata::default()),
        Err(OverlayBuildError::HeaderPolicy(
            IvmAdmissionError::MissingMaxCycles
        ))
    ));
    let at_ceiling = ivm::ProgramMetadata {
        max_cycles: 42,
        ..ivm::ProgramMetadata::default()
    };
    enforce_pre_execution_policy_for(BASELINE, ceiling, &at_ceiling)
        .expect("a baseline cycle limit at the ceiling is admitted");
    let above = ivm::ProgramMetadata {
        max_cycles: 43,
        ..ivm::ProgramMetadata::default()
    };
    assert!(matches!(
        enforce_pre_execution_policy_for(BASELINE, ceiling, &above),
        Err(OverlayBuildError::HeaderPolicy(
            IvmAdmissionError::MaxCyclesExceedsUpperBound(_)
        ))
    ));
}
#[test]
fn header_policy_binds_the_trace_bound_to_zk_mode() {
    assert!(matches!(
        validate_header_policy_for(C10, &zk_meta(0)),
        Err(IvmAdmissionError::MissingMaxCycles)
    ));
    let bound_without_zk = ivm::ProgramMetadata {
        max_cycles: 7,
        ..ivm::ProgramMetadata::default()
    };
    assert!(matches!(
        validate_header_policy_for(C10, &bound_without_zk),
        Err(IvmAdmissionError::BytecodeDecodingFailed(message)) if message.contains("trace bound 7")
    ));
    assert!(validate_header_policy_for(C10, &ivm::ProgramMetadata::default()).is_ok());
    assert!(validate_header_policy_for(C10, &zk_meta(1)).is_ok());
    // The baseline requires the cycle limit of every program, ZK or not.
    assert!(validate_header_policy_for(BASELINE, &bound_without_zk).is_ok());
}
#[test]
fn pre_execution_trace_ceiling_rejects_over_bound() {
    let error = enforce_pre_execution_policy_for(
        C10,
        NonZeroU64::new(42).expect("test ceiling is non-zero"),
        &zk_meta(43),
    )
    .expect_err("a ZK trace bound above the configured ceiling must fail closed");
    assert!(matches!(
        error,
        OverlayBuildError::HeaderPolicy(
            IvmAdmissionError::MaxCyclesExceedsUpperBound(info)
        ) if info.max_cycles == 43 && info.upper_bound == 42
    ));
}
#[test]
fn empty_overlay_is_noop() {
    let ovl = TxOverlay::default();
    assert!(ovl.is_empty());
}

#[test]
fn header_policy_rejects_retired_minor_before_other_policy_errors() {
    let retired = ivm::ProgramMetadata {
        version_minor: 0,
        mode: 0xff,
        max_cycles: 0,
        abi_version: 2,
        ..ivm::ProgramMetadata::default()
    };
    for engine in ivm::IvmEngine::ALL {
        assert!(matches!(validate_header_policy_for(engine, &retired),
            Err(IvmAdmissionError::UnsupportedVersion(info)) if info.major == 1 && info.minor == 0));
    }
    assert!(validate_header_policy_for(C10, &ivm::ProgramMetadata::default()).is_ok());
}
