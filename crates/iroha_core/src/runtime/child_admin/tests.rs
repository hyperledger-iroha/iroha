//! Original-source, threshold-action, scope and failure-atomicity component regressions.
//!
//! Genesis source authentication is covered by `block::genesis_child_runtime` tests; these tests
//! construct its resulting committed state and exercise the actual selected instruction handlers.

use super::*;
use crate::{
    executor::Executor,
    query::store::LiveQueryStore,
    state::{State, World, WorldReadOnly},
    sumeragi::lanes::routing::test_support,
};
use iroha_crypto::{Algorithm, HashOf, KeyPair};
use iroha_data_model::{
    NetworkId, Registrable,
    account::Account,
    block::BlockHeader,
    isi::{InstructionBox, Log},
    level::Level,
    nexus::{DataSpaceCatalog, DataSpaceMetadata},
    parameter::runtime_upgrade_policy::RuntimeUpgradePolicyV1,
    runtime::{
        child_admin::{ChildAdminSignatureV1, ChildRuntimeAdminPlanV1, ChildRuntimeAdminPolicyV1},
        lifecycle::{ChangeSetV1, RuntimeNoticeV1, SupportAttestationV1, committee_digest},
    },
    transaction::{Executable, FeePaymentIntent, SignedTransaction, TransactionBuilder},
};
use iroha_model_base::{peer::PeerId, topology::DataSpaceId};
use iroha_test_samples::{ALICE_ID, ALICE_KEYPAIR};

fn fixture() -> (State, Vec<KeyPair>) {
    fixture_with_recipe(true)
}

fn fixture_with_recipe(include_recipe: bool) -> (State, Vec<KeyPair>) {
    let ds = DataSpaceId::new((1 << 40) + 23);
    let scope = SumeragiRootScope::Dataspace {
        parent_network_id: NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(
            b"admin-parent",
        ))),
        dataspace_id: ds,
    };
    let world = World::with([], [Account::new(ALICE_ID.clone()).build(&ALICE_ID)], []);
    let mut parameters = crate::state::parameter_cell_block_for_testing(&world).unwrap();
    crate::state::set_parameter_cell_for_testing(
        &mut parameters,
        test_support::metadata(scope),
        world.sccp_committee_generations.allocation_budget(),
    )
    .unwrap();
    parameters.commit();
    let nexus = iroha_config::parameters::actual::Nexus {
        dataspace_catalog: DataSpaceCatalog::new(vec![
            DataSpaceMetadata::default(),
            DataSpaceMetadata {
                id: ds,
                alias: "child-admin-tests".into(),
                description: None,
                fault_tolerance: 1,
            },
        ])
        .unwrap(),
        ..Default::default()
    };
    let state = State::new_with_nexus_for_testing(world, nexus, LiveQueryStore::start_test());
    let mut keys: Vec<_> = (31..=33)
        .map(|seed| KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519))
        .collect();
    keys.sort_by(|a, b| a.public_key().cmp(b.public_key()));
    let notice = RuntimeNoticeV1 {
        blocks: 2,
        ms: 0,
        approval_cushion_ms: 0,
    };
    let root = ChildRootIdentityV1 {
        network: state.network_id,
        parent_network: match scope {
            SumeragiRootScope::Dataspace {
                parent_network_id, ..
            } => parent_network_id,
            _ => unreachable!(),
        },
        dataspace: ds,
        instance: scope
            .instance_id(
                &crate::sumeragi::crypto::BlsCrypto::new(),
                state.network_id,
                state.chain_id_ref().as_str(),
            )
            .unwrap()
            .0,
    };
    let (selection, definitions) = super::super::release_catalog()
        .unwrap()
        .child_foundation()
        .unwrap();
    let policy = ChildRuntimeAdminPolicyV1 {
        revision: 1,
        admins: keys.iter().map(|key| key.public_key().clone()).collect(),
        threshold: 2,
        notice: RuntimeUpgradePolicyV1 {
            activation: notice,
            disable: notice,
        },
        provenance: None,
    };
    let mut block = state.world.block();
    if include_recipe {
        let recipe = iroha_data_model::runtime::child_admin::foundation::ChildRuntimeFoundationV1 {
            selection: selection.id().unwrap(),
            policy: policy.clone(),
        };
        block
            .set_parameter_for_testing(iroha_data_model::parameter::Parameter::Custom(
                recipe.to_custom_parameter().unwrap(),
            ))
            .unwrap();
    }
    *block.runtime_selection.get_mut() = selection;
    for (id, definition) in definitions {
        block.runtime_definitions.insert(id, definition);
    }
    *block.runtime_child_admin_state.get_mut() = Some(ChildRuntimeAdminStateV1 {
        root,
        policy,
        next_nonce: 0,
    });
    block.commit();
    (state, keys)
}

fn header() -> BlockHeader {
    BlockHeader::new(std::num::NonZeroU64::new(5).unwrap(), None, None, 1_000, 0)
}
fn bind_scope(tx: &mut StateTransaction<'_, '_>) {
    let ds = tx
        .world
        .runtime_child_admin_state
        .get()
        .as_ref()
        .unwrap()
        .root
        .dataspace;
    tx.current_dataspace_id = Some(ds);
    tx.world.current_dataspace_id = Some(ds);
}
fn action(
    tx: &StateTransaction<'_, '_>,
    keys: &[KeyPair],
    action: ChildRuntimeAdminActionV1,
) -> AuthorizeChildRuntimeActionV1 {
    let committed = tx.world.runtime_child_admin_state.get().as_ref().unwrap();
    let plan = ChildRuntimeAdminPlanV1 {
        root: committed.root,
        policy_id: committed.policy.id().unwrap(),
        policy_revision: committed.policy.revision,
        nonce: committed.next_nonce,
        valid_from: 5,
        valid_until: 20,
        action,
    };
    let signatures = keys
        .iter()
        .map(|key| ChildAdminSignatureV1::sign(&plan, key).unwrap())
        .collect();
    AuthorizeChildRuntimeActionV1 {
        certificate: ChildRuntimeAdminCertificateV1 { plan, signatures },
    }
}
fn noop(tx: &StateTransaction<'_, '_>, height: u64) -> ChangeSetV1 {
    let root = tx
        .world
        .runtime_child_admin_state
        .get()
        .as_ref()
        .unwrap()
        .root;
    let selection = tx.world.runtime_selection.canonical().id().unwrap();
    ChangeSetV1 {
        network: root.network,
        root: RootScopeV1::DataSpace(root.dataspace),
        predecessor: selection,
        successor: selection,
        effective_height: height,
        publish: vec![],
        transitions: vec![],
        qualification: [9; 32],
    }
}
fn signed(tx: &StateTransaction<'_, '_>, items: Vec<InstructionBox>) -> SignedTransaction {
    TransactionBuilder::new(
        *tx.network_id(),
        ALICE_ID.clone(),
        FeePaymentIntent::authority(Vec::new(), core::num::NonZeroU64::new(50_000_000)),
    )
    .with_executable(Executable::Instructions(items.into()))
    .sign(ALICE_KEYPAIR.private_key())
}
fn bind(tx: &mut StateTransaction<'_, '_>, signed: &SignedTransaction) {
    tx.current_network_entrypoint_hash = Some(signed.hash_as_entrypoint());
    tx.tx_call_hash = Some(Hash::from(signed.hash_as_entrypoint()));
    tx.current_tx_hash = Some(signed.hash());
    tx.current_entrypoint_index = Some(7);
}
fn arm(tx: &mut StateTransaction<'_, '_>, instruction: &InstructionBox) {
    let signed = signed(tx, vec![instruction.clone()]);
    bind(tx, &signed);
    tx.child_runtime_admin_entrypoint =
        capture_direct(tx, &signed, &ALICE_ID, instruction, 0, true).unwrap();
    assert!(tx.child_runtime_admin_entrypoint.is_some());
}
fn execute(
    tx: &mut StateTransaction<'_, '_>,
    instruction: InstructionBox,
) -> Result<(), ValidationFail> {
    arm(tx, &instruction);
    Executor::Initial.execute_instruction(tx, &ALICE_ID, &instruction)
}

#[test]
fn threshold_schedule_cancel_and_policy_rotation_consume_exact_nonces() {
    let (state, keys) = fixture();
    let mut block = state.block(header());
    let mut tx = block.transaction();
    bind_scope(&mut tx);
    let change = noop(&tx, 7);
    let id = change.id().unwrap();
    let schedule = action(
        &tx,
        &keys[..2],
        ChildRuntimeAdminActionV1::Schedule {
            change_set: change,
            provenance: vec![],
        },
    );
    execute(&mut tx, schedule.clone().into()).unwrap();
    let record = tx.world.runtime_change_sets.get(&id).unwrap();
    assert_eq!(record.status, ChangeSetStatusV1::Scheduled);
    assert_eq!(record.notice.blocks, 2);
    assert!(record.attestations.is_empty());
    assert_eq!(
        tx.world
            .runtime_child_admin_state
            .get()
            .as_ref()
            .unwrap()
            .next_nonce,
        1
    );
    assert!(execute(&mut tx, schedule.into()).is_err());
    let cancel = action(&tx, &keys[..2], ChildRuntimeAdminActionV1::Cancel(id));
    execute(&mut tx, cancel.into()).unwrap();
    assert_eq!(
        tx.world.runtime_change_sets.get(&id).unwrap().status,
        ChangeSetStatusV1::Cancelled(5)
    );
    let mut next = tx
        .world
        .runtime_child_admin_state
        .get()
        .as_ref()
        .unwrap()
        .policy
        .clone();
    next.revision += 1;
    next.threshold = 3;
    let rotate = action(
        &tx,
        &keys[..2],
        ChildRuntimeAdminActionV1::RotatePolicy(next.clone()),
    );
    execute(&mut tx, rotate.into()).unwrap();
    let committed = tx.world.runtime_child_admin_state.get().as_ref().unwrap();
    assert_eq!(committed.policy, next);
    assert_eq!(committed.next_nonce, 3);
    let retry = action(&tx, &keys[..2], ChildRuntimeAdminActionV1::Cancel(id));
    assert!(execute(&mut tx, retry.into()).is_err());
    assert_eq!(
        tx.world
            .runtime_child_admin_state
            .get()
            .as_ref()
            .unwrap()
            .next_nonce,
        3
    );
}

#[test]
fn rejection_preserves_policy_nonce_history_and_original_source_is_one_use() {
    let (state, keys) = fixture();
    let mut block = state.block(header());
    let mut tx = block.transaction();
    bind_scope(&mut tx);
    let before = tx.world.runtime_child_admin_state.get().clone();
    let short = action(
        &tx,
        &keys[..2],
        ChildRuntimeAdminActionV1::Schedule {
            change_set: noop(&tx, 6),
            provenance: vec![],
        },
    );
    let boxed: InstructionBox = short.clone().into();
    arm(&mut tx, &boxed);
    assert!(short.clone().execute(&ALICE_ID, &mut tx).is_err());
    assert!(tx.child_runtime_admin_entrypoint.is_none());
    assert!(short.execute(&ALICE_ID, &mut tx).is_err());
    assert_eq!(*tx.world.runtime_child_admin_state.get(), before);
    assert_eq!(tx.world.runtime_change_sets.iter().count(), 0);
    let unknown = action(
        &tx,
        &keys[..2],
        ChildRuntimeAdminActionV1::Cancel(ChangeSetId([77; 32])),
    );
    assert!(execute(&mut tx, unknown.into()).is_err());
    assert_eq!(*tx.world.runtime_child_admin_state.get(), before);
    // A rejected action cannot leak its prepared nonce even if the enclosing transaction applies.
    tx.apply();
    block.commit();
    assert_eq!(*state.world.view().runtime_child_admin_state(), before);
}

#[test]
fn selected_empty_populated_unselected_foreign_and_global_authority_are_refused() {
    let (state, keys) = fixture();
    let mut block = state.block(header());
    let mut tx = block.transaction();
    bind_scope(&mut tx);
    let valid = action(
        &tx,
        &keys[..2],
        ChildRuntimeAdminActionV1::Cancel(ChangeSetId([1; 32])),
    );
    let saved = tx.world.runtime_child_admin_state.get().clone();
    *tx.world.runtime_child_admin_state.get_mut() = None;
    assert!(valid.clone().execute(&ALICE_ID, &mut tx).is_err());
    assert!(committed_authority(&mut tx).is_err());
    *tx.world.runtime_child_admin_state.get_mut() = saved;
    let selected = tx.world.runtime_selection.canonical().clone();
    *tx.world.runtime_selection.get_mut() =
        super::super::release_catalog().unwrap().baseline.clone();
    assert!(!enabled(tx.world.runtime_selection.canonical()).unwrap());
    assert!(committed_authority(&mut tx).is_err());
    *tx.world.runtime_selection.get_mut() = selected;
    tx.world
        .runtime_child_admin_state
        .get_mut()
        .as_mut()
        .unwrap()
        .root
        .instance[0] ^= 1;
    assert!(committed_authority(&mut tx).is_err());
    tx.world
        .runtime_child_admin_state
        .get_mut()
        .as_mut()
        .unwrap()
        .root
        .instance[0] ^= 1;
    tx.world
        .set_parameter_for_testing(test_support::metadata(SumeragiRootScope::Global))
        .unwrap();
    assert!(authenticated_identity(&mut tx).is_err());
}

#[test]
fn direct_source_refuses_substitution_nested_execution_wrong_entry_and_mixed_body() {
    let (state, keys) = fixture();
    let mut block = state.block(header());
    let mut tx = block.transaction();
    bind_scope(&mut tx);
    let first = action(
        &tx,
        &keys[..2],
        ChildRuntimeAdminActionV1::Cancel(ChangeSetId([1; 32])),
    );
    let second = action(
        &tx,
        &keys[..2],
        ChildRuntimeAdminActionV1::Cancel(ChangeSetId([2; 32])),
    );
    let boxed: InstructionBox = first.clone().into();
    let source = signed(&tx, vec![boxed.clone()]);
    bind(&mut tx, &source);
    for (index, direct) in [(0, false), (1, true), (usize::MAX, true)] {
        assert!(
            capture_direct(&mut tx, &source, &ALICE_ID, &boxed, index, direct)
                .unwrap()
                .is_none()
        );
    }
    assert!(
        capture_direct(&mut tx, &source, &ALICE_ID, &second.clone().into(), 0, true)
            .unwrap()
            .is_none()
    );
    tx.current_entrypoint_index = None;
    assert!(
        capture_direct(&mut tx, &source, &ALICE_ID, &boxed, 0, true)
            .unwrap()
            .is_none()
    );
    bind(&mut tx, &source);
    tx.deferred_expectations.push(None);
    assert!(
        capture_direct(&mut tx, &source, &ALICE_ID, &boxed, 0, true)
            .unwrap()
            .is_none()
    );
    tx.deferred_expectations.clear();
    let mixed = signed(
        &tx,
        vec![boxed.clone(), Log::new(Level::INFO, "extra".into()).into()],
    );
    bind(&mut tx, &mixed);
    assert!(
        capture_direct(&mut tx, &mixed, &ALICE_ID, &boxed, 0, true)
            .unwrap()
            .is_none()
    );
    arm(&mut tx, &boxed);
    assert!(second.execute(&ALICE_ID, &mut tx).is_err());
    assert!(tx.child_runtime_admin_entrypoint.is_none());
    arm(&mut tx, &boxed);
    tx.deferred_expectations.push(None);
    assert!(first.execute(&ALICE_ID, &mut tx).is_err());
    assert!(tx.child_runtime_admin_entrypoint.is_none());
}

#[test]
fn child_validator_support_is_separate_from_administrator_approval() {
    let (state, admins) = fixture();
    let mut block = state.block(header());
    let mut tx = block.transaction();
    bind_scope(&mut tx);
    let validators: Vec<_> = (80..84)
        .map(|seed| KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519))
        .collect();
    let committee: Vec<_> = validators
        .iter()
        .map(|key| key.public_key().clone())
        .collect();
    *tx.commit_topology.get_mut() = committee.iter().cloned().map(PeerId::new).collect();
    let change = noop(&tx, 7);
    let id = change.id().unwrap();
    let schedule = action(
        &tx,
        &admins[..2],
        ChildRuntimeAdminActionV1::Schedule {
            change_set: change.clone(),
            provenance: vec![],
        },
    );
    execute(&mut tx, schedule.into()).unwrap();
    let support = |key: &KeyPair| {
        SupportAttestationV1 {
            network: change.network,
            root: change.root,
            change_set: id,
            predecessor: change.predecessor,
            successor: change.successor,
            validator: key.public_key().clone(),
            committee: committee_digest(&committee).unwrap(),
            valid_from: 5,
            valid_until: 20,
            release: [7; 32],
            tested_checkpoint: 4,
        }
        .sign(key.private_key())
        .unwrap()
    };
    let admin_support = AttestChildRuntimeSupportV1 {
        attestation: support(&admins[0]),
    };
    assert!(execute(&mut tx, admin_support.into()).is_err());
    for validator in &validators {
        execute(
            &mut tx,
            AttestChildRuntimeSupportV1 {
                attestation: support(validator),
            }
            .into(),
        )
        .unwrap();
    }
    assert_eq!(
        tx.world
            .runtime_change_sets
            .get(&id)
            .unwrap()
            .attestations
            .len(),
        4
    );
    assert_eq!(
        tx.world
            .runtime_child_admin_state
            .get()
            .as_ref()
            .unwrap()
            .next_nonce,
        1
    );
}

#[test]
fn explicit_release_provenance_is_independent_and_failed_draft_is_atomic() {
    use iroha_data_model::{
        parameter::runtime_provenance_policy::RuntimeProvenancePolicyV1,
        runtime::lifecycle::ChangeSetEndorsementV1,
    };
    let (state, admins) = fixture();
    let mut block = state.block(header());
    let mut tx = block.transaction();
    bind_scope(&mut tx);
    let release = KeyPair::from_seed(vec![97; 32], Algorithm::Ed25519);
    let mut policy = tx
        .world
        .runtime_child_admin_state
        .get()
        .as_ref()
        .unwrap()
        .policy
        .clone();
    policy.revision += 1;
    policy.provenance = Some(RuntimeProvenancePolicyV1 {
        signers: vec![release.public_key().clone()],
        threshold: 1,
    });
    let rotate = action(
        &tx,
        &admins[..2],
        ChildRuntimeAdminActionV1::RotatePolicy(policy),
    );
    execute(&mut tx, rotate.into()).unwrap();
    let change = noop(&tx, 7);
    let id = change.id().unwrap();
    let unendorsed = action(
        &tx,
        &admins[..2],
        ChildRuntimeAdminActionV1::Schedule {
            change_set: change.clone(),
            provenance: vec![],
        },
    );
    assert!(execute(&mut tx, unendorsed.into()).is_err());
    assert!(tx.world.runtime_change_sets.get(&id).is_none());
    assert_eq!(
        tx.world
            .runtime_child_admin_state
            .get()
            .as_ref()
            .unwrap()
            .next_nonce,
        1
    );
    let endorsement =
        ChangeSetEndorsementV1::sign(id, release.public_key().clone(), release.private_key())
            .unwrap();
    let endorsed = action(
        &tx,
        &admins[..2],
        ChildRuntimeAdminActionV1::Schedule {
            change_set: change,
            provenance: vec![endorsement],
        },
    );
    execute(&mut tx, endorsed.into()).unwrap();
    assert!(tx.world.runtime_change_sets.get(&id).is_some());
}

#[test]
fn actual_child_draft_retains_root_identity_and_refuses_missing_or_foreign_authority() {
    let (state, _) = fixture();
    let mut block = state.block(header());
    let mut tx = block.transaction();
    bind_scope(&mut tx);
    let module = super::super::catalog::additions::RESTRICTED_DATASPACE_READ.module;
    let expected = tx
        .world
        .runtime_child_admin_state
        .get()
        .as_ref()
        .unwrap()
        .root;
    let change =
        super::super::draft_addition(&*tx.world, expected.network, module, 7, [9; 32]).unwrap();
    assert_eq!(change.network, expected.network);
    assert_eq!(change.root, RootScopeV1::DataSpace(expected.dataspace));
    tx.world
        .runtime_child_admin_state
        .get_mut()
        .as_mut()
        .unwrap()
        .root
        .parent_network = expected.network;
    assert!(
        super::super::draft_addition(&*tx.world, expected.network, module, 7, [9; 32]).is_err()
    );
    *tx.world.runtime_child_admin_state.get_mut() = None;
    assert!(
        super::super::draft_addition(&*tx.world, expected.network, module, 7, [9; 32]).is_err()
    );
}

#[test]
fn borrowed_overlay_cannot_inherit_original_direct_authority() {
    let (state, keys) = fixture();
    let mut block = state.block(header());
    let mut tx = block.transaction();
    bind_scope(&mut tx);
    let mut next = tx
        .world
        .runtime_child_admin_state
        .get()
        .as_ref()
        .unwrap()
        .policy
        .clone();
    next.revision += 1;
    let instruction: InstructionBox = action(
        &tx,
        &keys[..2],
        ChildRuntimeAdminActionV1::RotatePolicy(next),
    )
    .into();
    arm(&mut tx, &instruction);
    assert!(
        Executor::Initial
            .execute_borrowed_overlay_instruction(&mut tx, &ALICE_ID, &instruction, None)
            .is_err()
    );
    assert!(tx.child_runtime_admin_entrypoint.is_none());
    assert_eq!(
        tx.world
            .runtime_child_admin_state
            .get()
            .as_ref()
            .unwrap()
            .next_nonce,
        0
    );
}

#[test]
fn populated_selected_cell_alone_does_not_establish_original_foundation() {
    let (state, keys) = fixture_with_recipe(false);
    let mut block = state.block(header());
    let mut tx = block.transaction();
    bind_scope(&mut tx);
    let mut next = tx
        .world
        .runtime_child_admin_state
        .get()
        .as_ref()
        .unwrap()
        .policy
        .clone();
    next.revision += 1;
    let instruction = action(
        &tx,
        &keys[..2],
        ChildRuntimeAdminActionV1::RotatePolicy(next),
    );
    assert!(execute(&mut tx, instruction.into()).is_err());
    assert_eq!(
        tx.world
            .runtime_child_admin_state
            .get()
            .as_ref()
            .unwrap()
            .next_nonce,
        0
    );
}

#[test]
fn child_lifecycle_draft_uses_original_foundation_and_keeps_supported_composition() {
    let (state, _) = fixture();
    let mut block = state.block(header());
    let mut tx = block.transaction();
    bind_scope(&mut tx);
    let root = tx
        .world
        .runtime_child_admin_state
        .get()
        .as_ref()
        .unwrap()
        .root;
    let change = super::super::lifecycle_activation::prepare_lifecycle_change_set(
        &*tx.world,
        root.network,
        7,
        [9; 32],
    )
    .unwrap();
    assert_eq!(change.root, RootScopeV1::DataSpace(root.dataspace));
    assert_eq!(change.network, root.network);
    let applied = change
        .apply(
            tx.world.runtime_selection.canonical(),
            &PublishedDefinitions(&tx.world.runtime_definitions),
        )
        .unwrap();
    assert_eq!(
        super::super::release_catalog()
            .unwrap()
            .unsupported(&applied.selection),
        None
    );
    assert!(enabled(&applied.selection).unwrap());
    // A local overlay that merely replaces revision-one administrators cannot draft authority.
    tx.world
        .runtime_child_admin_state
        .get_mut()
        .as_mut()
        .unwrap()
        .policy
        .threshold = 1;
    assert!(
        super::super::lifecycle_activation::prepare_lifecycle_change_set(
            &*tx.world,
            root.network,
            7,
            [9; 32]
        )
        .is_err()
    );
    let (state, _) = fixture_with_recipe(false);
    let mut block = state.block(header());
    let tx = block.transaction();
    assert!(
        super::super::lifecycle_activation::prepare_lifecycle_change_set(
            &*tx.world,
            *tx.network_id(),
            7,
            [9; 32]
        )
        .is_err()
    );
}

#[test]
fn all_migration_draft_steps_keep_exact_child_scope_and_refuse_unfounded_state() {
    use super::super::{
        catalog::{
            MigrationStep,
            additions::{CHILD_ADMIN, RUNTIME_LIFECYCLE, test_additions::MIGRATED},
            migrations::test_migrations::{FROM, MODULE, SEQUENCES},
            test_catalog,
        },
        migration::draft_step,
    };
    test_catalog::with_release(
        &[MIGRATED, RUNTIME_LIFECYCLE, CHILD_ADMIN],
        &[SEQUENCES],
        || {
            let (state, _) = fixture();
            let mut block = state.block(header());
            let mut tx = block.transaction();
            bind_scope(&mut tx);
            let catalog = super::super::release_catalog().unwrap();
            let old = catalog.world_storage[FROM];
            tx.world
                .runtime_definitions
                .insert(old, catalog.definitions[&old].clone());
            tx.world
                .runtime_selection
                .get_mut()
                .installed_storage
                .push(old);
            tx.world
                .runtime_selection
                .get_mut()
                .installed_storage
                .sort();
            let root = tx
                .world
                .runtime_child_admin_state
                .get()
                .as_ref()
                .unwrap()
                .root;
            let authority = tx.world.runtime_child_admin_state.get().clone();
            *tx.world.runtime_child_admin_state.get_mut() = None;
            let error = draft_step(
                &*tx.world,
                root.network,
                MODULE,
                MigrationStep::Start,
                7,
                [9; 32],
            )
            .unwrap_err();
            assert!(error.to_string().contains("administrator"), "{error}");
            *tx.world.runtime_child_admin_state.get_mut() = authority;
            for (step, height) in [
                (MigrationStep::Start, 7),
                (MigrationStep::CutOver, 9),
                (MigrationStep::Retire, 11),
            ] {
                let change =
                    draft_step(&*tx.world, root.network, MODULE, step, height, [9; 32]).unwrap();
                assert_eq!(change.network, root.network);
                assert_eq!(change.root, RootScopeV1::DataSpace(root.dataspace));
                let applied = change
                    .apply(
                        tx.world.runtime_selection.canonical(),
                        &PublishedDefinitions(&tx.world.runtime_definitions),
                    )
                    .unwrap();
                for (id, definition) in applied.published {
                    tx.world.runtime_definitions.insert(id, definition);
                }
                *tx.world.runtime_selection.get_mut() = applied.selection;
            }
        },
    );
}

#[test]
fn shared_draft_scope_preserves_universal_drafts_and_retains_original_refusal() {
    let world = World::default();
    let network = NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(
        b"draft-universal",
    )));
    let selection = world.view().runtime_selection().id().unwrap();
    let change = ChangeSetV1 {
        network,
        root: RootScopeV1::Universal,
        predecessor: selection,
        successor: selection,
        effective_height: 7,
        publish: vec![],
        transitions: vec![],
        qualification: [9; 32],
    };
    assert_eq!(
        super::super::draft_scope::bind(&world.view(), network, change.clone()).unwrap(),
        change
    );
    let foreign =
        NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(b"different-root")));
    assert!(super::super::draft_scope::bind(&world.view(), foreign, change).is_err());
    let (state, _) = fixture();
    let mut block = state.block(header());
    let tx = block.transaction();
    let change = noop(&tx, 7);
    let limits = norito::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, 0, 64);
    let result = norito::core::with_decode_limits_scope(limits, || {
        super::super::draft_scope::bind(&*tx.world, *tx.network_id(), change)
    });
    assert!(matches!(result, Err(ExecutionAttemptError::Deferred(_))));
}
