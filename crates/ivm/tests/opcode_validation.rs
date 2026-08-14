//! Opcode validation checks for instruction slots.
use ivm::instruction;
#[test]
fn first_release_extension_opcodes_are_valid_but_reserved_iso_slots_are_invalid() {
    assert!(instruction::wide::is_valid_opcode(
        instruction::wide::system::SYSTEM
    ));
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
    for opcode in [
        instruction::wide::iso20022::MSG_CREATE,
        instruction::wide::iso20022::MSG_CLONE,
        instruction::wide::iso20022::MSG_SET,
        instruction::wide::iso20022::MSG_GET,
        instruction::wide::iso20022::MSG_ADD,
        instruction::wide::iso20022::MSG_REMOVE,
        instruction::wide::iso20022::MSG_CLEAR,
        instruction::wide::iso20022::MSG_PARSE,
        instruction::wide::iso20022::MSG_SERIALIZE,
        instruction::wide::iso20022::MSG_VALIDATE,
        instruction::wide::iso20022::MSG_SIGN,
        instruction::wide::iso20022::MSG_VERIFY_SIG,
        instruction::wide::iso20022::MSG_SEND,
        instruction::wide::iso20022::ENCODE_STR,
        instruction::wide::iso20022::DECODE_STR,
        instruction::wide::iso20022::VALIDATE_FORMAT,
    ] {
        assert!(
            !instruction::wide::is_valid_opcode(opcode),
            "reserved ISO 20022 opcode 0x{opcode:02x} must remain invalid in ABI v1"
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
