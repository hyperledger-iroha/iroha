//! Child foundation snapshots retain both authority generations without granting history.

use super::*;
use crate::query::store::LiveQueryStore;
use iroha_data_model::{
    NetworkId,
    block::consensus::SumeragiRootScope,
    parameter::Parameter,
    runtime::child_admin::{
        ChildRootIdentityV1, ChildRuntimeAdminPolicyV1, ChildRuntimeAdminStateV1,
        foundation::ChildRuntimeFoundationV1,
    },
};

fn fixture() -> (State, ChildRuntimeAdminStateV1) {
    let state = State::new_for_testing(
        World::default(),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    let parent_network_id = NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(
        Hash::new(b"child snapshot parent"),
    ));
    let dataspace_id = DataSpaceId::new((1_u64 << 42) + 71);
    let scope = SumeragiRootScope::Dataspace {
        parent_network_id,
        dataspace_id,
    };
    let (selection, definitions) = crate::runtime::release_catalog()
        .unwrap()
        .child_foundation()
        .unwrap();
    let authority = ChildRuntimeAdminStateV1 {
        root: ChildRootIdentityV1 {
            network: state.network_id,
            parent_network: parent_network_id,
            dataspace: dataspace_id,
            instance: scope
                .instance_id(
                    &crate::sumeragi::crypto::BlsCrypto::new(),
                    state.network_id,
                    state.chain_id_ref().as_str(),
                )
                .unwrap()
                .0,
        },
        policy: ChildRuntimeAdminPolicyV1 {
            revision: 1,
            admins: vec![iroha_test_samples::ALICE_KEYPAIR.public_key().clone()],
            threshold: 1,
            notice: Default::default(),
            provenance: None,
        },
        next_nonce: 0,
    };
    {
        let mut block = state.world.block();
        block
            .set_parameter_for_testing(crate::sumeragi::lanes::routing::test_support::metadata(
                scope,
            ))
            .unwrap();
        block
            .set_parameter_for_testing(Parameter::Custom(
                ChildRuntimeFoundationV1 {
                    selection: selection.id().unwrap(),
                    policy: authority.policy.clone(),
                }
                .to_custom_parameter()
                .unwrap(),
            ))
            .unwrap();
        for (id, descriptor) in definitions {
            block.runtime_definitions.insert(id, descriptor);
        }
        *block.runtime_selection.get_mut() = selection;
        *block.runtime_child_admin_state.get_mut() = Some(authority.clone());
        block.commit();
    }
    (state, authority)
}

fn write(state: &State, authority: ChildRuntimeAdminStateV1) {
    let mut block = state.world.block();
    *block.runtime_child_admin_state.get_mut() = Some(authority);
    block.commit();
}

fn written_world(state: &State) -> String {
    let snapshot = json::to_json(state).unwrap();
    json_object_members(&snapshot)
        .unwrap()
        .into_iter()
        .find(|(key, _)| *key == "world")
        .unwrap()
        .1
        .to_owned()
}

fn restore(encoded: &str) -> Result<World, StateRestoreError> {
    let mut map = SnapshotJsonMap::parse_world_fixture(encoded).unwrap();
    map.omit_uninstalled_world_members()?;
    let ivm = IVM::new(0);
    parse_world(
        &iroha_allocation::AllocationBudget::new(
            iroha_config::parameters::defaults::pipeline::IVM_EXECUTION_MAX_BYTES,
        ),
        map,
        &IvmSeed {
            ivm: &ivm,
            _marker: PhantomData,
        },
    )
}

#[test]
fn child_snapshot_preserves_current_policy_nonce_and_exact_predecessor() {
    let (mut state, first) = fixture();
    let mut next = first.clone();
    next.next_nonce = 4;
    next.policy.revision = 2;
    next.policy.admins = vec![iroha_test_samples::BOB_KEYPAIR.public_key().clone()];
    write(&state, next.clone());
    validate_world(&state.world, &state.ivm_execution_budget()).unwrap();
    validate_identity(&state).unwrap();
    let encoded_cell = json::to_json(&state.world.runtime_child_admin_state).unwrap();
    let restored = restore(&written_world(&state)).unwrap();
    assert_eq!(
        json::to_json(&restored.runtime_child_admin_state).unwrap(),
        encoded_cell
    );
    assert_eq!(restored.runtime_child_admin_state.view().get(), &Some(next));
    assert_eq!(
        restored.runtime_child_admin_state.block_and_revert().get(),
        &Some(first)
    );
    state.world = restored;
    validate_identity(&state).unwrap();
    assert_eq!(
        state.committed_height(),
        0,
        "component decoding grants no native history"
    );
}

#[test]
fn child_snapshot_field_is_required_only_when_installed() {
    let baseline = State::new_for_testing(
        World::default(),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    let encoded = written_world(&baseline);
    assert!(
        !json_object_members(&encoded)
            .unwrap()
            .iter()
            .any(|(key, _)| *key == "runtime_child_admin_state")
    );
    let restored = restore(&encoded).unwrap();
    assert!(restored.runtime_child_admin_state.view().get().is_none());

    let (state, _) = fixture();
    let encoded = written_world(&state);
    let members = json_object_members(&encoded).unwrap();
    assert!(
        members
            .iter()
            .any(|(key, _)| *key == "runtime_child_admin_state")
    );
    let missing = format!(
        "{{{}}}",
        members
            .iter()
            .filter(|(key, _)| *key != "runtime_child_admin_state")
            .map(|(key, raw)| format!("\"{key}\":{raw}"))
            .collect::<Vec<_>>()
            .join(",")
    );
    let error = restore(&missing)
        .err()
        .expect("installed authority cannot disappear");
    assert!(format!("{error}").contains("runtime_child_admin_state"));
}

#[test]
fn child_snapshot_rejects_invalid_current_and_retained_policy_without_mutation() {
    for predecessor in [false, true] {
        let (state, valid) = fixture();
        let mut invalid = valid.clone();
        invalid.policy.threshold = 0;
        write(&state, invalid);
        if predecessor {
            write(&state, valid);
        }
        let before = written_world(&state);
        let error = restore(&before)
            .err()
            .expect("invalid authority is refused");
        assert!(matches!(error, StateRestoreError::Serialization(_)));
        assert_eq!(written_world(&state), before);
    }
}

#[test]
fn child_snapshot_rechecks_network_and_instance_at_both_cuts() {
    for predecessor in [false, true] {
        for change_network in [false, true] {
            let (state, valid) = fixture();
            let mut foreign = valid.clone();
            if change_network {
                foreign.root.network = foreign.root.parent_network;
            } else {
                foreign.root.instance[0] ^= 1;
            }
            write(&state, foreign);
            if predecessor {
                write(&state, valid);
            }
            let before = written_world(&state);
            assert!(validate_identity(&state).is_err());
            assert_eq!(written_world(&state), before);
        }
    }
}

#[test]
fn child_snapshot_original_decoder_refusal_remains_retryable() {
    let (state, _) = fixture();
    let before = written_world(&state);
    let result = norito::core::with_decode_limits_scope(
        norito::core::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, 0, usize::MAX),
        || validate_world(&state.world, &state.ivm_execution_budget()),
    );
    assert!(matches!(
        result,
        Err(StateRestoreError::ExecutionDeferred(_))
    ));
    assert_eq!(written_world(&state), before);
    validate_world(&state.world, &state.ivm_execution_budget()).unwrap();
    assert_eq!(written_world(&state), before);
}

#[test]
fn child_snapshot_cannot_erase_current_or_predecessor_authority_but_retain_its_recipe() {
    for predecessor in [false, true] {
        let (state, authority) = fixture();
        {
            let mut block = state.world.block();
            *block.runtime_child_admin_state.get_mut() = None;
            block.commit();
        }
        if predecessor {
            write(&state, authority);
        }
        let before = written_world(&state);
        let error = restore(&before)
            .err()
            .expect("founded authority cannot be erased");
        assert!(error.to_string().contains("no administrator state"));
        assert_eq!(written_world(&state), before);
    }
}

#[test]
fn a_global_custom_marker_does_not_initialize_or_require_child_authority() {
    let (child, _) = fixture();
    let marker = child
        .world_view()
        .parameters()
        .custom()
        .get(ChildRuntimeFoundationV1::parameter_id())
        .unwrap()
        .clone();
    let state = State::new_for_testing(
        World::default(),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    {
        let mut block = state.world.block();
        block
            .set_parameter_for_testing(crate::sumeragi::lanes::routing::test_support::metadata(
                SumeragiRootScope::Global,
            ))
            .unwrap();
        block
            .set_parameter_for_testing(Parameter::Custom(marker))
            .unwrap();
        block.commit();
    }
    validate_world(&state.world, &state.ivm_execution_budget()).unwrap();
    let restored = restore(&written_world(&state)).unwrap();
    assert!(restored.runtime_child_admin_state.view().get().is_none());
}
