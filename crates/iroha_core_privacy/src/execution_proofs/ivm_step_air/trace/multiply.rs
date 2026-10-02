//! Full 64×64 product and signed high-word corrections with exact radix-2^16 carries.
//!
//! The caller supplies 64 workspace cells: product radix-four digits on multiply,
//! division and square-root rows, canonical count prefixes on other operations.
//! Their ranges are unconditional. Only the convolution is selected; inactive
//! carries are zero. Correction slots hold signed high corrections, division
//! magnitudes, or square-root input and root. Their ranges remain unconditional;
//! the selected instruction owner constrains each interpretation.
//! Carries fit 18 bits. Every convolution residual has absolute integer value
//! below 2^35, so Goldilocks equality cannot hide a modular integer discrepancy.

use super::super::{
    F, bit,
    word::{self, Sources},
};

pub(in super::super) const PRODUCT: usize = 0;
pub(in super::super) const CARRY: usize = PRODUCT + 8;
pub(in super::super) const CARRY_DIGITS: usize = CARRY + 7;
pub(in super::super) const SIGNED_UNSIGNED: usize = CARRY_DIGITS + 7 * 9;
pub(in super::super) const SIGNED_SIGNED: usize = SIGNED_UNSIGNED + 40;
pub(in super::super) const WIDTH: usize = SIGNED_SIGNED + 40;
pub(in super::super) const CONSTRAINTS: usize = 253;
pub(in super::super) const CORRECTION_DIGITS: usize = 4;
pub(in super::super) const BORROWS: usize = CORRECTION_DIGITS + 32;

pub(in super::super) fn product_digits(left: u64, right: u64) -> [F; 64] {
    let product = u128::from(left) * u128::from(right);
    std::array::from_fn(|index| F(((product >> (2 * index)) & 3) as u64))
}

pub(in super::super) fn correction_witness(input: u64, subtract: u64) -> [F; 40] {
    let mut row = [F::ZERO; 40];
    let result = input.wrapping_sub(subtract);
    let mut borrow = 0;
    for limb in 0..4 {
        row[limb] = F((result >> (16 * limb)) & 0xffff);
        let a = (input >> (16 * limb)) & 0xffff;
        let b = (subtract >> (16 * limb)) & 0xffff;
        borrow = u64::from(a < b + borrow);
        row[BORROWS + limb] = F(borrow);
    }
    word::fill_digits(&mut row[CORRECTION_DIGITS..BORROWS], result);
    row
}

pub(in super::super) fn witness(left: u64, right: u64, digits: &[F], active: bool) -> [F; WIDTH] {
    let mut bank = [F::ZERO; WIDTH];
    for limb in 0..8 {
        bank[PRODUCT + limb] = word::pack(&digits[8 * limb..8 * (limb + 1)], 2);
    }
    if active {
        let mut carry = 0_u64;
        for k in 0..8 {
            let sum = (0..4)
                .filter(|i| k >= *i && k - *i < 4)
                .fold(carry, |sum, i| {
                    sum + ((left >> (16 * i)) & 0xffff) * ((right >> (16 * (k - i))) & 0xffff)
                });
            carry = sum >> 16;
            if k < 7 {
                bank[CARRY + k] = F(carry);
                word::fill_digits(
                    &mut bank[CARRY_DIGITS + 9 * k..CARRY_DIGITS + 9 * (k + 1)],
                    carry,
                );
            } else {
                debug_assert_eq!(carry, 0);
            }
        }
    }
    let high = (0..4).fold(0_u64, |word, limb| {
        word | bank[PRODUCT + 4 + limb].0 << (16 * limb)
    });
    let first_subtract = if left >> 63 != 0 { right } else { 0 };
    let first = correction_witness(high, first_subtract);
    let intermediate = high.wrapping_sub(first_subtract);
    let second = correction_witness(intermediate, if right >> 63 != 0 { left } else { 0 });
    bank[SIGNED_UNSIGNED..SIGNED_SIGNED].copy_from_slice(&first);
    bank[SIGNED_SIGNED..].copy_from_slice(&second);
    bank
}

fn correction_residues(
    out: &mut Vec<F>,
    bank: &[F],
    input: &[F],
    sources: Sources<'_>,
    operand: usize,
    sign: F,
    enabled: F,
) {
    out.extend(
        bank[CORRECTION_DIGITS..BORROWS]
            .iter()
            .copied()
            .map(word::radix4),
    );
    for limb in 0..4 {
        out.push(bank[limb].sub(word::pack(
            &bank[CORRECTION_DIGITS + 8 * limb..CORRECTION_DIGITS + 8 * (limb + 1)],
            2,
        )));
        let borrow = bank[BORROWS + limb];
        let previous = if limb == 0 {
            F::ZERO
        } else {
            bank[BORROWS + limb - 1]
        };
        out.push(bit(borrow));
        out.push(
            enabled.mul(
                input[limb]
                    .sub(sign.mul(sources.limb(operand, limb)))
                    .sub(previous)
                    .sub(bank[limb])
                    .add(borrow.mul(F(1 << 16))),
            ),
        );
    }
}

/// Code-derived modes and verifier-owned terminal selection. Success is the
/// selected division/square mode minus its fixed trap flag, never a witness bit.
pub(in super::super) struct Selection {
    pub(in super::super) multiply: F,
    pub(in super::super) division: F,
    pub(in super::super) square: F,
    pub(in super::super) signed: F,
    pub(in super::super) success: F,
    pub(in super::super) quotient: [F; 4],
}

pub(in super::super) fn append_residues(
    out: &mut Vec<F>,
    bank: &[F],
    digits: &[F],
    sources: Sources<'_>,
    selection: Selection,
) {
    let Selection {
        multiply: active,
        division,
        square,
        signed,
        success,
        quotient,
    } = selection;
    let initial = out.len();
    out.extend(digits.iter().copied().map(word::radix4));
    for limb in 0..8 {
        out.push(bank[PRODUCT + limb].sub(word::pack(&digits[8 * limb..8 * (limb + 1)], 2)));
    }
    out.extend(
        bank[CARRY_DIGITS..SIGNED_UNSIGNED]
            .iter()
            .copied()
            .map(word::radix4),
    );
    for carry in 0..7 {
        out.push(bank[CARRY + carry].sub(word::pack(
            &bank[CARRY_DIGITS + 9 * carry..CARRY_DIGITS + 9 * (carry + 1)],
            2,
        )));
        out.push(F::ONE.sub(active).sub(success).mul(bank[CARRY + carry]));
    }
    for k in 0..8 {
        let incoming = if k == 0 { F::ZERO } else { bank[CARRY + k - 1] };
        let outgoing = if k == 7 { F::ZERO } else { bank[CARRY + k] };
        let product = (0..4)
            .filter(|i| k >= *i && k - *i < 4)
            .fold(incoming, |sum, i| {
                sum.add(sources.limb(0, i).mul(sources.limb(1, k - i)))
            });
        let division_product = (0..4)
            .filter(|i| k >= *i && k - *i < 4)
            .fold(incoming, |sum, i| {
                sum.add(bank[SIGNED_SIGNED + i].mul(quotient[k - i]))
            });
        let tail = bank[PRODUCT + k].add(outgoing.mul(F(1 << 16)));
        out.push(
            active
                .mul(product.sub(tail))
                .add(success.mul(division_product.sub(tail))),
        );
    }
    correction_residues(
        out,
        &bank[SIGNED_UNSIGNED..SIGNED_SIGNED],
        &bank[PRODUCT + 4..PRODUCT + 8],
        sources,
        1,
        sources.sign(0),
        F::ONE.sub(division).sub(square),
    );
    correction_residues(
        out,
        &bank[SIGNED_SIGNED..],
        &bank[SIGNED_UNSIGNED..SIGNED_UNSIGNED + 4],
        sources,
        0,
        sources.sign(1),
        F::ONE.sub(division).sub(square),
    );
    for (operand, offset) in [(0, SIGNED_UNSIGNED), (1, SIGNED_SIGNED)] {
        for limb in 0..4 {
            let source = sources.limb(operand, limb);
            let incoming = if limb == 0 {
                F::ZERO
            } else {
                bank[offset + BORROWS + limb - 1]
            };
            out.push(
                division.mul(
                    source
                        .sub(F(2).mul(signed).mul(sources.sign(operand)).mul(source))
                        .sub(incoming)
                        .sub(bank[offset + limb])
                        .add(F(1 << 16).mul(bank[offset + BORROWS + limb])),
                ),
            );
        }
    }
    debug_assert_eq!(out.len() - initial, CONSTRAINTS);
}

/// MUL, MULHU, MULHSU and MULH respectively; the caller derives this from fetch.
pub(in super::super) fn result_half(bank: &[F], kind: usize, half: usize) -> F {
    let offset = [PRODUCT, PRODUCT + 4, SIGNED_UNSIGNED, SIGNED_SIGNED][kind] + 2 * half;
    bank[offset].add(bank[offset + 1].mul(F(1 << 16)))
}
