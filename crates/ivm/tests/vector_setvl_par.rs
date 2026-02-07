use ivm::{IVM, ProgramMetadata, cost_of_with_params, encoding, instruction};

fn header() -> Vec<u8> {
    // Enable VECTOR mode to allow vector-related opcodes in the stream,
    // though SETVL/PAR* are logical/no-op and do not require hardware vectors.
    let meta = ProgramMetadata {
        mode: ivm::ivm_mode::VECTOR,
        ..ProgramMetadata::default()
    };
    meta.encode()
}

#[test]
fn setvl_sets_logical_length() {
    let mut code = header();
    // Encode SETVL with immediate = 1 (I-type high-8 encoding)
    let setvl = encoding::wide::encode_ri(instruction::wide::crypto::SETVL, 0, 0, 1);
    code.extend_from_slice(&setvl.to_le_bytes());
    code.extend_from_slice(&encoding::wide::encode_halt().to_le_bytes());

    let mut vm = IVM::new(10);
    vm.load_program(&code).unwrap();
    vm.run().unwrap();
    assert_eq!(vm.vector_length(), 1);

    // Gas consumed equals cost_of_with_params for SETVL + HALT (0)
    let expected = cost_of_with_params(setvl, /*vl*/ 1, 0).expect("valid opcode must have gas cost");
    assert_eq!(10 - vm.remaining_gas(), expected);
}

#[test]
fn par_markers_are_noops_and_free() {
    let mut code = header();
    let parbegin = encoding::wide::encode_rr(instruction::wide::crypto::PARBEGIN, 0, 0, 0);
    let parend = encoding::wide::encode_rr(instruction::wide::crypto::PAREND, 0, 0, 0);
    code.extend_from_slice(&parbegin.to_le_bytes());
    code.extend_from_slice(&parend.to_le_bytes());
    code.extend_from_slice(&encoding::wide::encode_halt().to_le_bytes());

    let start = 5u64;
    let mut vm = IVM::new(start);
    vm.load_program(&code).unwrap();
    vm.run().unwrap();

    // Both opcodes are no-ops and cost 0, HALT is 0 as well.
    assert_eq!(vm.remaining_gas(), start);
}
