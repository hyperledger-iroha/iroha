use ivm::{IVM, ProgramMetadata, encoding, instruction, kotodama::wide as kwide};

#[test]
fn backward_branch_negative_offset_loop() {
    // Build a tiny loop that increments x1 three times using BNE with a negative offset.
    // Layout (all 32-bit words):
    // 0: addi x5,x0,3   ; counter
    // 1: addi x1,x0,0   ; acc = 0
    // 2: addi x1,x1,1   ; label: acc++
    // 3: addi x5,x5,-1  ; counter--
    // 4: bne  x5,x0,-8  ; if counter != 0, jump back 2 instructions (8 bytes)
    // 5: halt
    let a_cnt = ivm::kotodama::compiler::encode_addi(5, 0, 3).expect("encode addi");
    let a_acc0 = ivm::kotodama::compiler::encode_addi(1, 0, 0).expect("encode addi");
    let a_inc = ivm::kotodama::compiler::encode_addi(1, 1, 1).expect("encode addi");
    let a_dec = ivm::kotodama::compiler::encode_addi(5, 5, -1).expect("encode addi");
    let bne_back =
        kwide::encode_branch_checked(instruction::wide::control::BNE, 5, 0, -2).expect("loop back");
    let halt = encoding::wide::encode_halt();

    let mut bytes = ProgramMetadata::default().encode();
    for w in [a_cnt, a_acc0, a_inc, a_dec, bne_back, halt] {
        bytes.extend_from_slice(&w.to_le_bytes());
    }
    let mut vm = IVM::new(10_000);
    vm.load_program(&bytes).unwrap();
    let res = vm.run();
    assert!(res.is_ok());
    // After three iterations, x1 should be 3 and x5 should be 0.
    assert_eq!(vm.register(1), 3);
    assert_eq!(vm.register(5), 0);
}

#[test]
fn backward_branch_beq_to_nonzero_sentinel() {
    // Similar to the BNE loop, but use BEQ against a non-zero sentinel to branch back exactly once.
    // Layout:
    // 0: addi x5,x0,3    ; counter = 3
    // 1: addi x6,x0,2    ; sentinel = 2 (non-zero)
    // 2: addi x1,x0,0    ; acc = 0
    // 3: addi x1,x1,1    ; acc++
    // 4: addi x5,x5,-1   ; counter--
    // 5: beq  x5,x6,-8   ; when counter==2, jump back to inc/dec once more
    // 6: halt
    let a_cnt = ivm::kotodama::compiler::encode_addi(5, 0, 3).expect("encode addi");
    let a_sen = ivm::kotodama::compiler::encode_addi(6, 0, 2).expect("encode addi");
    let a_acc0 = ivm::kotodama::compiler::encode_addi(1, 0, 0).expect("encode addi");
    let a_inc = ivm::kotodama::compiler::encode_addi(1, 1, 1).expect("encode addi");
    let a_dec = ivm::kotodama::compiler::encode_addi(5, 5, -1).expect("encode addi");
    let beq_back = kwide::encode_branch_checked(instruction::wide::control::BEQ, 5, 6, -2)
        .expect("branch back");
    let halt = encoding::wide::encode_halt();

    let mut bytes = ProgramMetadata::default().encode();
    for w in [a_cnt, a_sen, a_acc0, a_inc, a_dec, beq_back, halt] {
        bytes.extend_from_slice(&w.to_le_bytes());
    }
    let mut vm = IVM::new(10_000);
    vm.load_program(&bytes).unwrap();
    let res = vm.run();
    assert!(res.is_ok());
    // Expected: acc incremented twice (when counter went 3->2 then 2->1 after branch), sentinel protects single branch
    assert_eq!(vm.register(1), 2);
    assert_eq!(vm.register(5), 1);
}

#[test]
fn forward_beq_skips_block() {
    // Build a forward BEQ that skips over a two-instruction block when equal.
    // Case 1 (taken): x5==x6 => skip two adds (5 and 6) and only execute final add (+1).
    // Case 2 (not taken): x5!=x6 => execute both adds (5 and 6) plus final add (+1).

    // Common prologue: x1=0
    let a_acc0 = ivm::kotodama::compiler::encode_addi(1, 0, 0).expect("encode addi");
    let inc5 = ivm::kotodama::compiler::encode_addi(1, 1, 5).expect("encode addi");
    let inc6 = ivm::kotodama::compiler::encode_addi(1, 1, 6).expect("encode addi");
    let inc1 = ivm::kotodama::compiler::encode_addi(1, 1, 1).expect("encode addi");
    let halt = encoding::wide::encode_halt();

    // Taken: x5=7, x6=7, beq x5,x6, +12 (skip two 32-bit instructions)
    // High-8 branch immediates are relative to the current PC (not fallthrough),
    // so skipping two 32-bit words requires +12 bytes.
    let a5_7 = ivm::kotodama::compiler::encode_addi(5, 0, 7).expect("encode addi");
    let a6_7 = ivm::kotodama::compiler::encode_addi(6, 0, 7).expect("encode addi");
    let beq_skip = kwide::encode_branch_checked(instruction::wide::control::BEQ, 5, 6, 3)
        .expect("forward branch");
    let mut bytes = ProgramMetadata::default().encode();
    for w in [a_acc0, a5_7, a6_7, beq_skip, inc5, inc6, inc1, halt] {
        bytes.extend_from_slice(&w.to_le_bytes());
    }
    let mut vm = IVM::new(1000);
    vm.load_program(&bytes).unwrap();
    let res = vm.run();
    assert!(res.is_ok());
    assert_eq!(vm.register(1), 1, "branch taken should skip +5 and +6");

    // Not taken: x5=7, x6=6 -> branch not taken, so +5 and +6 execute, then +1 → total 12
    let a5_7 = ivm::kotodama::compiler::encode_addi(5, 0, 7).expect("encode addi");
    let a6_6 = ivm::kotodama::compiler::encode_addi(6, 0, 6).expect("encode addi");
    let mut bytes2 = ProgramMetadata::default().encode();
    for w in [a_acc0, a5_7, a6_6, beq_skip, inc5, inc6, inc1, halt] {
        bytes2.extend_from_slice(&w.to_le_bytes());
    }
    let mut vm2 = IVM::new(1000);
    vm2.load_program(&bytes2).unwrap();
    let res2 = vm2.run();
    assert!(res2.is_ok());
    assert_eq!(
        vm2.register(1),
        12,
        "branch not taken should execute +5,+6,+1"
    );
}
