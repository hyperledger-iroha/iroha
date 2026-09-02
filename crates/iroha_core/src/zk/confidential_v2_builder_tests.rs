#[test]
fn generated_confidential_unshield_v2_proof_verifies_against_cached_canonical_vk() {
    let network_id = network_id(b"confidential-unshield-v2-test-network");
    let asset_definition_id = "zcoin#wonderland";
    let spend_key = [0x91_u8; 32];
    let input_rho = [0x92_u8; 32];
    let input_diversifier = super::derive_confidential_diversifier_v2(b"unshield-v2-input");
    let input_owner_tag =
        super::derive_confidential_owner_tag_v2_with_diversifier(&spend_key, input_diversifier)
            .expect("input owner tag");
    let input_commitment =
        super::derive_confidential_note_v2(asset_definition_id, 9, input_rho, input_owner_tag)
            .expect("input commitment");
    let tree_commitments = vec![input_commitment];
    let root_hint =
        super::compute_confidential_root_v2(&tree_commitments).expect("confidential root");
    let vk_record =
        super::confidential_unshield_v2_vk_record("vk_unshield", 4).expect("unshield vk");
    let vk_box = vk_record.key.clone().expect("inline unshield vk");
    let proof = super::build_confidential_unshield_proof_v2(
        &network_id,
        asset_definition_id,
        &spend_key,
        &tree_commitments,
        &[super::ConfidentialUnshieldInputV2 {
            amount: 9,
            rho: input_rho,
            diversifier: input_diversifier,
            leaf_index: 0,
        }],
        9,
        root_hint,
        &vk_record.circuit_id,
        &vk_box,
    )
    .expect("build unshield v2 proof");
    assert_eq!(proof.nullifiers.len(), 1);
    assert_eq!(proof.root, root_hint);
    assert!(
        crate::zk::verify_backend(crate::zk::ZK_BACKEND_HALO2_IPA, &proof.proof, Some(&vk_box)),
        "generated confidential unshield v2 proof should verify against the cached canonical VK"
    );
    #[cfg(feature = "zk-halo2-ipa")]
    {
        const EXACT_BACKEND: &str =
            "halo2/pasta/confidential-unshield-full-merkle16-axiom-poseidon-v3";
        let (exact_proof, exact_vk) =
            crate::zk::relabel_halo2_ipa_open_verify_fixture(&proof.proof, &vk_box, EXACT_BACKEND);
        assert!(
            crate::zk::verify_backend(EXACT_BACKEND, &exact_proof, Some(&exact_vk)),
            "exact full-unshield registry label should reach the full-unshield verifier"
        );
    }
    let input_path = super::compute_confidential_merkle_path_v2(&tree_commitments, 0)
        .expect("input membership path");
    let dummy_path =
        super::compute_confidential_merkle_path_v2(&tree_commitments, tree_commitments.len())
            .expect("dummy membership path");
    let explicit_path_proof = super::build_confidential_unshield_proof_v2_with_paths(
        &network_id,
        asset_definition_id,
        &spend_key,
        &[input_path.clone(), dummy_path],
        &[super::ConfidentialUnshieldInputV2 {
            amount: 9,
            rho: input_rho,
            diversifier: input_diversifier,
            leaf_index: 0,
        }],
        9,
        root_hint,
        &vk_record.circuit_id,
        &vk_box,
    )
    .expect("build terminal full-redemption proof from explicit paths");
    assert_eq!(explicit_path_proof.nullifiers.len(), 1);
    assert_eq!(explicit_path_proof.root, root_hint);
    assert!(
        crate::zk::verify_backend(
            crate::zk::ZK_BACKEND_HALO2_IPA,
            &explicit_path_proof.proof,
            Some(&vk_box),
        ),
        "explicit-path full redemption must use the terminal full-unshield verifier",
    );
    let mut wrong_leaf_path = input_path;
    wrong_leaf_path.directions[0] ^= 1;
    assert!(
        super::build_confidential_unshield_proof_v2_with_paths(
            &network_id,
            asset_definition_id,
            &spend_key,
            &[
                wrong_leaf_path,
                super::compute_confidential_merkle_path_v2(
                    &tree_commitments,
                    tree_commitments.len(),
                )
                .expect("dummy membership path")
            ],
            &[super::ConfidentialUnshieldInputV2 {
                amount: 9,
                rho: input_rho,
                diversifier: input_diversifier,
                leaf_index: 0,
            }],
            9,
            root_hint,
            &vk_record.circuit_id,
            &vk_box,
        )
        .is_err(),
        "full redemption must reject a substituted input direction",
    );
    let mut tampered = proof.proof.clone();
    let mut envelope: iroha_data_model::zk::OpenVerifyEnvelope =
        norito::decode_from_bytes(&tampered.bytes).expect("OpenVerifyEnvelope");
    envelope.vk_hash[0] ^= 0x80;
    tampered.bytes = norito::to_bytes(&envelope).expect("OpenVerifyEnvelope encode");
    assert!(
        !crate::zk::verify_backend(crate::zk::ZK_BACKEND_HALO2_IPA, &tampered, Some(&vk_box)),
        "unshield v2 proof must reject verifier-key hash substitution"
    );
}
#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
#[test]
fn generated_confidential_unshield_v3_proof_verifies_and_rejects_bad_change() {
    let network_id = network_id(b"confidential-unshield-v3-test-network");
    let asset_definition_id = "zcoin#wonderland";
    let spend_key = [0xA1_u8; 32];
    let input_rho = [0xA2_u8; 32];
    let change_rho = [0xA3_u8; 32];
    let input_diversifier = super::derive_confidential_diversifier_v2(b"unshield-v3-input");
    let input_owner_tag =
        super::derive_confidential_owner_tag_v2_with_diversifier(&spend_key, input_diversifier)
            .expect("input owner tag");
    let input_commitment =
        super::derive_confidential_note_v2(asset_definition_id, 9, input_rho, input_owner_tag)
            .expect("input commitment");
    let tree_commitments = vec![input_commitment];
    let root_hint =
        super::compute_confidential_root_v2(&tree_commitments).expect("confidential root");
    let vk_record =
        super::confidential_unshield_v3_vk_record("vk_unshield_v3", 5).expect("unshield v3 vk");
    let vk_box = vk_record.key.clone().expect("inline unshield v3 vk");
    let terminal = super::build_confidential_unshield_proof_v3(
        &network_id,
        asset_definition_id,
        &spend_key,
        &tree_commitments,
        &[super::ConfidentialUnshieldInputV2 {
            amount: 9,
            rho: input_rho,
            diversifier: input_diversifier,
            leaf_index: 0,
        }],
        &[],
        9,
        root_hint,
        &vk_record.circuit_id,
        &vk_box,
    )
    .expect("build terminal full unshield under the V3 verifier");
    assert_eq!(terminal.nullifiers.len(), 1);
    assert!(terminal.output_commitments.is_empty());
    assert_eq!(terminal.root, root_hint);
    assert!(
        crate::zk::verify_backend(
            crate::zk::ZK_BACKEND_HALO2_IPA,
            &terminal.proof,
            Some(&vk_box),
        ),
        "terminal full unshield must verify under the deployed V3 verifier",
    );
    let input_path = super::compute_confidential_merkle_path_v2(&tree_commitments, 0)
        .expect("terminal input membership path");
    let dummy_path =
        super::compute_confidential_merkle_path_v2(&tree_commitments, tree_commitments.len())
            .expect("terminal dummy membership path");
    let terminal_with_paths = super::build_confidential_unshield_proof_v3_with_paths(
        &network_id,
        asset_definition_id,
        &spend_key,
        &[input_path, dummy_path],
        &[super::ConfidentialUnshieldInputV2 {
            amount: 9,
            rho: input_rho,
            diversifier: input_diversifier,
            leaf_index: 0,
        }],
        &[],
        9,
        root_hint,
        &vk_record.circuit_id,
        &vk_box,
    )
    .expect("build terminal full unshield from explicit paths under V3");
    assert!(terminal_with_paths.output_commitments.is_empty());
    assert!(crate::zk::verify_backend(
        crate::zk::ZK_BACKEND_HALO2_IPA,
        &terminal_with_paths.proof,
        Some(&vk_box),
    ));
    let missing_change = super::build_confidential_unshield_proof_v3(
        &network_id,
        asset_definition_id,
        &spend_key,
        &tree_commitments,
        &[super::ConfidentialUnshieldInputV2 {
            amount: 9,
            rho: input_rho,
            diversifier: input_diversifier,
            leaf_index: 0,
        }],
        &[],
        5,
        root_hint,
        &vk_record.circuit_id,
        &vk_box,
    )
    .expect_err("nonzero change must require a private change note");
    assert!(
        missing_change.contains("requires a private change output"),
        "unexpected missing-change error: {missing_change}"
    );
    let bad_change = super::build_confidential_unshield_proof_v3(
        &network_id,
        asset_definition_id,
        &spend_key,
        &tree_commitments,
        &[super::ConfidentialUnshieldInputV2 {
            amount: 9,
            rho: input_rho,
            diversifier: input_diversifier,
            leaf_index: 0,
        }],
        &[super::ConfidentialUnshieldOutputV3 {
            amount: 3,
            rho: change_rho,
        }],
        5,
        root_hint,
        &vk_record.circuit_id,
        &vk_box,
    )
    .expect_err("incorrect private change amount must reject");
    assert!(
        bad_change.contains("change note amount mismatch"),
        "unexpected bad-change error: {bad_change}"
    );
    let overflow_input_0_rho = [0xB1_u8; 32];
    let overflow_input_1_rho = [0xB2_u8; 32];
    let overflow_input_0_diversifier =
        super::derive_confidential_diversifier_v2(b"unshield-v3-overflow-input-0");
    let overflow_input_1_diversifier =
        super::derive_confidential_diversifier_v2(b"unshield-v3-overflow-input-1");
    let overflow_input_0_owner_tag = super::derive_confidential_owner_tag_v2_with_diversifier(
        &spend_key,
        overflow_input_0_diversifier,
    )
    .expect("overflow input 0 owner tag");
    let overflow_input_1_owner_tag = super::derive_confidential_owner_tag_v2_with_diversifier(
        &spend_key,
        overflow_input_1_diversifier,
    )
    .expect("overflow input 1 owner tag");
    let overflow_tree_commitments = vec![
        super::derive_confidential_note_v2(
            asset_definition_id,
            u128::MAX,
            overflow_input_0_rho,
            overflow_input_0_owner_tag,
        )
        .expect("overflow input 0 commitment"),
        super::derive_confidential_note_v2(
            asset_definition_id,
            1,
            overflow_input_1_rho,
            overflow_input_1_owner_tag,
        )
        .expect("overflow input 1 commitment"),
    ];
    let overflow_root_hint = super::compute_confidential_root_v2(&overflow_tree_commitments)
        .expect("overflow confidential root");
    let overflow = super::build_confidential_unshield_proof_v3(
        &network_id,
        asset_definition_id,
        &spend_key,
        &overflow_tree_commitments,
        &[
            super::ConfidentialUnshieldInputV2 {
                amount: u128::MAX,
                rho: overflow_input_0_rho,
                diversifier: overflow_input_0_diversifier,
                leaf_index: 0,
            },
            super::ConfidentialUnshieldInputV2 {
                amount: 1,
                rho: overflow_input_1_rho,
                diversifier: overflow_input_1_diversifier,
                leaf_index: 1,
            },
        ],
        &[],
        0,
        overflow_root_hint,
        &vk_record.circuit_id,
        &vk_box,
    )
    .expect_err("overflowing private input sum must reject");
    assert!(
        overflow.contains("input amount sum overflows u128"),
        "unexpected overflow error: {overflow}"
    );
    let proof = super::build_confidential_unshield_proof_v3(
        &network_id,
        asset_definition_id,
        &spend_key,
        &tree_commitments,
        &[super::ConfidentialUnshieldInputV2 {
            amount: 9,
            rho: input_rho,
            diversifier: input_diversifier,
            leaf_index: 0,
        }],
        &[super::ConfidentialUnshieldOutputV3 {
            amount: 4,
            rho: change_rho,
        }],
        5,
        root_hint,
        &vk_record.circuit_id,
        &vk_box,
    )
    .expect("build unshield v3 proof");
    let expected_change_owner_tag =
        super::derive_confidential_owner_tag_v2(&spend_key).expect("valid default owner tag");
    let expected_change_commitment = super::derive_confidential_note_v2(
        asset_definition_id,
        4,
        change_rho,
        expected_change_owner_tag,
    )
    .expect("expected change commitment");
    assert_eq!(proof.output_commitments, vec![expected_change_commitment]);
    assert_eq!(proof.nullifiers.len(), 1);
    assert_eq!(proof.root, root_hint);
    assert!(
        crate::zk::verify_backend(crate::zk::ZK_BACKEND_HALO2_IPA, &proof.proof, Some(&vk_box)),
        "generated confidential unshield v3 proof should verify against the cached canonical VK"
    );
    #[cfg(feature = "zk-halo2-ipa")]
    {
        const EXACT_BACKEND: &str =
            "halo2/pasta/confidential-unshield-change-merkle16-axiom-poseidon-v4";
        let (exact_proof, exact_vk) =
            crate::zk::relabel_halo2_ipa_open_verify_fixture(&proof.proof, &vk_box, EXACT_BACKEND);
        assert!(
            crate::zk::verify_backend(EXACT_BACKEND, &exact_proof, Some(&exact_vk)),
            "exact change-unshield registry label should reach the change-unshield verifier"
        );
    }
}
