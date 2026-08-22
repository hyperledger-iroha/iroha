    #[test]
    fn initial_executor_denies_transfer_asset_without_owner_signature() {
        let alice_id = ALICE_ID.clone();
        let users_domain_id: DomainId =
            DomainId::try_new("users", "universal").expect("users domain id");
        let user1 = checked_account_id();
        let user2 = checked_account_id();
        let users_domain = Domain::new(users_domain_id.clone()).build(&user1);
        let alice_account = Account::new(alice_id.clone()).build(&alice_id);
        let user1_account = Account::new(user1.clone()).build(&user1);
        let user2_account = Account::new(user2.clone()).build(&user2);
        let world = World::with(
            [users_domain],
            [alice_account, user1_account, user2_account],
            [],
        );
        let state = state_after_genesis(world);
        let header = BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let executor = super::Executor::Initial;
        let transfer_asset_id = AssetId::new(
            iroha_data_model::asset::AssetDefinitionId::derive_from_components(
                DomainId::try_new("users", "universal").unwrap(),
                "coin".parse().unwrap(),
            ),
            user1.clone(),
        );
        let instruction = InstructionBox::from(Transfer::asset_quantity(
            transfer_asset_id,
            1_u32,
            user2.clone(),
        ));
        let transfer = extract_transfer_asset(&instruction)
            .expect("expected to extract asset transfer from instruction");
        let mut stx = block.transaction();
        let allowed = can_transfer_asset(&stx.world, &alice_id, None, &transfer)
            .expect("asset transfer permission check");
        assert!(
            !allowed,
            "alice should not be allowed to transfer user1's asset"
        );
        assert!(
            !(stx._curr_block.is_genesis() && stx.block_hashes.is_empty()),
            "test must execute in non-genesis context"
        );
        let res = executor.execute_instruction(&mut stx, &alice_id, instruction);
        match res {
            Err(ValidationFail::NotPermitted(msg)) => assert!(
                msg.contains("source asset owner must sign the transaction"),
                "unexpected rejection message: {msg}"
            ),
            other => panic!(
                "initial executor should deny asset transfer without owner signature, got: {other:?}"
            ),
        }
    }
    #[test]
    fn initial_executor_allows_source_owner_and_both_exact_transfer_permissions() {
        let asset_domain_id = DomainId::try_new("assets", "universal").expect("asset domain id");
        let definition_owner = checked_account_id();
        let source = checked_account_id();
        let delegate = checked_account_id();
        let destination = checked_account_id();
        let asset_definition_id = AssetDefinitionId::derive_from_components(
            asset_domain_id.clone(),
            "coin".parse().unwrap(),
        );
        let source_asset_id = AssetId::new(asset_definition_id.clone(), source.clone());
        let authorities = [
            ("source owner", source.clone(), None),
            (
                "asset-specific permission",
                delegate.clone(),
                Some(Permission::from(
                    executor_permission::asset::CanTransferAsset {
                        asset: source_asset_id.clone(),
                    },
                )),
            ),
            (
                "asset-definition permission",
                delegate.clone(),
                Some(Permission::from(
                    executor_permission::asset::CanTransferAssetWithDefinition {
                        asset_definition: asset_definition_id.clone(),
                    },
                )),
            ),
        ];
        for (case, authority, permission) in authorities {
            let mut world = World::with_assets(
                [Domain::new(asset_domain_id.clone()).build(&definition_owner)],
                [
                    Account::new(definition_owner.clone()).build(&definition_owner),
                    Account::new(source.clone()).build(&source),
                    Account::new(delegate.clone()).build(&delegate),
                    Account::new(destination.clone()).build(&destination),
                ],
                [AssetDefinition::numeric(
                    asset_definition_id.clone(),
                    "coin".to_owned(),
                    iroha_data_model::asset::AssetBalancePolicy::Global,
                    None,
                )
                .build(&definition_owner)],
                [Asset::new(source_asset_id.clone(), Quantity::from(10_u64))],
                [],
            );
            if let Some(permission) = permission {
                world
                    .account_permissions
                    .insert(authority.clone(), BTreeSet::from([permission]));
            }
            let state = state_for_testing(world);
            let mut block = state.block(BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0));
            let mut transaction = block.transaction();
            transaction.tx_call_hash = Some(Hash::new(case.as_bytes()));
            let result = super::Executor::Initial.execute_instruction(
                &mut transaction,
                &authority,
                Transfer::asset_quantity(source_asset_id.clone(), 1_u32, destination.clone())
                    .into(),
            );
            assert!(
                result.is_ok(),
                "{case} must authorize only its exact asset transfer: {result:?}"
            );
        }
    }
    #[test]
    fn initial_executor_transfer_asset_batch_classifies_owner_atomic_and_rolls_back_mixed_sources()
    {
        let fixture = initial_batch_fixture();
        let instruction: InstructionBox = TransferAssetBatch::new(vec![
            TransferAssetBatchEntry::with_leg_id(
                "owner-a",
                fixture.source.clone(),
                fixture.first_destination.clone(),
                fixture.asset_definition.clone(),
                3_u32,
            ),
            TransferAssetBatchEntry::with_leg_id(
                "owner-b",
                fixture.source.clone(),
                fixture.second_destination.clone(),
                fixture.asset_definition.clone(),
                4_u32,
            ),
        ])
        .into();
        assert!(
            initial_native_instruction_is_explicitly_admitted(&instruction),
            "the concrete batch must be admitted to Core's per-leg authorization"
        );
        let state = state_for_testing(fixture.world);
        let mut block = state.block(BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0));
        let mut transaction = block.transaction();
        transaction.tx_call_hash = Some(Hash::new(b"initial-owner-atomic-batch"));
        super::Executor::Initial
            .execute_instruction(&mut transaction, &fixture.source, instruction)
            .expect("the source owner must be able to execute an atomic batch");
        assert_eq!(
            initial_batch_balance(&transaction.world, &fixture.source_asset),
            Quantity::from(13_u32)
        );
        assert_eq!(
            initial_batch_balance(
                &transaction.world,
                &AssetId::new(
                    fixture.asset_definition.clone(),
                    fixture.first_destination.clone(),
                ),
            ),
            Quantity::from(3_u32)
        );
        assert_eq!(
            initial_batch_balance(
                &transaction.world,
                &AssetId::new(
                    fixture.asset_definition.clone(),
                    fixture.second_destination.clone(),
                ),
            ),
            Quantity::from(4_u32)
        );
        let fixture = initial_batch_fixture();
        let state = state_for_testing(fixture.world);
        let mut block = state.block(BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0));
        let mut transaction = block.transaction();
        transaction.tx_call_hash = Some(Hash::new(b"initial-mixed-atomic-batch"));
        let error = super::Executor::Initial
            .execute_instruction(
                &mut transaction,
                &fixture.source,
                TransferAssetBatch::new(vec![
                    TransferAssetBatchEntry::with_leg_id(
                        "authorized-first",
                        fixture.source.clone(),
                        fixture.first_destination.clone(),
                        fixture.asset_definition.clone(),
                        3_u32,
                    ),
                    TransferAssetBatchEntry::with_leg_id(
                        "foreign-second",
                        fixture.foreign_source.clone(),
                        fixture.second_destination.clone(),
                        fixture.asset_definition.clone(),
                        4_u32,
                    ),
                ])
                .into(),
            )
            .expect_err("one unauthorized source must reject the whole atomic batch");
        assert!(
            format!("{error:?}").contains("lacks authority to transfer source asset"),
            "the classified batch must reach Core's exact source check: {error:?}"
        );
        assert_eq!(
            initial_batch_balance(&transaction.world, &fixture.source_asset),
            Quantity::from(20_u32)
        );
        assert_eq!(
            initial_batch_balance(&transaction.world, &fixture.foreign_source_asset),
            Quantity::from(20_u32)
        );
        for destination in [&fixture.first_destination, &fixture.second_destination] {
            assert_eq!(
                initial_batch_balance(
                    &transaction.world,
                    &AssetId::new(fixture.asset_definition.clone(), destination.clone()),
                ),
                Quantity::zero(),
                "atomic source rejection must leave every destination untouched"
            );
        }
    }
    #[test]
    fn initial_executor_transfer_asset_batch_independent_isolates_unauthorized_source_receipt() {
        let fixture = initial_batch_fixture();
        let state = state_for_testing(fixture.world);
        let mut block = state.block(BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0));
        let mut transaction = block.transaction();
        transaction.tx_call_hash = Some(Hash::new(b"initial-mixed-independent-batch"));
        super::Executor::Initial
            .execute_instruction(
                &mut transaction,
                &fixture.source,
                TransferAssetBatch::independent(vec![
                    TransferAssetBatchEntry::with_leg_id(
                        "authorized-first",
                        fixture.source.clone(),
                        fixture.first_destination.clone(),
                        fixture.asset_definition.clone(),
                        3_u32,
                    ),
                    TransferAssetBatchEntry::with_leg_id(
                        "foreign-second",
                        fixture.foreign_source.clone(),
                        fixture.second_destination.clone(),
                        fixture.asset_definition.clone(),
                        4_u32,
                    ),
                ])
                .into(),
            )
            .expect("independent settlement must isolate a per-leg source rejection");
        assert_eq!(
            initial_batch_balance(&transaction.world, &fixture.source_asset),
            Quantity::from(17_u32)
        );
        assert_eq!(
            initial_batch_balance(&transaction.world, &fixture.foreign_source_asset),
            Quantity::from(20_u32)
        );
        assert_eq!(
            initial_batch_balance(
                &transaction.world,
                &AssetId::new(
                    fixture.asset_definition.clone(),
                    fixture.first_destination.clone(),
                ),
            ),
            Quantity::from(3_u32)
        );
        assert_eq!(
            initial_batch_balance(
                &transaction.world,
                &AssetId::new(
                    fixture.asset_definition.clone(),
                    fixture.second_destination.clone(),
                ),
            ),
            Quantity::zero()
        );
        transaction.apply();
        let mut outcome_rows = block.drain_batch_transfer_outcomes().into_values();
        let outcomes = outcome_rows.next().expect("one transaction receipt row");
        assert!(
            outcome_rows.next().is_none(),
            "one batch must produce exactly one keyed receipt row"
        );
        assert_eq!(outcomes.len(), 2);
        assert!(matches!(
            &outcomes[0].status,
            AssetBatchTransferLegStatus::Applied
        ));
        assert!(matches!(
            &outcomes[1].status,
            AssetBatchTransferLegStatus::Rejected(rejection)
                if rejection.code == AssetBatchTransferRejectionCode::PolicyRejected
                    && rejection.message.contains("lacks authority to transfer source asset")
        ));
        assert_eq!(outcomes[0].leg_index, 0);
        assert_eq!(outcomes[0].leg_id, "authorized-first");
        assert_eq!(outcomes[1].leg_index, 1);
        assert_eq!(outcomes[1].leg_id, "foreign-second");
    }
    #[test]
    fn initial_executor_transfer_asset_batch_accepts_direct_definition_and_role_delegation() {
        for permission_case in ["direct asset", "direct definition", "role definition"] {
            let mut fixture = initial_batch_fixture();
            let permission = match permission_case {
                "direct asset" => Permission::from(executor_permission::asset::CanTransferAsset {
                    asset: fixture.source_asset.clone(),
                }),
                "direct definition" | "role definition" => {
                    Permission::from(executor_permission::asset::CanTransferAssetWithDefinition {
                        asset_definition: fixture.asset_definition.clone(),
                    })
                }
                _ => unreachable!("fixed permission cases"),
            };
            if permission_case == "role definition" {
                let role_id: RoleId = "batch_transfer_delegate".parse().expect("role id");
                let role = Role::new(role_id.clone(), fixture.delegate.clone())
                    .add_permission(permission)
                    .build(&fixture.delegate);
                fixture.world.roles.insert(role_id.clone(), role);
                fixture.world.account_roles.insert(
                    crate::role::RoleIdWithOwner::new(fixture.delegate.clone(), role_id),
                    (),
                );
            } else {
                fixture
                    .world
                    .account_permissions
                    .insert(fixture.delegate.clone(), BTreeSet::from([permission]));
            }
            let state = state_for_testing(fixture.world);
            let mut block = state.block(BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0));
            let mut transaction = block.transaction();
            transaction.tx_call_hash = Some(Hash::new(permission_case.as_bytes()));
            let result = super::Executor::Initial.execute_instruction(
                &mut transaction,
                &fixture.delegate,
                TransferAssetBatch::new(vec![TransferAssetBatchEntry::with_leg_id(
                    "delegated",
                    fixture.source.clone(),
                    fixture.first_destination.clone(),
                    fixture.asset_definition.clone(),
                    5_u32,
                )])
                .into(),
            );
            assert!(
                result.is_ok(),
                "{permission_case} must authorize the exact batch source: {result:?}"
            );
            assert_eq!(
                initial_batch_balance(&transaction.world, &fixture.source_asset),
                Quantity::from(15_u32)
            );
        }
    }
