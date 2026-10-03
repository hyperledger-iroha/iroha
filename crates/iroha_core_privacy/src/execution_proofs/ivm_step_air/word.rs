//! One canonical Boolean decomposition of the two pre-read virtual source words.
//!
//! Every arithmetic bank consumes this view. The caller owns both the Boolean
//! constraints and the links to its canonical operand reads; limbs, halves and
//! radix-four digits are linear expressions, never independent witnesses.

use super::{F, bit};

pub(super) const WIDTH: usize = 128;

#[derive(Clone, Copy)]
pub(super) struct Sources<'a>(&'a [F]);

impl<'a> Sources<'a> {
    pub(super) fn new(bits: &'a [F]) -> Self {
        assert_eq!(bits.len(), WIDTH);
        Self(bits)
    }

    pub(super) fn bits(self, operand: usize) -> &'a [F] {
        &self.0[operand * 64..(operand + 1) * 64]
    }

    pub(super) fn limb(self, operand: usize, limb: usize) -> F {
        pack(&self.bits(operand)[limb * 16..(limb + 1) * 16], 1)
    }

    pub(super) fn half(self, operand: usize, half: usize) -> F {
        pack(&self.bits(operand)[half * 32..(half + 1) * 32], 1)
    }

    pub(super) fn sign(self, operand: usize) -> F {
        self.bits(operand)[63]
    }

    pub(super) fn append_residues(
        self,
        out: &mut impl crate::execution_proofs::ivm_step_air::residues::Sink,
    ) {
        out.extend(self.0.iter().copied().map(bit));
    }
}

pub(super) fn witness(left: u64, right: u64) -> [F; WIDTH] {
    std::array::from_fn(|index| F(([left, right][index / 64] >> (index % 64)) & 1))
}

pub(super) fn radix4(value: F) -> F {
    value
        .mul(value.sub(F::ONE))
        .mul(value.sub(F(2)))
        .mul(value.sub(F(3)))
}

pub(super) fn pack(values: &[F], digit_bits: usize) -> F {
    values
        .iter()
        .enumerate()
        .fold(F::ZERO, |sum, (index, value)| {
            sum.add(value.mul(F(1 << (digit_bits * index))))
        })
}

pub(super) fn fill_digits(digits: &mut [F], value: u64) {
    for (index, digit) in digits.iter_mut().enumerate() {
        *digit = F((value >> (2 * index)) & 3);
    }
}
