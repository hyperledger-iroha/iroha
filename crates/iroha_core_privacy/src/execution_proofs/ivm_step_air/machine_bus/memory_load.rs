//! Conditional post-read LOAD64 equations, with no request authority.
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
pub(super) const CONSTRAINTS: usize = 159;
const ZERO_INVERSE: usize = 16;
const FULL_INVERSE: usize = 17;
/// Gas, PC and cycles as two u32 limbs each, followed by commit and privacy trap.
pub(super) const CONTROL_WIDTH: usize = 8;
/// Exact physical read-log address limbs, byte length and enabled flag.
pub(super) const READ_LOG_WIDTH: usize = 4;
const COMMIT: usize = 6;
const PRIVACY_TRAP: usize = 7;
/// Every admitted load reads one eight-byte half of a sixteen-byte cell.
const LENGTH: u64 = 8;
/// Native LOAD64 base tariff, debited before this read phase.
const GAS: u64 = 3;

/// Fixed qualification boundary, not an authenticated permission or request.
#[derive(Clone, Copy)]
pub(super) struct ReadPhase {
    vm: u8,
    address: u64,
    /// Memory read, then the destination's value write and its tag write.
    clocks: [u32; 3],
    destination: usize,
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
        clocks: [u32; 3],
        (gas, pc, cycles, stack_top): (u64, u64, u64, u64),
        (zk, base_private): (bool, bool),
    ) -> Option<Self> {
        if wide::opcode(instruction) != wide::memory::LOAD64 {
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
        {
            return None;
        }
        Some(Self {
            vm,
            address,
            clocks,
            destination: wide::rd(instruction),
            stack: address >= ivm::Memory::STACK_START && end <= stack_top,
            gas_after,
            pc,
            cycles,
        })
    }

    /// Private-mask bits of the addressed half of the sixteen-byte cell.
    fn selected_bits(self) -> core::ops::Range<usize> {
        let start = (self.address & 8) as usize;
        start..start + 8
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

/// Constrain the memory tuple and the two native ordered value/tag register tuples.
/// The read port also owns the physical read-log entry; architectural failure
/// cannot suppress it. No packet is synthesized from an unconstrained callback.
pub(super) fn append_residues(
    out: &mut Vec<F>,
    phase: ReadPhase,
    row: &[F; WIDTH],
    read: &[F; packet::WIDTH],
    writes: [&[F; packet::WIDTH]; 2],
    read_log: &[F; READ_LOG_WIDTH],
    control: &[F; CONTROL_WIDTH],
) {
    use packet::*;
    let start = out.len();
    let selected = phase.selected_bits();
    let count = row[selected.clone()].iter().copied().fold(F::ZERO, F::add);
    let (zero, full) = mask_flags(
        out,
        row[..16].try_into().unwrap(),
        read[BEFORE_TAG],
        count,
        F(selected.len() as u64),
        row[ZERO_INVERSE],
        row[FULL_INVERSE],
    );
    let private = full.mul(F(u64::from(phase.stack)));
    let commit = zero.add(private);
    header(
        out,
        read,
        Space::Memory,
        phase,
        (
            0,
            u32::from_le_bytes(
                u64::to_le_bytes(phase.address >> 4)[..4]
                    .try_into()
                    .expect("low-byte width"),
            ),
        ),
        F::ONE,
        F::ZERO,
    );
    for limb in 0..8 {
        out.push(read[AFTER + limb].sub(read[BEFORE + limb]));
    }
    out.push(read[AFTER_TAG].sub(read[BEFORE_TAG]));
    // Native order is the value write, then the tag write. In particular the
    // value write preserves its OLD tag. One atomic value/tag port would change
    // the observable RegEvent history when old and new tags differ.
    let enabled = commit.mul(F(u64::from(phase.destination != 0)));
    // The same address bit that selects the private-mask half selects the payload.
    let half = F((phase.address & 8) / 8);
    for (slot, write) in writes.iter().enumerate() {
        header(
            out,
            write,
            Space::Register,
            phase,
            (
                slot + 1,
                u32::try_from(phase.destination).expect("bounded circuit index"),
            ),
            enabled,
            enabled,
        );
        for value in *write {
            out.push(F::ONE.sub(enabled).mul(*value));
        }
        for limb in 4..8 {
            out.push(write[BEFORE + limb]);
        }
        for limb in 0..8 {
            let expected = if limb >= 4 {
                F::ZERO
            } else if slot == 0 {
                payload_limb(read[BEFORE + limb], read[BEFORE + 4 + limb], half)
            } else {
                write[BEFORE + limb]
            };
            out.push(write[AFTER + limb].sub(enabled.mul(expected)));
        }
        let tag = if slot == 0 {
            write[BEFORE_TAG]
        } else {
            private
        };
        out.push(write[AFTER_TAG].sub(enabled.mul(tag)));
        out.push(bit(write[BEFORE_TAG]));
    }
    // The tag write starts from the value write's result, including the
    // intermediate OLD tag after that value write.
    // The initial before-value/tag is owned by the typed state bus, not here.
    let [value, tag] = writes;
    for limb in 0..8 {
        out.push(enabled.mul(tag[BEFORE + limb].sub(value[AFTER + limb])));
    }
    out.push(enabled.mul(tag[BEFORE_TAG].sub(value[AFTER_TAG])));
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
        F(LENGTH),
        F::ONE,
    ]) {
        out.push(actual.sub(expected));
    }
    out.push(control[COMMIT].sub(commit));
    out.push(control[PRIVACY_TRAP].sub(F::ONE.sub(commit)));
    debug_assert_eq!(out.len() - start, CONSTRAINTS);
}

/// Shared exact private-byte range and canonical zero/full-count equations.
/// The enclosing original-owner bank must constrain the selected count itself.
pub(super) fn mask_flags(
    out: &mut Vec<F>,
    bits: &[F; 16],
    original_mask: F,
    count: F,
    length: F,
    zero_inverse: F,
    full_inverse: F,
) -> (F, F) {
    out.extend(bits.iter().copied().map(bit));
    let mask = bits
        .iter()
        .enumerate()
        .fold(F::ZERO, |sum, (i, x)| sum.add(x.mul(F(1 << i))));
    out.push(original_mask.sub(mask));
    let full_difference = count.sub(length);
    let zero = F::ONE.sub(count.mul(zero_inverse));
    let full = F::ONE.sub(full_difference.mul(full_inverse));
    out.push(bit(zero));
    out.push(bit(full));
    for (value, flag, inverse) in [
        (count, zero, zero_inverse),
        (full_difference, full, full_inverse),
    ] {
        out.push(value.mul(flag));
        out.push(value.mul(inverse).sub(F::ONE.sub(flag)));
        out.push(flag.mul(inverse));
    }
    (zero, full)
}

fn header(
    out: &mut Vec<F>,

    port: &[F; packet::WIDTH],

    space: packet::Space,

    phase: ReadPhase,
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
