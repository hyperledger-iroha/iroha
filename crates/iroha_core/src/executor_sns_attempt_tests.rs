// Original authoritative SNS bytes must retain operational identity through permission reads.

fn sns_permission_original_world() -> (World, AccountId, AccountId, StatePath, Vec<u8>) {
    use iroha_data_model::{
        account::{
            AccountAddress,
            rekey::{AccountAlias, AccountAliasDomain, AccountRekeyRecord},
        },
        sns::{NameControllerV1, NameRecordV1},
    };
    use norito::codec::Encode as _;
    let owner = ALICE_ID.clone();
    let subject = checked_account_id();
    let domain = DomainId::try_new("fi", "universal").unwrap();
    let mut world = World::with(
        [Domain::new(domain).build(&owner)],
        [
            Account::new(owner.clone()).build(&owner),
            Account::new(subject.clone()).build(&subject),
        ],
        [],
    );
    let alias = AccountAlias::new(
        "customer".parse().unwrap(),
        Some(AccountAliasDomain::new("fi".parse().unwrap())),
        DataSpaceId::UNIVERSAL,
    );
    let selector =
        crate::sns::selector_for_account_alias(&alias, world.view().dataspace_catalog()).unwrap();
    let record = NameRecordV1::new(
        selector.clone(),
        subject.clone(),
        vec![NameControllerV1::account(
            &AccountAddress::from_account_id(&subject).unwrap(),
        )],
        0,
        0,
        100,
        200,
        300,
        Metadata::default(),
    );
    let key = crate::sns::record_storage_key(&selector);
    let original = record.encode();
    world
        .smart_contract_state_mut_for_testing()
        .insert(key.clone(), original.clone());
    world.replace_account_rekey_record_for_testing(AccountRekeyRecord::new(
        alias.clone(),
        subject.clone(),
    ));
    world.account_aliases.insert(alias.clone(), subject.clone());
    world
        .account_aliases_by_account
        .insert(subject.clone(), BTreeSet::from([alias]));
    (world, owner, subject, key, original)
}

#[test]
fn original_sns_alias_domain_permission_refusal_retries_without_a_rejection() {
    use crate::execution_attempt::ExecutionAttemptError;
    let (world, owner, subject, key, original) = sns_permission_original_world();
    assert!(authority_owns_any_alias_domain(&world.view(), &owner, &subject, 50).unwrap());
    let refusal = norito::with_decode_limits_scope(
        norito::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, 0, 32),
        || authority_owns_any_alias_domain(&world.view(), &owner, &subject, 50).unwrap_err(),
    );
    assert!(
        matches!(refusal, ExecutionAttemptError::Deferred(ref reason) if reason.reason() == ivm::error::ExecutionDeferral::ActiveMemoryCapacity),
        "original local SNS refusal was erased: {refusal:?}"
    );
    assert_eq!(world.smart_contract_state.view().get(&key), Some(&original));
    assert!(authority_owns_any_alias_domain(&world.view(), &owner, &subject, 50).unwrap());
}

#[test]
fn original_sns_domain_transfer_permission_refusal_retries_without_a_rejection() {
    use crate::execution_attempt::ExecutionAttemptError;
    let (mut world, owner, subject, key, original) = sns_permission_original_world();
    let target = DomainId::try_new("target", "universal").unwrap();
    // Direct ownership cannot cover this transfer; the valid fi alias must do so.
    world
        .domains
        .insert(target.clone(), Domain::new(target.clone()).build(&subject));
    let destination = checked_account_id();
    let transfer = Transfer::domain(subject, target, destination);
    assert!(can_transfer_domain(&world.view(), &owner, &transfer, 50).unwrap());
    let refusal = norito::with_decode_limits_scope(
        norito::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, 0, 32),
        || can_transfer_domain(&world.view(), &owner, &transfer, 50).unwrap_err(),
    );
    assert!(
        matches!(refusal, ExecutionAttemptError::Deferred(ref reason) if reason.reason() == ivm::error::ExecutionDeferral::ActiveMemoryCapacity),
        "original local SNS refusal was erased: {refusal:?}"
    );
    assert_eq!(world.smart_contract_state.view().get(&key), Some(&original));
    assert!(can_transfer_domain(&world.view(), &owner, &transfer, 50).unwrap());
}
