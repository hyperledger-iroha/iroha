/// Genuine seeded IPA comparisons against the retained vector RLC circuit.
mod proof_tests {
    use super::*;
    use halo2_proofs::{
        SerdeFormat,
        plonk::{create_proof, keygen_pk2, keygen_vk_custom, verify_proof},
        poly::{
            VerificationStrategy as _,
            ipa::{
                commitment::IPACommitmentScheme,
                multiopen::{ProverIPA, VerifierIPA},
                strategy::SingleStrategy,
            },
        },
        transcript::{
            Blake2bRead, Blake2bWrite, Challenge255, TranscriptReadBuffer, TranscriptWriterBuffer,
        },
    };

    /// Preserve the original Base graph and change only the RLC assignment path.
    struct StreamingCircuit<F: KagemushaPoseidonFieldV1> {
        inner: ClaimRlcTestCircuit<F>,
    }

    impl<F: KagemushaPoseidonFieldV1> Circuit<F> for StreamingCircuit<F> {
        type Config = ClaimRlcTestConfig<F>;
        type FloorPlanner = V1;
        type Params = BaseCircuitParams;

        fn params(&self) -> Self::Params {
            self.inner.params()
        }

        fn without_witnesses(&self) -> Self {
            Self {
                inner: self.inner.without_witnesses(),
            }
        }

        fn configure_with_params(
            meta: &mut ConstraintSystem<F>,
            params: Self::Params,
        ) -> Self::Config {
            <ClaimRlcTestCircuit<F> as Circuit<F>>::configure_with_params(meta, params)
        }

        fn configure(_: &mut ConstraintSystem<F>) -> Self::Config {
            unreachable!("streaming RLC proof fixture uses Base parameters")
        }

        fn synthesize(
            &self,
            config: Self::Config,
            mut layouter: impl Layouter<F>,
        ) -> Result<(), PlonkError> {
            <BaseCircuitBuilder<F> as Circuit<F>>::synthesize(
                &self.inner.builder,
                config.base,
                layouter.namespace(|| "claim RLC test Base"),
            )?;
            layouter.assign_table(
                || "claim RLC compact test range",
                |mut table| {
                    // Keep the exact table from ClaimRlcTestCircuit. The production 15-bit
                    // table does not fit this k9 fixture and is tested separately.
                    for (row, value) in [
                        0_u64, 1, 2, 3, 4, 7, 8, 9, 18, 27, 83, 127, 255, 16_256, 32_512, 32_640,
                        32_766, 32_767,
                    ]
                    .into_iter()
                    .enumerate()
                    {
                        table.assign_cell(
                            || "compact range value",
                            config.carrier_rlc.range_table,
                            row,
                            || Value::known(F::from(value)),
                        )?;
                    }
                    Ok(())
                },
            )?;
            rlc_streaming::synthesize_with_capacity(
                &self.inner.machine,
                &config.carrier_rlc,
                &mut layouter,
                &self.inner.builder.core().copy_manager,
                self.inner.builder.witness_gen_only(),
                CLAIM_RLC_TEST_CAPACITY,
            )
        }
    }

    /// Compare every Base parameter without relying on a serialization format.
    fn assert_same_base_params(expected: &BaseCircuitParams, actual: &BaseCircuitParams) {
        assert_eq!(actual.k, expected.k);
        assert_eq!(actual.num_advice_per_phase, expected.num_advice_per_phase);
        assert_eq!(actual.num_fixed, expected.num_fixed);
        assert_eq!(
            actual.num_lookup_advice_per_phase,
            expected.num_lookup_advice_per_phase
        );
        assert_eq!(actual.lookup_bits, expected.lookup_bits);
        assert_eq!(actual.num_instance_columns, expected.num_instance_columns);
    }

    macro_rules! assert_seeded_parity {
        ($curve:ty, $field:ty) => {{
            for compressed in [false, true] {
                let params = ParamsIPA::<$curve>::new(CLAIM_RLC_TEST_K as u32);
                let original = claim_rlc_test_circuit_v1::<$field>(false, false);
                let streaming = StreamingCircuit {
                    inner: claim_rlc_test_circuit_v1::<$field>(false, false),
                };
                assert!(!original.builder.witness_gen_only());
                assert!(!streaming.inner.builder.witness_gen_only());
                assert_eq!(
                    original
                        .machine
                        .required_rows_with_capacity(CLAIM_RLC_TEST_CAPACITY)
                        .unwrap(),
                    72
                );
                let circuit_params = original.params();
                assert_eq!(circuit_params.k, CLAIM_RLC_TEST_K);
                assert_eq!(circuit_params.num_instance_columns, 0);
                assert_same_base_params(&circuit_params, &streaming.params());

                let original_pk = keygen_pk2(&params, &original, compressed)
                    .expect("retained vector RLC proving key");
                let streaming_pk = keygen_pk2(&params, &streaming, compressed)
                    .expect("streaming RLC proving key");
                let original_pk_bytes = original_pk.to_bytes(SerdeFormat::Processed);
                let original_vk_bytes = original_pk.get_vk().to_bytes(SerdeFormat::Processed);
                assert_eq!(
                    streaming_pk.to_bytes(SerdeFormat::Processed),
                    original_pk_bytes,
                    "streaming RLC changed the complete proving key"
                );
                assert_eq!(
                    streaming_pk.get_vk().to_bytes(SerdeFormat::Processed),
                    original_vk_bytes,
                    "streaming RLC changed the verifying key"
                );
                let break_points = original.builder.break_points();
                assert_eq!(streaming.inner.builder.break_points(), break_points);
                let original_unknown = original.without_witnesses();
                let streaming_unknown = streaming.without_witnesses();
                for unknown_vk in [
                    keygen_vk_custom(&params, &original_unknown, compressed)
                        .expect("retained vector RLC unknown verifying key"),
                    keygen_vk_custom(&params, &streaming_unknown, compressed)
                        .expect("streaming RLC unknown verifying key"),
                ] {
                    assert_eq!(
                        unknown_vk.to_bytes(SerdeFormat::Processed),
                        original_vk_bytes,
                        "unknown witnesses changed the RLC fixed or copy schedule"
                    );
                }

                // These are deliberately fixed, public test bytes, not production entropy.
                let seed = iroha_crypto::kagemusha::KagemushaRecoverySeedV1::from_unsealed([79; 32])
                    .expect("nonzero deterministic test seed");
                for challenges in [[2, 3], [3, 2]] {
                    let mut original_witness =
                        claim_rlc_test_circuit_with_challenges_v1::<$field>(false, false, challenges);
                    let mut streaming_witness = StreamingCircuit {
                        inner: claim_rlc_test_circuit_with_challenges_v1::<$field>(
                            false, false, challenges,
                        ),
                    };
                    original_witness.builder.set_params(circuit_params.clone());
                    original_witness
                        .builder
                        .set_break_points(break_points.clone());
                    streaming_witness
                        .inner
                        .builder
                        .set_params(circuit_params.clone());
                    streaming_witness
                        .inner
                        .builder
                        .set_break_points(break_points.clone());
                    assert!(!original_witness.builder.witness_gen_only());
                    assert!(!streaming_witness.inner.builder.witness_gen_only());

                    let columns: [&[$field]; 0] = [];
                    let mut original_transcript =
                        Blake2bWrite::<_, $curve, Challenge255<$curve>>::init(Vec::new());
                    create_proof::<IPACommitmentScheme<$curve>, ProverIPA<'_, $curve>, _, _, _, _>(
                        &params,
                        &original_pk,
                        &[original_witness],
                        &[&columns],
                        seed.rng(b"claim-rlc-streaming-test", &[0; 32])
                            .expect("retained vector test RNG"),
                        &mut original_transcript,
                    )
                    .expect("genuine retained vector RLC proof");
                    let mut streaming_transcript =
                        Blake2bWrite::<_, $curve, Challenge255<$curve>>::init(Vec::new());
                    create_proof::<IPACommitmentScheme<$curve>, ProverIPA<'_, $curve>, _, _, _, _>(
                        &params,
                        &streaming_pk,
                        &[streaming_witness],
                        &[&columns],
                        seed.rng(b"claim-rlc-streaming-test", &[0; 32])
                            .expect("streaming test RNG"),
                        &mut streaming_transcript,
                    )
                    .expect("genuine streaming RLC proof");
                    let original_proof = original_transcript.finalize();
                    let streaming_proof = streaming_transcript.finalize();
                    assert_eq!(
                        streaming_proof, original_proof,
                        "streaming RLC changed the complete seeded IPA proof"
                    );

                    let verify = |vk, bytes: &[u8]| {
                        let mut transcript =
                            Blake2bRead::<_, $curve, Challenge255<$curve>>::init(bytes);
                        verify_proof::<IPACommitmentScheme<$curve>, VerifierIPA<'_, $curve>, _, _, _>(
                            &params,
                            vk,
                            SingleStrategy::<$curve>::new(&params),
                            &[&columns],
                            &mut transcript,
                        )
                    };
                    verify(original_pk.get_vk(), &streaming_proof)
                        .expect("streaming proof verifies against retained vector key");
                    verify(streaming_pk.get_vk(), &original_proof)
                        .expect("retained vector proof verifies against streaming key");
                    let mut corrupted = streaming_proof;
                    *corrupted.last_mut().expect("nonempty IPA proof") ^= 1;
                    assert!(verify(original_pk.get_vk(), &corrupted).is_err());
                    assert!(verify(streaming_pk.get_vk(), &corrupted).is_err());
                    assert_eq!(
                        original_pk.to_bytes(SerdeFormat::Processed),
                        original_pk_bytes
                    );
                    assert_eq!(
                        streaming_pk.to_bytes(SerdeFormat::Processed),
                        original_pk_bytes
                    );
                }
            }
        }};
    }

    #[test]
    fn streaming_rlc_eq_preserves_keys_seeded_proofs_and_rejection() {
        assert_seeded_parity!(EqAffine, Fp);
    }

    #[test]
    fn streaming_rlc_ep_preserves_keys_seeded_proofs_and_rejection() {
        assert_seeded_parity!(EpAffine, Fq);
    }
}
