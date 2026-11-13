//! Publication, recognition, drafting and the selection inventory of the governed
//! content-publishing permission, down to the governed change sets block start activates.

use std::collections::BTreeMap;

use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair, PublicKey};
use iroha_data_model::{
    Registrable as _,
    account::{Account, AccountId},
    permission::Permissions,
    role::{Role, RoleId},
    runtime::{
        definition::{DefinitionKind, RestrictionEntryV1, RestrictionV1},
        lifecycle::{
            ChangeSetRecordV1, ChangeSetStatusV1, RestrictionKindV1, RuntimeNoticeV1,
            SupportAttestationV1, committee_digest,
        },
    },
};
use iroha_primitives::json::Json;

use super::*;
use crate::{
    runtime::{
        change_set::{PublishedDefinitions, activate_due},
        release_catalog,
    },
    state::World,
};

fn network() -> NetworkId {
    NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::prehashed([0x23; 32])))
}

fn account(seed: u8) -> AccountId {
    AccountId::new(
        KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519)
            .public_key()
            .clone(),
    )
}

fn validators() -> Vec<KeyPair> {
    (1..=4)
        .map(|seed| KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519))
        .collect()
}

fn committee() -> Vec<PublicKey> {
    validators()
        .iter()
        .map(|pair| pair.public_key().clone())
        .collect()
}

/// Record `change` as scheduled and attested by every validator.
fn schedule(world: &World, change: &ChangeSetV1) {
    let committee = committee();
    let mut record = ChangeSetRecordV1 {
        change_set: change.clone(),
        status: ChangeSetStatusV1::Scheduled,
        proposed_ms: 0,
        approved_height: 1,
        approved_ms: 0,
        notice: RuntimeNoticeV1 {
            blocks: 1,
            ms: 0,
            approval_cushion_ms: 0,
        },
        attestations: Vec::new(),
    };
    for pair in validators() {
        let attestation = SupportAttestationV1 {
            network: change.network,
            root: change.root,
            change_set: change.id().unwrap(),
            predecessor: change.predecessor,
            successor: change.successor,
            committee: committee_digest(&committee).unwrap(),
            tested_checkpoint: 1,
            valid_from: 0,
            valid_until: u64::MAX,
            release: [0x45; 32],
            validator: pair.public_key().clone(),
        }
        .sign(pair.private_key())
        .unwrap();
        crate::runtime::change_set::record_attestation(&mut record, attestation);
    }
    let mut block = world.block();
    block
        .runtime_change_sets
        .insert(change.id().unwrap(), record);
    block.commit();
}

/// Run the block start of `height` on `world` as the kernel does and return the status the
/// change set `change` reached.
fn start_block(world: &World, height: u64, change: &ChangeSetV1) -> ChangeSetStatusV1 {
    let mut block = world.block();
    let mut transaction = block
        .transaction_without_telemetry(iroha_config::parameters::actual::LaneConfig::default(), 0);
    activate_due(&mut transaction, height, 1_000, &committee()).expect("block start");
    transaction.apply();
    block.commit();
    world
        .view()
        .runtime_change_sets()
        .get(&change.id().unwrap())
        .expect("recorded")
        .status
}

/// The activation an operator drafts for `world` at `height`.
fn draft(world: &World, height: u64) -> ChangeSetV1 {
    crate::runtime::draft_addition(&world.view(), network(), MODULE, height, [0x31; 32])
        .expect("draft the activation")
}

/// The change set that disables and deactivates the definition at `height`.
fn retirement(world: &World, height: u64) -> ChangeSetV1 {
    let view = world.view();
    let current = view.runtime_selection().clone();
    let definition = descriptor().id().unwrap();
    let mut successor = current.clone();
    successor.active.retain(|id| *id != definition);
    let change = ChangeSetV1 {
        network: network(),
        root: RootScopeV1::Universal,
        predecessor: current.id().unwrap(),
        successor: successor.id().unwrap(),
        effective_height: height,
        publish: Vec::new(),
        transitions: vec![
            SelectionTransitionV1::Restrict {
                definition,
                restriction: RestrictionKindV1::Disabled,
            },
            SelectionTransitionV1::Deactivate(definition),
        ],
        qualification: [0x32; 32],
    };
    change
        .apply(&current, &PublishedDefinitions(view.runtime_definitions()))
        .expect("a closed retirement");
    change
}

/// Run `change` at `height` and return the status it reached.
fn run(world: &World, height: u64, change: &ChangeSetV1) -> ChangeSetStatusV1 {
    schedule(world, change);
    start_block(world, height, change)
}

fn set_direct(
    world: &World,
    holder: &AccountId,
    permissions: impl IntoIterator<Item = Permission>,
) {
    let mut block = world.block();
    block.account_permissions.insert(
        holder.clone(),
        permissions.into_iter().collect::<Permissions>(),
    );
    block.commit();
}

#[test]
fn the_permission_is_published_once_after_the_frozen_baseline() {
    let catalog = release_catalog().expect("release catalog");
    let id = descriptor().id().expect("permission identity");
    assert_eq!(catalog.definitions.get(&id), Some(&descriptor()));
    assert_eq!(descriptor().kind(), DefinitionKind::Permission);
    assert!(catalog.baseline_definitions.binary_search(&id).is_err());
    assert!(!selected(&catalog.baseline));
    assert!(!selected(&catalog.lifecycle));
    // The baseline inventory is generated from the Initial names, which stay unchanged.
    assert!(!crate::executor::INITIAL_EXECUTOR_PERMISSION_NAMES.contains(&NAME));
    assert_eq!(token().name(), NAME);
    assert_eq!(token().payload(), &Json::new(()));
}

#[test]
fn every_restriction_kind_stops_a_selected_definition_being_active() {
    let catalog = release_catalog().expect("release catalog");
    let id = descriptor().id().unwrap();
    let mut selection = catalog.baseline.clone();
    assert!(!selected(&selection));
    assert!(!active(&selection));
    selection.active.push(id);
    selection.active.sort_unstable();
    assert!(selected(&selection));
    assert!(active(&selection));
    for restriction in [
        RestrictionV1::Deprecated(3),
        RestrictionV1::AdmissionClosed(3),
        RestrictionV1::CompletionOnly(3),
        RestrictionV1::Disabled(3),
    ] {
        selection.restrictions = vec![RestrictionEntryV1 {
            definition: id,
            restriction,
        }];
        assert!(selected(&selection), "{restriction:?} keeps the selection");
        assert!(
            !active(&selection),
            "{restriction:?} stops publishing and grants"
        );
    }
    selection.restrictions = vec![RestrictionEntryV1 {
        definition: catalog.baseline.bindings[0].implementation,
        restriction: RestrictionV1::Disabled(3),
    }];
    assert!(
        active(&selection),
        "a restriction on another definition changes nothing"
    );
}

#[test]
fn the_draft_selects_the_permission_from_every_compiled_selection_once() {
    let mut catalog = release_catalog().expect("release catalog").clone();
    let mut selections = vec![catalog.baseline.clone(), catalog.lifecycle.clone()];
    if let Some(ironwood) = catalog.ironwood.clone() {
        selections.push(catalog.lifecycle_successor(&ironwood.selection).unwrap());
        selections.push(ironwood.selection);
    }
    let id = descriptor().id().unwrap();
    // Everything but the permission is already published, as on a running root.
    let published: BTreeMap<DefinitionId, DefinitionDescriptorV1> = catalog
        .definitions
        .iter()
        .filter(|(definition, _)| **definition != id)
        .map(|(definition, descriptor)| (*definition, descriptor.clone()))
        .collect();
    for current in selections {
        let change = activation(&current, &published, network(), 7, [3; 32]).expect("draft");
        assert_eq!(change.effective_height, 7);
        assert_eq!(change.publish, vec![descriptor()]);
        assert!(matches!(
            change.transitions.as_slice(),
            [SelectionTransitionV1::Activate(activated)] if *activated == id
        ));
        let applied = change.apply(&current, &published).expect("applies");
        assert!(active(&applied.selection));
        let mut expected = current.clone();
        expected.active.push(id);
        expected.active.sort_unstable();
        assert_eq!(
            applied.selection, expected,
            "it installs, binds and restricts nothing"
        );
        let mut after = published.clone();
        after.extend(applied.published);
        assert!(
            activation(&applied.selection, &after, network(), 8, [3; 32]).is_err(),
            "a root that selects the permission has nothing to activate"
        );
        assert!(
            activation(&current, &after, network(), 7, [3; 32])
                .expect("draft")
                .publish
                .is_empty(),
            "a definition already published is not republished"
        );
    }
}

#[test]
fn the_inventory_counts_every_held_permission_of_the_name() {
    let holder = account(0x61);
    let world = World::with([], [Account::new(holder.clone()).build(&holder)], []);
    assert!(validate_activation_inventory(&world.view()).is_ok());
    // A malformed token of the name counts too: selection would change its meaning.
    let malformed = Permission::new(NAME.into(), Json::new(5_u32));
    for held in [token(), malformed] {
        set_direct(&world, &holder, [held]);
        assert!(validate_activation_inventory(&world.view()).is_err());
    }
    set_direct(&world, &holder, []);
    assert!(validate_activation_inventory(&world.view()).is_ok());
    let role: RoleId = "content_publishers".parse().unwrap();
    let mut block = world.block();
    block.roles.insert(
        role.clone(),
        Role::new(role.clone(), holder.clone())
            .add_permission(token())
            .build(&holder),
    );
    block.commit();
    assert!(validate_activation_inventory(&world.view()).is_err());
}

#[test]
fn activation_and_retirement_lapse_while_the_permission_is_held() {
    let holder = account(0x62);
    let world = World::with([], [Account::new(holder.clone()).build(&holder)], []);
    // A token of the name granted under the baseline's rules, directly or through a role, never
    // becomes publishing authority: the activation lapses until it is removed.
    set_direct(&world, &holder, [token()]);
    let change = draft(&world, 5);
    assert_eq!(run(&world, 5, &change), ChangeSetStatusV1::Lapsed(5));
    assert!(!selected(world.view().runtime_selection()));
    set_direct(&world, &holder, []);
    let role: RoleId = "content_publishers".parse().unwrap();
    let mut block = world.block();
    block.roles.insert(
        role.clone(),
        Role::new(role.clone(), holder.clone())
            .add_permission(token())
            .build(&holder),
    );
    block.commit();
    let change = draft(&world, 6);
    assert_eq!(run(&world, 6, &change), ChangeSetStatusV1::Lapsed(6));
    let mut block = world.block();
    block.roles.remove(role);
    block.commit();
    let change = draft(&world, 7);
    assert_eq!(run(&world, 7, &change), ChangeSetStatusV1::Activated(7));
    assert!(active(world.view().runtime_selection()));

    // Retirement waits until no account holds it, so no grant outlives the definition.
    set_direct(&world, &holder, [token()]);
    let change = retirement(&world, 8);
    assert_eq!(run(&world, 8, &change), ChangeSetStatusV1::Lapsed(8));
    assert!(selected(world.view().runtime_selection()));
    set_direct(&world, &holder, []);
    let change = retirement(&world, 9);
    assert_eq!(run(&world, 9, &change), ChangeSetStatusV1::Activated(9));
    assert!(!selected(world.view().runtime_selection()));
    assert!(!active(world.view().runtime_selection()));
}

#[test]
fn operators_draft_the_activation_under_its_module_name() {
    let modules = crate::runtime::draftable_modules().expect("draftable modules");
    assert!(modules.contains(&MODULE));
    assert!(modules.contains(&crate::runtime::catalog::parameters::MODULE));
    let world = World::default();
    let change = draft(&world, 3);
    assert!(matches!(
        change.transitions.as_slice(),
        [SelectionTransitionV1::Activate(id)] if *id == descriptor().id().unwrap()
    ));
    assert_eq!(change.publish, vec![descriptor()]);
}
