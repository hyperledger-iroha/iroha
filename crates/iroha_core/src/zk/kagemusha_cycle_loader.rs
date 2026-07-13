//! Fixed Pasta-cycle field and curve instructions for the Kagemusha leapfrog verifier.
//!
//! This is the reviewed fixed-width loader shared by both parities. It represents
//! the proof curve's scalar field as three canonical 86-bit limbs in the opposite
//! Pasta field and constrains exact coordinate reduction for the Poseidon transcript.

use std::{marker::PhantomData, ops::Deref};

use halo2_base::{
    AssignedValue,
    QuantumCell::{Constant, Existing},
    gates::{GateInstructions, flex_gate::threads::SinglePhaseCoreManager},
    halo2_proofs::{arithmetic::Field as _, halo2curves::CurveAffine},
    utils::{
        BigPrimeField, CurveAffineExt, biguint_to_fe, decompose_biguint, fe_to_biguint, modulus,
    },
};
use halo2_ecc::{
    bigint::ProperCrtUint,
    ecc::{EcPoint as AssignedEcPoint, EccChip},
    fields::{FieldChip, Selectable, fp::FpChip},
};
use snark_verifier::{
    Error,
    loader::halo2::{EccInstructions, IntegerInstructions},
    system::halo2::transcript::halo2::NativeEncoding,
};

/// Limb width chosen so products of three-limb Pasta integers retain
/// ample native-field headroom in either parity.
pub(super) const LIMB_BITS: usize = 86;
/// Both Pasta fields fit in three 86-bit limbs.
pub(super) const LIMBS: usize = 3;

type Outer<C> = <C as CurveAffine>::Base;
type Inner<C> = <C as CurveAffine>::ScalarExt;
type Integer<C> = ProperCrtUint<Outer<C>>;
type Point<C> = AssignedEcPoint<Outer<C>, Integer<C>>;

/// Non-native scalar instructions used by `snark-verifier`.
#[derive(Clone, Debug)]
pub(super) struct PastaCycleScalarChip<'chip, C>
where
    C: CurveAffineExt,
    Outer<C>: BigPrimeField,
    Inner<C>: BigPrimeField,
{
    field: &'chip FpChip<'chip, Outer<C>, Inner<C>>,
    _curve: PhantomData<C>,
}

impl<'chip, C> PastaCycleScalarChip<'chip, C>
where
    C: CurveAffineExt,
    Outer<C>: BigPrimeField,
    Inner<C>: BigPrimeField,
{
    fn new(field: &'chip FpChip<'chip, Outer<C>, Inner<C>>) -> Self {
        Self {
            field,
            _curve: PhantomData,
        }
    }

    fn canonical(&self, ctx: &mut halo2_base::Context<Outer<C>>, value: Integer<C>) -> Integer<C> {
        self.field.enforce_less_than(ctx, value).into()
    }

    fn add(
        &self,
        ctx: &mut halo2_base::Context<Outer<C>>,
        lhs: Integer<C>,
        rhs: Integer<C>,
    ) -> Integer<C> {
        let sum = self.field.add_no_carry(ctx, lhs, rhs);
        self.field.carry_mod(ctx, sum)
    }

    fn mul(
        &self,
        ctx: &mut halo2_base::Context<Outer<C>>,
        lhs: Integer<C>,
        rhs: Integer<C>,
    ) -> Integer<C> {
        self.field.mul(ctx, lhs, rhs)
    }
}

impl<C> IntegerInstructions<Inner<C>> for PastaCycleScalarChip<'_, C>
where
    C: CurveAffineExt,
    Outer<C>: BigPrimeField,
    Inner<C>: BigPrimeField,
{
    type Context = SinglePhaseCoreManager<Outer<C>>;
    type AssignedCell = AssignedValue<Outer<C>>;
    type AssignedInteger = Integer<C>;

    fn assign_integer(&self, ctx: &mut Self::Context, integer: Inner<C>) -> Self::AssignedInteger {
        let value = self.field.load_private(ctx.main(), integer);
        self.canonical(ctx.main(), value)
    }

    fn assign_constant(&self, ctx: &mut Self::Context, integer: Inner<C>) -> Self::AssignedInteger {
        self.field.load_constant(ctx.main(), integer)
    }

    fn sum_with_coeff_and_const(
        &self,
        ctx: &mut Self::Context,
        values: &[(Inner<C>, impl Deref<Target = Self::AssignedInteger>)],
        constant: Inner<C>,
    ) -> Self::AssignedInteger {
        let ctx = ctx.main();
        let mut sum = self.field.load_constant(ctx, constant);
        for (coefficient, value) in values {
            let coefficient = self.field.load_constant(ctx, *coefficient);
            let term = self.mul(ctx, value.deref().clone(), coefficient);
            sum = self.add(ctx, sum, term);
        }
        sum
    }

    fn sum_products_with_coeff_and_const(
        &self,
        ctx: &mut Self::Context,
        values: &[(
            Inner<C>,
            impl Deref<Target = Self::AssignedInteger>,
            impl Deref<Target = Self::AssignedInteger>,
        )],
        constant: Inner<C>,
    ) -> Self::AssignedInteger {
        let ctx = ctx.main();
        let mut sum = self.field.load_constant(ctx, constant);
        for (coefficient, lhs, rhs) in values {
            let product = self.mul(ctx, lhs.deref().clone(), rhs.deref().clone());
            let coefficient = self.field.load_constant(ctx, *coefficient);
            let term = self.mul(ctx, product, coefficient);
            sum = self.add(ctx, sum, term);
        }
        sum
    }

    fn sub(
        &self,
        ctx: &mut Self::Context,
        lhs: &Self::AssignedInteger,
        rhs: &Self::AssignedInteger,
    ) -> Self::AssignedInteger {
        let difference = self
            .field
            .sub_no_carry(ctx.main(), lhs.clone(), rhs.clone());
        self.field.carry_mod(ctx.main(), difference)
    }

    fn neg(&self, ctx: &mut Self::Context, value: &Self::AssignedInteger) -> Self::AssignedInteger {
        self.field.negate(ctx.main(), value.clone())
    }

    fn invert(
        &self,
        ctx: &mut Self::Context,
        value: &Self::AssignedInteger,
    ) -> Self::AssignedInteger {
        let one = self.field.load_constant(ctx.main(), Inner::<C>::ONE);
        self.field.divide(ctx.main(), one, value.clone())
    }

    fn assert_equal(
        &self,
        ctx: &mut Self::Context,
        lhs: &Self::AssignedInteger,
        rhs: &Self::AssignedInteger,
    ) {
        self.field
            .assert_equal(ctx.main(), lhs.clone(), rhs.clone());
    }

    fn pow_var(
        &self,
        ctx: &mut Self::Context,
        base: &Self::AssignedInteger,
        exponent: &Self::AssignedInteger,
        max_bits: usize,
    ) -> Self::AssignedInteger {
        assert!(max_bits <= LIMB_BITS * LIMBS);
        let exponent = self.canonical(ctx.main(), exponent.clone());
        let gate = self.field.gate();
        let mut bits = Vec::with_capacity(LIMB_BITS * LIMBS);
        for limb in exponent.limbs() {
            bits.extend(gate.num_to_bits(ctx.main(), *limb, LIMB_BITS));
        }
        for bit in bits.iter().skip(max_bits) {
            gate.assert_is_const(ctx.main(), bit, &Outer::<C>::ZERO);
        }

        let mut result = self.field.load_constant(ctx.main(), Inner::<C>::ONE);
        let mut power = base.clone();
        for bit in bits.into_iter().take(max_bits) {
            let multiplied = self.mul(ctx.main(), result.clone(), power.clone());
            result = self.field.select(ctx.main(), multiplied, result, bit);
            power = self.mul(ctx.main(), power.clone(), power);
        }
        result
    }
}

/// Opposite-field curve instructions for one Pasta parity.
#[derive(Clone, Debug)]
pub(super) struct PastaCycleEccChip<'chip, C>
where
    C: CurveAffineExt,
    Outer<C>: BigPrimeField,
    Inner<C>: BigPrimeField,
{
    base: &'chip FpChip<'chip, Outer<C>, Outer<C>>,
    scalar: PastaCycleScalarChip<'chip, C>,
}

impl<'chip, C> PastaCycleEccChip<'chip, C>
where
    C: CurveAffineExt,
    Outer<C>: BigPrimeField,
    Inner<C>: BigPrimeField,
{
    pub(super) fn new(
        base: &'chip FpChip<'chip, Outer<C>, Outer<C>>,
        scalar: &'chip FpChip<'chip, Outer<C>, Inner<C>>,
    ) -> Self {
        Self {
            base,
            scalar: PastaCycleScalarChip::new(scalar),
        }
    }

    fn curve(&self) -> EccChip<'_, Outer<C>, FpChip<'chip, Outer<C>, Outer<C>>> {
        EccChip::new(self.base)
    }

    fn canonical_scalar(
        &self,
        ctx: &mut SinglePhaseCoreManager<Outer<C>>,
        scalar: &Integer<C>,
    ) -> Vec<AssignedValue<Outer<C>>> {
        self.scalar
            .canonical(ctx.main(), scalar.clone())
            .limbs()
            .to_vec()
    }

    pub(super) fn canonical_point(
        &self,
        ctx: &mut SinglePhaseCoreManager<Outer<C>>,
        point: Point<C>,
    ) -> Point<C> {
        let x = self.base.enforce_less_than(ctx.main(), point.x).into();
        let y = self.base.enforce_less_than(ctx.main(), point.y).into();
        AssignedEcPoint::new(x, y)
    }

    /// Convert a canonical base-field coordinate to the exact residue
    /// used by the native Poseidon transcript.  The quotient and every
    /// radix carry are boolean-constrained, so an outer-field wrap
    /// cannot create a second reduction witness.
    pub(super) fn coordinate_to_scalar(
        &self,
        ctx: &mut SinglePhaseCoreManager<Outer<C>>,
        coordinate: Integer<C>,
    ) -> Integer<C> {
        let ctx = ctx.main();
        self.base.enforce_less_than_p(ctx, coordinate.clone());
        let coordinate_value = self.base.get_assigned_value(coordinate.as_ref());
        let coordinate_integer = fe_to_biguint(&coordinate_value);
        let scalar_modulus = modulus::<Inner<C>>();
        let quotient = &coordinate_integer / &scalar_modulus;
        assert!(
            quotient.bits() <= 1,
            "Pasta cross-field quotient is boolean"
        );
        let residue_integer = &coordinate_integer % &scalar_modulus;
        let residue_value = biguint_to_fe::<Inner<C>>(&residue_integer);
        let residue = self.scalar.field.load_private(ctx, residue_value);
        self.scalar.field.enforce_less_than_p(ctx, residue.clone());

        let quotient_u64 = quotient.to_u64_digits().first().copied().unwrap_or(0);
        let quotient_cell = ctx.load_witness(Outer::<C>::from(quotient_u64));
        self.base.gate().assert_bit(ctx, quotient_cell);

        let one = quotient.clone() - quotient.clone() + 1u64;
        let radix_integer = &one << LIMB_BITS;
        let limb_mask = &radix_integer - &one;
        let radix = biguint_to_fe::<Outer<C>>(&radix_integer);
        let modulus_limbs = decompose_biguint::<Outer<C>>(&scalar_modulus, LIMBS, LIMB_BITS);
        let mut carry_integer = quotient.clone() - quotient.clone();
        let zero = ctx.load_zero();
        let mut carry_cell = zero;
        for index in 0..LIMBS {
            let shift = LIMB_BITS * index;
            let residue_limb = (&residue_integer >> shift) & &limb_mask;
            let modulus_limb = (&scalar_modulus >> shift) & &limb_mask;
            let coordinate_limb = (&coordinate_integer >> shift) & &limb_mask;
            let sum = residue_limb + &quotient * modulus_limb + carry_integer.clone();
            assert_eq!(&sum & &limb_mask, coordinate_limb);
            carry_integer = &sum >> LIMB_BITS;
            let carry_u64 = carry_integer.to_u64_digits().first().copied().unwrap_or(0);
            assert!(carry_u64 <= 1);
            let next_carry = ctx.load_witness(Outer::<C>::from(carry_u64));
            self.base.gate().assert_bit(ctx, next_carry);

            let quotient_modulus =
                self.base
                    .gate()
                    .mul(ctx, Existing(quotient_cell), Constant(modulus_limbs[index]));
            let with_residue = self.base.gate().add(
                ctx,
                Existing(residue.limbs()[index]),
                Existing(quotient_modulus),
            );
            let left = self
                .base
                .gate()
                .add(ctx, Existing(with_residue), Existing(carry_cell));
            let carry_radix = self
                .base
                .gate()
                .mul(ctx, Existing(next_carry), Constant(radix));
            let right = self.base.gate().add(
                ctx,
                Existing(coordinate.limbs()[index]),
                Existing(carry_radix),
            );
            ctx.constrain_equal(&left, &right);
            carry_cell = next_carry;
        }
        self.base
            .gate()
            .assert_is_const(ctx, &carry_cell, &Outer::<C>::ZERO);
        residue
    }
}

impl<'chip, C> EccInstructions<C> for PastaCycleEccChip<'chip, C>
where
    C: CurveAffineExt,
    Outer<C>: BigPrimeField,
    Inner<C>: BigPrimeField,
{
    type Context = SinglePhaseCoreManager<Outer<C>>;
    type ScalarChip = PastaCycleScalarChip<'chip, C>;
    type AssignedCell = AssignedValue<Outer<C>>;
    type AssignedScalar = Integer<C>;
    type AssignedEcPoint = Point<C>;

    fn scalar_chip(&self) -> &Self::ScalarChip {
        &self.scalar
    }

    fn assign_constant(&self, ctx: &mut Self::Context, point: C) -> Self::AssignedEcPoint {
        self.curve().assign_constant_point(ctx.main(), point)
    }

    fn assign_point(&self, ctx: &mut Self::Context, point: C) -> Self::AssignedEcPoint {
        self.curve().assign_point(ctx.main(), point)
    }

    fn sum_with_const(
        &self,
        ctx: &mut Self::Context,
        values: &[impl Deref<Target = Self::AssignedEcPoint>],
        constant: C,
    ) -> Self::AssignedEcPoint {
        let constant = (!bool::from(constant.is_identity()))
            .then(|| self.curve().assign_constant_point(ctx.main(), constant));
        self.curve().sum::<C>(
            ctx.main(),
            constant
                .into_iter()
                .chain(values.iter().map(|point| point.deref().clone())),
        )
    }

    fn fixed_base_msm(
        &mut self,
        ctx: &mut Self::Context,
        pairs: &[(impl Deref<Target = Self::AssignedScalar>, C)],
    ) -> Self::AssignedEcPoint {
        let (scalars, points): (Vec<_>, Vec<_>) = pairs
            .iter()
            .filter(|(_, point)| !bool::from(point.is_identity()))
            .map(|(scalar, point)| (self.canonical_scalar(ctx, scalar), *point))
            .unzip();
        if points.is_empty() {
            return self
                .curve()
                .assign_constant_point(ctx.main(), C::identity());
        }
        self.curve()
            .fixed_base_msm::<C>(ctx, &points, scalars, LIMB_BITS)
    }

    fn variable_base_msm(
        &mut self,
        ctx: &mut Self::Context,
        pairs: &[(
            impl Deref<Target = Self::AssignedScalar>,
            impl Deref<Target = Self::AssignedEcPoint>,
        )],
    ) -> Self::AssignedEcPoint {
        if pairs.is_empty() {
            return self
                .curve()
                .assign_constant_point(ctx.main(), C::identity());
        }
        let scalars = pairs
            .iter()
            .map(|(scalar, _)| self.canonical_scalar(ctx, scalar))
            .collect::<Vec<_>>();
        let points = pairs
            .iter()
            .map(|(_, point)| point.deref().clone())
            .collect::<Vec<_>>();
        self.curve()
            .variable_base_msm::<C>(ctx, &points, scalars, LIMB_BITS)
    }

    fn assert_equal(
        &self,
        ctx: &mut Self::Context,
        lhs: &Self::AssignedEcPoint,
        rhs: &Self::AssignedEcPoint,
    ) {
        self.curve()
            .assert_equal(ctx.main(), lhs.clone(), rhs.clone());
    }
}

impl<C> NativeEncoding<C> for PastaCycleEccChip<'_, C>
where
    C: CurveAffineExt,
    Outer<C>: BigPrimeField,
    Inner<C>: BigPrimeField,
{
    fn encode(
        &self,
        ctx: &mut Self::Context,
        point: &Self::AssignedEcPoint,
    ) -> Result<Vec<Self::AssignedScalar>, Error> {
        let point = self.canonical_point(ctx, point.clone());
        let identity = self.base.is_zero(ctx.main(), &point.y);
        self.base
            .gate()
            .assert_is_const(ctx.main(), &identity, &Outer::<C>::ZERO);
        Ok(vec![
            self.coordinate_to_scalar(ctx, point.x),
            self.coordinate_to_scalar(ctx, point.y),
        ])
    }
}
