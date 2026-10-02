//! Original private scalar/branch reads, tags, writes, comparisons and shifts.
//!
//! This bank consumes the enclosing dispatcher's private canonical fetch cells
//! and its sole original packet array. It has no independent instruction list,
//! public operand boundary, event digest or production proving adapter.

use super::super::super::{alu, branch, immediate_operand, semantic_opcode, shift, word};
use super::*;

const SOURCES: usize = 0;
const ALU: usize = SOURCES + word::WIDTH;
pub(super) const COMPARE: usize = ALU + alu::WIDTH;
pub(super) const SHIFT: usize = COMPARE + branch::BANK_WIDTH;
pub(super) const WIDTH: usize = SHIFT + shift::BANK_WIDTH;

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
    if matches!(
        wide::opcode(instruction),
        wide::arithmetic::ROTL_IMM | wide::arithmetic::ROTR_IMM
    ) {
        Some(u64::from(wide::imm8(instruction) as u8))
    } else {
        immediate_operand(instruction)
    }
}

fn is_alu(instruction: u32) -> bool {
    matches!(
        semantic_opcode(wide::opcode(instruction)),
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
        wide::arithmetic::SLT => Some(2),
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
    if is_branch(instruction) {
        wide::rd(instruction)
    } else {
        wide::rs1(instruction)
    }
}

fn right_register(instruction: u32) -> usize {
    if is_branch(instruction) {
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
    let register_right = select(&|w| right_immediate(w).is_none());
    let destination = select(&|w| has_destination(w));
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
                    F(right_register(w) as u64)
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
                    F(wide::rd(w) as u64)
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
    for limb in 0..4 {
        out.push(source.limb(0, limb).sub(p[SCALAR_LEFT][BEFORE + limb]));
        let immediate =
            weighted(&|w| right_immediate(w).map_or(F::ZERO, |v| constant_limb(v, limb)));
        out.push(
            source
                .limb(1, limb)
                .sub(p[SCALAR_RIGHT][BEFORE + limb])
                .sub(immediate),
        );
    }
    // A ZK binary register instruction rejects differently tagged operands,
    // even when its destination is r0. Immediate operands inherit rs1's tag.
    out.push(register_right.mul(p[SCALAR_LEFT][BEFORE_TAG].sub(p[SCALAR_RIGHT][BEFORE_TAG])));
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
            out.push(equal.mul(p[left][field].sub(p[right][field])));
        }
    }
    let bank = &row[SCALAR + ALU..SCALAR + COMPARE];
    let selectors = [
        wide::arithmetic::SUB,
        wide::arithmetic::AND,
        wide::arithmetic::OR,
        wide::arithmetic::XOR,
    ]
    .map(|opcode| select(&|w| semantic_opcode(wide::opcode(w)) == opcode));
    // All banks stay fully constrained on the same original private source
    // bits, even when their result is unused. The ALU defaults to ADD, while
    // comparison selectors all zero force TAKEN=0 but retain exact subtraction,
    // zero/inverse and equality witnesses. Quartic checks are never gated.
    // No bank allocates an intermediate private residual vector.
    alu::append_residues(out, bank, source, selectors);
    let comparison = &row[SCALAR + COMPARE..SCALAR + SHIFT];
    branch::append_bank_residues(
        out,
        comparison,
        core::array::from_fn(|operand| core::array::from_fn(|limb| source.limb(operand, limb))),
        [source.sign(0), source.sign(1)],
        core::array::from_fn(|predicate| select(&|w| comparison_predicate(w) == Some(predicate))),
    );
    let shifts = &row[SCALAR + SHIFT..super::WIDTH];
    let mut kinds = core::array::from_fn(|kind| select(&|w| shift_kind(w) == Some(kind)));
    // Canonical SLL on every non-shift row, including zero-source padding.
    kinds[0] = kinds[0].add(F::ONE.sub(select(&|w| shift_kind(w).is_some())));
    shift::append_bank_residues(out, shifts, source, kinds);
    let shift_destination = select(&|w| shift_kind(w).is_some() && has_destination(w));
    let alu_destination = select(&|w| is_alu(w) && wide::rd(w) != 0);
    let comparison_destination =
        select(&|w| comparison_predicate(w).is_some() && has_destination(w));
    for limb in 0..4 {
        let boolean = if limb == 0 {
            comparison[branch::TAKEN_BANK_OFFSET]
        } else {
            F::ZERO
        };
        out.push(
            p[SCALAR_DESTINATION][AFTER + limb]
                .sub(alu_destination.mul(bank[alu::RESULT + limb]))
                .sub(comparison_destination.mul(boolean))
                .sub(shift_destination.mul(shifts[shift::RESULT + limb])),
        );
    }
    out.push(p[SCALAR_DESTINATION][AFTER_TAG].sub(destination.mul(p[SCALAR_LEFT][BEFORE_TAG])));
}

#[cfg(test)]
mod tests;
