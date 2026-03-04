#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Instruction {
    Add {
        rd: u16,
        rs: u16,
        rt: u16,
    },
    Sub {
        rd: u16,
        rs: u16,
        rt: u16,
    },
    And {
        rd: u16,
        rs: u16,
        rt: u16,
    },
    Or {
        rd: u16,
        rs: u16,
        rt: u16,
    },
    AddImm {
        rd: u16,
        rs: u16,
        imm: i16,
    },
    SubImm {
        rd: u16,
        rs: u16,
        imm: i16,
    },
    Xor {
        rd: u16,
        rs: u16,
        rt: u16,
    },
    Sll {
        rd: u16,
        rs: u16,
        rt: u16,
    },
    Srl {
        rd: u16,
        rs: u16,
        rt: u16,
    },
    Sra {
        rd: u16,
        rs: u16,
        rt: u16,
    },
    Load {
        rd: u16,
        addr_reg: u16,
        offset: i8,
    },
    Store {
        rs: u16,
        addr_reg: u16,
        offset: i8,
    },
    /// Set the logical vector length for subsequent vector operations.
    SetVL {
        new_vl: u16,
    },
    /// Vector addition over `vector_length` lanes.
    Vadd {
        rd: u16,
        rs: u16,
        rt: u16,
    },
    Jump {
        target: u64,
    },
    Beq {
        rs: u16,
        rt: u16,
        offset: i16,
    },
    Halt,
    /// Compute SHA-256 over `len` bytes from memory at `src_addr` and store the
    /// 32-byte digest in registers starting at `dest` (little endian chunks).
    Sha256 {
        dest: u16,
        src_addr: u64,
        len: u64,
    },
    /// Verify an Ed25519 signature over a message in memory. The public key and
    /// signature are also read from memory. Result is placed in `result_reg`
    /// (1 for valid, 0 for invalid).
    Ed25519Verify {
        pubkey_addr: u64,
        sig_addr: u64,
        msg_addr: u64,
        msg_len: u64,
        result_reg: u16,
    },
    /// Verify a Dilithium signature over a message in memory using the
    /// specified security level (2, 3 or 5). Result is placed in `result_reg`.
    DilithiumVerify {
        level: u8,
        pubkey_addr: u64,
        sig_addr: u64,
        msg_addr: u64,
        msg_len: u64,
        result_reg: u16,
    },
}
