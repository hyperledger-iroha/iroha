//! Admission and runtime rejection of instruction words whose opcode byte is unassigned.
use crate::common;
use common::assemble;
use ivm::{IVM, VMError, encoding, instruction};

/// Reject `word` at artifact admission and again after loading it past the loader.
fn admission_and_runtime_rejections(word: u32) -> (VMError, VMError) {
    let mut code = word.to_le_bytes().to_vec();
    code.extend_from_slice(&encoding::wide::encode_halt().to_le_bytes());
    let admission = IVM::new(1_000)
        .load_program(&assemble(&code))
        .expect_err("an unassigned opcode must fail artifact admission");
    let mut raw = IVM::new(1_000);
    raw.memory.load_code(&code).expect("code fits");
    raw.pc = 0;
    let runtime = raw
        .run()
        .expect_err("an unassigned opcode must trap during execution");
    (admission, runtime)
}

#[test]
fn zero_opcode_byte_is_rejected_whatever_the_operand_bits_hold() {
    // Opcode byte 0x00 is unassigned. The operand bits (here 0x33) are not an opcode
    // and must not be reported as one.
    let word = 0x0000_0033u32;
    assert_eq!(instruction::wide::opcode(word), 0x00);
    let (admission, runtime) = admission_and_runtime_rejections(word);
    assert!(matches!(admission, VMError::InvalidOpcode(0x00)));
    assert!(matches!(runtime, VMError::InvalidOpcode(0x00)));
}

#[test]
fn unassigned_crypto_slots_report_their_opcode_byte_on_both_paths() {
    for opcode in 0x8B..=0x8F {
        // Operands differ from the opcode so a low-bit report would be caught.
        let word = encoding::wide::encode_rr(opcode, 3, 1, 2);
        let (admission, runtime) = admission_and_runtime_rejections(word);
        assert!(
            matches!(admission, VMError::InvalidOpcode(actual) if actual == opcode),
            "admission reported {admission:?} for unassigned opcode 0x{opcode:02x}"
        );
        assert!(
            matches!(runtime, VMError::InvalidOpcode(actual) if actual == opcode),
            "execution reported {runtime:?} for unassigned opcode 0x{opcode:02x}"
        );
    }
}

#[test]
fn removed_opcode_bytes_are_rejected_on_both_paths() {
    // LOAD128/STORE128, JR, GETGAS, the vector operations, the crypto operations, the
    // former ISO 20022 reservations and the zero-knowledge operations left the instruction set.
    let removed = [0x32u8, 0x33, 0x47, 0x61]
        .into_iter()
        .chain(0x70..=0x78)
        .chain(0x80..=0x8A)
        .chain(0x90..=0x9F)
        .chain(0xA0..=0xA6);
    for opcode in removed {
        // Operands differ from the opcode so a low-bit report would be caught.
        let word = encoding::wide::encode_rr(opcode, 3, 1, 2);
        let (admission, runtime) = admission_and_runtime_rejections(word);
        assert!(
            matches!(admission, VMError::InvalidOpcode(actual) if actual == opcode),
            "admission reported {admission:?} for removed opcode 0x{opcode:02x}"
        );
        assert!(
            matches!(runtime, VMError::InvalidOpcode(actual) if actual == opcode),
            "execution reported {runtime:?} for removed opcode 0x{opcode:02x}"
        );
    }
}

#[test]
fn invalid_opcode_message_names_the_opcode_byte() {
    assert_eq!(
        VMError::InvalidOpcode(0x8b).to_string(),
        "invalid or unknown opcode 0x8b"
    );
}
