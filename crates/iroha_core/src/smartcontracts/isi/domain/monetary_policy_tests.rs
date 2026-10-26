// Original and pending monetary references use real unregister execution and MV rollback.

fn monetary_account_parameter(
    role: &str,
    account: &AccountId,
) -> iroha_data_model::parameter::Parameter {
    let custom = match role {
        "committed root fee sink"
        | "committed sponsor vault custody"
        | "committed successful-claim exemption authority" => {
            let mut policy = super::monetary_test_support::fees();
            match role {
                "committed root fee sink" => policy.fee_sink_account_id = account.clone(),
                "committed sponsor vault custody" => {
                    policy.sponsor_vault_custody_account_id = account.clone();
                }
                _ => {
                    policy
                        .successful_claim_fee_exempt_authorities
                        .try_insert(account.clone())
                        .unwrap();
                }
            }
            policy.into_custom_parameter().unwrap()
        }
        "committed staking escrow account" | "committed staking slash sink account" => {
            let mut policy = super::monetary_test_support::staking();
            if role == "committed staking escrow account" {
                policy.stake_escrow_account_id = account.clone();
            } else {
                policy.slash_sink_account_id = account.clone();
            }
            policy.into_custom_parameter().unwrap()
        }
        _ => panic!("unknown fixture role"),
    };
    iroha_data_model::parameter::Parameter::Custom(custom)
}

const MONETARY_ACCOUNT_ROLES: [&str; 5] = [
    "committed root fee sink",
    "committed sponsor vault custody",
    "committed successful-claim exemption authority",
    "committed staking escrow account",
    "committed staking slash sink account",
];

#[test]
fn unregister_account_checks_original_and_pending_monetary_references_atomically() {
    for role in MONETARY_ACCOUNT_ROLES {
        for original in [true, false] {
            let mut state = test_state();
            let domain = DomainId::try_new("monetary-account", "world").unwrap();
            seed_domain(&mut state, &domain, &ALICE_ID);
            let target = AccountId::new(checked_keypair().public_key().clone());
            seed_account(&mut state, &target, &domain);
            if original {
                state
                    .world
                    .set_parameter_for_testing(monetary_account_parameter(role, &target))
                    .unwrap();
            }
            let mut block = state.block(BlockHeader::new(nonzero!(1_u64), None, None, 0, 0));
            {
                let mut tx = block.transaction();
                // The inverse replacement proves which exact cut retains the reference.
                tx.world
                    .set_parameter_for_testing(monetary_account_parameter(
                        role,
                        if original { &ALICE_ID } else { &target },
                    ))
                    .unwrap();
                let account_before = tx.world.accounts.get(&target).cloned();
                let events_before = tx.world.internal_event_buf.clone();
                let error = Unregister::account(target.clone())
                    .execute(&ALICE_ID, &mut tx)
                    .expect_err("both parameter cuts retain referenced accounts");
                let diagnostic = error.to_string();
                assert!(diagnostic.contains(role), "{diagnostic}");
                assert!(
                    diagnostic.contains(if original {
                        "in the original parameter cut"
                    } else {
                        "in the pending/current parameter cut"
                    }),
                    "{diagnostic}"
                );
                assert_eq!(tx.world.accounts.get(&target), account_before.as_ref());
                assert_eq!(tx.world.internal_event_buf, events_before);
            }
            // Dropping the rejected transaction rolls its parameter write back too.
            let mut sibling = block.transaction();
            let result = Unregister::account(target.clone()).execute(&ALICE_ID, &mut sibling);
            if original {
                assert!(
                    result
                        .unwrap_err()
                        .to_string()
                        .contains("in the original parameter cut")
                );
                assert!(sibling.world.accounts.get(&target).is_some());
            } else {
                result.expect("unapplied candidate policy must not escape to a sibling");
                assert!(sibling.world.accounts.get(&target).is_none());
            }
        }
    }
}

#[test]
fn unregister_account_checks_applied_sibling_monetary_references() {
    for role in MONETARY_ACCOUNT_ROLES {
        let mut state = test_state();
        let domain = DomainId::try_new("monetary-sibling", "world").unwrap();
        seed_domain(&mut state, &domain, &ALICE_ID);
        let target = AccountId::new(checked_keypair().public_key().clone());
        seed_account(&mut state, &target, &domain);
        let mut block = state.block(BlockHeader::new(nonzero!(1_u64), None, None, 0, 0));
        let mut first = block.transaction();
        first
            .world
            .set_parameter_for_testing(monetary_account_parameter(role, &target))
            .unwrap();
        first.apply();
        let mut sibling = block.transaction();
        let events_before = sibling.world.internal_event_buf.clone();
        let error = Unregister::account(target.clone())
            .execute(&ALICE_ID, &mut sibling)
            .unwrap_err();
        assert!(error.to_string().contains(role), "{error}");
        assert!(
            error.to_string().contains("pending/current parameter cut"),
            "{error}"
        );
        assert!(sibling.world.accounts.get(&target).is_some());
        assert_eq!(sibling.world.internal_event_buf, events_before);
    }
}

fn monetary_asset_parameter(
    fee: bool,
    definition: &AssetDefinitionId,
) -> iroha_data_model::parameter::Parameter {
    let custom = if fee {
        let mut policy = super::monetary_test_support::fees();
        policy.fee_asset_id = definition.clone();
        policy.into_custom_parameter().unwrap()
    } else {
        let mut policy = super::monetary_test_support::staking();
        policy.stake_asset_id = definition.clone();
        policy.into_custom_parameter().unwrap()
    };
    iroha_data_model::parameter::Parameter::Custom(custom)
}

#[test]
fn unregister_asset_checks_original_and_pending_monetary_currencies_atomically() {
    for fee in [true, false] {
        for original in [true, false] {
            let mut state = test_state();
            let domain = DomainId::try_new("monetary-asset", "world").unwrap();
            seed_domain(&mut state, &domain, &ALICE_ID);
            let definition =
                AssetDefinitionId::derive_from_components(domain, "coin".parse().unwrap());
            if original {
                state
                    .world
                    .set_parameter_for_testing(monetary_asset_parameter(fee, &definition))
                    .unwrap();
            }
            let mut block = state.block(BlockHeader::new(nonzero!(1_u64), None, None, 0, 0));
            let mut registration = block.transaction();
            Register::asset_definition(AssetDefinition::numeric(
                definition.clone(),
                "coin".to_owned(),
                AssetBalancePolicy::Global,
                None,
            ))
            .execute(&ALICE_ID, &mut registration)
            .unwrap();
            registration.apply();
            {
                let mut tx = block.transaction();
                let replacement_asset = if original {
                    super::monetary_test_support::asset()
                } else {
                    definition.clone()
                };
                tx.world
                    .set_parameter_for_testing(monetary_asset_parameter(fee, &replacement_asset))
                    .unwrap();
                let before = tx.world.asset_definitions.get(&definition).cloned();
                let events_before = tx.world.internal_event_buf.clone();
                let error = Unregister::asset_definition(definition.clone())
                    .execute(&ALICE_ID, &mut tx)
                    .unwrap_err();
                assert!(
                    error.to_string().contains(if fee {
                        "immutable committed root fee currency"
                    } else {
                        "committed staking asset definition"
                    }),
                    "{error}"
                );
                assert!(
                    error.to_string().contains(if original {
                        "in the original parameter cut"
                    } else {
                        "in the pending/current parameter cut"
                    }),
                    "{error}"
                );
                assert_eq!(tx.world.asset_definitions.get(&definition), before.as_ref());
                assert_eq!(tx.world.internal_event_buf, events_before);
            }
            let mut sibling = block.transaction();
            let result =
                Unregister::asset_definition(definition.clone()).execute(&ALICE_ID, &mut sibling);
            if original {
                assert!(
                    result
                        .unwrap_err()
                        .to_string()
                        .contains("original parameter cut")
                );
                assert!(sibling.world.asset_definitions.get(&definition).is_some());
            } else {
                result.expect("dropped pending currency cannot block an unrelated retirement");
                assert!(sibling.world.asset_definitions.get(&definition).is_none());
            }
        }
    }
}

#[test]
fn unregister_domain_checks_original_pending_and_sibling_currencies_before_any_mutation() {
    for fee in [true, false] {
        for original in [true, false] {
            let mut state = test_state();
            let domain = DomainId::try_new("monetary-cascade", "universal").unwrap();
            seed_domain(&mut state, &domain, &ALICE_ID);
            seed_account(&mut state, &ALICE_ID, &domain);
            let definition =
                AssetDefinitionId::derive_from_components(domain.clone(), "coin".parse().unwrap());
            if original {
                state
                    .world
                    .set_parameter_for_testing(monetary_asset_parameter(fee, &definition))
                    .unwrap();
            }
            let mut block = state.block(BlockHeader::new(nonzero!(1_u64), None, None, 0, 0));
            let mut registration = block.transaction();
            Register::asset_definition(AssetDefinition::numeric(
                definition.clone(),
                "coin".to_owned(),
                AssetBalancePolicy::Global,
                Some(domain.clone()),
            ))
            .execute(&ALICE_ID, &mut registration)
            .unwrap();
            let asset = AssetId::new(definition.clone(), ALICE_ID.clone());
            let quantity = Quantity::from(7_u32);
            let (_, value) = Asset::new(asset.clone(), quantity.clone()).into_key_value();
            registration.world.assets.insert(asset.clone(), value);
            registration.world.track_asset_holder(&asset);
            registration
                .world
                .increase_asset_total_amount(&definition, &quantity)
                .unwrap();
            let permission: Permission =
                iroha_executor_data_model::permission::domain::CanUnregisterDomain {
                    domain: domain.clone(),
                }
                .into();
            registration
                .world
                .account_permissions
                .insert(ALICE_ID.clone(), [permission.clone()].into_iter().collect());
            registration.apply();
            {
                let mut tx = block.transaction();
                let replacement_asset = if original {
                    super::monetary_test_support::asset()
                } else {
                    definition.clone()
                };
                tx.world
                    .set_parameter_for_testing(monetary_asset_parameter(fee, &replacement_asset))
                    .unwrap();
                let domain_before = tx.world.domains.get(&domain).cloned();
                let definition_before = tx.world.asset_definitions.get(&definition).cloned();
                let index_before = tx
                    .world
                    .domain_asset_definitions
                    .get(&(domain.clone(), definition.clone()))
                    .cloned();
                let balance_before = tx.world.assets.get(&asset).cloned();
                let total_before = tx.world.asset_total_amount(&definition).unwrap();
                let permissions_before = tx.world.account_permissions.get(&ALICE_ID).cloned();
                let events_before = tx.world.internal_event_buf.clone();
                let error = Unregister::domain(domain.clone())
                    .execute(&ALICE_ID, &mut tx)
                    .unwrap_err();
                assert!(
                    error.to_string().contains(if fee {
                        "immutable committed root fee currency"
                    } else {
                        "committed staking asset definition"
                    }),
                    "{error}"
                );
                assert!(
                    error.to_string().contains(if original {
                        "in the original parameter cut"
                    } else {
                        "in the pending/current parameter cut"
                    }),
                    "{error}"
                );
                assert_eq!(tx.world.domains.get(&domain), domain_before.as_ref());
                assert_eq!(
                    tx.world.asset_definitions.get(&definition),
                    definition_before.as_ref()
                );
                assert_eq!(
                    tx.world
                        .domain_asset_definitions
                        .get(&(domain.clone(), definition.clone())),
                    index_before.as_ref()
                );
                assert_eq!(tx.world.assets.get(&asset), balance_before.as_ref());
                assert_eq!(
                    tx.world.asset_total_amount(&definition).unwrap(),
                    total_before
                );
                assert_eq!(
                    tx.world.account_permissions.get(&ALICE_ID),
                    permissions_before.as_ref()
                );
                assert_eq!(tx.world.internal_event_buf, events_before);
            }
            let mut sibling = block.transaction();
            if original {
                let error = Unregister::domain(domain.clone())
                    .execute(&ALICE_ID, &mut sibling)
                    .unwrap_err();
                assert!(error.to_string().contains("original parameter cut"));
            } else {
                Unregister::domain(domain.clone())
                    .execute(&ALICE_ID, &mut sibling)
                    .expect("unrelated domain cascade succeeds after pending policy rollback");
                assert!(sibling.world.domains.get(&domain).is_none());
                assert!(sibling.world.asset_definitions.get(&definition).is_none());
            }
        }
    }
}

#[test]
fn transaction_local_monetary_initialization_cannot_replace_missing_original_policy() {
    let mut state = State::new_for_testing(
        World::default(),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    let domain = DomainId::try_new("missing-monetary", "world").unwrap();
    seed_domain(&mut state, &domain, &ALICE_ID);
    let target = AccountId::new(checked_keypair().public_key().clone());
    seed_account(&mut state, &target, &domain);
    let mut block = state.block(BlockHeader::new(nonzero!(1_u64), None, None, 0, 0));
    let mut tx = block.transaction();
    tx.world
        .set_parameter_for_testing(iroha_data_model::parameter::Parameter::Custom(
            super::monetary_test_support::fees()
                .into_custom_parameter()
                .unwrap(),
        ))
        .unwrap();
    tx.world
        .set_parameter_for_testing(iroha_data_model::parameter::Parameter::Custom(
            super::monetary_test_support::staking()
                .into_custom_parameter()
                .unwrap(),
        ))
        .unwrap();
    let before = tx.world.accounts.get(&target).cloned();
    let events_before = tx.world.internal_event_buf.clone();
    let error = Unregister::account(target.clone())
        .execute(&ALICE_ID, &mut tx)
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("original committed root fee policy is absent"),
        "{error}"
    );
    assert_eq!(tx.world.accounts.get(&target), before.as_ref());
    assert_eq!(tx.world.internal_event_buf, events_before);
}

#[test]
fn applied_sibling_currency_protects_individual_and_domain_retirement() {
    for fee in [true, false] {
        let mut state = test_state();
        let domain = DomainId::try_new("applied-currency", "universal").unwrap();
        seed_domain(&mut state, &domain, &ALICE_ID);
        let definition =
            AssetDefinitionId::derive_from_components(domain.clone(), "coin".parse().unwrap());
        let mut block = state.block(BlockHeader::new(nonzero!(1_u64), None, None, 0, 0));
        let mut first = block.transaction();
        Register::asset_definition(AssetDefinition::numeric(
            definition.clone(),
            "coin".to_owned(),
            AssetBalancePolicy::Global,
            Some(domain.clone()),
        ))
        .execute(&ALICE_ID, &mut first)
        .unwrap();
        first
            .world
            .set_parameter_for_testing(monetary_asset_parameter(fee, &definition))
            .unwrap();
        first.apply();
        let mut sibling = block.transaction();
        let events_before = sibling.world.internal_event_buf.clone();
        for error in [
            Unregister::asset_definition(definition.clone())
                .execute(&ALICE_ID, &mut sibling)
                .unwrap_err(),
            Unregister::domain(domain.clone())
                .execute(&ALICE_ID, &mut sibling)
                .unwrap_err(),
        ] {
            assert!(
                error.to_string().contains("pending/current parameter cut"),
                "{error}"
            );
            assert!(
                error.to_string().contains(if fee {
                    "immutable committed root fee currency"
                } else {
                    "committed staking asset definition"
                }),
                "{error}"
            );
        }
        assert!(sibling.world.domains.get(&domain).is_some());
        assert!(sibling.world.asset_definitions.get(&definition).is_some());
        assert_eq!(sibling.world.internal_event_buf, events_before);
    }
}

#[test]
fn transaction_local_staking_initialization_cannot_replace_missing_original_policy() {
    let mut state = State::new_for_testing(
        World::default(),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    state
        .world
        .set_parameter_for_testing(iroha_data_model::parameter::Parameter::Custom(
            super::monetary_test_support::fees()
                .into_custom_parameter()
                .unwrap(),
        ))
        .unwrap();
    let domain = DomainId::try_new("missing-staking", "world").unwrap();
    seed_domain(&mut state, &domain, &ALICE_ID);
    let target = AccountId::new(checked_keypair().public_key().clone());
    seed_account(&mut state, &target, &domain);
    let mut block = state.block(BlockHeader::new(nonzero!(1_u64), None, None, 0, 0));
    let mut tx = block.transaction();
    tx.world
        .set_parameter_for_testing(iroha_data_model::parameter::Parameter::Custom(
            super::monetary_test_support::staking()
                .into_custom_parameter()
                .unwrap(),
        ))
        .unwrap();
    let before = tx.world.accounts.get(&target).cloned();
    let events_before = tx.world.internal_event_buf.clone();
    let error = Unregister::account(target.clone())
        .execute(&ALICE_ID, &mut tx)
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("missing committed public-lane staking policy"),
        "{error}"
    );
    assert_eq!(tx.world.accounts.get(&target), before.as_ref());
    assert_eq!(tx.world.internal_event_buf, events_before);
}
