//! Gas schedule determinism guardrails.

use hex_literal::hex;

#[test]
fn schedule_hash_matches_expected_digest() {
    let digest = ivm::gas::schedule_hash();
    // Blake2b-32 over `(opcode || le_u64(cost))` table, LSB set per `iroha_crypto::Hash`.
    let expected = hex!("2160d7da698d444c3d41bab913edb3ba13079ca4e858fa75185dc20c4af153d9");
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
