// Exercise the production Initial dispatcher without genesis-only admission.
fn initial_sorafs_block_header() -> iroha_data_model::block::BlockHeader {
    iroha_data_model::block::BlockHeader::new(nonzero!(2_u64), None, None, None, 5_000, 0)
}

fn execute_initial_sorafs(
    instruction: impl Into<InstructionBox>,
    authority: &AccountId,
    state_transaction: &mut StateTransaction<'_, '_>,
) -> Result<(), InstructionExecutionError> {
    match crate::executor::Executor::Initial.execute_instruction(
        state_transaction,
        authority,
        instruction.into(),
    ) {
        Ok(()) => Ok(()),
        Err(iroha_data_model::executor::ValidationFail::InstructionFailed(error)) => Err(error),
        Err(error) => {
            panic!("reviewed SoraFS instruction must reach Core authorization: {error:?}")
        }
    }
}

trait ExecuteInitialSorafs: Into<InstructionBox> + Sized {
    fn execute_initial(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), InstructionExecutionError> {
        execute_initial_sorafs(self, authority, state_transaction)
    }
}

impl<T: Into<InstructionBox>> ExecuteInitialSorafs for T {}

#[test]
fn initial_executor_sorafs_unimplemented_citizen_bonds_are_closed_at_both_dispatch_boundaries() {
    use iroha_data_model::{
        isi::sorafs::{
            RegisterSorafsCitizenBond, RequestSorafsCitizenBondExit,
            RotateSorafsCitizenBondAuthorization,
        },
        sorafs::anonymity::{
            SORAFS_CITIZEN_BOND_VERSION_V1, SorafsCitizenBondStateV1, SorafsCitizenBondV1,
        },
    };

    let state = make_state();
    let mut block = state.block(initial_sorafs_block_header());
    let mut stx = block.transaction();
    let bond = SorafsCitizenBondV1 {
        version: SORAFS_CITIZEN_BOND_VERSION_V1,
        serial_commitment: [0x41; 32],
        authorization_commitment: [0x42; 32],
        authorization_revision: 1,
        locked_value_commitment: [0x43; 32],
        bond_asset: stx.gov.sorafs_pin_fee_asset_id.clone(),
        bond_atomic_units: 10_000,
        frozen_policy_root: [0x44; 32],
        bonded_at_height: 100,
        exit_delay_blocks: 300,
        state: SorafsCitizenBondStateV1::Active,
    };
    let instructions = [
        InstructionBox::from(RegisterSorafsCitizenBond { bond }),
        InstructionBox::from(RotateSorafsCitizenBondAuthorization {
            serial_commitment: [0x41; 32],
            expected_authorization_commitment: [0x42; 32],
            expected_revision: 1,
            next_authorization_commitment: [0x45; 32],
        }),
        InstructionBox::from(RequestSorafsCitizenBondExit {
            serial_commitment: [0x41; 32],
            expected_authorization_commitment: [0x42; 32],
            expected_revision: 1,
        }),
    ];
    let assets_before = stx
        .world
        .assets
        .iter()
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect::<Vec<_>>();
    let contract_state_before = stx
        .world
        .smart_contract_state
        .iter()
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect::<Vec<_>>();
    for instruction in instructions {
        let expected = crate::smartcontracts::isi::INITIAL_NATIVE_INSTRUCTION_CLOSED_REASON;
        let error = crate::executor::Executor::Initial
            .execute_instruction(&mut stx, &alice(), instruction.clone())
            .expect_err("Initial must reject unavailable citizen-bond semantics");
        assert!(
            matches!(error, iroha_data_model::executor::ValidationFail::NotPermitted(message)
            if message == expected)
        );
        let error = crate::smartcontracts::isi::execute_borrowed_instruction(
            &instruction,
            &alice(),
            &mut stx,
        )
        .expect_err("native dispatch must also reject unavailable citizen-bond semantics");
        assert!(
            matches!(error, InstructionExecutionError::InvariantViolation(message)
            if message.as_ref() == expected)
        );
        assert_eq!(
            stx.world
                .assets
                .iter()
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect::<Vec<_>>(),
            assets_before
        );
        assert_eq!(
            stx.world
                .smart_contract_state
                .iter()
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect::<Vec<_>>(),
            contract_state_before
        );
    }
}

#[test]
fn initial_executor_sorafs_direct_provider_owner_instructions_remain_closed() {
    use iroha_data_model::isi::Instruction as _;
    let state = make_state();
    let mut block = state.block(initial_sorafs_block_header());
    let mut stx = block.transaction();
    let provider = ProviderId::new([0xA1; 32]);
    stx.world.provider_owners.insert(provider, alice());
    for instruction in [
        Box::new(RegisterProviderOwner {
            provider_id: provider,
            owner: bob(),
        })
        .into_instruction_box(),
        Box::new(UnregisterProviderOwner {
            provider_id: provider,
        })
        .into_instruction_box(),
    ] {
        let error = crate::executor::Executor::Initial
            .execute_instruction(&mut stx, &alice(), instruction)
            .expect_err("retired direct owner changes remain closed even for permission holders");
        assert!(
            matches!(error, iroha_data_model::executor::ValidationFail::NotPermitted(message)
            if message.contains("explicitly closed"))
        );
        assert_eq!(stx.world.provider_owners.get(&provider), Some(&alice()));
    }
}

#[test]
fn initial_executor_sorafs_role_grant_use_and_revoke_are_exact() {
    use iroha_data_model::{
        isi::{Grant, Revoke},
        role::{Role, RoleId},
    };
    use iroha_executor_data_model::permission::sorafs::CanSetSorafsPricing;

    let mut state = make_state();
    let role_id: RoleId = "sorafs_pricing_operator".parse().expect("role ID");
    let role = Role::new(role_id.clone(), alice())
        .add_permission(Permission::from(CanSetSorafsPricing))
        .build(&alice());
    state.world.roles.insert(role_id.clone(), role);
    state.world.account_roles.insert(
        crate::role::RoleIdWithOwner::new(alice(), role_id.clone()),
        (),
    );
    let mut block = state.block(initial_sorafs_block_header());
    let mut stx = block.transaction();
    let pricing = SetPricingSchedule {
        schedule: PricingScheduleRecord::launch_default(),
    };
    execute_initial_sorafs(pricing.clone(), &bob(), &mut stx)
        .expect_err("unprivileged account cannot set pricing");
    crate::executor::Executor::Initial
        .execute_instruction(
            &mut stx,
            &alice(),
            Grant::account_role(role_id.clone(), bob()).into(),
        )
        .expect("an exact current role holder may delegate the role");
    assert!(
        !stx.world
            .account_permissions
            .get(&bob())
            .is_some_and(|permissions| {
                permissions.contains(&Permission::from(CanSetSorafsPricing))
            }),
        "role grants must work without materializing direct permission copies"
    );
    execute_initial_sorafs(pricing.clone(), &bob(), &mut stx)
        .expect("the granted exact role authorizes the Core operation");
    crate::executor::Executor::Initial
        .execute_instruction(
            &mut stx,
            &alice(),
            Revoke::account_role(role_id, bob()).into(),
        )
        .expect("an exact current role holder may revoke the role");
    let error = execute_initial_sorafs(pricing, &bob(), &mut stx)
        .expect_err("revoking the role immediately removes its authorization");
    assert!(smart_contract_error_message(&error).contains("CanSetSorafsPricing"));
}

#[test]
fn initial_executor_sorafs_rejects_malformed_unit_and_foreign_role_permissions() {
    use iroha_data_model::{
        isi::Grant,
        role::{Role, RoleId},
    };
    use iroha_executor_data_model::permission::sorafs::{CanBindSorafsAlias, CanSetSorafsPricing};

    let pricing_permission = Permission::from(CanSetSorafsPricing);
    for (index, permission) in [
        Permission::new(pricing_permission.name().to_owned(), Json::new("null")),
        Permission::new(pricing_permission.name().to_owned(), Json::new(false)),
        Permission::new(pricing_permission.name().to_owned(), Json::new(42_u32)),
        Permission::new(
            pricing_permission.name().to_owned(),
            Json::from_raw_json("{}".to_owned()).unwrap(),
        ),
        Permission::from(CanBindSorafsAlias),
    ]
    .into_iter()
    .enumerate()
    {
        let mut state = make_state();
        let role_id: RoleId = format!("sorafs_invalid_pricing_{index}").parse().unwrap();
        let role = Role::new(role_id.clone(), alice())
            .add_permission(permission.clone())
            .build(&alice());
        state.world.roles.insert(role_id.clone(), role);
        for account in [alice(), bob()] {
            state.world.account_roles.insert(
                crate::role::RoleIdWithOwner::new(account, role_id.clone()),
                (),
            );
        }
        let mut block = state.block(initial_sorafs_block_header());
        let mut stx = block.transaction();
        // Invalid retained role payloads cannot authorize an operation even before
        // a new grant boundary is reached. A different valid permission is no substitute.
        let error = execute_initial_sorafs(
            SetPricingSchedule {
                schedule: PricingScheduleRecord::launch_default(),
            },
            &bob(),
            &mut stx,
        )
        .expect_err("only the exact pricing unit permission authorizes pricing");
        assert!(smart_contract_error_message(&error).contains("CanSetSorafsPricing"));
        if permission.name() == pricing_permission.name() {
            let error = crate::executor::Executor::Initial
                .execute_instruction(
                    &mut stx,
                    &alice(),
                    Grant::account_permission(permission.clone(), bob()).into(),
                )
                .expect_err("malformed unit permission must fail at the Initial grant boundary");
            assert!(
                matches!(error, iroha_data_model::executor::ValidationFail::NotPermitted(message)
                if message.contains("Invalid permission payload"))
            );
            let error = crate::executor::Executor::Initial
                .execute_instruction(
                    &mut stx,
                    &alice(),
                    Grant::account_role(role_id, bob()).into(),
                )
                .expect_err("malformed role permission must fail before the Core grant handler");
            assert!(
                matches!(error, iroha_data_model::executor::ValidationFail::NotPermitted(message)
                if message.contains("Invalid permission payload"))
            );
        }
    }
}
