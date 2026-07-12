//! Gas schedule determinism guardrails.

use hex_literal::hex;

#[test]
fn schedule_hash_matches_expected_digest() {
    let digest = ivm::gas::schedule_hash();
    // Blake2b-32 over the canonical opcode/host/numeric schedule descriptor,
    // with the LSB set per `iroha_crypto::Hash`.
    let expected = hex!("34348936c3340c01a1bc2fe597d54f3fc3540c019528d9a2f41523a2beb29ea3");
    assert_eq!(digest.as_ref(), &expected);
}

#[test]
fn schedule_opcode_set_has_no_duplicates() {
    use std::collections::BTreeSet;
    let mut seen = BTreeSet::new();
    for &op in ivm::gas::SCHEDULE_OPCODES {
        assert!(seen.insert(op), "duplicate opcode 0x{op:02x} in schedule");
    }
}
