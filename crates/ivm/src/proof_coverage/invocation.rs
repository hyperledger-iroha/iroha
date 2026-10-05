//! Invocation-wide relation obligations that no single opcode or syscall owns.
//!
//! The complete relation proves one whole invocation: its public statement,
//! initialization, ordered history, terminal outcome, continuation and
//! masking. Each record names the unregistered components holding
//! component-level equations for it today, or none.

use super::{CoverageStatus, Obligation};

/// A source symbol that currently enforces or defines an obligation outside
/// any proof relation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Citation {
    /// Repository-relative source file.
    pub path: &'static str,
    /// Symbol text present in that file's non-test source.
    pub symbol: &'static str,
}

const fn cite(path: &'static str, symbol: &'static str) -> Citation {
    Citation { path, symbol }
}

/// One invocation-wide requirement and its current coverage.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InvocationObligation {
    /// Stable machine-readable identifier.
    pub id: &'static str,
    /// Obligation class.
    pub obligation: Obligation,
    /// What the complete relation must establish.
    pub requirement: &'static str,
    /// Identifiers of components holding component-level equations.
    pub components: &'static [&'static str],
    /// Source symbols that enforce the requirement today, outside any relation.
    pub citations: &'static [Citation],
    /// Current coverage; never complete while no relation is registered.
    pub status: CoverageStatus,
}

impl InvocationObligation {
    /// Attach the source symbols that enforce the requirement today.
    const fn citing(mut self, citations: &'static [Citation]) -> Self {
        self.citations = citations;
        self
    }
}

const fn open(
    id: &'static str,
    obligation: Obligation,
    requirement: &'static str,
) -> InvocationObligation {
    InvocationObligation {
        id,
        obligation,
        requirement,
        components: &[],
        citations: &[],
        status: CoverageStatus::Uncovered,
    }
}

const fn partial(
    id: &'static str,
    obligation: Obligation,
    requirement: &'static str,
    components: &'static [&'static str],
) -> InvocationObligation {
    InvocationObligation {
        id,
        obligation,
        requirement,
        components,
        citations: &[],
        status: CoverageStatus::ComponentOnly,
    }
}

const PROGRAM_HEADER: &str = "crates/ivm_abi/src/metadata/program_header.rs";
const ADMISSION: &str = "crates/iroha_core/src/pipeline/overlay.rs";
const ROOT_EFFECTS: &str = "crates/iroha_core/src/executor_execution_effects.rs";

/// Every invocation-wide obligation of the complete relation.
// TODO: Close each record in the single complete IVM AIR and its statement
// (task M.2). A component substrate never upgrades a record to complete.
pub const INVOCATION_OBLIGATIONS: &[InvocationObligation] = &[
    open(
        "statement.signed_intent",
        Obligation::StatementBinding,
        "the public statement derives from the signed intent; replay, attestation or a caller-supplied commitment cannot substitute",
    ),
    partial(
        "statement.abi_version",
        Obligation::StatementBinding,
        "the proved program declares canonical ABI version 1; any other version is rejected before execution",
        &["public_scalar_segment"],
    )
    .citing(&[cite(
        PROGRAM_HEADER,
        "VMError::UnsupportedProgramAbiVersion",
    )]),
    open(
        "statement.code_hash",
        Obligation::StatementBinding,
        "the executed code hash equals the code hash of the active on-chain manifest",
    )
    .citing(&[cite(
        ADMISSION,
        "IvmAdmissionError::ManifestCodeHashMismatch",
    )]),
    partial(
        "statement.manifest_abi_hash",
        Obligation::StatementBinding,
        "the manifest ABI hash equals the canonical ABI descriptor hash of the runtime, separately from the version and code-hash obligations",
        &["public_scalar_segment"],
    )
    .citing(&[
        cite("crates/ivm_abi/src/syscalls.rs", "pub fn compute_abi_hash("),
        cite(PROGRAM_HEADER, "VMError::ArtifactAbiHashMismatch"),
        cite(
            "crates/iroha_core/src/smartcontracts/ivm.rs",
            "IvmAdmissionError::ManifestAbiHashMismatch(",
        ),
    ]),
    partial(
        "statement.entrypoint_and_arguments",
        Obligation::StatementBinding,
        "the authorized entrypoint and every initialized argument word are bound; a private callable never authorizes a public invocation",
        &["native_invocation"],
    ),
    open(
        "statement.finalized_context",
        Obligation::StatementBinding,
        "chain, height, time, authority and the contract subject derive from the finalized anchor and runtime identity, never from node-local data",
    ),
    open(
        "statement.reads",
        Obligation::StateRead,
        "complete read dependencies are authenticated against the finalized anchor and rechecked against current State before effects commit",
    ),
    partial(
        "statement.returns_effects_events_gas",
        Obligation::StatementBinding,
        "exact returns, ordered effects and events and final gas are published in the statement",
        &["native_invocation"],
    ),
    open(
        "statement.execution_limits",
        Obligation::StatementBinding,
        "the gas limit, the cycle horizon, the shared signed-source cycle allowance and the host output limits are bound into the statement from committed protocol State; node-local configuration never selects a limit that changes the outcome",
    )
    .citing(&[
        cite("crates/ivm/src/ivm.rs", "pub fn set_gas_limit("),
        cite("crates/ivm/src/ivm.rs", "pub fn set_max_cycles("),
        cite("crates/ivm/src/ivm.rs", "pub struct VmCycleBudget"),
        cite(ROOT_EFFECTS, "self.pipeline.quarantine_tx_max_cycles"),
        cite(ROOT_EFFECTS, "max_instructions: self.pipeline.overlay_max_instructions"),
    ]),
    open(
        "statement.trigger_invocation",
        Obligation::StatementBinding,
        "a proof-backed trigger proves its actual later invocation; a stored proof for one anchor cannot authorize future executions",
    ),
    partial(
        "initialization.root",
        Obligation::Initialization,
        "root registers, frame, heap bounds, generation and protected words derive from the public artifact, gas and stack policy; a failed root-call initialization is a proven terminal outcome with zero completed steps, its exact error and gas, the argument-decode prepayment counted once",
        &["native_invocation"],
    )
    .citing(&[
        cite(
            "crates/ivm/src/call_runtime.rs",
            "pub(super) fn begin_root_call(",
        ),
        cite(
            "crates/ivm/src/argument_record.rs",
            "pub fn install_call_arguments(",
        ),
    ]),
    partial(
        "history.memory_register_owner_order",
        Obligation::MemoryOrdering,
        "one typed address/time-ordered history covers memory, registers, initialization and owner words for the whole invocation",
        &["machine_bus", "native_invocation"],
    ),
    partial(
        "terminal.success_and_padding",
        Obligation::Padding,
        "the terminal state and every padded ZK cycle up to the artifact cycle horizon are constrained with their gas; an initialization trap executes zero padding cycles, and any proof-shape padding preserves the terminal state and gas",
        &["native_invocation"],
    ),
    partial(
        "terminal.faults",
        Obligation::Faults,
        "every reachable initialization, interpreter and syscall trap is the unique proven terminal outcome with exact gas; prepare rejections yield no statement, host invariants are unreachable, and node-local deferrals and errors constructed outside the invocation are never provable",
        &["public_scalar_segment"],
    ),
    partial(
        "continuation.segments",
        Obligation::Continuation,
        "segment boundaries bind the same invocation, complete machine state, gas, reads and effects",
        &["machine_bus"],
    ),
    open(
        "composition.recursive_aggregate",
        Obligation::ProofComposition,
        "recursive or aggregate composition preserves every segment and nested-invocation statement",
    ),
    partial(
        "recursion.depth_limits",
        Obligation::VmRecursion,
        "the 1,024 protected callable-depth limit and the 32 nested-contract host limit are distinct, and both bound with their failure propagation",
        &["private_dispatch"],
    ),
    partial(
        "masking.private_trace",
        Obligation::PrivateMasking,
        "private trace columns and the joint invocation transcript are masked; raw witnesses never enter transactions or requests",
        &["machine_bus"],
    ),
];

/// Current production admission of proof-carrying IVM execution.
///
/// `IvmProved` verification rejects unconditionally until the complete native
/// relation, finalized State anchor and local private prover exist. Tests
/// check this statement against the rejecting verifier's source.
pub const PRODUCTION_ADMISSION: &str = "closed";
