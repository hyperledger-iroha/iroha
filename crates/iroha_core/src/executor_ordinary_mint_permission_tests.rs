// Exact ordinary Mint purpose tests using the real Initial executor and World permission paths.

#[test]
#[allow(clippy::too_many_lines)]
fn ordinary_mint_permission_requires_live_asset_owner_and_cannot_delegate() {
    use crate::smartcontracts::isi::kagemusha::ordinary_mint_permission::world_has_exact_ordinary_mint_issuer_permission_v1;
    use iroha_data_model::testing::ordinary_app_enrollment::KagemushaOrdinaryRetailEnrollmentFixtureV1;

    let mut policy = KagemushaOrdinaryRetailEnrollmentFixtureV1::new(false).issuer_policy;
    let owner = checked_account_id();
    let adjacent = checked_account_id();
    let issuer = AccountId::new(policy.issuer_public_key.clone());
    let sink = checked_account_id();
    let asset = policy.runtime.asset.clone();
    let mut world = World::with_assets(
        [],
        [
            owner.clone(),
            adjacent.clone(),
            issuer.clone(),
            sink.clone(),
        ]
        .map(|id| Account::new(id.clone()).build(&id)),
        [AssetDefinition::numeric(
            asset.clone(),
            "ordinary Mint asset",
            iroha_data_model::asset::AssetBalancePolicy::Global,
            None,
        )
        .build(&owner)],
        [],
        [],
    );
    world.account_permissions.insert(
        issuer.clone(),
        BTreeSet::from([executor_permission::kagemusha::CanManageKagemushaReserve.into()]),
    );
    let state = state_after_genesis(world);
    policy.runtime.network_id = state.network_id;
    {
        let mut world = state.world.block();
        world
            .axt_asset_incarnations
            .insert(asset, policy.runtime.asset_incarnation);
        world.commit();
    }
    let mut block = state.block(BlockHeader::new(
        nonzero!(2_u64),
        state.view().latest_block_hash(),
        None,
        1000,
        0,
    ));
    let mut transaction = block.transaction();
    let token = ordinary_mint_permission_fixture_token(policy.clone());
    // Public test-fixture DATA selection only; no installed service or funding authority.
    transaction
        .world
        .asset_definition_mut(&token.issuer_policy.runtime.asset)
        .unwrap()
        .metadata
        .insert(
            iroha_data_model::kagemusha::KAGEMUSHA_ORDINARY_LINEAGE_DATA_AUTHORITY_METADATA_KEY_V1
                .parse()
                .unwrap(),
            Json::new(token.lineage_data_authority.clone()),
        );
    let permission: Permission = token.clone().into();
    assert!(!initial_permission_is_genesis_only(&permission));
    assert!(
        !world_has_exact_ordinary_mint_issuer_permission_v1(&transaction.world, &token).unwrap()
    );
    for authority in [&adjacent, &issuer] {
        super::Executor::Initial
            .execute_instruction(
                &mut transaction,
                authority,
                Grant::account_permission(permission.clone(), issuer.clone()).into(),
            )
            .expect_err("adjacent or issuer cannot self-root purpose");
    }
    super::Executor::Initial
        .execute_instruction(
            &mut transaction,
            &owner,
            Grant::account_permission(permission.clone(), issuer.clone()).into(),
        )
        .expect("actual live asset owner grants exact purpose");
    assert!(
        world_has_exact_ordinary_mint_issuer_permission_v1(&transaction.world, &token).unwrap()
    );
    let admitted = crate::smartcontracts::isi::kagemusha::ordinary_mint_permission::admit_ordinary_mint_issuer_purpose_v1(&transaction, &token).expect("actual current World issuer purpose admitted");
    admitted.recheck(&transaction).unwrap();
    let mut future = token.clone();
    future.issuer_policy.valid_from_ms = 1001;
    future.issuer_policy.maximum_certificate_lifetime_ms = 1;
    super::Executor::Initial
        .execute_instruction(
            &mut transaction,
            &owner,
            Grant::account_permission(Permission::from(future.clone()), issuer.clone()).into(),
        )
        .expect("owner can schedule a scoped future issuer policy");
    crate::smartcontracts::isi::kagemusha::ordinary_mint_permission::admit_ordinary_mint_issuer_purpose_v1(&transaction, &future).expect_err("future original interval supplies no current issuer purpose");
    let mut expired = token.clone();
    expired.issuer_policy.expires_at_ms = 1000;
    expired.issuer_policy.maximum_certificate_lifetime_ms = 1;
    super::Executor::Initial
        .execute_instruction(
            &mut transaction,
            &owner,
            Grant::account_permission(Permission::from(expired.clone()), issuer.clone()).into(),
        )
        .expect("owner can retain an old exact token as historical data");
    crate::smartcontracts::isi::kagemusha::ordinary_mint_permission::admit_ordinary_mint_issuer_purpose_v1(&transaction, &expired).expect_err("expiry is exclusive even when exact token remains in World");
    assert_eq!(admitted.issuer_policy(), &policy);
    assert_eq!(admitted.release_id(), token.release_id);
    super::Executor::Initial
        .execute_instruction(
            &mut transaction,
            &issuer,
            Grant::account_permission(permission.clone(), sink.clone()).into(),
        )
        .expect_err("holder cannot delegate this separate purpose");
    let role: RoleId = "ordinary_mint_purpose".parse().unwrap();
    let registration = Register::role(
        Role::new(role.clone(), owner.clone()).add_permission(permission.clone()),
    );
    let denied = super::Executor::Initial
        .execute_instruction(&mut transaction, &owner, registration.clone().into())
        .expect_err("live asset ownership alone cannot administer roles");
    assert!(matches!(denied, ValidationFail::NotPermitted(reason) if reason == "Can't register role"));
    assert!(transaction.world.roles.get(&role).is_none());
    // Seed the separate exact fixture permission; the original executor still
    // verifies every ordinary Mint purpose and role-management condition.
    Grant::account_permission(executor_permission::role::CanManageRoles, owner.clone())
        .execute(&owner, &mut transaction)
        .expect("fixture grants the independent role-management permission");
    super::Executor::Initial
        .execute_instruction(
            &mut transaction,
            &owner,
            registration.into(),
        )
        .expect("actual asset owner with role management may create role with exact purpose");
    super::Executor::Initial
        .execute_instruction(
            &mut transaction,
            &owner,
            Grant::account_role(role.clone(), issuer.clone()).into(),
        )
        .expect("actual owner holding the role grants it to the scoped issuer");
    super::Executor::Initial
        .execute_instruction(
            &mut transaction,
            &issuer,
            Grant::account_role(role, sink.clone()).into(),
        )
        .expect_err("role holder cannot delegate issuer purpose");
    let mut changed_release = token.clone();
    changed_release.release_id[0] ^= 1;
    let mut changed_window = token.clone();
    changed_window.issuer_policy.expires_at_ms += 1;
    let mut changed_policy = token.clone();
    changed_policy.issuer_policy.issuer_policy_id[0] ^= 1;
    for changed in [changed_release, changed_window, changed_policy] {
        assert!(
            !world_has_exact_ordinary_mint_issuer_permission_v1(&transaction.world, &changed)
                .unwrap()
        );
    }
    let mut external = token.clone();
    external.issuer_policy.runtime.network_id =
        KagemushaOrdinaryRetailEnrollmentFixtureV1::new(false)
            .issuer_policy
            .runtime
            .network_id;
    assert_ne!(
        external.issuer_policy.runtime.network_id,
        policy.runtime.network_id
    );
    super::Executor::Initial
        .execute_instruction(
            &mut transaction,
            &owner,
            Grant::account_permission(Permission::from(external), issuer.clone()).into(),
        )
        .expect_err("asset owner cannot grant external network");
    let mut wrong_incarnation = token.clone();
    wrong_incarnation.issuer_policy.runtime.asset_incarnation =
        iroha_data_model::nexus::AxtAssetIncarnationV1::try_from_bytes(
            *iroha_crypto::Hash::new(b"external-incarnation").as_ref(),
        )
        .unwrap();
    super::Executor::Initial
        .execute_instruction(
            &mut transaction,
            &owner,
            Grant::account_permission(Permission::from(wrong_incarnation), issuer.clone()).into(),
        )
        .expect_err("asset owner cannot grant stale/different incarnation");
    super::Executor::Initial
        .execute_instruction(
            &mut transaction,
            &owner,
            Revoke::account_permission(permission, issuer.clone()).into(),
        )
        .expect("actual asset owner revokes exact direct grant");
    assert!(
        world_has_exact_ordinary_mint_issuer_permission_v1(&transaction.world, &token).unwrap()
    );
    admitted.recheck(&transaction).unwrap();
    // The role still carries the exact token; separately removing its role permission revokes it.
    super::Executor::Initial
        .execute_instruction(
            &mut transaction,
            &owner,
            Revoke::role_permission(
                Permission::from(token.clone()),
                "ordinary_mint_purpose".parse().unwrap(),
            )
            .into(),
        )
        .expect("actual asset owner revokes role purpose");
    assert!(
        !world_has_exact_ordinary_mint_issuer_permission_v1(&transaction.world, &token).unwrap()
    );
    admitted
        .recheck(&transaction)
        .expect_err("retained purpose cannot survive actual World revocation");
}

#[test]
fn ordinary_mint_permission_exact_policy_roundtrip_and_malformed_refusal() {
    use iroha_data_model::testing::ordinary_app_enrollment::KagemushaOrdinaryRetailEnrollmentFixtureV1;
    use iroha_executor_data_model::permission::kagemusha::CanAuthorizeKagemushaOrdinaryMint;
    let token = ordinary_mint_permission_fixture_token(
        KagemushaOrdinaryRetailEnrollmentFixtureV1::new(false).issuer_policy,
    );
    token.validate_scope().unwrap();
    let permission: Permission = token.clone().into();
    let decoded = CanAuthorizeKagemushaOrdinaryMint::try_from(&permission).unwrap();
    assert_eq!(decoded, token);
    validate_initial_permission_payload_constraints(&permission).unwrap();
    let mut zero = token.clone();
    zero.release_id = [0; 32];
    validate_initial_permission_payload_constraints(&zero.into())
        .expect_err("absent release refused before grant");
    for changed in 0..5 {
        let mut invalid_roots = token.clone();
        match changed {
            0 => invalid_roots.app_identity_authority.threshold = 0,
            1 => invalid_roots
                .app_identity_authority
                .authorized_signers
                .reverse(),
            2 => invalid_roots
                .app_identity_authority
                .authorized_signers
                .push(invalid_roots.app_identity_authority.authorized_signers[0].clone()),
            3 => invalid_roots.clock_selection_original_sha256 = [0; 32],
            _ => {
                invalid_roots
                    .app_identity_authority
                    .expected_identity_policy_id = [0; 32]
            }
        }
        validate_initial_permission_payload_constraints(&invalid_roots.into())
            .expect_err("invalid independent source roots refused before grant");
    }
    let mut invalid = token;
    invalid.issuer_policy.expires_at_ms = invalid.issuer_policy.valid_from_ms;
    validate_initial_permission_payload_constraints(&invalid.into())
        .expect_err("invalid original policy refused before grant");
}

fn ordinary_mint_permission_fixture_token(
    policy: iroha_data_model::kagemusha::KagemushaRetailEnrollmentIssuerPolicyV1,
) -> iroha_executor_data_model::permission::kagemusha::CanAuthorizeKagemushaOrdinaryMint {
    let fixture = iroha_data_model::testing::ordinary_app_enrollment::KagemushaOrdinaryRetailEnrollmentFixtureV1::new(false);
    let raw = fixture
        .ordinary_policy
        .identity_policy()
        .authority_original();
    let mut roots: iroha_data_model::kagemusha::KagemushaOrdinaryAppIdentityAuthorityPolicyV1 =
        norito::decode_canonical_with_limits(raw, norito::canonical_decode_limits(raw.len()))
            .unwrap();
    // Public test-only grant data, never an accepting credential/clock or funding capability.
    roots.network_id = policy.runtime.network_id;
    let lineage_data_authority =
        iroha_data_model::kagemusha::KagemushaOrdinaryLineageDataAuthorityV1 {
            version: 1,
            liability_pool_id: iroha_data_model::kagemusha::kagemusha_liability_pool_id_v1(
                &policy.runtime.network_id,
                &policy.runtime.asset,
                policy.runtime.asset_incarnation,
            )
            .unwrap(),
            service_identity_digest: [90; 32],
            data_incarnation_digest: [91; 32],
            dataspace: "mibank.bpng".into(),
            tenant: "mibank-core".into(),
            principal: "core-mibank".into(),
            collection: "retail_enrollments".into(),
        };
    iroha_executor_data_model::permission::kagemusha::CanAuthorizeKagemushaOrdinaryMint {
        issuer_policy: policy,
        release_id: [7; 32],
        app_identity_authority: roots,
        clock_selection_original_sha256: [8; 32],
        lineage_data_authority,
    }
}

#[test]
fn ordinary_node_mint_inert_transport_cannot_debit_or_populate_reserve_world() {
    use iroha_data_model::testing::ordinary_node_mint::kagemusha_ordinary_node_mint_codec_fixture_v1;
    let fixture = kagemusha_ordinary_node_mint_codec_fixture_v1();
    let owner = fixture
        .mint_fixture
        .request
        .authorization
        .statement
        .context
        .lineage
        .owner
        .account_id
        .clone();
    let definition = fixture
        .mint_fixture
        .request
        .authorization
        .statement
        .context
        .lineage
        .owner
        .runtime
        .asset
        .clone();
    let source = AssetId::new(definition.clone(), owner.clone());
    let world = World::with_assets(
        [],
        [Account::new(owner.clone()).build(&owner)],
        [AssetDefinition::numeric(
            definition,
            "ordinary Mint codec refusal",
            iroha_data_model::asset::AssetBalancePolicy::Global,
            None,
        )
        .build(&owner)],
        [Asset::new(source.clone(), Quantity::from(1000_u32))],
        [],
    );
    let state = state_after_genesis(world);
    let mut block = state.block(BlockHeader::new(
        nonzero!(2_u64),
        state.view().latest_block_hash(),
        None,
        1000,
        0,
    ));
    let mut transaction = block.transaction();
    let old_asset = (*transaction.world.asset(&source).unwrap()).clone();
    let instruction =
        iroha_data_model::isi::TopUpKagemushaOrdinaryV1::new(fixture.submission.clone()).unwrap();
    super::Executor::Initial.execute_instruction(&mut transaction, &owner, instruction.into()).expect_err("public fixture has no installed release, current World purpose, signed clock or genuine Mint proof");
    assert_eq!(*transaction.world.asset(&source).unwrap(), old_asset);
    assert!(
        transaction
            .world
            .kagemusha_reserve_operations
            .iter()
            .next()
            .is_none()
    );
    assert!(
        transaction
            .world
            .kagemusha_reserve_pools
            .iter()
            .next()
            .is_none()
    );
    assert!(
        transaction
            .world
            .kagemusha_mint_credit_operations
            .iter()
            .next()
            .is_none()
    );
    assert!(
        transaction
            .world
            .kagemusha_issuance_operations
            .iter()
            .next()
            .is_none()
    );
    let mut changed = fixture.submission;
    changed.preparation_control_original.push(0);
    assert!(iroha_data_model::isi::TopUpKagemushaOrdinaryV1::new(changed).is_err());
}
