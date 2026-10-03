//! Private aligned LOAD/STORE ownership and initialized-byte access decisions.
//!
//! This bank reads the same protected Owner and Initialization history packets
//! as the machine relation. No address, descriptor or permission result is fixed
//! public input. It reproduces the native top-frame priority and the root-table
//! ancestry reduction, conditional on the complete lifecycle descriptor bank.
// TODO: Join the original opcode/request dispatcher and authenticated descriptor
// producers before registration. The root-table reduction is sound only after
// child tables are constrained inside the immediate parent's stack. This held
// bank neither grants standalone memory authority nor proves region permissions.

use super::{F, bit, packet};

const ADDRESS: usize = 0;
const END: usize = ADDRESS + 64;
const CARRY: usize = END + 64;
const OWNERS: usize = CARRY + 64;
const COMPARISONS: usize = OWNERS + 10 * 64;
const COMPARE_WIDTH: usize = 68;
const FLAGS: usize = COMPARISONS + 12 * COMPARE_WIDTH;
const LIVE: usize = FLAGS;
const INVERSE: usize = LIVE + 1;
const STACK: usize = INVERSE + 1;
const ARGUMENTS: usize = STACK + 1;
const RESULTS: usize = ARGUMENTS + 1;
const ROOT_ARG_OVERLAP: usize = RESULTS + 1;
const ROOT_RESULT_OVERLAP: usize = ROOT_ARG_OVERLAP + 1;
const BELOW_STACK: usize = ROOT_RESULT_OVERLAP + 1;
const ORDINARY_NO_ARG: usize = BELOW_STACK + 1;
const ORDINARY: usize = ORDINARY_NO_ARG + 1;
const ARG_BRANCH: usize = ORDINARY + 1;
const RESULT_BASE: usize = ARG_BRANCH + 1;
const RESULT_BRANCH: usize = RESULT_BASE + 1;
const FALLBACK: usize = RESULT_BRANCH + 1;
const INITIALIZED_ENABLED: usize = FALLBACK + 1;
const STACK_GOOD: usize = INITIALIZED_ENABLED + 1;
const ACTIVE_ALLOWED: usize = STACK_GOOD + 1;
const ALLOWED: usize = ACTIVE_ALLOWED + 1;
const RANGE_ERROR: usize = ALLOWED + 1;
const INITIALIZED: usize = RANGE_ERROR + 1;
const ALL_SET: usize = INITIALIZED + 16;
/// Private word/comparison witnesses and the derived permission decision.
pub(super) const WIDTH: usize = ALL_SET + 17;
/// Fixed residue count across all four aligned memory-opcode roles.
pub(super) const CONSTRAINTS: usize = 2267;

/// Ten protected descriptor words: live stack/arguments/results, then root tables.
pub(super) const DESCRIPTOR_INDEXES: [u32; 10] = [4, 5, 6, 7, 8, 9, 16, 17, 18, 19];

/// Public opcode-slot geometry. Every role is present in a complete fixed trace.
#[derive(Clone, Copy)]
pub(super) struct Schedule {
    vm: u8,
    bytes: u8,
    write: bool,
    clocks: [u32; 12],
}
impl Schedule {
    pub(super) fn new(vm: u8, bytes: u8, write: bool, clocks: [u32; 12]) -> Option<Self> {
        (matches!(bytes, 8 | 16) && clocks.windows(2).all(|pair| pair[0] < pair[1])).then_some(
            Self {
                vm,
                bytes,
                write,
                clocks,
            },
        )
    }
}

/// Original committed request fields, derived by the complete instruction bank.
pub(super) struct Request<'a> {
    pub(super) selected: F,
    pub(super) address: &'a [F; 4],
}
/// Original packets; all reads are joined into the same ordered machine history.
pub(super) struct Ports<'a> {
    pub(super) active: &'a [F; packet::WIDTH],
    pub(super) descriptors: [&'a [F; packet::WIDTH]; 10],
    pub(super) initialized: &'a [F; packet::WIDTH],
}
/// Exact private decisions for the dispatcher; neither is a prover premise.
pub(super) struct Decision {
    pub(super) permitted: F,
    pub(super) range_error: F,
    /// Same original active-generation owner consumed by descriptor reads.
    pub(super) active_generation: F,
    /// Successful writes in the live stack or result table update this generation.
    pub(super) initialized_write: F,
}

fn pack(bits: &[F]) -> F {
    bits.iter()
        .enumerate()
        .fold(F::ZERO, |sum, (i, value)| sum.add(value.mul(F(1 << i))))
}

fn comparison_operands(row: &[F; WIDTH], index: usize) -> ([F; 64], [F; 64]) {
    let address = row[ADDRESS..END].try_into().unwrap();
    let end = row[END..CARRY].try_into().unwrap();
    let owner = |i: usize| {
        row[OWNERS + i * 64..OWNERS + (i + 1) * 64]
            .try_into()
            .unwrap()
    };
    let stack_start = core::array::from_fn(|i| F((ivm::Memory::STACK_START >> i) & 1));
    match index {
        0 => (address, owner(0)),
        1 => (owner(1), end),
        2 => (address, owner(2)),
        3 => (owner(3), end),
        4 => (address, owner(4)),
        5 => (owner(5), end),
        6 => (address, stack_start),
        7 => (stack_start, end),
        8 => (address, owner(7)),
        9 => (owner(6), end),
        10 => (address, owner(9)),
        11 => (owner(8), end),
        _ => unreachable!(),
    }
}

/// Derive permissions after native alignment checks, before region permission.
/// Failed permission checks still consume authoritative reads, never writes.
pub(super) fn append_residues(
    out: &mut Vec<F>,
    schedule: Schedule,
    row: &[F; WIDTH],
    request: Request<'_>,
    ports: Ports<'_>,
) -> Decision {
    use packet::*;
    let start = out.len();
    let selected = request.selected;
    out.push(bit(selected));
    for (limb, &value) in request.address.iter().enumerate() {
        out.push(value.sub(pack(&row[ADDRESS + limb * 16..ADDRESS + (limb + 1) * 16])));
        out.push(F::ONE.sub(selected).mul(value));
    }
    for &value in &row[ADDRESS..COMPARISONS] {
        out.push(bit(value));
    }
    for bit_index in 0..64 {
        let carry_in = if bit_index == 0 {
            F::ZERO
        } else {
            row[CARRY + bit_index - 1]
        };
        out.push(
            row[ADDRESS + bit_index]
                .add(F((u64::from(schedule.bytes) >> bit_index) & 1))
                .add(carry_in)
                .sub(row[END + bit_index])
                .sub(row[CARRY + bit_index].mul(F(2))),
        );
    }
    // Native aligned word opcodes reach this bank only after this exact check.
    for bit_index in 0..4 {
        out.push(selected.mul(row[ADDRESS + bit_index]).mul(F(u64::from(
            bit_index < schedule.bytes.trailing_zeros() as usize,
        ))));
    }
    read_header(out, ports.active, schedule, 0, 0, F::ZERO, selected);
    let active = ports.active[BEFORE];
    for limb in 1..4 {
        out.push(ports.active[BEFORE + limb]);
    }
    let live = row[LIVE];
    out.push(bit(live));
    out.push(active.mul(F::ONE.sub(live)));
    out.push(active.mul(row[INVERSE]).sub(live));
    out.push(F::ONE.sub(live).mul(row[INVERSE]));
    for (i, port) in ports.descriptors.iter().enumerate() {
        read_header(
            out,
            port,
            schedule,
            i + 1,
            DESCRIPTOR_INDEXES[i],
            if i < 6 { active } else { F::ZERO },
            live,
        );
        for limb in 0..4 {
            out.push(port[BEFORE + limb].sub(pack(
                &row[OWNERS + i * 64 + limb * 16..OWNERS + i * 64 + (limb + 1) * 16],
            )));
        }
    }
    let mut less = [F::ZERO; 12];
    for (i, result) in less.iter_mut().enumerate() {
        let start = COMPARISONS + i * COMPARE_WIDTH;
        let bank = &row[start..start + COMPARE_WIDTH];
        out.extend(bank.iter().copied().map(bit));
        let (left, right) = comparison_operands(row, i);
        for limb in 0..4 {
            let borrow_in = if limb == 0 {
                F::ZERO
            } else {
                bank[64 + limb - 1]
            };
            out.push(
                pack(&left[limb * 16..(limb + 1) * 16])
                    .sub(pack(&right[limb * 16..(limb + 1) * 16]))
                    .sub(borrow_in)
                    .sub(pack(&bank[limb * 16..(limb + 1) * 16]))
                    .add(bank[64 + limb].mul(F(1 << 16))),
            );
        }
        *result = bank[67];
    }
    for flag in STACK..=RANGE_ERROR {
        out.push(bit(row[flag]));
    }
    for (target, expected) in [
        (STACK, F::ONE.sub(less[0]).mul(F::ONE.sub(less[1]))),
        (ARGUMENTS, F::ONE.sub(less[2]).mul(F::ONE.sub(less[3]))),
        (RESULTS, F::ONE.sub(less[4]).mul(F::ONE.sub(less[5]))),
        (ROOT_ARG_OVERLAP, less[8].mul(less[9])),
        (ROOT_RESULT_OVERLAP, less[10].mul(less[11])),
        (BELOW_STACK, less[6].mul(F::ONE.sub(less[7]))),
        (
            ORDINARY_NO_ARG,
            row[BELOW_STACK].mul(F::ONE.sub(row[ROOT_ARG_OVERLAP])),
        ),
        (
            ORDINARY,
            row[ORDINARY_NO_ARG].mul(F::ONE.sub(row[ROOT_RESULT_OVERLAP])),
        ),
        (ARG_BRANCH, F::ONE.sub(row[STACK]).mul(row[ARGUMENTS])),
        (
            RESULT_BASE,
            F::ONE.sub(row[STACK]).mul(F::ONE.sub(row[ARGUMENTS])),
        ),
        (RESULT_BRANCH, row[RESULT_BASE].mul(row[RESULTS])),
        (FALLBACK, row[RESULT_BASE].mul(F::ONE.sub(row[RESULTS]))),
        (
            INITIALIZED_ENABLED,
            live.mul(F::ONE.sub(row[CARRY + 63]))
                .mul(row[STACK])
                .mul(F(u64::from(!schedule.write))),
        ),
    ] {
        out.push(row[target].sub(expected));
    }
    let initialized_enabled = row[INITIALIZED_ENABLED];
    initialized_header(
        out,
        ports.initialized,
        schedule,
        active,
        pack(&row[ADDRESS + 4..END]),
        initialized_enabled,
    );
    for &value in &row[INITIALIZED..ALL_SET] {
        out.push(bit(value));
        out.push(F::ONE.sub(initialized_enabled).mul(value));
    }
    out.push(ports.initialized[BEFORE].sub(pack(&row[INITIALIZED..ALL_SET])));
    for limb in 1..8 {
        out.push(ports.initialized[BEFORE + limb]);
    }
    out.push(row[ALL_SET].sub(F::ONE));
    for byte in 0..16 {
        let chosen = if schedule.bytes == 16 {
            F::ONE
        } else if byte < 8 {
            F::ONE.sub(row[ADDRESS + 3])
        } else {
            row[ADDRESS + 3]
        };
        let required = initialized_enabled.mul(chosen);
        let initialized = row[INITIALIZED + byte];
        out.push(
            row[ALL_SET + byte + 1]
                .sub(row[ALL_SET + byte].mul(F::ONE.sub(required).add(required.mul(initialized)))),
        );
    }
    let stack_good = if schedule.write {
        row[STACK]
    } else {
        row[STACK].mul(row[ALL_SET + 16])
    };
    out.push(row[STACK_GOOD].sub(stack_good));
    let branch_good = row[STACK_GOOD]
        .add(if schedule.write {
            row[RESULT_BRANCH]
        } else {
            row[ARG_BRANCH]
        })
        .add(row[FALLBACK].mul(row[ORDINARY]));
    out.push(row[ACTIVE_ALLOWED].sub(F::ONE.sub(row[CARRY + 63]).mul(branch_good)));
    out.push(row[ALLOWED].sub(selected.sub(live).add(live.mul(row[ACTIVE_ALLOWED]))));
    out.push(row[RANGE_ERROR].sub(live.mul(row[CARRY + 63])));
    debug_assert_eq!(out.len() - start, CONSTRAINTS);
    Decision {
        permitted: row[ALLOWED],
        range_error: row[RANGE_ERROR],
        active_generation: active,
        initialized_write: live
            .mul(row[STACK].add(row[RESULT_BRANCH]))
            .mul(F(u64::from(schedule.write))),
    }
}

fn read_header(
    out: &mut Vec<F>,
    port: &[F; packet::WIDTH],
    schedule: Schedule,
    slot: usize,
    index: u32,
    generation: F,
    enabled: F,
) {
    use packet::*;
    for (column, expected) in [
        (SPACE, enabled.mul(F(Space::Owner as u64))),
        (VM, enabled.mul(F(u64::from(schedule.vm)))),
        (GENERATION, generation),
        (INDEX, enabled.mul(F(u64::from(index)))),
        (
            KEY,
            enabled
                .mul(F(u64::from(index)
                    + (u64::from(schedule.vm) << 48)
                    + ((Space::Owner as u64) << 56)))
                .add(generation.mul(F(1 << 32))),
        ),
        (CLOCK, enabled.mul(F(u64::from(schedule.clocks[slot])))),
        (ENABLED, enabled),
        (WRITE, F::ZERO),
        (BEFORE_TAG, F::ZERO),
        (AFTER_TAG, F::ZERO),
    ] {
        out.push(port[column].sub(expected));
    }
    for limb in 0..8 {
        out.push(port[AFTER + limb].sub(port[BEFORE + limb]));
        out.push(F::ONE.sub(enabled).mul(port[BEFORE + limb]));
        if limb >= 4 {
            out.push(port[BEFORE + limb]);
        }
    }
}

fn initialized_header(
    out: &mut Vec<F>,
    port: &[F; packet::WIDTH],
    schedule: Schedule,
    active: F,
    index: F,
    enabled: F,
) {
    use packet::*;
    let generation = enabled.mul(active);
    for (column, expected) in [
        (SPACE, enabled.mul(F(Space::Initialization as u64))),
        (VM, enabled.mul(F(u64::from(schedule.vm)))),
        (GENERATION, generation),
        (INDEX, enabled.mul(index)),
        (
            KEY,
            enabled
                .mul(index.add(F(
                    (u64::from(schedule.vm) << 48) + ((Space::Initialization as u64) << 56)
                )))
                .add(generation.mul(F(1 << 32))),
        ),
        (CLOCK, enabled.mul(F(u64::from(schedule.clocks[11])))),
        (ENABLED, enabled),
        (WRITE, F::ZERO),
        (BEFORE_TAG, F::ZERO),
        (AFTER_TAG, F::ZERO),
    ] {
        out.push(port[column].sub(expected));
    }
    for limb in 0..8 {
        out.push(port[AFTER + limb].sub(port[BEFORE + limb]));
        out.push(F::ONE.sub(enabled).mul(port[BEFORE + limb]));
    }
}

#[cfg(test)]
pub(super) mod tests;
