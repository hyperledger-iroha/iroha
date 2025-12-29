#![cfg(feature = "ivm_zk_tests")]
use ivm::{halo2::compute_nullifier, poseidon2};

#[test]
fn test_compute_nullifier_matches_poseidon2() {
    let secret = 42u64;
    let serial = 17u64;
    let expected = poseidon2(secret, serial);
    assert_eq!(compute_nullifier(secret, serial), expected);
}
