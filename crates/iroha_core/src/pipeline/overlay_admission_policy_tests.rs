#[test]
fn checked_keypair_preserves_default_algorithm() {
    assert_eq!(checked_keypair().algorithm(), Algorithm::default());
}
#[test]
fn pre_execution_cycle_ceiling_accepts_exact_bound() {
    let meta = ivm::ProgramMetadata {
        max_cycles: 42,
        ..ivm::ProgramMetadata::default()
    };
    enforce_pre_execution_policy(
        NonZeroU64::new(42).expect("test ceiling is non-zero"),
        &meta,
    )
    .expect("artifact at the configured ceiling should be admitted");
}
#[test]
fn header_policy_rejects_zero_cycle_limit() {
    let meta = ivm::ProgramMetadata {
        max_cycles: 0,
        ..ivm::ProgramMetadata::default()
    };
    assert!(matches!(
        validate_header_policy(&meta),
        Err(IvmAdmissionError::MissingMaxCycles)
    ));
}
#[test]
fn pre_execution_cycle_ceiling_rejects_over_bound() {
    let meta = ivm::ProgramMetadata {
        max_cycles: 43,
        ..ivm::ProgramMetadata::default()
    };
    let error = enforce_pre_execution_policy(
        NonZeroU64::new(42).expect("test ceiling is non-zero"),
        &meta,
    )
    .expect_err("artifact above the configured ceiling must fail closed");
    assert!(matches!(
        error,
        OverlayBuildError::HeaderPolicy(
            IvmAdmissionError::MaxCyclesExceedsUpperBound(info)
        ) if info.max_cycles == 43 && info.upper_bound == 42
    ));
}
