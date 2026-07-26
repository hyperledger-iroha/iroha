use ivm::{encoding, instruction};

mod wide {
    use super::*;

    #[test]
    fn encode_decode_rr() {
        let word = encoding::wide::encode_rr(instruction::wide::arithmetic::ADD, 3, 1, 2);
        let (op, rd, rs1, rs2) = encoding::wide::decode_rr(word);
        assert_eq!(op, instruction::wide::arithmetic::ADD);
        assert_eq!(rd, 3);
        assert_eq!(rs1, 1);
        assert_eq!(rs2, 2);
    }

    #[test]
    fn encode_decode_ri() {
        let word = encoding::wide::encode_ri(instruction::wide::arithmetic::ADDI, 5, 5, 7);
        let (op, rd, rs1, imm) = encoding::wide::decode_ri(word);
        assert_eq!(op, instruction::wide::arithmetic::ADDI);
        assert_eq!(rd, 5);
        assert_eq!(rs1, 5);
        assert_eq!(imm, 7);
    }

    #[test]
    fn encode_decode_mem() {
        let load = encoding::wide::encode_load(instruction::wide::memory::LOAD64, 3, 1, -8);
        let (op_l, rd, base, imm_l) = encoding::wide::decode_mem(load);
        assert_eq!(op_l, instruction::wide::memory::LOAD64);
        assert_eq!(rd, 3);
        assert_eq!(base, 1);
        assert_eq!(imm_l, -8);

        let store = encoding::wide::encode_store(instruction::wide::memory::STORE64, 2, 5, 4);
        let (op_s, base_s, rs_s, imm_s) = encoding::wide::decode_mem(store);
        assert_eq!(op_s, instruction::wide::memory::STORE64);
        assert_eq!(base_s, 2);
        assert_eq!(rs_s, 5);
        assert_eq!(imm_s, 4);
    }

    #[test]
    fn encode_decode_mem128() {
        let load128 = encoding::wide::encode_load128(instruction::wide::memory::LOAD128, 9, 4, 10);
        let (op, rd_lo, base, rd_hi) = encoding::wide::decode_load128(load128);
        assert_eq!(op, instruction::wide::memory::LOAD128);
        assert_eq!(rd_lo, 9);
        assert_eq!(base, 4);
        assert_eq!(rd_hi, 10);

        let store128 =
            encoding::wide::encode_store128(instruction::wide::memory::STORE128, 3, 5, 6);
        let (op_s, base_s, rs_lo, rs_hi) = encoding::wide::decode_store128(store128);
        assert_eq!(op_s, instruction::wide::memory::STORE128);
        assert_eq!(base_s, 3);
        assert_eq!(rs_lo, 5);
        assert_eq!(rs_hi, 6);
    }

    #[test]
    fn encode_decode_branch() {
        let word = encoding::wide::encode_branch(instruction::wide::control::BEQ, 1, 2, -4);
        let (op, rs1, rs2, off) = encoding::wide::decode_mem(word);
        assert_eq!(op, instruction::wide::control::BEQ);
        assert_eq!(rs1, 1);
        assert_eq!(rs2, 2);
        assert_eq!(off, -4);
    }

    #[test]
    fn encode_decode_jump() {
        let word = encoding::wide::encode_jump(instruction::wide::control::JAL, 1, 1024);
        let (op, rd, off) = encoding::wide::decode_jump(word);
        assert_eq!(op, instruction::wide::control::JAL);
        assert_eq!(rd, 1);
        assert_eq!(off, 1024);
    }

    #[test]
    fn encode_decode_sys() {
        let sys = encoding::wide::encode_sys(instruction::wide::system::SCALL, 0xAB);
        let (op_sys, code) = encoding::wide::decode_sys(sys);
        assert_eq!(op_sys, instruction::wide::system::SCALL);
        assert_eq!(code, 0xAB);
    }

    #[test]
    fn encode_halt() {
        let word = encoding::wide::encode_halt();
        assert_eq!(word, (ivm::instruction::wide::control::HALT as u32) << 24);
    }
}
