//! Signed division rounded toward positive infinity from the exact DIV relation.
//!
//! The quotient/remainder bank already proves signed truncation and both native
//! arithmetic traps. The ABS workspace proves a one-unit correction iff the
//! remainder is nonzero and the operand signs agree. Its limbs and carries are
//! range constrained by the shared ABS owner, including on trapped rows.

use super::{F, absolute, division, word};

pub(super) const CONSTRAINTS: usize = 10;
const NONZERO: usize = 40;
pub(in super::super) const INVERSE: usize = 41;
const INCREMENT: usize = 42;

pub(in super::super) fn witness(quotient: &[F]) -> [F; absolute::WIDTH] {
    let mut bank = [F::ZERO; absolute::WIDTH];
    let delta = quotient[division::REMAINDER + 4..division::REMAINDER + 36]
        .iter()
        .copied()
        .fold(F::ZERO, F::add);
    let nonzero = delta != F::ZERO;
    let increment = nonzero && quotient[division::QUOTIENT_NEGATIVE] == F::ZERO;
    let mut carry = u64::from(increment);
    let mut result = 0_u64;
    for limb in 0..4 {
        let sum = quotient[division::QUOTIENT_RESULT + limb].0 + carry;
        bank[limb] = F(sum & 0xffff);
        result |= bank[limb].0 << (16 * limb);
        carry = sum >> 16;
        bank[36 + limb] = F(carry);
    }
    word::fill_digits(&mut bank[4..36], result);
    bank[NONZERO] = F(u64::from(nonzero));
    bank[INVERSE] = delta.inv().unwrap_or(F::ZERO);
    bank[INCREMENT] = F(u64::from(increment));
    bank
}

pub(in super::super) fn append_residues(out: &mut Vec<F>, bank: &[F], quotient: &[F], active: F) {
    let initial = out.len();
    // This sum is in 0..=96: a nonzero 64-bit remainder cannot alias field zero.
    let delta = quotient[division::REMAINDER + 4..division::REMAINDER + 36]
        .iter()
        .copied()
        .fold(F::ZERO, F::add);
    out.push(active.mul(delta.mul(bank[INVERSE]).sub(bank[NONZERO])));
    out.push(active.mul(delta.mul(F::ONE.sub(bank[NONZERO]))));
    out.push(active.mul(F::ONE.sub(bank[NONZERO]).mul(bank[INVERSE])));
    out.push(active.mul(
        bank[INCREMENT].sub(bank[NONZERO].mul(F::ONE.sub(quotient[division::QUOTIENT_NEGATIVE]))),
    ));
    for limb in 0..4 {
        let incoming = if limb == 0 {
            bank[INCREMENT]
        } else {
            bank[36 + limb - 1]
        };
        out.push(
            active.mul(
                quotient[division::QUOTIENT_RESULT + limb]
                    .add(incoming)
                    .sub(bank[limb])
                    .sub(F(1 << 16).mul(bank[36 + limb])),
            ),
        );
    }
    // With a nonzero same-sign remainder the quotient is strictly below i64::MAX;
    // the checked native increment cannot overflow. Enforce that fact directly.
    out.push(active.mul(bank[39]));
    out.push(active.mul(bank[43]));
    debug_assert_eq!(out.len() - initial, CONSTRAINTS);
}
