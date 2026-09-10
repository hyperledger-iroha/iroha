use iroha_data_model::executor::ValidationFail;

fn execute_initial_soracloud(
    instruction: impl Into<InstructionBox>,
    authority: &AccountId,
    state_transaction: &mut StateTransaction<'_, '_>,
) -> Result<(), ValidationFail> {
    crate::executor::Executor::Initial.execute_instruction(
        state_transaction,
        authority,
        instruction.into(),
    )
}

fn assert_initial_soracloud_core_denial(error: ValidationFail, message: &str) {
    let ValidationFail::InstructionFailed(error) = error else {
        panic!("instruction must reach Core authorization, got {error:?}");
    };
    let detail = match &error {
        InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(detail)) => {
            detail.as_str()
        }
        InstructionExecutionError::InvariantViolation(detail) => detail.as_ref(),
        _ => panic!("unexpected Core rejection category: {error:?}"),
    };
    assert!(
        detail.contains(message),
        "unexpected Core rejection: expected {message:?}, got {error:?}"
    );
}

#[test]
fn initial_executor_soracloud_host_lifecycle_preserves_exact_validator_authority()
-> Result<(), eyre::Report> {
    permissioned_soracloud_state!(kura, state);
    soracloud_transaction_at_height!(state, header, block, stx, 2);
    Register::account(Account::new(BOB_ID.clone()))
        .execute(&SAMPLE_GENESIS_ACCOUNT_ID, &mut stx)?;
    let peer = PeerId::from(
        KeyPair::try_from_seed(vec![0xA7; 32], Algorithm::BlsNormal)?
            .public_key()
            .clone(),
    );
    let validator_key = (LaneId::SINGLE, ALICE_ID.clone());
    stx.world
        .public_lane_validators
        .get_mut(&validator_key)
        .unwrap()
        .peer_id = peer.clone();
    let now = stx.block_unix_timestamp_ms().max(1);
    let mut capability = sample_inrou_host_capability(ALICE_ID.clone(), now, now + 10_000);
    capability.peer_id = peer.to_string();
    let advert = isi::AdvertiseSoracloudInrouHost {
        capability: capability.clone(),
        provenance: inrou_host_advertise_provenance(&capability),
    };
    let mut wrong_peer = advert.clone();
    wrong_peer.capability.peer_id =
        PeerId::from(BOB_ID.expect_single_signatory().clone()).to_string();
    wrong_peer.provenance = inrou_host_advertise_provenance(&wrong_peer.capability);
    assert_initial_soracloud_core_denial(
        execute_initial_soracloud(wrong_peer, &ALICE_ID, &mut stx).unwrap_err(),
        "active public-lane validator record",
    );
    let mut wrong_account = advert.clone();
    wrong_account.capability.validator_account_id = BOB_ID.clone();
    assert_initial_soracloud_core_denial(
        execute_initial_soracloud(wrong_account, &ALICE_ID, &mut stx).unwrap_err(),
        "must match the transaction authority",
    );
    let mut wrong_signer = advert.clone();
    wrong_signer.provenance = inrou_host_advertise_provenance_for(&BOB_KEYPAIR, &capability);
    let error = execute_initial_soracloud(wrong_signer, &ALICE_ID, &mut stx).unwrap_err();
    assert!(
        matches!(error, ValidationFail::InstructionFailed(_)),
        "{error:?}"
    );
    stx.world
        .public_lane_validators
        .get_mut(&validator_key)
        .unwrap()
        .status = PublicLaneValidatorStatus::PendingActivation(3);
    assert_initial_soracloud_core_denial(
        execute_initial_soracloud(advert.clone(), &ALICE_ID, &mut stx).unwrap_err(),
        "not an active public-lane validator",
    );
    stx.world
        .public_lane_validators
        .get_mut(&validator_key)
        .unwrap()
        .status = PublicLaneValidatorStatus::Active;
    assert!(
        stx.world
            .soracloud_inrou_host_capabilities
            .get(&ALICE_ID)
            .is_none()
    );

    // A validator needs no management permission to report its own host, and its BLS peer is
    // independently authoritative from the Ed25519 account/provenance signing key.
    stx.world.account_permissions.remove(ALICE_ID.clone());
    execute_initial_soracloud(advert, &ALICE_ID, &mut stx)?;
    assert_eq!(
        stx.world.soracloud_inrou_host_capabilities.get(&ALICE_ID),
        Some(&capability)
    );
    let error =
        execute_initial_soracloud(isi::ReconcileSoracloudInrouPlacements, &BOB_ID, &mut stx)
            .expect_err("an unrelated account cannot reconcile placements");
    assert!(
        matches!(error, ValidationFail::InstructionFailed(_)),
        "{error:?}"
    );
    execute_initial_soracloud(isi::ReconcileSoracloudInrouPlacements, &ALICE_ID, &mut stx)?;
    let withdrawal = isi::WithdrawSoracloudInrouHost {
        validator_account_id: ALICE_ID.clone(),
        provenance: inrou_host_withdraw_provenance_for(&ALICE_KEYPAIR, &ALICE_ID),
    };
    let mut forged_withdrawal = withdrawal.clone();
    forged_withdrawal.provenance = inrou_host_withdraw_provenance_for(&BOB_KEYPAIR, &ALICE_ID);
    let error = execute_initial_soracloud(forged_withdrawal, &ALICE_ID, &mut stx).unwrap_err();
    assert!(
        matches!(error, ValidationFail::InstructionFailed(_)),
        "{error:?}"
    );
    assert_eq!(
        stx.world.soracloud_inrou_host_capabilities.get(&ALICE_ID),
        Some(&capability)
    );
    execute_initial_soracloud(withdrawal, &ALICE_ID, &mut stx)?;
    assert!(
        stx.world
            .soracloud_inrou_host_capabilities
            .get(&ALICE_ID)
            .is_none()
    );
    Ok(())
}

#[test]
fn initial_executor_soracloud_roles_preserve_exact_permission_payloads_and_delegation()
-> Result<(), eyre::Report> {
    permissioned_soracloud_state!(kura, state);
    soracloud_transaction_at_height!(state, header, block, stx, 2);
    for account in [BOB_ID.clone(), CARPENTER_ID.clone()] {
        Register::account(Account::new(account)).execute(&SAMPLE_GENESIS_ACCOUNT_ID, &mut stx)?;
    }
    let scope = SoracloudFheGovernancePermissionScopeV1 {
        schema_version: SORACLOUD_FHE_GOVERNANCE_PERMISSION_SCOPE_VERSION_V1,
        service_name: "exact_service".parse()?,
        policy_name: "exact_policy".parse()?,
    };
    let manager = Permission::new(CAN_MANAGE_SORACLOUD_PERMISSION.into(), Json::new(()));
    let fhe = Permission::new(
        CAN_GOVERN_SORACLOUD_FHE_PERMISSION.into(),
        Json::new(scope.clone()),
    );
    for permission in [
        fhe.clone(),
        iroha_executor_data_model::permission::role::CanManageRoles.into(),
    ] {
        Grant::account_permission(permission, ALICE_ID.clone())
            .execute(&SAMPLE_GENESIS_ACCOUNT_ID, &mut stx)?;
    }
    let role_id: RoleId = "initial_soracloud_operator".parse()?;
    execute_initial_soracloud(
        Register::role(
            Role::new(role_id.clone(), ALICE_ID.clone())
                .add_permission(manager.clone())
                .add_permission(fhe.clone()),
        ),
        &ALICE_ID,
        &mut stx,
    )?;
    let error = execute_initial_soracloud(
        Grant::account_role(role_id.clone(), CARPENTER_ID.clone()),
        &BOB_ID,
        &mut stx,
    )
    .expect_err("an account cannot delegate a role it does not hold");
    assert!(
        matches!(error, ValidationFail::NotPermitted(_)),
        "{error:?}"
    );
    execute_initial_soracloud(
        Grant::account_role(role_id.clone(), BOB_ID.clone()),
        &ALICE_ID,
        &mut stx,
    )?;
    require_soracloud_permission(&BOB_ID, &stx)?;
    require_soracloud_fhe_governance_permission(
        &BOB_ID,
        &scope.service_name,
        &scope.policy_name,
        &stx,
    )?;
    execute_initial_soracloud(
        Grant::account_permission(manager, CARPENTER_ID.clone()),
        &BOB_ID,
        &mut stx,
    )?;
    let mut other_scope = scope.clone();
    other_scope.policy_name = "other_policy".parse()?;
    let other_fhe = Permission::new(
        CAN_GOVERN_SORACLOUD_FHE_PERMISSION.into(),
        Json::new(other_scope),
    );
    let error = execute_initial_soracloud(
        Grant::account_permission(other_fhe.clone(), CARPENTER_ID.clone()),
        &BOB_ID,
        &mut stx,
    )
    .expect_err("a held FHE scope must not authorize another policy");
    assert!(
        matches!(error, ValidationFail::NotPermitted(_)),
        "{error:?}"
    );
    assert!(
        !stx.world
            .account_permissions_iter(&CARPENTER_ID)?
            .any(|actual| actual == &other_fhe)
    );

    let mut unsupported_scope = scope;
    unsupported_scope.schema_version += 1;
    for (index, permission) in [
        Permission::new(CAN_MANAGE_SORACLOUD_PERMISSION.into(), Json::new(false)),
        Permission::new(CAN_GOVERN_SORACLOUD_FHE_PERMISSION.into(), Json::new(())),
        Permission::new(CAN_GOVERN_SORACLOUD_FHE_PERMISSION.into(), Json::new(unsupported_scope)),
        Permission::new(CAN_GOVERN_SORACLOUD_FHE_PERMISSION.into(),
            Json::from(norito::json!({"schema_version": 1, "service_name": "exact_service", "policy_name": "exact_policy", "extra": true}))),
    ].into_iter().enumerate() {
        let bad_role_id: RoleId = format!("invalid_soracloud_role_{index}").parse()?;
        for instruction in [
            InstructionBox::from(Grant::account_permission(permission.clone(), CARPENTER_ID.clone())),
            InstructionBox::from(Register::role(Role::new(bad_role_id.clone(), ALICE_ID.clone())
                .add_permission(permission.clone()))),
        ] {
            let error = execute_initial_soracloud(instruction, &ALICE_ID, &mut stx)
                .expect_err("malformed Soracloud capabilities must be rejected before mutation");
            assert!(matches!(&error, ValidationFail::NotPermitted(message)
                if message.contains("Invalid permission payload")), "{error:?}");
        }
        assert!(stx.world.roles.get(&bad_role_id).is_none());
        assert!(!stx.world.account_permissions_iter(&CARPENTER_ID)?.any(|actual| actual == &permission));
    }
    execute_initial_soracloud(
        Revoke::account_role(role_id.clone(), BOB_ID.clone()),
        &BOB_ID,
        &mut stx,
    )?;
    require_soracloud_permission(&BOB_ID, &stx)
        .expect_err("revoking membership must immediately remove role-held management authority");
    let error = execute_initial_soracloud(
        Grant::account_role(role_id.clone(), CARPENTER_ID.clone()),
        &BOB_ID,
        &mut stx,
    )
    .expect_err("a former role holder cannot delegate it after revocation");
    assert!(
        matches!(error, ValidationFail::NotPermitted(_)),
        "{error:?}"
    );
    execute_initial_soracloud(
        Revoke::role_permission(fhe.clone(), role_id.clone()),
        &ALICE_ID,
        &mut stx,
    )?;
    assert!(
        !stx.world
            .roles
            .get(&role_id)
            .unwrap()
            .permissions()
            .any(|actual| actual == &fhe)
    );
    execute_initial_soracloud(
        Grant::role_permission(fhe.clone(), role_id.clone()),
        &ALICE_ID,
        &mut stx,
    )?;
    assert!(
        stx.world
            .roles
            .get(&role_id)
            .unwrap()
            .permissions()
            .any(|actual| actual == &fhe)
    );
    execute_initial_soracloud(
        Grant::account_permission(fhe.clone(), CARPENTER_ID.clone()),
        &ALICE_ID,
        &mut stx,
    )?;
    execute_initial_soracloud(
        Revoke::account_permission(fhe.clone(), CARPENTER_ID.clone()),
        &ALICE_ID,
        &mut stx,
    )?;
    assert!(
        !stx.world
            .account_permissions_iter(&CARPENTER_ID)?
            .any(|actual| actual == &fhe)
    );
    Ok(())
}
