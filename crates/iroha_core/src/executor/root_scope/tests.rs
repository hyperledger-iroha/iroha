//! Actual owned, borrowed and nested private instruction boundaries.

mod amx_roles;

use super::*;
use crate::{
    executor::Executor,
    query::store::LiveQueryStore,
    smartcontracts::Execute as _,
    state::{State, World},
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
    private_state_with_world(World::with(
        [],
        [Account::new(ALICE_ID.clone()).build(&ALICE_ID)],
        [],
    ))
}

fn private_state_with_world(world: World) -> (State, DataSpaceId) {
    let ds = DataSpaceId::new((1_u64 << 40) + 17);
    let scope = SumeragiRootScope::Dataspace {
        parent_network_id: NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(
            b"parent",
        ))),
        dataspace_id: ds,
    };
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
        matches!(execution_root_scope(&mut tx).unwrap(), SumeragiRootScope::Dataspace { dataspace_id, .. } if dataspace_id == ds)
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
    assert!(ensure_instruction_scope(&instruction, &mut tx).is_err());
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
            matches!(ensure_instruction_scope(&unreviewed, &mut tx), Err(ValidationFail::NotPermitted(reason)) if reason.contains("reviewed private-root scope owner"))
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
        assert_eq!(ensure_contract_scope(&mut tx, &address).is_ok(), allowed);
        assert_eq!(
            ensure_committed_contract_scope(&tx.world, &address).is_ok(),
            allowed
        );
    }
    let bytecode = IvmBytecode::from_compiled(vec![1]);
    assert!(ensure_executable_scope(&mut tx, &Executable::Ivm(bytecode.clone())).is_err());
    assert!(
        ensure_executable_scope(
            &mut tx,
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
    assert!(ensure_contract_scope(&mut tx, &address).is_err());
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
        assert!(execution_root_scope(&mut tx).is_err());
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
    assert!(captured_artifact_id(&mut tx, hash).is_err());
    bind_scope(&mut tx, ds);
    let artifact = captured_artifact_id(&mut tx, hash).unwrap();
    assert_eq!(artifact.dataspace_id, ds);
    assert_eq!(artifact.code_hash, hash);
    tx.world.current_dataspace_id = Some(DataSpaceId::new(17));
    assert!(captured_artifact_id(&mut tx, hash).is_err());
    bind_scope(&mut tx, DataSpaceId::new(17));
    assert!(captured_artifact_id(&mut tx, hash).is_err());
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

#[test]
fn malformed_root_metadata_is_terminal_and_cannot_acquire_retry_authority() {
    use iroha_data_model::parameter::{
        Parameter, custom::CustomParameter, system::consensus_metadata::handshake_meta_id,
    };
    for payload in ["null", "{}", "false", "\"global\""] {
        let (state, ds) = private_state();
        let mut block = state.block(header(2));
        let mut tx = block.transaction();
        bind_scope(&mut tx, ds);
        tx.world
            .parameters
            .set_parameter(Parameter::Custom(CustomParameter::new(
                handshake_meta_id(),
                payload.parse::<iroha_primitives::json::Json>().unwrap(),
            )));
        assert!(matches!(
            execution_root_scope(&mut tx),
            Err(ValidationFail::NotPermitted(reason))
                if reason == "instruction execution requires immutable root scope"
        ));
        assert!(tx.require_storage_admission().is_ok());
        assert!(tx.execution_deferral().is_none());
    }
}

#[test]
fn readonly_manifest_query_preserves_malformed_and_foreign_rejections() {
    use crate::execution_attempt::ExecutionAttemptError;
    use crate::smartcontracts::ValidSingularQuery as _;
    use iroha_data_model::{
        parameter::{
            Parameter, custom::CustomParameter, system::consensus_metadata::handshake_meta_id,
        },
        query::{error::QueryExecutionFail, smart_contract::FindContractManifestByArtifactId},
        smart_contract::ContractArtifactId,
    };
    let (state, own) = private_state();
    let mut block = state.block(header(2));
    let mut tx = block.transaction();
    bind_scope(&mut tx, own);
    let hash = Hash::new(b"scope query negative control");
    let local = FindContractManifestByArtifactId::new(ContractArtifactId::new(own, hash));
    let foreign = FindContractManifestByArtifactId::new(ContractArtifactId::new(
        DataSpaceId::UNIVERSAL,
        hash,
    ));
    assert!(matches!(
        foreign.execute(&tx),
        Err(ExecutionAttemptError::Rejected(QueryExecutionFail::Conversion(message)))
            if message.contains("private root cannot access a foreign artifact dataspace")
    ));
    for payload in ["null", "{}", "false", "\"global\""] {
        tx.world
            .parameters
            .set_parameter(Parameter::Custom(CustomParameter::new(
                handshake_meta_id(),
                payload.parse::<iroha_primitives::json::Json>().unwrap(),
            )));
        assert!(matches!(
            local.execute(&tx),
            Err(ExecutionAttemptError::Rejected(QueryExecutionFail::Conversion(message)))
                if message.contains("artifact access requires immutable root scope")
        ));
        assert!(tx.require_storage_admission().is_ok());
        assert!(tx.execution_deferral().is_none());
    }
}

#[test]
fn native_global_read_permission_borrows_exact_direct_and_role_grants_under_refusal() {
    use iroha_data_model::{
        permission::Permission,
        role::{Role, RoleId},
    };
    use std::collections::{BTreeMap, BTreeSet};
    let (state, _) = private_state();
    let mut block = state.block(header(2));
    let mut tx = block.transaction();
    let role_id: RoleId = "borrowed_global_reader".parse().unwrap();
    for through_role in [false, true] {
        for payload in ["null", "\"null\"", "false", "{}", "{\"null\":null}"] {
            let permission = Permission::new(
                "CanReadAllLedgerData".to_owned(),
                payload.parse::<iroha_primitives::json::Json>().unwrap(),
            );
            tx.world
                .account_permissions
                .insert(ALICE_ID.clone(), BTreeSet::new());
            tx.world.roles.insert(
                role_id.clone(),
                Role {
                    id: role_id.clone(),
                    permissions: if through_role {
                        BTreeSet::from([permission.clone()])
                    } else {
                        BTreeSet::new()
                    },
                    permission_epochs: BTreeMap::new(),
                },
            );
            if through_role {
                tx.world.account_roles.insert(
                    crate::role::RoleIdWithOwner::new(ALICE_ID.clone(), role_id.clone()),
                    (),
                );
            } else {
                tx.world
                    .account_permissions
                    .insert(ALICE_ID.clone(), BTreeSet::from([permission]));
            }
            let allowed = norito::with_decode_limits_scope(
                norito::DecodeLimits::new(96, usize::MAX, usize::MAX, 0, 32),
                || super::super::authority_has_native_global_read_permission(&tx.world, &ALICE_ID),
            )
            .unwrap();
            assert_eq!(allowed, payload == "null");
        }
    }
}

#[test]
fn original_private_instruction_routing_refusal_latches_before_scope_verdict() {
    use crate::state::WorldReadOnly as _;
    use iroha_data_model::{
        account::AccountAddress,
        domain::Domain,
        isi::Register,
        sns::{NameControllerV1, NameRecordV1},
    };
    use iroha_model_base::metadata::Metadata;
    use mv::storage::StorageReadOnly as _;
    use norito::codec::Encode as _;

    let selector = crate::sns::selector_for_dataspace_alias("alpha").unwrap();
    let address = AccountAddress::from_account_id(&ALICE_ID).unwrap();
    let record = NameRecordV1::new(
        selector.clone(),
        ALICE_ID.clone(),
        vec![NameControllerV1::account(&address)],
        0,
        0,
        u64::MAX,
        u64::MAX,
        u64::MAX,
        Metadata::default(),
    );
    let key = crate::sns::record_storage_key(&selector);
    let original = record.encode();
    let mut world = World::with([], [Account::new(ALICE_ID.clone()).build(&ALICE_ID)], []);
    world
        .smart_contract_state_mut_for_testing()
        .insert(key.clone(), original.clone());
    let state = State::new_for_testing(
        world,
        crate::kura::Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    let ds = crate::sns::dataspace_id_for_sns_alias("alpha").unwrap();
    let instruction: InstructionBox = Register::domain(Domain::new(
        iroha_model_base::domain::DomainId::try_new("bank", "alpha").unwrap(),
    ))
    .into();
    let mut block = state.block(header(2));
    {
        let mut tx = block.transaction();
        bind_scope(&mut tx, ds);
        assert!(ensure_private_instruction(&instruction, &mut tx, ds, 0).is_ok());
        assert!(tx.execution_deferral().is_none());
        norito::with_decode_limits_scope(
            norito::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, 0, 32),
            || assert!(ensure_private_instruction(&instruction, &mut tx, ds, 0).is_err()),
        );
        assert_eq!(
            tx.execution_deferral().unwrap().reason(),
            ivm::error::ExecutionDeferral::ActiveMemoryCapacity
        );
        assert_eq!(tx.world.smart_contract_state().get(&key), Some(&original));
    }
    let mut retry = block.transaction();
    bind_scope(&mut retry, ds);
    assert!(ensure_private_instruction(&instruction, &mut retry, ds, 0).is_ok());
    assert!(retry.execution_deferral().is_none());
}

fn seed_entrypoint_scope_contract(
    tx: &mut StateTransaction<'_, '_>,
    dataspace: DataSpaceId,
    nonce: u64,
) -> ContractAddress {
    use iroha_data_model::IntoKeyValue as _;
    let address = ContractAddress::derive(&tx.network_id, &ALICE_ID, nonce, dataspace).unwrap();
    let subject = address.subject_id();
    let (_, account) = Account::new(subject.clone())
        .build(&ALICE_ID)
        .into_key_value();
    tx.world.accounts.insert(subject.clone(), account);
    let hash = Hash::new(b"entrypoint scope contract");
    tx.world.contract_instances.insert(address.clone(), hash);
    tx.world
        .contract_subject_addresses
        .insert(subject, address.clone());
    tx.world.contract_subject_bindings.insert(
        address.clone(),
        crate::smartcontracts::code::ContractSubjectBinding::new_direct(&address, ALICE_ID.clone())
            .with_active_code_hash(hash),
    );
    address
}

fn exact_entrypoint_scope_permission(
    contract: &ContractAddress,
    entrypoint: &str,
) -> iroha_data_model::permission::Permission {
    iroha_executor_data_model::permission::smart_contract::CanInvokeContractEntrypoint {
        contract: contract.clone(),
        entrypoint: entrypoint.to_owned(),
    }
    .into()
}

#[test]
fn exact_private_entrypoint_grants_and_revokes_execute_owned_borrowed_and_native() {
    use crate::state::WorldReadOnly as _;
    use iroha_data_model::isi::{Grant, Revoke};
    for path in 0..3 {
        let (state, ds) = private_state();
        let mut block = state.block(header(2));
        let mut tx = block.transaction();
        bind_scope(&mut tx, ds);
        let address = seed_entrypoint_scope_contract(&mut tx, ds, 0);
        for selector in ["hajimari", "run"] {
            let permission = exact_entrypoint_scope_permission(&address, selector);
            let grant: InstructionBox =
                Grant::account_permission(permission.clone(), ALICE_ID.clone()).into();
            let revoke: InstructionBox =
                Revoke::account_permission(permission.clone(), ALICE_ID.clone()).into();
            for (instruction, present) in [(grant, true), (revoke, false)] {
                match path {
                    0 => Executor::Initial.execute_instruction(&mut tx, &ALICE_ID, instruction),
                    1 => Executor::Initial.execute_borrowed_overlay_instruction(
                        &mut tx,
                        &ALICE_ID,
                        &instruction,
                        None,
                    ),
                    _ => instruction.execute(&ALICE_ID, &mut tx).map_err(Into::into),
                }
                .expect("exact lifecycle owner mutates only its private contract token");
                assert_eq!(
                    tx.world
                        .account_contains_inherent_permission(&ALICE_ID, &permission),
                    present
                );
            }
        }
    }
}

#[test]
fn private_entrypoint_scope_does_not_replace_actual_lifecycle_owner_authorization() {
    use iroha_data_model::{IntoKeyValue as _, isi::Grant};
    use iroha_test_samples::BOB_ID;
    let (state, ds) = private_state();
    let mut block = state.block(header(2));
    let mut tx = block.transaction();
    bind_scope(&mut tx, ds);
    let (_, bob) = Account::new(BOB_ID.clone())
        .build(&ALICE_ID)
        .into_key_value();
    tx.world.accounts.insert(BOB_ID.clone(), bob);
    let address = seed_entrypoint_scope_contract(&mut tx, ds, 0);
    let grant: InstructionBox = Grant::account_permission(
        exact_entrypoint_scope_permission(&address, "run"),
        BOB_ID.clone(),
    )
    .into();
    ensure_instruction_scope(&grant, &mut tx).expect("scope alone conveys no delegation authority");
    for borrowed in [false, true] {
        let error = if borrowed {
            Executor::Initial.execute_borrowed_overlay_instruction(&mut tx, &BOB_ID, &grant, None)
        } else {
            Executor::Initial.execute_instruction(&mut tx, &BOB_ID, grant.clone())
        }
        .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("current account lifecycle owner")
        );
    }
}

#[test]
fn exact_private_entrypoint_permissions_reject_foreign_contract_and_subject_scopes() {
    use crate::state::WorldReadOnly as _;
    use iroha_data_model::isi::{Grant, Revoke};
    let (state, ds) = private_state();
    let mut block = state.block(header(2));
    let mut tx = block.transaction();
    bind_scope(&mut tx, ds);
    let local = seed_entrypoint_scope_contract(&mut tx, ds, 0);
    let foreign = seed_entrypoint_scope_contract(&mut tx, DataSpaceId::new(17), 1);
    for (contract, destination) in [(&foreign, ALICE_ID.clone()), (&local, foreign.subject_id())] {
        let permission = exact_entrypoint_scope_permission(contract, "run");
        let instructions: [InstructionBox; 2] = [
            Grant::account_permission(permission.clone(), destination.clone()).into(),
            Revoke::account_permission(permission.clone(), destination.clone()).into(),
        ];
        for instruction in instructions {
            assert!(matches!(
                ensure_instruction_scope(&instruction, &mut tx),
                Err(ValidationFail::NotPermitted(reason)) if reason.contains("foreign")
            ));
            assert!(
                Executor::Initial
                    .execute_instruction(&mut tx, &ALICE_ID, instruction.clone())
                    .is_err()
            );
            assert!(
                Executor::Initial
                    .execute_borrowed_overlay_instruction(&mut tx, &ALICE_ID, &instruction, None)
                    .is_err()
            );
            assert!(instruction.execute(&ALICE_ID, &mut tx).is_err());
            assert!(
                !tx.world
                    .account_contains_inherent_permission(&destination, &permission)
            );
        }
    }
}

#[test]
fn private_entrypoint_scope_rejects_foreign_account_directory_and_broken_subject_index() {
    use crate::nexus::space_directory::AccountScopeDirectoryEntry;
    use iroha_data_model::isi::Grant;
    let (state, ds) = private_state();
    let mut block = state.block(header(2));
    let mut tx = block.transaction();
    bind_scope(&mut tx, ds);
    let address = seed_entrypoint_scope_contract(&mut tx, ds, 0);
    let grant: InstructionBox = Grant::account_permission(
        exact_entrypoint_scope_permission(&address, "run"),
        ALICE_ID.clone(),
    )
    .into();
    let mut foreign = AccountScopeDirectoryEntry::default();
    foreign.ensure_dataspace(DataSpaceId::new(17));
    tx.world
        .account_scope_directory
        .insert(ALICE_ID.clone(), foreign);
    assert!(ensure_instruction_scope(&grant, &mut tx).is_err());
    tx.world.account_scope_directory.remove(ALICE_ID.clone());
    tx.world
        .contract_subject_addresses
        .remove(address.subject_id());
    assert!(matches!(
        ensure_instruction_scope(&grant, &mut tx),
        Err(ValidationFail::NotPermitted(reason)) if reason.contains("original contract subject index")
    ));
}

#[test]
fn private_entrypoint_scope_refuses_malformed_unknown_unbound_and_global_permissions() {
    use iroha_data_model::{isi::Grant, permission::Permission};
    use iroha_primitives::json::Json;
    let (state, ds) = private_state();
    let mut block = state.block(header(2));
    let mut tx = block.transaction();
    bind_scope(&mut tx, ds);
    let address = seed_entrypoint_scope_contract(&mut tx, ds, 0);
    let valid = exact_entrypoint_scope_permission(&address, "run");
    let extra = format!(
        "{},\"unexpected\":true}}",
        valid.payload().get().strip_suffix('}').unwrap()
    );
    let unbound = ContractAddress::derive(&tx.network_id, &ALICE_ID, 9, ds).unwrap();
    let permissions = [
        Permission::new("CanInvokeContractEntrypoint".into(), Json::new(())),
        Permission::new(
            "CanInvokeContractEntrypoint".into(),
            Json::from_raw_json(extra).unwrap(),
        ),
        exact_entrypoint_scope_permission(&address, ""),
        exact_entrypoint_scope_permission(&address, " run"),
        exact_entrypoint_scope_permission(&unbound, "run"),
        iroha_executor_data_model::permission::smart_contract::CanManageSmartContractCode.into(),
        Permission::new("UnknownPrivatePermission".into(), Json::new(())),
    ];
    for permission in permissions {
        let instruction = Grant::account_permission(permission, ALICE_ID.clone()).into();
        assert!(ensure_instruction_scope(&instruction, &mut tx).is_err());
        assert!(
            tx.execution_deferral().is_none(),
            "completed malformed scope is a terminal refusal"
        );
    }
}

#[test]
fn nested_private_entrypoint_tokens_keep_their_exact_contract_scope() {
    use iroha_data_model::isi::Grant;
    let (state, ds) = private_state();
    let mut block = state.block(header(2));
    let mut tx = block.transaction();
    bind_scope(&mut tx, ds);
    let local = seed_entrypoint_scope_contract(&mut tx, ds, 0);
    let foreign = seed_entrypoint_scope_contract(&mut tx, DataSpaceId::new(17), 1);
    for (contract, allowed) in [(&local, true), (&foreign, false)] {
        let instruction = MultisigInstructionBox::Propose(
            iroha_executor_data_model::isi::multisig::MultisigPropose {
                account: ALICE_ID.clone(),
                instructions: vec![
                    Grant::account_permission(
                        exact_entrypoint_scope_permission(contract, "run"),
                        ALICE_ID.clone(),
                    )
                    .into(),
                ],
                transaction_ttl_ms: None,
            },
        )
        .into();
        assert_eq!(
            ensure_instruction_scope(&instruction, &mut tx).is_ok(),
            allowed
        );
    }
}

#[test]
fn private_entrypoint_payload_capacity_refusal_retains_original_retry_authority() {
    use crate::state::WorldReadOnly as _;
    use iroha_data_model::isi::Grant;
    let (state, ds) = private_state();
    let mut block = state.block(header(2));
    let mut tx = block.transaction();
    bind_scope(&mut tx, ds);
    let address = seed_entrypoint_scope_contract(&mut tx, ds, 0);
    let permission = exact_entrypoint_scope_permission(&address, "run");
    let instruction = Grant::account_permission(permission.clone(), ALICE_ID.clone()).into();
    assert!(ensure_private_entrypoint_permission_scope(&instruction, &mut tx, ds).unwrap());
    norito::with_decode_limits_scope(
        norito::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, 0, 32),
        || assert!(ensure_private_entrypoint_permission_scope(&instruction, &mut tx, ds).is_err()),
    );
    assert_eq!(
        tx.execution_deferral().unwrap().reason(),
        ivm::error::ExecutionDeferral::ActiveMemoryCapacity
    );
    assert!(
        !tx.world
            .account_contains_inherent_permission(&ALICE_ID, &permission)
    );
}

fn private_holding_limit_state() -> (
    State,
    DataSpaceId,
    iroha_data_model::asset::AssetDefinitionId,
) {
    use iroha_data_model::{
        asset::{AssetBalancePolicy, AssetDefinition, AssetDefinitionId},
        domain::Domain,
    };
    use iroha_model_base::domain::DomainId;
    use iroha_test_samples::BOB_ID;
    let domain = DomainId::try_new("bank", "private-scope-test").unwrap();
    let asset = AssetDefinitionId::derive_from_components(domain.clone(), "gas".parse().unwrap());
    let definition = AssetDefinition::numeric(
        asset.clone(),
        "Private scope gas",
        AssetBalancePolicy::DataspaceRestricted,
        Some(domain.clone()),
    )
    .build(&ALICE_ID);
    let world = World::with(
        [Domain::new(domain).build(&ALICE_ID)],
        [
            Account::new(ALICE_ID.clone()).build(&ALICE_ID),
            Account::new(BOB_ID.clone()).build(&ALICE_ID),
        ],
        [definition],
    );
    let (state, ds) = private_state_with_world(world);
    (state, ds, asset)
}

fn holding_limit_instruction(
    asset: &iroha_data_model::asset::AssetDefinitionId,
    limit: Option<u32>,
) -> InstructionBox {
    iroha_data_model::isi::SetAssetHoldingLimit::new(
        iroha_test_samples::BOB_ID.clone(),
        asset.clone(),
        limit.map(Into::into),
    )
    .into()
}

fn execute_holding_limit_boundary(
    path: u8,
    instruction: &InstructionBox,
    authority: &iroha_data_model::account::AccountId,
    tx: &mut StateTransaction<'_, '_>,
) -> Result<(), ValidationFail> {
    match path {
        0 => Executor::Initial.execute_instruction(tx, authority, instruction.clone()),
        1 => {
            Executor::Initial.execute_borrowed_overlay_instruction(tx, authority, instruction, None)
        }
        _ => instruction
            .clone()
            .execute(authority, tx)
            .map_err(Into::into),
    }
}

fn current_holding_limit(
    tx: &StateTransaction<'_, '_>,
    asset: &iroha_data_model::asset::AssetDefinitionId,
) -> Option<iroha_primitives::numeric::Quantity> {
    use crate::state::WorldReadOnly as _;
    let account = tx.world.account(&iroha_test_samples::BOB_ID).unwrap();
    crate::smartcontracts::isi::asset::isi::load_asset_transfer_control_store_from_account(
        &iroha_test_samples::BOB_ID,
        account.metadata(),
    )
    .unwrap()
    .find(asset)
    .and_then(|record| record.holding_limit.clone())
}

#[test]
fn private_holding_limits_execute_all_native_boundaries_and_persist_exact_caps() {
    for path in 0..3 {
        let (state, ds, asset) = private_holding_limit_state();
        let mut block = state.block(header(2));
        let mut tx = block.transaction();
        bind_scope(&mut tx, ds);
        // These original unlabeled accounts are retained by the authenticated private World.
        // They acquire no new label, caller-routing fallback or compatibility exception here.
        for limit in [Some(0), Some(9), None] {
            let instruction = holding_limit_instruction(&asset, limit);
            execute_holding_limit_boundary(path, &instruction, &ALICE_ID, &mut tx)
                .expect("actual asset owner changes only its private account-and-asset policy");
            assert_eq!(current_holding_limit(&tx, &asset), limit.map(Into::into));
        }
    }
}

#[test]
fn private_holding_limit_scope_preserves_asset_owner_and_exact_permission_authorization() {
    use iroha_executor_data_model::permission::asset::CanSetAssetHoldingLimit;
    use iroha_test_samples::BOB_ID;
    for path in 0..3 {
        let (state, ds, asset) = private_holding_limit_state();
        let mut block = state.block(header(2));
        let mut tx = block.transaction();
        bind_scope(&mut tx, ds);
        let instruction = holding_limit_instruction(&asset, Some(0));
        ensure_instruction_scope(&instruction, &mut tx).unwrap();
        for permission in [
            None,
            Some(
                CanSetAssetHoldingLimit {
                    account: ALICE_ID.clone(),
                    asset_definition: asset.clone(),
                }
                .into(),
            ),
        ] {
            if let Some(permission) = permission {
                tx.world.add_account_permission(&BOB_ID, permission);
            }
            let error =
                execute_holding_limit_boundary(path, &instruction, &BOB_ID, &mut tx).unwrap_err();
            assert!(
                matches!(
                    &error,
                    ValidationFail::InstructionFailed(
                        iroha_data_model::isi::error::InstructionExecutionError::InvariantViolation(
                            reason
                        )
                    ) if reason.contains("exact account-and-asset")
                ),
                "{error:?}"
            );
            assert_eq!(current_holding_limit(&tx, &asset), None);
        }
        tx.world.add_account_permission(
            &BOB_ID,
            CanSetAssetHoldingLimit {
                account: BOB_ID.clone(),
                asset_definition: asset.clone(),
            }
            .into(),
        );
        execute_holding_limit_boundary(path, &instruction, &BOB_ID, &mut tx)
            .expect("unchanged native handler recognizes only the exact target permission");
        assert_eq!(current_holding_limit(&tx, &asset), Some(0_u32.into()));
    }
}

#[test]
fn private_holding_limits_reject_foreign_global_missing_and_corrupt_resources() {
    use crate::nexus::space_directory::AccountScopeDirectoryEntry;
    use iroha_data_model::{
        asset::{AssetBalancePolicy, AssetDefinition},
        domain::Domain,
    };
    use iroha_model_base::domain::DomainId;
    use iroha_test_samples::BOB_ID;
    for case in 0..8 {
        let (state, ds, asset) = private_holding_limit_state();
        let mut block = state.block(header(2));
        let mut tx = block.transaction();
        bind_scope(&mut tx, ds);
        let mut instruction = holding_limit_instruction(&asset, Some(0));
        match case {
            0 | 1 => {
                let mut foreign = AccountScopeDirectoryEntry::default();
                foreign.ensure_dataspace(DataSpaceId::new(17));
                tx.world.account_scope_directory.insert(
                    if case == 0 {
                        BOB_ID.clone()
                    } else {
                        ALICE_ID.clone()
                    },
                    foreign,
                );
            }
            2 => {
                tx.world.asset_definitions.insert(
                    asset.clone(),
                    AssetDefinition::numeric(
                        asset.clone(),
                        "Global gas",
                        AssetBalancePolicy::Global,
                        None,
                    )
                    .build(&ALICE_ID),
                );
            }
            3 => {
                tx.world.asset_definitions.remove(asset.clone());
            }
            4 => {
                tx.world.asset_definition_domains.remove(asset.clone());
            }
            5 => {
                let domain = DomainId::try_new("bank", "universal").unwrap();
                tx.world
                    .domains
                    .insert(domain.clone(), Domain::new(domain.clone()).build(&ALICE_ID));
                tx.world
                    .asset_definition_domains
                    .insert(asset.clone(), domain.clone());
                tx.world.asset_definitions.insert(
                    asset.clone(),
                    AssetDefinition::numeric(
                        asset.clone(),
                        "Foreign gas",
                        AssetBalancePolicy::DataspaceRestricted,
                        Some(domain),
                    )
                    .build(&ALICE_ID),
                );
            }
            6 => {
                let subject =
                    seed_entrypoint_scope_contract(&mut tx, DataSpaceId::new(17), 8).subject_id();
                instruction = iroha_data_model::isi::SetAssetHoldingLimit::new(
                    subject,
                    asset.clone(),
                    Some(0_u32.into()),
                )
                .into();
            }
            _ => {
                tx.world.accounts.remove(BOB_ID.clone());
            }
        }
        assert!(
            ensure_instruction_scope(&instruction, &mut tx).is_err(),
            "case {case}"
        );
        for path in 0..3 {
            assert!(
                execute_holding_limit_boundary(path, &instruction, &ALICE_ID, &mut tx).is_err(),
                "case {case}, path {path}"
            );
        }
        assert!(
            tx.execution_deferral().is_none(),
            "completed invalid binding remains terminal"
        );
        if case != 7 {
            assert_eq!(current_holding_limit(&tx, &asset), None);
        }
    }
}

#[test]
fn private_holding_limits_require_retained_alias_and_domain_indexes() {
    use iroha_data_model::asset::AssetDefinitionAlias;
    use iroha_model_base::domain::DomainId;
    for case in 0..7 {
        let (state, ds, asset) = private_holding_limit_state();
        let mut block = state.block(header(2));
        let mut tx = block.transaction();
        bind_scope(&mut tx, ds);
        let alias: AssetDefinitionAlias = "gas#private-scope-test".parse().unwrap();
        tx.world
            .asset_definition_aliases
            .insert(alias.clone(), asset.clone());
        tx.world.asset_definition_alias_bindings.insert(
            asset.clone(),
            crate::state::AssetDefinitionAliasBindingRecord {
                alias: alias.clone(),
                lease_expiry_ms: Some(2_000),
                grace_until_ms: None,
                bound_at_ms: 0,
            },
        );
        match case {
            0 => {}
            1 => {
                tx.world.asset_definition_aliases.remove(alias.clone());
            }
            2 => {
                tx.world
                    .asset_definition_alias_bindings
                    .get_mut(&asset)
                    .unwrap()
                    .lease_expiry_ms = Some(999);
            }
            3 => {
                let foreign: AssetDefinitionAlias = "gas#universal".parse().unwrap();
                tx.world.asset_definition_aliases.remove(alias.clone());
                tx.world
                    .asset_definition_aliases
                    .insert(foreign.clone(), asset.clone());
                tx.world
                    .asset_definition_alias_bindings
                    .get_mut(&asset)
                    .unwrap()
                    .alias = foreign;
            }
            4 => {
                tx.world.asset_definition_domains.insert(
                    asset.clone(),
                    DomainId::try_new("bank", "universal").unwrap(),
                );
            }
            5 => {
                tx.world
                    .asset_definition_alias_bindings
                    .remove(asset.clone());
                tx.world.asset_definitions.get_mut(&asset).unwrap().alias = Some(alias.clone());
            }
            _ => {
                tx.world.asset_definitions.get_mut(&asset).unwrap().alias =
                    Some("other#private-scope-test".parse().unwrap());
            }
        }
        let instruction = holding_limit_instruction(&asset, Some(0));
        if case == 0 {
            execute_holding_limit_boundary(0, &instruction, &ALICE_ID, &mut tx).unwrap();
            assert_eq!(current_holding_limit(&tx, &asset), Some(0_u32.into()));
        } else {
            for path in 0..3 {
                assert!(
                    execute_holding_limit_boundary(path, &instruction, &ALICE_ID, &mut tx).is_err(),
                    "case {case}, path {path}"
                );
            }
            assert_eq!(current_holding_limit(&tx, &asset), None);
            assert!(tx.execution_deferral().is_none());
        }
    }
}

#[test]
fn nested_private_holding_limits_keep_exact_resource_scope() {
    use crate::nexus::space_directory::AccountScopeDirectoryEntry;
    use iroha_test_samples::BOB_ID;
    let (state, ds, asset) = private_holding_limit_state();
    let mut block = state.block(header(2));
    let mut tx = block.transaction();
    bind_scope(&mut tx, ds);
    let instruction: InstructionBox = MultisigInstructionBox::Propose(
        iroha_executor_data_model::isi::multisig::MultisigPropose {
            account: ALICE_ID.clone(),
            instructions: vec![holding_limit_instruction(&asset, Some(0))],
            transaction_ttl_ms: None,
        },
    )
    .into();
    ensure_instruction_scope(&instruction, &mut tx).unwrap();
    let mut foreign = AccountScopeDirectoryEntry::default();
    foreign.ensure_dataspace(DataSpaceId::new(17));
    tx.world
        .account_scope_directory
        .insert(BOB_ID.clone(), foreign);
    assert!(ensure_instruction_scope(&instruction, &mut tx).is_err());
    assert_eq!(current_holding_limit(&tx, &asset), None);
}

fn seed_holding_limit_dataspace_record(
    tx: &mut StateTransaction<'_, '_>,
    ds: DataSpaceId,
) -> (iroha_model_base::state_path::StatePath, Vec<u8>) {
    use iroha_data_model::{
        account::AccountAddress,
        sns::{NameControllerV1, NameRecordV1},
    };
    use iroha_model_base::metadata::Metadata;
    use iroha_primitives::json::Json;
    use norito::codec::Encode as _;
    let selector = crate::sns::selector_for_dataspace_alias("private-scope-test").unwrap();
    let address = AccountAddress::from_account_id(&ALICE_ID).unwrap();
    let mut metadata = Metadata::default();
    metadata.insert(
        crate::sns::SNS_DATASPACE_ID_METADATA_KEY.parse().unwrap(),
        Json::new(ds.as_u64()),
    );
    let record = NameRecordV1::new(
        selector.clone(),
        ALICE_ID.clone(),
        vec![NameControllerV1::account(&address)],
        0,
        0,
        u64::MAX,
        u64::MAX,
        u64::MAX,
        metadata,
    );
    let key = crate::sns::record_storage_key(&selector);
    let original = record.encode();
    tx.world
        .smart_contract_state
        .insert(key.clone(), original.clone());
    (key, original)
}

#[test]
fn private_holding_limit_sns_decode_refusal_retains_retry_and_original_state() {
    let (state, ds, asset) = private_holding_limit_state();
    let mut block = state.block(header(2));
    let instruction = holding_limit_instruction(&asset, Some(0));
    {
        let mut tx = block.transaction();
        bind_scope(&mut tx, ds);
        let (key, original) = seed_holding_limit_dataspace_record(&mut tx, ds);
        ensure_instruction_scope(&instruction, &mut tx).unwrap();
        norito::with_decode_limits_scope(
            norito::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, 0, 32),
            || assert!(ensure_private_instruction(&instruction, &mut tx, ds, 0).is_err()),
        );
        assert_eq!(
            tx.execution_deferral().unwrap().reason(),
            ivm::error::ExecutionDeferral::ActiveMemoryCapacity
        );
        assert_eq!(tx.world.smart_contract_state.get(&key), Some(&original));
        assert_eq!(current_holding_limit(&tx, &asset), None);
    }
    let mut retry = block.transaction();
    bind_scope(&mut retry, ds);
    seed_holding_limit_dataspace_record(&mut retry, ds);
    execute_holding_limit_boundary(0, &instruction, &ALICE_ID, &mut retry).unwrap();
    assert!(retry.execution_deferral().is_none());
    assert_eq!(current_holding_limit(&retry, &asset), Some(0_u32.into()));
}

#[test]
fn private_holding_limit_malformed_sns_is_terminal_and_preserves_original_state() {
    let (state, ds, asset) = private_holding_limit_state();
    let mut block = state.block(header(2));
    let mut tx = block.transaction();
    bind_scope(&mut tx, ds);
    let (key, original) = seed_holding_limit_dataspace_record(&mut tx, ds);
    let mut malformed = original;
    malformed.push(0);
    tx.world
        .smart_contract_state
        .insert(key.clone(), malformed.clone());
    let instruction = holding_limit_instruction(&asset, Some(0));
    for path in 0..3 {
        assert!(execute_holding_limit_boundary(path, &instruction, &ALICE_ID, &mut tx).is_err());
    }
    assert!(tx.execution_deferral().is_none());
    assert!(tx.require_storage_admission().is_ok());
    assert_eq!(tx.world.smart_contract_state.get(&key), Some(&malformed));
    assert_eq!(current_holding_limit(&tx, &asset), None);
}

fn private_account_metadata_value(
    tx: &StateTransaction<'_, '_>,
    account: &iroha_data_model::account::AccountId,
    key: &iroha_model_base::name::Name,
) -> Option<iroha_primitives::json::Json> {
    use crate::state::WorldReadOnly as _;
    tx.world.account(account).ok()?.metadata().get(key).cloned()
}

#[test]
fn private_account_metadata_set_and_remove_execute_all_native_boundaries() {
    use iroha_data_model::{
        isi::{RemoveKeyValue, SetKeyValue},
        private_transaction_counters::{
            PRIVATE_COUNTER_MANIFEST_METADATA_KEY_V1, PRIVATE_COUNTER_POLICY_METADATA_KEY_V1,
        },
    };
    use iroha_primitives::json::Json;
    for path in 0..3 {
        let (state, ds) = private_state();
        let mut block = state.block(header(2));
        let mut tx = block.transaction();
        bind_scope(&mut tx, ds);
        // These actual, unlabelled accounts are retained by the authenticated private World.
        // No caller route or account-label fallback is introduced by metadata scope admission.
        for text in [
            PRIVATE_COUNTER_POLICY_METADATA_KEY_V1,
            PRIVATE_COUNTER_MANIFEST_METADATA_KEY_V1,
        ] {
            let key: iroha_model_base::name::Name = text.parse().unwrap();
            let original = Json::new("original");
            let replacement = Json::new("replacement");
            for value in [original, replacement.clone()] {
                let set: InstructionBox =
                    SetKeyValue::account(ALICE_ID.clone(), key.clone(), value.clone()).into();
                execute_holding_limit_boundary(path, &set, &ALICE_ID, &mut tx)
                    .expect("actual metadata owner writes its existing private account");
                assert_eq!(
                    private_account_metadata_value(&tx, &ALICE_ID, &key),
                    Some(value)
                );
            }
            let remove: InstructionBox =
                RemoveKeyValue::account(ALICE_ID.clone(), key.clone()).into();
            execute_holding_limit_boundary(path, &remove, &ALICE_ID, &mut tx)
                .expect("actual metadata owner removes only its existing account key");
            assert_eq!(private_account_metadata_value(&tx, &ALICE_ID, &key), None);
        }
    }
}

#[test]
fn private_account_metadata_requires_native_owner_or_exact_permission() {
    use iroha_data_model::isi::{RemoveKeyValue, SetKeyValue};
    use iroha_executor_data_model::permission::account::CanModifyAccountMetadata;
    use iroha_primitives::json::Json;
    use iroha_test_samples::BOB_ID;
    // The internal boxed mutation boundary does not authorize delegation; these two real
    // Initial executor paths exercise the unchanged owner/exact-permission authorizer.
    for path in 0..2 {
        let world = World::with(
            [],
            [
                Account::new(ALICE_ID.clone()).build(&ALICE_ID),
                Account::new(BOB_ID.clone()).build(&ALICE_ID),
            ],
            [],
        );
        let (state, ds) = private_state_with_world(world);
        let mut block = state.block(header(2));
        let mut tx = block.transaction();
        bind_scope(&mut tx, ds);
        let key: iroha_model_base::name::Name = "account_scope_control".parse().unwrap();
        let original = Json::new("original");
        tx.world
            .account_mut(&ALICE_ID)
            .unwrap()
            .insert(key.clone(), original.clone());
        let set: InstructionBox =
            SetKeyValue::account(ALICE_ID.clone(), key.clone(), Json::new("replacement")).into();
        let remove: InstructionBox = RemoveKeyValue::account(ALICE_ID.clone(), key.clone()).into();
        for wrong_permission in [false, true] {
            if wrong_permission {
                tx.world.add_account_permission(
                    &BOB_ID,
                    CanModifyAccountMetadata {
                        account: BOB_ID.clone(),
                    }
                    .into(),
                );
            }
            for instruction in [&set, &remove] {
                ensure_instruction_scope(instruction, &mut tx).unwrap();
                let error = execute_holding_limit_boundary(path, instruction, &BOB_ID, &mut tx)
                    .unwrap_err();
                assert!(
                    matches!(&error, ValidationFail::NotPermitted(_)),
                    "{error:?}"
                );
                assert_eq!(
                    private_account_metadata_value(&tx, &ALICE_ID, &key),
                    Some(original.clone())
                );
            }
        }
        tx.world.add_account_permission(
            &BOB_ID,
            CanModifyAccountMetadata {
                account: ALICE_ID.clone(),
            }
            .into(),
        );
        execute_holding_limit_boundary(path, &set, &BOB_ID, &mut tx)
            .expect("unchanged Initial authorizer requires the exact existing target account");
        assert_eq!(
            private_account_metadata_value(&tx, &ALICE_ID, &key),
            Some(Json::new("replacement"))
        );
        execute_holding_limit_boundary(path, &remove, &BOB_ID, &mut tx).unwrap();
        assert_eq!(private_account_metadata_value(&tx, &ALICE_ID, &key), None);
    }
}

#[test]
fn private_account_metadata_refuses_foreign_missing_and_corrupt_resources() {
    use crate::nexus::space_directory::AccountScopeDirectoryEntry;
    use iroha_data_model::isi::{RemoveKeyValue, SetKeyValue};
    use iroha_primitives::json::Json;
    use iroha_test_samples::BOB_ID;
    for case in 0..7 {
        for path in 0..3 {
            let (state, ds) = private_state();
            let mut block = state.block(header(2));
            let mut tx = block.transaction();
            bind_scope(&mut tx, ds);
            let target = match case {
                0 | 1 => {
                    let mut entry = AccountScopeDirectoryEntry::default();
                    if case == 1 {
                        entry.ensure_dataspace(ds);
                    }
                    entry.ensure_dataspace(DataSpaceId::new(17));
                    tx.world
                        .account_scope_directory
                        .insert(ALICE_ID.clone(), entry);
                    ALICE_ID.clone()
                }
                2 => BOB_ID.clone(),
                3 | 4 => seed_entrypoint_scope_contract(
                    &mut tx,
                    if case == 3 {
                        DataSpaceId::new(17)
                    } else {
                        DataSpaceId::UNIVERSAL
                    },
                    1,
                )
                .subject_id(),
                5 => {
                    let original = seed_entrypoint_scope_contract(&mut tx, ds, 1);
                    let substituted = seed_entrypoint_scope_contract(&mut tx, ds, 2);
                    let subject = original.subject_id();
                    tx.world
                        .contract_subject_addresses
                        .insert(subject.clone(), substituted);
                    subject
                }
                _ => {
                    tx.current_dataspace_id = Some(DataSpaceId::new(17));
                    ALICE_ID.clone()
                }
            };
            let key: iroha_model_base::name::Name = "account_scope_control".parse().unwrap();
            let original = Json::new("original");
            if case != 2 {
                tx.world
                    .account_mut(&target)
                    .unwrap()
                    .insert(key.clone(), original.clone());
            }
            for instruction in [
                InstructionBox::from(SetKeyValue::account(
                    target.clone(),
                    key.clone(),
                    Json::new("replacement"),
                )),
                InstructionBox::from(RemoveKeyValue::account(target.clone(), key.clone())),
            ] {
                let original_scope_error = ensure_instruction_scope(&instruction, &mut tx)
                    .expect_err("the actual retained account scope must refuse this mutation");
                assert!(matches!(
                    &original_scope_error,
                    ValidationFail::NotPermitted(_)
                ));
                let expected_error = if path == 2 {
                    ValidationFail::InstructionFailed(
                        iroha_data_model::isi::error::InstructionExecutionError::Conversion(
                            original_scope_error.to_string(),
                        ),
                    )
                } else {
                    original_scope_error
                };
                let error = execute_holding_limit_boundary(path, &instruction, &ALICE_ID, &mut tx)
                    .unwrap_err();
                assert_eq!(error, expected_error, "case={case} path={path}");
                assert_eq!(
                    private_account_metadata_value(&tx, &target, &key),
                    if case == 2 {
                        None
                    } else {
                        Some(original.clone())
                    }
                );
                assert!(tx.execution_deferral().is_none());
            }
        }
    }
}

#[test]
fn private_account_metadata_preserves_reserved_key_and_value_size_refusals() {
    use iroha_data_model::{
        isi::{RemoveKeyValue, SetKeyValue},
        parameter::{CustomParameter, CustomParameterId, Parameter},
        private_transaction_counters::PRIVATE_COUNTER_POLICY_METADATA_KEY_V1,
    };
    use iroha_primitives::json::Json;
    for path in 0..3 {
        let (state, ds) = private_state();
        let mut block = state.block(header(2));
        let mut tx = block.transaction();
        bind_scope(&mut tx, ds);
        for text in [
            iroha_data_model::smart_contract::CONTRACT_DEPLOY_NONCE_METADATA_KEY,
            iroha_data_model::asset::ASSET_TRANSFER_CONTROL_METADATA_KEY,
        ] {
            let key: iroha_model_base::name::Name = text.parse().unwrap();
            for instruction in [
                InstructionBox::from(SetKeyValue::account(
                    ALICE_ID.clone(),
                    key.clone(),
                    Json::new(0_u64),
                )),
                InstructionBox::from(RemoveKeyValue::account(ALICE_ID.clone(), key.clone())),
            ] {
                ensure_instruction_scope(&instruction, &mut tx).unwrap();
                let error = execute_holding_limit_boundary(path, &instruction, &ALICE_ID, &mut tx)
                    .unwrap_err();
                assert!(
                    matches!(&error, ValidationFail::InstructionFailed(
                    iroha_data_model::isi::error::InstructionExecutionError::InvariantViolation(reason)
                ) if reason.contains("reserved")),
                    "{error:?}"
                );
                assert_eq!(private_account_metadata_value(&tx, &ALICE_ID, &key), None);
            }
        }
        drop(tx);
        drop(block);
        let world = World::with([], [Account::new(ALICE_ID.clone()).build(&ALICE_ID)], []);
        let mut parameters = world.parameters.block();
        parameters.set_parameter(Parameter::Custom(CustomParameter::new(
            "max_metadata_value_bytes"
                .parse::<CustomParameterId>()
                .unwrap(),
            Json::new(16_u64),
        )));
        parameters.commit();
        let (state, ds) = private_state_with_world(world);
        let mut block = state.block(header(2));
        let mut tx = block.transaction();
        bind_scope(&mut tx, ds);
        let key: iroha_model_base::name::Name =
            PRIVATE_COUNTER_POLICY_METADATA_KEY_V1.parse().unwrap();
        let instruction: InstructionBox =
            SetKeyValue::account(ALICE_ID.clone(), key.clone(), Json::new("X".repeat(32))).into();
        ensure_instruction_scope(&instruction, &mut tx).unwrap();
        let error =
            execute_holding_limit_boundary(path, &instruction, &ALICE_ID, &mut tx).unwrap_err();
        assert!(
            matches!(
                &error,
                ValidationFail::InstructionFailed(
                    iroha_data_model::isi::error::InstructionExecutionError::InvalidParameter(_)
                )
            ),
            "{error:?}"
        );
        assert_eq!(private_account_metadata_value(&tx, &ALICE_ID, &key), None);
    }
}

#[test]
fn nested_private_account_metadata_keeps_exact_scope_and_unreviewed_families_closed() {
    use crate::nexus::space_directory::AccountScopeDirectoryEntry;
    use iroha_data_model::isi::{RemoveKeyValue, SetKeyValue};
    use iroha_primitives::json::Json;
    use iroha_test_samples::BOB_ID;
    let world = World::with(
        [],
        [
            Account::new(ALICE_ID.clone()).build(&ALICE_ID),
            Account::new(BOB_ID.clone()).build(&ALICE_ID),
        ],
        [],
    );
    let (state, ds) = private_state_with_world(world);
    let mut block = state.block(header(2));
    let mut tx = block.transaction();
    bind_scope(&mut tx, ds);
    let key: iroha_model_base::name::Name = "account_scope_control".parse().unwrap();
    let original = Json::new("original");
    tx.world
        .account_mut(&BOB_ID)
        .unwrap()
        .insert(key.clone(), original.clone());
    let nested = InstructionBox::from(MultisigInstructionBox::Propose(
        iroha_executor_data_model::isi::multisig::MultisigPropose {
            account: ALICE_ID.clone(),
            instructions: vec![
                SetKeyValue::account(BOB_ID.clone(), key.clone(), Json::new("replacement")).into(),
                RemoveKeyValue::account(BOB_ID.clone(), key.clone()).into(),
            ],
            transaction_ttl_ms: None,
        },
    ));
    ensure_instruction_scope(&nested, &mut tx).unwrap();
    assert_eq!(
        private_account_metadata_value(&tx, &BOB_ID, &key),
        Some(original.clone())
    );
    let mut foreign = AccountScopeDirectoryEntry::default();
    foreign.ensure_dataspace(DataSpaceId::new(17));
    tx.world
        .account_scope_directory
        .insert(BOB_ID.clone(), foreign);
    assert!(matches!(
        ensure_instruction_scope(&nested, &mut tx),
        Err(ValidationFail::NotPermitted(_))
    ));
    assert_eq!(
        private_account_metadata_value(&tx, &BOB_ID, &key),
        Some(original)
    );
    let domain = iroha_model_base::domain::DomainId::try_new("missing", "unknown-scope").unwrap();
    let asset = iroha_data_model::asset::AssetDefinitionId::derive_from_components(
        domain.clone(),
        "missing".parse().unwrap(),
    );
    let trigger: iroha_data_model::trigger::TriggerId = "missing_scope_trigger".parse().unwrap();
    for unreviewed in [
        InstructionBox::from(SetKeyValue::domain(
            domain.clone(),
            key.clone(),
            Json::new(1_u32),
        )),
        InstructionBox::from(RemoveKeyValue::domain(domain, key.clone())),
        InstructionBox::from(SetKeyValue::asset_definition(
            asset.clone(),
            key.clone(),
            Json::new(1_u32),
        )),
        InstructionBox::from(RemoveKeyValue::asset_definition(asset, key.clone())),
        InstructionBox::from(SetKeyValue::trigger(
            trigger.clone(),
            key.clone(),
            Json::new(1_u32),
        )),
        InstructionBox::from(RemoveKeyValue::trigger(trigger, key.clone())),
        InstructionBox::from(CustomInstruction::new("unreviewed metadata mutation")),
    ] {
        assert!(ensure_instruction_scope(&unreviewed, &mut tx).is_err());
    }
}

#[test]
fn private_account_metadata_root_decode_refusal_retains_original_retry() {
    use iroha_data_model::isi::SetKeyValue;
    use iroha_model_base::metadata::Metadata;
    use iroha_primitives::json::Json;
    let key: iroha_model_base::name::Name = "account_scope_control".parse().unwrap();
    let original = Json::new("original");
    let mut metadata = Metadata::default();
    metadata.insert(key.clone(), original.clone());
    let world = World::with(
        [],
        [Account::new(ALICE_ID.clone())
            .with_metadata(metadata)
            .build(&ALICE_ID)],
        [],
    );
    let (state, ds) = private_state_with_world(world);
    let mut block = state
        .try_block(header(2))
        .expect("admit original State owner");
    let instruction: InstructionBox =
        SetKeyValue::account(ALICE_ID.clone(), key.clone(), Json::new("replacement")).into();
    {
        let mut tx = block.try_transaction().expect("admit original State child");
        bind_scope(&mut tx, ds);
        norito::with_decode_limits_scope(
            norito::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, 0, 32),
            || {
                assert!(
                    execute_holding_limit_boundary(1, &instruction, &ALICE_ID, &mut tx).is_err()
                )
            },
        );
        assert_eq!(
            tx.execution_deferral().unwrap().reason(),
            ivm::error::ExecutionDeferral::ActiveMemoryCapacity
        );
        assert_eq!(
            private_account_metadata_value(&tx, &ALICE_ID, &key),
            Some(original.clone())
        );
        assert_eq!(
            tx.require_storage_admission(),
            Err(StateStorageAdmissionError::RootScopeDecode(
                RootScopeDecodeRefusal::Budget
            ))
        );
    }
    // The first original refusal belongs to the entire failed block owner, even after
    // its child is discarded. A retry cannot clear that verdict or acquire another child.
    assert!(matches!(
        block.try_transaction(),
        Err(StateStorageAdmissionError::RootScopeDecode(
            RootScopeDecodeRefusal::Budget
        ))
    ));
    drop(block);
    // Reacquire an admitted owner from the same unchanged original State. Only local
    // decode capacity has recovered; no instruction, metadata or source policy is replaced.
    let mut retry_block = state
        .try_block(header(2))
        .expect("read original State for retry");
    let mut retry = retry_block
        .try_transaction()
        .expect("admit fresh retry child");
    bind_scope(&mut retry, ds);
    assert_eq!(
        private_account_metadata_value(&retry, &ALICE_ID, &key),
        Some(original)
    );
    execute_holding_limit_boundary(1, &instruction, &ALICE_ID, &mut retry).unwrap();
    assert!(retry.execution_deferral().is_none());
    assert!(retry.require_storage_admission().is_ok());
    assert_eq!(
        private_account_metadata_value(&retry, &ALICE_ID, &key),
        Some(Json::new("replacement"))
    );
}
