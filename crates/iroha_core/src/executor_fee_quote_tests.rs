    #[test]
    fn successful_claim_fee_exempt_draft_returns_zero_quote_in_receipt_mode() {
        let (world, mut nexus, pipeline, mut payload) = multi_component_fee_quote_fixture();
        let fee_asset = AssetDefinitionId::parse_address_literal(&nexus.fees.fee_asset_id)
            .expect("fixture fee asset address");
        payload.fee_payment = FeePaymentIntent::authority(
            vec![FeeChargeLimit::new(
                FeeChargeKind::Nexus,
                fee_asset.clone(),
                Quantity::from(9_u32),
            )],
            None,
        );
        let authority_literal = payload.authority.to_string();
        nexus
            .fees
            .successful_claim_fee_exempt_authorities
            .insert(payload.authority.clone());
        nexus.fees.settlement_mode =
            iroha_config::parameters::actual::NexusFeeSettlementMode::LaneRelayBurn;
        payload.metadata.insert(
            SORA_V2_CLAIM_TX_HASH_METADATA_KEY
                .parse()
                .expect("claim hash metadata key"),
            Json::new("ab".repeat(32)),
        );
        payload.metadata.insert(
            SORA_NEXUS_CLAIM_RECIPIENT_METADATA_KEY
                .parse()
                .expect("claim recipient metadata key"),
            Json::new(authority_literal),
        );
        payload.instructions = vec![InstructionBox::from(Mint::asset_quantity(
            1_u32,
            AssetId::new(fee_asset, payload.authority.clone()),
        ))]
        .into();
        assert_receipt_mode_fee_exempt_draft(world, &nexus, &pipeline, payload);
    }
    #[test]
    fn successful_claim_fee_exemption_uses_exact_account_identity() {
        let (_, mut nexus, _, payload) = multi_component_fee_quote_fixture();
        let authority = payload.authority;
        let (other_authority, _) = gen_account_in("fee_quote_other");
        assert!(!successful_claim_fee_authority_allowed(&nexus, &authority));
        nexus
            .fees
            .successful_claim_fee_exempt_authorities
            .insert(authority.clone());
        assert!(successful_claim_fee_authority_allowed(&nexus, &authority));
        assert!(!successful_claim_fee_authority_allowed(
            &nexus,
            &other_authority
        ));
    }
    #[test]
    fn fee_quote_discovers_pipeline_gas_and_matches_strict_signed_payload_quote() {
        let (world, nexus, pipeline, mut payload) = multi_component_fee_quote_fixture();
        let world = world.block();
        let draft = quote_nexus_fee_admission_draft(
            &world,
            &nexus,
            &pipeline,
            &payload,
            0,
            1,
            Some(DataSpaceId::UNIVERSAL),
        )
        .expect("draft quote");
        assert_eq!(
            draft
                .quote
                .charges
                .iter()
                .map(|charge| charge.kind)
                .collect::<Vec<_>>(),
            vec![FeeChargeKind::Nexus, FeeChargeKind::PipelineGas]
        );
        payload.fee_payment = draft.recommended_intent.clone();
        let strict = quote_nexus_fee_admission_payload(
            &world,
            &nexus,
            &pipeline,
            &payload,
            0,
            1,
            Some(DataSpaceId::UNIVERSAL),
        )
        .expect("strict quote for exact recommended intent");
        assert_eq!(strict, draft.quote);
        assert_eq!(strict.authority_balances.len(), 2);
        assert_eq!(strict.authority_charge_assets.len(), 2);
    }
