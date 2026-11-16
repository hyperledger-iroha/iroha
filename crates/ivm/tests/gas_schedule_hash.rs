//! Gas schedule determinism guardrails.
use hex_literal::hex;
#[test]
fn schedule_hash_matches_expected_digest() {
    let digest = ivm::gas::schedule_hash();
    // Blake2b-32 over the canonical opcode/host/numeric schedule descriptor,
    // with the LSB set per `iroha_crypto::Hash`.
    let expected = hex!("af5a27f5fc57ebfb9eae3c4f8a25091ce78d97ff7797dbc3b304a0091069731f");
    assert_eq!(digest.as_ref(), &expected);
}
#[test]
fn c10_schedule_hash_matches_expected_digest() {
    // The C10 engine's own schedule, which prices the runtime-lifecycle policy.
    let c10_expected = hex!("46aa31d828ba8b2dbc61017555b36817398bcaed9f417a54b555b927cce6e4cf");
    assert_eq!(ivm::gas::c10_schedule_hash().as_ref(), &c10_expected);
    assert_eq!(
        ivm::gas::schedule_hash_for(ivm::IvmEngine::C10),
        ivm::gas::c10_schedule_hash()
    );
}
#[test]
fn schedule_opcode_set_has_no_duplicates() {
    use std::collections::BTreeSet;
    let mut seen = BTreeSet::new();
    for &op in ivm::gas::SCHEDULE_OPCODES {
        assert!(seen.insert(op), "duplicate opcode 0x{op:02x} in schedule");
    }
}
