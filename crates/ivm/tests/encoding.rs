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
        let (op_l, rd, base, imm_l) = encoding::wide::decode_load(load);
        assert_eq!(op_l, instruction::wide::memory::LOAD64);
        assert_eq!(rd, 3);
        assert_eq!(base, 1);
        assert_eq!(imm_l, -8);
        let store = encoding::wide::encode_store(instruction::wide::memory::STORE64, 2, 5, 4);
        let (op_s, base_s, rs_s, imm_s) = encoding::wide::decode_store(store);
        assert_eq!(instruction::wide::store_value(store), 5);
        assert_eq!(instruction::wide::store_base(store), 2);
        assert_eq!(op_s, instruction::wide::memory::STORE64);
        assert_eq!(base_s, 2);
        assert_eq!(rs_s, 5);
        assert_eq!(imm_s, 4);
    }
    #[test]
    fn encode_decode_branch() {
        let word = encoding::wide::encode_branch(instruction::wide::control::BEQ, 1, 2, -4);
        let (op, rs1, rs2, off) = encoding::wide::decode_branch(word);
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
    fn encode_decode_scall() {
        for number in [0xAB, 0x01_0000, instruction::wide::SYSCALL_NUMBER_MAX] {
            let word = encoding::wide::encode_scall(number);
            assert_eq!(
                instruction::wide::opcode(word),
                instruction::wide::system::SCALL
            );
            assert_eq!(encoding::wide::decode_scall(word), number);
        }
    }
    #[test]
    fn encoded_words_have_no_reserved_bits() {
        for word in [
            encoding::wide::encode_rr(instruction::wide::arithmetic::ADD, 31, 30, 29),
            encoding::wide::encode_ri(instruction::wide::arithmetic::ADDI, 1, 2, -8192),
            encoding::wide::encode_store(instruction::wide::memory::STORE64, 2, 5, 8191),
            encoding::wide::encode_branch(instruction::wide::control::BNE, 1, 2, -1),
            encoding::wide::encode_jump(instruction::wide::control::JAL, 1, -262_144),
            encoding::wide::encode_literal(instruction::wide::memory::LDLIT, 9, u16::MAX),
            encoding::wide::encode_scall(0x00ff_ffff),
            encoding::wide::encode_halt(),
        ] {
            assert!(
                instruction::wide::validate_word(word).is_ok(),
                "{word:#010x}"
            );
        }
        let add = encoding::wide::encode_rr(instruction::wide::arithmetic::ADD, 1, 2, 3);
        assert_eq!(
            instruction::wide::validate_word(add | 1),
            Err(instruction::wide::WordError::ReservedBits)
        );
    }
    #[test]
    fn encode_halt() {
        let word = encoding::wide::encode_halt();
        assert_eq!(word, (ivm::instruction::wide::control::HALT as u32) << 24);
    }
}
