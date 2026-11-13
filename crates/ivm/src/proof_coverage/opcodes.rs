//! Admitted ABI V1 opcodes mapped to whole-invocation relation obligations.
//!
//! The table names every admitted opcode exactly once. Ordinary library
//! compilation fails when `wide::is_valid_opcode` and this table disagree, so a
//! newly admitted or retired opcode cannot bypass the proof-relation review.
//! The trap, helper and tag-surface columns restate the interpreter's fallible
//! and privacy-tag surface for each opcode; tests re-derive all three from
//! `crates/ivm/src/ivm.rs`.

use super::Obligation;
use crate::instruction::wide;

/// Coarse semantic family of an admitted opcode.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum OpcodeFamily {
    /// Scalar integer arithmetic, logic, comparison and bit operations.
    Arithmetic,
    /// Typed literals and guest memory loads and stores.
    Memory,
    /// Branches, jumps, protected calls, returns and halt.
    Control,
    /// Host calls.
    System,
}

impl OpcodeFamily {
    /// Stable machine-readable identifier.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Arithmetic => "arithmetic",
            Self::Memory => "memory",
            Self::Control => "control",
            Self::System => "system",
        }
    }
}

/// Architectural state one step of the opcode can change.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StepEffect {
    /// Scalar registers and their privacy tags.
    ScalarRegisters,
    /// Guest memory or literal tables together with scalar registers.
    MemoryAndRegisters,
    /// Program counter, protected call state and termination.
    Control,
    /// A host syscall selected by the instruction word.
    HostCall,
}

impl StepEffect {
    /// Stable machine-readable identifier.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::ScalarRegisters => "scalar_registers",
            Self::MemoryAndRegisters => "memory_and_registers",
            Self::Control => "control",
            Self::HostCall => "host_call",
        }
    }
}

/// Program-counter edge the relation must constrain for the opcode.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PcTransition {
    /// Advance by one four-byte instruction.
    Sequential,
    /// Signed 14-bit word offset when the predicate holds, else sequential.
    ConditionalRelative14,
    /// `JAL`: signed 19-bit word offset; writes `rd` unless it is `r0`, and a
    /// protected `rd = r1` also enters a child call.
    DirectRelative19WithOptionalLink,
    /// Signed 24-bit word offset.
    DirectRelative24,
    /// Signed 24-bit word offset, link in `r1` and a protected child call.
    DirectRelative24AndLink,
    /// `JALR`: masked register target; the protected canonical form returns
    /// and may halt at the outer sentinel.
    IndirectMaskedOrProtectedReturn,
    /// `HALT`: terminates raw code and traps inside a protected contract call.
    HaltOrStrictReturnTrap,
}

impl PcTransition {
    /// Stable machine-readable identifier.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Sequential => "sequential",
            Self::ConditionalRelative14 => "conditional_relative14",
            Self::DirectRelative19WithOptionalLink => "direct_relative19_with_optional_link",
            Self::DirectRelative24 => "direct_relative24",
            Self::DirectRelative24AndLink => "direct_relative24_and_link",
            Self::IndirectMaskedOrProtectedReturn => "indirect_masked_or_protected_return",
            Self::HaltOrStrictReturnTrap => "halt_or_strict_return_trap",
        }
    }
}

/// One admitted opcode and the relation obligations its semantics engage.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OpcodeEntry {
    /// Primary eight-bit opcode value.
    pub opcode: u8,
    /// Canonical constant name in `ivm_abi::instruction::wide`.
    pub name: &'static str,
    /// Constant module in `ivm_abi::instruction::wide`.
    pub module: &'static str,
    /// Semantic family.
    pub family: OpcodeFamily,
    /// Architectural state one step can change.
    pub effect: StepEffect,
    /// Program-counter edge.
    pub pc: PcTransition,
    /// Relation obligation classes the opcode's semantics engage.
    pub obligations: &'static [Obligation],
    /// `VMError` variants constructed directly in the opcode's interpreter arm.
    pub direct_traps: &'static [&'static str],
    /// Fallible helpers whose error the interpreter arm propagates with `?`.
    pub fallible_helpers: &'static [&'static str],
    /// Privacy-tag accessors ([`TAG_ACCESSORS`]) and privacy helpers
    /// ([`PRIVACY_HELPERS`]) the interpreter arm calls. An opcode engages
    /// [`Obligation::PrivateMasking`] exactly when this list is nonempty, the
    /// arm raises `PrivacyViolation` itself, or it dispatches a syscall.
    pub tag_surface: &'static [&'static str],
}

impl OpcodeEntry {
    /// Whether one step of the opcode reads or writes a privacy tag: its arm
    /// touches the tag surface or raises the privacy trap, or it dispatches a
    /// syscall, whose handler enforces the same surface for its registers.
    #[must_use]
    pub fn engages_privacy_tags(&self) -> bool {
        !self.tag_surface.is_empty()
            || self.direct_traps.contains(&"PrivacyViolation")
            || matches!(self.effect, StepEffect::HostCall)
    }
}

/// Register-tag accessors an interpreter arm can call directly: `tag` reads a
/// register tag and `set_tag` writes one.
pub const TAG_ACCESSORS: &[&str] = &["set_tag", "tag"];

/// Interpreter helpers that read, propagate or enforce privacy tags and that a
/// dispatch arm calls. Tests require this list to equal the functions of
/// `crates/ivm/src/ivm.rs` that touch a privacy tag and are called by an arm.
pub const PRIVACY_HELPERS: &[&str] = &[
    "memory_load_privacy_tag",
    "preflight_memory_store_privacy",
    "record_memory_store_privacy",
    "validate_memory_store_privacy",
    "zk_apply_tag",
    "zk_join_tags",
    "zk_require_public_trap_operands",
    "zk_unary_tag",
];

/// Functions through which syscall dispatch enforces the privacy-tag surface
/// for the selected syscall's input and output registers.
pub const SYSCALL_PRIVACY_FUNCTIONS: &[&str] = &[
    "validate_syscall_privacy",
    "sanitize_syscall_output_privacy",
    "finalize_syscall_output_privacy",
];

use Obligation::{
    Calls, Copyback, Faults, Fetch, Gas, HostResult, Initialization, MemoryOrdering, Padding,
    Pointers, PrivateMasking, TypedValues, VmRecursion,
};

/// Scalar register transition with tag propagation.
const SCALAR: &[Obligation] = &[Fetch, TypedValues, Gas, Faults, PrivateMasking];
/// Scalar 64-bit guest memory access.
const MEMORY: &[Obligation] = &[
    Fetch,
    TypedValues,
    Initialization,
    MemoryOrdering,
    Pointers,
    Gas,
    Faults,
    PrivateMasking,
];
/// `LDLIT`: typed pointer literal from the admitted literal table, written
/// with a public tag.
const POINTER_LITERAL: &[Obligation] = &[
    Fetch,
    TypedValues,
    Initialization,
    Pointers,
    Gas,
    Faults,
    PrivateMasking,
];
/// `LDI64`: scalar literal from the admitted literal table, written with a
/// public tag.
const SCALAR_LITERAL: &[Obligation] = &[
    Fetch,
    TypedValues,
    Initialization,
    Gas,
    Faults,
    PrivateMasking,
];
/// Conditional branch on public operands; a secret-tagged operand traps.
const BRANCH: &[Obligation] = &[Fetch, TypedValues, Gas, Faults, PrivateMasking];
/// Direct jump without link.
const JUMP: &[Obligation] = &[Fetch, Gas, Faults];
/// Direct jump with link; the protected form enters a child frame. The link
/// register is written with a public tag.
const CALL: &[Obligation] = &[
    Fetch,
    TypedValues,
    Initialization,
    Calls,
    VmRecursion,
    Gas,
    Faults,
    PrivateMasking,
];
/// `JALR`: the protected canonical form returns, copies back and may halt. A
/// secret-tagged target traps and the link register is written public.
const RETURN: &[Obligation] = &[
    Fetch,
    TypedValues,
    Initialization,
    Calls,
    Copyback,
    VmRecursion,
    Padding,
    Gas,
    Faults,
    PrivateMasking,
];
/// `HALT`: raw termination or a protected-call trap.
const HALT: &[Obligation] = &[Fetch, Padding, Gas, Faults];
/// `SCALL`: dispatch to the syscall relation of the selected 24-bit number.
const HOST_CALL: &[Obligation] = &[
    Fetch,
    TypedValues,
    Pointers,
    HostResult,
    Gas,
    Faults,
    PrivateMasking,
];

macro_rules! op {
    (
        $module:ident :: $name:ident,
        $family:ident,
        $effect:ident,
        $pc:ident,
        $profile:ident,
        [$($trap:literal),* $(,)?],
        [$($helper:literal),* $(,)?],
        [$($tag:literal),* $(,)?] $(,)?
    ) => {
        OpcodeEntry {
            opcode: wide::$module::$name,
            name: stringify!($name),
            module: stringify!($module),
            family: OpcodeFamily::$family,
            effect: StepEffect::$effect,
            pc: PcTransition::$pc,
            obligations: $profile,
            direct_traps: &[$($trap),*],
            fallible_helpers: &[$($helper),*],
            tag_surface: &[$($tag),*],
        }
    };
}

/// Number of admitted ABI V1 opcodes.
pub const OPCODE_COUNT: usize = 58;

/// Every admitted ABI V1 opcode in ascending opcode order.
pub const OPCODES: &[OpcodeEntry; OPCODE_COUNT] = &[
    op!(
        arithmetic::ADD,
        Arithmetic,
        ScalarRegisters,
        Sequential,
        SCALAR,
        [],
        [],
        ["zk_apply_tag", "zk_join_tags"],
    ),
    op!(
        arithmetic::SUB,
        Arithmetic,
        ScalarRegisters,
        Sequential,
        SCALAR,
        [],
        [],
        ["zk_apply_tag", "zk_join_tags"],
    ),
    op!(
        arithmetic::AND,
        Arithmetic,
        ScalarRegisters,
        Sequential,
        SCALAR,
        [],
        [],
        ["zk_apply_tag", "zk_join_tags"],
    ),
    op!(
        arithmetic::OR,
        Arithmetic,
        ScalarRegisters,
        Sequential,
        SCALAR,
        [],
        [],
        ["zk_apply_tag", "zk_join_tags"],
    ),
    op!(
        arithmetic::XOR,
        Arithmetic,
        ScalarRegisters,
        Sequential,
        SCALAR,
        [],
        [],
        ["zk_apply_tag", "zk_join_tags"],
    ),
    op!(
        arithmetic::SLL,
        Arithmetic,
        ScalarRegisters,
        Sequential,
        SCALAR,
        [],
        [],
        ["zk_apply_tag", "zk_join_tags"],
    ),
    op!(
        arithmetic::SRL,
        Arithmetic,
        ScalarRegisters,
        Sequential,
        SCALAR,
        [],
        [],
        ["zk_apply_tag", "zk_join_tags"],
    ),
    op!(
        arithmetic::SRA,
        Arithmetic,
        ScalarRegisters,
        Sequential,
        SCALAR,
        [],
        [],
        ["zk_apply_tag", "zk_join_tags"],
    ),
    op!(
        arithmetic::SLT,
        Arithmetic,
        ScalarRegisters,
        Sequential,
        SCALAR,
        [],
        [],
        ["zk_apply_tag", "zk_join_tags"],
    ),
    op!(
        arithmetic::SLTU,
        Arithmetic,
        ScalarRegisters,
        Sequential,
        SCALAR,
        [],
        [],
        ["zk_apply_tag", "zk_join_tags"],
    ),
    op!(
        arithmetic::CMOV,
        Arithmetic,
        ScalarRegisters,
        Sequential,
        SCALAR,
        ["PrivacyViolation"],
        [],
        ["tag", "zk_apply_tag", "zk_unary_tag"],
    ),
    op!(
        arithmetic::NOT,
        Arithmetic,
        ScalarRegisters,
        Sequential,
        SCALAR,
        [],
        [],
        ["zk_apply_tag", "zk_unary_tag"],
    ),
    op!(
        arithmetic::NEG,
        Arithmetic,
        ScalarRegisters,
        Sequential,
        SCALAR,
        [],
        [],
        ["zk_apply_tag", "zk_unary_tag"],
    ),
    op!(
        arithmetic::SEQ,
        Arithmetic,
        ScalarRegisters,
        Sequential,
        SCALAR,
        [],
        [],
        ["zk_apply_tag", "zk_join_tags"],
    ),
    op!(
        arithmetic::SNE,
        Arithmetic,
        ScalarRegisters,
        Sequential,
        SCALAR,
        [],
        [],
        ["zk_apply_tag", "zk_join_tags"],
    ),
    op!(
        arithmetic::MUL,
        Arithmetic,
        ScalarRegisters,
        Sequential,
        SCALAR,
        [],
        [],
        ["zk_apply_tag", "zk_join_tags"],
    ),
    op!(
        arithmetic::MULH,
        Arithmetic,
        ScalarRegisters,
        Sequential,
        SCALAR,
        [],
        [],
        ["zk_apply_tag", "zk_join_tags"],
    ),
    op!(
        arithmetic::MULHU,
        Arithmetic,
        ScalarRegisters,
        Sequential,
        SCALAR,
        [],
        [],
        ["zk_apply_tag", "zk_join_tags"],
    ),
    op!(
        arithmetic::MULHSU,
        Arithmetic,
        ScalarRegisters,
        Sequential,
        SCALAR,
        [],
        [],
        ["zk_apply_tag", "zk_join_tags"],
    ),
    op!(
        arithmetic::DIV,
        Arithmetic,
        ScalarRegisters,
        Sequential,
        SCALAR,
        [],
        ["checked_div_i64", "zk_require_public_trap_operands"],
        [
            "zk_apply_tag",
            "zk_join_tags",
            "zk_require_public_trap_operands"
        ],
    ),
    op!(
        arithmetic::DIVU,
        Arithmetic,
        ScalarRegisters,
        Sequential,
        SCALAR,
        [],
        ["checked_div_u64", "zk_require_public_trap_operands"],
        [
            "zk_apply_tag",
            "zk_join_tags",
            "zk_require_public_trap_operands"
        ],
    ),
    op!(
        arithmetic::REM,
        Arithmetic,
        ScalarRegisters,
        Sequential,
        SCALAR,
        [],
        ["checked_rem_i64", "zk_require_public_trap_operands"],
        [
            "zk_apply_tag",
            "zk_join_tags",
            "zk_require_public_trap_operands"
        ],
    ),
    op!(
        arithmetic::REMU,
        Arithmetic,
        ScalarRegisters,
        Sequential,
        SCALAR,
        [],
        ["checked_rem_u64", "zk_require_public_trap_operands"],
        [
            "zk_apply_tag",
            "zk_join_tags",
            "zk_require_public_trap_operands"
        ],
    ),
    op!(
        arithmetic::ROTL,
        Arithmetic,
        ScalarRegisters,
        Sequential,
        SCALAR,
        [],
        [],
        ["zk_apply_tag", "zk_join_tags"],
    ),
    op!(
        arithmetic::ROTR,
        Arithmetic,
        ScalarRegisters,
        Sequential,
        SCALAR,
        [],
        [],
        ["zk_apply_tag", "zk_join_tags"],
    ),
    op!(
        arithmetic::POPCNT,
        Arithmetic,
        ScalarRegisters,
        Sequential,
        SCALAR,
        [],
        [],
        ["zk_apply_tag", "zk_unary_tag"],
    ),
    op!(
        arithmetic::CLZ,
        Arithmetic,
        ScalarRegisters,
        Sequential,
        SCALAR,
        [],
        [],
        ["zk_apply_tag", "zk_unary_tag"],
    ),
    op!(
        arithmetic::CTZ,
        Arithmetic,
        ScalarRegisters,
        Sequential,
        SCALAR,
        [],
        [],
        ["zk_apply_tag", "zk_unary_tag"],
    ),
    op!(
        arithmetic::ISQRT,
        Arithmetic,
        ScalarRegisters,
        Sequential,
        SCALAR,
        [],
        [],
        ["zk_apply_tag", "zk_unary_tag"],
    ),
    op!(
        arithmetic::MIN,
        Arithmetic,
        ScalarRegisters,
        Sequential,
        SCALAR,
        [],
        [],
        ["zk_apply_tag", "zk_join_tags"],
    ),
    op!(
        arithmetic::MAX,
        Arithmetic,
        ScalarRegisters,
        Sequential,
        SCALAR,
        [],
        [],
        ["zk_apply_tag", "zk_join_tags"],
    ),
    op!(
        arithmetic::ADDI,
        Arithmetic,
        ScalarRegisters,
        Sequential,
        SCALAR,
        [],
        [],
        ["zk_apply_tag", "zk_unary_tag"],
    ),
    op!(
        arithmetic::ANDI,
        Arithmetic,
        ScalarRegisters,
        Sequential,
        SCALAR,
        [],
        [],
        ["zk_apply_tag", "zk_unary_tag"],
    ),
    op!(
        arithmetic::ORI,
        Arithmetic,
        ScalarRegisters,
        Sequential,
        SCALAR,
        [],
        [],
        ["zk_apply_tag", "zk_unary_tag"],
    ),
    op!(
        arithmetic::XORI,
        Arithmetic,
        ScalarRegisters,
        Sequential,
        SCALAR,
        [],
        [],
        ["zk_apply_tag", "zk_unary_tag"],
    ),
    op!(
        arithmetic::CMOVI,
        Arithmetic,
        ScalarRegisters,
        Sequential,
        SCALAR,
        ["PrivacyViolation"],
        [],
        ["set_tag", "tag"],
    ),
    op!(
        arithmetic::ROTL_IMM,
        Arithmetic,
        ScalarRegisters,
        Sequential,
        SCALAR,
        [],
        [],
        ["zk_apply_tag", "zk_unary_tag"],
    ),
    op!(
        arithmetic::ROTR_IMM,
        Arithmetic,
        ScalarRegisters,
        Sequential,
        SCALAR,
        [],
        [],
        ["zk_apply_tag", "zk_unary_tag"],
    ),
    op!(
        arithmetic::ABS,
        Arithmetic,
        ScalarRegisters,
        Sequential,
        SCALAR,
        [],
        ["checked_abs_i64", "zk_require_public_trap_operands"],
        [
            "zk_apply_tag",
            "zk_require_public_trap_operands",
            "zk_unary_tag"
        ],
    ),
    op!(
        arithmetic::DIV_CEIL,
        Arithmetic,
        ScalarRegisters,
        Sequential,
        SCALAR,
        [],
        ["div_ceil_i64", "zk_require_public_trap_operands"],
        [
            "zk_apply_tag",
            "zk_join_tags",
            "zk_require_public_trap_operands"
        ],
    ),
    op!(
        arithmetic::GCD,
        Arithmetic,
        ScalarRegisters,
        Sequential,
        SCALAR,
        [],
        [],
        ["zk_apply_tag", "zk_join_tags"],
    ),
    op!(
        arithmetic::MEAN,
        Arithmetic,
        ScalarRegisters,
        Sequential,
        SCALAR,
        [],
        [],
        ["zk_apply_tag", "zk_join_tags"],
    ),
    op!(
        memory::LOAD64,
        Memory,
        MemoryAndRegisters,
        Sequential,
        MEMORY,
        ["PrivacyViolation"],
        ["memory.load_u64", "memory_load_privacy_tag"],
        ["memory_load_privacy_tag", "set_tag", "tag"],
    ),
    op!(
        memory::STORE64,
        Memory,
        MemoryAndRegisters,
        Sequential,
        MEMORY,
        ["PrivacyViolation"],
        [
            "memory.store_u64",
            "preflight_memory_store_privacy",
            "validate_memory_store_privacy"
        ],
        [
            "preflight_memory_store_privacy",
            "record_memory_store_privacy",
            "tag",
            "validate_memory_store_privacy"
        ],
    ),
    op!(
        memory::LDLIT,
        Memory,
        MemoryAndRegisters,
        Sequential,
        POINTER_LITERAL,
        ["InvalidMetadata"],
        [],
        ["set_tag"],
    ),
    op!(
        memory::LDI64,
        Memory,
        MemoryAndRegisters,
        Sequential,
        SCALAR_LITERAL,
        ["InvalidMetadata"],
        [],
        ["set_tag"],
    ),
    op!(
        control::BEQ,
        Control,
        Control,
        ConditionalRelative14,
        BRANCH,
        ["PrivacyViolation"],
        [],
        ["tag"],
    ),
    op!(
        control::BNE,
        Control,
        Control,
        ConditionalRelative14,
        BRANCH,
        ["PrivacyViolation"],
        [],
        ["tag"],
    ),
    op!(
        control::BLT,
        Control,
        Control,
        ConditionalRelative14,
        BRANCH,
        ["PrivacyViolation"],
        [],
        ["tag"],
    ),
    op!(
        control::BGE,
        Control,
        Control,
        ConditionalRelative14,
        BRANCH,
        ["PrivacyViolation"],
        [],
        ["tag"],
    ),
    op!(
        control::BLTU,
        Control,
        Control,
        ConditionalRelative14,
        BRANCH,
        ["PrivacyViolation"],
        [],
        ["tag"],
    ),
    op!(
        control::BGEU,
        Control,
        Control,
        ConditionalRelative14,
        BRANCH,
        ["PrivacyViolation"],
        [],
        ["tag"],
    ),
    op!(
        control::JAL,
        Control,
        Control,
        DirectRelative19WithOptionalLink,
        CALL,
        ["ControlFlowViolation"],
        ["begin_child_call"],
        ["set_tag"],
    ),
    op!(
        control::JALR,
        Control,
        Control,
        IndirectMaskedOrProtectedReturn,
        RETURN,
        ["ControlFlowViolation", "PrivacyViolation"],
        ["finish_call"],
        ["set_tag", "tag"],
    ),
    op!(
        control::HALT,
        Control,
        Control,
        HaltOrStrictReturnTrap,
        HALT,
        ["ControlFlowViolation"],
        [],
        [],
    ),
    op!(
        control::JMP,
        Control,
        Control,
        DirectRelative24,
        JUMP,
        [],
        [],
        [],
    ),
    op!(
        control::JALS,
        Control,
        Control,
        DirectRelative24AndLink,
        CALL,
        [],
        ["begin_child_call"],
        ["set_tag"],
    ),
    op!(
        system::SCALL,
        System,
        HostCall,
        Sequential,
        HOST_CALL,
        ["UnknownSyscall"],
        ["execute_syscall_with_register_log"],
        [],
    ),
];

/// Return the inventory entry of one admitted opcode.
#[must_use]
pub const fn opcode_entry(opcode: u8) -> Option<&'static OpcodeEntry> {
    let mut index = 0;
    while index < OPCODES.len() {
        if OPCODES[index].opcode == opcode {
            return Some(&OPCODES[index]);
        }
        index += 1;
    }
    None
}

// Checked during ordinary library compilation, not only in tests.
// `is_valid_opcode` is the prepared/runtime admission gate: a newly admitted
// value cannot compile until it is inventoried, and a retired value cannot
// silently remain in the inventory.
const _: () = {
    let mut index = 1;
    while index < OPCODES.len() {
        assert!(OPCODES[index - 1].opcode < OPCODES[index].opcode);
        index += 1;
    }
    let mut candidate = 0_u16;
    while candidate < 256 {
        let opcode = candidate as u8;
        assert!(wide::is_valid_opcode(opcode) == opcode_entry(opcode).is_some());
        candidate += 1;
    }
};
