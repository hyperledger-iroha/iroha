    #[test]
    fn v4_generation_preflight_rejects_degree_21_before_parameter_allocation() {
        let mut degree_21 = first_release_generation_params_v4();
        degree_21.k = 21;
        degree_21.lookup_bits = 20;
        assert!(degree_21.validate().is_err());
        let error = preflight_kagemusha_generation_v4(&degree_21, &degree_21)
            .expect_err("degree-21 generation must fail before ParamsIPA allocation");
        assert!(error.contains("degree") || error.contains("layout"));
    }

    #[test]
    fn v4_generation_preflight_rejects_maximum_column_profile_before_allocation() {
        let mut maximum = first_release_generation_params_v4();
        maximum.num_advice_per_phase = vec![256, 256, 256];
        maximum.num_lookup_advice_per_phase = vec![256, 256, 256];
        maximum.num_fixed = 256;
        assert!(maximum.validate().is_err());
        assert!(preflight_kagemusha_generation_v4(&maximum, &maximum).is_err());
    }

    #[test]
    fn v4_generated_payload_size_gate_rejects_empty_and_corridor_limit() {
        validate_kagemusha_generated_payload_size_v4(1, "test payload")
            .expect("non-empty bounded payload");
        assert!(validate_kagemusha_generated_payload_size_v4(0, "test payload").is_err());

        let corridor_limit = usize::try_from(KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MAX_FILE_BYTES_V4)
            .expect("artifact corridor fits usize on supported hosts");
        validate_kagemusha_generated_payload_size_v4(corridor_limit - 1, "test payload")
            .expect("largest admitted payload");
        assert!(
            validate_kagemusha_generated_payload_size_v4(corridor_limit, "test payload").is_err()
        );
    }

    fn v4_complete_stage_plan() -> Vec<scalar_lineage_v1::DeferredEquationStageShapeV4> {
        use scalar_lineage_v1::{DeferredEquationGateV4 as Gate, DeferredEquationStageShapeV4};

        [
            Gate::ParentCurrent { slot: 0 },
            Gate::ParentCarriedFold { slot: 0 },
            Gate::ParentLineageSelect { slot: 0 },
            Gate::ParentCurrent { slot: 1 },
            Gate::ParentCarriedFold { slot: 1 },
            Gate::ParentLineageSelect { slot: 1 },
            Gate::BranchFold,
            Gate::BranchSelect,
        ]
        .into_iter()
        .enumerate()
        .map(|(index, gate)| DeferredEquationStageShapeV4 {
            range: index..index + 1,
            gate,
        })
        .collect()
    }

    #[test]
    fn v4_complete_stage_validator_rejects_omission_reorder_and_duplicate() {
        let stages = v4_complete_stage_plan();
        scalar_lineage_v1::validate_stage_shapes_v4(&stages, 8).expect("complete V4 stage plan");

        for omitted in 0..stages.len() {
            let mut candidate = stages.clone();
            candidate.remove(omitted);
            assert!(
                scalar_lineage_v1::validate_stage_shapes_v4(&candidate, 8).is_err(),
                "accepted omission {omitted}"
            );
        }

        for swapped in 0..stages.len() - 1 {
            let mut candidate = stages.clone();
            candidate.swap(swapped, swapped + 1);
            assert!(
                scalar_lineage_v1::validate_stage_shapes_v4(&candidate, 8).is_err(),
                "accepted reorder at {swapped}"
            );
        }

        for duplicated in 0..stages.len() - 1 {
            let mut candidate = stages.clone();
            candidate[duplicated + 1].gate = candidate[duplicated].gate;
            assert!(
                scalar_lineage_v1::validate_stage_shapes_v4(&candidate, 8).is_err(),
                "accepted duplicate at {duplicated}"
            );
        }
    }

    #[test]
    fn v4_every_enabled_stage_is_covered_by_a_present_complete_join() {
        use scalar_lineage_v1::DeferredEquationGateV4 as Gate;

        let stages = v4_complete_stage_plan();
        for parent_count in 0..=2 {
            let slot_present = [parent_count >= 1, parent_count == 2];
            let parent_has_carried = [true, false];
            for stage in &stages {
                let enabled = match stage.gate {
                    Gate::ParentCurrent { slot } | Gate::ParentLineageSelect { slot } => {
                        slot_present[slot]
                    }
                    Gate::ParentCarriedFold { slot } => {
                        slot_present[slot] && parent_has_carried[slot]
                    }
                    Gate::BranchFold => slot_present[1],
                    Gate::BranchSelect => slot_present[0],
                };
                if enabled {
                    assert!(
                        slot_present[0]
                            && scalar_lineage_v1::validate_stage_shapes_v4(&stages, 8).is_ok()
                            && stages.iter().any(|candidate| candidate == stage),
                        "enabled {:?} is not covered for parent count {parent_count}",
                        stage.gate
                    );
                }
            }
        }
    }

    fn assigned_digest_words<F: halo2_base::utils::ScalarField>(
        digest: &[halo2_base::AssignedValue<F>; 8],
    ) -> [u32; 8] {
        std::array::from_fn(|index| {
            u32::try_from(halo2_base::utils::fe_to_biguint(digest[index].value()))
                .expect("assigned digest word is canonical u32")
        })
    }

    #[test]
    fn v6_native_and_scalar_audit_commitments_match_in_both_parities() {
        use halo2_base::{gates::circuit::builder::BaseCircuitBuilder, utils::ScalarField};
        use halo2_ecc::fields::fp::FpChip;
        use halo2_proofs::{
            dev::MockProver,
            halo2curves::pasta::{EpAffine, EqAffine},
        };
        use snark_verifier::loader::halo2::{EccInstructions as _, Halo2Loader};

        use crate::zk::kagemusha_cycle_loader::{DeferredScalarEccChip, LIMB_BITS, LIMBS};

        fn assert_parity<C>()
        where
            C: halo2_base::utils::CurveAffineExt,
            C::Base: halo2_base::utils::BigPrimeField,
            C::ScalarExt: halo2_base::utils::BigPrimeField + ScalarField,
        {
            use scalar_lineage_v1::{
                AssignedDeferredEquationStageV4, DeferredEquationGateV4 as Gate,
            };

            let mut builder = BaseCircuitBuilder::<C::ScalarExt>::new(false)
                .use_k(17)
                .use_lookup_bits(16);
            let range = builder.range_chip();
            let coordinate = FpChip::<C::ScalarExt, C::Base>::new(&range, LIMB_BITS, LIMBS);
            let scalar_integer =
                FpChip::<C::ScalarExt, C::ScalarExt>::new(&range, LIMB_BITS, LIMBS);
            let loader = Halo2Loader::new(
                DeferredScalarEccChip::<C>::new(&coordinate, &scalar_integer),
                mem::take(builder.pool(0)),
            );
            let gates = [
                Gate::ParentCurrent { slot: 0 },
                Gate::ParentCarriedFold { slot: 0 },
                Gate::ParentLineageSelect { slot: 0 },
                Gate::ParentCurrent { slot: 1 },
                Gate::ParentCarriedFold { slot: 1 },
                Gate::ParentLineageSelect { slot: 1 },
                Gate::BranchFold,
                Gate::BranchSelect,
            ];
            let enabled = [true, true, true, false, false, false, false, true];
            let (stages, slot_present) = {
                let chip = loader.ecc_chip();
                let mut ctx = loader.ctx_mut();
                let when_true = chip.assign_point(&mut ctx, C::generator());
                let when_false = chip.assign_point(&mut ctx, -C::generator());
                let one = ctx.main().load_witness(C::ScalarExt::ONE);
                let zero = ctx.main().load_witness(C::ScalarExt::ZERO);
                let stages = gates
                    .into_iter()
                    .zip(enabled)
                    .enumerate()
                    .map(|(index, (gate, enabled))| {
                        let selector = if enabled { one } else { zero };
                        let _ = chip.select_point(&mut ctx, &when_true, &when_false, selector);
                        AssignedDeferredEquationStageV4 {
                            range: index..index + 1,
                            gate,
                            enabled: selector,
                        }
                    })
                    .collect::<Vec<_>>();
                (stages, [one, zero])
            };
            let witness = loader.ecc_chip().witness();
            let shapes = stages.iter().map(|stage| stage.shape()).collect::<Vec<_>>();
            let expected = kagemusha_deferred_audit_public_words_v6(&witness, &shapes, 1, [1, 0])
                .expect("native V6 audit commitment");
            let expected_cells = {
                let mut ctx = loader.ctx_mut();
                expected.map(|words| {
                    kagemusha_u32_words_to_u128_chunks_v5(&words)
                        .map(|chunk| ctx.main().load_witness(C::ScalarExt::from_u128(chunk)))
                })
            };
            let mut sha_jobs = KagemushaSha256JobsV4::default();
            let digest = scalar_lineage_v1::constrain_scalar_audit_identity_v6(
                &loader,
                &mut sha_jobs,
                &range,
                &stages,
                slot_present,
                [&expected_cells[0], &expected_cells[1]],
            )
            .expect("scalar V6 audit commitment");
            assert_eq!(assigned_digest_words(&digest), expected[0]);
            *builder.pool(0) = loader.take_ctx();
            builder.calculate_params(Some(9));
            MockProver::run(builder.config_params.k as u32, &builder, vec![])
                .expect("scalar V6 audit prover")
                .assert_satisfied();
        }

        assert_parity::<EqAffine>();
        assert_parity::<EpAffine>();
    }

    #[test]
    fn v6_host_deferred_audit_commitment_binds_complete_one_parent_branch_select() {
        use halo2_proofs::halo2curves::{group::prime::PrimeCurveAffine as _, pasta::EqAffine};

        use crate::zk::kagemusha_cycle_loader::{
            DeferredEquationWitness, KAGEMUSHA_DEFERRED_AUDIT_POSEIDON_DOMAIN_V6,
            KAGEMUSHA_DEFERRED_AUDIT_SHA256_DOMAIN_V6, KAGEMUSHA_DEFERRED_AUDIT_VERSION_V6,
            kagemusha_poseidon_domain_elements,
        };

        let source = EqAffine::generator();
        let coefficients = [3_u64, 5, 7, 11, 13, 17, 19, 23];
        let witness = DeferredEquationWitness::<EqAffine> {
            sources: vec![source],
            equations: coefficients
                .map(|coefficient| vec![(0, Fp::from(coefficient))])
                .to_vec(),
        };
        let stages = v4_complete_stage_plan();

        let expected_commitment = |selectors: [u8; 8], coefficients: [u64; 8]| {
            let mut elements = kagemusha_poseidon_domain_elements::<Fp>(
                KAGEMUSHA_DEFERRED_AUDIT_POSEIDON_DOMAIN_V6,
                KAGEMUSHA_DEFERRED_AUDIT_VERSION_V6,
            );
            elements.extend([Fp::from(1), Fp::from(8)]);
            elements.extend(
                kagemusha_compressed_point_poseidon_elements(source)
                    .expect("injective generator encoding"),
            );
            for ((gate_tag, coefficient), selector) in [1_u32, 3, 5, 2, 4, 6, 7, 8]
                .into_iter()
                .zip(coefficients)
                .zip(selectors)
            {
                elements.extend([
                    Fp::from(u64::from(gate_tag)),
                    Fp::from(u64::from(selector)),
                    Fp::from(1),
                    Fp::ZERO,
                    Fp::from(coefficient),
                ]);
            }
            let poseidon = kagemusha_native_poseidon_digest(&elements);
            kagemusha_sha256_public_words(
                kagemusha_short_poseidon_sha256(
                    KAGEMUSHA_DEFERRED_AUDIT_SHA256_DOMAIN_V6,
                    KAGEMUSHA_DEFERRED_AUDIT_VERSION_V6,
                    poseidon,
                )
                .expect("one-block audit wrapper"),
            )
        };

        let one_parent = kagemusha_deferred_audit_public_words_v6(&witness, &stages, 1, [1, 0])
            .expect("commit complete one-parent V6 audit");
        assert_eq!(
            one_parent[0],
            expected_commitment([1, 1, 1, 0, 0, 0, 0, 1], coefficients)
        );
        assert_ne!(one_parent[0], [0; 8]);
        assert_eq!(one_parent[1], [0; 8]);

        let mut tampered = witness.clone();
        tampered.equations[7] = vec![(0, Fp::from(29))];
        let tampered_one_parent =
            kagemusha_deferred_audit_public_words_v6(&tampered, &stages, 1, [1, 0])
                .expect("commit BranchSelect-tampered V6 audit");
        assert_ne!(one_parent[0], tampered_one_parent[0]);
        assert_eq!(tampered_one_parent[1], [0; 8]);

        let two_parent = kagemusha_deferred_audit_public_words_v6(&witness, &stages, 2, [1, 1])
            .expect("commit complete two-parent V6 audit");
        assert_eq!(two_parent[0], two_parent[1]);
        assert_eq!(two_parent[0], expected_commitment([1; 8], coefficients));

        let mut extra_source = witness.clone();
        extra_source.sources.push(-source);
        assert_ne!(
            one_parent,
            kagemusha_deferred_audit_public_words_v6(&extra_source, &stages, 1, [1, 0])
                .expect("commit source-count-tampered V6 audit"),
            "even an unused source must change the committed source count and namespace"
        );

        let mut extra_term = witness.clone();
        extra_term.equations[7].push((0, Fp::ZERO));
        assert_ne!(
            one_parent,
            kagemusha_deferred_audit_public_words_v6(&extra_term, &stages, 1, [1, 0])
                .expect("commit term-count-tampered V6 audit"),
            "a zero-valued extra term must still change the committed term count"
        );

        assert_eq!(
            kagemusha_deferred_audit_public_words_v6(&witness, &stages, 0, [0, 0])
                .expect("commit absent V6 slots"),
            [[0; 8]; 2]
        );
    }

    fn v4_reciprocal_audit_builder<C>(
        witness: &crate::zk::kagemusha_cycle_loader::DeferredEquationWitness<C>,
        stages: &[scalar_lineage_v1::DeferredEquationStageShapeV4],
        current_parent_count: u32,
        expected_words: [[u32; 8]; KAGEMUSHA_PASTA_PARENT_SLOTS_V1],
    ) -> halo2_base::gates::circuit::builder::BaseCircuitBuilder<C::Base>
    where
        C: halo2_base::utils::CurveAffineExt,
        C::Base: halo2_base::utils::BigPrimeField
            + halo2_base::utils::ScalarField
            + ff::WithSmallOrderMulGroup<3>,
        C::ScalarExt: halo2_base::utils::BigPrimeField + ff::WithSmallOrderMulGroup<3>,
    {
        use halo2_base::gates::circuit::builder::BaseCircuitBuilder;
        use halo2_ecc::fields::fp::FpChip;

        use crate::zk::kagemusha_cycle_loader::{LIMB_BITS, LIMBS};

        let mut builder = BaseCircuitBuilder::<C::Base>::new(false)
            .use_k(17)
            .use_lookup_bits(16);
        let range = builder.range_chip();
        let base = FpChip::<C::Base, C::Base>::new(&range, LIMB_BITS, LIMBS);
        let scalar = FpChip::<C::Base, C::ScalarExt>::new(&range, LIMB_BITS, LIMBS);
        let mut ctx = mem::take(builder.pool(0));
        let current_parent_count = ctx
            .main()
            .load_witness(C::Base::from(u64::from(current_parent_count)));
        let parent_counts = [
            ctx.main().load_witness(C::Base::ZERO),
            ctx.main().load_witness(C::Base::ZERO),
        ];
        let expected_words = expected_words.map(|words| {
            kagemusha_u32_words_to_u128_chunks_v5(&words)
                .map(|chunk| ctx.main().load_witness(C::Base::from_u128(chunk)))
        });
        let mut sha_jobs = KagemushaSha256JobsV4::default();
        constrain_reciprocal_point_audit_identity_v6::<C>(
            &mut ctx,
            &mut sha_jobs,
            &base,
            &scalar,
            witness,
            stages,
            current_parent_count,
            parent_counts,
            [&expected_words[0], &expected_words[1]],
            KagemushaDeferredMsmV5::GenericTest,
        )
        .expect("complete V4 reciprocal audit shape");
        *builder.pool(0) = ctx;
        builder.calculate_params(Some(9));
        builder
    }

    #[test]
    fn v4_one_parent_branch_select_reciprocal_substitution_fails_for_both_parities() {
        use halo2_proofs::{
            dev::MockProver,
            halo2curves::{
                group::prime::PrimeCurveAffine as _,
                pasta::{EpAffine, EqAffine},
            },
        };

        use crate::zk::kagemusha_cycle_loader::DeferredEquationWitness;

        fn assert_join<C>(source: C)
        where
            C: halo2_base::utils::CurveAffineExt,
            C::Base: halo2_base::utils::BigPrimeField + halo2_base::utils::ScalarField,
            C::ScalarExt: halo2_base::utils::BigPrimeField,
        {
            let stages = v4_complete_stage_plan();
            let original = DeferredEquationWitness::<C> {
                sources: vec![source],
                equations: vec![vec![(0, C::ScalarExt::ZERO)]; 8],
            };
            let expected = kagemusha_deferred_audit_public_words_v6(&original, &stages, 1, [0, 0])
                .expect("commit original one-parent V6 audit");
            assert_ne!(expected[0], [0; 8]);
            assert_eq!(expected[1], [0; 8]);

            let valid = v4_reciprocal_audit_builder(&original, &stages, 1, expected);
            MockProver::run(valid.config_params.k as u32, &valid, vec![])
                .expect("valid complete reciprocal audit prover")
                .assert_satisfied();

            let mut wrong_absent_slot = expected;
            wrong_absent_slot[1] = expected[0];
            let wrong_absent_slot =
                v4_reciprocal_audit_builder(&original, &stages, 1, wrong_absent_slot);
            assert!(
                MockProver::run(
                    wrong_absent_slot.config_params.k as u32,
                    &wrong_absent_slot,
                    vec![],
                )
                .expect("non-canonical one-parent reciprocal audit prover")
                .verify()
                .is_err(),
                "a one-parent step must expose canonical zero in slot one"
            );

            let two_parent =
                kagemusha_deferred_audit_public_words_v6(&original, &stages, 2, [0, 0])
                    .expect("commit original two-parent V6 audit");
            assert_ne!(two_parent[0], [0; 8]);
            assert_eq!(two_parent[0], two_parent[1]);
            let valid_two_parent = v4_reciprocal_audit_builder(&original, &stages, 2, two_parent);
            MockProver::run(
                valid_two_parent.config_params.k as u32,
                &valid_two_parent,
                vec![],
            )
            .expect("valid two-parent reciprocal audit prover")
            .assert_satisfied();

            let mut wrong_second_digest = two_parent;
            wrong_second_digest[1] = [0; 8];
            let wrong_second_digest =
                v4_reciprocal_audit_builder(&original, &stages, 2, wrong_second_digest);
            assert!(
                MockProver::run(
                    wrong_second_digest.config_params.k as u32,
                    &wrong_second_digest,
                    vec![],
                )
                .expect("mismatched two-parent reciprocal audit prover")
                .verify()
                .is_err(),
                "both present parent slots must expose the same complete digest"
            );

            let mut substituted = original;
            substituted.sources.push(-source);
            substituted.equations[7] = vec![(0, C::ScalarExt::ONE), (1, C::ScalarExt::ONE)];
            let adversarial = v4_reciprocal_audit_builder(&substituted, &stages, 1, expected);
            assert!(
                MockProver::run(adversarial.config_params.k as u32, &adversarial, vec![])
                    .expect("adversarial complete reciprocal audit prover")
                    .verify()
                    .is_err(),
                "a satisfiable BranchSelect substitution must fail the scalar-audit join"
            );
        }

        assert_join(EqAffine::generator());
        assert_join(EpAffine::generator());
    }

    fn v4_accumulator(
        parity: KagemushaPastaCycleParityV1,
        k: u32,
    ) -> KagemushaIpaAccumulatorWireV4 {
        use halo2_proofs::halo2curves::{
            group::{GroupEncoding as _, prime::PrimeCurveAffine as _},
            pasta::{EpAffine, EqAffine},
        };

        let folded_generator = match parity {
            KagemushaPastaCycleParityV1::StepEq => {
                let mut bytes = [0; 32];
                bytes.copy_from_slice(EqAffine::generator().to_bytes().as_ref());
                bytes
            }
            KagemushaPastaCycleParityV1::StepEp => {
                let mut bytes = [0; 32];
                bytes.copy_from_slice(EpAffine::generator().to_bytes().as_ref());
                bytes
            }
        };
        KagemushaIpaAccumulatorWireV4 {
            version: crate::zk::kagemusha_accumulation::KAGEMUSHA_IPA_ACCUMULATION_WIRE_VERSION_V4,
            round_count: k,
            round_challenges: vec![[0; 32]; usize::try_from(k).expect("test degree fits")],
            folded_generator,
        }
    }

    fn v4_fold(k: u32, tag: u8, has_parent: bool) -> KagemushaIpaAccumulationProofV4 {
        if !has_parent {
            return KagemushaIpaAccumulationProofV4::initialization(k)
                .expect("supported initialization degree");
        }
        let len = crate::zk::kagemusha_accumulation::kagemusha_ipa_accumulation_proof_bytes_v4(k)
            .expect("supported fold degree");
        KagemushaIpaAccumulationProofV4::from_fold_bytes(k, vec![tag; len])
            .expect("fixed-size fold fixture")
    }

    fn v4_public_inputs(step: u32, parent_count: u32) -> KagemushaPastaCyclePublicInputsV4 {
        assert!((1..=3).contains(&step));
        assert!(parent_count <= 2);
        let k = valid_step_circuit_params_v4().k;
        let mut parent_states = std::array::from_fn(|_| {
            vec![0; iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LIMBS_V5]
        });
        let mut parent_eq_deferred_sha256 = [[0; 8]; KAGEMUSHA_PASTA_PARENT_SLOTS_V1];
        let mut parent_ep_deferred_sha256 = [[0; 8]; KAGEMUSHA_PASTA_PARENT_SLOTS_V1];
        let eq_deferred_sha256 = std::array::from_fn(|index| 0xE410_0000 | index as u32 + 1);
        let ep_deferred_sha256 = std::array::from_fn(|index| 0xE420_0000 | index as u32 + 1);
        for slot in 0..usize::try_from(parent_count).expect("parent count fits") {
            parent_states[slot] =
                exact_state(step - parent_count + u32::try_from(slot).expect("slot fits"));
            parent_eq_deferred_sha256[slot] = eq_deferred_sha256;
            parent_ep_deferred_sha256[slot] = ep_deferred_sha256;
        }
        let has_parent = parent_count != 0;
        KagemushaPastaCyclePublicInputsV4 {
            public_statement_digest: std::array::from_fn(|index| {
                0xA410_0000 | step << 8 | index as u32 + 1
            }),
            operation: KagemushaStepOperationVectorV4::default(),
            parent_count,
            parent_states,
            result_state: exact_state(step),
            manifest_sha256: std::array::from_fn(|index| 0xA500_0000 | index as u32 + 1),
            step_eq_compiled_protocol_sha256: [0xC1C1_C1C1; 8],
            step_ep_compiled_protocol_sha256: [0xC2C2_C2C2; 8],
            parent_eq_lineage_accumulator: has_parent
                .then(|| v4_accumulator(KagemushaPastaCycleParityV1::StepEq, k)),
            parent_ep_lineage_accumulator: has_parent
                .then(|| v4_accumulator(KagemushaPastaCycleParityV1::StepEp, k)),
            parent_eq_deferred_sha256,
            parent_ep_deferred_sha256,
            live_selector: KAGEMUSHA_PASTA_PUBLIC_LIVE_SELECTOR_V4,
        }
    }

    fn v4_pair(step: u32, parent_count: u32) -> KagemushaPastaCycleProofPairV4 {
        let params = valid_step_circuit_params_v4();
        let has_parent = parent_count != 0;
        let private_inputs = v4_public_inputs(step, parent_count);
        KagemushaPastaCycleProofPairV4 {
            version: KAGEMUSHA_PASTA_PROOF_PAIR_VERSION_V4,
            proof_step_count: step,
            public_inputs: KagemushaCompactPublicInputsV5::from_private(&private_inputs, step),
            step_eq_proof_bytes: vec![0x41; params.max_parent_proof_bytes as usize],
            step_ep_proof_bytes: vec![0x42; params.max_parent_proof_bytes as usize],
            step_eq_accumulation_proof: v4_fold(params.k, 0xE1, has_parent),
            step_ep_accumulation_proof: v4_fold(params.k, 0xE2, has_parent),
        }
    }

    #[test]
    fn v4_manifest_preserves_exact_little_endian_state_limbs() {
        let params = valid_step_circuit_params_v4();
        let expected = std::array::from_fn(|index| 0xA500_0000 | index as u32 + 1);
        let mut manifest_bytes = [0_u8; 32];
        for (chunk, limb) in manifest_bytes.chunks_exact_mut(4).zip(expected) {
            chunk.copy_from_slice(&limb.to_le_bytes());
        }

        let exact = kagemusha_exact_u32_public_limbs(manifest_bytes);
        assert_eq!(exact, expected);
        assert_ne!(exact, kagemusha_sha256_public_words(manifest_bytes));

        let mut public_inputs = v4_public_inputs(1, 0);
        public_inputs.manifest_sha256 = exact;
        public_inputs
            .validate(1, &params)
            .expect("exact manifest limbs match the result-state binding");

        public_inputs.manifest_sha256 = kagemusha_sha256_public_words(manifest_bytes);
        assert!(public_inputs.validate(1, &params).is_err());
    }

    #[test]
    fn v5_eq_and_ep_public_columns_share_the_result_state_commitment() {
        use halo2_proofs::halo2curves::pasta::{Fp, Fq};

        let params = valid_step_circuit_params_v4();
        let mut public_inputs = v4_public_inputs(1, 0);
        let original_commitment = kagemusha_poseidon_commitment_chunks_v5(
            KAGEMUSHA_COMPACT_STATE_COMMITMENT_DOMAIN_V5,
            &public_inputs.result_state,
        );
        public_inputs.result_state[crate::zk::kagemusha_v2::S_NEXT_ZERO_LEAF_INDEX] = 37;
        let eq = public_inputs
            .instance_column::<Fp>(1, &params, KagemushaPastaCycleParityV1::StepEq)
            .expect("Eq public column");
        let ep = public_inputs
            .instance_column::<Fq>(1, &params, KagemushaPastaCycleParityV1::StepEp)
            .expect("Ep public column");
        let expected = kagemusha_poseidon_commitment_chunks_v5(
            KAGEMUSHA_COMPACT_STATE_COMMITMENT_DOMAIN_V5,
            &public_inputs.result_state,
        );
        assert_ne!(expected, original_commitment);
        for (index, expected) in expected.into_iter().enumerate() {
            let offset = KAGEMUSHA_COMPACT_RESULT_STATE_COMMITMENT_OFFSET_V5 + index;
            assert_eq!(eq[offset], Fp::from_u128(expected));
            assert_eq!(ep[offset], Fq::from_u128(expected));
        }
    }

    #[test]
    fn v4_public_boundary_rejects_non_live_and_bootstrap_pairs() {
        let params = valid_step_circuit_params_v4();
        let maximum =
            iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_PROOF_PAIR_ABSOLUTE_MAX_BYTES_V4;
        let mut selector_two = v4_public_inputs(1, 0);
        selector_two.live_selector = 2;
        assert!(selector_two.validate(1, &params).is_err());

        let mut bootstrap = v4_public_inputs(1, 0);
        bootstrap.live_selector = KAGEMUSHA_PASTA_PUBLIC_BOOTSTRAP_SELECTOR_V4;
        assert!(bootstrap.validate(1, &params).is_err());

        for selector in [KAGEMUSHA_PASTA_PUBLIC_BOOTSTRAP_SELECTOR_V4, 2] {
            let mut pair = v4_pair(1, 0);
            pair.validate(&params, &params, maximum)
                .expect("live compact pair baseline");
            *pair
                .public_inputs
                .common_header
                .last_mut()
                .expect("compact header carries its selector") = u128::from(selector);
            assert!(pair.validate(&params, &params, maximum).is_err());
            let encoded =
                norito::encode_canonical(&pair).expect("encode adversarial V4 pair canonically");
            assert!(
                validate_kagemusha_proof_pair_measurement_v4(&encoded, &params, &params, maximum,)
                    .is_err(),
                "the public opaque-pair parser must reject selector {selector}"
            );
        }
    }

    #[test]
    fn v4_audit_derivation_prepass_accepts_only_blank_derived_join_slots() {
        let params = valid_step_circuit_params_v4();
        let mut public_inputs = v4_public_inputs(2, 1);
        public_inputs
            .validate(2, &params)
            .expect("proof inputs require authenticated deferred-audit joins");
        assert!(
            public_inputs
                .validate_for_audit_derivation_prepass(2, &params)
                .is_err(),
            "audit derivation prepass must reject a preselected join digest"
        );

        public_inputs.parent_eq_deferred_sha256[0] = [0; 8];
        public_inputs.parent_ep_deferred_sha256[0] = [0; 8];
        public_inputs
            .validate_for_audit_derivation_prepass(2, &params)
            .expect("audit derivation prepass accepts a blank derived-join parent slot");
        assert!(
            public_inputs.validate(2, &params).is_err(),
            "a live proof must require every derived parent audit join"
        );
    }

    #[test]
    fn v4_circuit_mode_rejects_selector_two_nonzero_bootstrap_and_live_all_zero() {
        use halo2_base::gates::circuit::builder::BaseCircuitBuilder;
        use halo2_proofs::dev::MockProver;

        fn builder(
            mode: KagemushaStepPublicModeV4,
        ) -> (BaseCircuitBuilder<Fp>, Vec<Fp>, u32, usize) {
            let layout =
                KagemushaPastaPublicLayoutV4::for_ipa_round_count(valid_step_circuit_params_v4().k)
                    .expect("test public layout");
            let public_len = usize::try_from(layout.instance_column_limbs)
                .expect("test public length fits usize");
            let live_offset =
                usize::try_from(layout.live_selector_offset).expect("test live offset fits usize");
            let mut semantic = vec![Fp::ZERO; public_len];
            semantic[KAGEMUSHA_COMPACT_PROFILE_OFFSET_V5] =
                Fp::from(u64::from(KAGEMUSHA_COMPACT_PROFILE_VERSION_V5));
            semantic[KAGEMUSHA_COMPACT_PROOF_STEP_COUNT_OFFSET_V5] = Fp::ONE;
            semantic[live_offset] = Fp::ONE;
            let mut builder = BaseCircuitBuilder::<Fp>::new(false)
                .use_k(17)
                .use_lookup_bits(8)
                .use_instance_columns(1);
            assign_kagemusha_public_mode_v4(&mut builder, semantic.clone(), &layout, mode)
                .expect("assign test V4 public mode");
            let params = builder.calculate_params(Some(8));
            (
                builder,
                semantic,
                u32::try_from(params.k).expect("small k"),
                live_offset,
            )
        }

        let (bootstrap, _, bootstrap_k, live_offset) =
            builder(KagemushaStepPublicModeV4::Bootstrap);
        let mut zero = vec![Fp::ZERO; live_offset + 1];
        MockProver::run(bootstrap_k, &bootstrap, vec![zero.clone()])
            .expect("bootstrap public-mode prover")
            .assert_satisfied();

        zero[live_offset] = Fp::from(2);
        assert!(
            MockProver::run(bootstrap_k, &bootstrap, vec![zero.clone()])
                .expect("selector-two public-mode prover")
                .verify()
                .is_err()
        );
        zero[live_offset] = Fp::ZERO;
        zero[0] = Fp::ONE;
        assert!(
            MockProver::run(bootstrap_k, &bootstrap, vec![zero])
                .expect("nonzero-bootstrap public-mode prover")
                .verify()
                .is_err()
        );

        let (live, live_instance, live_k, _) = builder(KagemushaStepPublicModeV4::Live);
        MockProver::run(live_k, &live, vec![live_instance])
            .expect("live public-mode prover")
            .assert_satisfied();
        assert!(
            MockProver::run(live_k, &live, vec![vec![Fp::ZERO; live_offset + 1]])
                .expect("live-all-zero public-mode prover")
                .verify()
                .is_err()
        );
    }

    fn v4_bootstrap() -> KagemushaStepBootstrapV4 {
        let params = valid_step_circuit_params_v4();
        let layout = params.validate().expect("valid V4 params");
        KagemushaStepBootstrapV4 {
            version: KAGEMUSHA_STEP_BOOTSTRAP_VERSION_V4,
            parity: KagemushaPastaCycleParityV1::StepEq,
            circuit_params_sha256: params.sha256().expect("identify V4 params"),
            compiled_protocol_structure_sha256: [0x51; 32],
            bootstrap_compiled_protocol_sha256: [0x52; 32],
            circuit_break_points: vec![vec![1]],
            parent_slot: KagemushaStepBootstrapParentSlotV4 {
                instances: vec![vec![
                    0;
                    usize::try_from(layout.instance_column_limbs)
                        .expect("public length fits")
                ]],
                ordinary_proof_bytes: vec![0x53; params.max_parent_proof_bytes as usize],
                carried_lineage: v4_accumulator(KagemushaPastaCycleParityV1::StepEq, params.k),
                post_proof_fold: v4_fold(params.k, 0x54, true),
            },
            branch_merge_fold: v4_fold(params.k, 0x55, true),
        }
    }

    #[test]
    fn v4_bootstrap_is_canonical_manifest_independent_and_profile_bound() {
        let params = valid_step_circuit_params_v4();
        let structure = [0x51; 32];
        let bootstrap = v4_bootstrap();
        bootstrap
            .validate(&params, KagemushaPastaCycleParityV1::StepEq, structure)
            .expect("valid manifest-independent bootstrap");
        let encoded = bootstrap
            .encode_authenticated(&params, KagemushaPastaCycleParityV1::StepEq, structure)
            .expect("encode bootstrap");
        assert_eq!(
            KagemushaStepBootstrapV4::decode_authenticated(
                &encoded,
                &params,
                KagemushaPastaCycleParityV1::StepEq,
                structure,
            )
            .expect("decode canonical bootstrap"),
            bootstrap
        );
        let alternate = encode_with_alternate_norito_layout(&bootstrap);
        assert_ne!(alternate, encoded);
        assert_eq!(
            KagemushaStepBootstrapV4::decode_authenticated(
                &alternate,
                &params,
                KagemushaPastaCycleParityV1::StepEq,
                structure,
            )
            .expect_err("alternate-layout bootstrap must be rejected"),
            "Kagemusha V4 bootstrap payload is not canonical Norito"
        );
        let alternate_flags =
            norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN;
        let ambient_encoded = {
            let _alternate = norito::core::DecodeFlagsGuard::enter(alternate_flags);
            bootstrap
                .encode_authenticated(&params, KagemushaPastaCycleParityV1::StepEq, structure)
                .expect("encode bootstrap under alternate ambient layout")
        };
        assert_eq!(ambient_encoded, encoded);

        let mut missing_break_points = bootstrap.clone();
        missing_break_points.circuit_break_points.clear();
        assert!(
            missing_break_points
                .validate(&params, KagemushaPastaCycleParityV1::StepEq, structure,)
                .is_err(),
            "an authenticated runtime bootstrap must carry its keygen breakpoints"
        );
        let mut wrong_phase_count = bootstrap.clone();
        wrong_phase_count.circuit_break_points.push(vec![]);
        assert!(
            wrong_phase_count
                .validate(&params, KagemushaPastaCycleParityV1::StepEq, structure,)
                .is_err(),
            "breakpoints for a different phase shape must fail closed"
        );
        let mut non_increasing = bootstrap.clone();
        non_increasing.circuit_break_points = vec![vec![2, 2]];
        assert!(
            non_increasing
                .validate(&params, KagemushaPastaCycleParityV1::StepEq, structure,)
                .is_err(),
            "non-increasing cumulative breakpoints must fail closed"
        );
        let mut out_of_domain = bootstrap.clone();
        out_of_domain.circuit_break_points = vec![vec![
            u32::try_from(kagemusha_break_point_max_rows_v5(&params).expect("usable rows"))
                .expect("k17 rows fit u32"),
        ]];
        assert!(
            out_of_domain
                .validate(&params, KagemushaPastaCycleParityV1::StepEq, structure,)
                .is_err(),
            "an out-of-domain breakpoint segment must fail closed"
        );

        for mutation in [
            "version",
            "parity",
            "params_hash",
            "structure",
            "bootstrap_identity",
            "nonzero_instance",
            "short_proof",
            "long_proof",
            "parent_fold",
            "branch_fold",
        ] {
            let mut candidate = bootstrap.clone();
            match mutation {
                "version" => candidate.version ^= 1,
                "parity" => candidate.parity = KagemushaPastaCycleParityV1::StepEp,
                "params_hash" => candidate.circuit_params_sha256[0] ^= 1,
                "structure" => candidate.compiled_protocol_structure_sha256[0] ^= 1,
                "bootstrap_identity" => candidate.bootstrap_compiled_protocol_sha256 = [0; 32],
                "nonzero_instance" => candidate.parent_slot.instances[0][0] = 1,
                "short_proof" => {
                    candidate.parent_slot.ordinary_proof_bytes.pop();
                }
                "long_proof" => candidate.parent_slot.ordinary_proof_bytes.push(0),
                "parent_fold" => {
                    candidate.parent_slot.post_proof_fold.bytes.pop();
                }
                "branch_fold" => {
                    candidate.branch_merge_fold.bytes.pop();
                }
                _ => unreachable!(),
            }
            assert!(
                candidate
                    .validate(&params, KagemushaPastaCycleParityV1::StepEq, structure)
                    .is_err(),
                "bootstrap mutation {mutation} must fail"
            );
        }

        let wrong_profile = valid_step_circuit_params_for_k_v4(21);
        assert!(
            bootstrap
                .validate(
                    &wrong_profile,
                    KagemushaPastaCycleParityV1::StepEq,
                    structure,
                )
                .is_err()
        );
    }

    #[test]
    fn v4_pair_enforces_zero_one_two_parent_shapes_and_exact_bounds() {
        let params = valid_step_circuit_params_v4();
        let maximum =
            iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_PROOF_PAIR_ABSOLUTE_MAX_BYTES_V4;
        for (step, parent_count) in [(1, 0), (2, 1), (3, 2)] {
            let pair = v4_pair(step, parent_count);
            let layout = pair
                .validate(&params, &params, maximum)
                .expect("valid V4 selector shape");
            assert_eq!(
                pair.public_inputs
                    .instance_column::<Fp>(&params, KagemushaPastaCycleParityV1::StepEq)
                    .expect("V4 instance column")
                    .len(),
                usize::try_from(layout.instance_column_limbs).expect("public length fits")
            );
            let bytes = pair
                .encode_authenticated(&params, &params, maximum)
                .expect("encode bounded pair");
            assert_eq!(
                KagemushaPastaCycleProofPairV4::decode_authenticated(
                    &bytes, &params, &params, maximum,
                )
                .expect("decode canonical pair"),
                pair
            );
            let alternate = encode_with_alternate_norito_layout(&pair);
            assert_ne!(alternate, bytes);
            assert_eq!(
                KagemushaPastaCycleProofPairV4::decode_authenticated(
                    &alternate, &params, &params, maximum,
                )
                .expect_err("alternate-layout pair must be rejected"),
                "Kagemusha V4 proof pair is not canonical Norito"
            );
            assert!(
                pair.validate(
                    &params,
                    &params,
                    u32::try_from(bytes.len() - 1).expect("fixture size fits"),
                )
                .is_err(),
                "pair cap below the canonical payload must fail"
            );
        }

        let mut invalid_count = v4_pair(3, 2);
        invalid_count.public_inputs.common_header[KAGEMUSHA_COMPACT_PARENT_COUNT_OFFSET_V5] = 3;
        assert!(invalid_count.validate(&params, &params, maximum).is_err());

        let canonical = v4_pair(3, 2);
        let mut bundle_ordered_private = v4_public_inputs(3, 2);
        assert!(bundle_ordered_private.parent_states[0] < bundle_ordered_private.parent_states[1]);
        bundle_ordered_private.parent_states.swap(0, 1);
        bundle_ordered_private.parent_eq_deferred_sha256.swap(0, 1);
        bundle_ordered_private.parent_ep_deferred_sha256.swap(0, 1);
        assert!(bundle_ordered_private.parent_states[0] > bundle_ordered_private.parent_states[1]);
        bundle_ordered_private
            .validate(3, &params)
            .expect("private parent slots preserve bundle-digest order");
        let mut bundle_ordered = v4_pair(3, 2);
        bundle_ordered.public_inputs =
            KagemushaCompactPublicInputsV5::from_private(&bundle_ordered_private, 3);
        let parent_commitments = KAGEMUSHA_COMPACT_PARENT_STATE_COMMITMENTS_OFFSET_V5;
        assert_eq!(
            &bundle_ordered.public_inputs.common_header[parent_commitments..parent_commitments + 2],
            &canonical.public_inputs.common_header[parent_commitments + 2..parent_commitments + 4],
        );
        assert_eq!(
            &bundle_ordered.public_inputs.common_header
                [parent_commitments + 2..parent_commitments + 4],
            &canonical.public_inputs.common_header[parent_commitments..parent_commitments + 2],
        );
        assert_eq!(
            bundle_ordered.public_inputs.parent_eq_deferred_chunks[0],
            canonical.public_inputs.parent_eq_deferred_chunks[1],
        );
        assert_eq!(
            bundle_ordered.public_inputs.parent_ep_deferred_chunks[0],
            canonical.public_inputs.parent_ep_deferred_chunks[1],
        );
        bundle_ordered
            .validate(&params, &params, maximum)
            .expect("V5 compact parent slots follow bundle-digest order, not state-vector order");

        let mut short = v4_pair(2, 1);
        short.step_eq_proof_bytes.pop();
        assert!(short.validate(&params, &params, maximum).is_err());
        let mut long = v4_pair(2, 1);
        long.step_ep_proof_bytes.push(0);
        assert!(long.validate(&params, &params, maximum).is_err());

        let wrong_layout = valid_step_circuit_params_for_k_v4(21);
        assert!(
            v4_pair(1, 0)
                .validate(&params, &wrong_layout, maximum)
                .is_err()
        );
        let mut missing_manifest = v4_pair(2, 1);
        missing_manifest.public_inputs.common_header[KAGEMUSHA_COMPACT_MANIFEST_SHA256_OFFSET_V5
            ..KAGEMUSHA_COMPACT_MANIFEST_SHA256_OFFSET_V5 + 2]
            .fill(0);
        assert!(
            missing_manifest
                .validate(&params, &params, maximum)
                .is_err()
        );
    }

    #[test]
    fn v4_missing_bootstrap_rejects_without_generating_padding() {
        assert!(require_kagemusha_step_bootstrap_v4(None, "Eq").is_err());
        assert!(require_kagemusha_step_bootstrap_v4(None, "Ep").is_err());
    }

    /// Keep the exact same-field Pasta recursion tuples executable.
    ///
    /// An Eq IPA proof uses `ParamsIPA<EqAffine>` and has scalar field `Fp`, so
    /// its direct Axiom circuit verifier must also be an `Fp` circuit with a
    /// `Halo2Loader<EqAffine, BaseFieldEccChip<EqAffine>>`. The reciprocal Ep
    /// tuple is `ParamsIPA<EpAffine>` / `Fq` /
    /// `Halo2Loader<EpAffine, BaseFieldEccChip<EpAffine>>`. This test is a
    /// compile-time guard against accidentally diagnosing that supported path
    /// as a Pasta trait mismatch.
    #[test]
    fn same_field_pasta_loader_type_tuples_compile() {
        use halo2_base::gates::circuit::{BaseCircuitParams, builder::BaseCircuitBuilder};
        use halo2_ecc::{ecc::BaseFieldEccChip, fields::fp::FpChip};
        use halo2_proofs::halo2curves::pasta::{EpAffine, EqAffine};
        use snark_verifier::loader::halo2::Halo2Loader;

        const LIMB_BITS: usize = 86;
        const LIMBS: usize = 3;
        let seed = BaseCircuitParams {
            k: 12,
            num_advice_per_phase: vec![1],
            num_lookup_advice_per_phase: vec![1],
            num_fixed: 1,
            lookup_bits: Some(11),
            num_instance_columns: 1,
        };

        let mut eq_outer = BaseCircuitBuilder::<Fp>::new(false).use_params(seed.clone());
        let eq_range = eq_outer.range_chip();
        let eq_base = FpChip::<Fp, Fq>::new(&eq_range, LIMB_BITS, LIMBS);
        let eq_loader = Halo2Loader::new(
            BaseFieldEccChip::<EqAffine>::new(&eq_base),
            mem::take(eq_outer.pool(0)),
        );
        fn require_eq_tuple(_: &Rc<Halo2Loader<EqAffine, BaseFieldEccChip<'_, EqAffine>>>) {}
        require_eq_tuple(&eq_loader);
        *eq_outer.pool(0) = eq_loader.take_ctx();

        let mut ep_outer = BaseCircuitBuilder::<Fq>::new(false).use_params(seed);
        let ep_range = ep_outer.range_chip();
        let ep_base = FpChip::<Fq, Fp>::new(&ep_range, LIMB_BITS, LIMBS);
        let ep_loader = Halo2Loader::new(
            BaseFieldEccChip::<EpAffine>::new(&ep_base),
            mem::take(ep_outer.pool(0)),
        );
        fn require_ep_tuple(_: &Rc<Halo2Loader<EpAffine, BaseFieldEccChip<'_, EpAffine>>>) {}
        require_ep_tuple(&ep_loader);
        *ep_outer.pool(0) = ep_loader.take_ctx();
    }

    #[test]
    fn protocol_private_enum_projection_is_explicit_and_fail_closed() {
        use ciborium::value::Value;

        assert_eq!(
            encode_common_polynomial_value(Value::Text("Identity".to_owned()))
                .expect("identity common polynomial"),
            vec![1, 0]
        );
        let mut expected_lagrange = vec![1, 1];
        expected_lagrange.extend_from_slice(&(-7_i32).to_le_bytes());
        assert_eq!(
            encode_common_polynomial_value(Value::Map(vec![(
                Value::Text("Lagrange".to_owned()),
                Value::Integer((-7_i64).into()),
            )]))
            .expect("Lagrange common polynomial"),
            expected_lagrange
        );
        for malformed in [
            Value::Text("Unknown".to_owned()),
            Value::Map(Vec::new()),
            Value::Map(vec![(
                Value::Text("Lagrange".to_owned()),
                Value::Text("zero".to_owned()),
            )]),
            Value::Map(vec![(
                Value::Text("Unknown".to_owned()),
                Value::Integer(0.into()),
            )]),
            Value::Map(vec![(
                Value::Text("Lagrange".to_owned()),
                Value::Integer(i64::MAX.into()),
            )]),
        ] {
            assert!(encode_common_polynomial_value(malformed).is_err());
        }

        assert_eq!(encode_linearization_value(Value::Null), Ok(0));
        assert_eq!(
            encode_linearization_value(Value::Text("WithoutConstant".to_owned())),
            Ok(1)
        );
        assert_eq!(
            encode_linearization_value(Value::Text("MinusVanishingTimesQuotient".to_owned())),
            Ok(2)
        );
        assert!(
            encode_linearization_value(Value::Text("Unknown".to_owned())).is_err(),
            "an upstream enum extension requires an identity-version review"
        );
    }

    #[test]
    fn compact_poseidon_sha_wrappers_are_one_block_and_bind_metadata() {
        assert!(KAGEMUSHA_COMPILED_PROTOCOL_IDENTITY_SHA256_DOMAIN_V2.len() <= 18);
        assert!(
            crate::zk::kagemusha_cycle_loader::KAGEMUSHA_DEFERRED_AUDIT_SHA256_DOMAIN_V6.len()
                <= 18
        );
        let wrapper_domains: [&[u8]; 2] = [
            KAGEMUSHA_COMPILED_PROTOCOL_IDENTITY_SHA256_DOMAIN_V2,
            crate::zk::kagemusha_cycle_loader::KAGEMUSHA_DEFERRED_AUDIT_SHA256_DOMAIN_V6,
        ];
        for domain in wrapper_domains {
            assert!(domain.len() + 1 + 4 + 32 <= 55);
        }

        let digest = Fp::from(0x5a_u64);
        let protocol = kagemusha_short_poseidon_sha256(
            KAGEMUSHA_COMPILED_PROTOCOL_IDENTITY_SHA256_DOMAIN_V2,
            KAGEMUSHA_COMPILED_PROTOCOL_IDENTITY_VERSION_V2,
            digest,
        )
        .expect("protocol wrapper");
        assert_ne!(
            protocol,
            kagemusha_short_poseidon_sha256(
                KAGEMUSHA_COMPILED_PROTOCOL_IDENTITY_SHA256_DOMAIN_V2,
                KAGEMUSHA_COMPILED_PROTOCOL_IDENTITY_VERSION_V2 + 1,
                digest,
            )
            .expect("version-tampered wrapper")
        );
        assert_ne!(
            protocol,
            kagemusha_short_poseidon_sha256(
                crate::zk::kagemusha_cycle_loader::KAGEMUSHA_DEFERRED_AUDIT_SHA256_DOMAIN_V6,
                KAGEMUSHA_COMPILED_PROTOCOL_IDENTITY_VERSION_V2,
                digest,
            )
            .expect("domain-tampered wrapper")
        );
    }

    #[test]
    fn protocol_identity_v2_matches_native_scalar_and_reciprocal_in_both_parities() {
        use halo2_base::{
            gates::circuit::{BaseCircuitParams, builder::BaseCircuitBuilder},
            utils::ScalarField,
        };
        use halo2_ecc::fields::fp::FpChip;
        use halo2_proofs::{
            dev::MockProver,
            halo2curves::pasta::{EpAffine, EqAffine},
            poly::{commitment::ParamsProver as _, ipa::commitment::ParamsIPA},
        };
        use snark_verifier::loader::halo2::Halo2Loader;

        use crate::zk::kagemusha_cycle_loader::{
            DeferredScalarEccChip, LIMB_BITS, LIMBS, PastaCycleEccChip,
        };

        fn assert_parity<C>(parity: KagemushaPastaCycleParityV1)
        where
            C: halo2_base::utils::CurveAffineExt,
            C::Base: halo2_base::utils::BigPrimeField + ScalarField + ff::WithSmallOrderMulGroup<3>,
            C::ScalarExt:
                halo2_base::utils::BigPrimeField + ScalarField + ff::WithSmallOrderMulGroup<3>,
        {
            let protocol_params = BaseCircuitParams {
                k: 8,
                num_advice_per_phase: vec![2],
                num_lookup_advice_per_phase: vec![1],
                num_fixed: 1,
                lookup_bits: Some(7),
                num_instance_columns: 1,
            };
            let target = KagemushaUniversalProtocolTargetV1 {
                base_circuit_params: protocol_params,
                instance_column_lengths: vec![1],
            };
            let params = ParamsIPA::<C>::new(8);
            let protocol = kagemusha_bootstrap_compiled_protocol_v1(&params, &target)
                .expect("small compiled protocol");
            let structure = kagemusha_compiled_protocol_structure_sha256(&protocol, parity)
                .expect("small protocol structure");
            let native = kagemusha_compiled_protocol_identity_sha256(&protocol, parity)
                .expect("native V2 protocol identity");
            let native_words = kagemusha_sha256_public_words(native);
            let native_chunks = kagemusha_u32_words_to_u128_chunks_v5(&native_words);

            let mut scalar_builder = BaseCircuitBuilder::<C::ScalarExt>::new(false)
                .use_k(17)
                .use_lookup_bits(16);
            let scalar_range = scalar_builder.range_chip();
            let coordinate = FpChip::<C::ScalarExt, C::Base>::new(&scalar_range, LIMB_BITS, LIMBS);
            let scalar_integer =
                FpChip::<C::ScalarExt, C::ScalarExt>::new(&scalar_range, LIMB_BITS, LIMBS);
            let scalar_loader = Halo2Loader::new(
                DeferredScalarEccChip::<C>::new(&coordinate, &scalar_integer),
                mem::take(scalar_builder.pool(0)),
            );
            let scalar_expected = {
                let mut ctx = scalar_loader.ctx_mut();
                native_chunks.map(|chunk| ctx.main().load_witness(C::ScalarExt::from_u128(chunk)))
            };
            let mut scalar_sha_jobs = KagemushaSha256JobsV4::default();
            let loaded = scalar_lineage_v1::load_and_constrain_parent_protocol(
                &scalar_loader,
                &mut scalar_sha_jobs,
                &protocol,
                parity,
                structure,
                &scalar_expected,
            )
            .expect("scalar V2 protocol identity");
            assert_eq!(assigned_digest_words(&loaded.identity_digest), native_words);
            let identity = loaded.identity_witness.clone();
            let mut audit_witness = scalar_loader.ecc_chip().witness();
            assert_eq!(
                identity.preprocessed_source_indices.len(),
                identity.preprocessed.len()
            );
            assert!(
                identity
                    .preprocessed_source_indices
                    .windows(2)
                    .all(|indices| indices[0] < indices[1])
            );
            assert!(
                identity
                    .preprocessed
                    .iter()
                    .zip(&identity.preprocessed_source_indices)
                    .all(|(point, source_index)| {
                        audit_witness.sources.get(*source_index) == Some(point)
                    })
            );
            audit_witness.equations.push(vec![(0, C::ScalarExt::ZERO)]);
            drop(loaded);
            *scalar_builder.pool(0) = scalar_loader.take_ctx();
            scalar_builder.calculate_params(Some(9));
            MockProver::run(
                scalar_builder.config_params.k as u32,
                &scalar_builder,
                vec![],
            )
            .expect("scalar V2 protocol-identity prover")
            .assert_satisfied();

            let mut reciprocal_builder = BaseCircuitBuilder::<C::Base>::new(false)
                .use_k(17)
                .use_lookup_bits(16);
            let reciprocal_range = reciprocal_builder.range_chip();
            let base = FpChip::<C::Base, C::Base>::new(&reciprocal_range, LIMB_BITS, LIMBS);
            let scalar = FpChip::<C::Base, C::ScalarExt>::new(&reciprocal_range, LIMB_BITS, LIMBS);
            let mut ctx = mem::take(reciprocal_builder.pool(0));
            let reciprocal_expected =
                native_chunks.map(|chunk| ctx.main().load_witness(C::Base::from_u128(chunk)));
            let selectors = [ctx.main().load_witness(C::Base::ONE)];
            let reciprocal_chip = PastaCycleEccChip::<C>::new(&base, &scalar);
            let audit = reciprocal_chip
                .assign_deferred_equations_with_selectors(&mut ctx, &audit_witness, &selectors)
                .expect("reciprocal V6 source assignment");
            let (_, source_encodings) = reciprocal_chip
                .assigned_equation_poseidon_elements_v6(&mut ctx, &audit, &[0], &selectors)
                .expect("reciprocal V6 source encodings");
            let mut reciprocal_sha_jobs = KagemushaSha256JobsV4::default();
            let reciprocal_digest = constrain_reciprocal_protocol_identity::<C>(
                &mut ctx,
                &mut reciprocal_sha_jobs,
                &base,
                &scalar,
                &source_encodings,
                &identity,
                structure,
                &reciprocal_expected,
            )
            .expect("reciprocal V2 protocol identity");
            assert_eq!(assigned_digest_words(&reciprocal_digest), native_words);
            *reciprocal_builder.pool(0) = ctx;
            reciprocal_builder.calculate_params(Some(9));
            MockProver::run(
                reciprocal_builder.config_params.k as u32,
                &reciprocal_builder,
                vec![],
            )
            .expect("reciprocal V2 protocol-identity prover")
            .assert_satisfied();
        }

        assert_parity::<EqAffine>(KagemushaPastaCycleParityV1::StepEq);
        assert_parity::<EpAffine>(KagemushaPastaCycleParityV1::StepEp);
    }

    #[test]
    fn universal_protocol_bootstrap_converges_for_the_same_base_config() {
        use halo2_base::gates::{GateInstructions as _, RangeInstructions as _};
        use halo2_proofs::{
            SerdeFormat,
            halo2curves::pasta::EqAffine,
            plonk::{keygen_pk, keygen_vk},
            poly::{commitment::ParamsProver as _, ipa::commitment::ParamsIPA},
        };
        use snark_verifier::system::halo2::{Config, compile};

        let base_circuit_params = halo2_base::gates::circuit::BaseCircuitParams {
            k: 8,
            num_advice_per_phase: vec![2],
            num_lookup_advice_per_phase: vec![1],
            num_fixed: 1,
            lookup_bits: Some(7),
            num_instance_columns: 1,
        };
        let target = KagemushaUniversalProtocolTargetV1 {
            base_circuit_params: base_circuit_params.clone(),
            instance_column_lengths: vec![1],
        };
        let params = ParamsIPA::<EqAffine>::new(8);
        let bootstrap_circuit = KagemushaProtocolBootstrapCircuit {
            params: base_circuit_params.clone(),
            marker: std::marker::PhantomData,
        };
        let separate_vk = kagemusha_bootstrap_verifying_key_v1(&params, &target)
            .expect("separate bootstrap VK generation");
        let separate_pk = keygen_pk(&params, separate_vk, &bootstrap_circuit)
            .expect("separate bootstrap PK generation");
        let combined_pk = kagemusha_bootstrap_proving_key_v1(&params, &target, &bootstrap_circuit)
            .expect("single-synthesis bootstrap PK generation");
        assert_eq!(
            separate_pk.to_bytes(SerdeFormat::Processed),
            combined_pk.to_bytes(SerdeFormat::Processed),
            "single-synthesis bootstrap keygen must preserve the exact processed key"
        );
        drop(combined_pk);
        drop(separate_pk);
        let bootstrap = kagemusha_bootstrap_compiled_protocol_v1(&params, &target)
            .expect("deterministic bootstrap protocol");
        assert_eq!(bootstrap.num_instance, vec![1]);
        assert!(
            bootstrap.instance_committing_key.is_none(),
            "canonical V4 compilation must evaluate public instances directly"
        );
        let bootstrap_structure = kagemusha_compiled_protocol_structure_sha256(
            &bootstrap,
            KagemushaPastaCycleParityV1::StepEq,
        )
        .expect("canonical bootstrap structure");
        assert_eq!(
            bootstrap_structure,
            kagemusha_compiled_protocol_structure_sha256(
                &bootstrap,
                KagemushaPastaCycleParityV1::StepEq,
            )
            .expect("repeat canonical bootstrap structure"),
            "the explicit protocol descriptor must be stable"
        );
        assert_ne!(
            bootstrap_structure,
            kagemusha_compiled_protocol_structure_sha256(
                &bootstrap,
                KagemushaPastaCycleParityV1::StepEp,
            )
            .expect("opposite-parity protocol descriptor"),
            "the same protocol bytes must remain parity-domain-separated"
        );

        let assert_structure_changes = |label: &str, protocol: &PlonkProtocol<EqAffine>| {
            assert_ne!(
                bootstrap_structure,
                kagemusha_compiled_protocol_structure_sha256(
                    protocol,
                    KagemushaPastaCycleParityV1::StepEq,
                )
                .expect("mutated protocol structure"),
                "the {label} verifier-control-flow category must affect the descriptor"
            );
        };

        let mut changed_domain = bootstrap.clone();
        changed_domain.domain.k += 1;
        assert_structure_changes("domain", &changed_domain);

        let mut changed_instance_count = bootstrap.clone();
        changed_instance_count.num_instance.push(0);
        assert_structure_changes("instance count", &changed_instance_count);
        let mut changed_witness_count = bootstrap.clone();
        changed_witness_count.num_witness.push(1);
        assert_structure_changes("witness count", &changed_witness_count);
        let mut changed_challenge_count = bootstrap.clone();
        changed_challenge_count.num_challenge.push(1);
        assert_structure_changes("challenge count", &changed_challenge_count);

        let mut changed_evaluations = bootstrap.clone();
        changed_evaluations
            .evaluations
            .first_mut()
            .expect("compiled protocol has an evaluation")
            .poly += 1;
        assert_structure_changes("evaluation", &changed_evaluations);
        let mut changed_queries = bootstrap.clone();
        changed_queries
            .queries
            .first_mut()
            .expect("compiled protocol has an opening query")
            .rotation
            .0 += 1;
        assert_structure_changes("opening query", &changed_queries);

        let mut changed_quotient = bootstrap.clone();
        changed_quotient.quotient.chunk_degree += 1;
        assert_structure_changes("quotient", &changed_quotient);

        let bootstrap_vk = kagemusha_bootstrap_verifying_key_v1(&params, &target)
            .expect("deterministic bootstrap verifying key");
        let queried_instance_protocol = compile(
            &params,
            &bootstrap_vk,
            Config::ipa().with_num_instance(vec![1]),
        );
        assert!(
            queried_instance_protocol.instance_committing_key.is_some(),
            "the upstream IPA default remains queried-instance mode"
        );
        assert_structure_changes("queried-instance presence", &queried_instance_protocol);

        // `LinearizationStrategy` is intentionally not re-exported by the
        // pinned dependency. Its derived Ciborium representation still lets
        // this regression exercise the public protocol field without copying
        // the dependency's private enum into Iroha.
        let mut changed_linearization = bootstrap.clone();
        changed_linearization.linearization =
            ciborium::value::Value::Text("WithoutConstant".to_owned())
                .deserialized()
                .expect("deserialize explicit linearization variant");
        assert_structure_changes("linearization", &changed_linearization);

        let mut changed_accumulator_indices = bootstrap.clone();
        changed_accumulator_indices
            .accumulator_indices
            .push(vec![(0, 0)]);
        assert_structure_changes("accumulator indices", &changed_accumulator_indices);

        let mut changed_transcript_presence = bootstrap.clone();
        changed_transcript_presence.transcript_initial_state = None;
        assert_structure_changes("transcript presence", &changed_transcript_presence);

        let mut changed_preprocessed_length = bootstrap.clone();
        changed_preprocessed_length.preprocessed.pop();
        assert_structure_changes("preprocessed length", &changed_preprocessed_length);

        let bootstrap_identity = kagemusha_compiled_protocol_identity_sha256(
            &bootstrap,
            KagemushaPastaCycleParityV1::StepEq,
        )
        .expect("bootstrap identity");
        let mut changed_preprocessed_value = bootstrap.clone();
        changed_preprocessed_value.preprocessed[0] = EqAffine::identity();
        assert_eq!(
            bootstrap_structure,
            kagemusha_compiled_protocol_structure_sha256(
                &changed_preprocessed_value,
                KagemushaPastaCycleParityV1::StepEq,
            )
            .expect("structure with changed preprocessed value"),
            "only preprocessed point values are scrubbed from the fixed descriptor"
        );

        let mut changed_transcript_value = bootstrap.clone();
        changed_transcript_value.transcript_initial_state = changed_transcript_value
            .transcript_initial_state
            .map(|state| state + Fp::ONE);
        assert_eq!(
            bootstrap_structure,
            kagemusha_compiled_protocol_structure_sha256(
                &changed_transcript_value,
                KagemushaPastaCycleParityV1::StepEq,
            )
            .expect("structure with changed transcript value"),
            "only the transcript-state value is scrubbed from the fixed descriptor"
        );
        assert!(
            kagemusha_compiled_protocol_identity_sha256(
                &changed_preprocessed_value,
                KagemushaPastaCycleParityV1::StepEq,
            )
            .is_err(),
            "the complete identity must reject identity preprocessed points"
        );
        assert_ne!(
            bootstrap_identity,
            kagemusha_compiled_protocol_identity_sha256(
                &changed_transcript_value,
                KagemushaPastaCycleParityV1::StepEq,
            )
            .expect("identity with changed transcript value"),
            "the complete identity must authenticate the transcript-state value"
        );

        let mut missing_transcript_state = bootstrap.clone();
        missing_transcript_state.transcript_initial_state = None;
        assert!(
            kagemusha_compiled_protocol_identity_sha256(
                &missing_transcript_state,
                KagemushaPastaCycleParityV1::StepEq,
            )
            .is_err(),
            "a protocol without its authenticated transcript state must fail closed"
        );

        let mut final_builder =
            halo2_base::gates::circuit::builder::BaseCircuitBuilder::<Fp>::new(false)
                .use_params(base_circuit_params.clone());
        let range = final_builder.range_chip();
        let public = {
            let ctx = final_builder.main(0);
            let lhs = ctx.load_witness(Fp::from(17));
            let rhs = ctx.load_witness(Fp::from(25));
            range.range_check(ctx, lhs, 8);
            range.range_check(ctx, rhs, 8);
            range.gate().add(ctx, lhs, rhs)
        };
        final_builder.assigned_instances = vec![vec![public]];
        let final_vk = keygen_vk(&params, &final_builder).expect("final universal BaseConfig VK");
        let captured_break_points = final_builder.break_points();
        assert_eq!(
            kagemusha_break_points_from_wire_v4(
                &kagemusha_break_points_to_wire_v4(&captured_break_points)
                    .expect("encode captured breakpoints")
            )
            .expect("decode captured breakpoints"),
            captured_break_points,
            "captured breakpoints must round-trip through the portable header width"
        );
        let final_protocol = compile(&params, &final_vk, kagemusha_ipa_compile_config_v4(1));
        assert!(
            final_protocol.instance_committing_key.is_none(),
            "final V4 compilation must evaluate public instances directly"
        );
        kagemusha_require_protocol_structure_v1(
            &bootstrap,
            &final_protocol,
            KagemushaPastaCycleParityV1::StepEq,
        )
        .expect("the universal target must converge in one pass");
        assert_ne!(
            kagemusha_compiled_protocol_identity_sha256(
                &bootstrap,
                KagemushaPastaCycleParityV1::StepEq,
            )
            .expect("bootstrap identity"),
            kagemusha_compiled_protocol_identity_sha256(
                &final_protocol,
                KagemushaPastaCycleParityV1::StepEq,
            )
            .expect("final identity"),
            "the static shape converges while dynamic VK values remain distinct"
        );

        let final_pk = keygen_pk(&params, final_vk.clone(), &final_builder)
            .expect("direct-instance test proving key");
        assert_eq!(
            final_builder.break_points(),
            captured_break_points,
            "PK synthesis must reproduce the VK layout"
        );
        let mut prover_builder =
            halo2_base::gates::circuit::builder::BaseCircuitBuilder::<Fp>::prover(
                base_circuit_params,
                captured_break_points,
            );
        let range = prover_builder.range_chip();
        let public = {
            let ctx = prover_builder.main(0);
            let lhs = ctx.load_witness(Fp::from(17));
            let rhs = ctx.load_witness(Fp::from(25));
            range.range_check(ctx, lhs, 8);
            range.range_check(ctx, rhs, 8);
            range.gate().add(ctx, lhs, rhs)
        };
        prover_builder.assigned_instances = vec![vec![public]];
        assert!(
            prover_builder.witness_gen_only(),
            "the proof circuit must use the witness-only prover stage"
        );
        let instances = vec![vec![Fp::from(42)]];
        let (proof, _) =
            create_augmented_eq_proof_v4(&params, final_pk, prover_builder, &instances)
                .expect("direct-instance augmented proof");
        let decide = |candidate: &[Vec<Fp>]| -> Result<(), String> {
            let current = succinct_verify_step_eq_instances(
                &params,
                &final_vk,
                &proof,
                candidate,
                proof.len(),
            )?;
            let initialization = KagemushaIpaAccumulationProofV4::initialization(8)?;
            crate::zk::kagemusha_accumulation::verify_and_decide_eq_accumulation_v4(
                &params,
                8,
                current,
                None,
                &initialization,
            )
            .map(|_| ())
        };
        decide(&instances).expect("direct-instance IPA proof round-trip");
        assert!(
            decide(&[vec![Fp::from(43)]]).is_err(),
            "substituting a non-zero public instance must fail"
        );
    }

    fn exact_state(step: u32) -> Vec<u32> {
        let mut state =
            vec![0; iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LIMBS_V5];
        state[0] =
            iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LAYOUT_VERSION_V5;
        state[1] = step;
        for (index, limb) in state.iter_mut().enumerate().skip(2) {
            *limb = step
                .wrapping_mul(1_003)
                .wrapping_add(u32::try_from(index).expect("state-vector index fits u32"));
        }
        let offset = |field: &str| {
            crate::zk::kagemusha_v2::KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LAYOUT_V5
                .iter()
                .find_map(|(name, start, _)| (*name == field).then_some(*start))
                .expect("state fixture field exists")
        };
        state[offset("proof_step_count")] = step;
        state[offset("peer_hop_count")] = step
            .saturating_sub(1)
            .min(iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_MAX_PEER_HOPS_V2);
        let manifest = offset("artifact_manifest_sha256");
        for (index, limb) in state[manifest..manifest + 8].iter_mut().enumerate() {
            *limb = 0xA500_0000 | u32::try_from(index + 1).expect("digest index fits u32");
        }
        state
    }

    #[test]
    fn v5_pre_keygen_parent_extraction_accepts_only_provisional_empty_breakpoints() {
        let params = valid_step_circuit_params_v4();
        let structure = [0x51; 32];
        let mut bootstrap = v4_bootstrap();
        assert!(
            bootstrap
                .step_eq_parent_internal(
                    &params,
                    structure,
                    0,
                    KagemushaBootstrapParentValidationV4::ProvisionalPreKeygen,
                )
                .is_err(),
            "the provisional path must reject even valid populated keygen breakpoints"
        );
        bootstrap.circuit_break_points.clear();

        assert!(
            bootstrap.step_eq_parent(&params, structure, 0).is_err(),
            "strict parent extraction must require authenticated keygen breakpoints"
        );
        let parent = bootstrap
            .step_eq_parent_internal(
                &params,
                structure,
                0,
                KagemushaBootstrapParentValidationV4::ProvisionalPreKeygen,
            )
            .expect("the pre-keygen seed may omit breakpoints before they are captured");
        assert_eq!(
            parent.instances,
            vec![vec![Fp::ZERO; parent.instances[0].len()]]
        );

        bootstrap.circuit_break_points = vec![vec![], vec![]];
        assert!(
            bootstrap
                .step_eq_parent_internal(
                    &params,
                    structure,
                    0,
                    KagemushaBootstrapParentValidationV4::ProvisionalPreKeygen,
                )
                .is_err(),
            "the provisional path must still reject malformed non-empty breakpoints"
        );

        let mut ep_bootstrap = v4_bootstrap();
        ep_bootstrap.parity = KagemushaPastaCycleParityV1::StepEp;
        ep_bootstrap.circuit_break_points.clear();
        ep_bootstrap.parent_slot.carried_lineage =
            v4_accumulator(KagemushaPastaCycleParityV1::StepEp, params.k);
        assert!(
            ep_bootstrap.step_ep_parent(&params, structure, 0).is_err(),
            "strict Ep extraction must require authenticated keygen breakpoints"
        );
        ep_bootstrap
            .step_ep_parent_internal(
                &params,
                structure,
                0,
                KagemushaBootstrapParentValidationV4::ProvisionalPreKeygen,
            )
            .expect("the pre-keygen Ep seed may omit breakpoints before they are captured");
    }
