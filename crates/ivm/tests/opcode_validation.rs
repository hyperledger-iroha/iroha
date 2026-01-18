//! Opcode validation checks for reserved instruction slots.

use ivm::instruction;

#[test]
fn reserved_opcodes_are_invalid() {
    assert!(!instruction::wide::is_valid_opcode(
        instruction::wide::system::SYSTEM
    ));
    assert!(!instruction::wide::is_valid_opcode(
        instruction::wide::iso20022::MSG_CREATE
    ));
}
