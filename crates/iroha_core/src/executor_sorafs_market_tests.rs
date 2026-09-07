/// Orderbook and reserve admission reaches the exact native authority boundaries.
mod sorafs_market_admission {
    use super::*;
    use crate::executor::Executor;
    use crate::smartcontracts::ValidSingularQuery;
    use iroha_data_model::{
        isi::{
            error::{InstructionExecutionError, InvalidParameterError},
            sorafs::{
                AdvanceSorafsReserveLifecycle, CancelSorafsOrderbookOrder, ChargeSorafsReserveRent,
                DecideSorafsReserveAppeal, DecideSorafsReserveMovement, DrawSorafsReserveCredit,
                MaintainSorafsOrderbook, MatchSorafsOrderbook,
                RecordSorafsOrderbookSettlementReceipt, RegisterSorafsReserveAccount,
                RepaySorafsReserveCredit, RequestSorafsReserveMovement, SetSorafsOrderbookPolicy,
                SetSorafsReservePolicy, SubmitSorafsOrderbookOrder, SubmitSorafsReserveAppeal,
            },
        },
        query::sorafs::prelude::FindSorafsReserveProviderById,
        sorafs::{
            capacity::ProviderId,
            orderbook::{ORDERBOOK_ADMISSION_POLICY_VERSION_V1, OrderbookAdmissionPolicyV1},
            pin_registry::StorageClass,
            reserve::{
                RESERVE_AUTHORITY_POLICY_VERSION_V1, ReserveAuthorityPolicyV1, ReserveDuration,
                ReserveLifecycleStage, ReserveMovementKindV1, ReservePolicyV1,
                ReserveProviderTermsV1, ReserveTier,
            },
        },
    };
    use iroha_executor_data_model::permission::sorafs::{
        CanSetSorafsPricing, CanSetSorafsReservePolicy,
    };
    use sorafs_manifest::{
        XorQuantity,
        orderbook::{
            ByteRangeV1, ORDERBOOK_CANCEL_VERSION_V1, ORDERBOOK_ORDER_VERSION_V1,
            OrderCancelReasonV1, OrderCancelV1, OrderRequestV1, OrderSideV1, OrderTierV1,
            OrderbookSignatureV1, SETTLEMENT_RECEIPT_VERSION_V1, SettlementReceiptV1,
            derive_orderbook_order_id_v1, order_cancel_signature_digest_v1,
            order_request_signature_digest_v1, settlement_receipt_signature_digest_v1,
        },
        provider_advert::SignatureAlgorithm,
    };

    const PROVIDER: ProviderId = ProviderId::new([0xB1; 32]);
    const NOW: u64 = 2_000;

    fn fixture() -> (State, ReserveAuthorityPolicyV1) {
        let custody = checked_account_id();
        let domain = DomainId::try_new("market", "universal").expect("market domain");
        let definition_id = AssetDefinitionId::derive_from_components(
            domain.clone(),
            "xor".parse().expect("asset name"),
        );
        let definition = AssetDefinition::numeric(
            definition_id.clone(),
            "XOR".to_owned(),
            iroha_data_model::asset::AssetBalancePolicy::Global,
            None,
        )
        .build(&ALICE_ID);
        let mut world = World::with(
            [Domain::new(domain).build(&ALICE_ID)],
            [
                Account::new(ALICE_ID.clone()).build(&ALICE_ID),
                Account::new(BOB_ID.clone()).build(&BOB_ID),
                Account::new(custody.clone()).build(&custody),
            ],
            [definition],
        );
        world.account_permissions.insert(
            ALICE_ID.clone(),
            BTreeSet::from([
                Permission::from(CanSetSorafsPricing),
                Permission::from(CanSetSorafsReservePolicy),
            ]),
        );
        // A matching permission name with a noncanonical payload grants no authority.
        world.account_permissions.insert(
            BOB_ID.clone(),
            BTreeSet::from([
                Permission::new("CanSetSorafsPricing".to_owned(), Json::new("forged")),
                Permission::new("CanSetSorafsReservePolicy".to_owned(), Json::new("forged")),
            ]),
        );
        world.provider_owners.insert(PROVIDER, BOB_ID.clone());
        let policy = ReserveAuthorityPolicyV1 {
            version: RESERVE_AUTHORITY_POLICY_VERSION_V1,
            revision: 1,
            predecessor_policy_digest: None,
            economics: ReservePolicyV1::default(),
            asset_definition: definition_id,
            custody_account: custody,
            treasury_account: ALICE_ID.clone(),
            operations_authority: ALICE_ID.clone(),
            decision_authority: ALICE_ID.clone(),
            grace_period_days: 7,
            default_after_days: 30,
            max_provider_debt: "1000".parse().expect("bounded reserve debt"),
            max_pending_movements_per_provider: 4,
            max_open_appeals_per_provider: 2,
        };
        (state_after_genesis(world), policy)
    }

    fn reject_unchanged(
        transaction: &mut StateTransaction<'_, '_>,
        authority: &AccountId,
        instruction: InstructionBox,
        marker: &str,
    ) {
        let snapshot = |transaction: &StateTransaction<'_, '_>| {
            transaction
                .world
                .smart_contract_state
                .iter()
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect::<Vec<_>>()
        };
        let before = snapshot(transaction);
        let error = Executor::Initial
            .execute_instruction(transaction, authority, instruction)
            .expect_err("native market handler must reject unauthorized or invalid action");
        assert!(
            matches!(error, ValidationFail::InstructionFailed(InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(ref message))) if message.contains(marker)),
            "expected native rejection containing {marker:?}, got {error:?}"
        );
        assert_eq!(
            snapshot(transaction),
            before,
            "rejection changed market ledger state"
        );
    }

    fn signature() -> OrderbookSignatureV1 {
        let (_, public_key) = ALICE_KEYPAIR
            .public_key()
            .try_to_bytes()
            .expect("Ed25519 key bytes");
        OrderbookSignatureV1 {
            algorithm: SignatureAlgorithm::Ed25519,
            public_key: public_key.to_vec(),
            signature: Vec::new(),
        }
    }

    fn sign(digest: [u8; 32]) -> Vec<u8> {
        Signature::try_new(ALICE_KEYPAIR.private_key(), &digest)
            .expect("sign canonical market payload")
            .payload()
            .to_vec()
    }

    #[test]
    fn initial_executor_orderbook_preserves_governance_matcher_and_payload_authority() {
        let (state, _) = fixture();
        let mut block = state.block(BlockHeader::new(
            nonzero!(2_u64),
            None,
            None,
            None,
            NOW * 1_000,
            0,
        ));
        let mut transaction = block.transaction();
        let policy = OrderbookAdmissionPolicyV1 {
            version: ORDERBOOK_ADMISSION_POLICY_VERSION_V1,
            revision: 1,
            predecessor_policy_digest: None,
            market_id: [0xA1; 32],
            matcher_authority: ALICE_ID.clone(),
            settlement_authority: ALICE_ID.clone(),
            paused: false,
            min_order_gib: 2,
            max_order_gib: 1024,
            price_tick_micro_xor: 10,
            max_maker_fee_bps: 100,
            max_taker_fee_bps: 200,
            max_order_lifetime_secs: 3600,
            max_receipt_age_secs: 300,
            max_clock_skew_secs: 5,
            max_receipt_bytes: 1024,
            max_receipts_per_channel: 2,
        };
        let digest = policy.digest().expect("valid market policy digest");
        let activation: InstructionBox = SetSorafsOrderbookPolicy::new(policy).into();
        reject_unchanged(
            &mut transaction,
            &BOB_ID,
            activation.clone(),
            "CanSetSorafsPricing",
        );
        Executor::Initial
            .execute_instruction(&mut transaction, &ALICE_ID, activation)
            .expect("governance activates native orderbook policy");
        let matching: InstructionBox = MatchSorafsOrderbook::new(digest, 0, 1).into();
        reject_unchanged(
            &mut transaction,
            &BOB_ID,
            matching.clone(),
            "governed authority",
        );
        reject_unchanged(
            &mut transaction,
            &ALICE_ID,
            matching,
            "exhaustive match scan",
        );
        let maintenance: InstructionBox = MaintainSorafsOrderbook::new(digest, 0, 1).into();
        reject_unchanged(
            &mut transaction,
            &BOB_ID,
            maintenance.clone(),
            "governed authority",
        );
        Executor::Initial
            .execute_instruction(&mut transaction, &ALICE_ID, maintenance)
            .expect("governed matcher performs native maintenance");

        let owner = ALICE_ID.to_string().into_bytes();
        let mut order = OrderRequestV1 {
            version: ORDERBOOK_ORDER_VERSION_V1,
            order_id: derive_orderbook_order_id_v1(&owner, 1),
            side: OrderSideV1::Bid,
            tier: OrderTierV1::Hot,
            price_per_gib: XorQuantity::try_from_micro(100).expect("price"),
            quantity_gib: 10,
            remaining_gib: 10,
            owner_account: owner.clone(),
            provider_id: None,
            expiry_unix: NOW + 100,
            nonce: 1,
            maker_fee_bps: 10,
            taker_fee_bps: 20,
            signature: signature(),
        };
        order.signature.signature =
            sign(order_request_signature_digest_v1(&order).expect("order digest"));
        reject_unchanged(
            &mut transaction,
            &BOB_ID,
            SubmitSorafsOrderbookOrder::new(
                norito::to_bytes(&order).expect("signed order"),
                digest,
            )
            .into(),
            "does not match transaction authority",
        );
        let mut cancel = OrderCancelV1 {
            version: ORDERBOOK_CANCEL_VERSION_V1,
            order_id: order.order_id,
            owner_account: owner,
            reason: OrderCancelReasonV1::OwnerRequested,
            nonce: 2,
            signature: signature(),
        };
        cancel.signature.signature =
            sign(order_cancel_signature_digest_v1(&cancel).expect("cancellation digest"));
        reject_unchanged(
            &mut transaction,
            &BOB_ID,
            CancelSorafsOrderbookOrder::new(
                norito::to_bytes(&cancel).expect("signed cancellation"),
                digest,
            )
            .into(),
            "does not match transaction authority",
        );

        let mut receipt = SettlementReceiptV1 {
            version: SETTLEMENT_RECEIPT_VERSION_V1,
            receipt_id: [0xC1; 32],
            channel_id: [0xC2; 32],
            trade_id: [0xC3; 32],
            range: ByteRangeV1 { start: 0, end: 10 },
            chunk_hash: [0xC4; 32],
            bytes_delivered: 10,
            xor_debited: "1".parse().expect("debit"),
            provider_credit: "1".parse().expect("credit"),
            fee_amount: XorQuantity::zero(),
            issued_at_unix: NOW,
            settlement_signature: signature(),
        };
        receipt.settlement_signature.signature =
            sign(settlement_receipt_signature_digest_v1(&receipt).expect("receipt digest"));
        // Relay authority may differ from the payload signer; native channel custody
        // validation must still reject a correctly signed but unbound receipt.
        reject_unchanged(
            &mut transaction,
            &BOB_ID,
            RecordSorafsOrderbookSettlementReceipt::new(
                norito::to_bytes(&receipt).expect("signed receipt"),
                digest,
            )
            .into(),
            "unknown channel",
        );
        receipt.settlement_signature.signature[0] ^= 1;
        reject_unchanged(
            &mut transaction,
            &BOB_ID,
            RecordSorafsOrderbookSettlementReceipt::new(
                norito::to_bytes(&receipt).expect("forged receipt"),
                digest,
            )
            .into(),
            "invalid settlement receipt signature",
        );
    }

    #[test]
    fn initial_executor_reserve_preserves_governed_operations_decisions_and_provider_requests() {
        let (mut state, policy) = fixture();
        let digest = policy.digest().expect("reserve policy digest");
        let header = BlockHeader::new(nonzero!(2_u64), None, None, None, NOW * 1_000, 0);
        let block_hash = HashOf::new(&header);
        let mut block = state.block(header);
        let mut transaction = block.transaction();
        let activation: InstructionBox = SetSorafsReservePolicy::new(policy).into();
        reject_unchanged(
            &mut transaction,
            &BOB_ID,
            activation.clone(),
            "CanSetSorafsReservePolicy",
        );
        Executor::Initial
            .execute_instruction(&mut transaction, &ALICE_ID, activation)
            .expect("governance activates native reserve policy");
        let registration: InstructionBox = RegisterSorafsReserveAccount::new(
            ReserveProviderTermsV1 {
                provider_id: PROVIDER,
                provider_account: BOB_ID.clone(),
                tier: ReserveTier::TierA,
                storage_class: StorageClass::Hot,
                duration: ReserveDuration::Monthly,
                capacity_gib: 10,
            },
            digest,
        )
        .into();
        for instruction in [
            registration.clone(),
            ChargeSorafsReserveRent::new(PROVIDER, 1, 1, digest).into(),
            AdvanceSorafsReserveLifecycle::new(PROVIDER, 1, 0, digest).into(),
            DrawSorafsReserveCredit::new(PROVIDER, 1, "1".parse().expect("draw amount"), digest)
                .into(),
        ] {
            reject_unchanged(
                &mut transaction,
                &BOB_ID,
                instruction,
                "governed operations account",
            );
        }
        Executor::Initial
            .execute_instruction(&mut transaction, &ALICE_ID, registration)
            .expect("operations account registers the provider reserve partition");
        let movement: InstructionBox = RequestSorafsReserveMovement::new(
            [0xD1; 32],
            PROVIDER,
            ReserveMovementKindV1::TopUp,
            "1".parse().expect("top-up amount"),
            1,
            digest,
        )
        .into();
        let appeal = |revision| -> InstructionBox {
            SubmitSorafsReserveAppeal::new(
                [0xD2; 32],
                PROVIDER,
                revision,
                ReserveLifecycleStage::Active,
                "provider counter-evidence".to_owned(),
                Some([0xD3; 32]),
                digest,
            )
            .into()
        };
        for instruction in [
            movement.clone(),
            RepaySorafsReserveCredit::new(PROVIDER, 1, "1".parse().expect("repayment"), digest)
                .into(),
            appeal(1),
        ] {
            reject_unchanged(
                &mut transaction,
                &ALICE_ID,
                instruction,
                "not the provider account",
            );
        }
        let movement_decision: InstructionBox = DecideSorafsReserveMovement::new(
            [0xD1; 32],
            2,
            digest,
            false,
            "governed denial".to_owned(),
        )
        .into();
        let appeal_decision: InstructionBox = DecideSorafsReserveAppeal::new(
            [0xD2; 32],
            4,
            digest,
            false,
            "governed denial".to_owned(),
        )
        .into();
        for instruction in [movement_decision.clone(), appeal_decision.clone()] {
            reject_unchanged(
                &mut transaction,
                &BOB_ID,
                instruction,
                "governed decision account",
            );
        }
        Executor::Initial
            .execute_instruction(&mut transaction, &BOB_ID, movement.clone())
            .expect("provider submits native reserve request");
        reject_unchanged(&mut transaction, &BOB_ID, movement, "already recorded");
        Executor::Initial
            .execute_instruction(&mut transaction, &ALICE_ID, movement_decision)
            .expect("governed decision resolves the request");
        Executor::Initial
            .execute_instruction(&mut transaction, &BOB_ID, appeal(3))
            .expect("provider submits native reserve appeal");
        Executor::Initial
            .execute_instruction(&mut transaction, &ALICE_ID, appeal_decision.clone())
            .expect("governed decision resolves the appeal");
        reject_unchanged(
            &mut transaction,
            &ALICE_ID,
            appeal_decision,
            "already decided",
        );
        transaction.apply();
        block
            .commit_world_overlay_for_testing()
            .expect("commit native reserve state");
        state.push_block_hash_for_testing(block_hash);
        let record = FindSorafsReserveProviderById::new(PROVIDER)
            .execute(&state.view())
            .expect("read authoritative reserve partition");
        assert_eq!(record.revision, 5);
        assert_eq!(record.pending_movements, 0);
        assert_eq!(record.open_appeals, 0);
        assert_eq!(record.terms.provider_account, *BOB_ID);
        assert!(record.reserve_balance.is_zero());
    }
}
