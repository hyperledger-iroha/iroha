//! Opcode validation checks for instruction slots.

use ivm::instruction;

#[test]
fn syscallx_opcode_is_valid_but_reserved_iso_slot_is_invalid() {
    assert!(instruction::wide::is_valid_opcode(
        instruction::wide::system::SYSTEM
    ));
    assert!(!instruction::wide::is_valid_opcode(
        instruction::wide::iso20022::MSG_CREATE
    ));
}
