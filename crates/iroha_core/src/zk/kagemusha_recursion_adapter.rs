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

            let deferred_packet_upper_bound = scalar_count * 32
                + (explicit_challenge_count + EXTRA_CHALLENGE_UPPER_BOUND) * 16
                + (point_count + protocol.preprocessed.len() + EXTRA_COEFFICIENT_UPPER_BOUND) * 32;
            eprintln!(
                "Kagemusha deferred packet proof={} scalars={} points={} explicit_challenges={} preprocessed={} packet_upper={}",
                proof_bytes.len(),
                scalar_count,
                point_count,
                explicit_challenge_count,
                protocol.preprocessed.len(),
                deferred_packet_upper_bound,
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
