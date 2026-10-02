//! Original private scalar/branch reads, tags, writes, comparisons and shifts.
//! Total NOT/NEG reuse the ALU; signed MIN/MAX select full original words.
//! Four multiply variants reuse the exact full-width product/correction bank.
//!
//! This bank consumes the enclosing dispatcher's private canonical fetch cells
//! and its sole original packet array. It has no independent instruction list,
//! public operand boundary, event digest or production proving adapter.

use super::super::super::{
    alu, branch, immediate_operand, semantic_opcode, shift,
    trace::{bit_count, multiply},
    word,
};
use super::*;

const SOURCES: usize = 0;
const ALU: usize = SOURCES + word::WIDTH;
pub(super) const COMPARE: usize = ALU + alu::WIDTH;
const PRODUCT_DIGITS: usize = COMPARE + branch::BANK_WIDTH;
const MULTIPLY: usize = PRODUCT_DIGITS + 64;
pub(super) const COUNT: usize = MULTIPLY + multiply::WIDTH;
pub(super) const MOVE: usize = COUNT + bit_count::WIDTH;
const MOVE_ZERO: usize = MOVE;
const MOVE_INVERSE: usize = MOVE + 1;
pub(super) const SHIFT: usize = MOVE + 2;
pub(super) const WIDTH: usize = SHIFT + shift::BANK_WIDTH;

/// Native successful conditional moves read the condition first, then the
/// optional source and destination only when the full condition is nonzero.
pub(super) fn is_conditional_move(instruction: u32) -> bool {
    matches!(
        wide::opcode(instruction),
        wide::arithmetic::CMOV | wide::arithmetic::CMOVI
    )
}

pub(super) fn is_bit_count(instruction: u32) -> bool {
    matches!(
        wide::opcode(instruction),
        wide::arithmetic::POPCNT | wide::arithmetic::CLZ | wide::arithmetic::CTZ
    )
}

fn multiply_kind(instruction: u32) -> Option<usize> {
    match wide::opcode(instruction) {
        wide::arithmetic::MUL => Some(0),
        wide::arithmetic::MULHU => Some(1),
        wide::arithmetic::MULHSU => Some(2),
        wide::arithmetic::MULH => Some(3),
        _ => None,
    }
}

pub(super) fn is_multiply(instruction: u32) -> bool {
    multiply_kind(instruction).is_some()
}

fn shift_kind(instruction: u32) -> Option<usize> {
    match wide::opcode(instruction) {
        wide::arithmetic::SLL => Some(0),
        wide::arithmetic::SRL => Some(1),
        wide::arithmetic::SRA => Some(2),
        wide::arithmetic::ROTL | wide::arithmetic::ROTL_IMM => Some(3),
        wide::arithmetic::ROTR | wide::arithmetic::ROTR_IMM => Some(4),
        _ => None,
    }
}

pub(super) fn is_rotate(instruction: u32) -> bool {
    matches!(shift_kind(instruction), Some(3 | 4))
}

fn right_immediate(instruction: u32) -> Option<u64> {
    match wide::opcode(instruction) {
        // Unary instructions never read the encoded rs2 byte. NEG routes the
        // original rs1 packet into the right arithmetic operand separately.
        wide::arithmetic::NEG
        | wide::arithmetic::POPCNT
        | wide::arithmetic::CLZ
        | wide::arithmetic::CTZ => return Some(0),
        wide::arithmetic::NOT => return Some(u64::MAX),
        wide::arithmetic::CMOVI => return Some(i64::from(wide::imm8(instruction)) as u64),
        _ => {}
    }
    if matches!(
        wide::opcode(instruction),
        wide::arithmetic::ROTL_IMM | wide::arithmetic::ROTR_IMM
    ) {
        Some(u64::from(wide::imm8(instruction) as u8))
    } else {
        immediate_operand(instruction)
    }
}

fn alu_opcode(instruction: u32) -> u8 {
    match wide::opcode(instruction) {
        wide::arithmetic::NEG => wide::arithmetic::SUB,
        wide::arithmetic::NOT => wide::arithmetic::XOR,
        opcode => semantic_opcode(opcode),
    }
}

fn is_alu(instruction: u32) -> bool {
    matches!(
        alu_opcode(instruction),
        wide::arithmetic::ADD
            | wide::arithmetic::SUB
            | wide::arithmetic::AND
            | wide::arithmetic::OR
            | wide::arithmetic::XOR
    )
}

/// The predicate is selected only from the canonical artifact fetch, never a
/// caller predicate or a scalar word reduced modulo the proof field.
fn comparison_predicate(instruction: u32) -> Option<usize> {
    match wide::opcode(instruction) {
        wide::control::BEQ => Some(0),
        wide::control::BNE => Some(1),
        wide::control::BLT => Some(2),
        wide::control::BGE => Some(3),
        wide::control::BLTU => Some(4),
        wide::control::BGEU => Some(5),
        wide::arithmetic::SLT | wide::arithmetic::MIN | wide::arithmetic::MAX => Some(2),
        wide::arithmetic::SLTU => Some(4),
        wide::arithmetic::SEQ => Some(0),
        wide::arithmetic::SNE => Some(1),
        _ => None,
    }
}

pub(super) fn is_branch(instruction: u32) -> bool {
    matches!(
        wide::opcode(instruction),
        wide::control::BEQ
            | wide::control::BNE
            | wide::control::BLT
            | wide::control::BGE
            | wide::control::BLTU
            | wide::control::BGEU
    )
}

fn left_register(instruction: u32) -> usize {
    if wide::opcode(instruction) == wide::arithmetic::CMOV {
        wide::rs2(instruction)
    } else if is_branch(instruction) {
        wide::rd(instruction)
    } else {
        wide::rs1(instruction)
    }
}

fn right_register(instruction: u32) -> usize {
    if is_branch(instruction) || wide::opcode(instruction) == wide::arithmetic::CMOV {
        wide::rs1(instruction)
    } else {
        wide::rs2(instruction)
    }
}

fn has_destination(instruction: u32) -> bool {
    !is_branch(instruction) && wide::rd(instruction) != 0
}

/// The same private comparison result controls the original PC producer.
pub(super) fn branch_taken(row: &[F; super::WIDTH]) -> F {
    row[SCALAR + COMPARE + branch::TAKEN_BANK_OFFSET]
}

pub(super) fn is_supported(instruction: u32) -> bool {
    is_alu(instruction)
        || comparison_predicate(instruction).is_some()
        || shift_kind(instruction).is_some()
        || is_multiply(instruction)
        || is_bit_count(instruction)
        || is_conditional_move(instruction)
}

pub(super) fn append_residues(
    out: &mut Vec<F>,
    program: &Program,
    schedule: Schedule,
    row: &[F; super::WIDTH],
    packets: &OriginalPackets,
) {
    let weighted = |value: &dyn Fn(u32) -> F| {
        program
            .words
            .iter()
            .copied()
            .enumerate()
            .fold(F::ZERO, |sum, (i, w)| {
                if is_supported(w) {
                    sum.add(row[FETCH + i].mul(value(w)))
                } else {
                    sum
                }
            })
    };
    let select = |predicate: &dyn Fn(u32) -> bool| weighted(&|w| F(u64::from(predicate(w))));
    let active = select(&|_| true);
    let move_selected = select(&|w| is_conditional_move(w));
    let register_move = select(&|w| wide::opcode(w) == wide::arithmetic::CMOV);
    let move_taken = F::ONE.sub(row[SCALAR + MOVE_ZERO]);
    let register_right =
        select(&|w| right_immediate(w).is_none()).sub(register_move.mul(F::ONE.sub(move_taken)));
    let move_destination =
        select(&|w| is_conditional_move(w) && has_destination(w)).mul(move_taken);
    let ordinary_destination = select(&|w| !is_conditional_move(w) && has_destination(w));
    let destination = ordinary_destination.add(move_destination);
    for (slot, index, enabled, write) in [
        (
            SCALAR_LEFT,
            weighted(&|w| F(left_register(w) as u64)),
            active,
            F::ZERO,
        ),
        (
            SCALAR_RIGHT,
            weighted(&|w| {
                if right_immediate(w).is_none() {
                    F(right_register(w) as u64).mul(if is_conditional_move(w) {
                        move_taken
                    } else {
                        F::ONE
                    })
                } else {
                    F::ZERO
                }
            }),
            register_right,
            F::ZERO,
        ),
        (
            SCALAR_DESTINATION,
            weighted(&|w| {
                if has_destination(w) {
                    F(wide::rd(w) as u64).mul(if is_conditional_move(w) {
                        move_taken
                    } else {
                        F::ONE
                    })
                } else {
                    F::ZERO
                }
            }),
            destination,
            destination,
        ),
    ] {
        header(
            out,
            schedule,
            packets,
            slot,
            Space::Register,
            index,
            F::ZERO,
            enabled,
            write,
        );
    }
    let p = &packets.fields;
    let source = word::Sources::new(&row[SCALAR + SOURCES..SCALAR + ALU]);
    source.append_residues(out);
    let negating = select(&|w| wide::opcode(w) == wide::arithmetic::NEG);
    for limb in 0..4 {
        // Architectural rs1 remains the original left read for tags, aliases
        // and private history; only the shared ALU operands become (0, rs1).
        let left = p[SCALAR_LEFT][BEFORE + limb];
        out.push(source.limb(0, limb).sub(F::ONE.sub(negating).mul(left)));
        let immediate =
            weighted(&|w| right_immediate(w).map_or(F::ZERO, |v| constant_limb(v, limb)));
        out.push(
            source
                .limb(1, limb)
                .sub(p[SCALAR_RIGHT][BEFORE + limb])
                .sub(immediate)
                .sub(negating.mul(left)),
        );
    }
    // A ZK binary register instruction rejects differently tagged operands,
    // even when its destination is r0. Constants and unary operations inherit rs1's tag.
    out.push(
        register_right
            .sub(register_move.mul(move_taken))
            .mul(p[SCALAR_LEFT][BEFORE_TAG].sub(p[SCALAR_RIGHT][BEFORE_TAG])),
    );
    // CMOV may copy a secret-tagged value, but its condition must be public,
    // including a false condition or rd0. CMOVI writes a public immediate.
    out.push(move_selected.mul(p[SCALAR_LEFT][BEFORE_TAG]));
    // Conditional branches are forbidden on either private-tagged operand,
    // including equal tags and r0 aliases; their execution path stays private
    // in this relation without changing native tag policy.
    let branching = select(&|w| is_branch(w));
    out.push(branching.mul(p[SCALAR_LEFT][BEFORE_TAG]));
    out.push(branching.mul(p[SCALAR_RIGHT][BEFORE_TAG]));
    for (slot, zero) in [
        (SCALAR_LEFT, select(&|w| left_register(w) == 0)),
        (
            SCALAR_RIGHT,
            select(&|w| right_immediate(w).is_none() && right_register(w) == 0),
        ),
    ] {
        for field in (BEFORE..BEFORE + 4).chain([BEFORE_TAG]) {
            out.push(zero.mul(p[slot][field]));
        }
    }
    // These are pre-read aliases, not reads of the just-written destination.
    // The full private history additionally binds all surrounding instructions.
    for (left, right, equal) in [
        (
            SCALAR_LEFT,
            SCALAR_RIGHT,
            select(&|w| right_immediate(w).is_none() && left_register(w) == right_register(w)),
        ),
        (
            SCALAR_LEFT,
            SCALAR_DESTINATION,
            select(&|w| has_destination(w) && wide::rd(w) == left_register(w)),
        ),
        (
            SCALAR_RIGHT,
            SCALAR_DESTINATION,
            select(&|w| {
                right_immediate(w).is_none()
                    && has_destination(w)
                    && wide::rd(w) == right_register(w)
            }),
        ),
    ] {
        for field in (BEFORE..BEFORE + 4).chain([BEFORE_TAG]) {
            // Conditional reads/writes do not exist on the untaken path.
            // Original packet enable cells keep this alias check at degree4.
            out.push(
                equal
                    .mul(p[left][ENABLED])
                    .mul(p[right][ENABLED])
                    .mul(p[left][field].sub(p[right][field])),
            );
        }
    }
    let bank = &row[SCALAR + ALU..SCALAR + COMPARE];
    let selectors = [
        wide::arithmetic::SUB,
        wide::arithmetic::AND,
        wide::arithmetic::OR,
        wide::arithmetic::XOR,
    ]
    .map(|opcode| select(&|w| alu_opcode(w) == opcode));
    // All banks stay fully constrained on the same original private source
    // bits, even when their result is unused. The ALU defaults to ADD, while
    // comparison selectors all zero force TAKEN=0 but retain exact subtraction,
    // zero/inverse and equality witnesses. Quartic checks are never gated.
    // No bank allocates an intermediate private residual vector.
    alu::append_residues(out, bank, source, selectors);
    let comparison = &row[SCALAR + COMPARE..SCALAR + PRODUCT_DIGITS];
    branch::append_bank_residues(
        out,
        comparison,
        core::array::from_fn(|operand| core::array::from_fn(|limb| source.limb(operand, limb))),
        [source.sign(0), source.sign(1)],
        core::array::from_fn(|predicate| select(&|w| comparison_predicate(w) == Some(predicate))),
    );
    // Like the ALU and shift banks, the exact product stays constrained on
    // every row. Keeping range equations unconditional preserves degree four;
    // only destination selection depends on the private canonical fetch.
    let product = &row[SCALAR + MULTIPLY..SCALAR + COUNT];
    multiply::append_residues(
        out,
        product,
        &row[SCALAR + PRODUCT_DIGITS..SCALAR + MULTIPLY],
        source,
        multiply::Selection {
            multiply: F::ONE,
            division: F::ZERO,
            square: F::ZERO,
            signed: F::ZERO,
            success: F::ZERO,
            quotient: [F::ZERO; 4],
        },
    );
    let product_destinations = core::array::from_fn::<_, 4, _>(|kind| {
        select(&|w| multiply_kind(w) == Some(kind) && has_destination(w))
    });
    // The original canonical bits uniquely determine every prefix, including
    // unrelated instructions and padding. Only CLZ reverses this public-code
    // selected traversal; no secret value controls a branch in these equations.
    let count_prefixes = &row[SCALAR + COUNT..SCALAR + MOVE];
    bit_count::append_residues(
        out,
        count_prefixes,
        source.bits(0),
        select(&|w| wide::opcode(w) == wide::arithmetic::CLZ),
    );
    let prefix_count = count_prefixes.iter().copied().fold(F::ZERO, F::add);
    let population = source.bits(0).iter().copied().fold(F::ZERO, F::add);
    // Sum of 64 canonical bits cannot wrap the field. The unconditional
    // zero-safe inverse uniquely binds taken and canonical padding cells.
    let zero = row[SCALAR + MOVE_ZERO];
    let inverse = row[SCALAR + MOVE_INVERSE];
    out.push(bit(zero));
    out.push(population.mul(zero));
    out.push(population.mul(inverse).sub(F::ONE.sub(zero)));
    out.push(zero.mul(inverse));
    let population_destination =
        select(&|w| wide::opcode(w) == wide::arithmetic::POPCNT && has_destination(w));
    let prefix_destination = select(&|w| {
        matches!(
            wide::opcode(w),
            wide::arithmetic::CLZ | wide::arithmetic::CTZ
        ) && has_destination(w)
    });
    let shifts = &row[SCALAR + SHIFT..super::WIDTH];
    let mut kinds = core::array::from_fn(|kind| select(&|w| shift_kind(w) == Some(kind)));
    // Canonical SLL on every non-shift row, including zero-source padding.
    kinds[0] = kinds[0].add(F::ONE.sub(select(&|w| shift_kind(w).is_some())));
    shift::append_bank_residues(out, shifts, source, kinds);
    let shift_destination = select(&|w| shift_kind(w).is_some() && has_destination(w));
    let alu_destination = select(&|w| is_alu(w) && wide::rd(w) != 0);
    let minimum_destination =
        select(&|w| wide::opcode(w) == wide::arithmetic::MIN && has_destination(w));
    let maximum_destination =
        select(&|w| wide::opcode(w) == wide::arithmetic::MAX && has_destination(w));
    let comparison_destination = select(&|w| {
        comparison_predicate(w).is_some()
            && !matches!(
                wide::opcode(w),
                wide::arithmetic::MIN | wide::arithmetic::MAX
            )
            && has_destination(w)
    });
    let taken = comparison[branch::TAKEN_BANK_OFFSET];
    for limb in 0..4 {
        let boolean = if limb == 0 {
            comparison[branch::TAKEN_BANK_OFFSET]
        } else {
            F::ZERO
        };
        // The predicate and operands are canonical bank cells. Multiplying
        // this quadratic full-word selection by fetch keeps degree three.
        let left = source.limb(0, limb);
        let right = source.limb(1, limb);
        let minimum = taken.mul(left).add(F::ONE.sub(taken).mul(right));
        let maximum = taken.mul(right).add(F::ONE.sub(taken).mul(left));
        let multiplied = [
            multiply::PRODUCT,
            multiply::PRODUCT + 4,
            multiply::SIGNED_UNSIGNED,
            multiply::SIGNED_SIGNED,
        ]
        .into_iter()
        .zip(product_destinations)
        .fold(F::ZERO, |sum, (offset, selected)| {
            sum.add(selected.mul(product[offset + limb]))
        });
        out.push(
            p[SCALAR_DESTINATION][AFTER + limb]
                .sub(multiplied)
                .sub(move_destination.mul(source.limb(1, limb)))
                .sub(if limb == 0 {
                    population_destination
                        .mul(population)
                        .add(prefix_destination.mul(prefix_count))
                } else {
                    F::ZERO
                })
                .sub(alu_destination.mul(bank[alu::RESULT + limb]))
                .sub(comparison_destination.mul(boolean))
                .sub(minimum_destination.mul(minimum))
                .sub(maximum_destination.mul(maximum))
                .sub(shift_destination.mul(shifts[shift::RESULT + limb])),
        );
    }
    let moved_tag = select(&|w| wide::opcode(w) == wide::arithmetic::CMOV && has_destination(w))
        .mul(move_taken)
        .mul(p[SCALAR_RIGHT][BEFORE_TAG]);
    out.push(
        p[SCALAR_DESTINATION][AFTER_TAG]
            .sub(ordinary_destination.mul(p[SCALAR_LEFT][BEFORE_TAG]))
            .sub(moved_tag),
    );
}

#[cfg(test)]
mod tests;
