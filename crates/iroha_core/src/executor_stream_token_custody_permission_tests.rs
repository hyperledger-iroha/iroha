/// Stream-token custody uses a dedicated provider-scoped native permission.
mod stream_token_custody_permission_tests {
    use super::*;
    use crate::executor::Executor;
    use iroha_data_model::{
        isi::sorafs::MutateSorafsStreamTokenCustody,
        sorafs::{
            capacity::ProviderId,
            stream_token_custody::{
                SorafsStreamTokenCustodyActionV1, SorafsStreamTokenCustodyRevocationV1,
            },
        },
    };
    use iroha_executor_data_model::permission::sorafs::CanManageSorafsStreamTokenCustody;

    fn permission(byte: u8) -> Permission {
        CanManageSorafsStreamTokenCustody {
            provider_id: ProviderId::new([byte; 32]),
        }
        .into()
    }
    fn mutation() -> InstructionBox {
        MutateSorafsStreamTokenCustody {
            provider_id: ProviderId::new([1; 32]),
            expected_revision: 1,
            expected_digest: [7; 32],
            action: SorafsStreamTokenCustodyActionV1::Revoke(
                SorafsStreamTokenCustodyRevocationV1 {
                    signer: true,
                    attester: false,
                },
            ),
        }
        .into()
    }
    fn fixture() -> State {
        let mut world = World::with(
            [],
            [
                Account::new(ALICE_ID.clone()).build(&ALICE_ID),
                Account::new(BOB_ID.clone()).build(&BOB_ID),
            ],
            [],
        );
        world
            .account_permissions
            .insert(ALICE_ID.clone(), BTreeSet::from([permission(1)]));
        world.account_permissions.insert(
            BOB_ID.clone(),
            BTreeSet::from([
                permission(2),
                executor_permission::sorafs::CanSetSorafsPricing.into(),
            ]),
        );
        world
            .provider_owners
            .insert(ProviderId::new([1; 32]), BOB_ID.clone());
        state_after_genesis(world)
    }

    #[test]
    fn native_stream_token_custody_requires_exact_provider_permission_even_at_genesis() {
        let state = fixture();
        let mut block = state.block(BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0));
        let transaction = block.transaction();
        let instruction = mutation();
        assert!(initial_native_instruction_is_explicitly_admitted(
            &instruction
        ));
        assert!(is_builtin_initial_permission_name(
            "CanManageSorafsStreamTokenCustody"
        ));
        for genesis in [false, true] {
            assert!(
                validate_initial_native_instruction_authority(
                    &transaction,
                    &ALICE_ID,
                    &instruction,
                    genesis
                )
                .is_ok()
            );
            assert!(
                validate_initial_native_instruction_authority(
                    &transaction,
                    &BOB_ID,
                    &instruction,
                    genesis
                )
                .is_err(),
                "provider ownership, another scope and pricing authority do not confer custody"
            );
        }
        let malformed = Permission::new(
            "CanManageSorafsStreamTokenCustody".to_owned(),
            Json::new(()),
        );
        assert!(validate_initial_permission_payload_constraints(&malformed).is_err());
        assert!(normalize_role_permission_for_initial_executor(&transaction, &malformed).is_err());
        assert_eq!(
            normalize_role_permission_for_initial_executor(&transaction, &permission(1))
                .expect("catalog knows scoped token"),
            permission(1)
        );
    }

    #[test]
    fn native_stream_token_custody_direct_and_role_delegation_preserve_exact_scope() {
        let state = fixture();
        let mut block = state.block(BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0));
        let mut transaction = block.transaction();
        let role: RoleId = "stream_token_custody_operator".parse().expect("role id");
        Register::role(Role::new(role.clone(), ALICE_ID.clone()).add_permission(permission(1)))
            .execute(&ALICE_ID, &mut transaction)
            .expect("seed dedicated role");
        transaction
            .world
            .account_permissions
            .insert(ALICE_ID.clone(), BTreeSet::new());
        assert!(authority_has_role(&transaction.world, &ALICE_ID, &role));
        assert!(
            validate_initial_native_instruction_authority(
                &transaction,
                &ALICE_ID,
                &mutation(),
                false
            )
            .is_ok(),
            "assigned role confers exact custody permission"
        );
        for is_revoke in [false, true] {
            let exact: InstructionBox = if is_revoke {
                Revoke::account_permission(permission(1), BOB_ID.clone()).into()
            } else {
                Grant::account_permission(permission(1), BOB_ID.clone()).into()
            };
            Executor::Initial
                .execute_instruction(&mut transaction, &ALICE_ID, exact)
                .expect("exact role holder delegates or revokes");
            assert_eq!(
                validate_initial_native_instruction_authority(
                    &transaction,
                    &BOB_ID,
                    &mutation(),
                    false
                )
                .is_ok(),
                !is_revoke
            );
            let other: InstructionBox = if is_revoke {
                Revoke::role_permission(permission(2), role.clone()).into()
            } else {
                Grant::role_permission(permission(2), role.clone()).into()
            };
            assert!(
                validate_initial_permission_or_role_mutation(
                    &transaction,
                    &ALICE_ID,
                    &other,
                    false,
                    None
                )
                .is_err(),
                "role ownership cannot expand provider scope"
            );
        }
        for is_revoke in [false, true] {
            let change: InstructionBox = if is_revoke {
                Revoke::account_role(role.clone(), BOB_ID.clone()).into()
            } else {
                Grant::account_role(role.clone(), BOB_ID.clone()).into()
            };
            Executor::Initial
                .execute_instruction(&mut transaction, &ALICE_ID, change)
                .expect("exact role holder manages assigned role");
            assert_eq!(
                validate_initial_native_instruction_authority(
                    &transaction,
                    &BOB_ID,
                    &mutation(),
                    false
                )
                .is_ok(),
                !is_revoke
            );
        }
        let malformed = Permission::new(
            "CanManageSorafsStreamTokenCustody".to_owned(),
            Json::new(()),
        );
        for genesis in [false, true] {
            let invalid: InstructionBox =
                Grant::account_permission(malformed.clone(), BOB_ID.clone()).into();
            assert!(
                validate_initial_permission_or_role_mutation(
                    &transaction,
                    &ALICE_ID,
                    &invalid,
                    genesis,
                    None
                )
                .is_err()
            );
        }
    }
}
