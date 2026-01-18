//! Instruction opcode constants, field extractors and helpers for the canonical
//! IVM wide instruction format. Each 32-bit word encodes an 8-bit primary
//! opcode followed by three 8-bit operand slots. The nested modules enumerate
//! opcode families (arithmetic, memory, control-flow, crypto and zero-knowledge
//! helpers) and provide convenience extractors for working with raw instruction
//! words. ISO20022 opcode values remain reserved for future releases.

/// Helpers for the canonical wide encoding (8-bit opcode + three 8-bit operands).
pub mod wide {
    /// Arithmetic and logic operations (register-register).
    pub mod arithmetic {
        pub const ADD: u8 = 0x01;
        pub const SUB: u8 = 0x02;
        pub const AND: u8 = 0x03;
        pub const OR: u8 = 0x04;
        pub const XOR: u8 = 0x05;
        pub const SLL: u8 = 0x06;
        pub const SRL: u8 = 0x07;
        pub const SRA: u8 = 0x08;
        pub const SLT: u8 = 0x09;
        pub const SLTU: u8 = 0x0A;
        pub const CMOV: u8 = 0x0B;
        pub const NOT: u8 = 0x0C;
        pub const NEG: u8 = 0x0D;
        pub const SEQ: u8 = 0x0E;
        pub const SNE: u8 = 0x0F;

        pub const MUL: u8 = 0x10;
        pub const MULH: u8 = 0x11;
        pub const MULHU: u8 = 0x12;
        pub const MULHSU: u8 = 0x13;
        pub const DIV: u8 = 0x14;
        pub const DIVU: u8 = 0x15;
        pub const REM: u8 = 0x16;
        pub const REMU: u8 = 0x17;

        pub const ROTL: u8 = 0x18;
        pub const ROTR: u8 = 0x19;
        pub const POPCNT: u8 = 0x1A;
        pub const CLZ: u8 = 0x1B;
        pub const CTZ: u8 = 0x1C;
        pub const ISQRT: u8 = 0x1D;
        pub const MIN: u8 = 0x1E;
        pub const MAX: u8 = 0x1F;
        pub const ABS: u8 = 0x27;
        pub const DIV_CEIL: u8 = 0x28;
        pub const GCD: u8 = 0x29;
        pub const MEAN: u8 = 0x2A;

        pub const ADDI: u8 = 0x20;
        pub const ANDI: u8 = 0x21;
        pub const ORI: u8 = 0x22;
        pub const XORI: u8 = 0x23;
        pub const CMOVI: u8 = 0x24;
        pub const ROTL_IMM: u8 = 0x25;
        pub const ROTR_IMM: u8 = 0x26;
    }

    /// Memory access.
    pub mod memory {
        pub const LOAD64: u8 = 0x30;
        pub const STORE64: u8 = 0x31;
        pub const LOAD128: u8 = 0x32;
        pub const STORE128: u8 = 0x33;
    }

    /// Control flow.
    pub mod control {
        pub const BEQ: u8 = 0x40;
        pub const BNE: u8 = 0x41;
        pub const BLT: u8 = 0x42;
        pub const BGE: u8 = 0x43;
        pub const BLTU: u8 = 0x44;
        pub const BGEU: u8 = 0x45;
        pub const JAL: u8 = 0x46;
        pub const JR: u8 = 0x47;
        pub const JALR: u8 = 0x48;
        pub const HALT: u8 = 0x49;
        pub const JMP: u8 = 0x4A;
        pub const JALS: u8 = 0x4B;
    }

    /// System / syscall.
    pub mod system {
        pub const SCALL: u8 = 0x60;
        pub const GETGAS: u8 = 0x61;
        pub const SYSTEM: u8 = 0x62;
    }

    /// Vector and crypto helpers.
    pub mod crypto {
        pub const VADD32: u8 = 0x70;
        pub const VADD64: u8 = 0x71;
        pub const VAND: u8 = 0x72;
        pub const VXOR: u8 = 0x73;
        pub const VOR: u8 = 0x74;
        pub const VROT32: u8 = 0x75;
        pub const SETVL: u8 = 0x76;
        pub const PARBEGIN: u8 = 0x77;
        pub const PAREND: u8 = 0x78;

        pub const SHA256BLOCK: u8 = 0x80;
        pub const SHA3BLOCK: u8 = 0x81;
        pub const POSEIDON2: u8 = 0x82;
        pub const POSEIDON6: u8 = 0x83;
        pub const PUBKGEN: u8 = 0x84;
        pub const VALCOM: u8 = 0x85;
        pub const ECADD: u8 = 0x86;
        pub const ECMUL_VAR: u8 = 0x87;
        pub const AESENC: u8 = 0x88;
        pub const AESDEC: u8 = 0x89;
        pub const BLAKE2S: u8 = 0x8A;
        pub const ED25519VERIFY: u8 = 0x8B;
        pub const ED25519BATCHVERIFY: u8 = 0x8F;
        pub const ECDSAVERIFY: u8 = 0x8C;
        pub const DILITHIUMVERIFY: u8 = 0x8D;
        pub const PAIRING: u8 = 0x8E;
    }

    /// Reserved ISO20022 helpers (not enabled in ABI v1).
    pub mod iso20022 {
        pub const MSG_CREATE: u8 = 0x90;
        pub const MSG_CLONE: u8 = 0x91;
        pub const MSG_SET: u8 = 0x92;
        pub const MSG_GET: u8 = 0x93;
        pub const MSG_ADD: u8 = 0x94;
        pub const MSG_REMOVE: u8 = 0x95;
        pub const MSG_CLEAR: u8 = 0x96;
        pub const MSG_PARSE: u8 = 0x97;
        pub const MSG_SERIALIZE: u8 = 0x98;
        pub const MSG_VALIDATE: u8 = 0x99;
        pub const MSG_SIGN: u8 = 0x9A;
        pub const MSG_VERIFY_SIG: u8 = 0x9B;
        pub const MSG_SEND: u8 = 0x9C;
        pub const ENCODE_STR: u8 = 0x9D;
        pub const DECODE_STR: u8 = 0x9E;
        pub const VALIDATE_FORMAT: u8 = 0x9F;
    }

    /// Zero-knowledge helpers.
    pub mod zk {
        pub const ASSERT: u8 = 0xA0;
        pub const ASSERT_EQ: u8 = 0xA1;
        pub const FADD: u8 = 0xA2;
        pub const FSUB: u8 = 0xA3;
        pub const FMUL: u8 = 0xA4;
        pub const FINV: u8 = 0xA5;
        pub const ASSERT_RANGE: u8 = 0xA6;
    }

    #[inline]
    pub fn opcode(word: u32) -> u8 {
        (word >> 24) as u8
    }

    #[inline]
    pub fn rd(word: u32) -> usize {
        ((word >> 16) & 0xFF) as usize
    }

    #[inline]
    pub fn rs1(word: u32) -> usize {
        ((word >> 8) & 0xFF) as usize
    }

    #[inline]
    pub fn rs2(word: u32) -> usize {
        (word & 0xFF) as usize
    }

    #[inline]
    pub fn imm8(word: u32) -> i8 {
        word as u8 as i8
    }

    #[inline]
    pub fn imm16(word: u32) -> i16 {
        (word & 0xFFFF) as u16 as i16
    }

    /// Returns true when `op` is one of the defined wide opcode values.
    pub fn is_valid_opcode(op: u8) -> bool {
        matches!(
            op,
            // Integer arithmetic and logical operations
            arithmetic::ADD
                | arithmetic::SUB
                | arithmetic::AND
                | arithmetic::OR
                | arithmetic::XOR
                | arithmetic::SLL
                | arithmetic::SRL
                | arithmetic::SRA
                | arithmetic::SLT
                | arithmetic::SLTU
                | arithmetic::CMOV
                | arithmetic::NOT
                | arithmetic::NEG
                | arithmetic::SEQ
                | arithmetic::SNE
                | arithmetic::MUL
                | arithmetic::MULH
                | arithmetic::MULHU
                | arithmetic::MULHSU
                | arithmetic::DIV
                | arithmetic::DIVU
                | arithmetic::REM
                | arithmetic::REMU
                | arithmetic::ROTL
                | arithmetic::ROTR
                | arithmetic::POPCNT
                | arithmetic::CLZ
                | arithmetic::CTZ
                | arithmetic::ISQRT
                | arithmetic::MIN
                | arithmetic::MAX
                | arithmetic::ABS
                | arithmetic::DIV_CEIL
                | arithmetic::GCD
                | arithmetic::MEAN
                | arithmetic::ADDI
                | arithmetic::ANDI
                | arithmetic::ORI
                | arithmetic::XORI
                | arithmetic::CMOVI
                | arithmetic::ROTL_IMM
                | arithmetic::ROTR_IMM
                // Memory
                | memory::LOAD64
                | memory::STORE64
                | memory::LOAD128
                | memory::STORE128
                // Control flow
                | control::BEQ
                | control::BNE
                | control::BLT
                | control::BGE
                | control::BLTU
                | control::BGEU
                | control::JAL
                | control::JR
                | control::JALR
                | control::HALT
                | control::JMP
                | control::JALS
                // System
                | system::SCALL
                | system::GETGAS
                // NOTE: system::SYSTEM is reserved and currently invalid.
                // Crypto/vector
                | crypto::VADD32
                | crypto::VADD64
                | crypto::VAND
                | crypto::VXOR
                | crypto::VOR
                | crypto::VROT32
                | crypto::SETVL
                | crypto::PARBEGIN
                | crypto::PAREND
                | crypto::SHA256BLOCK
                | crypto::SHA3BLOCK
                | crypto::POSEIDON2
                | crypto::POSEIDON6
                | crypto::PUBKGEN
                | crypto::VALCOM
                | crypto::ECADD
                | crypto::ECMUL_VAR
                | crypto::AESENC
                | crypto::AESDEC
                | crypto::BLAKE2S
                | crypto::ED25519VERIFY
                | crypto::ED25519BATCHVERIFY
                | crypto::ECDSAVERIFY
                | crypto::DILITHIUMVERIFY
                | crypto::PAIRING
                // Zero-knowledge helpers
                | zk::ASSERT
                | zk::ASSERT_EQ
                | zk::FADD
                | zk::FSUB
                | zk::FMUL
                | zk::FINV
                | zk::ASSERT_RANGE
        )
    }
}
