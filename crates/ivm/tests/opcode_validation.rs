//! Opcode validation checks for instruction slots.
use ivm::instruction;

/// Opcode bytes the first release assigns. Every other byte is unassigned.
const fn is_assigned_opcode(opcode: u8) -> bool {
    matches!(
        opcode,
        0x01..=0x2A | 0x30 | 0x31 | 0x34 | 0x35 | 0x40..=0x46 | 0x48..=0x4B | 0x60
    )
}

#[test]
fn first_release_extension_opcodes_are_valid_but_unassigned_slots_are_invalid() {
    assert!(instruction::wide::is_valid_opcode(
        instruction::wide::system::SCALL
    ));
    assert!(!instruction::wide::is_valid_opcode(0x62));
    assert!(instruction::wide::is_valid_opcode(
        instruction::wide::memory::LDLIT
    ));
    assert!(instruction::wide::is_valid_opcode(
        instruction::wide::memory::LDI64
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
    for opcode in u8::MIN..=u8::MAX {
        if is_assigned_opcode(opcode) {
            assert!(
                instruction::wide::is_valid_opcode(opcode),
                "assigned opcode 0x{opcode:02x} must be valid in ABI v1"
            );
            continue;
        }
        assert!(
            !instruction::wide::is_valid_opcode(opcode),
            "unassigned opcode 0x{opcode:02x} must be invalid in ABI v1"
        );
        assert_eq!(
            ivm::cost_of(u32::from(opcode) << 24),
            None,
            "unassigned opcode 0x{opcode:02x} must have no gas schedule entry"
        );
    }
}
#[test]
fn first_release_iso_helpers_have_no_network_transport_surface() {
    let source = include_str!("../src/iso20022.rs");
    for forbidden in [
        "TcpStream",
        "std::net",
        "pub fn msg_send",
        "MsgSendCallback",
    ] {
        assert!(
            !source.contains(forbidden),
            "IVM ISO 20022 helpers must remain transport-free; found `{forbidden}`"
        );
    }
}
