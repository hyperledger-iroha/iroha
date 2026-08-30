//! Opcode validation checks for instruction slots.
use ivm::instruction;

#[test]
fn first_release_crypto_opcode_layout_is_dense() {
    assert_eq!(
        [
            instruction::wide::crypto::SHA256BLOCK,
            instruction::wide::crypto::SHA3BLOCK,
            instruction::wide::crypto::POSEIDON2,
            instruction::wide::crypto::POSEIDON6,
            instruction::wide::crypto::AESENC,
            instruction::wide::crypto::AESDEC,
            instruction::wide::crypto::BLAKE2S,
            instruction::wide::crypto::ED25519VERIFY,
            instruction::wide::crypto::ED25519BATCHVERIFY,
            instruction::wide::crypto::ECDSAVERIFY,
            instruction::wide::crypto::DILITHIUMVERIFY,
        ],
        [
            0x80, 0x81, 0x82, 0x83, 0x84, 0x85, 0x86, 0x87, 0x88, 0x89, 0x8A,
        ]
    );
}

#[test]
fn first_release_extension_opcodes_are_valid_but_unassigned_slots_are_invalid() {
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
        0x8B,
        0x8C,
        0x8D,
        0x8E,
        0x8F,
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
