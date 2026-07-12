//! Fail-closed boundary for Kagemusha Pasta-cycle recursion.
//!
//! The reviewed Axiom `PoseidonTranscript` hashes in `C::Scalar` and explicitly
//! assumes that field is native to the verifier circuit.  A generic
//! `Halo2Loader` adapter across the Pasta cycle therefore emulates every
//! transcript scalar.  The measured Ep-to-Fp prototype required 39,275,522
//! advice cells and 7,436,318 lookup cells (about 4.1 GiB live RSS); bounded
//! CRT batching and native curve coordinates still required 18,040,862 advice
//! cells, 2,669,809 lookup cells, 100.35 seconds to construct, and
//! 2,414,559,232 bytes peak RSS.  Proof parsing consumed 8,287,023 advice cells
//! and fold-transcript parsing another 5,835,004.  That construction is
//! structurally outside the wallet's 128 MiB preparation gate and is not kept
//! as a production fallback.
//!
//! The tests below retain only the smallest sound compatibility boundary
//! already supported by the pinned dependencies: fixed-key Poseidon proof
//! wires for both Pasta parities, canonical BGH19 IPA folding, exact bounded
//! proof bytes, and native terminal decisions.  Production availability stays
//! false until a fixed-VK cross-field transcript/verifier constrains those same
//! operations without generic scalar emulation and passes the device gates.

#[cfg(test)]
mod tests {
    use halo2_proofs::{
        arithmetic::Field,
        circuit::{Layouter, SimpleFloorPlanner, Value},
        plonk::{Advice, Circuit, Column, ConstraintSystem, Error as PlonkError, Instance},
    };

    use crate::zk::halo2_backend::assign_advice_compat;

    /// Native-value loader which preserves every MSM as a canonical linear
    /// equation instead of evaluating it away.  This is audit instrumentation
    /// for the fixed-VK deferred-verifier wire: scalar arithmetic remains the
    /// exact field arithmetic used by `snark-verifier`, while every curve
    /// assertion records the complete base/coefficient vector that the
    /// opposite-field circuit would have to authenticate.
    mod deferred_audit {
        use std::{
            cell::RefCell,
            fmt,
            io::Read,
            marker::PhantomData,
            ops::{Add, AddAssign, Mul, MulAssign, Neg, Sub, SubAssign},
            rc::Rc,
        };

        use snark_verifier::{
            Error,
            loader::{EcPointLoader, LoadedEcPoint, LoadedScalar, Loader, ScalarLoader},
            util::{
                arithmetic::{
                    Curve, CurveAffine, Field, FieldExt, FieldOps, Group, PrimeField, fe_to_fe,
                },
                hash::Poseidon,
                transcript::{Transcript, TranscriptRead},
            },
        };

        #[derive(Clone, Debug, PartialEq, Eq)]
        pub(super) struct EquationTerm {
            pub(super) point: Vec<u8>,
            pub(super) coefficient: Vec<u8>,
        }

        #[derive(Clone, Debug, PartialEq, Eq)]
        pub(super) struct Equation {
            pub(super) annotation: String,
            pub(super) terms: Vec<EquationTerm>,
        }

        struct State {
            equations: Vec<Equation>,
        }

        #[derive(Clone)]
        pub(super) struct RecordingLoader<C: CurveAffine> {
            state: Rc<RefCell<State>>,
            _curve: PhantomData<C>,
        }

        impl<C: CurveAffine> fmt::Debug for RecordingLoader<C> {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.debug_struct("RecordingLoader").finish_non_exhaustive()
            }
        }

        impl<C: CurveAffine> RecordingLoader<C> {
            pub(super) fn new() -> Self {
                Self {
                    state: Rc::new(RefCell::new(State {
                        equations: Vec::new(),
                    })),
                    _curve: PhantomData,
                }
            }

            pub(super) fn equations(&self) -> Vec<Equation> {
                self.state.borrow().equations.clone()
            }

            fn same(&self, other: &Self) {
                assert!(
                    Rc::ptr_eq(&self.state, &other.state),
                    "deferred audit values cannot cross loader instances"
                );
            }
        }

        #[derive(Clone)]
        pub(super) struct RecordedScalar<C: CurveAffine> {
            value: C::Scalar,
            loader: RecordingLoader<C>,
        }

        impl<C: CurveAffine> fmt::Debug for RecordedScalar<C> {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.debug_tuple("RecordedScalar").field(&self.value).finish()
            }
        }

        impl<C: CurveAffine> PartialEq for RecordedScalar<C> {
            fn eq(&self, other: &Self) -> bool {
                self.loader.same(&other.loader);
                self.value == other.value
            }
        }

        impl<C: CurveAffine> RecordedScalar<C> {
            pub(super) fn canonical_bytes(&self) -> Vec<u8> {
                self.value.to_repr().as_ref().to_vec()
            }
        }

        macro_rules! scalar_binop {
            ($trait:ident, $method:ident, $assign_trait:ident, $assign_method:ident, $op:tt) => {
                impl<C: CurveAffine> $trait for RecordedScalar<C> {
                    type Output = Self;

                    fn $method(mut self, rhs: Self) -> Self::Output {
                        self.loader.same(&rhs.loader);
                        self.value = self.value $op rhs.value;
                        self
                    }
                }

                impl<C: CurveAffine> $trait<&Self> for RecordedScalar<C> {
                    type Output = Self;

                    fn $method(mut self, rhs: &Self) -> Self::Output {
                        self.loader.same(&rhs.loader);
                        self.value = self.value $op rhs.value;
                        self
                    }
                }

                impl<C: CurveAffine> $assign_trait for RecordedScalar<C> {
                    fn $assign_method(&mut self, rhs: Self) {
                        self.loader.same(&rhs.loader);
                        self.value = self.value $op rhs.value;
                    }
                }

                impl<C: CurveAffine> $assign_trait<&Self> for RecordedScalar<C> {
                    fn $assign_method(&mut self, rhs: &Self) {
                        self.loader.same(&rhs.loader);
                        self.value = self.value $op rhs.value;
                    }
                }
            };
        }

        scalar_binop!(Add, add, AddAssign, add_assign, +);
        scalar_binop!(Sub, sub, SubAssign, sub_assign, -);
        scalar_binop!(Mul, mul, MulAssign, mul_assign, *);

        impl<C: CurveAffine> Neg for RecordedScalar<C> {
            type Output = Self;

            fn neg(mut self) -> Self::Output {
                self.value = -self.value;
                self
            }
        }

        impl<C: CurveAffine> FieldOps for RecordedScalar<C> {
            fn invert(&self) -> Option<Self> {
                Option::<C::Scalar>::from(Field::invert(&self.value)).map(|value| Self {
                    value,
                    loader: self.loader.clone(),
                })
            }
        }

        impl<C: CurveAffine> LoadedScalar<C::Scalar> for RecordedScalar<C> {
            type Loader = RecordingLoader<C>;

            fn loader(&self) -> &Self::Loader {
                &self.loader
            }

            fn pow_var(&self, exp: &Self, _: usize) -> Self {
                self.loader.same(&exp.loader);
                let repr = exp.value.to_repr();
                let mut limbs = Vec::with_capacity(repr.as_ref().len().div_ceil(8));
                for chunk in repr.as_ref().chunks(8) {
                    let mut limb = [0_u8; 8];
                    limb[..chunk.len()].copy_from_slice(chunk);
                    limbs.push(u64::from_le_bytes(limb));
                }
                Self {
                    value: self.value.pow_vartime(limbs),
                    loader: self.loader.clone(),
                }
            }
        }

        #[derive(Clone)]
        struct LinearTerm<C: CurveAffine> {
            point: C,
            coefficient: C::Scalar,
        }

        #[derive(Clone)]
        pub(super) struct RecordedPoint<C: CurveAffine> {
            value: C,
            terms: Vec<LinearTerm<C>>,
            loader: RecordingLoader<C>,
        }

        impl<C: CurveAffine> fmt::Debug for RecordedPoint<C> {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.debug_struct("RecordedPoint")
                    .field("value", &self.value)
                    .field("terms", &self.terms.len())
                    .finish()
            }
        }

        impl<C: CurveAffine> PartialEq for RecordedPoint<C> {
            fn eq(&self, other: &Self) -> bool {
                self.loader.same(&other.loader);
                self.value == other.value
            }
        }

        impl<C: CurveAffine> RecordedPoint<C> {
            pub(super) fn canonical_bytes(&self) -> Vec<u8> {
                self.value.to_bytes().as_ref().to_vec()
            }
        }

        impl<C: CurveAffine> LoadedEcPoint<C> for RecordedPoint<C> {
            type Loader = RecordingLoader<C>;

            fn loader(&self) -> &Self::Loader {
                &self.loader
            }
        }

        fn push_term<C: CurveAffine>(
            terms: &mut Vec<LinearTerm<C>>,
            point: C,
            coefficient: C::Scalar,
        ) {
            if coefficient == C::Scalar::ZERO {
                return;
            }
            if let Some(existing) = terms.iter_mut().find(|term| term.point == point) {
                existing.coefficient += coefficient;
                if existing.coefficient == C::Scalar::ZERO {
                    let index = terms
                        .iter()
                        .position(|term| term.point == point)
                        .expect("existing term index");
                    terms.remove(index);
                }
            } else {
                terms.push(LinearTerm { point, coefficient });
            }
        }

        impl<C: CurveAffine> ScalarLoader<C::Scalar> for RecordingLoader<C> {
            type LoadedScalar = RecordedScalar<C>;

            fn load_const(&self, value: &C::Scalar) -> Self::LoadedScalar {
                RecordedScalar {
                    value: *value,
                    loader: self.clone(),
                }
            }

            fn assert_eq(
                &self,
                annotation: &str,
                lhs: &Self::LoadedScalar,
                rhs: &Self::LoadedScalar,
            ) {
                lhs.loader.same(self);
                rhs.loader.same(self);
                assert_eq!(lhs.value, rhs.value, "{annotation}");
            }
        }

        impl<C: CurveAffine> EcPointLoader<C> for RecordingLoader<C> {
            type LoadedEcPoint = RecordedPoint<C>;

            fn ec_point_load_const(&self, value: &C) -> Self::LoadedEcPoint {
                RecordedPoint {
                    value: *value,
                    terms: vec![LinearTerm {
                        point: *value,
                        coefficient: C::Scalar::ONE,
                    }],
                    loader: self.clone(),
                }
            }

            fn ec_point_assert_eq(
                &self,
                annotation: &str,
                lhs: &Self::LoadedEcPoint,
                rhs: &Self::LoadedEcPoint,
            ) {
                lhs.loader.same(self);
                rhs.loader.same(self);
                assert_eq!(lhs.value, rhs.value, "{annotation}");
                let mut terms = Vec::new();
                for term in &lhs.terms {
                    push_term(&mut terms, term.point, term.coefficient);
                }
                for term in &rhs.terms {
                    push_term(&mut terms, term.point, -term.coefficient);
                }
                let terms = terms
                    .into_iter()
                    .map(|term| EquationTerm {
                        point: term.point.to_bytes().as_ref().to_vec(),
                        coefficient: term.coefficient.to_repr().as_ref().to_vec(),
                    })
                    .collect();
                self.state.borrow_mut().equations.push(Equation {
                    annotation: annotation.to_owned(),
                    terms,
                });
            }

            fn multi_scalar_multiplication(
                pairs: &[(
                    &<Self as ScalarLoader<C::Scalar>>::LoadedScalar,
                    &Self::LoadedEcPoint,
                )],
            ) -> Self::LoadedEcPoint {
                let (first_scalar, first_point) = pairs.first().expect("non-empty MSM");
                let loader = first_scalar.loader.clone();
                first_point.loader.same(&loader);
                let mut value = C::Curve::identity();
                let mut terms = Vec::new();
                for (scalar, point) in pairs {
                    scalar.loader.same(&loader);
                    point.loader.same(&loader);
                    value += point.value * scalar.value;
                    for term in &point.terms {
                        push_term(&mut terms, term.point, term.coefficient * scalar.value);
                    }
                }
                RecordedPoint {
                    value: value.to_affine(),
                    terms,
                    loader,
                }
            }
        }

        impl<C: CurveAffine> Loader<C> for RecordingLoader<C> {}

        pub(super) struct RecordingPoseidonTranscript<
            C: CurveAffine,
            R,
            const T: usize,
            const RATE: usize,
            const R_F: usize,
            const R_P: usize,
        > {
            loader: RecordingLoader<C>,
            stream: R,
            poseidon: Poseidon<C::Scalar, RecordedScalar<C>, T, RATE>,
            pub(super) scalar_count: usize,
            pub(super) point_count: usize,
            pub(super) point_sources: Vec<Vec<u8>>,
        }

        impl<
            C: CurveAffine,
            R,
            const T: usize,
            const RATE: usize,
            const R_F: usize,
            const R_P: usize,
        > RecordingPoseidonTranscript<C, R, T, RATE, R_F, R_P>
        where
            C::Scalar: FieldExt,
        {
            pub(super) fn new<const SECURE_MDS: usize>(
                loader: RecordingLoader<C>,
                stream: R,
            ) -> Self {
                let poseidon = Poseidon::new::<R_F, R_P, SECURE_MDS>(&loader);
                Self {
                    loader,
                    stream,
                    poseidon,
                    scalar_count: 0,
                    point_count: 0,
                    point_sources: Vec::new(),
                }
            }
        }

        impl<
            C: CurveAffine,
            R,
            const T: usize,
            const RATE: usize,
            const R_F: usize,
            const R_P: usize,
        > Transcript<C, RecordingLoader<C>> for RecordingPoseidonTranscript<C, R, T, RATE, R_F, R_P>
        where
            C::Scalar: FieldExt,
        {
            fn loader(&self) -> &RecordingLoader<C> {
                &self.loader
            }

            fn squeeze_challenge(&mut self) -> RecordedScalar<C> {
                self.poseidon.squeeze()
            }

            fn common_ec_point(&mut self, point: &RecordedPoint<C>) -> Result<(), Error> {
                point.loader.same(&self.loader);
                let coordinates: Option<snark_verifier::util::arithmetic::Coordinates<C>> =
                    point.value.coordinates().into();
                let coordinates = coordinates.ok_or_else(|| {
                    Error::Transcript(
                        std::io::ErrorKind::InvalidData,
                        "identity point cannot enter the Poseidon transcript".to_owned(),
                    )
                })?;
                let x = self.loader.load_const(&fe_to_fe(*coordinates.x()));
                let y = self.loader.load_const(&fe_to_fe(*coordinates.y()));
                self.poseidon.update(&[x, y]);
                Ok(())
            }

            fn common_scalar(&mut self, scalar: &RecordedScalar<C>) -> Result<(), Error> {
                scalar.loader.same(&self.loader);
                self.poseidon.update(std::slice::from_ref(scalar));
                Ok(())
            }
        }

        impl<
            C: CurveAffine,
            R: Read,
            const T: usize,
            const RATE: usize,
            const R_F: usize,
            const R_P: usize,
        > TranscriptRead<C, RecordingLoader<C>>
            for RecordingPoseidonTranscript<C, R, T, RATE, R_F, R_P>
        where
            C::Scalar: FieldExt,
        {
            fn read_scalar(&mut self) -> Result<RecordedScalar<C>, Error> {
                let mut repr = <C::Scalar as PrimeField>::Repr::default();
                self.stream.read_exact(repr.as_mut()).map_err(|error| {
                    Error::Transcript(error.kind(), "truncated scalar field".to_owned())
                })?;
                let value = C::Scalar::from_repr_vartime(repr).ok_or_else(|| {
                    Error::Transcript(
                        std::io::ErrorKind::InvalidData,
                        "non-canonical scalar field".to_owned(),
                    )
                })?;
                let value = self.loader.load_const(&value);
                self.common_scalar(&value)?;
                self.scalar_count += 1;
                Ok(value)
            }

            fn read_ec_point(&mut self) -> Result<RecordedPoint<C>, Error> {
                let mut repr = C::Repr::default();
                self.stream.read_exact(repr.as_mut()).map_err(|error| {
                    Error::Transcript(error.kind(), "truncated curve point".to_owned())
                })?;
                let value = Option::<C>::from(C::from_bytes(&repr)).ok_or_else(|| {
                    Error::Transcript(
                        std::io::ErrorKind::InvalidData,
                        "non-canonical curve point".to_owned(),
                    )
                })?;
                self.point_sources.push(repr.as_ref().to_vec());
                let value = self.loader.ec_point_load_const(&value);
                self.common_ec_point(&value)?;
                self.point_count += 1;
                Ok(value)
            }
        }
    }

    #[derive(Clone, Default)]
    struct PublicValue<F: Field> {
        value: F,
    }

    impl<F: Field> Circuit<F> for PublicValue<F> {
        type Config = (Column<Advice>, Column<Instance>);
        type FloorPlanner = SimpleFloorPlanner;
        type Params = ();

        fn without_witnesses(&self) -> Self {
            Self::default()
        }

        fn configure(meta: &mut ConstraintSystem<F>) -> Self::Config {
            let advice = meta.advice_column();
            let instance = meta.instance_column();
            meta.enable_equality(advice);
            meta.enable_equality(instance);
            (advice, instance)
        }

        fn synthesize(
            &self,
            (advice, instance): Self::Config,
            mut layouter: impl Layouter<F>,
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

    /// Fixed-key compatibility and soundness checks for the Eq proof/fold wire.
    mod pasta_ipa_poseidon_wire {
        use std::panic::{AssertUnwindSafe, catch_unwind};

        use halo2_base::halo2_proofs::{
            halo2curves::{
                CurveExt as _,
                group::{Curve as _, GroupEncoding},
                pasta::{Eq, EqAffine, Fp},
            },
            plonk::{Circuit, ProvingKey, create_proof, verify_proof},
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
            loader::ScalarLoader,
            loader::native::NativeLoader,
            pcs::{
                AccumulationDecider, AccumulationScheme, AccumulationSchemeProver,
                ipa::{
                    Bgh19, IpaAccumulator, IpaAs, IpaDecidingKey, IpaProvingKey,
                    IpaSuccinctVerifyingKey,
                },
            },
            system::halo2::{
                Config, compile,
                strategy::ipa::SingleStrategy as FoldedGeneratorStrategy,
                transcript::halo2::{ChallengeScalar, PoseidonTranscript, TranscriptObject},
            },
            util::arithmetic::{Domain, root_of_unity},
            verifier::{
                SnarkVerifier,
                plonk::{PlonkSuccinctVerifier, PlonkVerifier},
            },
        };

        use super::PublicValue;
        use super::deferred_audit::{RecordingLoader, RecordingPoseidonTranscript};
        use crate::zk::halo2_backend::{Scalar, keygen_pk, keygen_vk, params_new};
        use snark_verifier::util::arithmetic::PrimeCurveAffine as _;

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

        fn canonical_folding_key(params: &ParamsIPA<EqAffine>) -> IpaProvingKey<EqAffine> {
            let svk = canonical_svk(params);
            IpaProvingKey::new(svk.domain.clone(), params.get_g().to_vec(), svk.h, svk.s)
        }

        fn create_poseidon_proof<CircuitT>(
            params: &ParamsIPA<EqAffine>,
            pk: &ProvingKey<EqAffine>,
            circuit: CircuitT,
            instances: &[&[&[Scalar]]],
        ) -> Vec<u8>
        where
            CircuitT: Circuit<Scalar>,
        {
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

        fn create_fold_proof(
            params: &ParamsIPA<EqAffine>,
            accumulators: &[IpaAccumulator<EqAffine, NativeLoader>],
        ) -> (Vec<u8>, IpaAccumulator<EqAffine, NativeLoader>) {
            let key = canonical_folding_key(params);
            let mut transcript = Transcript::<NativeLoader, _>::new::<SECURE_MDS>(Vec::<u8>::new());
            let folded = <As as AccumulationSchemeProver<EqAffine>>::create_proof(
                &key,
                accumulators,
                &mut transcript,
                OsRng,
            )
            .expect("create canonical Pasta IPA fold proof");
            (transcript.finalize(), folded)
        }

        #[test]
        fn transition_deferred_packet_has_a_bounded_wire_budget() {
            use crate::zk::kagemusha_v2::{
                KAGEMUSHA_RECURSIVE_SPEND_V2_INSTANCE_ROWS,
                KagemushaRecursiveSpendTransitionCircuitV2,
                kagemusha_recursive_spend_transition_instance_column_v2,
            };

            const PRODUCTION_K: u32 = 12;
            // Until the fixed verifier exposes its exact coefficient vector,
            // reserve a deliberately conservative number of 128-bit
            // challenges and full-width MSM coefficients above those visible
            // in the proof transcript.
            const EXTRA_CHALLENGE_UPPER_BOUND: usize = 64;
            const EXTRA_COEFFICIENT_UPPER_BOUND: usize = 64;

            let params = params_new(PRODUCTION_K);
            let circuit = KagemushaRecursiveSpendTransitionCircuitV2::default();
            let instance_column =
                kagemusha_recursive_spend_transition_instance_column_v2(&circuit.values);
            assert_eq!(
                instance_column.len(),
                KAGEMUSHA_RECURSIVE_SPEND_V2_INSTANCE_ROWS
            );
            let vk = keygen_vk(&params, &circuit).expect("transition deferred-packet VK");
            let pk =
                keygen_pk(&params, vk.clone(), &circuit).expect("transition deferred-packet PK");
            let columns: [&[Scalar]; 1] = [&instance_column];
            let proof_instances: [&[&[Scalar]]; 1] = [&columns];
            let proof_without_generator =
                create_poseidon_proof(&params, &pk, circuit, &proof_instances);
            let generator =
                folded_generator(&params, &vk, &proof_without_generator, &proof_instances);
            let mut proof_bytes = proof_without_generator;
            proof_bytes.extend_from_slice(generator.to_bytes().as_ref());

            let svk = canonical_svk(&params);
            let deciding_key = IpaDecidingKey::new(svk, params.get_g().to_vec());
            let protocol = compile(
                &params,
                &vk,
                Config::ipa().with_num_instance(vec![instance_column.len()]),
            );
            let instances = vec![instance_column];
            let mut transcript =
                Transcript::<NativeLoader, _>::new::<SECURE_MDS>(proof_bytes.as_slice());
            let parsed = SuccinctVerifier::read_proof(
                deciding_key.as_ref(),
                &protocol,
                &instances,
                &mut transcript,
            )
            .expect("parse fixed transition proof");
            let scalar_count = transcript
                .loaded_stream
                .iter()
                .filter(|object| matches!(object, TranscriptObject::Scalar(_)))
                .count();
            let point_count = transcript
                .loaded_stream
                .iter()
                .filter(|object| matches!(object, TranscriptObject::EcPoint(_)))
                .count();
            let explicit_challenge_count = parsed.challenges.len() + 1;
            let mut accumulators =
                SuccinctVerifier::verify(deciding_key.as_ref(), &protocol, &instances, &parsed)
                    .expect("verify fixed transition proof");
            assert_eq!(accumulators.len(), 1);
            <As as AccumulationDecider<EqAffine, NativeLoader>>::decide(
                &deciding_key,
                accumulators.pop().expect("one transition accumulator"),
            )
            .expect("terminal transition decision");

            // Re-run the exact fixed-key verifier with native scalar
            // arithmetic and symbolic curve arithmetic. This extracts the
            // complete MSM coefficient vectors rather than guessing from the
            // number of transcript objects.
            let recording_loader = RecordingLoader::<EqAffine>::new();
            let loaded_protocol = protocol.loaded(&recording_loader);
            let loaded_instances = instances
                .iter()
                .map(|column| {
                    column
                        .iter()
                        .map(|value| recording_loader.load_const(value))
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>();
            let mut recording_transcript =
                RecordingPoseidonTranscript::<EqAffine, _, T, RATE, R_F, R_P>::new::<SECURE_MDS>(
                    recording_loader.clone(),
                    proof_bytes.as_slice(),
                );
            let recorded = SuccinctVerifier::read_proof(
                deciding_key.as_ref(),
                &loaded_protocol,
                &loaded_instances,
                &mut recording_transcript,
            )
            .expect("parse fixed transition proof for deferred audit");
            let recorded_accumulators = SuccinctVerifier::verify(
                deciding_key.as_ref(),
                &loaded_protocol,
                &loaded_instances,
                &recorded,
            )
            .expect("extract fixed transition residual equations");
            assert_eq!(recorded_accumulators.len(), 1);
            let recorded_accumulator = &recorded_accumulators[0];
            assert_eq!(recorded_accumulator.xi.len(), PRODUCTION_K as usize);
            let equations = recording_loader.equations();
            assert_eq!(
                equations.len(),
                1,
                "the fixed IPA verifier must expose exactly one opening-residual MSM"
            );

            // Canonical point-source namespace: transcript points first in
            // transcript order, followed by fixed protocol/SVK points. The
            // packet carries only a u16 source index plus a canonical scalar;
            // proof and artifact bytes supply the points themselves.
            let mut point_sources = recording_transcript.point_sources.clone();
            let svk = deciding_key.as_ref();
            let mut add_fixed_source = |point: EqAffine| {
                let bytes = point.to_bytes().as_ref().to_vec();
                if !point_sources.iter().any(|existing| existing == &bytes) {
                    point_sources.push(bytes);
                }
            };
            for point in &protocol.preprocessed {
                add_fixed_source(*point);
            }
            add_fixed_source(svk.g);
            add_fixed_source(svk.h);
            if let Some(point) = svk.s {
                add_fixed_source(point);
            }
            add_fixed_source(EqAffine::generator());
            if let Some(instance_key) = &protocol.instance_committing_key {
                for point in &instance_key.bases {
                    add_fixed_source(*point);
                }
                if let Some(point) = instance_key.constant {
                    add_fixed_source(point);
                }
            }
            assert!(
                point_sources.len() <= usize::from(u16::MAX),
                "deferred packet point namespace must fit u16"
            );

            let mut coefficient_count = 0_usize;
            for equation in &equations {
                assert!(!equation.terms.is_empty());
                for term in &equation.terms {
                    assert_eq!(term.point.len(), 32);
                    assert_eq!(term.coefficient.len(), 32);
                    assert!(
                        point_sources.iter().any(|source| source == &term.point),
                        "every residual base must resolve to proof or fixed-VK material"
                    );
                }
                coefficient_count += equation.terms.len();
            }
            let accumulator_u = recorded_accumulator.u.canonical_bytes();
            assert!(
                point_sources.iter().any(|source| source == &accumulator_u),
                "the output accumulator point must be a proof point"
            );
            for xi in &recorded_accumulator.xi {
                assert_eq!(xi.canonical_bytes().len(), 32);
            }

            // Complete optimized packet layout:
            // magic/version/parity/counts + schema/VK/instance/manifest hashes
            // + proof length/source count/accumulator count + packet digest,
            // followed by proof bytes, length-prefixed equations of
            // (u16 source, scalar), and the accumulator xi/U representation.
            const PACKET_FIXED_BYTES: usize = 8 + 2 + 1 + 1 + 4 * 32 + 2 + 2 + 1 + 1 + 32;
            const EQUATION_HEADER_BYTES: usize = 2;
            const EQUATION_TERM_BYTES: usize = 2 + 32;
            let complete_deferred_packet_bytes = PACKET_FIXED_BYTES
                + proof_bytes.len()
                + equations.len() * EQUATION_HEADER_BYTES
                + coefficient_count * EQUATION_TERM_BYTES
                + recorded_accumulator.xi.len() * 32
                + 2;

            let deferred_packet_upper_bound = scalar_count * 32
                + (explicit_challenge_count + EXTRA_CHALLENGE_UPPER_BOUND) * 16
                + (point_count + protocol.preprocessed.len() + EXTRA_COEFFICIENT_UPPER_BOUND) * 32;
            eprintln!(
                "Kagemusha deferred packet proof={} scalars={} points={} explicit_challenges={} preprocessed={} residual_equations={} residual_coefficients={} point_sources={} packet_exact={} packet_upper={}",
                proof_bytes.len(),
                scalar_count,
                point_count,
                explicit_challenge_count,
                protocol.preprocessed.len(),
                equations.len(),
                coefficient_count,
                point_sources.len(),
                complete_deferred_packet_bytes,
                deferred_packet_upper_bound,
            );
            assert!(
                complete_deferred_packet_bytes <= deferred_packet_upper_bound,
                "the legacy conservative bound must cover the canonical packet"
            );
            assert!(
                deferred_packet_upper_bound <= 9_216,
                "a public deferred packet cannot exceed the complete raw peer envelope"
            );
        }

        #[test]
        fn canonical_ipa_fold_is_constant_size_decidable_and_substitution_safe() {
            let fixture = fixture();
            let accumulator = succinct_accumulator(&fixture);
            let inputs = [accumulator.clone(), accumulator];
            let (proof_bytes, expected) = create_fold_proof(&fixture.params, &inputs);
            let expected_wire_bytes = (8 + 2 * INNER_K as usize) * 32;
            assert_eq!(
                proof_bytes.len(),
                expected_wire_bytes,
                "the canonical Poseidon IPA fold wire must not gain metadata or a host receipt"
            );
            assert!(
                proof_bytes.len() <= 4_096,
                "canonical IPA fold proof must fit the recursive proof budget"
            );

            let svk = canonical_svk(&fixture.params);
            let mut transcript =
                Transcript::<NativeLoader, _>::new::<SECURE_MDS>(proof_bytes.as_slice());
            let proof = <As as AccumulationScheme<EqAffine, NativeLoader>>::read_proof(
                &svk,
                &inputs,
                &mut transcript,
            )
            .expect("parse canonical IPA fold proof");
            let folded =
                <As as AccumulationScheme<EqAffine, NativeLoader>>::verify(&svk, &inputs, &proof)
                    .expect("verify canonical IPA fold proof");
            assert_eq!(folded.xi, expected.xi);
            assert_eq!(folded.u, expected.u);
            <As as AccumulationDecider<EqAffine, NativeLoader>>::decide(
                &fixture.deciding_key,
                folded,
            )
            .expect("terminally decide folded IPA accumulator");

            let mut substituted_inputs = inputs;
            substituted_inputs[0].u = fixture.params.get_g()[1];
            let rejected = catch_unwind(AssertUnwindSafe(|| {
                let mut transcript =
                    Transcript::<NativeLoader, _>::new::<SECURE_MDS>(proof_bytes.as_slice());
                let proof = <As as AccumulationScheme<EqAffine, NativeLoader>>::read_proof(
                    &svk,
                    &substituted_inputs,
                    &mut transcript,
                )
                .expect("a canonical substituted point remains parseable");
                <As as AccumulationScheme<EqAffine, NativeLoader>>::verify(
                    &svk,
                    &substituted_inputs,
                    &proof,
                )
            }));
            assert!(
                rejected.is_err() || rejected.expect("no panic").is_err(),
                "an input-accumulator substitution must invalidate the fold"
            );
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

    /// Reciprocal Pasta parity.  The production cycle is sound only if an
    /// Ep/Pallas proof over Fq is authenticated inside an Fp circuit with the
    /// same transcript, VK, public-instance, and fold bindings as Eq/Vesta.
    mod pasta_ipa_poseidon_wire_ep {
        use std::panic::{AssertUnwindSafe, catch_unwind};

        use halo2_base::halo2_proofs::{
            halo2curves::{
                CurveExt as _,
                group::{Curve as _, GroupEncoding},
                pasta::{Ep, EpAffine, Fq},
            },
            plonk::{ProvingKey, create_proof, keygen_pk, keygen_vk, verify_proof},
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
                AccumulationDecider, AccumulationScheme, AccumulationSchemeProver,
                ipa::{
                    Bgh19, IpaAccumulator, IpaAs, IpaDecidingKey, IpaProvingKey,
                    IpaSuccinctVerifyingKey,
                },
            },
            system::halo2::{
                Config, compile,
                strategy::ipa::SingleStrategy as FoldedGeneratorStrategy,
                transcript::halo2::{ChallengeScalar, PoseidonTranscript},
            },
            util::arithmetic::{Domain, root_of_unity},
            verifier::{SnarkVerifier, plonk::PlonkSuccinctVerifier},
        };

        use super::PublicValue;

        const T: usize = 3;
        const RATE: usize = 2;
        const R_F: usize = 8;
        const R_P: usize = 57;
        const SECURE_MDS: usize = 0;
        const INNER_K: u32 = 5;

        type As = IpaAs<EpAffine, Bgh19>;
        type SuccinctVerifier = PlonkSuccinctVerifier<As>;
        type Transcript<L, S> = PoseidonTranscript<EpAffine, L, S, T, RATE, R_F, R_P>;

        struct Fixture {
            params: ParamsIPA<EpAffine>,
            protocol: snark_verifier::verifier::plonk::PlonkProtocol<EpAffine>,
            deciding_key: IpaDecidingKey<EpAffine>,
            proof_without_folded_generator: Vec<u8>,
            augmented_proof: Vec<u8>,
            instances: Vec<Vec<Fq>>,
        }

        fn canonical_svk(params: &ParamsIPA<EpAffine>) -> IpaSuccinctVerifyingKey<EpAffine> {
            let hash_to_curve = Ep::hash_to_curve("Halo2-Parameters");
            let w = hash_to_curve(&[1]).to_affine();
            let u = hash_to_curve(&[2]).to_affine();
            IpaSuccinctVerifyingKey::new(
                Domain::new(params.k() as usize, root_of_unity(params.k() as usize)),
                params.get_g()[0],
                u,
                Some(w),
            )
        }

        fn canonical_folding_key(params: &ParamsIPA<EpAffine>) -> IpaProvingKey<EpAffine> {
            let svk = canonical_svk(params);
            IpaProvingKey::new(svk.domain.clone(), params.get_g().to_vec(), svk.h, svk.s)
        }

        fn create_poseidon_proof(
            params: &ParamsIPA<EpAffine>,
            pk: &ProvingKey<EpAffine>,
            circuit: PublicValue<Fq>,
            instances: &[&[&[Fq]]],
        ) -> Vec<u8> {
            let mut transcript = Transcript::<NativeLoader, _>::new::<SECURE_MDS>(Vec::<u8>::new());
            create_proof::<
                IPACommitmentScheme<EpAffine>,
                ProverIPA<'_, EpAffine>,
                ChallengeScalar<EpAffine>,
                _,
                _,
                _,
            >(params, pk, &[circuit], instances, OsRng, &mut transcript)
            .expect("create reciprocal Pasta IPA Poseidon proof");
            transcript.finalize()
        }

        fn folded_generator(
            params: &ParamsIPA<EpAffine>,
            vk: &halo2_base::halo2_proofs::plonk::VerifyingKey<EpAffine>,
            proof: &[u8],
            instances: &[&[&[Fq]]],
        ) -> EpAffine {
            let mut transcript = Transcript::<NativeLoader, _>::new::<SECURE_MDS>(proof);
            verify_proof::<
                IPACommitmentScheme<EpAffine>,
                VerifierIPA<'_, EpAffine>,
                ChallengeScalar<EpAffine>,
                _,
                _,
            >(
                params,
                vk,
                FoldedGeneratorStrategy::new(params),
                instances,
                &mut transcript,
            )
            .expect("complete reciprocal native verification computes folded generator")
        }

        fn fixture() -> Fixture {
            let params = ParamsIPA::<EpAffine>::new(INNER_K);
            let value = Fq::from(11);
            let circuit = PublicValue { value };
            let vk = keygen_vk(&params, &circuit).expect("tiny reciprocal Pasta verifier key");
            let pk = keygen_pk(&params, vk.clone(), &circuit)
                .expect("tiny reciprocal Pasta proving key");
            let column = [value];
            let columns: [&[Fq]; 1] = [&column];
            let proof_instances: [&[&[Fq]]; 1] = [&columns];
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
                protocol,
                deciding_key,
                proof_without_folded_generator,
                augmented_proof,
                instances: vec![vec![value]],
            }
        }

        fn succinct_accumulator(fixture: &Fixture) -> IpaAccumulator<EpAffine, NativeLoader> {
            let mut transcript = Transcript::<NativeLoader, _>::new::<SECURE_MDS>(
                fixture.augmented_proof.as_slice(),
            );
            let parsed = SuccinctVerifier::read_proof(
                fixture.deciding_key.as_ref(),
                &fixture.protocol,
                &fixture.instances,
                &mut transcript,
            )
            .expect("parse reciprocal augmented IPA proof");
            let mut accumulators = SuccinctVerifier::verify(
                fixture.deciding_key.as_ref(),
                &fixture.protocol,
                &fixture.instances,
                &parsed,
            )
            .expect("verify reciprocal PLONK residual");
            assert_eq!(accumulators.len(), 1);
            accumulators.pop().expect("one reciprocal accumulator")
        }

        fn create_fold_proof(
            params: &ParamsIPA<EpAffine>,
            accumulators: &[IpaAccumulator<EpAffine, NativeLoader>],
        ) -> (Vec<u8>, IpaAccumulator<EpAffine, NativeLoader>) {
            let key = canonical_folding_key(params);
            let mut transcript = Transcript::<NativeLoader, _>::new::<SECURE_MDS>(Vec::<u8>::new());
            let folded = <As as AccumulationSchemeProver<EpAffine>>::create_proof(
                &key,
                accumulators,
                &mut transcript,
                OsRng,
            )
            .expect("create reciprocal Pasta IPA fold proof");
            (transcript.finalize(), folded)
        }

        #[test]
        fn reciprocal_poseidon_wire_fold_and_tamper_contract() {
            let fixture = fixture();
            assert_eq!(
                fixture.augmented_proof.len(),
                fixture.proof_without_folded_generator.len()
                    + std::mem::size_of::<<EpAffine as GroupEncoding>::Repr>()
            );
            let accumulator = succinct_accumulator(&fixture);
            let inputs = [accumulator.clone(), accumulator];
            let (fold_bytes, expected) = create_fold_proof(&fixture.params, &inputs);
            assert_eq!(fold_bytes.len(), (8 + 2 * INNER_K as usize) * 32);

            let svk = canonical_svk(&fixture.params);
            let mut transcript =
                Transcript::<NativeLoader, _>::new::<SECURE_MDS>(fold_bytes.as_slice());
            let proof = <As as AccumulationScheme<EpAffine, NativeLoader>>::read_proof(
                &svk,
                &inputs,
                &mut transcript,
            )
            .expect("parse reciprocal fold proof");
            let folded =
                <As as AccumulationScheme<EpAffine, NativeLoader>>::verify(&svk, &inputs, &proof)
                    .expect("verify reciprocal fold proof");
            assert_eq!(folded.xi, expected.xi);
            assert_eq!(folded.u, expected.u);
            <As as AccumulationDecider<EpAffine, NativeLoader>>::decide(
                &fixture.deciding_key,
                folded,
            )
            .expect("terminally decide reciprocal folded accumulator");

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
                .expect("a reciprocal substituted canonical point remains parseable");
                SuccinctVerifier::verify(
                    fixture.deciding_key.as_ref(),
                    &fixture.protocol,
                    &fixture.instances,
                    &parsed,
                )
            }));
            assert!(
                rejected.is_err() || rejected.expect("no panic").is_err(),
                "a reciprocal folded-generator substitution must reject"
            );
        }
    }
}
