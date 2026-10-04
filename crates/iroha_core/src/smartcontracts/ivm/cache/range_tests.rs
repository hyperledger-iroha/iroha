//! Shared admission ranges without a preliminary decoded CNTR graph.

use super::*;
use norito::core::{DecodeLimits, with_decode_limits_measured, with_decode_limits_scope};

#[test]
fn executable_summary_uses_one_canonical_contract_admission_pass() {
    let bytes = super::tests::minimal_program();
    let limits = DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, usize::MAX, usize::MAX);
    let (admitted, usage) =
        with_decode_limits_measured(limits, || ivm::verify_contract_artifact(&bytes));
    let admitted = admitted.unwrap();
    assert!(usage.total_allocated_bytes() > 0);
    let original = iroha_allocation::AllocationBudget::new(16 * 1024 * 1024);
    let prepared = PreparedContractCache::with_execution_budget(0, original.clone());
    let mut cache = IvmCache::with_prepared_contract_cache(0, prepared);
    let allowance = DecodeLimits::new(
        usize::MAX,
        usize::MAX,
        usize::MAX,
        usage.total_allocated_bytes(),
        usize::MAX,
    );
    let summary = with_decode_limits_scope(allowance, || cache.summarize_executable(&bytes))
        .expect("header-only dispatch leaves the full CNTR allowance for shared admission");
    let ExecutableProgramSummary::Contract(summary) = summary else {
        panic!("a CNTR declaration must not downgrade to generic admission");
    };
    assert_eq!(summary.code_hash, admitted.code_hash);
    assert_eq!(summary.code_offset, admitted.code_offset);
    assert_eq!(summary.prepared.artifact(), bytes);
    assert_eq!(
        summary.prepared.contract_interface(),
        &admitted.contract_interface
    );
    assert_eq!(summary.prepared.manifest(), &admitted.manifest);
    let retained = summary.prepared.clone();
    drop(summary);
    drop(cache);
    let charged = original.reserved_bytes();
    assert!(charged > 0);
    original.set_limit_bytes(0);
    assert_eq!(retained.artifact(), bytes);
    assert_eq!(original.reserved_bytes(), charged);
    drop(retained);
    assert_eq!(original.reserved_bytes(), 0);
}
