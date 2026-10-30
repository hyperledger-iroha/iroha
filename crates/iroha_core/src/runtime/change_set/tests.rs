//! Change-set admission, attestation and activation readiness.

use std::collections::BTreeMap;

use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair};
use iroha_data_model::{
    NetworkId,
    parameter::runtime_upgrade_policy::RuntimeUpgradePolicyV1,
    runtime::{
        definition::{DefinitionDescriptorV1, RestrictionEntryV1},
        lifecycle::{ChangeSetRecordV1, RestrictionKindV1, RuntimeNoticeV1, SupportAttestationV1},
    },
};
use mv::storage::Storage;

use super::{test_support, *};
use crate::runtime::release_catalog;

fn network() -> NetworkId {
    NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::prehashed([0x11; 32])))
}

fn baseline() -> RootSelectionV1 {
    release_catalog().expect("catalog").baseline.clone()
}

fn published() -> Storage<DefinitionId, DefinitionDescriptorV1> {
    crate::runtime::founded_definitions().expect("founded definitions")
}

fn add_instruction(effective_height: u64) -> ChangeSetV1 {
    test_support::add_instruction(network(), effective_height)
}

fn notice() -> RuntimeNoticeV1 {
    RuntimeNoticeV1 {
        blocks: 100,
        ms: 1_000,
        approval_cushion_ms: 500,
    }
}

#[test]
fn a_change_set_that_adds_an_instruction_is_admitted() {
    let storage = published();
    let view = storage.view();
    let change = add_instruction(200);
    let id = admit(
        &change,
        &network(),
        100,
        notice(),
        &baseline(),
        &BTreeMap::new(),
        &PublishedDefinitions(&view),
    )
    .expect("admitted");
    assert_eq!(id, change.id().unwrap());
}

#[test]
fn admission_refuses_short_notice_foreign_networks_and_data_space_roots() {
    let storage = published();
    let view = storage.view();
    let lookup = PublishedDefinitions(&view);
    let admit_at = |change: &ChangeSetV1, height| {
        admit(
            change,
            &network(),
            height,
            notice(),
            &baseline(),
            &BTreeMap::new(),
            &lookup,
        )
    };
    assert!(
        admit_at(&add_instruction(199), 100).is_err(),
        "99 blocks of notice"
    );
    assert!(
        admit_at(&add_instruction(200), 100).is_ok(),
        "exactly 100 blocks"
    );

    let mut foreign = add_instruction(200);
    foreign.network =
        NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::prehashed([0x22; 32])));
    assert!(admit_at(&foreign, 100).is_err());

    let mut data_space = add_instruction(200);
    data_space.root = RootScopeV1::DataSpace(iroha_model_base::topology::DataSpaceId::new(7));
    assert!(admit_at(&data_space, 100).is_err());
}

#[test]
fn a_scheduled_change_set_needs_the_committed_provenance() {
    let signers = test_support::release_signers(3);
    let action = test_support::schedule(network(), 200);
    let cancel = RuntimeUpgradeActionV1::Cancel(ChangeSetId([9; 32]));
    let endorse = |signer: &KeyPair| test_support::endorse(&action, signer);
    let mut parameters = iroha_data_model::parameter::Parameters::default();
    check_provenance(&action, &[], &parameters).expect("no policy, no endorsement");
    let refusal = check_provenance(&action, &[endorse(&signers[0])], &parameters)
        .expect_err("nothing judges endorsements without a committed policy");
    assert!(
        refusal
            .to_string()
            .contains("committed provenance policy judges"),
        "{refusal}"
    );

    parameters.set_parameter(iroha_data_model::parameter::Parameter::Custom(
        test_support::provenance_policy(&signers, 2)
            .to_custom_parameter()
            .expect("valid policy"),
    ));
    check_provenance(&cancel, &[], &parameters).expect("a cancellation publishes nothing");
    assert!(check_provenance(&cancel, &[endorse(&signers[0])], &parameters).is_err());
    let refusal = check_provenance(&action, &[endorse(&signers[0])], &parameters)
        .expect_err("one of two endorsements");
    assert!(refusal.to_string().contains("1 of the 2"), "{refusal}");
    let repeated = [endorse(&signers[0]), endorse(&signers[0])];
    assert!(
        check_provenance(&action, &repeated, &parameters).is_err(),
        "one signer twice"
    );
    let outsider = KeyPair::from_seed(vec![0x7f; 32], Algorithm::Ed25519);
    let untrusted = [endorse(&signers[0]), endorse(&outsider)];
    assert!(
        check_provenance(&action, &untrusted, &parameters).is_err(),
        "untrusted signer"
    );
    let other = test_support::schedule(network(), 201);
    let misdirected = [
        endorse(&signers[0]),
        test_support::endorse(&other, &signers[1]),
    ];
    assert!(
        check_provenance(&action, &misdirected, &parameters).is_err(),
        "another change set"
    );
    let spam = [
        endorse(&signers[0]),
        endorse(&signers[1]),
        endorse(&signers[2]),
        endorse(&signers[2]),
    ];
    let refusal = check_provenance(&action, &spam, &parameters)
        .expect_err("more endorsements than trusted signers");
    assert!(
        refusal.to_string().contains("exceed the 3 trusted"),
        "{refusal}"
    );
    check_provenance(
        &action,
        &[endorse(&signers[2]), endorse(&signers[0])],
        &parameters,
    )
    .expect("two distinct trusted signers");
}

#[test]
fn a_change_set_meets_the_selection_scheduled_changes_leave() {
    let storage = published();
    let view = storage.view();
    let lookup = PublishedDefinitions(&view);
    let first = add_instruction(200);
    let first_id = first.id().unwrap();
    let mut scheduled = BTreeMap::new();
    scheduled.insert(
        first_id,
        ChangeSetRecordV1 {
            change_set: first.clone(),
            status: ChangeSetStatusV1::Scheduled,
            proposed_ms: 0,
            approved_height: 50,
            approved_ms: 0,
            notice: notice(),
            attestations: Vec::new(),
        },
    );
    // Starting again from the baseline conflicts with the scheduled change.
    assert!(
        admit(
            &add_instruction(300),
            &network(),
            100,
            notice(),
            &baseline(),
            &scheduled,
            &lookup
        )
        .is_err()
    );
    // Before the scheduled change, the baseline is still what a change meets.
    let (meets, staged) = projected_selection(&baseline(), &scheduled, 200, &lookup).unwrap();
    assert_eq!(meets, baseline());
    assert!(staged.is_empty());
    let (meets, staged) = projected_selection(&baseline(), &scheduled, 201, &lookup).unwrap();
    assert_eq!(meets.id().unwrap(), first.successor);
    assert_eq!(staged.len(), 2);
}

fn record(change: ChangeSetV1) -> ChangeSetRecordV1 {
    ChangeSetRecordV1 {
        change_set: change,
        status: ChangeSetStatusV1::Scheduled,
        proposed_ms: 1_000,
        approved_height: 100,
        approved_ms: 1_500,
        notice: notice(),
        attestations: Vec::new(),
    }
}

fn attest(
    change: &ChangeSetV1,
    committee: &[PublicKey],
    pair: &KeyPair,
) -> SignedSupportAttestationV1 {
    SupportAttestationV1 {
        network: change.network,
        root: change.root,
        change_set: change.id().unwrap(),
        predecessor: change.predecessor,
        successor: change.successor,
        committee: committee_digest(committee).unwrap(),
        tested_checkpoint: 90,
        valid_from: 100,
        valid_until: 1_000,
        release: [0x44; 32],
        validator: pair.public_key().clone(),
    }
    .sign(pair.private_key())
    .unwrap()
}

#[test]
fn activation_needs_every_validator_and_the_notice() {
    let validators: Vec<KeyPair> = (1..=4)
        .map(|seed| KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519))
        .collect();
    let committee: Vec<PublicKey> = validators
        .iter()
        .map(|pair| pair.public_key().clone())
        .collect();
    let change = add_instruction(200);
    let id = change.id().unwrap();
    let mut record = record(change.clone());
    for pair in &validators[..3] {
        let signed = attest(&change, &committee, pair);
        check_attestation(&record, id, &signed, &committee).expect("valid attestation");
        record_attestation(&mut record, signed);
    }
    assert!(
        !ready(&record, id, 200, 10_000, &committee),
        "three of four"
    );
    let last = attest(&change, &committee, &validators[3]);
    record_attestation(&mut record, last.clone());
    record_attestation(&mut record, last);
    assert_eq!(
        record.attestations.len(),
        4,
        "a repeated attestation replaces"
    );
    assert!(ready(&record, id, 200, 10_000, &committee));
    assert!(
        !ready(&record, id, 199, 10_000, &committee),
        "only at its height"
    );
    assert!(
        !ready(&record, id, 200, 1_999, &committee),
        "approval cushion"
    );

    let outsider = KeyPair::from_seed(vec![9; 32], Algorithm::Ed25519);
    assert!(
        check_attestation(
            &record,
            id,
            &attest(&change, &committee, &outsider),
            &committee
        )
        .is_err()
    );
    let rotated = &committee[1..];
    assert!(
        check_attestation(
            &record,
            id,
            &attest(&change, rotated, &validators[1]),
            &committee
        )
        .is_err(),
        "an attestation for another committee does not count"
    );
}

#[test]
fn restricting_takes_the_disable_notice() {
    let mut change = add_instruction(200);
    assert!(!needs_disable_notice(&change));
    change.transitions.push(SelectionTransitionV1::Restrict {
        definition: baseline().bindings[0].implementation,
        restriction: RestrictionKindV1::Deprecated,
    });
    assert!(needs_disable_notice(&change));
    let policy = RuntimeUpgradePolicyV1::default();
    assert_eq!(notice_for(&policy, &change), policy.disable);
    let _ = RestrictionEntryV1 {
        definition: DefinitionId([0; 32]),
        restriction: RestrictionKindV1::Disabled.at(1),
    };
}

fn validators() -> Vec<KeyPair> {
    (1..=4)
        .map(|seed| KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519))
        .collect()
}

/// A State whose committee is `validators` and whose World records `change` as scheduled,
/// attested by the first `attesting` validators.
fn scheduled_state(
    change: &ChangeSetV1,
    validators: &[KeyPair],
    attesting: usize,
) -> crate::state::State {
    let state = crate::state::State::new_for_testing(
        crate::state::World::default(),
        crate::kura::Kura::blank_kura_for_testing(),
        crate::query::store::LiveQueryStore::start_test(),
    );
    let committee: Vec<PublicKey> = validators
        .iter()
        .map(|pair| pair.public_key().clone())
        .collect();
    {
        let mut topology = state.commit_topology.block();
        topology.clear();
        topology.extend(
            committee
                .iter()
                .cloned()
                .map(iroha_model_base::peer::PeerId::new),
        );
        topology.commit();
    }
    let mut scheduled = record(change.clone());
    scheduled.proposed_ms = 0;
    scheduled.approved_ms = 0;
    for pair in &validators[..attesting] {
        record_attestation(&mut scheduled, attest(change, &committee, pair));
    }
    let mut change_sets = state.world.runtime_change_sets.block();
    change_sets.insert(change.id().unwrap(), scheduled);
    change_sets.commit();
    state
}

fn header_at(height: u64) -> iroha_data_model::block::BlockHeader {
    iroha_data_model::block::BlockHeader::new(
        std::num::NonZeroU64::new(height).expect("nonzero height"),
        None,
        None,
        10_000,
        0,
    )
}

#[test]
fn block_start_activates_a_supported_change_set_at_its_height() {
    let change = test_support::deprecate_baseline_bundle(network(), 150);
    let id = change.id().unwrap();
    let state = scheduled_state(&change, &validators(), 4);
    let block = state.try_block(header_at(150)).expect("block start");
    assert_eq!(
        block
            .world
            .runtime_change_sets
            .get(&id)
            .map(|record| record.status),
        Some(ChangeSetStatusV1::Activated(150))
    );
    assert_eq!(
        block.world.runtime_selection.get().id().unwrap(),
        change.successor,
        "the successor executes from the effective height"
    );
}

#[test]
fn block_start_lapses_a_change_set_that_would_strand_a_live_escrow() {
    let wire_id =
        iroha_data_model::isi::wire_id_of::<iroha_data_model::isi::escrow::ReleaseAssetEscrow>()
            .expect("registered");
    let change = test_support::restrict_baseline_interface(
        network(),
        150,
        wire_id,
        RestrictionKindV1::Disabled,
    );
    let id = change.id().unwrap();
    let status = |state: &crate::state::State| {
        let block = state.try_block(header_at(150)).expect("block start");
        let status = block
            .world
            .runtime_change_sets
            .get(&id)
            .map(|record| record.status);
        (status, block.world.runtime_selection.get().clone())
    };

    let state = scheduled_state(&change, &validators(), 4);
    {
        let escrow = test_support::live_escrow(1);
        let mut escrows = state.world.asset_escrows.block();
        escrows.insert(escrow.id, escrow);
        escrows.commit();
    }
    let (lapsed, selection) = status(&state);
    assert_eq!(lapsed, Some(ChangeSetStatusV1::Lapsed(150)));
    assert_eq!(selection, baseline(), "escrow release stays executable");

    let drained = scheduled_state(&change, &validators(), 4);
    let (activated, selection) = status(&drained);
    assert_eq!(activated, Some(ChangeSetStatusV1::Activated(150)));
    assert_eq!(selection.id().unwrap(), change.successor);
}

#[test]
fn block_start_lapses_a_change_set_without_every_validator() {
    let change = test_support::deprecate_baseline_bundle(network(), 150);
    let id = change.id().unwrap();
    let state = scheduled_state(&change, &validators(), 3);
    let block = state.try_block(header_at(150)).expect("block start");
    assert_eq!(
        block
            .world
            .runtime_change_sets
            .get(&id)
            .map(|record| record.status),
        Some(ChangeSetStatusV1::Lapsed(150))
    );
    assert_eq!(
        *block.world.runtime_selection.get(),
        baseline(),
        "a lapsed change set never applies"
    );
}

#[test]
fn block_start_stops_a_release_that_cannot_execute_the_successor() {
    let change = add_instruction(150);
    let state = scheduled_state(&change, &validators(), 4);
    let error = state
        .try_block(header_at(150))
        .err()
        .expect("this release does not implement the added instruction");
    let crate::state::StateBlockStartError::Storage(
        crate::state::StateStorageAdmissionError::UnsupportedRuntime {
            height: 150,
            reason,
        },
    ) = &error
    else {
        panic!("{error:?}");
    };
    assert!(
        reason.contains(&change.successor.to_string()),
        "the halt names the unsupported selection: {reason}"
    );
    assert_eq!(
        *state.world.runtime_selection.view().get(),
        baseline(),
        "the refused block leaves the committed selection"
    );
}
