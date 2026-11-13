//! Governed content cleanup remains executable through restrictions and change sets, and holds
//! back no other change.

use std::num::NonZeroU64;

use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair, PublicKey};
use iroha_data_model::{
    NetworkId, Registrable as _,
    account::{Account, AccountId},
    block::BlockHeader,
    isi::{
        Grant, Revoke, RevokeBox,
        content::{PublishContentBundle, RetireContentBundle},
    },
    runtime::{
        definition::RestrictionEntryV1,
        lifecycle::{
            ChangeSetRecordV1, ChangeSetStatusV1, ChangeSetV1, RestrictionKindV1, RootScopeV1,
            RuntimeNoticeV1, SelectionTransitionV1, SupportAttestationV1, committee_digest,
        },
    },
};

use super::*;
use crate::{
    executor::Executor,
    runtime::{
        change_set::{PublishedDefinitions, activate_due, record_attestation},
        content_publish_permission as governed,
    },
    smartcontracts::Execute as _,
    state::{State, World, WorldReadOnly as _},
};

fn issuer() -> AccountId {
    iroha_test_samples::ALICE_ID.clone()
}
fn publisher() -> AccountId {
    iroha_test_samples::BOB_ID.clone()
}
fn network() -> NetworkId {
    NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::prehashed([0x57; 32])))
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
fn header(state: &State) -> BlockHeader {
    let height = state
        .world
        .view()
        .runtime_change_sets()
        .iter()
        .map(|(_, record)| record.change_set.effective_height)
        .max()
        .unwrap_or(1)
        + 1;
    BlockHeader::new(NonZeroU64::new(height).unwrap(), None, None, 0, 0)
}

fn fixture(selected: bool) -> State {
    let world = World::with(
        [],
        [issuer(), publisher()].map(|id| Account::new(id.clone()).build(&id)),
        [],
    );
    let mut block = world.block();
    block.account_permissions.insert(
        issuer(),
        [iroha_executor_data_model::permission::parameter::CanSetParameters.into()]
            .into_iter()
            .collect(),
    );
    block.commit();
    if selected {
        let activation = crate::runtime::draft_addition(
            &world.view(),
            network(),
            governed::MODULE,
            5,
            [0x58; 32],
        )
        .unwrap();
        assert_eq!(
            activate(&world, &activation),
            ChangeSetStatusV1::Activated(5)
        );
    }
    // The real executor reads immutable root scope from authenticated genesis metadata.
    // A synthetic non-genesis header alone grants no instruction execution authority.
    let config = crate::sumeragi::test_chain::TestChainConfig::new(world, 0);
    let genesis_account = AccountId::new(config.genesis_key.public_key().clone());
    let consensus_mode = config.consensus_mode;
    let prepared = crate::sumeragi::test_chain::CertifiedTestChain::prepare(config)
        .expect("prepare signed content fixture genesis");
    let state = std::sync::Arc::try_unwrap(prepared.state)
        .unwrap_or_else(|_| panic!("unpublished content fixture State is unique"));
    crate::sumeragi::startup::apply_genesis(
        &state,
        prepared.genesis.block().clone(),
        &genesis_account,
        consensus_mode.into(),
        None,
    )
    .expect("apply signed content fixture genesis");
    state
}

/// Exercise the actual block-start transition with every validator's support statement.
fn activate(world: &World, change: &ChangeSetV1) -> ChangeSetStatusV1 {
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
            release: [0x59; 32],
            validator: pair.public_key().clone(),
        }
        .sign(pair.private_key())
        .unwrap();
        record_attestation(&mut record, attestation);
    }
    let mut block = world.block();
    block
        .runtime_change_sets
        .insert(change.id().unwrap(), record);
    let mut tx = block
        .transaction_without_telemetry(iroha_config::parameters::actual::LaneConfig::default(), 0);
    activate_due(&mut tx, change.effective_height, 1_000, &committee).unwrap();
    tx.apply();
    block.commit();
    world
        .view()
        .runtime_change_sets()
        .get(&change.id().unwrap())
        .unwrap()
        .status
}

fn restriction(world: &World, wire_id: &str, kind: RestrictionKindV1, height: u64) -> ChangeSetV1 {
    let view = world.view();
    let current = view.runtime_selection();
    let definition = binding(current, wire_id).unwrap().interface;
    let mut successor = current.clone();
    successor
        .restrictions
        .retain(|entry| entry.definition != definition);
    successor.restrictions.push(RestrictionEntryV1 {
        definition,
        restriction: match kind {
            RestrictionKindV1::Deprecated => RestrictionV1::Deprecated(height),
            RestrictionKindV1::AdmissionClosed => RestrictionV1::AdmissionClosed(height),
            RestrictionKindV1::CompletionOnly => RestrictionV1::CompletionOnly(height),
            RestrictionKindV1::Disabled => RestrictionV1::Disabled(height),
        },
    });
    successor.restrictions.sort_by_key(|entry| entry.definition);
    let change = ChangeSetV1 {
        network: network(),
        root: RootScopeV1::Universal,
        predecessor: current.id().unwrap(),
        successor: successor.id().unwrap(),
        effective_height: height,
        publish: Vec::new(),
        transitions: vec![SelectionTransitionV1::Restrict {
            definition,
            restriction: kind,
        }],
        qualification: [0x5a; 32],
    };
    change
        .apply(current, &PublishedDefinitions(view.runtime_definitions()))
        .unwrap();
    change
}

fn publication() -> PublishContentBundle {
    let mut tarball = vec![0; 512];
    tarball[..10].copy_from_slice(b"index.html");
    tarball[124..136].copy_from_slice(b"00000000001\0");
    tarball[156] = b'0';
    tarball.push(b'x');
    tarball.resize(4 * 512, 0);
    PublishContentBundle {
        bundle_id: Hash::new(&tarball),
        tarball,
        expires_at_height: Some(6),
        manifest: None,
    }
}

fn execute(
    state: &State,
    authority: &AccountId,
    instruction: impl Into<InstructionBox>,
) -> Result<(), iroha_data_model::ValidationFail> {
    let mut block = state.block(header(state));
    let mut tx = block.transaction();
    Executor::Initial.execute_instruction(&mut tx, authority, &instruction.into())?;
    tx.apply();
    block.commit_world_overlay_for_testing().unwrap();
    Ok(())
}

fn grant(state: &State) {
    execute(
        state,
        &issuer(),
        Grant::account_permission(governed::token(), publisher()),
    )
    .unwrap();
}

#[test]
fn governed_content_retirement_survives_admission_closure_and_completion_only() {
    for kind in [
        RestrictionKindV1::AdmissionClosed,
        RestrictionKindV1::CompletionOnly,
    ] {
        let state = fixture(true);
        grant(&state);
        let publish = publication();
        let retire = RetireContentBundle {
            bundle_id: publish.bundle_id,
        };
        execute(&state, &publisher(), publish).unwrap();
        let change = restriction(
            &state.world,
            wire_id_of::<RetireContentBundle>().unwrap(),
            kind,
            6,
        );
        assert_eq!(
            activate(&state.world, &change),
            ChangeSetStatusV1::Activated(6)
        );
        let refused = execute(&state, &issuer(), retire.clone()).unwrap_err();
        assert!(format!("{refused:?}").contains("only the content bundle creator"));
        execute(&state, &publisher(), retire.clone())
            .expect("existing bundle retirement completes admitted work");
        assert!(
            state
                .world
                .content_bundles
                .view()
                .get(&retire.bundle_id)
                .is_none()
        );
        execute(&state, &publisher(), retire)
            .expect_err("an absent bundle is not admitted completion work");
    }
}

#[test]
fn governed_grant_revocation_survives_completion_only_and_keeps_authority_checks() {
    let state = fixture(true);
    grant(&state);
    let change = restriction(
        &state.world,
        wire_id_of::<RevokeBox>().unwrap(),
        RestrictionKindV1::CompletionOnly,
        6,
    );
    assert_eq!(
        activate(&state.world, &change),
        ChangeSetStatusV1::Activated(6)
    );
    execute(
        &state,
        &issuer(),
        Revoke::account_permission(governed::token(), issuer()),
    )
    .expect_err("absent grant is not a completion");
    execute(
        &state,
        &publisher(),
        Revoke::account_permission(governed::token(), publisher()),
    )
    .expect("holder may renounce an outstanding grant under completion-only");
    assert!(
        !state
            .world
            .account_permissions
            .view()
            .get(&publisher())
            .is_some_and(|permissions| permissions.contains(&governed::token()))
    );
}

#[test]
fn content_cleanup_cannot_be_disabled_until_bundles_and_grants_drain() {
    let state = fixture(true);
    grant(&state);
    let publish = publication();
    let bundle_id = publish.bundle_id;
    execute(&state, &publisher(), publish).unwrap();
    for wire_id in [
        wire_id_of::<RetireContentBundle>().unwrap(),
        wire_id_of::<RevokeBox>().unwrap(),
    ] {
        let change = restriction(&state.world, wire_id, RestrictionKindV1::Disabled, 6);
        assert_eq!(
            activate(&state.world, &change),
            ChangeSetStatusV1::Lapsed(6)
        );
    }
    execute(
        &state,
        &issuer(),
        Revoke::account_permission(governed::token(), publisher()),
    )
    .unwrap();
    let change = restriction(
        &state.world,
        wire_id_of::<RetireContentBundle>().unwrap(),
        RestrictionKindV1::Disabled,
        7,
    );
    assert_eq!(
        activate(&state.world, &change),
        ChangeSetStatusV1::Lapsed(7),
        "expired bundle still needs its original cleanup"
    );
    execute(&state, &publisher(), RetireContentBundle { bundle_id }).unwrap();
    let change = restriction(
        &state.world,
        wire_id_of::<RetireContentBundle>().unwrap(),
        RestrictionKindV1::Disabled,
        8,
    );
    assert_eq!(
        activate(&state.world, &change),
        ChangeSetStatusV1::Activated(8)
    );
}

#[test]
fn unselected_content_keeps_the_frozen_baseline_obligation_rules() {
    let state = fixture(false);
    let publish = publication();
    let bundle_id = publish.bundle_id;
    let revoke: InstructionBox = Revoke::account_permission(governed::token(), publisher()).into();
    let retire: InstructionBox = RetireContentBundle { bundle_id }.into();
    {
        let mut block = state.block(header(&state));
        let mut tx = block.transaction();
        Grant::account_permission(governed::token(), publisher())
            .execute(&issuer(), &mut tx)
            .unwrap();
        publish.execute(&publisher(), &mut tx).unwrap();
        assert_eq!(content_step(&retire, &tx.world), None);
        assert_eq!(content_step(&revoke, &tx.world), None);
        assert!(LiveObligations::of(&tx.world).is_empty());
        tx.apply();
        block.commit_world_overlay_for_testing().unwrap();
    }
    let change = restriction(
        &state.world,
        wire_id_of::<RetireContentBundle>().unwrap(),
        RestrictionKindV1::Disabled,
        6,
    );
    assert_eq!(
        activate(&state.world, &change),
        ChangeSetStatusV1::Activated(6)
    );
}

#[test]
fn selected_custom_executor_grants_keep_only_their_exact_cleanup_path() {
    use iroha_data_model::permission::Permission;
    use iroha_primitives::json::Json;

    let state = fixture(true);
    let token = Permission::new(governed::NAME.into(), Json::new(7_u32));
    {
        let mut block = state.block(header(&state));
        let mut tx = block.transaction();
        // The native handler preserves a payload authorized by an installed executor.
        Grant::account_permission(token.clone(), publisher())
            .execute(&issuer(), &mut tx)
            .unwrap();
        tx.apply();
        block.commit_world_overlay_for_testing().unwrap();
    }
    let change = restriction(
        &state.world,
        wire_id_of::<RevokeBox>().unwrap(),
        RestrictionKindV1::CompletionOnly,
        6,
    );
    assert_eq!(
        activate(&state.world, &change),
        ChangeSetStatusV1::Activated(6)
    );
    let mut block = state.block(header(&state));
    let mut tx = block.transaction();
    for permission in [
        governed::token(),
        Permission::new(governed::NAME.into(), Json::new(8_u32)),
    ] {
        let absent: InstructionBox = Revoke::account_permission(permission, publisher()).into();
        assert_eq!(content_step(&absent, &tx.world), None);
        crate::runtime::resolve_executing(&tx, &absent)
            .expect_err("a different payload cannot borrow the held grant's completion");
    }
    let revoke = Revoke::account_permission(token, publisher());
    let instruction: InstructionBox = revoke.clone().into();
    assert_eq!(content_step(&instruction, &tx.world), Some(Step::Complete));
    crate::runtime::resolve_executing(&tx, &instruction)
        .expect("an installed executor may authorize exact cleanup");
    Executor::Initial
        .execute_instruction(&mut tx, &issuer(), &instruction)
        .expect_err("Initial still requires the exact unit payload");
    revoke
        .execute(&issuer(), &mut tx)
        .expect("native cleanup removes the exact executor-authorized grant");
    assert_eq!(content_step(&instruction, &tx.world), None);
}

#[test]
fn outstanding_content_state_holds_back_no_rebinding_and_cleanup_follows_it() {
    let state = fixture(true);
    grant(&state);
    let publish = publication();
    let bundle_id = publish.bundle_id;
    execute(&state, &publisher(), publish).unwrap();
    {
        // A grant and a bundle are durable State without custody: no closure is pinned to them,
        // so neither a change set nor an executor replacement waits for them to drain.
        let mut block = state.block(header(&state));
        let tx = block.transaction();
        assert!(LiveObligations::of(&tx.world).is_empty());
    }
    // The restricted-read addition rebinds `iroha.revoke` while the grant is outstanding.
    let revoke = wire_id_of::<RevokeBox>().unwrap();
    let original = binding(state.world.view().runtime_selection(), revoke)
        .unwrap()
        .implementation;
    let rebinding = crate::runtime::draft_addition(
        &state.world.view(),
        network(),
        crate::runtime::catalog::additions::RESTRICTED_DATASPACE_READ.module,
        6,
        [0x5b; 32],
    )
    .unwrap();
    assert_eq!(
        activate(&state.world, &rebinding),
        ChangeSetStatusV1::Activated(6)
    );
    assert_ne!(
        binding(state.world.view().runtime_selection(), revoke)
            .unwrap()
            .implementation,
        original
    );
    // Cleanup runs through the implementation the root now selects.
    execute(
        &state,
        &issuer(),
        Revoke::account_permission(governed::token(), publisher()),
    )
    .expect("the issuer revokes through the rebound instruction");
    execute(&state, &publisher(), RetireContentBundle { bundle_id })
        .expect("the creator retires its bundle after the rebinding");
}

#[test]
fn cleanup_check_refuses_only_a_selection_that_stops_a_needed_instruction() {
    let state = fixture(true);
    grant(&state);
    let publish = publication();
    let bundle_id = publish.bundle_id;
    execute(&state, &publisher(), publish).unwrap();
    let current = state.world.view().runtime_selection().clone();
    let without = |wire_id: &str| {
        let mut next = current.clone();
        next.bindings
            .retain(|bound| bound.wire_id.as_str() != wire_id);
        next
    };
    let no_revoke = without(wire_id_of::<RevokeBox>().unwrap());
    let no_retire = without(wire_id_of::<RetireContentBundle>().unwrap());
    let check =
        |next: &RootSelectionV1| governed::check_cleanup(&state.world.view(), &current, next);
    assert_eq!(check(&current), Ok(()));
    assert!(check(&no_revoke).unwrap_err().contains("direct grant"));
    assert!(check(&no_retire).unwrap_err().contains("content bundle"));
    // Each instruction is needed only while the State it cleans up remains.
    execute(
        &state,
        &issuer(),
        Revoke::account_permission(governed::token(), publisher()),
    )
    .unwrap();
    assert_eq!(check(&no_revoke), Ok(()));
    assert!(check(&no_retire).is_err());
    execute(&state, &publisher(), RetireContentBundle { bundle_id }).unwrap();
    assert_eq!(check(&no_retire), Ok(()));
}
