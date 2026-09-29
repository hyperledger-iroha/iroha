//! Shared wrapping arithmetic and bitwise result bank over canonical source bits.

use super::{
    F, bit, bitwise, semantic_opcode, wide,
    word::{self, Sources},
};

pub(super) const RESULT: usize = 0;
pub(super) const DIGITS: usize = 4;
pub(super) const TRANSFER: usize = DIGITS + 32;
pub(super) const WIDTH: usize = TRANSFER + 4;
pub(super) const CONSTRAINTS: usize = 80;

pub(super) fn witness(opcode: u8, left: u64, right: u64) -> [F; WIDTH] {
    let opcode = semantic_opcode(opcode);
    let result = match opcode {
        wide::arithmetic::ADD => left.wrapping_add(right),
        wide::arithmetic::SUB => left.wrapping_sub(right),
        wide::arithmetic::AND => left & right,
        wide::arithmetic::OR => left | right,
        wide::arithmetic::XOR => left ^ right,
        _ => unreachable!("validated scalar ALU opcode"),
    };
    let mut row = [F::ZERO; WIDTH];
    let mut transfer = 0;
    for limb in 0..4 {
        row[RESULT + limb] = F((result >> (16 * limb)) & 0xffff);
        if matches!(opcode, wide::arithmetic::ADD | wide::arithmetic::SUB) {
            let a = (left >> (16 * limb)) & 0xffff;
            let b = (right >> (16 * limb)) & 0xffff;
            transfer = if opcode == wide::arithmetic::SUB {
                u64::from(a < b + transfer)
            } else {
                (a + b + transfer) >> 16
            };
            row[TRANSFER + limb] = F(transfer);
        }
    }
    word::fill_digits(&mut row[DIGITS..TRANSFER], result);
    row
}

pub(super) fn residues(bank: &[F], sources: Sources<'_>, selectors: [F; 4]) -> Vec<F> {
    let mut out = Vec::with_capacity(CONSTRAINTS);
    let [is_sub, is_and, is_or, is_xor] = selectors;
    let is_bitwise = is_and.add(is_or).add(is_xor);
    let is_add = F::ONE.sub(is_sub).sub(is_bitwise);
    for limb in 0..4 {
        out.push(bit(bank[TRANSFER + limb]));
        out.push(is_bitwise.mul(bank[TRANSFER + limb]));
    }
    out.extend(bank[DIGITS..TRANSFER].iter().copied().map(word::radix4));
    for limb in 0..4 {
        out.push(bank[RESULT + limb].sub(word::pack(
            &bank[DIGITS + 8 * limb..DIGITS + 8 * (limb + 1)],
            2,
        )));
        let incoming = if limb == 0 {
            F::ZERO
        } else {
            bank[TRANSFER + limb - 1]
        };
        let left = sources.limb(0, limb);
        let right = sources.limb(1, limb);
        let result = bank[RESULT + limb];
        let outgoing = bank[TRANSFER + limb].mul(F(1 << 16));
        let add = left.add(right).add(incoming).sub(result).sub(outgoing);
        let sub = left.sub(right).sub(incoming).sub(result).add(outgoing);
        out.push(is_add.mul(add).add(is_sub.mul(sub)));
    }
    bitwise::append_residues(
        &mut out,
        &bank[DIGITS..TRANSFER],
        sources,
        [is_and, is_or, is_xor],
    );
    debug_assert_eq!(out.len(), CONSTRAINTS);
    out
}
