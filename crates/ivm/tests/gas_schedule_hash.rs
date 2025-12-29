//! Gas schedule determinism guardrails.

use hex_literal::hex;

#[test]
fn schedule_hash_matches_expected_digest() {
    let digest = ivm::gas::schedule_hash();
    // Blake2b-32 over `(opcode || le_u64(cost))` table, LSB set per `iroha_crypto::Hash`.
    let expected = hex!("dadfa7f0d9da4288472fc907cf9634cdc4e2f22f1720b3b00cfd19b0d0ca463b");
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
