use ivm::{IVM, ProgramMetadata, cost_of_with_params, encoding, instruction};

fn header_with_mode(mode: u8) -> Vec<u8> {
    let meta = ProgramMetadata {
        mode,
        ..ProgramMetadata::default()
    };
    meta.encode()
}

#[test]
fn gas_scales_with_setvl_for_vector_op() {
    // SETVL=8 (clamped to host max lanes); VADD32; HALT under VECTOR mode
    let mut code = header_with_mode(ivm::ivm_mode::VECTOR);
    let setvl = encoding::wide::encode_rr(instruction::wide::crypto::SETVL, 0, 0, 8);
    let vadd = encoding::wide::encode_rr(instruction::wide::crypto::VADD32, 2, 0, 1);
    code.extend_from_slice(&setvl.to_le_bytes());
    code.extend_from_slice(&vadd.to_le_bytes());
    code.extend_from_slice(&encoding::wide::encode_halt().to_le_bytes());

    let mut vm = IVM::new(10_000);
    vm.load_program(&code).unwrap();
    vm.run().unwrap();

    // Sum expected with vector_len = clamped host max lanes for VADD32
    let vl = vm.vector_length();
    let used = 10_000 - vm.remaining_gas();
    let expected = cost_of_with_params(setvl, 1, 0).expect("valid opcode must have gas cost")
        + cost_of_with_params(vadd, vl, 0).expect("valid opcode must have gas cost")
        + cost_of_with_params(encoding::wide::encode_halt(), 4, 0)
            .expect("valid opcode must have gas cost");
    assert_eq!(vl, vm.vector_length());
    assert_eq!(used, expected, "vl={vl} used={used} expected={expected}");
}

#[test]
fn branch_executed_set_gas_matches() {
    // addi x1, x0, 1; addi x2, x0, 2; beq x1,x2, +8; jal 0, +4; halt
    let a1 = ivm::kotodama::compiler::encode_addi(1, 0, 1).expect("encode addi");
    let a2 = ivm::kotodama::compiler::encode_addi(2, 0, 2).expect("encode addi");
    let beq = encoding::wide::encode_branch(instruction::wide::control::BEQ, 1, 2, 2);
    let jmp = encoding::wide::encode_jump(instruction::wide::control::JMP, 0, 1);
    let halt = encoding::wide::encode_halt();
    let words = [a1, a2, beq, jmp, halt];
    let mut code = header_with_mode(0);
    for w in &words {
        code.extend_from_slice(&w.to_le_bytes());
    }
    let mut vm = IVM::new(10_000);
    vm.load_program(&code).unwrap();
    vm.run().unwrap();
    // Executed-set: a1, a2, beq (not taken), jmp, halt
    let executed = [a1, a2, beq, jmp, halt];
    let expected: u64 = executed
        .iter()
        .map(|&w| cost_of_with_params(w, 1, 0).expect("valid opcode must have gas cost"))
        .sum();
    assert_eq!(10_000 - vm.remaining_gas(), expected);
}

#[test]
fn vector_op_gas_table_matches_under_various_setvl() {
    // Build small programs that set VL and execute a single vector op; compare gas used.
    let ops: &[(u8, &str)] = &[
        (instruction::wide::crypto::VADD32, "VADD32"),
        (instruction::wide::crypto::VAND, "VAND"),
        (instruction::wide::crypto::VXOR, "VXOR"),
        (instruction::wide::crypto::VOR, "VOR"),
    ];
    let setvls: &[i32] = &[1, 2, 4, 8, 16];
    for &vl in setvls {
        for &(op, _name) in ops {
            // VECTOR mode header
            let mut code = header_with_mode(ivm::ivm_mode::VECTOR);
            // SETVL
            let setvl = encoding::wide::encode_rr(instruction::wide::crypto::SETVL, 0, 0, vl as u8);
            let word = encoding::wide::encode_rr(op, 2, 0, 1);
            code.extend_from_slice(&setvl.to_le_bytes());
            code.extend_from_slice(&word.to_le_bytes());
            code.extend_from_slice(&encoding::wide::encode_halt().to_le_bytes());

            let mut vm = IVM::new(100_000);
            vm.load_program(&code).unwrap();
            vm.run().unwrap();
            // Expected gas: SETVL (vector_len ignored for SETVL) + op (scaled by vm.vector_length()) + HALT
            let vl = vm.vector_length();
            let used = 100_000 - vm.remaining_gas();
            let expected = cost_of_with_params(setvl, 1, 0)
                .expect("valid opcode must have gas cost")
                + cost_of_with_params(word, vl, 0).expect("valid opcode must have gas cost")
                + cost_of_with_params(encoding::wide::encode_halt(), 4, 0)
                    .expect("valid opcode must have gas cost");
            assert_eq!(used, expected, "vl={vl} used={used} expected={expected}");
        }
    }
}
