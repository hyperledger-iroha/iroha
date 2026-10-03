//! Successful aligned STORE effects on frame-generation byte initialization.
//!
//! The memory packet and byte log are the same tuples consumed by `memory_store`,
//! not independent success flags. Absolute 16-byte initialization cells normalize
//! the native region-relative bitmaps, including regions starting eight bytes
//! into a cell. Disjoint stack and result regions share one generation namespace.
// TODO: Constrain the fixed frame descriptors/generation through authenticated
// call entry/return and invocation ownership; enforce fresh generations in the
// typed bus. This unregistered conditional bank neither grants memory permission
// nor proves an absent memory effect was correctly refused. Native child-return
// copying into the parent's initialization state remains a separate transition.

use super::{F, bit, memory_store, packet};

/// Prior initialization bits, owned by the same typed state-history packet.
pub(super) const WIDTH: usize = 16;
pub(super) const CONSTRAINTS: usize = 131;

/// Fixed component descriptors, not a caller-supplied substitute for frame authority.
#[derive(Clone, Copy, Debug)]
pub(super) struct FrameRegions {
    /// Inclusive start and exclusive end of the live stack frame.
    pub(super) stack: [u64; 2],
    /// Inclusive start and exclusive end of its separate result table.
    pub(super) results: [u64; 2],
}

/// Public qualification boundary shared with the store transition, not admission.
#[derive(Clone, Copy, Debug)]
pub(super) struct WritePhase {
    vm: u8,
    generation: u16,
    address: u64,
    length: usize,
    memory_clock: u32,
    initialization_clock: u32,
    tracked: bool,
}

impl WritePhase {
    /// Reject malformed or crossing descriptors; valid descriptors still require
    /// the missing owner/lifecycle relation, never a witness-selected Boolean.
    pub(super) fn from_fixed_input(
        vm: u8,
        generation: u16,
        address: u64,
        length: usize,
        memory_clock: u32,
        initialization_clock: u32,
        frame: Option<FrameRegions>,
    ) -> Option<Self> {
        if !matches!(length, 8 | 16)
            || !address.is_multiple_of(length as u64)
            || address >> 4 > u64::from(u32::MAX)
            || initialization_clock <= memory_clock
        {
            return None;
        }
        let end = address.checked_add(length as u64)?;
        let mut tracked = false;
        if let Some(frame) = frame {
            for [start, end] in [frame.stack, frame.results] {
                if start > end || !start.is_multiple_of(8) || !end.is_multiple_of(8) {
                    return None;
                }
            }
            if !(frame.stack[1] - frame.stack[0]).is_multiple_of(16)
                || frame.stack[0] < ivm::Memory::STACK_START
                || frame.stack[1] > ivm::Memory::STACK_START + ivm::Memory::STACK_SIZE
                || (frame.stack[0] < frame.results[1] && frame.results[0] < frame.stack[1])
            {
                return None;
            }
            for region in [frame.stack, frame.results] {
                if address >= region[0] && end <= region[1] {
                    tracked = true;
                } else if address < region[1] && region[0] < end {
                    // No successful native write can cross a frame/table boundary.
                    return None;
                }
            }
        }
        Some(Self {
            vm,
            generation,
            address,
            length,
            memory_clock,
            initialization_clock,
            tracked,
        })
    }
}

/// A disabled effect cannot disclose otherwise inaccessible initialization state.
pub(super) fn witness(before: u16, enabled: bool) -> [F; WIDTH] {
    core::array::from_fn(|byte| F(u64::from(enabled && before & (1 << byte) != 0)))
}

/// Link the existing store packet/log to an exact OR update of initialized bytes.
/// The machine bus owns before-values and limb ranges; STORE owns enable, payload,
/// native failure order and private-mask replacement. No extra enable is supplied.
pub(super) fn append_residues(
    out: &mut Vec<F>,
    phase: WritePhase,
    row: &[F; WIDTH],
    memory: &[F; packet::WIDTH],
    log: &[F; memory_store::WRITE_LOG_WIDTH],
    initialized: &[F; packet::WIDTH],
) {
    use packet::*;
    let start = out.len();
    let committed = memory[ENABLED];
    let enabled = committed.mul(F(u64::from(phase.tracked)));
    out.push(bit(committed));
    header(
        out,
        memory,
        Space::Memory,
        phase,
        0,
        phase.memory_clock,
        committed,
    );
    for value in memory {
        out.push(F::ONE.sub(committed).mul(*value));
    }
    // The byte log selects the same half as the memory packet. This prevents a
    // separately valid store in the other half from initializing these bytes.
    for (actual, expected) in log[..4].iter().zip([
        F(phase.address & 0xffff_ffff),
        F(phase.address >> 32),
        F(phase.length as u64),
        F::ONE,
    ]) {
        out.push(actual.sub(committed.mul(expected)));
    }
    let offset = (phase.address & 15) as usize / 2;
    for limb in 0..8 {
        let expected = if limb < phase.length / 2 {
            memory[AFTER + offset + limb]
        } else {
            F::ZERO
        };
        out.push(log[4 + limb].sub(expected));
    }
    for value in row {
        out.push(bit(*value));
        out.push(F::ONE.sub(enabled).mul(*value));
    }
    header(
        out,
        initialized,
        Space::Initialization,
        phase,
        phase.generation,
        phase.initialization_clock,
        enabled,
    );
    for value in initialized {
        out.push(F::ONE.sub(enabled).mul(*value));
    }
    let (before, after) = initialized_masks(
        row,
        F(u64::from(phase.address & 8 != 0)),
        phase.length == 16,
    );
    out.push(initialized[BEFORE].sub(before));
    out.push(initialized[AFTER].sub(enabled.mul(after)));
    for offset in [BEFORE, AFTER] {
        for limb in 1..8 {
            out.push(initialized[offset + limb]);
        }
    }
    out.push(initialized[BEFORE_TAG]);
    out.push(initialized[AFTER_TAG]);
    debug_assert_eq!(out.len() - start, CONSTRAINTS);
}

/// Exact OR of the selected bytes, shared with native STORE composition.
/// Inputs remain witness fields; owner relations constrain all bits and enable.
pub(super) fn initialized_masks(row: &[F; WIDTH], high_half: F, wide: bool) -> (F, F) {
    row.iter()
        .enumerate()
        .fold((F::ZERO, F::ZERO), |(before, after), (byte, old)| {
            let selected = if wide {
                F::ONE
            } else if byte < 8 {
                F::ONE.sub(high_half)
            } else {
                high_half
            };
            let weight = F(1 << byte);
            (
                before.add(old.mul(weight)),
                after.add(old.add(selected.mul(F::ONE.sub(*old))).mul(weight)),
            )
        })
}

fn header(
    out: &mut Vec<F>,
    port: &[F; packet::WIDTH],
    space: packet::Space,
    phase: WritePhase,
    generation: u16,
    clock: u32,
    enabled: F,
) {
    use packet::*;
    let index = phase.address >> 4;
    for (column, expected) in [
        (SPACE, F(space as u64)),
        (VM, F(u64::from(phase.vm))),
        (GENERATION, F(u64::from(generation))),
        (INDEX, F(index)),
        (
            KEY,
            F(index
                + (u64::from(generation) << 32)
                + (u64::from(phase.vm) << 48)
                + ((space as u64) << 56)),
        ),
        (CLOCK, F(u64::from(clock))),
        (ENABLED, F::ONE),
        (WRITE, F::ONE),
    ] {
        out.push(port[column].sub(enabled.mul(expected)));
    }
}

#[cfg(test)]
mod tests;
