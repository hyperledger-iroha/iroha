//! Circuit-native Pasta-cycle recursion for Kagemusha.
//!
//! Each parity parses the complete opposite-curve Halo2 proof as circuit
//! advice, recomputes the Poseidon Fiat--Shamir transcript, constrains the full
//! verification key and public instances, evaluates every PLONK argument, and
//! emits a canonical IPA accumulator. Final verification decides both carried
//! accumulators against the content-addressed Pasta parameter artifacts.

#[cfg(test)]
mod tests {
    use halo2_proofs::{
        circuit::{Layouter, SimpleFloorPlanner, Value},
        plonk::{Advice, Circuit, Column, ConstraintSystem, Error as PlonkError, Instance},
    };

    use crate::zk::halo2_backend::{
        Scalar, assign_advice_compat, keygen_pk, keygen_vk, params_new,
    };

    #[derive(Clone, Default)]
    struct PublicValue {
        value: Scalar,
    }

    impl Circuit<Scalar> for PublicValue {
        type Config = (Column<Advice>, Column<Instance>);
        type FloorPlanner = SimpleFloorPlanner;
        type Params = ();

        fn without_witnesses(&self) -> Self {
            Self::default()
        }

        fn configure(meta: &mut ConstraintSystem<Scalar>) -> Self::Config {
            let advice = meta.advice_column();
            let instance = meta.instance_column();
            meta.enable_equality(advice);
            meta.enable_equality(instance);
            (advice, instance)
        }

        fn synthesize(
            &self,
            (advice, instance): Self::Config,
            mut layouter: impl Layouter<Scalar>,
        ) -> Result<(), PlonkError> {
            let cell = layouter.assign_region(
                || "public value",
                |mut region| {
                    let cell = assign_advice_compat(
                        &mut region,
                        || "value",
                        advice,
                        0,
                        || Value::known(self.value),
                    )?;
                    Ok(cell.cell())
                },
            )?;
            layouter.constrain_instance(cell, instance, 0);
            Ok(())
        }
    }

    /// Cycle-native `Halo2Loader` instructions for verifying one Pasta proof
    /// in the scalar field of the opposite Pasta curve.  Curve coordinates are
    /// represented canonically in the outer circuit field while verifier
    /// scalars are represented as three ranged limbs in the other field.
    ///
    /// This module is deliberately generic over the two Pasta parities.  It is
    /// the primitive used by the V3 transition/state design.
    mod pasta_cycle_loader {
        use std::{marker::PhantomData, ops::Deref};

        use halo2_base::{
            AssignedValue,
            QuantumCell::{Constant, Existing},
            gates::{GateInstructions, flex_gate::threads::SinglePhaseCoreManager},
            halo2_proofs::{arithmetic::Field as _, halo2curves::CurveAffine},
            utils::{
                BigPrimeField, CurveAffineExt, biguint_to_fe, decompose_biguint, fe_to_biguint,
                modulus,
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

            fn canonical(
                &self,
                ctx: &mut halo2_base::Context<Outer<C>>,
                value: Integer<C>,
            ) -> Integer<C> {
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

            fn assign_integer(
                &self,
                ctx: &mut Self::Context,
                integer: Inner<C>,
            ) -> Self::AssignedInteger {
                let value = self.field.load_private(ctx.main(), integer);
                self.canonical(ctx.main(), value)
            }

            fn assign_constant(
                &self,
                ctx: &mut Self::Context,
                integer: Inner<C>,
            ) -> Self::AssignedInteger {
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

            fn neg(
                &self,
                ctx: &mut Self::Context,
                value: &Self::AssignedInteger,
            ) -> Self::AssignedInteger {
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
                let modulus_limbs =
                    decompose_biguint::<Outer<C>>(&scalar_modulus, LIMBS, LIMB_BITS);
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

                    let quotient_modulus = self.base.gate().mul(
                        ctx,
                        Existing(quotient_cell),
                        Constant(modulus_limbs[index]),
                    );
                    let with_residue = self.base.gate().add(
                        ctx,
                        Existing(residue.limbs()[index]),
                        Existing(quotient_modulus),
                    );
                    let left =
                        self.base
                            .gate()
                            .add(ctx, Existing(with_residue), Existing(carry_cell));
                    let carry_radix =
                        self.base
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

        impl<'chip, C> NativeEncoding<C> for PastaCycleEccChip<'chip, C>
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
    }

    #[test]
    fn pasta_cycle_cross_field_encoding_is_exact_in_both_parities() {
        use halo2_base::{
            gates::circuit::{BaseCircuitParams, builder::BaseCircuitBuilder},
            halo2_proofs::{
                arithmetic::Field as _,
                dev::MockProver,
                halo2curves::pasta::{EpAffine, EqAffine},
            },
            utils::{biguint_to_fe, decompose_biguint, modulus},
        };
        use halo2_ecc::fields::{FieldChip, fp::FpChip};

        use self::pasta_cycle_loader::{LIMB_BITS, LIMBS, PastaCycleEccChip};

        const K: u32 = 10;
        macro_rules! check_parity {
            ($curve:ty, $outer:ty, $inner:ty) => {{
                let outer_modulus = modulus::<$outer>();
                let inner_modulus = modulus::<$inner>();
                let coordinate_integer = if outer_modulus > inner_modulus {
                    &inner_modulus + 5_u64
                } else {
                    &outer_modulus - 5_u64
                };
                let coordinate_value = biguint_to_fe::<$outer>(&coordinate_integer);
                let expected_integer = &coordinate_integer % &inner_modulus;
                let expected_instances =
                    decompose_biguint::<$outer>(&expected_integer, LIMBS, LIMB_BITS);

                let seed = BaseCircuitParams {
                    k: K as usize,
                    num_advice_per_phase: vec![1],
                    num_lookup_advice_per_phase: vec![1],
                    num_fixed: 1,
                    lookup_bits: Some(K as usize - 1),
                    num_instance_columns: 1,
                };
                let mut circuit = BaseCircuitBuilder::<$outer>::new(false).use_params(seed);
                let range = circuit.range_chip();
                let base = FpChip::<$outer, $outer>::new(&range, LIMB_BITS, LIMBS);
                let scalar = FpChip::<$outer, $inner>::new(&range, LIMB_BITS, LIMBS);
                let chip = PastaCycleEccChip::<$curve>::new(&base, &scalar);
                let coordinate = base.load_private(circuit.pool(0).main(), coordinate_value);
                let encoded = chip.coordinate_to_scalar(circuit.pool(0), coordinate);
                circuit.assigned_instances[0] = encoded.limbs().to_vec();
                let params = circuit.calculate_params(Some(9));
                circuit.set_params(params);

                MockProver::run(K, &circuit, vec![expected_instances.clone()])
                    .expect("cycle cross-field MockProver")
                    .assert_satisfied();

                let mut substituted = expected_instances;
                substituted[0] += <$outer>::ONE;
                let prover = MockProver::run(K, &circuit, vec![substituted])
                    .expect("substituted cross-field MockProver");
                assert!(
                    prover.verify().is_err(),
                    "cross-field transcript residue substitution must reject"
                );
            }};
        }

        check_parity!(
            EqAffine,
            halo2_base::halo2_proofs::halo2curves::pasta::Fq,
            halo2_base::halo2_proofs::halo2curves::pasta::Fp
        );
        check_parity!(
            EpAffine,
            halo2_base::halo2_proofs::halo2curves::pasta::Fp,
            halo2_base::halo2_proofs::halo2curves::pasta::Fq
        );
    }

    /// Compatibility and soundness checks for the Pasta IPA proof wire used by
    /// the circuit verifier.  This module is test-only until the application
    /// circuit emits Poseidon proofs and the recursive verifier circuit is
    /// promoted to an artifact-backed production implementation.
    mod pasta_ipa_poseidon_wire {
        use std::panic::{AssertUnwindSafe, catch_unwind};

        use halo2_base::halo2_proofs::{
            halo2curves::{
                CurveExt as _,
                group::{Curve as _, GroupEncoding},
                pasta::{Eq, EqAffine, Fp},
            },
            plonk::{ProvingKey, create_proof, verify_proof},
            poly::{
                VerificationStrategy as _,
                commitment::{Params as _, ParamsProver as _},
                ipa::{
                    commitment::{IPACommitmentScheme, ParamsIPA},
                    multiopen::{ProverIPA, VerifierIPA},
                },
            },
        };
        use rand_core_06::OsRng;
        use snark_verifier::{
            loader::native::NativeLoader,
            pcs::{
                AccumulationDecider,
                ipa::{Bgh19, IpaAccumulator, IpaAs, IpaDecidingKey, IpaSuccinctVerifyingKey},
            },
            system::halo2::{
                Config, compile,
                strategy::ipa::SingleStrategy as FoldedGeneratorStrategy,
                transcript::halo2::{ChallengeScalar, PoseidonTranscript},
            },
            util::arithmetic::{Domain, root_of_unity},
            verifier::{
                SnarkVerifier,
                plonk::{PlonkSuccinctVerifier, PlonkVerifier},
            },
        };

        use super::PublicValue;
        use crate::zk::halo2_backend::{Scalar, keygen_pk, keygen_vk, params_new};

        const T: usize = 3;
        const RATE: usize = 2;
        const R_F: usize = 8;
        const R_P: usize = 57;
        const SECURE_MDS: usize = 0;
        const INNER_K: u32 = 5;

        type As = IpaAs<EqAffine, Bgh19>;
        type FullVerifier = PlonkVerifier<As>;
        type SuccinctVerifier = PlonkSuccinctVerifier<As>;
        type Transcript<L, S> = PoseidonTranscript<EqAffine, L, S, T, RATE, R_F, R_P>;

        struct Fixture {
            params: ParamsIPA<EqAffine>,
            pk: ProvingKey<EqAffine>,
            protocol: snark_verifier::verifier::plonk::PlonkProtocol<EqAffine>,
            deciding_key: IpaDecidingKey<EqAffine>,
            proof_without_folded_generator: Vec<u8>,
            augmented_proof: Vec<u8>,
            instances: Vec<Vec<Fp>>,
        }

        fn canonical_svk(params: &ParamsIPA<EqAffine>) -> IpaSuccinctVerifyingKey<EqAffine> {
            let hash_to_curve = Eq::hash_to_curve("Halo2-Parameters");
            let w = hash_to_curve(&[1]).to_affine();
            let u = hash_to_curve(&[2]).to_affine();
            IpaSuccinctVerifyingKey::new(
                Domain::new(params.k() as usize, root_of_unity(params.k() as usize)),
                params.get_g()[0],
                u,
                Some(w),
            )
        }

        fn create_poseidon_proof(
            params: &ParamsIPA<EqAffine>,
            pk: &ProvingKey<EqAffine>,
            circuit: PublicValue,
            instances: &[&[&[Scalar]]],
        ) -> Vec<u8> {
            let mut transcript = Transcript::<NativeLoader, _>::new::<SECURE_MDS>(Vec::<u8>::new());
            create_proof::<
                IPACommitmentScheme<EqAffine>,
                ProverIPA<'_, EqAffine>,
                ChallengeScalar<EqAffine>,
                _,
                _,
                _,
            >(params, pk, &[circuit], instances, OsRng, &mut transcript)
            .expect("create Pasta IPA Poseidon proof");
            transcript.finalize()
        }

        fn folded_generator(
            params: &ParamsIPA<EqAffine>,
            vk: &halo2_base::halo2_proofs::plonk::VerifyingKey<EqAffine>,
            proof: &[u8],
            instances: &[&[&[Scalar]]],
        ) -> EqAffine {
            let mut transcript = Transcript::<NativeLoader, _>::new::<SECURE_MDS>(proof);
            verify_proof::<
                IPACommitmentScheme<EqAffine>,
                VerifierIPA<'_, EqAffine>,
                ChallengeScalar<EqAffine>,
                _,
                _,
            >(
                params,
                vk,
                FoldedGeneratorStrategy::new(params),
                instances,
                &mut transcript,
            )
            .expect("complete native verification computes folded generator")
        }

        fn fixture() -> Fixture {
            let params = params_new(INNER_K);
            let value = Scalar::from(7);
            let circuit = PublicValue { value };
            let vk = keygen_vk(&params, &circuit).expect("tiny Pasta verifier key");
            let pk = keygen_pk(&params, vk.clone(), &circuit).expect("tiny Pasta proving key");
            let column = [value];
            let columns: [&[Scalar]; 1] = [&column];
            let proof_instances: [&[&[Scalar]]; 1] = [&columns];
            let proof_without_folded_generator =
                create_poseidon_proof(&params, &pk, circuit, &proof_instances);
            let generator = folded_generator(
                &params,
                &vk,
                &proof_without_folded_generator,
                &proof_instances,
            );
            let mut augmented_proof = proof_without_folded_generator.clone();
            augmented_proof.extend_from_slice(generator.to_bytes().as_ref());
            let svk = canonical_svk(&params);
            let deciding_key = IpaDecidingKey::new(svk, params.get_g().to_vec());
            let protocol = compile(&params, &vk, Config::ipa().with_num_instance(vec![1]));
            Fixture {
                params,
                pk,
                protocol,
                deciding_key,
                proof_without_folded_generator,
                augmented_proof,
                instances: vec![vec![value]],
            }
        }

        fn succinct_accumulator(fixture: &Fixture) -> IpaAccumulator<EqAffine, NativeLoader> {
            let mut transcript = Transcript::<NativeLoader, _>::new::<SECURE_MDS>(
                fixture.augmented_proof.as_slice(),
            );
            let parsed = SuccinctVerifier::read_proof(
                fixture.deciding_key.as_ref(),
                &fixture.protocol,
                &fixture.instances,
                &mut transcript,
            )
            .expect("parse augmented Axiom IPA proof as BGH19");
            let mut accumulators = SuccinctVerifier::verify(
                fixture.deciding_key.as_ref(),
                &fixture.protocol,
                &fixture.instances,
                &parsed,
            )
            .expect("verify the full PLONK residual and produce an IPA accumulator");
            assert_eq!(accumulators.len(), 1, "one proof yields one accumulator");
            accumulators.pop().expect("one accumulator")
        }

        /// The first real cycle-native verifier slice: an Eq/Vesta proof is
        /// parsed, Fiat--Shamir challenged, PLONK-checked, and reduced to its
        /// IPA accumulator inside an Fq circuit.  Its public output is the
        /// exact inner instance followed by the canonical accumulator limbs;
        /// terminal verification decides that same accumulator natively.
        mod cycle_native_verifier {
            use std::{mem, rc::Rc};

            use halo2_base::{
                gates::circuit::{BaseCircuitParams, builder::BaseCircuitBuilder},
                halo2_proofs::{
                    arithmetic::Field as _,
                    dev::MockProver,
                    halo2curves::{CurveAffine as _, pasta::Fq},
                },
                utils::{decompose_biguint, fe_to_biguint},
            };
            use halo2_ecc::fields::fp::FpChip;
            use snark_verifier::{
                loader::halo2::Halo2Loader,
                pcs::AccumulationDecider,
                verifier::{SnarkVerifier, plonk::PlonkSuccinctVerifier},
            };

            use super::super::pasta_cycle_loader::{LIMB_BITS, LIMBS, PastaCycleEccChip};
            use super::*;

            const OUTER_K: u32 = 18;
            const OUTER_LOOKUP_BITS: usize = OUTER_K as usize - 1;

            type CycleChip<'chip> = PastaCycleEccChip<'chip, EqAffine>;
            type CycleLoader<'chip> = Halo2Loader<EqAffine, CycleChip<'chip>>;

            fn seed_outer_params() -> BaseCircuitParams {
                BaseCircuitParams {
                    k: OUTER_K as usize,
                    num_advice_per_phase: vec![5],
                    num_lookup_advice_per_phase: vec![2],
                    num_fixed: 1,
                    lookup_bits: Some(OUTER_LOOKUP_BITS),
                    num_instance_columns: 1,
                }
            }

            fn build_outer_verifier(
                params: BaseCircuitParams,
                fixture: &Fixture,
                protocol: &snark_verifier::verifier::plonk::PlonkProtocol<EqAffine>,
                instances: &[Vec<Fp>],
                proof: &[u8],
            ) -> BaseCircuitBuilder<Fq> {
                let mut builder = BaseCircuitBuilder::new(false).use_params(params);
                let range = builder.range_chip();
                let base = FpChip::<Fq, Fq>::new(&range, LIMB_BITS, LIMBS);
                let scalar = FpChip::<Fq, Fp>::new(&range, LIMB_BITS, LIMBS);
                let chip = CycleChip::new(&base, &scalar);
                let loader = CycleLoader::new(chip, mem::take(builder.pool(0)));

                // The protocol is loaded as circuit constants. A different
                // transition VK therefore changes the outer VK; it is not an
                // unbound witness selected by the caller.
                let loaded_protocol = protocol.loaded(&loader);
                let loaded_instances = instances
                    .iter()
                    .map(|column| {
                        column
                            .iter()
                            .map(|value| loader.assign_scalar(*value))
                            .collect::<Vec<_>>()
                    })
                    .collect::<Vec<_>>();
                let mut transcript =
                    Transcript::<Rc<CycleLoader<'_>>, _>::new::<SECURE_MDS>(&loader, proof);
                let parsed = PlonkSuccinctVerifier::<As>::read_proof(
                    fixture.deciding_key.as_ref(),
                    &loaded_protocol,
                    &loaded_instances,
                    &mut transcript,
                )
                .expect("parse Eq proof in the Fq circuit");
                let mut accumulators = PlonkSuccinctVerifier::<As>::verify(
                    fixture.deciding_key.as_ref(),
                    &loaded_protocol,
                    &loaded_instances,
                    &parsed,
                )
                .expect("constrain the complete Eq PLONK residual");
                assert_eq!(accumulators.len(), 1);
                let accumulator = accumulators.pop().expect("one Eq accumulator");

                let mut public = loaded_instances
                    .iter()
                    .flat_map(|column| column.iter())
                    .flat_map(|value| value.clone().into_assigned().limbs().to_vec())
                    .collect::<Vec<_>>();
                for challenge in accumulator.xi {
                    public.extend(challenge.into_assigned().limbs().iter().copied());
                }
                let assigned_u = accumulator.u.into_assigned();
                let canonical_u = loader
                    .ecc_chip()
                    .canonical_point(&mut loader.ctx_mut(), assigned_u);
                public.extend(canonical_u.x.limbs().iter().copied());
                public.extend(canonical_u.y.limbs().iter().copied());

                *builder.pool(0) = loader.take_ctx();
                builder.assigned_instances[0] = public;
                builder
            }

            fn assigned_instances(circuit: &BaseCircuitBuilder<Fq>) -> Vec<Vec<Fq>> {
                circuit
                    .assigned_instances
                    .iter()
                    .map(|column| column.iter().map(|value| *value.value()).collect())
                    .collect()
            }

            fn native_public_accumulator_encoding(
                fixture: &Fixture,
                accumulator: &IpaAccumulator<EqAffine, NativeLoader>,
            ) -> Vec<Fq> {
                let mut encoded = fixture
                    .instances
                    .iter()
                    .flat_map(|column| column.iter())
                    .flat_map(|value| {
                        decompose_biguint::<Fq>(&fe_to_biguint(value), LIMBS, LIMB_BITS)
                    })
                    .collect::<Vec<_>>();
                for challenge in &accumulator.xi {
                    encoded.extend(decompose_biguint::<Fq>(
                        &fe_to_biguint(challenge),
                        LIMBS,
                        LIMB_BITS,
                    ));
                }
                let coordinates = accumulator
                    .u
                    .coordinates()
                    .expect("IPA accumulator cannot be identity");
                for coordinate in [coordinates.x(), coordinates.y()] {
                    encoded.extend(decompose_biguint::<Fq>(
                        &fe_to_biguint(coordinate),
                        LIMBS,
                        LIMB_BITS,
                    ));
                }
                encoded
            }

            fn assert_mock_rejects(
                label: &str,
                circuit: &BaseCircuitBuilder<Fq>,
                instances: Vec<Vec<Fq>>,
            ) {
                let prover = MockProver::run(OUTER_K, circuit, instances)
                    .unwrap_or_else(|error| panic!("{label} MockProver setup failed: {error}"));
                assert!(prover.verify().is_err(), "{label} substitution must reject");
            }

            #[test]
            #[ignore = "explicit cycle-native verifier resource measurement"]
            fn eq_proof_is_authenticated_in_fq_and_terminally_decided() {
                let fixture = fixture();
                let native_accumulator = succinct_accumulator(&fixture);
                let mut valid = build_outer_verifier(
                    seed_outer_params(),
                    &fixture,
                    &fixture.protocol,
                    &fixture.instances,
                    &fixture.augmented_proof,
                );
                let calculated = valid.calculate_params(Some(9));
                valid.set_params(calculated.clone());
                let canonical_instances = assigned_instances(&valid);
                assert_eq!(canonical_instances.len(), 1);
                assert_eq!(
                    canonical_instances[0],
                    native_public_accumulator_encoding(&fixture, &native_accumulator),
                    "the outer circuit must expose the exact terminally decided accumulator"
                );
                MockProver::run(OUTER_K, &valid, canonical_instances.clone())
                    .expect("canonical cycle-native MockProver")
                    .assert_satisfied();
                <As as AccumulationDecider<EqAffine, NativeLoader>>::decide(
                    &fixture.deciding_key,
                    native_accumulator,
                )
                .expect("terminal IPA decision");

                let mut accumulator_substitution = canonical_instances.clone();
                let last = accumulator_substitution[0]
                    .last_mut()
                    .expect("non-empty public accumulator");
                *last += Fq::ONE;
                assert_mock_rejects("accumulator", &valid, accumulator_substitution);

                let substituted_instances = vec![vec![Fp::from(8)]];
                let instance_substitution = build_outer_verifier(
                    calculated.clone(),
                    &fixture,
                    &fixture.protocol,
                    &substituted_instances,
                    &fixture.augmented_proof,
                );
                assert_mock_rejects(
                    "instance",
                    &instance_substitution,
                    assigned_instances(&instance_substitution),
                );

                let substituted_circuit = PublicValue { value: Fp::from(8) };
                let substituted_column = [Fp::from(8)];
                let substituted_columns: [&[Fp]; 1] = [&substituted_column];
                let substituted_proof_instances: [&[&[Fp]]; 1] = [&substituted_columns];
                let substituted_proof_without_generator = create_poseidon_proof(
                    &fixture.params,
                    &fixture.pk,
                    substituted_circuit,
                    &substituted_proof_instances,
                );
                let generator = folded_generator(
                    &fixture.params,
                    fixture.pk.get_vk(),
                    &substituted_proof_without_generator,
                    &substituted_proof_instances,
                );
                let mut substituted_proof = substituted_proof_without_generator;
                substituted_proof.extend_from_slice(generator.to_bytes().as_ref());
                let proof_substitution = build_outer_verifier(
                    calculated.clone(),
                    &fixture,
                    &fixture.protocol,
                    &fixture.instances,
                    &substituted_proof,
                );
                assert_mock_rejects(
                    "proof",
                    &proof_substitution,
                    assigned_instances(&proof_substitution),
                );

                let mut substituted_protocol = fixture.protocol.clone();
                substituted_protocol.preprocessed[0] = fixture.params.get_g()[1];
                let vk_substitution = build_outer_verifier(
                    calculated,
                    &fixture,
                    &substituted_protocol,
                    &fixture.instances,
                    &fixture.augmented_proof,
                );
                assert_mock_rejects(
                    "verifier key",
                    &vk_substitution,
                    assigned_instances(&vk_substitution),
                );
            }
        }

        #[test]
        fn axiom_poseidon_wire_appends_exactly_one_folded_generator() {
            let fixture = fixture();
            assert_eq!(
                fixture.augmented_proof.len(),
                fixture.proof_without_folded_generator.len()
                    + std::mem::size_of::<<EqAffine as GroupEncoding>::Repr>(),
                "the recursion wire is the ordinary Axiom proof plus one compressed point"
            );

            let accumulator = succinct_accumulator(&fixture);
            <As as AccumulationDecider<EqAffine, NativeLoader>>::decide(
                &fixture.deciding_key,
                accumulator.clone(),
            )
            .expect("terminal decision recomputes the folded canonical generator basis");

            let mut transcript = Transcript::<NativeLoader, _>::new::<SECURE_MDS>(
                fixture.augmented_proof.as_slice(),
            );
            let parsed = FullVerifier::read_proof(
                &fixture.deciding_key,
                &fixture.protocol,
                &fixture.instances,
                &mut transcript,
            )
            .expect("full verifier parses augmented proof");
            FullVerifier::verify(
                &fixture.deciding_key,
                &fixture.protocol,
                &fixture.instances,
                &parsed,
            )
            .expect("full verifier includes terminal IPA decision");

            let substituted =
                IpaAccumulator::new(accumulator.xi.clone(), fixture.params.get_g()[1]);
            assert!(
                <As as AccumulationDecider<EqAffine, NativeLoader>>::decide(
                    &fixture.deciding_key,
                    substituted,
                )
                .is_err(),
                "carrying a substituted accumulator point is not a terminal decision"
            );
        }

        #[test]
        fn folded_generator_is_constrained_by_the_plonk_opening_residual() {
            let fixture = fixture();
            let mut substituted = fixture.augmented_proof.clone();
            let replacement = fixture.params.get_g()[1].to_bytes();
            let offset = substituted.len() - replacement.as_ref().len();
            substituted[offset..].copy_from_slice(replacement.as_ref());

            let rejected = catch_unwind(AssertUnwindSafe(|| {
                let mut transcript =
                    Transcript::<NativeLoader, _>::new::<SECURE_MDS>(substituted.as_slice());
                let parsed = SuccinctVerifier::read_proof(
                    fixture.deciding_key.as_ref(),
                    &fixture.protocol,
                    &fixture.instances,
                    &mut transcript,
                )
                .expect("a substituted canonical point remains parseable");
                SuccinctVerifier::verify(
                    fixture.deciding_key.as_ref(),
                    &fixture.protocol,
                    &fixture.instances,
                    &parsed,
                )
            }));
            assert!(
                rejected.is_err() || rejected.expect("no panic").is_err(),
                "a substituted folded generator must fail the constrained residual"
            );
        }
    }
}
