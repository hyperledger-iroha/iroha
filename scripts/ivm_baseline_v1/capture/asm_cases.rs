//! Hand-assembled baseline programs.
//!
//! Each program is built with the anchor's own `ivm::encoding::wide` helpers and
//! `ProgramMetadata::encode`, the way the anchor's interpreter tests build theirs. Together they
//! reach every opcode family of the pre-C10 instruction set, the 49-byte header fields C10
//! removed, and the admission, fault, gas and cycle behaviour C10 changed.

use ivm::{
    LiteralKindV1, Memory, PointerType, ProgramMetadata, encoding::wide as enc,
    instruction::wide as op, ivm_mode, syscalls,
};

use crate::oracle::{Case, Gas, HostKind, Policy, Step, tlv};

/// Header cycle limit of the ordinary hand-assembled programs: positive, so the anchor Core
/// admits them, and far above what any of them executes.
pub const DEFAULT_MAX_CYCLES: u64 = 100_000;
const HALT: u32 = enc::encode_halt();
/// First general register of the vector register file (`IVM::VECTOR_BASE`).
const VECTOR_BASE: usize = 32;
const HEAP: u64 = Memory::HEAP_START;

fn meta(mode: u8, vector_length: u8, max_cycles: u64) -> ProgramMetadata {
    ProgramMetadata {
        version_major: 1,
        version_minor: 1,
        mode,
        vector_length,
        max_cycles,
        abi_version: 1,
    }
}
fn image(meta: &ProgramMetadata, words: &[u32]) -> Vec<u8> {
    let mut out = meta.encode();
    for word in words {
        out.extend_from_slice(&word.to_le_bytes());
    }
    out
}
fn plain(words: &[u32]) -> Vec<u8> {
    image(&meta(0, 0, DEFAULT_MAX_CYCLES), words)
}
fn vector(vector_length: u8, words: &[u32]) -> Vec<u8> {
    image(
        &meta(ivm_mode::VECTOR, vector_length, DEFAULT_MAX_CYCLES),
        words,
    )
}
fn zk(max_cycles: u64, words: &[u32]) -> Vec<u8> {
    image(&meta(ivm_mode::ZK, 0, max_cycles), words)
}
/// Program with an `LTLB` indexed-literal section between the header and the code.
fn literal_image(literals: &[(LiteralKindV1, Vec<u8>)], words: &[u32]) -> Vec<u8> {
    let data_start = 16 + 8 * literals.len();
    let mut entries = Vec::new();
    let mut data = Vec::new();
    for (kind, bytes) in literals {
        let offset = u64::try_from(data_start + data.len()).expect("literal offset fits u64");
        entries.push(
            ivm::encode_literal_descriptor(*kind, offset).expect("literal offset fits 56 bits"),
        );
        data.extend_from_slice(bytes);
    }
    let post_pad = (4 - ((data_start + data.len()) % 4)) % 4;
    let mut out = meta(0, 0, DEFAULT_MAX_CYCLES).encode();
    out.extend_from_slice(&ivm_abi::metadata::LITERAL_SECTION_MAGIC);
    out.extend_from_slice(
        &u32::try_from(literals.len())
            .expect("literal count")
            .to_le_bytes(),
    );
    out.extend_from_slice(
        &u32::try_from(post_pad)
            .expect("literal padding")
            .to_le_bytes(),
    );
    out.extend_from_slice(
        &u32::try_from(data.len())
            .expect("literal data")
            .to_le_bytes(),
    );
    for entry in entries {
        out.extend_from_slice(&entry.to_le_bytes());
    }
    out.extend_from_slice(&data);
    out.extend(std::iter::repeat_n(0_u8, post_pad));
    for word in words {
        out.extend_from_slice(&word.to_le_bytes());
    }
    out
}
fn regs(values: &[(usize, u64)]) -> Vec<Step> {
    values
        .iter()
        .map(|(index, value)| Step::SetRegister {
            index: *index,
            value: *value,
        })
        .collect()
}
fn rr(opcode: u8, rd: usize, rs1: usize, rs2: usize) -> u32 {
    enc::encode_rr(opcode, reg(rd), reg(rs1), reg(rs2))
}
fn ri(opcode: u8, rd: usize, rs1: usize, imm: i8) -> u32 {
    enc::encode_ri(opcode, reg(rd), reg(rs1), imm)
}
fn reg(index: usize) -> u8 {
    u8::try_from(index).expect("register index fits the 8-bit operand")
}
fn scall(number: u32) -> u32 {
    u8::try_from(number).map_or_else(
        |_| enc::encode_syscallx(number),
        |compact| enc::encode_sys(op::system::SCALL, compact),
    )
}
fn neg(value: i64) -> u64 {
    value as u64
}

/// Accumulates cases; every case of one artifact name shares one program image.
#[derive(Default)]
pub struct Corpus {
    cases: Vec<Case>,
}
impl Corpus {
    fn add(
        &mut self,
        artifact: &str,
        run: &str,
        covers: &[&str],
        program: Vec<u8>,
        steps: Vec<Step>,
    ) -> &mut Case {
        let id = if run.is_empty() {
            format!("asm/{artifact}")
        } else {
            format!("asm/{artifact}#{run}")
        };
        self.cases.push(Case {
            id,
            artifact: format!("artifacts/asm/{artifact}.to.bin"),
            program,
            covers: covers.iter().map(|tag| (*tag).to_owned()).collect(),
            gas: Gas::FromHeader,
            host: HostKind::Default,
            public_inputs: Vec::new(),
            arguments_json: None,
            prelude: Vec::new(),
            steps,
            policy: Policy::anchor_default(),
            cycle_budget: None,
            traced: true,
        });
        self.cases.last_mut().expect("case was just pushed")
    }
}

const BINARY_OPS: &[(&str, u8)] = &[
    ("ADD", op::arithmetic::ADD),
    ("SUB", op::arithmetic::SUB),
    ("AND", op::arithmetic::AND),
    ("OR", op::arithmetic::OR),
    ("XOR", op::arithmetic::XOR),
    ("SLL", op::arithmetic::SLL),
    ("SRL", op::arithmetic::SRL),
    ("SRA", op::arithmetic::SRA),
    ("SLT", op::arithmetic::SLT),
    ("SLTU", op::arithmetic::SLTU),
    ("SEQ", op::arithmetic::SEQ),
    ("SNE", op::arithmetic::SNE),
    ("MUL", op::arithmetic::MUL),
    ("MULH", op::arithmetic::MULH),
    ("MULHU", op::arithmetic::MULHU),
    ("MULHSU", op::arithmetic::MULHSU),
    ("DIV", op::arithmetic::DIV),
    ("DIVU", op::arithmetic::DIVU),
    ("REM", op::arithmetic::REM),
    ("REMU", op::arithmetic::REMU),
    ("ROTL", op::arithmetic::ROTL),
    ("ROTR", op::arithmetic::ROTR),
    ("MIN", op::arithmetic::MIN),
    ("MAX", op::arithmetic::MAX),
    ("GCD", op::arithmetic::GCD),
    ("MEAN", op::arithmetic::MEAN),
    ("DIV_CEIL", op::arithmetic::DIV_CEIL),
];
const UNARY_OPS: &[(&str, u8)] = &[
    ("NOT", op::arithmetic::NOT),
    ("NEG", op::arithmetic::NEG),
    ("POPCNT", op::arithmetic::POPCNT),
    ("CLZ", op::arithmetic::CLZ),
    ("CTZ", op::arithmetic::CTZ),
    ("ISQRT", op::arithmetic::ISQRT),
];

fn arithmetic(c: &mut Corpus) {
    // Every non-trapping register-register and unary operation, with results in registers
    // above 31 (the baseline register file has 256 registers; C10 keeps 32).
    let mut words = Vec::new();
    for (index, (_, opcode)) in BINARY_OPS.iter().enumerate() {
        words.push(rr(*opcode, 40 + index, 1, 2));
    }
    for (index, (_, opcode)) in UNARY_OPS.iter().enumerate() {
        words.push(rr(*opcode, 100 + index, 1, 0));
    }
    words.push(rr(op::arithmetic::CMOV, 200, 1, 2));
    words.push(rr(op::arithmetic::CMOV, 201, 1, 0));
    words.push(rr(op::arithmetic::ADD, 255, 1, 2));
    words.push(HALT);
    let program = plain(&words);
    for (run, a, b) in [
        ("small", 7, 3),
        ("negative", neg(-9), 4),
        ("wide", 0xFEDC_BA98_7654_3210, 0x0123_4567_89AB_CDEF),
        ("shift63", 0x8000_0000_0000_0001, 63),
        ("shift65", 0x8000_0000_0000_0001, 65),
        ("min", neg(i64::MIN), 2),
        ("minus_one", u64::MAX, u64::MAX),
        ("zero_dividend", 0, 5),
    ] {
        c.add(
            "arith_all",
            run,
            &["family:arithmetic", "high-registers"],
            program.clone(),
            regs(&[(1, a), (2, b)]),
        );
    }
    // Arithmetic the baseline reports as `AssertionFailed` (C10: structured numeric faults).
    for (name, opcode, runs) in [
        (
            "div",
            op::arithmetic::DIV,
            &[
                ("ok", 7, 2),
                ("by_zero", 1, 0),
                ("overflow", neg(i64::MIN), neg(-1)),
            ][..],
        ),
        (
            "divu",
            op::arithmetic::DIVU,
            &[("ok", 7, 2), ("by_zero", 1, 0)][..],
        ),
        (
            "rem",
            op::arithmetic::REM,
            &[
                ("ok", neg(-7), 2),
                ("by_zero", 1, 0),
                ("overflow", neg(i64::MIN), neg(-1)),
            ][..],
        ),
        (
            "remu",
            op::arithmetic::REMU,
            &[("ok", 7, 2), ("by_zero", 1, 0)][..],
        ),
        (
            "div_ceil",
            op::arithmetic::DIV_CEIL,
            &[
                ("ok_up", 7, 2),
                ("ok_negative", neg(-7), 2),
                ("by_zero", 1, 0),
                ("overflow", neg(i64::MIN), neg(-1)),
            ][..],
        ),
        (
            "abs",
            op::arithmetic::ABS,
            &[("ok", neg(-5), 0), ("overflow", neg(i64::MIN), 0)][..],
        ),
    ] {
        let program = plain(&[rr(opcode, 3, 1, 2), HALT]);
        for (run, a, b) in runs {
            c.add(
                &format!("arith_trap_{name}"),
                run,
                &["family:arithmetic", "fault:AssertionFailed-for-arithmetic"],
                program.clone(),
                regs(&[(1, *a), (2, *b)]),
            );
        }
    }
    // 8-bit sign-extended immediates (C10: 14-bit immediates).
    let program = plain(&[
        ri(op::arithmetic::ADDI, 40, 1, 127),
        ri(op::arithmetic::ADDI, 41, 1, -128),
        ri(op::arithmetic::ADDI, 42, 1, -1),
        ri(op::arithmetic::ANDI, 43, 1, 0x0F),
        ri(op::arithmetic::ANDI, 44, 1, -16),
        ri(op::arithmetic::ORI, 45, 1, 0x55),
        ri(op::arithmetic::ORI, 46, 1, -128),
        ri(op::arithmetic::XORI, 47, 1, -1),
        ri(op::arithmetic::XORI, 48, 1, 0x7F),
        ri(op::arithmetic::ROTL_IMM, 49, 1, 1),
        ri(op::arithmetic::ROTL_IMM, 50, 1, 63),
        ri(op::arithmetic::ROTR_IMM, 51, 1, 8),
        ri(op::arithmetic::ROTR_IMM, 52, 1, -1),
        ri(op::arithmetic::CMOVI, 53, 1, 42),
        ri(op::arithmetic::CMOVI, 54, 0, 42),
        ri(op::arithmetic::CMOVI, 55, 1, -7),
        HALT,
    ]);
    for (run, value) in [
        ("pattern", 0x1234_5678_9ABC_DEF0),
        ("one", 1),
        ("all_ones", u64::MAX),
    ] {
        c.add(
            "arith_imm8",
            run,
            &["family:arithmetic-immediate", "encoding:imm8"],
            program.clone(),
            regs(&[(1, value)]),
        );
    }
}

fn memory(c: &mut Corpus) {
    let program = plain(&[
        enc::encode_store(op::memory::STORE64, 4, 5, 0),
        enc::encode_load(op::memory::LOAD64, 6, 4, 0),
        HALT,
    ]);
    for (run, base) in [
        ("heap", HEAP + 0x100),
        ("heap_misaligned", HEAP + 0x101),
        ("heap_last_word", HEAP + Memory::HEAP_SIZE - 8),
        ("code", 0),
        ("input", Memory::INPUT_START),
        ("output", Memory::OUTPUT_START),
        ("stack_base", Memory::STACK_START),
        ("unmapped", 0x7FFF_FFF0),
        ("address_overflow", u64::MAX - 3),
    ] {
        c.add(
            "mem_store_load64",
            run,
            &["family:memory", "fault:memory"],
            program.clone(),
            regs(&[(4, base), (5, 0xDEAD_BEEF_CAFE_BABE)]),
        );
    }
    // Signed 8-bit displacements and the loader-provided stack pointer (r31).
    c.add(
        "mem_displacement",
        "",
        &["family:memory", "encoding:imm8"],
        plain(&[
            enc::encode_store(op::memory::STORE64, 4, 5, 8),
            enc::encode_load(op::memory::LOAD64, 6, 4, 8),
            enc::encode_store(op::memory::STORE64, 4, 5, -8),
            enc::encode_load(op::memory::LOAD64, 7, 4, -8),
            enc::encode_store(op::memory::STORE64, 4, 5, 120),
            enc::encode_load(op::memory::LOAD64, 8, 4, 120),
            ri(op::arithmetic::ADDI, 9, 31, -64),
            enc::encode_store(op::memory::STORE64, 9, 5, 0),
            enc::encode_load(op::memory::LOAD64, 10, 9, 0),
            HALT,
        ]),
        regs(&[(4, HEAP + 0x200), (5, 0x0123_4567_89AB_CDEF)]),
    );
    // 128-bit memory operations exist only in the baseline; they need vector mode.
    let words = [
        enc::encode_store128(op::memory::STORE128, 4, 200, 201),
        enc::encode_load128(op::memory::LOAD128, 210, 4, 255),
        HALT,
    ];
    let seed = |base: u64| {
        regs(&[
            (4, base),
            (200, 0xDEAD_BEEF_CAFE_BABE),
            (201, 0x0123_4567_89AB_CDEF),
        ])
    };
    for (run, base) in [
        ("aligned", HEAP + 0x80),
        ("misaligned", HEAP + 0x88),
        ("code", 0),
        ("unmapped", 0x7FFF_FFF0),
    ] {
        c.add(
            "mem_128",
            run,
            &["family:memory-128", "high-registers"],
            vector(0, &words),
            seed(base),
        );
    }
    c.add(
        "mem_128_no_vector_mode",
        "",
        &["family:memory-128", "fault:VectorExtensionDisabled"],
        plain(&words),
        seed(HEAP + 0x80),
    );
    c.add(
        "mem_load128_no_vector_mode",
        "",
        &["family:memory-128", "fault:VectorExtensionDisabled"],
        plain(&[enc::encode_load128(op::memory::LOAD128, 210, 4, 255), HALT]),
        seed(HEAP + 0x80),
    );
    // Indexed literals.
    let literals = [
        (LiteralKindV1::I64, (-42_i64).to_le_bytes().to_vec()),
        (
            LiteralKindV1::PointerTlv,
            tlv(PointerType::Blob, b"baseline literal"),
        ),
        (LiteralKindV1::I64, i64::MIN.to_le_bytes().to_vec()),
    ];
    c.add(
        "lit_indexed",
        "",
        &["family:memory-literal"],
        literal_image(
            &literals,
            &[
                enc::encode_literal(op::memory::LDI64, 5, 0),
                enc::encode_literal(op::memory::LDLIT, 6, 1),
                enc::encode_literal(op::memory::LDI64, 200, 2),
                HALT,
            ],
        ),
        Vec::new(),
    );
    c.add(
        "lit_kind_mismatch",
        "",
        &["family:memory-literal", "admission:literal"],
        literal_image(
            &literals,
            &[enc::encode_literal(op::memory::LDI64, 5, 1), HALT],
        ),
        Vec::new(),
    );
    c.add(
        "lit_index_out_of_range",
        "",
        &["family:memory-literal", "admission:literal"],
        literal_image(
            &literals,
            &[enc::encode_literal(op::memory::LDLIT, 5, 7), HALT],
        ),
        Vec::new(),
    );
    c.add(
        "lit_without_table",
        "",
        &["family:memory-literal", "admission:literal"],
        plain(&[enc::encode_literal(op::memory::LDI64, 5, 0), HALT]),
        Vec::new(),
    );
    let mut truncated = meta(0, 0, DEFAULT_MAX_CYCLES).encode();
    truncated.extend_from_slice(b"LTLB");
    truncated.extend_from_slice(&1_u32.to_le_bytes());
    c.add(
        "lit_truncated_table",
        "",
        &["admission:literal"],
        truncated,
        Vec::new(),
    );
}

fn control(c: &mut Corpus) {
    // r(40 + i) becomes 1 when branch i is NOT taken.
    let mut words = Vec::new();
    for (index, opcode) in [
        op::control::BEQ,
        op::control::BNE,
        op::control::BLT,
        op::control::BGE,
        op::control::BLTU,
        op::control::BGEU,
    ]
    .into_iter()
    .enumerate()
    {
        words.push(enc::encode_branch(opcode, 1, 2, 2));
        words.push(ri(op::arithmetic::ADDI, 40 + index, 0, 1));
    }
    words.push(HALT);
    let program = plain(&words);
    for (run, a, b) in [
        ("equal", 5, 5),
        ("less", 3, 9),
        ("signed_vs_unsigned", u64::MAX, 1),
    ] {
        c.add(
            "ctl_branches",
            run,
            &["family:control-branch", "encoding:branch-imm8"],
            program.clone(),
            regs(&[(1, a), (2, b)]),
        );
    }
    // Backward branch: r5 = 1 + 2 + ... + r1.
    let sum_loop = [
        rr(op::arithmetic::ADD, 5, 5, 1),
        ri(op::arithmetic::ADDI, 1, 1, -1),
        enc::encode_branch(op::control::BNE, 1, 0, -2),
        HALT,
    ];
    for (run, count) in [("ten", 10), ("one", 1), ("thousand", 1000)] {
        c.add(
            "ctl_loop_sum",
            run,
            &["family:control-branch", "encoding:branch-imm8"],
            plain(&sum_loop),
            regs(&[(1, count)]),
        );
    }
    // Branch reach of the 8-bit offset: +127 words forward, then -127 back.
    let mut words = vec![ri(op::arithmetic::ADDI, 0, 0, 0); 130];
    words[0] = enc::encode_branch(op::control::BEQ, 0, 0, 127);
    words[1] = HALT;
    words[127] = ri(op::arithmetic::ADDI, 40, 0, 1);
    words[128] = enc::encode_branch(op::control::BEQ, 0, 0, -127);
    c.add(
        "ctl_branch_reach",
        "",
        &["family:control-branch", "encoding:branch-imm8"],
        plain(&words),
        Vec::new(),
    );
    // JAL with its 16-bit offset and explicit link register; JALR; JR; JMP.
    let mut words = vec![ri(op::arithmetic::ADDI, 0, 0, 0); 306];
    words[0] = enc::encode_jump(op::control::JAL, 200, 300);
    words[1] = ri(op::arithmetic::ADDI, 41, 0, 1);
    words[2] = enc::encode_offset24(op::control::JMP, 3);
    words[5] = rr(op::control::JR, 201, 0, 0);
    words[300] = ri(op::arithmetic::ADDI, 40, 0, 1);
    words[301] = enc::encode_ri(op::control::JALR, 202, 200, 0);
    words[304] = HALT;
    c.add(
        "ctl_jumps",
        "",
        &[
            "family:control-jump",
            "encoding:jal-imm16",
            "encoding:jmp-imm24",
            "high-registers",
        ],
        plain(&words),
        regs(&[(201, 304 * 4)]),
    );
    c.add(
        "ctl_jalr_alignment",
        "",
        &["family:control-jump"],
        plain(&[
            enc::encode_ri(op::control::JALR, 5, 1, 3),
            ri(op::arithmetic::ADDI, 40, 0, 1),
            ri(op::arithmetic::ADDI, 41, 0, 1),
            HALT,
        ]),
        regs(&[(1, 8)]),
    );
    c.add(
        "ctl_jmp_backward",
        "",
        &["family:control-jump", "encoding:jmp-imm24"],
        plain(&[
            enc::encode_offset24(op::control::JMP, 2),
            HALT,
            ri(op::arithmetic::ADDI, 40, 0, 1),
            enc::encode_offset24(op::control::JMP, -2),
        ]),
        Vec::new(),
    );
    c.add(
        "ctl_jals",
        "",
        &["family:control-jump", "encoding:jals-imm24"],
        plain(&[
            enc::encode_offset24(op::control::JALS, 2),
            HALT,
            ri(op::arithmetic::ADDI, 40, 0, 1),
            enc::encode_ri(op::control::JALR, 0, 1, 0),
        ]),
        Vec::new(),
    );
    let jr = plain(&[rr(op::control::JR, 1, 0, 0), HALT]);
    for (run, target) in [
        ("to_halt", 4),
        ("misaligned", 2),
        ("past_code", 8),
        ("far", 0x1000_0000),
    ] {
        c.add(
            "ctl_jr_target",
            run,
            &["family:control-jump", "fault:execute-permission"],
            jr.clone(),
            regs(&[(1, target)]),
        );
    }
    c.add(
        "ctl_branch_out_of_code",
        "",
        &["family:control-branch", "fault:execute-permission"],
        plain(&[enc::encode_branch(op::control::BEQ, 0, 0, 100), HALT]),
        Vec::new(),
    );
    c.add(
        "ctl_halt_only",
        "",
        &["family:control"],
        plain(&[HALT]),
        Vec::new(),
    );
    c.add(
        "ctl_missing_halt",
        "",
        &["fault:MissingHalt"],
        plain(&[ri(op::arithmetic::ADDI, 40, 0, 1)]),
        Vec::new(),
    );
    c.add(
        "ctl_empty_code",
        "",
        &["fault:MissingHalt"],
        plain(&[]),
        Vec::new(),
    );
}

fn system(c: &mut Corpus) {
    c.add(
        "sys_getgas",
        "",
        &["family:system"],
        plain(&[
            rr(op::system::GETGAS, 40, 0, 0),
            rr(op::system::GETGAS, 200, 0, 0),
            HALT,
        ]),
        Vec::new(),
    );
    // The baseline `SCALL` carries an 8-bit number (C10: 24 bits); `SYSTEM` carries 24 bits.
    c.add(
        "sys_scall_exit",
        "",
        &["family:system", "encoding:scall-imm8"],
        plain(&[
            scall(syscalls::SYSCALL_EXIT),
            ri(op::arithmetic::ADDI, 40, 0, 1),
            HALT,
        ]),
        regs(&[(10, 7)]),
    );
    c.add(
        "sys_scall_abort",
        "",
        &[
            "family:system",
            "encoding:scall-imm8",
            "fault:AssertionFailed",
        ],
        plain(&[scall(syscalls::SYSCALL_ABORT), HALT]),
        Vec::new(),
    );
    let alloc = plain(&[
        scall(syscalls::SYSCALL_ALLOC),
        ri(op::arithmetic::ADDI, 40, 10, 0),
        enc::encode_store(op::memory::STORE64, 40, 5, 0),
        enc::encode_load(op::memory::LOAD64, 41, 40, 0),
        HALT,
    ]);
    for (run, size) in [
        ("small", 64),
        ("zero", 0),
        ("huge", 0x7FFF_FFFF),
        ("max", u64::MAX),
    ] {
        c.add(
            "sys_scall_alloc",
            run,
            &["family:system", "syscall:ALLOC", "fault:memory-limit"],
            alloc.clone(),
            regs(&[(10, size), (5, 0x1122_3344_5566_7788)]),
        );
    }
    c.add(
        "sys_scall_grow_heap",
        "",
        &["family:system", "syscall:GROW_HEAP"],
        plain(&[scall(syscalls::SYSCALL_GROW_HEAP), HALT]),
        regs(&[(10, 4096)]),
    );
    c.add(
        "sys_scall_misc",
        "",
        &["family:system", "syscall:context"],
        plain(&[
            scall(syscalls::SYSCALL_CURRENT_TIME_MS),
            ri(op::arithmetic::ADDI, 40, 10, 0),
            scall(syscalls::SYSCALL_COMMIT_OUTPUT),
            ri(op::arithmetic::ADDI, 41, 10, 0),
            HALT,
        ]),
        Vec::new(),
    );
    // The summary syscall prices and reports the VM's own trace logs, so this case is recorded
    // without the additional PC-traced run.
    c.add(
        "sys_scall_execution_summary",
        "",
        &["family:system", "syscall:EXECUTION_SUMMARY"],
        plain(&[
            ri(op::arithmetic::ADDI, 40, 0, 5),
            scall(syscalls::SYSCALL_EXECUTION_SUMMARY),
            ri(op::arithmetic::ADDI, 41, 10, 0),
            scall(syscalls::SYSCALL_TLV_LEN),
            HALT,
        ]),
        Vec::new(),
    )
    .traced = false;
    // Pointer-ABI syscalls over INPUT TLVs.
    let hash = plain(&[
        scall(syscalls::SYSCALL_SHA256_HASH),
        ri(op::arithmetic::ADDI, 40, 10, 0),
        scall(syscalls::SYSCALL_TLV_LEN),
        HALT,
    ]);
    c.add(
        "sys_scall_sha256",
        "blob",
        &["family:system", "syscall:pointer-abi"],
        hash.clone(),
        vec![Step::AllocInputTlv {
            tlv: tlv(PointerType::Blob, b"abc"),
            register: 10,
        }],
    );
    c.add(
        "sys_scall_sha256",
        "wrong_pointer_type",
        &[
            "family:system",
            "syscall:pointer-abi",
            "fault:NoritoInvalid",
        ],
        hash.clone(),
        vec![Step::AllocInputTlv {
            tlv: tlv(PointerType::Json, b"{}"),
            register: 10,
        }],
    );
    c.add(
        "sys_scall_sha256",
        "null_pointer",
        &["family:system", "syscall:pointer-abi", "fault:pointer"],
        hash.clone(),
        Vec::new(),
    );
    let mut corrupt = tlv(PointerType::Blob, b"abc");
    let last = corrupt.len() - 1;
    corrupt[last] ^= 1;
    c.add(
        "sys_scall_sha256",
        "hash_mismatch",
        &["family:system", "syscall:pointer-abi", "fault:pointer"],
        hash,
        vec![Step::AllocInputTlv {
            tlv: corrupt,
            register: 10,
        }],
    );
    // Public input lookup by name.
    let name: iroha_model_base::name::Name = "payload".parse().expect("fixture input name");
    let name_tlv = tlv(
        PointerType::Name,
        &ivm::codec::encode_canonical_norito(&name).expect("canonical name"),
    );
    let lookup = plain(&[
        scall(syscalls::SYSCALL_GET_PUBLIC_INPUT),
        ri(op::arithmetic::ADDI, 40, 10, 0),
        scall(syscalls::SYSCALL_TLV_LEN),
        HALT,
    ]);
    let case = c.add(
        "sys_scall_public_input",
        "present",
        &["family:system", "syscall:public-input"],
        lookup.clone(),
        vec![Step::AllocInputTlv {
            tlv: name_tlv.clone(),
            register: 10,
        }],
    );
    case.public_inputs = vec![(
        "payload".to_owned(),
        tlv(PointerType::Blob, b"baseline public input"),
    )];
    c.add(
        "sys_scall_public_input",
        "absent",
        &[
            "family:system",
            "syscall:public-input",
            "fault:PermissionDenied",
        ],
        lookup,
        vec![Step::AllocInputTlv {
            tlv: name_tlv,
            register: 10,
        }],
    );
    // Numeric V1 syscalls through the 24-bit `SYSTEM` form.
    let numeric = |operation: u32| {
        plain(&[
            ri(op::arithmetic::ADDI, 10, 1, 0),
            scall(syscalls::SYSCALL_INT_FROM_I64),
            ri(op::arithmetic::ADDI, 20, 10, 0),
            ri(op::arithmetic::ADDI, 10, 2, 0),
            scall(syscalls::SYSCALL_INT_FROM_I64),
            ri(op::arithmetic::ADDI, 11, 10, 0),
            ri(op::arithmetic::ADDI, 10, 20, 0),
            ri(op::arithmetic::ADDI, 12, 0, 0),
            scall(operation),
            ri(op::arithmetic::ADDI, 40, 10, 0),
            scall(syscalls::SYSCALL_INT_TO_I64),
            HALT,
        ])
    };
    for (name, operation, runs) in [
        (
            "add",
            syscalls::SYSCALL_INT_ADD,
            &[("small", 40, 2), ("negative", neg(-40), 2)][..],
        ),
        (
            "div",
            syscalls::SYSCALL_INT_DIV,
            &[("ok", 84, 2), ("by_zero", 1, 0)][..],
        ),
        (
            "mul",
            syscalls::SYSCALL_INT_MUL,
            &[("wide", neg(i64::MIN), neg(i64::MIN))][..],
        ),
    ] {
        let program = numeric(operation);
        for (run, a, b) in runs {
            c.add(
                &format!("sys_numeric_int_{name}"),
                run,
                &[
                    "family:system",
                    "encoding:system-imm24",
                    "syscall:numeric",
                    "fault:numeric",
                ],
                program.clone(),
                regs(&[(1, *a), (2, *b)]),
            );
        }
    }
    c.add(
        "sys_numeric_null_pointers",
        "",
        &["family:system", "syscall:numeric", "fault:pointer-abi"],
        plain(&[scall(syscalls::SYSCALL_INT_ADD), HALT]),
        Vec::new(),
    );
    // Numbers outside the ABI surface, including the hardware-probe numbers that stay refused.
    for (name, word) in [
        (
            "scall_unassigned_54",
            enc::encode_sys(op::system::SCALL, 0x54),
        ),
        ("scall_f2", enc::encode_sys(op::system::SCALL, 0xF2)),
        ("scall_f3", enc::encode_sys(op::system::SCALL, 0xF3)),
        ("system_unassigned", enc::encode_syscallx(0x01_0040)),
        ("system_max", enc::encode_syscallx(0x00FF_FFFF)),
    ] {
        c.add(
            &format!("sys_unknown_{name}"),
            "",
            &["family:system", "admission:syscall", "fault:UnknownSyscall"],
            plain(&[word, HALT]),
            Vec::new(),
        );
    }
    let contract_only = syscalls::abi_syscall_list()
        .iter()
        .copied()
        .find(|number| {
            !syscalls::is_generic_program_syscall_allowed(ivm::SyscallPolicy::AbiV1, *number)
        })
        .expect("the ABI has a contract-only syscall");
    c.add(
        "sys_generic_denied",
        "",
        &["family:system", "admission:GenericSyscallNotAllowed"],
        plain(&[scall(contract_only), HALT]),
        Vec::new(),
    );
}

fn vector_ops(c: &mut Corpus) {
    let lanes = |register: usize, stride: usize, values: &[u64]| -> Vec<(usize, u64)> {
        values
            .iter()
            .enumerate()
            .map(|(lane, value)| (VECTOR_BASE + register * stride + lane, *value))
            .collect()
    };
    let all = [
        rr(op::crypto::VADD32, 2, 0, 1),
        rr(op::crypto::VADD64, 3, 0, 1),
        rr(op::crypto::VAND, 4, 0, 1),
        rr(op::crypto::VXOR, 5, 0, 1),
        rr(op::crypto::VOR, 6, 0, 1),
        ri(op::crypto::VROT32, 7, 0, 8),
        ri(op::crypto::VROT32, 8, 0, -1),
        HALT,
    ];
    let a = [
        0xFFFF_FFFF,
        0x1,
        0x1234_5678,
        0x8000_0000,
        5,
        6,
        7,
        0xFFFF_0000,
    ];
    let b = [
        0x1,
        0xFFFF_FFFF,
        0xFFFF_FFFF,
        0x8000_0000,
        50,
        60,
        70,
        0x0000_FFFF,
    ];
    for (run, vector_length) in [("default_length", 0_u8), ("length_4", 4), ("length_8", 8)] {
        let stride = if vector_length == 0 {
            4
        } else {
            usize::from(vector_length)
        };
        let mut seed = lanes(0, stride, &a[..stride]);
        seed.extend(lanes(1, stride, &b[..stride]));
        c.add(
            &format!("vec_all_{run}"),
            "",
            &[
                "family:vector",
                "header:vector-length",
                "gas:vector-scaling",
            ],
            vector(vector_length, &all),
            regs(&seed),
        );
    }
    // Tail lanes: a length that is not a multiple of four.
    let mut seed = lanes(0, 6, &a[..6]);
    seed.extend(lanes(1, 6, &b[..6]));
    c.add(
        "vec_tail_length_6",
        "",
        &["family:vector", "header:vector-length"],
        vector(
            6,
            &[
                rr(op::crypto::VADD32, 2, 0, 1),
                rr(op::crypto::VXOR, 3, 0, 1),
                ri(op::crypto::VROT32, 4, 0, 4),
                rr(op::crypto::VADD64, 5, 0, 1),
                HALT,
            ],
        ),
        regs(&seed),
    );
    // SETVL changes the logical length and therefore register stride and gas.
    let mut seed = lanes(0, 2, &a[..2]);
    seed.extend(lanes(1, 2, &b[..2]));
    c.add(
        "vec_setvl",
        "",
        &["family:vector", "opcode:SETVL", "gas:vector-scaling"],
        vector(
            4,
            &[
                rr(op::crypto::SETVL, 0, 0, 2),
                rr(op::crypto::VADD32, 2, 0, 1),
                rr(op::crypto::SETVL, 0, 0, 0),
                rr(op::crypto::VADD32, 20, 0, 1),
                rr(op::crypto::SETVL, 0, 0, 64),
                rr(op::crypto::VAND, 1, 0, 0),
                HALT,
            ],
        ),
        regs(&seed),
    );
    c.add(
        "vec_setvl_too_large",
        "",
        &["family:vector", "fault:InvalidVectorLength"],
        vector(4, &[rr(op::crypto::SETVL, 0, 0, 65), HALT]),
        Vec::new(),
    );
    c.add(
        "vec_vadd64_odd_length",
        "",
        &["family:vector", "fault:InvalidVectorLength"],
        vector(
            4,
            &[
                rr(op::crypto::SETVL, 0, 0, 3),
                rr(op::crypto::VADD64, 2, 0, 1),
                HALT,
            ],
        ),
        Vec::new(),
    );
    c.add(
        "vec_register_out_of_bounds",
        "",
        &["family:vector", "fault:RegisterOutOfBounds"],
        vector(64, &[rr(op::crypto::VADD32, 3, 0, 1), HALT]),
        Vec::new(),
    );
    c.add(
        "vec_max_header_length",
        "",
        &["family:vector", "header:vector-length"],
        vector(64, &[rr(op::crypto::VOR, 2, 0, 1), HALT]),
        regs(&[
            (VECTOR_BASE, 0xF0),
            (VECTOR_BASE + 64, 0x0F),
            (VECTOR_BASE + 127, 9),
        ]),
    );
    for (name, word) in [
        ("vadd32", rr(op::crypto::VADD32, 2, 0, 1)),
        ("vadd64", rr(op::crypto::VADD64, 2, 0, 1)),
        ("vand", rr(op::crypto::VAND, 2, 0, 1)),
        ("vxor", rr(op::crypto::VXOR, 2, 0, 1)),
        ("vor", rr(op::crypto::VOR, 2, 0, 1)),
        ("vrot32", ri(op::crypto::VROT32, 2, 0, 1)),
        ("setvl", rr(op::crypto::SETVL, 0, 0, 4)),
    ] {
        c.add(
            &format!("vec_disabled_{name}"),
            "",
            &["family:vector", "fault:VectorExtensionDisabled"],
            plain(&[word, HALT]),
            Vec::new(),
        );
    }
    // The header may carry a vector length while the VECTOR mode bit is off.
    c.add(
        "vec_length_without_mode",
        "",
        &[
            "family:vector",
            "header:vector-length",
            "fault:VectorExtensionDisabled",
        ],
        image(
            &meta(0, 8, DEFAULT_MAX_CYCLES),
            &[rr(op::crypto::VADD32, 2, 0, 1), HALT],
        ),
        Vec::new(),
    );
    c.add(
        "par_markers",
        "",
        &["family:parallel"],
        plain(&[
            rr(op::crypto::PARBEGIN, 0, 0, 0),
            ri(op::arithmetic::ADDI, 40, 0, 1),
            rr(op::crypto::PAREND, 0, 0, 0),
            rr(op::crypto::PARBEGIN, 9, 8, 7),
            rr(op::crypto::PAREND, 255, 255, 255),
            HALT,
        ]),
        Vec::new(),
    );
}

/// RFC 8032 section 7.1 TEST 2 (one-byte message).
const ED25519_PUBLIC: &str = "3d4017c3e843895a92b70aa74d1b7ebc9c982ccf2ec4968cc0cd55f12af4660c";
const ED25519_MESSAGE: &str = "72";
const ED25519_SIGNATURE: &str = "92a009a9f0d4cab8720e820b5f642540a2b27b5416503f8fb3762223ebdb69da085ac1e43e15996e458f3613d0f11d8c387b2eaeb4302aeeb00d291612bb0c00";

fn crypto(c: &mut Corpus) {
    // SHA-256 compression of the padded message "abc" from the initial state.
    let mut block = [0_u8; 64];
    block[..3].copy_from_slice(b"abc");
    block[3] = 0x80;
    block[63] = 24;
    let iv: [u64; 8] = [
        0x6a09_e667,
        0xbb67_ae85,
        0x3c6e_f372,
        0xa54f_f53a,
        0x510e_527f,
        0x9b05_688c,
        0x1f83_d9ab,
        0x5be0_cd19,
    ];
    let mut steps = vec![Step::StoreBytes {
        addr: HEAP + 0x200,
        bytes: block.to_vec(),
    }];
    steps.extend(regs(&[(10, HEAP + 0x200)]));
    for (lane, word) in iv.iter().enumerate() {
        steps.push(Step::SetRegister {
            index: VECTOR_BASE + 8 + lane,
            value: *word,
        });
    }
    let sha256 = [rr(op::crypto::SHA256BLOCK, 2, 10, 0), HALT];
    c.add(
        "crypto_sha256block",
        "abc",
        &["family:direct-crypto", "opcode:SHA256BLOCK"],
        vector(4, &sha256),
        steps.clone(),
    );
    c.add(
        "crypto_sha256block_no_vector_mode",
        "",
        &["family:direct-crypto", "fault:VectorExtensionDisabled"],
        plain(&sha256),
        steps,
    );
    c.add(
        "crypto_sha256block",
        "unmapped_block",
        &["family:direct-crypto", "fault:memory"],
        vector(4, &sha256),
        regs(&[(10, 0x7FFF_FFF0)]),
    );
    // SHA-3 absorb of one rate block into a zero state.
    let mut block = [0_u8; 136];
    for (index, byte) in block.iter_mut().enumerate() {
        *byte = index as u8;
    }
    c.add(
        "crypto_sha3block",
        "",
        &["family:direct-crypto", "opcode:SHA3BLOCK"],
        plain(&[rr(op::crypto::SHA3BLOCK, 12, 10, 11), HALT]),
        vec![
            Step::StoreBytes {
                addr: HEAP + 0x600,
                bytes: block.to_vec(),
            },
            Step::SetRegister {
                index: 10,
                value: HEAP + 0x400,
            },
            Step::SetRegister {
                index: 11,
                value: HEAP + 0x600,
            },
            Step::SetRegister {
                index: 12,
                value: HEAP + 0x800,
            },
        ],
    );
    let aes = [
        rr(op::crypto::AESENC, 40, 1, 3),
        rr(op::crypto::AESDEC, 42, 40, 3),
        rr(op::crypto::AESDEC, 200, 1, 3),
        HALT,
    ];
    let aes_seed = regs(&[
        (1, 0x0706_0504_0302_0100),
        (2, 0x0F0E_0D0C_0B0A_0908),
        (3, 0x1716_1514_1312_1110),
        (4, 0x1F1E_1D1C_1B1A_1918),
    ]);
    c.add(
        "crypto_aes_round",
        "",
        &[
            "family:direct-crypto",
            "opcode:AESENC",
            "opcode:AESDEC",
            "high-registers",
        ],
        vector(4, &aes),
        aes_seed.clone(),
    );
    c.add(
        "crypto_aes_round_no_vector_mode",
        "",
        &["family:direct-crypto", "fault:VectorExtensionDisabled"],
        plain(&aes),
        aes_seed,
    );
    c.add(
        "crypto_aes_register_out_of_bounds",
        "",
        &["family:direct-crypto", "fault:RegisterOutOfBounds"],
        vector(4, &[rr(op::crypto::AESENC, 255, 1, 3), HALT]),
        Vec::new(),
    );
    let mut input = [0_u8; 64];
    for (index, byte) in input.iter_mut().enumerate() {
        *byte = (index as u8).wrapping_mul(3);
    }
    c.add(
        "crypto_blake2s",
        "",
        &["family:direct-crypto", "opcode:BLAKE2S"],
        plain(&[rr(op::crypto::BLAKE2S, 40, 10, 0), HALT]),
        vec![
            Step::StoreBytes {
                addr: HEAP + 0x200,
                bytes: input.to_vec(),
            },
            Step::SetRegister {
                index: 10,
                value: HEAP + 0x200,
            },
        ],
    );
    c.add(
        "crypto_poseidon",
        "",
        &[
            "family:direct-crypto",
            "opcode:POSEIDON2",
            "opcode:POSEIDON6",
            "high-registers",
        ],
        plain(&[
            enc::encode_poseidon2(40, 1, 2),
            enc::encode_poseidon6(41, 1),
            enc::encode_poseidon6(42, 250),
            HALT,
        ]),
        regs(&[
            (1, 1),
            (2, 2),
            (3, 3),
            (4, 4),
            (5, 5),
            (6, 6),
            (250, 11),
            (255, 12),
        ]),
    );
    c.add(
        "crypto_poseidon6_reserved_operand",
        "",
        &["family:direct-crypto", "fault:DecodeError"],
        plain(&[rr(op::crypto::POSEIDON6, 41, 1, 1), HALT]),
        Vec::new(),
    );
    c.add(
        "crypto_poseidon6_input_overflow",
        "",
        &["family:direct-crypto", "fault:DecodeError"],
        plain(&[rr(op::crypto::POSEIDON6, 41, 251, 0), HALT]),
        Vec::new(),
    );
    // Signature opcodes take three Blob TLVs: rd = public key (and result), rs1 = message,
    // rs2 = signature.
    let blob = |hex_text: &str| tlv(PointerType::Blob, &hex::decode(hex_text).expect("hex"));
    let inputs = |public: Vec<u8>, message: Vec<u8>, signature: Vec<u8>| {
        vec![
            Step::AllocInputTlv {
                tlv: public,
                register: 12,
            },
            Step::AllocInputTlv {
                tlv: message,
                register: 10,
            },
            Step::AllocInputTlv {
                tlv: signature,
                register: 11,
            },
        ]
    };
    let mut bad_signature = hex::decode(ED25519_SIGNATURE).expect("hex");
    bad_signature[0] ^= 1;
    for (name, opcode) in [
        ("ed25519verify", op::crypto::ED25519VERIFY),
        ("ecdsaverify", op::crypto::ECDSAVERIFY),
        ("dilithiumverify", op::crypto::DILITHIUMVERIFY),
    ] {
        let program = plain(&[
            rr(opcode, 12, 10, 11),
            rr(op::system::GETGAS, 40, 0, 0),
            HALT,
        ]);
        c.add(
            &format!("crypto_{name}"),
            "ed25519_vector",
            &["family:direct-crypto", "gas:signature-payload"],
            program.clone(),
            inputs(
                blob(ED25519_PUBLIC),
                blob(ED25519_MESSAGE),
                blob(ED25519_SIGNATURE),
            ),
        );
        c.add(
            &format!("crypto_{name}"),
            "bad_signature",
            &["family:direct-crypto"],
            program.clone(),
            inputs(
                blob(ED25519_PUBLIC),
                blob(ED25519_MESSAGE),
                tlv(PointerType::Blob, &bad_signature),
            ),
        );
        c.add(
            &format!("crypto_{name}"),
            "null_pointers",
            &["family:direct-crypto"],
            program,
            Vec::new(),
        );
    }
    let batch = plain(&[rr(op::crypto::ED25519BATCHVERIFY, 40, 10, 41), HALT]);
    c.add(
        "crypto_ed25519batchverify",
        "null_pointer",
        &["family:direct-crypto", "opcode:ED25519BATCHVERIFY"],
        batch.clone(),
        regs(&[(41, 99)]),
    );
    c.add(
        "crypto_ed25519batchverify",
        "not_a_request",
        &[
            "family:direct-crypto",
            "opcode:ED25519BATCHVERIFY",
            "gas:signature-payload",
        ],
        batch,
        vec![
            Step::AllocInputTlv {
                tlv: tlv(PointerType::NoritoBytes, &[0xAB; 40]),
                register: 10,
            },
            Step::SetRegister {
                index: 41,
                value: 99,
            },
        ],
    );
}

fn zk_ops(c: &mut Corpus) {
    let field = [
        rr(op::zk::FADD, 40, 1, 2),
        rr(op::zk::FSUB, 41, 1, 2),
        rr(op::zk::FMUL, 42, 1, 2),
        rr(op::zk::FINV, 43, 1, 0),
        rr(op::zk::FSUB, 200, 2, 1),
        HALT,
    ];
    for (run, a, b) in [
        ("small", 7, 3),
        ("wrap", u64::MAX, u64::MAX - 1),
        ("inverse_of_zero", 0, 5),
    ] {
        c.add(
            "zk_field",
            run,
            &["family:zk-helper", "header:zk-mode", "gas:zk-padding"],
            zk(64, &field),
            regs(&[(1, a), (2, b)]),
        );
    }
    let asserts = [
        rr(op::zk::ASSERT, 0, 1, 0),
        rr(op::zk::ASSERT_EQ, 0, 2, 3),
        ri(op::zk::ASSERT_RANGE, 0, 4, 8),
        ri(op::arithmetic::ADDI, 40, 0, 1),
        HALT,
    ];
    for (run, zero, left, right, ranged) in [
        ("hold", 0, 9, 9, 255),
        ("assert_fails", 1, 9, 9, 255),
        ("assert_eq_fails", 0, 9, 8, 255),
        ("assert_range_fails", 0, 9, 9, 256),
    ] {
        c.add(
            "zk_asserts",
            run,
            &[
                "family:zk-helper",
                "header:zk-mode",
                "fault:AssertionFailed",
            ],
            zk(64, &asserts),
            regs(&[(1, zero), (2, left), (3, right), (4, ranged)]),
        );
    }
    c.add(
        "zk_assert_range_bits",
        "",
        &["family:zk-helper", "header:zk-mode"],
        zk(
            64,
            &[
                ri(op::zk::ASSERT_RANGE, 0, 1, 64),
                ri(op::zk::ASSERT_RANGE, 0, 2, 0),
                ri(op::zk::ASSERT_RANGE, 0, 2, 65),
                HALT,
            ],
        ),
        regs(&[(1, u64::MAX)]),
    );
    for (name, word) in [
        ("assert", rr(op::zk::ASSERT, 0, 1, 0)),
        ("assert_eq", rr(op::zk::ASSERT_EQ, 0, 1, 1)),
        ("fadd", rr(op::zk::FADD, 40, 1, 2)),
        ("fsub", rr(op::zk::FSUB, 40, 1, 2)),
        ("fmul", rr(op::zk::FMUL, 40, 1, 2)),
        ("finv", rr(op::zk::FINV, 40, 1, 0)),
        ("assert_range", ri(op::zk::ASSERT_RANGE, 0, 1, 8)),
    ] {
        c.add(
            &format!("zk_disabled_{name}"),
            "",
            &["family:zk-helper", "fault:ZkExtensionDisabled"],
            plain(&[word, HALT]),
            Vec::new(),
        );
    }
    // Baseline privacy rule: operand tags must be equal (C10 joins them with OR).
    let tagged = |words: &[u32], private: &[usize]| -> (Vec<u8>, Vec<Step>) {
        let mut steps = regs(&[(1, 6), (2, 3)]);
        steps.extend(
            private
                .iter()
                .map(|index| Step::TagRegister { index: *index }),
        );
        (zk(64, words), steps)
    };
    for (name, words, private) in [
        (
            "add_mixed_tags",
            vec![rr(op::arithmetic::ADD, 40, 1, 2), HALT],
            vec![1],
        ),
        (
            "add_private_operands",
            vec![
                rr(op::arithmetic::ADD, 40, 1, 2),
                rr(op::arithmetic::NOT, 41, 40, 0),
                HALT,
            ],
            vec![1, 2],
        ),
        (
            "div_private_operands",
            vec![rr(op::arithmetic::DIV, 40, 1, 2), HALT],
            vec![1, 2],
        ),
        (
            "branch_on_private",
            vec![enc::encode_branch(op::control::BEQ, 1, 2, 1), HALT],
            vec![1],
        ),
        (
            "assert_private",
            vec![rr(op::zk::ASSERT, 0, 1, 0), HALT],
            vec![1],
        ),
        (
            "fadd_mixed_tags",
            vec![rr(op::zk::FADD, 40, 1, 2), HALT],
            vec![2],
        ),
        (
            "poseidon_private",
            vec![enc::encode_poseidon2(40, 1, 2), HALT],
            vec![1],
        ),
        (
            "cmov_private_condition",
            vec![rr(op::arithmetic::CMOV, 40, 1, 2), HALT],
            vec![2],
        ),
        (
            "store_private_to_heap",
            vec![enc::encode_store(op::memory::STORE64, 4, 1, 0), HALT],
            vec![1],
        ),
    ] {
        let (program, mut steps) = tagged(&words, &private);
        steps.push(Step::SetRegister {
            index: 4,
            value: HEAP + 0x100,
        });
        c.add(
            &format!("zk_privacy_{name}"),
            "",
            &[
                "family:zk-helper",
                "privacy:equal-tags",
                "fault:PrivacyViolation",
            ],
            program,
            steps,
        );
    }
    // ZK padding: the trace is padded to `max_cycles`, one gas unit per padded cycle.
    let padded = [ri(op::arithmetic::ADDI, 40, 0, 1), HALT];
    c.add(
        "zk_padding_16",
        "",
        &["header:zk-mode", "gas:zk-padding"],
        zk(16, &padded),
        Vec::new(),
    );
    c.add(
        "zk_padding_16",
        "gas_short_of_padding",
        &["header:zk-mode", "gas:zk-padding", "fault:OutOfGas"],
        zk(16, &padded),
        Vec::new(),
    )
    .gas = Gas::Explicit(10);
    c.add(
        "zk_padding_default_cycles",
        "",
        &[
            "header:zk-mode",
            "gas:zk-padding",
            "admission:MissingMaxCycles",
        ],
        zk(0, &padded),
        Vec::new(),
    );
    c.add(
        "zk_vector_mode",
        "",
        &["header:zk-mode", "family:vector"],
        image(
            &meta(ivm_mode::ZK | ivm_mode::VECTOR, 4, 32),
            &[
                rr(op::crypto::VADD32, 2, 0, 1),
                rr(op::zk::FADD, 40, 1, 2),
                HALT,
            ],
        ),
        regs(&[(VECTOR_BASE, 1), (VECTOR_BASE + 4, 2), (1, 5), (2, 6)]),
    );
}

fn budgets(c: &mut Corpus) {
    let sum_loop = plain(&[
        rr(op::arithmetic::ADD, 5, 5, 1),
        ri(op::arithmetic::ADDI, 1, 1, -1),
        enc::encode_branch(op::control::BNE, 1, 0, -2),
        HALT,
    ]);
    // Gas boundaries of the ten-iteration loop: the exact cost is whatever the anchor charges;
    // these limits bracket it from far below to far above.
    for limit in [
        0, 1, 2, 10, 28, 29, 30, 31, 32, 33, 40, 60, 61, 62, 63, 64, 100, 1_000,
    ] {
        c.add(
            "ctl_loop_sum",
            &format!("gas_{limit}"),
            &["gas:boundary", "fault:OutOfGas"],
            sum_loop.clone(),
            regs(&[(1, 10)]),
        )
        .gas = Gas::Explicit(limit);
    }
    // Cycle ceiling of a non-ZK program (C10: positive `max_cycles` only with ZK).
    let words = [
        rr(op::arithmetic::ADD, 5, 5, 1),
        ri(op::arithmetic::ADDI, 1, 1, -1),
        enc::encode_branch(op::control::BNE, 1, 0, -2),
        HALT,
    ];
    for max_cycles in [1, 3, 30, 31, 32, 0] {
        c.add(
            &format!("cycles_loop_max_{max_cycles}"),
            "",
            &["cycles:non-zk-ceiling", "fault:ExceededMaxCycles"],
            image(&meta(0, 0, max_cycles), &words),
            regs(&[(1, 10)]),
        );
    }
    // Multi-cycle instructions against the ceiling: ISQRT 6, GCD and DIV_CEIL 12, MEAN 3.
    for (name, opcode) in [
        ("isqrt", op::arithmetic::ISQRT),
        ("gcd", op::arithmetic::GCD),
        ("div_ceil", op::arithmetic::DIV_CEIL),
        ("mean", op::arithmetic::MEAN),
    ] {
        for max_cycles in [2, 3, 4, 6, 7, 12, 13] {
            c.add(
                &format!("cycles_{name}_max_{max_cycles}"),
                "",
                &["cycles:multi-cycle-opcode", "fault:ExceededMaxCycles"],
                image(&meta(0, 0, max_cycles), &[rr(opcode, 3, 1, 2), HALT]),
                regs(&[(1, 48), (2, 18)]),
            );
        }
    }
    // One shared cycle allowance (`VmCycleBudget`) bounds the run independently of the header:
    // the interpreter reserves each instruction's full cycle weight before executing it.
    for budget in [1, 5, 30, 31, 32] {
        c.add(
            "ctl_loop_sum",
            &format!("cycle_budget_{budget}"),
            &["cycles:shared-budget", "fault:ExceededMaxCycles"],
            sum_loop.clone(),
            regs(&[(1, 10)]),
        )
        .cycle_budget = Some(budget);
    }
    for (name, opcode) in [
        ("isqrt", op::arithmetic::ISQRT),
        ("gcd", op::arithmetic::GCD),
        ("mean", op::arithmetic::MEAN),
    ] {
        for budget in [1, 3, 4, 6, 7, 12, 13] {
            c.add(
                &format!("cycles_{name}"),
                &format!("cycle_budget_{budget}"),
                &["cycles:shared-budget", "cycles:multi-cycle-opcode"],
                plain(&[rr(opcode, 3, 1, 2), HALT]),
                regs(&[(1, 48), (2, 18)]),
            )
            .cycle_budget = Some(budget);
        }
    }
    for budget in [1, 10, 16, 17] {
        c.add(
            "zk_padding_16",
            &format!("cycle_budget_{budget}"),
            &["cycles:shared-budget", "gas:zk-padding"],
            zk(16, &[ri(op::arithmetic::ADDI, 40, 0, 1), HALT]),
            Vec::new(),
        )
        .cycle_budget = Some(budget);
    }
    // The host API can override the header's cycle limit.
    c.add(
        "ctl_loop_sum",
        "set_max_cycles_5",
        &["cycles:non-zk-ceiling", "fault:ExceededMaxCycles"],
        sum_loop,
        vec![
            Step::SetRegister {
                index: 1,
                value: 10,
            },
            Step::SetMaxCycles { max: 5 },
        ],
    );
}

fn admission(c: &mut Corpus) {
    let code = [ri(op::arithmetic::ADDI, 40, 0, 1), HALT];
    let good = plain(&code);
    let patched = |edit: &dyn Fn(&mut Vec<u8>)| {
        let mut bytes = good.clone();
        edit(&mut bytes);
        bytes
    };
    // Header byte layout: magic 0..4, major 4, minor 5, mode 6, vector_length 7,
    // max_cycles 8..16, abi_version 16, abi_hash 17..49.
    let cases: Vec<(&str, &str, Vec<u8>)> = vec![
        ("ok", "admission:ok", good.clone()),
        ("bad_magic", "admission:header", patched(&|b| b[0] = b'X')),
        (
            "version_major_2",
            "admission:UnsupportedVersion",
            patched(&|b| b[4] = 2),
        ),
        (
            "version_minor_0",
            "admission:UnsupportedVersion",
            patched(&|b| b[5] = 0),
        ),
        (
            "version_minor_2",
            "admission:UnsupportedVersion",
            patched(&|b| b[5] = 2),
        ),
        (
            "mode_unknown_bit",
            "admission:UnsupportedFeatureBits",
            patched(&|b| b[6] = 0x80),
        ),
        (
            "mode_unknown_with_known",
            "admission:UnsupportedFeatureBits",
            patched(&|b| b[6] = 0x07),
        ),
        (
            "abi_version_0",
            "admission:UnsupportedAbiVersion",
            patched(&|b| b[16] = 0),
        ),
        (
            "abi_version_2",
            "admission:UnsupportedAbiVersion",
            patched(&|b| b[16] = 2),
        ),
        ("vector_length_64", "admission:ok", patched(&|b| b[7] = 64)),
        (
            "vector_length_65",
            "admission:VectorLengthTooLarge",
            patched(&|b| b[7] = 65),
        ),
        (
            "vector_length_255",
            "admission:VectorLengthTooLarge",
            patched(&|b| b[7] = 255),
        ),
        (
            "vector_length_65_with_mode",
            "admission:VectorLengthTooLarge",
            patched(&|b| {
                b[6] = ivm_mode::VECTOR;
                b[7] = 65;
            }),
        ),
        (
            "abi_hash_mismatch",
            "admission:ArtifactAbiHashMismatch",
            patched(&|b| b[17] ^= 0xFF),
        ),
        (
            "abi_hash_zero",
            "admission:ArtifactAbiHashMismatch",
            patched(&|b| b[17..49].fill(0)),
        ),
        (
            "precedence_version_first",
            "admission:precedence",
            patched(&|b| {
                b[4] = 2;
                b[6] = 0x80;
                b[7] = 65;
                b[16] = 3;
                b[17] ^= 0xFF;
            }),
        ),
        (
            "precedence_mode_before_abi",
            "admission:precedence",
            patched(&|b| {
                b[6] = 0x80;
                b[7] = 65;
                b[16] = 3;
                b[17] ^= 0xFF;
            }),
        ),
        (
            "precedence_abi_before_vector",
            "admission:precedence",
            patched(&|b| {
                b[7] = 65;
                b[16] = 3;
                b[17] ^= 0xFF;
            }),
        ),
        (
            "precedence_vector_before_hash",
            "admission:precedence",
            patched(&|b| {
                b[7] = 65;
                b[17] ^= 0xFF;
            }),
        ),
        ("truncated_header", "admission:header", good[..48].to_vec()),
        ("empty", "admission:header", Vec::new()),
        ("code_not_word_aligned", "admission:code", {
            let mut bytes = good.clone();
            bytes.extend_from_slice(&[0x49, 0, 0]);
            bytes
        }),
        (
            "invalid_opcode_ff",
            "admission:InvalidOpcode",
            plain(&[0xFFFF_FFFF, HALT]),
        ),
        (
            "invalid_opcode_00",
            "admission:InvalidOpcode",
            plain(&[0, HALT]),
        ),
        (
            "reserved_iso20022_opcode",
            "admission:InvalidOpcode",
            plain(&[u32::from(op::iso20022::MSG_CREATE) << 24, HALT]),
        ),
        (
            "invalid_opcode_after_halt",
            "admission:InvalidOpcode",
            plain(&[HALT, 0xFFFF_FFFF]),
        ),
        (
            "max_cycles_zero",
            "admission:MissingMaxCycles",
            image(&meta(0, 0, 0), &code),
        ),
        (
            "max_cycles_at_upper_bound",
            "admission:ok",
            image(&meta(0, 0, 1_000_000), &code),
        ),
        (
            "max_cycles_above_upper_bound",
            "admission:MaxCyclesExceedsUpperBound",
            image(&meta(0, 0, 1_000_001), &code),
        ),
        (
            "max_cycles_at_fuel",
            "admission:MaxCyclesExceedsUpperBound",
            image(&meta(0, 0, 55_000_000), &code),
        ),
        (
            "max_cycles_above_fuel",
            "admission:MaxCyclesExceedsFuel",
            image(&meta(0, 0, 55_000_001), &code),
        ),
        (
            "max_cycles_u64_max",
            "admission:MaxCyclesExceedsFuel",
            image(&meta(0, 0, u64::MAX), &code),
        ),
        (
            "zk_mode_generic",
            "admission:zk",
            image(&meta(ivm_mode::ZK, 0, 64), &code),
        ),
        (
            "zk_and_vector_modes",
            "admission:ok",
            image(&meta(ivm_mode::ZK | ivm_mode::VECTOR, 8, 64), &code),
        ),
    ];
    for (name, tag, program) in cases {
        c.add(
            &format!("admission_{name}"),
            "",
            &[tag, "header:49-byte"],
            program,
            Vec::new(),
        );
    }
    // The same header under governance fuel below the node ceiling.
    let low_fuel = Policy {
        upper_bound: 1_000_000,
        fuel: 1_000,
    };
    for max_cycles in [1_000, 1_001, 5_000] {
        c.add(
            &format!("admission_low_fuel_max_cycles_{max_cycles}"),
            "",
            &["admission:MaxCyclesExceedsFuel", "gas:header-to-gas"],
            image(&meta(0, 0, max_cycles), &code),
            Vec::new(),
        )
        .policy = low_fuel;
    }
    let low_ceiling = Policy {
        upper_bound: 500,
        fuel: 55_000_000,
    };
    for max_cycles in [500, 501] {
        c.add(
            &format!("admission_low_ceiling_max_cycles_{max_cycles}"),
            "",
            &["admission:MaxCyclesExceedsUpperBound", "gas:header-to-gas"],
            image(&meta(0, 0, max_cycles), &code),
            Vec::new(),
        )
        .policy = low_ceiling;
    }
}

/// Every hand-assembled case.
pub fn build() -> Vec<Case> {
    let mut corpus = Corpus::default();
    arithmetic(&mut corpus);
    memory(&mut corpus);
    control(&mut corpus);
    system(&mut corpus);
    vector_ops(&mut corpus);
    crypto(&mut corpus);
    zk_ops(&mut corpus);
    budgets(&mut corpus);
    admission(&mut corpus);
    corpus.cases
}
