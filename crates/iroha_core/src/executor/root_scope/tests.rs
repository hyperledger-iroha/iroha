//! Actual owned, borrowed and nested private instruction boundaries.

use super::*;
use crate::{
    executor::Executor,
    query::store::LiveQueryStore,
    smartcontracts::Execute as _,
    state::{State, StateReadOnly as _, World},
    sumeragi::lanes::routing::test_support,
};
use iroha_crypto::{Hash, HashOf};
use iroha_data_model::{
    NetworkId, Registrable,
    account::Account,
    block::BlockHeader,
    isi::{CustomInstruction, RegisterPeerWithPop, SetParameter},
    nexus::{DataSpaceCatalog, DataSpaceMetadata},
    smart_contract::ContractAddress,
    transaction::{Executable, IvmBytecode, IvmProved},
};
use iroha_model_base::topology::DataSpaceId;
use iroha_test_samples::ALICE_ID;

fn private_state() -> (State, DataSpaceId) {
    let ds = DataSpaceId::new((1_u64 << 40) + 17);
    let scope = SumeragiRootScope::Dataspace {
        parent_network_id: NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(
            b"parent",
        ))),
        dataspace_id: ds,
    };
    let world = World::with([], [Account::new(ALICE_ID.clone()).build(&ALICE_ID)], []);
    let mut parameters = world.parameters.block();
    parameters.set_parameter(test_support::metadata(scope));
    parameters.commit();
    let mut nexus = iroha_config::parameters::actual::Nexus::default();
    nexus.dataspace_catalog = DataSpaceCatalog::new(vec![
        DataSpaceMetadata::default(),
        DataSpaceMetadata {
            id: ds,
            alias: "private-scope-test".into(),
            description: None,
            fault_tolerance: 1,
        },
    ])
    .unwrap();
    (
        State::new_with_nexus_for_testing(world, nexus, LiveQueryStore::start_test()),
        ds,
    )
}

fn bind_scope(tx: &mut StateTransaction<'_, '_>, ds: DataSpaceId) {
    tx.current_dataspace_id = Some(ds);
    tx.world.current_dataspace_id = Some(ds);
}

fn header(height: u64) -> BlockHeader {
    BlockHeader::new(height.try_into().unwrap(), None, None, 1_000, 0)
}

#[test]
fn owned_borrowed_and_native_boxed_paths_reject_private_global_control_before_effects() {
    let (state, ds) = private_state();
    let mut block = state.block(header(2));
    let mut tx = block.transaction();
    bind_scope(&mut tx, ds);
    let control: InstructionBox =
        SetParameter::new(test_support::metadata(SumeragiRootScope::Global)).into();
    assert!(
        Executor::Initial
            .execute_instruction(&mut tx, &ALICE_ID, control.clone())
            .is_err()
    );
    assert!(
        Executor::Initial
            .execute_borrowed_overlay_instruction(&mut tx, &ALICE_ID, &control, None)
            .is_err()
    );
    assert!(control.execute(&ALICE_ID, &mut tx).is_err());
    assert!(
        matches!(execution_root_scope(&tx).unwrap(), SumeragiRootScope::Dataspace { dataspace_id, .. } if dataspace_id == ds)
    );
    let log: InstructionBox = Log::new(iroha_data_model::Level::INFO, "root-local".into()).into();
    Executor::Initial
        .execute_instruction(&mut tx, &ALICE_ID, log.clone())
        .unwrap();
    Executor::Initial
        .execute_borrowed_overlay_instruction(&mut tx, &ALICE_ID, &log, None)
        .unwrap();
    log.execute(&ALICE_ID, &mut tx).unwrap();
}

#[test]
fn parameter_control_cannot_hide_in_deferred_multisig_or_an_unknown_local_target() {
    let (state, ds) = private_state();
    let mut block = state.block(header(2));
    let mut tx = block.transaction();
    bind_scope(&mut tx, ds);
    let propose = iroha_executor_data_model::isi::multisig::MultisigPropose {
        account: ALICE_ID.clone(),
        instructions: vec![
            SetParameter::new(test_support::metadata(SumeragiRootScope::Global)).into(),
        ],
        transaction_ttl_ms: None,
    };
    let instruction = InstructionBox::from(MultisigInstructionBox::Propose(propose));
    assert!(ensure_instruction_scope(&instruction, &tx).is_err());
    let no_scope: InstructionBox =
        CustomInstruction::new("unreviewed private-root operation").into();
    let peer_key =
        iroha_crypto::KeyPair::from_seed(vec![71; 32], iroha_crypto::Algorithm::BlsNormal);
    let register_peer: InstructionBox = RegisterPeerWithPop::new(
        iroha_model_base::peer::PeerId::new(peer_key.public_key().clone()),
        iroha_crypto::bls_normal_pop_prove(peer_key.private_key()).expect("fixture PoP"),
    )
    .into();
    let register_account: InstructionBox =
        iroha_data_model::isi::Register::account(Account::new(ALICE_ID.clone())).into();
    for unreviewed in [no_scope, register_peer, register_account] {
        assert!(
            matches!(ensure_instruction_scope(&unreviewed, &tx), Err(ValidationFail::NotPermitted(reason)) if reason.contains("reviewed private-root scope owner"))
        );
    }
}

#[test]
fn private_contract_calls_require_exact_scope_and_raw_overlays_remain_closed() {
    let (state, ds) = private_state();
    let mut block = state.block(header(2));
    let mut tx = block.transaction();
    bind_scope(&mut tx, ds);
    for (target, allowed) in [
        (ds, true),
        (DataSpaceId::new(17), false),
        (DataSpaceId::UNIVERSAL, false),
    ] {
        let address = ContractAddress::derive(&tx.network_id, &ALICE_ID, 0, target).unwrap();
        assert_eq!(ensure_contract_scope(&tx, &address).is_ok(), allowed);
        assert_eq!(
            ensure_committed_contract_scope(&tx.world, &address).is_ok(),
            allowed
        );
    }
    let bytecode = IvmBytecode::from_compiled(vec![1]);
    assert!(ensure_executable_scope(&tx, &Executable::Ivm(bytecode.clone())).is_err());
    assert!(
        ensure_executable_scope(
            &tx,
            &Executable::IvmProved(IvmProved {
                bytecode,
                overlay: Vec::new().into(),
                events_commitment: Hash::new(b"events"),
                gas_policy_commitment: Hash::new(b"gas"),
            })
        )
        .is_err()
    );
    tx.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
    let address = ContractAddress::derive(&tx.network_id, &ALICE_ID, 0, ds).unwrap();
    assert!(ensure_contract_scope(&tx, &address).is_err());
}

#[test]
fn height_one_without_authenticated_genesis_capability_has_no_instruction_authority() {
    for world in [World::new(), test_support::world(SumeragiRootScope::Global)] {
        let state = State::new_for_testing(
            world,
            crate::kura::Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        );
        let mut block = state.block(header(1));
        let mut tx = block.transaction();
        let log: InstructionBox =
            Log::new(iroha_data_model::Level::INFO, "forged bootstrap".into()).into();
        assert!(execution_root_scope(&tx).is_err());
        assert!(
            Executor::Initial
                .execute_instruction(&mut tx, &ALICE_ID, log.clone())
                .is_err()
        );
        assert!(log.execute(&ALICE_ID, &mut tx).is_err());
    }
}

#[test]
fn artifact_lookup_requires_both_captured_namespaces_and_preserves_full_width() {
    let (state, ds) = private_state();
    let mut block = state.block(header(2));
    let mut tx = block.transaction();
    let hash = Hash::new(b"artifact");
    assert!(captured_artifact_id(&tx, hash).is_err());
    bind_scope(&mut tx, ds);
    let artifact = captured_artifact_id(&tx, hash).unwrap();
    assert_eq!(artifact.dataspace_id, ds);
    assert_eq!(artifact.code_hash, hash);
    tx.world.current_dataspace_id = Some(DataSpaceId::new(17));
    assert!(captured_artifact_id(&tx, hash).is_err());
    bind_scope(&mut tx, DataSpaceId::new(17));
    assert!(captured_artifact_id(&tx, hash).is_err());
}

#[test]
fn durable_authorization_rejects_foreign_contract_before_registry_or_permission_lookup() {
    let (state, _) = private_state();
    let mut block = state.block(header(2));
    let tx = block.transaction();
    let foreign =
        ContractAddress::derive(&tx.network_id, &ALICE_ID, 0, DataSpaceId::UNIVERSAL).unwrap();
    let authorization = crate::executor::ContractEntrypointAuthorizationSnapshot {
        authority: ALICE_ID.clone(),
        entrypoint: "write".into(),
        permission: None,
        contract_address: foreign,
        contract_alias: None,
        contract_alias_binding: None,
        code_hash: Hash::new(b"foreign-artifact"),
        parent: None,
    };
    let error = authorization.validate(&tx.world).unwrap_err();
    assert!(error.to_string().contains("foreign contract dataspace"));
}

#[test]
fn artifact_registry_reads_require_immutable_scope_and_exact_private_owner() {
    use iroha_data_model::smart_contract::ContractArtifactId;
    let (state, own) = private_state();
    let view = state.view();
    let hash = Hash::new(b"shared artifact bytes");
    let local = ContractArtifactId::new(own, hash);
    assert!(ensure_committed_artifact_scope(view.world(), &local).is_ok());
    for ds in [DataSpaceId::UNIVERSAL, DataSpaceId::new(17)] {
        assert!(
            ensure_committed_artifact_scope(view.world(), &ContractArtifactId::new(ds, hash))
                .is_err()
        );
    }
    assert!(ensure_committed_artifact_scope(&World::new().view(), &local).is_err());
    let global = test_support::world(SumeragiRootScope::Global);
    assert!(ensure_committed_artifact_scope(&global.view(), &local).is_ok());
}

#[test]
fn private_roots_cannot_register_or_anchor_children_in_the_parent_registry() {
    use iroha_data_model::isi::private_dataspace::{
        AnchorPrivateDataspace, RegisterPrivateDataspace,
    };
    let (state, own) = private_state();
    let mut block = state.block(header(2));
    let mut tx = block.transaction();
    bind_scope(&mut tx, own);
    let operations: Vec<InstructionBox> = vec![
        RegisterPrivateDataspace {
            alias: "child".into(),
            expected_ownership_generation: 0,
            registration: vec![],
        }
        .into(),
        AnchorPrivateDataspace {
            dataspace_id: own,
            anchor: vec![],
        }
        .into(),
    ];
    for instruction in operations {
        let target = crate::queue::native_instruction_execution_target(
            &*instruction,
            &tx.nexus.dataspace_catalog,
            &tx.world,
            0,
        )
        .unwrap();
        assert_eq!(target.dataspace, Some(DataSpaceId::UNIVERSAL));
        assert!(target.global);
        let error = Executor::Initial
            .execute_instruction(&mut tx, &ALICE_ID, instruction)
            .unwrap_err();
        assert!(error.to_string().contains("global-control"));
    }
}
