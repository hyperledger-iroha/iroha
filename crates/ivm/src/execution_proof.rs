//! Deterministic execution-proof summaries returned by the IVM proof syscall.

/// Version emitted by the first-release execution-proof summary format.
pub const EXECUTION_PROOF_VERSION_V1: u16 = 1;

/// Deterministic commitment to an IVM execution.
///
/// This is not a SNARK/STARK proof. It is a byte-stable proof summary built
/// from the VM's recorded trace, constraint, memory, register, and Merkle-root
/// logs. Full cryptographic proof systems can use these commitments as stable
/// public material while preserving identical output across hardware.
#[derive(Debug, Clone, PartialEq, Eq, norito::NoritoSerialize, norito::NoritoDeserialize)]
pub struct ExecutionProof {
    /// Summary format version.
    pub version: u16,
    /// Hash of the loaded executable code.
    pub code_hash: [u8; 32],
    /// Root of the final register file.
    pub final_register_root: [u8; 32],
    /// Root of the final memory image.
    pub final_memory_root: [u8; 32],
    /// Hash of the VM output region.
    pub output_hash: [u8; 32],
    /// Hash of the compact runtime PC trace.
    pub pc_trace_hash: [u8; 32],
    /// Hash of the compact delta-register trace.
    pub delta_trace_hash: [u8; 32],
    /// Hash of the ZK register-state trace.
    pub register_trace_hash: [u8; 32],
    /// Hash of logged constraints.
    pub constraint_hash: [u8; 32],
    /// Hash of logged memory accesses and proofs.
    pub memory_log_hash: [u8; 32],
    /// Hash of logged register accesses and proofs.
    pub register_log_hash: [u8; 32],
    /// Hash of per-cycle register and memory roots.
    pub step_log_hash: [u8; 32],
    /// Cycles consumed at the time the proof summary was created.
    pub cycles: u64,
    /// Maximum cycle bound active for this execution.
    pub max_cycles: u64,
    /// Gas consumed at the time the proof summary was created.
    pub gas_used: u64,
    /// Gas remaining at the time the proof summary was created.
    pub gas_remaining: u64,
    /// Number of compact PC trace entries.
    pub pc_trace_len: u64,
    /// Number of compact delta-register trace entries.
    pub delta_trace_len: u64,
    /// Number of expanded ZK register-state trace entries.
    pub register_trace_len: u64,
    /// Number of logged constraints.
    pub constraint_len: u64,
    /// Number of logged memory events.
    pub memory_log_len: u64,
    /// Number of logged register events.
    pub register_log_len: u64,
    /// Number of per-cycle Merkle-root entries.
    pub step_log_len: u64,
    /// Whether ZK mode was active.
    pub zk_mode: bool,
    /// Whether execution had halted when the summary was created.
    pub halted: bool,
    /// Whether a constraint failure had been observed when the summary was created.
    pub constraint_failed: bool,
}
