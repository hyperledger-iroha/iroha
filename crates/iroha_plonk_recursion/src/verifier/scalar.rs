//! Bounded foreign-scalar arithmetic and exact S6/87-bit limb bridges.
//!
//! The bridge checks three bounded integer equalities. It never identifies a
//! foreign scalar by its residue in the circuit field, which would alias the
//! two Pasta moduli. Internal values may be unreduced integers congruent to the
//! scalar; comparisons and transfer to ECC/transcript cells canonicalize them.
//! Zero denominators select one before
//! inversion and return a separately constrained nonzero bit.
//!
//! Delaying normalization does not widen any FF gate's proven envelope. The
//! existing FF add/sub/neg routines track nonnegative limb bounds, reduce before
//! a result would exceed `2^94 - 1`, and subtraction pads by a fixed multiple of
//! the scalar modulus that dominates each subtrahend limb. Multiplication calls
//! `make_admissible`: it reduces until both limbs remain in that envelope and
//! `max(a) max(b) < m 2^261`. Division similarly checks its existing padding and
//! quotient bound. Thus every actual fused block still satisfies the original
//! carry and quotient preconditions; no wider carry bound is assumed here.
//!
//! More explicitly, for each limb with tracked bounds `A_i,B_i`, addition has
//! bound `A_i+B_i`. Subtraction chooses a structural multiple `K=t*m` with
//! limbs `K_i>=B_i`, so `0<=a_i+K_i-b_i<=A_i+K_i`; negation has
//! `0<=K_i-b_i<=K_i`. Each accepted bound is at most `2^94-1`, far below
//! either native modulus, so those glue equations cannot wrap. Selection takes
//! the componentwise maximum of the two bounds. The existing reduction path
//! writes a proper result with bounds `(2^87-1,2^87-1,2^82-1)` before retrying
//! an otherwise inadmissible operation. The actual integer may differ from
//! its canonical scalar by a multiple of `m`; its native-field residue must
//! therefore never be used as the scalar value. Only the canonical integer
//! bridge exports to S6, ECC scalar bits, transcript absorption or public cells.

use crate::codec::ScalarCells;
use core::marker::PhantomData;
use ff::{Field, PrimeField};
use iroha_pasta::{PastaCurve, PastaField};
use iroha_plonk::frontend::{Error, Region};
use iroha_plonk_gadgets::{
    Bit,
    ff::{FfChip, FfValue, ForeignModulus, Nat},
    range::u128::UintChip,
};

pub type Scalar<C> = FfValue<<C as PastaCurve>::Base>;

#[derive(Debug)]
pub struct Arithmetic<C: PastaCurve> {
    pub ff: FfChip<C::Base>,
    marker: PhantomData<C>,
}

impl<C: PastaCurve> Arithmetic<C> {
    pub fn new(ff: FfChip<C::Base>) -> Self {
        Self {
            ff,
            marker: PhantomData,
        }
    }
    pub fn modulus() -> ForeignModulus {
        if C::ScalarExt::MODULUS == iroha_pasta::Fp::MODULUS {
            ForeignModulus::PASTA_FP
        } else {
            ForeignModulus::PASTA_FQ
        }
    }
    pub fn constant(
        &self,
        uint: &mut UintChip<'_, C::Base>,
        region: &mut Region<'_, C::Base>,
        value: C::ScalarExt,
    ) -> Result<Scalar<C>, Error> {
        self.ff.constant(
            uint.glue(),
            region,
            Self::modulus(),
            &Nat::from_words(value.to_canonical_limbs()),
        )
    }
    pub fn import(
        &mut self,
        uint: &mut UintChip<'_, C::Base>,
        region: &mut Region<'_, C::Base>,
        value: &ScalarCells<C>,
    ) -> Result<Scalar<C>, Error> {
        self.ff.import_s6(uint, region, value.canonical())
    }
    pub fn export(
        &mut self,
        uint: &mut UintChip<'_, C::Base>,
        region: &mut Region<'_, C::Base>,
        value: &Scalar<C>,
    ) -> Result<ScalarCells<C>, Error> {
        let value = self.ff.export_s6(uint, region, value)?;
        ScalarCells::from_canonical(uint, region, value)
    }
    pub fn add(
        &mut self,
        uint: &mut UintChip<'_, C::Base>,
        region: &mut Region<'_, C::Base>,
        a: &Scalar<C>,
        b: &Scalar<C>,
    ) -> Result<Scalar<C>, Error> {
        self.ff.add(uint.glue(), region, a, b)
    }
    pub fn sub(
        &mut self,
        uint: &mut UintChip<'_, C::Base>,
        region: &mut Region<'_, C::Base>,
        a: &Scalar<C>,
        b: &Scalar<C>,
    ) -> Result<Scalar<C>, Error> {
        self.ff.sub(uint.glue(), region, a, b)
    }
    pub fn neg(
        &mut self,
        uint: &mut UintChip<'_, C::Base>,
        region: &mut Region<'_, C::Base>,
        value: &Scalar<C>,
    ) -> Result<Scalar<C>, Error> {
        self.ff.neg(uint.glue(), region, value)
    }
    pub fn mul(
        &mut self,
        region: &mut Region<'_, C::Base>,
        a: &Scalar<C>,
        b: &Scalar<C>,
    ) -> Result<Scalar<C>, Error> {
        self.ff.mul(region, a, b)
    }
    pub fn square(
        &mut self,
        region: &mut Region<'_, C::Base>,
        value: &Scalar<C>,
    ) -> Result<Scalar<C>, Error> {
        self.mul(region, value, value)
    }
    pub fn select(
        &self,
        uint: &mut UintChip<'_, C::Base>,
        region: &mut Region<'_, C::Base>,
        bit: &Bit<C::Base>,
        a: &Scalar<C>,
        b: &Scalar<C>,
    ) -> Result<Scalar<C>, Error> {
        self.ff.select(uint.glue(), region, bit, a, b)
    }
    pub fn equal(
        &mut self,
        uint: &mut UintChip<'_, C::Base>,
        region: &mut Region<'_, C::Base>,
        a: &Scalar<C>,
        b: &Scalar<C>,
    ) -> Result<Bit<C::Base>, Error> {
        let a = self.ff.assert_canonical(region, a)?;
        let b = self.ff.assert_canonical(region, b)?;
        let mut matches = uint.glue().is_equal(region, &a.limbs()[0], &b.limbs()[0])?;
        for (a, b) in a.limbs().iter().zip(b.limbs()).skip(1) {
            let next = uint.glue().is_equal(region, a, b)?;
            matches = uint.glue().and(region, &matches, &next)?;
        }
        Ok(matches)
    }
    pub fn nonzero(
        &mut self,
        uint: &mut UintChip<'_, C::Base>,
        region: &mut Region<'_, C::Base>,
        value: &Scalar<C>,
    ) -> Result<Bit<C::Base>, Error> {
        let zero = self.constant(uint, region, C::ScalarExt::ZERO)?;
        let is_zero = self.equal(uint, region, value, &zero)?;
        uint.glue().not(region, &is_zero)
    }
    pub fn inverse(
        &mut self,
        uint: &mut UintChip<'_, C::Base>,
        region: &mut Region<'_, C::Base>,
        value: &Scalar<C>,
    ) -> Result<(Scalar<C>, Bit<C::Base>), Error> {
        let nonzero = self.nonzero(uint, region, value)?;
        let one = self.constant(uint, region, C::ScalarExt::ONE)?;
        let safe = self.select(uint, region, &nonzero, value, &one)?;
        let out = self.ff.inverse(region, &safe)?;
        Ok((out, nonzero))
    }
    pub fn pow(
        &mut self,
        uint: &mut UintChip<'_, C::Base>,
        region: &mut Region<'_, C::Base>,
        value: &Scalar<C>,
        exponent: u64,
    ) -> Result<Scalar<C>, Error> {
        let mut out = self.constant(uint, region, C::ScalarExt::ONE)?;
        let mut power = value.clone();
        let mut exponent = exponent;
        while exponent != 0 {
            if exponent & 1 != 0 {
                out = self.mul(region, &out, &power)?;
            }
            exponent >>= 1;
            if exponent != 0 {
                power = self.square(region, &power)?;
            }
        }
        Ok(out)
    }
}
