//! Successful V1 frame descriptor publication through original private ports.
//!
//! Root entry uses the original Memory stack-top Owner word. Child entry uses
//! the original public-tagged r31 and immediate-parent stack Owner words. Exact
//! callable fields are borrowed from the program relation, not a host digest.
// TODO: Join authenticated callable lookup and per-word role validation, root
// table public-memory scans, child argument initialization scans, call gas and
// actual success/fault dispatch. This bank constrains successful publication;
// a prover cannot be allowed to select an arbitrary successful-entry activation.

use super::{F, bit, packet};
use packet::{
    AFTER, AFTER_TAG, BEFORE, BEFORE_TAG, CLOCK, ENABLED, GENERATION, INDEX, KEY, SPACE, Space, VM,
    WRITE,
};

const SP: usize = 0;
const ARGUMENT: usize = 1;
const ARGUMENT_COUNT: usize = 2;
const RESULT: usize = 3;
const RESULT_COUNT: usize = 4;
const FRAME: usize = 5;
const ENTRY: usize = 6;
const STACK_TOP: usize = 7;
const STACK_START: usize = 8;
const ARGUMENT_END: usize = 9;
const RESULT_END: usize = 10;
const PARENT_START: usize = 11;
const PARENT_END: usize = 12;
const HEAP_END: usize = 13;
const WORDS: usize = 14;
const CARRIES: usize = WORDS * 64;
const COMPARISONS: usize = CARRIES + 12;
const COMPARE_WIDTH: usize = 68;
const ARGUMENT_NONZERO: usize = COMPARISONS + 16 * COMPARE_WIDTH;
const ARGUMENT_INVERSE: usize = ARGUMENT_NONZERO + 1;
const RESULT_INVERSE: usize = ARGUMENT_INVERSE + 1;
/// Private bit/range witnesses; all semantic words come from original ports.
pub(super) const WIDTH: usize = RESULT_INVERSE + 1;
/// Fixed polynomial residue count for both root and child publication slots.
pub(super) const CONSTRAINTS: usize = 4093;

/// All protected reads/writes except the already-owned lifecycle active packet.
pub(super) const PORTS: usize = 21;
/// Per-generation publication order: six region endpoints, saved SP and entry PC.
pub(super) const DESCRIPTOR_INDEXES: [u32; 8] = [4, 5, 6, 7, 8, 9, 10, 11];

/// Padded public slot role; actual activation remains private dispatcher state.
#[derive(Clone, Copy)]
pub(super) struct Schedule {
    vm: u8,
    root: bool,
    clocks: [u32; PORTS],
}
impl Schedule {
    pub(super) fn new(vm: u8, root: bool, clocks: [u32; PORTS]) -> Option<Self> {
        clocks
            .windows(2)
            .all(|pair| pair[0] < pair[1])
            .then_some(Self { vm, root, clocks })
    }
}

/// Original selected callable fields owned by authenticated artifact lookup.
pub(super) struct Callable<'a> {
    pub(super) entry_pc: &'a [F; 4],
    pub(super) frame_bytes: F,
    pub(super) argument_words: F,
    pub(super) result_words: F,
}
/// Original lifecycle packet plus exact ordered semantic producer/read packets.
pub(super) struct Ports<'a> {
    /// Same active-generation write constrained by `frame_lifecycle`.
    pub(super) active: &'a [F; packet::WIDTH],
    /// r10/r11/r12/r13/r31, stack-top, heap-end, parent bounds, eight fresh
    /// descriptors, then four root-table globals. Disabled slots are zero.
    pub(super) packets: [&'a [F; packet::WIDTH]; PORTS],
}

fn bits(row: &[F; WIDTH], word: usize) -> &[F] {
    &row[word * 64..(word + 1) * 64]
}
fn pack(bits: &[F]) -> F {
    bits.iter()
        .enumerate()
        .fold(F::ZERO, |sum, (i, bit)| sum.add(bit.mul(F(1 << i))))
}
fn limb(row: &[F; WIDTH], word: usize, index: usize) -> F {
    pack(&bits(row, word)[index * 16..(index + 1) * 16])
}
fn constant(value: u64) -> [F; 64] {
    core::array::from_fn(|i| F((value >> i) & 1))
}

fn comparison_operands(row: &[F; WIDTH], index: usize) -> ([F; 64], [F; 64]) {
    let word = |index: usize| bits(row, index).try_into().unwrap();
    match index {
        0 => (word(STACK_TOP), word(SP)),
        1 => (word(STACK_START), constant(ivm::Memory::STACK_START)),
        2 => (word(ARGUMENT), word(RESULT_END)),
        3 => (word(RESULT), word(ARGUMENT_END)),
        4 => (word(STACK_START), word(ARGUMENT_END)),
        5 => (word(ARGUMENT), word(SP)),
        6 => (word(STACK_START), word(RESULT_END)),
        7 => (word(RESULT), word(SP)),
        8 => (word(ARGUMENT), word(PARENT_START)),
        9 => (word(PARENT_END), word(ARGUMENT_END)),
        10 => (word(RESULT), word(PARENT_START)),
        11 => (word(PARENT_END), word(RESULT_END)),
        12 => (word(ARGUMENT), constant(ivm::Memory::HEAP_START)),
        13 => (word(HEAP_END), word(ARGUMENT_END)),
        14 => (word(RESULT), constant(ivm::Memory::HEAP_START)),
        15 => (word(HEAP_END), word(RESULT_END)),
        _ => unreachable!(),
    }
}

/// Constrain every successful descriptor to original call operands and bounds.
/// This neither selects success nor erases native failed-call register/gas work.
pub(super) fn append_residues(
    out: &mut Vec<F>,
    schedule: Schedule,
    row: &[F; WIDTH],
    selected: F,
    callable: Callable<'_>,
    ports: Ports<'_>,
) {
    let start = out.len();
    out.push(bit(selected));
    let root = F(u64::from(schedule.root));
    let child = F::ONE.sub(root);
    let root_enabled = selected.mul(root);
    let child_enabled = selected.mul(child);
    // The shared lifecycle bank owns all 26 fields and fresh generation logic.
    // Requiring the same activation prevents a separately enabled descriptor.
    out.push(ports.active[ENABLED].sub(selected));
    let generation = ports.active[AFTER];
    let parent = ports.active[BEFORE];
    for value in &row[..CARRIES] {
        out.push(bit(*value));
        out.push(F::ONE.sub(selected).mul(*value));
    }
    for (i, register) in [10_u32, 11, 12, 13, 31].into_iter().enumerate() {
        let enabled = if i == 4 { child_enabled } else { selected };
        read(
            out,
            ports.packets[i],
            schedule,
            i,
            Space::Register,
            register,
            F::ZERO,
            enabled,
        );
        let word = [ARGUMENT, ARGUMENT_COUNT, RESULT, RESULT_COUNT, SP][i];
        for limb_index in 0..4 {
            let actual = limb(row, word, limb_index);
            let expected = if i == 4 {
                ports.packets[i][BEFORE + limb_index]
                    .add(root.mul(ports.packets[5][BEFORE + limb_index]))
            } else {
                ports.packets[i][BEFORE + limb_index]
            };
            out.push(actual.sub(expected));
        }
    }
    read(
        out,
        ports.packets[5],
        schedule,
        5,
        Space::Owner,
        20,
        F::ZERO,
        selected,
    );
    read(
        out,
        ports.packets[6],
        schedule,
        6,
        Space::Owner,
        21,
        F::ZERO,
        root_enabled,
    );
    for (slot, index, word, enabled, owner_generation) in [
        (5, 20, STACK_TOP, selected, F::ZERO),
        (6, 21, HEAP_END, root_enabled, F::ZERO),
        (7, 4, PARENT_START, child_enabled, child.mul(parent)),
        (8, 5, PARENT_END, child_enabled, child.mul(parent)),
    ] {
        if slot >= 7 {
            read(
                out,
                ports.packets[slot],
                schedule,
                slot,
                Space::Owner,
                index,
                owner_generation,
                enabled,
            );
        }
        for limb_index in 0..4 {
            out.push(limb(row, word, limb_index).sub(ports.packets[slot][BEFORE + limb_index]));
        }
    }
    for limb_index in 0..4 {
        out.push(limb(row, ENTRY, limb_index).sub(callable.entry_pc[limb_index]));
    }
    out.push(pack(&bits(row, FRAME)[..32]).sub(callable.frame_bytes));
    out.push(pack(&bits(row, ARGUMENT_COUNT)[..14]).sub(callable.argument_words));
    out.push(pack(&bits(row, RESULT_COUNT)[..14]).sub(callable.result_words));
    // Exact inclusive power-of-two caps. A set top bit requires all lower bits zero.
    for (word, cap_bit) in [(FRAME, 22), (ARGUMENT_COUNT, 13), (RESULT_COUNT, 13)] {
        for &value in &bits(row, word)[cap_bit + 1..] {
            out.push(value);
        }
        for &value in &bits(row, word)[..cap_bit] {
            out.push(bits(row, word)[cap_bit].mul(value));
        }
    }
    for (word, alignment_bits) in [(SP, 3), (ARGUMENT, 3), (RESULT, 3), (FRAME, 4), (ENTRY, 2)] {
        out.extend(bits(row, word)[..alignment_bits].iter().copied());
    }
    let argument_count = pack(&bits(row, ARGUMENT_COUNT)[..14]);
    let result_count = pack(&bits(row, RESULT_COUNT)[..14]);
    let argument_nonzero = row[ARGUMENT_NONZERO];
    out.push(bit(argument_nonzero));
    out.push(argument_count.mul(F::ONE.sub(argument_nonzero)));
    out.push(
        argument_count
            .mul(row[ARGUMENT_INVERSE])
            .sub(argument_nonzero),
    );
    out.push(F::ONE.sub(argument_nonzero).mul(row[ARGUMENT_INVERSE]));
    out.push(result_count.mul(row[RESULT_INVERSE]).sub(selected));
    out.push(F::ONE.sub(selected).mul(row[RESULT_INVERSE]));
    for limb_index in 0..4 {
        out.push(
            F::ONE
                .sub(argument_nonzero)
                .mul(limb(row, ARGUMENT, limb_index)),
        );
    }
    out.extend(row[CARRIES..COMPARISONS].iter().copied().map(bit));
    for operation in 0..3 {
        for limb_index in 0..4 {
            let carry = &row[CARRIES + operation * 4..CARRIES + (operation + 1) * 4];
            let incoming = if limb_index == 0 {
                F::ZERO
            } else {
                carry[limb_index - 1]
            };
            let residual = if operation == 0 {
                limb(row, SP, limb_index)
                    .sub(limb(row, FRAME, limb_index))
                    .sub(incoming)
                    .sub(limb(row, STACK_START, limb_index))
                    .add(carry[limb_index].mul(F(1 << 16)))
            } else {
                let (base, count, end) = if operation == 1 {
                    (ARGUMENT, ARGUMENT_COUNT, ARGUMENT_END)
                } else {
                    (RESULT, RESULT_COUNT, RESULT_END)
                };
                let shifted = core::array::from_fn::<_, 16, _>(|bit_index| {
                    let bit_index = limb_index * 16 + bit_index;
                    if bit_index < 3 {
                        F::ZERO
                    } else {
                        bits(row, count)[bit_index - 3]
                    }
                });
                limb(row, base, limb_index)
                    .add(pack(&shifted))
                    .add(incoming)
                    .sub(limb(row, end, limb_index))
                    .sub(carry[limb_index].mul(F(1 << 16)))
            };
            out.push(residual);
        }
        out.push(row[CARRIES + operation * 4 + 3]);
    }
    let mut less = [F::ZERO; 16];
    for (index, result) in less.iter_mut().enumerate() {
        let bank =
            &row[COMPARISONS + index * COMPARE_WIDTH..COMPARISONS + (index + 1) * COMPARE_WIDTH];
        let (left, right) = comparison_operands(row, index);
        out.extend(bank.iter().copied().map(bit));
        for limb_index in 0..4 {
            let incoming = if limb_index == 0 {
                F::ZERO
            } else {
                bank[64 + limb_index - 1]
            };
            out.push(
                pack(&left[limb_index * 16..(limb_index + 1) * 16])
                    .sub(pack(&right[limb_index * 16..(limb_index + 1) * 16]))
                    .sub(incoming)
                    .sub(pack(&bank[limb_index * 16..(limb_index + 1) * 16]))
                    .add(bank[64 + limb_index].mul(F(1 << 16))),
            );
        }
        *result = bank[67];
    }
    out.push(selected.mul(less[0])); // SP <= actual Memory stack top.
    out.push(selected.mul(less[1])); // Checked frame start >= STACK_START.
    out.push(selected.mul(less[2]).mul(less[3])); // Exact argument/result non-overlap.
    for (left, right) in [(4, 5), (6, 7)] {
        out.push(root_enabled.mul(less[left]).mul(less[right]));
    }
    for limb_index in 0..4 {
        out.push(child_enabled.mul(limb(row, SP, limb_index).sub(limb(
            row,
            PARENT_START,
            limb_index,
        ))));
    }
    for index in [8, 9] {
        out.push(child_enabled.mul(argument_nonzero).mul(less[index]));
    }
    for index in [10, 11] {
        out.push(child_enabled.mul(less[index]));
    }
    for index in [12, 13] {
        out.push(root_enabled.mul(argument_nonzero).mul(less[index]));
    }
    for index in [14, 15] {
        out.push(root_enabled.mul(less[index]));
    }
    for (i, word) in [
        STACK_START,
        SP,
        ARGUMENT,
        ARGUMENT_END,
        RESULT,
        RESULT_END,
        SP,
        ENTRY,
    ]
    .into_iter()
    .enumerate()
    {
        write(
            out,
            ports.packets[9 + i],
            schedule,
            9 + i,
            DESCRIPTOR_INDEXES[i],
            generation,
            selected,
            row,
            word,
            true,
        );
    }
    for (i, word) in [ARGUMENT, ARGUMENT_END, RESULT, RESULT_END]
        .into_iter()
        .enumerate()
    {
        write(
            out,
            ports.packets[17 + i],
            schedule,
            17 + i,
            16 + i as u32,
            F::ZERO,
            root_enabled,
            row,
            word,
            false,
        );
    }
    debug_assert_eq!(out.len() - start, CONSTRAINTS);
}

fn header(
    out: &mut Vec<F>,
    port: &[F; packet::WIDTH],
    schedule: Schedule,
    slot: usize,
    space: packet::Space,
    index: u32,
    generation: F,
    enabled: F,
    write: bool,
) {
    for (column, expected) in [
        (SPACE, enabled.mul(F(space as u64))),
        (VM, enabled.mul(F(u64::from(schedule.vm)))),
        (GENERATION, generation),
        (INDEX, enabled.mul(F(u64::from(index)))),
        (
            KEY,
            enabled
                .mul(F(u64::from(index)
                    + (u64::from(schedule.vm) << 48)
                    + ((space as u64) << 56)))
                .add(generation.mul(F(1 << 32))),
        ),
        (CLOCK, enabled.mul(F(u64::from(schedule.clocks[slot])))),
        (ENABLED, enabled),
        (WRITE, enabled.mul(F(u64::from(write)))),
        (BEFORE_TAG, F::ZERO),
        (AFTER_TAG, F::ZERO),
    ] {
        out.push(port[column].sub(expected));
    }
    for offset in [BEFORE, AFTER] {
        for limb_index in 0..8 {
            out.push(F::ONE.sub(enabled).mul(port[offset + limb_index]));
            if limb_index >= 4 {
                out.push(port[offset + limb_index]);
            }
        }
    }
}
fn read(
    out: &mut Vec<F>,
    port: &[F; packet::WIDTH],
    schedule: Schedule,
    slot: usize,
    space: packet::Space,
    index: u32,
    generation: F,
    enabled: F,
) {
    header(
        out, port, schedule, slot, space, index, generation, enabled, false,
    );
    for limb_index in 0..4 {
        out.push(port[AFTER + limb_index].sub(port[BEFORE + limb_index]));
    }
}
fn write(
    out: &mut Vec<F>,
    port: &[F; packet::WIDTH],
    schedule: Schedule,
    slot: usize,
    index: u32,
    generation: F,
    enabled: F,
    row: &[F; WIDTH],
    word: usize,
    fresh: bool,
) {
    header(
        out,
        port,
        schedule,
        slot,
        Space::Owner,
        index,
        generation,
        enabled,
        true,
    );
    for limb_index in 0..4 {
        out.push(port[AFTER + limb_index].sub(enabled.mul(limb(row, word, limb_index))));
        // Root-global endpoints may overwrite the last completed root's values.
        // Fresh per-generation cells must never inherit a previous frame.
        out.push(F(u64::from(fresh)).mul(port[BEFORE + limb_index]));
    }
}

#[cfg(test)]
mod tests;
