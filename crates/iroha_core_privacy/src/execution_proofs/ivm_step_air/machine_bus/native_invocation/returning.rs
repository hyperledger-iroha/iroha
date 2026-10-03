//! Original Unit return operands, staged validation and the complete scan.
//!
//! This component borrows the sealed native packet owner. The existing full
//! history pass authenticates its actual clocks; no separate events or clock
//! assignments are accepted. General memory/fault/typed coverage and terminal
//! statement publication with a masked invocation transcript remain open.

use super::super::{callable_lookup::SelectedCallable, packet, return_copyback};
use super::{F, Fields};
use crate::execution_proofs::ivm_step_air::residues::{Scratch, Sink, Stream};
use iroha_allocation::{AllocationBudget, ChargedBuffer, ChargedBufferError};
use ivm::execution_packets::{MAX_STEPS, NativeInvocation, instruction_clocks};

mod validation;
use validation::Validation;

struct Witness([F; return_copyback::WIDTH]);
impl Witness {
    fn clear(&mut self) {
        for field in &mut self.0 {
            field.zeroize_v1();
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
    Profile,
    Witness(return_copyback::native_witness::Error),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in super::super) enum Row {
    Operands,
    Validation,
    Scan(usize),
    Gap(usize),
}

pub(super) struct Returning {
    rows: ChargedBuffer<Witness>,
    validation: Validation,
    callable: SelectedCallable,
}
impl Returning {
    pub(super) const BYTES: usize = return_copyback::CELLS * core::mem::size_of::<Witness>();

    pub(super) fn new(native: &NativeInvocation, budget: &AllocationBudget) -> Result<Self, Error> {
        let callable = SelectedCallable::native_unit_root(native).ok_or(Error::Profile)?;
        // No private row or validation workspace is computed before the full
        // fixed allocation is admitted against its final owner's original pool.
        let mut rows =
            ChargedBuffer::new(return_copyback::CELLS, budget).map_err(Error::Allocation)?;
        let fixed = Fixed::new(native);
        let validation = Validation::new([&fixed.gas[0], &fixed.gas[1]], &fixed.memory);
        for offset in 0..return_copyback::CELLS {
            let cell = Cell::new(native, offset);
            let mut row = Witness([F::ZERO; return_copyback::WIDTH]);
            return_copyback::native_witness::fill(&mut row.0, offset, fixed.ports(&cell))
                .map_err(Error::Witness)?;
            rows.push_reserved(row);
        }
        Ok(Self {
            rows,
            validation,
            callable,
        })
    }

    /// Every event remains in the surrounding Source's one complete history.
    /// This method consumes no history challenge or independently supplied bank.
    pub(super) fn evaluate<E>(
        &self,
        native: &NativeInvocation,
        scratch: &mut Scratch,
        mut consume: impl FnMut(Row, &[F]) -> Result<(), E>,
    ) -> Result<(), E> {
        let fixed = Fixed::new(native);
        let first = first();
        {
            let mut emit = |values: &[F]| consume(Row::Operands, values);
            let mut out = Stream::new(scratch, &mut emit);
            // A successful root return is mandatory for this narrow source,
            // and cannot be erased by choosing a disabled lifecycle candidate.
            out.push(fixed.active.0[packet::ENABLED].sub(F::ONE));
            out.push(fixed.active.0[packet::BEFORE].sub(F::ONE));
            out.push(fixed.parent.0[packet::BEFORE]);
            return_copyback::append_operand_residues(
                &mut out,
                return_copyback::OperandSchedule::new(
                    0,
                    [15, 16, 17, 18, 19].map(|offset| first + offset),
                )
                .unwrap(),
                &self.rows.as_slice()[0].0,
                &self.callable,
                return_copyback::OperandPorts {
                    active: &fixed.active.0,
                    packets: core::array::from_fn(|index| &fixed.operands[index].0),
                },
            );
            out.finish()?;
        }
        {
            let mut emit = |values: &[F]| consume(Row::Validation, values);
            let mut out = Stream::new(scratch, &mut emit);
            self.validation.append_residues(
                &mut out,
                first,
                [&fixed.gas[0], &fixed.gas[1]],
                &fixed.memory,
            );
            out.finish()?;
        }
        for (offset, schedule) in return_copyback::schedules(
            0,
            [46, 47, 20, 21].map(|offset| first + offset),
            first + 100,
        )
        .unwrap()
        .enumerate()
        {
            let cell = Cell::new(native, offset);
            let mut emit = |values: &[F]| consume(Row::Scan(offset), values);
            let mut out = Stream::new(scratch, &mut emit);
            return_copyback::append_residues(
                &mut out,
                schedule,
                &self.rows.as_slice()[offset].0,
                fixed.ports(&cell),
            );
            debug_assert_eq!(out.len(), return_copyback::CONSTRAINTS);
            out.finish()?;
        }
        let clocks = instruction_clocks(MAX_STEPS).unwrap();
        for clock in first..=clocks[20] {
            let offset = clock - first;
            if clocks.contains(&clock)
                || (15..=24).contains(&offset)
                || (100..100 + 2 * return_copyback::CELLS as u32).contains(&offset)
            {
                continue;
            }
            let fields = Fields::native(&native.packets()[clock as usize]);
            let mut emit = |values: &[F]| consume(Row::Gap(offset as usize), values);
            let mut out = Stream::new(scratch, &mut emit);
            out.extend(fields.0);
            out.finish()?;
        }
        Ok(())
    }
}

fn first() -> u32 {
    instruction_clocks(MAX_STEPS).expect("fixed native return window")[0]
}
fn original(native: &NativeInvocation, offset: u32) -> Fields {
    Fields::native(&native.packets()[(first() + offset) as usize])
}
struct Fixed {
    active: Fields,
    parent: Fields,
    start: Fields,
    end: Fields,
    operands: [Fields; return_copyback::OPERAND_PORTS],
    gas: [Fields; 2],
    memory: Fields,
}
impl Fixed {
    fn new(native: &NativeInvocation) -> Self {
        Self {
            active: original(native, 46),
            parent: original(native, 47),
            start: original(native, 20),
            end: original(native, 21),
            operands: core::array::from_fn(|index| original(native, 15 + index as u32)),
            gas: [original(native, 22), original(native, 23)],
            memory: original(native, 24),
        }
    }
    fn ports<'a>(&'a self, cell: &'a Cell) -> return_copyback::Ports<'a> {
        return_copyback::Ports {
            active: &self.active.0,
            parent: &self.parent.0,
            result_start: &self.start.0,
            result_end: &self.end.0,
            child: &cell.child.0,
            copyback: &cell.copyback.0,
        }
    }
}
struct Cell {
    child: Fields,
    copyback: Fields,
}
impl Cell {
    fn new(native: &NativeInvocation, offset: usize) -> Self {
        Self {
            child: original(native, 100 + 2 * offset as u32),
            copyback: original(native, 101 + 2 * offset as u32),
        }
    }
}

#[cfg(test)]
mod tests;
