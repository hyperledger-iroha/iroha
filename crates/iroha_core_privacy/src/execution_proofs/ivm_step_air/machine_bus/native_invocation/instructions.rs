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
    /// Initialize in funded backing without placing or moving a full row on the stack.
    fn push_with<E>(
        rows: &mut ChargedBuffer<Self>,
        fill: impl FnOnce(&mut [F; private_dispatch::WIDTH]) -> Result<(), E>,
    ) -> Result<(), E> {
        let initialized = rows.as_slice().len();
        let slot = rows
            .spare_capacity_mut()
            .first_mut()
            .expect("one admitted instruction row remains");
        // SAFETY: `slot` is an exclusive, aligned slot within the original
        // allocation. Addressing its sole array field creates no reference to
        // uninitialized storage. Each in-bounds element receives a valid F::ZERO;
        // no zero-bit representation is assumed. These writes cannot fail and
        // introduce no private values or destructor obligations. Only after all
        // fields are initialized do we expose this one additional valid Witness.
        #[allow(unsafe_code)]
        unsafe {
            let fields = core::ptr::addr_of_mut!((*slot.as_mut_ptr()).0).cast::<F>();
            for column in 0..private_dispatch::WIDTH {
                fields.add(column).write(F::ZERO);
            }
            rows.set_initialized_len(initialized + 1);
        }
        // On error or unwind, drop and wipe the unfinished row before returning
        // to the caller, preserving its original order relative to memory rows.
        struct PendingRow<'a> {
            rows: &'a mut ChargedBuffer<Witness>,
            initialized: usize,
        }
        impl Drop for PendingRow<'_> {
            fn drop(&mut self) {
                self.rows.truncate(self.initialized);
            }
        }
        let mut pending = PendingRow { rows, initialized };
        fill(&mut pending.rows.as_mut_slice()[initialized].0)?;
        pending.initialized += 1;
        Ok(())
    }

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
            Witness::push_with(&mut rows, |row| {
                private_dispatch::native_witness::fill(row, &program, &packets)
            })
            .map_err(Error::Witness)?;
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
