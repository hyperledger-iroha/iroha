//! Strict candidate parity and physical allocation/cache observations.

use super::{
    Ed25519Sha512, PUBLIC_KEY_PARSE_CACHE_CONSULTED, public_key_parse_cache_consulted_for_tests,
    public_key_parse_cache_stats_for_tests,
};
use crate::{
    Algorithm, PublicKey, ed25519_public_key_is_valid, test_allocations::without_allocations,
};
use curve25519_dalek::constants::{ED25519_BASEPOINT_POINT, EIGHT_TORSION};

fn strict_cases() -> [([u8; 32], bool); 7] {
    let valid = ED25519_BASEPOINT_POINT.compress().to_bytes();
    let mut identity = [0; 32];
    identity[0] = 1;
    let mut noncanonical_identity = [0xff; 32];
    noncanonical_identity[0] = 0xee;
    noncanonical_identity[31] = 0x7f;
    let mut noncanonical_point = noncanonical_identity;
    noncanonical_point[0] = 0xf0;
    let mixed_torsion = (ED25519_BASEPOINT_POINT + EIGHT_TORSION[1])
        .compress()
        .to_bytes();
    [
        (valid, true),
        ([0; 32], false),
        (identity, false),
        (noncanonical_identity, false),
        (noncanonical_point, false),
        ([2; 32], false),
        (mixed_torsion, false),
    ]
}

#[test]
fn candidate_rejects_every_strict_invalid_point_class_without_allocating() {
    for (candidate, expected) in strict_cases() {
        let actual = without_allocations(|| ed25519_public_key_is_valid(&candidate));
        assert_eq!(actual, expected, "candidate {candidate:02x?}");
        assert_eq!(
            actual,
            PublicKey::from_bytes(Algorithm::Ed25519, &candidate).is_ok(),
            "owning parser parity for {candidate:02x?}",
        );
    }
}

#[test]
fn candidate_never_consults_cold_or_populated_parse_cache() {
    std::thread::spawn(|| {
        let cases = strict_cases();
        assert!(!public_key_parse_cache_consulted_for_tests());
        without_allocations(|| {
            for (candidate, expected) in cases {
                assert_eq!(ed25519_public_key_is_valid(&candidate), expected);
            }
        });
        assert!(!public_key_parse_cache_consulted_for_tests());
        // A real cached parse is a positive control for this same thread's observer.
        Ed25519Sha512::parse_public_key(&cases[0].0).expect("valid basepoint");
        assert!(public_key_parse_cache_consulted_for_tests());
        let before = public_key_parse_cache_stats_for_tests();
        PUBLIC_KEY_PARSE_CACHE_CONSULTED.with(|consulted| consulted.set(false));
        without_allocations(|| {
            for (candidate, expected) in cases {
                assert_eq!(ed25519_public_key_is_valid(&candidate), expected);
            }
        });
        assert!(!public_key_parse_cache_consulted_for_tests());
        assert_eq!(public_key_parse_cache_stats_for_tests(), before);
    })
    .join()
    .expect("candidate observation thread");
}

#[test]
fn candidate_matches_owning_parser_for_deterministic_candidate_corpus() {
    for seed in 0_u16..256 {
        let candidate: [u8; 32] = crate::Hash::new(seed.to_be_bytes()).into();
        let expected = PublicKey::from_bytes(Algorithm::Ed25519, &candidate).is_ok();
        let actual = without_allocations(|| ed25519_public_key_is_valid(&candidate));
        assert_eq!(actual, expected, "seed {seed}");
    }
}
