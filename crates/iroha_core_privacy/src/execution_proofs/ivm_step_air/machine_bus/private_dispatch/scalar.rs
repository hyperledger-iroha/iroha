//! Original private scalar reads, matched tags, wrapping writes and comparisons.
//!
//! This bank consumes the enclosing dispatcher's private canonical fetch cells
//! and its sole original packet array. It has no independent instruction list,
//! public operand boundary, event digest or production proving adapter.

use super::super::super::{alu, branch, immediate_operand, semantic_opcode, word};
use super::*;

const SOURCES: usize = 0;
const ALU: usize = SOURCES + word::WIDTH;
pub(super) const COMPARE: usize = ALU + alu::WIDTH;
pub(super) const WIDTH: usize = COMPARE + branch::BANK_WIDTH;

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
        wide::arithmetic::SLT => Some(2),
        wide::arithmetic::SLTU => Some(4),
        wide::arithmetic::SEQ => Some(0),
        wide::arithmetic::SNE => Some(1),
        _ => None,
    }
}

pub(super) fn is_supported(instruction: u32) -> bool {
    is_alu(instruction) || comparison_predicate(instruction).is_some()
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
    let register_right = select(&|w| immediate_operand(w).is_none());
    let destination = select(&|w| wide::rd(w) != 0);
    for (slot, index, enabled, write) in [
        (
            SCALAR_LEFT,
            weighted(&|w| F(wide::rs1(w) as u64)),
            active,
            F::ZERO,
        ),
        (
            SCALAR_RIGHT,
            weighted(&|w| {
                if immediate_operand(w).is_none() {
                    F(wide::rs2(w) as u64)
                } else {
                    F::ZERO
                }
            }),
            register_right,
            F::ZERO,
        ),
        (
            SCALAR_DESTINATION,
            weighted(&|w| F(wide::rd(w) as u64)),
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
            weighted(&|w| immediate_operand(w).map_or(F::ZERO, |v| constant_limb(v, limb)));
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
    for (slot, zero) in [
        (SCALAR_LEFT, select(&|w| wide::rs1(w) == 0)),
        (
            SCALAR_RIGHT,
            select(&|w| immediate_operand(w).is_none() && wide::rs2(w) == 0),
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
            select(&|w| immediate_operand(w).is_none() && wide::rs1(w) == wide::rs2(w)),
        ),
        (
            SCALAR_LEFT,
            SCALAR_DESTINATION,
            select(&|w| wide::rd(w) != 0 && wide::rd(w) == wide::rs1(w)),
        ),
        (
            SCALAR_RIGHT,
            SCALAR_DESTINATION,
            select(&|w| {
                immediate_operand(w).is_none() && wide::rd(w) != 0 && wide::rd(w) == wide::rs2(w)
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
    // Both banks stay fully constrained on the same original private source
    // bits, even when their result is unused. The ALU defaults to ADD, while
    // comparison selectors all zero force TAKEN=0 but retain exact subtraction,
    // zero/inverse and equality witnesses. Quartic checks are never gated.
    // Neither bank allocates an intermediate private residual vector.
    alu::append_residues(out, bank, source, selectors);
    let comparison = &row[SCALAR + COMPARE..super::WIDTH];
    branch::append_bank_residues(
        out,
        comparison,
        core::array::from_fn(|operand| core::array::from_fn(|limb| source.limb(operand, limb))),
        [source.sign(0), source.sign(1)],
        core::array::from_fn(|predicate| select(&|w| comparison_predicate(w) == Some(predicate))),
    );
    let alu_destination = select(&|w| is_alu(w) && wide::rd(w) != 0);
    let comparison_destination = select(&|w| comparison_predicate(w).is_some() && wide::rd(w) != 0);
    for limb in 0..4 {
        let boolean = if limb == 0 {
            comparison[branch::TAKEN_BANK_OFFSET]
        } else {
            F::ZERO
        };
        out.push(
            p[SCALAR_DESTINATION][AFTER + limb]
                .sub(alu_destination.mul(bank[alu::RESULT + limb]))
                .sub(comparison_destination.mul(boolean)),
        );
    }
    out.push(p[SCALAR_DESTINATION][AFTER_TAG].sub(destination.mul(p[SCALAR_LEFT][BEFORE_TAG])));
}

#[cfg(test)]
mod tests;
