//! Private frame-generation and immediate-parent ownership transitions.
//!
//! The three ports are the exact Owner packets in the ordered/sorted machine
//! bus, not public descriptor digests or host permission decisions. Every
//! successful entry advances a persistent generation counter, so returning and
//! reentering cannot revive a prior generation's initialized bytes.
// TODO: Connect this held bank to the constrained CALL/RETURN dispatcher,
// artifact-owned callable descriptors, initialized-byte range scans and the
// descriptor/access banks. Their successful acceptance must precede these
// atomic commit slots. These equations alone confer no frame or proof authority.

use super::{F, bit, packet};

/// A fixed slot role; original private dispatcher columns select its activity.
#[derive(Clone, Copy, Debug)]
pub(super) enum Transition {
    RootEntry,
    ChildEntry,
    Return,
}

/// Public geometry; neither generation nor a descriptor is public input.
#[derive(Clone, Copy, Debug)]
pub(super) struct Schedule {
    vm: u8,
    transition: Transition,
    clocks: [u32; 3],
}

impl Schedule {
    /// Ordered slots are supplied by the eventual complete fixed dispatcher.
    pub(super) fn new(vm: u8, transition: Transition, clocks: [u32; 3]) -> Option<Self> {
        clocks
            .windows(2)
            .all(|pair| pair[0] < pair[1])
            .then_some(Self {
                vm,
                transition,
                clocks,
            })
    }
}

/// One canonical inverse of the active generation for child entry and return.
pub(super) const WIDTH: usize = 1;
/// Three complete typed headers/payloads plus the exact lifecycle equations.
pub(super) const CONSTRAINTS: usize = 89;

/// Protected Owner indexes. Parent is namespaced by its fresh frame generation.
const ACTIVE: u32 = 0;
const GENERATION_COUNTER: u32 = 1;
const PARENT: u32 = 2;

/// Original typed history ports; callers cannot replace them with success flags.
pub(super) struct Ports<'a> {
    pub(super) counter: &'a [F; packet::WIDTH],
    pub(super) active: &'a [F; packet::WIDTH],
    pub(super) parent: &'a [F; packet::WIDTH],
}

/// Candidate witness only; the evaluator enforces the unique canonical inverse.
pub(super) fn witness(active_generation: u16, transition: Transition) -> [F; WIDTH] {
    [match transition {
        Transition::RootEntry => F::ZERO,
        Transition::ChildEntry | Transition::Return => {
            F(u64::from(active_generation)).inv().unwrap_or(F::ZERO)
        }
    }]
}

/// Bind fresh generations and exact immediate-parent return to the same history.
///
/// The typed bus enforces the u16 limbs and full read/after continuity. Requiring
/// all higher limbs to vanish makes counter wrap unsatisfiable, not a new root.
/// The eventual resource profile must bound lifecycle entries by this namespace
/// or widen the shared key; this bank never silently wraps or reuses a key.
pub(super) fn append_residues(
    out: &mut impl crate::execution_proofs::ivm_step_air::residues::Sink,
    schedule: Schedule,
    row: &[F; WIDTH],
    selection: &[F; 3],
    ports: Ports<'_>,
) {
    use packet::{AFTER, BEFORE};
    let start = out.len();
    // These are the original private dispatcher columns in the joined relation,
    // not public flags or a host predicate. The fixed schedule includes every
    // root-entry, child-entry and return slot, including disabled slots.
    out.extend(selection.iter().copied().map(bit));
    out.push(bit(selection.iter().copied().fold(F::ZERO, F::add)));
    let selected = selection[match schedule.transition {
        Transition::RootEntry => 0,
        Transition::ChildEntry => 1,
        Transition::Return => 2,
    }];
    let entry = !matches!(schedule.transition, Transition::Return);
    let generation = if entry {
        ports.counter[AFTER]
    } else {
        ports.active[BEFORE]
    };
    header(
        out,
        ports.counter,
        schedule,
        0,
        GENERATION_COUNTER,
        F::ZERO,
        entry,
        selected,
    );
    header(
        out,
        ports.active,
        schedule,
        1,
        ACTIVE,
        F::ZERO,
        true,
        selected,
    );
    header(
        out,
        ports.parent,
        schedule,
        2,
        PARENT,
        generation,
        entry,
        selected,
    );
    for port in [ports.counter, ports.active, ports.parent] {
        for offset in [BEFORE, AFTER] {
            out.push(F::ONE.sub(selected).mul(port[offset]));
            for limb in 1..8 {
                out.push(port[offset + limb]);
            }
        }
    }
    out.push(
        ports.counter[AFTER]
            .sub(ports.counter[BEFORE])
            .sub(selected.mul(F(u64::from(entry)))),
    );
    if entry {
        out.push(ports.active[AFTER].sub(generation));
        out.push(ports.parent[BEFORE]);
        out.push(ports.parent[AFTER].sub(ports.active[BEFORE]));
    } else {
        out.push(ports.active[AFTER].sub(ports.parent[BEFORE]));
        out.push(ports.parent[AFTER].sub(ports.parent[BEFORE]));
        // The same persistent counter survives a return, including root return.
        out.push(ports.counter[AFTER].sub(ports.counter[BEFORE]));
    }
    let root = F(u64::from(matches!(
        schedule.transition,
        Transition::RootEntry
    )));
    out.push(F::ONE.sub(selected).mul(row[0]));
    out.push(root.mul(ports.active[BEFORE]));
    out.push(
        root.mul(row[0]).add(
            F::ONE
                .sub(root)
                .mul(ports.active[BEFORE].mul(row[0]).sub(selected)),
        ),
    );
    debug_assert_eq!(out.len() - start, CONSTRAINTS);
}

fn header(
    out: &mut impl crate::execution_proofs::ivm_step_air::residues::Sink,
    port: &[F; packet::WIDTH],
    schedule: Schedule,
    slot: usize,
    index: u32,
    generation: F,
    write: bool,
    selected: F,
) {
    use packet::*;
    for (column, expected) in [
        (SPACE, selected.mul(F(Space::Owner as u64))),
        (VM, selected.mul(F(u64::from(schedule.vm)))),
        (GENERATION, generation),
        (INDEX, selected.mul(F(u64::from(index)))),
        (
            KEY,
            F(u64::from(index) + (u64::from(schedule.vm) << 48) + ((Space::Owner as u64) << 56))
                .mul(selected)
                .add(generation.mul(F(1 << 32))),
        ),
        (CLOCK, selected.mul(F(u64::from(schedule.clocks[slot])))),
        (ENABLED, selected),
        (WRITE, selected.mul(F(u64::from(write)))),
        (BEFORE_TAG, F::ZERO),
        (AFTER_TAG, F::ZERO),
    ] {
        out.push(port[column].sub(expected));
    }
}

#[cfg(test)]
mod tests;
