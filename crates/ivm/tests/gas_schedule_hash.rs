//! Gas schedule determinism guardrails.
use hex_literal::hex;
#[test]
fn schedule_hash_matches_expected_digest() {
    let digest = ivm::gas::schedule_hash();
    // Blake2b-32 over the canonical opcode/host/numeric schedule descriptor,
    // with the LSB set per `iroha_crypto::Hash`.
    let expected = hex!("b5067c60883d9a72c2af520fc2f5612e065cf6cdd5ae0113b5f2583f9eda04ad");
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
