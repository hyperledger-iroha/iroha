//! Deterministic execution-proof summaries returned by the IVM proof syscall.
/// Version emitted by the first-release execution-proof summary format.
pub const EXECUTION_PROOF_VERSION_V1: u16 = 1;
/// Deterministic commitment to an IVM execution.
///
/// This is not a SNARK/STARK proof. It is a byte-stable proof summary built
/// from the VM's recorded trace, constraint, memory, register, and Merkle-root
/// logs. Full cryptographic proof systems can use these commitments as stable
/// public material while preserving identical output across hardware.
#[derive(
    Debug, Clone, Default, PartialEq, Eq, norito::NoritoSerialize, norito::NoritoDeserialize,
)]
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
impl ExecutionProof {
    /// Return the exact framed Norito length of a v1 proof summary.
    ///
    /// Every v1 field has a fixed-width representation. Deriving the length
    /// through the canonical encoder keeps gas preparation coupled to the
    /// schema instead of duplicating a numeric ceiling that can drift.
    pub(crate) fn encoded_len_v1() -> Result<usize, norito::Error> {
        norito::encode_canonical(&Self::default()).map(|bytes| bytes.len())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn v1_encoded_length_is_value_independent_and_exact() {
        let empty = ExecutionProof::default();
        let populated = ExecutionProof {
            version: EXECUTION_PROOF_VERSION_V1,
            code_hash: [0x01; 32],
            final_register_root: [0x02; 32],
            final_memory_root: [0x03; 32],
            output_hash: [0x04; 32],
            pc_trace_hash: [0x05; 32],
            delta_trace_hash: [0x06; 32],
            register_trace_hash: [0x07; 32],
            constraint_hash: [0x08; 32],
            memory_log_hash: [0x09; 32],
            register_log_hash: [0x0a; 32],
            step_log_hash: [0x0b; 32],
            cycles: u64::MAX,
            max_cycles: u64::MAX - 1,
            gas_used: u64::MAX - 2,
            gas_remaining: u64::MAX - 3,
            pc_trace_len: u64::MAX - 4,
            delta_trace_len: u64::MAX - 5,
            register_trace_len: u64::MAX - 6,
            constraint_len: u64::MAX - 7,
            memory_log_len: u64::MAX - 8,
            register_log_len: u64::MAX - 9,
            step_log_len: u64::MAX - 10,
            zk_mode: true,
            halted: true,
            constraint_failed: true,
        };
        let expected = ExecutionProof::encoded_len_v1().expect("fixed proof schema encodes");
        assert_eq!(
            norito::encode_canonical(&empty)
                .expect("encode canonical empty proof")
                .len(),
            expected
        );
        assert_eq!(
            norito::encode_canonical(&populated)
                .expect("encode canonical populated proof")
                .len(),
            expected
        );
        let alternate_flags =
            norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN;
        let _ambient = norito::core::DecodeFlagsGuard::enter(alternate_flags);
        assert_eq!(
            ExecutionProof::encoded_len_v1().expect("canonical length ignores ambient layout"),
            expected
        );
    }
}
