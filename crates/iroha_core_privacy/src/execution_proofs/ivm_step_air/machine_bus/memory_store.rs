//! Conditional aligned STORE64 transitions, without memory authority.
//!
//! The fixed boundary has passed the gas and public-base checks at an aligned
//! address. Two ordered register reads survive subsequent privacy/access/local failures.
//! Exact memory, byte-log and output-cursor effects occur only on successful store.
// TODO: Connect this unregistered bank to authenticated fetch/address/frame,
// permissions, initialization, private-range allocation and memory-log custody.
// The fixed MemoryOutcome is a component premise, never permission from a witness.

use super::{F, bit, packet, wide};

/// Previous private-byte mask, shared by neither an unbound callback nor a flag.
pub(super) const WIDTH: usize = 16;
pub(super) const CONSTRAINTS: usize = 176;
/// Address limbs, length, enable, four u16 payload limbs, then four zero limbs.
/// The integrated store effect bank shares this twelve-field physical layout.
pub(super) const WRITE_LOG_WIDTH: usize = 12;
/// Gas/PC/cycles/output cursor limbs; commit/privacy/access/local-defer flags.
pub(super) const CONTROL_WIDTH: usize = 12;
const COMMIT: usize = 8;
const PRIVACY_TRAP: usize = 9;
const ACCESS_TRAP: usize = 10;
const LOCAL_DEFER: usize = 11;
/// Every admitted store writes one eight-byte half of a sixteen-byte cell.
const LENGTH: u64 = 8;
/// Native STORE64 base tariff, debited before the operand reads.
const GAS: u64 = 3;

/// A fixed external premise, not an AIR-derived permission or allocation result.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum MemoryOutcome {
    /// All ordinary memory and local allocation checks accept the complete write.
    Ready,
    /// Authoritative memory permissions/frame/output checks refuse the write.
    AccessRefused,
    /// Privacy-range or write tracking allocation refuses before guest mutation.
    AllocationDeferred,
}

/// Qualification input after the early checks, with no initialization authority.
#[derive(Clone, Copy)]
pub(super) struct StorePhase {
    vm: u8,
    address: u64,
    base_value: u64,
    /// Base read, value read, then the memory write.
    clocks: [u32; 3],
    /// Base register, then value register.
    registers: [usize; 2],
    stack: bool,
    gas_after: u64,
    pc: u64,
    cycles: u64,
    output_before: u64,
    output_after: u64,
    outcome: MemoryOutcome,
}

impl StorePhase {
    /// The caller must eventually prove every premise through the whole machine.
    /// Unaligned stores are excluded: their privacy preflight precedes Memory's
    /// alignment error, which this successful-or-refused boundary does not model.
    pub(super) fn from_fixed_input(
        instruction: u32,
        address: u64,
        vm: u8,
        clocks: [u32; 3],
        (gas, pc, cycles, stack_top, output_before): (u64, u64, u64, u64, u64),
        (zk, base_private): (bool, bool),
        outcome: MemoryOutcome,
    ) -> Option<Self> {
        if wide::opcode(instruction) != wide::memory::STORE64 {
            return None;
        }
        let end = address.checked_add(LENGTH)?;
        let gas_after = gas.checked_sub(GAS)?;
        if !zk
            || base_private
            || !address.is_multiple_of(LENGTH)
            || address >> 4 > u64::from(u32::MAX)
            || !clocks.windows(2).all(|pair| pair[0] < pair[1])
            || cycles == u64::MAX
            || output_before > ivm::Memory::OUTPUT_SIZE
        {
            return None;
        }
        let output_after = if address >= ivm::Memory::OUTPUT_START
            && end <= ivm::Memory::OUTPUT_START + ivm::Memory::OUTPUT_SIZE
        {
            let offset = address - ivm::Memory::OUTPUT_START;
            if outcome == MemoryOutcome::Ready && offset < output_before {
                return None;
            }
            output_before.max(offset + LENGTH)
        } else {
            output_before
        };
        Some(Self {
            vm,
            address,
            base_value: address
                .wrapping_sub(i64::cast_unsigned(i64::from(wide::imm14(instruction)))),
            clocks,
            registers: [
                wide::store_base(instruction),
                wide::store_value(instruction),
            ],
            stack: address >= ivm::Memory::STACK_START && end <= stack_top,
            gas_after,
            pc,
            cycles,
            output_before,
            output_after,
            outcome,
        })
    }

    /// Private-mask bits of the addressed half of the sixteen-byte cell.
    fn selected_bits(self) -> core::ops::Range<usize> {
        let first = (self.address & 8) as usize;
        first..first + 8
    }
}

/// Decompose the prior mask only for an enabled memory effect; failed stores
/// cannot acquire a read of the forbidden cell through an unused witness.
pub(super) fn witness(mask: u16, committed: bool) -> [F; WIDTH] {
    core::array::from_fn(|i| F(u64::from(committed && mask & (1 << i) != 0)))
}

/// Bind exact typed reads (including r0), the single memory write and byte log.
/// Call-frame initialization consumes the same write range in the missing owner
/// relation; this component does not replace or authorize that relation.
pub(super) fn append_residues(
    out: &mut Vec<F>,
    phase: StorePhase,
    row: &[F; WIDTH],
    reads: [&[F; packet::WIDTH]; 2],
    memory: &[F; packet::WIDTH],
    write_log: &[F; WRITE_LOG_WIDTH],
    control: &[F; CONTROL_WIDTH],
) {
    use packet::*;
    let start = out.len();
    // Both operand reads are unconditional once this boundary is reached.
    for (slot, read) in reads.iter().enumerate() {
        header(
            out,
            read,
            Space::Register,
            phase,
            (
                slot,
                u32::try_from(phase.registers[slot]).expect("bounded circuit index"),
            ),
            F::ONE,
            F::ZERO,
        );
        for limb in 4..8 {
            out.push(read[BEFORE + limb]);
        }
        for limb in 0..8 {
            out.push(read[AFTER + limb].sub(read[BEFORE + limb]));
        }
        out.push(read[AFTER_TAG].sub(read[BEFORE_TAG]));
        out.push(bit(read[BEFORE_TAG]));
        for column in (BEFORE..BEFORE + 8).chain([BEFORE_TAG]) {
            out.push(read[column].mul(F(u64::from(phase.registers[slot] == 0))));
        }
    }
    for limb in 0..4 {
        out.push(reads[0][BEFORE + limb].sub(F((phase.base_value >> (16 * limb)) & 0xffff)));
    }
    out.push(reads[0][BEFORE_TAG]);
    // Native alias reads both see the same unchanged register, in original order.
    let linked = F(u64::from(phase.registers[0] == phase.registers[1]));
    for column in (BEFORE..BEFORE + 8).chain([BEFORE_TAG]) {
        out.push(linked.mul(reads[0][column].sub(reads[1][column])));
    }
    let value_tag = reads[1][BEFORE_TAG];
    // Exact Boolean truth table: a stack store accepts either tag; outside the
    // stack the stored value must be public. There is no witness-chosen enable.
    let privacy_ok = if phase.stack {
        F::ONE
    } else {
        F::ONE.sub(value_tag)
    };
    let committed = privacy_ok.mul(F(u64::from(phase.outcome == MemoryOutcome::Ready)));
    for value in row {
        out.push(bit(*value));
        out.push(F::ONE.sub(committed).mul(*value));
    }
    let before_mask = row
        .iter()
        .enumerate()
        .fold(F::ZERO, |sum, (i, x)| sum.add(x.mul(F(1 << i))));
    header(
        out,
        memory,
        Space::Memory,
        phase,
        (
            2,
            u32::from_le_bytes(
                u64::to_le_bytes(phase.address >> 4)[..4]
                    .try_into()
                    .expect("low-byte width"),
            ),
        ),
        committed,
        committed,
    );
    for value in memory {
        out.push(F::ONE.sub(committed).mul(*value));
    }
    out.push(memory[BEFORE_TAG].sub(before_mask));
    let selected = phase.selected_bits();
    for limb in 0..8 {
        let expected = payload_limb(
            memory[BEFORE + limb],
            reads[1][BEFORE + limb % 4],
            F(u64::from(phase.address & 8 != 0)),
            limb,
        );
        out.push(memory[AFTER + limb].sub(committed.mul(expected)));
    }
    // The addressed half takes the stored value's tag; the other half keeps its bits.
    let after_mask = row.iter().enumerate().fold(F::ZERO, |sum, (i, old)| {
        let private = if selected.contains(&i) {
            value_tag
        } else {
            *old
        };
        sum.add(private.mul(F(1 << i)))
    });
    out.push(memory[AFTER_TAG].sub(committed.mul(after_mask)));
    // A STORE never records a guest memory read; this log owns only the exact
    // written range and source bytes, followed by its four zero limbs.
    for (actual, expected) in write_log[..4].iter().zip([
        F(phase.address & u64::from(u32::MAX)),
        F(phase.address >> 32),
        F(LENGTH),
        F::ONE,
    ]) {
        out.push(actual.sub(committed.mul(expected)));
    }
    for limb in 0..8 {
        let expected = if limb < 4 {
            reads[1][BEFORE + limb]
        } else {
            F::ZERO
        };
        out.push(write_log[4 + limb].sub(committed.mul(expected)));
    }
    for (offset, before, after) in [
        (0, phase.gas_after, phase.gas_after),
        (2, phase.pc, phase.pc.wrapping_add(4)),
        (4, phase.cycles, phase.cycles + 1),
        (6, phase.output_before, phase.output_after),
    ] {
        for limb in 0..2 {
            let a = F((before >> (32 * limb)) & u64::from(u32::MAX));
            let b = F((after >> (32 * limb)) & u64::from(u32::MAX));
            out.push(control[offset + limb].sub(a.add(committed.mul(b.sub(a)))));
        }
    }
    for (column, expected) in [
        (COMMIT, committed),
        (PRIVACY_TRAP, F::ONE.sub(privacy_ok)),
        (
            ACCESS_TRAP,
            privacy_ok.mul(F(u64::from(phase.outcome == MemoryOutcome::AccessRefused))),
        ),
        (
            LOCAL_DEFER,
            privacy_ok.mul(F(u64::from(
                phase.outcome == MemoryOutcome::AllocationDeferred,
            ))),
        ),
    ] {
        out.push(control[column].sub(expected));
    }
    debug_assert_eq!(out.len() - start, CONSTRAINTS);
}

/// The shared payload selection arithmetic: the addressed half of the cell takes
/// the stored value limb and the other half keeps its prior limb. The caller
/// constrains selection, address, permission and the original source packets;
/// this supplies no owner.
pub(super) fn payload_limb(before: F, value: F, high_half: F, limb: usize) -> F {
    let selected = if limb < 4 {
        F::ONE.sub(high_half)
    } else {
        high_half
    };
    before.add(selected.mul(value.sub(before)))
}

fn header(
    out: &mut Vec<F>,

    port: &[F; packet::WIDTH],

    space: packet::Space,

    phase: StorePhase,
    (slot, index): (usize, u32),

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
