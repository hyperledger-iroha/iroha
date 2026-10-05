//! Actual alias fixtures and independent full-work arithmetic for owner tests.

use super::*;
use crate::test_allocations::allocations_during;
use iroha_data_model::account::{AccountController, AccountDetails};
use iroha_model_base::topology::DataSpaceId;
use iroha_test_samples::{ALICE_ID, BOB_ID};

/// One domainless universal-dataspace alias with its exact stored spelling.
pub(in crate::state) fn alias(label: &str) -> AccountAlias {
    AccountAlias::domainless(label.parse().unwrap(), DataSpaceId::UNIVERSAL)
}
/// Stored account details with an independently optional primary label.
pub(in crate::state) fn details(label: Option<AccountAlias>) -> AccountValue {
    let mut details = AccountDetails::default();
    details.set_label(label);
    AccountValue::new(details)
}
/// Two domainless accounts, one alias, and its actual exact reverse bucket.
pub(in crate::state) fn fixture() -> World {
    let mut world = World::default();
    world.accounts.insert(ALICE_ID.clone(), details(None));
    world.accounts.insert(BOB_ID.clone(), details(None));
    world
        .account_aliases
        .insert(alias("merchant"), ALICE_ID.clone());
    world.rebuild_account_alias_index().unwrap();
    world
}
/// Require actual validation/currentness to allocate nothing.
pub(in crate::state) fn without_allocations<T>(run: impl FnOnce() -> T) -> T {
    let mut result = None;
    assert_eq!(allocations_during(|| result = Some(run())), 0);
    result.unwrap()
}
fn account_bytes(account: &AccountId) -> u64 {
    match account.controller() {
        AccountController::Single(key) => 2 + key.input_payload_len() as u64,
        AccountController::Multisig(policy) => {
            12 + policy.members().len() as u64
                + policy
                    .members()
                    .iter()
                    .map(|member| 3 + member.public_key().input_payload_len() as u64)
                    .sum::<u64>()
        }
    }
}
fn alias_bytes(alias: &AccountAlias) -> u64 {
    alias.label.as_ref().len() as u64
        + 1
        + 8
        + alias
            .domain
            .as_ref()
            .map_or(0, |domain| domain.name().as_ref().len() as u64)
}
fn original_visit<'a, K: mv::Key, V: mv::Value>(
    rows: &'a impl RawStorageImages<K, V>,
    image: AliasImage,
    work: &mut u64,
    key_bytes: fn(&K) -> u64,
    mut inspect: impl FnMut(&'a K, &'a V, &mut u64),
) {
    for (key, value) in rows.current_entries() {
        *work += 1;
        let mut masked = false;
        if image == AliasImage::Predecessor {
            for (prior, _) in rows.undo_entries() {
                *work += 1 + key_bytes(key) + key_bytes(prior);
                masked |= key == prior;
            }
        }
        if !masked {
            inspect(key, value, work);
        }
    }
    if image == AliasImage::Predecessor {
        for (key, prior) in rows.undo_entries() {
            *work += 1;
            if let Some(value) = prior {
                inspect(key, value, work);
            }
        }
    }
}
fn original_lookup<'a, K: mv::Key, V: mv::Value>(
    rows: &'a impl RawStorageImages<K, V>,
    image: AliasImage,
    key: &K,
    work: &mut u64,
    key_bytes: fn(&K) -> u64,
) -> Option<&'a V> {
    let mut found = None;
    original_visit(rows, image, work, key_bytes, |candidate, value, work| {
        *work += key_bytes(candidate) + key_bytes(key);
        if candidate == key {
            found = Some(value);
        }
    });
    found
}
/// Count valid original geometry independently of the validator or Work helper.
///
/// This reference directly totals advances, full mask/lookup/member comparisons,
/// primary-option/empty flags and complete label scans. It does not binary search
/// production acceptance or reuse production equality/controller admission code.
pub(in crate::state) fn exact_work(
    accounts: &impl RawStorageImages<AccountId, AccountValue>,
    aliases: &impl RawStorageImages<AccountAlias, AccountId>,
    reverse: &impl RawStorageImages<AccountId, BTreeSet<AccountAlias>>,
) -> u64 {
    let mut total = 0;
    for image in [AliasImage::Current, AliasImage::Predecessor] {
        original_visit(
            accounts,
            image,
            &mut total,
            account_bytes,
            |account, value, work| {
                *work += 1;
                if let Some(label) = value.as_ref().label() {
                    *work += label.label.as_ref().len() as u64;
                    let bound = original_lookup(aliases, image, label, work, alias_bytes).unwrap();
                    *work += account_bytes(bound) + account_bytes(account);
                    assert_eq!(bound, account);
                }
            },
        );
        original_visit(
            aliases,
            image,
            &mut total,
            alias_bytes,
            |label, account, work| {
                *work += label.label.as_ref().len() as u64;
                assert!(original_lookup(accounts, image, account, work, account_bytes).is_some());
                let members =
                    original_lookup(reverse, image, account, work, account_bytes).unwrap();
                let mut found = false;
                for member in members {
                    *work += 1 + alias_bytes(member) + alias_bytes(label);
                    found |= member == label;
                }
                assert!(found);
            },
        );
        original_visit(
            reverse,
            image,
            &mut total,
            account_bytes,
            |account, members, work| {
                *work += 1;
                assert!(!members.is_empty());
                for label in members {
                    *work += 1;
                    let bound = original_lookup(aliases, image, label, work, alias_bytes).unwrap();
                    *work += account_bytes(bound) + account_bytes(account);
                    assert_eq!(bound, account);
                }
            },
        );
    }
    total
}
/// Count the current actual committed reader set without cloning its private rows.
pub(in crate::state) fn exact_world_work(world: &World) -> u64 {
    let accounts = world.accounts.try_committed_view_nonblocking().unwrap();
    let aliases = world
        .account_aliases
        .try_committed_view_nonblocking()
        .unwrap();
    let reverse = world
        .account_aliases_by_account
        .try_committed_view_nonblocking()
        .unwrap();
    exact_work(&accounts, &aliases, &reverse)
}
