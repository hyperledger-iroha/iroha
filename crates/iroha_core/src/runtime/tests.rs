//! Execution resolves every instruction against the root's selection.

use iroha_data_model::{
    isi::{InstructionBox, Log, error::InstructionExecutionError},
    level::Level,
    runtime::definition::RestrictionEntryV1,
};

use super::*;

fn log() -> InstructionBox {
    Log::new(Level::INFO, "resolved".into()).into()
}

fn baseline() -> RootSelectionV1 {
    founded_selection().expect("baseline")
}

fn wire_id() -> &'static str {
    iroha_data_model::isi::wire_id_of::<Log>().expect("registered")
}

#[test]
fn the_baseline_resolves_every_baseline_instruction() {
    let selection = baseline();
    assert_eq!(
        resolve(&log(), &selection, 1, || None),
        Ok(Some(selection.bindings[0].implementation)),
        "the bound implementation executes it"
    );
    // A release addition's instruction stays unbound until a change set activates it.
    let bound: std::collections::BTreeSet<&str> = selection
        .bindings
        .iter()
        .map(|binding| binding.wire_id.as_str())
        .collect();
    let baseline_instructions: std::collections::BTreeSet<&str> =
        iroha_data_model::isi::registry::built_in_instruction_schemas()
            .filter(|schema| !schema.tolerant)
            .map(|schema| schema.wire_id)
            .collect();
    assert_eq!(bound, baseline_instructions);
}

#[test]
fn an_unbound_wire_id_is_inactive() {
    let mut selection = baseline();
    selection
        .bindings
        .retain(|binding| binding.wire_id != wire_id());
    assert_eq!(
        resolve(&log(), &selection, 1, || None),
        Err(InstructionExecutionError::Runtime(
            RuntimeRejectionV1::DefinitionInactive {
                wire_id: wire_id().to_owned(),
            }
        ))
    );
}

#[test]
fn restrictions_stop_new_work_from_their_height() {
    let selection = baseline();
    let binding = selection
        .bindings
        .iter()
        .find(|binding| binding.wire_id == wire_id())
        .expect("bound")
        .clone();
    for (definition, restriction) in [
        (binding.interface, RestrictionV1::Disabled(5)),
        (binding.implementation, RestrictionV1::AdmissionClosed(5)),
        (binding.implementation, RestrictionV1::CompletionOnly(5)),
    ] {
        let mut restricted = selection.clone();
        restricted.restrictions = vec![RestrictionEntryV1 {
            definition,
            restriction,
        }];
        assert_eq!(
            resolve(&log(), &restricted, 4, || None),
            Ok(Some(binding.implementation)),
            "before its height"
        );
        assert_eq!(
            resolve(&log(), &restricted, 5, || None),
            Err(InstructionExecutionError::Runtime(
                RuntimeRejectionV1::DefinitionRestricted {
                    definition,
                    restriction,
                }
            ))
        );
    }

    let mut deprecated = selection;
    deprecated.restrictions = vec![RestrictionEntryV1 {
        definition: binding.implementation,
        restriction: RestrictionV1::Deprecated(1),
    }];
    assert_eq!(
        resolve(&log(), &deprecated, 9, || None),
        Ok(Some(binding.implementation)),
        "deprecation keeps execution"
    );
}

#[test]
fn a_live_obligation_continues_or_completes_through_its_restriction() {
    use obligations::Step;
    let selection = baseline();
    let binding = selection
        .bindings
        .iter()
        .find(|binding| binding.wire_id == wire_id())
        .expect("bound")
        .clone();
    let restricted = |restriction| {
        let mut restricted = selection.clone();
        restricted.restrictions = vec![RestrictionEntryV1 {
            definition: binding.interface,
            restriction,
        }];
        restricted
    };
    for (restriction, step, runs) in [
        (
            RestrictionV1::AdmissionClosed(5),
            Some(Step::Continue),
            true,
        ),
        (
            RestrictionV1::AdmissionClosed(5),
            Some(Step::Complete),
            true,
        ),
        (RestrictionV1::AdmissionClosed(5), None, false),
        (
            RestrictionV1::CompletionOnly(5),
            Some(Step::Continue),
            false,
        ),
        (RestrictionV1::CompletionOnly(5), Some(Step::Complete), true),
        (RestrictionV1::Disabled(5), Some(Step::Complete), false),
    ] {
        let result = resolve(&log(), &restricted(restriction), 5, || step);
        assert_eq!(result.is_ok(), runs, "{restriction:?} {step:?}");
    }
    let mut asked = 0;
    resolve(&log(), &selection, 5, || {
        asked += 1;
        None
    })
    .expect("unrestricted");
    assert_eq!(asked, 0, "an unrestricted binding never reads obligations");
}

#[test]
fn a_new_world_is_founded_on_the_baseline() {
    let world = crate::state::World::default();
    let catalog = release_catalog().expect("catalog");
    assert_eq!(*world.runtime_selection.view().get(), catalog.baseline);
    let definitions = world.runtime_definitions.view();
    assert_eq!(
        mv::storage::StorageReadOnly::iter(&definitions).count(),
        catalog.baseline_definitions.len()
    );
    for id in &catalog.baseline_definitions {
        assert_eq!(
            mv::storage::StorageReadOnly::get(&definitions, id),
            catalog.definitions.get(id)
        );
    }
}

#[test]
fn a_node_drafts_the_activation_of_its_release_additions() {
    use crate::state::WorldReadOnly as _;
    use catalog::{
        additions::test_additions::{LOG, LOG_MODULE, LOG_WIRE_ID},
        test_catalog,
    };
    let network = iroha_data_model::NetworkId::from_genesis_hash(
        iroha_crypto::HashOf::from_untyped_unchecked(iroha_crypto::Hash::prehashed([0x61; 32])),
    );
    test_catalog::with_additions(&[LOG], || {
        let state = crate::state::State::new_for_testing(
            crate::state::World::default(),
            crate::kura::Kura::blank_kura_for_testing(),
            crate::query::store::LiveQueryStore::start_test(),
        );
        let view = state.world_view();
        let change = draft_addition(&view, network, LOG_MODULE, 10, [1; 32]).expect("draft");
        let catalog = release_catalog().expect("test catalog");
        assert_eq!(change.predecessor, catalog.baseline_id);
        assert_eq!(change.network, network);
        assert_eq!(change.effective_height, 10);
        let applied = change
            .apply(
                &catalog.baseline,
                &change_set::PublishedDefinitions(view.runtime_definitions()),
            )
            .expect("the draft applies to the committed selection");
        assert!(
            applied
                .selection
                .bindings
                .iter()
                .any(|binding| binding.wire_id == LOG_WIRE_ID)
        );
        assert!(
            draft_addition(&view, network, "iroha.test.unknown", 10, [1; 32]).is_err(),
            "only additions of this release are drafted"
        );
    });
}

#[test]
fn ironwood_world_storage_guard_checks_current_and_retained_policy_keys() {
    use iroha_data_model::privacy::PrivacyProtocolPolicyKeyV1;
    use iroha_data_model::privacy::{PrivacyProtocolPolicyV1, PrivacyRuntimeProtocolIdV1};
    use mv::storage::StorageReadOnly as _;
    let mut world = crate::state::World::default();
    validate_privacy_world_storage_v1(&mut world).unwrap();
    let protocol = PrivacyRuntimeProtocolIdV1::IronwoodHalo2ActionsV1;
    let key = PrivacyProtocolPolicyKeyV1::new(protocol);
    world
        .privacy_protocol_policies
        .insert(key, PrivacyProtocolPolicyV1::canonical_default_v1(protocol));
    let before = norito::json::to_json(&world.privacy_protocol_policies).unwrap();
    assert!(
        validate_privacy_world_storage_v1(&mut world)
            .unwrap_err()
            .contains("privacy_protocol_policies")
    );
    assert_eq!(
        norito::json::to_json(&world.privacy_protocol_policies).unwrap(),
        before,
        "the guard must not mutate current or undo bytes"
    );
    {
        let mut block = world.privacy_protocol_policies.block();
        block.remove(key);
        block.commit();
    }
    assert!(
        world
            .privacy_protocol_policies
            .view()
            .iter()
            .next()
            .is_none()
    );
    assert!(
        validate_privacy_world_storage_v1(&mut world).is_err(),
        "a deleted Ironwood row still lives in retained predecessor storage"
    );
}

#[test]
fn ironwood_world_storage_guard_inspects_raw_undo_keys_and_values() {
    use crate::privacy_state::{
        PrivacyCommitmentKeyV1, PrivacyNullifierKeyV1, PrivacyRootKeyV1, PrivacyRootProvenanceV1,
    };
    use iroha_data_model::privacy::*;
    use std::collections::BTreeMap;

    let old_protocol = PrivacyRuntimeProtocolIdV1::OrchardHalo2ActionsV1;
    let ironwood = PrivacyRuntimeProtocolIdV1::IronwoodHalo2ActionsV1;
    let namespace = PrivacyRuntimeNamespaceV1::new(
        ironwood,
        PrivacyNamespaceScopeV1::Pool(PrivacyPoolNamespaceV1 {
            pool_id: PrivacyPoolIdV1::new([7; 32]),
        }),
    );
    let old_key = PrivacyProtocolPolicyKeyV1::new(old_protocol);
    let old_policy = PrivacyProtocolPolicyV1::canonical_default_v1(old_protocol);
    let ironwood_policy = PrivacyProtocolPolicyV1::canonical_default_v1(ironwood);
    let mut wrong_limits = old_policy.clone();
    wrong_limits.current_limits = ironwood_policy.current_limits;
    let mut wrong_pending = old_policy;
    wrong_pending.pending_update = Some(PrivacyProtocolLimitsUpdateV1 {
        scheduled_at_height: 1,
        effective_at_height: 2,
        next_limits: ironwood_policy.current_limits,
    });
    for undo_value in [ironwood_policy, wrong_limits, wrong_pending] {
        let mut world = crate::state::World::default();
        world.privacy_protocol_policies = Storage::from_snapshot_parts(
            BTreeMap::new(),
            BTreeMap::from([(old_key, Some(undo_value))]),
        );
        assert!(validate_privacy_world_storage_v1(&mut world).is_err());
    }

    // These accessors identify their fixed role, not the protocol in an unvalidated raw key.
    let hidden_nullifier = PrivacyNullifierKeyV1::ZkX509CertificateNullifier {
        namespace,
        nullifier: PrivacyNullifierV1::new([8; 32]),
    };
    assert_eq!(
        hidden_nullifier.protocol_id(),
        PrivacyRuntimeProtocolIdV1::IrohaZkX509StarkP256V1
    );
    let mut world = crate::state::World::default();
    world.privacy_nullifiers =
        Storage::from_snapshot_parts(BTreeMap::new(), BTreeMap::from([(hidden_nullifier, None)]));
    assert!(validate_privacy_world_storage_v1(&mut world).is_err());

    let hidden_commitment = PrivacyCommitmentKeyV1::ZkAmsPhc {
        namespace,
        phc_hash: PrivacyZkAmsPhcHashV1::new([9; 32]),
    };
    assert_eq!(
        hidden_commitment.protocol_id(),
        PrivacyRuntimeProtocolIdV1::IrohaZkAmsV1
    );
    let mut world = crate::state::World::default();
    world.privacy_commitments =
        Storage::from_snapshot_parts(BTreeMap::new(), BTreeMap::from([(hidden_commitment, None)]));
    assert!(validate_privacy_world_storage_v1(&mut world).is_err());

    let old_namespace = PrivacyRuntimeNamespaceV1::new(old_protocol, namespace.scope());
    let old_root_key = PrivacyRootKeyV1::new(
        old_namespace,
        PrivacyRootRoleV1::NoteCommitmentAnchor,
        1,
        PrivacyRootV1::new([10; 32]),
    )
    .unwrap();
    let hidden_root_value = PrivacyRootProvenanceV1::ProofManagedPoolBootstrap {
        bootstrap_digest: PrivacyProofManagedPoolBootstrapDigestV1::new([11; 32]),
        protocol_id: ironwood,
        admitted_at_height: 1,
    };
    let mut world = crate::state::World::default();
    world.privacy_roots = Storage::from_snapshot_parts(
        BTreeMap::new(),
        BTreeMap::from([(old_root_key, Some(hidden_root_value))]),
    );
    let before = norito::json::to_json(&world.privacy_roots).unwrap();
    assert!(validate_privacy_world_storage_v1(&mut world).is_err());
    assert_eq!(norito::json::to_json(&world.privacy_roots).unwrap(), before);
}

#[test]
fn ironwood_coherent_candidate_pool_cannot_enter_the_baseline_world() {
    use crate::privacy_state::*;
    use iroha_data_model::{asset::AssetId, privacy::*};
    let mut world = crate::state::World::default();
    let sample = privacy_action_role_samples_v1()
        .unwrap()
        .into_iter()
        .find(|sample| sample.role == PrivacyActionRoleV1::OrchardActions)
        .unwrap();
    let PrivacyRuntimeStatementV1::OrchardHalo2ActionsV1(statement) = sample.envelope.statement
    else {
        unreachable!()
    };
    let bootstrap = PrivacyIronwoodPoolBootstrapV1::new(
        statement.pool_id,
        statement.asset_definition_id,
        statement.public_balance_scope,
        iroha_test_samples::ALICE_ID.clone(),
    )
    .unwrap();
    let namespace = bootstrap.namespace();
    let reserve_asset = AssetId::with_scope(
        bootstrap.asset_definition_id.clone(),
        bootstrap.reserve_account.clone(),
        bootstrap.public_balance_scope,
    );
    let state = PrivacyOrchardPoolStateV1::bootstrap_ironwood(bootstrap).unwrap();
    let provenance =
        PrivacyRootProvenanceV1::orchard_pool_bootstrap(state.bootstrap_digest(), 1).unwrap();
    world.privacy_roots.insert(
        PrivacyRootKeyV1::new(
            namespace,
            PrivacyRootRoleV1::NoteCommitmentAnchor,
            state.epoch(),
            state.root(),
        )
        .unwrap(),
        provenance.clone(),
    );
    world.privacy_root_heads.insert(
        PrivacyRootHeadKeyV1::new(namespace, PrivacyRootRoleV1::NoteCommitmentAnchor).unwrap(),
        PrivacyRootHeadRecordV1::new(state.epoch(), state.root(), provenance, None, 1).unwrap(),
    );
    world.privacy_commitments.insert(
        PrivacyCommitmentKeyV1::public_reserve_custody(namespace.protocol_id(), &reserve_asset)
            .unwrap(),
        PrivacyStateItemRecordV1::public_reserve_custody(
            reserve_asset,
            PrivacyPublicReserveOwnerV1::Orchard {
                namespace,
                bootstrap_digest: state.bootstrap_digest(),
            },
        )
        .unwrap(),
    );
    world.privacy_commitments.insert(
        PrivacyCommitmentKeyV1::orchard_pool_state(namespace).unwrap(),
        PrivacyStateItemRecordV1::orchard_pool_state(state).unwrap(),
    );
    validate_privacy_persisted_state_v1(
        world.privacy_consensus_policy.view().get(),
        &world.privacy_protocol_policies.view(),
        PrivacyPgcStateReadV1 {
            accounts: &world.privacy_pgc_accounts.view(),
            pool_invariants: &world.privacy_pgc_pool_invariants.view(),
        },
        &world.privacy_nullifiers.view(),
        &world.privacy_commitments.view(),
        &world.privacy_roots.view(),
        &world.privacy_root_heads.view(),
    )
    .unwrap();
    assert!(
        validate_privacy_world_storage_v1(&mut world)
            .unwrap_err()
            .contains("privacy_commitments"),
        "valid candidate cryptographic state does not authorize the frozen storage grammar"
    );
}
