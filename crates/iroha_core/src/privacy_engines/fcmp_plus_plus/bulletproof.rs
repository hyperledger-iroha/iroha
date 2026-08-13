//! FCMP++ use of the shared generalized-Bulletproof backend.
//!
//! The implementation lives in `iroha_zkp_halo2`; this module retains the
//! FCMP-focused compatibility and adversarial tests against its frozen
//! Blake2b transcript adapter.

pub(super) use iroha_zkp_halo2::generalized_bulletproof::{
    ArithmeticCircuitStatement, ArithmeticCircuitWitness, LinComb, Variable,
    VectorCommitmentOpening,
};

#[cfg(test)]
use super::proof_math::FcmpProofRandomSource;

#[cfg(test)]
mod tests {
    use rand_08::{SeedableRng as _, rngs::StdRng};
    use rand_core_06::{CryptoRng, RngCore};

    use super::*;
    use crate::privacy_engines::fcmp_plus_plus::{
        FailingRngV1, FcmpNativeErrorV1,
        field::{Field25519, SelenePoint},
        proof_math::{
            ProofGeneratorView, ProofScalar, ProverTranscript, SecretMultiexpBuilder, SeleneSuite,
            VerifierTranscript, selene_bp_generators,
        },
    };
    use iroha_zkp_halo2::generalized_bulletproof::{
        GeneralizedBulletproofErrorV1, MAX_PROVER_SCALAR_ATTEMPTS_V1, ProofGenerators,
        random_scalar,
    };

    #[derive(Default)]
    struct NonCanonicalRng {
        calls: usize,
    }

    impl RngCore for NonCanonicalRng {
        fn next_u32(&mut self) -> u32 {
            u32::MAX
        }

        fn next_u64(&mut self) -> u64 {
            u64::MAX
        }

        fn fill_bytes(&mut self, destination: &mut [u8]) {
            destination.fill(0xff);
        }

        fn try_fill_bytes(&mut self, destination: &mut [u8]) -> Result<(), rand_core_06::Error> {
            self.calls += 1;
            destination.fill(0xff);
            Ok(())
        }
    }

    impl CryptoRng for NonCanonicalRng {}

    fn circuit_constraints() -> Vec<LinComb<Field25519>> {
        vec![
            LinComb::empty()
                .term(Field25519::ONE, Variable::aO(0))
                .constant(-Field25519::from_u64(12)),
            LinComb::empty()
                .term(Field25519::ONE, Variable::aO(1))
                .constant(-Field25519::from_u64(30)),
            LinComb::empty()
                .term(Field25519::ONE, Variable::aL(0))
                .term(
                    Field25519::ONE,
                    Variable::CG {
                        commitment: 0,
                        index: 0,
                    },
                )
                .constant(-Field25519::from_u64(10)),
            LinComb::empty()
                .term(Field25519::ONE, Variable::aR(1))
                .term(
                    Field25519::ONE,
                    Variable::CG {
                        commitment: 1,
                        index: 3,
                    },
                )
                .constant(-Field25519::from_u64(20)),
        ]
    }

    fn commitment(
        generators: ProofGeneratorView<'_, SeleneSuite>,
        opening: &VectorCommitmentOpening<Field25519>,
    ) -> SelenePoint {
        let mut terms = SecretMultiexpBuilder::<SeleneSuite>::new(opening.values.len() + 1)
            .expect("fixed test commitment capacity");
        for (scalar, point) in opening.values.0.iter().zip(generators.g_bold) {
            terms
                .push(scalar, point)
                .expect("test commitment stays within its fixed capacity");
        }
        terms
            .push(&opening.mask, &generators.h)
            .expect("test commitment mask fits its fixed capacity");
        terms.evaluate().expect("complete test commitment")
    }

    fn duplicate_test_openings(
        openings: &[VectorCommitmentOpening<Field25519>],
    ) -> Vec<VectorCommitmentOpening<Field25519>> {
        openings
            .iter()
            .map(|opening| VectorCommitmentOpening::new(opening.values.0.clone(), opening.mask))
            .collect()
    }

    fn verify_test_circuit(context: [u8; 32], proof: &[u8]) -> Result<(), FcmpNativeErrorV1> {
        let generators = selene_bp_generators().reduce(4)?;
        let mut transcript = VerifierTranscript::new(context, proof);
        let (vector_commitments, scalar_commitments) =
            transcript.read_commitments::<SeleneSuite>(2, 0)?;
        ArithmeticCircuitStatement::new(
            generators,
            circuit_constraints(),
            vector_commitments,
            scalar_commitments,
        )?
        .verify(&mut transcript)?;
        if transcript.consumed() != proof.len() {
            return Err(FcmpNativeErrorV1::TranscriptConsumption);
        }
        Ok(())
    }

    #[test]
    fn native_arithmetic_circuit_prover_round_trips_and_tampering_fails_closed() {
        let context = [0x42_u8; 32];
        let generators = selene_bp_generators().reduce(4).expect("generators");
        let openings = vec![
            VectorCommitmentOpening::new(
                vec![
                    Field25519::from_u64(7),
                    Field25519::from_u64(8),
                    Field25519::from_u64(9),
                    Field25519::from_u64(10),
                ],
                Field25519::from_u64(13),
            ),
            VectorCommitmentOpening::new(
                vec![
                    Field25519::from_u64(11),
                    Field25519::from_u64(12),
                    Field25519::from_u64(13),
                    Field25519::from_u64(14),
                ],
                Field25519::from_u64(17),
            ),
        ];
        let commitments = openings
            .iter()
            .map(|opening| commitment(generators, opening))
            .collect::<Vec<_>>();
        let witness = ArithmeticCircuitWitness::<SeleneSuite>::new(
            vec![Field25519::from_u64(3), Field25519::from_u64(5)],
            vec![Field25519::from_u64(4), Field25519::from_u64(6)],
            duplicate_test_openings(&openings),
        )
        .expect("witness");
        let mut transcript = ProverTranscript::new(context);
        transcript.write_commitments::<SeleneSuite>(commitments.clone(), Vec::new());
        let statement = ArithmeticCircuitStatement::new(
            generators,
            circuit_constraints(),
            commitments.clone(),
            Vec::new(),
        )
        .expect("statement");
        let mut rng = StdRng::seed_from_u64(0xfca5_0001);
        statement
            .prove(
                &mut FcmpProofRandomSource::new(&mut rng),
                &mut transcript,
                witness,
            )
            .expect("proof");
        let proof = transcript.complete();
        assert_eq!(proof.len() % 32, 0);
        verify_test_circuit(context, &proof).expect("native proof verifies");

        // Every serialized point/scalar phase is bound either by the
        // transcript or a checked proof equation.
        for element in 0..(proof.len() / 32) {
            let mut mutated = proof.clone();
            mutated[element * 32] ^= 1;
            assert!(
                verify_test_circuit(context, &mutated).is_err(),
                "mutated proof element {element} was accepted"
            );
        }
        assert!(verify_test_circuit([0x43; 32], &proof).is_err());
        let mut extended = proof.clone();
        extended.extend_from_slice(&[0_u8; 32]);
        assert!(verify_test_circuit(context, &extended).is_err());

        // Bad multiplication values and bad Pedersen openings are rejected
        // before an arithmetic proof can be emitted.
        let invalid_gate_witness = ArithmeticCircuitWitness::<SeleneSuite>::new(
            vec![Field25519::from_u64(4), Field25519::from_u64(5)],
            vec![Field25519::from_u64(4), Field25519::from_u64(6)],
            duplicate_test_openings(&openings),
        )
        .expect("shape-valid witness");
        let mut bad_gate_transcript = ProverTranscript::new(context);
        bad_gate_transcript.write_commitments::<SeleneSuite>(commitments.clone(), Vec::new());
        assert!(
            ArithmeticCircuitStatement::new(
                generators,
                circuit_constraints(),
                commitments.clone(),
                Vec::new(),
            )
            .expect("statement")
            .prove(
                &mut FcmpProofRandomSource::new(&mut rng),
                &mut bad_gate_transcript,
                invalid_gate_witness,
            )
            .is_err()
        );

        let mut bad_openings = openings;
        bad_openings[0].values[0] += Field25519::ONE;
        let invalid_opening_witness = ArithmeticCircuitWitness::<SeleneSuite>::new(
            vec![Field25519::from_u64(3), Field25519::from_u64(5)],
            vec![Field25519::from_u64(4), Field25519::from_u64(6)],
            bad_openings,
        )
        .expect("shape-valid witness");
        let mut bad_opening_transcript = ProverTranscript::new(context);
        bad_opening_transcript.write_commitments::<SeleneSuite>(commitments.clone(), Vec::new());
        assert!(
            ArithmeticCircuitStatement::new(
                generators,
                circuit_constraints(),
                commitments,
                Vec::new(),
            )
            .expect("statement")
            .prove(
                &mut FcmpProofRandomSource::new(&mut rng),
                &mut bad_opening_transcript,
                invalid_opening_witness,
            )
            .is_err()
        );
    }

    #[test]
    fn statement_rejects_forged_indices_and_malformed_generator_views() {
        let basis = selene_bp_generators();
        let valid = basis.reduce(4).expect("valid view");
        let commitments = vec![basis.g];
        let scalar_commitments = vec![basis.h];

        let rejects = |constraint| {
            assert_eq!(
                ArithmeticCircuitStatement::new(
                    valid,
                    vec![constraint],
                    commitments.clone(),
                    scalar_commitments.clone(),
                )
                .unwrap_err(),
                GeneralizedBulletproofErrorV1::ArithmeticInvariant
            );
        };

        let mut forged_l = LinComb::empty().term(Field25519::ONE, Variable::aL(4));
        forged_l.highest_a_index = Some(0);
        rejects(forged_l);
        let mut forged_r = LinComb::empty().term(Field25519::ONE, Variable::aR(4));
        forged_r.highest_a_index = Some(0);
        rejects(forged_r);
        let mut forged_o = LinComb::empty().term(Field25519::ONE, Variable::aO(4));
        forged_o.highest_a_index = Some(0);
        rejects(forged_o);
        let mut forged_cg = LinComb::empty().term(
            Field25519::ONE,
            Variable::CG {
                commitment: 1,
                index: 0,
            },
        );
        forged_cg.highest_c_index = Some(0);
        rejects(forged_cg);
        let mut forged_v = LinComb::empty().term(Field25519::ONE, Variable::V(1));
        forged_v.highest_v_index = Some(0);
        rejects(forged_v);

        let empty: [SelenePoint; 0] = [];
        let one_g = [basis.g_bold[0]];
        let three_g = [basis.g_bold[0], basis.g_bold[1], basis.g_bold[2]];
        let three_h = [basis.h_bold[0], basis.h_bold[1], basis.h_bold[2]];
        let identity_g = [SelenePoint::identity(), basis.g_bold[1]];
        let two_h = [basis.h_bold[0], basis.h_bold[1]];
        let foreign_g = [basis.g_bold[0] + basis.h, basis.g_bold[1]];

        let rejects_view = |view: ProofGeneratorView<'_, SeleneSuite>| {
            assert_eq!(
                ArithmeticCircuitStatement::new(view, Vec::new(), Vec::new(), Vec::new())
                    .unwrap_err(),
                GeneralizedBulletproofErrorV1::ArithmeticInvariant
            );
        };
        rejects_view(ProofGeneratorView {
            g: basis.g,
            h: basis.h,
            g_bold: &empty,
            h_bold: &empty,
        });
        rejects_view(ProofGeneratorView {
            g: basis.g,
            h: basis.h,
            g_bold: &one_g,
            h_bold: &empty,
        });
        rejects_view(ProofGeneratorView {
            g: basis.g,
            h: basis.h,
            g_bold: &three_g,
            h_bold: &three_h,
        });
        rejects_view(ProofGeneratorView {
            g: SelenePoint::identity(),
            h: basis.h,
            g_bold: &basis.g_bold[..2],
            h_bold: &basis.h_bold[..2],
        });
        rejects_view(ProofGeneratorView {
            g: basis.g,
            h: basis.h,
            g_bold: &identity_g,
            h_bold: &two_h,
        });
        rejects_view(ProofGeneratorView {
            g: basis.g,
            h: basis.h,
            g_bold: &foreign_g,
            h_bold: &two_h,
        });
    }

    #[test]
    fn generator_constructor_rejects_identity_and_degenerate_prefixes() {
        let basis = selene_bp_generators();
        let identity = SelenePoint::identity();
        let valid_g = vec![basis.g_bold[0], basis.g_bold[1]];
        let valid_h = vec![basis.h_bold[0], basis.h_bold[1]];

        assert!(
            ProofGenerators::<SeleneSuite>::new(
                identity,
                basis.h,
                valid_g.clone(),
                valid_h.clone()
            )
            .is_err()
        );
        assert!(
            ProofGenerators::<SeleneSuite>::new(
                basis.g,
                basis.h,
                vec![identity, basis.g_bold[1]],
                valid_h,
            )
            .is_err()
        );
        assert!(
            ProofGenerators::<SeleneSuite>::new(
                basis.g,
                basis.h,
                valid_g,
                vec![basis.h_bold[0], -basis.h_bold[0]],
            )
            .is_err()
        );
    }

    #[test]
    fn generalized_bulletproof_randomness_rejects_noncanonical_rng_at_fixed_bound() {
        let mut rng = NonCanonicalRng::default();
        assert_eq!(
            random_scalar::<Field25519, _>(&mut FcmpProofRandomSource::new(&mut rng)),
            Err(GeneralizedBulletproofErrorV1::ProverRandomnessExhausted)
        );
        assert_eq!(rng.calls, MAX_PROVER_SCALAR_ATTEMPTS_V1);
        assert_eq!(MAX_PROVER_SCALAR_ATTEMPTS_V1, 128);
        assert_eq!(
            random_scalar::<Field25519, _>(&mut FcmpProofRandomSource::new(&mut FailingRngV1)),
            Err(GeneralizedBulletproofErrorV1::RandomnessUnavailable)
        );
    }
}
