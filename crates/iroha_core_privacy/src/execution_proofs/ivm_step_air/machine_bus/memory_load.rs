//! Conditional post-read LOAD64/LOAD128 equations, with no request authority.
//!
//! These ports must be the same canonical typed tuples constrained by the
//! machine bus. Its initializer/history constraints own their previous values
//! and u16 ranges. A fixed component input identifies a physically reached read
//! phase; neither this input nor the public-event bus authorizes reaching it.
// TODO: Bind this unregistered bank to constrained fetch/address/register,
// owner/initialization, native trap ordering and complete read/effect ports.

use super::{F, bit, packet, wide};

mod payload;
pub(super) use payload::payload_limb;

/// Sixteen private-mask bits and two canonical zero-test inverses.
/// The zero/full flags are derived polynomials, not extra witness columns.
pub(super) const WIDTH: usize = 18;
pub(super) const CONSTRAINTS: usize = 273;
const ZERO_INVERSE: usize = 16;
const FULL_INVERSE: usize = 17;
/// Gas, PC and cycles as two u32 limbs each, followed by commit and privacy trap.
pub(super) const CONTROL_WIDTH: usize = 8;
/// Exact physical read-log address limbs, byte length and enabled flag.
pub(super) const READ_LOG_WIDTH: usize = 4;
const COMMIT: usize = 6;
const PRIVACY_TRAP: usize = 7;

/// Fixed qualification boundary, not an authenticated permission or request.
#[derive(Clone, Copy)]
pub(super) struct ReadPhase {
    vm: u8,
    address: u64,
    clocks: [u32; 5],
    destinations: [usize; 2],
    wide: bool,
    stack: bool,
    gas_after: u64,
    pc: u64,
    cycles: u64,
}

impl ReadPhase {
    /// Reject known pre-read failures. Frame/region/initializer authority still
    /// belongs to the absent upstream relation, never to an input Boolean here.
    pub(super) fn from_fixed_input(
        instruction: u32,
        address: u64,
        vm: u8,
        clocks: [u32; 5],
        gas: u64,
        pc: u64,
        cycles: u64,
        stack_top: u64,
        zk: bool,
        vector: bool,
        base_private: bool,
    ) -> Option<Self> {
        let wide = match wide::opcode(instruction) {
            wide::memory::LOAD64 => false,
            wide::memory::LOAD128 => true,
            _ => return None,
        };
        let length = if wide { 16 } else { 8 };
        let end = address.checked_add(length)?;
        let gas_after = gas.checked_sub(if wide { 5 } else { 3 })?;
        if !zk
            || base_private
            || (wide && !vector)
            || !address.is_multiple_of(length)
            || address >> 4 > u64::from(u32::MAX)
            || !clocks.windows(2).all(|pair| pair[0] < pair[1])
            || cycles == u64::MAX
        {
            return None;
        }
        Some(Self {
            vm,
            address,
            clocks,
            destinations: [wide::rd(instruction), wide::rs2(instruction)],
            wide,
            stack: address >= ivm::Memory::STACK_START && end <= stack_top,
            gas_after,
            pc,
            cycles,
        })
    }

    fn selected_bits(self) -> core::ops::Range<usize> {
        if self.wide {
            0..16
        } else {
            let start = (self.address & 8) as usize;
            start..start + 8
        }
    }
}

/// Candidate witnesses; all bits and canonical inverse equalities are constrained.
pub(super) fn witness(private_mask: u16, phase: ReadPhase) -> [F; WIDTH] {
    let mut row = [F::ZERO; WIDTH];
    for (index, value) in row[..16].iter_mut().enumerate() {
        *value = F(u64::from((private_mask >> index) & 1));
    }
    let selected = phase.selected_bits();
    let count = row[selected.clone()].iter().copied().fold(F::ZERO, F::add);
    row[ZERO_INVERSE] = count.inv().unwrap_or(F::ZERO);
    row[FULL_INVERSE] = count.sub(F(selected.len() as u64)).inv().unwrap_or(F::ZERO);
    row
}

/// Constrain the memory tuple and four native ordered value/tag register tuples.
/// The read port also owns the physical read-log entry; architectural failure
/// cannot suppress it. No packet is synthesized from an unconstrained callback.
pub(super) fn append_residues(
    out: &mut Vec<F>,
    phase: ReadPhase,
    row: &[F; WIDTH],
    read: &[F; packet::WIDTH],
    writes: [&[F; packet::WIDTH]; 4],
    read_log: &[F; READ_LOG_WIDTH],
    control: &[F; CONTROL_WIDTH],
) {
    use packet::*;
    let start = out.len();
    out.extend(row[..16].iter().copied().map(bit));
    let mask = row[..16]
        .iter()
        .enumerate()
        .fold(F::ZERO, |sum, (i, x)| sum.add(x.mul(F(1 << i))));
    out.push(read[BEFORE_TAG].sub(mask));
    let selected = phase.selected_bits();
    let count = row[selected.clone()].iter().copied().fold(F::ZERO, F::add);
    // For x != 0, x * flag = 0 forces inverse = 1/x. For x = 0,
    // flag is exactly one and flag * inverse = 0 forces inverse = 0.
    // The derived flags are quadratic; all residuals remain degree at most
    // four, including enabled * private in the native tag-write ports.
    let full_difference = count.sub(F(selected.len() as u64));
    let zero = F::ONE.sub(count.mul(row[ZERO_INVERSE]));
    let full = F::ONE.sub(full_difference.mul(row[FULL_INVERSE]));
    out.push(bit(zero));
    out.push(bit(full));
    for (value, flag, inverse) in [
        (count, zero, row[ZERO_INVERSE]),
        (full_difference, full, row[FULL_INVERSE]),
    ] {
        out.push(value.mul(flag));
        out.push(value.mul(inverse).sub(F::ONE.sub(flag)));
        out.push(flag.mul(inverse));
    }
    let private = full.mul(F(u64::from(phase.stack)));
    let commit = zero.add(private);
    header(
        out,
        read,
        Space::Memory,
        phase,
        0,
        (phase.address >> 4) as u32,
        F::ONE,
        F::ZERO,
    );
    for limb in 0..8 {
        out.push(read[AFTER + limb].sub(read[BEFORE + limb]));
    }
    out.push(read[AFTER_TAG].sub(read[BEFORE_TAG]));
    // Native order is value-low, value-high, tag-low, tag-high. In
    // particular a value write preserves its OLD tag. Atomic value/tag ports
    // would change the observable RegEvent history when old and new tags differ.
    for (slot, write) in writes.iter().enumerate() {
        let operand = slot % 2;
        let destination = phase.destinations[operand];
        let enabled = commit.mul(F(u64::from(
            destination != 0 && (operand == 0 || phase.wide),
        )));
        header(
            out,
            write,
            Space::Register,
            phase,
            slot + 1,
            destination as u32,
            enabled,
            enabled,
        );
        for value in *write {
            out.push(F::ONE.sub(enabled).mul(*value));
        }
        for limb in 4..8 {
            out.push(write[BEFORE + limb]);
        }
        let half = if phase.wide {
            operand
        } else {
            (phase.address as usize & 8) / 8
        };
        for limb in 0..8 {
            let expected = if limb >= 4 {
                F::ZERO
            } else if slot < 2 {
                payload_limb(read[BEFORE + limb], read[BEFORE + 4 + limb], F(half as u64))
            } else {
                write[BEFORE + limb]
            };
            out.push(write[AFTER + limb].sub(enabled.mul(expected)));
        }
        let tag = if slot < 2 { write[BEFORE_TAG] } else { private };
        out.push(write[AFTER_TAG].sub(enabled.mul(tag)));
        out.push(bit(write[BEFORE_TAG]));
    }
    // Every subsequent write to the same register starts from the most recent
    // earlier event, including the intermediate OLD tag after a value write.
    // The initial before-value/tag is owned by the typed state bus, not here.
    for slot in 1..4 {
        let operand = slot % 2;
        let previous = (0..slot).rev().find(|prior| {
            phase.destinations[*prior % 2] == phase.destinations[operand]
                && (*prior % 2 == 0 || phase.wide)
        });
        let enabled = commit.mul(F(u64::from(
            phase.destinations[operand] != 0 && (operand == 0 || phase.wide) && previous.is_some(),
        )));
        let previous = previous.unwrap_or(0);
        for limb in 0..8 {
            out.push(enabled.mul(writes[slot][BEFORE + limb].sub(writes[previous][AFTER + limb])));
        }
        out.push(enabled.mul(writes[slot][BEFORE_TAG].sub(writes[previous][AFTER_TAG])));
    }
    for (offset, before, after) in [
        (0, phase.gas_after, phase.gas_after),
        (2, phase.pc, phase.pc.wrapping_add(4)),
        (4, phase.cycles, phase.cycles + 1),
    ] {
        for limb in 0..2 {
            let a = F((before >> (32 * limb)) & u64::from(u32::MAX));
            let b = F((after >> (32 * limb)) & u64::from(u32::MAX));
            out.push(control[offset + limb].sub(a.add(commit.mul(b.sub(a)))));
        }
    }
    // This exact byte range survives a late privacy failure, independently of
    // the coarser 16-byte cell used by the state bus.
    for (actual, expected) in read_log.iter().zip([
        F(phase.address & u64::from(u32::MAX)),
        F(phase.address >> 32),
        F(if phase.wide { 16 } else { 8 }),
        F::ONE,
    ]) {
        out.push(actual.sub(expected));
    }
    out.push(control[COMMIT].sub(commit));
    out.push(control[PRIVACY_TRAP].sub(F::ONE.sub(commit)));
    debug_assert_eq!(out.len() - start, CONSTRAINTS);
}

fn header(
    out: &mut Vec<F>,
    port: &[F; packet::WIDTH],
    space: packet::Space,
    phase: ReadPhase,
    slot: usize,
    index: u32,
    enabled: F,
    write: F,
) {
    use packet::*;
    for (column, expected) in [
        (SPACE, F(space as u64)),
        (VM, F(u64::from(phase.vm))),
        (GENERATION, F::ZERO),
        (INDEX, F(u64::from(index))),
        (
            KEY,
            F(u64::from(index) + (u64::from(phase.vm) << 48) + ((space as u64) << 56)),
        ),
        (CLOCK, F(u64::from(phase.clocks[slot]))),
    ] {
        out.push(port[column].sub(enabled.mul(expected)));
    }
    out.push(port[ENABLED].sub(enabled));
    out.push(port[WRITE].sub(write));
}

#[cfg(test)]
mod tests;
