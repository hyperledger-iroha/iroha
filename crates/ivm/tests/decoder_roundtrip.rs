use ivm::{encoding, instruction};

#[test]
fn roundtrip_rr() {
    let word = encoding::wide::encode_rr(instruction::wide::arithmetic::XOR, 2, 4, 5);
    let (op, rd, rs1, rs2) = encoding::wide::decode_rr(word);
    assert_eq!(op, instruction::wide::arithmetic::XOR);
    assert_eq!((rd, rs1, rs2), (2, 4, 5));
}

#[test]
fn roundtrip_ri() {
    let word = encoding::wide::encode_ri(instruction::wide::arithmetic::ORI, 7, 1, -3);
    let (op, rd, rs1, imm) = encoding::wide::decode_ri(word);
    assert_eq!(op, instruction::wide::arithmetic::ORI);
    assert_eq!((rd, rs1, imm), (7, 1, -3));
}

#[test]
fn roundtrip_mem() {
    let load = encoding::wide::encode_load(instruction::wide::memory::LOAD64, 5, 3, 6);
    let (op_l, rd, base, imm_l) = encoding::wide::decode_mem(load);
    assert_eq!(op_l, instruction::wide::memory::LOAD64);
    assert_eq!((rd, base, imm_l), (5, 3, 6));

    let store = encoding::wide::encode_store(instruction::wide::memory::STORE64, 9, 8, -7);
    let (op_s, base_s, rs_s, imm_s) = encoding::wide::decode_mem(store);
    assert_eq!(op_s, instruction::wide::memory::STORE64);
    assert_eq!((base_s, rs_s, imm_s), (9, 8, -7));
}

#[test]
fn roundtrip_mem128() {
    let load128 = encoding::wide::encode_load128(instruction::wide::memory::LOAD128, 4, 7, 8);
    let (op, rd_lo, base, rd_hi) = encoding::wide::decode_load128(load128);
    assert_eq!(op, instruction::wide::memory::LOAD128);
    assert_eq!((rd_lo, base, rd_hi), (4, 7, 8));

    let store128 = encoding::wide::encode_store128(instruction::wide::memory::STORE128, 11, 12, 13);
    let (op_s, base_s, rs_lo, rs_hi) = encoding::wide::decode_store128(store128);
    assert_eq!(op_s, instruction::wide::memory::STORE128);
    assert_eq!((base_s, rs_lo, rs_hi), (11, 12, 13));
}

#[test]
fn roundtrip_branch_jump_sys() {
    let beq = encoding::wide::encode_branch(instruction::wide::control::BEQ, 1, 2, 6);
    let (op_b, rs1, rs2, off) = encoding::wide::decode_mem(beq);
    assert_eq!(op_b, instruction::wide::control::BEQ);
    assert_eq!((rs1, rs2, off), (1, 2, 6));

    let jal = encoding::wide::encode_jump(instruction::wide::control::JAL, 3, -32);
    let (op_j, rd, off_j) = encoding::wide::decode_jump(jal);
    assert_eq!(op_j, instruction::wide::control::JAL);
    assert_eq!((rd, off_j), (3, -32));

    let sys = encoding::wide::encode_sys(instruction::wide::system::SCALL, 0x1A);
    let (op_s, imm8) = encoding::wide::decode_sys(sys);
    assert_eq!(op_s, instruction::wide::system::SCALL);
    assert_eq!(imm8, 0x1A);
}
