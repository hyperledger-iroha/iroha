//! Cold-profile, ICU growth and stable-sort allocation custody in one fresh process.

use iroha_allocation::AllocationBudget;
use iroha_model_base::name::{MAX_NAME_BYTES, Name};
#[path = "name_nfc_scratch_allocation/observer.rs"]
mod observer;
#[path = "name_nfc_scratch_allocation/state_path.rs"]
mod state_path;
use observer::measured;

// One test is intentional: this binary has no earlier Name call or parallel Name
// test that could initialize the process-global profile before the first measurement.
#[test]
fn cold_profile_and_all_nfc_backings_stay_within_the_reserved_request_bound() {
    #[derive(Clone, Copy)]
    struct SortValue(u32);
    let (cold, profile) = measured(|| Name::validate_canonical("first-cold-profile"));
    cold.unwrap();
    assert_eq!(
        profile.count, 0,
        "baked profile fingerprint and OnceLock initialization allocate nothing"
    );
    assert!(!profile.invalid);

    // CharacterAndClass is one u32. Exercise the pinned standard library's
    // maximum possible stable sort geometry separately, with backing made first.
    let mut sort_values: Vec<_> = (0..u32::try_from(MAX_NAME_BYTES * 4).unwrap())
        .rev()
        .map(SortValue)
        .collect();
    let ((), sort) = measured(|| sort_values.sort_by_key(|value| value.0));
    assert_eq!(
        sort.count, 0,
        "the complete <=1020-element sort stays on its 4 KiB stack buffer"
    );
    assert!(!sort.invalid);
    assert!(sort_values.windows(2).all(|pair| pair[0].0 <= pair[1].0));

    // Mix short ascending/descending runs and repeated keys. Pack original
    // positions into the same four-byte value so the general stable path must
    // preserve equal-key order without changing CharacterAndClass geometry.
    let mut interleaved: Vec<_> = (0..u32::try_from(MAX_NAME_BYTES * 4).unwrap())
        .map(|position| SortValue((((position * 73) % 127) << 10) | position))
        .collect();
    assert!(
        interleaved
            .windows(2)
            .any(|pair| pair[0].0 >> 10 < pair[1].0 >> 10)
    );
    assert!(
        interleaved
            .windows(2)
            .any(|pair| pair[0].0 >> 10 > pair[1].0 >> 10)
    );
    let ((), general_sort) = measured(|| interleaved.sort_by_key(|value| value.0 >> 10));
    assert_eq!(
        general_sort.count, 0,
        "general stable sorting must use the pinned stack scratch"
    );
    assert!(!general_sort.invalid);
    assert!(
        interleaved.windows(2).all(|pair| {
            let (left_key, right_key) = (pair[0].0 >> 10, pair[1].0 >> 10);
            left_key < right_key
                || (left_key == right_key && (pair[0].0 & 1023) < (pair[1].0 & 1023))
        }),
        "equal keys preserve original positions"
    );

    let mut corpus = vec![
        "ascii".to_owned(),
        "é".to_owned(),
        "e\u{301}".to_owned(),
        "a\u{202e}b".to_owned(),
    ];
    for count in [18, 31, 32, 33, 63, 64, 65, 100, 126] {
        corpus.push(format!("q{}", "\u{301}".repeat(count)));
        corpus.push(format!("q{}\u{300}", "\u{315}".repeat(count)));
    }
    let mut saw_growth = false;
    let mut saw_rejection = false;
    for raw in corpus {
        assert!(raw.len() <= MAX_NAME_BYTES);
        let demand = Name::canonical_validation_scratch_bytes(&raw);
        let budget = AllocationBudget::new(demand);
        let lease = budget.try_reserve_bytes(demand).unwrap();
        let (outcome, observed) = measured(|| Name::validate_canonical(&raw));
        saw_rejection |= outcome.is_err();
        saw_growth |= observed.count > 1;
        assert!(
            !observed.invalid,
            "unexpected parallel or retained allocation for {raw:?}"
        );
        assert_eq!(
            observed.live_allocations(),
            0,
            "physical ICU backing must be freed before its caller returns"
        );
        assert_eq!(observed.live_bytes, 0);
        assert!(
            observed.bytes <= demand,
            "request sum exceeds the original reservation for {raw:?}"
        );
        assert!(
            observed.peak <= demand,
            "old/new growth peak exceeds reservation for {raw:?}"
        );
        let requests = &observed.requests[..observed.count];
        assert!(
            requests
                .iter()
                .all(|bytes| *bytes >= 32 * 4 && bytes.is_power_of_two())
        );
        assert!(
            requests.windows(2).all(|pair| pair[0] < pair[1]),
            "one SmallVec grows monotonically; extra sort storage is not permitted"
        );
        assert_eq!(
            budget.reserved_bytes(),
            demand,
            "original lease remains held after physical scratch reclamation"
        );
        drop(lease);
        assert_eq!(budget.reserved_bytes(), 0);
    }
    assert!(saw_growth && saw_rejection);
    let (positive, observed) = measured(|| {
        let mut bytes = Vec::<u8>::with_capacity(std::hint::black_box(32));
        bytes.resize(32, 0);
        bytes.reserve_exact(std::hint::black_box(64));
        drop(bytes);
        true
    });
    assert!(
        positive && observed.count >= 2 && observed.live_allocations() == 0 && !observed.invalid
    );
    state_path::long_path_census();
}
