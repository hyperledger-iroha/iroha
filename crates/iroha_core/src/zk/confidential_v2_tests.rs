#[cfg(test)]
mod tests {
    fn schema_public_input_order(schema: &[u8]) -> Vec<String> {
        let value: norito::json::Value =
            norito::json::from_slice(schema).expect("public-input schema must be valid JSON");
        let norito::json::Value::Object(fields) = value else {
            panic!("public-input schema must be a JSON object");
        };
        fields
            .get("public_inputs")
            .and_then(norito::json::Value::as_array)
            .expect("public-input schema must carry a public_inputs array")
            .iter()
            .map(|value| {
                value
                    .as_str()
                    .expect("public-input column names must be strings")
                    .to_owned()
            })
            .collect()
    }

    #[test]
    fn topup_and_unshield_named_binding_orders_match_pinned_schemas() {
        for (schema, assigned_order) in [
            (
                super::KAGEMUSHA_TOPUP_SHIELD_V2_PUBLIC_INPUTS_SCHEMA_V1,
                super::KAGEMUSHA_TOPUP_SHIELD_V2_PUBLIC_INPUT_ORDER_V1,
            ),
            (
                super::CONFIDENTIAL_UNSHIELD_V2_PUBLIC_INPUTS_SCHEMA_V1,
                super::CONFIDENTIAL_UNSHIELD_V2_PUBLIC_INPUT_ORDER_V1,
            ),
            (
                super::CONFIDENTIAL_UNSHIELD_V3_PUBLIC_INPUTS_SCHEMA_V1,
                super::CONFIDENTIAL_UNSHIELD_V3_PUBLIC_INPUT_ORDER_V1,
            ),
        ] {
            let schema_order = schema_public_input_order(schema);
            assert!(
                schema_order
                    .iter()
                    .map(String::as_str)
                    .eq(assigned_order.iter().copied()),
                "named circuit binding order drifted from the authenticated schema"
            );
        }
    }

    #[test]
    fn public_input_extraction_requires_canonical_outer_and_rejects_raw_zk1() {
        use halo2_proofs::halo2curves::pasta::Fp;

        let columns = (1_u64..=9)
            .map(|value| vec![Fp::from(value)])
            .collect::<Vec<_>>();
        let column_refs = columns.iter().map(Vec::as_slice).collect::<Vec<_>>();
        let mut zk1 = crate::zk::zk1_test_helpers::wrap_start();
        crate::zk::zk1_test_helpers::wrap_append_proof(&mut zk1, &[0xA5]);
        crate::zk::zk1_test_helpers::wrap_append_instances_pasta_fp_cols(&column_refs, &mut zk1);
        let envelope = iroha_data_model::zk::OpenVerifyEnvelope {
            backend: iroha_data_model::zk::BackendTag::Halo2IpaPasta,
            circuit_id: super::CONFIDENTIAL_TRANSFER_V2_CIRCUIT_ID.to_owned(),
            vk_hash: [0x42; 32],
            public_inputs: super::CONFIDENTIAL_TRANSFER_V2_PUBLIC_INPUTS_SCHEMA_V1.to_vec(),
            proof_bytes: zk1.clone(),
            aux: Vec::new(),
        };
        let canonical =
            norito::encode_canonical(&envelope).expect("encode canonical confidential envelope");
        let parsed = super::parse_transfer_public_inputs(&canonical)
            .expect("canonical confidential envelope exposes public inputs");
        assert_eq!(parsed.0[0], scalar_bytes(1));

        let alternate_flags =
            norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN;
        let alternate_outer = {
            let _alternate = norito::core::DecodeFlagsGuard::enter(alternate_flags);
            norito::to_bytes(&envelope)
                .expect("encode alternate-layout confidential outer envelope")
        };
        assert_ne!(alternate_outer, canonical);
        norito::decode_from_bytes::<iroha_data_model::zk::OpenVerifyEnvelope>(&alternate_outer)
            .expect("ordinary Norito accepts the advertised layout");
        assert!(
            super::parse_transfer_public_inputs(&alternate_outer).is_err(),
            "alternate-layout outer envelope must be rejected"
        );
        assert!(
            super::parse_transfer_public_inputs(&zk1).is_err(),
            "raw ZK1 payload must not bypass the V1 outer envelope"
        );
    }

    #[test]
    fn public_input_extraction_rejects_alternate_layout_stark_wrapper() {
        let columns = (1_u8..=9)
            .map(|value| vec![[value; 32]])
            .collect::<Vec<_>>();
        let wrapper = iroha_data_model::zk::StarkFriOpenProofV1 {
            version: 1,
            public_inputs: columns,
            envelope_bytes: vec![0xA5],
        };
        let canonical_wrapper =
            norito::encode_canonical(&wrapper).expect("encode canonical STARK wrapper");
        let envelope = iroha_data_model::zk::OpenVerifyEnvelope {
            backend: iroha_data_model::zk::BackendTag::Stark,
            circuit_id: "stark/fri/sha256-goldilocks:confidential-test".to_owned(),
            vk_hash: [0x42; 32],
            public_inputs: b"confidential:test:schema:v1".to_vec(),
            proof_bytes: canonical_wrapper,
            aux: Vec::new(),
        };
        let canonical_outer =
            norito::encode_canonical(&envelope).expect("encode canonical STARK outer envelope");
        assert!(
            super::parse_transfer_public_inputs(&canonical_outer).is_ok(),
            "canonical nested wrapper must expose its public inputs"
        );

        let alternate_flags =
            norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN;
        let alternate_wrapper = {
            let _alternate = norito::core::DecodeFlagsGuard::enter(alternate_flags);
            norito::to_bytes(&wrapper).expect("encode alternate-layout STARK wrapper")
        };
        norito::decode_from_bytes::<iroha_data_model::zk::StarkFriOpenProofV1>(&alternate_wrapper)
            .expect("ordinary Norito accepts the advertised layout");
        let mut alternate_nested = envelope;
        alternate_nested.proof_bytes = alternate_wrapper;
        let alternate_nested = norito::encode_canonical(&alternate_nested)
            .expect("encode canonical outer around alternate STARK wrapper");
        assert!(
            super::parse_transfer_public_inputs(&alternate_nested).is_err(),
            "alternate-layout nested STARK wrapper must be rejected"
        );
    }

    #[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
    fn scalar_bytes(value: u64) -> [u8; 32] {
        use halo2_proofs::halo2curves::{ff::PrimeField as _, pasta::Fp};

        Fp::from(value)
            .to_repr()
            .as_ref()
            .try_into()
            .expect("Pallas scalar representation")
    }

    #[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
    fn dense_confidential_tree_layers_v3_reference(
        commitments: &[[u8; 32]],
        tree_width: usize,
        empty_leaf: super::Scalar,
    ) -> Vec<Vec<super::Scalar>> {
        assert!(tree_width.is_power_of_two());
        assert!(commitments.len() <= tree_width);

        let mut layers = vec![
            (0..tree_width)
                .map(|index| {
                    commitments.get(index).map_or(empty_leaf, |commitment| {
                        super::confidential_commitment_leaf_v3(*commitment, index)
                            .expect("canonical reference commitment")
                    })
                })
                .collect::<Vec<_>>(),
        ];
        while layers.last().expect("dense reference leaf layer").len() > 1 {
            let next = layers
                .last()
                .expect("dense reference layer")
                .chunks_exact(2)
                .map(|pair| super::merkle_parent_v3(pair[0], pair[1]))
                .collect();
            layers.push(next);
        }
        layers
    }

    #[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
    #[test]
    fn canonical_empty_root_constant_matches_poseidon_profile() {
        let computed = super::scalar_to_repr_bytes(
            super::confidential_empty_subtree_roots_v3()[super::CONFIDENTIAL_TREE_DEPTH_V2],
        );
        assert_eq!(
            computed,
            iroha_data_model::zk::CONFIDENTIAL_TREE_POSEIDON_PASTA_V1_EMPTY_ROOT
        );
        assert_eq!(super::poseidon_empty_root_v2(), computed);
        assert_eq!(
            super::compute_confidential_root_v2(&[]).expect("empty profile root"),
            computed
        );
    }

    #[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
    #[test]
    fn incremental_prefix_roots_match_recursive_profile() {
        let commitments = (1_u64..=64).map(scalar_bytes).collect::<Vec<_>>();
        let prefix_roots = super::compute_confidential_prefix_roots_v2(&commitments)
            .expect("canonical prefix roots");
        let empty_roots = super::confidential_empty_subtree_roots_v3();

        for prefix_len in 1..=commitments.len() {
            let recursive = super::confidential_subtree_root_v3(
                &commitments[..prefix_len],
                0,
                super::CONFIDENTIAL_TREE_DEPTH_V2,
                &empty_roots,
            )
            .map(super::scalar_to_repr_bytes)
            .expect("recursive profile root");
            assert_eq!(prefix_roots[prefix_len - 1], recursive);
        }
    }

    #[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
    #[test]
    fn sparse_confidential_subtree_roots_match_dense_reference() {
        let all_commitments = (1_u64..=64).map(scalar_bytes).collect::<Vec<_>>();
        let empty_roots = super::confidential_empty_subtree_roots_v3();

        for len in [0_usize, 1, 2, 3, 7, 16, 37, 64] {
            let commitments = &all_commitments[..len];
            let dense_layers = dense_confidential_tree_layers_v3_reference(
                commitments,
                all_commitments.len(),
                empty_roots[0],
            );
            for height in 0..=6 {
                let width = 1_usize << height;
                for start in (0..64).step_by(width) {
                    let sparse = super::confidential_subtree_root_v3(
                        commitments,
                        start,
                        height,
                        &empty_roots,
                    )
                    .expect("sparse subtree root");
                    let dense = dense_layers[height][start / width];
                    assert_eq!(sparse, dense, "len={len} start={start} height={height}");
                }
            }
        }
    }

    #[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
    #[test]
    fn compact_projection_matches_legacy_paths_and_incremental_frontier() {
        let commitments = (1_u64..=64).map(scalar_bytes).collect::<Vec<_>>();
        let projection = super::ConfidentialTreeProjectionV2::build(&commitments)
            .expect("compact confidential projection");
        let prefix_roots = super::compute_confidential_prefix_roots_v2(&commitments)
            .expect("canonical prefix roots");
        assert_eq!(projection.root(), prefix_roots[commitments.len() - 1]);

        let append = super::append_confidential_tree_frontier_v2(
            0,
            [None; super::CONFIDENTIAL_TREE_DEPTH_V2],
            super::poseidon_empty_root_v2(),
            &commitments,
        )
        .expect("incremental append");
        assert_eq!(
            projection.frontier().expect("projection frontier"),
            append.frontier
        );
        assert_eq!(projection.root(), append.current_root);
        assert_eq!(append.appended_roots, prefix_roots);

        for leaf_index in [0_usize, 1, 2, 31, 63, 64] {
            let projected = projection
                .compute_path(leaf_index)
                .expect("projected authentication path");
            let legacy = super::compute_confidential_merkle_path_v3(&commitments, leaf_index)
                .expect("legacy authentication path");
            assert_eq!(projected.siblings, legacy.siblings);
            assert_eq!(projected.directions, legacy.directions);
            assert_eq!(projected.witness_nodes, legacy.witness_nodes);
            assert_eq!(projected.root, legacy.root);
        }
    }

    #[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
    #[test]
    fn incremental_frontier_preserves_prefix_shape_and_full_tree_transition() {
        let commitments = (1_u64..=3).map(scalar_bytes).collect::<Vec<_>>();
        let expected_roots = super::compute_confidential_prefix_roots_v2(&commitments)
            .expect("canonical prefix roots");
        let mut frontier = [None; super::CONFIDENTIAL_TREE_DEPTH_V2];
        let mut current_root = super::poseidon_empty_root_v2();
        for (index, commitment) in commitments.iter().enumerate() {
            let append = super::append_confidential_tree_frontier_v2(
                index,
                frontier,
                current_root,
                core::slice::from_ref(commitment),
            )
            .expect("single prefix append");
            frontier = append.frontier;
            current_root = append.current_root;
            assert_eq!(append.appended_roots.as_slice(), &[expected_roots[index]]);

            let projection = super::ConfidentialTreeProjectionV2::build(&commitments[..=index])
                .expect("canonical prefix projection");
            assert_eq!(
                frontier,
                projection.frontier().expect("canonical prefix frontier")
            );
            assert_eq!(current_root, projection.root());
            super::validate_confidential_tree_frontier_v2(index + 1, &frontier, current_root)
                .expect("prefix frontier remains self-consistent");
        }

        let full_frontier_scalars: [super::Scalar; super::CONFIDENTIAL_TREE_DEPTH_V2] =
            core::array::from_fn(|level| {
                super::Scalar::from(u64::try_from(level + 1).expect("tree level fits u64"))
            });
        let full_frontier =
            full_frontier_scalars.map(|node| Some(super::scalar_to_repr_bytes(node)));
        let empty_roots = super::confidential_empty_subtree_roots_v3();
        let mut prior_root = empty_roots[0];
        for left in full_frontier_scalars {
            prior_root = super::merkle_parent_v3(left, prior_root);
        }
        let final_commitment = scalar_bytes(0xA5);
        let mut expected_full_root = super::confidential_commitment_leaf_v3(
            final_commitment,
            super::CONFIDENTIAL_TREE_CAPACITY_V2 - 1,
        )
        .expect("canonical final commitment");
        for left in full_frontier_scalars {
            expected_full_root = super::merkle_parent_v3(left, expected_full_root);
        }
        let expected_full_root = super::scalar_to_repr_bytes(expected_full_root);
        let full = super::append_confidential_tree_frontier_v2(
            super::CONFIDENTIAL_TREE_CAPACITY_V2 - 1,
            full_frontier,
            super::scalar_to_repr_bytes(prior_root),
            &[final_commitment],
        )
        .expect("final-capacity append");
        assert!(full.frontier.iter().all(Option::is_none));
        assert_eq!(full.current_root, expected_full_root);
        assert_eq!(full.appended_roots.as_slice(), &[expected_full_root]);
        super::validate_confidential_tree_frontier_v2(
            super::CONFIDENTIAL_TREE_CAPACITY_V2,
            &full.frontier,
            full.current_root,
        )
        .expect("full tree retains its separately persisted root");
    }

    #[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
    #[test]
    fn compact_projection_hashes_each_commitment_once_for_many_paths() {
        let commitments = (1_u64..=128).map(scalar_bytes).collect::<Vec<_>>();
        let expected_root =
            super::compute_confidential_root_v2(&commitments).expect("canonical confidential root");
        super::reset_confidential_commitment_leaf_hash_calls_v3();

        let projection = super::ConfidentialTreeProjectionV2::build(&commitments)
            .expect("compact confidential projection");
        assert_eq!(
            super::confidential_commitment_leaf_hash_calls_v3(),
            commitments.len(),
            "projection construction must hash each commitment exactly once"
        );
        for leaf_index in 0..=commitments.len() {
            projection
                .compute_path(leaf_index)
                .expect("requested or next-zero authentication path");
        }
        assert_eq!(projection.root(), expected_root);
        assert_eq!(
            super::confidential_commitment_leaf_hash_calls_v3(),
            commitments.len(),
            "path count must not cause another commitment scan"
        );
    }

    #[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
    #[test]
    fn incremental_frontier_append_work_depends_only_on_batch_and_depth() {
        let commitments = (1_u64..=128).map(scalar_bytes).collect::<Vec<_>>();
        let expected_roots = super::compute_confidential_prefix_roots_v2(&commitments)
            .expect("canonical prefix roots");
        super::reset_confidential_commitment_leaf_hash_calls_v3();
        super::reset_confidential_frontier_append_parent_hash_calls_v2();

        let append = super::append_confidential_tree_frontier_v2(
            0,
            [None; super::CONFIDENTIAL_TREE_DEPTH_V2],
            super::poseidon_empty_root_v2(),
            &commitments,
        )
        .expect("incremental append");
        assert_eq!(append.appended_roots, expected_roots);
        assert_eq!(
            super::confidential_commitment_leaf_hash_calls_v3(),
            commitments.len()
        );
        assert_eq!(
            super::confidential_frontier_append_parent_hash_calls_v2(),
            commitments.len() * super::CONFIDENTIAL_TREE_DEPTH_V2
        );
        super::validate_confidential_tree_frontier_v2(
            commitments.len(),
            &append.frontier,
            append.current_root,
        )
        .expect("appended frontier remains self-consistent");
    }

    #[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
    #[test]
    fn recursive_step_inactive_relation_padding_is_fully_valid() {
        let padding = super::KagemushaStepSecureWitnessV3::deterministic_padding()
            .expect("fixed Step padding");
        super::secure_relation_v3::validate_topup_witness::<{ super::CONFIDENTIAL_TREE_DEPTH_V2 }>(
            &padding.topup,
        )
        .expect("top-up padding");
        super::secure_relation_v3::validate_transfer_witness::<
            { super::CONFIDENTIAL_TREE_DEPTH_V2 },
        >(&padding.transfer)
        .expect("transfer padding");
        super::secure_relation_v3::validate_unshield_v3_witness::<
            { super::CONFIDENTIAL_TREE_DEPTH_V2 },
        >(&padding.unshield_change)
        .expect("unshield padding");
        assert_eq!(
            padding.transfer.input_0_path.root,
            padding.transfer.input_1_path.root
        );
        assert_eq!(
            padding.unshield_change.input_0_path.root,
            padding.unshield_change.input_1_path.root
        );
    }

    #[test]
    fn production_circuit_selectors_reject_noncanonical_aliases() {
        let selectors: [(&str, fn(&str) -> bool); 4] = [
            (
                super::CONFIDENTIAL_TRANSFER_V2_CIRCUIT_ID,
                super::is_confidential_transfer_v2_circuit_id,
            ),
            (
                super::KAGEMUSHA_TOPUP_SHIELD_V2_CIRCUIT_ID,
                super::is_kagemusha_topup_shield_v2_circuit_id,
            ),
            (
                super::CONFIDENTIAL_UNSHIELD_V2_CIRCUIT_ID,
                super::is_confidential_unshield_v2_circuit_id,
            ),
            (
                super::CONFIDENTIAL_UNSHIELD_V3_CIRCUIT_ID,
                super::is_confidential_unshield_v3_circuit_id,
            ),
        ];
        for (canonical, accepts) in selectors {
            assert!(accepts(canonical));
            assert!(!accepts(&format!(" {canonical}")));
            assert!(!accepts(&format!("{canonical} ")));
            let bare = canonical
                .strip_prefix("halo2/pasta/ipa/")
                .expect("production circuit IDs use the canonical IPA prefix");
            assert!(!accepts(bare));
            assert!(!accepts(&format!("halo2/pasta/{bare}")));
        }
    }

    #[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
    #[test]
    fn retired_single_expression_poseidon_pair_has_constructive_collisions() {
        use halo2_proofs::halo2curves::{
            ff::Field,
            pasta::{Fp, Fq},
        };

        fn fifth_power<F: Field>(value: F) -> F {
            let square = value.square();
            square.square() * value
        }

        fn broken_pair<F>(lhs: F, rhs: F) -> F
        where
            F: Field + From<u64>,
        {
            F::from(2) * fifth_power(lhs + F::from(7)) + F::from(3) * fifth_power(rhs + F::from(13))
        }

        fn assert_constructive_collision<F>(inverse_five: [u64; 4])
        where
            F: Field + From<u64> + PartialEq + core::fmt::Debug,
        {
            let lhs = F::from(5);
            let rhs = F::from(9);
            let replacement_shifted_rhs = F::from(31);
            let shifted_lhs = lhs + F::from(7);
            let shifted_rhs = rhs + F::from(13);
            let half = F::from(2).invert().unwrap();
            let replacement_shifted_lhs_fifth = fifth_power(shifted_lhs)
                + F::from(3)
                    * half
                    * (fifth_power(shifted_rhs) - fifth_power(replacement_shifted_rhs));
            let replacement_shifted_lhs = replacement_shifted_lhs_fifth.pow_vartime(inverse_five);
            let replacement = (
                replacement_shifted_lhs - F::from(7),
                replacement_shifted_rhs - F::from(13),
            );
            assert_ne!((lhs, rhs), replacement);
            assert_eq!(
                broken_pair(lhs, rhs),
                broken_pair(replacement.0, replacement.1)
            );
        }

        assert_constructive_collision::<Fp>([
            0xe0f0_f3f0_cccc_cccd,
            0x4e9e_e0c9_a10a_60e2,
            0x3333_3333_3333_3333,
            0x3333_3333_3333_3333,
        ]);
        assert_constructive_collision::<Fq>([
            0xd69f_2280_cccc_cccd,
            0x4e9e_e0c9_a143_ba4a,
            0x3333_3333_3333_3333,
            0x3333_3333_3333_3333,
        ]);
    }

    #[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
    #[test]
    fn cached_confidential_poseidon_matches_fresh_engine_on_both_pasta_fields() {
        use halo2_proofs::halo2curves::pasta::{Fp, Fq};
        use snark_verifier::{
            loader::native::LOADER,
            util::{arithmetic::FieldExt, hash::Poseidon},
        };

        fn check<F>()
        where
            F: FieldExt + super::ConfidentialPoseidonFieldV3,
        {
            let mut fresh = Poseidon::<
                F,
                F,
                { super::CONFIDENTIAL_POSEIDON_T_V3 },
                { super::CONFIDENTIAL_POSEIDON_RATE_V3 },
            >::new::<
                { super::CONFIDENTIAL_POSEIDON_FULL_ROUNDS_V3 },
                { super::CONFIDENTIAL_POSEIDON_PARTIAL_ROUNDS_V3 },
                { super::CONFIDENTIAL_POSEIDON_SECURE_MDS_V3 },
            >(&*LOADER);
            let uses = [
                (super::CONFIDENTIAL_POSEIDON_OWNER_DOMAIN_V3, &[3, 5][..]),
                (super::CONFIDENTIAL_POSEIDON_NOTE_DOMAIN_V3, &[3, 5, 8, 13]),
                (
                    super::CONFIDENTIAL_POSEIDON_NULLIFIER_DOMAIN_V3,
                    &[3, 5, 8, 13],
                ),
                (super::CONFIDENTIAL_POSEIDON_MERKLE_LEAF_DOMAIN_V3, &[3]),
                (super::CONFIDENTIAL_POSEIDON_MERKLE_NODE_DOMAIN_V3, &[3, 5]),
                (super::CONFIDENTIAL_POSEIDON_ASSET_DOMAIN_V3, &[3]),
                (super::CONFIDENTIAL_POSEIDON_CHAIN_DOMAIN_V3, &[3]),
                (super::CONFIDENTIAL_POSEIDON_PAYER_DOMAIN_V3, &[3]),
                (super::CONFIDENTIAL_POSEIDON_OPERATION_DOMAIN_V3, &[3]),
            ];
            for (domain, input_words) in uses {
                let inputs = input_words.iter().copied().map(F::from).collect::<Vec<_>>();
                let mut preimage = Vec::with_capacity(inputs.len() + 2);
                preimage.push(F::from(domain));
                preimage.push(F::from_u128(inputs.len() as u128));
                preimage.extend_from_slice(&inputs);
                fresh.clear();
                fresh.update(&preimage);
                let expected = fresh.squeeze();
                let cached = super::confidential_poseidon_hash_v3(domain, &inputs);
                assert_eq!(cached, expected, "domain={domain:#018x}");
            }
        }

        check::<Fp>();
        check::<Fq>();
    }

    #[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
    #[test]
    fn secure_confidential_poseidon_host_and_chip_match_all_domains_on_both_pasta_fields() {
        use halo2_base::{gates::circuit::builder::BaseCircuitBuilder, utils::BigPrimeField};
        use halo2_proofs::{
            dev::MockProver,
            halo2curves::pasta::{Fp, Fq},
        };
        use snark_verifier::util::arithmetic::FieldExt;

        fn check<F>()
        where
            F: BigPrimeField + FieldExt + super::ConfidentialPoseidonFieldV3,
        {
            const K: usize = 11;
            let uses = [
                (super::CONFIDENTIAL_POSEIDON_OWNER_DOMAIN_V3, &[3, 5][..]),
                (super::CONFIDENTIAL_POSEIDON_NOTE_DOMAIN_V3, &[3, 5, 8, 13]),
                (
                    super::CONFIDENTIAL_POSEIDON_NULLIFIER_DOMAIN_V3,
                    &[3, 5, 8, 13],
                ),
                (super::CONFIDENTIAL_POSEIDON_MERKLE_LEAF_DOMAIN_V3, &[3]),
                (super::CONFIDENTIAL_POSEIDON_MERKLE_NODE_DOMAIN_V3, &[3, 5]),
                (super::CONFIDENTIAL_POSEIDON_ASSET_DOMAIN_V3, &[3]),
                (super::CONFIDENTIAL_POSEIDON_CHAIN_DOMAIN_V3, &[3]),
                (super::CONFIDENTIAL_POSEIDON_PAYER_DOMAIN_V3, &[3]),
                (super::CONFIDENTIAL_POSEIDON_OPERATION_DOMAIN_V3, &[3]),
            ];
            let expected = uses
                .iter()
                .map(|(domain, inputs)| {
                    let inputs = inputs.iter().copied().map(F::from).collect::<Vec<_>>();
                    super::confidential_poseidon_hash_v3(*domain, &inputs)
                })
                .collect::<Vec<_>>();
            assert!(
                expected
                    .iter()
                    .all(|value| { value.to_repr().as_ref().iter().any(|byte| *byte != 0) })
            );

            let mut builder = BaseCircuitBuilder::new(false)
                .use_k(K)
                .use_lookup_bits(K - 1)
                .use_instance_columns(1);
            let range = builder.range_chip();
            let outputs = {
                let ctx = builder.main(0);
                let chip = super::confidential_relation_gadget::ConfidentialPoseidonChipV3::new(
                    ctx, &range,
                );
                uses.iter()
                    .map(|(domain, inputs)| {
                        let assigned = ctx.assign_witnesses(inputs.iter().copied().map(F::from));
                        chip.hash(ctx, &range, *domain, &assigned)
                    })
                    .collect::<Vec<_>>()
            };
            builder.assigned_instances = vec![outputs];
            builder.calculate_params(Some(9));
            MockProver::run(K as u32, &builder, vec![expected])
                .expect("secure Poseidon mock prover")
                .assert_satisfied();
        }

        check::<Fp>();
        check::<Fq>();
    }

    #[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
    #[test]
    fn secure_confidential_poseidon_kats_pin_both_pasta_fields_and_domains() {
        use halo2_proofs::halo2curves::pasta::{Fp, Fq};

        fn repr<F>(domain: u64) -> [u8; 32]
        where
            F: super::ConfidentialPoseidonFieldV3,
        {
            repr_inputs::<F>(domain, &[3, 5, 8, 13])
        }

        fn repr_inputs<F>(domain: u64, inputs: &[u64]) -> [u8; 32]
        where
            F: super::ConfidentialPoseidonFieldV3,
        {
            let inputs = inputs.iter().copied().map(F::from).collect::<Vec<_>>();
            let value = super::confidential_poseidon_hash_v3(domain, &inputs);
            value
                .to_repr()
                .as_ref()
                .try_into()
                .expect("32-byte Pasta repr")
        }

        fn hex32(value: &str) -> [u8; 32] {
            assert_eq!(value.len(), 64);
            std::array::from_fn(|index| {
                u8::from_str_radix(&value[index * 2..index * 2 + 2], 16)
                    .expect("valid KAT hex byte")
            })
        }

        let vectors = [
            (
                super::CONFIDENTIAL_POSEIDON_OWNER_DOMAIN_V3,
                [
                    0xce, 0x9c, 0x57, 0xdb, 0x56, 0x29, 0x51, 0xd1, 0xdd, 0x72, 0xe8, 0x34, 0xbf,
                    0xac, 0xcc, 0x74, 0xa9, 0xe2, 0x5f, 0x5c, 0xa2, 0xc1, 0xcd, 0x7d, 0xa1, 0xec,
                    0x5c, 0x3c, 0xaf, 0x45, 0x45, 0x3d,
                ],
                [
                    0x83, 0x82, 0xed, 0x00, 0xbb, 0x4b, 0xcb, 0xf7, 0x7d, 0x0c, 0x9b, 0xcc, 0x8e,
                    0xf1, 0x22, 0xac, 0x6f, 0x67, 0xa8, 0x8f, 0x68, 0xce, 0x46, 0x51, 0xce, 0x23,
                    0x7b, 0x67, 0x33, 0x4a, 0x65, 0x30,
                ],
            ),
            (
                super::CONFIDENTIAL_POSEIDON_NOTE_DOMAIN_V3,
                [
                    0xcd, 0xb8, 0x44, 0xf8, 0xa4, 0x78, 0xeb, 0xf3, 0x14, 0x54, 0x6c, 0xc9, 0xa8,
                    0x14, 0x5b, 0xbc, 0xa0, 0x5b, 0x42, 0x21, 0xa3, 0x1a, 0x9c, 0xee, 0x2a, 0x34,
                    0xa6, 0xb2, 0xd8, 0x98, 0x86, 0x2c,
                ],
                [
                    0x22, 0x2f, 0xe8, 0xdf, 0xb1, 0x1b, 0x68, 0xb9, 0x38, 0x47, 0xd2, 0x86, 0x94,
                    0xdb, 0x28, 0xc5, 0x63, 0x6c, 0x5b, 0xbf, 0x78, 0xa7, 0xb7, 0xdb, 0x73, 0xc6,
                    0x2b, 0x3e, 0x38, 0x9a, 0xc0, 0x2d,
                ],
            ),
            (
                super::CONFIDENTIAL_POSEIDON_NULLIFIER_DOMAIN_V3,
                [
                    0x00, 0x76, 0x08, 0x32, 0xfe, 0x2d, 0x8d, 0x60, 0x37, 0x3d, 0x15, 0xeb, 0x76,
                    0x43, 0x6a, 0x21, 0x6d, 0xec, 0x7d, 0xef, 0xaa, 0xf1, 0xda, 0x69, 0xd5, 0x23,
                    0x3c, 0xce, 0x5c, 0x98, 0xab, 0x06,
                ],
                [
                    0xb4, 0x6a, 0x51, 0x8a, 0x68, 0x0c, 0xdf, 0x75, 0x06, 0x9e, 0x35, 0x78, 0x4d,
                    0x7f, 0xd5, 0x80, 0x3c, 0x8d, 0xbf, 0xc1, 0xa3, 0xb8, 0x66, 0xc1, 0xff, 0xd0,
                    0x3a, 0x2b, 0x35, 0xdf, 0x0d, 0x00,
                ],
            ),
            (
                super::CONFIDENTIAL_POSEIDON_MERKLE_LEAF_DOMAIN_V3,
                [
                    0x66, 0x12, 0x9a, 0x24, 0xba, 0x49, 0x66, 0xae, 0xd5, 0xe6, 0xf5, 0x69, 0x56,
                    0xe8, 0x09, 0x16, 0xd5, 0x07, 0xcf, 0x6a, 0x68, 0xa6, 0xe2, 0x61, 0xb9, 0x2d,
                    0x0a, 0x9f, 0x9d, 0x13, 0x9c, 0x33,
                ],
                [
                    0x34, 0x22, 0xab, 0xe3, 0x43, 0x31, 0x71, 0x93, 0x0e, 0xb6, 0x7c, 0xa9, 0xb4,
                    0xe0, 0x5a, 0xdf, 0x27, 0xf8, 0x23, 0x62, 0xed, 0xe7, 0x8c, 0x8a, 0x65, 0x5e,
                    0x2e, 0x79, 0x85, 0xc0, 0xc5, 0x38,
                ],
            ),
            (
                super::CONFIDENTIAL_POSEIDON_MERKLE_NODE_DOMAIN_V3,
                [
                    0xe6, 0x44, 0x99, 0x62, 0xdd, 0xc1, 0xd2, 0x3d, 0x9d, 0x62, 0x94, 0x57, 0x72,
                    0xb9, 0x68, 0x8c, 0xea, 0x4e, 0x03, 0x82, 0x4f, 0x3c, 0xaf, 0x77, 0x3f, 0x3a,
                    0x74, 0x10, 0x4d, 0x4b, 0xb2, 0x34,
                ],
                [
                    0x1e, 0x00, 0xc2, 0xeb, 0xab, 0x3d, 0x5c, 0x05, 0x74, 0xcb, 0xc7, 0xf6, 0x47,
                    0xb5, 0xfe, 0xb4, 0xc4, 0xff, 0x27, 0x1b, 0xd8, 0x4f, 0xb7, 0x7b, 0xbb, 0x0c,
                    0xc0, 0xf3, 0xda, 0x60, 0x70, 0x39,
                ],
            ),
        ];
        for (domain, fp, fq) in vectors {
            assert_eq!(repr::<Fp>(domain), fp);
            assert_eq!(repr::<Fq>(domain), fq);
        }
        for (domain, inputs, fp, fq) in [
            (
                super::CONFIDENTIAL_POSEIDON_OWNER_DOMAIN_V3,
                &[3, 5][..],
                "612ad09a40970302036fef4c16385a98a7b337143c086d7ec4c0f9fc4792610d",
                "da41767db79387f7bfb20625144da612661c38f7ea94dc3a62f330e9ddbbef10",
            ),
            (
                super::CONFIDENTIAL_POSEIDON_NOTE_DOMAIN_V3,
                &[3, 5, 8, 13],
                "cdb844f8a478ebf314546cc9a8145bbca05b4221a31a9cee2a34a6b2d898862c",
                "222fe8dfb11b68b93847d28694db28c5636c5bbf78a7b7db73c62b3e389ac02d",
            ),
            (
                super::CONFIDENTIAL_POSEIDON_NULLIFIER_DOMAIN_V3,
                &[3, 5, 8, 13],
                "00760832fe2d8d60373d15eb76436a216dec7defaaf1da69d5233cce5c98ab06",
                "b46a518a680cdf75069e35784d7fd5803c8dbfc1a3b866c1ffd03a2b35df0d00",
            ),
            (
                super::CONFIDENTIAL_POSEIDON_MERKLE_LEAF_DOMAIN_V3,
                &[3],
                "75b309c05d81f516d4ceadaca9640d240c24f365453f476db07b4d8e3c943713",
                "a447fb1114387ca98a59cdc3bbc721bdcf6a74b0cfe9ad7ae45125f07538a532",
            ),
            (
                super::CONFIDENTIAL_POSEIDON_MERKLE_NODE_DOMAIN_V3,
                &[3, 5],
                "22a66785c01757e9f8b6c401f5e1f08f6649cc52a0083bb452af4378d15b2228",
                "3f39495312f7cdfe4af7346fc00f674709cca1fce1686e2881c708ff5034842a",
            ),
            (
                super::CONFIDENTIAL_POSEIDON_ASSET_DOMAIN_V3,
                &[3],
                "e12530abfe9e4f7c1f95d510191b65c89546e4d9b8e9ed79d3e3521772f02930",
                "45591fdcac6208fef59f1955ef819d2296dab0aeba1023a3813ccf2d4e52eb03",
            ),
            (
                super::CONFIDENTIAL_POSEIDON_CHAIN_DOMAIN_V3,
                &[3],
                "fca84dd79474290906d03758d1c9dd2ab58a8a97117c2265eb9dccca8652801f",
                "870b2059b229ac2c6039448efe1fb1ee2b84eab4a3a471f71c87d4c221f4902b",
            ),
            (
                super::CONFIDENTIAL_POSEIDON_PAYER_DOMAIN_V3,
                &[3],
                "9cbed996e9fa7df2defec498c6a0b03c230ac514bb36b02cdf6c0566dee6f120",
                "d4b100e87bdadbe867edd65ad713c0021856edfd117637be7b520392bb654a3a",
            ),
            (
                super::CONFIDENTIAL_POSEIDON_OPERATION_DOMAIN_V3,
                &[3],
                "de6686c1d1e59eecf8b522355c36624ea6d2ceeec8cd8607dadbbcc13ac08812",
                "b1697fa1593829176a2b72416bebdec305cf036b621480bc2d5ba74d1d339a03",
            ),
        ] {
            assert_eq!(repr_inputs::<Fp>(domain, inputs), hex32(fp));
            assert_eq!(repr_inputs::<Fq>(domain, inputs), hex32(fq));
        }
    }

    #[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
    #[test]
    fn confidential_v3_native_derivations_are_domain_separated_and_fail_closed() {
        use std::collections::BTreeSet;

        let asset =
            super::derive_confidential_asset_tag_v3("rose#wonderland").expect("V3 asset tag");
        let chain = super::derive_confidential_chain_tag_v3("00000000-0000-0000-0000-000000000001")
            .expect("V3 chain tag");
        let payer = super::derive_kagemusha_topup_payer_tag_v3("alice").expect("V3 payer tag");
        let operation =
            super::derive_kagemusha_topup_operation_tag_v3(&[7; 32]).expect("V3 operation tag");
        assert_eq!(
            BTreeSet::from([asset, chain, payer, operation]).len(),
            4,
            "distinct use domains must not alias the same preimage"
        );

        let spend_key = [11; 32];
        let diversifier = super::scalar_to_repr_bytes(super::Scalar::from(13));
        let owner =
            super::derive_confidential_owner_tag_v3_with_diversifier(&spend_key, diversifier)
                .expect("V3 owner");
        let rho = [17; 32];
        let note = super::derive_confidential_note_v3(asset, 19, rho, owner).expect("V3 note");
        let nullifier = super::derive_confidential_nullifier_v3(&spend_key, rho, asset, chain)
            .expect("V3 nullifier");
        assert_ne!(note, nullifier);
        assert!(
            super::derive_confidential_owner_tag_v3_with_diversifier(&[0; 32], diversifier)
                .is_err()
        );
        assert!(
            super::derive_confidential_owner_tag_v3_with_diversifier(&spend_key, [0xff; 32])
                .is_err()
        );
        assert!(super::derive_confidential_asset_tag_v3("  ").is_err());
        assert!(super::derive_confidential_asset_tag_v3(" rose#wonderland").is_err());
        assert!(
            super::derive_confidential_chain_tag_v3("00000000-0000-0000-0000-000000000001 ")
                .is_err()
        );
        assert!(super::derive_kagemusha_topup_payer_tag_v3("alice ").is_err());
        assert!(super::derive_kagemusha_topup_operation_tag_v3(&[0; 32]).is_err());
        assert!(super::derive_confidential_note_v3(asset, 0, rho, owner).is_err());
        assert!(
            super::derive_confidential_nullifier_v3(&spend_key, [0; 32], asset, chain).is_err()
        );
    }

    #[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
    #[test]
    fn generated_confidential_v2_vk_records_parse_as_matching_circuits() {
        let transfer = super::confidential_transfer_v2_vk_record("vk_transfer", 3)
            .expect("transfer vk record");
        let unshield = super::confidential_unshield_v2_vk_record("vk_unshield", 4)
            .expect("unshield vk record");
        let unshield_v3 = super::confidential_unshield_v3_vk_record("vk_unshield_v3", 5)
            .expect("unshield v3 vk record");

        assert_eq!(
            transfer.circuit_id,
            super::CONFIDENTIAL_TRANSFER_V2_CIRCUIT_ID
        );
        assert_eq!(
            unshield.circuit_id,
            super::CONFIDENTIAL_UNSHIELD_V2_CIRCUIT_ID
        );
        assert_eq!(
            unshield_v3.circuit_id,
            super::CONFIDENTIAL_UNSHIELD_V3_CIRCUIT_ID
        );
        assert!(transfer.is_active());
        assert!(unshield.is_active());
        assert!(unshield_v3.is_active());
        assert!(transfer.max_proof_bytes > 0);
        assert!(unshield.max_proof_bytes > 0);
        assert!(unshield_v3.max_proof_bytes > 0);

        let transfer_key = transfer.key.as_ref().expect("transfer key");
        let unshield_key = unshield.key.as_ref().expect("unshield key");
        let unshield_v3_key = unshield_v3.key.as_ref().expect("unshield v3 key");
        super::parse_vk_for_transfer(&transfer.circuit_id, transfer_key)
            .expect("transfer key must parse as confidential transfer v2");
        super::parse_vk_for_unshield_v2(&unshield.circuit_id, unshield_key)
            .expect("unshield key must parse as confidential unshield v2");
        super::parse_vk_for_unshield_v3(&unshield_v3.circuit_id, unshield_v3_key)
            .expect("unshield v3 key must parse as confidential unshield v3");
    }

    #[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
    #[test]
    fn supplied_confidential_merkle_path_recomputes_witness_nodes() {
        let commitments = vec![[0x11; 32], [0x22; 32], [0x33; 32]];
        let path =
            super::compute_confidential_merkle_path_v2(&commitments, 2).expect("computed path");
        let mut supplied = path.clone();
        supplied.witness_nodes.clear();

        let normalized = super::normalize_supplied_confidential_merkle_path_v2(
            [0x33; 32],
            Some(2),
            &supplied,
            path.root,
            "test path",
        )
        .expect("supplied path should validate");

        assert_eq!(normalized.root, path.root);
        assert_eq!(normalized.witness_nodes, path.witness_nodes);

        let mut tampered = supplied;
        tampered.directions[0] ^= 1;
        assert!(
            super::normalize_supplied_confidential_merkle_path_v2(
                [0x33; 32],
                Some(2),
                &tampered,
                path.root,
                "test path",
            )
            .is_err()
        );
    }

    #[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
    #[test]
    fn next_zero_confidential_path_matches_padded_tree_path() {
        for len in 1usize..12 {
            let commitments: Vec<[u8; 32]> = (0..len)
                .map(|index| {
                    let mut commitment = [0u8; 32];
                    commitment[0] = 0x40;
                    commitment[31] = u8::try_from(index + 1).expect("fixture index fits in u8");
                    commitment
                })
                .collect();
            let previous_index = commitments.len() - 1;
            let previous_path =
                super::compute_confidential_merkle_path_v2(&commitments, previous_index)
                    .expect("previous path");
            let expected_next_zero =
                super::compute_confidential_merkle_path_v2(&commitments, commitments.len())
                    .expect("expected zero path");
            let derived = super::derive_confidential_next_zero_path_v2(
                commitments[previous_index],
                previous_index,
                &previous_path,
                previous_path.root,
            )
            .expect("derived next zero path");

            assert_eq!(derived.root, expected_next_zero.root, "len={len}");
            assert_eq!(
                derived.siblings, expected_next_zero.siblings,
                "siblings len={len}"
            );
            assert_eq!(
                derived.directions, expected_next_zero.directions,
                "directions len={len}"
            );
            assert_eq!(
                derived.witness_nodes, expected_next_zero.witness_nodes,
                "witness nodes len={len}"
            );
        }
    }

    #[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
    #[test]
    fn sequential_append_paths_match_complete_tree_recomputation() {
        for initial_len in 0usize..10 {
            let initial: Vec<[u8; 32]> = (0..initial_len)
                .map(|index| scalar_bytes(u64::try_from(800 + index).expect("fixture fits")))
                .collect();
            for output_count in 1usize..=2 {
                let outputs: Vec<[u8; 32]> = (0..output_count)
                    .map(|index| scalar_bytes(u64::try_from(900 + index).expect("fixture fits")))
                    .collect();
                let initial_frontier =
                    super::compute_confidential_merkle_path_v3(&initial, initial.len())
                        .expect("initial next-zero frontier");
                let derived = super::derive_confidential_sequential_append_paths_v3(
                    initial.len(),
                    &initial_frontier,
                    &outputs,
                )
                .expect("sequential append paths");
                let mut final_commitments = initial.clone();
                final_commitments.extend_from_slice(&outputs);
                let expected_final_root =
                    super::compute_confidential_root_v3(&final_commitments).expect("final root");
                assert_eq!(derived.initial_root, initial_frontier.root);
                assert_eq!(derived.final_root, expected_final_root);
                assert_eq!(derived.leaves.len(), output_count);
                for (offset, leaf) in derived.leaves.iter().enumerate() {
                    let mut before = initial.clone();
                    before.extend_from_slice(&outputs[..offset]);
                    let expected_update =
                        super::compute_confidential_merkle_path_v3(&before, initial.len() + offset)
                            .expect("expected update path");
                    let expected_membership = super::compute_confidential_merkle_path_v3(
                        &final_commitments,
                        initial.len() + offset,
                    )
                    .expect("expected final membership path");
                    assert_eq!(leaf.leaf_index, initial.len() + offset);
                    assert_eq!(leaf.update_path.root, expected_update.root);
                    assert_eq!(leaf.update_path.siblings, expected_update.siblings);
                    assert_eq!(leaf.update_path.directions, expected_update.directions);
                    assert_eq!(leaf.membership_path.root, expected_membership.root);
                    assert_eq!(leaf.membership_path.siblings, expected_membership.siblings);
                    assert_eq!(
                        leaf.membership_path.directions,
                        expected_membership.directions
                    );
                }
                let expected_frontier = super::compute_confidential_merkle_path_v3(
                    &final_commitments,
                    final_commitments.len(),
                )
                .expect("expected final frontier");
                assert_eq!(derived.next_zero_leaf_index, final_commitments.len());
                assert_eq!(derived.next_zero_path.root, expected_frontier.root);
                assert_eq!(derived.next_zero_path.siblings, expected_frontier.siblings);
                assert_eq!(
                    derived.next_zero_path.directions,
                    expected_frontier.directions
                );
            }
        }
    }

    #[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
    #[test]
    fn sequential_append_paths_reject_tamper_and_invalid_cardinality() {
        let commitments = vec![scalar_bytes(1001), scalar_bytes(1002)];
        let frontier = super::compute_confidential_merkle_path_v3(&commitments, commitments.len())
            .expect("frontier");
        let output = scalar_bytes(1003);

        let mut wrong_root = frontier.clone();
        wrong_root.root[0] ^= 1;
        assert!(
            super::derive_confidential_sequential_append_paths_v3(
                commitments.len(),
                &wrong_root,
                &[output],
            )
            .is_err()
        );

        let mut wrong_direction = frontier.clone();
        wrong_direction.directions[0] ^= 1;
        assert!(
            super::derive_confidential_sequential_append_paths_v3(
                commitments.len(),
                &wrong_direction,
                &[output],
            )
            .is_err()
        );
        assert!(
            super::derive_confidential_sequential_append_paths_v3(
                commitments.len(),
                &frontier,
                &[],
            )
            .is_err()
        );
        assert!(
            super::derive_confidential_sequential_append_paths_v3(
                commitments.len(),
                &frontier,
                &[output, scalar_bytes(1004), scalar_bytes(1005)],
            )
            .is_err()
        );
        assert!(
            super::derive_confidential_sequential_append_paths_v3(
                commitments.len(),
                &frontier,
                &[[0; 32]],
            )
            .is_err()
        );
    }

    #[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
    #[test]
    fn canonical_kagemusha_vk_digests_match_reviewed_goldens() {
        let topup = super::kagemusha_topup_shield_v2_vk_box().expect("canonical top-up vk");
        let full = super::confidential_unshield_v2_vk_box().expect("canonical full-unshield vk");
        let change =
            super::confidential_unshield_v3_vk_box().expect("canonical change-unshield vk");
        assert_eq!(
            [
                hex::encode(crate::zk::hash_vk(&topup)),
                hex::encode(crate::zk::hash_vk(&full)),
                hex::encode(crate::zk::hash_vk(&change)),
            ],
            [
                hex::encode(super::KAGEMUSHA_TOPUP_SHIELD_V2_VK_DIGEST_V1),
                hex::encode(super::CONFIDENTIAL_UNSHIELD_V2_VK_DIGEST_V1),
                hex::encode(super::CONFIDENTIAL_UNSHIELD_V3_VK_DIGEST_V1),
            ],
            "canonical verifier-key layout changed; review the circuit/schema version before updating these goldens",
        );
    }

    #[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
    #[test]
    fn kagemusha_topup_canonical_vk_guard_rejects_one_byte_substitution() {
        let canonical =
            super::kagemusha_topup_shield_v2_vk_box().expect("canonical top-up verifier key");
        super::ensure_kagemusha_topup_shield_v2_canonical_vk_box(&canonical)
            .expect("reviewed top-up verifier key");
        let mut mutated = canonical;
        *mutated
            .bytes
            .last_mut()
            .expect("non-empty canonical top-up verifier key") ^= 1;
        assert!(
            super::ensure_kagemusha_topup_shield_v2_canonical_vk_box(&mutated).is_err(),
            "one-byte same-circuit verifier substitution must fail closed",
        );
    }

    #[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
    #[test]
    fn confidential_transfer_v2_canonical_vk_guard_rejects_self_consistent_key_substitution() {
        use iroha_data_model::proof::VerifyingKeyBox;

        let canonical = super::confidential_transfer_v2_vk_box().expect("canonical transfer vk");
        let cached = super::confidential_transfer_v2_vk_box().expect("cached transfer vk");
        assert_eq!(
            canonical, cached,
            "confidential transfer v2 verifier key generation should be cached and deterministic"
        );
        super::ensure_confidential_transfer_v2_canonical_vk_box(&canonical)
            .expect("canonical transfer verifier key should pass");
        let proving_key = super::cached_confidential_transfer_v2_proving_key()
            .expect("canonical transfer proving key");
        let cached_proving_key = super::cached_confidential_transfer_v2_proving_key()
            .expect("cached transfer proving key");
        assert!(
            std::ptr::eq(proving_key, cached_proving_key),
            "confidential transfer v2 proving key generation should be cached"
        );

        let mut mutated = canonical.clone();
        let last = mutated
            .bytes
            .last_mut()
            .expect("canonical transfer verifier key bytes");
        *last ^= 0x01;
        let err = super::ensure_confidential_transfer_v2_canonical_vk_box(&mutated)
            .expect_err("mutated self-consistent verifier key must reject");
        assert!(
            err.contains("canonical semantic circuit key"),
            "unexpected mutated-key error: {err}"
        );

        let wrong_backend =
            VerifyingKeyBox::new("halo2/ipa:kzg".to_owned(), canonical.bytes.clone());
        let err = super::ensure_confidential_transfer_v2_canonical_vk_box(&wrong_backend)
            .expect_err("wrong backend must reject before canonical bytes are considered");
        assert!(err.contains("backend"), "unexpected backend error: {err}");

        let empty = VerifyingKeyBox::new(crate::zk::ZK_BACKEND_HALO2_IPA.to_owned(), Vec::new());
        let err = super::ensure_confidential_transfer_v2_canonical_vk_box(&empty)
            .expect_err("empty verifier key must reject");
        assert!(
            err.contains("non-empty"),
            "unexpected empty-key error: {err}"
        );
    }

    #[cfg(not(any(feature = "zk-halo2", feature = "zk-halo2-ipa")))]
    #[test]
    fn canonical_vk_guards_fail_closed_without_halo2_ipa() {
        use iroha_data_model::proof::VerifyingKeyBox;

        let opaque =
            VerifyingKeyBox::new(crate::zk::ZK_BACKEND_HALO2_IPA.to_owned(), vec![0xA5; 32]);
        for result in [
            super::ensure_kagemusha_topup_shield_v2_canonical_vk_box(&opaque),
            super::ensure_confidential_transfer_v2_canonical_vk_box(&opaque),
            super::ensure_confidential_unshield_v2_canonical_vk_box(&opaque),
            super::ensure_confidential_unshield_v3_canonical_vk_box(&opaque),
        ] {
            let err = result.expect_err(
                "a build without Halo2/IPA cannot establish canonical verifier-key equality",
            );
            assert!(
                err.contains("requires the Halo2/IPA backend"),
                "unexpected fail-closed error: {err}"
            );
        }
    }

    #[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
    #[test]
    fn confidential_transfer_v2_canonical_vk_guard_rejects_malformed_key_preflight() {
        use iroha_data_model::proof::VerifyingKeyBox;

        let malformed =
            VerifyingKeyBox::new(crate::zk::ZK_BACKEND_HALO2_IPA.to_owned(), vec![0xC9; 32]);
        let err = super::ensure_confidential_transfer_v2_canonical_vk_box(&malformed)
            .expect_err("malformed verifier key must reject before canonical key generation");
        assert!(
            err.contains("invalid CID1/Halo2 IPA verifier-key envelope"),
            "unexpected malformed-key error: {err}"
        );
    }

    #[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
    #[test]
    fn confidential_unshield_v2_v3_canonical_caches_reject_key_substitution() {
        use iroha_data_model::proof::VerifyingKeyBox;

        let v2 = super::confidential_unshield_v2_vk_box().expect("canonical unshield v2 vk");
        let v2_cached = super::confidential_unshield_v2_vk_box().expect("cached unshield v2 vk");
        assert_eq!(v2, v2_cached);
        super::ensure_confidential_unshield_v2_canonical_vk_box(&v2)
            .expect("canonical unshield v2 verifier key should pass");

        let v3 = super::confidential_unshield_v3_vk_box().expect("canonical unshield v3 vk");
        let v3_cached = super::confidential_unshield_v3_vk_box().expect("cached unshield v3 vk");
        assert_eq!(v3, v3_cached);
        super::ensure_confidential_unshield_v3_canonical_vk_box(&v3)
            .expect("canonical unshield v3 verifier key should pass");

        let v2_pk = super::cached_confidential_unshield_v2_proving_key()
            .expect("canonical unshield v2 proving key");
        let v2_pk_cached = super::cached_confidential_unshield_v2_proving_key()
            .expect("cached unshield v2 proving key");
        assert!(
            std::ptr::eq(v2_pk, v2_pk_cached),
            "unshield v2 proving key should come from a process-local cache"
        );

        let v3_pk = super::cached_confidential_unshield_v3_proving_key()
            .expect("canonical unshield v3 proving key");
        let v3_pk_cached = super::cached_confidential_unshield_v3_proving_key()
            .expect("cached unshield v3 proving key");
        assert!(
            std::ptr::eq(v3_pk, v3_pk_cached),
            "unshield v3 proving key should come from a process-local cache"
        );

        fn assert_rejects_key_substitution(
            label: &str,
            canonical: &VerifyingKeyBox,
            ensure: fn(&VerifyingKeyBox) -> Result<(), String>,
        ) {
            let mut mutated = canonical.clone();
            *mutated
                .bytes
                .last_mut()
                .expect("canonical unshield verifier key bytes") ^= 0x01;
            let err = match ensure(&mutated) {
                Ok(()) => panic!("{label} mutated verifier key must reject"),
                Err(err) => err,
            };
            assert!(
                err.contains("canonical semantic circuit key"),
                "unexpected {label} mutated-key error: {err}"
            );

            let wrong_backend =
                VerifyingKeyBox::new("halo2/ipa:kzg".to_owned(), canonical.bytes.clone());
            let err = match ensure(&wrong_backend) {
                Ok(()) => panic!("{label} wrong backend must reject"),
                Err(err) => err,
            };
            assert!(
                err.contains("backend"),
                "unexpected {label} backend error: {err}"
            );
        }
        assert_rejects_key_substitution(
            "unshield v2",
            &v2,
            super::ensure_confidential_unshield_v2_canonical_vk_box,
        );
        assert_rejects_key_substitution(
            "unshield v3",
            &v3,
            super::ensure_confidential_unshield_v3_canonical_vk_box,
        );

        let err = super::ensure_confidential_unshield_v3_canonical_vk_box(&v2)
            .expect_err("unshield v2 key must not satisfy unshield v3 canonical guard");
        assert!(
            err.contains("CID1"),
            "unexpected v2-as-v3 canonical-guard error: {err}"
        );
        let err = super::ensure_confidential_unshield_v2_canonical_vk_box(&v3)
            .expect_err("unshield v3 key must not satisfy unshield v2 canonical guard");
        assert!(
            err.contains("CID1"),
            "unexpected v3-as-v2 canonical-guard error: {err}"
        );
    }

    #[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
    #[test]
    fn generated_confidential_transfer_v2_one_input_one_output_verifies_against_generated_vk() {
        use halo2_proofs::halo2curves::{ff::Field as _, pasta::Fp};
        use iroha_data_model::ChainId;

        let chain_id: ChainId = "fc56984b-2be7-431d-840e-21514d1883f0"
            .parse()
            .expect("valid chain id");
        let asset_definition_id = "xor#universal";
        let spend_key = [0x11_u8; 32];
        let input_rho = [0x22_u8; 32];
        let input_diversifier = super::derive_confidential_diversifier_v2(b"input");
        let input_owner_tag =
            super::derive_confidential_owner_tag_v2_with_diversifier(&spend_key, input_diversifier)
                .expect("input owner tag");
        let input_commitment =
            super::derive_confidential_note_v2(asset_definition_id, 7, input_rho, input_owner_tag)
                .expect("input commitment");
        let tree_commitments = vec![input_commitment];
        let root = super::compute_confidential_root_v2(&tree_commitments).expect("root");

        let recipient_key = [0x33_u8; 32];
        let output_rho = [0x44_u8; 32];
        let output_diversifier = super::derive_confidential_diversifier_v2(b"recipient");
        let output_owner_tag = super::derive_confidential_owner_tag_v2_with_diversifier(
            &recipient_key,
            output_diversifier,
        )
        .expect("output owner tag");
        let transfer_vk =
            super::confidential_transfer_v2_vk_record("vk_transfer", 3).expect("transfer vk");
        let transfer_key = transfer_vk.key.as_ref().expect("inline transfer vk");
        let input_path =
            super::compute_confidential_merkle_path_v2(&tree_commitments, 0).expect("input path");
        let empty_path =
            super::compute_confidential_merkle_path_v2(&tree_commitments, tree_commitments.len())
                .expect("empty input path");
        let output_commitment = super::derive_confidential_note_v2(
            asset_definition_id,
            7,
            output_rho,
            output_owner_tag,
        )
        .expect("output commitment");
        let asset_tag = super::derive_confidential_asset_tag_v2(asset_definition_id);
        let chain_tag = super::derive_confidential_chain_tag_v2(chain_id.as_str());
        let nullifier = super::derive_confidential_nullifier_v2(
            chain_id.as_str(),
            asset_definition_id,
            &spend_key,
            input_rho,
        );
        let witness = super::ConfidentialTransferWitnessV2 {
            include_input_1: false,
            include_output_1: false,
            input_0_amount: 7,
            input_1_amount: 0,
            output_0_amount: 7,
            output_1_amount: 0,
            input_0_rho: input_rho,
            input_1_rho: [0u8; 32],
            output_0_rho: output_rho,
            output_1_rho: [0u8; 32],
            spend_scalar: super::scalar_to_repr_bytes(super::hash_to_scalar(
                b"iroha.confidential.v3.spend_scalar",
                &[&spend_key],
            )),
            input_0_diversifier: input_diversifier,
            input_1_diversifier: [0u8; 32],
            output_0_owner_tag: output_owner_tag,
            output_1_owner_tag: [0u8; 32],
            asset_tag,
            chain_tag,
            input_0_path: input_path,
            input_1_path: empty_path,
        };
        let circuit = super::secure_relation_v3::ConfidentialTransferCircuitV3::<
            { super::CONFIDENTIAL_TREE_DEPTH_V2 },
        > {
            witness: Some(witness),
        };
        let instance_columns = vec![
            vec![super::scalar_from_repr(input_commitment).expect("input commitment")],
            vec![Fp::ZERO],
            vec![super::scalar_from_repr(nullifier).expect("nullifier")],
            vec![Fp::ZERO],
            vec![super::scalar_from_repr(output_commitment).expect("output commitment")],
            vec![Fp::ZERO],
            vec![super::scalar_from_repr(root).expect("root")],
            vec![super::scalar_from_repr(asset_tag).expect("asset tag")],
            vec![super::scalar_from_repr(chain_tag).expect("chain tag")],
        ];
        halo2_proofs::dev::MockProver::run(
            super::CONFIDENTIAL_TRANSFER_V2_IPA_K,
            &circuit,
            instance_columns,
        )
        .expect("mock prover")
        .assert_satisfied();

        let proof = super::build_confidential_transfer_proof_v2(
            &chain_id,
            asset_definition_id,
            &spend_key,
            &tree_commitments,
            &[super::ConfidentialTransferInputV2 {
                amount: 7,
                rho: input_rho,
                diversifier: input_diversifier,
                leaf_index: 0,
            }],
            &[super::ConfidentialTransferOutputV2 {
                amount: 7,
                rho: output_rho,
                owner_tag: output_owner_tag,
            }],
            root,
            &transfer_vk.circuit_id,
            transfer_key,
        )
        .expect("transfer proof");

        assert!(
            crate::zk::verify_backend(
                crate::zk::ZK_BACKEND_HALO2_IPA,
                &proof.proof,
                Some(transfer_key),
            ),
            "generated one-input one-output confidential transfer v2 proof should verify against the generated VK"
        );

        let wrong_cid_key = super::build_confidential_v2_vk_box(
            super::CONFIDENTIAL_TRANSFER_V2_IPA_K,
            super::CONFIDENTIAL_UNSHIELD_V2_CIRCUIT_ID,
            &super::secure_relation_v3::ConfidentialTransferCircuitV3::<
                { super::CONFIDENTIAL_TREE_DEPTH_V2 },
            >::default(),
        )
        .expect("transfer-shaped verifier key with wrong CID1");
        assert_ne!(
            crate::zk::hash_vk(transfer_key),
            crate::zk::hash_vk(&wrong_cid_key)
        );
        let wrong_cid_proof = super::build_confidential_transfer_proof_v2(
            &chain_id,
            asset_definition_id,
            &spend_key,
            &tree_commitments,
            &[super::ConfidentialTransferInputV2 {
                amount: 7,
                rho: input_rho,
                diversifier: input_diversifier,
                leaf_index: 0,
            }],
            &[super::ConfidentialTransferOutputV2 {
                amount: 7,
                rho: output_rho,
                owner_tag: output_owner_tag,
            }],
            root,
            &transfer_vk.circuit_id,
            &wrong_cid_key,
        )
        .expect("transfer proof with wrong-CID verifier key");
        assert!(
            !crate::zk::verify_backend(
                crate::zk::ZK_BACKEND_HALO2_IPA,
                &wrong_cid_proof.proof,
                Some(&wrong_cid_key),
            ),
            "verifier must reject a cryptographically valid proof whose VK CID1 names another circuit"
        );
    }

    #[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
    #[test]
    fn generated_confidential_transfer_v2_proof_verifies_against_generated_vk() {
        let chain_id = iroha_data_model::ChainId::from("confidential-transfer-v2-test");
        let asset_definition_id = "zcoin#wonderland";
        let spend_key = [0x11_u8; 32];
        let input_0_rho = [0x21_u8; 32];
        let input_1_rho = [0x22_u8; 32];
        let output_0_rho = [0x31_u8; 32];
        let output_1_rho = [0x32_u8; 32];
        let input_0_diversifier = super::default_confidential_diversifier_v2();
        let input_1_diversifier = super::derive_confidential_diversifier_v2(b"input-1");
        let output_0_owner_tag = super::derive_confidential_owner_tag_v2_with_diversifier(
            &spend_key,
            input_0_diversifier,
        )
        .expect("owner tag");
        let recipient_diversifier = super::derive_confidential_diversifier_v2(b"recipient");
        let output_1_owner_tag = super::derive_confidential_owner_tag_v2_with_diversifier(
            &[0x44_u8; 32],
            recipient_diversifier,
        )
        .expect("recipient owner tag");

        let input_0_commitment = super::derive_confidential_note_v2(
            asset_definition_id,
            7,
            input_0_rho,
            output_0_owner_tag,
        )
        .expect("input 0 commitment");
        let input_1_owner_tag = super::derive_confidential_owner_tag_v2_with_diversifier(
            &spend_key,
            input_1_diversifier,
        )
        .expect("input 1 owner tag");
        let input_1_commitment = super::derive_confidential_note_v2(
            asset_definition_id,
            5,
            input_1_rho,
            input_1_owner_tag,
        )
        .expect("input 1 commitment");

        let mut tree_commitments = Vec::new();
        tree_commitments.push(input_0_commitment);
        tree_commitments.push(super::scalar_to_repr_bytes(super::Scalar::from(0x99_u64)));
        tree_commitments.push(input_1_commitment);
        let root_hint =
            super::compute_confidential_root_v2(&tree_commitments).expect("confidential root");

        let vk_record =
            super::confidential_transfer_v2_vk_record("vk_transfer", 3).expect("transfer vk");
        let vk_box = vk_record.key.clone().expect("inline transfer vk");
        let proof = super::build_confidential_transfer_proof_v2(
            &chain_id,
            asset_definition_id,
            &spend_key,
            &tree_commitments,
            &[
                super::ConfidentialTransferInputV2 {
                    amount: 7,
                    rho: input_0_rho,
                    diversifier: input_0_diversifier,
                    leaf_index: 0,
                },
                super::ConfidentialTransferInputV2 {
                    amount: 5,
                    rho: input_1_rho,
                    diversifier: input_1_diversifier,
                    leaf_index: 2,
                },
            ],
            &[
                super::ConfidentialTransferOutputV2 {
                    amount: 8,
                    rho: output_0_rho,
                    owner_tag: output_0_owner_tag,
                },
                super::ConfidentialTransferOutputV2 {
                    amount: 4,
                    rho: output_1_rho,
                    owner_tag: output_1_owner_tag,
                },
            ],
            root_hint,
            &vk_record.circuit_id,
            &vk_box,
        )
        .expect("build transfer proof");

        assert!(
            crate::zk::verify_backend(crate::zk::ZK_BACKEND_HALO2_IPA, &proof.proof, Some(&vk_box)),
            "generated confidential transfer v2 proof should verify against the generated VK"
        );
    }

    #[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
    #[test]
    fn generated_confidential_transfer_v2_one_input_two_outputs_verifies_against_generated_vk() {
        let chain_id = iroha_data_model::ChainId::from("confidential-transfer-v2-one-input-test");
        let asset_definition_id = "zcoin#wonderland";
        let spend_key = [0x61_u8; 32];
        let input_rho = [0x71_u8; 32];
        let recipient_output_rho = [0x81_u8; 32];
        let change_output_rho = [0x82_u8; 32];
        let input_diversifier = super::default_confidential_diversifier_v2();
        let sender_owner_tag =
            super::derive_confidential_owner_tag_v2_with_diversifier(&spend_key, input_diversifier)
                .expect("sender owner tag");
        let recipient_diversifier = super::derive_confidential_diversifier_v2(b"recipient");
        let recipient_owner_tag = super::derive_confidential_owner_tag_v2_with_diversifier(
            &[0x72_u8; 32],
            recipient_diversifier,
        )
        .expect("recipient owner tag");
        let input_commitment =
            super::derive_confidential_note_v2(asset_definition_id, 2, input_rho, sender_owner_tag)
                .expect("input commitment");
        let tree_commitments = vec![input_commitment];
        let root_hint =
            super::compute_confidential_root_v2(&tree_commitments).expect("confidential root");
        let vk_record =
            super::confidential_transfer_v2_vk_record("vk_transfer", 3).expect("transfer vk");
        let vk_box = vk_record.key.clone().expect("inline transfer vk");
        let proof = super::build_confidential_transfer_proof_v2(
            &chain_id,
            asset_definition_id,
            &spend_key,
            &tree_commitments,
            &[super::ConfidentialTransferInputV2 {
                amount: 2,
                rho: input_rho,
                diversifier: input_diversifier,
                leaf_index: 0,
            }],
            &[
                super::ConfidentialTransferOutputV2 {
                    amount: 1,
                    rho: recipient_output_rho,
                    owner_tag: recipient_owner_tag,
                },
                super::ConfidentialTransferOutputV2 {
                    amount: 1,
                    rho: change_output_rho,
                    owner_tag: sender_owner_tag,
                },
            ],
            root_hint,
            &vk_record.circuit_id,
            &vk_box,
        )
        .expect("build transfer proof");

        assert!(
            crate::zk::verify_backend(crate::zk::ZK_BACKEND_HALO2_IPA, &proof.proof, Some(&vk_box)),
            "generated one-input confidential transfer v2 proof should verify against the generated VK"
        );
    }

    #[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
    #[test]
    fn kagemusha_topup_shield_v2_binds_every_public_field_and_rejects_substitution() {
        let chain_id = iroha_data_model::ChainId::from("kagemusha-topup-shield-test");
        let asset_definition_id = "pkr#sbp";
        let payer = "ed0120AABBCC@sbp";
        let operation_id = [0x41_u8; 32];
        let spend_key = [0x42_u8; 32];
        let rho = [0x43_u8; 32];
        let diversifier = super::derive_confidential_diversifier_v2(b"topup-owner");
        let atomic_amount = 10_750_000_000_u128;
        let asset_scale = 9;
        let tree_commitments = vec![
            super::scalar_to_repr_bytes(super::scalar_from_u128(0x51)),
            super::scalar_to_repr_bytes(super::scalar_from_u128(0x52)),
        ];
        let leaf_index = u32::try_from(tree_commitments.len()).expect("fixture index");
        let zero_path =
            super::compute_confidential_merkle_path_v2(&tree_commitments, leaf_index as usize)
                .expect("next-zero path");
        let vk_box = super::kagemusha_topup_shield_v2_vk_box().expect("canonical shield vk");
        let result = super::build_kagemusha_topup_shield_proof_v2(
            &chain_id,
            asset_definition_id,
            payer,
            operation_id,
            atomic_amount,
            asset_scale,
            &spend_key,
            rho,
            diversifier,
            leaf_index,
            &zero_path,
            super::KAGEMUSHA_TOPUP_SHIELD_V2_CIRCUIT_ID,
            &vk_box,
        )
        .expect("build Kagemusha top-up shield proof");
        assert!(crate::zk::verify_backend(
            crate::zk::ZK_BACKEND_HALO2_IPA,
            &result.proof,
            Some(&vk_box)
        ));
        let public = super::parse_kagemusha_topup_shield_public_inputs_v2(&result.proof.bytes)
            .expect("parse shield public inputs");
        assert_eq!(public.output_commitment, result.output_commitment);
        assert_eq!(public.spend_nullifier, result.spend_nullifier);
        assert_eq!(public.initial_root, result.initial_root);
        assert_eq!(public.finalized_root, result.finalized_root);
        assert_eq!(
            public.atomic_amount,
            super::encode_confidential_amount_v2(atomic_amount)
        );
        assert_eq!(
            public.asset_scale,
            super::encode_kagemusha_topup_u32_v2(asset_scale)
        );
        assert_eq!(
            public.leaf_index,
            super::encode_kagemusha_topup_u32_v2(leaf_index)
        );
        assert_eq!(
            public.asset_tag,
            super::derive_confidential_asset_tag_v2(asset_definition_id)
        );
        assert_eq!(
            public.chain_tag,
            super::derive_confidential_chain_tag_v2(chain_id.as_str())
        );
        assert_eq!(
            public.payer_tag,
            super::derive_kagemusha_topup_payer_tag_v2(payer)
        );
        assert_eq!(
            public.operation_tag,
            super::derive_kagemusha_topup_operation_tag_v2(&operation_id)
        );

        let envelope: iroha_data_model::zk::OpenVerifyEnvelope =
            norito::decode_from_bytes(&result.proof.bytes).expect("outer proof envelope");
        let (transcript, columns) =
            crate::zk::zkparse::strict_proof_and_instances(&envelope.proof_bytes)
                .expect("inner proof and instances");
        assert_eq!(columns.len(), 11);
        let spend_scalar =
            super::hash_to_scalar(b"iroha.confidential.v3.spend_scalar", &[&spend_key]);
        let output_nodes =
            super::kagemusha_topup_output_path_nodes_v2(result.output_commitment, &zero_path)
                .expect("output path nodes");
        let circuit = super::secure_relation_v3::KagemushaTopUpShieldCircuitV3::<
            { super::CONFIDENTIAL_TREE_DEPTH_V2 },
        > {
            witness: Some(super::KagemushaTopUpShieldWitnessV2 {
                amount: atomic_amount,
                asset_scale,
                leaf_index,
                rho,
                spend_scalar: super::scalar_to_repr_bytes(spend_scalar),
                diversifier,
                asset_tag: super::derive_confidential_asset_tag_v2(asset_definition_id),
                chain_tag: super::derive_confidential_chain_tag_v2(chain_id.as_str()),
                payer_tag: super::derive_kagemusha_topup_payer_tag_v2(payer),
                operation_tag: super::derive_kagemusha_topup_operation_tag_v2(&operation_id),
                zero_path: zero_path.clone(),
                output_nodes,
            }),
        };
        halo2_proofs::dev::MockProver::run(
            super::KAGEMUSHA_TOPUP_SHIELD_V2_IPA_K,
            &circuit,
            columns.clone(),
        )
        .expect("canonical Kagemusha top-up shield mock prover")
        .assert_satisfied();
        for substituted_column in 0..columns.len() {
            let mut substituted = columns.clone();
            substituted[substituted_column][0] += super::Scalar::from(1_u64);
            let substituted_mock = halo2_proofs::dev::MockProver::run(
                super::KAGEMUSHA_TOPUP_SHIELD_V2_IPA_K,
                &circuit,
                substituted.clone(),
            )
            .expect("substituted Kagemusha top-up shield mock prover");
            assert!(
                substituted_mock.verify().is_err(),
                "fixed witness must reject substituted public input column {substituted_column}"
            );
            let mut inner = crate::zk::zk1::wrap_start();
            crate::zk::zk1::wrap_append_proof(&mut inner, &transcript);
            let refs: Vec<&[super::Scalar]> = substituted.iter().map(Vec::as_slice).collect();
            crate::zk::zk1::wrap_append_instances_pasta_fp_cols(&refs, &mut inner);
            let mut substituted_envelope = envelope.clone();
            substituted_envelope.proof_bytes = inner;
            let substituted_proof = iroha_data_model::proof::ProofBox::new(
                crate::zk::ZK_BACKEND_HALO2_IPA.to_owned(),
                norito::to_bytes(&substituted_envelope).expect("encode substituted envelope"),
            );
            assert!(
                !crate::zk::verify_backend(
                    crate::zk::ZK_BACKEND_HALO2_IPA,
                    &substituted_proof,
                    Some(&vk_box),
                ),
                "substituting public input column {substituted_column} must invalidate the proof"
            );
        }
    }

    #[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
    #[test]
    fn kagemusha_topup_shield_v2_statement_rejects_zero_and_colliding_fields() {
        let commitment = super::scalar_to_repr_bytes(super::scalar_from_u128(1));
        let nullifier = super::scalar_to_repr_bytes(super::scalar_from_u128(2));
        let initial_root = super::scalar_to_repr_bytes(super::scalar_from_u128(3));
        let finalized_root = super::scalar_to_repr_bytes(super::scalar_from_u128(4));

        for (output, nullifier, initial, finalized, expected) in [
            (
                [0; 32],
                nullifier,
                initial_root,
                finalized_root,
                "output commitment must be non-zero",
            ),
            (
                commitment,
                [0; 32],
                initial_root,
                finalized_root,
                "spend nullifier must be non-zero",
            ),
            (
                commitment,
                commitment,
                initial_root,
                finalized_root,
                "must be distinct",
            ),
            (
                commitment,
                nullifier,
                [0; 32],
                finalized_root,
                "initial root must be non-zero",
            ),
            (
                commitment,
                nullifier,
                initial_root,
                [0; 32],
                "finalized root must be non-zero",
            ),
            (
                commitment,
                nullifier,
                initial_root,
                initial_root,
                "must change the confidential root",
            ),
        ] {
            let error = super::validate_kagemusha_topup_shield_statement_v2(
                output, nullifier, initial, finalized,
            )
            .expect_err("invalid Kagemusha top-up statement must reject");
            assert!(
                error.contains(expected),
                "unexpected statement validation error `{error}`; expected `{expected}`"
            );
        }
    }

    include!("confidential_v2_builder_tests.rs");
}
