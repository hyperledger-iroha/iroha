//! Opcode validation checks for instruction slots.

use ivm::instruction;

#[test]
fn first_release_extension_opcodes_are_valid_but_reserved_iso_slot_is_invalid() {
    assert!(instruction::wide::is_valid_opcode(
        instruction::wide::system::SYSTEM
    ));
    assert!(instruction::wide::is_valid_opcode(
        instruction::wide::memory::LDLIT
    ));
    for opcode in [
        instruction::wide::control::JAL,
        instruction::wide::control::JMP,
        instruction::wide::control::JALS,
    ] {
        assert!(
            instruction::wide::is_valid_opcode(opcode),
            "compact direct-transfer opcode 0x{opcode:02x} must ship in ABI v1"
        );
    }
    assert!(!instruction::wide::is_valid_opcode(
        instruction::wide::iso20022::MSG_CREATE
    ));
}
