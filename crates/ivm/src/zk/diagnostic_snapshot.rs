//! Move-only, original-pool custody for detached diagnostic trace snapshots.
//!
//! These records are diagnostics, not an execution proof. Live source log growth
//! and caller-created copies remain separate custody boundaries.

use super::{Constraint, DeltaEntry, MemEvent, RegEvent, RegisterState, StepEntry};
use crate::{
    VMError,
    error::ExecutionDeferral,
    execution_memory::{ExecutionBuffer, ExecutionMemoryLease, ExecutionMemoryPlan},
};
use iroha_crypto::zeroize_value_for_confidential_discard as erase;
use mv::allocation::AllocationBudget;
use std::ops::Range;

/// Borrowed register records supplied to a detached diagnostic capture.
pub enum DiagnosticRegisterSource<'a> {
    /// Already expanded cycle states.
    States(&'a [RegisterState]),
    /// Compact cycle changes, expanded directly into funded backing.
    Deltas(&'a [DeltaEntry]),
}

/// Borrowed source records whose complete backing demand is planned before copying.
pub struct DiagnosticTraceSource<'a> {
    /// Per-cycle register data.
    pub registers: DiagnosticRegisterSource<'a>,
    /// Recorded assertions.
    pub constraints: &'a [Constraint],
    /// Ordered memory events and their path snapshots.
    pub memory_events: &'a [MemEvent],
    /// Ordered register events and authentication paths.
    pub register_events: &'a [RegEvent],
    /// Per-step roots.
    pub steps: &'a [StepEntry],
}

struct MemoryRow {
    written: bool,
    address: u64,
    value: u128,
    size: u8,
    path: Range<usize>,
    root: [u8; 32],
}
struct RegisterRow {
    written: bool,
    index: usize,
    value: u64,
    tag: bool,
    path: Range<usize>,
    root: [u8; 32],
}

/// Borrowed memory event backed by its retained snapshot owner.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DiagnosticMemoryEvent<'a> {
    /// Store rather than load.
    pub written: bool,
    /// Absolute address.
    pub address: u64,
    /// Observed scalar value.
    pub value: u128,
    /// Access width.
    pub size: u8,
    /// Borrowed leaf-to-root siblings.
    pub path: &'a [[u8; 32]],
    /// Borrowed root bytes.
    pub root: &'a [u8; 32],
}
/// Borrowed register event backed by its retained snapshot owner.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DiagnosticRegisterEvent<'a> {
    /// Write rather than read.
    pub written: bool,
    /// Register position.
    pub index: usize,
    /// Observed scalar value.
    pub value: u64,
    /// Observed privacy tag.
    pub tag: bool,
    /// Borrowed leaf-to-root siblings.
    pub path: &'a [[u8; 32]],
    /// Borrowed root bytes.
    pub root: &'a [u8; 32],
}

/// Fixed, move-only detached records. Original charges remain attached to backing.
///
/// No owned extraction or unchecked clone is exposed. Drop volatile-erases all
/// initialized private values before the fixed allocations release their credit.
pub struct DiagnosticTraceSnapshot {
    states: ExecutionBuffer<RegisterState>,
    constraints: ExecutionBuffer<Constraint>,
    memory_events: ExecutionBuffer<MemoryRow>,
    register_events: ExecutionBuffer<RegisterRow>,
    paths: ExecutionBuffer<[u8; 32]>,
    steps: ExecutionBuffer<StepEntry>,
}

fn unavailable() -> VMError {
    VMError::ExecutionDeferred(ExecutionDeferral::AllocationUnavailable)
}
fn memory_parts(event: &MemEvent) -> (bool, u64, u128, u8, &[[u8; 32]], &[u8; 32]) {
    match event {
        MemEvent::Load {
            addr,
            value,
            size,
            path,
            root,
        } => (false, *addr, *value, *size, path, root.as_ref()),
        MemEvent::Store {
            addr,
            value,
            size,
            path,
            root,
        } => (true, *addr, *value, *size, path, root.as_ref()),
    }
}
fn register_parts(event: &RegEvent) -> (bool, usize, u64, bool, &[[u8; 32]], &[u8; 32]) {
    match event {
        RegEvent::Read {
            index,
            value,
            tag,
            path,
            root,
        } => (false, *index, *value, *tag, path, root.as_ref()),
        RegEvent::Write {
            index,
            value,
            tag,
            path,
            root,
        } => (true, *index, *value, *tag, path, root.as_ref()),
    }
}

impl DiagnosticTraceSource<'_> {
    fn state_count(&self) -> usize {
        match &self.registers {
            DiagnosticRegisterSource::States(rows) => rows.len(),
            DiagnosticRegisterSource::Deltas(rows) => rows.len(),
        }
    }
    fn path_count(&self) -> Result<usize, VMError> {
        self.memory_events
            .iter()
            .map(|event| memory_parts(event).4.len())
            .chain(
                self.register_events
                    .iter()
                    .map(|event| register_parts(event).4.len()),
            )
            .try_fold(0_usize, |count, len| {
                count.checked_add(len).ok_or_else(unavailable)
            })
    }
    /// Complete checked demand, including every nested Merkle sibling.
    ///
    /// # Errors
    /// Refuses overflowing allocation geometry or a malformed delta index.
    pub fn allocation_plan(&self) -> Result<ExecutionMemoryPlan, VMError> {
        if let DiagnosticRegisterSource::Deltas(rows) = &self.registers {
            if rows
                .iter()
                .any(|row| row.changes.iter().any(|(index, _, _)| *index >= 256))
            {
                return Err(VMError::DecodeError);
            }
        }
        let mut plan = ExecutionMemoryPlan::array::<RegisterState>(self.state_count())
            .map_err(|_| unavailable())?;
        for child in [
            ExecutionMemoryPlan::array::<Constraint>(self.constraints.len()),
            ExecutionMemoryPlan::array::<MemoryRow>(self.memory_events.len()),
            ExecutionMemoryPlan::array::<RegisterRow>(self.register_events.len()),
            ExecutionMemoryPlan::array::<[u8; 32]>(self.path_count()?),
            ExecutionMemoryPlan::array::<StepEntry>(self.steps.len()),
        ] {
            plan.include_child(child.map_err(|_| unavailable())?)
                .map_err(|_| unavailable())?;
        }
        Ok(plan)
    }
    /// Capture through the supplied original pool, before copying any private data.
    ///
    /// # Errors
    /// Returns the original admission refusal or a local allocator deferral.
    pub fn try_snapshot(
        &self,
        budget: &AllocationBudget,
    ) -> Result<DiagnosticTraceSnapshot, VMError> {
        let plan = self.allocation_plan()?;
        let mut parent =
            ExecutionMemoryLease::reserve(budget, plan).map_err(VMError::AllocationDeferred)?;
        self.try_snapshot_from_parent(&mut parent)
    }
    /// Partition this complete capture from an already admitted original parent.
    ///
    /// # Errors
    /// Refuses before copying if parent credit, geometry or allocation is insufficient.
    pub fn try_snapshot_from_parent(
        &self,
        parent: &mut ExecutionMemoryLease,
    ) -> Result<DiagnosticTraceSnapshot, VMError> {
        let plan = self.allocation_plan()?;
        let mut lease = parent.partition(plan).map_err(|_| unavailable())?;
        // Allocate every fixed backing before populating any private field.
        // A partial allocation failure therefore holds no plaintext copies.
        let mut snapshot = DiagnosticTraceSnapshot {
            states: ExecutionBuffer::new(self.state_count(), &mut lease)
                .map_err(|_| unavailable())?,
            constraints: ExecutionBuffer::new(self.constraints.len(), &mut lease)
                .map_err(|_| unavailable())?,
            memory_events: ExecutionBuffer::new(self.memory_events.len(), &mut lease)
                .map_err(|_| unavailable())?,
            register_events: ExecutionBuffer::new(self.register_events.len(), &mut lease)
                .map_err(|_| unavailable())?,
            paths: ExecutionBuffer::new(self.path_count()?, &mut lease)
                .map_err(|_| unavailable())?,
            steps: ExecutionBuffer::new(self.steps.len(), &mut lease).map_err(|_| unavailable())?,
        };
        match &self.registers {
            DiagnosticRegisterSource::States(rows) => {
                for row in *rows {
                    snapshot.states.push_reserved(RegisterState {
                        pc: row.pc,
                        gpr: row.gpr,
                        tags: row.tags,
                    });
                }
            }
            DiagnosticRegisterSource::Deltas(rows) => {
                let mut scratch = ExpansionState {
                    gpr: [0; 256],
                    tags: [false; 256],
                };
                for row in *rows {
                    for &(index, value, tag) in &row.changes {
                        scratch.gpr[index] = value;
                        scratch.tags[index] = tag;
                    }
                    snapshot.states.push_reserved(RegisterState {
                        pc: row.pc,
                        gpr: scratch.gpr,
                        tags: scratch.tags,
                    });
                }
            }
        }
        for row in self.constraints {
            snapshot.constraints.push_reserved(*row);
        }
        for row in self.memory_events {
            let (written, address, value, size, path, root) = memory_parts(row);
            let start = snapshot.paths.as_slice().len();
            snapshot
                .paths
                .append(path)
                .expect("complete path preflight");
            snapshot.memory_events.push_reserved(MemoryRow {
                written,
                address,
                value,
                size,
                path: start..snapshot.paths.as_slice().len(),
                root: *root,
            });
        }
        for row in self.register_events {
            let (written, index, value, tag, path, root) = register_parts(row);
            let start = snapshot.paths.as_slice().len();
            snapshot
                .paths
                .append(path)
                .expect("complete path preflight");
            snapshot.register_events.push_reserved(RegisterRow {
                written,
                index,
                value,
                tag,
                path: start..snapshot.paths.as_slice().len(),
                root: *root,
            });
        }
        for row in self.steps {
            snapshot.steps.push_reserved(row.clone());
        }
        Ok(snapshot)
    }
}

struct ExpansionState {
    gpr: [u64; 256],
    tags: [bool; 256],
}
impl Drop for ExpansionState {
    fn drop(&mut self) {
        erase(&mut self.gpr);
        erase(&mut self.tags);
    }
}

impl DiagnosticTraceSnapshot {
    /// Borrow expanded cycle states without detaching their owner.
    pub fn states(&self) -> &[RegisterState] {
        self.states.as_slice()
    }
    /// Borrow the recorded constraints.
    pub fn constraints(&self) -> &[Constraint] {
        self.constraints.as_slice()
    }
    /// Borrow the recorded step roots.
    pub fn steps(&self) -> &[StepEntry] {
        self.steps.as_slice()
    }
    /// Number of retained memory events.
    pub fn memory_event_count(&self) -> usize {
        self.memory_events.as_slice().len()
    }
    /// Number of retained register events.
    pub fn register_event_count(&self) -> usize {
        self.register_events.as_slice().len()
    }
    /// Iterate borrowed memory events and paths in their original order.
    pub fn memory_events(&self) -> impl ExactSizeIterator<Item = DiagnosticMemoryEvent<'_>> + '_ {
        self.memory_events
            .as_slice()
            .iter()
            .map(|row| DiagnosticMemoryEvent {
                written: row.written,
                address: row.address,
                value: row.value,
                size: row.size,
                path: &self.paths.as_slice()[row.path.clone()],
                root: &row.root,
            })
    }
    /// Inspect one register event without copying its path.
    pub fn register_event(&self, index: usize) -> Option<DiagnosticRegisterEvent<'_>> {
        self.register_events
            .as_slice()
            .get(index)
            .map(|row| DiagnosticRegisterEvent {
                written: row.written,
                index: row.index,
                value: row.value,
                tag: row.tag,
                path: &self.paths.as_slice()[row.path.clone()],
                root: &row.root,
            })
    }
    /// Iterate borrowed register events and paths in their original order.
    pub fn register_events(
        &self,
    ) -> impl ExactSizeIterator<Item = DiagnosticRegisterEvent<'_>> + '_ {
        (0..self.register_event_count()).map(|index| {
            self.register_event(index)
                .expect("initialized register descriptor")
        })
    }
}

impl Drop for DiagnosticTraceSnapshot {
    fn drop(&mut self) {
        for state in self.states.as_mut_slice() {
            erase(&mut state.pc);
            erase(&mut state.gpr);
            erase(&mut state.tags);
        }
        for row in self.memory_events.as_mut_slice() {
            erase(&mut row.written);
            erase(&mut row.path.start);
            erase(&mut row.path.end);
            erase(&mut row.address);
            erase(&mut row.value);
            erase(&mut row.size);
            erase(&mut row.root);
        }
        for row in self.register_events.as_mut_slice() {
            erase(&mut row.written);
            erase(&mut row.path.start);
            erase(&mut row.path.end);
            erase(&mut row.index);
            erase(&mut row.value);
            erase(&mut row.tag);
            erase(&mut row.root);
        }
        for path in self.paths.as_mut_slice() {
            erase(path);
        }
        for constraint in self.constraints.as_mut_slice() {
            match constraint {
                Constraint::Zero { reg, cycle } => {
                    erase(reg);
                    erase(cycle);
                }
                Constraint::Eq { reg1, reg2, cycle } => {
                    erase(reg1);
                    erase(reg2);
                    erase(cycle);
                }
                Constraint::Range { reg, bits, cycle } => {
                    erase(reg);
                    erase(bits);
                    erase(cycle);
                }
            }
        }
        for step in self.steps.as_mut_slice() {
            erase(&mut step.pc);
            let zero = iroha_crypto::HashOf::from_untyped_unchecked(iroha_crypto::Hash::prehashed(
                [0; 32],
            ));
            // SAFETY: write valid typed hash values into initialized, exclusive
            // fields. Never zero enum discriminants, padding or owner metadata.
            unsafe {
                std::ptr::write_volatile(&mut step.reg_root, zero);
                std::ptr::write_volatile(&mut step.mem_root, zero);
            }
        }
    }
}

#[cfg(test)]
mod tests;
