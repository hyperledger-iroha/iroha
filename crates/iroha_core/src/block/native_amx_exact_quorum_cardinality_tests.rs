// Exact first-release Native AMX block-admission signer-cardinality tests.

#[test]
fn native_amx_receipt_accepts_three_and_rejects_four_signers_for_four_member_qcs() {
    let paynet = DataSpaceId::new(7);
    let cbuae = DataSpaceId::new(8);
    let (tx, tx_hash) =
        signed_domain_registration_tx(&[("merchant", "paynet"), ("treasury", "cbuae")]);
    let dataspace_catalog = native_amx_test_catalog(paynet, cbuae);
    let routing_plan = crate::queue::RoutingPlan::native_amx(
        crate::queue::RoutingDecision::new(LaneId::new(1), paynet),
        vec![
            crate::queue::RouteLeg::new(
                crate::queue::RoutingDecision::new(LaneId::new(1), paynet),
                crate::queue::RouteLegRole::Participant,
            ),
            crate::queue::RouteLeg::new(
                crate::queue::RoutingDecision::new(LaneId::new(2), cbuae),
                crate::queue::RouteLegRole::Participant,
            ),
        ],
    );
    let (world, keypairs) = native_amx_test_world_with_keys();
    let mut source_id = [0u8; iroha_crypto::Hash::LENGTH];
    source_id.copy_from_slice(tx_hash.as_ref());
    let coordinator_proposal = native_amx_test_coordinator_proposal(
        routing_plan.coordinator_route(),
        tx.hash_as_entrypoint(),
        42,
        &keypairs,
    );
    let authority = native_amx_test_authority(world, &keypairs);
    let validate = |receipt: &NativeAmxReceipt| {
        validate_native_amx_receipt_against_plan(
            receipt,
            &coordinator_proposal,
            tx.hash_as_entrypoint(),
            &routing_plan,
            source_id,
            native_amx_test_network_id(),
            &dataspace_catalog,
            &authority,
            Some(expected_native_amx_test_context(42)),
        )
    };
    let exact = signed_native_amx_receipt_with_signer_count(
        source_id,
        tx.hash_as_entrypoint(),
        &routing_plan,
        42,
        &keypairs,
        3,
    );
    validate(&exact).expect("3-of-4 AMX QCs should validate");
    assert_eq!(exact.legs[0].prepare_qc.validator_set().len(), 4);
    assert_eq!(exact.legs[0].prepare_qc.signers_bitmap, vec![0b0000_0111]);
    let superset = signed_native_amx_receipt_with_signer_count(
        source_id,
        tx.hash_as_entrypoint(),
        &routing_plan,
        42,
        &keypairs,
        4,
    );
    assert!(
        validate(&superset)
            .expect_err("4-of-4 AMX QCs must be rejected")
            .contains("signer count mismatch: expected exactly 3, got 4")
    );
}
