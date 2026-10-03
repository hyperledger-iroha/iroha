//! Four fixed phases composing the complete successful private STORE64 step.
//!
//! Forty original packets are joined exactly once to the same private history.
//! A fixed memory outcome, caller permission, descriptor copy or public event
//! digest cannot authorize a write. The existing dispatcher owns gas and all
//! architectural completion packets; their clocks follow the memory effects.
// TODO: Join invocation initialization, descriptor publication, every remaining
// opcode and terminal finalized State before registration. This successful-step
// relation rejects traps rather than proving trap results or local allocation.

mod effect;

use super::super::{frame_access, packet, permutation, private_history};
use super::{F, OriginalPackets, Program, bit};
use packet::{
    AFTER, AFTER_TAG, BEFORE, BEFORE_TAG, CLOCK, ENABLED, GENERATION, INDEX, KEY, SPACE, Space, VM,
    WRITE,
};

/// Existing dispatch tuples plus four policies, twelve frame slots and three effects.
pub(super) const PORTS: usize = 40;
/// Fixed semantic phases; their shape never depends on the private opcode.
pub(super) const PHASES: usize = 4;
const WORK: usize = 0;
const WORK_WIDTH: usize = frame_access::WIDTH;
const PACKETS: usize = WORK + WORK_WIDTH;
const CARRY: usize = PACKETS + super::PORTS * packet::WIDTH;
const ADDRESS: usize = 0;
const STORE: usize = ADDRESS + 4;
const SOURCE: usize = STORE + 1;
const COMPLETION: usize = SOURCE + packet::WIDTH;
const POLICY: usize = COMPLETION + 3 * packet::WIDTH;
const PERMITTED: usize = POLICY + 4 * packet::WIDTH;
const ACTIVE: usize = PERMITTED + 1;
const INITIALIZE: usize = ACTIVE + 1;
const RANGE_ERROR: usize = INITIALIZE + 1;
const CARRY_WIDTH: usize = RANGE_ERROR + 1;
/// Full semantic row: workspace, largest phase packet envelope and exact carry.
pub(super) const WIDTH: usize = CARRY + CARRY_WIDTH;
const _: () = assert!(WORK_WIDTH == 1700 && CARRY_WIDTH == 217 && WIDTH == 2463);
const _: () = assert!(super::WIDTH == 1512 && effect::WIDTH == 1604);
const _: () = assert!(ivm::Memory::STACK_SLOP == 0);

/// Protected Memory owners; stack and heap retain the existing descriptor roles.
const POLICY_INDEXES: [u32; 4] = [20, 21, 22, 23];

/// Public padded geometry only. No address, value or success premise is accepted.
#[derive(Clone, Copy)]
pub(super) struct Schedule {
    vm: u8,
    first_clock: u32,
}
impl Schedule {
    pub(super) fn new(vm: u8, first_clock: u32) -> Option<Self> {
        first_clock.checked_add(PORTS as u32 - 1)?;
        Some(Self { vm, first_clock })
    }
    fn clock(self, slot: usize) -> u32 {
        self.first_clock + slot as u32
    }
    fn dispatch(self) -> super::Schedule {
        super::Schedule::new(
            self.vm,
            core::array::from_fn(|slot| self.clock(if slot < 18 { slot } else { slot + 19 })),
        )
        .unwrap()
    }
}

fn port(row: &[F; WIDTH], slot: usize) -> &[F; packet::WIDTH] {
    row[PACKETS + slot * packet::WIDTH..PACKETS + (slot + 1) * packet::WIDTH]
        .try_into()
        .unwrap()
}
fn carry_port(row: &[F; WIDTH], offset: usize) -> &[F; packet::WIDTH] {
    row[CARRY + offset..CARRY + offset + packet::WIDTH]
        .try_into()
        .unwrap()
}
fn link(out: &mut Vec<F>, left: &[F], right: &[F]) {
    assert_eq!(left.len(), right.len());
    out.extend(left.iter().zip(right).map(|(a, b)| a.sub(*b)));
}

/// Borrowed fixed semantic rows of the one committed execution allocation.
pub(super) struct Rows<'a> {
    pub(super) phases: [&'a [F; WIDTH]; PHASES],
}
impl Rows<'_> {
    /// Exactly one original producer for every scheduled clock, including disabled slots.
    fn producer(&self, slot: usize) -> &[F; packet::WIDTH] {
        match slot {
            0..18 => port(self.phases[0], slot),
            18..22 => port(self.phases[1], slot - 18),
            22..34 => port(self.phases[2], slot - 22),
            34..40 => port(self.phases[3], slot - 34),
            _ => unreachable!("closed original STORE packet schedule"),
        }
    }
}

/// Require all semantic phases and all eight history stages of every original port.
/// The enclosing adapter supplies the original contiguous fixed record placement.
pub(super) fn append_residues(
    out: &mut Vec<F>,
    program: &Program,
    schedule: Schedule,
    rows: &Rows<'_>,
    history: &[super::HistoryRow<'_>; PORTS * super::super::PHASES],
    challenges: &permutation::Challenges,
) {
    append_semantics(out, program, schedule, rows);
    for (index, history) in history.iter().enumerate() {
        // Fixed placement belongs to this complete forty-slot record, including
        // disabled packets; repeating or swapping a history stage cannot pass.
        out.push(
            history.fixed[super::super::SLOT]
                .sub(F(u64::from(schedule.clock(index / super::super::PHASES)))),
        );
        for phase in 0..super::super::PHASES {
            out.push(
                history.fixed[super::super::PHASE_OFFSET + phase]
                    .sub(F(u64::from(phase == index % super::super::PHASES))),
            );
        }
        private_history::append_residues(
            out,
            history.current,
            history.next,
            history.aux,
            history.next_aux,
            history.fixed,
            rows.producer(index / super::super::PHASES),
            challenges,
        );
    }
}

fn append_semantics(out: &mut Vec<F>, program: &Program, schedule: Schedule, rows: &Rows<'_>) {
    for pair in rows.phases.windows(2) {
        link(out, &pair[0][CARRY..], &pair[1][CARRY..]);
    }
    for (phase, (&row, used)) in rows.phases.iter().zip([21, 4, 12, 6]).enumerate() {
        for &value in &row[PACKETS + used * packet::WIDTH..CARRY] {
            out.push(value);
        }
        let used_work = [super::WIDTH, 0, frame_access::WIDTH, effect::WIDTH][phase];
        for &value in &row[WORK + used_work..PACKETS] {
            out.push(value);
        }
    }
    let dispatch = rows.phases[0];
    let originals = OriginalPackets::candidate(core::array::from_fn(|i| *port(dispatch, i)));
    let decoded = super::append_residues(
        out,
        program,
        schedule.dispatch(),
        dispatch[..super::WIDTH].try_into().unwrap(),
        &originals,
    );
    link(
        out,
        &dispatch[CARRY + ADDRESS..CARRY + ADDRESS + 4],
        &decoded.store_address,
    );
    out.push(dispatch[CARRY + STORE].sub(decoded.store));
    // Only a complete STORE success or canonical inactive padding belongs here.
    out.push(decoded.store.sub(port(dispatch, 20)[BEFORE]));
    link(out, carry_port(dispatch, SOURCE), decoded.store_value);
    for i in 0..3 {
        link(
            out,
            carry_port(dispatch, COMPLETION + i * packet::WIDTH),
            port(dispatch, 18 + i),
        );
    }
    let policies = rows.phases[1];
    for (i, index) in POLICY_INDEXES.into_iter().enumerate() {
        let original = port(policies, i);
        header(
            out,
            original,
            schedule,
            Space::Owner,
            F::ZERO,
            F(u64::from(index)),
            18 + i,
            policies[CARRY + STORE],
            F::ZERO,
        );
        link(
            out,
            &original[AFTER..AFTER + 8],
            &original[BEFORE..BEFORE + 8],
        );
        for &value in &original[BEFORE + 4..BEFORE + 8] {
            out.push(value);
        }
        out.push(original[BEFORE_TAG]);
        out.push(original[AFTER_TAG]);
        link(
            out,
            carry_port(policies, POLICY + i * packet::WIDTH),
            original,
        );
    }
    let frame = rows.phases[2];
    let decision = frame_access::append_residues(
        out,
        frame_access::Schedule::new(
            schedule.vm,
            8,
            true,
            core::array::from_fn(|i| schedule.clock(22 + i)),
        )
        .unwrap(),
        frame[..frame_access::WIDTH].try_into().unwrap(),
        frame_access::Request {
            selected: frame[CARRY + STORE],
            address: frame[CARRY + ADDRESS..CARRY + ADDRESS + 4]
                .try_into()
                .unwrap(),
        },
        frame_access::Ports {
            active: port(frame, 0),
            descriptors: core::array::from_fn(|i| port(frame, i + 1)),
            initialized: port(frame, 11),
        },
    );
    for (offset, value) in [
        (PERMITTED, decision.permitted),
        (ACTIVE, decision.active_generation),
        (INITIALIZE, decision.initialized_write),
        (RANGE_ERROR, decision.range_error),
    ] {
        out.push(frame[CARRY + offset].sub(value));
    }
    let effects = rows.phases[3];
    out.push(effects[CARRY + PERMITTED].sub(effects[CARRY + STORE]));
    out.push(effects[CARRY + RANGE_ERROR]);
    effect::append_residues(out, schedule, effects);
    for i in 0..3 {
        link(
            out,
            port(effects, 3 + i),
            carry_port(effects, COMPLETION + i * packet::WIDTH),
        );
    }
}

fn header(
    out: &mut Vec<F>,
    p: &[F; packet::WIDTH],
    schedule: Schedule,
    space: Space,
    generation: F,
    index: F,
    clock: usize,
    enabled: F,
    write: F,
) {
    out.push(bit(enabled));
    out.push(bit(write));
    out.push(write.mul(F::ONE.sub(enabled)));
    for (column, expected) in [
        (SPACE, enabled.mul(F(space as u64))),
        (VM, enabled.mul(F(u64::from(schedule.vm)))),
        (GENERATION, enabled.mul(generation)),
        (INDEX, enabled.mul(index)),
        (
            KEY,
            enabled.mul(
                index
                    .add(generation.mul(F(1 << 32)))
                    .add(F((u64::from(schedule.vm) << 48) + ((space as u64) << 56))),
            ),
        ),
        (CLOCK, enabled.mul(F(u64::from(schedule.clock(clock))))),
        (ENABLED, enabled),
        (WRITE, write),
    ] {
        out.push(p[column].sub(expected));
    }
    for &value in p {
        out.push(F::ONE.sub(enabled).mul(value));
    }
}

#[cfg(test)]
mod tests;
