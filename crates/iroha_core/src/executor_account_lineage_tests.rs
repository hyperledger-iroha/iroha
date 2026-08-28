#[test]
fn initial_account_lineage_requires_live_explicit_account_id_rekey_provenance() {
    use iroha_data_model::{
        account::{
            AccountAddress,
            rekey::{AccountAlias, AccountRekeyRecord, AccountRekeyTransitionProvenance},
        },
        nexus::{DataSpaceCatalog, DataSpaceId},
        sns::{NameControllerV1, NameRecordV1, NameStatus, NameTombstoneStateV1},
    };
    let retired = checked_account_id();
    let active = checked_account_id();
    let unrelated = checked_account_id();
    let mut world = World::with(
        [],
        [
            Account::new(active.clone()).build(&active),
            Account::new(unrelated.clone()).build(&active),
        ],
        [],
    );
    let alias = AccountAlias::domainless(
        "executor-lineage".parse().expect("alias label"),
        DataSpaceId::UNIVERSAL,
    );
    let selector = crate::sns::selector_for_account_alias(&alias, &DataSpaceCatalog::default())
        .expect("alias selector");
    let address = AccountAddress::from_account_id(&active).expect("active account address");
    let mut lease = NameRecordV1::new(
        selector.clone(),
        active.clone(),
        vec![NameControllerV1::account(&address)],
        0,
        0,
        100,
        200,
        300,
        Metadata::default(),
    );
    let storage_key = crate::sns::record_storage_key(&selector);
    world
        .smart_contract_state_mut_for_testing()
        .insert(storage_key.clone(), lease.encode());
    world.account_aliases.insert(alias.clone(), active.clone());
    let canonical = AccountRekeyRecord::new(alias.clone(), retired.clone())
        .repoint_for_account_id_rekey(active.clone())
        .expect("canonical account-id rekey fixture");
    world.replace_account_rekey_record_for_testing(canonical.clone());
    let state = State::new_for_testing(
        world,
        Kura::blank_kura_for_testing(),
        query::store::LiveQueryStore::start_test(),
    );
    let mut block = state.block(BlockHeader::new(nonzero!(2_u64), None, None, None, 50, 0));
    let mut state_transaction = block.transaction();
    assert!(
        initial_accounts_share_active_lineage(&state_transaction, &retired, &active)
            .expect("lineage check")
    );
    assert!(
        initial_accounts_share_active_lineage(&state_transaction, &active, &retired)
            .expect("reverse lineage check")
    );
    assert!(
        !initial_accounts_share_active_lineage(&state_transaction, &unrelated, &active)
            .expect("unrelated lineage check")
    );
    lease.status = NameStatus::Tombstoned(NameTombstoneStateV1 {
        reason: "revoked".to_owned(),
    });
    state_transaction
        .world
        .smart_contract_state
        .insert(storage_key.clone(), lease.encode());
    assert!(
        !initial_accounts_share_active_lineage(&state_transaction, &retired, &active)
            .expect("revoked lineage check")
    );
    lease.status = NameStatus::Active;
    lease.expires_at_ms = 40;
    lease.grace_expires_at_ms = 45;
    lease.redemption_expires_at_ms = 50;
    state_transaction
        .world
        .smart_contract_state
        .insert(storage_key.clone(), lease.encode());
    assert!(
        !initial_accounts_share_active_lineage(&state_transaction, &retired, &active)
            .expect("stale lineage check")
    );
    lease.expires_at_ms = 100;
    lease.grace_expires_at_ms = 200;
    lease.redemption_expires_at_ms = 300;
    state_transaction
        .world
        .smart_contract_state
        .insert(storage_key, lease.encode());
    state_transaction.world.replace_account_rekey_record(
        AccountRekeyRecord::new(alias.clone(), retired.clone())
            .reassign_alias_to_account(active.clone())
            .expect("alias reassignment fixture"),
    );
    assert!(
        !initial_accounts_share_active_lineage(&state_transaction, &retired, &active)
            .expect("alias reassignment lineage check")
    );
    let mut cyclic = canonical;
    cyclic.previous_account_ids.push(active.clone());
    cyclic
        .transition_provenance
        .push(AccountRekeyTransitionProvenance::AccountIdRekey);
    state_transaction.world.replace_account_rekey_record(cyclic);
    assert!(
        !initial_accounts_share_active_lineage(&state_transaction, &retired, &active)
            .expect("malformed lineage check")
    );
}
