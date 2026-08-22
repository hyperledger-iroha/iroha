    #[test]
    fn native_amx_receipt_validation_accepts_signed_participant_qcs() {
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
        let receipt = signed_native_amx_receipt(
            source_id,
            tx.hash_as_entrypoint(),
            &routing_plan,
            42,
            &keypairs,
        );
        let coordinator_proposal = native_amx_test_coordinator_proposal(
            routing_plan.coordinator_route(),
            tx.hash_as_entrypoint(),
            42,
            &keypairs,
        );
        let authority = native_amx_test_authority(world, &keypairs);
        validate_native_amx_receipt_against_plan(
            &receipt,
            &coordinator_proposal,
            tx.hash_as_entrypoint(),
            &routing_plan,
            source_id,
            native_amx_test_network_id(),
            &dataspace_catalog,
            &authority,
            Some(expected_native_amx_test_context(42)),
        )
        .expect("signed AMX QCs should validate");
        assert_eq!(receipt.version, 2);
        assert_eq!(receipt.source_id.as_slice(), tx_hash.as_ref());
        assert_eq!(receipt.lane_id, LaneId::new(1));
        assert_eq!(receipt.dataspace_id, paynet);
        assert_eq!(receipt.plan_digest, routing_plan.digest());
        assert_eq!(receipt.authority_context_height, 42);
        assert_eq!(receipt.lane_block_height, 7);
        assert_eq!(
            receipt
                .legs
                .iter()
                .map(|leg| {
                    (
                        leg.dataspace_id,
                        leg.prepare_qc.body.phase,
                        leg.commit_qc.body.phase,
                    )
                })
                .collect::<Vec<_>>(),
            vec![
                (paynet, NativeAmxPhase::Prepare, NativeAmxPhase::Commit),
                (cbuae, NativeAmxPhase::Prepare, NativeAmxPhase::Commit)
            ]
        );
    }
    #[test]
    fn native_amx_receipt_validation_binds_sealed_reveal_source_to_outer_entrypoint() {
        let paynet = DataSpaceId::new(7);
        let cbuae = DataSpaceId::new(8);
        let (signed, _) =
            signed_domain_registration_tx(&[("merchant", "paynet"), ("treasury", "cbuae")]);
        let reveal = TransactionEntrypoint::SealedReveal(SealedTransactionReveal::new(
            Hash::new(b"native-amx-sealed-reveal-commitment"),
            signed.clone(),
            [0xA5; 32],
        ));
        let entrypoint_hash = reveal.hash();
        let source_id = native_amx_source_id_from_entrypoint_hash(entrypoint_hash);
        let inner_source_id =
            native_amx_source_id_from_entrypoint_hash(signed.hash_as_entrypoint());
        assert_ne!(
            source_id, inner_source_id,
            "a sealed reveal must retain its outer entrypoint identity"
        );
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
        let receipt =
            signed_native_amx_receipt(source_id, entrypoint_hash, &routing_plan, 42, &keypairs);
        let coordinator_proposal = native_amx_test_coordinator_proposal(
            routing_plan.coordinator_route(),
            entrypoint_hash,
            42,
            &keypairs,
        );
        let authority = native_amx_test_authority(world, &keypairs);
        let validate = |expected_source_id| {
            validate_native_amx_receipt_against_plan(
                &receipt,
                &coordinator_proposal,
                entrypoint_hash,
                &routing_plan,
                expected_source_id,
                native_amx_test_network_id(),
                &dataspace_catalog,
                &authority,
                Some(expected_native_amx_test_context(42)),
            )
        };
        validate(source_id).expect("outer sealed-reveal source identity must validate");
        assert_eq!(
            validate(inner_source_id),
            Err("native AMX receipt source entrypoint mismatch".to_owned()),
            "the underlying signed-transaction identity must not replace the sealed entrypoint"
        );
    }
