//! Content publishing after genesis changes only through the governed permission definition
//! (`crate::runtime::content_publish_permission`).

use super::*;
use crate::{executor::Executor, runtime::content_publish_permission as governed};
use iroha_data_model::{
    isi::content::{PublishContentBundle, RetireContentBundle},
    runtime::definition::RestrictionV1,
};

/// What the root of a fixture selected.
#[derive(Clone, Copy)]
enum Root {
    Baseline,
    Selected,
    Restricted(RestrictionV1),
}

/// A root after genesis on which `issuer` holds `CanSetParameters`; `publisher` and `stranger`
/// hold nothing.
fn fixture(root: Root) -> (State, AccountId, AccountId, AccountId) {
    let issuer = checked_account_id();
    let publisher = checked_account_id();
    let stranger = checked_account_id();
    let mut world = World::with(
        [],
        [&issuer, &publisher, &stranger].map(|id| Account::new(id.clone()).build(id)),
        [],
    );
    world.account_permissions.insert(
        issuer.clone(),
        BTreeSet::from([executor_permission::parameter::CanSetParameters.into()]),
    );
    match root {
        Root::Baseline => {}
        Root::Selected => governed::activate_for_testing(&world).unwrap(),
        Root::Restricted(restriction) => {
            governed::activate_for_testing(&world).unwrap();
            governed::restrict_for_testing(&world, restriction);
        }
    }
    (state_after_genesis(world), issuer, publisher, stranger)
}

fn header(state: &State) -> BlockHeader {
    BlockHeader::new(
        nonzero!(2_u64),
        Some(state.view().latest_block_hash().unwrap()),
        None,
        0,
        0,
    )
}

/// A one-file archive.
fn tarball(body: &[u8]) -> Vec<u8> {
    const BLOCK: usize = 512;
    let mut archive = vec![0_u8; BLOCK];
    archive[..10].copy_from_slice(b"index.html");
    let size = format!("{:0>11o}\0", body.len());
    archive[124..124 + size.len()].copy_from_slice(size.as_bytes());
    archive[156] = b'0';
    archive.extend_from_slice(body);
    archive.resize(archive.len().div_ceil(BLOCK) * BLOCK + 2 * BLOCK, 0);
    archive
}

fn publication(body: &[u8]) -> (Hash, PublishContentBundle) {
    let tarball = tarball(body);
    let bundle_id = Hash::new(&tarball);
    let instruction = PublishContentBundle {
        bundle_id,
        tarball,
        expires_at_height: None,
        manifest: None,
    };
    (bundle_id, instruction)
}

fn publish(body: &[u8]) -> (Hash, InstructionBox) {
    let (bundle_id, instruction) = publication(body);
    (bundle_id, instruction.into())
}

fn retire(bundle_id: Hash) -> InstructionBox {
    RetireContentBundle { bundle_id }.into()
}

fn run(
    tx: &mut StateTransaction<'_, '_>,
    authority: &AccountId,
    instruction: impl Into<InstructionBox>,
) -> Result<(), ValidationFail> {
    Executor::Initial.execute_instruction(tx, authority, &instruction.into())
}

fn holds(tx: &StateTransaction<'_, '_>, account: &AccountId, permission: &Permission) -> bool {
    tx.world
        .account_permissions_iter(account)
        .unwrap()
        .any(|held| held == permission)
}

/// The refusal, with the Core error it carries.
fn rejection(result: Result<(), ValidationFail>) -> String {
    format!(
        "{:?}",
        result.expect_err("the Initial executor must refuse")
    )
}

#[test]
fn a_root_without_the_definition_keeps_the_frozen_baseline_rules() {
    let (state, issuer, publisher, stranger) = fixture(Root::Baseline);
    let mut block = state.block(header(&state));
    let mut tx = block.transaction();
    let token = governed::token();
    // A token of the name granted under the baseline's rules, as genesis could.
    Grant::account_permission(token.clone(), publisher.clone())
        .execute(&issuer, &mut tx)
        .expect("seed an opaque grant");

    let (bundle_id, instruction) = publish(b"baseline");
    for denied in [instruction, retire(bundle_id)] {
        assert!(
            rejection(run(&mut tx, &publisher, denied))
                .contains("administrative instruction requires an explicit governed lifecycle")
        );
    }
    assert!(tx.world.content_bundles.get(&bundle_id).is_none());
    // The parameter authority gains nothing over an unknown name; its holder may pass it on,
    // and a role may carry it.
    run(
        &mut tx,
        &issuer,
        Grant::account_permission(token.clone(), stranger.clone()),
    )
    .expect_err("the baseline lets only a holder delegate an unknown name");
    run(
        &mut tx,
        &publisher,
        Grant::account_permission(token.clone(), stranger.clone()),
    )
    .expect("the baseline lets a holder delegate an unknown name");
    assert!(holds(&tx, &stranger, &token));
    let role: RoleId = "content_publishers".parse().unwrap();
    assert!(
        rejection(run(
            &mut tx,
            &issuer,
            Register::role(Role::new(role.clone(), issuer.clone()).add_permission(token.clone())),
        ))
        .contains("Unknown permission")
    );
    Register::role(Role::new(role, issuer.clone()).add_permission(token))
        .execute(&issuer, &mut tx)
        .expect("Core keeps the baseline's opaque role permissions");
}

#[test]
fn publishing_after_genesis_requires_a_direct_grant() {
    let (state, issuer, publisher, stranger) = fixture(Root::Selected);
    let mut block = state.block(header(&state));
    let mut tx = block.transaction();
    let token = governed::token();

    let (first, instruction) = publish(b"direct");
    assert!(
        rejection(run(&mut tx, &publisher, instruction.clone()))
            .contains("requires a direct CanPublishContentBundle grant")
    );
    assert!(tx.world.content_bundles.get(&first).is_none());
    run(
        &mut tx,
        &issuer,
        Grant::account_permission(token.clone(), publisher.clone()),
    )
    .expect("the parameter authority issues the permission");
    run(&mut tx, &publisher, instruction).expect("a direct holder publishes");
    let record = tx.world.content_bundles.get(&first).cloned().unwrap();
    assert_eq!(record.created_by, publisher);

    // No role carries it, so assigning a role never confers publishing: the Initial executor
    // refuses, and Core refuses for every executor.
    let role: RoleId = "content_publishers".parse().unwrap();
    assert!(
        rejection(run(
            &mut tx,
            &issuer,
            Register::role(Role::new(role.clone(), issuer.clone()).add_permission(token.clone())),
        ))
        .contains("direct account permission")
    );
    let refusal =
        Register::role(Role::new(role.clone(), stranger.clone()).add_permission(token.clone()))
            .execute(&issuer, &mut tx)
            .expect_err("Core keeps the permission off roles");
    assert!(format!("{refusal:?}").contains("direct account permissions"));
    Register::role(Role::new(role.clone(), stranger.clone()))
        .execute(&issuer, &mut tx)
        .expect("seed an empty role");
    let refusal = Grant::role_permission(token.clone(), role.clone())
        .execute(&issuer, &mut tx)
        .expect_err("Core keeps the permission off an existing role");
    assert!(format!("{refusal:?}").contains("direct account permissions"));
    assert!(
        rejection(run(&mut tx, &stranger, Grant::role_permission(token, role)))
            .contains("direct account permission")
    );
    let (second, instruction) = publish(b"through a role");
    run(&mut tx, &stranger, instruction).expect_err("holding a role confers nothing");
    assert!(tx.world.content_bundles.get(&second).is_none());

    // Only the creator retires a bundle; Core decides that after the executor admits it.
    assert!(
        rejection(run(&mut tx, &stranger, retire(first)))
            .contains("only the content bundle creator may retire it")
    );
    run(&mut tx, &publisher, retire(first)).expect("the creator retires its bundle");
    assert!(tx.world.content_bundles.get(&first).is_none());
}

#[test]
fn only_the_parameter_authority_grants_or_revokes_the_permission() {
    let (state, issuer, publisher, stranger) = fixture(Root::Selected);
    let mut block = state.block(header(&state));
    let mut tx = block.transaction();
    let token = governed::token();

    run(
        &mut tx,
        &stranger,
        Grant::account_permission(token.clone(), stranger.clone()),
    )
    .expect_err("an account without CanSetParameters cannot issue it");
    let malformed = Permission::new(governed::NAME.into(), Json::new(5_u32));
    assert!(
        rejection(run(
            &mut tx,
            &issuer,
            Grant::account_permission(malformed, publisher.clone()),
        ))
        .contains("unit payload")
    );
    run(
        &mut tx,
        &issuer,
        Grant::account_permission(token.clone(), publisher.clone()),
    )
    .expect("the parameter authority issues it");
    run(
        &mut tx,
        &publisher,
        Grant::account_permission(token.clone(), stranger.clone()),
    )
    .expect_err("holding it never authorizes delegation");
    run(
        &mut tx,
        &stranger,
        Revoke::account_permission(token.clone(), publisher.clone()),
    )
    .expect_err("an account without CanSetParameters cannot withdraw it");
    run(
        &mut tx,
        &publisher,
        Revoke::account_permission(token.clone(), publisher.clone()),
    )
    .expect("a holder renounces its own grant");
    assert!(!holds(&tx, &publisher, &token));
    run(
        &mut tx,
        &issuer,
        Grant::account_permission(token.clone(), publisher.clone()),
    )
    .expect("the parameter authority issues it again");
    run(
        &mut tx,
        &issuer,
        Revoke::account_permission(token.clone(), publisher.clone()),
    )
    .expect("the parameter authority withdraws it");
    assert!(!holds(&tx, &publisher, &token));
}

#[test]
fn every_restriction_stops_publishing_and_grants_but_not_revocation_or_retirement() {
    for restriction in [
        RestrictionV1::Deprecated(1),
        RestrictionV1::AdmissionClosed(1),
        RestrictionV1::CompletionOnly(1),
        RestrictionV1::Disabled(1),
    ] {
        let (state, issuer, publisher, stranger) = fixture(Root::Restricted(restriction));
        let mut block = state.block(header(&state));
        let mut tx = block.transaction();
        let token = governed::token();
        // Grants and a bundle from before the restriction.
        for holder in [&publisher, &stranger] {
            Grant::account_permission(token.clone(), holder.clone())
                .execute(&issuer, &mut tx)
                .expect("seed a grant");
        }
        let (bundle_id, seeded) = publication(b"before the restriction");
        seeded.execute(&publisher, &mut tx).expect("seed a bundle");

        let (_, later) = publish(b"after the restriction");
        assert!(
            rejection(run(&mut tx, &publisher, later)).contains("content publishing is restricted"),
            "{restriction:?}"
        );
        let fresh = checked_account_id();
        assert!(
            rejection(run(
                &mut tx,
                &issuer,
                Grant::account_permission(token.clone(), fresh),
            ))
            .contains("cannot be granted"),
            "{restriction:?}"
        );
        run(&mut tx, &publisher, retire(bundle_id))
            .unwrap_or_else(|error| panic!("{restriction:?}: the creator retires: {error:?}"));
        assert!(tx.world.content_bundles.get(&bundle_id).is_none());
        run(
            &mut tx,
            &publisher,
            Revoke::account_permission(token.clone(), publisher.clone()),
        )
        .unwrap_or_else(|error| panic!("{restriction:?}: a holder renounces: {error:?}"));
        run(
            &mut tx,
            &issuer,
            Revoke::account_permission(token.clone(), stranger.clone()),
        )
        .unwrap_or_else(|error| panic!("{restriction:?}: the issuer withdraws: {error:?}"));
        assert!(!holds(&tx, &publisher, &token));
        assert!(!holds(&tx, &stranger, &token));
    }
}

#[test]
fn retirement_waits_for_grants_and_post_genesis_bundles() {
    let (state, issuer, publisher, _) = fixture(Root::Selected);
    let mut block = state.block(header(&state));
    let mut tx = block.transaction();
    let token = governed::token();
    let current = tx.world.runtime_selection().clone();
    let definition = governed::descriptor().id().unwrap();
    let mut retired = current.clone();
    retired.active.retain(|id| *id != definition);
    assert!(governed::check_retirement(&tx.world, &current, &retired).is_ok());
    // Every other change of the selection is not a retirement.
    assert!(governed::check_retirement(&tx.world, &current, &current).is_ok());

    run(
        &mut tx,
        &issuer,
        Grant::account_permission(token.clone(), publisher.clone()),
    )
    .expect("issue the permission");
    assert!(
        governed::check_retirement(&tx.world, &current, &retired)
            .unwrap_err()
            .contains("direct grant")
    );
    let (bundle_id, instruction) = publish(b"published under the permission");
    run(&mut tx, &publisher, instruction).expect("publish after genesis");
    run(
        &mut tx,
        &issuer,
        Revoke::account_permission(token, publisher.clone()),
    )
    .expect("withdraw the permission");
    // A bundle published after genesis keeps the definition, even once it has expired:
    // retirement would leave it without its only removal path.
    let record = tx.world.content_bundles.get(&bundle_id).cloned().unwrap();
    assert!(record.created_height > 1);
    assert!(
        governed::check_retirement(&tx.world, &current, &retired)
            .unwrap_err()
            .contains("content bundle")
    );
    let expired_id = Hash::new(b"an expired bundle");
    tx.world.content_bundles.insert(
        expired_id,
        iroha_data_model::content::ContentBundleRecord {
            bundle_id: expired_id,
            expires_at_height: Some(record.created_height),
            ..record.clone()
        },
    );
    run(&mut tx, &publisher, retire(bundle_id)).expect("the creator retires its bundle");
    assert!(governed::check_retirement(&tx.world, &current, &retired).is_err());
    tx.world.content_bundles.remove(expired_id);
    // A bundle genesis published never depended on the definition.
    let genesis_id = Hash::new(b"a genesis bundle");
    tx.world.content_bundles.insert(
        genesis_id,
        iroha_data_model::content::ContentBundleRecord {
            bundle_id: genesis_id,
            created_height: 1,
            ..record
        },
    );
    assert!(governed::check_retirement(&tx.world, &current, &retired).is_ok());
}
