//! Internal invocation namespaces come only from authenticated root authority.

use super::capture_internal_root_dataspace;
use crate::{
    executor::root_scope,
    kura::Kura,
    query::store::LiveQueryStore,
    state::{State, World},
    sumeragi::lanes::routing::test_support,
};
use iroha_crypto::{Hash, HashOf};
use iroha_data_model::{
    NetworkId,
    block::{BlockHeader, consensus::SumeragiRootScope},
    nexus::{DataSpaceCatalog, DataSpaceMetadata},
    parameter::{Parameter, custom::CustomParameter, system::consensus_metadata},
    smart_contract::ContractAddress,
};
use iroha_model_base::topology::DataSpaceId;
use iroha_test_samples::ALICE_ID;

fn private_scope() -> SumeragiRootScope {
    SumeragiRootScope::Dataspace {
        parent_network_id: NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(
            b"internal-root-parent",
        ))),
        dataspace_id: DataSpaceId::new((1_u64 << 40) + 17),
    }
}

fn state(world: World, scope: Option<SumeragiRootScope>) -> State {
    if let Some(SumeragiRootScope::Dataspace { dataspace_id, .. }) = scope {
        let mut nexus = iroha_config::parameters::actual::Nexus::default();
        nexus.dataspace_catalog = DataSpaceCatalog::new(vec![
            DataSpaceMetadata::default(),
            DataSpaceMetadata {
                id: dataspace_id,
                alias: "internal-private-root".into(),
                description: None,
                fault_tolerance: 1,
            },
        ])
        .unwrap();
        State::new_with_nexus_for_testing(world, nexus, LiveQueryStore::start_test())
    } else {
        State::new_for_testing(
            world,
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        )
    }
}

fn header(height: u64) -> BlockHeader {
    BlockHeader::new(height.try_into().unwrap(), None, None, 1_000, 0)
}

#[test]
fn internal_capture_binds_both_namespaces_to_the_complete_committed_root() {
    for scope in [SumeragiRootScope::Global, private_scope()] {
        let state = state(test_support::world(scope), Some(scope));
        let mut block = state.block(header(2));
        let mut tx = block.transaction();
        assert_eq!(tx.current_dataspace_id, None);
        assert_eq!(tx.world.current_dataspace_id, None);
        let expected = scope.dataspace_id();
        assert_eq!(capture_internal_root_dataspace(&mut tx).unwrap(), expected);
        assert_eq!(tx.current_dataspace_id, Some(expected));
        assert_eq!(tx.world.current_dataspace_id, Some(expected));
        let hash = Hash::new(b"internal-program");
        let artifact = root_scope::captured_artifact_id(&tx, hash).unwrap();
        assert_eq!(artifact.dataspace_id, expected);
        assert_eq!(artifact.code_hash, hash);
        let generic = crate::smartcontracts::ivm::validate_generic_execution_context(
            &tx.world,
            &Default::default(),
            artifact,
        );
        assert_eq!(generic.is_ok(), scope == SumeragiRootScope::Global);
    }
}

#[test]
fn internal_private_capture_keeps_foreign_contracts_and_namespace_substitution_closed() {
    let scope = private_scope();
    let expected = scope.dataspace_id();
    let state = state(test_support::world(scope), Some(scope));
    let mut block = state.block(header(2));
    let mut tx = block.transaction();
    capture_internal_root_dataspace(&mut tx).unwrap();
    for (target, allowed) in [
        (expected, true),
        (DataSpaceId::new(17), false),
        (DataSpaceId::UNIVERSAL, false),
    ] {
        let address = ContractAddress::derive(&tx.network_id, &ALICE_ID, 0, target).unwrap();
        assert_eq!(
            root_scope::ensure_contract_scope(&tx, &address).is_ok(),
            allowed
        );
    }
    tx.world.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
    assert!(root_scope::captured_artifact_id(&tx, Hash::new(b"program")).is_err());
    tx.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
    assert!(root_scope::captured_artifact_id(&tx, Hash::new(b"program")).is_err());
}

#[test]
fn internal_capture_rejects_missing_malformed_or_unauthenticated_genesis_root() {
    let malformed = World::new();
    let mut parameters = malformed.parameters.block();
    parameters.set_parameter(Parameter::Custom(CustomParameter::new(
        consensus_metadata::handshake_meta_id(),
        iroha_primitives::json::Json::from_norito_value_ref(&norito::json::Value::Bool(false))
            .unwrap(),
    )));
    parameters.commit();
    for (world, height) in [
        (World::new(), 2),
        (malformed, 2),
        (test_support::world(SumeragiRootScope::Global), 1),
    ] {
        let state = state(world, None);
        let mut block = state.block(header(height));
        let mut tx = block.transaction();
        assert!(capture_internal_root_dataspace(&mut tx).is_err());
        assert_eq!(tx.current_dataspace_id, None);
        assert_eq!(tx.world.current_dataspace_id, None);
    }
}

#[test]
fn internal_capture_never_overwrites_any_preexisting_namespace() {
    let scope = private_scope();
    let expected = scope.dataspace_id();
    let state = state(test_support::world(scope), Some(scope));
    let mut block = state.block(header(2));
    for (own, world) in [
        (Some(expected), None),
        (None, Some(expected)),
        (Some(expected), Some(expected)),
        (Some(DataSpaceId::UNIVERSAL), Some(expected)),
        (Some(expected), Some(DataSpaceId::UNIVERSAL)),
    ] {
        let mut tx = block.transaction();
        tx.current_dataspace_id = own;
        tx.world.current_dataspace_id = world;
        assert!(capture_internal_root_dataspace(&mut tx).is_err());
        assert_eq!(tx.current_dataspace_id, own);
        assert_eq!(tx.world.current_dataspace_id, world);
    }
}
