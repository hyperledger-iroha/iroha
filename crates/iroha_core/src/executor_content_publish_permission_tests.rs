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
    Disabled,
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
        Root::Disabled => {
            governed::activate_for_testing(&world).unwrap();
            governed::restrict_for_testing(&world, RestrictionV1::Disabled(1));
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

fn publish(body: &[u8]) -> (Hash, InstructionBox) {
    let tarball = tarball(body);
    let bundle_id = Hash::new(&tarball);
    let instruction = PublishContentBundle {
        bundle_id,
        tarball,
        expires_at_height: None,
        manifest: None,
    }
    .into();
    (bundle_id, instruction)
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
    format!("{:?}", result.expect_err("the Initial executor must refuse"))
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
    // The parameter authority gains nothing over an unknown name; its holder may pass it on.
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
            Register::role(Role::new(role, issuer.clone()).add_permission(token)),
        ))
        .contains("Unknown permission")
    );
}

#[test]
fn publishing_after_genesis_requires_the_permission_directly_or_through_a_role() {
    let (state, issuer, publisher, stranger) = fixture(Root::Selected);
    let mut block = state.block(header(&state));
    let mut tx = block.transaction();
    let token = governed::token();

    let (first, instruction) = publish(b"direct");
    assert!(
        rejection(run(&mut tx, &publisher, instruction.clone()))
            .contains("requires CanPublishContentBundle")
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

    // Holding it through a role is enough.
    let role: RoleId = "content_publishers".parse().unwrap();
    Register::role(Role::new(role, stranger.clone()).add_permission(token))
        .execute(&issuer, &mut tx)
        .expect("seed a publisher role");
    let (second, instruction) = publish(b"through a role");
    run(&mut tx, &stranger, instruction).expect("a role holder publishes");
    assert!(tx.world.content_bundles.get(&second).is_some());

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

    // Roles: a role holder cannot spread or strip it; the parameter authority strips it from a
    // role it does not hold, but adds it only to a role it holds.
    let role: RoleId = "content_publishers".parse().unwrap();
    Register::role(Role::new(role.clone(), stranger.clone()).add_permission(token.clone()))
        .execute(&issuer, &mut tx)
        .expect("seed a publisher role");
    run(
        &mut tx,
        &stranger,
        Grant::account_role(role.clone(), publisher.clone()),
    )
    .expect_err("a role holder cannot pass the permission on through the role");
    run(
        &mut tx,
        &stranger,
        Revoke::role_permission(token.clone(), role.clone()),
    )
    .expect_err("a role holder cannot withdraw it from the role");
    run(
        &mut tx,
        &issuer,
        Revoke::role_permission(token.clone(), role.clone()),
    )
    .expect("the parameter authority removes it from any role");
    assert!(
        !tx.world
            .roles()
            .get(&role)
            .unwrap()
            .permissions()
            .any(|permission| permission == &token)
    );
    run(&mut tx, &issuer, Grant::role_permission(token, role))
        .expect_err("adding it to a role still requires holding that role");
}

#[test]
fn a_disabled_definition_stops_publishing_and_grants_but_not_revocation_or_retirement() {
    let (state, issuer, publisher, stranger) = fixture(Root::Disabled);
    let mut block = state.block(header(&state));
    let mut tx = block.transaction();
    let token = governed::token();
    // A grant and a bundle from before the restriction.
    Grant::account_permission(token.clone(), publisher.clone())
        .execute(&issuer, &mut tx)
        .expect("seed a grant");
    let archive = tarball(b"before the restriction");
    let bundle_id = Hash::new(&archive);
    PublishContentBundle {
        bundle_id,
        tarball: archive,
        expires_at_height: None,
        manifest: None,
    }
    .execute(&publisher, &mut tx)
    .expect("seed a bundle");

    let (_, later) = publish(b"after the restriction");
    assert!(rejection(run(&mut tx, &publisher, later)).contains("content publishing is disabled"));
    assert!(
        rejection(run(
            &mut tx,
            &issuer,
            Grant::account_permission(token.clone(), stranger.clone()),
        ))
        .contains("cannot be granted")
    );
    run(&mut tx, &publisher, retire(bundle_id)).expect("the creator still retires its bundle");
    assert!(tx.world.content_bundles.get(&bundle_id).is_none());
    run(
        &mut tx,
        &issuer,
        Revoke::account_permission(token.clone(), publisher.clone()),
    )
    .expect("the parameter authority still withdraws it");
    assert!(!holds(&tx, &publisher, &token));
}
