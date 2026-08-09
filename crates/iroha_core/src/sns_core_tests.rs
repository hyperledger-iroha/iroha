// Same-scope regression coverage extracted to keep the parent source budget bounded.

#[test]
fn default_namespace_lease_price_is_exact() {
    assert_eq!(default_namespace_lease_price().to_string(), "0.5");
}

#[test]
fn absolute_renewal_target_requires_positive_whole_year_delta() {
    let current = 10_000;
    assert_eq!(
        resolved_renewal_term_years(current, current + MS_PER_YEAR).expect("one whole year"),
        1
    );
    assert!(resolved_renewal_term_years(current, current).is_err());
    assert!(resolved_renewal_term_years(current, current - 1).is_err());
    assert!(resolved_renewal_term_years(current, current + MS_PER_YEAR - 1).is_err());
    assert!(
        resolved_renewal_term_years(
            current,
            current + MS_PER_YEAR.saturating_mul(u64::from(u8::MAX) + 1),
        )
        .is_err()
    );
}

#[test]
fn auto_renew_state_storage_is_target_bound_and_revision_preserving() {
    let target = AliasTargetV1::AccountAlias(ResolvedAccountAliasV1::new(
        "merchant@universal"
            .parse::<AccountAliasName>()
            .expect("account alias"),
        DataSpaceId::UNIVERSAL,
    ));
    let state = AliasAutoRenewStateV1::new(target.clone(), owner(), 7, None);
    let key = alias_auto_renew_storage_key(&target).expect("auto-renew storage key");
    let mut world = World::default();
    world
        .smart_contract_state_mut_for_testing()
        .insert(key, state.encode());
    assert_eq!(
        alias_auto_renew_state(&world.view(), &target).expect("decode auto-renew state"),
        Some(state)
    );

    let other = AliasTargetV1::AccountAlias(ResolvedAccountAliasV1::new(
        "other@universal"
            .parse::<AccountAliasName>()
            .expect("other account alias"),
        DataSpaceId::UNIVERSAL,
    ));
    assert_eq!(
        alias_auto_renew_state(&world.view(), &other).expect("other target is absent"),
        None
    );
}

#[test]
fn auto_renew_storage_selection_is_bounded_and_wraps_after_cursor() {
    let mut entries = ["alpha@universal", "bravo@universal", "charlie@universal"]
        .into_iter()
        .map(|literal| {
            let target = AliasTargetV1::AccountAlias(ResolvedAccountAliasV1::new(
                literal.parse::<AccountAliasName>().expect("account alias"),
                DataSpaceId::UNIVERSAL,
            ));
            let key = alias_auto_renew_storage_key(&target).expect("storage key");
            let state = AliasAutoRenewStateV1::new(target, owner(), 1, None);
            (key, state)
        })
        .collect::<Vec<_>>();
    entries.sort_by(|left, right| left.0.cmp(&right.0));
    let mut world = World::default();
    for (key, state) in &entries {
        world
            .smart_contract_state_mut_for_testing()
            .insert(key.clone(), state.encode());
    }
    let keys = entries
        .iter()
        .map(|(key, _)| key.clone())
        .collect::<Vec<_>>();

    assert_eq!(
        alias_auto_renew_candidate_keys(&world.view(), None, 2),
        keys[..2]
    );
    assert_eq!(
        alias_auto_renew_candidate_keys(&world.view(), Some(&keys[1]), 2),
        vec![keys[2].clone(), keys[0].clone()]
    );
    assert_eq!(
        alias_auto_renew_candidate_keys(&world.view(), Some(&keys[2]), 1),
        vec![keys[0].clone()]
    );
    assert_eq!(
        alias_auto_renew_candidate_keys(&world.view(), Some(&keys[1]), 3),
        vec![keys[2].clone(), keys[0].clone(), keys[1].clone()],
        "a full sweep must revisit the cursor key after wrapping"
    );
}

fn owner() -> AccountId {
    let public_key = "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03"
        .parse()
        .expect("public key");
    AccountId::new(public_key)
}

fn checked_keypair() -> KeyPair {
    KeyPair::try_random().expect("SNS fixture key generation should succeed")
}

fn checked_account_id() -> AccountId {
    AccountId::new(checked_keypair().public_key().clone())
}
