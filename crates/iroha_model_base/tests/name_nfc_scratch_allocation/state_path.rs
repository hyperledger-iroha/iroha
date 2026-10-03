//! Long canonical paths exercise ICU growth and simultaneous stable-sort scratch.

use super::*;
use iroha_model_base::state_path::{MAX_STATE_PATH_BYTES, StatePath};

pub(super) fn long_path_census() {
    let mut cases = vec![
        "ascii/path".repeat(1024),
        "q\u{301}".repeat(1024),
        format!("q{}", "\u{301}".repeat(8191)),
        format!("q{}\u{300}", "\u{315}".repeat(8190)),
        format!("q{}{}", "\u{323}".repeat(2048), "\u{301}".repeat(2048)),
    ];
    // Multiple independent heap-sorted runs exercise the cumulative demand,
    // rather than only the maximum size of one ICU buffer or one sort.
    // U+1E08 decomposes to C + cedilla(202) + acute(230). Later dot-below
    // marks(220) require sorting, but recomposition preserves this exact NFC
    // spelling. Every segment is therefore visited before successful return.
    let multiple_sorts = format!("\u{1e08}{}", "\u{323}".repeat(1200)).repeat(6);
    cases.push(multiple_sorts.clone());
    let mut saw_sort_overlap = false;
    let mut saw_rejection = false;
    let mut saw_canonical = false;
    for raw in &cases {
        assert!(raw.len() <= MAX_STATE_PATH_BYTES);
        let demand = StatePath::canonical_validation_scratch_bytes(raw);
        let budget = AllocationBudget::new(demand);
        let lease = budget.try_reserve_bytes(demand).unwrap();
        let (outcome, observed) = measured(|| StatePath::validate_canonical(raw));
        saw_rejection |= outcome.is_err();
        saw_canonical |= outcome.is_ok();
        saw_sort_overlap |= observed.max_live >= 2;
        assert!(!observed.invalid, "untracked physical allocation");
        assert_eq!(observed.live_allocations(), 0);
        assert_eq!(observed.live_bytes, 0);
        assert!(
            observed.bytes <= demand,
            "cumulative requests exceed demand"
        );
        assert!(
            observed.peak <= demand,
            "simultaneous backings exceed demand"
        );
        assert_eq!(budget.reserved_bytes(), demand);
        if raw == &multiple_sorts {
            assert!(outcome.is_ok(), "all six sorted segments remain canonical");
            let sort_requests = observed.requests[..observed.count]
                .iter()
                .filter(|bytes| **bytes > 4096 && !bytes.is_power_of_two())
                .count();
            assert!(
                sort_requests >= 6,
                "each original segment must exercise physical sort scratch"
            );
            assert!(observed.max_live >= 2);
        }
        drop(lease);
        assert_eq!(budget.reserved_bytes(), 0);
        // The previous parser's exact normalization equality is a test-only
        // oracle; construction is outside the physical census.
        let normalized = Name::normalize(raw).unwrap();
        assert_eq!(outcome.is_ok(), normalized.as_ref() == raw);
        assert_eq!(raw.parse::<StatePath>().is_ok(), outcome.is_ok());
    }
    assert!(saw_sort_overlap && saw_rejection && saw_canonical);

    let raw = format!("q{}", "\u{301}".repeat(4096));
    let demand = StatePath::canonical_validation_scratch_bytes(&raw);
    let budget = AllocationBudget::new(demand - 1);
    let (refused, observed) = measured(|| {
        let _lease = budget.try_reserve_bytes(demand)?;
        Ok::<_, iroha_allocation::AllocationRefusal>(StatePath::validate_canonical(&raw))
    });
    assert!(refused.is_err());
    assert_eq!(observed.count, 0, "refusal precedes every ICU allocation");
    assert_eq!(budget.reserved_bytes(), 0);
    budget.set_limit_bytes(demand);
    let _lease = budget.try_reserve_bytes(demand).unwrap();
    StatePath::validate_canonical(&raw).unwrap();
}
