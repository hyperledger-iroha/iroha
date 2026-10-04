//! Initialized scalar CALL arguments and exact ordered gas through completion.
//!
//! All47 existing producers remain. Public padded argument slots own early and
//! repeated initialization reads, NODE/WORD gas and original public memory halves.
//! Flat Unit/Bool/exact nominal Error argument forests only; recursive nodes refuse.
//! A final gas-owner read binds the last debit before descriptor publication.
// TODO: Complete pointer roles, allocation/preflight, failed-call effects,
// full native8192-word geometry, root/return/copyback and finalized invocation
// authority before any capability or production verifier may be activated.

use super::{F, Program, packet, permutation, private_history};
use ivm_abi::{call::CallTypeNodeV1, entrypoint::EntrypointValueKindV1};
use packet::{
    AFTER, AFTER_TAG, BEFORE, BEFORE_TAG, CLOCK, ENABLED, GENERATION, INDEX, KEY, SPACE, Space, VM,
    WRITE,
};

const ADDRESS: usize = 0;
const ADD_CARRY: usize = ADDRESS + 64;
const INITIALIZED: usize = ADD_CARRY + 4;
const PRIVATE: usize = INITIALIZED + 16;
const VALUE: usize = PRIVATE + 16;
const GAS: usize = VALUE + 64;
const BORROW: usize = GAS + 64;
const NODE_GAS: usize = BORROW + 4;
const NODE_BORROW: usize = NODE_GAS + 64;
const WORD_WIDTH: usize = NODE_BORROW + 4;
const WORD_PORTS: usize = 5;
const EARLY: usize = 0;
const NODE: usize = 1;
const DEBIT: usize = 2;
const LATE: usize = 3;
const MEMORY: usize = 4;
const _: () = assert!(WORD_WIDTH == 300 && WORD_PORTS * packet::WIDTH == 130);

const MAX_PLAN_WORDS: usize = (super::super::super::MAX_PACKETS - super::PORTS - 1) / WORD_PORTS;
const ELIGIBILITY_LIMBS: usize = super::super::MAX_WORDS.div_ceil(64);
const _: () = assert!(MAX_PLAN_WORDS == 3267 && MAX_PLAN_WORDS * 256 <= u32::MAX as usize);

/// Public-only shape derived once from every original admitted callable.
///
/// Fixed u32 prefix offsets and packed eligibility bits allocate no heap storage,
/// including on clone. The complete plan occupies 13,088 bytes on 64-bit hosts;
/// the much larger semantic witness and history never live in this structure.
#[derive(Clone)]
pub(super) struct Shape {
    words: usize,
    supported: [u64; ELIGIBILITY_LIMBS],
    errors: [u32; MAX_PLAN_WORDS + 1],
}
impl Shape {
    pub(super) fn new(program: &Program) -> Option<Self> {
        let callables = &program.artifact().contract_interface().callables;
        if callables.len() > super::super::MAX_WORDS {
            return None;
        }
        let mut shape = Self {
            words: 0,
            supported: [0; ELIGIBILITY_LIMBS],
            errors: [0; MAX_PLAN_WORDS + 1],
        };
        for (index, callable) in callables.iter().enumerate() {
            shape.words = shape
                .words
                .max(program.callables().argument_word_count(index)?);
            if callable.arguments.nodes.iter().all(|node| {
                matches!(
                    node,
                    CallTypeNodeV1::Unit
                        | CallTypeNodeV1::Leaf(EntrypointValueKindV1::Bool)
                        | CallTypeNodeV1::Error(_)
                )
            }) {
                shape.supported[index / 64] |= 1 << (index % 64);
            }
        }
        if shape.words > MAX_PLAN_WORDS {
            return None;
        }
        for (index, callable) in callables.iter().enumerate() {
            if !shape.supports(index) {
                continue;
            }
            for (position, node) in callable.arguments.nodes.iter().enumerate() {
                if let CallTypeNodeV1::Error(error) = node {
                    // Admission owns full canonical identity, names and nonzero codes.
                    let variants = u32::try_from(error.variants.len()).ok()?;
                    if variants > 256 {
                        return None;
                    }
                    let width = shape.errors.get_mut(position.checked_add(1)?)?;
                    *width = (*width).max(variants);
                }
            }
        }
        for index in 1..=shape.words {
            shape.errors[index] = shape.errors[index].checked_add(shape.errors[index - 1])?;
        }
        Some(shape)
    }
    fn supports(&self, index: usize) -> bool {
        self.supported
            .get(index / 64)
            .is_some_and(|limb| limb & (1 << (index % 64)) != 0)
    }
    pub(super) fn ports(&self) -> usize {
        super::PORTS + 1 + WORD_PORTS * self.words
    }
    pub(super) fn width(&self) -> usize {
        super::WIDTH
            + packet::WIDTH
            + (WORD_WIDTH + WORD_PORTS * packet::WIDTH) * self.words
            + self.error_width()
    }
    fn error_width(&self) -> usize {
        self.errors[self.words] as usize
    }
    fn error_range(&self, index: usize) -> core::ops::Range<usize> {
        self.errors[index] as usize..self.errors[index + 1] as usize
    }
    pub(super) fn history_rows(&self) -> usize {
        self.ports() * super::super::super::PHASES
    }
    fn old_slot(&self, slot: usize) -> usize {
        slot + if slot < 15 {
            0
        } else if slot < 20 {
            self.words
        } else {
            WORD_PORTS * self.words + 1
        }
    }
    fn word_slot(&self, index: usize, port: usize) -> usize {
        if port == EARLY {
            15 + index
        } else {
            20 + self.words + 4 * index + port - 1
        }
    }
    fn completion_slot(&self) -> usize {
        20 + WORD_PORTS * self.words
    }
}

/// Fixed original clocks and schema plan tied to one immutable original program.
#[derive(Clone)]
pub(super) struct Schedule<'p> {
    program: &'p Program,
    vm: u8,
    first_clock: u32,
    shape: Shape,
}
impl<'p> Schedule<'p> {
    pub(super) fn new(program: &'p Program, vm: u8, first_clock: u32) -> Option<Self> {
        let shape = Shape::new(program)?;
        first_clock.checked_add(u32::try_from(shape.ports() - 1).ok()?)?;
        Some(Self {
            program,
            vm,
            first_clock,
            shape,
        })
    }
    fn clock(&self, slot: usize) -> u32 {
        self.first_clock + slot as u32
    }
    fn original(&self) -> super::Schedule {
        super::Schedule::with_clocks(
            self.vm,
            core::array::from_fn(|i| self.clock(self.shape.old_slot(i))),
        )
        .unwrap()
    }
    fn leaf(&self, instruction: usize, position: usize) -> Option<&CallTypeNodeV1> {
        let index = self.program.callables().child_index(instruction)?;
        if !self.shape.supports(index) {
            return None;
        }
        self.program.artifact().contract_interface().callables[index]
            .arguments
            .nodes
            .get(position)
    }
}

/// Borrowed original banks, public padded arguments and final gas owner.
pub(super) struct Row<'a> {
    pub(super) original: super::Row<'a>,
    pub(super) words: &'a [[F; WORD_WIDTH]],
    pub(super) packets: &'a [[[F; packet::WIDTH]; WORD_PORTS]],
    pub(super) error_selectors: &'a [F],
    pub(super) completion_gas: &'a [F; packet::WIDTH],
}
impl Row<'_> {
    fn producer(&self, shape: &Shape, slot: usize) -> &[F; packet::WIDTH] {
        if slot < 15 {
            self.original.packets[slot]
        } else if slot < 15 + shape.words {
            &self.packets[slot - 15][EARLY]
        } else if slot < 20 + shape.words {
            self.original.packets[slot - shape.words]
        } else if slot < shape.completion_slot() {
            let i = slot - 20 - shape.words;
            &self.packets[i / 4][1 + i % 4]
        } else if slot == shape.completion_slot() {
            self.completion_gas
        } else {
            self.original.packets[slot - 5 * shape.words - 1]
        }
    }
}
fn pack(bits: &[F]) -> F {
    bits.iter()
        .enumerate()
        .fold(F::ZERO, |sum, (i, bit)| sum.add(bit.mul(F(1 << i))))
}
fn limb(row: &[F; WORD_WIDTH], offset: usize, index: usize) -> F {
    pack(&row[offset + 16 * index..offset + 16 * (index + 1)])
}
fn roles(schedule: &Schedule<'_>, row: &Row<'_>, index: usize) -> [F; 3] {
    let mut roles = [F::ZERO; 3];
    for slot in 0..schedule.program.words.len() {
        let role = match schedule.leaf(slot, index) {
            Some(CallTypeNodeV1::Unit) => 0,
            Some(CallTypeNodeV1::Leaf(EntrypointValueKindV1::Bool)) => 1,
            Some(CallTypeNodeV1::Error(_)) => 2,
            _ => continue,
        };
        roles[role] = roles[role].add(row.original.dispatch[super::super::FETCH + slot]);
    }
    roles
}
fn activity(schedule: &Schedule<'_>, row: &Row<'_>, index: usize) -> F {
    roles(schedule, row, index)
        .into_iter()
        .fold(F::ZERO, F::add)
}
fn read_header(
    out: &mut Vec<F>,
    p: &[F; packet::WIDTH],
    schedule: &Schedule<'_>,
    index: usize,
    port: usize,
    space: Space,
    generation: F,
    cell: F,
    enabled: F,
) {
    let space = F(space as u64);
    let vm = F(u64::from(schedule.vm));
    let key = cell
        .add(generation.mul(F(1 << 32)))
        .add(vm.mul(F(1 << 48)))
        .add(space.mul(F(1 << 56)));
    for (field, value) in [
        (SPACE, enabled.mul(space)),
        (VM, enabled.mul(vm)),
        (GENERATION, enabled.mul(generation)),
        (INDEX, cell),
        (KEY, enabled.mul(key)),
        (
            CLOCK,
            enabled.mul(F(u64::from(
                schedule.clock(schedule.shape.word_slot(index, port)),
            ))),
        ),
        (ENABLED, enabled),
        (WRITE, F::ZERO),
    ] {
        out.push(p[field].sub(value));
    }
    for i in 0..8 {
        out.push(p[AFTER + i].sub(p[BEFORE + i]));
    }
    out.push(p[AFTER_TAG].sub(p[BEFORE_TAG]));
}

fn append_word(out: &mut Vec<F>, schedule: &Schedule<'_>, row: &Row<'_>, index: usize) {
    let start = out.len();
    let w = &row.words[index];
    let p = &row.packets[index];
    let roles = roles(schedule, row, index);
    let enabled = roles.into_iter().fold(F::ZERO, F::add);
    out.push(super::super::bit(enabled));
    for value in w {
        out.push(super::super::bit(*value));
        out.push(F::ONE.sub(enabled).mul(*value));
    }
    for original in p {
        for value in original {
            out.push(F::ONE.sub(enabled).mul(*value));
        }
    }
    let argument = row.original.packets[super::descriptor_slot(0)];
    for i in 0..4 {
        let incoming = if i == 0 {
            F::ZERO
        } else {
            w[ADD_CARRY + i - 1]
        };
        let offset = enabled.mul(super::super::constant_limb(index as u64 * 8, i));
        out.push(
            enabled
                .mul(argument[BEFORE + i])
                .add(offset)
                .add(incoming)
                .sub(limb(w, ADDRESS, i))
                .sub(w[ADD_CARRY + i].mul(F(1 << 16))),
        );
    }
    out.push(w[ADD_CARRY + 3]);
    out.extend(w[ADDRESS..ADDRESS + 3].iter().copied());
    out.extend(w[ADDRESS + 36..ADDRESS + 64].iter().copied());
    let half = w[ADDRESS + 3];
    let cell = pack(&w[ADDRESS + 4..ADDRESS + 36]);
    let parent = row.original.packets[super::dispatch_slot(super::super::CHILD_ACTIVE)][BEFORE];
    read_header(
        out,
        &p[EARLY],
        schedule,
        index,
        EARLY,
        Space::Initialization,
        parent,
        cell,
        enabled,
    );
    out.push(p[EARLY][BEFORE].sub(pack(&w[INITIALIZED..INITIALIZED + 16])));
    for i in 1..8 {
        out.push(p[EARLY][BEFORE + i]);
    }
    out.push(p[EARLY][BEFORE_TAG]);
    for byte in 0..16 {
        let chosen = if byte < 8 { enabled.sub(half) } else { half };
        out.push(chosen.mul(F::ONE.sub(w[INITIALIZED + byte])));
    }
    // Memory permissions repeat the exact parent bitmap after WORD gas.
    for field in 0..packet::WIDTH {
        let expected = if field == CLOCK {
            enabled.mul(F(u64::from(
                schedule.clock(schedule.shape.word_slot(index, LATE)),
            )))
        } else {
            p[EARLY][field]
        };
        out.push(p[LATE][field].sub(expected));
    }
    for (port, gas, borrow, charge) in [(NODE, NODE_GAS, NODE_BORROW, 1), (DEBIT, GAS, BORROW, 8)] {
        let previous = if port == DEBIT {
            &p[NODE]
        } else if index == 0 {
            row.original.packets[super::FRAME_DEBIT]
        } else {
            &row.packets[index - 1][DEBIT]
        };
        for field in 0..packet::WIDTH {
            let expected = match field {
                CLOCK => enabled.mul(F(u64::from(
                    schedule.clock(schedule.shape.word_slot(index, port)),
                ))),
                BEFORE..AFTER => enabled.mul(previous[AFTER + field - BEFORE]),
                AFTER..=23 => {
                    if field < AFTER + 4 {
                        limb(w, gas, field - AFTER)
                    } else {
                        F::ZERO
                    }
                }
                _ => enabled.mul(previous[field]),
            };
            out.push(p[port][field].sub(expected));
        }
        for i in 0..4 {
            let incoming = if i == 0 { F::ZERO } else { w[borrow + i - 1] };
            let cost = if i == 0 {
                enabled.mul(F(charge))
            } else {
                F::ZERO
            };
            out.push(
                p[port][BEFORE + i]
                    .sub(cost)
                    .sub(incoming)
                    .sub(limb(w, gas, i))
                    .add(w[borrow + i].mul(F(1 << 16))),
            );
        }
        out.push(w[borrow + 3]);
    }
    read_header(
        out,
        &p[MEMORY],
        schedule,
        index,
        MEMORY,
        Space::Memory,
        F::ZERO,
        cell,
        enabled,
    );
    out.push(p[MEMORY][BEFORE_TAG].sub(pack(&w[PRIVATE..PRIVATE + 16])));
    for byte in 0..16 {
        let chosen = if byte < 8 { enabled.sub(half) } else { half };
        out.push(chosen.mul(w[PRIVATE + byte]));
    }
    for i in 0..4 {
        out.push(
            limb(w, VALUE, i).sub(
                enabled
                    .sub(half)
                    .mul(p[MEMORY][BEFORE + i])
                    .add(half.mul(p[MEMORY][BEFORE + 4 + i])),
            ),
        );
    }
    for bit in 0..64 {
        let must_zero = roles[0]
            .add(if bit >= 1 { roles[1] } else { F::ZERO })
            .add(if bit >= 32 { roles[2] } else { F::ZERO });
        out.push(must_zero.mul(w[VALUE + bit]));
    }
    assert_eq!(out.len() - start, 999);
}

fn append_error_membership(out: &mut Vec<F>, schedule: &Schedule<'_>, row: &Row<'_>, index: usize) {
    let error = roles(schedule, row, index)[2];
    let mut sum = F::ZERO;
    let mut value = F::ZERO;
    for (ordinal, &selector) in row.error_selectors[schedule.shape.error_range(index)]
        .iter()
        .enumerate()
    {
        let mut allowed = F::ZERO;
        let mut code = F::ZERO;
        for slot in 0..schedule.program.words.len() {
            let Some(CallTypeNodeV1::Error(descriptor)) = schedule.leaf(slot, index) else {
                continue;
            };
            let Some(variant) = descriptor.variants.get(ordinal) else {
                continue;
            };
            let fetch = row.original.dispatch[super::super::FETCH + slot];
            allowed = allowed.add(fetch);
            code = code.add(fetch.mul(F(u64::from(variant.code))));
        }
        out.push(super::super::bit(selector));
        out.push(selector.mul(F::ONE.sub(allowed)));
        sum = sum.add(selector);
        value = value.add(selector.mul(code));
    }
    out.push(sum.sub(error));
    out.push(
        error
            .mul(pack(&row.words[index][VALUE..VALUE + 32]))
            .sub(value),
    );
}

fn append_semantics(out: &mut Vec<F>, schedule: &Schedule<'_>, row: &Row<'_>) {
    let program = schedule.program;
    assert_eq!(row.words.len(), schedule.shape.words);
    assert_eq!(row.packets.len(), schedule.shape.words);
    assert_eq!(row.error_selectors.len(), schedule.shape.error_width());
    super::append_semantics(out, program, schedule.original(), &row.original);
    let added_start = out.len();
    let active = row.original.packets[super::dispatch_slot(super::super::RUNNING_WRITE)][BEFORE];
    let mut scalar = F::ZERO;
    for slot in 0..program.words.len() {
        if program
            .callables()
            .child_index(slot)
            .is_some_and(|index| schedule.shape.supports(index))
        {
            scalar = scalar.add(row.original.dispatch[super::super::FETCH + slot]);
        }
    }
    out.push(scalar.sub(active));
    for index in 0..schedule.shape.words {
        append_word(out, schedule, row, index);
        append_error_membership(out, schedule, row, index);
    }
    let frame = row.original.packets[super::FRAME_DEBIT];
    let first = if schedule.shape.words == 0 {
        F::ZERO
    } else {
        activity(schedule, row, 0)
    };
    let mut final_gas =
        core::array::from_fn::<_, 8, _>(|i| active.sub(first).mul(frame[AFTER + i]));
    for index in 0..schedule.shape.words {
        let next = if index + 1 == schedule.shape.words {
            F::ZERO
        } else {
            activity(schedule, row, index + 1)
        };
        let last = activity(schedule, row, index).sub(next);
        for (i, value) in final_gas.iter_mut().enumerate() {
            *value = value.add(last.mul(row.packets[index][DEBIT][AFTER + i]));
        }
    }
    for field in 0..packet::WIDTH {
        let expected = match field {
            CLOCK => active.mul(F(u64::from(
                schedule.clock(schedule.shape.completion_slot()),
            ))),
            WRITE => F::ZERO,
            BEFORE..AFTER => final_gas[field - BEFORE],
            AFTER..=23 => final_gas[field - AFTER],
            _ => frame[field],
        };
        out.push(row.completion_gas[field].sub(expected));
    }
    assert_eq!(
        out.len() - added_start,
        27 + 1001 * schedule.shape.words + 2 * schedule.shape.error_width()
    );
}

/// Join every original packet field to all8 typed history stages.
pub(super) fn append_residues(
    out: &mut Vec<F>,
    schedule: &Schedule<'_>,
    row: &Row<'_>,
    history: &[super::super::HistoryRow<'_>],
    challenges: &permutation::Challenges,
) {
    append_semantics(out, schedule, row);
    assert_eq!(history.len(), schedule.shape.history_rows());
    for (index, history) in history.iter().enumerate() {
        let phases = super::super::super::PHASES;
        out.push(
            history.fixed[super::super::super::SLOT]
                .sub(F(u64::from(schedule.clock(index / phases)))),
        );
        for phase in 0..phases {
            out.push(
                history.fixed[super::super::super::PHASE_OFFSET + phase]
                    .sub(F(u64::from(phase == index % phases))),
            );
        }
        private_history::append_residues(
            out,
            history.current,
            history.next,
            history.aux,
            history.next_aux,
            history.fixed,
            row.producer(&schedule.shape, index / phases),
            challenges,
        );
    }
}

#[cfg(test)]
mod tests;
