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
fn unsupported_storage_effects_are_rejected_before_scheduling() {
    let storage = published();
    let view = storage.view();
    let lookup = PublishedDefinitions(&view);
    for (change, operation) in [
        (
            test_support::retire_baseline_storage(network(), 200),
            "storage migration cutover",
        ),
        (
            test_support::activate_migration(network(), 200),
            "migration execution",
        ),
    ] {
        change
            .apply(&baseline(), &lookup)
            .expect("the descriptor transition is closed");
        let error = admit(
            &change,
            &network(),
            100,
            notice(),
            &baseline(),
            &BTreeMap::new(),
            &lookup,
        )
        .expect_err("closed definitions cannot substitute for lifecycle effects");
        assert!(error.to_string().contains(operation), "{error}");
    }
    for cell in [false, true] {
        let storage = test_support::install_storage(network(), 200, cell);
        admit(
            &storage,
            &network(),
            100,
            notice(),
            &baseline(),
            &BTreeMap::new(),
            &lookup,
        )
        .expect("new storage may be proposed before its implementing release is installed");
    }
}

#[test]
fn completion_selection_edits_cannot_substitute_for_registered_custody() {
    use iroha_data_model::runtime::definition::CompletionClosureV1;
    let catalog = release_catalog().expect("catalog");
    let baseline = baseline();
    let closure = CompletionClosureV1 {
        obligation: [0x75; 32],
        definitions: vec![baseline.bindings[0].implementation],
    };
    let mut retained = baseline.clone();
    retained.completion_closures.push(closure.clone());
    let storage = published();
    let view = storage.view();
    for (predecessor, successor, transition) in [
        (
            &baseline,
            &retained,
            SelectionTransitionV1::RetainCompletion(closure.clone()),
        ),
        (
            &retained,
            &baseline,
            SelectionTransitionV1::ReleaseCompletion(closure.obligation),
        ),
    ] {
        let mut change = test_support::deprecate_baseline_bundle(network(), 200);
        change.predecessor = predecessor.id().unwrap();
        change.successor = successor.id().unwrap();
        change.transitions = vec![transition];
        change
            .apply(predecessor, &catalog.definitions)
            .expect("a structurally valid selection edit");
        let error = admit(
            &change,
            &network(),
            100,
            notice(),
            predecessor,
            &BTreeMap::new(),
            &PublishedDefinitions(&view),
        )
        .expect_err("selection entries provide no kernel registration or transfer proof");
        assert!(
            error
                .to_string()
                .contains("kernel-registered completion capabilities"),
            "{error}"
        );
    }
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
    for wire_id in [
        iroha_data_model::isi::wire_id_of::<iroha_data_model::isi::escrow::ReleaseAssetEscrow>()
            .expect("registered"),
        // The baseline escrow implementation can invoke this interface. Keeping its direct
        // completion bound is insufficient if a declared callee is disabled.
        iroha_data_model::isi::wire_id_of::<iroha_data_model::isi::Log>()
            .expect("registered callee"),
    ] {
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
fn block_start_lapses_attested_but_unimplemented_storage_effects() {
    for change in [
        test_support::retire_baseline_storage(network(), 150),
        test_support::activate_migration(network(), 150),
    ] {
        let id = change.id().expect("identity");
        let state = scheduled_state(&change, &validators(), 4);
        let block = state
            .try_block(header_at(150))
            .expect("unsupported effects must not halt the old selection");
        assert_eq!(
            block
                .world
                .runtime_change_sets
                .get(&id)
                .map(|record| record.status),
            Some(ChangeSetStatusV1::Lapsed(150)),
        );
        assert_eq!(*block.world.runtime_selection.get(), baseline());
        assert!(
            change.publish.iter().all(|descriptor| block
                .world
                .runtime_definitions
                .get(&descriptor.id().expect("definition identity"))
                .is_none()),
            "a lapsed change publishes no definitions"
        );
    }
}

#[test]
fn merged_lifecycle_capacity_support_stops_at_activation_for_both_owner_profiles() {
    use iroha_data_model::{
        block::output_budget::{ExecutionOutputPhase as Phase, ExecutionOutputTerminalCeilings},
        parameter::{ExecutionOutputPolicyV1, Parameter, system::BlockParameter},
    };
    let catalog = release_catalog().unwrap();
    let old = ExecutionOutputTerminalCeilings::derive().unwrap();
    let mut policy = ExecutionOutputPolicyV1::bootstrap();
    policy.max_pipeline_triggers = 0;
    policy.max_time_triggers = 1;
    policy.max_time_invocations = 1;
    policy.max_outputs = 2;
    policy.max_output_bytes = old
        .for_phase(Phase::Network)
        .max(old.for_phase(Phase::Time));
    policy.validate().unwrap();
    for ironwood_first in [false, true] {
        let mut published: BTreeMap<_, _> = catalog
            .baseline_definitions
            .iter()
            .map(|id| (*id, catalog.definitions[id].clone()))
            .collect();
        let previous = if ironwood_first {
            let change =
                crate::runtime::ironwood::activation_change_set(network(), 100, [0x91; 32])
                    .unwrap();
            let applied = change.apply(&catalog.baseline, &published).unwrap();
            published.extend(applied.published);
            applied.selection
        } else {
            catalog.baseline.clone()
        };
        let change = crate::runtime::lifecycle_activation::from_selection(
            catalog.clone(),
            &previous,
            &published,
            network(),
            150,
            [0x92; 32],
        )
        .unwrap();
        let state = scheduled_state(&change, &validators(), 4);
        {
            let mut definitions = state.world.runtime_definitions.block();
            for (id, definition) in published {
                definitions.insert(id, definition);
            }
            definitions.commit();
            let mut selection = state.world.runtime_selection.block();
            *selection.get_mut() = previous.clone();
            selection.commit();
            let mut parameters =
                crate::state::parameter_cell_block_for_testing(&state.world).unwrap();
            crate::state::set_parameter_cell_for_testing(
                &mut parameters,
                Parameter::Block(BlockParameter::ExecutionOutput(policy)),
                state.world.sccp_committee_generations.allocation_budget(),
            )
            .unwrap();
            parameters.commit();
        }
        // Existing output semantics must remain executable before the effective height.
        drop(
            state
                .try_block(header_at(149))
                .expect("predecessor capacity is sufficient"),
        );
        let error = state
            .try_block(header_at(150))
            .err()
            .expect("V2 terminal capacity is unsupported");
        assert!(
            matches!(error,
            crate::state::StateBlockStartError::Storage(
                crate::state::StateStorageAdmissionError::UnsupportedRuntime { height: 150, ref reason }
            ) if reason == "execution output envelope has invalid terminal ceilings"),
            "{error:?}"
        );
        assert_eq!(*state.world.runtime_selection.view().get(), previous);
        assert_eq!(
            state
                .world
                .runtime_change_sets
                .view()
                .get(&change.id().unwrap())
                .unwrap()
                .status,
            ChangeSetStatusV1::Scheduled,
            "a support stop must neither lapse nor commit the activation"
        );
    }
}

#[test]
fn ironwood_cutover_activates_only_with_all_four_validator_attestations() {
    let change =
        crate::runtime::ironwood::activation_change_set(network(), 150, [0x51; 32]).unwrap();
    let storage = published();
    admit(
        &change,
        &network(),
        50,
        notice(),
        &baseline(),
        &BTreeMap::new(),
        &PublishedDefinitions(&storage.view()),
    )
    .unwrap();
    for attesting in [3, 4] {
        let state = scheduled_state(&change, &validators(), attesting);
        let before = state.try_block(header_at(149)).unwrap();
        assert!(!crate::runtime::ironwood::enabled(
            before.world.runtime_selection.get()
        ));
        drop(before);
        let block = state.try_block(header_at(150)).unwrap();
        let active = attesting == 4;
        assert_eq!(
            crate::runtime::ironwood::enabled(block.world.runtime_selection.get()),
            active
        );
        assert_eq!(
            block
                .world
                .runtime_change_sets
                .get(&change.id().unwrap())
                .unwrap()
                .status,
            if active {
                ChangeSetStatusV1::Activated(150)
            } else {
                ChangeSetStatusV1::Lapsed(150)
            }
        );
        assert_eq!(
            block.world.runtime_selection.get_before_block(),
            &baseline()
        );
    }
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

#[test]
fn block_start_stops_a_release_that_cannot_install_the_successor_storage() {
    for cell in [false, true] {
        let change = test_support::install_storage(network(), 150, cell);
        let state = scheduled_state(&change, &validators(), 4);
        let error = state
            .try_block(header_at(150))
            .err()
            .expect("this release does not implement the installed storage");
        assert!(
            matches!(
                error,
                crate::state::StateBlockStartError::Storage(
                    crate::state::StateStorageAdmissionError::UnsupportedRuntime {
                        height: 150,
                        ..
                    }
                )
            ),
            "{error:?}"
        );
        assert_eq!(*state.world.runtime_selection.view().get(), baseline());
    }
}

#[test]
fn block_start_activates_an_addition_and_installs_its_cell() {
    use crate::runtime::catalog::{
        additions::test_additions::{CELL_INITIAL, LOG, LOG_MODULE, LOG_WIRE_ID},
        test_catalog,
    };
    test_catalog::with_additions(&[LOG], || {
        let catalog = release_catalog().expect("test catalog");
        let addition = &catalog.additions[0];
        let founded = crate::runtime::founded_definitions().expect("founded definitions");
        let change = catalog
            .activation(
                LOG_MODULE,
                &catalog.baseline,
                &PublishedDefinitions(&founded.view()),
                network(),
                150,
                [0x51; 32],
            )
            .expect("the addition activates from the baseline");
        let id = change.id().unwrap();
        let state = scheduled_state(&change, &validators(), 4);
        assert_eq!(
            *state.world.soracloud_sequence_watermark.view().get(),
            0,
            "an uninstalled cell holds no value"
        );
        let block = state
            .try_block(header_at(150))
            .expect("this release implements the addition");
        assert_eq!(
            block
                .world
                .runtime_change_sets
                .get(&id)
                .map(|record| record.status),
            Some(ChangeSetStatusV1::Activated(150))
        );
        let selection = block.world.runtime_selection.get();
        assert_eq!(selection.id().unwrap(), change.successor);
        assert!(
            selection
                .bindings
                .iter()
                .any(|binding| binding.wire_id == LOG_WIRE_ID
                    && binding.implementation == addition.implementation)
        );
        assert_eq!(
            *block.world.soracloud_sequence_watermark.get(),
            CELL_INITIAL,
            "installation writes the published initial value"
        );
        for definition in addition.definitions() {
            assert!(block.world.runtime_definitions.get(&definition).is_some());
        }
    });
}

#[test]
fn block_start_lapses_a_change_set_that_would_strand_a_live_amx_transaction() {
    use iroha_data_model::{
        isi::sumeragi_amx::ExpireAmxV1,
        sumeragi_amx::{AmxBeginV1, AmxGlobalEntryV1, SumeragiAmxState},
    };
    use iroha_model_base::topology::DataSpaceId;
    let change = test_support::restrict_baseline_interface(
        network(),
        150,
        ExpireAmxV1::WIRE_ID,
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
        let mut amx = state.world.sumeragi_amx.block();
        *amx.get_mut() = SumeragiAmxState {
            dataspaces: Vec::new(),
            transactions: vec![AmxGlobalEntryV1 {
                begin: AmxBeginV1 {
                    tx: [0x71; 32],
                    participants: vec![DataSpaceId::new(41), DataSpaceId::new(42)],
                    deadline: 10,
                    expires_at_ms: 1_000,
                },
                begun_at: 1,
                begun_time_ms: 1,
                original_cadence_ms: 100,
                yes: Vec::new(),
                decided: None,
            }],
        };
        amx.commit();
    }
    let (lapsed, selection) = status(&state);
    assert_eq!(lapsed, Some(ChangeSetStatusV1::Lapsed(150)));
    assert_eq!(
        selection,
        baseline(),
        "the undecided transaction can still expire"
    );

    let decided = scheduled_state(&change, &validators(), 4);
    let (activated, selection) = status(&decided);
    assert_eq!(activated, Some(ChangeSetStatusV1::Activated(150)));
    assert_eq!(selection.id().unwrap(), change.successor);
}

#[test]
fn ironwood_future_codec_without_a_cutover_refuses_admission_and_lapses() {
    use iroha_data_model::runtime::definition::SchemaV1;
    let catalog = release_catalog().unwrap();
    let extension = catalog.ironwood.as_ref().unwrap();
    let previous = &extension.selection;
    let old_storage = extension.world_storage["world.smart_contract_state"];
    let DefinitionDescriptorV1::Storage(mut storage) = catalog.definitions[&old_storage].clone()
    else {
        unreachable!()
    };
    let SchemaV1::Codec(codec) = &mut storage.value else {
        unreachable!()
    };
    codec.push_str("-future-qualified-codec");
    let storage = DefinitionDescriptorV1::Storage(storage);
    let new_storage = storage.id().unwrap();
    let old_bundle = previous.bindings[0].implementation;
    let DefinitionDescriptorV1::Implementation(mut bundle) =
        catalog.definitions[&old_bundle].clone()
    else {
        unreachable!()
    };
    for accesses in [&mut bundle.reads, &mut bundle.writes, &mut bundle.scans] {
        for owner in accesses.iter_mut() {
            if *owner == old_storage {
                *owner = new_storage;
            }
        }
        accesses.sort_unstable();
    }
    let bundle = DefinitionDescriptorV1::Implementation(bundle);
    let new_bundle = bundle.id().unwrap();
    let mut next = previous.clone();
    next.installed_storage.retain(|id| *id != old_storage);
    next.installed_storage.push(new_storage);
    next.installed_storage.sort_unstable();
    next.active.retain(|id| *id != old_bundle);
    next.active.push(new_bundle);
    next.active.sort_unstable();
    next.restrictions
        .retain(|entry| entry.definition != old_bundle);
    for binding in &mut next.bindings {
        binding.implementation = new_bundle;
    }
    let mut transitions = vec![
        SelectionTransitionV1::Restrict {
            definition: old_bundle,
            restriction: RestrictionKindV1::Disabled,
        },
        SelectionTransitionV1::Deactivate(old_bundle),
        SelectionTransitionV1::RetireStorage(old_storage),
        SelectionTransitionV1::InstallStorage(new_storage),
        SelectionTransitionV1::Activate(new_bundle),
    ];
    transitions.extend(
        next.bindings
            .iter()
            .cloned()
            .map(SelectionTransitionV1::Bind),
    );
    let mut publish = vec![storage, bundle];
    publish.sort_by_key(|descriptor| descriptor.id().unwrap());
    let change = ChangeSetV1 {
        network: network(),
        root: RootScopeV1::Universal,
        predecessor: previous.id().unwrap(),
        successor: next.id().unwrap(),
        effective_height: 150,
        publish,
        transitions,
        qualification: [0x68; 32],
    };
    let applied = change.apply(previous, &catalog.definitions).unwrap();
    assert_eq!(applied.selection, next);
    assert!(catalog.unsupported(&next).is_some());
    let mut definitions = catalog.definitions.clone();
    definitions.extend(applied.published);
    assert!(crate::runtime::ironwood::validate_cutover(previous, &next, &definitions).is_err());
    let published: Storage<_, _> = catalog
        .definitions
        .iter()
        .map(|(id, descriptor)| (*id, descriptor.clone()))
        .collect();
    let error = admit(
        &change,
        &network(),
        50,
        notice(),
        previous,
        &BTreeMap::new(),
        &PublishedDefinitions(&published.view()),
    )
    .expect_err("an unknown codec has no implemented storage migration cutover");
    assert!(
        error
            .to_string()
            .contains("unimplemented storage migration cutover"),
        "{error}"
    );
    // Injected unit fixtures independently exercise block-start support checks.
    // They are not admitted proposals or governance qualification evidence.
    for attesting in [3, 4] {
        let mut state = scheduled_state(&change, &validators(), attesting);
        crate::runtime::ironwood::install_for_test(&mut state.world);
        // An unknown definition cannot legalize a forbidden storage transition.
        // Valid additive unknown successors separately exercise the release stop.
        let block = state
            .try_block(header_at(150))
            .expect("the invalid cutover lapses without halting the original selection");
        assert_eq!(block.world.runtime_selection.get(), previous);
        assert_eq!(
            block
                .world
                .runtime_change_sets
                .get(&change.id().unwrap())
                .unwrap()
                .status,
            ChangeSetStatusV1::Lapsed(150),
            "even unanimous attestations cannot admit an unimplemented cutover"
        );
        assert!(change.publish.iter().all(|descriptor| {
            block
                .world
                .runtime_definitions
                .get(&descriptor.id().unwrap())
                .is_none()
        }));
    }
}

#[test]
fn sorafs_authority_activation_checks_inventory_before_publishing_any_definition() {
    use iroha_data_model::{Identifiable, Registrable, permission::Permissions, role::Role};
    use iroha_executor_data_model::permission::sorafs::CanSetSorafsReservePolicy;
    use iroha_test_samples::ALICE_ID;

    let catalog = release_catalog().unwrap();
    let storage = published();
    let view = storage.view();
    let change = crate::runtime::sorafs_authority::draft(
        catalog,
        &baseline(),
        &PublishedDefinitions(&view),
        network(),
        150,
        [0x75; 32],
    )
    .unwrap();
    let id = change.id().unwrap();
    // Empty inventory activates. A direct grant, inherited grant, or reserved empty
    // role must lapse at the exact cut without publishing any successor definition.
    for inventory in 0..=3 {
        let state = scheduled_state(&change, &validators(), 4);
        if inventory == 1 {
            let mut permissions = state.world.account_permissions.block();
            permissions.insert(
                ALICE_ID.clone(),
                Permissions::from([CanSetSorafsReservePolicy.into()]),
            );
            permissions.commit();
        } else if inventory >= 2 {
            let role_id = if inventory == 2 {
                "ordinary_role"
            } else {
                "sorafs_gateway_compliance_operator"
            };
            let mut role = Role::new(role_id.parse().unwrap(), ALICE_ID.clone());
            if inventory == 2 {
                role = role.add_permission(CanSetSorafsReservePolicy);
            }
            let role = role.build(&ALICE_ID);
            let mut roles = state.world.roles.block();
            roles.insert(role.id().clone(), role);
            roles.commit();
        }
        let block = state.try_block(header_at(150)).expect("activation cut");
        assert_eq!(
            block.world.runtime_change_sets.get(&id).unwrap().status,
            if inventory == 0 {
                ChangeSetStatusV1::Activated(150)
            } else {
                ChangeSetStatusV1::Lapsed(150)
            }
        );
        assert_eq!(
            block.world.runtime_selection.get().id().unwrap(),
            if inventory == 0 {
                change.successor
            } else {
                change.predecessor
            }
        );
        for descriptor in &change.publish {
            assert_eq!(
                block
                    .world
                    .runtime_definitions
                    .get(&descriptor.id().unwrap())
                    .is_some(),
                inventory == 0,
                "a lapsed cut must leave unpublished definitions absent"
            );
        }
        assert!(
            block
                .world
                .sorafs_authority_proposals
                .iter()
                .next()
                .is_none()
        );
        assert!(block.world.sorafs_authority_heads.iter().next().is_none());
        if inventory == 1 {
            assert!(
                block
                    .world
                    .account_permissions
                    .get(&ALICE_ID)
                    .unwrap()
                    .contains(&CanSetSorafsReservePolicy.into()),
                "the old direct grant must remain unchanged"
            );
        }
    }
}
