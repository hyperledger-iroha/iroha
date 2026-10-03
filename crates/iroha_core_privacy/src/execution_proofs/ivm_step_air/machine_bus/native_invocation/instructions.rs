//! Bounded native instruction and public-root memory workspaces.
//!
//! The original atomic destination joins public straight-line arithmetic and bit
//! instructions, admitted LDI64 and initialized public-stack LOAD64. STORE64
//! and root JALR retain their original memory,
//! lifecycle and control packets. Private/wide loads and faults remain open;
//! no diagnostic tag event or caller-provided literal is admitted here.

use super::super::{packet, private_dispatch};
use super::{F, Fields, root};

mod memory_access;
mod schedule;
use iroha_allocation::{AllocationBudget, ChargedBuffer, ChargedBufferError};
use ivm::execution_packets::{INSTRUCTION_WINDOWS, NativeInvocation, instruction_clocks};

struct Witness([F; private_dispatch::WIDTH]);
impl Witness {
    fn clear(&mut self) {
        for value in &mut self.0 {
            value.zeroize_v1();
        }
    }
}
impl Drop for Witness {
    fn drop(&mut self) {
        self.clear();
    }
}

#[derive(Debug)]
pub(in super::super) enum Error {
    Allocation(ChargedBufferError),
    Artifact,
    Witness(private_dispatch::native_witness::Error),
}

/// Private to the native source owner: no independent raw-row constructor is
/// exposed to composed consumers. The source stores this beside its sole native
/// owner, preventing a caller from pairing different original packet banks.
pub(super) struct Instructions {
    program: private_dispatch::Program,
    rows: ChargedBuffer<Witness>,
    memory: memory_access::MemoryAccesses,
}
impl Instructions {
    pub(super) const BYTES: usize = INSTRUCTION_WINDOWS * core::mem::size_of::<Witness>()
        + memory_access::MemoryAccesses::BYTES;

    pub(super) fn new(
        native: &NativeInvocation,
        root: &root::Plan,
        budget: &AllocationBudget,
    ) -> Result<Self, Error> {
        // Fund every mandatory row before computing any private workspace.
        let mut rows =
            ChargedBuffer::new(INSTRUCTION_WINDOWS, budget).map_err(Error::Allocation)?;
        let program =
            private_dispatch::Program::new(native.artifact().clone()).ok_or(Error::Artifact)?;
        let memory =
            memory_access::MemoryAccesses::new(native, root, budget).map_err(Error::Allocation)?;
        for window in 0..INSTRUCTION_WINDOWS {
            let packets = original(native, window);
            let mut row = Witness([F::ZERO; private_dispatch::WIDTH]);
            private_dispatch::native_witness::fill(&mut row.0, &program, &packets)
                .map_err(Error::Witness)?;
            rows.push_reserved(row);
        }
        Ok(Self {
            program,
            rows,
            memory,
        })
    }

    /// Existing polynomial banks consume the same original ports as history.
    /// One small projection bank is wiped after each window; the source packet
    /// allocation remains the sole owner and no rows are omitted or reclocked.
    pub(super) fn append_residues(
        &self,
        out: &mut impl crate::execution_proofs::ivm_step_air::residues::Sink,
        native: &NativeInvocation,
        window: usize,
    ) {
        let packets = original(native, window);
        let schedule = private_dispatch::Schedule::new(
            0,
            instruction_clocks(window).expect("mandatory native window"),
        )
        .expect("ordered native clocks");
        private_dispatch::native_witness::append_subset_residues(
            out,
            &self.program,
            &self.rows.as_slice()[window].0,
            &packets,
        );
        let decoded = private_dispatch::append_residues(
            out,
            &self.program,
            schedule,
            &self.rows.as_slice()[window].0,
            &packets,
        );
        if window < ivm::execution_packets::MAX_STEPS {
            self.memory.append_residues(out, native, window, &decoded);
            schedule::append_native(out, native, window, &decoded);
        }
    }
}

fn original(native: &NativeInvocation, window: usize) -> private_dispatch::OriginalPackets {
    let clocks = instruction_clocks(window).expect("mandatory native window");
    private_dispatch::OriginalPackets::candidate(core::array::from_fn(|port| {
        let fields = Fields::native(&native.packets()[clocks[port] as usize]);
        fields.0
    }))
}

#[cfg(test)]
mod literals;
#[cfg(test)]
mod multiply_counts;
#[cfg(test)]
mod scalars;
#[cfg(test)]
mod shifts;
#[cfg(test)]
pub(super) mod tests;
