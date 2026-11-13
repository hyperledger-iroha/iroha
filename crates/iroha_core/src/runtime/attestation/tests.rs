//! Release support and node-owned attestation of executable runtime transitions.

use iroha_crypto::{Algorithm, Hash, HashOf};
use iroha_data_model::{
    NetworkId,
    runtime::{
        definition::{DefinitionDescriptorV1, ImplementationDescriptorV1},
        lifecycle::{ChangeSetV1, RuntimeNoticeV1},
    },
};

use super::*;
use crate::runtime::change_set::{check_attestation, test_support};

fn network() -> NetworkId {
    NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::prehashed([0x11; 32])))
}

fn validators() -> Vec<KeyPair> {
    (1..=4)
        .map(|seed| KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519))
        .collect()
}

fn keys(pairs: &[KeyPair]) -> Vec<PublicKey> {
    pairs.iter().map(|pair| pair.public_key().clone()).collect()
}

/// A State whose committee is `committee` and whose World records `change` with `status`.
fn state_with(
    change: &ChangeSetV1,
    committee: &[PublicKey],
    status: ChangeSetStatusV1,
) -> crate::state::State {
    let state = crate::state::State::new_for_testing(
        crate::state::World::default(),
        crate::kura::Kura::blank_kura_for_testing(),
        crate::query::store::LiveQueryStore::start_test(),
    );
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
    let mut change_sets = state.world.runtime_change_sets.block();
    change_sets.insert(
        change.id().expect("change-set identity"),
        ChangeSetRecordV1 {
            change_set: change.clone(),
            status,
            proposed_ms: 0,
            approved_height: 10,
            approved_ms: 0,
            notice: RuntimeNoticeV1 {
                blocks: 100,
                ms: 0,
                approval_cushion_ms: 0,
            },
            attestations: Vec::new(),
        },
    );
    change_sets.commit();
    state
}

#[test]
fn a_supported_change_set_is_attested_with_the_node_key_for_its_committee() {
    let change = test_support::deprecate_baseline_bundle(network(), 150);
    let id = change.id().expect("change-set identity");
    let pairs = validators();
    let committee = keys(&pairs);
    let state = state_with(&change, &committee, ChangeSetStatusV1::Scheduled);
    let world = state.world_view();

    let signed = sign_support(&world, &committee, 42, id, &pairs[2], [0x55; 32])
        .expect("this release executes the successor");

    let attestation = &signed.attestation;
    assert_eq!(attestation.validator, *pairs[2].public_key());
    assert_eq!(attestation.successor, change.successor);
    assert_eq!(attestation.predecessor, change.predecessor);
    assert_eq!(
        attestation.committee,
        committee_digest(&committee).expect("committee digest")
    );
    assert_eq!(attestation.release, [0x55; 32]);
    assert_eq!(attestation.tested_checkpoint, 42);
    assert_eq!(
        (attestation.valid_from, attestation.valid_until),
        (150, 151)
    );
    let record = world
        .runtime_change_sets()
        .get(&id)
        .expect("recorded change set");
    check_attestation(record, id, &signed, &committee)
        .expect("Core accepts the node's attestation");
}

#[test]
fn unfinished_game_export_is_not_drafted_or_signed_without_rewriting_frozen_consensus() {
    use crate::runtime::catalog::additions::{ENDORSED_RUNTIME_UPGRADE_PROPOSAL, GAME_PARTITIONS};
    let catalog = release_catalog().unwrap();
    let world = crate::state::World::new();
    crate::runtime::activate_additions_for_testing(
        &world,
        ENDORSED_RUNTIME_UPGRADE_PROPOSAL.module,
    )
    .unwrap();
    let change = {
        let view = world.view();
        assert!(
            crate::runtime::draft_addition(&view, network(), GAME_PARTITIONS.module, 200, [1; 32])
                .unwrap_err()
                .to_string()
                .contains("export completion custody")
        );
        catalog
            .activation(
                GAME_PARTITIONS.module,
                view.runtime_selection(),
                &PublishedDefinitions(view.runtime_definitions()),
                network(),
                200,
                [1; 32],
            )
            .unwrap()
    };
    let id = change.id().unwrap();
    {
        let mut block = world.block();
        block.runtime_change_sets.insert(
            id,
            ChangeSetRecordV1 {
                change_set: change.clone(),
                status: ChangeSetStatusV1::Scheduled,
                proposed_ms: 0,
                approved_height: 10,
                approved_ms: 0,
                notice: RuntimeNoticeV1 {
                    blocks: 100,
                    ms: 0,
                    approval_cushion_ms: 0,
                },
                attestations: Vec::new(),
            },
        );
        block.commit();
    }
    let world = world.view();
    let pairs = validators();
    let committee = keys(&pairs);
    assert!(matches!(
        successor_selection(&world, id),
        Err(NodeAttestationError::Unqualified { .. })
    ));
    assert!(matches!(
        sign_support(&world, &committee, 42, id, &pairs[0], [1; 32]),
        Err(NodeAttestationError::Unqualified { .. })
    ));
    let selection = successor(
        &change,
        id,
        world.runtime_selection(),
        &BTreeMap::new(),
        &PublishedDefinitions(world.runtime_definitions()),
    )
    .expect("the frozen consensus kernel retains its original transition semantics");
    assert_eq!(
        release_support(catalog, &selection),
        ReleaseSupport::Supported,
        "local qualification must not relabel historical code as missing at startup"
    );
    assert!(
        super::super::catalog::unqualified_native_activation(
            &selection,
            &selection,
            &catalog.definitions
        )
        .is_none(),
        "retained historical definitions do not become newly activated"
    );
}

#[test]
fn a_release_that_lacks_a_successor_definition_refuses_to_attest() {
    let change = test_support::add_instruction(network(), 150);
    let id = change.id().expect("change-set identity");
    let pairs = validators();
    let committee = keys(&pairs);
    let state = state_with(&change, &committee, ChangeSetStatusV1::Scheduled);
    let world = state.world_view();

    let error = sign_support(&world, &committee, 42, id, &pairs[0], [0x55; 32])
        .expect_err("this release does not implement the added instruction");
    let NodeAttestationError::Unsupported {
        selection,
        definition,
    } = error
    else {
        panic!("{error:?}");
    };
    assert_eq!(selection, change.successor);
    let (_, successor) = successor_selection(&world, id).expect("the change set applies");
    assert_eq!(
        successor.id().expect("successor identity"),
        change.successor
    );
    assert_eq!(
        release_support(release_catalog().expect("catalog"), &successor),
        ReleaseSupport::Missing(definition)
    );
    assert!(
        change
            .publish
            .iter()
            .any(|descriptor| descriptor.id().expect("identity") == definition),
        "the missing definition is one the change set publishes"
    );
}

#[test]
fn a_release_refuses_to_attest_unimplemented_storage_effects() {
    let pairs = validators();
    let committee = keys(&pairs);
    for (change, operation) in [
        (
            test_support::retire_baseline_storage(network(), 150),
            "storage migration cutover",
        ),
        (
            test_support::activate_migration(network(), 150),
            "migration execution",
        ),
    ] {
        let id = change.id().expect("identity");
        let state = state_with(&change, &committee, ChangeSetStatusV1::Scheduled);
        let world = state.world_view();
        let error = sign_support(&world, &committee, 42, id, &pairs[0], [0x55; 32])
            .expect_err("understanding the successor's definitions is insufficient");
        assert!(
            matches!(error, NodeAttestationError::NoLongerApplies(_)),
            "{error}"
        );
        assert!(error.to_string().contains(operation), "{error}");
    }
    // Installing storage is an effect this release performs; it still attests only storage its
    // release implements.
    for cell in [false, true] {
        let change = test_support::install_storage(network(), 150, cell);
        let id = change.id().expect("identity");
        let state = state_with(&change, &committee, ChangeSetStatusV1::Scheduled);
        let world = state.world_view();
        let error = sign_support(&world, &committee, 42, id, &pairs[0], [0x55; 32])
            .expect_err("this release does not implement the installed storage");
        assert!(
            matches!(error, NodeAttestationError::Unsupported { .. }),
            "{error}"
        );
    }
    let retirement = test_support::retire_baseline_storage(network(), 150);
    let catalog = release_catalog().expect("catalog");
    let applied = retirement
        .apply(&catalog.baseline, &catalog.definitions)
        .expect("closed selection");
    assert_eq!(
        release_support(catalog, &applied.selection),
        ReleaseSupport::Supported,
        "the regression needs no unknown definition: it removes compiled storage"
    );
}

#[test]
fn only_a_committee_member_attests_a_recorded_scheduled_change_set() {
    let change = test_support::deprecate_baseline_bundle(network(), 150);
    let id = change.id().expect("change-set identity");
    let pairs = validators();
    let committee = keys(&pairs[..3]);
    let state = state_with(&change, &committee, ChangeSetStatusV1::Scheduled);
    let world = state.world_view();
    assert_eq!(
        sign_support(&world, &committee, 42, id, &pairs[3], [0x55; 32]),
        Err(NodeAttestationError::NotInCommittee)
    );
    let unknown = ChangeSetId([0xEE; 32]);
    assert_eq!(
        sign_support(&world, &committee, 42, unknown, &pairs[0], [0x55; 32]),
        Err(NodeAttestationError::NotRecorded(unknown))
    );

    let activated = state_with(&change, &committee, ChangeSetStatusV1::Activated(150));
    assert_eq!(
        sign_support(
            &activated.world_view(),
            &committee,
            42,
            id,
            &pairs[0],
            [0x55; 32]
        ),
        Err(NodeAttestationError::NotScheduled(id))
    );
}

#[test]
fn the_release_digest_binds_the_build_and_every_implemented_definition() {
    let catalog = release_catalog().expect("catalog");
    let build = BuildStatus {
        version: "3.0.0".into(),
        git_commit_sha: "0123456789abcdef0123456789abcdef01234567".into(),
        dpn_validator_release_commit: "unknown".into(),
        cargo_features: "default".into(),
        target_triple: "aarch64-apple-darwin".into(),
        wire_schema_hash: "00".repeat(32),
    };
    let digest = release_digest(&build, catalog).expect("release digest");
    assert_eq!(release_digest(&build, catalog), Ok(digest), "deterministic");

    let mut other_build = build.clone();
    other_build.cargo_features = "default,telemetry".into();
    assert_ne!(release_digest(&other_build, catalog), Ok(digest));

    let mut fewer = catalog.clone();
    let dropped = fewer
        .definitions
        .keys()
        .copied()
        .find(|id| {
            matches!(
                fewer.definitions[id],
                DefinitionDescriptorV1::Implementation(ImplementationDescriptorV1 { .. })
            )
        })
        .expect("an implementation");
    fewer.definitions.remove(&dropped);
    assert_ne!(release_digest(&build, &fewer), Ok(digest));
}

#[test]
fn the_release_advertises_dormant_profiles_while_each_root_requires_its_selection() {
    let catalog = release_catalog().expect("catalog");
    let implemented = release_host_profiles(catalog);
    assert!(
        !implemented.is_empty(),
        "the release implements the baseline host profile"
    );
    assert!(implemented.iter().all(|id| matches!(
        catalog.definitions[id],
        DefinitionDescriptorV1::HostProfile(_)
    )));
    let required = active_host_profiles(&catalog.baseline, &catalog.definitions);
    assert_eq!(required.len(), 1);
    assert!(required.iter().all(|id| implemented.contains(id)));
    let named = catalog.definitions.iter().find_map(|(id, descriptor)| {
        matches!(descriptor, DefinitionDescriptorV1::HostProfile(host)
            if host.abi_hash == ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1RuntimeLifecycle))
            .then_some(*id)
    }).expect("named lifecycle host is compiled");
    assert!(implemented.contains(&named));
    assert!(!required.contains(&named));
    let mut selected = catalog.baseline.clone();
    selected.active.push(named);
    selected.active.sort();
    selected.active.dedup();
    assert_eq!(
        active_host_profiles(&selected, &catalog.definitions),
        implemented
    );
    let state = crate::state::State::new_for_testing(
        crate::state::World::default(),
        crate::kura::Kura::blank_kura_for_testing(),
        crate::query::store::LiveQueryStore::start_test(),
    );
    assert_eq!(
        committed_host_profiles(&state.world_view()),
        required,
        "a World founded on the baseline requires exactly its host profiles"
    );
}
