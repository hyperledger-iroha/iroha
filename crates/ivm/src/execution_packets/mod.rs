//! Original native packet custody for one bounded empty-argument Unit root.
//!
//! The ordinary interpreter produces these accesses while it executes. The only
//! public constructor owns a fresh VM and its admitted artifact; callers cannot
//! provide initial registers, memory, packets, clocks, frame IDs or a host.
//! This is an unregistered execution component, not a proof or finalized-State
//! authority. The privacy crate's initializer/history relation consumes this
//! owner, including its Unit validation gas and successful padding. General
//! instruction/typed semantics and complete invocation statement/transcript
//! joins remain required before these packets can authorize proof admission.
//! No diagnostic snapshots or native recomputation establish those constraints.
//! Public straight-line arithmetic and bit-operation capture shares the exact opcode/operand
//! shape lookup with its constrained consumer; private scalar operands and
//! broader instructions remain outside this bounded component.

mod scalar;
mod schedule;
mod storage;
use crate::{
    PreparedContract, VMError,
    execution_memory::{ExecutionMemoryLease, ExecutionMemoryPlan},
};
use iroha_allocation::{
    AllocationBudget, AllocationRefusal, InsufficientReservation, PrepaidBufferError,
};
pub use scalar::public_scalar_operands;
pub use schedule::{
    INSTRUCTION_WINDOWS, MAX_STEPS, PACKET_SLOTS, RETURN_CELLS, ROOT_SLOTS, instruction_clocks,
};
pub use storage::NativePacket;

/// Disjoint native state classes; these are not serialized ABI identifiers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PacketSpace {
    /// One physical sixteen-byte cell.
    Memory = 1,
    /// One general register.
    Register = 2,
    /// Sixteen initialized-byte bits owned by a frame generation.
    Initialization = 3,
    /// One protected runtime/control word.
    Owner = 4,
}
/// Local component refusal or a genuine error returned by ordinary execution.
#[derive(Debug)]
pub enum CaptureError {
    /// The fixed local packet/step profile was exhausted.
    Capacity,
    /// A supplied parent belongs to a different original allocation pool.
    PoolMismatch,
    /// Preserve the original error calculating the exact backing demand.
    Plan(AllocationRefusal),
    /// Preserve the unchanged original parent's exact shortage.
    Reservation(InsufficientReservation),
    /// Preserve physical allocator and prepaid-buffer refusal custody.
    Allocation(PrepaidBufferError),
    /// The artifact, selector or reached operation is outside this component.
    Unsupported,
    /// Ordinary native execution failed; no accepted packet owner is returned.
    Execution(VMError),
}
impl core::fmt::Display for CaptureError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Capacity => f.write_str("native component capacity unavailable"),
            Self::PoolMismatch => f.write_str("native component parent belongs to another pool"),
            Self::Plan(error) => error.fmt(f),
            Self::Reservation(error) => error.fmt(f),
            Self::Allocation(error) => error.fmt(f),
            Self::Unsupported => f.write_str("invocation outside native component coverage"),
            Self::Execution(error) => write!(f, "native invocation failed: {error}"),
        }
    }
}
impl std::error::Error for CaptureError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Plan(error) => Some(error),
            Self::Reservation(error) => Some(error),
            Self::Allocation(error) => Some(error),
            Self::Execution(error) => Some(error),
            Self::Capacity | Self::PoolMismatch | Self::Unsupported => None,
        }
    }
}

/// Sealed successful native output. Private backing is erased on final drop.
///
/// Coverage is exactly one public empty-argument Unit root, no syscalls or child
/// calls, at most 64 total instructions and 64 configured cycles. This is
/// intentionally not a complete IVM proof capability.
pub struct NativeInvocation {
    pub(crate) contract: PreparedContract,
    pub(crate) entrypoint: usize,
    pub(crate) initial_gas: u64,
    pub(crate) final_gas: u64,
    pub(crate) cycles: u64,
    pub(crate) instructions: usize,
    pub(crate) packets: storage::Storage,
}
impl NativeInvocation {
    /// Exact fixed backing demand; VM and artifact allocations are separate owners.
    pub fn allocation_plan() -> Result<ExecutionMemoryPlan, CaptureError> {
        storage::Storage::plan()
    }
    /// Run the ordinary interpreter from an internally constructed fresh VM.
    ///
    /// Packet backing is partitioned from the original parent before the VM is
    /// constructed; the VM's existing owners use `budget`. Profile refusal is a
    /// local proving refusal and never changes ordinary transaction validity.
    pub fn run_unit_root(
        contract: PreparedContract,
        selector: &str,
        initial_gas: u64,
        parent: &mut ExecutionMemoryLease,
        budget: &AllocationBudget,
    ) -> Result<Self, CaptureError> {
        if !parent.belongs_to(budget) {
            return Err(CaptureError::PoolMismatch);
        }
        crate::IVM::capture_unit_root(contract, selector, initial_gas, parent, budget)
    }
    /// Original admitted immutable artifact, retained rather than identified by a supplied hash.
    pub fn artifact(&self) -> &PreparedContract {
        &self.contract
    }
    /// Index of the actual public entrypoint in the retained artifact.
    pub fn entrypoint_index(&self) -> usize {
        self.entrypoint
    }
    /// Public initial gas supplied to the fresh native invocation.
    pub fn initial_gas(&self) -> u64 {
        self.initial_gas
    }
    /// Exact remaining gas, including staged validation and native ZK padding.
    pub fn remaining_gas(&self) -> u64 {
        self.final_gas
    }
    /// Actual completed cycles, including native ZK padding.
    pub fn cycles(&self) -> u64 {
        self.cycles
    }
    /// Actual number of fetched/executed instructions, including the root return.
    pub fn instructions(&self) -> usize {
        self.instructions
    }
    /// Borrow the complete private padded packet allocation.
    pub fn packets(&self) -> &[NativePacket] {
        self.packets.packets()
    }
}

pub(crate) use schedule::{
    COMPACT_DISPATCH, PADDING_FIRST, RETURN_DISPATCH, RETURN_FIRST, SCAN_OFFSET, STEP_SLOTS,
};
pub(crate) use storage::Storage;

#[cfg(test)]
pub(crate) mod tests;
