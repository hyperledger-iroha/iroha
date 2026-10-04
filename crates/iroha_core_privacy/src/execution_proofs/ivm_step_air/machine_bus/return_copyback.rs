//! Exact successful-return initialization scans and immediate-parent copyback.
//!
//! All 4097 public cell slots are mandatory in the joined bounded relation.
//! Private result endpoints derive each byte mask; no caller mask, initialized
//! count or selected subset may substitute. The source Owner packets are the
//! original lifecycle/result reads, emitted once and shared by every cell bank.
// The callable_lookup component joins original active-generation operands and
// artifact-derived result counts to every cell in the complete fixed scan.
// TODO: Join typed-word checks, gas/fault dispatch and completed-root output
// publication to the complete invocation and its original initialization.
// Failed returns must have no lifecycle/copyback commit, with their native error
// separately proved. No instruction may interleave these atomic commit slots.

use super::{F, bit, packet};
use packet::{
    AFTER, AFTER_TAG, BEFORE, BEFORE_TAG, CLOCK, ENABLED, GENERATION, INDEX, KEY, SPACE, Space, VM,
    WRITE,
};

pub(super) mod native_witness;

/// A 64-KiB result interval shifted by eight bytes spans 4097 absolute cells.
pub(super) const CELLS: usize = 4097;
const START: usize = 0;
const END: usize = START + 36;
const CELL: usize = END + 36;
const COMPARE: usize = CELL + 33;
const COMPARE_WIDTH: usize = 45;
const ACTIVE: usize = COMPARE + 2 * COMPARE_WIDTH;
const PARENT: usize = ACTIVE + 16;
const LENGTH: usize = PARENT + 16;
const LENGTH_INVERSE: usize = LENGTH + 17;
const ACTIVE_INVERSE: usize = LENGTH_INVERSE + 1;
const HAS_PARENT: usize = ACTIVE_INVERSE + 1;
const PARENT_INVERSE: usize = HAS_PARENT + 1;
const CHILD_MASK: usize = PARENT_INVERSE + 1;
const PARENT_MASK: usize = CHILD_MASK + 16;
const LOWER: usize = PARENT_MASK + 16;
const UPPER: usize = LOWER + 1;
const CHILD_ENABLED: usize = UPPER + 1;
const PARENT_ENABLED: usize = CHILD_ENABLED + 1;
/// Private bounded endpoint/comparison/mask witnesses for one fixed cell slot.
pub(super) const WIDTH: usize = PARENT_ENABLED + 1;
/// Exact number of local residues for one fixed cell slot.
pub(super) const CONSTRAINTS: usize = 643;

/// Private return activity is derived from the original lifecycle active packet.
#[derive(Clone, Copy)]
pub(super) struct Schedule {
    vm: u8,
    offset: usize,
    owner_clocks: [u32; 4],
    child_clock: u32,
    parent_clock: u32,
}

/// Enumerate the entire bounded scan. Partial iteration cannot qualify a return.
pub(super) fn schedules(
    vm: u8,
    owner_clocks: [u32; 4],
    first_cell_clock: u32,
) -> Option<impl ExactSizeIterator<Item = Schedule>> {
    let last = first_cell_clock.checked_add((2 * CELLS - 1) as u32)?;
    if owner_clocks
        .iter()
        .any(|clock| (first_cell_clock..=last).contains(clock))
        || owner_clocks
            .iter()
            .enumerate()
            .any(|(i, clock)| owner_clocks[..i].contains(clock))
    {
        return None;
    }
    Some((0..CELLS).map(move |offset| Schedule {
        vm,
        offset,
        owner_clocks,
        child_clock: first_cell_clock + 2 * offset as u32,
        parent_clock: first_cell_clock + 2 * offset as u32 + 1,
    }))
}

/// The same authoritative owner packets and per-cell Initialization producers.
pub(super) struct Ports<'a> {
    pub(super) active: &'a [F; packet::WIDTH],
    pub(super) parent: &'a [F; packet::WIDTH],
    pub(super) result_start: &'a [F; packet::WIDTH],
    pub(super) result_end: &'a [F; packet::WIDTH],
    pub(super) child: &'a [F; packet::WIDTH],
    pub(super) copyback: &'a [F; packet::WIDTH],
}

fn pack(bits: &[F]) -> F {
    bits.iter()
        .enumerate()
        .fold(F::ZERO, |sum, (i, bit)| sum.add(bit.mul(F(1 << i))))
}
fn padded_limb(bits: &[F], limb: usize) -> F {
    (0..16).fold(F::ZERO, |sum, i| {
        sum.add(
            bits.get(limb * 16 + i)
                .copied()
                .unwrap_or(F::ZERO)
                .mul(F(1 << i)),
        )
    })
}

/// Constrain one mandatory public cell slot of a successful atomic return.
pub(super) fn append_residues(
    out: &mut impl crate::execution_proofs::ivm_step_air::residues::Sink,
    schedule: Schedule,
    row: &[F; WIDTH],
    ports: Ports<'_>,
) {
    let residue_start = out.len();
    let selected = ports.active[ENABLED];
    out.push(bit(selected));
    for value in &row[START..LENGTH_INVERSE] {
        out.push(bit(*value));
    }
    let active = pack(&row[ACTIVE..PARENT]);
    let parent = pack(&row[PARENT..LENGTH]);
    let length = pack(&row[LENGTH..LENGTH_INVERSE]);
    let cell = pack(&row[CELL..COMPARE]);
    for (port, index, generation, write, clock) in [
        (ports.active, 0, F::ZERO, true, schedule.owner_clocks[0]),
        (ports.parent, 2, active, false, schedule.owner_clocks[1]),
        (
            ports.result_start,
            8,
            active,
            false,
            schedule.owner_clocks[2],
        ),
        (ports.result_end, 9, active, false, schedule.owner_clocks[3]),
    ] {
        header(
            out,
            port,
            schedule.vm,
            Space::Owner,
            F(index),
            generation,
            selected,
            write,
            clock,
        );
        if !write {
            for limb in 0..4 {
                out.push(port[AFTER + limb].sub(port[BEFORE + limb]));
            }
        } else {
            for limb in 0..4 {
                out.push(port[AFTER + limb].sub(if limb == 0 { parent } else { F::ZERO }));
            }
        }
    }
    for (port, expected) in [(ports.active, active), (ports.parent, parent)] {
        out.push(port[BEFORE].sub(expected));
        for limb in 1..4 {
            out.push(port[BEFORE + limb]);
        }
    }
    out.push(active.mul(row[ACTIVE_INVERSE]).sub(selected));
    out.push(F::ONE.sub(selected).mul(row[ACTIVE_INVERSE]));
    out.push(bit(row[HAS_PARENT]));
    out.push(parent.mul(F::ONE.sub(row[HAS_PARENT])));
    out.push(parent.mul(row[PARENT_INVERSE]).sub(row[HAS_PARENT]));
    out.push(F::ONE.sub(row[HAS_PARENT]).mul(row[PARENT_INVERSE]));
    for (port, endpoint) in [
        (ports.result_start, &row[START..END]),
        (ports.result_end, &row[END..CELL]),
    ] {
        for limb in 0..4 {
            out.push(port[BEFORE + limb].sub(padded_limb(endpoint, limb)));
        }
        out.extend(endpoint[..3].iter().copied());
    }
    // All values are <2^36 or <=2^16, so this equality has no field-wrap alias.
    out.push(
        pack(&row[END..CELL])
            .sub(pack(&row[START..END]))
            .sub(length),
    );
    for lower in &row[LENGTH..LENGTH + 16] {
        out.push(row[LENGTH + 16].mul(*lower));
    }
    // Admitted callable results have one schema root and at least one word,
    // including Unit. Generic native empty regions are not compiled returns.
    out.push(length.mul(row[LENGTH_INVERSE]).sub(selected));
    out.push(F::ONE.sub(selected).mul(row[LENGTH_INVERSE]));
    out.push(
        cell.sub(pack(&row[START + 4..END]))
            .sub(F(schedule.offset as u64)),
    );
    let mut less = [F::ZERO; 2];
    for (half, result) in less.iter_mut().enumerate() {
        let bank = &row[COMPARE + half * COMPARE_WIDTH..COMPARE + (half + 1) * COMPARE_WIDTH];
        let left: [F; 40] = core::array::from_fn(|i| {
            if i == 3 {
                F(half as u64)
            } else if (4..37).contains(&i) {
                row[CELL + i - 4]
            } else {
                F::ZERO
            }
        });
        let right: [F; 40] =
            core::array::from_fn(|i| row[END..CELL].get(i).copied().unwrap_or(F::ZERO));
        for limb in 0..5 {
            let incoming = if limb == 0 {
                F::ZERO
            } else {
                bank[40 + limb - 1]
            };
            out.push(
                pack(&left[limb * 8..(limb + 1) * 8])
                    .sub(pack(&right[limb * 8..(limb + 1) * 8]))
                    .sub(incoming)
                    .sub(pack(&bank[limb * 8..(limb + 1) * 8]))
                    .add(bank[40 + limb].mul(F(256))),
            );
        }
        *result = bank[44];
    }
    for &value in &row[LOWER..WIDTH] {
        out.push(bit(value));
    }
    let first = F(u64::from(schedule.offset == 0));
    out.push(
        row[LOWER].sub(
            selected
                .mul(F::ONE.sub(first.mul(row[START + 3])))
                .mul(less[0]),
        ),
    );
    out.push(row[UPPER].sub(selected.mul(less[1])));
    out.push(row[CHILD_ENABLED].sub(row[LOWER].add(row[UPPER]).sub(row[LOWER].mul(row[UPPER]))));
    out.push(row[PARENT_ENABLED].sub(row[CHILD_ENABLED].mul(row[HAS_PARENT])));
    for (port, generation, enabled, write, clock, mask) in [
        (
            ports.child,
            active,
            row[CHILD_ENABLED],
            false,
            schedule.child_clock,
            &row[CHILD_MASK..PARENT_MASK],
        ),
        (
            ports.copyback,
            parent,
            row[PARENT_ENABLED],
            true,
            schedule.parent_clock,
            &row[PARENT_MASK..LOWER],
        ),
    ] {
        header(
            out,
            port,
            schedule.vm,
            Space::Initialization,
            cell,
            enabled.mul(generation),
            enabled,
            write,
            clock,
        );
        for &value in mask {
            out.push(bit(value));
            out.push(F::ONE.sub(enabled).mul(value));
        }
        out.push(port[BEFORE].sub(pack(mask)));
        for limb in 1..8 {
            out.push(port[BEFORE + limb]);
            out.push(port[AFTER + limb]);
        }
        if write {
            let after = mask
                .iter()
                .enumerate()
                .fold(F::ZERO, |sum, (byte, before)| {
                    let required = if byte < 8 { row[LOWER] } else { row[UPPER] };
                    sum.add(
                        before
                            .add(required)
                            .sub(before.mul(required))
                            .mul(F(1 << byte)),
                    )
                });
            out.push(port[AFTER].sub(enabled.mul(after)));
        } else {
            out.push(port[AFTER].sub(port[BEFORE]));
        }
    }
    for byte in 0..16 {
        let required = if byte < 8 { row[LOWER] } else { row[UPPER] };
        out.push(required.mul(F::ONE.sub(row[CHILD_MASK + byte])));
    }
    debug_assert_eq!(out.len() - residue_start, CONSTRAINTS);
}

/// Original successful-return operands: r10/r11/r31, saved SP, and entry PC.
pub(super) const OPERAND_PORTS: usize = 5;
/// Exact residue count for the once-per-return operand join.
pub(super) const OPERAND_CONSTRAINTS: usize = 207;

/// Fixed read clocks; the complete machine schedule must exclude all aliases.
#[derive(Clone, Copy)]
pub(super) struct OperandSchedule {
    vm: u8,
    clocks: [u32; OPERAND_PORTS],
}
impl OperandSchedule {
    pub(super) fn new(vm: u8, clocks: [u32; OPERAND_PORTS]) -> Option<Self> {
        clocks
            .windows(2)
            .all(|pair| pair[0] < pair[1])
            .then_some(Self { vm, clocks })
    }
}

/// Same original successful lifecycle packet plus original public operand reads.
pub(super) struct OperandPorts<'a> {
    pub(super) active: &'a [F; packet::WIDTH],
    pub(super) packets: [&'a [F; packet::WIDTH]; OPERAND_PORTS],
}

/// Bind successful return operands once, using the same first-cell witness and
/// original active packet constrained by `append_residues` and the lifecycle.
/// The caller must use original columns, not separately supplied matching data.
/// This checks successful values; failed native word/gas/SP ordering is separate.
pub(super) fn append_operand_residues(
    out: &mut impl crate::execution_proofs::ivm_step_air::residues::Sink,
    schedule: OperandSchedule,
    row: &[F; WIDTH],
    callable: &super::callable_lookup::SelectedCallable,
    ports: OperandPorts<'_>,
) {
    let residue_start = out.len();
    let selected = ports.active[ENABLED];
    let active = ports.active[BEFORE];
    for (slot, space, index, generation) in [
        (0, Space::Register, 10, F::ZERO),
        (1, Space::Register, 11, F::ZERO),
        (2, Space::Register, 31, F::ZERO),
        (3, Space::Owner, 10, active),
        (4, Space::Owner, 11, active),
    ] {
        let port = ports.packets[slot];
        header(
            out,
            port,
            schedule.vm,
            space,
            F(index),
            generation,
            selected,
            false,
            schedule.clocks[slot],
        );
        for limb in 0..4 {
            out.push(port[AFTER + limb].sub(port[BEFORE + limb]));
        }
    }
    for limb in 0..4 {
        out.push(ports.packets[0][BEFORE + limb].sub(padded_limb(&row[START..END], limb)));
        out.push(
            ports.packets[1][BEFORE + limb]
                .sub(padded_limb(&row[LENGTH + 3..LENGTH_INVERSE], limb)),
        );
        out.push(ports.packets[2][BEFORE + limb].sub(ports.packets[3][BEFORE + limb]));
        out.push(ports.packets[4][BEFORE + limb].sub(callable.entry_pc()[limb]));
    }
    out.push(
        callable
            .result_words()
            .sub(pack(&row[LENGTH + 3..LENGTH_INVERSE])),
    );
    debug_assert_eq!(out.len() - residue_start, OPERAND_CONSTRAINTS);
}

fn header(
    out: &mut impl crate::execution_proofs::ivm_step_air::residues::Sink,
    port: &[F; packet::WIDTH],
    vm: u8,
    space: packet::Space,
    index: F,
    generation: F,
    enabled: F,
    write: bool,
    clock: u32,
) {
    for (column, expected) in [
        (SPACE, enabled.mul(F(space as u64))),
        (VM, enabled.mul(F(u64::from(vm)))),
        (GENERATION, generation),
        (INDEX, enabled.mul(index)),
        (
            KEY,
            enabled
                .mul(index.add(F((u64::from(vm) << 48) + ((space as u64) << 56))))
                .add(generation.mul(F(1 << 32))),
        ),
        (CLOCK, enabled.mul(F(u64::from(clock)))),
        (ENABLED, enabled),
        (WRITE, enabled.mul(F(u64::from(write)))),
        (BEFORE_TAG, F::ZERO),
        (AFTER_TAG, F::ZERO),
    ] {
        out.push(port[column].sub(expected));
    }
    for offset in [BEFORE, AFTER] {
        for limb in 0..8 {
            out.push(F::ONE.sub(enabled).mul(port[offset + limb]));
            if limb >= 4 {
                out.push(port[offset + limb]);
            }
        }
    }
}

#[cfg(test)]
pub(super) mod tests;
