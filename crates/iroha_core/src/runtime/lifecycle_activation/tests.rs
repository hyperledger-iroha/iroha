//! Governed change-set activation and read-only provenance; fixture scheduling is not governance qualification.

use super::*;
use crate::{
    state::{StorageReadOnly, World, WorldReadOnly},
    sumeragi::test_chain::{CertifiedTestChain, TestChainConfig, fixture_keys},
};
use iroha_data_model::runtime::lifecycle::{
    ChangeSetRecordV1, RuntimeNoticeV1, SupportAttestationV1, committee_digest,
};

/// Supply an approved record only through the test setup boundary. Its admission,
/// validator signatures, activation readiness and subsequent blocks use production code.
fn schedule(chain: &CertifiedTestChain, attest: bool) -> ChangeSetId {
    let view = chain.state().view();
    let change =
        prepare_lifecycle_change_set(view.world(), chain.network_id(), 3, [0x91; 32]).unwrap();
    let notice = RuntimeNoticeV1 {
        blocks: 1,
        ms: 0,
        approval_cushion_ms: 0,
    };
    let id = super::super::change_set::admit(
        &change,
        &chain.network_id(),
        chain.height(),
        notice,
        view.world().runtime_selection(),
        &std::collections::BTreeMap::new(),
        &super::super::change_set::PublishedDefinitions(view.world().runtime_definitions()),
    )
    .unwrap();
    drop(view);
    let committee: Vec<_> = chain
        .validators()
        .iter()
        .map(|(peer, _)| peer.public_key().clone())
        .collect();
    let mut record = ChangeSetRecordV1 {
        change_set: change.clone(),
        status: ChangeSetStatusV1::Scheduled,
        proposed_ms: 0,
        approved_height: 1,
        approved_ms: 0,
        notice,
        attestations: Vec::new(),
    };
    if attest {
        for key in fixture_keys() {
            let signed = SupportAttestationV1 {
                network: change.network,
                root: change.root,
                change_set: id,
                predecessor: change.predecessor,
                successor: change.successor,
                committee: committee_digest(&committee).unwrap(),
                tested_checkpoint: 1,
                valid_from: 1,
                valid_until: 4,
                release: [0x92; 32],
                validator: key.public_key().clone(),
            }
            .sign(key.private_key())
            .unwrap();
            super::super::change_set::record_attestation(&mut record, signed);
        }
    }
    chain.setup_world_at(1_001, |tx| {
        tx.world.runtime_change_sets.insert(id, record);
    });
    id
}

#[test]
fn governed_lifecycle_boundary_and_fingerprint_follow_committed_activation() {
    let mut chain = CertifiedTestChain::start(TestChainConfig::new(World::new(), 1_000)).unwrap();
    let before = chain
        .state()
        .view()
        .lifecycle_consensus_config_fingerprint()
        .unwrap();
    let id = schedule(&chain, true);
    // A proposed or scheduled activation is not a committed rollout boundary.
    let policy = chain.state().view().lifecycle_activation_policy().unwrap();
    assert!(!policy.active);
    assert_eq!(policy.activation, None);
    assert_eq!(
        chain
            .state()
            .view()
            .lifecycle_consensus_config_fingerprint()
            .unwrap(),
        before
    );
    chain.commit_at(1_002, vec![]);
    assert!(
        !chain
            .state()
            .view()
            .lifecycle_activation_policy()
            .unwrap()
            .active
    );
    assert!(
        chain
            .committed(2)
            .block()
            .execution_outputs()
            .iter()
            .all(|row| row.trigger_lifecycle().is_none())
    );
    chain.commit_at(1_003, vec![]);
    let policy = chain.state().view().lifecycle_activation_policy().unwrap();
    assert!(policy.active);
    assert_eq!(
        policy.activation,
        Some((id, std::num::NonZeroU64::new(3).unwrap()))
    );
    assert!(
        chain
            .committed(3)
            .block()
            .execution_outputs()
            .iter()
            .all(|row| row.trigger_lifecycle().is_some())
    );
    let selected = chain
        .state()
        .view()
        .lifecycle_consensus_config_fingerprint()
        .unwrap();
    assert_ne!(before, selected);
    chain.commit_at(1_004, vec![]);
    assert_eq!(
        chain.state().view().lifecycle_activation_policy().unwrap(),
        policy
    );
    assert_eq!(
        chain
            .state()
            .view()
            .lifecycle_consensus_config_fingerprint()
            .unwrap(),
        selected
    );
}

#[test]
fn missing_unanimous_support_never_selects_lifecycle_boundary() {
    let mut chain = CertifiedTestChain::start(TestChainConfig::new(World::new(), 1_000)).unwrap();
    let id = schedule(&chain, false);
    chain.commit_at(1_002, vec![]);
    chain.commit_at(1_003, vec![]);
    let view = chain.state().view();
    assert_eq!(
        view.world().runtime_change_sets().get(&id).unwrap().status,
        ChangeSetStatusV1::Lapsed(3)
    );
    let policy = view.lifecycle_activation_policy().unwrap();
    assert!(!policy.active);
    assert_eq!(policy.activation, None);
}

#[test]
fn selected_lifecycle_without_original_activated_record_fails_closed() {
    for mutation in 0..3 {
        let mut chain =
            CertifiedTestChain::start(TestChainConfig::new(World::new(), 1_000)).unwrap();
        let id = schedule(&chain, true);
        chain.commit_at(1_002, vec![]);
        chain.commit_at(1_003, vec![]);
        chain.setup_world_at(1_004, |tx| {
            let mut original = tx.world.runtime_change_sets.get(&id).unwrap().clone();
            match mutation {
                0 => {
                    tx.world.runtime_change_sets.remove(id);
                }
                1 => {
                    original.status = ChangeSetStatusV1::Lapsed(3);
                    tx.world.runtime_change_sets.insert(id, original);
                }
                _ => {
                    original.status = ChangeSetStatusV1::Activated(2);
                    tx.world.runtime_change_sets.insert(id, original);
                }
            }
        });
        assert!(chain.state().view().lifecycle_activation_policy().is_err());
        assert!(
            chain
                .state()
                .view()
                .lifecycle_consensus_config_fingerprint()
                .is_err()
        );
    }
}

#[test]
fn fingerprint_binds_genesis_selection_and_original_boundary() {
    let genesis = iroha_crypto::Hash::new(b"original signed genesis");
    let mut policy = CommittedLifecyclePolicy {
        selection: release_catalog().unwrap().lifecycle_id,
        active: true,
        activation: Some((ChangeSetId([1; 32]), std::num::NonZeroU64::new(3).unwrap())),
    };
    let original = configuration_fingerprint(genesis, &policy);
    assert_ne!(
        original,
        configuration_fingerprint(iroha_crypto::Hash::new(b"other genesis"), &policy)
    );
    policy.activation.as_mut().unwrap().0 = ChangeSetId([2; 32]);
    assert_ne!(original, configuration_fingerprint(genesis, &policy));
    policy.activation.as_mut().unwrap().0 = ChangeSetId([1; 32]);
    policy.activation.as_mut().unwrap().1 = std::num::NonZeroU64::new(4).unwrap();
    assert_ne!(original, configuration_fingerprint(genesis, &policy));
    policy.selection = release_catalog().unwrap().baseline_id;
    assert_ne!(original, configuration_fingerprint(genesis, &policy));
}

/// Seed a committed child foundation for provenance-reader tests only. This fixture grants no
/// genesis authentication, administrative approval, finality or release qualification.
fn child_foundation_fixture(
    include_recipe: bool,
) -> (
    World,
    NetworkId,
    iroha_data_model::runtime::child_admin::foundation::ChildRuntimeFoundationV1,
) {
    use iroha_data_model::{
        block::consensus::SumeragiRootScope,
        parameter::Parameter,
        runtime::child_admin::{
            ChildRootIdentityV1, ChildRuntimeAdminPolicyV1, ChildRuntimeAdminStateV1,
            foundation::ChildRuntimeFoundationV1,
        },
    };
    let network = NetworkId::from_genesis_hash(iroha_crypto::HashOf::from_untyped_unchecked(
        iroha_crypto::Hash::new(b"lineage child genesis"),
    ));
    let parent_network =
        NetworkId::from_genesis_hash(iroha_crypto::HashOf::from_untyped_unchecked(
            iroha_crypto::Hash::new(b"lineage parent genesis"),
        ));
    let dataspace = iroha_model_base::topology::DataSpaceId::new((1_u64 << 43) + 17);
    let scope = SumeragiRootScope::Dataspace {
        parent_network_id: parent_network,
        dataspace_id: dataspace,
    };
    let world = crate::sumeragi::lanes::routing::test_support::world(scope);
    let (selection, definitions) = release_catalog().unwrap().child_foundation().unwrap();
    let recipe = ChildRuntimeFoundationV1 {
        selection: selection.id().unwrap(),
        policy: ChildRuntimeAdminPolicyV1 {
            revision: 1,
            admins: vec![iroha_test_samples::ALICE_KEYPAIR.public_key().clone()],
            threshold: 1,
            notice: Default::default(),
            provenance: None,
        },
    };
    let mut block = world.block();
    for (id, definition) in definitions {
        block.runtime_definitions.insert(id, definition);
    }
    *block.runtime_selection.get_mut() = selection;
    *block.runtime_child_admin_state.get_mut() = Some(ChildRuntimeAdminStateV1 {
        root: ChildRootIdentityV1 {
            network,
            parent_network,
            dataspace,
            instance: scope
                .instance_id(
                    &crate::sumeragi::crypto::BlsCrypto::new(),
                    network,
                    "lineage-child",
                )
                .unwrap()
                .0,
        },
        policy: recipe.policy.clone(),
        next_nonce: 0,
    });
    if include_recipe {
        block
            .set_parameter_for_testing(Parameter::Custom(recipe.to_custom_parameter().unwrap()))
            .unwrap();
    }
    block.commit();
    (world, network, recipe)
}

#[test]
fn child_foundation_anchors_lineage_without_fabricating_trigger_activation_and_survives_rotation() {
    let (world, network, recipe) = child_foundation_fixture(true);
    let policy = committed_policy(&world.view(), network, 1).unwrap();
    assert_eq!(policy.selection, recipe.selection);
    assert!(!policy.active);
    assert_eq!(policy.activation, None);
    validate_child_foundation(&world.view(), network).unwrap();
    let mut block = world.block();
    let authority = block.runtime_child_admin_state.get_mut().as_mut().unwrap();
    authority.policy.revision = 2;
    authority.policy.admins = vec![iroha_test_samples::BOB_KEYPAIR.public_key().clone()];
    authority.next_nonce = 1;
    block.commit();
    assert_eq!(committed_policy(&world.view(), network, 2).unwrap(), policy);
}

#[test]
fn child_lineage_requires_its_original_recipe_root_identity_and_founded_definitions() {
    use iroha_data_model::{
        parameter::{CustomParameter, Parameter},
        runtime::child_admin::foundation::ChildRuntimeFoundationV1,
    };
    let (world, network, _) = child_foundation_fixture(false);
    assert!(committed_policy(&world.view(), network, 1).is_err());
    for mutation in 0..9 {
        let (world, network, recipe) = child_foundation_fixture(true);
        let mut block = world.block();
        match mutation {
            0 => *block.runtime_child_admin_state.get_mut() = None,
            1 => {
                block
                    .runtime_child_admin_state
                    .get_mut()
                    .as_mut()
                    .unwrap()
                    .root
                    .network =
                    NetworkId::from_genesis_hash(iroha_crypto::HashOf::from_untyped_unchecked(
                        iroha_crypto::Hash::new(b"foreign child"),
                    ))
            }
            2 => {
                block
                    .runtime_child_admin_state
                    .get_mut()
                    .as_mut()
                    .unwrap()
                    .root
                    .parent_network = network
            }
            3 => {
                block
                    .runtime_child_admin_state
                    .get_mut()
                    .as_mut()
                    .unwrap()
                    .root
                    .dataspace = iroha_model_base::topology::DataSpaceId::new(17)
            }
            4 => {
                block
                    .runtime_child_admin_state
                    .get_mut()
                    .as_mut()
                    .unwrap()
                    .policy
                    .admins = vec![iroha_test_samples::BOB_KEYPAIR.public_key().clone()]
            }
            5 => {
                block
                    .set_parameter_for_testing(Parameter::Custom(CustomParameter::new(
                        ChildRuntimeFoundationV1::parameter_id().clone(),
                        iroha_primitives::json::Json::new("malformed recipe"),
                    )))
                    .unwrap();
            }
            6 => {
                let mut different = recipe;
                different.selection.0[0] ^= 1;
                block
                    .set_parameter_for_testing(Parameter::Custom(
                        different.to_custom_parameter().unwrap(),
                    ))
                    .unwrap();
            }
            7 => *block.runtime_selection.get_mut() = release_catalog().unwrap().baseline.clone(),
            _ => {
                block
                    .runtime_definitions
                    .remove(release_catalog().unwrap().baseline.kernel);
            }
        }
        block.commit();
        assert!(
            committed_policy(&world.view(), network, 2).is_err(),
            "mutation {mutation}"
        );
    }
}

/// Produce a complete applied change and its retained activated record for reader conformance.
/// Administrative and validator authorization are exercised by the separate child-admin tests.
fn child_restriction_record(world: &World, network: NetworkId) -> ChangeSetId {
    use iroha_data_model::runtime::definition::{RestrictionEntryV1, RestrictionV1};
    let mut block = world.block();
    let predecessor = block.runtime_selection.get().clone();
    let authority = block.runtime_child_admin_state.get().as_ref().unwrap();
    let root = RootScopeV1::DataSpace(authority.root.dataspace);
    let definition = predecessor
        .bindings
        .iter()
        .find(|binding| binding.wire_id == "iroha.log")
        .unwrap()
        .interface;
    let mut successor = predecessor.clone();
    successor.restrictions.push(RestrictionEntryV1 {
        definition,
        restriction: RestrictionV1::AdmissionClosed(3),
    });
    successor.restrictions.sort();
    let change = ChangeSetV1 {
        network,
        root,
        predecessor: predecessor.id().unwrap(),
        successor: successor.id().unwrap(),
        effective_height: 3,
        publish: Vec::new(),
        qualification: [0x71; 32],
        transitions: vec![SelectionTransitionV1::Restrict {
            definition,
            restriction: RestrictionKindV1::AdmissionClosed,
        }],
    };
    let applied = change
        .apply(
            &predecessor,
            &super::super::change_set::PublishedDefinitions(&block.runtime_definitions),
        )
        .unwrap();
    let id = change.id().unwrap();
    block.runtime_change_sets.insert(
        id,
        ChangeSetRecordV1 {
            change_set: change,
            status: ChangeSetStatusV1::Activated(3),
            proposed_ms: 1,
            approved_height: 1,
            approved_ms: 1,
            notice: RuntimeNoticeV1 {
                blocks: 1,
                ms: 0,
                approval_cushion_ms: 0,
            },
            attestations: Vec::new(),
        },
    );
    *block.runtime_selection.get_mut() = applied.selection;
    block.commit();
    id
}

#[test]
fn child_activated_lineage_accepts_only_its_exact_data_space_and_network() {
    for mutation in 0..5 {
        let (world, network, _) = child_foundation_fixture(true);
        let id = child_restriction_record(&world, network);
        let mut block = world.block();
        let mut record = block.runtime_change_sets.get(&id).unwrap().clone();
        match mutation {
            0 => {}
            1 => record.change_set.root = RootScopeV1::Universal,
            2 => {
                record.change_set.root =
                    RootScopeV1::DataSpace(iroha_model_base::topology::DataSpaceId::new(17))
            }
            3 => {
                record.change_set.network =
                    NetworkId::from_genesis_hash(iroha_crypto::HashOf::from_untyped_unchecked(
                        iroha_crypto::Hash::new(b"foreign lineage"),
                    ))
            }
            _ => record.status = ChangeSetStatusV1::Scheduled,
        }
        block.runtime_change_sets.insert(id, record);
        block.commit();
        let result = committed_policy(&world.view(), network, 3);
        if mutation == 0 {
            let policy = result.unwrap();
            assert!(!policy.active);
            assert_eq!(policy.activation, None);
        } else {
            assert!(result.is_err(), "mutation {mutation}");
        }
    }
}

#[test]
fn private_foundation_reader_preserves_original_recipe_decode_refusal() {
    let (world, network, _) = child_foundation_fixture(true);
    let catalog = release_catalog().unwrap();
    let view = world.view();
    founded_lineage(&view, network, catalog).unwrap();
    let zero = norito::core::DecodeBudgetContext::new(norito::DecodeLimits::new(
        usize::MAX,
        usize::MAX,
        usize::MAX,
        0,
        usize::MAX,
    ));
    let result = zero.with(|| founded_lineage(&view, network, catalog));
    let Err(crate::execution_attempt::ExecutionAttemptError::Deferred(original)) = result else {
        panic!(
            "the new private reader must preserve local decoder refusal before the public text boundary"
        );
    };
    assert!(original.decode_refusal().is_some());
    founded_lineage(&view, network, catalog).unwrap();
}

#[test]
fn global_custom_foundation_marker_remains_inert_but_child_authority_cannot_use_global_scope() {
    use iroha_data_model::{
        block::consensus::SumeragiRootScope,
        parameter::{CustomParameter, Parameter},
        runtime::child_admin::foundation::ChildRuntimeFoundationV1,
    };
    let (child, network, _) = child_foundation_fixture(true);
    let world = crate::sumeragi::lanes::routing::test_support::world(SumeragiRootScope::Global);
    let mut block = world.block();
    block
        .set_parameter_for_testing(Parameter::Custom(CustomParameter::new(
            ChildRuntimeFoundationV1::parameter_id().clone(),
            iroha_primitives::json::Json::new("inert ordinary global custom parameter"),
        )))
        .unwrap();
    block.commit();
    let policy = committed_policy(&world.view(), network, 1).unwrap();
    assert_eq!(policy.selection, release_catalog().unwrap().baseline_id);
    assert!(!policy.active);
    assert!(validate_child_foundation(&world.view(), network).is_err());
    let mut block = child.block();
    block
        .set_parameter_for_testing(crate::sumeragi::lanes::routing::test_support::metadata(
            SumeragiRootScope::Global,
        ))
        .unwrap();
    block.commit();
    assert!(committed_policy(&child.view(), network, 1).is_err());
}
