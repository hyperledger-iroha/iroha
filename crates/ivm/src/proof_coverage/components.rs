//! Unregistered proof components that currently hold IVM relation equations.
//!
//! Each component is listed with its reviewed opcode acceptance, its true
//! restrictions and its fixed geometry. `referenced_opcodes` is only a drift
//! guard: tests require it to equal the opcode constants named by the
//! component's non-test sources, so a changed proof file forces this review.
//! A referenced constant is not a coverage claim, and none of these components
//! is a registered verifier or an invocation proof.

use super::Obligation;
use crate::{VmTrapKind, instruction::wide};

/// One admitted opcode a component's acceptance predicate and equations cover.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ComponentOpcode {
    /// Primary opcode value.
    pub opcode: u8,
    /// Operand or context restriction of the covered form, when narrower than
    /// the opcode's full semantics.
    pub restriction: Option<&'static str>,
}

/// One fixed geometry constant of a component and where it is declared.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ComponentGeometry {
    /// Stable machine-readable name.
    pub name: &'static str,
    /// Declared value.
    pub value: u64,
    /// Repository-relative file declaring the constant.
    pub path: &'static str,
    /// Constant identifier in that file.
    pub constant: &'static str,
}

/// One unregistered proof component and its true coverage.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProofComponent {
    /// Stable machine-readable identifier.
    pub id: &'static str,
    /// What the component's equations constrain.
    pub summary: &'static str,
    /// Repository-relative non-test source files owned by the component.
    pub sources: &'static [&'static str],
    /// Reviewed opcode acceptance.
    pub opcodes: &'static [ComponentOpcode],
    /// Opcode constants named by [`Self::sources`]; a drift guard only.
    pub referenced_opcodes: &'static [&'static str],
    /// Trap outcomes for which the component holds a relation.
    pub traps: &'static [VmTrapKind],
    /// Unsupported scope that keeps the component from being an invocation proof.
    pub restrictions: &'static [&'static str],
    /// Obligation classes for which the component holds component-level equations.
    pub substrate: &'static [Obligation],
    /// Fixed geometry.
    pub geometry: &'static [ComponentGeometry],
    /// Whether a production verifier registers the component.
    pub registered: bool,
}

macro_rules! cov {
    ($module:ident :: $name:ident) => {
        ComponentOpcode {
            opcode: wide::$module::$name,
            restriction: None,
        }
    };
    ($module:ident :: $name:ident, $restriction:literal) => {
        ComponentOpcode {
            opcode: wide::$module::$name,
            restriction: Some($restriction),
        }
    };
}

const fn geometry(
    name: &'static str,
    value: u64,
    path: &'static str,
    constant: &'static str,
) -> ComponentGeometry {
    ComponentGeometry {
        name,
        value,
        path,
        constant,
    }
}

/// Every unregistered proof component, in dependency order.
// TODO: Replace these substrates with the single complete IVM AIR and its
// registered verifier (task M.2); until then no entry may claim completion.
pub const COMPONENTS: &[ProofComponent] = &[
    ProofComponent {
        id: "public_alu_step",
        summary: "Public single-step STARK adapter for wrapping add, subtract and bitwise operations with register or signed eight-bit immediate operands.",
        sources: &[
            "crates/iroha_core_privacy/src/execution_proofs/ivm_step_air.rs",
            "crates/iroha_core_privacy/src/execution_proofs/ivm_step_air/alu.rs",
            "crates/iroha_core_privacy/src/execution_proofs/ivm_step_air/bitwise.rs",
            "crates/iroha_core_privacy/src/execution_proofs/ivm_step_air/residues.rs",
            "crates/iroha_core_privacy/src/execution_proofs/ivm_step_air/word.rs",
        ],
        opcodes: &[
            cov!(arithmetic::ADD),
            cov!(arithmetic::SUB),
            cov!(arithmetic::AND),
            cov!(arithmetic::OR),
            cov!(arithmetic::XOR),
            cov!(arithmetic::ADDI),
            cov!(arithmetic::ANDI),
            cov!(arithmetic::ORI),
            cov!(arithmetic::XORI),
        ],
        referenced_opcodes: &[
            "ADD", "ADDI", "AND", "ANDI", "OR", "ORI", "SUB", "XOR", "XORI",
        ],
        traps: &[],
        restrictions: &[
            "one public step; operands, result and tags are public inputs and secret-tagged operands are rejected",
            "no authenticated fetch, other registers, memory, host effects or invocation continuity",
            "unit base gas, pc + 4 and one cycle only",
        ],
        substrate: &[Obligation::TypedValues, Obligation::Gas],
        geometry: &[geometry(
            "trace_log2",
            13,
            "crates/iroha_core_privacy/src/execution_proofs/ivm_step_air.rs",
            "TRACE_LOG2",
        )],
        registered: false,
    },
    ProofComponent {
        id: "public_branch_step",
        summary: "Public single-step STARK adapter for the six conditional branches.",
        sources: &["crates/iroha_core_privacy/src/execution_proofs/ivm_step_air/branch.rs"],
        opcodes: &[
            cov!(control::BEQ),
            cov!(control::BNE),
            cov!(control::BLT),
            cov!(control::BGE),
            cov!(control::BLTU),
            cov!(control::BGEU),
        ],
        referenced_opcodes: &["BEQ", "BGE", "BGEU", "BLT", "BLTU", "BNE"],
        traps: &[],
        restrictions: &[
            "one public step; both operands and their tags are public inputs",
            "no authenticated fetch, other registers, instruction boundaries or invocation continuity",
        ],
        substrate: &[Obligation::TypedValues, Obligation::Gas],
        geometry: &[],
        registered: false,
    },
    ProofComponent {
        id: "public_shift_step",
        summary: "Public single-step STARK adapter for the seven shift and rotate opcodes with six-bit amount masking.",
        sources: &["crates/iroha_core_privacy/src/execution_proofs/ivm_step_air/shift.rs"],
        opcodes: &[
            cov!(arithmetic::SLL),
            cov!(arithmetic::SRL),
            cov!(arithmetic::SRA),
            cov!(arithmetic::ROTL),
            cov!(arithmetic::ROTR),
            cov!(arithmetic::ROTL_IMM),
            cov!(arithmetic::ROTR_IMM),
        ],
        referenced_opcodes: &["ROTL", "ROTL_IMM", "ROTR", "ROTR_IMM", "SLL", "SRA", "SRL"],
        traps: &[],
        restrictions: &[
            "one public step; operands, amount and tags are public inputs",
            "no authenticated fetch, other registers, memory, private values or invocation continuity",
        ],
        substrate: &[Obligation::TypedValues, Obligation::Gas],
        geometry: &[],
        registered: false,
    },
    ProofComponent {
        id: "public_scalar_segment",
        summary: "Bounded public scalar segment with canonical artifact fetch, all 256 registers, gas, cycles and a typed final-attempt outcome.",
        sources: &[
            "crates/iroha_core_privacy/src/execution_proofs/ivm_step_air/trace.rs",
            "crates/iroha_core_privacy/src/execution_proofs/ivm_step_air/trace/absolute.rs",
            "crates/iroha_core_privacy/src/execution_proofs/ivm_step_air/trace/bit_count.rs",
            "crates/iroha_core_privacy/src/execution_proofs/ivm_step_air/trace/ceiling.rs",
            "crates/iroha_core_privacy/src/execution_proofs/ivm_step_air/trace/division.rs",
            "crates/iroha_core_privacy/src/execution_proofs/ivm_step_air/trace/gcd.rs",
            "crates/iroha_core_privacy/src/execution_proofs/ivm_step_air/trace/mean.rs",
            "crates/iroha_core_privacy/src/execution_proofs/ivm_step_air/trace/multiply.rs",
            "crates/iroha_core_privacy/src/execution_proofs/ivm_step_air/trace/square_root.rs",
        ],
        opcodes: &[
            cov!(arithmetic::ADD),
            cov!(arithmetic::SUB),
            cov!(arithmetic::AND),
            cov!(arithmetic::OR),
            cov!(arithmetic::XOR),
            cov!(arithmetic::SLL),
            cov!(arithmetic::SRL),
            cov!(arithmetic::SRA),
            cov!(arithmetic::SLT),
            cov!(arithmetic::SLTU),
            cov!(arithmetic::CMOV),
            cov!(arithmetic::NOT),
            cov!(arithmetic::NEG),
            cov!(arithmetic::SEQ),
            cov!(arithmetic::SNE),
            cov!(arithmetic::MUL),
            cov!(arithmetic::MULH),
            cov!(arithmetic::MULHU),
            cov!(arithmetic::MULHSU),
            cov!(arithmetic::DIV),
            cov!(arithmetic::DIVU),
            cov!(arithmetic::REM),
            cov!(arithmetic::REMU),
            cov!(arithmetic::ROTL),
            cov!(arithmetic::ROTR),
            cov!(arithmetic::POPCNT),
            cov!(arithmetic::CLZ),
            cov!(arithmetic::CTZ),
            cov!(arithmetic::ISQRT),
            cov!(arithmetic::MIN),
            cov!(arithmetic::MAX),
            cov!(arithmetic::ADDI),
            cov!(arithmetic::ANDI),
            cov!(arithmetic::ORI),
            cov!(arithmetic::XORI),
            cov!(arithmetic::CMOVI),
            cov!(arithmetic::ROTL_IMM),
            cov!(arithmetic::ROTR_IMM),
            cov!(arithmetic::ABS),
            cov!(arithmetic::DIV_CEIL),
            cov!(arithmetic::GCD),
            cov!(arithmetic::MEAN),
            cov!(control::BEQ),
            cov!(control::BNE),
            cov!(control::BLT),
            cov!(control::BGE),
            cov!(control::BLTU),
            cov!(control::BGEU),
            cov!(control::JAL, "rd = r0 only; linked calls are excluded"),
            cov!(control::JMP),
            cov!(system::GETGAS),
        ],
        referenced_opcodes: &[
            "ABS", "ADD", "AND", "BEQ", "BGE", "BGEU", "BLT", "BLTU", "BNE", "CLZ", "CMOV",
            "CMOVI", "CTZ", "DIV", "DIVU", "DIV_CEIL", "GCD", "GETGAS", "ISQRT", "JAL", "JMP",
            "MAX", "MEAN", "MIN", "MUL", "MULH", "MULHSU", "MULHU", "NEG", "NOT", "OR", "POPCNT",
            "REM", "REMU", "ROTL", "ROTL_IMM", "ROTR", "ROTR_IMM", "SEQ", "SLL", "SLT", "SLTU",
            "SNE", "SRA", "SRL", "SUB", "XOR",
        ],
        traps: &[VmTrapKind::AssertionFailed, VmTrapKind::OutOfGas],
        restrictions: &[
            "1..=64 attempted steps over code of at most 64 words between explicit public boundaries",
            "every register tag is public and zero; private values are outside the relation",
            "no call entry or return, memory, host effects, statement, finality or invocation completion",
            "trap outcomes cover only the final attempted step: out of gas and arithmetic assertion",
            "artifact cycle limit only; host cycle overrides are outside the relation",
            "the interpreter step recorder supplies untrusted witness material, never authority",
        ],
        substrate: &[
            Obligation::Fetch,
            Obligation::TypedValues,
            Obligation::Gas,
            Obligation::Faults,
        ],
        geometry: &[
            geometry(
                "max_steps",
                64,
                "crates/iroha_core_privacy/src/execution_proofs/ivm_step_air/trace.rs",
                "MAX_STEPS",
            ),
            geometry(
                "max_code_words",
                64,
                "crates/iroha_core_privacy/src/execution_proofs/ivm_step_air/trace.rs",
                "MAX_WORDS",
            ),
            geometry(
                "registers",
                256,
                "crates/iroha_core_privacy/src/execution_proofs/ivm_step_air/trace.rs",
                "REGISTERS",
            ),
        ],
        registered: false,
    },
    ProofComponent {
        id: "machine_bus",
        summary: "Typed state-packet permutation, sorted-state continuity and private producer-to-history equations.",
        sources: &[
            "crates/iroha_core_privacy/src/execution_proofs/ivm_step_air/machine_bus.rs",
            "crates/iroha_core_privacy/src/execution_proofs/ivm_step_air/machine_bus/packet.rs",
            "crates/iroha_core_privacy/src/execution_proofs/ivm_step_air/machine_bus/permutation.rs",
            "crates/iroha_core_privacy/src/execution_proofs/ivm_step_air/machine_bus/private_history.rs",
            "crates/iroha_core_privacy/src/execution_proofs/ivm_step_air/machine_bus/private_history/view.rs",
            "crates/iroha_core_privacy/src/execution_proofs/ivm_step_air/machine_bus/sorted.rs",
        ],
        opcodes: &[],
        referenced_opcodes: &["BLTU"],
        traps: &[],
        restrictions: &[
            "packet multiset equality, typed cells, read preservation and address/time order only",
            "no initialization authority, memory request, frame ownership or register semantics",
            "the history view borrows at most two segments of one bounded window; no joint masked invocation transcript binds the segment commitments",
            "the unsigned less-than comparison bank is referenced for sorted order, not as a branch relation",
        ],
        substrate: &[
            Obligation::MemoryOrdering,
            Obligation::Continuation,
            Obligation::PrivateMasking,
        ],
        geometry: &[
            geometry(
                "packet_width",
                26,
                "crates/iroha_core_privacy/src/execution_proofs/ivm_step_air/machine_bus/packet.rs",
                "WIDTH",
            ),
            geometry(
                "max_history_segments",
                2,
                "crates/iroha_core_privacy/src/execution_proofs/ivm_step_air/machine_bus/private_history.rs",
                "MAX_SEGMENTS",
            ),
        ],
        registered: false,
    },
    ProofComponent {
        id: "private_dispatch",
        summary: "Private canonical fetch and original call, memory, scalar, branch and control producers over one packet array.",
        sources: &[
            "crates/iroha_core_privacy/src/execution_proofs/ivm_step_air/machine_bus/private_dispatch.rs",
            "crates/iroha_core_privacy/src/execution_proofs/ivm_step_air/machine_bus/private_dispatch/code_words.rs",
            "crates/iroha_core_privacy/src/execution_proofs/ivm_step_air/machine_bus/private_dispatch/scalar.rs",
            "crates/iroha_core_privacy/src/execution_proofs/ivm_step_air/machine_bus/private_dispatch/scalar/gcd.rs",
        ],
        opcodes: &[
            cov!(arithmetic::ADD),
            cov!(arithmetic::SUB),
            cov!(arithmetic::AND),
            cov!(arithmetic::OR),
            cov!(arithmetic::XOR),
            cov!(arithmetic::SLL),
            cov!(arithmetic::SRL),
            cov!(arithmetic::SRA),
            cov!(arithmetic::SLT),
            cov!(arithmetic::SLTU),
            cov!(arithmetic::CMOV),
            cov!(arithmetic::NOT),
            cov!(arithmetic::NEG),
            cov!(arithmetic::SEQ),
            cov!(arithmetic::SNE),
            cov!(arithmetic::MUL),
            cov!(arithmetic::MULH),
            cov!(arithmetic::MULHU),
            cov!(arithmetic::MULHSU),
            cov!(
                arithmetic::DIV,
                "successful step with public trap-sensitive operands; the trap outcome is rejected, not proven"
            ),
            cov!(
                arithmetic::DIVU,
                "successful step with public trap-sensitive operands; the trap outcome is rejected, not proven"
            ),
            cov!(
                arithmetic::REM,
                "successful step with public trap-sensitive operands; the trap outcome is rejected, not proven"
            ),
            cov!(
                arithmetic::REMU,
                "successful step with public trap-sensitive operands; the trap outcome is rejected, not proven"
            ),
            cov!(arithmetic::ROTL),
            cov!(arithmetic::ROTR),
            cov!(arithmetic::POPCNT),
            cov!(arithmetic::CLZ),
            cov!(arithmetic::CTZ),
            cov!(arithmetic::ISQRT),
            cov!(arithmetic::MIN),
            cov!(arithmetic::MAX),
            cov!(arithmetic::ADDI),
            cov!(arithmetic::ANDI),
            cov!(arithmetic::ORI),
            cov!(arithmetic::XORI),
            cov!(arithmetic::CMOVI),
            cov!(arithmetic::ROTL_IMM),
            cov!(arithmetic::ROTR_IMM),
            cov!(
                arithmetic::ABS,
                "successful step with public trap-sensitive operands; the trap outcome is rejected, not proven"
            ),
            cov!(
                arithmetic::DIV_CEIL,
                "successful step with public trap-sensitive operands; the trap outcome is rejected, not proven"
            ),
            cov!(arithmetic::GCD),
            cov!(arithmetic::MEAN),
            cov!(
                memory::LOAD64,
                "successful step composed with the private memory banks"
            ),
            cov!(
                memory::STORE64,
                "successful step composed with the private memory banks"
            ),
            cov!(memory::LDI64, "admitted scalar literal"),
            cov!(control::BEQ),
            cov!(control::BNE),
            cov!(control::BLT),
            cov!(control::BGE),
            cov!(control::BLTU),
            cov!(control::BGEU),
            cov!(
                control::JAL,
                "rd = r0 jump or rd = r1 protected child entry"
            ),
            cov!(
                control::JALR,
                "canonical protected return JALR r0, r1, 0 only"
            ),
            cov!(control::JMP),
            cov!(control::JALS),
            cov!(system::GETGAS),
        ],
        referenced_opcodes: &[
            "ABS", "ADD", "AND", "BEQ", "BGE", "BGEU", "BLT", "BLTU", "BNE", "CLZ", "CMOV",
            "CMOVI", "CTZ", "DIV", "DIVU", "DIV_CEIL", "GCD", "GETGAS", "ISQRT", "JAL", "JALR",
            "JALS", "JMP", "LDI64", "LOAD64", "MAX", "MEAN", "MIN", "MUL", "MULH", "MULHSU",
            "MULHU", "NEG", "NOT", "OR", "POPCNT", "REM", "REMU", "ROTL", "ROTL_IMM", "ROTR",
            "ROTR_IMM", "SEQ", "SLL", "SLT", "SLTU", "SNE", "SRA", "SRL", "STORE64", "SUB", "XOR",
        ],
        traps: &[],
        restrictions: &[
            "successful steps only: trap results are rejected rather than proven",
            "private fetch over at most 64 code words in a fixed qualification geometry",
            "callable depth is constrained to 0..=1024 with exact push and pop; the nested-contract host limit is outside the relation",
            "no invocation initialization, terminal publication, statement or finalized-State binding",
            "no production adapter or verifier registration",
        ],
        substrate: &[
            Obligation::Fetch,
            Obligation::TypedValues,
            Obligation::Gas,
            Obligation::Calls,
            Obligation::VmRecursion,
        ],
        geometry: &[
            geometry(
                "max_code_words",
                64,
                "crates/iroha_core_privacy/src/execution_proofs/ivm_step_air/machine_bus/private_dispatch.rs",
                "MAX_WORDS",
            ),
            geometry(
                "dispatcher_ports",
                21,
                "crates/iroha_core_privacy/src/execution_proofs/ivm_step_air/machine_bus/private_dispatch.rs",
                "PORTS",
            ),
        ],
        registered: false,
    },
    ProofComponent {
        id: "private_call_frames",
        summary: "Artifact-owned callable selection, CALL descriptor and frame-work debit, frame lifecycle and the return initialization scan with parent copyback.",
        sources: &[
            "crates/iroha_core_privacy/src/execution_proofs/ivm_step_air/machine_bus/callable_lookup.rs",
            "crates/iroha_core_privacy/src/execution_proofs/ivm_step_air/machine_bus/callable_lookup/native_root.rs",
            "crates/iroha_core_privacy/src/execution_proofs/ivm_step_air/machine_bus/callable_lookup/scan_storage.rs",
            "crates/iroha_core_privacy/src/execution_proofs/ivm_step_air/machine_bus/frame_descriptor.rs",
            "crates/iroha_core_privacy/src/execution_proofs/ivm_step_air/machine_bus/frame_lifecycle.rs",
            "crates/iroha_core_privacy/src/execution_proofs/ivm_step_air/machine_bus/private_dispatch/call_descriptor.rs",
            "crates/iroha_core_privacy/src/execution_proofs/ivm_step_air/machine_bus/private_dispatch/call_descriptor/scalar_arguments.rs",
            "crates/iroha_core_privacy/src/execution_proofs/ivm_step_air/machine_bus/return_copyback.rs",
            "crates/iroha_core_privacy/src/execution_proofs/ivm_step_air/machine_bus/return_copyback/native_witness.rs",
        ],
        opcodes: &[
            cov!(control::JAL, "rd = r1 protected child entry"),
            cov!(control::JALS),
        ],
        referenced_opcodes: &["JAL", "JALS"],
        traps: &[],
        restrictions: &[
            "successful CALL descriptor lookup, repeated table reads and frame-work debit only",
            "initialized scalar arguments for flat Unit, Bool and exact nominal Error forests; recursive nodes refuse",
            "return operands and all 4,097 initialization and copyback cells of one bounded window, driven by the dispatcher return role",
            "no root entry authority, pointer roles, allocation preflight, failed-call effects or typed traversal",
        ],
        substrate: &[
            Obligation::Initialization,
            Obligation::Calls,
            Obligation::Copyback,
            Obligation::Gas,
            Obligation::VmRecursion,
        ],
        geometry: &[geometry(
            "return_cells",
            4097,
            "crates/iroha_core_privacy/src/execution_proofs/ivm_step_air/machine_bus/return_copyback.rs",
            "CELLS",
        )],
        registered: false,
    },
    ProofComponent {
        id: "private_memory",
        summary: "Effective-address, frame-access, initialized-byte, load and store banks with the composed successful private LOAD64 and STORE64 steps.",
        sources: &[
            "crates/iroha_core_privacy/src/execution_proofs/ivm_step_air/memory_address.rs",
            "crates/iroha_core_privacy/src/execution_proofs/ivm_step_air/machine_bus/frame_access.rs",
            "crates/iroha_core_privacy/src/execution_proofs/ivm_step_air/machine_bus/memory_initialization.rs",
            "crates/iroha_core_privacy/src/execution_proofs/ivm_step_air/machine_bus/memory_load.rs",
            "crates/iroha_core_privacy/src/execution_proofs/ivm_step_air/machine_bus/memory_load/payload.rs",
            "crates/iroha_core_privacy/src/execution_proofs/ivm_step_air/machine_bus/memory_store.rs",
            "crates/iroha_core_privacy/src/execution_proofs/ivm_step_air/machine_bus/private_dispatch/load_success.rs",
            "crates/iroha_core_privacy/src/execution_proofs/ivm_step_air/machine_bus/private_dispatch/load_success/effect.rs",
            "crates/iroha_core_privacy/src/execution_proofs/ivm_step_air/machine_bus/private_dispatch/store_success.rs",
            "crates/iroha_core_privacy/src/execution_proofs/ivm_step_air/machine_bus/private_dispatch/store_success/effect.rs",
        ],
        opcodes: &[
            cov!(
                memory::LOAD64,
                "aligned initialized read as a successful private step"
            ),
            cov!(
                memory::STORE64,
                "aligned store as a successful private step"
            ),
            cov!(
                memory::LOAD128,
                "conditional bank equations only; not composed into a dispatcher role"
            ),
            cov!(
                memory::STORE128,
                "conditional bank equations only; not composed into a dispatcher role"
            ),
        ],
        referenced_opcodes: &["LOAD128", "LOAD64", "STORE128", "STORE64"],
        traps: &[],
        restrictions: &[
            "conditional post-read and aligned-store equations with no request authority",
            "frame ownership and initialized-byte decisions are conditional on the lifecycle descriptor bank",
            "privacy, access and local-deferral outcomes are fixed premises, not proven results",
            "effective-address equations are not linked to fetch or register reads on their own",
        ],
        substrate: &[
            Obligation::TypedValues,
            Obligation::Initialization,
            Obligation::MemoryOrdering,
            Obligation::Pointers,
            Obligation::Gas,
        ],
        geometry: &[
            geometry(
                "load_ports",
                37,
                "crates/iroha_core_privacy/src/execution_proofs/ivm_step_air/machine_bus/private_dispatch/load_success.rs",
                "PORTS",
            ),
            geometry(
                "store_ports",
                40,
                "crates/iroha_core_privacy/src/execution_proofs/ivm_step_air/machine_bus/private_dispatch/store_success.rs",
                "PORTS",
            ),
        ],
        registered: false,
    },
    ProofComponent {
        id: "native_invocation",
        summary: "Original native packet owner joined to public root initialization, complete history, bounded straight-line instructions, the public leaf return and successful padding.",
        sources: &[
            "crates/iroha_core_privacy/src/execution_proofs/ivm_step_air/machine_bus/native_invocation.rs",
            "crates/iroha_core_privacy/src/execution_proofs/ivm_step_air/machine_bus/native_invocation/instructions.rs",
            "crates/iroha_core_privacy/src/execution_proofs/ivm_step_air/machine_bus/native_invocation/instructions/memory_access.rs",
            "crates/iroha_core_privacy/src/execution_proofs/ivm_step_air/machine_bus/native_invocation/instructions/schedule.rs",
            "crates/iroha_core_privacy/src/execution_proofs/ivm_step_air/machine_bus/native_invocation/returning.rs",
            "crates/iroha_core_privacy/src/execution_proofs/ivm_step_air/machine_bus/native_invocation/returning/validation.rs",
            "crates/iroha_core_privacy/src/execution_proofs/ivm_step_air/machine_bus/native_invocation/root.rs",
            "crates/iroha_core_privacy/src/execution_proofs/ivm_step_air/machine_bus/native_invocation/terminal.rs",
            "crates/iroha_core_privacy/src/execution_proofs/ivm_step_air/machine_bus/private_dispatch/native_witness.rs",
            "crates/iroha_core_privacy/src/execution_proofs/ivm_step_air/machine_bus/private_dispatch/scalar/native_witness.rs",
            "crates/ivm/src/execution_packets/mod.rs",
            "crates/ivm/src/execution_packets/runtime.rs",
            "crates/ivm/src/execution_packets/scalar.rs",
            "crates/ivm/src/execution_packets/schedule.rs",
            "crates/ivm/src/execution_packets/storage.rs",
        ],
        opcodes: &[
            cov!(arithmetic::ADD, "public operands only"),
            cov!(arithmetic::SUB, "public operands only"),
            cov!(arithmetic::AND, "public operands only"),
            cov!(arithmetic::OR, "public operands only"),
            cov!(arithmetic::XOR, "public operands only"),
            cov!(arithmetic::SLL, "public operands only"),
            cov!(arithmetic::SRL, "public operands only"),
            cov!(arithmetic::SRA, "public operands only"),
            cov!(arithmetic::SLT, "public operands only"),
            cov!(arithmetic::SLTU, "public operands only"),
            cov!(arithmetic::NOT, "public operands only"),
            cov!(arithmetic::NEG, "public operands only"),
            cov!(arithmetic::SEQ, "public operands only"),
            cov!(arithmetic::SNE, "public operands only"),
            cov!(arithmetic::MUL, "public operands only"),
            cov!(arithmetic::MULH, "public operands only"),
            cov!(arithmetic::MULHU, "public operands only"),
            cov!(arithmetic::MULHSU, "public operands only"),
            cov!(arithmetic::ROTL, "public operands only"),
            cov!(arithmetic::ROTR, "public operands only"),
            cov!(arithmetic::POPCNT, "public operands only"),
            cov!(arithmetic::CLZ, "public operands only"),
            cov!(arithmetic::CTZ, "public operands only"),
            cov!(arithmetic::MIN, "public operands only"),
            cov!(arithmetic::MAX, "public operands only"),
            cov!(arithmetic::ADDI, "public operands only"),
            cov!(arithmetic::ANDI, "public operands only"),
            cov!(arithmetic::ORI, "public operands only"),
            cov!(arithmetic::XORI, "public operands only"),
            cov!(arithmetic::ROTL_IMM, "public operands only"),
            cov!(arithmetic::ROTR_IMM, "public operands only"),
            cov!(
                memory::LOAD64,
                "initialized bytes of the public root stack frame only"
            ),
            cov!(
                memory::STORE64,
                "public root stack frame or public leaf result region only"
            ),
            cov!(memory::LDI64, "admitted scalar literal"),
            cov!(
                control::JALR,
                "the single root return of a public Unit or Bool leaf"
            ),
        ],
        referenced_opcodes: &[
            "ADD", "ADDI", "AND", "ANDI", "BEQ", "BGE", "BGEU", "BLT", "BLTU", "BNE", "CLZ", "CTZ",
            "JALR", "LDI64", "LOAD64", "MAX", "MIN", "MUL", "MULH", "MULHSU", "MULHU", "NEG",
            "NOT", "OR", "ORI", "POPCNT", "ROTL", "ROTL_IMM", "ROTR", "ROTR_IMM", "SEQ", "SLL",
            "SLT", "SLTU", "SNE", "SRA", "SRL", "STORE64", "SUB", "XOR", "XORI",
        ],
        traps: &[],
        restrictions: &[
            "one fresh public root invocation with empty public arguments and a Unit or Bool result",
            "no child calls, syscalls, private or wide memory, pointer literals, branches or faults",
            "at most 64 executed instructions including the root return",
            "successful ZK padding only; no terminal statement publication or masked invocation transcript",
            "native capture is witness production; its operand-shape lookup bounds observation coverage, not AIR acceptance",
        ],
        substrate: &[
            Obligation::Fetch,
            Obligation::TypedValues,
            Obligation::Initialization,
            Obligation::MemoryOrdering,
            Obligation::Pointers,
            Obligation::Calls,
            Obligation::Copyback,
            Obligation::Gas,
            Obligation::Padding,
        ],
        geometry: &[
            geometry(
                "packet_slots",
                16384,
                "crates/ivm/src/execution_packets/schedule.rs",
                "PACKET_SLOTS",
            ),
            geometry(
                "max_steps",
                64,
                "crates/ivm/src/execution_packets/schedule.rs",
                "MAX_STEPS",
            ),
            geometry(
                "return_cells",
                4097,
                "crates/ivm/src/execution_packets/schedule.rs",
                "RETURN_CELLS",
            ),
            geometry(
                "root_slots",
                64,
                "crates/ivm/src/execution_packets/schedule.rs",
                "ROOT_SLOTS",
            ),
        ],
        registered: false,
    },
];

/// Return one component by identifier.
#[must_use]
pub fn component(id: &str) -> Option<&'static ProofComponent> {
    COMPONENTS.iter().find(|entry| entry.id == id)
}

/// Components whose reviewed acceptance covers `opcode`.
pub fn components_for_opcode(opcode: u8) -> impl Iterator<Item = &'static ProofComponent> {
    COMPONENTS
        .iter()
        .filter(move |entry| entry.opcodes.iter().any(|covered| covered.opcode == opcode))
}

/// Components that hold a relation for the trap outcome `kind`.
pub fn components_for_trap(kind: VmTrapKind) -> impl Iterator<Item = &'static ProofComponent> {
    COMPONENTS
        .iter()
        .filter(move |entry| entry.traps.contains(&kind))
}
