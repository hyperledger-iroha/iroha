//! Executor authorization, execution and custody regression tests.
use super::*;
use crate::{
    kura::Kura,
    query,
    state::{State, World},
};
#[cfg(feature = "telemetry")]
use iroha_config::parameters::actual::{GasLiquidity, GasVolatility};
use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair, Signature};
use iroha_data_model::{
    asset::{
        ASSET_TRANSFER_CONTROL_METADATA_KEY, AssetTransferAvailability, AssetTransferControlRecord,
        AssetTransferControlStoreV1, AssetTransferControlWindow,
    },
    events::data::prelude::{AssetBatchTransferLegStatus, AssetBatchTransferRejectionCode},
    executor::{self as data_model_executor, ExecutorDataModel},
    isi::{
        Grant, SetAssetTransferAvailability, SetAssetTransferControl,
        transfer::{TransferAssetBatch, TransferAssetBatchEntry},
    },
    parameter::{CustomParameter, CustomParameterId},
    prelude::*,
    query::{QueryRequest, SingularQueryBox, prelude::FindParameters},
    smart_contract::{ContractAddress, ContractEmergencyHoldV1},
    transaction::executable::IvmBytecode,
};
use iroha_executor_data_model::isi::multisig::{
    MultisigApprove, MultisigCancel, MultisigProposalId, MultisigProposalNonce, MultisigPropose,
    MultisigRegister, MultisigSpec,
};
use iroha_model_base::name::Name;
use iroha_model_base::{chain::ChainId, peer::PeerId};
use iroha_primitives::json::Json;
use iroha_test_samples::{
    ALICE_ID, ALICE_KEYPAIR, BOB_ID, SAMPLE_GENESIS_ACCOUNT_ID, gen_account_in,
};
use mv::storage::StorageReadOnly;
use nonzero_ext::nonzero;
#[cfg(feature = "zk-stark")]
mod zk_hydration_route_tests {
    //! Real contract and trigger dispatch retain cold-registry refusals until abandonment.

    use super::*;
    use crate::execution_attempt::{ExecutionAttemptError, ExecutionDeferred};
    use crate::tx::TransactionRejectionReason;
    use iroha_data_model::{
        confidential::ConfidentialStatus,
        proof::{VerifyingKeyBox, VerifyingKeyId, VerifyingKeyRecord},
        zk::BackendTag,
    };

    #[inline(never)]
    fn route_state() -> Box<State> {
        Box::new(state_after_genesis(World::with(
            [],
            [Account::new(ALICE_ID.clone()).build(&ALICE_ID)],
            [],
        )))
    }

    fn next_header(state: &State) -> BlockHeader {
        BlockHeader::new(
            nonzero!(2_u64),
            Some(
                state
                    .view()
                    .latest_block_hash()
                    .expect("signed genesis parent"),
            ),
            None,
            0,
            0,
        )
    }

    // Each route has a distinct registry identity. Installing its bytes does not
    // prepare the key, so the real validation path needs no memo reset or refusal hook.
    fn install_cold_key(state: &State, suffix: &str) -> (VerifyingKeyId, Vec<u8>, usize) {
        let backend = "stark/fri/poseidon-x7-goldilocks-6x64-v1";
        // Use the legal maximum STARK label so the one cold String allocation
        // remains distinguishable from the already measured route preparation.
        let mut circuit = format!("{backend}:hydration-production-route-{suffix}");
        assert!(circuit.len() <= 128);
        circuit.push_str(&"x".repeat(128 - circuit.len()));
        let key = crate::zk_stark::StarkFriVerifyingKeyV1 {
            version: 1,
            circuit_id: circuit.clone(),
            n_log2: crate::zk_stark::STARK_FRI_CONSENSUS_MIN_N_LOG2,
            blowup_log2: crate::zk_stark::STARK_FRI_CONSENSUS_MIN_BLOWUP_LOG2,
            fold_arity: 2,
            queries: crate::zk_stark::STARK_FRI_CONSENSUS_MIN_QUERIES,
            merkle_arity: 2,
        };
        let bytes = norito::encode_canonical(&key).unwrap();
        let vk = VerifyingKeyBox::new(backend.into(), bytes.clone());
        let id = VerifyingKeyId::new(backend, format!("hydration-production-route-{suffix}"));
        let mut record = VerifyingKeyRecord::new_with_owner(
            1,
            circuit.clone(),
            None,
            "hydration",
            BackendTag::Stark,
            "goldilocks",
            [0x59; 32],
            crate::zk::hash_vk(&vk),
        );
        record.status = ConfidentialStatus::Active;
        record.max_proof_bytes = 1_048_576;
        record.gas_schedule_id = Some("stark_default".to_owned());
        record.vk_len = u32::try_from(bytes.len()).unwrap();
        record.key = Some(vk);
        let mut world = state.world.block();
        world.verifying_keys.insert(id.clone(), record);
        world.commit();
        (id, bytes, circuit.len())
    }

    fn route_decode_budget(allocation_limit: usize) -> norito::core::DecodeBudgetContext {
        norito::core::DecodeBudgetContext::new(norito::DecodeLimits::new(
            usize::MAX,
            usize::MAX,
            usize::MAX,
            allocation_limit,
            usize::MAX,
        ))
    }

    fn assert_original_registry_refusal(
        owner: ExecutionDeferred,
        circuit_bytes: usize,
        budget: &norito::core::DecodeBudgetContext,
        allocation_limit: usize,
    ) {
        assert!(
            owner.verification_refusal().is_some(),
            "the actual cold verifier producer must refuse: {owner:?}"
        );
        assert!(
            owner.allocation_refusal().is_none(),
            "no fabricated pool wake source"
        );
        let original = owner
            .decode_refusal()
            .expect("original decoder owner")
            .clone();
        assert_eq!(
            original.kind(),
            norito::core::DecodeLocalRefusalKind::EnclosingLimit
        );
        assert_eq!(
            original.resource_error(),
            norito::core::DecodeResourceError::TotalAllocationExceeded {
                attempted: budget
                    .consumed_allocated_bytes()
                    .checked_add(u64::try_from(circuit_bytes).unwrap())
                    .unwrap(),
                limit: u64::try_from(allocation_limit).unwrap(),
            },
            "the cold circuit String must exhaust this original route budget, not an earlier decoder"
        );
        assert!(
            std::error::Error::source(&original)
                .and_then(|error| error.downcast_ref::<norito::core::ScopedDecodeResourceError>())
                .is_some()
        );
        let moved = ExecutionDeferred::try_from_vm_error(owner.into_vm_error()).unwrap();
        assert_eq!(
            moved.decode_refusal(),
            Some(&original),
            "the exact original scope owner crosses the enclosing retry boundary"
        );
    }

    fn assert_refused_transaction(transaction: &mut StateTransaction<'_, '_>) {
        assert!(transaction.is_execution_deferred());
        assert_eq!(transaction.last_tx_gas_used, 0);
        assert_eq!(transaction.confidential_gas_used_in_tx, 0);
        assert!(transaction.pending_nexus_fee_receipt.is_none());
        assert!(transaction.world.external_event_buf.is_empty());
    }

    #[test]
    fn contract_route_preserves_cold_registry_owner_and_fresh_state_retry() {
        let state = route_state();
        let manifest_signing = crate::manifest_signing_test_support::ManifestSigningFixture::new();
        let (program, manifest) = kotodama_lang::compiler::Compiler::new()
            .compile_source_with_manifest(
                "seiyaku HydrationRoute { kotoage fn run() authorize(anyone) { } }",
            )
            .expect("compile real deployed contract");
        let code_hash = ivm::contract_code_hash(&program);
        let address =
            ContractAddress::derive(&state.network_id, &ALICE_ID, 587, DataSpaceId::UNIVERSAL)
                .unwrap();
        let artifact = iroha_data_model::smart_contract::ContractArtifactId::new(
            DataSpaceId::UNIVERSAL,
            code_hash,
        );
        let mut world = state.world.block();
        world.contract_code.insert(artifact, program);
        world.contract_manifests.insert(
            artifact,
            manifest
                .try_signed(
                    manifest_signing.context(),
                    manifest_signing.max_frame_bytes(),
                    &ALICE_KEYPAIR,
                )
                .unwrap(),
        );
        world.commit();
        bind_executor_test_contract_after_genesis(&state, &address, &ALICE_ID, code_hash);
        let signed = TransactionBuilder::new(
            state.network_id,
            ALICE_ID.clone(),
            FeePaymentIntent::authority(Vec::new(), core::num::NonZeroU64::new(1_000_000)),
        )
        .with_executable(Executable::ContractCall(ContractInvocation {
            contract_address: address,
            expected_code_hash: code_hash,
            entrypoint: "run".to_owned(),
            arguments: None,
        }))
        .sign(ALICE_KEYPAIR.private_key());
        let signed_bytes = signed.encode_wire_v1().unwrap();
        let mut cache = IvmCache::new();
        let (id, key_bytes, circuit_bytes) = install_cold_key(&state, "contract");
        // A zero commitment is rejected by the actual registry validator before its
        // backend decoder. Measure this real route prefix, not the full successful
        // route, whose later fee readers would leave slack before the cold String.
        // The restored record below differs only in its fixed-size commitment.
        let valid_record = {
            let mut world = state.world.block();
            let record = world.verifying_keys.get(&id).unwrap().clone();
            assert_ne!(record.commitment, [0; 32]);
            let mut invalid = record.clone();
            invalid.commitment = [0; 32];
            world.verifying_keys.insert(id.clone(), invalid);
            world.commit();
            record
        };
        let calibration_baseline = crate::snapshot::canonical_state_snapshot_bytes(&state);
        let mut route_allocation_limit = 0;
        for _ in 0..2 {
            let budget = route_decode_budget(usize::MAX);
            let mut warm_block = state.block(next_header(&state));
            let mut warm =
                warm_block.transaction_for_fastpq_testing(Hash::from(signed.hash_as_entrypoint()));
            warm.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
            warm.world.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
            let attempt = signed.clone();
            let error = budget
                .with(|| {
                    crate::executor::Executor::Initial
                        .execute_transaction(&mut warm, &ALICE_ID, &attempt, &mut cache)
                })
                .expect_err("the real registry must reject the zero commitment before decoding");
            let ExecutionAttemptError::Rejected(ValidationFail::InternalError(message)) = error
            else {
                panic!("calibration must reach the actual invalid registry: {error:?}");
            };
            assert_eq!(
                message,
                format!("invalid ZK snapshot state: {}", ivm::VMError::NoritoInvalid)
            );
            route_allocation_limit = usize::try_from(budget.consumed_allocated_bytes()).unwrap();
            assert!(!warm.is_execution_deferred());
            warm.ensure_execution_ready().unwrap();
            assert_eq!(warm.last_tx_gas_used, 0);
            assert_eq!(warm.confidential_gas_used_in_tx, 0);
            assert!(warm.pending_nexus_fee_receipt.is_none());
            assert!(warm.world.external_event_buf.is_empty());
            drop(warm);
            assert_eq!(warm_block.committed_fragment_count(), 0);
            assert_eq!(warm_block.gas_used_in_block, 0);
            drop(warm_block);
            assert_eq!(
                crate::snapshot::canonical_state_snapshot_bytes(&state),
                calibration_baseline
            );
        }
        assert!(
            route_allocation_limit > 0,
            "the real route prefix decodes before the registry validator"
        );
        {
            let mut world = state.world.block();
            let mut restored = world.verifying_keys.get(&id).unwrap().clone();
            assert_eq!(restored.commitment, [0; 32]);
            restored.commitment = valid_record.commitment;
            assert_eq!(restored, valid_record);
            assert_eq!(restored.key.as_ref().unwrap().bytes, key_bytes);
            world.verifying_keys.insert(id.clone(), restored);
            world.commit();
        }
        let baseline = crate::snapshot::canonical_state_snapshot_bytes(&state);
        let key_pointer = state
            .view()
            .world()
            .verifying_keys
            .get(&id)
            .unwrap()
            .key
            .as_ref()
            .unwrap()
            .bytes
            .as_ptr();
        let mut block = state.block(next_header(&state));
        let mut transaction =
            block.transaction_for_fastpq_testing(Hash::from(signed.hash_as_entrypoint()));
        transaction.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
        transaction.world.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
        let refusal_budget = route_decode_budget(route_allocation_limit);
        let attempt = signed.clone();
        let error = refusal_budget
            .with(|| {
                crate::executor::Executor::Initial.execute_transaction(
                    &mut transaction,
                    &ALICE_ID,
                    &attempt,
                    &mut cache,
                )
            })
            .expect_err("actual contract hydration must export its original local refusal");
        let ExecutionAttemptError::Deferred(owner) = error else {
            panic!("cold registry became a certified contract rejection: {error:?}");
        };
        assert_original_registry_refusal(
            owner,
            circuit_bytes,
            &refusal_budget,
            route_allocation_limit,
        );
        assert_refused_transaction(&mut transaction);
        assert_eq!(
            transaction.ensure_execution_ready().unwrap_err().reason(),
            ivm::error::ExecutionDeferral::LocalInvariantViolation
        );
        drop(transaction);
        assert_eq!(block.committed_fragment_count(), 0);
        assert_eq!(block.gas_used_in_block, 0);
        drop(block);
        assert_eq!(
            crate::snapshot::canonical_state_snapshot_bytes(&state),
            baseline
        );
        assert_eq!(
            state
                .view()
                .world()
                .verifying_keys
                .get(&id)
                .unwrap()
                .key
                .as_ref()
                .unwrap()
                .bytes
                .as_ptr(),
            key_pointer
        );
        assert_eq!(signed.encode_wire_v1().unwrap(), signed_bytes);
        signed.verify_signature().unwrap();
        let mut fresh_block = state.block(next_header(&state));
        let mut fresh =
            fresh_block.transaction_for_fastpq_testing(Hash::from(signed.hash_as_entrypoint()));
        fresh.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
        fresh.world.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
        crate::executor::Executor::Initial
            .execute_transaction(&mut fresh, &ALICE_ID, &signed, &mut cache)
            .expect("fresh attempt executes the same signed contract and same committed registry");
        assert!(!fresh.is_execution_deferred());
        assert!(fresh.last_tx_gas_used > 0);
        assert_eq!(
            fresh
                .world
                .verifying_keys
                .get(&id)
                .unwrap()
                .key
                .as_ref()
                .unwrap()
                .bytes,
            key_bytes
        );
        drop(fresh);
        assert_eq!(fresh_block.committed_fragment_count(), 0);
        drop(fresh_block);
        assert_eq!(
            crate::snapshot::canonical_state_snapshot_bytes(&state),
            baseline
        );
    }

    #[test]
    fn trigger_route_preserves_cold_registry_owner_and_fresh_state_retry() {
        let state = route_state();
        let trigger_id: TriggerId = "hydration_registry_callback".parse().unwrap();
        let mut program = ivm::ProgramMetadata {
            max_cycles: 100,
            ..ivm::ProgramMetadata::default()
        }
        .encode();
        // HALT itself costs zero gas; execute one real arithmetic instruction so
        // the unchanged fresh-retry gas assertion witnesses actual VM execution.
        program.extend_from_slice(
            &ivm::encoding::wide::encode_ri(ivm::instruction::wide::arithmetic::ADDI, 5, 0, 1)
                .to_le_bytes(),
        );
        program.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());
        let trigger = Trigger::new(
            trigger_id.clone(),
            Action::new(
                Executable::Ivm(IvmBytecode::from_compiled(program)),
                Repeats::Indefinitely,
                ALICE_ID.clone(),
                ExecuteTriggerEventFilter::new()
                    .for_trigger(trigger_id.clone())
                    .under_authority(ALICE_ID.clone()),
            )
            .unwrap(),
        );
        {
            let mut setup = state.block(next_header(&state));
            let mut transaction = setup.transaction_for_callback_testing();
            Register::trigger(trigger)
                .execute(&ALICE_ID, &mut transaction)
                .unwrap();
            transaction.apply_callback_for_testing().unwrap();
            setup.commit_world_overlay_for_testing().unwrap();
        }
        let event = ExecuteTriggerEvent {
            trigger_id: trigger_id.clone(),
            authority: ALICE_ID.clone(),
            args: Json::default(),
        };
        // The first real attempt prepares the program cache. Measure the second
        // complete attempt using the same cache and a fresh transaction, before the
        // cold key exists. This leaves every ordinary route reader enough headroom.
        let mut route_allocation_limit = 0;
        for _ in 0..2 {
            let budget = route_decode_budget(usize::MAX);
            let mut warm_block = state.block(next_header(&state));
            let mut warm = warm_block.transaction_for_callback_testing();
            warm.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
            warm.world.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
            budget
                .with(|| warm.execute_called_trigger(&trigger_id, &event))
                .expect("the real registered trigger route reaches its generic VM");
            route_allocation_limit = usize::try_from(budget.consumed_allocated_bytes()).unwrap();
        }
        assert!(
            route_allocation_limit > 0,
            "real route preparation decodes before hydration"
        );
        let (id, key_bytes, circuit_bytes) = install_cold_key(&state, "trigger");
        let baseline = crate::snapshot::canonical_state_snapshot_bytes(&state);
        let key_pointer = state
            .view()
            .world()
            .verifying_keys
            .get(&id)
            .unwrap()
            .key
            .as_ref()
            .unwrap()
            .bytes
            .as_ptr();
        let mut block = state.block(next_header(&state));
        let mut transaction = block.transaction_for_callback_testing();
        transaction.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
        transaction.world.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
        let refusal_budget = route_decode_budget(route_allocation_limit);
        let error = refusal_budget
            .with(|| transaction.execute_called_trigger(&trigger_id, &event))
            .expect_err("actual trigger hydration must retain a local refusal");
        assert!(matches!(
            error,
            TransactionRejectionReason::Validation(ValidationFail::InternalError(_))
        ));
        assert_refused_transaction(&mut transaction);
        let original = transaction
            .borrow_execution_deferral()
            .unwrap()
            .decode_refusal()
            .unwrap_or_else(|| {
                panic!(
                    "cold trigger must retain the original decoder: {:?}",
                    transaction.borrow_execution_deferral()
                )
            })
            .clone();
        let owner = transaction.ensure_execution_ready().unwrap_err();
        assert_eq!(owner.decode_refusal(), Some(&original));
        assert_original_registry_refusal(
            owner,
            circuit_bytes,
            &refusal_budget,
            route_allocation_limit,
        );
        assert_eq!(
            transaction.ensure_execution_ready().unwrap_err().reason(),
            ivm::error::ExecutionDeferral::LocalInvariantViolation
        );
        drop(transaction);
        assert_eq!(block.committed_fragment_count(), 0);
        assert_eq!(block.gas_used_in_block, 0);
        drop(block);
        assert_eq!(
            crate::snapshot::canonical_state_snapshot_bytes(&state),
            baseline
        );
        assert_eq!(
            state
                .view()
                .world()
                .verifying_keys
                .get(&id)
                .unwrap()
                .key
                .as_ref()
                .unwrap()
                .bytes
                .as_ptr(),
            key_pointer
        );
        let mut fresh_block = state.block(next_header(&state));
        let mut fresh = fresh_block.transaction_for_callback_testing();
        fresh.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
        fresh.world.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
        let step = fresh
            .execute_called_trigger(&trigger_id, &event)
            .expect("fresh attempt executes the same registered trigger and committed registry");
        assert!(step.0.is_empty());
        assert!(!fresh.is_execution_deferred());
        fresh.ensure_execution_ready().unwrap();
        assert!(fresh.last_tx_gas_used > 0);
        assert!(
            fresh
                .world
                .external_event_buf
                .iter()
                .any(|event| matches!(event, EventBox::ExecuteTrigger(_)))
        );
        assert_eq!(
            fresh
                .world
                .verifying_keys
                .get(&id)
                .unwrap()
                .key
                .as_ref()
                .unwrap()
                .bytes,
            key_bytes
        );
        drop(fresh);
        assert_eq!(fresh_block.committed_fragment_count(), 0);
        drop(fresh_block);
        assert_eq!(
            crate::snapshot::canonical_state_snapshot_bytes(&state),
            baseline
        );
    }
}

#[test]
fn multisig_role_namespace_reservation_is_process_independent() {
    for name in [
        "MULTISIG_SIGNATORY",
        "MULTISIG_SIGNATORY/domain/address",
        "MULTISIG_SIGNATORY//opaque",
    ] {
        let role_id: RoleId = name.parse().expect("valid reserved role id");
        assert!(
            is_reserved_multisig_role_id(&role_id),
            "must reserve {name}"
        );
    }
    for name in [
        "MULTISIG_SIGNATORY_ADJACENT",
        "MULTISIG_SIGNATORY2/domain/address",
        "ordinary-role",
    ] {
        let role_id: RoleId = name.parse().expect("valid ordinary role id");
        assert!(
            !is_reserved_multisig_role_id(&role_id),
            "must not reserve {name}"
        );
    }
}
#[test]
fn executor_byte_array_accepts_maximum_byte() {
    let value = json::Value::Array(vec![json::Value::from(u64::from(u8::MAX))]);
    let decoded = decode_executor_bytes(value, "norito").expect("decode byte array");
    assert_eq!(decoded, vec![u8::MAX]);
}
#[test]
fn executor_byte_array_rejects_byte_overflow() {
    let value = json::Value::Array(vec![json::Value::from(u64::from(u8::MAX) + 1)]);
    let error = decode_executor_bytes(value, "norito").expect_err("reject byte overflow");
    match error {
        json::Error::InvalidField { field, message } => {
            assert_eq!(field, "norito");
            assert_eq!(message, "expected byte in range 0..=255");
        }
        other => panic!("unexpected error: {other}"),
    }
}
fn checked_keypair() -> KeyPair {
    KeyPair::try_random().expect("executor fixture key generation should succeed")
}
fn executor_test_network_id(seed: &[u8]) -> NetworkId {
    NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
        seed,
    )))
}
fn state_for_testing(world: World) -> State {
    // These component fixtures execute on an explicitly committed global root.
    let mut parameters = crate::state::parameter_cell_block_for_testing(&world)
        .expect("funded original parameter-only fixture block");
    crate::state::set_parameter_cell_for_testing(
        &mut parameters,
        crate::sumeragi::lanes::routing::test_support::metadata(
            iroha_data_model::block::consensus::SumeragiRootScope::Global,
        ),
        world.sccp_committee_generations.allocation_budget(),
    )
    .expect("valid fixture parameters");
    parameters.commit();
    State::new_for_testing(
        world,
        Kura::blank_kura_for_testing(),
        query::store::LiveQueryStore::start_test(),
    )
}
/// Structural post-genesis fixture with an explicit committed Global root.
/// This metadata grants no authenticated genesis or block-publication capability.
fn component_world_for_testing(world: World) -> World {
    let mut parameters = crate::state::parameter_cell_block_for_testing(&world)
        .expect("funded original parameter-only fixture block");
    assert!(
        !parameters.custom().contains_key(
            &iroha_data_model::parameter::system::consensus_metadata::handshake_meta_id()
        ),
        "component root must be selected before State construction"
    );
    crate::state::set_parameter_cell_for_testing(
        &mut parameters,
        crate::sumeragi::lanes::routing::test_support::metadata(
            iroha_data_model::block::consensus::SumeragiRootScope::Global,
        ),
        world.sccp_committee_generations.allocation_budget(),
    )
    .expect("valid fixture parameters");
    // Explicit ordinary component input, never a production default or
    // authenticated genesis/publication capability.
    crate::state::set_parameter_cell_for_testing(
        &mut parameters,
        crate::sumeragi::test_chain::crypto_fixture_policy()
            .into_parameter()
            .expect("canonical component crypto policy"),
        world.sccp_committee_generations.allocation_budget(),
    )
    .expect("install the component's original crypto policy");
    parameters.commit();
    world
}
#[test]
fn executor_component_fixture_owns_crypto_without_committed_genesis() {
    let state = component_state_for_testing(World::new());
    assert_eq!(state.committed_height(), 0);
    assert_eq!(state.kura().blocks_count(), 0);
    assert_eq!(
        state
            .view()
            .world()
            .parameters()
            .require_crypto_policy()
            .unwrap(),
        &crate::sumeragi::test_chain::crypto_fixture_policy(),
    );
}
fn component_state_for_testing(world: World) -> State {
    state_for_testing(component_world_for_testing(world))
}
/// Seed canonical contract storage for an ordinary component fixture after real genesis.
/// This changes only its fixture World; it does not grant root scope or publication authority.
fn bind_executor_test_contract_after_genesis(
    state: &State,
    address: &ContractAddress,
    owner: &AccountId,
    code_hash: Hash,
) {
    let mut world = state.world.block();
    world.accounts.insert(
        address.subject_id(),
        iroha_data_model::account::AccountValue::new(
            iroha_data_model::account::AccountDetails::default(),
        ),
    );
    world.contract_instances.insert(address.clone(), code_hash);
    world
        .contract_subject_addresses
        .insert(address.subject_id(), address.clone());
    world.contract_subject_bindings.insert(
        address.clone(),
        crate::smartcontracts::code::ContractSubjectBinding::new_direct(address, owner.clone())
            .with_active_code_hash(code_hash),
    );
    world.commit();
}

#[test]
fn ordinary_contract_fixture_retains_authenticated_genesis_identity() {
    let owner = ALICE_ID.clone();
    let state = state_after_genesis(World::with(
        [],
        [Account::new(owner.clone()).build(&owner)],
        [],
    ));
    let parent = state.view().latest_block_hash().unwrap();
    let root = crate::sumeragi::lanes::routing::committed_root_scope(state.view().world()).unwrap();
    let address =
        ContractAddress::derive(&state.network_id, &owner, 501, DataSpaceId::UNIVERSAL).unwrap();
    let code_hash = Hash::new(b"ordinary-contract-fixture-identity");
    bind_executor_test_contract_after_genesis(&state, &address, &owner, code_hash);
    let view = state.view();
    assert_eq!(state.committed_height(), 1);
    assert_eq!(view.latest_block_hash(), Some(parent));
    assert_eq!(
        crate::sumeragi::lanes::routing::committed_root_scope(view.world()),
        Some(root)
    );
    assert_eq!(
        view.world().contract_instances.get(&address),
        Some(&code_hash)
    );
    assert_eq!(
        view.world()
            .contract_subject_addresses
            .get(&address.subject_id()),
        Some(&address)
    );
    assert!(view.world().account(&address.subject_id()).is_ok());
}

fn state_after_genesis(world: World) -> State {
    state_after_genesis_with_instructions(world, |_| Vec::new())
}

fn state_after_genesis_with_instructions(
    world: World,
    instructions: impl FnOnce(&AccountId) -> Vec<InstructionBox>,
) -> State {
    use crate::sumeragi::{
        startup,
        test_chain::{CertifiedTestChain, TestChainConfig},
    };

    let mut config = TestChainConfig::new(world, 0);
    let genesis_account = AccountId::new(config.genesis_key.public_key().clone());
    config.genesis_instructions = instructions(&genesis_account);
    let consensus_mode = config.consensus_mode;
    let prepared = CertifiedTestChain::prepare(config).expect("prepare signed executor genesis");
    // Preparation has not spawned a StateExecutor. Consume the original unique
    // State, then apply its actual signed genesis without copying root metadata.
    let state = Arc::try_unwrap(prepared.state)
        .unwrap_or_else(|_| panic!("unpublished executor State is unique"));
    startup::apply_genesis(
        &state,
        prepared.genesis.block().clone(),
        &genesis_account,
        consensus_mode.into(),
        None,
    )
    .expect("apply signed executor genesis");
    state
}

#[test]
fn post_genesis_fixture_retains_original_world_and_authenticated_root() {
    let marker: Name = "executor_fixture_original".parse().unwrap();
    let mut metadata = Metadata::default();
    metadata.insert(marker.clone(), Json::new(true));
    let world = World::with(
        [],
        [Account::new(ALICE_ID.clone())
            .with_metadata(metadata)
            .build(&ALICE_ID)],
        [],
    );
    let state = state_after_genesis(world);
    assert_eq!(state.committed_height(), 1);
    assert_eq!(state.kura().blocks_count(), 1);
    let genesis = state
        .kura()
        .get_block(nonzero!(1_usize), &state.ivm_execution_budget())
        .expect("completed original State read")
        .unwrap();
    assert_eq!(
        state.network_id_ref(),
        &NetworkId::from_genesis_hash(genesis.hash())
    );
    let signed =
        iroha_data_model::sumeragi_finality::signed_genesis_consensus_metadata(&genesis).unwrap();
    let view = state.view();
    assert_eq!(
        crate::sumeragi::lanes::routing::committed_root_scope(view.world()),
        Some(signed.sumeragi_context.root_scope)
    );
    assert_eq!(
        view.world()
            .account(&ALICE_ID)
            .unwrap()
            .metadata()
            .get(&marker),
        Some(&Json::new(true))
    );

    // Refusal fixtures remain genuinely pre-genesis, with no routing authority.
    let pristine = State::new_for_testing(
        World::new(),
        Kura::blank_kura_for_testing(),
        query::store::LiveQueryStore::start_test(),
    );
    assert_eq!(pristine.committed_height(), 0);
    assert_eq!(pristine.kura().blocks_count(), 0);
    assert!(
        crate::sumeragi::lanes::routing::committed_root_scope(pristine.view().world()).is_none()
    );
}
#[test]
fn borrowed_batch_iterator_retains_original_items_and_exact_remaining_length() {
    let network = "hash:0000000000000000000000000000000000000000000000000000000000000001#C50E"
        .parse()
        .expect("canonical fixture network");
    let call = ContractInvocation {
        contract_address: ContractAddress::derive(&network, &ALICE_ID, 31, DataSpaceId::UNIVERSAL)
            .expect("canonical fixture contract"),
        expected_code_hash: Hash::new(b"borrowed mixed batch"),
        entrypoint: "main".to_owned(),
        arguments: None,
    };
    let items = [
        ExecutableBatchItem::ContractCall(call.clone()),
        ExecutableBatchItem::Instruction(Log::new(Level::INFO, "first".to_owned()).into()),
        ExecutableBatchItem::ContractCall(call.clone()),
        ExecutableBatchItem::Instruction(Log::new(Level::INFO, "second".to_owned()).into()),
        ExecutableBatchItem::ContractCall(call),
    ];
    let mut borrowed = AuthoredBatchInstructions::new(&items);
    assert_eq!(borrowed.len(), 2);
    assert_eq!(borrowed.size_hint(), (2, Some(2)));
    let ExecutableBatchItem::Instruction(first) = &items[1] else {
        unreachable!()
    };
    assert!(std::ptr::eq(borrowed.next().unwrap(), first));
    assert_eq!(borrowed.len(), 1);
    let mut independently_traversed = borrowed.clone();
    let ExecutableBatchItem::Instruction(second) = &items[3] else {
        unreachable!()
    };
    assert!(std::ptr::eq(borrowed.next().unwrap(), second));
    assert!(std::ptr::eq(
        independently_traversed.next().unwrap(),
        second
    ));
    assert_eq!(borrowed.size_hint(), (0, Some(0)));
    assert!(borrowed.next().is_none());
    assert!(borrowed.next().is_none());
    assert_eq!(independently_traversed.len(), 0);
    assert_eq!(AuthoredBatchInstructions::new(&items[..1]).len(), 0);
    assert!(AuthoredBatchInstructions::new(&items[..1]).next().is_none());
    assert!(AuthoredBatchInstructions::new(&[]).next().is_none());
}

#[test]
fn borrowed_instruction_validator_payload_has_exact_canonical_owned_wire() {
    let instruction =
        InstructionBox::from(Log::new(Level::INFO, "canonical validation".to_owned()));
    let context = || ExecutorContext {
        authority: ALICE_ID.clone(),
        curr_block: BlockHeader::new(nonzero!(1_u64), None, None, 0, 0),
    };
    let borrowed = ValidatePayload {
        context: context(),
        target: norito::core::PayloadRef(&instruction),
    };
    assert!(std::ptr::eq(&*borrowed.target, &instruction));
    let owned = ValidatePayload {
        context: context(),
        target: instruction.clone(),
    };
    let borrowed_bytes = encode_executor_input(&borrowed)
        .expect("borrowed input through the actual fixed-V1 executor encoder");
    let owned_bytes =
        encode_executor_input(&owned).expect("owned input through the same executor encoder");
    assert_eq!(borrowed_bytes, owned_bytes);
    assert_eq!(EXECUTOR_LENGTH_PREFIX_BYTES, 8);
    assert_eq!(
        u64::from_le_bytes(borrowed_bytes[..8].try_into().expect("fixed u64 prefix")),
        u64::try_from(borrowed_bytes.len()).expect("complete encoded input length"),
    );
    let mut payload = &borrowed_bytes[8..];
    let decoded = ValidatePayload::<InstructionBox>::decode(&mut payload)
        .expect("the existing validator decodes the exact bare V1 payload");
    assert!(
        payload.is_empty(),
        "the validator must consume the complete payload"
    );
    assert_eq!(decoded.target, instruction);
    assert_eq!(decoded.context.authority, owned.context.authority);
    assert_eq!(decoded.context.curr_block, owned.context.curr_block);
}

#[test]
fn borrowed_execution_fee_meter_remains_bound_to_original_signed_fee_intent() {
    for substitute_fee_intent in [false, true] {
        let state = state_after_genesis(World::with(
            [],
            [Account::new(ALICE_ID.clone()).build(&ALICE_ID)],
            [],
        ));
        let make_signed = |gas_limit| {
            let mut builder = TransactionBuilder::new(
                *state.network_id_ref(),
                ALICE_ID.clone(),
                FeePaymentIntent::authority(Vec::new(), core::num::NonZeroU64::new(gas_limit)),
            );
            builder.set_creation_time(std::time::Duration::from_millis(0));
            builder
                .with_instructions([Log::new(Level::INFO, "source-bound fee work".to_owned())])
                .sign(ALICE_KEYPAIR.private_key())
        };
        let signed = make_signed(50_000);
        let changed_fee = make_signed(60_000);
        assert_eq!(signed.instructions(), changed_fee.instructions());
        assert_ne!(
            signed.fee_payment_intent(),
            changed_fee.fee_payment_intent()
        );
        assert_ne!(signed.hash(), changed_fee.hash());
        let mut changed_payload = signed.payload().clone();
        changed_payload.fee_payment = changed_fee.fee_payment_intent().clone();
        assert_eq!(
            &changed_payload,
            changed_fee.payload(),
            "only signed fee intent differs"
        );
        let source = signed.encode_wire_v1().unwrap();
        let source_probe = signed_source_observation::Probe::new(&signed);
        let header = BlockHeader::new(
            nonzero!(2_u64),
            state.view().latest_block_hash(),
            None,
            0,
            0,
        );
        let mut block = state.block(header);
        let mut execution =
            block.transaction_for_fastpq_testing(Hash::from(signed.hash_as_entrypoint()));
        execution.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
        execution.world.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
        super::Executor::Initial
            .execute_transaction(&mut execution, &ALICE_ID, &signed, &mut IvmCache::new())
            .expect("original signed body performs real metered work");
        source_probe.assert_observed_once();
        assert!(execution.last_tx_gas_used > 0);
        let settlement = execution
            .take_execution_fee_settlement()
            .expect("actual root closed its fee record")
            .expect("ordinary authored work retains its source-bound fee record");
        // A discarded business overlay can transfer only this exact root's one-shot fee record.
        drop(execution);
        let mut fee_overlay =
            block.transaction_for_fastpq_testing(Hash::from(signed.hash_as_entrypoint()));
        fee_overlay.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
        fee_overlay.world.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
        fee_overlay.current_tx_hash = Some(signed.hash());
        fee_overlay.tx_call_hash = Some(Hash::from(signed.hash_as_entrypoint()));
        let result = settlement.settle(
            &mut fee_overlay,
            if substitute_fee_intent {
                &changed_fee
            } else {
                &signed
            },
        );
        if substitute_fee_intent {
            assert!(
                matches!(result, Err(ExecutionFeeSettlementError::Owner(reason))
                    if reason == "rejection fee record differs from its exact fresh source overlay")
            );
        } else {
            assert!(
                matches!(result, Ok(false)),
                "this fixture's exact committed fee policy charges zero"
            );
        }
        assert_eq!(
            fee_overlay.last_tx_gas_used, 0,
            "settlement cannot recount execution work"
        );
        assert_eq!(signed.encode_wire_v1().unwrap(), source);
        signed
            .verify_signature()
            .expect("original signed fee intent remains intact");
    }
}

#[test]
fn borrowed_execute_transaction_keeps_genuine_accepted_carrier_and_completes_work() {
    let account = Account::new(ALICE_ID.clone()).build(&ALICE_ID);
    let state = state_after_genesis(World::with([], [account], []));
    let instruction = InstructionBox::from(Log::new(Level::INFO, "retained accepted".into()));
    let expected_gas = isi_gas::meter_instructions(
        core::slice::from_ref(&instruction),
        &crate::gas::DEFAULT_ZK_GAS,
    )
    .expect("actual authored instruction quote");
    let signed = TransactionBuilder::new(
        *state.network_id_ref(),
        ALICE_ID.clone(),
        FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([instruction])
    .sign(ALICE_KEYPAIR.private_key());
    // Existing unchecked fixture construction isolates source retention; this test does not
    // replace ingress signature/admission checks or claim any native proof acceptance.
    let accepted = crate::tx::AcceptedTransaction::new_unchecked(std::borrow::Cow::Owned(signed));
    let original = accepted.external().unwrap();
    let signed_address = std::ptr::from_ref(original);
    let executable_address = std::ptr::from_ref(original.instructions());
    let before = original.encode_wire_v1().unwrap();
    let mut block = state.block(BlockHeader::new(
        nonzero!(2_u64),
        Some(
            state
                .view()
                .latest_block_hash()
                .expect("genuine genesis parent"),
        ),
        None,
        0,
        0,
    ));
    let mut tx = block.transaction_for_fastpq_testing(Hash::from(original.hash_as_entrypoint()));
    tx.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
    tx.world.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
    let source_probe = signed_source_observation::Probe::new(original);
    super::Executor::Initial
        .execute_transaction(&mut tx, &ALICE_ID, original, &mut IvmCache::new())
        .expect("canonical public borrowed boundary completes actual authored work");
    source_probe.assert_observed_once();
    assert_eq!(tx.last_tx_gas_used, expected_gas);
    assert!(!tx.is_execution_deferred());
    let retained = accepted.external().unwrap();
    assert_eq!(std::ptr::from_ref(retained), signed_address);
    assert_eq!(
        std::ptr::from_ref(retained.instructions()),
        executable_address
    );
    assert_eq!(retained.encode_wire_v1().unwrap(), before);
    retained
        .verify_signature()
        .expect("same signed source remains intact");
}

#[test]
fn borrowed_transaction_body_keeps_original_signed_native_and_batch_sources() {
    for batch in [false, true] {
        let account = Account::new(ALICE_ID.clone()).build(&ALICE_ID);
        let state = state_after_genesis(World::with([], [account], []));
        let instruction = InstructionBox::from(Log::new(Level::INFO, "borrowed original".into()));
        let expected_gas = isi_gas::meter_instructions(
            core::slice::from_ref(&instruction),
            &crate::gas::DEFAULT_ZK_GAS,
        )
        .expect("actual authored instruction quote");
        let executable = if batch {
            Executable::Batch(vec![ExecutableBatchItem::Instruction(instruction)].into())
        } else {
            Executable::Instructions(vec![instruction].into())
        };
        let signed = TransactionBuilder::new(
            *state.network_id_ref(),
            ALICE_ID.clone(),
            FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_executable(executable)
        .sign(ALICE_KEYPAIR.private_key());
        let original = signed.encode_wire_v1().expect("original signed source");
        let executable_address = std::ptr::from_ref(signed.instructions());
        let mut block = state.block(BlockHeader::new(
            nonzero!(2_u64),
            Some(
                state
                    .view()
                    .latest_block_hash()
                    .expect("genuine genesis parent"),
            ),
            None,
            0,
            0,
        ));
        let mut tx = block.transaction_for_fastpq_testing(Hash::from(signed.hash_as_entrypoint()));
        tx.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
        tx.world.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
        let source_probe = signed_source_observation::Probe::new(&signed);
        super::Executor::Initial
            .execute_transaction_body(&mut tx, &ALICE_ID, &signed, &mut IvmCache::new())
            .expect("actual borrowed authored execution");
        source_probe.assert_observed_once();
        assert_eq!(tx.last_tx_gas_used, expected_gas);
        tx.finish_execution_effect_budget()
            .expect("complete authored effect budget");
        tx.close_execution_fee_meter();
        assert_eq!(
            std::ptr::from_ref(signed.instructions()),
            executable_address
        );
        assert_eq!(signed.encode_wire_v1().unwrap(), original);
        signed
            .verify_signature()
            .expect("retained original signature");
    }
}

#[test]
fn borrowed_transaction_body_keeps_original_attachment_and_unsupported_verdict() {
    use iroha_data_model::transaction::{
        SignedAttachmentPlanV1,
        signed_attachments::{
            ActionAuthorityBindingV1, ActionInvocationBindingV1, ActionOperationV1,
            ActionPublicInputsDigestV1, ActionRoleV1, AttachmentKindV1, AttachmentRoleV1,
            PlannedActionV1, PlannedAttachmentV1, SignedAttachmentErrorV1,
        },
    };
    let account = Account::new(ALICE_ID.clone()).build(&ALICE_ID);
    let state = state_after_genesis(World::with([], [account], []));
    let instruction = InstructionBox::from(Log::new(Level::INFO, "unsupported original".into()));
    let mut plan = SignedAttachmentPlanV1::new();
    plan.push_action(PlannedActionV1 {
        operation: ActionOperationV1::NativeInstruction(
            iroha_data_model::isi::instruction_wire_id(&instruction)
                .unwrap()
                .to_owned(),
        ),
        role: ActionRoleV1::ProofEffect,
        authority: ActionAuthorityBindingV1::TransactionAuthority,
        invocation: ActionInvocationBindingV1::Direct {
            instruction_index: 0,
        },
        public_inputs_digest: ActionPublicInputsDigestV1::new([0x5a; 32]),
        attachments: vec![PlannedAttachmentV1 {
            kind: AttachmentKindV1::Proof,
            role: AttachmentRoleV1::ActionProof,
            bytes: vec![0xaa; 48],
        }],
    });
    let signed = TransactionBuilder::new(
        *state.network_id_ref(),
        ALICE_ID.clone(),
        FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([instruction])
    .with_attachment_plan(plan)
    .expect("real canonical declaration; no proof validity claimed")
    .sign(ALICE_KEYPAIR.private_key());
    signed
        .validate_signed_attachments_v1()
        .expect("complete descriptor binding");
    let original = signed.encode_wire_v1().unwrap();
    let occurrence = &signed.attachments().unwrap().occurrences()[0].bytes;
    let original_pointer = occurrence.as_ptr();
    let original_length = occurrence.len();
    let expected = format!(
        "malformed signed attachments: {}",
        SignedAttachmentErrorV1::UnsupportedOperation { action_index: 0 }
    );
    let mut block = state.block(BlockHeader::new(
        nonzero!(2_u64),
        Some(
            state
                .view()
                .latest_block_hash()
                .expect("genuine genesis parent"),
        ),
        None,
        0,
        0,
    ));
    let mut tx = block.transaction_for_fastpq_testing(Hash::from(signed.hash_as_entrypoint()));
    tx.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
    tx.world.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
    let source_probe = signed_source_observation::Probe::new(&signed);
    let error = super::Executor::Initial
        .execute_transaction_body(&mut tx, &ALICE_ID, &signed, &mut IvmCache::new())
        .expect_err("unfinished native consuming route remains refused");
    source_probe.assert_observed_once();
    assert!(matches!(error, ValidationFail::NotPermitted(message) if message == expected));
    assert_eq!(tx.last_tx_gas_used, 0);
    assert!(!tx.is_execution_deferred());
    let retained = &signed.attachments().unwrap().occurrences()[0].bytes;
    assert_eq!(
        (retained.as_ptr(), retained.len()),
        (original_pointer, original_length)
    );
    assert_eq!(signed.encode_wire_v1().unwrap(), original);
    signed
        .verify_signature()
        .expect("unchanged source remains signed");
}

#[test]
fn borrowed_transaction_proof_rejects_without_authoritative_relation() {
    use iroha_data_model::{
        proof::{ProofAttachment, ProofBox, VerifyingKeyId},
        transaction::signed_attachments::SignedAttachmentErrorV1,
    };
    let account = Account::new(ALICE_ID.clone()).build(&ALICE_ID);
    let state = state_after_genesis(World::with([], [account], []));
    let signed = TransactionBuilder::new(
        *state.network_id_ref(),
        ALICE_ID.clone(),
        FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([Log::new(
        Level::INFO,
        "advisory carrier has no relation".into(),
    )])
    .with_proof_attachments([ProofAttachment::new_ref(
        "halo2/ipa".into(),
        ProofBox::new("halo2/ipa".into(), vec![1, 2, 3]),
        VerifyingKeyId::new("halo2/ipa", "advisory_only"),
    )])
    .expect("canonical carrier wrapper")
    .sign(ALICE_KEYPAIR.private_key());
    signed
        .validate_signed_attachments_v1()
        .expect("canonical shape");
    let original = signed.encode_wire_v1().unwrap();
    let occurrence = &signed.attachments().unwrap().occurrences()[0].bytes;
    let original_pointer = occurrence.as_ptr();
    let original_length = occurrence.len();
    let expected = format!(
        "malformed signed attachments: {}",
        SignedAttachmentErrorV1::UnsupportedOperation { action_index: 0 }
    );
    let mut block = state.block(BlockHeader::new(
        nonzero!(2_u64),
        Some(
            state
                .view()
                .latest_block_hash()
                .expect("genuine genesis parent"),
        ),
        None,
        0,
        0,
    ));
    let mut tx = block.transaction_for_fastpq_testing(Hash::from(signed.hash_as_entrypoint()));
    tx.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
    tx.world.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
    let source_probe = signed_source_observation::Probe::new(&signed);
    let error = super::Executor::Initial
        .execute_transaction_body(&mut tx, &ALICE_ID, &signed, &mut IvmCache::new())
        .expect_err("advisory carrier cannot bypass the absent authoritative relation");
    source_probe.assert_observed_once();
    assert!(matches!(error, ValidationFail::NotPermitted(message) if message == expected));
    assert_eq!(tx.last_tx_gas_used, 0);
    assert!(!tx.is_execution_deferred());
    let retained = &signed.attachments().unwrap().occurrences()[0].bytes;
    assert_eq!(
        (retained.as_ptr(), retained.len()),
        (original_pointer, original_length)
    );
    assert_eq!(signed.encode_wire_v1().unwrap(), original);
    signed
        .verify_signature()
        .expect("unchanged source remains signed");
}

#[test]
fn borrowed_raw_ivm_keeps_original_bytecode_and_completed_work() {
    let account = Account::new(ALICE_ID.clone()).build(&ALICE_ID);
    let state = state_after_genesis(World::with([], [account], []));
    let mut program = ivm::ProgramMetadata {
        max_cycles: 100,
        ..ivm::ProgramMetadata::default()
    }
    .encode();
    program.extend_from_slice(
        &ivm::encoding::wide::encode_ri(ivm::instruction::wide::arithmetic::ADDI, 1, 0, 1)
            .to_le_bytes(),
    );
    program.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());
    let signed = TransactionBuilder::new(
        *state.network_id_ref(),
        ALICE_ID.clone(),
        FeePaymentIntent::authority(Vec::new(), core::num::NonZeroU64::new(50_000)),
    )
    .with_executable(Executable::Ivm(IvmBytecode::from_compiled(program)))
    .sign(ALICE_KEYPAIR.private_key());
    let Executable::Ivm(bytes) = signed.instructions() else {
        unreachable!()
    };
    let original_pointer = bytes.as_ref().as_ptr();
    let original = signed.encode_wire_v1().unwrap();
    let mut block = state.block(BlockHeader::new(
        nonzero!(2_u64),
        Some(
            state
                .view()
                .latest_block_hash()
                .expect("genuine genesis parent"),
        ),
        None,
        0,
        0,
    ));
    let mut tx = block.transaction_for_fastpq_testing(Hash::from(signed.hash_as_entrypoint()));
    tx.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
    tx.world.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
    let source_probe = signed_source_observation::Probe::new(&signed);
    super::Executor::Initial
        .execute_transaction_body(&mut tx, &ALICE_ID, &signed, &mut IvmCache::new())
        .expect("actual generic VM run from borrowed bytecode");
    source_probe.assert_observed_once();
    assert!(tx.last_tx_gas_used > 0);
    tx.finish_execution_effect_budget()
        .expect("complete actual VM effect budget");
    tx.close_execution_fee_meter();
    let Executable::Ivm(retained) = signed.instructions() else {
        unreachable!()
    };
    assert_eq!(retained.as_ref().as_ptr(), original_pointer);
    assert_eq!(signed.encode_wire_v1().unwrap(), original);
    signed.verify_signature().unwrap();
}

#[test]
fn rejected_standalone_ballot_records_equal_gas_for_instruction_and_batch_carriers() {
    let ballot = InstructionBox::from(iroha_data_model::isi::governance::CastZkBallot {
        election_id: "gas.parity.v1".to_owned(),
        proof_b64: "AA==".to_owned(),
        public_inputs_json: "{}".to_owned(),
    });
    let expected_gas =
        isi_gas::meter_instructions(core::slice::from_ref(&ballot), &crate::gas::DEFAULT_ZK_GAS)
            .expect("completed fixture gas quote");
    assert!(expected_gas > 0);
    let execute_rejected = |executable: Executable| {
        let account = Account::new(ALICE_ID.clone()).build(&ALICE_ID);
        let state = state_after_genesis(World::with([], [account], []));
        let signed = TransactionBuilder::new(
            *state.network_id_ref(),
            ALICE_ID.clone(),
            FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_executable(executable)
        .sign(ALICE_KEYPAIR.private_key());
        let mut block = state.block(BlockHeader::new(
            nonzero!(2_u64),
            Some(
                state
                    .view()
                    .latest_block_hash()
                    .expect("genuine genesis parent"),
            ),
            None,
            0,
            0,
        ));
        let mut transaction =
            block.transaction_for_fastpq_testing(Hash::from(signed.hash_as_entrypoint()));
        transaction.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
        transaction.world.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
        let mut cache = IvmCache::new();
        crate::executor::Executor::Initial
            .execute_transaction(&mut transaction, &ALICE_ID, &signed, &mut cache)
            .map_err(crate::execution_attempt::expect_completed_rejection)
            .expect_err("an unpermitted ballot must reject");
        transaction.last_tx_gas_used
    };
    let instructions_gas = execute_rejected(Executable::Instructions(vec![ballot.clone()].into()));
    let batch_gas = execute_rejected(Executable::Batch(
        vec![ExecutableBatchItem::Instruction(ballot)].into(),
    ));
    assert_eq!(instructions_gas, expected_gas);
    assert_eq!(batch_gas, expected_gas);
}
#[test]
fn successful_live_batch_settlement_retains_nested_execute_trigger_ivm_gas() {
    let account = Account::new(ALICE_ID.clone()).build(&ALICE_ID);
    let state = state_after_genesis(World::with([], [account], []));
    let trigger_id: TriggerId = "live_batch_ivm_gas".parse().expect("trigger id");
    let mut program = ivm::ProgramMetadata {
        max_cycles: 100,
        ..ivm::ProgramMetadata::default()
    }
    .encode();
    program.extend_from_slice(
        &ivm::encoding::wide::encode_ri(ivm::instruction::wide::arithmetic::ADDI, 1, 0, 1)
            .to_le_bytes(),
    );
    program.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());
    let trigger = Trigger::new(
        trigger_id.clone(),
        Action::new(
            Executable::Ivm(IvmBytecode::from_compiled(program)),
            Repeats::Indefinitely,
            ALICE_ID.clone(),
            ExecuteTriggerEventFilter::new()
                .for_trigger(trigger_id.clone())
                .under_authority(ALICE_ID.clone()),
        )
        .expect("valid generic IVM trigger action"),
    );
    let mut setup_block = state.block(BlockHeader::new(
        nonzero!(2_u64),
        Some(
            state
                .view()
                .latest_block_hash()
                .expect("genuine genesis parent"),
        ),
        None,
        0,
        0,
    ));
    {
        let mut setup_tx = setup_block.transaction_for_callback_testing();
        Register::trigger(trigger)
            .execute(&ALICE_ID, &mut setup_tx)
            .expect("register generic IVM trigger");
        setup_tx
            .apply_callback_for_testing()
            .expect("capture successful component callbacks");
    }
    setup_block
        .commit_world_overlay_for_testing()
        .expect("commit trigger fixture");

    let mut block = state.block(BlockHeader::new(
        nonzero!(3_u64),
        Some(
            state
                .view()
                .latest_block_hash()
                .expect("genuine genesis parent"),
        ),
        None,
        0,
        0,
    ));
    let nested_ivm_gas = {
        let mut baseline_tx = block.transaction_for_callback_testing();
        baseline_tx.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
        baseline_tx.world.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
        baseline_tx
            .execute_called_trigger(
                &trigger_id,
                &ExecuteTriggerEvent {
                    trigger_id: trigger_id.clone(),
                    authority: ALICE_ID.clone(),
                    args: Json::default(),
                },
            )
            .expect("baseline generic IVM trigger execution");
        baseline_tx.last_tx_gas_used
    };
    assert!(nested_ivm_gas > 0, "generic IVM trigger must consume gas");
    let execute_trigger = InstructionBox::from(ExecuteTrigger::new(trigger_id.clone()));
    let direct_gas = isi_gas::meter_instructions(
        core::slice::from_ref(&execute_trigger),
        &crate::gas::DEFAULT_ZK_GAS,
    )
    .expect("completed fixture gas quote");
    let transaction = TransactionBuilder::new(
        *state.network_id_ref(),
        ALICE_ID.clone(),
        FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_executable(Executable::Batch(
        vec![ExecutableBatchItem::Instruction(execute_trigger)].into(),
    ))
    .sign(ALICE_KEYPAIR.private_key());
    let mut state_tx =
        block.transaction_for_fastpq_testing(Hash::from(transaction.hash_as_entrypoint()));
    state_tx.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
    state_tx.world.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
    super::Executor::Initial
        .execute_transaction(&mut state_tx, &ALICE_ID, &transaction, &mut IvmCache::new())
        .map_err(crate::execution_attempt::expect_completed_rejection)
        .expect("live batch executes its generic IVM trigger");
    assert_eq!(
        state_tx.last_tx_gas_used,
        direct_gas.saturating_add(nested_ivm_gas),
        "fee settlement must retain nested trigger VM gas"
    );
}
#[test]
fn rejected_live_batch_retains_nested_execute_trigger_gas() {
    let account = Account::new(ALICE_ID.clone()).build(&ALICE_ID);
    let state = state_after_genesis(World::with([], [account], []));
    let trigger_id: TriggerId = "rejected_live_batch_gas".parse().expect("trigger id");
    let trigger_body = vec![
        InstructionBox::from(Log::new(Level::INFO, "meter rejected trigger".to_owned())),
        InstructionBox::from(Unregister::domain(
            DomainId::try_new("missing", "universal").expect("missing domain id"),
        )),
    ];
    let trigger = Trigger::new(
        trigger_id.clone(),
        Action::new(
            trigger_body.clone(),
            Repeats::Indefinitely,
            ALICE_ID.clone(),
            ExecuteTriggerEventFilter::new()
                .for_trigger(trigger_id.clone())
                .under_authority(ALICE_ID.clone()),
        )
        .expect("valid failing trigger action"),
    );
    let mut block = state.block(BlockHeader::new(
        nonzero!(2_u64),
        Some(
            state
                .view()
                .latest_block_hash()
                .expect("genuine genesis parent"),
        ),
        None,
        0,
        0,
    ));
    let execute_trigger = InstructionBox::from(ExecuteTrigger::new(trigger_id));
    let direct_gas = isi_gas::meter_instructions(
        core::slice::from_ref(&execute_trigger),
        &crate::gas::DEFAULT_ZK_GAS,
    )
    .expect("completed fixture gas quote");
    let nested_gas = isi_gas::meter_instructions(&trigger_body, &crate::gas::DEFAULT_ZK_GAS)
        .expect("completed fixture gas quote");
    let transaction = TransactionBuilder::new(
        *state.network_id_ref(),
        ALICE_ID.clone(),
        FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_executable(Executable::Batch(
        vec![ExecutableBatchItem::Instruction(execute_trigger)].into(),
    ))
    .sign(ALICE_KEYPAIR.private_key());
    let mut state_tx =
        block.transaction_for_fastpq_testing(Hash::from(transaction.hash_as_entrypoint()));
    state_tx.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
    state_tx.world.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
    Register::trigger(trigger)
        .execute(&ALICE_ID, &mut state_tx)
        .expect("register failing trigger");
    super::Executor::Initial
        .execute_transaction(&mut state_tx, &ALICE_ID, &transaction, &mut IvmCache::new())
        .map_err(crate::execution_attempt::expect_completed_rejection)
        .expect_err("failing trigger rejects its enclosing live batch");
    assert_eq!(
        state_tx.last_tx_gas_used,
        direct_gas.saturating_add(nested_gas),
        "rejected live execution must retain authored and nested trigger gas"
    );
}
fn seed_test_asset_supply(world: &mut World, asset_definition_id: &AssetDefinitionId) {
    let total = world
        .assets
        .view()
        .iter()
        .filter(|(asset_id, _)| asset_id.definition() == asset_definition_id)
        .try_fold(Quantity::zero(), |total, (_, value)| {
            total.checked_add(value.as_ref())
        })
        .expect("fixture asset supply must add exactly");
    let mut definition = world
        .asset_definitions
        .view()
        .get(asset_definition_id)
        .cloned()
        .expect("fixture asset definition exists");
    definition.total_quantity = total;
    world
        .asset_definitions
        .insert(asset_definition_id.clone(), definition);
}
struct InitialBatchFixture {
    world: World,
    source: AccountId,
    foreign_source: AccountId,
    delegate: AccountId,
    first_destination: AccountId,
    second_destination: AccountId,
    asset_definition: AssetDefinitionId,
    source_asset: AssetId,
    foreign_source_asset: AssetId,
}
fn initial_batch_fixture() -> InitialBatchFixture {
    let source = checked_account_id();
    let foreign_source = checked_account_id();
    let delegate = checked_account_id();
    let first_destination = checked_account_id();
    let second_destination = checked_account_id();
    let domain_id = DomainId::try_new("batch_assets", "universal").expect("domain id");
    let asset_definition = AssetDefinitionId::derive_from_components(
        domain_id.clone(),
        "coin".parse().expect("asset name"),
    );
    let source_asset = AssetId::new(asset_definition.clone(), source.clone());
    let foreign_source_asset = AssetId::new(asset_definition.clone(), foreign_source.clone());
    let mut world = World::with_assets(
        [Domain::new(domain_id).build(&source)],
        [
            Account::new(source.clone()).build(&source),
            Account::new(foreign_source.clone()).build(&foreign_source),
            Account::new(delegate.clone()).build(&delegate),
            Account::new(first_destination.clone()).build(&first_destination),
            Account::new(second_destination.clone()).build(&second_destination),
        ],
        [AssetDefinition::numeric(
            asset_definition.clone(),
            "batch coin".to_owned(),
            iroha_data_model::asset::AssetBalancePolicy::Global,
            None,
        )
        .build(&source)],
        [
            Asset::new(source_asset.clone(), Quantity::from(20_u32)),
            Asset::new(foreign_source_asset.clone(), Quantity::from(20_u32)),
        ],
        [],
    );
    seed_test_asset_supply(&mut world, &asset_definition);
    InitialBatchFixture {
        world,
        source,
        foreign_source,
        delegate,
        first_destination,
        second_destination,
        asset_definition,
        source_asset,
        foreign_source_asset,
    }
}
fn initial_batch_balance(world: &impl WorldReadOnly, asset_id: &AssetId) -> Quantity {
    world.asset(asset_id).map_or_else(
        |_| Quantity::zero(),
        |asset| asset.value().clone().into_inner(),
    )
}
fn checked_keypair_with_algorithm(algorithm: Algorithm) -> KeyPair {
    KeyPair::try_random_with_algorithm(algorithm)
        .expect("executor algorithm-specific fixture key generation should succeed")
}
fn checked_account_id() -> AccountId {
    AccountId::new(checked_keypair().public_key().clone())
}
#[test]
fn native_escrow_scope_binds_current_owner_incarnation_and_archived_request() {
    use iroha_data_model::{
        nexus::AxtAssetIncarnationV1,
        query::{asset_escrow_scope::FindAssetEscrowsByAssetDefinitionV1, parameters::QueryParams},
    };
    let fixture = initial_batch_fixture();
    let owner = fixture.source.clone();
    let outsider = fixture.delegate.clone();
    let definition = fixture.asset_definition.clone();
    let incarnation = AxtAssetIncarnationV1::try_from_bytes(
        *iroha_crypto::Hash::new("scope-owner-test").as_ref(),
    )
    .unwrap();
    let state = state_for_testing(fixture.world);
    let mut block = state.block(BlockHeader::new(nonzero!(2_u64), None, None, 0, 0));
    let mut tx = block.transaction();
    tx.world
        .axt_asset_incarnations
        .insert(definition.clone(), incarnation);
    tx.world.account_permissions.insert(
        outsider.clone(),
        BTreeSet::from([
            executor_permission::query::CanReadAllLedgerData.into(),
            executor_permission::query::CanReadAccountData {
                account: owner.clone(),
            }
            .into(),
        ]),
    );
    let request = QueryRequest::Start(QueryWithParams {
        query: (),
        query_payload: FindAssetEscrowsByAssetDefinitionV1 {
            asset_definition_id: definition.clone(),
            expected_incarnation: incarnation,
        }
        .encode(),
        item: QueryItemKind::AssetEscrowScopeOriginalV1,
        predicate_bytes: Vec::new(),
        selector_bytes: Vec::new(),
        params: QueryParams::default(),
    });
    validate_builtin_native_query_permission(tx.world(), &owner, &request, 0).unwrap();
    assert!(
        validate_builtin_native_query_permission(tx.world(), &outsider, &request, 0).is_err(),
        "global read cannot substitute for exact ownership"
    );
    tx.world
        .asset_definitions
        .get_mut(&definition)
        .unwrap()
        .set_owned_by(outsider.clone());
    assert!(
        validate_builtin_native_query_permission(tx.world(), &owner, &request, 0).is_err(),
        "an archived request loses authorization when ownership changes"
    );
    validate_builtin_native_query_permission(tx.world(), &outsider, &request, 0).unwrap();
    tx.world.axt_asset_incarnations.insert(
        definition,
        AxtAssetIncarnationV1::try_from_bytes(*iroha_crypto::Hash::new("new-incarnation").as_ref())
            .unwrap(),
    );
    assert!(
        validate_builtin_native_query_permission(tx.world(), &outsider, &request, 0).is_err(),
        "re-registration cannot revive an old selected scope"
    );
}

#[test]
fn native_uid_queries_require_exact_asset_owner_even_with_global_read() {
    use iroha_data_model::query::{asset_uid::FindAssetUidPoliciesV1, parameters::QueryParams};
    let fixture = initial_batch_fixture();
    let owner = fixture.source.clone();
    let participant = fixture.delegate.clone();
    let definition = fixture.asset_definition.clone();
    let state = state_for_testing(fixture.world);
    let mut block = state.block(BlockHeader::new(nonzero!(2_u64), None, None, 0, 0));
    let mut tx = block.transaction();
    for (uid, wallet_account_id) in [
        (None, None),
        (Some([7; 32]), None),
        (Some([7; 32]), Some(participant.clone())),
    ] {
        let envelope = QueryWithParams {
            query: (),
            query_payload: FindAssetUidPoliciesV1 {
                asset_definition_id: definition.clone(),
                uid,
                wallet_account_id,
            }
            .encode(),
            item: QueryItemKind::AssetUidPolicyViewV1,
            predicate_bytes: Vec::new(),
            selector_bytes: Vec::new(),
            params: QueryParams::default(),
        };
        let request = QueryRequest::Start(envelope);
        validate_builtin_native_query_permission(tx.world(), &owner, &request, 0)
            .expect("exact asset owner may read UID policy");
        assert!(
            validate_builtin_native_query_permission(tx.world(), &participant, &request, 0)
                .is_err()
        );
        tx.world.account_permissions.insert(
            participant.clone(),
            BTreeSet::from([
                executor_permission::query::CanReadAllLedgerData.into(),
                executor_permission::query::CanReadAccountData {
                    account: owner.clone(),
                }
                .into(),
            ]),
        );
        assert!(
            validate_builtin_native_query_permission(tx.world(), &participant, &request, 0)
                .is_err(),
            "general read grants must not reveal cross-participant UID linkage"
        );
        tx.world.account_permissions.remove(participant.clone());
    }
}
#[test]
fn native_fi_context_query_is_exact_current_asset_owner_only() {
    use iroha_data_model::{
        asset::fi_context::AssetFiContextTargetV1,
        isi::asset_fi::TransferToLinkedFiV1,
        query::{asset_fi_context::FindAssetFiContextV1, parameters::QueryParams},
    };
    let fixture = initial_batch_fixture();
    let owner = fixture.source.clone();
    let wallet = fixture.first_destination.clone();
    let outsider = fixture.delegate.clone();
    let definition = fixture.asset_definition.clone();
    let state = state_for_testing(fixture.world);
    let mut block = state.block(BlockHeader::new(nonzero!(2_u64), None, None, 0, 0));
    let mut tx = block.transaction();
    tx.world.account_permissions.insert(
        outsider.clone(),
        BTreeSet::from([
            executor_permission::query::CanReadAllLedgerData.into(),
            executor_permission::query::CanReadAccountData {
                account: wallet.clone(),
            }
            .into(),
        ]),
    );
    let query = FindAssetFiContextV1 {
        target: AssetFiContextTargetV1::Transfer {
            instruction: TransferToLinkedFiV1 {
                asset_definition_id: definition.clone(),
                account_id: wallet.clone(),
                amount: 5_u32.into(),
                expected_link_revision: 1,
                expected_policy_revision: 1,
            },
            fi_account_id: outsider.clone(),
            reference_digest: [9; 32],
            expected_fi_revision: 1,
            expected_attestation_revision: 1,
        },
        expected_incarnation: iroha_data_model::nexus::AxtAssetIncarnationV1::try_from_bytes(
            *Hash::new("context-incarnation").as_ref(),
        )
        .unwrap(),
        expected_uid: [7; 32],
    };
    let request = QueryRequest::Start(QueryWithParams {
        query: (),
        query_payload: query.encode(),
        item: QueryItemKind::AssetFiContextSnapshotV1,
        predicate_bytes: Vec::new(),
        selector_bytes: Vec::new(),
        params: QueryParams::default(),
    });
    validate_builtin_native_query_permission(tx.world(), &owner, &request, 0).unwrap();
    for denied in [&wallet, &outsider] {
        assert!(
            validate_builtin_native_query_permission(tx.world(), denied, &request, 0).is_err(),
            "wallet/FI/general grants do not confer current-context authority"
        );
    }
    tx.world
        .asset_definitions
        .get_mut(&definition)
        .unwrap()
        .set_owned_by(outsider.clone());
    assert!(validate_builtin_native_query_permission(tx.world(), &owner, &request, 0).is_err());
    validate_builtin_native_query_permission(tx.world(), &outsider, &request, 0).unwrap();
    let QueryRequest::Start(mut malformed) = request else {
        unreachable!()
    };
    malformed.query_payload.push(0);
    assert!(
        validate_builtin_native_query_permission(
            tx.world(),
            &outsider,
            &QueryRequest::Start(malformed),
            0
        )
        .is_err()
    );
}

#[test]
fn native_fi_receipt_query_is_exact_wallet_or_current_global_asset_owner_only() {
    use iroha_data_model::query::{
        asset_fi::FindAssetFiTransferReceiptV1, parameters::QueryParams,
    };
    let fixture = initial_batch_fixture();
    let owner = fixture.source.clone();
    let wallet = fixture.first_destination.clone();
    let outsider = fixture.delegate.clone();
    let definition = fixture.asset_definition.clone();
    let state = state_for_testing(fixture.world);
    let mut block = state.block(BlockHeader::new(nonzero!(2_u64), None, None, 0, 0));
    let mut tx = block.transaction();
    tx.world.account_permissions.insert(
        outsider.clone(),
        BTreeSet::from([
            executor_permission::query::CanReadAllLedgerData.into(),
            executor_permission::query::CanReadAccountData {
                account: wallet.clone(),
            }
            .into(),
        ]),
    );
    let mut query = FindAssetFiTransferReceiptV1 {
        asset_definition_id: definition.clone(),
        account_id: wallet.clone(),
        expected_incarnation: iroha_data_model::nexus::AxtAssetIncarnationV1::try_from_bytes(
            *Hash::new("receipt-incarnation").as_ref(),
        )
        .unwrap(),
        execution_call_hash: Hash::new("private-FI-occurrence"),
        transcript_index: 0,
    };
    let request = |query: &FindAssetFiTransferReceiptV1| {
        QueryRequest::Start(QueryWithParams {
            query: (),
            query_payload: query.encode(),
            item: QueryItemKind::AssetFiTransferReceiptV1,
            predicate_bytes: Vec::new(),
            selector_bytes: Vec::new(),
            params: QueryParams::default(),
        })
    };
    validate_builtin_native_query_permission(tx.world(), &owner, &request(&query), 0).unwrap();
    validate_builtin_native_query_permission(tx.world(), &wallet, &request(&query), 0).unwrap();
    assert!(
        validate_builtin_native_query_permission(tx.world(), &outsider, &request(&query), 0)
            .is_err(),
        "global/account grants do not reveal receipts"
    );
    tx.world
        .asset_definitions
        .get_mut(&definition)
        .unwrap()
        .set_owned_by(outsider.clone());
    assert!(
        validate_builtin_native_query_permission(tx.world(), &owner, &request(&query), 0).is_err(),
        "archived requests lose former-owner authorization"
    );
    validate_builtin_native_query_permission(tx.world(), &outsider, &request(&query), 0).unwrap();
    tx.world.axt_asset_incarnations.insert(
        definition.clone(),
        iroha_data_model::nexus::AxtAssetIncarnationV1::try_from_bytes(
            *Hash::new("later-live-incarnation").as_ref(),
        )
        .unwrap(),
    );
    validate_builtin_native_query_permission(tx.world(), &wallet, &request(&query), 0).unwrap();
    validate_builtin_native_query_permission(tx.world(), &outsider, &request(&query), 0).unwrap();
    query.account_id = owner.clone();
    assert!(
        validate_builtin_native_query_permission(tx.world(), &wallet, &request(&query), 0).is_err(),
        "wallet cannot select another source wallet"
    );
    let QueryRequest::Start(mut malformed) = request(&query) else {
        unreachable!()
    };
    malformed.query_payload.push(0);
    assert!(
        validate_builtin_native_query_permission(
            tx.world(),
            &outsider,
            &QueryRequest::Start(malformed),
            0
        )
        .is_err(),
        "authorization requires the exact complete payload"
    );
}

#[test]
fn native_fi_relationship_and_enrollment_queries_keep_exact_owner_boundaries() {
    use iroha_data_model::query::{
        asset_fi::FindAssetFiLinkV1, asset_uid_enrollment::FindAssetUidEnrollmentV1,
        parameters::QueryParams,
    };
    let fixture = initial_batch_fixture();
    let owner = fixture.source.clone();
    let wallet = fixture.first_destination.clone();
    let outsider = fixture.delegate.clone();
    let definition = fixture.asset_definition.clone();
    let state = state_for_testing(fixture.world);
    let mut block = state.block(BlockHeader::new(nonzero!(2_u64), None, None, 0, 0));
    let mut tx = block.transaction();
    tx.world.account_permissions.insert(
        outsider.clone(),
        BTreeSet::from([
            executor_permission::query::CanReadAllLedgerData.into(),
            executor_permission::query::CanReadAccountData {
                account: wallet.clone(),
            }
            .into(),
        ]),
    );
    let start = |item, query_payload| {
        QueryRequest::Start(QueryWithParams {
            query: (),
            query_payload,
            item,
            predicate_bytes: Vec::new(),
            selector_bytes: Vec::new(),
            params: QueryParams::default(),
        })
    };
    let request = start(
        QueryItemKind::AssetFiRelationshipViewV1,
        FindAssetFiLinkV1 {
            asset_definition_id: definition.clone(),
            account_id: wallet.clone(),
            fi_account_id: owner.clone(),
            reference_digest: [19; 32],
        }
        .encode(),
    );
    validate_builtin_native_query_permission(tx.world(), &owner, &request, 0).unwrap();
    validate_builtin_native_query_permission(tx.world(), &wallet, &request, 0).unwrap();
    validate_builtin_native_query_permission(tx.world(), &outsider, &request, 0)
        .expect_err("public/global/account read must not reveal private FI relationships");
    let request = start(
        QueryItemKind::AssetUidEnrollmentViewV1,
        FindAssetUidEnrollmentV1 {
            asset_definition_id: definition,
        }
        .encode(),
    );
    validate_builtin_native_query_permission(tx.world(), &owner, &request, 0).unwrap();
    validate_builtin_native_query_permission(tx.world(), &wallet, &request, 0)
        .expect_err("activation is an owner observation");
    validate_builtin_native_query_permission(tx.world(), &outsider, &request, 0)
        .expect_err("global read does not grant enrollment administration");
}
#[test]
fn native_settlement_receipt_query_uses_exact_payer_read_access() {
    let fixture = initial_batch_fixture();
    let query = SingularQueryBox::from(
        iroha_data_model::query::settlement::FindSettlementReceiptById::new(
            iroha_data_model::isi::SettlementId::new(
                fixture.source.clone(),
                "business_receipt".parse().expect("settlement reference"),
            ),
        ),
    );
    assert_eq!(
        native_singular_query_access(&query),
        NativeQueryAccess::Account(fixture.source.clone())
    );
    let payer = fixture.source.clone();
    let authority = fixture.delegate.clone();
    let foreign_payer = fixture.foreign_source.clone();
    let state = state_for_testing(fixture.world);
    let mut block = state.block(BlockHeader::new(nonzero!(2_u64), None, None, 0, 0));
    let mut tx = block.transaction();
    let request = QueryRequest::Singular(query);
    validate_builtin_native_query_permission(tx.world(), &payer, &request, 0)
        .expect("payer can read its own receipt without a ledger-wide grant");
    assert!(validate_builtin_native_query_permission(tx.world(), &authority, &request, 0).is_err());
    tx.world.account_permissions.insert(
        authority.clone(),
        BTreeSet::from([executor_permission::query::CanReadAccountData {
            account: authority.clone(),
        }
        .into()]),
    );
    assert!(validate_builtin_native_query_permission(tx.world(), &authority, &request, 0).is_err());
    tx.world.account_permissions.insert(
        authority.clone(),
        BTreeSet::from([executor_permission::query::CanReadAccountData {
            account: foreign_payer,
        }
        .into()]),
    );
    assert!(validate_builtin_native_query_permission(tx.world(), &authority, &request, 0).is_err());
    tx.world.account_permissions.insert(
        authority.clone(),
        BTreeSet::from([executor_permission::query::CanReadAccountData { account: payer }.into()]),
    );
    validate_builtin_native_query_permission(tx.world(), &authority, &request, 0)
        .expect("reader has the exact payer-scoped grant");
    tx.world.account_permissions.remove(authority.clone());
    assert!(validate_builtin_native_query_permission(tx.world(), &authority, &request, 0).is_err());
    tx.world.account_permissions.insert(
        authority.clone(),
        BTreeSet::from([executor_permission::query::CanReadAllLedgerData.into()]),
    );
    validate_builtin_native_query_permission(tx.world(), &authority, &request, 0)
        .expect("ledger-wide receipt reader");
    tx.world.account_permissions.remove(authority.clone());
    assert!(validate_builtin_native_query_permission(tx.world(), &authority, &request, 0).is_err());
}
#[test]
fn native_sorafs_citizen_bond_queries_require_all_ledger_access() {
    use iroha_data_model::query::sorafs::prelude::{
        FindSorafsCitizenBondBySerialCommitment, FindSorafsCitizenBondSnapshot,
    };

    let queries: [SingularQueryBox; 2] = [
        FindSorafsCitizenBondBySerialCommitment::new([0x11; 32]).into(),
        FindSorafsCitizenBondSnapshot.into(),
    ];
    for query in &queries {
        assert_eq!(
            native_singular_query_access(query),
            NativeQueryAccess::AllLedger
        );
    }
}
#[test]
fn asset_definition_listing_requires_all_ledger_access() {
    use iroha_data_model::query::{
        asset::prelude::{
            FindAssetDefinitionById, FindAssetDefinitionDirectHome, FindAssetDefinitions,
        },
        parameters::QueryParams,
    };
    let envelope = QueryWithParams {
        query: (),
        query_payload: FindAssetDefinitions.encode(),
        item: QueryItemKind::AssetDefinition,
        predicate_bytes: Vec::new(),
        selector_bytes: Vec::new(),
        params: QueryParams::default(),
    };
    assert_eq!(
        native_iterable_query_access(&envelope).expect("authorize definition listing"),
        NativeQueryAccess::AllLedger
    );
    let id = iroha_data_model::asset::AssetDefinitionId::derive_from_components(
        iroha_model_base::domain::DomainId::try_new("homes", "universal").unwrap(),
        "coin".parse().unwrap(),
    );
    for query in [
        SingularQueryBox::from(FindAssetDefinitionById::new(id.clone())),
        SingularQueryBox::from(FindAssetDefinitionDirectHome::new(id)),
    ] {
        assert_eq!(
            native_singular_query_access(&query),
            NativeQueryAccess::Registered
        );
    }
}
#[test]
fn native_escrow_query_authorization_uses_query_specific_tags() {
    use iroha_data_model::{
        escrow::AssetEscrowStatus,
        query::{
            escrow::prelude::{
                FindAssetEscrowsByBuyer, FindAssetEscrowsBySeller, FindAssetEscrowsByStatus,
            },
            parameters::QueryParams,
        },
    };
    let seller = checked_account_id();
    let buyer = checked_account_id();
    let seller_query = FindAssetEscrowsBySeller {
        seller: seller.clone(),
    };
    let buyer_query = FindAssetEscrowsByBuyer {
        buyer: buyer.clone(),
    };
    let status_query = FindAssetEscrowsByStatus {
        status: AssetEscrowStatus::Accepted,
    };
    let seller_envelope = QueryWithParams {
        query: (),
        query_payload: seller_query.encode(),
        item: QueryItemKind::AssetEscrowsBySeller,
        predicate_bytes: Vec::new(),
        selector_bytes: Vec::new(),
        params: QueryParams::default(),
    };
    assert_eq!(
        native_iterable_query_access(&seller_envelope).expect("authorize seller query"),
        NativeQueryAccess::Account(seller)
    );
    let buyer_envelope = QueryWithParams {
        query: (),
        query_payload: buyer_query.encode(),
        item: QueryItemKind::AssetEscrowsByBuyer,
        predicate_bytes: Vec::new(),
        selector_bytes: Vec::new(),
        params: QueryParams::default(),
    };
    assert_eq!(
        native_iterable_query_access(&buyer_envelope).expect("authorize buyer query"),
        NativeQueryAccess::Account(buyer)
    );
    let status_envelope = QueryWithParams {
        query: (),
        query_payload: status_query.encode(),
        item: QueryItemKind::AssetEscrowsByStatus,
        predicate_bytes: Vec::new(),
        selector_bytes: Vec::new(),
        params: QueryParams::default(),
    };
    assert_eq!(
        native_iterable_query_access(&status_envelope).expect("authorize status query"),
        NativeQueryAccess::AllLedger
    );
}
#[test]
fn native_escrow_query_authorization_rejects_wrong_or_malformed_tags() {
    use iroha_data_model::query::{
        escrow::prelude::FindAssetEscrowsBySeller, parameters::QueryParams,
    };
    let seller_query = FindAssetEscrowsBySeller {
        seller: checked_account_id(),
    };
    let legacy_item_tag = QueryWithParams {
        query: (),
        query_payload: seller_query.encode(),
        item: QueryItemKind::AssetEscrowRecord,
        predicate_bytes: Vec::new(),
        selector_bytes: Vec::new(),
        params: QueryParams::default(),
    };
    assert!(native_iterable_query_access(&legacy_item_tag).is_err());
    let status_tag_with_account_payload = QueryWithParams {
        query: (),
        query_payload: seller_query.encode(),
        item: QueryItemKind::AssetEscrowsByStatus,
        predicate_bytes: Vec::new(),
        selector_bytes: Vec::new(),
        params: QueryParams::default(),
    };
    assert!(native_iterable_query_access(&status_tag_with_account_payload).is_err());
    let malformed_seller = QueryWithParams {
        query: (),
        query_payload: vec![0xff],
        item: QueryItemKind::AssetEscrowsBySeller,
        predicate_bytes: Vec::new(),
        selector_bytes: Vec::new(),
        params: QueryParams::default(),
    };
    assert!(native_iterable_query_access(&malformed_seller).is_err());
}
#[test]
fn fee_sponsor_operations_preserve_every_mixed_batch_item() {
    let authority = checked_account_id();
    let contract_address = ContractAddress::derive(
        &"hash:0000000000000000000000000000000000000000000000000000000000000001#C50E"
            .parse()
            .expect("canonical test network id"),
        &authority,
        31,
        DataSpaceId::UNIVERSAL,
    )
    .expect("derive contract address");
    let expected_code_hash = Hash::new(b"sponsored-mixed-batch");
    let executable = Executable::Batch(
        vec![
            ExecutableBatchItem::Instruction(InstructionBox::from(Log::new(
                Level::INFO,
                "sponsored instruction".to_owned(),
            ))),
            ExecutableBatchItem::ContractCall(ContractInvocation {
                contract_address: contract_address.clone(),
                expected_code_hash,
                entrypoint: "main".to_owned(),
                arguments: None,
            }),
        ]
        .into(),
    );
    let operations = fee_sponsor_operations(&executable).expect("resolve sponsor operations");
    assert_eq!(operations.len(), 2);
    assert!(matches!(
        &operations[0],
        FeeSponsorOperation::NativeInstruction { .. }
    ));
    assert!(matches!(
        &operations[1],
        FeeSponsorOperation::ContractCall {
            contract_address: seen_address,
            code_hash: seen_hash,
            entrypoint,
        } if seen_address == &contract_address
            && seen_hash == &expected_code_hash
            && entrypoint == "main"
    ));
}
#[test]
fn fee_sponsor_multisig_operations_decode_variant_and_exact_target_account() {
    let target = checked_account_id();
    let signatory = checked_account_id();
    let proposed_instructions = vec![InstructionBox::from(Log::new(
        Level::INFO,
        "multisig proposal".to_owned(),
    ))];
    let instructions_hash = HashOf::new(&proposed_instructions);
    let nonce = MultisigProposalNonce::new([7; 32]).unwrap();
    let proposal_id = MultisigProposalId::derive(
        &executor_test_network_id(b"fee-sponsor-occurrence"),
        &target,
        &nonce,
    );
    let spec = MultisigSpec::new(
        BTreeMap::from([(signatory, 1)]),
        nonzero!(1_u16),
        nonzero!(60_000_u64),
    );
    let instructions = [
        InstructionBox::from(MultisigPropose::new(
            target.clone(),
            nonce,
            proposed_instructions,
            None,
        )),
        InstructionBox::from(MultisigApprove::new(
            target.clone(),
            proposal_id,
            instructions_hash,
        )),
        InstructionBox::from(MultisigCancel::new(
            target.clone(),
            proposal_id,
            instructions_hash,
        )),
        InstructionBox::from(MultisigRegister::new(target.clone(), None, spec)),
    ];
    let expected = [
        FeeSponsorMultisigOperation::Propose,
        FeeSponsorMultisigOperation::Approve,
        FeeSponsorMultisigOperation::Cancel,
        FeeSponsorMultisigOperation::Register,
    ];
    for (instruction, expected_operation) in instructions.iter().zip(expected) {
        assert_eq!(
            fee_sponsor_instruction_operation(instruction)
                .expect("decode exact multisig sponsor operation"),
            FeeSponsorOperation::Multisig {
                operation: expected_operation,
                account_id: target.clone(),
            }
        );
    }
}
#[test]
fn fee_sponsor_multisig_selector_is_exact_and_register_is_opt_in() {
    let target = checked_account_id();
    let other_target = checked_account_id();
    let propose_instruction = InstructionBox::from(MultisigPropose::new(
        target.clone(),
        MultisigProposalNonce::new([7; 32]).unwrap(),
        Vec::new(),
        None,
    ));
    assert_eq!(
        iroha_data_model::isi::instruction_wire_id(&propose_instruction),
        Some("iroha.custom"),
        "regression fixture must exercise a custom multisig instruction"
    );
    let propose = fee_sponsor_instruction_operation(&propose_instruction)
        .expect("decode propose sponsor operation");
    let approve = FeeSponsorOperation::Multisig {
        operation: FeeSponsorMultisigOperation::Approve,
        account_id: target.clone(),
    };
    let cancel = FeeSponsorOperation::Multisig {
        operation: FeeSponsorMultisigOperation::Cancel,
        account_id: target.clone(),
    };
    let register = FeeSponsorOperation::Multisig {
        operation: FeeSponsorMultisigOperation::Register,
        account_id: target.clone(),
    };
    let other_account = FeeSponsorOperation::Multisig {
        operation: FeeSponsorMultisigOperation::Propose,
        account_id: other_target,
    };
    let mut selector = iroha_data_model::nexus::FeeSponsorMultisigSelector {
        operations: vec![
            FeeSponsorMultisigOperation::Propose,
            FeeSponsorMultisigOperation::Approve,
            FeeSponsorMultisigOperation::Cancel,
        ],
        account_ids: vec![target],
    };
    let selector_box = FeeSponsorRuleSelector::Multisig(selector.clone());
    assert!(fee_sponsor_selector_matches_operation(
        &selector_box,
        &propose
    ));
    assert!(fee_sponsor_selector_matches_operation(
        &selector_box,
        &approve
    ));
    assert!(fee_sponsor_selector_matches_operation(
        &selector_box,
        &cancel
    ));
    assert!(!fee_sponsor_selector_matches_operation(
        &selector_box,
        &register
    ));
    assert!(!fee_sponsor_selector_matches_operation(
        &selector_box,
        &other_account
    ));
    let broad_custom = FeeSponsorRuleSelector::NativeInstruction(
        iroha_data_model::nexus::FeeSponsorNativeInstructionSelector {
            wire_id: "iroha.custom".to_owned(),
            asset_definition_id: None,
        },
    );
    assert!(
        !fee_sponsor_selector_matches_operation(&broad_custom, &propose),
        "decoded multisig payloads must not fall back to broad iroha.custom sponsorship"
    );
    selector
        .operations
        .push(FeeSponsorMultisigOperation::Register);
    assert!(fee_sponsor_selector_matches_operation(
        &FeeSponsorRuleSelector::Multisig(selector),
        &register
    ));
}
include!("executor_dpn_domain_enrollment_tests.rs");
include!("executor_account_lineage_tests.rs");
include!("executor_sns_attempt_tests.rs");
include!("executor_raw_ivm_work_tests.rs");
include!("executor_effect_budget_tests.rs");
include!("executor_sorafs_repair_tests.rs");
include!("executor_stream_token_custody_permission_tests.rs");
include!("executor_cross_scope_permission_tests.rs");
include!("executor_stream_token_direct_source_tests.rs");
include!("executor_stream_token_gateway_direct_source_tests.rs");
include!("executor_stream_token_gateway_permission_tests.rs");
include!("executor_sorafs_market_tests.rs");
include!("executor_sorafs_provider_governance_tests.rs");
include!("executor_sorafs_pop_registry_tests.rs");
include!("executor_asset_lock_admission_tests.rs");
include!("executor_conditional_escrow_repo_admission_tests.rs");
include!("executor_bond_admission_tests.rs");
macro_rules! concrete_instruction_box {
    ($instruction_ty:ty, $instruction:expr) => {{
        const TEST_WIRE_ID: &str = "iroha.test.concrete_instruction.v1";
        let instruction: $instruction_ty = $instruction;
        let registry = iroha_data_model::isi::InstructionRegistry::new()
            .register_with_id::<$instruction_ty>(TEST_WIRE_ID);
        let (payload, flags) = norito::codec::encode_with_header_flags(&instruction);
        let framed = norito::core::frame_bare_with_header_flags::<$instruction_ty>(&payload, flags)
            .expect("frame concrete instruction");
        let decoded = registry
            .decode(TEST_WIRE_ID, &framed)
            .expect("concrete instruction type is registered")
            .expect("decode concrete instruction");
        assert!(
            decoded.as_any().is::<$instruction_ty>(),
            "test fixture must preserve the concrete instruction dynamic shape",
        );
        decoded
    }};
}
include!("executor_contract_deployment_tests.rs");
include!("executor_contract_owner_permission_tests.rs");
#[test]
fn initial_executor_genesis_rejects_unclassified_oracle_instruction() {
    let keypair = checked_keypair();
    let authority = AccountId::new(keypair.public_key().clone());
    let account = Account::new(authority.clone()).build(&authority);
    let instruction: InstructionBox = iroha_data_model::isi::oracle::AggregateOracleFeed {
        feed_id: "genesis_oracle".parse().expect("feed id"),
        slot: 0,
        request_hash: Hash::new(b"genesis oracle request"),
        evidence_hashes: Vec::new(),
    }
    .into();
    // This negative source is signed by the maintained builder; its actual Oracle
    // input must be refused by Initial before this candidate could publish genesis.
    let chain_id = ChainId::from("genesis-oracle-native-refusal");
    let source = crate::sumeragi::test_chain::signed_genesis_fixture(
        &chain_id,
        &keypair,
        &crate::sumeragi::test_chain::fixture_validators(),
        vec![instruction.clone()],
        0,
        iroha_data_model::parameter::system::ConsensusMode::Permissioned,
        None,
    )
    .expect("build actual signed Oracle negative input");
    let state = State::new_with_chain_and_network_id_for_testing(
        World::with(
            [Domain::new(iroha_genesis::GENESIS_DOMAIN_ID.clone()).build(&authority)],
            [account],
            [],
        ),
        Kura::blank_kura_for_testing(),
        query::store::LiveQueryStore::start_test(),
        chain_id,
        NetworkId::from_genesis_hash(source.hash()),
    );
    let index = source.external_transactions().position(|signed| matches!(signed.instructions(),
            Executable::Instructions(instructions) if instructions.iter().eq(core::iter::once(&instruction))
        )).expect("source contains exactly the tested Oracle instruction");
    let mut block = state
        .block_with_pristine_carrier_stage(&source, |_| Ok::<_, String>(()))
        .expect("capture original Oracle carrier before effects");
    let mut state_transaction = block
        .transaction_for_original_genesis_testing(&source, index, &authority, &instruction)
        .expect("authenticate the exact original Oracle bootstrap input");
    assert!(
        state_transaction._curr_block.is_genesis() && state_transaction.block_hashes.is_empty(),
        "the regression must exercise the authenticated genesis context"
    );
    let error = super::Executor::Initial
        .execute_instruction(&mut state_transaction, &authority, instruction)
        .expect_err("genesis must not bypass the native instruction allowlist");
    assert!(
        matches!(&error, ValidationFail::NotPermitted(reason)
            if reason.contains("does not admit unclassified native instruction")),
        "{error:?}"
    );
    drop(state_transaction);
    assert_eq!(block.committed_fragment_count(), 0);
    assert_eq!(state.committed_height(), 0);
}
#[test]
fn initial_executor_admits_the_complete_kaigi_surface() {
    use iroha_data_model::{
        isi::kaigi::{
            CreateKaigi, EndKaigi, JoinKaigi, LeaveKaigi, RecordKaigiUsage, RegisterKaigiRelay,
            ReportKaigiRelayHealth, SetKaigiRelayManifest, UnregisterKaigiRelay,
        },
        kaigi::{KaigiId, KaigiRelayHealthStatus, KaigiRelayRegistration, NewKaigi},
    };

    let host = checked_account_id();
    let participant = checked_account_id();
    let relay = checked_account_id();
    let domain_id = DomainId::try_new("kaigi", "universal").expect("domain id");
    let call_id = KaigiId::new(
        domain_id.clone(),
        Name::from_str("initial-admission").expect("call name"),
    );
    let instructions: Vec<InstructionBox> = vec![
        CreateKaigi {
            call: NewKaigi::with_defaults(call_id.clone(), host.clone()),
            commitment: None,
            nullifier: None,
            roster_root: None,
            proof: None,
        }
        .into(),
        JoinKaigi {
            call_id: call_id.clone(),
            participant: participant.clone(),
            commitment: None,
            nullifier: None,
            roster_root: None,
            proof: None,
        }
        .into(),
        LeaveKaigi {
            call_id: call_id.clone(),
            participant,
            commitment: None,
            nullifier: None,
            roster_root: None,
            proof: None,
        }
        .into(),
        EndKaigi {
            call_id: call_id.clone(),
            ended_at_ms: None,
            commitment: None,
            nullifier: None,
            roster_root: None,
            proof: None,
        }
        .into(),
        RecordKaigiUsage {
            call_id: call_id.clone(),
            duration_ms: 1,
            billed_gas: 0,
            usage_commitment: None,
            proof: None,
        }
        .into(),
        SetKaigiRelayManifest {
            call_id: call_id.clone(),
            relay_manifest: None,
        }
        .into(),
        RegisterKaigiRelay {
            relay: KaigiRelayRegistration {
                relay_id: relay.clone(),
                hpke_public_key: crate::smartcontracts::isi::kaigi::test_relay_hpke_key(1),
                bandwidth_class: 1,
            },
        }
        .into(),
        UnregisterKaigiRelay {
            relay_id: relay.clone(),
        }
        .into(),
        ReportKaigiRelayHealth {
            call_id,
            relay_id: relay,
            status: KaigiRelayHealthStatus::Healthy,
            reported_at_ms: 0,
            notes: None,
        }
        .into(),
    ];

    for instruction in instructions {
        assert!(
            initial_native_instruction_is_explicitly_admitted(&instruction),
            "{} must reach its native Core authorization checks",
            instruction.id()
        );
    }
}
#[test]
fn initial_executor_executes_kaigi_create_through_core() {
    use iroha_data_model::{
        isi::kaigi::CreateKaigi,
        kaigi::{KaigiId, NewKaigi, kaigi_metadata_key},
    };

    let host = checked_account_id();
    let domain_id = DomainId::try_new("kaigi", "universal").expect("domain id");
    let call_name = Name::from_str("initial-create").expect("call name");
    let call_id = KaigiId::new(domain_id.clone(), call_name.clone());
    let world = World::with(
        [Domain::new(domain_id.clone()).build(&host)],
        [Account::new(host.clone()).build(&host)],
        [],
    );
    let state = state_after_genesis(world);
    let mut block = state.block(BlockHeader::new(
        nonzero!(2_u64),
        Some(
            state
                .view()
                .latest_block_hash()
                .expect("genuine genesis parent"),
        ),
        None,
        1,
        0,
    ));
    let mut state_transaction = block.transaction();
    let instruction: InstructionBox = CreateKaigi {
        call: NewKaigi::with_defaults(call_id, host.clone()),
        commitment: None,
        nullifier: None,
        roster_root: None,
        proof: None,
    }
    .into();

    super::Executor::Initial
        .execute_instruction(&mut state_transaction, &host, instruction)
        .expect("Initial executor must dispatch Kaigi create to Core");
    let key = kaigi_metadata_key(&call_name).expect("Kaigi metadata key");
    assert!(
        state_transaction
            .world
            .domain(&domain_id)
            .expect("domain")
            .metadata()
            .contains(&key),
        "Core Kaigi execution must persist the call record"
    );
}
#[test]
fn initial_executor_rejects_kaigi_create_in_foreign_domain() {
    use iroha_data_model::{
        isi::kaigi::CreateKaigi,
        kaigi::{KaigiId, NewKaigi, kaigi_metadata_key},
    };

    let owner = checked_account_id();
    let intruder = checked_account_id();
    let domain_id = DomainId::try_new("kaigi", "universal").expect("domain id");
    let call_name = Name::from_str("squatted-call").expect("call name");
    let call_id = KaigiId::new(domain_id.clone(), call_name.clone());
    let world = World::with(
        [Domain::new(domain_id.clone()).build(&owner)],
        [
            Account::new(owner.clone()).build(&owner),
            Account::new(intruder.clone()).build(&owner),
        ],
        [],
    );
    let state = state_after_genesis(world);
    let mut block = state.block(BlockHeader::new(
        nonzero!(2_u64),
        Some(
            state
                .view()
                .latest_block_hash()
                .expect("genuine genesis parent"),
        ),
        None,
        1,
        0,
    ));
    let mut state_transaction = block.transaction();
    let instruction: InstructionBox = CreateKaigi {
        call: NewKaigi::with_defaults(call_id, intruder.clone()),
        commitment: None,
        nullifier: None,
        roster_root: None,
        proof: None,
    }
    .into();

    let error = super::Executor::Initial
        .execute_instruction(&mut state_transaction, &intruder, instruction)
        .expect_err("a non-owner must not squat another domain's Kaigi namespace");
    assert!(
        matches!(
            error,
            ValidationFail::NotPermitted(ref message)
                if message == "authority cannot create a Kaigi in this domain"
        ),
        "unexpected foreign-domain Kaigi rejection: {error:?}",
    );
    let key = kaigi_metadata_key(&call_name).expect("Kaigi metadata key");
    assert!(
        !state_transaction
            .world
            .domain(&domain_id)
            .expect("domain")
            .metadata()
            .contains(&key),
        "denied Kaigi creation must not reserve the foreign call name",
    );
}
#[test]
fn initial_executor_rejects_direct_native_kaigi_metadata_writes() {
    use iroha_data_model::kaigi::kaigi_metadata_key;

    let authority = checked_account_id();
    let domain_id = DomainId::try_new("kaigi", "universal").expect("domain id");
    let key = kaigi_metadata_key(&Name::from_str("forged").expect("call name"))
        .expect("Kaigi metadata key");
    let world = World::with(
        [Domain::new(domain_id.clone()).build(&authority)],
        [Account::new(authority.clone()).build(&authority)],
        [],
    );
    let state = state_after_genesis(world);
    let mut block = state.block(BlockHeader::new(
        nonzero!(2_u64),
        Some(
            state
                .view()
                .latest_block_hash()
                .expect("genuine genesis parent"),
        ),
        None,
        1,
        0,
    ));
    let mut state_transaction = block.transaction();
    let instruction = SetKeyValue::domain(domain_id, key, Json::new("forged record")).into();

    let error = super::Executor::Initial
        .execute_instruction(&mut state_transaction, &authority, instruction)
        .expect_err("ordinary domain metadata must not overwrite native Kaigi state");
    assert!(
        matches!(
            error,
            ValidationFail::NotPermitted(ref message)
                if message == "native Kaigi metadata keys cannot be changed directly"
        ),
        "unexpected reserved Kaigi metadata rejection: {error:?}"
    );
}
#[test]
fn initial_executor_committee_publication_preserves_owner_proofs_and_parameter_authority() {
    use crate::{
        beacon::{
            InMemoryGlobalThresholdBeaconPartialSignerV1, ValidatedGlobalThresholdBeaconSessionV1,
            prepared_session_and_signers_fixture_for_keys_v1,
            prove_global_threshold_beacon_seat_readiness_v1,
            seat_readiness::verify_global_threshold_beacon_seat_readiness_v1,
        },
        sumeragi::test_chain::CertifiedTestChain,
    };
    use iroha_crypto::SignatureOf;
    use iroha_data_model::{
        block::consensus::PublicLaneStakingPolicy,
        consensus::GlobalThresholdBeaconDkgSessionV1,
        isi::{
            PublicLaneCandidateAuthorization, RegisterPublicLaneCandidate,
            RegisterPublicLaneValidator, SetParameter,
        },
        nexus::{
            AdmitValidatorCommitteeSeatV1, PrepareValidatorCommitteeCredentialsV1,
            PublicLaneMonetaryPlanV1, PublicLaneMonetaryPreconditionV1,
            PublicLaneMonetaryRegistrationV1, PublicLaneMonetaryScopeV1,
            ValidatorCommitteeCredentialsV1, ValidatorCommitteeOperationV1,
            ValidatorCommitteeSeatReadinessV1,
        },
        parameter::{Parameter, custom::CustomParameter, system::SumeragiNposParameters},
        sumeragi::epoch::InstalledBeaconEpochBindingV1,
    };
    use iroha_model_base::topology::LaneId;
    struct Beacon {
        session: ValidatedGlobalThresholdBeaconSessionV1,
        signers: Vec<InMemoryGlobalThresholdBeaconPartialSignerV1>,
    }
    #[inline(never)]
    fn prepared_chain() -> (
        CertifiedTestChain,
        Vec<KeyPair>,
        Vec<KeyPair>,
        Beacon,
        PrepareValidatorCommitteeCredentialsV1,
    ) {
        use crate::beacon::{
            FinalizedGlobalThresholdBeaconKeySessionRecordV1, GlobalThresholdBeaconPartialSignerV1,
            GlobalThresholdBeaconPulseAggregatorV1,
        };
        use crate::sumeragi::test_chain::{Signers, TestChainConfig};
        use iroha_data_model::{
            consensus::{GlobalThresholdBeaconChainAnchorV1, GlobalThresholdBeaconPulseContextV1},
            isi::{
                RegisterPublicLaneValidator,
                consensus_keys::{
                    ApplyThresholdKeyLifecycleCertificateV1, ThresholdKeyLifecycleActionV1,
                    ThresholdKeyLifecycleCertificateV1, ThresholdKeyLifecycleSignatureV1,
                },
            },
            nexus::{PrepareValidatorCommitteeCredentialsV1, PublicLaneMonetaryPlanV1},
            parameter::system::SumeragiParameter,
        };
        use std::num::NonZeroU64;

        const EPOCH: u64 = 16;
        fn ceremony(
            chain: &CertifiedTestChain,
            keys: &[KeyPair],
            generation: u64,
            session_id: [u8; 32],
            attempt_id: [u8; 32],
            start: u64,
        ) -> Beacon {
            let roster = keys
                .iter()
                .map(|key| PeerId::new(key.public_key().clone()))
                .collect::<Vec<_>>();
            let (session, signers) = prepared_session_and_signers_fixture_for_keys_v1(
                GlobalThresholdBeaconDkgSessionV1 {
                    version: 1,
                    network_id: chain.network_id(),
                    session_id,
                    attempt_id,
                    authority_generation: generation,
                    roster_hash: crate::beacon::global_threshold_beacon_roster_hash_v1(&roster),
                    committee_size: 4,
                    threshold: 2,
                    start_height: start,
                    commitments_end_height: start + 1,
                    deliveries_end_height: start + 2,
                    acceptances_end_height: start + 3,
                },
                keys,
                &chain.state().ivm_execution_budget(),
            );
            assert!(session.belongs_to(&chain.state().ivm_execution_budget()));
            Beacon { session, signers }
        }
        fn install(
            chain: &mut CertifiedTestChain,
            beacon: &Beacon,
            keys: &[KeyPair],
            owner: &KeyPair,
        ) {
            let height = chain.height() + 1;
            let roster = keys
                .iter()
                .map(|key| PeerId::new(key.public_key().clone()))
                .collect::<Vec<_>>();
            let record = FinalizedGlobalThresholdBeaconKeySessionRecordV1::new(
                beacon.session.record().clone(),
                &chain.state().ivm_execution_budget(),
            )
            .unwrap();
            let mut certificate = ThresholdKeyLifecycleCertificateV1 {
                version: crate::state::THRESHOLD_KEY_LIFECYCLE_CERTIFICATE_VERSION_V1,
                action: ThresholdKeyLifecycleActionV1::FinalizeGlobalBeaconKey,
                expected_active_session_id: chain
                    .state()
                    .view()
                    .world()
                    .active_global_beacon_key_session(),
                effective_height: height,
                network_id: chain.network_id(),
                roster_hash: crate::beacon::global_threshold_beacon_roster_hash_v1(&roster),
                committee_size: 4,
                quorum: 3,
                session_id: record.session.session_id,
                transcript_hash: record.session.transcript_hash,
                public_state: norito::encode_canonical(&record).unwrap(),
                signatures: Vec::new(),
            };
            let preimage =
                crate::state::threshold_key_lifecycle_certificate_preimage_v1(&certificate)
                    .unwrap();
            certificate.signatures = keys
                .iter()
                .take(3)
                .enumerate()
                .map(|(index, key)| ThresholdKeyLifecycleSignatureV1 {
                    signer_index: u16::try_from(index).expect("fixture value fits u16"),
                    signature: iroha_crypto::Signature::try_new(key.private_key(), &preimage)
                        .unwrap(),
                })
                .collect();
            crate::state::verify_threshold_key_lifecycle_certificate_v1(
                &certificate,
                &chain.network_id(),
                height,
                &roster,
            )
            .unwrap();
            let signed = chain.sign(
                owner,
                [ApplyThresholdKeyLifecycleCertificateV1 { certificate }.into()],
                height * 1_000,
            );
            assert_eq!(chain.commit_at(height * 1_000, vec![signed]), [true]);
        }
        fn advance(chain: &mut CertifiedTestChain, target: u64, beacon: &Beacon) {
            while chain.height() < target {
                let height = chain.height() + 1;
                let current = chain
                    .state()
                    .view()
                    .world()
                    .consensus_schedule()
                    .ready(height)
                    .unwrap()
                    .epoch
                    .clone();
                let control = if height + 1 == current.authorization.last_height {
                    let parent = chain.committed(chain.height());
                    let epoch = crate::sumeragi::schedule::core_epoch(&current).unwrap().id;
                    let mut pulse = GlobalThresholdBeaconPulseAggregatorV1::new(
                        beacon.session.clone(),
                        height,
                        GlobalThresholdBeaconChainAnchorV1 {
                            height: chain.height(),
                            block_hash: parent.block_hash(),
                        },
                        GlobalThresholdBeaconPulseContextV1 {
                            instance: chain.instance().0,
                            epoch: epoch.epoch,
                            epoch_context_id: epoch.context.0,
                            parent_consensus_hash: parent.core_hash().0,
                            parent_result: parent.result().0,
                        },
                    )
                    .unwrap();
                    for signer in beacon.signers.iter().take(2) {
                        pulse
                            .accept_partial(
                                signer
                                    .sign_partial(pulse.session(), pulse.payload())
                                    .unwrap(),
                            )
                            .unwrap();
                    }
                    crate::sumeragi::epoch_beacon::control::encode(
                        Some(pulse.finalize().unwrap()).as_ref(),
                    )
                    .unwrap()
                } else {
                    iroha_sumeragi::types::ControlWitness::empty()
                };
                chain.commit_with_control(
                    Some(height * 1_000),
                    Vec::new(),
                    Signers::Quorum,
                    control,
                );
                assert_eq!(chain.height(), height);
            }
        }
        let mut keys = (0x51..=0x54)
            .map(|seed| KeyPair::from_seed(vec![seed; 32], Algorithm::BlsNormal))
            .collect::<Vec<_>>();
        keys.sort_by_key(|key| PeerId::new(key.public_key().clone()));
        let owners = (0xD1..=0xD4)
            .map(|seed| KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519))
            .chain(
                [0xE1, 0xE2]
                    .into_iter()
                    .map(|seed| KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519)),
            )
            .collect::<Vec<_>>();
        let accounts = owners
            .iter()
            .map(|key| AccountId::new(key.public_key().clone()))
            .collect::<Vec<_>>();
        let mut config = TestChainConfig::new(World::new(), 1_000);
        assert_eq!(config.genesis_parameters.len(), 1);
        let staking = config
            .genesis_parameters
            .iter()
            .find_map(|parameter| match parameter {
                Parameter::Custom(custom)
                    if custom.id() == &PublicLaneStakingPolicy::parameter_id() =>
                {
                    Some(PublicLaneStakingPolicy::from_custom_parameter(custom).unwrap())
                }
                _ => None,
            })
            .expect("the fixture owns its original signed staking policy");
        let monetary = staking.clone();
        let definition: AssetDefinitionId = staking.stake_asset_id;
        let escrow = staking.stake_escrow_account_id;
        let world = World::with_assets(
            [Domain::new(DomainId::try_new("nexus", "universal").unwrap()).build(&accounts[0])],
            accounts
                .iter()
                .map(|account| Account::new(account.clone()).build(account))
                .chain([Account::new(escrow.clone()).build(&escrow)]),
            [AssetDefinition::new(
                definition.clone(),
                "Staked XOR",
                iroha_primitives::numeric::NumericSpec::fractional(9),
                iroha_data_model::asset::AssetBalancePolicy::Global,
                None,
            )
            .build(&accounts[0])],
            accounts.iter().map(|account| {
                Asset::new(
                    AssetId::new(definition.clone(), account.clone()),
                    Quantity::from(1_000u64),
                )
            }),
            [],
        );
        config.world = world;
        config.validator_keys = Some(keys.clone());
        config.consensus_mode = iroha_data_model::parameter::system::SumeragiConsensusMode::Npos;
        config.genesis_parameters.extend([
            Parameter::Sumeragi(SumeragiParameter::EpochLengthBlocks(
                NonZeroU64::new(EPOCH).unwrap(),
            )),
            Parameter::Custom(
                SumeragiNposParameters {
                    epoch_length_blocks: NonZeroU64::new(EPOCH).unwrap(),
                    max_validators: 4,
                    evidence_horizon_blocks: EPOCH,
                    slashing_delay_blocks: 2,
                    ..SumeragiNposParameters::default()
                }
                .into_custom_parameter(),
            ),
        ]);
        for (account, key) in accounts.iter().zip(&keys) {
            let source = AssetId::new(definition.clone(), account.clone());
            let destination = AssetId::new(definition.clone(), escrow.clone());
            config.genesis_instructions.push(
                RegisterPublicLaneValidator::new(
                    LaneId::SINGLE,
                    account.clone(),
                    PeerId::new(key.public_key().clone()),
                    account.clone(),
                    Quantity::from(1_000u64),
                    Metadata::default(),
                    PublicLaneMonetaryPlanV1::genesis_registration(
                        source,
                        destination,
                        Quantity::from(1_000u64),
                    ),
                )
                .into(),
            );
        }
        let mut chain = CertifiedTestChain::start(config).unwrap();
        assert_eq!(
            chain
                .state()
                .view()
                .world()
                .parameters()
                .require_staking_policy()
                .unwrap(),
            &monetary,
            "actual signed genesis retains the policy that funded its registration plans",
        );

        let bootstrap = ceremony(&chain, &keys, 0, [0x71; 32], [0x71; 32], 1);
        advance(&mut chain, 3, &bootstrap);
        install(&mut chain, &bootstrap, &keys, &owners[0]);
        advance(&mut chain, EPOCH, &bootstrap);
        let preparation = chain
            .state()
            .view()
            .world()
            .validator_committee_transitions()
            .get(&2)
            .unwrap()
            .preparation
            .clone();
        assert_eq!(preparation.selection_height, EPOCH);
        assert_eq!(
            preparation.selection_anchor,
            chain.committed(EPOCH - 1).block_hash()
        );
        assert_eq!(
            preparation.generation().validators,
            keys.iter()
                .map(|key| PeerId::new(key.public_key().clone()))
                .collect::<Vec<_>>()
        );
        let target = ceremony(
            &chain,
            &keys,
            1,
            preparation.beacon_session_id().unwrap(),
            preparation.transition_id().unwrap(),
            EPOCH + 1,
        );
        advance(&mut chain, EPOCH + 4, &bootstrap);
        install(&mut chain, &target, &keys, &owners[0]);
        let command = PrepareValidatorCommitteeCredentialsV1 {
            transition_id: preparation.transition_id().unwrap(),
            target_epoch: 2,
            credentials: ValidatorCommitteeCredentialsV1 {
                beacon: InstalledBeaconEpochBindingV1 {
                    session_id: target.session.record().session_id,
                    transcript_hash: target.session.record().transcript_hash,
                },
            },
        };
        assert!(
            chain
                .state()
                .view()
                .world()
                .validator_committee_transitions()
                .get(&2)
                .unwrap()
                .credentials
                .is_none()
        );
        (chain, owners, keys, target, command)
    }

    fn operation(command: ValidatorCommitteeOperationV1) -> InstructionBox {
        SetParameter::new(Parameter::Custom(command.into_custom_parameter())).into()
    }
    fn submit(chain: &mut CertifiedTestChain, inputs: &[(&KeyPair, InstructionBox, Option<&str>)]) {
        let height = chain.height() + 1;
        let time = height * 1_000;
        let transactions = inputs
            .iter()
            .map(|(key, instruction, _)| chain.sign(key, [instruction.clone()], time))
            .collect();
        let outcomes = chain.commit_at(time, transactions);
        assert_eq!(chain.height(), height);
        let committed = chain.committed(height);
        for (index, (_, _, expected)) in inputs.iter().enumerate() {
            let error = committed.block().output_error(index);
            assert_eq!(
                outcomes[index],
                expected.is_none(),
                "input {index}: {error:?}"
            );
            if let Some(expected) = expected {
                assert!(
                    format!("{error:?}").contains(expected),
                    "input {index}: {error:?}"
                );
            } else {
                assert!(error.is_none(), "input {index}: {error:?}");
            }
        }
    }

    // This fixture executes the original genesis, H16 selection, and both actual DKG installs.
    // Commands below are signed account work through Initial; no World preparation is injected.
    let (mut chain, owners, original_keys, target, credentials) = prepared_chain();
    assert_eq!(chain.height(), 21);
    let before = chain
        .state()
        .view()
        .world()
        .validator_committee_transitions()
        .get(&2)
        .unwrap()
        .clone();
    let authority_before =
        crate::state::validator_committee::current_authority(&chain.state().view()).unwrap();
    let candidate_owner = AccountId::new(owners[4].public_key().clone());
    let intruder = &owners[5];
    let candidate_key = KeyPair::from_seed(vec![0x61; 32], Algorithm::BlsNormal);
    let peer = PeerId::new(candidate_key.public_key().clone());
    let (definition, escrow) = {
        let view = chain.state().view();
        let staking = view
            .world()
            .parameters()
            .require_staking_policy()
            .expect("the original signed genesis commits staking policy");
        (
            staking.stake_asset_id.clone(),
            staking.stake_escrow_account_id.clone(),
        )
    };
    let source = AssetId::new(definition.clone(), candidate_owner.clone());
    let destination = AssetId::new(definition, escrow);
    // The H16 boundary already froze epoch 2; this new peer may target only epoch 3.
    let activation_height = before.preparation.last_height.checked_add(1).unwrap();
    let registration = RegisterPublicLaneValidator::new(
        LaneId::SINGLE,
        candidate_owner.clone(),
        peer.clone(),
        candidate_owner.clone(),
        Quantity::from(1_000u64),
        Metadata::default(),
        PublicLaneMonetaryPlanV1 {
            network_scope: PublicLaneMonetaryScopeV1::Network(chain.network_id()),
            valid_until_height: 28,
            source_asset: source.clone(),
            destination_asset: destination,
            amount: Quantity::from(1_000u64),
            precondition: PublicLaneMonetaryPreconditionV1::Registration(
                PublicLaneMonetaryRegistrationV1 { activation_height },
            ),
        },
    );
    let consent = PublicLaneCandidateAuthorization::new(
        chain.network_id(),
        registration.clone(),
        activation_height,
    );
    let candidate = RegisterPublicLaneCandidate {
        registration,
        activation_height,
        proof_of_possession: iroha_crypto::bls_normal_pop_prove(candidate_key.private_key())
            .unwrap(),
        peer_signature: SignatureOf::new(candidate_key.private_key(), &consent),
    };
    candidate
        .peer_signature
        .verify(candidate_key.public_key(), &consent)
        .unwrap();
    let mut foreign_consent = consent.clone();
    foreign_consent.network_id =
        executor_test_network_id(b"foreign candidate committee publication");
    let mut foreign = candidate.clone();
    foreign.peer_signature = SignatureOf::new(candidate_key.private_key(), &foreign_consent);
    foreign
        .peer_signature
        .verify(candidate_key.public_key(), &foreign_consent)
        .unwrap();
    let mut later = candidate.clone();
    later.activation_height = activation_height + 16;
    let mut bad_pop = candidate.clone();
    bad_pop.proof_of_possession[0] ^= 1;
    submit(
        &mut chain,
        &[
            (
                intruder,
                candidate.clone().into(),
                Some("public validator lifecycle requires the validator account authority"),
            ),
            (
                &owners[4],
                later.into(),
                Some("consent targets activation height"),
            ),
            (
                &owners[4],
                foreign.into(),
                Some("does not authorize this network and registration"),
            ),
            (
                &owners[4],
                bad_pop.into(),
                Some("invalid BLS-normal proof of possession"),
            ),
        ],
    );
    {
        let view = chain.state().view();
        assert!(
            view.world()
                .public_lane_validators()
                .get(&(LaneId::SINGLE, candidate_owner.clone()))
                .is_none()
        );
        assert!(!view.world().peers().iter().any(|value| value == &peer));
        assert!(
            view.world()
                .consensus_keys_by_pk()
                .get(&peer.public_key().to_string())
                .is_none()
        );
        assert!(
            view.world()
                .public_lane_stake_custody()
                .get(&(LaneId::SINGLE, candidate_owner.clone()))
                .is_none()
        );
        assert_eq!(
            initial_batch_balance(view.world(), &source),
            Quantity::from(1_000u64)
        );
        assert_eq!(
            view.world().validator_committee_transitions().get(&2),
            Some(&before)
        );
    }
    submit(&mut chain, &[(&owners[4], candidate.clone().into(), None)]);
    {
        let view = chain.state().view();
        let registered = view
            .world()
            .public_lane_validators()
            .get(&(LaneId::SINGLE, candidate_owner.clone()))
            .unwrap();
        assert_eq!(registered.peer_id, peer);
        assert_eq!(registered.activation_height, activation_height);
        assert_eq!(registered.total_stake, Quantity::from(1_000u64));
        assert_eq!(
            initial_batch_balance(view.world(), &source),
            Quantity::zero()
        );
        assert_eq!(
            view.world().validator_committee_transitions().get(&2),
            Some(&before)
        );
    }
    let mut absent = credentials;
    absent.target_epoch = 1;
    let mut wrong_attempt = credentials;
    wrong_attempt.transition_id[0] ^= 1;
    let mut forged_transcript = credentials;
    forged_transcript.credentials.beacon.transcript_hash[0] ^= 1;
    submit(
        &mut chain,
        &[
            (
                intruder,
                operation(ValidatorCommitteeOperationV1::PrepareCredentials(
                    credentials,
                )),
                Some("exact target validator owner"),
            ),
            (
                &owners[0],
                operation(ValidatorCommitteeOperationV1::PrepareCredentials(absent)),
                Some("attempt has not been frozen"),
            ),
            (
                &owners[0],
                operation(ValidatorCommitteeOperationV1::PrepareCredentials(
                    wrong_attempt,
                )),
                Some("terminal, replayed or outside its preparation cutoff"),
            ),
            (
                &owners[0],
                operation(ValidatorCommitteeOperationV1::PrepareCredentials(
                    forged_transcript,
                )),
                Some("outside its exact preparation attempt or cutoff"),
            ),
        ],
    );
    assert_eq!(
        chain
            .state()
            .view()
            .world()
            .validator_committee_transitions()
            .get(&2),
        Some(&before)
    );
    submit(
        &mut chain,
        &[(
            &owners[0],
            operation(ValidatorCommitteeOperationV1::PrepareCredentials(
                credentials,
            )),
            None,
        )],
    );
    let mut expected = before;
    expected.credentials = Some(credentials.credentials);
    assert_eq!(
        chain
            .state()
            .view()
            .world()
            .validator_committee_transitions()
            .get(&2),
        Some(&expected)
    );
    let generation = expected.preparation.generation();
    let context = expected.readiness_context(0).unwrap();
    let proof = prove_global_threshold_beacon_seat_readiness_v1(
        &target.signers[0],
        &target.session,
        &generation,
        &context,
    )
    .unwrap();
    verify_global_threshold_beacon_seat_readiness_v1(
        &target.session,
        &generation,
        &context,
        &proof,
    )
    .unwrap();
    let ready = AdmitValidatorCommitteeSeatV1 {
        transition_id: credentials.transition_id,
        target_epoch: credentials.target_epoch,
        readiness: ValidatorCommitteeSeatReadinessV1 {
            validator_index: 0,
            beacon: proof,
        },
    };
    let proof_for_generation = |number| {
        let mut foreign_generation = generation.clone();
        foreign_generation.generation = number;
        let mut foreign_context = context;
        foreign_context.authority_generation = number;
        foreign_context.authority_id = foreign_generation.generation_id().unwrap();
        // This is a genuine adaptive share proof for a different complete challenge.
        // It is not authority for the immutable generation selected on this chain.
        let foreign_proof = prove_global_threshold_beacon_seat_readiness_v1(
            &target.signers[0],
            &target.session,
            &foreign_generation,
            &foreign_context,
        )
        .unwrap();
        verify_global_threshold_beacon_seat_readiness_v1(
            &target.session,
            &foreign_generation,
            &foreign_context,
            &foreign_proof,
        )
        .unwrap();
        assert!(
            verify_global_threshold_beacon_seat_readiness_v1(
                &target.session,
                &generation,
                &context,
                &foreign_proof,
            )
            .is_err()
        );
        let mut command = ready.clone();
        command.readiness.beacon = foreign_proof;
        command
    };
    let previous_generation = proof_for_generation(0);
    let next_generation = proof_for_generation(2);
    let mut corrupted = ready.clone();
    corrupted.readiness.beacon.proof.z_s[0] ^= 1;
    submit(
        &mut chain,
        &[
            (
                intruder,
                operation(ValidatorCommitteeOperationV1::AdmitSeat(ready.clone())),
                Some("exact target validator owner"),
            ),
            (
                &owners[0],
                operation(ValidatorCommitteeOperationV1::AdmitSeat(
                    previous_generation,
                )),
                Some("custody proof is invalid"),
            ),
            (
                &owners[0],
                operation(ValidatorCommitteeOperationV1::AdmitSeat(next_generation)),
                Some("custody proof is invalid"),
            ),
            (
                &owners[0],
                operation(ValidatorCommitteeOperationV1::AdmitSeat(corrupted)),
                Some("custody proof is invalid"),
            ),
        ],
    );
    assert_eq!(
        chain
            .state()
            .view()
            .world()
            .validator_committee_transitions()
            .get(&2),
        Some(&expected)
    );
    submit(
        &mut chain,
        &[(
            &owners[0],
            operation(ValidatorCommitteeOperationV1::AdmitSeat(ready.clone())),
            None,
        )],
    );
    expected.readiness.push(ready.readiness.clone());
    assert_eq!(
        chain
            .state()
            .view()
            .world()
            .validator_committee_transitions()
            .get(&2),
        Some(&expected)
    );
    // A peer admitted for a future election is not in the incumbent commit topology.
    // Its replay reaches that live-key gate before the validator-row duplicate guard.
    let fresh_peer_refusal = "global validator peer must be present in the commit topology; a prepared epoch key transition is required for a fresh peer";
    let incumbent_owner = AccountId::new(owners[0].public_key().clone());
    let incumbent_peer = PeerId::new(original_keys[0].public_key().clone());
    let incumbent_source = AssetId::new(source.definition().clone(), incumbent_owner.clone());
    let mut incumbent_registration = candidate.registration.clone();
    incumbent_registration.validator = incumbent_owner.clone();
    incumbent_registration.stake_account = incumbent_owner.clone();
    incumbent_registration.peer_id = incumbent_peer.clone();
    incumbent_registration.monetary_plan.source_asset = incumbent_source.clone();
    let incumbent_consent = PublicLaneCandidateAuthorization::new(
        chain.network_id(),
        incumbent_registration.clone(),
        activation_height,
    );
    let incumbent_duplicate = RegisterPublicLaneCandidate {
        registration: incumbent_registration,
        activation_height,
        proof_of_possession: iroha_crypto::bls_normal_pop_prove(original_keys[0].private_key())
            .unwrap(),
        peer_signature: SignatureOf::new(original_keys[0].private_key(), &incumbent_consent),
    };
    iroha_crypto::bls_normal_pop_verify(
        incumbent_peer.public_key(),
        &incumbent_duplicate.proof_of_possession,
    )
    .unwrap();
    incumbent_duplicate
        .peer_signature
        .verify(incumbent_peer.public_key(), &incumbent_consent)
        .unwrap();
    let replay_accounts = [
        (&candidate_owner, &source),
        (&incumbent_owner, &incumbent_source),
    ];
    let replay_before = {
        let view = chain.state().view();
        replay_accounts.map(|(owner, source)| {
            (
                view.world()
                    .public_lane_validators()
                    .get(&(LaneId::SINGLE, owner.clone()))
                    .unwrap()
                    .clone(),
                view.world()
                    .public_lane_stake_shares()
                    .get(&(LaneId::SINGLE, owner.clone(), owner.clone()))
                    .unwrap()
                    .clone(),
                initial_batch_balance(view.world(), source),
            )
        })
    };
    assert_eq!(replay_before[1].0.peer_id, incumbent_peer);
    let ordinary = SetParameter::new(Parameter::Custom(CustomParameter::new(
        "ordinary_setting".parse().unwrap(),
        Json::new(1_u64),
    )));
    submit(
        &mut chain,
        &[
            (&owners[4], candidate.into(), Some(fresh_peer_refusal)),
            (
                &owners[0],
                incumbent_duplicate.into(),
                Some("validator already registered for lane"),
            ),
            (
                &owners[0],
                operation(ValidatorCommitteeOperationV1::PrepareCredentials(
                    credentials,
                )),
                Some("prepared credentials cannot be replaced"),
            ),
            (
                &owners[0],
                operation(ValidatorCommitteeOperationV1::AdmitSeat(ready)),
                Some("target seat readiness is already recorded"),
            ),
            (&owners[0], ordinary.into(), Some("CanSetParameters")),
        ],
    );
    let committed = chain.committed(28);
    for (index, refusal) in [fresh_peer_refusal, "validator already registered for lane"]
        .into_iter()
        .enumerate()
    {
        assert_eq!(
            committed.block().output_error(index),
            Some(&crate::tx::TransactionRejectionReason::Validation(
                ValidationFail::InstructionFailed(InstructionExecutionError::InvariantViolation(
                    refusal.into()
                ))
            )),
            "each replay must reach its exact native registration guard"
        );
    }
    let view = chain.state().view();
    assert_eq!(chain.height(), 28);
    for ((owner, source), (record, share, balance)) in
        replay_accounts.into_iter().zip(replay_before)
    {
        assert_eq!(
            view.world()
                .public_lane_validators()
                .get(&(LaneId::SINGLE, owner.clone())),
            Some(&record),
            "rejected replay must preserve the complete original validator record"
        );
        assert_eq!(
            view.world().public_lane_stake_shares().get(&(
                LaneId::SINGLE,
                owner.clone(),
                owner.clone()
            )),
            Some(&share),
            "rejected replay must preserve the original bonded share"
        );
        assert_eq!(
            initial_batch_balance(view.world(), source),
            balance,
            "rejected replay must not transfer a second original stake"
        );
    }
    assert_eq!(
        view.world().validator_committee_transitions().get(&2),
        Some(&expected)
    );
    assert_eq!(
        crate::state::validator_committee::current_authority(&view).unwrap(),
        authority_before
    );
    assert_eq!(
        authority_before.0.validators,
        original_keys
            .iter()
            .map(|key| PeerId::new(key.public_key().clone()))
            .collect::<Vec<_>>()
    );
    assert!(expected.outcome.is_none());
    assert!(
        !view
            .world()
            .parameters()
            .custom()
            .contains_key(&ValidatorCommitteeOperationV1::parameter_id())
    );
    assert!(
        !view
            .world()
            .parameters()
            .custom()
            .contains_key(&"ordinary_setting".parse().unwrap())
    );
}
#[test]
fn initial_executor_citizenship_preserves_owner_bond_and_release_checks() {
    use crate::sumeragi::test_chain::{CertifiedTestChain, TestChainConfig};
    use iroha_data_model::isi::governance::{RegisterCitizen, UnregisterCitizen};
    let owner_key = checked_keypair();
    let escrow_key = checked_keypair();
    let owner = AccountId::new(owner_key.public_key().clone());
    let escrow = AccountId::new(escrow_key.public_key().clone());
    let domain = DomainId::try_new("citizens", "universal").unwrap();
    let definition =
        AssetDefinitionId::derive_from_components(domain.clone(), "bond".parse().unwrap());
    let owner_asset = AssetId::new(definition.clone(), owner.clone());
    let escrow_asset = AssetId::new(definition.clone(), escrow.clone());
    let world = World::with_assets(
        [Domain::new(domain).build(&owner)],
        [
            Account::new(owner.clone()).build(&owner),
            Account::new(escrow.clone()).build(&escrow),
        ],
        [AssetDefinition::numeric(
            definition.clone(),
            "bond",
            iroha_data_model::asset::AssetBalancePolicy::Global,
            None,
        )
        .build(&owner)],
        [
            Asset::new(owner_asset.clone(), Quantity::from(100_u32)),
            Asset::new(escrow_asset.clone(), Quantity::zero()),
        ],
        [],
    );
    let mut governance = state_after_genesis(World::new()).gov.clone();
    governance.citizenship_asset_id = definition;
    governance.citizenship_bond_amount = Quantity::from(50_u32);
    governance.citizenship_escrow_account = escrow.clone();
    let mut config = TestChainConfig::new(world, 1_000);
    config.governance = Some(governance);
    let mut chain = CertifiedTestChain::start(config).expect("signed governance policy");
    let submit =
        |chain: &mut CertifiedTestChain, key: &KeyPair, instruction, expected: Option<&str>| {
            let time = (chain.height() + 1) * 1_000;
            let transaction = chain.sign(key, [instruction], time);
            let outcome = chain.commit_at(time, vec![transaction]);
            let committed = chain.committed(chain.height());
            let error = committed.block().output_error(0);
            if let Some(expected) = expected {
                assert_eq!(outcome, [false]);
                assert!(format!("{error:?}").contains(expected), "{error:?}");
            } else {
                assert_eq!(outcome, [true], "{error:?}");
                assert!(error.is_none());
            }
        };
    let register = |amount| {
        InstructionBox::from(RegisterCitizen {
            owner: owner.clone(),
            amount: Quantity::from(amount),
        })
    };
    for (key, amount, expected) in [
        (&escrow_key, 50_u32, "owner must equal authority"),
        (&owner_key, 49, "citizenship bond below minimum"),
    ] {
        submit(&mut chain, key, register(amount), Some(expected));
        let view = chain.state().view();
        assert!(view.world().citizens().get(&owner).is_none());
        assert_eq!(
            initial_batch_balance(view.world(), &owner_asset),
            Quantity::from(100_u32)
        );
    }
    submit(&mut chain, &owner_key, register(50_u32), None);
    let bonded_height = chain.height();
    {
        let view = chain.state().view();
        assert_eq!(
            view.world().citizens().get(&owner).unwrap().bonded_height,
            bonded_height
        );
        assert_eq!(
            initial_batch_balance(view.world(), &owner_asset),
            Quantity::from(50_u32)
        );
        assert_eq!(
            initial_batch_balance(view.world(), &escrow_asset),
            Quantity::from(50_u32)
        );
    }
    submit(&mut chain, &owner_key, register(60_u32), None);
    submit(
        &mut chain,
        &owner_key,
        register(50_u32),
        Some("citizenship bond cannot decrease"),
    );
    let unregister = || {
        InstructionBox::from(UnregisterCitizen {
            owner: owner.clone(),
        })
    };
    submit(
        &mut chain,
        &escrow_key,
        unregister(),
        Some("owner must equal authority"),
    );
    {
        let view = chain.state().view();
        let citizen = view.world().citizens().get(&owner).unwrap();
        assert_eq!(citizen.bonded_height, bonded_height);
        assert_eq!(citizen.amount, Quantity::from(60_u32));
        assert_eq!(
            initial_batch_balance(view.world(), &owner_asset),
            Quantity::from(40_u32)
        );
        assert_eq!(
            initial_batch_balance(view.world(), &escrow_asset),
            Quantity::from(60_u32)
        );
    }
    submit(&mut chain, &owner_key, unregister(), None);
    let view = chain.state().view();
    assert!(view.world().citizens().get(&owner).is_none());
    assert_eq!(
        initial_batch_balance(view.world(), &owner_asset),
        Quantity::from(100_u32)
    );
    assert!(view.world().assets().get(&escrow_asset).is_none());
}
#[test]
fn initial_executor_classifies_verifying_key_bootstrap_as_genesis_only() {
    use iroha_data_model::{
        isi::verifying_keys::{RegisterVerifyingKey, UpdateVerifyingKey},
        proof::{VerifyingKeyId, VerifyingKeyRecord},
        zk::BackendTag,
    };
    let id = VerifyingKeyId::new("halo2/ipa", "genesis_vk");
    let record = VerifyingKeyRecord::new(
        1,
        "genesis-vk-circuit",
        BackendTag::NativePipaRPasta,
        "pallas",
        [0x11; 32],
        [0x22; 32],
    );
    let instructions: [InstructionBox; 2] = [
        RegisterVerifyingKey {
            id: id.clone(),
            record: record.clone(),
        }
        .into(),
        UpdateVerifyingKey { id, record }.into(),
    ];
    for instruction in &instructions {
        assert!(
            initial_genesis_instruction_is_explicitly_admitted(instruction),
            "{} must be admitted during authenticated genesis",
            instruction.id()
        );
        assert!(
            !initial_native_instruction_is_explicitly_admitted(instruction),
            "{} must remain unavailable through the post-genesis Initial executor",
            instruction.id()
        );
    }
}
#[test]
fn initial_executor_routes_native_privacy_corridor_to_core() {
    use crate::privacy_profiles::compiled_privacy_profile_v1;
    use iroha_data_model::{
        isi::privacy::SubmitPrivacyProofV1,
        privacy::{
            IrohaJindoPolynomialCommitmentStatementV1, PrivacyJindoFieldElementV1,
            PrivacyJindoLatticeCommitmentV1, PrivacyProofBytesV1, PrivacyProofEnvelopeV1,
            PrivacyProofV1, PrivacyProtocolIdV1, PrivacyStatementContextV1, PrivacyStatementV1,
            PrivacyTransactionIntentDigestV1,
        },
    };

    let profile =
        compiled_privacy_profile_v1(PrivacyProtocolIdV1::IrohaJindoPolynomialCommitmentV1)
            .expect("load compiled Jindo profile");
    let statement = PrivacyStatementV1::IrohaJindoPolynomialCommitmentV1(
        IrohaJindoPolynomialCommitmentStatementV1 {
            context: PrivacyStatementContextV1 {
                network_id: executor_test_network_id(b"initial-privacy-corridor"),
                action_index: 0,
                transaction_intent_digest: PrivacyTransactionIntentDigestV1::new([0x11; 32]),
                parameter_id: profile.parameter_id,
                parameter_digest: profile.parameter_digest,
                verifier_digest: profile.verifier_digest,
                statement_schema_digest: profile.statement_schema_digest,
                engine_manifest_digest: profile.engine_manifest_digest,
            },
            polynomial_commitments: vec![PrivacyJindoLatticeCommitmentV1::new(vec![0x22])],
            evaluation_point: PrivacyJindoFieldElementV1::new([0x33; 32]),
            claimed_evaluations: vec![PrivacyJindoFieldElementV1::new([0x44; 32])],
        },
    );
    let statement_digest = statement.digest().expect("hash Jindo statement fixture");
    let envelope = PrivacyProofEnvelopeV1 {
        wire_magic: iroha_data_model::privacy::PrivacyProofWireMagicV1::default(),
        catalog_commitment: iroha_data_model::privacy::PrivacyExact12CatalogCommitmentV1::default(),
        protocol_id: profile.protocol_id,
        proof_system_id: profile.proof_system_id,
        engine_id: profile.engine_id,
        parameter_id: profile.parameter_id,
        parameter_digest: profile.parameter_digest,
        verifier_digest: profile.verifier_digest,
        statement_schema_digest: profile.statement_schema_digest,
        engine_manifest_digest: profile.engine_manifest_digest,
        statement_digest,
        statement,
        proof: PrivacyProofV1::IrohaJindoPolynomialCommitmentV1(PrivacyProofBytesV1::new(vec![
            0x55,
        ])),
    };
    let instructions: [InstructionBox; 1] = [SubmitPrivacyProofV1::new(envelope).into()];
    for instruction in &instructions {
        assert!(
            initial_native_instruction_is_explicitly_admitted(instruction),
            "{} must reach its exact native Core authorization and verification gates",
            instruction.id(),
        );
    }
}
#[test]
fn initial_executor_routes_threshold_key_lifecycle_to_core_qc_authentication() {
    use iroha_data_model::isi::consensus_keys::{
        ApplyThresholdKeyLifecycleCertificateV1, ThresholdKeyLifecycleActionV1,
        ThresholdKeyLifecycleCertificateV1,
    };

    let authority = checked_account_id();
    let world = World::with([], [Account::new(authority.clone()).build(&authority)], []);
    let state = state_after_genesis(world);
    let mut block = state.block(BlockHeader::new(
        nonzero!(2_u64),
        Some(
            state
                .view()
                .latest_block_hash()
                .expect("genuine genesis parent"),
        ),
        None,
        1,
        0,
    ));
    let mut state_transaction = block.transaction();
    let instruction: InstructionBox = ApplyThresholdKeyLifecycleCertificateV1 {
        certificate: ThresholdKeyLifecycleCertificateV1 {
            version: crate::state::THRESHOLD_KEY_LIFECYCLE_CERTIFICATE_VERSION_V1,
            action: ThresholdKeyLifecycleActionV1::RetireParliamentTleKey,
            expected_active_session_id: Some([0x31; 32]),
            effective_height: 2,
            network_id: executor_test_network_id(b"initial threshold lifecycle admission"),
            roster_hash: [0x32; 32],
            committee_size: 4,
            quorum: 3,
            session_id: [0x31; 32],
            transcript_hash: [0x33; 32],
            public_state: Vec::new(),
            signatures: Vec::new(),
        },
    }
    .into();
    assert!(
        initial_native_instruction_is_explicitly_admitted(&instruction),
        "the proof-carrying lifecycle instruction must reach Core"
    );

    let error = super::Executor::Initial
        .execute_instruction(&mut state_transaction, &authority, instruction)
        .expect_err("an empty validator QC must fail in Core");
    assert!(
        format!("{error:?}").contains("threshold-key lifecycle certificate authentication failed"),
        "the Initial executor must route the instruction to exact Core QC verification: {error:?}"
    );
}
#[test]
fn initial_executor_cannot_bypass_native_asset_transfer_control_metadata() {
    let authority = checked_account_id();
    let world = World::with([], [Account::new(authority.clone()).build(&authority)], []);
    let state = state_after_genesis(world);
    let mut block = state.block(BlockHeader::new(
        nonzero!(2_u64),
        Some(
            state
                .view()
                .latest_block_hash()
                .expect("genuine genesis parent"),
        ),
        None,
        1,
        0,
    ));
    let mut state_transaction = block.transaction();
    let key: Name = iroha_data_model::asset::ASSET_TRANSFER_CONTROL_METADATA_KEY
        .parse()
        .expect("asset transfer control metadata key");
    let instructions: [InstructionBox; 2] = [
        SetKeyValue::account(
            authority.clone(),
            key.clone(),
            Json::new("generic replacement"),
        )
        .into(),
        RemoveKeyValue::account(authority.clone(), key).into(),
    ];

    for instruction in instructions {
        let error = super::Executor::Initial
            .execute_instruction(&mut state_transaction, &authority, instruction)
            .expect_err("Initial executor must preserve Core's reserved metadata guard");
        assert!(
            format!("{error:?}").contains("reserved for native asset transfer controls"),
            "unexpected Initial-executor rejection: {error:?}"
        );
    }
}
#[test]
fn initial_executor_rejects_reserved_native_metadata_during_account_registration() {
    let authority = checked_account_id();
    let world = World::with([], [Account::new(authority.clone()).build(&authority)], []);
    let state = state_after_genesis(world);
    let mut block = state.block(BlockHeader::new(
        nonzero!(2_u64),
        Some(
            state
                .view()
                .latest_block_hash()
                .expect("genuine genesis parent"),
        ),
        None,
        1,
        0,
    ));
    let mut state_transaction = block.transaction();
    for key in [
        ASSET_TRANSFER_CONTROL_METADATA_KEY,
        iroha_data_model::smart_contract::CONTRACT_DEPLOY_NONCE_METADATA_KEY,
    ] {
        let target = checked_account_id();
        let mut metadata = Metadata::default();
        metadata.insert(
            key.parse().expect("reserved metadata key"),
            Json::new(7_u64),
        );
        let instruction: InstructionBox =
            Register::account(NewAccount::new(target.clone()).with_metadata(metadata)).into();
        let error = super::Executor::Initial
            .execute_instruction(&mut state_transaction, &authority, instruction)
            .expect_err("Initial executor must reject native metadata seeding");
        assert!(
            error.to_string().contains("reserved native metadata"),
            "unexpected Initial-executor rejection: {error}"
        );
        assert!(state_transaction.world.account(&target).is_err());
    }
}
#[test]
fn initial_executor_rejects_account_removal_with_transfer_control_state() {
    let target = checked_account_id();
    let asset_definition = AssetDefinitionId::derive_from_components(
        DomainId::try_new("transfer", "controls").expect("domain id"),
        "rose".parse().expect("asset name"),
    );
    let mut record = AssetTransferControlRecord::new(asset_definition);
    record.blacklisted = true;
    let mut metadata = Metadata::default();
    metadata.insert(
        ASSET_TRANSFER_CONTROL_METADATA_KEY
            .parse()
            .expect("transfer-control metadata key"),
        Json::new(AssetTransferControlStoreV1 {
            controls: vec![record],
        }),
    );
    let world = World::with(
        [],
        [Account::new(target.clone())
            .with_metadata(metadata)
            .build(&target)],
        [],
    );
    let state = state_after_genesis(world);
    let mut block = state.block(BlockHeader::new(
        nonzero!(2_u64),
        Some(
            state
                .view()
                .latest_block_hash()
                .expect("genuine genesis parent"),
        ),
        None,
        1,
        0,
    ));
    let mut state_transaction = block.transaction();
    let error = super::Executor::Initial
        .execute_instruction(
            &mut state_transaction,
            &target,
            Unregister::account(target.clone()).into(),
        )
        .expect_err("Initial executor must preserve native transfer-control history");
    assert!(
        error.to_string().contains("dedicated instructions"),
        "unexpected Initial-executor rejection: {error}"
    );
    assert!(state_transaction.world.account(&target).is_ok());
}
#[test]
fn initial_executor_keeps_the_complete_vpn_lifecycle_allowlisted() {
    let source = include_str!("executor_initial_permission_authority.rs");
    let allowlist = source
        .split_once("fn initial_native_instruction_is_explicitly_admitted(")
        .expect("native Initial-executor classifier")
        .1
        .split_once("fn initial_genesis_instruction_is_explicitly_admitted(")
        .expect("end of native Initial-executor classifier")
        .0;
    for instruction in [
        "OpenVpnLeaseEscrow",
        "SettleVpnLease",
        "RefundExpiredVpnLease",
    ] {
        assert!(
            allowlist.contains(&format!("iroha_data_model::isi::vpn::{instruction},")),
            "Initial executor VPN lifecycle allowlist omitted {instruction}"
        );
    }
}
#[test]
fn initial_executor_validator_lifecycle_requires_owner_without_peer_management() {
    use iroha_data_model::isi::staking::{
        ActivatePublicLaneValidator, ExitPublicLaneValidator, RegisterPublicLaneValidator,
    };
    let authority = checked_account_id();
    let intruder = checked_account_id();
    let world = World::with([], [Account::new(authority.clone()).build(&authority)], []);
    let state = state_after_genesis(world);
    let mut block = state.block(BlockHeader::new(nonzero!(2_u64), None, None, 1, 0));
    let stx = block.transaction();
    let lane = iroha_model_base::topology::LaneId::SINGLE;
    let peer = iroha_model_base::peer::PeerId::new(authority.expect_single_signatory().clone());
    let stake_asset = AssetDefinitionId::derive_from_components(
        DomainId::try_new("staking", "universal").unwrap(),
        "stake".parse().unwrap(),
    );
    let escrow = checked_account_id();
    let instructions: [InstructionBox; 3] = [
        RegisterPublicLaneValidator::new(
            lane,
            authority.clone(),
            peer,
            authority.clone(),
            Quantity::from(1_u64),
            Metadata::default(),
            iroha_data_model::nexus::PublicLaneMonetaryPlanV1 {
                network_scope: iroha_data_model::nexus::PublicLaneMonetaryScopeV1::Network(
                    *state.network_id_ref(),
                ),
                valid_until_height: 2,
                source_asset: AssetId::of(stake_asset.clone(), authority.clone()),
                destination_asset: AssetId::of(stake_asset, escrow),
                amount: Quantity::one(),
                precondition:
                    iroha_data_model::nexus::PublicLaneMonetaryPreconditionV1::Registration(
                        iroha_data_model::nexus::PublicLaneMonetaryRegistrationV1 {
                            activation_height: 2,
                        },
                    ),
            },
        )
        .into(),
        ActivatePublicLaneValidator::new(lane, authority.clone()).into(),
        ExitPublicLaneValidator {
            lane_id: lane,
            validator: authority.clone(),
            release_at_ms: 10,
        }
        .into(),
    ];
    for instruction in instructions {
        validate_initial_native_instruction_authority(&stx, &authority, &instruction, false)
            .expect("self-owned lifecycle reaches Core");
        assert!(matches!(
            validate_initial_native_instruction_authority(&stx, &intruder, &instruction, false)
                .map_err(crate::execution_attempt::expect_completed_rejection),
            Err(ValidationFail::NotPermitted(_))
        ));
    }
}
#[test]
fn initial_executor_routes_self_authorized_public_lane_user_actions_to_core() {
    use iroha_data_model::isi::staking::{
        BondPublicLaneStake, ClaimPublicLaneRewards, FinalizePublicLaneUnbond,
        RebindPublicLaneValidatorPeer, SchedulePublicLaneUnbond,
    };

    use iroha_data_model::nexus::{
        PublicLaneMonetaryBondV1, PublicLaneMonetaryPlanV1, PublicLaneMonetaryPreconditionV1,
        PublicLaneMonetaryScopeV1, PublicLaneMonetaryUnbondV1, PublicLaneRewardClaimPlanV1,
        PublicLaneUnbonding, public_lane_unbonding_commitment,
    };
    let validator = checked_account_id();
    let staker = checked_account_id();
    let rebind_key = checked_keypair();
    let peer = iroha_model_base::peer::PeerId::new(rebind_key.public_key().clone());
    let rebind_consent = iroha_data_model::isi::staking::PublicLanePeerBindingAuthorization::new(
        executor_test_network_id(b"staking-classification-rebind"),
        iroha_model_base::topology::LaneId::SINGLE,
        validator.clone(),
        peer.clone(),
        1,
        iroha_model_base::peer::PeerId::new(validator.expect_single_signatory().clone()),
    );
    let request_id = Hash::prehashed([0xA5; Hash::LENGTH]);
    // This test checks dispatch and self-authority, while Core separately
    // authenticates these exact tenure, custody and withdrawal bindings.
    let stake_asset = AssetDefinitionId::derive_from_components(
        DomainId::try_new("staking", "universal").unwrap(),
        "stake".parse().unwrap(),
    );
    let escrow = checked_account_id();
    let staker_asset = AssetId::of(stake_asset.clone(), staker.clone());
    let escrow_asset = AssetId::of(stake_asset, escrow);
    let network_scope =
        PublicLaneMonetaryScopeV1::Network(executor_test_network_id(b"public lane user dispatch"));
    let request = PublicLaneUnbonding {
        request_id,
        amount: Quantity::one(),
        release_at_ms: 1,
        slashable_through_height: 1,
        liability_release_height: 2,
    };
    let bond_plan = PublicLaneMonetaryPlanV1 {
        network_scope,
        valid_until_height: 2,
        source_asset: staker_asset.clone(),
        destination_asset: escrow_asset.clone(),
        amount: Quantity::one(),
        precondition: PublicLaneMonetaryPreconditionV1::Bond(PublicLaneMonetaryBondV1 {
            activation_height: 1,
            peer_id: peer.clone(),
        }),
    };
    let unbond_plan = PublicLaneMonetaryPlanV1 {
        network_scope,
        valid_until_height: 2,
        source_asset: escrow_asset,
        destination_asset: staker_asset,
        amount: request.amount.clone(),
        precondition: PublicLaneMonetaryPreconditionV1::Unbond(PublicLaneMonetaryUnbondV1 {
            activation_height: 1,
            request_hash: public_lane_unbonding_commitment(&request).unwrap(),
        }),
    };
    let instructions: [InstructionBox; 5] = [
        RebindPublicLaneValidatorPeer::new(
            iroha_model_base::topology::LaneId::SINGLE,
            validator.clone(),
            peer,
            iroha_crypto::SignatureOf::try_new(rebind_key.private_key(), &rebind_consent)
                .expect("staking classification peer consent"),
        )
        .into(),
        BondPublicLaneStake {
            lane_id: iroha_model_base::topology::LaneId::SINGLE,
            validator: validator.clone(),
            staker: staker.clone(),
            amount: Quantity::from(1_u32),
            metadata: Metadata::default(),
            monetary_plan: bond_plan,
        }
        .into(),
        SchedulePublicLaneUnbond {
            lane_id: iroha_model_base::topology::LaneId::SINGLE,
            validator: validator.clone(),
            staker: staker.clone(),
            request_id,
            amount: Quantity::from(1_u32),
            release_at_ms: 1,
        }
        .into(),
        FinalizePublicLaneUnbond {
            lane_id: iroha_model_base::topology::LaneId::SINGLE,
            validator,
            staker: staker.clone(),
            request_id,
            monetary_plan: unbond_plan.clone(),
        }
        .into(),
        ClaimPublicLaneRewards {
            lane_id: iroha_model_base::topology::LaneId::SINGLE,
            account: staker.clone(),
            claim_plan: PublicLaneRewardClaimPlanV1 {
                fee_claim: iroha_data_model::nexus::PublicLaneFeeRewardClaimV1 {
                    lifecycle_seal: [1; 32],
                    beneficiary_id: staker.clone(),
                    beneficiary_revision: 0,
                    source_asset: unbond_plan.source_asset.clone(),
                    destination_asset: unbond_plan.destination_asset.clone(),
                    amount: Quantity::one(),
                    expected_claim_sequence: 0,
                },
                network_scope,
                valid_until_height: 2,
            },
        }
        .into(),
    ];
    for instruction in &instructions {
        assert!(
            initial_native_instruction_is_explicitly_admitted(instruction),
            "{} must reach its exact self-authority and state checks in Core",
            instruction.id()
        );
        assert!(
            !initial_genesis_instruction_is_explicitly_admitted(instruction),
            "{} is an ordinary signed user action, not a genesis-only administrative path",
            instruction.id()
        );
    }
}
#[test]
#[allow(clippy::too_many_lines)]
fn initial_executor_routes_exact_scoped_governance_isis_through_core_authorization() {
    use iroha_data_model::governance::types::{AbiVersion, ContractAbiHash, ContractCodeHash};
    use iroha_data_model::isi::governance as gov;
    use iroha_executor_data_model::permission::governance::{
        CanProposeContractDeployment, CanProposeRuntimeChangeSet, CanRestituteGovernanceLock,
        CanSlashGovernanceLock, CanSubmitGovernanceBallot,
    };
    let authority = checked_account_id();
    let citizen_target = checked_account_id();
    let account = Account::new(authority.clone()).build(&authority);
    let mut state = state_after_genesis(World::with([], [account], []));
    let network_id = state.network_id;
    let contract_address =
        ContractAddress::derive(&network_id, &authority, 1, DataSpaceId::UNIVERSAL)
            .expect("canonical contract address");
    let other_contract_address =
        ContractAddress::derive(&network_id, &authority, 2, DataSpaceId::UNIVERSAL)
            .expect("second canonical contract address");
    let abi_hash = ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1);
    // A change set of another network passes authorization and fails Core admission.
    let foreign_network = iroha_data_model::NetworkId::from_genesis_hash(
        iroha_crypto::HashOf::from_untyped_unchecked(Hash::prehashed([0xEE; 32])),
    );
    let referendum_id = "exact-governance-scope".to_owned();
    let probes: Vec<(InstructionBox, &str, &str)> = vec![
        (
            gov::ProposeDeployContract {
                contract_address: contract_address.clone(),
                code_hash: ContractCodeHash::new([0x11; 32]),
                abi_hash: ContractAbiHash::new(abi_hash),
                abi_version: AbiVersion::new(2),
                manifest_provenance: None,
            }
            .into(),
            "CanProposeContractDeployment",
            "abi_version must be exactly 1",
        ),
        (
            gov::ProposeRuntimeUpgradeProposal {
                action: crate::runtime::change_set::test_support::schedule(foreign_network, 100),
                provenance: Vec::new(),
            }
            .into(),
            "CanProposeRuntimeChangeSet",
            "the change set belongs to another network",
        ),
        (
            gov::CastZkBallot {
                election_id: referendum_id.clone(),
                proof_b64: "AA==".to_owned(),
                public_inputs_json: r#"{"unknown":"field"}"#.to_owned(),
            }
            .into(),
            "CanSubmitGovernanceBallot",
            "unknown field",
        ),
        (
            gov::SlashGovernanceLock {
                referendum_id: referendum_id.clone(),
                owner: citizen_target.clone(),
                amount: Quantity::zero(),
                reason: "scope regression".to_owned(),
            }
            .into(),
            "CanSlashGovernanceLock",
            "slash amount must be > 0",
        ),
        (
            gov::RestituteGovernanceLock {
                referendum_id: referendum_id.clone(),
                owner: citizen_target.clone(),
                amount: Quantity::zero(),
                reason: "scope regression".to_owned(),
            }
            .into(),
            "CanRestituteGovernanceLock",
            "restitution amount must be > 0",
        ),
    ];
    for (instruction, _, _) in &probes {
        assert!(
            initial_native_instruction_is_explicitly_admitted(instruction),
            "{} must be admitted to its Core authorization gate",
            instruction.id()
        );
    }
    let mut world = state.world.block();
    world.account_permissions.insert(
        authority.clone(),
        BTreeSet::from([
            Permission::from(CanProposeContractDeployment {
                contract_address: other_contract_address,
            }),
            Permission::from(CanSubmitGovernanceBallot {
                referendum_id: "other-governance-scope".to_owned(),
            }),
            Permission::from(CanSlashGovernanceLock {
                referendum_id: "other-governance-scope".to_owned(),
            }),
            Permission::from(CanRestituteGovernanceLock {
                referendum_id: "other-governance-scope".to_owned(),
            }),
        ]),
    );
    world.commit();
    state.gov.citizenship_bond_amount = Quantity::zero();
    let mut block = state.block(BlockHeader::new(
        nonzero!(2_u64),
        Some(
            state
                .view()
                .latest_block_hash()
                .expect("genuine genesis parent"),
        ),
        None,
        0,
        0,
    ));
    let mut state_transaction = block.transaction();
    crate::runtime::change_set::test_support::commit_policy(
        &mut state_transaction,
        &crate::runtime::change_set::test_support::short_policy(1),
    );
    for (instruction, permission_name, _) in &probes {
        let error = super::Executor::Initial
            .execute_instruction(&mut state_transaction, &authority, instruction.clone())
            .expect_err("an adjacent governance scope must fail closed");
        assert!(
            format!("{error:?}").contains(&format!("exact {permission_name} target"))
                || (*permission_name == "CanProposeRuntimeChangeSet"
                    && format!("{error:?}").contains("CanProposeRuntimeChangeSet required")),
            "unexpected wrong-scope {permission_name} rejection: {error:?}"
        );
    }
    assert!(
        state_transaction
            .world
            .governance_proposals
            .iter()
            .next()
            .is_none()
    );
    assert!(
        state_transaction
            .world
            .governance_referenda
            .iter()
            .next()
            .is_none()
    );
    assert!(state_transaction.world.elections.iter().next().is_none());
    assert!(
        state_transaction
            .world
            .governance_locks
            .iter()
            .next()
            .is_none()
    );
    assert!(
        state_transaction
            .world
            .governance_slashes
            .iter()
            .next()
            .is_none()
    );
    assert!(state_transaction.world.citizens.iter().next().is_none());
    state_transaction.world.account_permissions.insert(
        authority.clone(),
        BTreeSet::from([
            Permission::from(CanProposeContractDeployment { contract_address }),
            Permission::from(CanProposeRuntimeChangeSet),
            Permission::from(CanSubmitGovernanceBallot {
                referendum_id: referendum_id.clone(),
            }),
            Permission::from(CanSlashGovernanceLock {
                referendum_id: referendum_id.clone(),
            }),
            Permission::from(CanRestituteGovernanceLock { referendum_id }),
        ]),
    );
    for (instruction, permission_name, downstream_error) in probes {
        let error = super::Executor::Initial
            .execute_instruction(&mut state_transaction, &authority, instruction)
            .expect_err("the exact scope must reach the deliberately invalid Core probe");
        assert!(
            format!("{error:?}").contains(downstream_error),
            "exact {permission_name} scope did not reach Core validation: {error:?}"
        );
    }
}
#[test]
fn initial_executor_keeps_low_level_submit_ballot_behind_the_ivm_latch() {
    let backend = "halo2/ipa";
    let proof = iroha_data_model::proof::ProofAttachment::new_ref(
        backend.into(),
        iroha_data_model::proof::ProofBox::new(backend.into(), vec![0x01]),
        iroha_data_model::proof::VerifyingKeyId::new(backend, "ballot-v1"),
    );
    let instruction: InstructionBox = iroha_data_model::isi::zk::SubmitBallot {
        election_id: "latched-election".to_owned(),
        ciphertext: vec![0x02],
        ballot_proof: proof,
        nullifier: [0x03; 32],
    }
    .into();
    assert!(
        !initial_native_instruction_is_explicitly_admitted(&instruction),
        "direct signed SubmitBallot would bypass the IVM host's one-shot verification latch"
    );
}
#[test]
fn initial_executor_keeps_explicitly_standalone_referendum_ballots() {
    use iroha_data_model::isi::governance::{CastPlainBallot, CastZkBallot, UpdatePlainConviction};

    let ballots = [
        InstructionBox::from(CastPlainBallot {
            referendum_id: "standalone-plain".to_owned(),
            owner: checked_account_id(),
            amount: 1_u64.into(),
            duration_blocks: 1,
            direction: 0,
        }),
        InstructionBox::from(UpdatePlainConviction {
            referendum_id: "standalone-plain".to_owned(),
            owner: checked_account_id(),
            amount: 2_u64.into(),
            duration_blocks: 2,
        }),
        InstructionBox::from(CastZkBallot {
            election_id: "standalone-zk".to_owned(),
            proof_b64: "AA==".to_owned(),
            public_inputs_json: "{}".to_owned(),
        }),
    ];
    for ballot in &ballots {
        assert!(
            initial_native_instruction_is_explicitly_admitted(ballot),
            "standalone referendum ballot must remain reachable after Parliament cutover"
        );
    }
}
#[test]
fn initial_executor_admits_public_ministry_agenda_submissions() {
    use iroha_data_model::{
        isi::ministry::SubmitAgendaProposal,
        ministry::{
            AGENDA_PROPOSAL_VERSION_V1, AgendaProposalAction, AgendaProposalSubmitter,
            AgendaProposalSummary, AgendaProposalV1,
        },
    };
    let instruction: InstructionBox = SubmitAgendaProposal {
        proposal: AgendaProposalV1 {
            version: AGENDA_PROPOSAL_VERSION_V1,
            proposal_id: "AC-2026-001".to_owned(),
            submitted_at_unix_ms: 1,
            language: "en".to_owned(),
            action: AgendaProposalAction::AmendPolicy,
            summary: AgendaProposalSummary {
                title: "Public agenda proposal".to_owned(),
                motivation: "Exercise the signed public submission surface.".to_owned(),
                expected_impact: "The initial executor reaches exact Core validation.".to_owned(),
            },
            tags: Vec::new(),
            targets: Vec::new(),
            evidence: Vec::new(),
            submitter: AgendaProposalSubmitter {
                name: "Citizen".to_owned(),
                contact: "citizen@example.org".to_owned(),
                organization: None,
                pgp_fingerprint: None,
            },
            duplicates: Vec::new(),
        },
    }
    .into();
    assert!(
        initial_native_instruction_is_explicitly_admitted(&instruction),
        "signed public Ministry proposals must reach exact Core validation"
    );
}
#[test]
fn initial_executor_denies_chain_and_foreign_controller_takeover_paths() {
    let attacker = checked_account_id();
    let victim = checked_account_id();
    let world = World::with(
        [],
        [
            Account::new(attacker.clone()).build(&attacker),
            Account::new(victim.clone()).build(&victim),
        ],
        [],
    );
    let state = state_for_testing(world);
    let mut block = state.block(BlockHeader::new(nonzero!(2_u64), None, None, 0, 0));
    let mut state_transaction = block.transaction();
    let custom_id: CustomParameterId = "attacker_parameter".parse().expect("parameter id");
    let set_parameter =
        iroha_data_model::isi::SetParameter::new(iroha_data_model::parameter::Parameter::Custom(
            CustomParameter::new(custom_id, Json::new(1_u32)),
        ));
    let upgrade = iroha_data_model::isi::Upgrade::new(data_model_executor::Executor::new(
        IvmBytecode::from_compiled(generate_denied_program("attacker executor")),
    ));
    let foreign_signatory = checked_keypair().public_key().clone();
    let instructions: Vec<InstructionBox> = vec![
        set_parameter.into(),
        upgrade.into(),
        iroha_data_model::isi::AddSignatory::new(victim.clone(), foreign_signatory.clone()).into(),
        iroha_data_model::isi::RemoveSignatory::new(victim.clone(), foreign_signatory).into(),
        iroha_data_model::isi::SetAccountQuorum::new(
            victim.clone(),
            std::num::NonZeroU16::new(1).expect("quorum"),
        )
        .into(),
    ];
    for instruction in instructions {
        let error = super::Executor::Initial
            .execute_instruction(&mut state_transaction, &attacker, instruction)
            .expect_err("an ordinary account must not mutate chain policy or a foreign controller");
        assert!(
            matches!(error, ValidationFail::NotPermitted(_)),
            "{error:?}"
        );
    }
    assert!(matches!(
        &*state_transaction.world.executor,
        super::Executor::Initial
    ));
    assert!(state_transaction.world.account(&victim).is_ok());
}
#[test]
fn initial_executor_reserves_hijiri_parameters_for_dedicated_permission() {
    use iroha_data_model::hijiri::{FeeMultiplierBand, HijiriFeePolicy, HijiriParametersV1, Q16};
    let generic_parameter_admin = checked_account_id();
    let hijiri_admin = checked_account_id();
    let mut world = World::with(
        [],
        [
            Account::new(generic_parameter_admin.clone()).build(&generic_parameter_admin),
            Account::new(hijiri_admin.clone()).build(&hijiri_admin),
        ],
        [],
    );
    world.account_permissions.insert(
        generic_parameter_admin.clone(),
        BTreeSet::from([executor_permission::parameter::CanSetParameters.into()]),
    );
    world.account_permissions.insert(
        hijiri_admin.clone(),
        BTreeSet::from([executor_permission::parameter::CanSetHijiriParameters.into()]),
    );
    let state = state_after_genesis(world);
    let mut block = state.block(BlockHeader::new(
        nonzero!(2_u64),
        Some(
            state
                .view()
                .latest_block_hash()
                .expect("genuine genesis parent"),
        ),
        None,
        0,
        0,
    ));
    let mut state_transaction = block.transaction();
    let parameters = HijiriParametersV1::try_new(
        1,
        None,
        HijiriFeePolicy::new(
            vec![FeeMultiplierBand::new(Q16::ONE, Q16::ONE).unwrap()],
            Q16::ONE,
        )
        .unwrap(),
        Q16::ONE,
    )
    .unwrap();
    let instruction = || {
        InstructionBox::from(iroha_data_model::isi::SetParameter::new(
            iroha_data_model::parameter::Parameter::Custom(
                parameters.clone().into_custom_parameter(),
            ),
        ))
    };
    let error = super::Executor::Initial
        .execute_instruction(
            &mut state_transaction,
            &generic_parameter_admin,
            instruction(),
        )
        .expect_err("generic parameter permission must not control Hijiri fees");
    assert!(
        matches!(error, ValidationFail::NotPermitted(ref message)
                if message.contains("CanSetHijiriParameters")),
        "unexpected generic-admin rejection: {error:?}"
    );
    super::Executor::Initial
        .execute_instruction(&mut state_transaction, &hijiri_admin, instruction())
        .expect("dedicated Hijiri permission must install the initial parameter");
    assert!(
        state_transaction
            .world
            .parameters()
            .custom()
            .contains_key(&HijiriParametersV1::parameter_id())
    );
}
#[test]
fn initial_executor_treats_sccp_proposer_permission_as_genesis_only() {
    let permission: Permission = executor_permission::sccp::CanProposeSccpRouteGovernance.into();
    assert!(is_builtin_initial_permission_name(permission.name()));
    assert!(
        initial_permission_is_genesis_only(&permission),
        "SCCP route governance proposers are granted and revoked only in genesis"
    );
}
#[test]
#[allow(clippy::too_many_lines)]
fn initial_executor_enforces_capability_roots_for_every_scoped_permission() {
    use iroha_data_model::{
        account::rekey::AccountAliasDomain,
        events::execute_trigger::ExecuteTriggerEventFilter,
        nexus::UniversalAccountId,
        trigger::action::{Action, Repeats},
    };
    use iroha_executor_data_model::permission::account::AccountAliasPermissionScope;
    let legitimate_root = checked_account_id();
    let adjacent_owner = checked_account_id();
    let delegation_sink = checked_account_id();
    let governed_domain = DomainId::try_new("grant_policy", "universal").expect("governed domain");
    let adjacent_domain =
        DomainId::try_new("adjacent_owner", "universal").expect("adjacent domain");
    let asset_definition = AssetDefinitionId::derive_from_components(
        governed_domain.clone(),
        "root_asset".parse().expect("asset name"),
    );
    let root_asset = AssetId::new(asset_definition.clone(), legitimate_root.clone());
    // The exact mint permission deliberately names the attacker as destination. Mint
    // authority belongs to the definition owner, never to the destination account.
    let nft_id = NftId::new(
        governed_domain.clone(),
        "root_nft".parse().expect("NFT name"),
    );
    let trigger_id: TriggerId = "grant_policy_trigger".parse().expect("trigger id");
    let world = World::with_assets(
        [
            Domain::new(governed_domain.clone()).build(&legitimate_root),
            Domain::new(adjacent_domain.clone()).build(&adjacent_owner),
        ],
        [
            Account::new(legitimate_root.clone()).build(&legitimate_root),
            Account::new(adjacent_owner.clone()).build(&adjacent_owner),
            Account::new(delegation_sink.clone()).build(&delegation_sink),
        ],
        [AssetDefinition::numeric(
            asset_definition.clone(),
            "root asset".to_owned(),
            iroha_data_model::asset::AssetBalancePolicy::Global,
            None,
        )
        .build(&legitimate_root)],
        [],
        [Nft::new(nft_id.clone(), Metadata::default()).build(&legitimate_root)],
    );
    let state = state_after_genesis(world);
    // The address deliberately embeds the attacker as its subject. Contract subjects are
    // not code-management authorities and therefore cannot mint invocation permissions.
    let contract = ContractAddress::derive(
        &state.network_id,
        &adjacent_owner,
        77,
        DataSpaceId::UNIVERSAL,
    )
    .expect("contract address");
    let dataspace = DataSpaceId::new(7);
    let manifest_root: Permission =
        executor_permission::nexus::CanPublishSpaceDirectoryManifest { dataspace }.into();
    let contract_proposal_permission: Permission =
        executor_permission::governance::CanProposeContractDeployment {
            contract_address: contract.clone(),
        }
        .into();
    let runtime_proposal_permission: Permission =
        executor_permission::governance::CanProposeRuntimeChangeSet.into();
    let ballot_permission: Permission =
        executor_permission::governance::CanSubmitGovernanceBallot {
            referendum_id: "grant-policy-referendum".to_owned(),
        }
        .into();
    let slash_permission: Permission = executor_permission::governance::CanSlashGovernanceLock {
        referendum_id: "grant-policy-referendum".to_owned(),
    }
    .into();
    let restitute_permission: Permission =
        executor_permission::governance::CanRestituteGovernanceLock {
            referendum_id: "grant-policy-referendum".to_owned(),
        }
        .into();
    let mut world = state.world.block();
    world.account_permissions.insert(
        legitimate_root.clone(),
        BTreeSet::from([
            executor_permission::smart_contract::CanManageSmartContractCode.into(),
            executor_permission::settlement::CanManageFxCorridors.into(),
            manifest_root,
            contract_proposal_permission.clone(),
            runtime_proposal_permission.clone(),
            ballot_permission.clone(),
            slash_permission.clone(),
            restitute_permission.clone(),
        ]),
    );
    world.commit();
    let mut block = state.block(BlockHeader::new(
        nonzero!(2_u64),
        Some(
            state
                .view()
                .latest_block_hash()
                .expect("genuine genesis parent"),
        ),
        None,
        0,
        0,
    ));
    let mut state_transaction = block.transaction();
    seed_owner_permission_contract(
        &mut state_transaction,
        &contract,
        &legitimate_root,
        Hash::new(b"scoped contract capability fixture"),
    );
    Register::trigger(Trigger::new(
        trigger_id.clone(),
        Action::new(
            Vec::<InstructionBox>::new(),
            Repeats::Indefinitely,
            legitimate_root.clone(),
            ExecuteTriggerEventFilter::new()
                .for_trigger(trigger_id.clone())
                .under_authority(legitimate_root.clone()),
        )
        .expect("trigger action fixture satisfies validation invariants"),
    ))
    .execute(&legitimate_root, &mut state_transaction)
    .expect("seed trigger authority");
    let alias_scope = AccountAliasPermissionScope::Domain(governed_domain.clone());
    let asset_alias_scope =
        executor_permission::asset_definition::AssetDefinitionAliasPermissionScope::Domain(
            governed_domain.clone(),
        );
    let program_id = FeeSponsorProgramId::new(
        legitimate_root.clone(),
        "root_program".parse().expect("program name"),
    );
    let policy_id: Name = "root_policy".parse().expect("policy name");
    // `false` means that the permission intentionally has no ownership-derived root and must
    // be bootstrapped once, after which only exact holders may propagate it.
    let cases: Vec<(&str, Permission, bool)> = vec![
        (
            "CanUnregisterDomain",
            executor_permission::domain::CanUnregisterDomain {
                domain: governed_domain.clone(),
            }
            .into(),
            true,
        ),
        (
            "CanModifyDomainMetadata",
            executor_permission::domain::CanModifyDomainMetadata {
                domain: governed_domain.clone(),
            }
            .into(),
            true,
        ),
        (
            "CanRegisterAccount",
            executor_permission::account::CanRegisterAccount {
                domain: governed_domain.clone(),
            }
            .into(),
            true,
        ),
        (
            "CanUnregisterAccount",
            executor_permission::account::CanUnregisterAccount {
                account: legitimate_root.clone(),
            }
            .into(),
            true,
        ),
        (
            "CanModifyAccountMetadata",
            executor_permission::account::CanModifyAccountMetadata {
                account: legitimate_root.clone(),
            }
            .into(),
            true,
        ),
        (
            "CanReplaceAccountController",
            executor_permission::account::CanReplaceAccountController {
                account: legitimate_root.clone(),
            }
            .into(),
            true,
        ),
        (
            "CanResolveAccountAlias",
            executor_permission::account::CanResolveAccountAlias {
                scope: alias_scope.clone(),
            }
            .into(),
            true,
        ),
        (
            "CanDelegateAccountAliasResolution",
            executor_permission::account::CanDelegateAccountAliasResolution {
                scope: alias_scope.clone(),
            }
            .into(),
            true,
        ),
        (
            "CanManageAccountAlias",
            executor_permission::account::CanManageAccountAlias { scope: alias_scope }.into(),
            true,
        ),
        (
            "CanManageAssetDefinitionAlias",
            executor_permission::asset_definition::CanManageAssetDefinitionAlias {
                scope: asset_alias_scope,
            }
            .into(),
            true,
        ),
        (
            "CanUnregisterAssetDefinition",
            executor_permission::asset_definition::CanUnregisterAssetDefinition {
                asset_definition: asset_definition.clone(),
            }
            .into(),
            true,
        ),
        (
            "CanModifyAssetDefinitionMetadata",
            executor_permission::asset_definition::CanModifyAssetDefinitionMetadata {
                asset_definition: asset_definition.clone(),
            }
            .into(),
            true,
        ),
        (
            "CanManageAssetDefinitionConfidentialPolicy",
            executor_permission::asset_definition::CanManageAssetDefinitionConfidentialPolicy {
                asset_definition: asset_definition.clone(),
            }
            .into(),
            true,
        ),
        (
            "CanMintAssetWithDefinition",
            executor_permission::asset::CanMintAssetWithDefinition {
                asset_definition: asset_definition.clone(),
            }
            .into(),
            true,
        ),
        (
            "CanBurnAssetWithDefinition",
            executor_permission::asset::CanBurnAssetWithDefinition {
                asset_definition: asset_definition.clone(),
            }
            .into(),
            true,
        ),
        (
            "CanTransferAssetWithDefinition",
            executor_permission::asset::CanTransferAssetWithDefinition {
                asset_definition: asset_definition.clone(),
            }
            .into(),
            true,
        ),
        (
            "CanModifyAssetMetadataWithDefinition",
            executor_permission::asset::CanModifyAssetMetadataWithDefinition {
                asset_definition: asset_definition.clone(),
            }
            .into(),
            true,
        ),
        (
            "CanSetAssetTransferAvailability",
            executor_permission::asset::CanSetAssetTransferAvailability {
                account: adjacent_owner.clone(),
                asset_definition: asset_definition.clone(),
            }
            .into(),
            true,
        ),
        (
            "CanSetAssetTransferDailyLimit",
            executor_permission::asset::CanSetAssetTransferDailyLimit {
                asset_definition: asset_definition.clone(),
                account_domain: AccountAliasDomain::new(
                    "retail".parse().expect("account alias domain"),
                ),
                account_dataspace: dataspace,
            }
            .into(),
            true,
        ),
        (
            "CanSetAssetHoldingLimit",
            executor_permission::asset::CanSetAssetHoldingLimit {
                account: adjacent_owner.clone(),
                asset_definition: asset_definition.clone(),
            }
            .into(),
            true,
        ),
        (
            "CanMintAssetToAccount",
            executor_permission::asset::CanMintAssetToAccount {
                asset_definition: asset_definition.clone(),
                account: adjacent_owner.clone(),
            }
            .into(),
            true,
        ),
        (
            "CanBurnAsset",
            executor_permission::asset::CanBurnAsset {
                asset: root_asset.clone(),
            }
            .into(),
            true,
        ),
        (
            "CanTransferAsset",
            executor_permission::asset::CanTransferAsset {
                asset: root_asset.clone(),
            }
            .into(),
            true,
        ),
        (
            "CanModifyAssetMetadata",
            executor_permission::asset::CanModifyAssetMetadata {
                asset: root_asset.clone(),
            }
            .into(),
            true,
        ),
        (
            "CanRegisterNft",
            executor_permission::nft::CanRegisterNft {
                domain: governed_domain.clone(),
            }
            .into(),
            true,
        ),
        (
            "CanUnregisterNft",
            executor_permission::nft::CanUnregisterNft {
                nft: nft_id.clone(),
            }
            .into(),
            true,
        ),
        (
            "CanTransferNft",
            executor_permission::nft::CanTransferNft {
                nft: nft_id.clone(),
            }
            .into(),
            true,
        ),
        (
            "CanModifyNftMetadata",
            executor_permission::nft::CanModifyNftMetadata {
                nft: nft_id.clone(),
            }
            .into(),
            true,
        ),
        (
            "CanRegisterTrigger",
            executor_permission::trigger::CanRegisterTrigger {
                authority: legitimate_root.clone(),
            }
            .into(),
            true,
        ),
        (
            "CanUnregisterTrigger",
            executor_permission::trigger::CanUnregisterTrigger {
                trigger: trigger_id.clone(),
            }
            .into(),
            true,
        ),
        (
            "CanModifyTrigger",
            executor_permission::trigger::CanModifyTrigger {
                trigger: trigger_id.clone(),
            }
            .into(),
            true,
        ),
        (
            "CanExecuteTrigger",
            executor_permission::trigger::CanExecuteTrigger {
                trigger: trigger_id.clone(),
            }
            .into(),
            true,
        ),
        (
            "CanModifyTriggerMetadata",
            executor_permission::trigger::CanModifyTriggerMetadata {
                trigger: trigger_id,
            }
            .into(),
            true,
        ),
        (
            "CanInvokeContractEntrypoint",
            executor_permission::smart_contract::CanInvokeContractEntrypoint {
                contract: contract.clone(),
                entrypoint: "hajimari".to_owned(),
            }
            .into(),
            true,
        ),
        (
            "CanUseContractPermission",
            executor_permission::smart_contract::CanUseContractPermission {
                contract,
                permission: "write".parse().unwrap(),
            }
            .into(),
            true,
        ),
        (
            "CanExecuteSettlement",
            executor_permission::settlement::CanExecuteSettlement {
                debited_asset: root_asset,
                settlement_id: iroha_data_model::isi::SettlementId::new(
                    legitimate_root.clone(),
                    "grant_policy_settlement"
                        .parse()
                        .expect("settlement reference"),
                ),
                intent_hash: Hash::new(b"grant-policy-settlement"),
            }
            .into(),
            true,
        ),
        (
            "CanSetFxCorridorPolicy",
            executor_permission::settlement::CanSetFxCorridorPolicy {
                policy_id: policy_id.clone(),
            }
            .into(),
            true,
        ),
        (
            "CanPublishSpaceDirectoryManifest",
            executor_permission::nexus::CanPublishSpaceDirectoryManifest { dataspace }.into(),
            false,
        ),
        (
            "CanPublishSpaceDirectoryManifestForUaid",
            executor_permission::nexus::CanPublishSpaceDirectoryManifestForUaid {
                dataspace,
                uaid: UniversalAccountId::from_hash(Hash::new(b"grant-policy-uaid")),
            }
            .into(),
            true,
        ),
        (
            "CanPublishSpaceDirectoryManifestForAccountDomain",
            executor_permission::nexus::CanPublishSpaceDirectoryManifestForAccountDomain {
                dataspace,
                // The attacker owns this domain, but only the explicit dataspace-wide parent
                // is a delegation root for this permission.
                domain: adjacent_domain,
            }
            .into(),
            true,
        ),
        (
            "CanManageFeeSponsorProgram",
            executor_permission::nexus::CanManageFeeSponsorProgram {
                sponsor: legitimate_root.clone(),
            }
            .into(),
            true,
        ),
        (
            "CanEnrollFeeSponsorProgram",
            executor_permission::nexus::CanEnrollFeeSponsorProgram { program_id }.into(),
            true,
        ),
        (
            "CanProposeContractDeployment",
            contract_proposal_permission,
            false,
        ),
        (
            "CanProposeRuntimeChangeSet",
            runtime_proposal_permission,
            false,
        ),
        ("CanSubmitGovernanceBallot", ballot_permission, false),
        ("CanSlashGovernanceLock", slash_permission, false),
        ("CanRestituteGovernanceLock", restitute_permission, false),
    ];
    assert_eq!(cases.len(), 47, "update this table for every scoped arm");
    assert_eq!(
        cases
            .iter()
            .map(|(name, _, _)| *name)
            .collect::<BTreeSet<_>>()
            .len(),
        cases.len(),
        "permission cases must be unique",
    );
    for name in [
        "CanProposeContractDeployment",
        "CanSubmitGovernanceBallot",
        "CanSlashGovernanceLock",
        "CanRestituteGovernanceLock",
    ] {
        let malformed = Permission::new(name.to_owned(), Json::new(()));
        let error = initial_permission_capability_root_authority(
            &state_transaction,
            &legitimate_root,
            &malformed,
        )
        .map_err(crate::execution_attempt::expect_completed_rejection)
        .expect_err("malformed scoped governance payload must fail closed");
        assert!(
            matches!(&error, ValidationFail::NotPermitted(message)
                    if message.contains(name) && message.contains("Invalid permission payload")),
            "unexpected malformed {name} rejection: {error:?}"
        );
        super::Executor::Initial
            .execute_instruction(
                &mut state_transaction,
                &legitimate_root,
                Grant::account_permission(malformed, adjacent_owner.clone()).into(),
            )
            .expect_err("malformed scoped governance grant must fail before storage");
        assert!(
            !state_transaction
                .world
                .account_permissions_iter(&adjacent_owner)
                .expect("adjacent account permissions")
                .any(|permission| permission.name() == name),
            "malformed {name} permission reached account storage"
        );
    }
    for (name, permission, expected_root) in &cases {
        assert_eq!(permission.name(), *name);
        assert_eq!(
            initial_permission_capability_root_authority(
                &state_transaction,
                &legitimate_root,
                permission,
            )
            .expect("legitimate root lookup"),
            Some(*expected_root),
            "wrong capability root decision for {name}",
        );
        assert!(
            initial_permission_delegation_allowed(&state_transaction, &legitimate_root, permission)
                .expect("legitimate delegation lookup"),
            "legitimate authority must be able to grant {name}",
        );
        assert_eq!(
            initial_permission_capability_root_authority(
                &state_transaction,
                &adjacent_owner,
                permission,
            )
            .expect("adjacent-owner root lookup"),
            Some(false),
            "adjacent ownership must not root {name}",
        );
        assert!(
            !initial_permission_delegation_allowed(&state_transaction, &adjacent_owner, permission)
                .expect("adjacent-owner delegation lookup"),
            "unprivileged authority must not self-grant {name}",
        );
        super::Executor::Initial
            .execute_instruction(
                &mut state_transaction,
                &adjacent_owner,
                Grant::account_permission(permission.clone(), adjacent_owner.clone()).into(),
            )
            .expect_err("adjacent owner must not self-grant a scoped permission");
    }
    for (name, permission, _) in &cases {
        super::Executor::Initial
            .execute_instruction(
                &mut state_transaction,
                &legitimate_root,
                Grant::account_permission(permission.clone(), adjacent_owner.clone()).into(),
            )
            .unwrap_or_else(|error| panic!("legitimate root could not grant {name}: {error}"));
        let holder_delegable = !matches!(
            *name,
            "CanResolveAccountAlias"
                | "CanExecuteSettlement"
                | "CanSetFxCorridorPolicy"
                | "CanManageFeeSponsorProgram"
        );
        assert_eq!(
            initial_permission_delegation_allowed(&state_transaction, &adjacent_owner, permission)
                .expect("exact-holder delegation lookup"),
            holder_delegable,
            "wrong exact-holder delegation decision for {name}",
        );
        if !holder_delegable {
            let error = super::Executor::Initial
                .execute_instruction(
                    &mut state_transaction,
                    &adjacent_owner,
                    Grant::account_permission(permission.clone(), delegation_sink.clone()).into(),
                )
                .expect_err("a leaf holder must not bypass its dedicated delegation root");
            assert!(
                matches!(error, ValidationFail::NotPermitted(_)),
                "{error:?}"
            );
            assert!(
                !state_transaction
                    .world
                    .account_permissions_iter(&delegation_sink)
                    .expect("delegation sink permissions")
                    .any(|candidate| candidate == permission),
                "unauthorized {name} grant reached storage",
            );
        }
        super::Executor::Initial
            .execute_instruction(
                &mut state_transaction,
                &legitimate_root,
                Revoke::account_permission(permission.clone(), adjacent_owner.clone()).into(),
            )
            .unwrap_or_else(|error| panic!("legitimate root could not revoke {name}: {error}"));
    }
}
#[test]
fn initial_executor_rejects_noncanonical_governance_permission_selectors_before_storage() {
    use iroha_executor_data_model::permission::governance::{
        CanRestituteGovernanceLock, CanSlashGovernanceLock, CanSubmitGovernanceBallot,
    };
    let authority = checked_account_id();
    let destination = checked_account_id();
    let invalid_permissions = vec![
        Permission::from(CanSubmitGovernanceBallot {
            referendum_id: ".hidden-ballot".to_owned(),
        }),
        Permission::from(CanSlashGovernanceLock {
            referendum_id: "slash/alias".to_owned(),
        }),
        Permission::from(CanRestituteGovernanceLock {
            referendum_id: "restitution\nalias".to_owned(),
        }),
    ];
    let mut world = World::with(
        [],
        [
            Account::new(authority.clone()).build(&authority),
            Account::new(destination.clone()).build(&destination),
        ],
        [],
    );
    // Seed the malformed tokens as exact holdings so this regression proves that canonical
    // payload validation happens before the ordinary exact-holder delegation rule.
    world.account_permissions.insert(
        authority.clone(),
        invalid_permissions.iter().cloned().collect(),
    );
    let state = state_after_genesis(world);
    let mut block = state.block(BlockHeader::new(
        nonzero!(2_u64),
        Some(
            state
                .view()
                .latest_block_hash()
                .expect("genuine genesis parent"),
        ),
        None,
        0,
        0,
    ));
    let mut state_transaction = block.transaction();
    let role_id: RoleId = "governance_selector_sink".parse().expect("role id");
    Register::role(Role::new(role_id.clone(), authority.clone()))
        .execute(&authority, &mut state_transaction)
        .expect("seed empty role fixture");
    for permission in invalid_permissions {
        let permission_name: &str = permission.name();
        for (path, instruction) in [
            (
                "account",
                Grant::account_permission(permission.clone(), destination.clone()).into(),
            ),
            (
                "role",
                Grant::role_permission(permission.clone(), role_id.clone()).into(),
            ),
        ] {
            let error = super::Executor::Initial
                .execute_instruction(&mut state_transaction, &authority, instruction)
                .unwrap_err();
            assert!(
                matches!(&error, ValidationFail::NotPermitted(message)
                        if message.contains(permission_name)
                            && message.contains("canonical governance selector V1")),
                "unexpected {path} grant rejection: {error:?}"
            );
        }
        assert!(
            !state_transaction
                .world
                .account_permissions_iter(&destination)
                .expect("destination permissions")
                .any(|stored| stored == &permission),
            "noncanonical permission reached account storage"
        );
        assert!(
            !state_transaction
                .world
                .roles()
                .get(&role_id)
                .expect("role fixture")
                .permissions()
                .any(|stored| stored == &permission),
            "noncanonical permission reached role storage"
        );
    }
}
#[test]
fn initial_executor_requires_canonical_operational_governance_unit_permissions() {
    use iroha_executor_data_model::permission::governance::{
        CanManageConfidentialParams, CanManageConsensusKeys, CanProposeRuntimeChangeSet,
    };

    let authority = checked_account_id();
    let destination = checked_account_id();
    let canonical_permissions = vec![
        Permission::from(CanProposeRuntimeChangeSet),
        Permission::from(CanManageConsensusKeys),
        Permission::from(CanManageConfidentialParams),
    ];
    let mut world = World::with(
        [],
        [
            Account::new(authority.clone()).build(&authority),
            Account::new(destination.clone()).build(&destination),
        ],
        [],
    );
    world.account_permissions.insert(
        authority.clone(),
        canonical_permissions.iter().cloned().collect(),
    );
    let state = state_after_genesis(world);
    let mut block = state.block(BlockHeader::new(
        nonzero!(2_u64),
        Some(
            state
                .view()
                .latest_block_hash()
                .expect("genuine genesis parent"),
        ),
        None,
        0,
        0,
    ));
    let mut state_transaction = block.transaction();
    let role_id: RoleId = "operational_governance_sink".parse().expect("role id");
    Register::role(Role::new(role_id.clone(), authority.clone()))
        .execute(&authority, &mut state_transaction)
        .expect("seed empty role fixture");

    for canonical in canonical_permissions {
        let name = canonical.name().to_string();
        assert!(INITIAL_EXECUTOR_PERMISSION_NAMES.contains(&name.as_str()));
        validate_initial_permission_payload_constraints(&canonical)
            .unwrap_or_else(|error| panic!("canonical {name} token rejected: {error:?}"));
        for instruction in [
            Grant::account_permission(canonical.clone(), destination.clone()).into(),
            Grant::role_permission(canonical.clone(), role_id.clone()).into(),
        ] {
            super::Executor::Initial
                .execute_instruction(&mut state_transaction, &authority, instruction)
                .unwrap_or_else(|error| panic!("exact holder could not delegate {name}: {error}"));
        }

        let malformed =
            Permission::new(name.clone(), Json::new("invented-scope-must-not-authorize"));
        for (path, instruction) in [
            (
                "account",
                Grant::account_permission(malformed.clone(), destination.clone()).into(),
            ),
            (
                "role",
                Grant::role_permission(malformed.clone(), role_id.clone()).into(),
            ),
        ] {
            let error = super::Executor::Initial
                .execute_instruction(&mut state_transaction, &authority, instruction)
                .expect_err("same-name non-unit payload must fail before storage");
            assert!(
                matches!(&error, ValidationFail::NotPermitted(message)
                        if message.contains(&name) && message.contains("Invalid permission payload")),
                "unexpected {path} {name} rejection: {error:?}",
            );
        }
        assert!(
            !state_transaction
                .world
                .account_permissions_iter(&destination)
                .expect("destination permissions")
                .any(|stored| stored == &malformed),
            "malformed {name} permission reached account storage",
        );
        assert!(
            !state_transaction
                .world
                .roles()
                .get(&role_id)
                .expect("role fixture")
                .permissions()
                .any(|stored| stored == &malformed),
            "malformed {name} permission reached role storage",
        );
    }
}
#[test]
fn initial_executor_keeps_vpn_quote_issuer_leaf_manager_controlled() {
    let manager = checked_account_id();
    let issuer = checked_account_id();
    let destination = checked_account_id();
    let domain = DomainId::try_new("vpn_issuer_policy", "universal").expect("domain id");
    let manager_permission: Permission =
        executor_permission::soranet::CanManageSoranetVpnQuoteIssuers.into();
    let issuer_permission: Permission =
        executor_permission::soranet::CanIssueSoranetVpnQuote.into();
    let mut world = World::with(
        [Domain::new(domain).build(&manager)],
        [
            Account::new(manager.clone()).build(&manager),
            Account::new(issuer.clone()).build(&issuer),
            Account::new(destination.clone()).build(&destination),
        ],
        [],
    );
    world
        .account_permissions
        .insert(manager.clone(), BTreeSet::from([manager_permission]));
    world
        .account_permissions
        .insert(issuer.clone(), BTreeSet::from([issuer_permission.clone()]));
    let state = state_after_genesis(world);
    let mut block = state.block(BlockHeader::new(
        nonzero!(2_u64),
        Some(
            state
                .view()
                .latest_block_hash()
                .expect("genuine genesis parent"),
        ),
        None,
        0,
        0,
    ));
    let mut state_transaction = block.transaction();
    assert!(
        !initial_permission_delegation_allowed(&state_transaction, &issuer, &issuer_permission)
            .expect("issuer-leaf delegation decision")
    );
    assert!(
        initial_permission_delegation_allowed(&state_transaction, &manager, &issuer_permission)
            .expect("issuer-manager delegation decision")
    );
    super::Executor::Initial
        .execute_instruction(
            &mut state_transaction,
            &issuer,
            Grant::account_permission(issuer_permission.clone(), destination.clone()).into(),
        )
        .expect_err("an issuer leaf must not appoint another issuer");
    super::Executor::Initial
        .execute_instruction(
            &mut state_transaction,
            &manager,
            Grant::account_permission(issuer_permission.clone(), destination.clone()).into(),
        )
        .expect("the issuer manager may appoint an issuer");
    super::Executor::Initial
        .execute_instruction(
            &mut state_transaction,
            &manager,
            Revoke::account_permission(issuer_permission, destination).into(),
        )
        .expect("the issuer manager may revoke an issuer");
}
#[test]
fn initial_executor_requires_direct_dpn_admin_for_dpn_permission_lifecycle() {
    use iroha_executor_data_model::permission::dpn::{
        DpnAdmin, DpnEprGuard, DpnInori, DpnSettlement, DpnUser,
    };

    let admin = checked_account_id();
    let leaf_holder = checked_account_id();
    let role_admin = checked_account_id();
    let destination = checked_account_id();
    let admin_permission: Permission = DpnAdmin.into();
    let leaf_permissions: Vec<Permission> = vec![
        DpnUser.into(),
        DpnInori.into(),
        DpnSettlement.into(),
        DpnEprGuard.into(),
    ];
    let mut world = World::with(
        [],
        [
            Account::new(admin.clone()).build(&admin),
            Account::new(leaf_holder.clone()).build(&leaf_holder),
            Account::new(role_admin.clone()).build(&role_admin),
            Account::new(destination.clone()).build(&destination),
        ],
        [],
    );
    world
        .account_permissions
        .insert(admin.clone(), BTreeSet::from([admin_permission.clone()]));
    world.account_permissions.insert(
        leaf_holder.clone(),
        leaf_permissions.iter().cloned().collect(),
    );
    let state = state_after_genesis(world);
    let mut block = state.block(BlockHeader::new(
        nonzero!(2_u64),
        Some(
            state
                .view()
                .latest_block_hash()
                .expect("genuine genesis parent"),
        ),
        None,
        0,
        0,
    ));
    let mut state_transaction = block.transaction();
    let admin_role: RoleId = "initial_dpn_role_admin".parse().expect("role id");
    Register::role(
        Role::new(admin_role.clone(), admin.clone()).add_permission(admin_permission.clone()),
    )
    .execute(&admin, &mut state_transaction)
    .expect("seed DPN admin role");
    Grant::account_role(admin_role, role_admin.clone())
        .execute(&admin, &mut state_transaction)
        .expect("seed role-derived DPN admin");

    for permission in &leaf_permissions {
        for unauthorized in [&leaf_holder, &role_admin] {
            assert!(
                !initial_permission_delegation_allowed(
                    &state_transaction,
                    unauthorized,
                    permission
                )
                .expect("DPN delegation decision"),
                "only a direct DpnAdmin may manage {}",
                permission.name(),
            );
            super::Executor::Initial
                .execute_instruction(
                    &mut state_transaction,
                    unauthorized,
                    Grant::account_permission(permission.clone(), destination.clone()).into(),
                )
                .expect_err("a DPN leaf or role-derived admin must not delegate DPN markers");
        }
    }

    for permission in std::iter::once(&admin_permission).chain(&leaf_permissions) {
        super::Executor::Initial
            .execute_instruction(
                &mut state_transaction,
                &admin,
                Grant::account_permission(permission.clone(), destination.clone()).into(),
            )
            .unwrap_or_else(|error| {
                panic!(
                    "direct DpnAdmin could not grant {}: {error}",
                    permission.name()
                )
            });
        super::Executor::Initial
            .execute_instruction(
                &mut state_transaction,
                &admin,
                Revoke::account_permission(permission.clone(), destination.clone()).into(),
            )
            .unwrap_or_else(|error| {
                panic!(
                    "direct DpnAdmin could not revoke {}: {error}",
                    permission.name()
                )
            });
    }
}
#[test]
fn initial_executor_exact_asset_alias_lifecycle_survives_clear_without_issuer_guessing() {
    use iroha_data_model::asset::AssetDefinitionAlias;
    use iroha_executor_data_model::permission::{
        account::{AccountAliasPermissionScope, CanManageAccountAlias},
        asset_definition::{AssetDefinitionAliasPermissionScope, CanManageAssetDefinitionAlias},
    };
    let namespace_root = checked_account_id();
    let asset_owner = checked_account_id();
    let holder = checked_account_id();
    let unrelated = checked_account_id();
    let domain = DomainId::try_new("banka", "universal").expect("alias domain");
    let alias: AssetDefinitionAlias = "usd#banka.universal".parse().expect("asset alias");
    let definition_id = AssetDefinitionId::derive_from_components(
        domain.clone(),
        "usd".parse().expect("asset definition name"),
    );
    let account_alias_permission: Permission = CanManageAccountAlias {
        scope: AccountAliasPermissionScope::Domain(domain.clone()),
    }
    .into();
    let namespace_permission: Permission = CanManageAssetDefinitionAlias {
        scope: AssetDefinitionAliasPermissionScope::Domain(domain.clone()),
    }
    .into();
    let exact_permission: Permission = CanManageAssetDefinitionAlias {
        scope: AssetDefinitionAliasPermissionScope::Alias(ResolvedAssetDefinitionAliasV1::new(
            alias.clone(),
            DataSpaceId::UNIVERSAL,
            definition_id.clone(),
        )),
    }
    .into();
    let mut world = World::with_assets(
        [Domain::new(domain).build(&namespace_root)],
        [
            Account::new(namespace_root.clone()).build(&namespace_root),
            Account::new(asset_owner.clone()).build(&asset_owner),
            Account::new(holder.clone()).build(&holder),
            Account::new(unrelated.clone()).build(&unrelated),
        ],
        [AssetDefinition::numeric(
            definition_id.clone(),
            "usd".to_owned(),
            AssetBalancePolicy::Global,
            None,
        )
        .with_alias(Some(alias.clone()))
        .build(&asset_owner)],
        [],
        [],
    );
    world.account_permissions.insert(
        asset_owner.clone(),
        BTreeSet::from([account_alias_permission]),
    );
    let state = state_after_genesis(world);
    let mut block = state.block(BlockHeader::new(
        nonzero!(2_u64),
        Some(
            state
                .view()
                .latest_block_hash()
                .expect("genuine genesis parent"),
        ),
        None,
        10_000,
        0,
    ));
    let mut state_transaction = block.transaction();
    let malformed_permission =
        Permission::new("CanManageAssetDefinitionAlias".to_owned(), Json::new(()));
    let malformed = Grant::account_permission(malformed_permission, holder.clone())
        .execute(&asset_owner, &mut state_transaction)
        .expect_err("Core must reject malformed built-in asset-alias permission payloads");
    assert!(malformed.to_string().contains("current live binding"));
    let mismatched_permission: Permission = CanManageAssetDefinitionAlias {
        scope: AssetDefinitionAliasPermissionScope::Alias(ResolvedAssetDefinitionAliasV1::new(
            alias.clone(),
            DataSpaceId::new(7),
            definition_id.clone(),
        )),
    }
    .into();
    let mismatch = Grant::account_permission(mismatched_permission, holder.clone())
        .execute(&asset_owner, &mut state_transaction)
        .expect_err("Core must reject a text/ID pair that does not match the live catalog");
    assert!(mismatch.to_string().contains("current live binding"));
    let live_binding = state_transaction
        .world
        .asset_definition_alias_bindings
        .get(&definition_id)
        .cloned()
        .expect("fixture alias binding");
    {
        let binding = state_transaction
            .world
            .asset_definition_alias_bindings
            .get_mut(&definition_id)
            .expect("fixture alias binding");
        binding.lease_expiry_ms = Some(1);
        binding.grace_until_ms = None;
    }
    Grant::account_permission(exact_permission.clone(), holder.clone())
        .execute(&asset_owner, &mut state_transaction)
        .expect_err("Core must reject a grace-expired binding pending cleanup");
    state_transaction
        .world
        .asset_definition_alias_bindings
        .insert(definition_id.clone(), live_binding);
    let other_definition_id = AssetDefinitionId::derive_from_components(
        DomainId::try_new("banka", "universal").expect("alias domain"),
        "eur".parse().expect("asset definition name"),
    );
    let rebound_target: Permission = CanManageAssetDefinitionAlias {
        scope: AssetDefinitionAliasPermissionScope::Alias(ResolvedAssetDefinitionAliasV1::new(
            alias.clone(),
            DataSpaceId::UNIVERSAL,
            other_definition_id,
        )),
    }
    .into();
    Grant::account_permission(rebound_target, holder.clone())
        .execute(&asset_owner, &mut state_transaction)
        .expect_err("Core must reject an exact label capability for a different definition");
    for authority in [&namespace_root, &asset_owner] {
        super::Executor::Initial
                .execute_instruction(
                    &mut state_transaction,
                    authority,
                    Grant::account_permission(exact_permission.clone(), holder.clone()).into(),
                )
                .expect_err(
                    "neither namespace ownership alone nor account-alias permission may grant an exact asset alias",
                );
    }
    state_transaction
        .world
        .add_account_permission(&asset_owner, namespace_permission);
    super::Executor::Initial
        .execute_instruction(
            &mut state_transaction,
            &asset_owner,
            Grant::account_permission(exact_permission.clone(), holder.clone()).into(),
        )
        .expect("active asset owner with asset-alias namespace authority may grant exact scope");
    let permission_role: RoleId = "asset_alias_permission_lifecycle".parse().expect("role id");
    Register::role(
        Role::new(permission_role.clone(), namespace_root.clone())
            .add_permission(exact_permission.clone()),
    )
    .execute(&namespace_root, &mut state_transaction)
    .expect("seed exact role permission while the matching binding is live");
    let membership_role: RoleId = "asset_alias_membership_lifecycle".parse().expect("role id");
    Register::role(
        Role::new(membership_role.clone(), namespace_root.clone())
            .add_permission(exact_permission.clone()),
    )
    .execute(&namespace_root, &mut state_transaction)
    .expect("seed exact role membership while the matching binding is live");
    Grant::account_role(membership_role.clone(), holder.clone())
        .execute(&namespace_root, &mut state_transaction)
        .expect("seed exact role membership revocation fixture");
    state_transaction
        .world
        .clear_asset_definition_alias(&definition_id);
    assert!(
        state_transaction
            .world
            .asset_definition_aliases()
            .get(&alias)
            .is_none(),
        "test must exercise revocation after the binding is cleared",
    );
    Grant::account_permission(exact_permission.clone(), unrelated.clone())
        .execute(&namespace_root, &mut state_transaction)
        .expect_err("Core must reject a new exact grant after the binding is cleared");
    for authority in [&unrelated, &holder, &asset_owner] {
        super::Executor::Initial
                .execute_instruction(
                    &mut state_transaction,
                    authority,
                    Revoke::account_permission(exact_permission.clone(), holder.clone()).into(),
                )
                .expect_err(
                    "unrelated, exact-holder, and former grant-issuer identities are not namespace roots",
                );
    }
    assert!(
        state_transaction
            .world
            .account_permissions_iter(&holder)
            .expect("holder permissions")
            .any(|permission| permission == &exact_permission),
        "failed revocations must preserve the exact capability",
    );
    super::Executor::Initial
        .execute_instruction(
            &mut state_transaction,
            &namespace_root,
            Revoke::account_permission(exact_permission.clone(), holder.clone()).into(),
        )
        .expect("native namespace root may revoke exact scope after clear");
    assert!(
        !state_transaction
            .world
            .account_permissions_iter(&holder)
            .expect("holder permissions")
            .any(|permission| permission == &exact_permission),
    );
    super::Executor::Initial
        .execute_instruction(
            &mut state_transaction,
            &namespace_root,
            Revoke::role_permission(exact_permission.clone(), permission_role.clone()).into(),
        )
        .expect("native namespace root may revoke exact role permission after clear");
    assert!(
        !state_transaction
            .world
            .roles()
            .get(&permission_role)
            .expect("permission role")
            .permissions()
            .any(|permission| permission == &exact_permission),
    );
    super::Executor::Initial
        .execute_instruction(
            &mut state_transaction,
            &namespace_root,
            Revoke::account_role(membership_role.clone(), holder.clone()).into(),
        )
        .expect("native namespace root may revoke exact role membership after clear");
    assert!(!authority_has_role(
        &state_transaction.world,
        &holder,
        &membership_role,
    ));
}
#[test]
#[allow(clippy::too_many_lines)]
fn initial_executor_authorizes_every_permission_and_role_mutation_path() {
    let attacker = checked_account_id();
    let administrator = checked_account_id();
    let attacker_account = Account::new(attacker.clone()).build(&attacker);
    let administrator_account = Account::new(administrator.clone()).build(&administrator);
    let ordinary_permission: Permission = executor_permission::parameter::CanSetParameters.into();
    let genesis_only_permission: Permission =
        executor_permission::sccp::CanProposeSccpRouteGovernance.into();
    let mut world = World::with([], [attacker_account, administrator_account], []);
    world.account_permissions.insert(
        administrator.clone(),
        BTreeSet::from([ordinary_permission.clone(), genesis_only_permission.clone()]),
    );
    let state = state_after_genesis(world);
    let mut block = state.block(BlockHeader::new(
        nonzero!(2_u64),
        Some(
            state
                .view()
                .latest_block_hash()
                .expect("genuine genesis parent"),
        ),
        None,
        0,
        0,
    ));
    let mut state_transaction = block.transaction();
    let ordinary_role: RoleId = "initial_executor_ordinary_role".parse().expect("role id");
    Register::role(
        Role::new(ordinary_role.clone(), administrator.clone())
            .add_permission(ordinary_permission.clone()),
    )
    .execute(&administrator, &mut state_transaction)
    .expect("seed ordinary role fixture");
    let genesis_only_role: RoleId = "initial_executor_genesis_only_role"
        .parse()
        .expect("role id");
    Register::role(
        Role::new(genesis_only_role.clone(), administrator.clone())
            .add_permission(genesis_only_permission.clone()),
    )
    .execute(&administrator, &mut state_transaction)
    .expect("seed governed role fixture");
    let denied = [
        Grant::account_permission(ordinary_permission.clone(), attacker.clone()).into(),
        Revoke::account_permission(ordinary_permission.clone(), administrator.clone()).into(),
        Grant::role_permission(ordinary_permission.clone(), ordinary_role.clone()).into(),
        Revoke::role_permission(ordinary_permission.clone(), ordinary_role.clone()).into(),
        Grant::account_role(ordinary_role.clone(), attacker.clone()).into(),
        Revoke::account_role(ordinary_role.clone(), administrator.clone()).into(),
        Unregister::role(ordinary_role.clone()).into(),
    ];
    for instruction in denied {
        super::Executor::Initial
            .execute_instruction(&mut state_transaction, &attacker, instruction)
            .expect_err("an unprivileged authority must not mutate permissions or roles");
    }
    assert!(
        !state_transaction
            .world
            .account_permissions_iter(&attacker)
            .expect("attacker permissions")
            .any(|permission| permission == &ordinary_permission)
    );
    assert!(!authority_has_role(
        &state_transaction.world,
        &attacker,
        &ordinary_role
    ));
    super::Executor::Initial
        .execute_instruction(
            &mut state_transaction,
            &administrator,
            Grant::account_permission(ordinary_permission.clone(), attacker.clone()).into(),
        )
        .expect("an exact holder may delegate an ordinary exact permission");
    assert!(
        state_transaction
            .world
            .account_permissions_iter(&attacker)
            .expect("attacker permissions")
            .any(|permission| permission == &ordinary_permission)
    );
    for instruction in [
        Grant::account_permission(genesis_only_permission.clone(), attacker.clone()).into(),
        Grant::account_role(genesis_only_role.clone(), attacker.clone()).into(),
    ] {
        super::Executor::Initial
            .execute_instruction(&mut state_transaction, &administrator, instruction)
            .expect_err("genesis-only authority must not be delegated directly or through a role");
    }
    assert!(
        !state_transaction
            .world
            .account_permissions_iter(&attacker)
            .expect("attacker permissions")
            .any(|permission| permission == &genesis_only_permission)
    );
    assert!(!authority_has_role(
        &state_transaction.world,
        &attacker,
        &genesis_only_role
    ));
}
#[test]
fn initial_executor_restricted_reader_cannot_mutate_direct_or_role_grants() {
    let holder = checked_account_id();
    let grant_destination = checked_account_id();
    let revoke_destination = checked_account_id();
    let exact: Permission = executor_permission::query::CanReadRestrictedDataspace {
        dataspace: DataSpaceId::new(10),
    }
    .into();
    let can_manage_roles: Permission = executor_permission::role::CanManageRoles.into();
    let mut world = World::with(
        [],
        [
            Account::new(holder.clone()).build(&holder),
            Account::new(grant_destination.clone()).build(&grant_destination),
            Account::new(revoke_destination.clone()).build(&revoke_destination),
        ],
        [],
    );
    world.account_permissions.insert(
        holder.clone(),
        BTreeSet::from([exact.clone(), can_manage_roles]),
    );
    let state = state_after_genesis(world);
    let mut block = state.block(BlockHeader::new(
        nonzero!(2_u64),
        Some(
            state
                .view()
                .latest_block_hash()
                .expect("genuine genesis parent"),
        ),
        None,
        0,
        0,
    ));
    let mut state_transaction = block.transaction();
    let reader_role: RoleId = "restricted_reader_role".parse().expect("role id");
    Register::role(Role::new(reader_role.clone(), holder.clone()).add_permission(exact.clone()))
        .execute(&holder, &mut state_transaction)
        .expect("seed restricted-reader role fixture");
    Grant::account_role(reader_role.clone(), revoke_destination.clone())
        .execute(&holder, &mut state_transaction)
        .expect("seed revocation membership fixture");
    let empty_role: RoleId = "empty_reader_role".parse().expect("role id");
    Register::role(Role::new(empty_role.clone(), holder.clone()))
        .execute(&holder, &mut state_transaction)
        .expect("seed empty role fixture");
    let attempted_registration: RoleId =
        "attempted_restricted_reader_role".parse().expect("role id");
    let denied = [
        (
            "direct grant",
            Grant::account_permission(exact.clone(), grant_destination.clone()).into(),
        ),
        (
            "direct revoke",
            Revoke::account_permission(exact.clone(), holder.clone()).into(),
        ),
        (
            "role membership grant",
            Grant::account_role(reader_role.clone(), grant_destination.clone()).into(),
        ),
        (
            "role membership revoke",
            Revoke::account_role(reader_role.clone(), revoke_destination.clone()).into(),
        ),
        (
            "role permission grant",
            Grant::role_permission(exact.clone(), empty_role.clone()).into(),
        ),
        (
            "role permission revoke",
            Revoke::role_permission(exact.clone(), reader_role.clone()).into(),
        ),
        (
            "role registration",
            InstructionBox::from(RegisterBox::Role(Register::role(
                Role::new(attempted_registration.clone(), holder.clone())
                    .add_permission(exact.clone()),
            ))),
        ),
    ];
    for (path, instruction) in denied {
        let error = super::Executor::Initial
            .execute_instruction(&mut state_transaction, &holder, instruction)
            .expect_err("restricted-read permission mutations must be genesis-only");
        assert!(matches!(error, ValidationFail::NotPermitted(_)));
        assert!(
            error.to_string().contains("CanReadRestrictedDataspace"),
            "unexpected rejection for {path}: {error}",
        );
    }
    let grant_destination_permissions = state_transaction
        .world
        .account_permissions_iter(&grant_destination)
        .expect("grant destination permissions")
        .cloned()
        .collect::<BTreeSet<_>>();
    assert!(!grant_destination_permissions.contains(&exact));
    assert!(
        state_transaction
            .world
            .account_permissions_iter(&holder)
            .expect("holder permissions")
            .any(|permission| permission == &exact),
        "rejected direct revoke must leave the exact permission in place",
    );
    assert!(!authority_has_role(
        &state_transaction.world,
        &grant_destination,
        &reader_role,
    ));
    assert!(
        authority_has_role(&state_transaction.world, &revoke_destination, &reader_role,),
        "rejected role revoke must leave membership in place",
    );
    assert!(
        !state_transaction
            .world
            .roles()
            .get(&empty_role)
            .expect("empty role")
            .permissions()
            .any(|permission| permission == &exact),
        "rejected role-permission grant must not mutate the role",
    );
    assert!(
        state_transaction
            .world
            .roles()
            .get(&reader_role)
            .expect("reader role")
            .permissions()
            .any(|permission| permission == &exact),
        "rejected role-permission revoke must leave the token in place",
    );
    assert!(
        state_transaction
            .world
            .roles()
            .get(&attempted_registration)
            .is_none(),
        "rejected role registration must not create the role",
    );
}
#[test]
fn lifecycle_runtime_context_rejects_binding_mutations_for_every_executor_path() {
    let subject = checked_account_id();
    let contract_address = ContractAddress::derive(
        &"hash:0000000000000000000000000000000000000000000000000000000000000001#C50E"
            .parse()
            .expect("canonical test network id"),
        &subject,
        404,
        DataSpaceId::UNIVERSAL,
    )
    .expect("contract address");
    let instructions = [
        InstructionBox::from(
            iroha_data_model::isi::smart_contract_code::ActivateContractInstance {
                contract_address: contract_address.clone(),
                expected_revision: 1,
                code_hash: Hash::new(b"executor-lifecycle-activation"),
            },
        ),
        InstructionBox::from(
            iroha_data_model::isi::smart_contract_code::DeactivateContractInstance {
                contract_address: contract_address.clone(),
                expected_revision: 1,
                reason: Some("executor lifecycle guard".to_owned()),
            },
        ),
        InstructionBox::from(
            iroha_data_model::isi::smart_contract_code::CommitContractDeployment {
                expected_deploy_nonce: 404,
                contract_address: contract_address.clone(),
                code_hash: Hash::new(b"executor-lifecycle-atomic-deployment"),
                contract_alias: "payments::universal".parse().expect("contract alias"),
                lease_expiry_ms: None,
                expected_previous_contract_address: None,
            },
        ),
    ];
    for entrypoint in ["hajimari", "始まり", "kaizen", "改善"] {
        let context = ContractRuntimeExecutionContext {
            contract_address: contract_address.clone(),
            contract_subject: contract_address.subject_id(),
            contract_alias: None,
            entrypoint: entrypoint.to_owned(),
        };
        for instruction in &instructions {
            assert!(matches!(
                ensure_lifecycle_hook_cannot_mutate_contract_binding(Some(&context), instruction,),
                Err(ValidationFail::NotPermitted(_))
            ));
        }
    }
    let ordinary_context = ContractRuntimeExecutionContext {
        contract_address,
        contract_subject: subject,
        contract_alias: None,
        entrypoint: "kotoage".to_owned(),
    };
    for instruction in &instructions {
        ensure_lifecycle_hook_cannot_mutate_contract_binding(Some(&ordinary_context), instruction)
            .expect("ordinary kotoage dispatch is governed by the instruction permission layer");
    }
}
#[test]
fn contract_runtime_permission_boundary_precedes_user_executor_dispatch() {
    let deployer = checked_account_id();
    let destination = checked_account_id();
    let state = state_after_genesis(World::with(
        [],
        [
            Account::new(deployer.clone()).build(&deployer),
            Account::new(destination.clone()).build(&destination),
        ],
        [],
    ));
    let contract_address =
        ContractAddress::derive(&state.network_id, &deployer, 505, DataSpaceId::UNIVERSAL)
            .expect("contract address");
    let contract_subject = contract_address.subject_id();
    let context = ContractRuntimeExecutionContext {
        contract_subject: contract_subject.clone(),
        contract_address: contract_address.clone(),
        contract_alias: None,
        entrypoint: "main".to_owned(),
    };
    let mut block = state.block(BlockHeader::new(
        nonzero!(2_u64),
        Some(
            state
                .view()
                .latest_block_hash()
                .expect("genuine genesis parent"),
        ),
        None,
        0,
        0,
    ));
    let mut state_transaction = block.transaction();
    state_transaction.world.accounts.insert(
        contract_subject.clone(),
        iroha_data_model::account::AccountValue::new(
            iroha_data_model::account::AccountDetails::default(),
        ),
    );
    state_transaction.tx_call_hash = Some(Hash::prehashed([0xD8; Hash::LENGTH]));
    let raw = data_model_executor::Executor::new(IvmBytecode::from_compiled(
        generate_denied_program("user executor reached"),
    ));
    let executor = super::Executor::UserProvided(
        super::LoadedExecutor::load(raw).expect("load denying executor"),
    );
    let role_id: RoleId = "contract_forbidden_role".parse().expect("role id");
    let role = Role::new(role_id.clone(), destination.clone())
        .add_permission(executor_permission::parameter::CanSetParameters);
    let register_role = Register::role(role);
    let unregister_role = Unregister::role(role_id.clone());
    let ordinary_permission: Permission = executor_permission::parameter::CanSetParameters.into();
    let boxed_forbidden = vec![
        InstructionBox::from(RegisterBox::Role(register_role.clone())),
        InstructionBox::from(UnregisterBox::Role(unregister_role.clone())),
        Grant::account_role(role_id.clone(), destination.clone()).into(),
        Revoke::account_role(role_id.clone(), destination.clone()).into(),
        Grant::role_permission(ordinary_permission.clone(), role_id.clone()).into(),
        Revoke::role_permission(ordinary_permission.clone(), role_id.clone()).into(),
        Grant::account_permission(ordinary_permission.clone(), destination.clone()).into(),
        Revoke::account_permission(ordinary_permission.clone(), destination.clone()).into(),
    ];
    let concrete_forbidden = vec![
        concrete_instruction_box!(Register<Role>, register_role),
        concrete_instruction_box!(Unregister<Role>, unregister_role),
        concrete_instruction_box!(
            Grant<RoleId, Account>,
            Grant::account_role(role_id.clone(), destination.clone())
        ),
        concrete_instruction_box!(
            Revoke<RoleId, Account>,
            Revoke::account_role(role_id.clone(), destination.clone())
        ),
        concrete_instruction_box!(
            Grant<Permission, Role>,
            Grant::role_permission(ordinary_permission.clone(), role_id.clone())
        ),
        concrete_instruction_box!(
            Revoke<Permission, Role>,
            Revoke::role_permission(ordinary_permission.clone(), role_id.clone())
        ),
        concrete_instruction_box!(
            Grant<Permission, Account>,
            Grant::account_permission(ordinary_permission.clone(), destination.clone())
        ),
        concrete_instruction_box!(
            Revoke<Permission, Account>,
            Revoke::account_permission(ordinary_permission, destination.clone())
        ),
    ];
    for instruction in boxed_forbidden.into_iter().chain(concrete_forbidden) {
        let error = executor
            .execute_instruction_with_contract_runtime_context(
                &mut state_transaction,
                &contract_subject,
                instruction,
                Some(&context),
            )
            .expect_err("the common contract boundary must reject before user executor IVM");
        assert!(
            !error.to_string().contains("user executor reached"),
            "forbidden mutation reached the user executor: {error}",
        );
        assert!(matches!(error, ValidationFail::NotPermitted(_)));
    }
    let borrowed_role_grant: InstructionBox =
        Grant::account_role(role_id, destination.clone()).into();
    let error = executor
        .execute_borrowed_overlay_instruction(
            &mut state_transaction,
            &contract_subject,
            &borrowed_role_grant,
            Some(&context),
        )
        .expect_err("the borrowed path must apply the same common contract boundary");
    assert!(
        !error.to_string().contains("user executor reached"),
        "borrowed role mutation reached the user executor: {error}",
    );
    seed_owner_permission_contract(
        &mut state_transaction,
        &contract_address,
        &deployer,
        Hash::new(b"contract permission boundary"),
    );
    let exact: Permission = executor_permission::smart_contract::CanUseContractPermission {
        contract: contract_address.clone(),
        permission: "write".parse().unwrap(),
    }
    .into();
    state_transaction
        .world
        .add_account_permission(&contract_subject, exact.clone());
    let error = executor
        .execute_instruction_with_contract_runtime_context(
            &mut state_transaction,
            &contract_subject,
            Grant::account_permission(exact, destination.clone()).into(),
            Some(&context),
        )
        .expect_err("the denying user executor must receive an exact bound token");
    assert!(
        error.to_string().contains("user executor reached"),
        "an exact bound token should pass the common boundary: {error}",
    );
    let inconsistent_context = ContractRuntimeExecutionContext {
        contract_subject: destination.clone(),
        contract_address: contract_address.clone(),
        contract_alias: None,
        entrypoint: "main".to_owned(),
    };
    let inconsistent_exact: Permission =
        executor_permission::smart_contract::CanUseContractPermission {
            contract: contract_address.clone(),
            permission: "write".parse().unwrap(),
        }
        .into();
    let error = executor
        .execute_instruction_with_contract_runtime_context(
            &mut state_transaction,
            &destination,
            Grant::account_permission(inconsistent_exact, destination.clone()).into(),
            Some(&inconsistent_context),
        )
        .expect_err("an inconsistent contract subject/address context must fail closed");
    assert!(
        !error.to_string().contains("user executor reached"),
        "inconsistent contract context reached the user executor: {error}",
    );
    let sibling_address =
        ContractAddress::derive(&state.network_id, &deployer, 506, DataSpaceId::UNIVERSAL)
            .expect("sibling contract address");
    let sibling: Permission = executor_permission::smart_contract::CanUseContractPermission {
        contract: sibling_address,
        permission: "write".parse().unwrap(),
    }
    .into();
    let error = executor
        .execute_instruction_with_contract_runtime_context(
            &mut state_transaction,
            &contract_subject,
            Grant::account_permission(sibling, destination).into(),
            Some(&context),
        )
        .expect_err("a contract may not delegate a sibling contract token");
    assert!(
        !error.to_string().contains("user executor reached"),
        "sibling token reached the user executor: {error}",
    );
}
// These private-call fixtures supply post-verification replay metadata.
// Their real signatures select the IvmProved root kind, but neither the Halt
// envelope nor the supplied commitments claim cryptographic replay verification.
fn supplied_replay_fixture_source(
    state: &State,
    authority: &AccountId,
    keypair: &KeyPair,
    overlay: Vec<InstructionBox>,
) -> SignedTransaction {
    let mut bytecode = ivm::ProgramMetadata::default().encode();
    bytecode.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());
    TransactionBuilder::new(
        state.network_id,
        authority.clone(),
        FeePaymentIntent::authority(Vec::new(), core::num::NonZeroU64::new(50_000)),
    )
    .with_executable(Executable::IvmProved(
        iroha_data_model::transaction::IvmProved {
            bytecode: IvmBytecode::from_compiled(bytecode),
            overlay: overlay.into(),
            events_commitment: Hash::new(b"events"),
            gas_policy_commitment: Hash::new(b"supplied replay gas-policy fixture"),
        },
    ))
    .sign(keypair.private_key())
}

fn bind_supplied_replay_fixture_root(
    transaction: &mut StateTransaction<'_, '_>,
    signed: &SignedTransaction,
    replay: &crate::pipeline::overlay::IvmProvedReplay,
) {
    assert!(matches!(signed.instructions(), Executable::IvmProved(_)));
    assert_eq!(
        transaction.current_dataspace_id,
        transaction.world.current_dataspace_id
    );
    transaction.current_tx_hash = Some(signed.hash());
    transaction.tx_call_hash = Some(Hash::from(signed.hash_as_entrypoint()));
    transaction
        .begin_execution_effect_budget(signed)
        .expect("bind exact supplied-replay root");
    // Explicit supplied post-verification handoff, not measured VM work.
    transaction.last_tx_gas_used = replay.gas_used;
    transaction
        .record_execution_fee_instructions(
            replay
                .queued
                .iter()
                .filter(|effect| effect.instruction().is_some())
                .count(),
            replay.gas_used,
        )
        .expect("bind supplied replay fee work");
}

#[test]
fn proved_empty_overlay_accounts_verified_replay_gas() {
    let keypair = checked_keypair();
    let authority = AccountId::new(keypair.public_key().clone());
    let kura = Kura::blank_kura_for_testing();
    let query_handle = query::store::LiveQueryStore::start_test();
    let state = State::new_for_testing(World::default(), kura, query_handle);
    let tx = supplied_replay_fixture_source(&state, &authority, &keypair, Vec::new());
    let replay_gas = 40_000;
    let (axt_descriptor, axt_binding) = ivm::axt::AxtDescriptor::builder()
        .dataspace(DataSpaceId::UNIVERSAL)
        .build_with_binding()
        .expect("AXT descriptor");
    let mut completed_axt = ivm::axt::HostAxtState::new(axt_descriptor, axt_binding);
    completed_axt
        .record_proof(
            DataSpaceId::UNIVERSAL,
            Some(ivm::axt::ProofBlob {
                payload: vec![1],
                expiry_slot: None,
            }),
            None,
        )
        .expect("record AXT proof");
    completed_axt
        .validate_commit()
        .expect("completed AXT fixture");
    let replay = crate::pipeline::overlay::IvmProvedReplay {
        queued: Vec::new(),
        completed_axt: vec![completed_axt],
        durable_state_overlay: BTreeMap::new(),
        durable_state_authorizations: BTreeMap::new(),
        access_log: None,
        gas_used: replay_gas,
    };
    let header = BlockHeader::new(nonzero!(1_u64), None, None, 0, 0);
    let mut block = state.block(header);
    let mut state_tx = block.transaction();
    bind_supplied_replay_fixture_root(&mut state_tx, &tx, &replay);
    super::Executor::Initial
        .execute_metered_instructions(
            &mut state_tx,
            &authority,
            &tx,
            &[],
            (Some(replay), None, None),
            InstructionFeeContext {
                tx_bytes_len: 0,
                gas_limit_md: Some(50_000),
                require_gas_limit: true,
                gas_asset_opt: None,
                fee_sponsor: None,
                skip_nexus_fee: true,
            },
            None,
        )
        .expect("empty proved overlay should retain replay gas");
    state_tx
        .finish_execution_effect_budget()
        .expect("close supplied-replay root");
    assert_eq!(state_tx.last_tx_gas_used, replay_gas);
    state_tx.apply();
    assert_eq!(
        block.axt_envelopes().len(),
        1,
        "direct proved replay must persist completed AXT envelopes"
    );
    assert_eq!(block.axt_envelopes()[0].binding.as_bytes(), &axt_binding);
}
#[test]
fn proved_replay_applies_durable_state_with_exact_per_path_authorization() {
    let keypair = checked_keypair();
    let authority = AccountId::new(keypair.public_key().clone());
    let domain =
        Domain::new(DomainId::try_new("wonderland", "universal").expect("valid test domain"))
            .build(&authority);
    let account = Account::new(authority.clone()).build(&authority);
    let state = state_after_genesis(World::with([domain], [account], []));
    let contract_address =
        ContractAddress::derive(&state.network_id, &authority, 405, DataSpaceId::UNIVERSAL)
            .expect("contract address");
    let manifest_signing = crate::manifest_signing_test_support::ManifestSigningFixture::new();
    let (program, manifest) = kotodama_lang::compiler::Compiler::new()
        .compile_source_with_manifest(
            "seiyaku ProvedDurable { kotoage fn write() authorize(anyone) {} }",
        )
        .expect("compile exact replay authorization");
    let code_hash = ivm::contract_code_hash(&program);
    bind_executor_test_contract_after_genesis(&state, &contract_address, &authority, code_hash);
    {
        let mut world = state.world.block();
        world.contract_manifests.insert(
            iroha_data_model::smart_contract::ContractArtifactId::for_address(
                &contract_address,
                code_hash,
            )
            .unwrap(),
            manifest
                .try_signed(
                    manifest_signing.context(),
                    manifest_signing.max_frame_bytes(),
                    &keypair,
                )
                .expect("sign replay authorization manifest"),
        );
        world.commit();
    }
    let tx = supplied_replay_fixture_source(&state, &authority, &keypair, Vec::new());
    let authorization = ContractEntrypointAuthorizationSnapshot::new(
        authority.clone(),
        "write".to_owned(),
        iroha_data_model::smart_contract::manifest::EntrypointAuthorizationV1::Anyone,
        &code::BoundContractIdentity {
            lifecycle_revision: 1,
            contract_address: contract_address.clone(),
            contract_alias: None,
            contract_alias_binding: None,
            code_hash,
        },
    );
    let runtime_context = ContractRuntimeExecutionContext {
        contract_subject: contract_address.subject_id(),
        contract_address: contract_address.clone(),
        contract_alias: None,
        entrypoint: "write".to_owned(),
    };
    let digest = hex::encode(Hash::new(contract_address.to_string().as_bytes()).as_ref());
    let marker: StatePath = format!("sc/{digest}/Values/fixture")
        .parse()
        .expect("scoped durable state marker");
    let stored = vec![0xA5];
    let replay = crate::pipeline::overlay::IvmProvedReplay {
        queued: Vec::new(),
        completed_axt: Vec::new(),
        durable_state_overlay: BTreeMap::from([(marker.clone(), Some(stored.clone()))]),
        durable_state_authorizations: BTreeMap::from([(
            marker.clone(),
            Some(authorization.clone()),
        )]),
        access_log: None,
        gas_used: 0,
    };
    let header = BlockHeader::new(
        nonzero!(2_u64),
        Some(
            state
                .view()
                .latest_block_hash()
                .expect("genuine genesis parent"),
        ),
        None,
        0,
        0,
    );
    let mut block = state.block(header);
    let mut state_tx = block.transaction();
    bind_supplied_replay_fixture_root(&mut state_tx, &tx, &replay);
    super::Executor::Initial
        .execute_metered_instructions(
            &mut state_tx,
            &authority,
            &tx,
            &[],
            (Some(replay), Some(&runtime_context), Some(&authorization)),
            InstructionFeeContext {
                tx_bytes_len: 0,
                gas_limit_md: Some(50_000),
                require_gas_limit: true,
                gas_asset_opt: None,
                fee_sponsor: None,
                skip_nexus_fee: true,
            },
            None,
        )
        .expect("proved replay applies its authorized durable write");
    state_tx
        .finish_execution_effect_budget()
        .expect("close supplied-replay root");
    assert_eq!(
        state_tx.world.smart_contract_state.get(&marker),
        Some(&stored)
    );
    drop(state_tx);
    drop(block);
    let malformed_replay = crate::pipeline::overlay::IvmProvedReplay {
        queued: Vec::new(),
        completed_axt: Vec::new(),
        durable_state_overlay: BTreeMap::from([(marker.clone(), Some(stored))]),
        durable_state_authorizations: BTreeMap::new(),
        access_log: None,
        gas_used: 0,
    };
    let header = BlockHeader::new(
        nonzero!(2_u64),
        Some(
            state
                .view()
                .latest_block_hash()
                .expect("genuine genesis parent"),
        ),
        None,
        0,
        0,
    );
    let mut malformed_block = state.block(header);
    let mut malformed_tx = malformed_block.transaction();
    bind_supplied_replay_fixture_root(&mut malformed_tx, &tx, &malformed_replay);
    let error = super::Executor::Initial
        .execute_metered_instructions(
            &mut malformed_tx,
            &authority,
            &tx,
            &[],
            (
                Some(malformed_replay),
                Some(&runtime_context),
                Some(&authorization),
            ),
            InstructionFeeContext {
                tx_bytes_len: 0,
                gas_limit_md: Some(50_000),
                require_gas_limit: true,
                gas_asset_opt: None,
                fee_sponsor: None,
                skip_nexus_fee: true,
            },
            None,
        )
        .expect_err("post-verification replay metadata must retain an exact authorization map");
    malformed_tx
        .finish_execution_effect_budget()
        .expect("close supplied-replay root");
    assert!(matches!(
        error,
        ValidationFail::InternalError(message)
            if message.contains("structurally inconsistent")
    ));
    assert!(
        malformed_tx
            .world
            .smart_contract_state
            .get(&marker)
            .is_none(),
        "malformed replay authorization metadata must apply zero durable writes"
    );
    drop(malformed_tx);
    drop(malformed_block);
    let foreign_digest = hex::encode(Hash::new(b"foreign contract namespace").as_ref());
    let foreign_path: StatePath = format!("sc/{foreign_digest}/Values/fixture")
        .parse()
        .expect("foreign scoped durable state marker");
    let foreign_replay = crate::pipeline::overlay::IvmProvedReplay {
        queued: Vec::new(),
        completed_axt: Vec::new(),
        durable_state_overlay: BTreeMap::from([(foreign_path.clone(), Some(vec![0x5A]))]),
        durable_state_authorizations: BTreeMap::from([(
            foreign_path.clone(),
            Some(authorization.clone()),
        )]),
        access_log: None,
        gas_used: 0,
    };
    let header = BlockHeader::new(
        nonzero!(2_u64),
        Some(
            state
                .view()
                .latest_block_hash()
                .expect("genuine genesis parent"),
        ),
        None,
        0,
        0,
    );
    let mut foreign_block = state.block(header);
    let mut foreign_tx = foreign_block.transaction();
    bind_supplied_replay_fixture_root(&mut foreign_tx, &tx, &foreign_replay);
    let error = super::Executor::Initial
        .execute_metered_instructions(
            &mut foreign_tx,
            &authority,
            &tx,
            &[],
            (
                Some(foreign_replay),
                Some(&runtime_context),
                Some(&authorization),
            ),
            InstructionFeeContext {
                tx_bytes_len: 0,
                gas_limit_md: Some(50_000),
                require_gas_limit: true,
                gas_asset_opt: None,
                fee_sponsor: None,
                skip_nexus_fee: true,
            },
            None,
        )
        .expect_err("one contract's snapshot must not authorize another state namespace");
    foreign_tx
        .finish_execution_effect_budget()
        .expect("close supplied-replay root");
    assert!(matches!(
        error,
        ValidationFail::NotPermitted(message)
            if message.contains("does not belong to its contract authorization snapshot")
    ));
    assert!(
        foreign_tx
            .world
            .smart_contract_state
            .get(&foreign_path)
            .is_none(),
        "a foreign per-path snapshot must apply zero durable writes"
    );
}
#[test]
fn proved_replay_rejects_durable_state_without_root_authorization_before_effects() {
    let keypair = checked_keypair();
    let authority = AccountId::new(keypair.public_key().clone());
    let kura = Kura::blank_kura_for_testing();
    let query_handle = query::store::LiveQueryStore::start_test();
    let state = State::new_for_testing(World::default(), kura, query_handle);
    let tx = supplied_replay_fixture_source(&state, &authority, &keypair, Vec::new());
    let marker: StatePath = "proved_replay_forbidden_marker"
        .parse()
        .expect("durable state marker");
    let replay = crate::pipeline::overlay::IvmProvedReplay {
        queued: Vec::new(),
        completed_axt: Vec::new(),
        durable_state_overlay: BTreeMap::from([(marker.clone(), Some(vec![0xA5]))]),
        durable_state_authorizations: BTreeMap::from([(marker.clone(), None)]),
        access_log: None,
        gas_used: 0,
    };
    let header = BlockHeader::new(nonzero!(1_u64), None, None, 0, 0);
    let mut block = state.block(header);
    let mut state_tx = block.transaction();
    bind_supplied_replay_fixture_root(&mut state_tx, &tx, &replay);
    let error = super::Executor::Initial
        .execute_metered_instructions(
            &mut state_tx,
            &authority,
            &tx,
            &[],
            (Some(replay), None, None),
            InstructionFeeContext {
                tx_bytes_len: 0,
                gas_limit_md: Some(50_000),
                require_gas_limit: true,
                gas_asset_opt: None,
                fee_sponsor: None,
                skip_nexus_fee: true,
            },
            None,
        )
        .expect_err("proved replay durable state writes require root authorization");
    state_tx
        .finish_execution_effect_budget()
        .expect("close supplied-replay root");
    assert!(matches!(
        error,
        ValidationFail::NotPermitted(message)
            if message.contains("missing its root authorization snapshot")
    ));
    assert!(
        state_tx.world.smart_contract_state.get(&marker).is_none(),
        "rejected proved replay must apply no durable state"
    );
}
#[test]
fn supplied_replay_group_limits_preserve_work_and_refuse_before_effects() {
    let instructions: Vec<InstructionBox> = ["replay_effect_first", "replay_effect_second"]
        .into_iter()
        .map(|key| {
            SetKeyValue::account(ALICE_ID.clone(), key.parse().unwrap(), Json::new(true)).into()
        })
        .collect();
    let exact_bytes: u64 = instructions
        .iter()
        .map(|instruction| u64::try_from(instruction.encode().len()).unwrap())
        .sum();
    let replay_gas = 40_000;
    for (count_cap, byte_cap, expected_error) in [
        (2, 0, None),
        (
            1,
            0,
            Some("overlay exceeds max instructions: 2 > 1".to_owned()),
        ),
        (0, exact_bytes, None),
        (
            0,
            exact_bytes - 1,
            Some(format!(
                "overlay exceeds max bytes: more than {}",
                exact_bytes - 1
            )),
        ),
    ] {
        let state = state_after_genesis(World::with(
            [],
            [Account::new(ALICE_ID.clone()).build(&ALICE_ID)],
            [],
        ));
        let signed =
            supplied_replay_fixture_source(&state, &ALICE_ID, &ALICE_KEYPAIR, instructions.clone());
        let replay = crate::pipeline::overlay::IvmProvedReplay {
            queued: instructions
                .iter()
                .cloned()
                .map(
                    |instruction| crate::smartcontracts::ivm::host::QueuedEffect {
                        payload: crate::smartcontracts::ivm::host::QueuedEffectPayload::Instruction(
                            instruction,
                        ),
                        authority: ALICE_ID.clone(),
                        contract_runtime_context: None,
                        entrypoint_authorization: None,
                    },
                )
                .collect(),
            completed_axt: Vec::new(),
            durable_state_overlay: BTreeMap::new(),
            durable_state_authorizations: BTreeMap::new(),
            access_log: None,
            gas_used: replay_gas,
        };
        let mut block = state.block(BlockHeader::new(
            nonzero!(2_u64),
            Some(
                state
                    .view()
                    .latest_block_hash()
                    .expect("genuine genesis parent"),
            ),
            None,
            0,
            0,
        ));
        let fragments = block.committed_fragment_count();
        let mut transaction = block.transaction();
        transaction.pipeline.overlay_max_instructions = count_cap;
        transaction.pipeline.overlay_max_bytes = byte_cap;
        bind_supplied_replay_fixture_root(&mut transaction, &signed, &replay);
        let result = super::Executor::Initial.execute_metered_instructions(
            &mut transaction,
            &ALICE_ID,
            &signed,
            &instructions,
            (Some(replay), None, None),
            InstructionFeeContext {
                tx_bytes_len: 0,
                gas_limit_md: Some(50_000),
                require_gas_limit: true,
                gas_asset_opt: None,
                fee_sponsor: None,
                skip_nexus_fee: true,
            },
            None,
        );
        let finished = transaction.finish_execution_effect_budget();
        assert_eq!(
            transaction.last_tx_gas_used, replay_gas,
            "supplied completed replay work survives later group admission refusal"
        );
        let rejected = expected_error.is_some();
        if let Some(expected) = expected_error {
            for error in [
                result.expect_err("one-below replay group refuses"),
                finished.expect_err("refusal remains sticky at close"),
            ] {
                assert!(
                    matches!(error, ValidationFail::NotPermitted(ref actual) if actual == &expected),
                    "{error:?}"
                );
            }
            assert!(transaction.execution_effect_limit_exceeded());
            for key in ["replay_effect_first", "replay_effect_second"] {
                assert!(
                    transaction
                        .world
                        .account(&ALICE_ID)
                        .unwrap()
                        .metadata()
                        .get(key)
                        .is_none(),
                    "refused replay group must execute no first instruction"
                );
            }
            drop(transaction);
        } else {
            result.expect("exact replay group fits");
            finished.expect("exact replay root closes");
            assert!(!transaction.execution_effect_limit_exceeded());
            for key in ["replay_effect_first", "replay_effect_second"] {
                assert_eq!(
                    transaction
                        .world
                        .account(&ALICE_ID)
                        .unwrap()
                        .metadata()
                        .get(key),
                    Some(&Json::new(true))
                );
            }
            transaction.apply();
        }
        assert_eq!(
            block.committed_fragment_count(),
            fragments + usize::from(!rejected)
        );
        for key in ["replay_effect_first", "replay_effect_second"] {
            assert_eq!(
                block
                    .world
                    .account(&ALICE_ID)
                    .unwrap()
                    .metadata()
                    .get(key)
                    .is_some(),
                !rejected
            );
        }
    }
}
fn make_peer_id() -> iroha_model_base::peer::PeerId {
    let kp = checked_keypair_with_algorithm(Algorithm::BlsNormal);
    iroha_model_base::peer::PeerId::new(kp.public_key().clone())
}
#[test]
fn checked_keypair_helpers_preserve_requested_algorithm() {
    assert_eq!(checked_keypair().algorithm(), Algorithm::default());
    assert_eq!(
        checked_keypair_with_algorithm(Algorithm::Ed25519).algorithm(),
        Algorithm::Ed25519
    );
    assert_eq!(
        checked_keypair_with_algorithm(Algorithm::BlsNormal).algorithm(),
        Algorithm::BlsNormal
    );
}
fn alice() -> AccountId {
    iroha_test_samples::ALICE_ID.clone()
}
fn pipeline_fee_state_fixture(
    state_constructor: fn(World) -> State,
) -> (
    State,
    KeyPair,
    AccountId,
    AccountId,
    AccountId,
    AssetDefinitionId,
    AssetDefinitionId,
) {
    let (world, keypair, authority, initial_tech, updated_tech, fee_asset, alternate_asset) =
        pipeline_fee_world_fixture();
    (
        state_constructor(world),
        keypair,
        authority,
        initial_tech,
        updated_tech,
        fee_asset,
        alternate_asset,
    )
}
fn pipeline_fee_world_fixture() -> (
    World,
    KeyPair,
    AccountId,
    AccountId,
    AccountId,
    AssetDefinitionId,
    AssetDefinitionId,
) {
    let (authority, authority_keypair) = gen_account_in("pipeline_fee");
    let (initial_tech, _) = gen_account_in("pipeline_fee");
    let (updated_tech, _) = gen_account_in("pipeline_fee");
    let domain_id = DomainId::try_new("pipeline_fee", "universal").expect("pipeline fee domain");
    let gas_asset = AssetDefinitionId::derive_from_components(
        domain_id.clone(),
        "gas".parse().expect("pipeline gas asset name"),
    );
    let alternate_asset = AssetDefinitionId::derive_from_components(
        domain_id.clone(),
        "alternate".parse().expect("alternate gas asset name"),
    );
    let mut world = World::with_assets(
        [Domain::new(domain_id).build(&authority)],
        [
            Account::new(authority.clone()).build(&authority),
            Account::new(initial_tech.clone()).build(&authority),
            Account::new(updated_tech.clone()).build(&authority),
        ],
        [
            AssetDefinition::numeric(
                gas_asset.clone(),
                "pipeline gas".to_owned(),
                AssetBalancePolicy::Global,
                None,
            )
            .build(&authority),
            AssetDefinition::numeric(
                alternate_asset.clone(),
                "alternate gas".to_owned(),
                AssetBalancePolicy::Global,
                None,
            )
            .build(&authority),
        ],
        [Asset::new(
            AssetId::new(gas_asset.clone(), authority.clone()),
            Quantity::from(1_000_000_u32),
        )],
        [],
    );
    seed_test_asset_supply(&mut world, &gas_asset);
    seed_test_asset_supply(&mut world, &alternate_asset);
    world.account_permissions.insert(
        authority.clone(),
        BTreeSet::from([executor_permission::parameter::CanSetParameters.into()]),
    );
    (
        world,
        authority_keypair,
        authority,
        initial_tech,
        updated_tech,
        gas_asset,
        alternate_asset,
    )
}
fn configure_pipeline_fee_snapshot(
    state_transaction: &mut StateTransaction<'_, '_>,
    tech_account: &AccountId,
    gas_asset: &AssetDefinitionId,
    units_per_gas: u64,
) {
    let gas_asset = gas_asset.canonical_address();
    state_transaction.pipeline.gas.tech_account_id = tech_account.to_string();
    state_transaction.pipeline.gas.accepted_assets = vec![gas_asset.clone()];
    state_transaction.pipeline.gas.units_per_gas =
        vec![iroha_config::parameters::actual::GasRate {
            asset: gas_asset,
            units_per_gas,
            twap_local_per_xor: Numeric::one(),
            liquidity: GasLiquidity::Tier1,
            volatility: GasVolatility::Stable,
        }];
}
pub(super) fn sponsored_pipeline_fee_fixture(
    lease_allocation: Option<Quantity>,
) -> (
    State,
    SignedTransaction,
    AccountId,
    AccountId,
    AccountId,
    AssetDefinitionId,
    FeeSponsorProgramId,
    Hash,
) {
    let (sponsor, _) = gen_account_in("sponsored_pipeline_fee");
    let (beneficiary, beneficiary_keypair) = gen_account_in("sponsored_pipeline_fee");
    let (custody, _) = gen_account_in("sponsored_pipeline_fee");
    let (tech_account, _) = gen_account_in("sponsored_pipeline_fee");
    let domain_id = DomainId::try_new("sponsored_pipeline_fee", "universal")
        .expect("sponsored pipeline fee domain");
    let asset_definition_id = AssetDefinitionId::derive_from_components(
        domain_id.clone(),
        "gas".parse().expect("sponsored pipeline gas asset name"),
    );
    let instruction =
        InstructionBox::from(Log::new(Level::INFO, "sponsored pipeline gas".to_owned()));
    let wire_id = iroha_data_model::isi::instruction_wire_id(&instruction)
        .expect("Log instruction has a registered wire id")
        .to_owned();
    let program_id = FeeSponsorProgramId::new(
        sponsor.clone(),
        "pipeline".parse().expect("sponsor program name"),
    );
    let mut world = World::with_assets(
        [Domain::new(domain_id.clone()).build(&sponsor)],
        [
            Account::new(sponsor.clone()).build(&sponsor),
            Account::new(beneficiary.clone()).build(&sponsor),
            Account::new(custody.clone()).build(&sponsor),
            Account::new(tech_account.clone()).build(&sponsor),
        ],
        [AssetDefinition::numeric(
            asset_definition_id.clone(),
            "sponsored pipeline gas".to_owned(),
            AssetBalancePolicy::Global,
            None,
        )
        .build(&sponsor)],
        [Asset::new(
            AssetId::new(asset_definition_id.clone(), custody.clone()),
            Quantity::from(10_u32),
        )],
        [],
    );
    seed_test_asset_supply(&mut world, &asset_definition_id);
    let mut program = iroha_data_model::nexus::FeeSponsorProgram::new(
        program_id.clone(),
        program_id.sponsor.clone(),
    );
    program.lifecycle = FeeSponsorProgramLifecycle::Active;
    program.active_revision = Some(1);
    world
        .fee_sponsor_programs
        .insert(program_id.clone(), program);
    world.fee_sponsor_program_revisions.insert(
        FeeSponsorProgramRevisionKey::new(program_id.clone(), 1),
        FeeSponsorProgramRevision {
            program_id: program_id.clone(),
            revision: 1,
            eligibility: FeeSponsorEligibility::EnrolledOnly,
            rules: vec![iroha_data_model::nexus::FeeSponsorRule {
                id: "allow_log".parse().expect("sponsor rule id"),
                effect: FeeSponsorRuleEffect::Allow,
                selectors: vec![FeeSponsorRuleSelector::NativeInstruction(
                    iroha_data_model::nexus::FeeSponsorNativeInstructionSelector {
                        wire_id,
                        asset_definition_id: None,
                    },
                )],
            }],
            asset_budgets: vec![iroha_data_model::nexus::FeeSponsorAssetBudget {
                asset_definition_id: asset_definition_id.clone(),
                per_transaction: Quantity::from(10_u32),
                per_block: Quantity::from(10_u32),
                per_program_epoch: Quantity::from(10_u32),
                per_beneficiary_epoch: Quantity::from(10_u32),
                reserve_floor: Quantity::zero(),
                epoch_length_blocks: nonzero!(1_u64),
            }],
        },
    );
    let enrollment_key = FeeSponsorEnrollmentKey {
        program_id: program_id.clone(),
        beneficiary: beneficiary.clone(),
    };
    world.fee_sponsor_enrollments.insert(
        enrollment_key.clone(),
        iroha_data_model::nexus::FeeSponsorEnrollment {
            key: enrollment_key,
            enrolled_at_height: 1,
        },
    );
    let vault_key = FeeSponsorVaultKey {
        program_id: program_id.clone(),
        asset_definition_id: asset_definition_id.clone(),
    };
    world.fee_sponsor_vaults.insert(
        vault_key.clone(),
        iroha_data_model::nexus::FeeSponsorVault {
            key: vault_key,
            balance: Quantity::from(10_u32),
        },
    );
    let lease_id = Hash::new(b"sponsored-pipeline-fee-lease");
    if let Some(allocation) = lease_allocation {
        insert_relay_allocation(
            &mut world,
            &relay_allocation_fixture(
                program_id.clone(),
                1,
                asset_definition_id.clone(),
                allocation,
                DataSpaceId::UNIVERSAL,
                20,
                lease_id,
            ),
        );
    }
    let mut fees = iroha_config::parameters::defaults::nexus::fees::genesis_policy();
    fees.base_fee = Quantity::zero();
    fees.per_byte_fee = Quantity::zero();
    fees.per_instruction_fee = Quantity::zero();
    fees.per_gas_unit_fee = Quantity::zero();
    fees.sponsor_vault_custody_account_id = custody.clone();
    author_component_fee_policy(&world, &fees);
    let state = state_after_genesis(world);
    let transaction = TransactionBuilder::new(
        state.network_id,
        beneficiary.clone(),
        FeePaymentIntent::sponsor(
            program_id.clone(),
            1,
            vec![FeeChargeLimit::new(
                FeeChargeKind::PipelineGas,
                asset_definition_id.clone(),
                Quantity::from(2_u32),
            )],
            None,
        ),
    )
    .with_instructions([instruction])
    .sign(beneficiary_keypair.private_key());
    (
        state,
        transaction,
        beneficiary,
        custody,
        tech_account,
        asset_definition_id,
        program_id,
        lease_id,
    )
}
fn configure_sponsored_pipeline_fee_transaction(
    state_transaction: &mut StateTransaction<'_, '_>,
    custody: &AccountId,
    tech_account: &AccountId,
    asset_definition_id: &AssetDefinitionId,
) {
    state_transaction.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
    let original = state_transaction
        .root_fee_policy()
        .expect("original sponsored fees");
    assert!(original.per_gas_unit_fee.is_zero());
    assert_eq!(&original.sponsor_vault_custody_account_id, custody);
    configure_pipeline_fee_snapshot(state_transaction, tech_account, asset_definition_id, 1);
}
fn install_fee_fixture_network_currency(
    state_transaction: &mut StateTransaction<'_, '_>,
    fee_asset: &AssetDefinitionId,
) {
    let mut parameters = state_transaction
        .world
        .sumeragi_npos_parameters()
        .expect("original policy decoder completes")
        .unwrap_or_default();
    parameters.xor_asset_definition_id = fee_asset.clone();
    state_transaction
        .world
        .set_parameter_for_testing(iroha_data_model::parameter::Parameter::Custom(
            parameters.into_custom_parameter(),
        ))
        .expect("valid fixture parameters");
}
// Explicit component authoring precedes the original block cut. The signed
// genesis history remains unchanged; this does not manufacture signed authority.
fn author_direct_fee_policy(state: &State, fee_asset: &AssetDefinitionId, base_fee: Quantity) {
    let mut fees = state
        .world
        .parameters
        .view()
        .root_fees()
        .expect("original fixture fee policy")
        .clone();
    fees.fee_asset_id = fee_asset.clone();
    fees.base_fee = base_fee;
    fees.per_byte_fee = Quantity::zero();
    fees.per_instruction_fee = Quantity::zero();
    fees.per_gas_unit_fee = Quantity::zero();
    author_component_fee_policy(&state.world, &fees);
}
fn test_asset_balance(
    state_transaction: &StateTransaction<'_, '_>,
    asset_id: &AssetId,
) -> Quantity {
    state_transaction
        .world
        .assets()
        .get(asset_id)
        .map_or_else(Quantity::zero, |asset| asset.as_ref().clone())
}
fn configure_direct_ivm_fee_fixture(
    state_transaction: &mut StateTransaction<'_, '_>,
    authority: &AccountId,
    fee_asset: &AssetDefinitionId,
) -> (AssetId, Quantity, Quantity) {
    install_fee_fixture_network_currency(state_transaction, fee_asset);
    let original = state_transaction
        .root_fee_policy()
        .expect("original direct fees");
    assert_eq!(&original.fee_asset_id, fee_asset);
    assert_eq!(original.base_fee, Quantity::from(2_u32));
    assert!(original.per_byte_fee.is_zero());
    assert!(original.per_instruction_fee.is_zero());
    assert!(original.per_gas_unit_fee.is_zero());
    let payer_asset_id = AssetId::new(fee_asset.clone(), authority.clone());
    let payer_before = test_asset_balance(state_transaction, &payer_asset_id);
    let supply_before = state_transaction
        .world
        .asset_definition(fee_asset)
        .expect("direct-fee asset definition")
        .total_quantity()
        .clone();
    (payer_asset_id, payer_before, supply_before)
}
// The signed producer owns every bootstrap input and its execution/fee configuration.
// Neither a fake H1 carrier nor a caller-built Genesis-domain IVM transaction is admitted.
fn authenticated_genesis_fee_config() -> (
    crate::sumeragi::test_chain::TestChainConfig,
    AccountId,
    AssetDefinitionId,
) {
    let (authority, keypair) = gen_account_in("authenticated_genesis_fee");
    let domain = DomainId::try_new("authenticated_genesis_fee", "universal")
        .expect("authenticated genesis fee domain");
    // Permissioned global genesis uses the canonical default XOR identity;
    // an arbitrary pipeline gas token is not its signed network currency.
    let fee_asset = AssetDefinitionId::parse_address_literal(
        &iroha_config::parameters::defaults::nexus::fees::fee_asset_id(),
    )
    .expect("canonical permissioned network XOR identity");
    let mut world = World::with_assets(
        [Domain::new(domain).build(&authority)],
        [Account::new(authority.clone()).build(&authority)],
        [AssetDefinition::new(
            fee_asset.clone(),
            "Genesis XOR",
            iroha_primitives::numeric::NumericSpec::fractional(9),
            AssetBalancePolicy::Global,
            None,
        )
        .build(&authority)],
        [Asset::new(
            AssetId::new(fee_asset.clone(), authority.clone()),
            Quantity::from(1_000_000_u32),
        )],
        [],
    );
    world.account_permissions.insert(
        authority.clone(),
        BTreeSet::from([executor_permission::parameter::CanSetParameters.into()]),
    );
    let mut config = crate::sumeragi::test_chain::TestChainConfig::new(world, 0);
    config.genesis_key = keypair;
    config.consensus_mode = iroha_data_model::parameter::system::SumeragiConsensusMode::Npos;
    let mut nexus = iroha_config::parameters::actual::Nexus::default();
    nexus.fees.fee_asset_id = fee_asset.clone();
    nexus.fees.base_fee = Quantity::from(2_u32);
    nexus.fees.per_byte_fee = Quantity::zero();
    nexus.fees.per_instruction_fee = Quantity::zero();
    nexus.fees.per_gas_unit_fee = Quantity::zero();
    let fee_parameter = iroha_data_model::parameter::Parameter::Custom(
        nexus
            .fees
            .clone()
            .into_custom_parameter()
            .expect("signed original fee recipe"),
    );
    if let Some(slot) = config.genesis_parameters.iter_mut().find(|parameter| {
        matches!(
            parameter,
            iroha_data_model::parameter::Parameter::Custom(value)
                if value.id() == &RootFeePolicy::parameter_id()
        )
    }) {
        *slot = fee_parameter;
    } else {
        config.genesis_parameters.push(fee_parameter);
    }
    config.nexus = Some(nexus);
    config
        .genesis_parameters
        .push(iroha_data_model::parameter::Parameter::Custom(
            iroha_data_model::parameter::system::SumeragiNposParameters {
                xor_asset_definition_id: fee_asset.clone(),
                ..Default::default()
            }
            .into_custom_parameter(),
        ));
    (config, authority, fee_asset)
}

fn execute_authenticated_genesis_fee_fixture<'state>(
    prepared: &'state crate::sumeragi::test_chain::PreparedTestChainConfig,
    authority: &AccountId,
) -> Box<crate::state::StateBlock<'state>> {
    let topology = crate::sumeragi::network_topology::Topology::new(
        prepared
            .validator_keys
            .iter()
            .map(|key| PeerId::new(key.public_key().clone())),
    );
    let (valid, block) = {
        let selected_genesis = prepared.genesis.block().clone();
        let selected_scope = crate::sumeragi::epoch::authenticated_genesis(&selected_genesis)
            .expect("signed fixture root scope")
            .into_parts()
            .1;
        crate::block::ValidBlock::validate_signed_genesis(
            selected_genesis,
            &topology,
            authority,
            &iroha_primitives::time::TimeSource::new_system(),
            &prepared.state,
            iroha_data_model::parameter::system::ConsensusMode::Permissioned,
            selected_scope,
        )
    }
    .unpack(|_| {})
    .expect("execute the exact validated signed fee fixture genesis");
    assert!(
        valid
            .as_ref()
            .execution_outputs()
            .iter()
            .all(|output| output.result().is_ok())
    );
    block
}

#[test]
fn stateful_fee_admission_exempts_authenticated_genesis_with_missing_limit() {
    let (mut config, authority, _) = authenticated_genesis_fee_config();
    config
        .genesis_instructions
        .push(Log::new(Level::INFO, "genesis fee exemption".to_owned()).into());
    let prepared = crate::sumeragi::test_chain::CertifiedTestChain::prepare(config)
        .expect("prepare original signed fee-exempt genesis without fee limits");
    assert!(
        prepared
            .genesis
            .block()
            .external_transactions()
            .all(|transaction| transaction.fee_payment_intent().charge_limits().is_empty())
    );
    let block = execute_authenticated_genesis_fee_fixture(&prepared, &authority);
    // Inspect the completed original genesis cut without borrowing a new transaction.
    assert!(block._curr_block.is_genesis() && block.block_hashes.is_empty());
    assert_eq!(prepared.state.committed_height(), 0);
}
#[test]
fn initial_genesis_context_rejects_height_one_replay_over_committed_history() {
    let (state, _, _, _, _, _, _) = pipeline_fee_state_fixture(state_for_testing);
    state
        .block(BlockHeader::new(nonzero!(1_u64), None, None, 0, 0))
        .commit_empty_block_for_testing()
        .expect("commit the prior authenticated genesis block");
    let mut block = state.block(BlockHeader::new(nonzero!(1_u64), None, None, 0, 0));
    let state_transaction = block.transaction();
    assert!(state_transaction._curr_block.is_genesis());
    assert!(
        !is_initial_genesis_context(&state_transaction),
        "a genesis-shaped header over committed state must not regain bootstrap authority"
    );
}
#[test]
fn transaction_execution_keeps_authenticated_genesis_fee_free() {
    let (mut config, authority, fee_asset) = authenticated_genesis_fee_config();
    config
        .genesis_instructions
        .push(Log::new(Level::INFO, "fee-free genesis execution".to_owned()).into());
    let payer_asset_id = AssetId::new(fee_asset.clone(), authority.clone());
    let payer_before = config
        .world
        .assets
        .view()
        .get(&payer_asset_id)
        .unwrap()
        .as_ref()
        .clone();
    let supply_before = config
        .world
        .asset_definitions
        .view()
        .get(&fee_asset)
        .unwrap()
        .total_quantity()
        .clone();
    let prepared = crate::sumeragi::test_chain::CertifiedTestChain::prepare(config)
        .expect("prepare original signed native fee fixture");
    assert!(
        prepared
            .genesis
            .block()
            .external_transactions()
            .all(|transaction| transaction.fee_payment_intent().charge_limits().is_empty())
    );
    let block = execute_authenticated_genesis_fee_fixture(&prepared, &authority);
    assert_eq!(
        block
            .world
            .assets
            .get(&payer_asset_id)
            .unwrap()
            .as_ref()
            .clone(),
        payer_before,
        "native genesis execution must not debit its payer"
    );
    assert_eq!(
        block
            .world
            .asset_definition(&fee_asset)
            .expect("direct-fee asset definition")
            .total_quantity(),
        &supply_before,
        "native genesis execution must not burn fee-asset supply"
    );
}
#[test]
fn genesis_raw_ivm_carriers_reject_before_debit_or_supply_change() {
    let (prepared, _) = contract_program_with_entrypoint("run", None);
    for program in [generate_ok_program(), prepared] {
        let (world, keypair, authority, _, _, fee_asset, _) = pipeline_fee_world_fixture();
        let payer = AssetId::new(fee_asset.clone(), authority.clone());
        let payer_before = world.assets.view().get(&payer).unwrap().as_ref().clone();
        let supply_before = world
            .asset_definitions
            .view()
            .get(&fee_asset)
            .unwrap()
            .total_quantity()
            .clone();
        let signed = TransactionBuilder::new_genesis(
            authority.clone(),
            FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_executable(Executable::Ivm(IvmBytecode::from_compiled(program)))
        .sign(keypair.private_key());
        let original = iroha_data_model::block::SignedBlock::genesis(
            vec![signed.clone()],
            keypair.private_key(),
            None,
            None,
        );
        assert_eq!(
            crate::block::authenticate_genesis_block_intents(&original, &authority).unwrap_err(),
            crate::block::InvalidGenesisError::NotInstructions
        );
        let state = state_for_testing(world);
        let mut block = state.block(BlockHeader::new(nonzero!(1_u64), None, None, 0, 0));
        let mut transaction = block.transaction();
        let error = super::Executor::Initial
            .execute_transaction(&mut transaction, &authority, &signed, &mut IvmCache::new())
            .map_err(crate::execution_attempt::expect_completed_rejection)
            .expect_err("raw IVM cannot acquire an original native genesis capability");
        assert!(matches!(error, ValidationFail::NotPermitted(reason)
                if reason == "genesis instruction execution requires its authenticated source capability"));
        assert_eq!(transaction.last_tx_gas_used, 0);
        assert_eq!(
            transaction.world.assets.get(&payer).unwrap().as_ref(),
            &payer_before
        );
        assert_eq!(
            transaction
                .world
                .asset_definitions
                .get(&fee_asset)
                .unwrap()
                .total_quantity(),
            &supply_before
        );
        drop(transaction);
        assert_eq!(block.committed_fragment_count(), 0);
    }
}
#[test]
fn transaction_execution_keeps_authenticated_genesis_generic_ivm_fee_free() {
    let (mut config, authority, fee_asset) = authenticated_genesis_fee_config();
    let trigger_id: TriggerId = "authenticated_genesis_generic_ivm".parse().unwrap();
    let mut program = ivm::ProgramMetadata {
        max_cycles: 100,
        ..Default::default()
    }
    .encode();
    program.extend_from_slice(
        &ivm::encoding::wide::encode_ri(ivm::instruction::wide::arithmetic::ADDI, 5, 0, 1)
            .to_le_bytes(),
    );
    program.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());
    let trigger = Trigger::new(
        trigger_id.clone(),
        Action::new(
            Executable::Ivm(IvmBytecode::from_compiled(program)),
            Repeats::Exactly(1),
            authority.clone(),
            ExecuteTriggerEventFilter::new()
                .for_trigger(trigger_id.clone())
                .under_authority(authority.clone()),
        )
        .expect("supported generic IVM genesis trigger"),
    );
    config.genesis_instructions = vec![
        Register::trigger(trigger).into(),
        ExecuteTrigger::new(trigger_id.clone()).into(),
    ];
    let payer_asset_id = AssetId::new(fee_asset.clone(), authority.clone());
    let payer_before = config
        .world
        .assets
        .view()
        .get(&payer_asset_id)
        .unwrap()
        .as_ref()
        .clone();
    let supply_before = config
        .world
        .asset_definitions
        .view()
        .get(&fee_asset)
        .unwrap()
        .total_quantity()
        .clone();
    let prepared = crate::sumeragi::test_chain::CertifiedTestChain::prepare(config)
        .expect("prepare exact signed generic trigger genesis");
    let block = execute_authenticated_genesis_fee_fixture(&prepared, &authority);
    assert!(
        block.gas_used_in_block > 0,
        "authentic genesis callback retains measured work"
    );
    let events = block.world.pending_external_events();
    assert!(events.iter().any(|event| matches!(event,
            EventBox::TriggerCompleted(event) if event.trigger_id() == &trigger_id &&
                event.outcome() == &iroha_data_model::events::trigger_completed::TriggerCompletedOutcome::Success
        )), "the actual generic IVM callback completed successfully");
    assert_eq!(
        block
            .world
            .assets
            .get(&payer_asset_id)
            .unwrap()
            .as_ref()
            .clone(),
        payer_before,
        "generic IVM genesis execution must not debit its payer"
    );
    assert_eq!(
        block
            .world
            .asset_definition(&fee_asset)
            .expect("direct-fee asset definition")
            .total_quantity(),
        &supply_before,
        "generic IVM genesis execution must not burn fee-asset supply"
    );
}
#[test]
fn local_checkout_deferral_abandons_transaction_without_gas_or_fee() {
    check_local_checkout_abandons_transaction(ivm::error::ExecutionDeferral::ActiveMemoryCapacity);
}
#[test]
fn local_custody_invariant_abandons_transaction_without_gas_or_fee() {
    check_local_checkout_abandons_transaction(
        ivm::error::ExecutionDeferral::LocalInvariantViolation,
    );
}
fn check_local_checkout_abandons_transaction(reason: ivm::error::ExecutionDeferral) {
    let (state, keypair, authority, _, _, fee_asset, _) =
        pipeline_fee_state_fixture(state_after_genesis);
    let mut program = ivm::ProgramMetadata {
        max_cycles: 100,
        ..ivm::ProgramMetadata::default()
    }
    .encode();
    program.extend_from_slice(
        &ivm::encoding::wide::encode_ri(ivm::instruction::wide::arithmetic::ADDI, 5, 0, 1)
            .to_le_bytes(),
    );
    program.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());
    let transaction = TransactionBuilder::new(
        state.network_id,
        authority.clone(),
        FeePaymentIntent::authority(
            vec![FeeChargeLimit::new(
                FeeChargeKind::Nexus,
                fee_asset.clone(),
                Quantity::from(2_u32),
            )],
            core::num::NonZeroU64::new(1_000_000),
        ),
    )
    .with_executable(Executable::Ivm(IvmBytecode::from_compiled(program)))
    .sign(keypair.private_key());
    let entrypoint_hash = transaction.hash_as_entrypoint();
    let signed_hash = transaction.hash();
    // This direct component test supplies the exact signed source fields that
    // the Network producer captures before execution and fee settlement.
    let bind_source = |child: &mut StateTransaction<'_, '_>| {
        child.current_entrypoint_index = Some(0);
        child.current_network_entrypoint_hash = Some(entrypoint_hash);
        child.current_tx_hash = Some(signed_hash);
        child.current_lane_id = Some(iroha_model_base::topology::LaneId::SINGLE);
        child.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
        child.world.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
    };
    author_direct_fee_policy(&state, &fee_asset, Quantity::from(2_u32));
    let mut block = state.block(BlockHeader::new(
        nonzero!(2_u64),
        Some(
            state
                .view()
                .latest_block_hash()
                .expect("genuine genesis parent"),
        ),
        None,
        0,
        0,
    ));
    let mut state_transaction =
        block.transaction_for_fastpq_testing(Hash::from(transaction.hash_as_entrypoint()));
    let (payer_asset_id, payer_before, supply_before) =
        configure_direct_ivm_fee_fixture(&mut state_transaction, &authority, &fee_asset);
    bind_source(&mut state_transaction);
    let retry_transaction = transaction.clone();
    let mut ivm_cache = IvmCache::new();
    let shared_cache = ivm_cache.prepared_contract_cache();
    shared_cache.set_checkout_refusal_for_test(Some(reason));
    let result = super::Executor::Initial.execute_transaction(
        &mut state_transaction,
        &authority,
        &transaction,
        &mut ivm_cache,
    );
    assert!(
        matches!(result, Err(crate::execution_attempt::ExecutionAttemptError::Deferred(original)) if original.reason() == reason)
    );
    assert_eq!(state_transaction.last_tx_gas_used, 0);
    assert!(state_transaction.is_execution_deferred());
    assert!(
        state_transaction.borrow_execution_deferral().is_none(),
        "outer terminal transferred the owner once"
    );
    // Releasing local pressure cannot bless this already incomplete overlay.
    shared_cache.set_checkout_refusal_for_test(None);
    assert_eq!(
        test_asset_balance(&state_transaction, &payer_asset_id),
        payer_before,
        "deferred generic IVM execution must not debit its payer"
    );
    assert_eq!(
        state_transaction
            .world
            .asset_definition(&fee_asset)
            .expect("direct-fee asset definition")
            .total_quantity(),
        &supply_before,
        "deferred generic IVM execution must not burn fee-asset supply"
    );
    drop(state_transaction);
    let mut retry =
        block.transaction_for_fastpq_testing(Hash::from(retry_transaction.hash_as_entrypoint()));
    configure_direct_ivm_fee_fixture(&mut retry, &authority, &fee_asset);
    bind_source(&mut retry);
    super::Executor::Initial
        .execute_transaction(&mut retry, &authority, &retry_transaction, &mut ivm_cache)
        .expect("fresh attempt executes after local capacity returns");
    let retry_gas = retry.last_tx_gas_used;
    let retry_balance = test_asset_balance(&retry, &payer_asset_id);
    drop(retry);
    let mut cold =
        block.transaction_for_fastpq_testing(Hash::from(retry_transaction.hash_as_entrypoint()));
    configure_direct_ivm_fee_fixture(&mut cold, &authority, &fee_asset);
    bind_source(&mut cold);
    super::Executor::Initial
        .execute_transaction(
            &mut cold,
            &authority,
            &retry_transaction,
            &mut IvmCache::new(),
        )
        .expect("cold attempt executes identically");
    assert_eq!(cold.last_tx_gas_used, retry_gas);
    assert!(
        retry_gas > 0,
        "the compared successful executions actually consumed gas"
    );
    assert_eq!(test_asset_balance(&cold, &payer_asset_id), retry_balance);
}
#[test]
fn transaction_execution_keeps_rejected_genesis_prepared_contract_fee_free() {
    let (mut config, authority, fee_asset) = authenticated_genesis_fee_config();
    let (program, _) =
        contract_program_with_entrypoint("run", Some("CanRunGenesisPreparedContract"));
    ivm::verify_contract_artifact(&program).expect("verify current prepared artifact");
    let trigger_id: TriggerId = "authenticated_genesis_prepared_ivm".parse().unwrap();
    let mut metadata = Metadata::default();
    metadata.insert("contract_entrypoint".parse().unwrap(), Json::new("run"));
    let trigger = Trigger::new(
        trigger_id.clone(),
        Action::new(
            Executable::Ivm(IvmBytecode::from_compiled(program)),
            Repeats::Exactly(1),
            authority.clone(),
            ExecuteTriggerEventFilter::new()
                .for_trigger(trigger_id.clone())
                .under_authority(authority.clone()),
        )
        .expect("schema-valid prepared genesis trigger")
        .with_metadata(metadata),
    );
    config.genesis_instructions = vec![
        Register::trigger(trigger).into(),
        ExecuteTrigger::new(trigger_id).into(),
    ];
    let payer_asset_id = AssetId::new(fee_asset.clone(), authority.clone());
    let payer_before = config
        .world
        .assets
        .view()
        .get(&payer_asset_id)
        .unwrap()
        .as_ref()
        .clone();
    let supply_before = config
        .world
        .asset_definitions
        .view()
        .get(&fee_asset)
        .unwrap()
        .total_quantity()
        .clone();
    let failure = crate::sumeragi::test_chain::CertifiedTestChain::prepare(config)
        .expect_err("signed genesis cannot invent a committed prepared-contract identity");
    assert!(
        matches!(&failure.error,
            crate::sumeragi::test_chain::TestChainError::OriginalGenesisExecution(error)
            if matches!(error.as_ref(),
                crate::block::BlockValidationError::InvalidGenesis(
                    crate::block::InvalidGenesisError::RejectedOutput(_)
                )
            )
        ),
        "unexpected authentic producer refusal: {}",
        failure.error
    );
    assert_eq!(
        failure.state.committed_height(),
        0,
        "rejected genesis publishes no effects"
    );
    let mut block = failure
        .state
        .block(BlockHeader::new(nonzero!(1_u64), None, None, 0, 0));
    assert_eq!(
        block.gas_used_in_block, 0,
        "rejected genesis retains no gas effects"
    );
    let state_transaction = block.transaction();
    assert_eq!(
        test_asset_balance(&state_transaction, &payer_asset_id),
        payer_before,
        "prepared-contract IVM genesis execution must not debit its payer"
    );
    assert_eq!(
        state_transaction
            .world
            .asset_definition(&fee_asset)
            .expect("direct-fee asset definition")
            .total_quantity(),
        &supply_before,
        "prepared-contract IVM genesis execution must not burn fee-asset supply"
    );
}
#[test]
fn stateful_fee_admission_does_not_exempt_non_genesis_with_missing_limit() {
    let (state, keypair, authority, _, _, fee_asset, _) =
        pipeline_fee_state_fixture(state_after_genesis);
    let transaction = TransactionBuilder::new(
        state.network_id,
        authority,
        FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([Log::new(
        Level::INFO,
        "non-genesis fee validation".to_owned(),
    )])
    .sign(keypair.private_key());
    author_direct_fee_policy(&state, &fee_asset, Quantity::from(2_u32));
    let mut block = state.block(BlockHeader::new(
        nonzero!(2_u64),
        Some(
            state
                .view()
                .latest_block_hash()
                .expect("genuine genesis parent"),
        ),
        None,
        0,
        0,
    ));
    let mut state_transaction = block.transaction();
    install_fee_fixture_network_currency(&mut state_transaction, &fee_asset);
    assert!(!is_initial_genesis_context(&state_transaction));
    let error = validate_transaction_fee_admission(&mut state_transaction, &transaction)
        .expect_err("non-genesis must validate its signed Nexus fee limit");
    assert!(matches!(
        error,
        ValidationFail::NotPermitted(reason)
            if reason.contains("signed fee intent is missing Nexus charge limit")
    ));
}
#[test]
fn stateful_fee_admission_rejects_understated_authority_limit_before_effects() {
    let (state, keypair, authority, tech_account, _, gas_asset, _) =
        pipeline_fee_state_fixture(state_after_genesis);
    let effect_parameter_id: CustomParameterId = "fee_admission_business_effect"
        .parse()
        .expect("effect parameter id");
    let instructions = vec![InstructionBox::from(
        iroha_data_model::isi::SetParameter::new(iroha_data_model::parameter::Parameter::Custom(
            CustomParameter::new(effect_parameter_id.clone(), Json::new(1_u32)),
        )),
    )];
    assert!(
        isi_gas::meter_instructions(&instructions, &crate::gas::DEFAULT_ZK_GAS)
            .expect("completed fixture gas quote")
            > 0
    );
    let transaction = TransactionBuilder::new(
        state.network_id,
        authority.clone(),
        FeePaymentIntent::authority(
            vec![FeeChargeLimit::new(
                FeeChargeKind::PipelineGas,
                gas_asset.clone(),
                Quantity::from(1_u32),
            )],
            None,
        ),
    )
    .with_instructions(instructions)
    .sign(keypair.private_key());
    let mut block = state.block(BlockHeader::new(
        nonzero!(2_u64),
        Some(
            state
                .view()
                .latest_block_hash()
                .expect("genuine genesis parent"),
        ),
        None,
        0,
        0,
    ));
    let mut state_transaction = block.transaction();
    configure_pipeline_fee_snapshot(&mut state_transaction, &tech_account, &gas_asset, 2);
    let error = validate_transaction_fee_admission(&mut state_transaction, &transaction)
        .expect_err("an understated signed limit must fail stateful admission");
    assert!(matches!(
        error,
        ValidationFail::NotPermitted(reason)
            if reason.contains("exceeds signed maximum")
    ));
    assert!(
        state_transaction
            .world
            .parameters
            .get()
            .custom()
            .get(&effect_parameter_id)
            .is_none(),
        "fee rejection must precede transaction business effects"
    );
    assert_eq!(
        state_transaction
            .world
            .assets()
            .get(&AssetId::new(gas_asset.clone(), authority))
            .expect("authority gas balance")
            .as_ref(),
        &Quantity::from(1_000_000_u32),
        "fee admission must not debit the authority"
    );
    assert!(
        state_transaction
            .world
            .assets()
            .get(&AssetId::new(gas_asset, tech_account))
            .is_none(),
        "fee admission must not credit the fee destination"
    );
}
#[test]
fn pipeline_fee_charge_defensively_rejects_understated_authority_limit() {
    let (state, keypair, authority, tech_account, _, gas_asset, _) =
        pipeline_fee_state_fixture(state_after_genesis);
    let transaction = TransactionBuilder::new(
        state.network_id,
        authority.clone(),
        FeePaymentIntent::authority(
            vec![FeeChargeLimit::new(
                FeeChargeKind::PipelineGas,
                gas_asset.clone(),
                Quantity::from(1_u32),
            )],
            None,
        ),
    )
    .with_instructions([Log::new(Level::INFO, "bounded".to_owned())])
    .sign(keypair.private_key());
    let mut block = state.block(BlockHeader::new(
        nonzero!(2_u64),
        Some(
            state
                .view()
                .latest_block_hash()
                .expect("genuine genesis parent"),
        ),
        None,
        0,
        0,
    ));
    let mut state_transaction = block.transaction();
    configure_pipeline_fee_snapshot(&mut state_transaction, &tech_account, &gas_asset, 2);
    let error = super::Executor::charge_pipeline_gas_asset_fee(
        &mut state_transaction,
        &authority,
        &transaction,
        &gas_asset.canonical_address(),
        1,
        None,
    )
    .expect_err("actual authority charge must be checked against the signed limit");
    assert!(matches!(
        error,
        ValidationFail::NotPermitted(reason)
            if reason.contains("exceeds signed maximum")
    ));
    assert_eq!(
        state_transaction
            .world
            .assets()
            .get(&AssetId::new(gas_asset.clone(), authority))
            .expect("authority gas balance")
            .as_ref(),
        &Quantity::from(1_000_000_u32)
    );
    assert!(
        state_transaction
            .world
            .assets()
            .get(&AssetId::new(gas_asset, tech_account))
            .is_none()
    );
}
#[test]
fn sponsored_pipeline_gas_direct_fee_preserves_unrelated_lease() {
    let (
        state,
        transaction,
        beneficiary,
        custody,
        tech_account,
        asset_definition_id,
        program_id,
        lease_id,
    ) = sponsored_pipeline_fee_fixture(Some(Quantity::from(10_u32)));
    let mut block = state.block(BlockHeader::new(
        nonzero!(10_u64),
        Some(
            state
                .view()
                .latest_block_hash()
                .expect("genuine genesis parent"),
        ),
        None,
        0,
        0,
    ));
    let mut state_transaction =
        block.transaction_for_fastpq_testing(Hash::from(transaction.hash_as_entrypoint()));
    configure_sponsored_pipeline_fee_transaction(
        &mut state_transaction,
        &custody,
        &tech_account,
        &asset_definition_id,
    );
    super::Executor::charge_pipeline_gas_asset_fee(
        &mut state_transaction,
        &beneficiary,
        &transaction,
        &asset_definition_id.canonical_address(),
        2,
        Some(&program_id),
    )
    .expect("sponsored direct PipelineGas settles actual custody");
    let executed_key =
        fee_sponsor_vault_allocation_usage_state_key(&lease_id).expect("executed usage key");
    let settled_key =
        fee_sponsor_vault_allocation_settled_usage_state_key(&lease_id).expect("settled usage key");
    assert_eq!(
        fee_sponsor_vault_allocation_quantity_at(&state_transaction.world, &executed_key)
            .expect("executed usage"),
        Quantity::zero()
    );
    assert_eq!(
        fee_sponsor_vault_allocation_quantity_at(&state_transaction.world, &settled_key)
            .expect("settled usage"),
        Quantity::zero()
    );
    let vault_key = FeeSponsorVaultKey {
        program_id,
        asset_definition_id: asset_definition_id.clone(),
    };
    assert_eq!(
        state_transaction
            .world
            .fee_sponsor_vaults
            .get(&vault_key)
            .expect("sponsor vault")
            .balance,
        Quantity::from(8_u32)
    );
    assert_eq!(
        state_transaction
            .world
            .assets()
            .get(&AssetId::new(asset_definition_id.clone(), custody))
            .expect("custody balance")
            .as_ref(),
        &Quantity::from(8_u32)
    );
    assert_eq!(
        state_transaction
            .world
            .assets()
            .get(&AssetId::new(asset_definition_id, tech_account))
            .expect("technical-account balance")
            .as_ref(),
        &Quantity::from(2_u32)
    );
}
#[test]
fn sponsored_pipeline_gas_direct_mode_does_not_require_or_consume_lease() {
    let (
        state,
        transaction,
        beneficiary,
        custody,
        tech_account,
        asset_definition_id,
        program_id,
        lease_id,
    ) = sponsored_pipeline_fee_fixture(None);
    let mut block = state.block(BlockHeader::new(
        nonzero!(10_u64),
        Some(
            state
                .view()
                .latest_block_hash()
                .expect("genuine genesis parent"),
        ),
        None,
        0,
        0,
    ));
    let mut state_transaction =
        block.transaction_for_fastpq_testing(Hash::from(transaction.hash_as_entrypoint()));
    configure_sponsored_pipeline_fee_transaction(
        &mut state_transaction,
        &custody,
        &tech_account,
        &asset_definition_id,
    );
    super::Executor::charge_pipeline_gas_asset_fee(
        &mut state_transaction,
        &beneficiary,
        &transaction,
        &asset_definition_id.canonical_address(),
        2,
        Some(&program_id),
    )
    .expect("direct-settled sponsored PipelineGas does not require a spend lease");
    let executed_key =
        fee_sponsor_vault_allocation_usage_state_key(&lease_id).expect("executed usage key");
    let settled_key =
        fee_sponsor_vault_allocation_settled_usage_state_key(&lease_id).expect("settled usage key");
    assert_eq!(
        fee_sponsor_vault_allocation_quantity_at(&state_transaction.world, &executed_key)
            .expect("executed usage"),
        Quantity::zero()
    );
    assert_eq!(
        fee_sponsor_vault_allocation_quantity_at(&state_transaction.world, &settled_key)
            .expect("settled usage"),
        Quantity::zero()
    );
    let vault_key = FeeSponsorVaultKey {
        program_id,
        asset_definition_id: asset_definition_id.clone(),
    };
    assert_eq!(
        state_transaction
            .world
            .fee_sponsor_vaults
            .get(&vault_key)
            .unwrap()
            .balance,
        Quantity::from(8_u32)
    );
    assert_eq!(
        test_asset_balance(
            &state_transaction,
            &AssetId::new(asset_definition_id.clone(), custody)
        ),
        Quantity::from(8_u32)
    );
    assert_eq!(
        test_asset_balance(
            &state_transaction,
            &AssetId::new(asset_definition_id, tech_account)
        ),
        Quantity::from(2_u32)
    );
}
#[test]
fn missing_committed_policy_fails_before_sponsored_debit_or_transfer() {
    for allocation in [1_u32, 10_u32] {
        let (
            state,
            transaction,
            beneficiary,
            custody,
            tech_account,
            asset_definition_id,
            program_id,
            lease_id,
        ) = sponsored_pipeline_fee_fixture(Some(Quantity::from(allocation)));
        let mut block = state.block(BlockHeader::new(
            nonzero!(10_u64),
            Some(
                state
                    .view()
                    .latest_block_hash()
                    .expect("genuine genesis parent"),
            ),
            None,
            0,
            0,
        ));
        let mut state_transaction =
            block.transaction_for_fastpq_testing(Hash::from(transaction.hash_as_entrypoint()));
        configure_sponsored_pipeline_fee_transaction(
            &mut state_transaction,
            &custody,
            &tech_account,
            &asset_definition_id,
        );
        {
            // Deliberately malformed structural fixture; no production removal bypass.
            let mut parameters = state_transaction.world.parameters.get().get().clone();
            let id = RootFeePolicy::parameter_id();
            let original = parameters.custom().get(&id).cloned();
            assert!(
                original.is_some(),
                "fixture starts with its committed fee policy"
            );
            assert_eq!(
                parameters
                    .remove_custom_parameter(&id)
                    .expect("remove only the fee fixture policy"),
                original,
            );
            let retained =
                crate::pipeline_gas_policy::RetainedStateParameters::try_copy_for_testing(
                    &parameters,
                    state_transaction
                        .world
                        .sccp_committee_generations
                        .allocation_budget(),
                )
                .expect("funded malformed predecessor fixture");
            *state_transaction.world.parameters.get_mut() = retained;
        }
        let error = super::Executor::charge_pipeline_gas_asset_fee(
            &mut state_transaction,
            &beneficiary,
            &transaction,
            &asset_definition_id.canonical_address(),
            2,
            Some(&program_id),
        )
        .expect_err("missing committed policy rejects even a fully funded spend lease");
        assert!(
            matches!(error, ValidationFail::InternalError(reason) if reason.contains("committed root fee policy is absent"))
        );
        let vault_key = FeeSponsorVaultKey {
            program_id,
            asset_definition_id: asset_definition_id.clone(),
        };
        assert_eq!(
            state_transaction
                .world
                .fee_sponsor_vaults
                .get(&vault_key)
                .expect("sponsor vault")
                .balance,
            Quantity::from(10_u32)
        );
        assert_eq!(
            state_transaction
                .world
                .assets()
                .get(&AssetId::new(asset_definition_id.clone(), custody))
                .expect("custody balance")
                .as_ref(),
            &Quantity::from(10_u32)
        );
        assert!(
            state_transaction
                .world
                .assets()
                .get(&AssetId::new(asset_definition_id.clone(), tech_account))
                .is_none()
        );
        let executed_key =
            fee_sponsor_vault_allocation_usage_state_key(&lease_id).expect("executed usage key");
        let settled_key = fee_sponsor_vault_allocation_settled_usage_state_key(&lease_id)
            .expect("settled usage key");
        assert_eq!(
            fee_sponsor_vault_allocation_quantity_at(&state_transaction.world, &executed_key)
                .expect("executed usage"),
            Quantity::zero()
        );
        assert_eq!(
            fee_sponsor_vault_allocation_quantity_at(&state_transaction.world, &settled_key)
                .expect("settled usage"),
            Quantity::zero()
        );
    }
}
#[test]
fn sponsor_resolution_predicts_scheduled_revision_only_after_old_leases_drain() {
    let (
        state,
        transaction,
        beneficiary,
        custody,
        tech_account,
        asset_definition_id,
        program_id,
        _,
    ) = sponsored_pipeline_fee_fixture(Some(Quantity::from(10_u32)));
    let mut block = state.block(BlockHeader::new(
        nonzero!(10_u64),
        Some(
            state
                .view()
                .latest_block_hash()
                .expect("genuine genesis parent"),
        ),
        None,
        0,
        0,
    ));
    let mut state_transaction = block.transaction();
    configure_sponsored_pipeline_fee_transaction(
        &mut state_transaction,
        &custody,
        &tech_account,
        &asset_definition_id,
    );
    let revision_one = state_transaction
        .world
        .fee_sponsor_program_revisions
        .get(&FeeSponsorProgramRevisionKey::new(program_id.clone(), 1))
        .cloned()
        .expect("revision one");
    let mut revision_two = revision_one;
    revision_two.revision = 2;
    state_transaction
        .world
        .fee_sponsor_program_revisions
        .insert(
            FeeSponsorProgramRevisionKey::new(program_id.clone(), 2),
            revision_two,
        );
    let mut program = state_transaction
        .world
        .fee_sponsor_programs
        .get(&program_id)
        .cloned()
        .expect("sponsor program");
    program.staged_revision = Some(2);
    program.scheduled_activation = Some(iroha_data_model::nexus::FeeSponsorProgramActivation {
        revision: 2,
        activate_at_height: 10,
    });
    state_transaction
        .world
        .fee_sponsor_programs
        .insert(program_id.clone(), program);
    let current = resolve_fee_sponsor_program(
        &state_transaction.world,
        &state_transaction.nexus,
        (&program_id, 1),
        &beneficiary,
        transaction.payload(),
        (Some(DataSpaceId::UNIVERSAL), 10),
        fee_sponsor_multisig::RuleContext::admission(1),
    )
    .expect("old revision stays effective while its lease is live");
    assert_eq!(current.revision.revision, 1);
    let early_error = resolve_fee_sponsor_program(
        &state_transaction.world,
        &state_transaction.nexus,
        (&program_id, 2),
        &beneficiary,
        transaction.payload(),
        (Some(DataSpaceId::UNIVERSAL), 10),
        fee_sponsor_multisig::RuleContext::admission(1),
    )
    .expect_err("scheduled revision must not be predicted before old leases drain");
    let crate::execution_attempt::ExecutionAttemptError::Rejected(early_error) = early_error else {
        panic!("revision mismatch must be a completed rejection");
    };
    assert_eq!(early_error.code(), FeeRejectionCode::RevisionNotActive);
    let drained = resolve_fee_sponsor_program(
        &state_transaction.world,
        &state_transaction.nexus,
        (&program_id, 2),
        &beneficiary,
        transaction.payload(),
        (Some(DataSpaceId::UNIVERSAL), 21),
        fee_sponsor_multisig::RuleContext::admission(1),
    )
    .expect("scheduled revision becomes effective after the old lease expiry");
    assert_eq!(drained.revision.revision, 2);
}
#[test]
fn overlay_fee_uses_pre_effect_gas_policy_snapshot() {
    let (state, keypair, authority, initial_tech, updated_tech, gas_asset, alternate_asset) =
        pipeline_fee_state_fixture(state_after_genesis);
    let gas_asset_address = gas_asset.canonical_address();
    let alternate_asset_address = alternate_asset.canonical_address();
    let governed_rates = Json::from_str_norito(&format!(
            concat!(
                r#"[{{"asset":"{gas_asset}","liquidity_profile":"tier1","twap_local_per_xor":"1","units_per_gas":9,"volatility_class":"stable"}},"#,
                r#"{{"asset":"{alternate_asset}","liquidity_profile":"tier1","twap_local_per_xor":"1","units_per_gas":1,"volatility_class":"stable"}}]"#
            ),
            gas_asset = gas_asset_address,
            alternate_asset = alternate_asset_address,
        ))
        .expect("valid governed gas rates");
    let parameter_instruction = |id: &str, payload: Json| {
        InstructionBox::from(iroha_data_model::isi::SetParameter::new(
            iroha_data_model::parameter::Parameter::Custom(CustomParameter::new(
                id.parse().expect("governed gas parameter id"),
                payload,
            )),
        ))
    };
    let instructions = vec![
        parameter_instruction(
            "ivm_gas_tech_account_id",
            Json::new(updated_tech.to_string()),
        ),
        parameter_instruction(
            "ivm_gas_accepted_assets",
            Json::new(vec![gas_asset_address.clone(), alternate_asset_address]),
        ),
        parameter_instruction("ivm_gas_units_per_gas", governed_rates),
    ];
    let gas_used = isi_gas::meter_instructions(&instructions, &crate::gas::DEFAULT_ZK_GAS)
        .expect("completed fixture gas quote");
    assert!(gas_used > 0);
    let expected_fee = Quantity::from(u128::from(gas_used));
    let transaction = TransactionBuilder::new(
        state.network_id,
        authority.clone(),
        FeePaymentIntent::authority(
            vec![FeeChargeLimit::new(
                FeeChargeKind::PipelineGas,
                gas_asset.clone(),
                expected_fee.clone(),
            )],
            None,
        ),
    )
    .with_instructions(instructions.clone())
    .sign(keypair.private_key());
    let overlay = crate::pipeline::overlay::TxOverlay::from_instructions(instructions);
    let mut block = state.block(BlockHeader::new(
        nonzero!(2_u64),
        Some(
            state
                .view()
                .latest_block_hash()
                .expect("genuine genesis parent"),
        ),
        None,
        0,
        0,
    ));
    let mut state_transaction =
        block.transaction_for_fastpq_testing(Hash::from(transaction.hash_as_entrypoint()));
    configure_pipeline_fee_snapshot(&mut state_transaction, &initial_tech, &gas_asset, 1);
    state_transaction.current_tx_hash = Some(transaction.hash());
    validate_transaction_fee_admission(&mut state_transaction, &transaction)
        .expect("pre-effect policy accepts its exact signed limit");
    let (gas_used, instruction_count) =
        applied_overlay_base_gas(&transaction, &overlay, &state_transaction.zk.gas)
            .expect("meter the owned overlay before applying its effects");
    overlay
        .apply_with_chunk(&mut state_transaction, &authority, instruction_count)
        .expect("apply governed gas parameter effects");
    charge_fees_for_applied_overlay(
        &mut state_transaction,
        &authority,
        &transaction,
        gas_used,
        instruction_count,
    )
    .expect("settle against the pre-effect gas policy snapshot");
    assert_eq!(
        state_transaction.pipeline.gas.tech_account_id,
        initial_tech.to_string()
    );
    assert_eq!(
        state_transaction.pipeline.gas.accepted_assets,
        vec![gas_asset_address]
    );
    assert_eq!(
        state_transaction.pipeline.gas.units_per_gas[0].units_per_gas,
        1
    );
    assert_eq!(
        state_transaction
            .world
            .assets()
            .get(&AssetId::new(gas_asset.clone(), initial_tech))
            .expect("pre-effect fee destination balance")
            .as_ref(),
        &expected_fee
    );
    assert!(
        state_transaction
            .world
            .assets()
            .get(&AssetId::new(gas_asset.clone(), updated_tech))
            .is_none(),
        "the transaction must not redirect its own fee"
    );
    let expected_authority_balance = Quantity::from(1_000_000_u32)
        .checked_sub(&expected_fee)
        .expect("fixture balance covers fee");
    assert_eq!(
        state_transaction
            .world
            .assets()
            .get(&AssetId::new(gas_asset, authority))
            .expect("authority gas balance")
            .as_ref(),
        &expected_authority_balance
    );
}
#[test]
fn governed_gas_rate_owner_rejects_malformed_strings_without_partial_update() {
    for (label, twap, liquidity, volatility, expected_error) in [
        (
            "twap",
            "not-a-number",
            "tier1",
            "stable",
            "twap `not-a-number`",
        ),
        (
            "liquidity",
            "1",
            "not-a-tier",
            "stable",
            "liquidity `not-a-tier`",
        ),
        (
            "volatility",
            "1",
            "tier1",
            "not-a-class",
            "volatility `not-a-class`",
        ),
    ] {
        let (state, _, _, tech_account, _, gas_asset, _) =
            pipeline_fee_state_fixture(state_after_genesis);
        let gas_asset_address = gas_asset.canonical_address();
        let payload = Json::from_str_norito(&format!(
                concat!(
                    r#"[{{"asset":"{asset}","liquidity_profile":"{liquidity}","#,
                    r#""twap_local_per_xor":"{twap}","units_per_gas":9,"volatility_class":"{volatility}"}}]"#
                ),
                asset = gas_asset_address,
                twap = twap,
                liquidity = liquidity,
                volatility = volatility,
            ))
            .expect("well-formed gas-rate JSON");
        let parameter_id: CustomParameterId = "ivm_gas_units_per_gas"
            .parse()
            .expect("gas rate parameter id");
        let mut block = state.block(BlockHeader::new(
            nonzero!(2_u64),
            Some(
                state
                    .view()
                    .latest_block_hash()
                    .expect("genuine genesis parent"),
            ),
            None,
            0,
            0,
        ));
        let mut state_transaction = block.transaction();
        configure_pipeline_fee_snapshot(&mut state_transaction, &tech_account, &gas_asset, 1);
        let original = state_transaction.world.parameters.get().clone();
        let error = state_transaction
            .world
            .set_parameter_for_testing(iroha_data_model::parameter::Parameter::Custom(
                CustomParameter::new(parameter_id, payload),
            ))
            .expect_err("malformed governed rate must fail closed");
        assert!(
            error.to_string().contains(expected_error),
            "unexpected {label} parameter error: {error}"
        );
        assert_eq!(state_transaction.world.parameters.get(), &original);
        assert_eq!(
            state_transaction.pipeline.gas.tech_account_id,
            tech_account.to_string()
        );
        assert_eq!(
            state_transaction.pipeline.gas.accepted_assets,
            vec![gas_asset_address.clone()]
        );
        assert_eq!(state_transaction.pipeline.gas.units_per_gas.len(), 1);
        assert_eq!(
            state_transaction.pipeline.gas.units_per_gas[0].asset,
            gas_asset_address
        );
        assert_eq!(
            state_transaction.pipeline.gas.units_per_gas[0].units_per_gas,
            1
        );
    }
}
#[test]
fn pipeline_gas_asset_charge_is_disabled_when_nexus_gas_fee_is_active() {
    let mut nexus_fees = iroha_config::parameters::defaults::nexus::fees::genesis_policy();
    let gas_asset = Some("xor#universal".to_owned());
    nexus_fees.per_gas_unit_fee = Quantity::zero();
    assert!(should_charge_pipeline_gas_asset(
        false,
        &nexus_fees,
        gas_asset.as_deref()
    ));
    nexus_fees.per_gas_unit_fee = "0.001".parse().expect("valid gas fee");
    assert!(!should_charge_pipeline_gas_asset(
        false,
        &nexus_fees,
        gas_asset.as_deref()
    ));
    assert!(!should_charge_pipeline_gas_asset(
        true,
        &nexus_fees,
        gas_asset.as_deref()
    ));
    assert!(!should_charge_pipeline_gas_asset(false, &nexus_fees, None));
}
fn multi_component_fee_quote_fixture() -> (
    World,
    iroha_config::parameters::actual::Nexus,
    Pipeline,
    TransactionPayload,
) {
    let (authority, _) = gen_account_in("fee_quote");
    let (sink, _) = gen_account_in("fee_quote");
    let domain_id = DomainId::try_new("fee_quote", "universal").expect("fee quote domain");
    let nexus_asset = AssetDefinitionId::derive_from_components(
        domain_id.clone(),
        "nexus".parse().expect("nexus asset name"),
    );
    let gas_asset = AssetDefinitionId::derive_from_components(
        domain_id.clone(),
        "gas".parse().expect("gas asset name"),
    );
    let mut world = World::with_assets(
        [Domain::new(domain_id).build(&authority)],
        [
            Account::new(authority.clone()).build(&authority),
            Account::new(sink.clone()).build(&sink),
        ],
        [
            AssetDefinition::numeric(
                nexus_asset.clone(),
                "nexus".to_owned(),
                iroha_data_model::asset::AssetBalancePolicy::Global,
                None,
            )
            .build(&authority),
            AssetDefinition::numeric(
                gas_asset.clone(),
                "gas".to_owned(),
                iroha_data_model::asset::AssetBalancePolicy::Global,
                None,
            )
            .build(&authority),
        ],
        [
            Asset::new(
                AssetId::new(nexus_asset.clone(), authority.clone()),
                Quantity::from(1_000_000_u32),
            ),
            Asset::new(
                AssetId::new(gas_asset.clone(), authority.clone()),
                Quantity::from(1_000_000_u32),
            ),
        ],
        [],
    );
    seed_test_asset_supply(&mut world, &nexus_asset);
    seed_test_asset_supply(&mut world, &gas_asset);
    let mut nexus = iroha_config::parameters::actual::Nexus::default();
    nexus.fees.base_fee = Quantity::from(2_u32);
    nexus.fees.per_byte_fee = Quantity::zero();
    nexus.fees.per_instruction_fee = Quantity::zero();
    nexus.fees.per_gas_unit_fee = Quantity::zero();
    nexus.fees.fee_asset_id = nexus_asset.clone();
    let params = iroha_data_model::parameter::system::SumeragiNposParameters {
        xor_asset_definition_id: nexus_asset.clone(),
        ..Default::default()
    };
    let mut parameter_block = crate::state::parameter_cell_block_for_testing(&world)
        .expect("funded original parameter-only fixture block");
    crate::state::set_parameter_cell_for_testing(
        &mut parameter_block,
        crate::sumeragi::lanes::routing::test_support::metadata(
            iroha_data_model::block::consensus::SumeragiRootScope::Global,
        ),
        world.sccp_committee_generations.allocation_budget(),
    )
    .expect("valid fixture parameters");
    crate::state::set_parameter_cell_for_testing(
        &mut parameter_block,
        iroha_data_model::parameter::Parameter::Custom(params.into_custom_parameter()),
        world.sccp_committee_generations.allocation_budget(),
    )
    .expect("valid fixture parameters");
    parameter_block.commit();
    nexus.fees.fee_sink_account_id = sink.clone();
    let mut parameter_block = crate::state::parameter_cell_block_for_testing(&world)
        .expect("funded original parameter-only fixture block");
    crate::state::set_parameter_cell_for_testing(
        &mut parameter_block,
        iroha_data_model::parameter::Parameter::Custom(
            nexus.fees.clone().into_custom_parameter().unwrap(),
        ),
        world.sccp_committee_generations.allocation_budget(),
    )
    .unwrap();
    parameter_block.commit();
    let mut pipeline = Pipeline::default();
    pipeline.gas.accepted_assets = vec![gas_asset.canonical_address()];
    pipeline.gas.units_per_gas = vec![iroha_config::parameters::actual::GasRate {
        asset: gas_asset.canonical_address(),
        units_per_gas: 3,
        twap_local_per_xor: Numeric::from(1_u32),
        liquidity: GasLiquidity::Tier1,
        volatility: GasVolatility::Stable,
    }];
    let payload = TransactionBuilder::new(
        executor_test_network_id(b"fee-quote"),
        authority,
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([Log::new(Level::INFO, "quoted".to_owned())])
    .into_payload()
    .expect("canonical unsigned payload");
    (world, nexus, pipeline, payload)
}
fn author_component_fee_policy(
    world: &World,
    fees: &iroha_data_model::block::consensus::RootFeePolicy,
) {
    let mut parameters = crate::state::parameter_cell_block_for_testing(world)
        .expect("funded original parameter-only fixture block");
    crate::state::set_parameter_cell_for_testing(
        &mut parameters,
        iroha_data_model::parameter::Parameter::Custom(
            fees.clone()
                .into_custom_parameter()
                .expect("canonical fixture policy"),
        ),
        world.sccp_committee_generations.allocation_budget(),
    )
    .expect("committed component fee policy");
    parameters.commit();
}
fn assert_direct_fee_exempt_draft(
    world: &World,
    nexus: &iroha_config::parameters::actual::Nexus,
    pipeline: &Pipeline,
    mut payload: TransactionPayload,
) {
    let authority = payload.authority.clone();
    author_component_fee_policy(world, &nexus.fees);
    let world = world.block();
    let draft = quote_nexus_fee_admission_draft(
        &world,
        nexus,
        pipeline,
        &crate::gas::DEFAULT_ZK_GAS,
        &payload,
        0,
        None, /* World-only pre-genesis fee fixture */
        1,
        Some(DataSpaceId::UNIVERSAL),
    )
    .expect("fee-exempt draft must retain its zero direct fee plan");
    assert!(draft.quote.charges.is_empty());
    assert!(draft.quote.capacities.is_empty());
    assert!(draft.quote.authority_balances.is_empty());
    assert!(draft.quote.authority_charge_assets.is_empty());
    assert!(draft.quote.relay_leases.is_empty());
    assert_eq!(draft.quote.program_revision, None);
    assert_eq!(draft.quote.debit_source, FeeDebitSource::Account(authority));
    assert_eq!(
        draft.recommended_intent,
        FeePaymentIntent::authority(Vec::new(), None)
    );
    payload.fee_payment = draft.recommended_intent.clone();
    let strict = quote_nexus_fee_admission_payload(
        &world,
        nexus,
        pipeline,
        &crate::gas::DEFAULT_ZK_GAS,
        &payload,
        0,
        None, /* World-only pre-genesis fee fixture */
        1,
        Some(DataSpaceId::UNIVERSAL),
    )
    .expect("the recommended empty intent must pass strict quoting");
    assert_eq!(strict, draft.quote);
}
#[test]
fn protocol_fee_exempt_draft_returns_zero_quote_in_direct_mode() {
    let (world, nexus, pipeline, mut payload) = multi_component_fee_quote_fixture();
    let fee_asset = nexus.fees.fee_asset_id.clone();
    payload.fee_payment = FeePaymentIntent::authority(
        vec![FeeChargeLimit::new(
            FeeChargeKind::Nexus,
            fee_asset.clone(),
            Quantity::from(9_u32),
        )],
        None,
    );
    payload.instructions = vec![InstructionBox::from(
        iroha_data_model::isi::nexus::RegisterVerifiedFeeSponsorVaultAllocation {
            program_id: FeeSponsorProgramId::new(
                payload.authority.clone(),
                "quote_exempt".parse().expect("program name"),
            ),
            program_revision: 1,
            asset_definition_id: fee_asset,
            verified_allocation: Quantity::from(1_u32),
            source_dataspace_id: DataSpaceId::UNIVERSAL,
            source_height: 1,
            source_state_root: Hash::new(b"quote-source"),
            expires_at_height: 2,
            lease_id: Hash::new(b"quote-lease"),
            manifest_root: [1; 32],
            proof_blob: iroha_data_model::nexus::ProofBlob {
                payload: vec![1],
                expiry_slot: None,
            },
        },
    )]
    .into();
    assert_direct_fee_exempt_draft(&(world), &nexus, &pipeline, payload);
}
include!("executor_fee_quote_tests.rs");
#[test]
fn missing_committed_policy_rejects_quotes_before_signing() {
    let (world, nexus, pipeline, payload) = multi_component_fee_quote_fixture();
    let mut parameters = crate::state::parameter_cell_block_for_testing(&world)
        .expect("funded original parameter-only fixture block");
    // Deliberately malformed structural predecessor, not a permitted runtime transition.
    // Admit the exact authored projection in the original pool; only this Cell commits.
    let mut authored = parameters.get().get().clone();
    let id = RootFeePolicy::parameter_id();
    let original = authored.custom().get(&id).cloned();
    assert!(
        original.is_some(),
        "fixture starts with its committed fee policy"
    );
    assert_eq!(
        authored
            .remove_custom_parameter(&id)
            .expect("remove only the fee fixture policy"),
        original,
    );
    let retained = crate::pipeline_gas_policy::RetainedStateParameters::try_copy_for_testing(
        &authored,
        world.sccp_committee_generations.allocation_budget(),
    )
    .expect("funded malformed predecessor fixture");
    *parameters.get_mut() = retained;
    parameters.commit();
    let world = world.block();
    let error = quote_nexus_fee_admission_draft(
        &world,
        &nexus,
        &pipeline,
        &crate::gas::DEFAULT_ZK_GAS,
        &payload,
        0,
        None, /* World-only pre-genesis fee fixture */
        1,
        Some(DataSpaceId::UNIVERSAL),
    )
    .expect_err("quotes require an exact committed policy");
    assert!(
        matches!(crate::execution_attempt::expect_completed_rejection(error), NexusFeeAdmissionError::ConfigInvalid(reason)
            if reason.contains("committed root fee policy is absent"))
    );
}
#[test]
fn missing_committed_policy_rejects_execution_before_actual_fee_debit() {
    let authority = ALICE_ID.clone();
    let domain_id = DomainId::try_new("receipt_execution", "universal").expect("fee domain id");
    let fee_asset = AssetDefinitionId::derive_from_components(
        domain_id.clone(),
        "xor".parse().expect("fee asset name"),
    );
    let mut world = World::with_assets(
        [Domain::new(domain_id).build(&authority)],
        [Account::new(authority.clone()).build(&authority)],
        [AssetDefinition::numeric(
            fee_asset.clone(),
            "fee XOR".to_owned(),
            iroha_data_model::asset::AssetBalancePolicy::Global,
            None,
        )
        .build(&authority)],
        [Asset::new(
            AssetId::new(fee_asset.clone(), authority.clone()),
            Quantity::from(100_u32),
        )],
        [],
    );
    seed_test_asset_supply(&mut world, &fee_asset);
    let state = state_after_genesis(world);
    let transaction = TransactionBuilder::new(
        state.network_id,
        authority.clone(),
        FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([Log::new(Level::INFO, "receipt guard".to_owned())])
    .sign(ALICE_KEYPAIR.private_key());
    author_direct_fee_policy(&state, &fee_asset, Quantity::from(1_u32));
    let mut block = state.block(BlockHeader::new(
        nonzero!(2_u64),
        Some(
            state
                .view()
                .latest_block_hash()
                .expect("genuine genesis parent"),
        ),
        None,
        0,
        0,
    ));
    let mut state_tx = block.transaction();
    {
        // Deliberately malformed structural fixture; no production removal bypass.
        let mut parameters = state_tx.world.parameters.get().get().clone();
        let id = RootFeePolicy::parameter_id();
        let original = parameters.custom().get(&id).cloned();
        assert!(
            original.is_some(),
            "fixture starts with its committed fee policy"
        );
        assert_eq!(
            parameters
                .remove_custom_parameter(&id)
                .expect("remove only the fee fixture policy"),
            original,
        );
        let retained = crate::pipeline_gas_policy::RetainedStateParameters::try_copy_for_testing(
            &parameters,
            state_tx
                .world
                .sccp_committee_generations
                .allocation_budget(),
        )
        .expect("funded malformed predecessor fixture");
        *state_tx.world.parameters.get_mut() = retained;
    }
    install_fee_fixture_network_currency(&mut state_tx, &fee_asset);
    let error =
        super::Executor::charge_nexus_fees(&mut state_tx, &authority, &transaction, None, 1, 1, 0)
            .expect_err("execution must reject an absent committed policy");
    assert!(matches!(
        error,
        ValidationFail::InternalError(reason)
            if reason.contains("committed root fee policy is absent")
    ));
    assert_eq!(
        state_tx
            .world
            .assets()
            .get(&AssetId::new(fee_asset, authority))
            .expect("authority fee balance")
            .as_ref(),
        &Quantity::from(100_u32),
        "the rejected receipt path must not debit public XOR locally"
    );
}
#[test]
fn nexus_fee_charge_defensively_rejects_understated_authority_limit() {
    let (authority, keypair) = gen_account_in("nexus_actual_bound");
    let domain_id =
        DomainId::try_new("nexus_actual_bound", "universal").expect("Nexus fee domain id");
    let fee_asset = AssetDefinitionId::derive_from_components(
        domain_id.clone(),
        "xor".parse().expect("fee asset name"),
    );
    let mut world = World::with_assets(
        [Domain::new(domain_id).build(&authority)],
        [Account::new(authority.clone()).build(&authority)],
        [AssetDefinition::numeric(
            fee_asset.clone(),
            "fee XOR".to_owned(),
            iroha_data_model::asset::AssetBalancePolicy::Global,
            None,
        )
        .build(&authority)],
        [Asset::new(
            AssetId::new(fee_asset.clone(), authority.clone()),
            Quantity::from(100_u32),
        )],
        [],
    );
    seed_test_asset_supply(&mut world, &fee_asset);
    let state = state_for_testing(world);
    let transaction = TransactionBuilder::new(
        state.network_id,
        authority.clone(),
        FeePaymentIntent::authority(
            vec![FeeChargeLimit::new(
                FeeChargeKind::Nexus,
                fee_asset.clone(),
                Quantity::from(1_u32),
            )],
            None,
        ),
    )
    .with_instructions([Log::new(Level::INFO, "bounded Nexus fee".to_owned())])
    .sign(keypair.private_key());
    author_direct_fee_policy(&state, &fee_asset, Quantity::from(2_u32));
    let mut block = state.block(BlockHeader::new(nonzero!(2_u64), None, None, 0, 0));
    let mut state_transaction = block.transaction();
    install_fee_fixture_network_currency(&mut state_transaction, &fee_asset);
    let error = super::Executor::charge_nexus_fees(
        &mut state_transaction,
        &authority,
        &transaction,
        None,
        1,
        1,
        0,
    )
    .expect_err("actual Nexus charge must be checked against the signed limit");
    assert!(matches!(
        error,
        ValidationFail::NotPermitted(reason)
            if reason.contains("exceeds signed maximum")
    ));
    assert_eq!(
        state_transaction
            .world
            .assets()
            .get(&AssetId::new(fee_asset, authority))
            .expect("authority fee balance")
            .as_ref(),
        &Quantity::from(100_u32)
    );
}
#[test]
fn fee_quote_configuration_failure_has_stable_rejection_code() {
    let (world, nexus, mut pipeline, payload) = multi_component_fee_quote_fixture();
    let world = world.block();
    pipeline.gas.units_per_gas.clear();
    let err = quote_nexus_fee_admission_draft(
        &world,
        &nexus,
        &pipeline,
        &crate::gas::DEFAULT_ZK_GAS,
        &payload,
        0,
        None, /* World-only pre-genesis fee fixture */
        1,
        Some(DataSpaceId::UNIVERSAL),
    )
    .expect_err("missing conversion rate must reject the quote");
    assert_eq!(
        crate::execution_attempt::expect_completed_rejection(err).code(),
        FeeRejectionCode::InvalidProgramConfiguration
    );
}
#[test]
fn sponsor_capacity_rejects_dataspace_scoped_fee_assets() {
    let (sponsor, _) = gen_account_in("sponsor_scope");
    let domain_id = DomainId::try_new("sponsor_scope", "universal").expect("sponsor scope domain");
    let asset_definition_id = AssetDefinitionId::derive_from_components(
        domain_id.clone(),
        "fee".parse().expect("sponsor fee asset name"),
    );
    let world = World::with_assets(
        [Domain::new(domain_id.clone()).build(&sponsor)],
        [Account::new(sponsor.clone()).build(&sponsor)],
        [AssetDefinition::numeric(
            asset_definition_id.clone(),
            "scoped fee".to_owned(),
            AssetBalancePolicy::DataspaceRestricted,
            Some(domain_id),
        )
        .build(&sponsor)],
        [],
        [],
    );
    let program_id = FeeSponsorProgramId::new(
        sponsor.clone(),
        "scoped_fee".parse().expect("sponsor program name"),
    );
    let resolved = ResolvedSponsorProgram {
        id: program_id.clone(),
        revision: FeeSponsorProgramRevision {
            program_id,
            revision: 1,
            eligibility: FeeSponsorEligibility::EnrolledOnly,
            rules: Vec::new(),
            asset_budgets: vec![iroha_data_model::nexus::FeeSponsorAssetBudget {
                asset_definition_id: asset_definition_id.clone(),
                per_transaction: Quantity::from(10_u32),
                per_block: Quantity::from(10_u32),
                per_program_epoch: Quantity::from(10_u32),
                per_beneficiary_epoch: Quantity::from(10_u32),
                reserve_floor: Quantity::zero(),
                epoch_length_blocks: nonzero!(1_u64),
            }],
        },
    };
    let charge = FeeChargeBound {
        kind: FeeChargeKind::PipelineGas,
        asset_definition_id,
        max_bound: Quantity::from(1_u32),
    };
    let world = world.block();
    let error = evaluate_fee_sponsor_capacity(&world, &resolved, &sponsor, 1, &[charge])
        .expect_err("sponsor accounting must reject dataspace-scoped fee assets");
    assert_eq!(error.code(), FeeRejectionCode::InvalidProgramConfiguration);
    assert!(error.reason().contains("Global balance scope"));
}
fn relay_allocation_fixture(
    program_id: FeeSponsorProgramId,
    program_revision: u64,
    asset_definition_id: AssetDefinitionId,
    verified_allocation: Quantity,
    source_dataspace_id: DataSpaceId,
    expires_at_height: u64,
    lease_id: Hash,
) -> VerifiedFeeSponsorVaultAllocation {
    VerifiedFeeSponsorVaultAllocation::new(
        program_id,
        program_revision,
        asset_definition_id,
        verified_allocation,
        source_dataspace_id,
        2,
        Hash::new(b"relay-allocation-source-state"),
        expires_at_height,
        lease_id,
        Hash::new(b"relay-allocation-proof"),
        *Hash::new(b"relay-allocation-statement").as_ref(),
        Hash::new(b"relay-allocation-proof-digest"),
        3,
        *Hash::new(b"relay-allocation-manifest").as_ref(),
        iroha_data_model::nexus::AxtFastpqBinding {
            parameter: "fastpq-state-transition-stark-v1".to_owned(),
            source_dsid: source_dataspace_id.as_u64(),
            source_dataspace: format!("dataspace-{}", source_dataspace_id.as_u64()),
            source_receipt_id: "relay-allocation-receipt".to_owned(),
            source_tx_commitment: "aa".repeat(32),
            claim_type: "fee_sponsor_vault_allocation".to_owned(),
            claim_digest: "bb".repeat(32),
            witness_commitment: "cc".repeat(32),
            policy_commitment: "dd".repeat(32),
            verified_effect_type: "fee_sponsor_vault_allocation".to_owned(),
            corridor: "fee-sponsor".to_owned(),
            verifier_id: "fastpq".to_owned(),
            verifier_version: "v1".to_owned(),
            target_dsids: vec![DataSpaceId::UNIVERSAL.as_u64()],
            effect_binding: None,
            remote_spend_intent_commitments: Vec::new(),
        },
    )
}
fn insert_relay_allocation(world: &mut World, record: &VerifiedFeeSponsorVaultAllocation) {
    let key: StatePath = VerifiedFeeSponsorVaultAllocation::state_key_for(
        &record.program_id,
        &record.asset_definition_id,
        &record.lease_id,
    )
    .parse()
    .expect("canonical relay allocation key");
    let json = Json::try_new(record.clone()).expect("encode relay allocation JSON");
    let payload = norito::to_bytes(&json).expect("encode relay allocation state");
    world
        .smart_contract_state_mut_for_testing()
        .insert(key, payload);
}
#[test]
fn sponsor_relay_lease_selection_is_exact_canonical_and_capacity_aware() {
    let program_id = FeeSponsorProgramId::new(
        checked_account_id(),
        "relay-program".parse().expect("program name"),
    );
    let asset_definition_id = AssetDefinitionId::derive_from_components(
        DomainId::try_new("relay", "universal").expect("relay domain"),
        "xor".parse().expect("asset name"),
    );
    let dataspace_id = DataSpaceId::new(17);
    let lease_a = Hash::new(b"relay-lease-a");
    let lease_b = Hash::new(b"relay-lease-b");
    let matching_a = relay_allocation_fixture(
        program_id.clone(),
        4,
        asset_definition_id.clone(),
        Quantity::from(10_u32),
        dataspace_id,
        20,
        lease_a,
    );
    let matching_b = relay_allocation_fixture(
        program_id.clone(),
        4,
        asset_definition_id.clone(),
        Quantity::from(10_u32),
        dataspace_id,
        20,
        lease_b,
    );
    let wrong_revision = relay_allocation_fixture(
        program_id.clone(),
        3,
        asset_definition_id.clone(),
        Quantity::from(100_u32),
        dataspace_id,
        20,
        Hash::new(b"wrong-revision-lease"),
    );
    let mut world = World::default();
    insert_relay_allocation(&mut world, &matching_a);
    insert_relay_allocation(&mut world, &matching_b);
    insert_relay_allocation(&mut world, &wrong_revision);
    let selected_lease = if lease_a.as_ref() < lease_b.as_ref() {
        lease_a
    } else {
        lease_b
    };
    for lease_id in [lease_a, lease_b] {
        let executed_key: StatePath =
            VerifiedFeeSponsorVaultAllocation::usage_state_key_for(&lease_id)
                .parse()
                .expect("executed usage key");
        let settled_key: StatePath =
            VerifiedFeeSponsorVaultAllocation::settled_usage_state_key_for(&lease_id)
                .parse()
                .expect("settled usage key");
        world.smart_contract_state_mut_for_testing().insert(
            executed_key,
            norito::to_bytes(&Quantity::from(3_u32)).expect("executed usage"),
        );
        world.smart_contract_state_mut_for_testing().insert(
            settled_key,
            norito::to_bytes(&Quantity::from(6_u32)).expect("settled usage"),
        );
    }
    let world = world.block();
    let (selected, remaining) = select_fee_sponsor_relay_lease(
        &world,
        &program_id,
        4,
        &asset_definition_id,
        Some(dataspace_id),
        10,
        &Quantity::from(4_u32),
    )
    .expect("exact lease has four units remaining");
    assert_eq!(selected.lease_id, selected_lease);
    assert_eq!(remaining, Quantity::from(4_u32));
    let error = select_fee_sponsor_relay_lease(
        &world,
        &program_id,
        4,
        &asset_definition_id,
        Some(dataspace_id),
        10,
        &Quantity::from(5_u32),
    )
    .expect_err("settled usage must cap the proof-bound allocation");
    assert_eq!(error.code(), FeeRejectionCode::RelayCapacityUnavailable);
}
#[test]
fn sponsor_relay_lease_selection_aggregates_same_asset_and_selects_distinct_assets() {
    let program_id = FeeSponsorProgramId::new(
        checked_account_id(),
        "multi-asset-relay".parse().expect("program name"),
    );
    let domain_id = DomainId::try_new("relay_multi", "universal").expect("relay domain");
    let shared_asset = AssetDefinitionId::derive_from_components(
        domain_id.clone(),
        "shared".parse().expect("shared asset name"),
    );
    let distinct_asset = AssetDefinitionId::derive_from_components(
        domain_id,
        "distinct".parse().expect("distinct asset name"),
    );
    let dataspace_id = DataSpaceId::new(23);
    let shared_lease = Hash::new(b"shared-component-relay-lease");
    let distinct_lease = Hash::new(b"distinct-component-relay-lease");
    let mut world = World::default();
    insert_relay_allocation(
        &mut world,
        &relay_allocation_fixture(
            program_id.clone(),
            7,
            shared_asset.clone(),
            Quantity::from(10_u32),
            dataspace_id,
            20,
            shared_lease,
        ),
    );
    insert_relay_allocation(
        &mut world,
        &relay_allocation_fixture(
            program_id.clone(),
            7,
            distinct_asset.clone(),
            Quantity::from(8_u32),
            dataspace_id,
            20,
            distinct_lease,
        ),
    );
    let world = world.block();
    let same_asset_charges = [
        FeeChargeBound {
            kind: FeeChargeKind::Nexus,
            asset_definition_id: shared_asset.clone(),
            max_bound: Quantity::from(4_u32),
        },
        FeeChargeBound {
            kind: FeeChargeKind::PipelineGas,
            asset_definition_id: shared_asset.clone(),
            max_bound: Quantity::from(6_u32),
        },
    ];
    let same_asset_selection = select_fee_sponsor_relay_leases(
        &world,
        &program_id,
        7,
        Some(dataspace_id),
        10,
        &same_asset_charges,
    )
    .expect("aggregate lease capacity covers both same-asset components");
    assert_eq!(same_asset_selection.len(), 1);
    assert_eq!(same_asset_selection[&shared_asset].lease_id, shared_lease);
    assert_eq!(
        same_asset_selection[&shared_asset].remaining,
        Quantity::from(10_u32)
    );
    let distinct_asset_charges = [
        FeeChargeBound {
            kind: FeeChargeKind::Nexus,
            asset_definition_id: shared_asset.clone(),
            max_bound: Quantity::from(4_u32),
        },
        FeeChargeBound {
            kind: FeeChargeKind::PipelineGas,
            asset_definition_id: distinct_asset.clone(),
            max_bound: Quantity::from(7_u32),
        },
    ];
    let distinct_asset_selections = select_fee_sponsor_relay_leases(
        &world,
        &program_id,
        7,
        Some(dataspace_id),
        10,
        &distinct_asset_charges,
    )
    .expect("each distinct charged asset has its own lease capacity");
    assert_eq!(distinct_asset_selections.len(), 2);
    assert_eq!(
        distinct_asset_selections[&shared_asset].lease_id,
        shared_lease
    );
    assert_eq!(
        distinct_asset_selections[&distinct_asset].lease_id,
        distinct_lease
    );
    let over_capacity = [
        FeeChargeBound {
            kind: FeeChargeKind::Nexus,
            asset_definition_id: shared_asset.clone(),
            max_bound: Quantity::from(5_u32),
        },
        FeeChargeBound {
            kind: FeeChargeKind::PipelineGas,
            asset_definition_id: shared_asset,
            max_bound: Quantity::from(6_u32),
        },
    ];
    let error = select_fee_sponsor_relay_leases(
        &world,
        &program_id,
        7,
        Some(dataspace_id),
        10,
        &over_capacity,
    )
    .expect_err("same-asset components must be checked as one lease charge");
    assert_eq!(error.code(), FeeRejectionCode::RelayCapacityUnavailable);
}
#[test]
fn sponsor_relay_lease_selection_rejects_noncanonical_record_key() {
    let program_id = FeeSponsorProgramId::new(
        checked_account_id(),
        "relay-program".parse().expect("program name"),
    );
    let asset_definition_id = AssetDefinitionId::derive_from_components(
        DomainId::try_new("relay", "universal").expect("relay domain"),
        "xor".parse().expect("asset name"),
    );
    let record = relay_allocation_fixture(
        program_id.clone(),
        1,
        asset_definition_id.clone(),
        Quantity::from(10_u32),
        DataSpaceId::UNIVERSAL,
        20,
        Hash::new(b"noncanonical-relay-lease"),
    );
    let mut world = World::default();
    let key: StatePath =
        format!("{VERIFIED_FEE_SPONSOR_VAULT_ALLOCATION_STATE_KEY_PREFIX}_noncanonical")
            .parse()
            .expect("noncanonical fixture key");
    let json = Json::try_new(record).expect("encode relay allocation JSON");
    world.smart_contract_state_mut_for_testing().insert(
        key,
        norito::to_bytes(&json).expect("encode relay allocation state"),
    );
    let world = world.block();
    let error = select_fee_sponsor_relay_lease(
        &world,
        &program_id,
        1,
        &asset_definition_id,
        Some(DataSpaceId::UNIVERSAL),
        10,
        &Quantity::from(1_u32),
    )
    .expect_err("noncanonical allocation state must fail closed");
    assert_eq!(error.code(), FeeRejectionCode::InvalidProgramConfiguration);
}
#[test]
fn sponsor_relay_lease_consumption_tracks_direct_and_receipt_settlement() {
    let program_id = FeeSponsorProgramId::new(
        checked_account_id(),
        "relay-consumption".parse().expect("program name"),
    );
    let asset_definition_id = AssetDefinitionId::derive_from_components(
        DomainId::try_new("relay_consumption", "universal").expect("relay domain"),
        "xor".parse().expect("asset name"),
    );
    let lease_id = Hash::new(b"relay-consumption-lease");
    let mut world = World::default();
    insert_relay_allocation(
        &mut world,
        &relay_allocation_fixture(
            program_id.clone(),
            9,
            asset_definition_id.clone(),
            Quantity::from(10_u32),
            DataSpaceId::UNIVERSAL,
            20,
            lease_id,
        ),
    );
    let state = state_for_testing(world);
    let mut block = state.block(BlockHeader::new(nonzero!(10_u64), None, None, 0, 0));
    let mut state_transaction = block.transaction();
    state_transaction.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
    super::Executor::consume_fee_sponsor_relay_lease(
        &mut state_transaction,
        &program_id,
        9,
        &asset_definition_id,
        &Quantity::from(3_u32),
        true,
    )
    .expect("direct-settled PipelineGas consumes both usage counters");
    let executed_key =
        fee_sponsor_vault_allocation_usage_state_key(&lease_id).expect("executed usage key");
    let settled_key =
        fee_sponsor_vault_allocation_settled_usage_state_key(&lease_id).expect("settled usage key");
    assert_eq!(
        fee_sponsor_vault_allocation_quantity_at(&state_transaction.world, &executed_key)
            .expect("executed usage"),
        Quantity::from(3_u32)
    );
    assert_eq!(
        fee_sponsor_vault_allocation_quantity_at(&state_transaction.world, &settled_key)
            .expect("settled usage"),
        Quantity::from(3_u32)
    );
    super::Executor::consume_fee_sponsor_relay_lease(
        &mut state_transaction,
        &program_id,
        9,
        &asset_definition_id,
        &Quantity::from(2_u32),
        false,
    )
    .expect("receipt-settled Nexus charge advances executed usage only");
    assert_eq!(
        fee_sponsor_vault_allocation_quantity_at(&state_transaction.world, &executed_key)
            .expect("executed usage"),
        Quantity::from(5_u32)
    );
    assert_eq!(
        fee_sponsor_vault_allocation_quantity_at(&state_transaction.world, &settled_key)
            .expect("settled usage"),
        Quantity::from(3_u32)
    );
    super::Executor::consume_fee_sponsor_relay_lease(
        &mut state_transaction,
        &program_id,
        9,
        &asset_definition_id,
        &Quantity::from(1_u32),
        true,
    )
    .expect("a later direct charge must not mark the pending receipt as settled");
    assert_eq!(
        fee_sponsor_vault_allocation_quantity_at(&state_transaction.world, &executed_key)
            .expect("executed usage"),
        Quantity::from(6_u32)
    );
    assert_eq!(
        fee_sponsor_vault_allocation_quantity_at(&state_transaction.world, &settled_key)
            .expect("settled usage"),
        Quantity::from(4_u32)
    );
    let error = super::Executor::consume_fee_sponsor_relay_lease(
        &mut state_transaction,
        &program_id,
        9,
        &asset_definition_id,
        &Quantity::from(5_u32),
        true,
    )
    .expect_err("insufficient remaining capacity must reject before usage mutation");
    assert!(matches!(error, ValidationFail::NotPermitted(_)));
    assert_eq!(
        fee_sponsor_vault_allocation_quantity_at(&state_transaction.world, &executed_key)
            .expect("executed usage"),
        Quantity::from(6_u32)
    );
    assert_eq!(
        fee_sponsor_vault_allocation_quantity_at(&state_transaction.world, &settled_key)
            .expect("settled usage"),
        Quantity::from(4_u32)
    );
}
#[test]
fn detached_register_peer_forces_sequential_path() {
    let peer_id = make_peer_id();
    let isi = iroha_data_model::isi::register::RegisterPeerWithPop::new(peer_id, Vec::new());
    let mut delta = crate::state::DetachedStateTransactionDelta::default();
    let err = execute_instruction_detached(&alice(), &InstructionBox::from(isi), &mut delta)
        .expect_err("peer registration must be unsupported in detached mode");
    assert!(matches!(err, ValidationFail::InternalError(msg) if msg.contains("registration")));
}
#[test]
fn detached_register_committee_peer_forces_sequential_path() {
    let peer_id = make_peer_id();
    let isi =
        iroha_data_model::isi::register::RegisterCommitteePeerWithPop::new(peer_id, Vec::new());
    let mut delta = crate::state::DetachedStateTransactionDelta::default();
    let err = execute_instruction_detached(&alice(), &InstructionBox::from(isi), &mut delta)
        .expect_err("committee peer registration must be unsupported in detached mode");
    assert!(matches!(err, ValidationFail::InternalError(msg) if msg.contains("registration")));
}
#[test]
fn detached_unregister_peer_forces_sequential_path() {
    let peer_id = make_peer_id();
    let isi = iroha_data_model::isi::Unregister::peer(peer_id);
    let mut delta = crate::state::DetachedStateTransactionDelta::default();
    let err = execute_instruction_detached(&alice(), &InstructionBox::from(isi), &mut delta)
        .expect_err("peer removal must be unsupported in detached mode");
    assert!(matches!(err, ValidationFail::InternalError(msg) if msg.contains("removal")));
}
#[test]
fn detached_execute_trigger_forces_sequential_path() {
    let trigger_id = "detached_trigger".parse().expect("valid trigger id");
    let instruction = InstructionBox::from(iroha_data_model::isi::ExecuteTrigger::new(trigger_id));
    let mut delta = crate::state::DetachedStateTransactionDelta::default();
    let error = execute_instruction_detached(&alice(), &instruction, &mut delta)
        .expect_err("trigger execution needs the live executor and trigger state");
    assert!(matches!(error, ValidationFail::InternalError(message) if
            message.contains("live authorization") && message.contains("sequential")));
}
#[test]
fn detached_supply_changes_force_sequential_path() {
    let definition_id = iroha_data_model::asset::AssetDefinitionId::derive_from_components(
        iroha_model_base::domain::DomainId::try_new("wonderland", "universal")
            .expect("valid domain id"),
        "rose".parse().expect("valid asset name"),
    );
    let asset_id = iroha_data_model::asset::AssetId::new(definition_id, alice());
    let instructions = [
        InstructionBox::from(iroha_data_model::isi::Mint::asset_quantity(
            1_u32,
            asset_id.clone(),
        )),
        InstructionBox::from(iroha_data_model::isi::Burn::asset_quantity(1_u32, asset_id)),
    ];
    for instruction in instructions {
        let mut delta = crate::state::DetachedStateTransactionDelta::default();
        let error = execute_instruction_detached(&alice(), &instruction, &mut delta)
            .expect_err("supply changes need canonical sequential execution");
        assert!(matches!(error, ValidationFail::InternalError(message) if
                message.contains("live authorization") && message.contains("sequential")));
    }
}
#[test]
fn detached_transfers_keep_authorization_in_the_canonical_executor() {
    let authority = alice();
    let other = checked_account_id();
    let destination = checked_account_id();
    let domain_id = DomainId::try_new("wonderland", "universal").expect("valid domain identifier");
    let definition_id = AssetDefinitionId::derive_from_components(
        domain_id.clone(),
        "rose".parse().expect("valid asset name"),
    );
    let delegated_asset = InstructionBox::from(Transfer::asset_quantity(
        AssetId::new(definition_id.clone(), other.clone()),
        1_u32,
        destination.clone(),
    ));
    let domain = InstructionBox::from(Transfer::domain(
        authority.clone(),
        domain_id.clone(),
        destination.clone(),
    ));
    let asset_definition = InstructionBox::from(Transfer::asset_definition(
        authority.clone(),
        definition_id,
        destination.clone(),
    ));
    let nft = InstructionBox::from(Transfer::nft(
        authority.clone(),
        NftId::new(domain_id, "ticket".parse().expect("valid NFT name")),
        destination,
    ));
    for instruction in [delegated_asset, domain, asset_definition, nft] {
        let mut delta = crate::state::DetachedStateTransactionDelta::default();
        let error = execute_instruction_detached(&authority, &instruction, &mut delta)
            .expect_err("authorization-sensitive transfers must execute sequentially");
        assert!(matches!(error, ValidationFail::InternalError(message) if
                message.contains("sequential authorization")));
    }
    let owned_definition = AssetDefinitionId::derive_from_components(
        DomainId::try_new("wonderland", "universal").expect("valid domain identifier"),
        "owned".parse().expect("valid asset name"),
    );
    let owned_source = AssetId::new(owned_definition, authority.clone());
    let owned_transfer = InstructionBox::from(Transfer::asset_quantity(
        owned_source.clone(),
        1_u32,
        other.clone(),
    ));
    let mut delta = crate::state::DetachedStateTransactionDelta::default();
    execute_instruction_detached(&authority, &owned_transfer, &mut delta)
        .expect("source-owned transparent transfer may use canonical detached replay");
    assert_eq!(
        delta.single_transfer_delta(),
        Some((owned_source, other, Quantity::from(1_u32)))
    );
}
#[test]
fn detached_contract_deployment_permission_mutation_forces_sequential_path() {
    let authority = alice();
    let instruction: InstructionBox =
        Grant::account_permission(contract_deployment_permission(), authority.clone()).into();
    let mut delta = crate::state::DetachedStateTransactionDelta::default();
    let error = execute_instruction_detached(&authority, &instruction, &mut delta)
        .expect_err("deployment permission mutation must fall back to the consensus gate");
    assert!(matches!(error, ValidationFail::InternalError(message) if
            message.contains("CanManageSmartContractCode")
                && message.contains("sequential consensus gate")));
}
#[test]
fn detached_asset_instructions_cannot_be_constructed_with_negative_quantities() {
    let negative = Numeric::new(-1_i32, 0);
    assert!(Quantity::try_from_numeric(negative).is_err());
}
#[test]
fn detached_nft_metadata_forces_sequential_authorization() {
    let nft_id: NftId = "nft_detached$wonderland.universal".parse().expect("nft id");
    let key: Name = "meta".parse().expect("key");
    let set = SetKeyValue::nft(nft_id, key, "value");
    let mut delta = crate::state::DetachedStateTransactionDelta::default();
    let error = execute_instruction_detached(&alice(), &InstructionBox::from(set), &mut delta)
        .expect_err("metadata authorization requires the live sequential world");
    assert!(matches!(error, ValidationFail::InternalError(message) if
            message.contains("live authorization") && message.contains("sequential")));
}
use std::collections::{BTreeMap, BTreeSet};
#[allow(dead_code)]
fn encode_load(rd: u8, base: u8, imm12: u16, funct3: u8) -> u32 {
    let imm = u32::from(imm12 & 0x0fff);
    (imm << 20)
        | ((u32::from(base) & 0x1f) << 15)
        | ((u32::from(funct3) & 0x7) << 12)
        | ((u32::from(rd) & 0x1f) << 7)
        | 0x03
}
#[allow(dead_code)]
fn encode_store(base: u8, rs: u8, imm12: u16, funct3: u8) -> u32 {
    let imm = u32::from(imm12 & 0x0fff);
    let imm_hi = (imm >> 5) & 0x7f;
    let imm_lo = imm & 0x1f;
    (imm_hi << 25)
        | ((u32::from(rs) & 0x1f) << 20)
        | ((u32::from(base) & 0x1f) << 15)
        | ((u32::from(funct3) & 0x7) << 12)
        | (imm_lo << 7)
        | 0x23
}
#[cfg(all(feature = "zk-preverify", feature = "zk-stark"))]
fn preverify_stark_fixture() -> (
    String,
    iroha_data_model::proof::VerifyingKeyBox,
    iroha_data_model::proof::ProofBox,
) {
    let backend = "stark/fri/poseidon-x7-goldilocks-6x64-v1";
    let circuit_id = format!("{backend}:executor-preverify-v1");
    let key = crate::zk_stark::StarkFriVerifyingKeyV1 {
        version: 1,
        circuit_id: circuit_id.clone(),
        n_log2: crate::zk_stark::STARK_FRI_CONSENSUS_MIN_N_LOG2,
        blowup_log2: crate::zk_stark::STARK_FRI_CONSENSUS_MIN_BLOWUP_LOG2,
        fold_arity: 2,
        queries: crate::zk_stark::STARK_FRI_CONSENSUS_MIN_QUERIES,
        merkle_arity: 2,
    };
    let vk = iroha_data_model::proof::VerifyingKeyBox::new(
        backend.into(),
        norito::encode_canonical(&key).expect("canonical STARK verifier key"),
    );
    let proof = crate::zk::prove_stark_fri_open_verify_envelope(
        backend,
        &circuit_id,
        &vk,
        b"executor-preverify-v1",
        vec![vec![[0x11; 32]]],
    )
    .expect("native STARK preverify fixture");
    (circuit_id, vk, proof)
}
#[cfg(all(feature = "zk-preverify", feature = "zk-stark"))]
#[test]
fn preverify_and_dedup_across_transactions_in_block() {
    use iroha_data_model::{
        isi::verifying_keys::RegisterVerifyingKey,
        proof::{ProofAttachment, VerifyingKeyId, VerifyingKeyRecord},
        transaction::{Executable, TransactionBuilder},
        zk::BackendTag,
    };
    use iroha_executor_data_model::permission::governance::CanManageVerifyingKeys;
    use iroha_schema::Ident;
    use iroha_test_samples::{ALICE_ID, ALICE_KEYPAIR};
    let (_sink_id, _sink_kp) = gen_account_in("wonderland");
    let (_sponsor_id, _sponsor_kp) = gen_account_in("wonderland");
    let domain_id: DomainId = DomainId::try_new("wonderland", "universal").expect("domain id");
    let domain: Domain = Domain::new(domain_id.clone()).build(&ALICE_ID);
    let alice_account = Account::new(ALICE_ID.clone()).build(&ALICE_ID);
    let world = World::with([domain], [alice_account], []);
    let backend: Ident = "stark/fri/poseidon-x7-goldilocks-6x64-v1"
        .parse()
        .expect("backend ident");
    let (circuit_id, vk, proof) = preverify_stark_fixture();
    let vk_id = VerifyingKeyId::new(backend.clone(), "vk_preverify");
    let vk_commitment = crate::zk::hash_vk(&vk);
    let mut vk_record = VerifyingKeyRecord::new_with_owner(
        1,
        circuit_id,
        None,
        "test",
        BackendTag::Stark,
        "goldilocks",
        iroha_crypto::Hash::new(b"executor-preverify-v1").into(),
        vk_commitment,
    );
    vk_record.status = iroha_data_model::confidential::ConfidentialStatus::Active;
    vk_record.vk_len = u32::try_from(vk.bytes.len()).expect("fixture vk length fits");
    vk_record.max_proof_bytes = u32::try_from(proof.bytes.len()).expect("bounded proof size");
    vk_record.gas_schedule_id = Some("stark_default".to_owned());
    vk_record.key = Some(vk);
    let state = state_after_genesis_with_instructions(world, |genesis_account| {
        vec![
            Grant::account_permission(CanManageVerifyingKeys, genesis_account.clone()).into(),
            RegisterVerifyingKey {
                id: vk_id.clone(),
                record: vk_record,
            }
            .into(),
        ]
    });
    let block_header = BlockHeader::new(
        nonzero!(2_u64),
        Some(
            state
                .view()
                .latest_block_hash()
                .expect("genuine genesis parent"),
        ),
        None,
        0,
        0,
    );
    let mut block = state.block(block_header);
    let mut attachment = ProofAttachment::new_ref(backend, proof, vk_id);
    attachment.vk_commitment = Some(vk_commitment);
    let attachments = vec![attachment.clone()];
    let attachments_dup = vec![attachment];
    let tx1 = TransactionBuilder::new(
        state.network_id,
        ALICE_ID.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_executable(Executable::Instructions(Vec::new().into()))
    .with_proof_attachments(attachments)
    .expect("valid proof carriers")
    .sign(ALICE_KEYPAIR.private_key());
    let tx2 = TransactionBuilder::new(
        state.network_id,
        ALICE_ID.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_executable(Executable::Instructions(Vec::new().into()))
    .with_proof_attachments(attachments_dup)
    .expect("valid proof carriers")
    .sign(ALICE_KEYPAIR.private_key());
    let executor = super::Executor::Initial;
    let mut ivm_cache = crate::smartcontracts::ivm::cache::IvmCache::new();
    // First transaction preverify accepted
    {
        let mut state_tx =
            block.transaction_for_fastpq_testing(Hash::from(tx1.hash_as_entrypoint()));
        state_tx.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
        state_tx.world.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
        let original_wire = norito::encode_canonical(&tx1).unwrap();
        tx1.validate_signed_attachments_v1()
            .expect("registered STARK carrier has valid structure and bindings");
        tx1.verify_signature().expect("genuine signed carrier");
        super::Executor::preverify_attachments(&mut state_tx, &tx1).expect("preverify accepted");
        // A real registered native proof passing advisory preverification
        // still cannot authorize effects without the consuming relation.
        let error = executor
            .execute_transaction(&mut state_tx, &ALICE_ID.clone(), &tx1, &mut ivm_cache)
            .map_err(crate::execution_attempt::expect_completed_rejection)
            .expect_err("advisory preverification does not establish execution authority");
        let expected = format!(
                "malformed signed attachments: {}",
                iroha_data_model::transaction::signed_attachments::SignedAttachmentErrorV1::UnsupportedOperation { action_index: 0 }
            );
        assert!(matches!(error, ValidationFail::NotPermitted(message) if message == expected));
        assert_eq!(norito::encode_canonical(&tx1).unwrap(), original_wire);
    }
    // Second identical proof should be flagged as duplicate by per-block dedup
    {
        let mut state_tx =
            block.transaction_for_fastpq_testing(Hash::from(tx2.hash_as_entrypoint()));
        state_tx.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
        state_tx.world.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
        let res = super::Executor::preverify_attachments(&mut state_tx, &tx2);
        assert!(res.is_err(), "duplicate proof should be rejected");
    }
}
#[cfg(all(feature = "zk-preverify", feature = "zk-stark"))]
#[test]
fn preverify_attachments_enforce_verifying_key_height_window() {
    use iroha_data_model::{
        isi::verifying_keys::RegisterVerifyingKey,
        proof::{ProofAttachment, VerifyingKeyId, VerifyingKeyRecord},
        transaction::{Executable, TransactionBuilder},
        zk::BackendTag,
    };
    use iroha_executor_data_model::permission::governance::CanManageVerifyingKeys;
    use iroha_schema::Ident;
    use iroha_test_samples::{ALICE_ID, ALICE_KEYPAIR};
    fn execute_with_window(
        activation_height: Option<u64>,
        withdraw_height: Option<u64>,
        block_height: u64,
    ) -> Result<(), ValidationFail> {
        let domain_id: DomainId = DomainId::try_new("wonderland", "universal").expect("domain id");
        let domain: Domain = Domain::new(domain_id).build(&ALICE_ID);
        let alice_account = Account::new(ALICE_ID.clone()).build(&ALICE_ID);
        let world = World::with([domain], [alice_account], []);
        let backend: Ident = "stark/fri/poseidon-x7-goldilocks-6x64-v1"
            .parse()
            .expect("backend ident");
        let (circuit_id, vk, proof) = preverify_stark_fixture();
        let vk_id = VerifyingKeyId::new(backend.clone(), "vk_height_window");
        let vk_commitment = crate::zk::hash_vk(&vk);
        let mut vk_record = VerifyingKeyRecord::new_with_owner(
            1,
            circuit_id,
            None,
            "test",
            BackendTag::Stark,
            "goldilocks",
            iroha_crypto::Hash::new(b"executor-preverify-v1").into(),
            vk_commitment,
        );
        vk_record.status = iroha_data_model::confidential::ConfidentialStatus::Active;
        vk_record.activation_height = activation_height;
        vk_record.withdraw_height = withdraw_height;
        vk_record.vk_len = u32::try_from(vk.bytes.len()).expect("fixture vk length fits");
        vk_record.max_proof_bytes = u32::try_from(proof.bytes.len()).expect("bounded proof size");
        vk_record.gas_schedule_id = Some("stark_default".to_owned());
        vk_record.key = Some(vk);
        let state = state_after_genesis_with_instructions(world, |genesis_account| {
            vec![
                Grant::account_permission(CanManageVerifyingKeys, genesis_account.clone()).into(),
                RegisterVerifyingKey {
                    id: vk_id.clone(),
                    record: vk_record,
                }
                .into(),
            ]
        });
        let mut attachment = ProofAttachment::new_ref(backend, proof, vk_id);
        attachment.vk_commitment = Some(vk_commitment);
        let tx = TransactionBuilder::new(
            state.network_id,
            ALICE_ID.clone(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_executable(Executable::Instructions(Vec::new().into()))
        .with_proof_attachments(vec![attachment])
        .expect("valid proof carriers")
        .sign(ALICE_KEYPAIR.private_key());
        let block_header = BlockHeader::new(
            std::num::NonZeroU64::new(block_height).expect("nonzero block height"),
            Some(
                state
                    .view()
                    .latest_block_hash()
                    .expect("genuine genesis parent"),
            ),
            None,
            0,
            0,
        );
        let mut block = state.block(block_header);
        let mut state_tx =
            block.transaction_for_fastpq_testing(Hash::from(tx.hash_as_entrypoint()));
        state_tx.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
        state_tx.world.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
        super::Executor::preverify_attachments(&mut state_tx, &tx)
    }
    for (label, activation_height, withdraw_height, block_height) in [
        ("future", Some(3), None, 2),
        ("withdrawn", Some(1), Some(2), 2),
        ("expired", Some(2), Some(3), 3),
    ] {
        let err = execute_with_window(activation_height, withdraw_height, block_height)
            .expect_err("out-of-window verifying key must reject");
        match err {
            ValidationFail::NotPermitted(msg) => assert!(
                msg.contains("verifying key inactive"),
                "case {label}: unexpected error: {msg}"
            ),
            other => panic!("case {label}: unexpected error: {other:?}"),
        }
    }
    execute_with_window(Some(2), Some(3), 2)
        .expect("in-window active verifying key must preverify");
}
#[cfg(feature = "zk-preverify")]
#[test]
fn preverify_attachment_original_reader_refusal_stays_sticky_and_retry_is_semantic() {
    use iroha_data_model::proof::{ProofAttachment, ProofBox, VerifyingKeyId};
    let state = state_after_genesis(World::with(
        [],
        [Account::new(ALICE_ID.clone()).build(&ALICE_ID)],
        [],
    ));
    let backend = "halo2/unknown-native-v1";
    let source = TransactionBuilder::new(
        state.network_id,
        ALICE_ID.clone(),
        FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([Log::new(Level::INFO, "preverify original reader".into())])
    .with_proof_attachments([ProofAttachment::new_ref(
        backend.into(),
        ProofBox::new(backend.into(), vec![1, 2, 3]),
        VerifyingKeyId::new(backend, "original_preverify"),
    )])
    .expect("valid carrier, deliberately unsupported backend")
    .sign(ALICE_KEYPAIR.private_key());
    let wire = norito::encode_canonical(&source).unwrap();
    let header = BlockHeader::new(
        std::num::NonZeroU64::new(2).unwrap(),
        state.view().latest_block_hash(),
        None,
        0,
        0,
    );
    let mut block = state.block(header);
    {
        let mut transaction =
            block.transaction_for_fastpq_testing(Hash::from(source.hash_as_entrypoint()));
        let limits = norito::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, 0, usize::MAX);
        norito::with_decode_limits_scope(limits, || {
            super::Executor::preverify_attachments(&mut transaction, &source)
        })
        .expect_err("actual carrier decode is locally refused");
        let observed = transaction
            .borrow_execution_deferral()
            .expect("sticky original")
            .decode_refusal()
            .expect("real codec owner")
            .clone();
        assert_eq!(
            observed.kind(),
            norito::core::DecodeLocalRefusalKind::EnclosingLimit
        );
        assert!(
            matches!(observed.resource_error(), norito::core::DecodeResourceError::TotalAllocationExceeded { attempted, limit: 0 } if attempted > 0)
        );
        let retained = transaction
            .take_execution_deferral()
            .expect("single export");
        assert_eq!(retained.decode_refusal(), Some(&observed));
        assert!(retained.allocation_refusal().is_none());
        assert!(transaction.take_execution_deferral().is_none());
        assert!(
            transaction.is_execution_deferred(),
            "taking ownership cannot clear publication refusal"
        );
        assert!(transaction.ensure_execution_ready().is_err());
    }
    let mut retry = block.transaction_for_fastpq_testing(Hash::from(source.hash_as_entrypoint()));
    let completed = super::Executor::preverify_attachments(&mut retry, &source).unwrap_err();
    assert!(
        matches!(completed, ValidationFail::NotPermitted(reason) if reason == "unsupported proof backends are not supported")
    );
    assert!(!retry.is_execution_deferred());
    assert_eq!(norito::encode_canonical(&source).unwrap(), wire);
}

#[cfg(feature = "zk-preverify")]
#[test]
fn preverify_attachments_reject_non_production_backend_labels_before_vk_lookup() {
    use iroha_data_model::{
        proof::{ProofAttachment, ProofBox, VerifyingKeyId},
        transaction::{Executable, TransactionBuilder},
    };
    use iroha_schema::Ident;
    use iroha_test_samples::{ALICE_ID, ALICE_KEYPAIR};
    let domain_id: DomainId = DomainId::try_new("wonderland", "universal").expect("domain id");
    let domain: Domain = Domain::new(domain_id).build(&ALICE_ID);
    let alice_account = Account::new(ALICE_ID.clone()).build(&ALICE_ID);
    let world = World::with([domain], [alice_account], []);
    let state = state_after_genesis(world);
    let block_header = BlockHeader::new(
        nonzero!(2_u64),
        Some(
            state
                .view()
                .latest_block_hash()
                .expect("genuine genesis parent"),
        ),
        None,
        0,
        0,
    );
    let mut block = state.block(block_header);
    for (idx, (backend, expected_msg)) in [
        (
            "halo2/ipa:production-ready",
            "readiness-claim proof backends",
        ),
        ("halo2/ipa:kzg", "trusted-setup proof backends"),
        ("halo2/ipa:dev-fixture", "developer-only proof backends"),
        ("halo2/unknown-native-v1", "unsupported proof backends"),
    ]
    .into_iter()
    .enumerate()
    {
        let backend_ident: Ident = backend.parse().expect("backend ident");
        let proof = ProofBox::new(
            backend_ident.clone(),
            vec![0xA0 | u8::try_from(idx).unwrap()],
        );
        let attachment = ProofAttachment::new_ref(
            backend_ident.clone(),
            proof,
            VerifyingKeyId::new(backend_ident, format!("missing_vk_{idx}")),
        );
        let tx = TransactionBuilder::new(
            state.network_id,
            ALICE_ID.clone(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_executable(Executable::Instructions(Vec::new().into()))
        .with_proof_attachments(vec![attachment])
        .expect("valid proof carriers")
        .sign(ALICE_KEYPAIR.private_key());
        let mut state_tx =
            block.transaction_for_fastpq_testing(Hash::from(tx.hash_as_entrypoint()));
        state_tx.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
        state_tx.world.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
        let err = super::Executor::preverify_attachments(&mut state_tx, &tx)
            .expect_err("non-production proof backend label must fail before vk lookup");
        match err {
            ValidationFail::NotPermitted(msg) => {
                assert!(
                    msg.contains(expected_msg),
                    "unexpected msg for {backend}: {msg}"
                );
                assert!(
                    !msg.contains("referenced verifying key missing"),
                    "backend classification for {backend} must precede vk lookup: {msg}"
                );
            }
            other => panic!("unexpected error for {backend}: {other:?}"),
        }
    }
}
#[cfg(feature = "zk-preverify")]
#[test]
fn preverify_attachments_reject_malformed_attachment_shapes_before_vk_lookup() {
    use iroha_data_model::{
        proof::{ProofAttachment, ProofBox, VerifyingKeyId},
        transaction::{Executable, TransactionBuilder},
    };
    use iroha_test_samples::{ALICE_ID, ALICE_KEYPAIR};
    let domain_id: DomainId = DomainId::try_new("wonderland", "universal").expect("domain id");
    let domain: Domain = Domain::new(domain_id).build(&ALICE_ID);
    let alice_account = Account::new(ALICE_ID.clone()).build(&ALICE_ID);
    let world = World::with([domain], [alice_account], []);
    let state = state_after_genesis(world);
    let block_header = BlockHeader::new(
        nonzero!(2_u64),
        Some(
            state
                .view()
                .latest_block_hash()
                .expect("genuine genesis parent"),
        ),
        None,
        0,
        0,
    );
    let mut block = state.block(block_header);
    let mut zero_vk_commitment = ProofAttachment::new_ref(
        "halo2/ipa".into(),
        ProofBox::new("halo2/ipa".into(), vec![1, 2, 3]),
        VerifyingKeyId::new("halo2/ipa", "vk_preverify"),
    );
    zero_vk_commitment.vk_commitment = Some([0u8; 32]);
    let mut zero_envelope_hash = ProofAttachment::new_ref(
        "halo2/ipa".into(),
        ProofBox::new("halo2/ipa".into(), vec![1, 2, 3]),
        VerifyingKeyId::new("halo2/ipa", "vk_preverify"),
    );
    zero_envelope_hash.envelope_hash = Some([0u8; 32]);
    let mut forged_envelope_hash = ProofAttachment::new_ref(
        "halo2/ipa".into(),
        ProofBox::new("halo2/ipa".into(), vec![1, 2, 3]),
        VerifyingKeyId::new("halo2/ipa", "vk_preverify"),
    );
    let mut forged_hash: [u8; 32] =
        iroha_crypto::Hash::new(&forged_envelope_hash.proof.bytes).into();
    forged_hash[0] ^= 0x80;
    forged_envelope_hash.envelope_hash = Some(forged_hash);
    let unvalidated_proof_carriers = |attachments: &[ProofAttachment]| {
        use iroha_data_model::transaction::signed_attachments::{
            PlannedActionV1, SignedAttachmentPlanV1,
        };
        let mut plan = SignedAttachmentPlanV1::new();
        for attachment in attachments {
            plan.push_action(
                PlannedActionV1::transaction_proof_unvalidated(attachment)
                    .expect("canonical wrapper"),
            );
        }
        plan.declare()
    };
    let cases = [
        (
            "proof-backend-mismatch",
            vec![ProofAttachment::new_ref(
                "halo2/ipa".into(),
                ProofBox::new("stark/fri".into(), vec![1, 2, 3]),
                VerifyingKeyId::new("halo2/ipa", "vk_preverify"),
            )],
            "proof.backend",
        ),
        (
            "nonportable-vk-ref-name",
            vec![ProofAttachment::new_ref(
                "halo2/ipa".into(),
                ProofBox::new("halo2/ipa".into(), vec![1, 2, 3]),
                VerifyingKeyId::new("halo2/ipa", "VkPreverify"),
            )],
            "vk_ref",
        ),
        (
            "empty-proof-bytes",
            vec![ProofAttachment::new_ref(
                "halo2/ipa".into(),
                ProofBox::new("halo2/ipa".into(), Vec::new()),
                VerifyingKeyId::new("halo2/ipa", "vk_preverify"),
            )],
            "proof.bytes",
        ),
        (
            "zero-vk-commitment",
            vec![zero_vk_commitment],
            "vk_commitment",
        ),
        (
            "zero-envelope-hash",
            vec![zero_envelope_hash],
            "envelope_hash",
        ),
        (
            "forged-envelope-hash",
            vec![forged_envelope_hash],
            "envelope_hash",
        ),
    ];
    for (label, attachments, expected_msg) in cases {
        let tx = TransactionBuilder::new(
            state.network_id,
            ALICE_ID.clone(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_executable(Executable::Instructions(Vec::new().into()))
        .with_signed_attachments(unvalidated_proof_carriers(&attachments))
        .sign(ALICE_KEYPAIR.private_key());
        let mut state_tx =
            block.transaction_for_fastpq_testing(Hash::from(tx.hash_as_entrypoint()));
        state_tx.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
        state_tx.world.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
        let err = super::Executor::preverify_attachments(&mut state_tx, &tx)
            .expect_err("malformed proof attachment must fail before vk lookup");
        match err {
            ValidationFail::NotPermitted(msg) => {
                assert!(
                    msg.contains(expected_msg),
                    "case {label}: expected {expected_msg:?} in error message: {msg}"
                );
                assert!(
                    !msg.contains("referenced verifying key missing"),
                    "case {label}: malformed attachment shape must reject before vk lookup: {msg}"
                );
            }
            other => panic!("case {label}: unexpected error: {other:?}"),
        }
    }
}
#[test]
fn initial_executor_denies_asset_definition_without_permission() {
    let alice_id = ALICE_ID.clone();
    let genesis_id = SAMPLE_GENESIS_ACCOUNT_ID.clone();
    let domain_id: DomainId = DomainId::try_new("wonderland", "universal").expect("domain id");
    let domain: Domain = Domain::new(domain_id.clone()).build(&genesis_id);
    let alice_account = Account::new(alice_id.clone()).build(&alice_id);
    let genesis_account = Account::new(genesis_id.clone()).build(&genesis_id);
    let world = World::with([domain], [alice_account, genesis_account], []);
    let kura = Kura::blank_kura_for_testing();
    let query_handle = query::store::LiveQueryStore::start_test();
    let state = State::new(world, kura, query_handle);
    state
        .block(BlockHeader::new(nonzero!(1_u64), None, None, 0, 0))
        .commit_empty_block_for_testing()
        .expect("commit bootstrap block");
    let header = BlockHeader::new(nonzero!(2_u64), None, None, 0, 0);
    let mut block = state.block(header);
    {
        let mut stx = block.transaction();
        Transfer::domain(genesis_id.clone(), domain_id.clone(), alice_id.clone())
            .execute(&genesis_id, &mut stx)
            .expect("domain transfer to succeed");
        stx.apply();
    }
    let executor = super::Executor::Initial;
    let asset_definition_id: AssetDefinitionId =
        iroha_data_model::asset::AssetDefinitionId::derive_from_components(
            DomainId::try_new("wonderland", "universal").unwrap(),
            "invalid".parse().unwrap(),
        );
    let instruction = InstructionBox::from(Register::asset_definition({
        let __asset_definition_id = asset_definition_id;
        AssetDefinition::numeric(
            __asset_definition_id.clone(),
            "invalid".to_owned(),
            iroha_data_model::asset::AssetBalancePolicy::Global,
            None,
        )
    }));
    let mut stx = block.transaction();
    let res = executor.execute_instruction(&mut stx, &genesis_id, instruction);
    assert!(
        matches!(res, Err(ValidationFail::NotPermitted(_))),
        "initial executor should deny registering asset definition without permission"
    );
}
#[test]
fn borrowed_overlay_apply_matches_owned_initial_executor_for_register_domain() {
    let executor = super::Executor::Initial;
    let domain_id: DomainId =
        DomainId::try_new("borrowed-overlay", "universal").expect("domain id");
    let owned = prepared_native_domain_genesis_fixture(&domain_id);
    let source = owned.genesis.block();
    let owned_instruction: InstructionBox = Register::domain(Domain::new(domain_id.clone())).into();
    let index = exact_native_genesis_fixture_index(source, &owned_instruction);
    let mut owned_block = owned
        .state
        .block_with_pristine_carrier_stage(source, |_| Ok::<_, String>(()))
        .expect("capture exact owned native genesis carrier");
    let mut owned_tx = owned_block
        .transaction_for_original_genesis_testing(source, index, &ALICE_ID, &owned_instruction)
        .expect("authenticate actual owned RegisterDomain input");
    executor
        .execute_instruction(&mut owned_tx, &ALICE_ID.clone(), owned_instruction)
        .expect("owned initial executor applies instruction");
    assert!(owned_tx.world.domains.get(&domain_id).is_some());
    let overlay_state = prepared_native_domain_genesis_fixture(&domain_id);
    let source = overlay_state.genesis.block();
    let overlay_instruction: InstructionBox =
        Register::domain(Domain::new(domain_id.clone())).into();
    let index = exact_native_genesis_fixture_index(source, &overlay_instruction);
    let mut overlay_block = overlay_state
        .state
        .block_with_pristine_carrier_stage(source, |_| Ok::<_, String>(()))
        .expect("capture exact borrowed native genesis carrier");
    let mut overlay_tx = overlay_block
        .transaction_for_original_genesis_testing(source, index, &ALICE_ID, &overlay_instruction)
        .expect("authenticate actual borrowed RegisterDomain input");
    let overlay = crate::pipeline::overlay::TxOverlay::from_instructions(vec![overlay_instruction]);
    overlay
        .apply_with_chunk(&mut overlay_tx, &ALICE_ID.clone(), 1)
        .expect("borrowed overlay applies instruction");
    assert!(overlay_tx.world.domains.get(&domain_id).is_some());
}
#[test]
fn borrowed_overlay_apply_matches_owned_initial_executor_for_register_account() {
    let account = checked_account_id();
    let instruction: InstructionBox = Register::account(Account::new(account.clone())).into();
    let mut outcomes = Vec::new();
    for borrowed in [false, true] {
        let state = state_after_genesis(World::with(
            [],
            [Account::new(ALICE_ID.clone()).build(&ALICE_ID)],
            [],
        ));
        let mut block = state.block(BlockHeader::new(
            nonzero!(2_u64),
            state.view().latest_block_hash(),
            None,
            0,
            0,
        ));
        let mut tx =
            block.transaction_for_fastpq_testing(Hash::new(b"owned-borrowed account registration"));
        if borrowed {
            let overlay =
                crate::pipeline::overlay::TxOverlay::from_instructions(vec![instruction.clone()]);
            overlay
                .apply_with_chunk(&mut tx, &ALICE_ID, 1)
                .expect("borrowed account registration");
        } else {
            super::Executor::Initial
                .execute_instruction(&mut tx, &ALICE_ID, instruction.clone())
                .expect("owned account registration");
        }
        assert!(tx.world.accounts.get(&account).is_some());
        let account_value = tx.world.accounts.get(&account).unwrap().clone();
        tx.apply();
        outcomes.push((account_value, block.world.take_external_events()));
        assert_eq!(block.committed_fragment_count(), 1);
        assert!(block.world.accounts.get(&account).is_some());
    }
    assert_eq!(outcomes[0], outcomes[1]);
}
#[test]
fn original_native_genesis_requires_source_capability_for_owned_and_borrowed_execution() {
    let world = || World::with([], [Account::new(ALICE_ID.clone()).build(&ALICE_ID)], []);
    let domain_id = DomainId::try_new("borrowed-overlay", "universal").unwrap();
    let instruction: InstructionBox = Register::domain(Domain::new(domain_id.clone())).into();
    // Height-only component overlays cannot execute genesis through either API.
    for borrowed in [false, true] {
        let state = component_state_for_testing(world());
        let mut block = state.block(BlockHeader::new(nonzero!(1_u64), None, None, 0, 0));
        let mut tx = block.transaction();
        let error = if borrowed {
            super::Executor::Initial.execute_borrowed_overlay_instruction(
                &mut tx,
                &ALICE_ID,
                &instruction,
                None,
            )
        } else {
            super::Executor::Initial.execute_instruction(&mut tx, &ALICE_ID, instruction.clone())
        }
        .expect_err("missing original genesis capability rejects");
        assert!(
            matches!(&error, ValidationFail::NotPermitted(reason)
                if reason == "genesis instruction execution requires its authenticated source capability"),
            "{error:?}"
        );
        assert!(tx.world.domains.get(&domain_id).is_none());
        drop(tx);
        assert_eq!(block.committed_fragment_count(), 0);
    }
}
fn prepared_native_domain_genesis_fixture(
    domain_id: &DomainId,
) -> crate::sumeragi::test_chain::PreparedTestChainConfig {
    let wonderland = DomainId::try_new("wonderland", "universal").expect("domain id");
    let world = World::with(
        [Domain::new(wonderland).build(&ALICE_ID)],
        [Account::new(ALICE_ID.clone()).build(&ALICE_ID)],
        [],
    );
    let mut config = crate::sumeragi::test_chain::TestChainConfig::new(world, 0);
    config.genesis_key = ALICE_KEYPAIR.clone();
    config
        .genesis_instructions
        .push(Register::domain(Domain::new(domain_id.clone())).into());
    crate::sumeragi::test_chain::CertifiedTestChain::prepare(config)
        .expect("prepare genuinely validated native domain genesis")
}

fn exact_native_genesis_fixture_index(
    source: &iroha_data_model::block::SignedBlock,
    instruction: &InstructionBox,
) -> usize {
    source.external_transactions().position(|signed| matches!(signed.instructions(),
            Executable::Instructions(instructions) if instructions.iter().eq(core::iter::once(instruction))
        )).expect("original signed input contains exactly the component instruction")
}

#[test]
fn original_native_genesis_component_refuses_source_and_instruction_substitution() {
    let domain_id = DomainId::try_new("source-custody", "universal").unwrap();
    let prepared = prepared_native_domain_genesis_fixture(&domain_id);
    let source = prepared.genesis.block();
    let instruction: InstructionBox = Register::domain(Domain::new(domain_id)).into();
    let index = exact_native_genesis_fixture_index(source, &instruction);
    // Each phase retains one original acquisition in its own debug stack frame
    // while checking the same signed sources and refusal assertions.
    assert_native_genesis_absent_original_source(&prepared, &instruction, index);
    assert_native_genesis_component_identity_refusals(&prepared, &instruction, index);
    assert_native_genesis_foreign_signed_source(&prepared);
    assert_native_genesis_foreign_state_network(&prepared, &instruction, index);
    assert_native_genesis_successor_header(&prepared, &instruction, index);
    assert_native_genesis_committed_bootstrap_refusal(&prepared, &instruction, index);
}

#[inline(never)]
fn assert_native_genesis_absent_original_source(
    prepared: &crate::sumeragi::test_chain::PreparedTestChainConfig,
    instruction: &InstructionBox,
    index: usize,
) {
    let source = prepared.genesis.block();
    let mut absent = prepared.state.block(source.header());
    assert!(
        absent
            .transaction_for_original_genesis_testing(source, index, &ALICE_ID, instruction)
            .is_err(),
        "H1 without retained original route must refuse"
    );
    drop(absent);
}

#[inline(never)]
fn assert_native_genesis_component_identity_refusals(
    prepared: &crate::sumeragi::test_chain::PreparedTestChainConfig,
    instruction: &InstructionBox,
    index: usize,
) {
    let source = prepared.genesis.block();
    let mut block = prepared
        .state
        .block_with_pristine_carrier_stage(source, |_| Ok::<_, String>(()))
        .unwrap();
    assert!(
        block
            .transaction_for_original_genesis_testing(source, index, &BOB_ID, instruction)
            .is_err(),
        "configured signer mismatch must refuse"
    );
    assert!(
        block
            .transaction_for_original_genesis_testing(
                source,
                source.network_entrypoint_count(),
                &ALICE_ID,
                instruction
            )
            .is_err(),
        "absent original ordinal must refuse"
    );
    assert!(
        block
            .transaction_for_original_genesis_testing(source, 0, &ALICE_ID, instruction)
            .is_err(),
        "a different valid original ordinal must not authenticate the tested instruction"
    );
    let substituted: InstructionBox =
        Log::new(Level::INFO, "substituted component effect".to_owned()).into();
    assert!(
        block
            .transaction_for_original_genesis_testing(source, index, &ALICE_ID, &substituted)
            .is_err(),
        "authentic RegisterDomain must not bless a substituted Log"
    );
    drop(block);
}

#[inline(never)]
fn assert_native_genesis_foreign_signed_source(
    prepared: &crate::sumeragi::test_chain::PreparedTestChainConfig,
) {
    let source = prepared.genesis.block();
    let other_domain = DomainId::try_new("foreign-source", "universal").unwrap();
    let foreign = prepared_native_domain_genesis_fixture(&other_domain);
    let foreign_instruction: InstructionBox = Register::domain(Domain::new(other_domain)).into();
    let foreign_index =
        exact_native_genesis_fixture_index(foreign.genesis.block(), &foreign_instruction);
    let mut block = prepared
        .state
        .block_with_pristine_carrier_stage(source, |_| Ok::<_, String>(()))
        .unwrap();
    assert!(
        block
            .transaction_for_original_genesis_testing(
                foreign.genesis.block(),
                foreign_index,
                &ALICE_ID,
                &foreign_instruction
            )
            .is_err(),
        "foreign signed source/hash must refuse"
    );
    drop(block);
}

#[inline(never)]
fn assert_native_genesis_foreign_state_network(
    prepared: &crate::sumeragi::test_chain::PreparedTestChainConfig,
    instruction: &InstructionBox,
    index: usize,
) {
    let source = prepared.genesis.block();
    let foreign_network_state = state_for_testing(World::new());
    let mut foreign_network_block = foreign_network_state
        .block_with_pristine_carrier_stage(source, |_| Ok::<_, String>(()))
        .unwrap();
    let network_error = foreign_network_block
        .transaction_for_original_genesis_testing(source, index, &ALICE_ID, instruction)
        .err()
        .expect("a source cannot grant bootstrap scope to another State network");
    assert!(network_error.contains("another State network"));
    drop(foreign_network_block);
}

#[inline(never)]
fn assert_native_genesis_successor_header(
    prepared: &crate::sumeragi::test_chain::PreparedTestChainConfig,
    instruction: &InstructionBox,
    index: usize,
) {
    let source = prepared.genesis.block();
    let mut wrong_header = prepared.state.block(BlockHeader::new(
        nonzero!(2_u64),
        Some(source.hash()),
        None,
        0,
        0,
    ));
    let header_error = wrong_header
        .transaction_for_original_genesis_testing(source, index, &ALICE_ID, instruction)
        .err()
        .expect("a source cannot grant bootstrap scope to a successor header");
    assert!(header_error.contains("another block header"));
    drop(wrong_header);
}

#[inline(never)]
fn assert_native_genesis_committed_bootstrap_refusal(
    prepared: &crate::sumeragi::test_chain::PreparedTestChainConfig,
    instruction: &InstructionBox,
    index: usize,
) {
    let source = prepared.genesis.block();
    crate::sumeragi::startup::apply_genesis(
        &prepared.state,
        source.clone(),
        &ALICE_ID,
        iroha_data_model::parameter::system::ConsensusMode::Permissioned,
        None,
    )
    .unwrap();
    let mut replay = prepared
        .state
        .block_with_pristine_carrier_stage(source, |_| Ok::<_, String>(()))
        .unwrap();
    assert!(
        replay
            .transaction_for_original_genesis_testing(source, index, &ALICE_ID, instruction)
            .is_err(),
        "committed history must not regain original bootstrap scope"
    );
}

#[test]
fn initial_executor_rejects_raw_domain_registration_after_genesis() {
    use iroha_executor_data_model::permission::domain::CanRegisterDomain;
    let existing_domain = DomainId::try_new("wonderland", "universal").expect("existing domain id");
    let world = World::with(
        [Domain::new(existing_domain).build(&ALICE_ID)],
        [Account::new(ALICE_ID.clone()).build(&ALICE_ID)],
        [],
    );
    let state = state_after_genesis(world);
    let mut block = state.block(BlockHeader::new(
        nonzero!(2_u64),
        Some(
            state
                .view()
                .latest_block_hash()
                .expect("genuine genesis parent"),
        ),
        None,
        0,
        0,
    ));
    let mut state_transaction = block.transaction();
    Grant::account_permission(CanRegisterDomain, ALICE_ID.clone())
        .execute(&ALICE_ID, &mut state_transaction)
        .expect("seed legacy permission directly");
    let domain_id = DomainId::try_new("planned", "universal").expect("planned domain id");
    let error = super::Executor::Initial
        .execute_instruction(
            &mut state_transaction,
            &ALICE_ID,
            Register::domain(Domain::new(domain_id)).into(),
        )
        .expect_err("raw domain registration must be denied after genesis");
    assert!(
        matches!(error, ValidationFail::NotPermitted(ref message) if message.contains("reserved for genesis"))
    );
}
#[test]
fn initial_executor_allows_native_escrow_open_without_transfer_permission() {
    let seller = ALICE_ID.clone();
    let asset_definition_id = AssetDefinitionId::derive_from_components(
        DomainId::try_new("wonderland", "universal").expect("domain id"),
        "xor".parse().expect("asset name"),
    );
    let domain = Domain::new(DomainId::try_new("wonderland", "universal").expect("domain id"))
        .build(&seller);
    let seller_account = Account::new(seller.clone()).build(&seller);
    let asset_definition = AssetDefinition::numeric(
        asset_definition_id.clone(),
        "XOR".to_owned(),
        iroha_data_model::asset::AssetBalancePolicy::Global,
        None,
    )
    .build(&seller);
    let seller_asset_id = AssetId::of(asset_definition_id.clone(), seller.clone());
    let seller_asset = Asset::new(seller_asset_id.clone(), Quantity::from(100_u64));
    let world = World::with_assets(
        [domain],
        [seller_account, Account::new(BOB_ID.clone()).build(&seller)],
        [asset_definition],
        [seller_asset],
        [],
    );
    let state = state_after_genesis(world);
    let header = BlockHeader::new(
        nonzero!(2_u64),
        Some(
            state
                .view()
                .latest_block_hash()
                .expect("genuine genesis parent"),
        ),
        None,
        0,
        0,
    );
    let mut block = state.block(header);
    let mut stx = block.transaction_for_fastpq_testing(Hash::prehashed([0xE5; Hash::LENGTH]));
    let escrow_id = EscrowId::new(Hash::new("executor-native-escrow-open"));
    let instruction = iroha_data_model::isi::escrow::OpenAssetEscrow::new(
        escrow_id,
        asset_definition_id.clone(),
        Quantity::from(40_u64),
    );
    let res = super::Executor::Initial.execute_instruction(
        &mut stx,
        &seller,
        InstructionBox::from(instruction),
    );
    assert!(
        res.is_ok(),
        "native escrow opening should not require generic CanTransferAsset permission: {res:?}"
    );
    let record = stx
        .world
        .asset_escrows
        .get(&escrow_id)
        .expect("escrow record");
    let custody_asset_id = AssetId::of(asset_definition_id, record.custody.clone());
    let seller_balance = stx
        .world
        .assets
        .get(&seller_asset_id)
        .map(|value| value.as_ref().clone())
        .expect("seller balance");
    let custody_balance = stx
        .world
        .assets
        .get(&custody_asset_id)
        .map(|value| value.as_ref().clone())
        .expect("custody balance");
    assert_eq!(seller_balance, Quantity::from(60_u64));
    assert_eq!(custody_balance, Quantity::from(40_u64));
    let cancel = iroha_data_model::isi::escrow::CancelAssetEscrow::new(escrow_id);
    let error = super::Executor::Initial
        .execute_instruction(&mut stx, &BOB_ID, cancel.clone().into())
        .expect_err("a different authority cannot refund the seller's escrow");
    assert!(
        format!("{error:?}").contains("only seller may cancel escrow"),
        "the escrow must reach its native ownership guard: {error:?}"
    );
    assert_eq!(
        stx.world
            .assets
            .get(&custody_asset_id)
            .map(|value| value.as_ref().clone()),
        Some(Quantity::from(40_u64)),
        "a denied refund leaves custody untouched"
    );
    super::Executor::Initial
        .execute_instruction(&mut stx, &seller, cancel.into())
        .expect("the seller can terminalize native escrow custody");
    assert_eq!(
        stx.world
            .assets
            .get(&seller_asset_id)
            .map(|value| value.as_ref().clone()),
        Some(Quantity::from(100_u64))
    );
}
#[test]
fn initial_executor_rejects_domainless_asset_definition_registration_after_genesis() {
    let alice_id = ALICE_ID.clone();
    let domain_id: DomainId = DomainId::try_new("wonderland", "universal").expect("domain id");
    let domain: Domain = Domain::new(domain_id.clone()).build(&alice_id);
    let alice_account = Account::new(alice_id.clone()).build(&alice_id);
    let world = World::with([domain], [alice_account], []);
    let state = state_after_genesis(world);
    let header = BlockHeader::new(
        nonzero!(2_u64),
        Some(
            state
                .view()
                .latest_block_hash()
                .expect("genuine genesis parent"),
        ),
        None,
        0,
        0,
    );
    let mut block = state.block(header);
    let executor = super::Executor::Initial;
    let asset_definition_id = AssetDefinitionId::from_uuid_bytes([
        0x2e, 0x3d, 0x34, 0xbe, 0xb8, 0xa8, 0x42, 0x39, 0xb3, 0xd9, 0x59, 0x07, 0x70, 0xf1, 0x18,
        0x9e,
    ])
    .expect("opaque asset definition id");
    let instruction = InstructionBox::from(Register::asset_definition(AssetDefinition::numeric(
        asset_definition_id.clone(),
        "cbdc".to_owned(),
        iroha_data_model::asset::AssetBalancePolicy::Global,
        None,
    )));
    let mut stx = block.transaction();
    let result = executor.execute_instruction(&mut stx, &alice_id, instruction);
    assert!(
        matches!(
            result,
            Err(ValidationFail::NotPermitted(ref message))
                if message.contains("domainless asset definitions may only be registered in genesis")
        ),
        "post-genesis opaque asset registration must fail closed: {result:?}"
    );
    assert!(
        stx.world.asset_definition(&asset_definition_id).is_err(),
        "a rejected opaque asset definition must not enter world state"
    );
}
#[test]
fn initial_executor_authorizes_wire_decoded_asset_definition_from_explicit_owner() {
    let domain_id = DomainId::try_new("wire_registration", "universal").expect("domain identifier");
    let projected_id = AssetDefinitionId::derive_from_components(
        domain_id.clone(),
        "coin".parse().expect("asset name"),
    );
    let world = World::with(
        [Domain::new(domain_id.clone()).build(&ALICE_ID)],
        [Account::new(ALICE_ID.clone()).build(&ALICE_ID)],
        [],
    );
    let state = state_after_genesis(world);
    let registration = Register::asset_definition(AssetDefinition::numeric(
        projected_id,
        "coin".to_owned(),
        iroha_data_model::asset::AssetBalancePolicy::Global,
        Some(domain_id.clone()),
    ));
    let encoded = registration.encode();
    let mut input = encoded.as_slice();
    let decoded = Register::<AssetDefinition>::decode(&mut input)
        .expect("decode asset-definition registration");
    assert!(input.is_empty());
    let opaque_id = decoded.object().id().clone();
    assert_eq!(decoded.object().owning_domain.as_ref(), Some(&domain_id));
    assert!(decoded.object().alias.is_none());
    let mut block = state.block(BlockHeader::new(
        nonzero!(2_u64),
        Some(
            state
                .view()
                .latest_block_hash()
                .expect("genuine genesis parent"),
        ),
        None,
        0,
        0,
    ));
    let mut transaction = block.transaction();
    super::Executor::Initial
        .execute_instruction(&mut transaction, &ALICE_ID, InstructionBox::from(decoded))
        .expect("explicit owning domain must authorize after wire decoding");
    assert!(transaction.world.asset_definition(&opaque_id).is_ok());
    assert_eq!(
        transaction.world.asset_definition_domains.get(&opaque_id),
        Some(&domain_id),
        "registration must derive the domain index from explicit ownership"
    );
}
#[test]
fn asset_definition_registration_alias_cannot_supply_owning_domain_authority() {
    let owned_domain = DomainId::try_new("owned", "universal").expect("owned domain");
    let foreign_domain = DomainId::try_new("foreign", "universal").expect("foreign domain");
    let foreign_owner = checked_account_id();
    let state = state_after_genesis(World::with(
        [
            Domain::new(owned_domain.clone()).build(&ALICE_ID),
            Domain::new(foreign_domain.clone()).build(&foreign_owner),
        ],
        [
            Account::new(ALICE_ID.clone()).build(&ALICE_ID),
            Account::new(foreign_owner.clone()).build(&foreign_owner),
        ],
        [],
    ));
    let asset_id = AssetDefinitionId::derive_from_components(
        owned_domain,
        "coin".parse().expect("asset name"),
    );
    let owned_alias: iroha_data_model::asset::AssetDefinitionAlias =
        "coin#owned.universal".parse().expect("asset alias");
    let mut block = state.block(BlockHeader::new(nonzero!(2_u64), None, None, 0, 0));
    for (owning_domain, alias, expected) in [
        (
            None,
            Some(owned_alias.clone()),
            "domainless asset definitions may only be registered in genesis",
        ),
        (
            Some(foreign_domain.clone()),
            Some(owned_alias),
            "Can't register asset definition",
        ),
        (
            Some(foreign_domain),
            None,
            "Can't register asset definition",
        ),
    ] {
        let registration = Register::asset_definition(
            AssetDefinition::numeric(
                asset_id.clone(),
                "coin",
                iroha_data_model::asset::AssetBalancePolicy::Global,
                owning_domain,
            )
            .with_alias(alias),
        );
        let mut transaction = block.transaction();
        let error = super::ensure_asset_definition_registration_allowed(
            &mut transaction,
            &ALICE_ID,
            &registration,
        )
        .expect_err("only the owner of the explicit owning domain may register an asset");
        assert!(matches!(error, ValidationFail::NotPermitted(message) if message == expected));
        assert!(transaction.world.asset_definition(&asset_id).is_err());
        assert!(
            transaction
                .world
                .asset_definition_domains
                .get(&asset_id)
                .is_none()
        );
    }
}
#[test]
fn initial_executor_enforces_exact_pkr_mint_and_metadata_permissions() {
    let owner = ALICE_ID.clone();
    let retail = checked_account_id();
    let domain_id = DomainId::try_new("wonderland", "universal").expect("domain id");
    let pkr = AssetDefinitionId::from_uuid_bytes([
        0x5d, 0x9f, 0x4d, 0x8d, 0x11, 0x5a, 0x49, 0xc0, 0x95, 0x3a, 0x6a, 0x9b, 0xe8, 0x22, 0x77,
        0x01,
    ])
    .expect("opaque PKR definition id");
    let world = World::with(
        [Domain::new(domain_id).build(&owner)],
        [
            Account::new(owner.clone()).build(&owner),
            Account::new(retail.clone()).build(&retail),
        ],
        [AssetDefinition::numeric(
            pkr.clone(),
            "PKR".to_owned(),
            iroha_data_model::asset::AssetBalancePolicy::Global,
            None,
        )
        .build(&owner)],
    );
    let state = state_after_genesis(world);
    let mut block = state.block(BlockHeader::new(
        nonzero!(2_u64),
        Some(
            state
                .view()
                .latest_block_hash()
                .expect("genuine genesis parent"),
        ),
        None,
        0,
        0,
    ));
    let mut stx = block.transaction();
    let executor = super::Executor::Initial;
    let retail_pkr = AssetId::new(pkr.clone(), retail.clone());
    stx.world.account_permissions.insert(
        retail.clone(),
        BTreeSet::from([Permission::new(
            "CanMintAsset".to_owned(),
            norito::json!({"asset": (retail_pkr.to_string())}),
        )]),
    );
    let legacy_permission_mint = executor.execute_instruction(
        &mut stx,
        &retail,
        InstructionBox::from(Mint::asset_quantity(1_u32, retail_pkr.clone())),
    );
    assert!(
        matches!(
            legacy_permission_mint,
            Err(ValidationFail::NotPermitted(ref message))
                if message.contains("exact mint permission")
        ),
        "the retired CanMintAsset token must be inert"
    );
    stx.world.account_permissions.insert(
        retail.clone(),
        BTreeSet::from([Permission::from(
            executor_permission::asset::CanMintAssetToAccount {
                asset_definition: pkr.clone(),
                account: owner.clone(),
            },
        )]),
    );
    let wrong_destination_mint = executor.execute_instruction(
        &mut stx,
        &retail,
        InstructionBox::from(Mint::asset_quantity(1_u32, retail_pkr.clone())),
    );
    assert!(
        matches!(
            wrong_destination_mint,
            Err(ValidationFail::NotPermitted(ref message))
                if message.contains("exact mint permission")
        ),
        "a destination-scoped permission must not authorize another account"
    );
    let metadata_key: Name = "display.category".parse().expect("metadata key");
    let unprivileged_metadata = executor.execute_instruction(
        &mut stx,
        &retail,
        InstructionBox::from(iroha_data_model::isi::SetKeyValue::asset_definition(
            pkr.clone(),
            metadata_key.clone(),
            Json::new("retail"),
        )),
    );
    assert!(
        matches!(
            unprivileged_metadata,
            Err(ValidationFail::NotPermitted(ref message))
                if message.contains("metadata")
        ),
        "unexpected unprivileged PKR metadata result: {unprivileged_metadata:?}"
    );
    stx.world.account_permissions.insert(
        retail.clone(),
        BTreeSet::from([
            Permission::from(executor_permission::asset::CanMintAssetToAccount {
                asset_definition: pkr.clone(),
                account: retail.clone(),
            }),
            Permission::from(
                executor_permission::asset_definition::CanModifyAssetDefinitionMetadata {
                    asset_definition: pkr.clone(),
                },
            ),
        ]),
    );
    executor
        .execute_instruction(
            &mut stx,
            &retail,
            InstructionBox::from(Mint::asset_quantity(1_u32, retail_pkr.clone())),
        )
        .expect("the exact PKR destination mint grant must authorize minting");
    executor
        .execute_instruction(
            &mut stx,
            &retail,
            InstructionBox::from(iroha_data_model::isi::SetKeyValue::asset_definition(
                pkr.clone(),
                metadata_key,
                Json::new("retail"),
            )),
        )
        .expect("the exact PKR metadata grant must authorize metadata changes");
    assert_eq!(
        stx.world
            .assets
            .get(&retail_pkr)
            .expect("minted retail PKR balance")
            .as_ref(),
        &Quantity::from(1_u32),
    );
    assert_eq!(
        stx.world
            .asset_definition(&pkr)
            .expect("PKR definition")
            .metadata()
            .get(&"display.category".parse::<Name>().expect("metadata key")),
        Some(&Json::new("retail")),
    );
}
#[test]
fn extract_transfer_asset_definition_ignores_register_asset_definition_instruction() {
    let asset_definition_id: AssetDefinitionId = AssetDefinitionId::derive_from_components(
        DomainId::try_new("defs", "universal").expect("defs domain id"),
        "bond".parse().expect("asset definition name"),
    );
    let instruction = InstructionBox::from(Register::asset_definition(AssetDefinition::numeric(
        asset_definition_id,
        "bond".to_owned(),
        iroha_data_model::asset::AssetBalancePolicy::Global,
        None,
    )));
    assert!(
        extract_transfer_asset_definition(&instruction).is_none(),
        "register asset-definition instruction must not decode as transfer"
    );
}
#[test]
fn direct_dataspace_registration_extracts_only_its_distinct_instruction() {
    let id = AssetDefinitionId::derive_from_components(
        DomainId::try_new("defs", "universal").expect("domain id"),
        "bond".parse().expect("name"),
    );
    let object = AssetDefinition::numeric(
        id,
        "Bond".to_owned(),
        iroha_data_model::asset::AssetBalancePolicy::DataspaceRestricted,
        None,
    );
    let direct = iroha_data_model::isi::RegisterDataspaceAssetDefinition::new(
        DataSpaceId::new(7),
        object.clone(),
    )
    .expect("direct registration");
    let instruction = InstructionBox::from(direct.clone());
    assert_eq!(
        extract_register_dataspace_asset_definition(&instruction),
        Some(direct)
    );
    assert!(extract_register_asset_definition(&instruction).is_none());
    let original = InstructionBox::from(Register::asset_definition(object));
    assert!(extract_register_dataspace_asset_definition(&original).is_none());
}
#[test]
fn extract_register_asset_definition_accepts_register_asset_definition_instruction() {
    let asset_definition_id: AssetDefinitionId = AssetDefinitionId::derive_from_components(
        DomainId::try_new("defs", "universal").expect("defs domain id"),
        "bond".parse().expect("asset definition name"),
    );
    let instruction = InstructionBox::from(Register::asset_definition(AssetDefinition::numeric(
        asset_definition_id.clone(),
        "bond".to_owned(),
        iroha_data_model::asset::AssetBalancePolicy::Global,
        None,
    )));
    let reg = extract_register_asset_definition(&instruction)
        .expect("expected to extract register asset-definition instruction");
    assert_eq!(reg.object().id(), &asset_definition_id);
}
#[test]
fn initial_executor_denies_transfer_domain_without_ownership() {
    let alice_id = ALICE_ID.clone();
    let users_domain_id: DomainId =
        DomainId::try_new("users", "universal").expect("users domain id");
    let foo_domain_id: DomainId = DomainId::try_new("foo", "universal").expect("foo domain id");
    let user1 = checked_account_id();
    let user2 = checked_account_id();
    let users_domain = Domain::new(users_domain_id.clone()).build(&user1);
    let foo_domain = Domain::new(foo_domain_id.clone()).build(&user1);
    let alice_account = Account::new(alice_id.clone()).build(&alice_id);
    let user1_account = Account::new(user1.clone()).build(&user1);
    let user2_account = Account::new(user2.clone()).build(&user2);
    let world = World::with(
        [users_domain, foo_domain],
        [alice_account, user1_account, user2_account],
        [],
    );
    let state = state_after_genesis(world);
    let header = BlockHeader::new(nonzero!(2_u64), None, None, 0, 0);
    let mut block = state.block(header);
    let executor = super::Executor::Initial;
    let instruction = InstructionBox::from(Transfer::domain(
        user1.clone(),
        foo_domain_id,
        user2.clone(),
    ));
    let transfer = extract_transfer_domain(&instruction)
        .expect("expected to extract domain transfer from instruction");
    let mut stx = block.transaction();
    assert_eq!(
        stx.world
            .domain(&users_domain_id)
            .expect("users domain should exist")
            .owned_by(),
        &user1
    );
    assert_eq!(
        stx.world
            .domain(transfer.object())
            .expect("foo domain should exist")
            .owned_by(),
        &user1
    );
    let allowed = can_transfer_domain(&stx.world, &alice_id, &transfer, 0)
        .expect("domain transfer permission check");
    assert!(
        !allowed,
        "alice should not be allowed to transfer foo domain"
    );
    assert!(
        !(stx._curr_block.is_genesis() && stx.block_hashes.is_empty()),
        "test must execute in non-genesis context"
    );
    let res = executor.execute_instruction(&mut stx, &alice_id, instruction);
    assert!(
        matches!(res, Err(ValidationFail::NotPermitted(_))),
        "initial executor should deny domain transfer from another account, got: {res:?}"
    );
}
#[test]
fn initial_executor_denies_transfer_asset_by_asset_definition_domain_owner() {
    let alice_id = ALICE_ID.clone();
    let users_domain_id: DomainId =
        DomainId::try_new("users", "universal").expect("users domain id");
    let user1 = checked_account_id();
    let user2 = checked_account_id();
    let users_domain = Domain::new(users_domain_id.clone()).build(&alice_id);
    let alice_account = Account::new(alice_id.clone()).build(&alice_id);
    let user1_account = Account::new(user1.clone()).build(&user1);
    let user2_account = Account::new(user2.clone()).build(&user2);
    let asset_definition_id =
        AssetDefinitionId::derive_from_components(users_domain_id.clone(), "coin".parse().unwrap());
    let asset_definition = AssetDefinition::numeric(
        asset_definition_id.clone(),
        "coin".to_owned(),
        iroha_data_model::asset::AssetBalancePolicy::Global,
        None,
    )
    .build(&user1);
    let transfer_asset_id = AssetId::new(asset_definition_id, user1.clone());
    let source_balance = Asset::new(transfer_asset_id.clone(), Quantity::from(10_u64));
    let world = World::with_assets(
        [users_domain],
        [alice_account, user1_account, user2_account],
        [asset_definition],
        [source_balance],
        [],
    );
    let state = state_after_genesis(world);
    let header = BlockHeader::new(
        nonzero!(2_u64),
        Some(
            state
                .view()
                .latest_block_hash()
                .expect("genuine genesis parent"),
        ),
        None,
        0,
        0,
    );
    let mut block = state.block(header);
    let instruction = InstructionBox::from(Transfer::asset_quantity(
        transfer_asset_id,
        1_u32,
        user2.clone(),
    ));
    let transfer = extract_transfer_asset(&instruction)
        .expect("expected to extract asset transfer from instruction");
    let mut stx = block.transaction();
    let allowed = can_transfer_asset(&stx.world, &alice_id, None, &transfer)
        .expect("asset transfer permission check");
    assert!(
        !allowed,
        "asset-definition domain ownership must not authorize transfers from another account"
    );
    let result = super::Executor::Initial.execute_instruction(&mut stx, &alice_id, instruction);
    assert!(
        matches!(
            result,
            Err(ValidationFail::NotPermitted(ref message))
                if message.contains("source asset owner must sign")
        ),
        "asset-definition domain owner bypass must fail before applying the transfer: {result:?}"
    );
}
#[test]
fn initial_executor_denies_transfer_asset_by_active_alias_domain_owner_for_all_shapes() {
    use iroha_data_model::{
        account::{
            AccountAddress,
            rekey::{AccountAlias, AccountAliasDomain},
        },
        nexus::DataSpaceCatalog,
        sns::{NameControllerV1, NameRecordV1},
    };
    let alias_domain_owner = ALICE_ID.clone();
    let source = checked_account_id();
    let destination = checked_account_id();
    let alias_domain_id = DomainId::try_new("fi", "universal").expect("alias domain id");
    let asset_domain_id = DomainId::try_new("assets", "universal").expect("asset domain id");
    let asset_definition_id =
        AssetDefinitionId::derive_from_components(asset_domain_id.clone(), "coin".parse().unwrap());
    let source_asset_id = AssetId::new(asset_definition_id.clone(), source.clone());
    let mut world = World::with_assets(
        [
            Domain::new(alias_domain_id).build(&alias_domain_owner),
            Domain::new(asset_domain_id).build(&source),
        ],
        [
            Account::new(alias_domain_owner.clone()).build(&alias_domain_owner),
            Account::new(source.clone()).build(&source),
            Account::new(destination.clone()).build(&destination),
        ],
        [AssetDefinition::numeric(
            asset_definition_id,
            "coin".to_owned(),
            iroha_data_model::asset::AssetBalancePolicy::Global,
            None,
        )
        .build(&source)],
        [Asset::new(source_asset_id.clone(), Quantity::from(10_u64))],
        [],
    );
    let alias = AccountAlias::new(
        "customer".parse().expect("alias label"),
        Some(AccountAliasDomain::new("fi".parse().expect("alias domain"))),
        DataSpaceId::UNIVERSAL,
    );
    let selector = crate::sns::selector_for_account_alias(&alias, &DataSpaceCatalog::default())
        .expect("account alias selector");
    let address = AccountAddress::from_account_id(&source).expect("source address");
    let lease = NameRecordV1::new(
        selector.clone(),
        source.clone(),
        vec![NameControllerV1::account(&address)],
        0,
        0,
        100,
        200,
        300,
        Metadata::default(),
    );
    world
        .smart_contract_state_mut_for_testing()
        .insert(crate::sns::record_storage_key(&selector), lease.encode());
    world.account_aliases.insert(alias.clone(), source.clone());
    world
        .account_aliases_by_account
        .insert(source.clone(), BTreeSet::from([alias.clone()]));
    world.replace_account_rekey_record_for_testing(
        iroha_data_model::account::rekey::AccountRekeyRecord::new(alias, source.clone()),
    );
    assert!(
        authority_owns_any_alias_domain(&world.view(), &alias_domain_owner, &source, 50)
            .expect("active alias-domain ownership check"),
        "fixture must prove that the attacker owns an active alias domain for the source"
    );
    let state = state_after_genesis(world);
    let mut block = state.block(BlockHeader::new(
        nonzero!(2_u64),
        Some(
            state
                .view()
                .latest_block_hash()
                .expect("genuine genesis parent"),
        ),
        None,
        50,
        0,
    ));
    let mut transaction = block.transaction();
    let transfer = Transfer::asset_quantity(source_asset_id, 1_u32, destination.clone());
    let boxed = InstructionBox::from(transfer.clone());
    let concrete = concrete_instruction_box!(Transfer<Asset, Quantity, Account>, transfer);
    let boxed_result =
        super::Executor::Initial.execute_instruction(&mut transaction, &alias_domain_owner, boxed);
    assert!(
        matches!(
            boxed_result,
            Err(ValidationFail::NotPermitted(ref message))
                if message.contains("source asset owner must sign")
        ),
        "active alias-domain ownership must not authorize TransferBox::Asset: {boxed_result:?}"
    );
    let concrete_result = super::Executor::Initial.execute_borrowed_overlay_instruction(
        &mut transaction,
        &alias_domain_owner,
        &concrete,
        None,
    );
    assert!(
        matches!(concrete_result, Err(ValidationFail::NotPermitted(_))),
        "active alias-domain ownership must not authorize a borrowed concrete transfer: {concrete_result:?}"
    );
}
#[test]
fn initial_executor_trigger_registration_requires_exact_canonical_authority() {
    use crate::smartcontracts::triggers::set::SetReadOnly as _;
    use iroha_data_model::{
        account::{
            AccountAddress,
            rekey::{AccountAlias, AccountAliasDomain},
        },
        events::execute_trigger::ExecuteTriggerEventFilter,
        nexus::DataSpaceCatalog,
        sns::{NameControllerV1, NameRecordV1},
    };

    let alias_domain_owner = ALICE_ID.clone();
    let trigger_owner = checked_account_id();
    let alias_domain_id = DomainId::try_new("triggerbank", "universal").expect("alias domain id");
    let mut world = World::with(
        [Domain::new(alias_domain_id).build(&alias_domain_owner)],
        [
            Account::new(alias_domain_owner.clone()).build(&alias_domain_owner),
            Account::new(trigger_owner.clone()).build(&trigger_owner),
        ],
        [],
    );
    let alias = AccountAlias::new(
        "customer".parse().expect("alias label"),
        Some(AccountAliasDomain::new(
            "triggerbank".parse().expect("alias domain"),
        )),
        DataSpaceId::UNIVERSAL,
    );
    let selector = crate::sns::selector_for_account_alias(&alias, &DataSpaceCatalog::default())
        .expect("account alias selector");
    let address = AccountAddress::from_account_id(&trigger_owner).expect("owner address");
    let lease = NameRecordV1::new(
        selector.clone(),
        trigger_owner.clone(),
        vec![NameControllerV1::account(&address)],
        0,
        0,
        100,
        200,
        300,
        Metadata::default(),
    );
    world
        .smart_contract_state_mut_for_testing()
        .insert(crate::sns::record_storage_key(&selector), lease.encode());
    world
        .account_aliases
        .insert(alias.clone(), trigger_owner.clone());
    world
        .account_aliases_by_account
        .insert(trigger_owner.clone(), BTreeSet::from([alias.clone()]));
    world.replace_account_rekey_record_for_testing(
        iroha_data_model::account::rekey::AccountRekeyRecord::new(alias, trigger_owner.clone()),
    );
    assert!(
        authority_owns_any_alias_domain(&world.view(), &alias_domain_owner, &trigger_owner, 50,)
            .expect("active alias-domain ownership check"),
        "fixture must expose the retired authorization shortcut",
    );

    let state = state_after_genesis(world);
    let mut block = state.block(BlockHeader::new(
        nonzero!(2_u64),
        Some(
            state
                .view()
                .latest_block_hash()
                .expect("genuine genesis parent"),
        ),
        None,
        50,
        0,
    ));
    let mut transaction = block.transaction();
    let trigger_id: TriggerId = "initial_exact_trigger_owner".parse().expect("trigger id");
    let registration = || {
        Register::trigger(Trigger::new(
            trigger_id.clone(),
            Action::new(
                Vec::<InstructionBox>::new(),
                Repeats::Indefinitely,
                trigger_owner.clone(),
                ExecuteTriggerEventFilter::new().for_trigger(trigger_id.clone()),
            )
            .expect("valid trigger action"),
        ))
        .into()
    };

    let attacker_result = super::Executor::Initial.execute_instruction(
        &mut transaction,
        &alias_domain_owner,
        registration(),
    );
    assert!(
        matches!(
            &attacker_result,
            Err(ValidationFail::InstructionFailed(
                InstructionExecutionError::InvalidParameter(
                    iroha_data_model::isi::error::InvalidParameterError::SmartContract(message)
                )
            )) if message.contains("Missing CanRegisterTrigger")
        ),
        "alias-domain owner must not install code for the canonical account: {attacker_result:?}",
    );
    super::Executor::Initial
        .execute_instruction(&mut transaction, &trigger_owner, registration())
        .expect("the exact trigger owner must not need an alias or self-grant");
    assert!(
        transaction
            .world
            .triggers
            .inspect_by_id(&trigger_id, |_| ())
            .is_some(),
        "exact-owner registration must reach trigger storage",
    );
}
include!("executor_initial_batch_authorization_tests.rs");
#[test]
fn initial_executor_transfer_asset_batch_requires_active_canonical_contract_context_for_both_modes()
{
    #[derive(Clone, Copy, core::fmt::Debug)]
    enum Provenance {
        Active,
        MissingReverse,
        InactiveAddress,
    }
    for independent in [false, true] {
        for provenance in [
            Provenance::Active,
            Provenance::MissingReverse,
            Provenance::InactiveAddress,
        ] {
            let deployer = checked_account_id();
            let destination = checked_account_id();
            let state = state_after_genesis(World::with(
                [],
                [
                    Account::new(deployer.clone()).build(&deployer),
                    Account::new(destination.clone()).build(&destination),
                ],
                [],
            ));
            let network_id = state.network_id;
            let contract_address =
                ContractAddress::derive(&network_id, &deployer, 810, DataSpaceId::UNIVERSAL)
                    .expect("contract address");
            let contract_subject = contract_address.subject_id();
            let asset_definition = AssetDefinitionId::from_uuid_bytes([
                0xb8, 0xb8, 0xb8, 0xb8, 0xb8, 0xb8, 0x48, 0xb8, 0xb8, 0xb8, 0xb8, 0xb8, 0xb8, 0xb8,
                0xb8, 0xb8,
            ])
            .expect("opaque asset definition");
            let source_asset = AssetId::new(asset_definition.clone(), contract_subject.clone());
            let mut world = World::with_assets(
                [],
                [
                    Account::new(deployer.clone()).build(&deployer),
                    Account::new(contract_subject.clone()).build(&contract_subject),
                    Account::new(destination.clone()).build(&destination),
                ],
                [AssetDefinition::numeric(
                    asset_definition.clone(),
                    "contract batch coin".to_owned(),
                    iroha_data_model::asset::AssetBalancePolicy::Global,
                    None,
                )
                .build(&deployer)],
                [Asset::new(source_asset.clone(), Quantity::from(10_u32))],
                [],
            );
            seed_test_asset_supply(&mut world, &asset_definition);
            let contract_code_hash = Hash::new(b"contract-batch-code");
            world
                .contract_instances
                .insert(contract_address.clone(), contract_code_hash);
            world.contract_subject_bindings.insert(
                contract_address.clone(),
                crate::smartcontracts::code::ContractSubjectBinding::new_direct(
                    &contract_address,
                    deployer.clone(),
                )
                .with_active_code_hash(contract_code_hash),
            );
            world
                .contract_subject_addresses
                .insert(contract_subject.clone(), contract_address.clone());
            let context_address = if matches!(provenance, Provenance::InactiveAddress) {
                ContractAddress::derive(&network_id, &deployer, 811, DataSpaceId::UNIVERSAL)
                    .expect("inactive contract address")
            } else {
                contract_address.clone()
            };
            let context = ContractRuntimeExecutionContext {
                contract_address: context_address,
                contract_subject: contract_subject.clone(),
                contract_alias: None,
                entrypoint: "execute_batch".to_owned(),
            };
            let mut seeded = state.world.block();
            seeded.asset_definitions.insert(
                asset_definition.clone(),
                world
                    .asset_definitions
                    .view()
                    .get(&asset_definition)
                    .unwrap()
                    .clone(),
            );
            seeded.assets.insert(
                source_asset.clone(),
                world.assets.view().get(&source_asset).unwrap().clone(),
            );
            seeded.commit();
            bind_executor_test_contract_after_genesis(
                &state,
                &contract_address,
                &deployer,
                contract_code_hash,
            );
            let mut block = state.block(BlockHeader::new(
                nonzero!(2_u64),
                Some(
                    state
                        .view()
                        .latest_block_hash()
                        .expect("genuine genesis parent"),
                ),
                None,
                0,
                0,
            ));
            let mut transaction = block.transaction_for_fastpq_testing(Hash::new(
                format!("contract-batch-{independent}-{provenance:?}").as_bytes(),
            ));
            if matches!(provenance, Provenance::MissingReverse) {
                transaction
                    .world
                    .contract_subject_addresses
                    .remove(contract_subject.clone());
            }
            let entries = vec![TransferAssetBatchEntry::with_leg_id(
                "contract-owned",
                contract_subject.clone(),
                destination.clone(),
                asset_definition.clone(),
                2_u32,
            )];
            let batch = if independent {
                TransferAssetBatch::independent(entries)
            } else {
                TransferAssetBatch::new(entries)
            };
            let result = super::Executor::Initial
                .execute_instruction_with_contract_runtime_context(
                    &mut transaction,
                    &contract_subject,
                    batch.into(),
                    Some(&context),
                );
            if matches!(provenance, Provenance::Active) {
                result.unwrap_or_else(|error| {
                    panic!(
                        "active contract batch (independent={independent}) was rejected: {error:?}"
                    )
                });
                assert_eq!(
                    initial_batch_balance(&transaction.world, &source_asset),
                    Quantity::from(8_u32)
                );
            } else {
                assert!(
                    matches!(result, Err(ValidationFail::NotPermitted(_))),
                    "invalid {provenance:?} context (independent={independent}) must reject the whole batch: {result:?}"
                );
                assert_eq!(
                    initial_batch_balance(&transaction.world, &source_asset),
                    Quantity::from(10_u32),
                    "provenance rejection must precede either settlement mode"
                );
            }
        }
    }
}
#[test]
fn initial_executor_requires_an_active_consistent_contract_context_for_contract_assets() {
    let deployer = checked_account_id();
    let destination = checked_account_id();
    let state = state_after_genesis(World::with(
        [],
        [
            Account::new(deployer.clone()).build(&deployer),
            Account::new(destination.clone()).build(&destination),
        ],
        [],
    ));
    let contract_address =
        ContractAddress::derive(&state.network_id, &deployer, 808, DataSpaceId::UNIVERSAL)
            .expect("contract address");
    let contract_subject = contract_address.subject_id();
    let asset_definition_id = AssetDefinitionId::from_uuid_bytes([
        0xa8, 0xa8, 0xa8, 0xa8, 0xa8, 0xa8, 0x48, 0xa8, 0xa8, 0xa8, 0xa8, 0xa8, 0xa8, 0xa8, 0xa8,
        0xa8,
    ])
    .expect("opaque asset definition");
    let source_asset_id = AssetId::new(asset_definition_id.clone(), contract_subject.clone());
    let mut world = World::with_assets(
        [],
        [
            Account::new(deployer.clone()).build(&deployer),
            Account::new(contract_subject.clone()).build(&contract_subject),
            Account::new(destination.clone()).build(&destination),
        ],
        [AssetDefinition::numeric(
            asset_definition_id,
            "contract coin".to_owned(),
            iroha_data_model::asset::AssetBalancePolicy::Global,
            None,
        )
        .build(&deployer)],
        [Asset::new(source_asset_id.clone(), Quantity::from(10_u64))],
        [],
    );
    let code_hash = Hash::new(b"contract-transfer-context");
    world
        .contract_instances
        .insert(contract_address.clone(), code_hash);
    world.contract_subject_bindings.insert(
        contract_address.clone(),
        crate::smartcontracts::code::ContractSubjectBinding::new_direct(
            &contract_address,
            deployer.clone(),
        )
        .with_active_code_hash(code_hash),
    );
    world
        .contract_subject_addresses
        .insert(contract_subject.clone(), contract_address.clone());
    let mut seeded = state.world.block();
    seeded.asset_definitions.insert(
        source_asset_id.definition().clone(),
        world
            .asset_definitions
            .view()
            .get(source_asset_id.definition())
            .unwrap()
            .clone(),
    );
    seeded.assets.insert(
        source_asset_id.clone(),
        world.assets.view().get(&source_asset_id).unwrap().clone(),
    );
    seeded.commit();
    bind_executor_test_contract_after_genesis(&state, &contract_address, &deployer, code_hash);
    let mut block = state.block(BlockHeader::new(
        nonzero!(2_u64),
        Some(
            state
                .view()
                .latest_block_hash()
                .expect("genuine genesis parent"),
        ),
        None,
        0,
        0,
    ));
    let mut transaction =
        block.transaction_for_fastpq_testing(Hash::new(b"contract-transfer-test"));
    let context = ContractRuntimeExecutionContext {
        contract_address: contract_address.clone(),
        contract_subject: contract_subject.clone(),
        contract_alias: None,
        entrypoint: "execute".to_owned(),
    };
    let transfer = Transfer::asset_quantity(source_asset_id.clone(), 1_u32, destination.clone());
    let boxed = InstructionBox::from(transfer.clone());
    let concrete = concrete_instruction_box!(Transfer<Asset, Quantity, Account>, transfer);
    super::Executor::Initial
        .execute_borrowed_overlay_instruction(
            &mut transaction,
            &contract_subject,
            &boxed,
            Some(&context),
        )
        .expect("active contract subject must be able to transfer its own asset");
    let concrete_result = super::Executor::Initial.execute_borrowed_overlay_instruction(
        &mut transaction,
        &contract_subject,
        &concrete,
        Some(&context),
    );
    assert!(
        matches!(concrete_result, Err(ValidationFail::NotPermitted(_))),
        "borrowed concrete transfers must remain outside the admitted native surface: \
             {concrete_result:?}"
    );
    transaction
        .world
        .contract_subject_addresses
        .remove(contract_subject.clone());
    let missing_reverse_binding = super::Executor::Initial
        .execute_instruction_with_contract_runtime_context(
            &mut transaction,
            &contract_subject,
            Transfer::asset_quantity(source_asset_id.clone(), 1_u32, destination.clone()).into(),
            Some(&context),
        );
    assert!(
        matches!(
            missing_reverse_binding,
            Err(ValidationFail::NotPermitted(_))
        ),
        "a contract context without the canonical reverse subject binding must fail closed: \
             {missing_reverse_binding:?}"
    );
    transaction
        .world
        .contract_subject_addresses
        .insert(contract_subject.clone(), contract_address.clone());
    let inactive_address =
        ContractAddress::derive(&state.network_id, &deployer, 809, DataSpaceId::UNIVERSAL)
            .expect("inactive contract address");
    let inconsistent_context = ContractRuntimeExecutionContext {
        contract_address: inactive_address,
        contract_subject: contract_subject.clone(),
        contract_alias: None,
        entrypoint: "execute".to_owned(),
    };
    let rejected = super::Executor::Initial.execute_instruction_with_contract_runtime_context(
        &mut transaction,
        &contract_subject,
        Transfer::asset_quantity(source_asset_id, 1_u32, destination).into(),
        Some(&inconsistent_context),
    );
    assert!(
        matches!(rejected, Err(ValidationFail::NotPermitted(_))),
        "an inactive or inconsistent contract context must fail closed: {rejected:?}"
    );
}
#[test]
fn contract_runtime_context_alias_does_not_bypass_asset_transfer_authorization() {
    let alice_id = ALICE_ID.clone();
    let users_domain_id: DomainId =
        DomainId::try_new("users", "universal").expect("users domain id");
    let defs_domain_id: DomainId = DomainId::try_new("defs", "universal").expect("defs domain id");
    let alice_domain_id: DomainId =
        DomainId::try_new("wonderland", "universal").expect("domain id");
    let user1 = checked_account_id();
    let user2 = checked_account_id();
    let users_domain = Domain::new(users_domain_id.clone()).build(&user1);
    let defs_domain = Domain::new(defs_domain_id.clone()).build(&user1);
    let alice_domain = Domain::new(alice_domain_id.clone()).build(&alice_id);
    let alice_account = Account::new(alice_id.clone()).build(&alice_id);
    let user1_account = Account::new(user1.clone()).build(&user1);
    let user2_account = Account::new(user2.clone()).build(&user2);
    let asset_definition_id: AssetDefinitionId =
        AssetDefinitionId::derive_from_components(defs_domain_id.clone(), "coin".parse().unwrap());
    let asset_definition = AssetDefinition::numeric(
        asset_definition_id.clone(),
        "coin".to_owned(),
        iroha_data_model::asset::AssetBalancePolicy::Global,
        None,
    )
    .build(&user1);
    let transfer_asset_id = AssetId::new(asset_definition_id.clone(), user1.clone());
    let source_balance = Asset::new(transfer_asset_id.clone(), Quantity::from(10_u64));
    let world = World::with_assets(
        [alice_domain, users_domain, defs_domain],
        [alice_account, user1_account, user2_account],
        [asset_definition],
        [source_balance],
        [],
    );
    let state = state_after_genesis(world);
    let header = BlockHeader::new(
        nonzero!(2_u64),
        Some(
            state
                .view()
                .latest_block_hash()
                .expect("genuine genesis parent"),
        ),
        None,
        0,
        0,
    );
    let mut block = state.block(header);
    let executor = super::Executor::Initial;
    let instruction = InstructionBox::from(Transfer::asset_quantity(
        transfer_asset_id,
        1_u32,
        user2.clone(),
    ));
    let contract_address = ContractAddress::derive(
        &"hash:0000000000000000000000000000000000000000000000000000000000000001#C50E"
            .parse()
            .expect("canonical test network id"),
        &alice_id,
        0,
        DataSpaceId::UNIVERSAL,
    )
    .expect("benefit contract address");
    let context = ContractRuntimeExecutionContext {
        contract_subject: contract_address.subject_id(),
        contract_address,
        contract_alias: Some("benefit::benefit".parse().expect("benefit alias")),
        entrypoint: "spend_to_merchant".to_owned(),
    };
    let mut stx = block.transaction();
    stx.tx_call_hash = Some(Hash::prehashed([0xE6; Hash::LENGTH]));
    let result = executor.execute_instruction_with_contract_runtime_context(
        &mut stx,
        &alice_id,
        instruction,
        Some(&context),
    );
    assert!(
        matches!(
            result,
            Err(ValidationFail::NotPermitted(ref message))
                if message.contains("source asset owner must sign the transaction")
        ),
        "contract alias must not bypass source-owner authorization: {result:?}"
    );
}
#[test]
fn contract_runtime_context_does_not_bypass_non_benefit_spend_entrypoints() {
    let alice_id = ALICE_ID.clone();
    let users_domain_id: DomainId =
        DomainId::try_new("users", "universal").expect("users domain id");
    let defs_domain_id: DomainId = DomainId::try_new("defs", "universal").expect("defs domain id");
    let alice_domain_id: DomainId =
        DomainId::try_new("wonderland", "universal").expect("domain id");
    let user1 = checked_account_id();
    let user2 = checked_account_id();
    let users_domain = Domain::new(users_domain_id.clone()).build(&user1);
    let defs_domain = Domain::new(defs_domain_id.clone()).build(&user1);
    let alice_domain = Domain::new(alice_domain_id.clone()).build(&alice_id);
    let alice_account = Account::new(alice_id.clone()).build(&alice_id);
    let user1_account = Account::new(user1.clone()).build(&user1);
    let user2_account = Account::new(user2.clone()).build(&user2);
    let asset_definition_id: AssetDefinitionId =
        AssetDefinitionId::derive_from_components(defs_domain_id.clone(), "coin".parse().unwrap());
    let asset_definition = AssetDefinition::numeric(
        asset_definition_id.clone(),
        "coin".to_owned(),
        iroha_data_model::asset::AssetBalancePolicy::Global,
        None,
    )
    .build(&user1);
    let transfer_asset_id = AssetId::new(asset_definition_id.clone(), user1.clone());
    let source_balance = Asset::new(transfer_asset_id.clone(), Quantity::from(10_u64));
    let world = World::with_assets(
        [alice_domain, users_domain, defs_domain],
        [alice_account, user1_account, user2_account],
        [asset_definition],
        [source_balance],
        [],
    );
    let state = state_after_genesis(world);
    let header = BlockHeader::new(
        nonzero!(2_u64),
        Some(
            state
                .view()
                .latest_block_hash()
                .expect("genuine genesis parent"),
        ),
        None,
        0,
        0,
    );
    let mut block = state.block(header);
    let executor = super::Executor::Initial;
    let instruction = InstructionBox::from(Transfer::asset_quantity(
        transfer_asset_id,
        1_u32,
        user2.clone(),
    ));
    let contract_address = ContractAddress::derive(
        &"hash:0000000000000000000000000000000000000000000000000000000000000001#C50E"
            .parse()
            .expect("canonical test network id"),
        &alice_id,
        0,
        DataSpaceId::UNIVERSAL,
    )
    .expect("benefit contract address");
    let context = ContractRuntimeExecutionContext {
        contract_subject: contract_address.subject_id(),
        contract_address,
        contract_alias: Some("benefit::benefit".parse().expect("benefit alias")),
        entrypoint: "create_tranche".to_owned(),
    };
    let mut stx = block.transaction();
    let res = executor.execute_instruction_with_contract_runtime_context(
        &mut stx,
        &alice_id,
        instruction,
        Some(&context),
    );
    match res {
        Err(ValidationFail::NotPermitted(msg)) => assert!(
            msg.contains("source asset owner must sign the transaction"),
            "unexpected rejection message: {msg}"
        ),
        other => panic!(
            "non-spend contract runtime context must not bypass asset transfer checks, got: {other:?}"
        ),
    }
}
#[test]
fn contract_runtime_context_alias_does_not_bypass_permission_grant_authorization() {
    let alice_id = ALICE_ID.clone();
    let beneficiary = checked_account_id();
    let domain_id: DomainId = DomainId::try_new("wonderland", "universal").expect("domain id");
    let domain = Domain::new(domain_id).build(&alice_id);
    let alice_account = Account::new(alice_id.clone()).build(&alice_id);
    let beneficiary_account = Account::new(beneficiary.clone()).build(&beneficiary);
    let world = World::with([domain], [alice_account, beneficiary_account], []);
    let state = state_after_genesis(world);
    let header = BlockHeader::new(
        nonzero!(2_u64),
        Some(
            state
                .view()
                .latest_block_hash()
                .expect("genuine genesis parent"),
        ),
        None,
        0,
        0,
    );
    let mut block = state.block(header);
    let raw = data_model_executor::Executor::new(IvmBytecode::from_compiled(
        generate_denied_program("executor denies permission grants"),
    ));
    let executor = super::Executor::UserProvided(
        super::LoadedExecutor::load(raw).expect("load denying executor"),
    );
    let instruction = InstructionBox::from(Grant::account_permission(
        Permission::new("BispSpend".to_owned(), Json::new(())),
        beneficiary.clone(),
    ));
    let contract_address = ContractAddress::derive(
        &"hash:0000000000000000000000000000000000000000000000000000000000000001#C50E"
            .parse()
            .expect("canonical test network id"),
        &alice_id,
        0,
        DataSpaceId::UNIVERSAL,
    )
    .expect("bisp contract address");
    let context = ContractRuntimeExecutionContext {
        contract_subject: contract_address.subject_id(),
        contract_address,
        contract_alias: Some("bisp_bisp::sbp".parse().expect("bisp alias")),
        entrypoint: "create_tranche".to_owned(),
    };
    let mut stx = block.transaction();
    stx.tx_call_hash = Some(Hash::prehashed([0xE7; Hash::LENGTH]));
    let result = executor.execute_instruction_with_contract_runtime_context(
        &mut stx,
        &alice_id,
        instruction,
        Some(&context),
    );
    assert!(
        matches!(
            result,
            Err(ValidationFail::NotPermitted(ref message))
                if message.contains("deployed contract")
        ),
        "contract alias must not bypass the common permission boundary: {result:?}"
    );
}
#[test]
fn initial_executor_contract_alias_never_bypasses_permission_grant_validation() {
    fn execute_case(
        alias: &str,
        entrypoint: &str,
        permission_name: &str,
    ) -> Result<(), ValidationFail> {
        let alice_id = ALICE_ID.clone();
        let beneficiary = checked_account_id();
        let domain_id = DomainId::try_new("wonderland", "universal").expect("domain id");
        let world = World::with(
            [Domain::new(domain_id).build(&alice_id)],
            [
                Account::new(alice_id.clone()).build(&alice_id),
                Account::new(beneficiary.clone()).build(&beneficiary),
            ],
            [],
        );
        let state = state_after_genesis(world);
        let mut block = state.block(BlockHeader::new(nonzero!(2_u64), None, None, 0, 0));
        let contract_address = ContractAddress::derive(
            &"hash:0000000000000000000000000000000000000000000000000000000000000001#C50E"
                .parse()
                .expect("canonical test network id"),
            &alice_id,
            0,
            DataSpaceId::UNIVERSAL,
        )
        .expect("contract address");
        let context = ContractRuntimeExecutionContext {
            contract_subject: contract_address.subject_id(),
            contract_address,
            contract_alias: Some(alias.parse().expect("contract alias")),
            entrypoint: entrypoint.to_owned(),
        };
        let instruction = InstructionBox::from(Grant::account_permission(
            Permission::new(permission_name.to_owned(), Json::new(())),
            beneficiary,
        ));
        super::Executor::Initial.execute_instruction_with_contract_runtime_context(
            &mut block.transaction(),
            &alice_id,
            instruction,
            Some(&context),
        )
    }
    for entrypoint in ["create_tranche", "set_beneficiary_spend_authority"] {
        assert!(matches!(
            execute_case("bisp_bisp::sbp", entrypoint, "BispSpend"),
            Err(ValidationFail::NotPermitted(_))
        ));
    }
    for (alias, entrypoint, permission) in [
        (
            "bisp_bisp::sbp",
            "grant_beneficiary_spend_permission",
            "BispSpend",
        ),
        ("bisp_bisp::sbp", "unrelated", "BispSpend"),
        ("unrelated::sbp", "create_tranche", "BispSpend"),
        ("bisp_bisp::sbp", "create_tranche", "CanSetParameters"),
    ] {
        assert!(
            matches!(
                execute_case(alias, entrypoint, permission),
                Err(ValidationFail::NotPermitted(_))
            ),
            "contract {alias}/{entrypoint} must not grant {permission}"
        );
    }
}
#[test]
fn initial_executor_contract_alias_never_bypasses_transfer_control_validation() {
    fn assert_rejected(result: &Result<(), ValidationFail>, context: &str) {
        assert!(
            matches!(
                result,
                Err(ValidationFail::NotPermitted(_)
                    | ValidationFail::InstructionFailed(
                        InstructionExecutionError::InvariantViolation(_)
                    ))
            ),
            "{context} must be rejected by authorization or the matching execution invariant: {result:?}"
        );
    }
    fn execute_case(
        alias: &str,
        entrypoint: &str,
        instruction_kind: &str,
        window: AssetTransferControlWindow,
    ) -> Result<(), ValidationFail> {
        let caller = ALICE_ID.clone();
        let owner = checked_account_id();
        let target = checked_account_id();
        let domain_id = DomainId::try_new("cbdc", "sbp").expect("domain id");
        let asset_definition_id = AssetDefinitionId::derive_from_components(
            domain_id.clone(),
            "pkr".parse().expect("asset name"),
        );
        let world = World::with(
            [Domain::new(domain_id).build(&owner)],
            [
                Account::new(caller.clone()).build(&caller),
                Account::new(owner.clone()).build(&owner),
                Account::new(target.clone()).build(&target),
            ],
            [AssetDefinition::numeric(
                asset_definition_id.clone(),
                "PKR".to_owned(),
                iroha_data_model::asset::AssetBalancePolicy::Global,
                None,
            )
            .build(&owner)],
        );
        let state = state_after_genesis(world);
        let mut block = state.block(BlockHeader::new(nonzero!(2_u64), None, None, 0, 0));
        let instruction = match instruction_kind {
            "availability" => InstructionBox::from(SetAssetTransferAvailability::new(
                target,
                asset_definition_id,
                0,
                AssetTransferAvailability::Enabled,
                AssetTransferAvailability::Disabled,
                Some("branded contract availability fixture".to_owned()),
            )),
            "limit" => InstructionBox::from(SetAssetTransferControl::new(
                target,
                asset_definition_id,
                vec![iroha_data_model::asset::AssetTransferLimit {
                    window,
                    cap_amount: Some(Quantity::from(100_u32)),
                }],
            )),
            other => panic!("unsupported test instruction kind {other}"),
        };
        let contract_address = ContractAddress::derive(
            &"hash:0000000000000000000000000000000000000000000000000000000000000001#C50E"
                .parse()
                .expect("canonical test network id"),
            &caller,
            0,
            DataSpaceId::UNIVERSAL,
        )
        .expect("contract address");
        let context = ContractRuntimeExecutionContext {
            contract_subject: contract_address.subject_id(),
            contract_address,
            contract_alias: Some(alias.parse().expect("contract alias")),
            entrypoint: entrypoint.to_owned(),
        };
        super::Executor::Initial.execute_instruction_with_contract_runtime_context(
            &mut block.transaction(),
            &caller,
            instruction,
            Some(&context),
        )
    }
    assert_rejected(
        &(execute_case(
            "apps_freeze::sbp",
            "apply_freeze",
            "availability",
            AssetTransferControlWindow::Day,
        )),
        "unprivileged branded freeze",
    );
    assert_rejected(
        &(execute_case(
            "apps_limits_update::sbp",
            "apply_limits",
            "limit",
            AssetTransferControlWindow::Day,
        )),
        "unprivileged branded limit update",
    );
    for (alias, entrypoint, kind, window) in [
        (
            "apps_freeze::sbp",
            "wrong",
            "availability",
            AssetTransferControlWindow::Day,
        ),
        (
            "wrong::sbp",
            "apply_freeze",
            "availability",
            AssetTransferControlWindow::Day,
        ),
        (
            "apps_freeze::sbp",
            "apply_freeze",
            "limit",
            AssetTransferControlWindow::Day,
        ),
        (
            "apps_limits_update::sbp",
            "apply_limits",
            "availability",
            AssetTransferControlWindow::Day,
        ),
        (
            "apps_limits_update::sbp",
            "apply_limits",
            "limit",
            AssetTransferControlWindow::Week,
        ),
    ] {
        assert_rejected(
            &(execute_case(alias, entrypoint, kind, window)),
            &format!("contract {alias}/{entrypoint} must not emit {kind}/{window}"),
        );
    }
}
#[test]
fn initial_executor_denies_transfer_asset_definition_without_ownership() {
    let alice_id = ALICE_ID.clone();
    let users_domain_id: DomainId =
        DomainId::try_new("users", "universal").expect("users domain id");
    let defs_domain_id: DomainId = DomainId::try_new("defs", "universal").expect("defs domain id");
    let user1 = checked_account_id();
    let user2 = checked_account_id();
    let alice_domain_id: DomainId =
        DomainId::try_new("wonderland", "universal").expect("domain id");
    let users_domain = Domain::new(users_domain_id.clone()).build(&user1);
    let defs_domain = Domain::new(defs_domain_id.clone()).build(&user1);
    let alice_domain = Domain::new(alice_domain_id.clone()).build(&alice_id);
    let alice_account = Account::new(alice_id.clone()).build(&alice_id);
    let user1_account = Account::new(user1.clone()).build(&user1);
    let user2_account = Account::new(user2.clone()).build(&user2);
    let asset_definition_id: AssetDefinitionId = AssetDefinitionId::derive_from_components(
        defs_domain_id.clone(),
        "bond".parse().expect("asset definition name"),
    );
    let asset_definition = AssetDefinition::numeric(
        asset_definition_id.clone(),
        "bond".to_owned(),
        iroha_data_model::asset::AssetBalancePolicy::Global,
        None,
    )
    .build(&user1);
    let world = World::with(
        [alice_domain, users_domain, defs_domain],
        [alice_account, user1_account, user2_account],
        [asset_definition],
    );
    let state = state_after_genesis(world);
    let header = BlockHeader::new(
        nonzero!(2_u64),
        Some(
            state
                .view()
                .latest_block_hash()
                .expect("genuine genesis parent"),
        ),
        None,
        0,
        0,
    );
    let mut block = state.block(header);
    let executor = super::Executor::Initial;
    let instruction = InstructionBox::from(Transfer::asset_definition(
        user1.clone(),
        asset_definition_id.clone(),
        user2.clone(),
    ));
    let transfer = extract_transfer_asset_definition(&instruction)
        .expect("expected to extract asset-definition transfer from instruction");
    let mut stx = block.transaction();
    let allowed = can_transfer_asset_definition(&stx.world, &alice_id, &transfer)
        .expect("asset-definition transfer permission check");
    assert!(
        !allowed,
        "alice should not be allowed to transfer user1-owned asset definition"
    );
    let res = executor.execute_instruction(&mut stx, &alice_id, instruction);
    match res {
        Err(ValidationFail::NotPermitted(msg)) => assert!(
            msg.contains("Can't transfer asset definition"),
            "unexpected rejection message: {msg}"
        ),
        other => panic!(
            "initial executor should deny asset-definition transfer from another account, got: {other:?}"
        ),
    }
}
#[test]
fn initial_executor_denies_transfer_asset_definition_by_definition_domain_owner() {
    let alice_id = ALICE_ID.clone();
    let users_domain_id: DomainId =
        DomainId::try_new("users", "universal").expect("users domain id");
    let defs_domain_id: DomainId = DomainId::try_new("defs", "universal").expect("defs domain id");
    let user1 = checked_account_id();
    let user2 = checked_account_id();
    let alice_domain_id: DomainId =
        DomainId::try_new("wonderland", "universal").expect("domain id");
    let users_domain = Domain::new(users_domain_id.clone()).build(&user1);
    let defs_domain = Domain::new(defs_domain_id.clone()).build(&alice_id);
    let alice_domain = Domain::new(alice_domain_id.clone()).build(&alice_id);
    let alice_account = Account::new(alice_id.clone()).build(&alice_id);
    let user1_account = Account::new(user1.clone()).build(&user1);
    let user2_account = Account::new(user2.clone()).build(&user2);
    let asset_definition_id: AssetDefinitionId = AssetDefinitionId::derive_from_components(
        defs_domain_id.clone(),
        "bond".parse().expect("asset definition name"),
    );
    let asset_definition = AssetDefinition::numeric(
        asset_definition_id.clone(),
        "bond".to_owned(),
        iroha_data_model::asset::AssetBalancePolicy::Global,
        None,
    )
    .build(&user1);
    let world = World::with(
        [alice_domain, users_domain, defs_domain],
        [alice_account, user1_account, user2_account],
        [asset_definition],
    );
    let state = state_after_genesis(world);
    let header = BlockHeader::new(
        nonzero!(2_u64),
        Some(
            state
                .view()
                .latest_block_hash()
                .expect("genuine genesis parent"),
        ),
        None,
        0,
        0,
    );
    let mut block = state.block(header);
    let instruction = InstructionBox::from(Transfer::asset_definition(
        user1.clone(),
        asset_definition_id.clone(),
        user2.clone(),
    ));
    let transfer = extract_transfer_asset_definition(&instruction)
        .expect("expected to extract asset-definition transfer from instruction");
    let mut stx = block.transaction();
    let allowed = can_transfer_asset_definition(&stx.world, &alice_id, &transfer)
        .expect("asset-definition transfer permission check");
    assert!(
        !allowed,
        "definition-domain ownership must not authorize transfer without source ownership"
    );
    let res = super::Executor::Initial.execute_instruction(&mut stx, &alice_id, instruction);
    match res {
        Err(ValidationFail::NotPermitted(msg)) => assert!(
            msg.contains("Can't transfer asset definition"),
            "unexpected rejection message: {msg}"
        ),
        other => panic!(
            "initial executor should deny asset-definition transfer by non-source owner, got: {other:?}"
        ),
    }
}
#[test]
fn initial_executor_denies_transfer_nft_without_ownership() {
    let alice_id = ALICE_ID.clone();
    let users_domain_id: DomainId =
        DomainId::try_new("users", "universal").expect("users domain id");
    let user1 = checked_account_id();
    let user2 = checked_account_id();
    let alice_domain_id: DomainId =
        DomainId::try_new("wonderland", "universal").expect("domain id");
    let users_domain = Domain::new(users_domain_id.clone()).build(&user1);
    let alice_domain = Domain::new(alice_domain_id.clone()).build(&alice_id);
    let alice_account = Account::new(alice_id.clone()).build(&alice_id);
    let user1_account = Account::new(user1.clone()).build(&user1);
    let user2_account = Account::new(user2.clone()).build(&user2);
    let nft_id: NftId = "ticket$users.universal".parse().expect("nft id");
    let nft = Nft::new(nft_id.clone(), Metadata::default()).build(&user1);
    let world = World::with_assets(
        [alice_domain, users_domain],
        [alice_account, user1_account, user2_account],
        [],
        [],
        [nft],
    );
    let state = state_after_genesis(world);
    let header = BlockHeader::new(
        nonzero!(2_u64),
        Some(
            state
                .view()
                .latest_block_hash()
                .expect("genuine genesis parent"),
        ),
        None,
        0,
        0,
    );
    let mut block = state.block(header);
    let executor = super::Executor::Initial;
    let instruction =
        InstructionBox::from(Transfer::nft(user1.clone(), nft_id.clone(), user2.clone()));
    let transfer = extract_transfer_nft(&instruction)
        .expect("expected to extract nft transfer from instruction");
    let mut stx = block.transaction();
    let allowed =
        can_transfer_nft(&stx.world, &alice_id, &transfer).expect("nft transfer permission check");
    assert!(
        !allowed,
        "alice should not be allowed to transfer user1-owned nft"
    );
    let res = executor.execute_instruction(&mut stx, &alice_id, instruction);
    match res {
        Err(ValidationFail::NotPermitted(msg)) => assert!(
            msg.contains("Can't transfer NFT"),
            "unexpected rejection message: {msg}"
        ),
        other => {
            panic!("initial executor should deny nft transfer from another account, got: {other:?}")
        }
    }
}
#[test]
fn initial_executor_allows_transfer_nft_by_nft_domain_owner() {
    let alice_id = ALICE_ID.clone();
    let users_domain_id: DomainId =
        DomainId::try_new("users", "universal").expect("users domain id");
    let user1 = checked_account_id();
    let user2 = checked_account_id();
    let alice_domain_id: DomainId =
        DomainId::try_new("wonderland", "universal").expect("domain id");
    let users_domain = Domain::new(users_domain_id.clone()).build(&alice_id);
    let alice_domain = Domain::new(alice_domain_id.clone()).build(&alice_id);
    let alice_account = Account::new(alice_id.clone()).build(&alice_id);
    let user1_account = Account::new(user1.clone()).build(&user1);
    let user2_account = Account::new(user2.clone()).build(&user2);
    let nft_id: NftId = "ticket$users.universal".parse().expect("nft id");
    let nft = Nft::new(nft_id.clone(), Metadata::default()).build(&user1);
    let world = World::with_assets(
        [alice_domain, users_domain],
        [alice_account, user1_account, user2_account],
        [],
        [],
        [nft],
    );
    let state = state_after_genesis(world);
    let header = BlockHeader::new(
        nonzero!(2_u64),
        Some(
            state
                .view()
                .latest_block_hash()
                .expect("genuine genesis parent"),
        ),
        None,
        0,
        0,
    );
    let mut block = state.block(header);
    let instruction =
        InstructionBox::from(Transfer::nft(user1.clone(), nft_id.clone(), user2.clone()));
    let transfer = extract_transfer_nft(&instruction)
        .expect("expected to extract nft transfer from instruction");
    let mut stx = block.transaction();
    let allowed =
        can_transfer_nft(&stx.world, &alice_id, &transfer).expect("nft transfer permission check");
    assert!(
        allowed,
        "nft-domain owner should be allowed to transfer ownership"
    );
    let res = super::Executor::Initial.execute_instruction(&mut stx, &alice_id, instruction);
    assert!(res.is_ok(), "expected transfer to succeed, got {res:?}");
}
#[test]
fn initial_executor_denies_nft_metadata_edit_in_transaction() {
    let (bob_id, bob_kp) = gen_account_in("wonderland");
    let domain_id: DomainId = DomainId::try_new("wonderland", "universal").expect("domain id");
    let domain: Domain = Domain::new(domain_id.clone()).build(&ALICE_ID);
    let alice_account = Account::new(ALICE_ID.clone()).build(&ALICE_ID);
    let bob_account = Account::new(bob_id.clone()).build(&bob_id);
    let nft_id: NftId = "nft_owner_modify$wonderland.universal"
        .parse()
        .expect("nft id");
    let nft = Nft::new(nft_id.clone(), Metadata::default()).build(&bob_id);
    let world = World::with_assets([domain], [alice_account, bob_account], [], [], [nft]);
    let kura = Kura::blank_kura_for_testing();
    let query_handle = query::store::LiveQueryStore::start_test();
    let chain: ChainId = "test-chain".parse().unwrap();
    let state = State::new_with_chain(world, kura, query_handle, chain);
    let header = BlockHeader::new(nonzero!(2_u64), None, None, 0, 0);
    let mut block = state.block(header);
    let instruction = SetKeyValue::nft(nft_id, "foo".parse().expect("key"), "value");
    let tx = TransactionBuilder::new(
        state.network_id,
        bob_id.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([instruction])
    .sign(bob_kp.private_key());
    let executor = super::Executor::Initial;
    let mut ivm_cache = crate::smartcontracts::ivm::cache::IvmCache::new();
    let mut stx = block.transaction_for_fastpq_testing(Hash::from(tx.hash_as_entrypoint()));
    let res = executor
        .execute_transaction(&mut stx, &bob_id, &tx, &mut ivm_cache)
        .map_err(crate::execution_attempt::expect_completed_rejection);
    assert!(
        matches!(res, Err(ValidationFail::NotPermitted(_))),
        "initial executor should deny NFT metadata edits by non-domain owners"
    );
}
#[test]
fn bench_profile_runs_without_logger() {
    let authority = ALICE_ID.clone();
    let account = Account::new(authority.clone()).build(&authority);
    let world = World::with([], [account], []);
    let state = state_after_genesis(world);
    let mut block = state.block(BlockHeader::new(
        nonzero!(2_u64),
        Some(
            state
                .view()
                .latest_block_hash()
                .expect("genuine genesis parent"),
        ),
        None,
        0,
        0,
    ));
    let mut tx = block.transaction();
    let executor = super::Executor::default();
    let instr: InstructionBox = Log::new(Level::INFO, "bench profile".to_owned()).into();
    executor
        .execute_instruction_with_profile(
            &mut tx,
            &authority,
            instr,
            InstructionExecutionProfile::Bench,
        )
        .expect("bench profile should execute without logger");
}
#[test]
fn multisig_account_direct_signing_is_rejected() {
    assert_multisig_direct_signed_transaction_is_rejected(Vec::new());
}
#[test]
fn multisig_account_direct_signing_custom_envelope_is_rejected() {
    assert_multisig_direct_signed_transaction_is_rejected(vec![
        CustomInstruction::new("multisig-envelope").into(),
    ]);
}
fn assert_multisig_direct_signed_transaction_is_rejected(instructions: Vec<InstructionBox>) {
    let domain_id: DomainId = DomainId::try_new("wonderland", "universal").expect("domain id");
    let signer = checked_keypair();
    let member = iroha_data_model::account::MultisigMember::new(signer.public_key().clone(), 1)
        .expect("valid member");
    let policy = iroha_data_model::account::MultisigPolicy::new(1, vec![member]).expect("policy");
    let multisig_id = AccountId::new_multisig(policy);
    let domain: Domain = Domain::new(domain_id.clone()).build(&multisig_id);
    let multisig_account = Account::new(multisig_id.clone()).build(&multisig_id);
    let world = World::with([domain], [multisig_account], []);
    let state = state_after_genesis(world);
    let block_header = BlockHeader::new(
        nonzero!(2_u64),
        Some(
            state
                .view()
                .latest_block_hash()
                .expect("genuine genesis parent"),
        ),
        None,
        0,
        0,
    );
    let mut block = state.block(block_header);
    let tx = TransactionBuilder::new(
        state.network_id,
        multisig_id.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_executable(Executable::Instructions(instructions.into()))
    .sign_multisig([signer.private_key()]);
    tx.verify_signature()
        .expect("direct-signing rejection must use a valid multisig signature bundle");
    let executor = super::Executor::Initial;
    let mut ivm_cache = crate::smartcontracts::ivm::cache::IvmCache::new();
    let mut stx = block.transaction_for_fastpq_testing(Hash::from(tx.hash_as_entrypoint()));
    stx.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
    stx.world.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
    let res = executor
        .execute_transaction(&mut stx, &multisig_id, &tx, &mut ivm_cache)
        .map_err(crate::execution_attempt::expect_completed_rejection);
    match res {
        Err(ValidationFail::NotPermitted(msg)) => assert!(
            msg.contains("direct signing with multisig accounts is forbidden"),
            "unexpected message: {msg}"
        ),
        other => panic!("expected multisig direct signing rejection, got {other:?}"),
    }
    #[cfg(feature = "telemetry")]
    {
        assert_eq!(
            stx.telemetry
                .metrics_ref()
                .multisig_direct_sign_reject_total
                .get(),
            1
        );
    }
}
// Shared test helpers for generating or loading executor bytecode
fn read_default_bytecode() -> Option<Vec<u8>> {
    std::env::var_os("IROHA_TEST_USE_DEFAULT_EXECUTOR")?;
    let path1 =
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../defaults/executor.to");
    if let Ok(b) = std::fs::read(&path1) {
        return Some(b);
    }
    if let Ok(b) = std::fs::read("defaults/executor.to") {
        return Some(b);
    }
    None
}
fn generate_migration_program(
    verdict: &Result<ExecutorDataModel, iroha_data_model::ValidationFail>,
) -> Vec<u8> {
    use norito::codec::Encode as _;
    let payload = match verdict {
        Ok(model) => MigrationResultPayload::Ok(model.clone()),
        Err(err) => MigrationResultPayload::Err(err.clone()),
    };
    let verdict_bytes = payload.encode();
    build_program_from_encoded_result(&verdict_bytes)
}
fn generate_ok_program() -> Vec<u8> {
    let verdict = Ok(());
    generate_verdict_program(&verdict)
}
fn executor_result_test_context() -> ExecutorContext {
    ExecutorContext {
        authority: ALICE_ID.clone(),
        curr_block: BlockHeader::new(nonzero!(1_u64), None, None, 0, 0),
    }
}
fn loaded_executor_with_result_prefix(declared_len: u64, encoded_payload: &[u8]) -> LoadedExecutor {
    let mut program = build_program_from_encoded_result(encoded_payload);
    let parsed = ivm::ProgramMetadata::parse(&program).expect("parse executor test program");
    let literal_section = parsed
        .literal_section
        .expect("encoded result program has a literal section");
    program[literal_section.data_start..literal_section.data_start + 8]
        .copy_from_slice(&declared_len.to_le_bytes());
    let raw = data_model_executor::Executor::new(IvmBytecode::from_compiled(program));
    LoadedExecutor::load(raw).expect("load executor test program")
}
#[test]
fn executor_pool_eviction_keeps_active_borrower_and_releases_indexes() {
    let loaded = LoadedExecutor::load(data_model_executor::Executor::new(
        IvmBytecode::from_compiled(generate_ok_program()),
    ))
    .unwrap();
    let parameters = iroha_data_model::parameter::SmartContractParameters::default();
    let mut lease = loaded
        .checkout_runtime_for_gas_limit(
            parameters.fuel().get(),
            parameters.memory().get(),
            &executor_test_budget(),
        )
        .unwrap();
    {
        let mut pool = loaded.runtime_pool.lock().unwrap();
        pool.clear_storage();
        assert_eq!(pool.index_memory.bytes(), 0);
        assert_eq!(pool.order.capacity(), 0);
        assert!(pool.variants.is_empty());
    }
    lease.memory.preload_input(0, &[0x5a]).unwrap();
    assert_eq!(
        lease.memory.load_region(Memory::INPUT_START, 1).unwrap(),
        &[0x5a]
    );
    drop(lease);
    assert!(
        loaded.runtime_pool.lock().unwrap().variants.is_empty(),
        "an evicted borrower cannot republish its stale pool identity"
    );
    loaded
        .checkout_runtime_for_gas_limit(
            parameters.fuel().get(),
            parameters.memory().get(),
            &executor_test_budget(),
        )
        .unwrap();
}
#[test]
fn executor_pool_unwind_returns_reset_runtime_and_zero_capacity_stays_cold() {
    let loaded = LoadedExecutor::load(data_model_executor::Executor::new(
        IvmBytecode::from_compiled(generate_ok_program()),
    ))
    .unwrap();
    let parameters = iroha_data_model::parameter::SmartContractParameters::default();
    assert!(
        iroha_panic_hook::catch_unwind_suppressed(std::panic::AssertUnwindSafe(|| {
            let mut lease = loaded
                .checkout_runtime_for_gas_limit(
                    parameters.fuel().get(),
                    parameters.memory().get(),
                    &executor_test_budget(),
                )
                .unwrap();
            lease.memory.preload_input(0, &[0x5a]).unwrap();
            panic!("unwind through executor lease");
        }))
        .is_err()
    );
    let lease = loaded
        .checkout_runtime_for_gas_limit(
            parameters.fuel().get(),
            parameters.memory().get(),
            &executor_test_budget(),
        )
        .unwrap();
    assert_eq!(
        lease.memory.load_region(Memory::INPUT_START, 1).unwrap(),
        &[0]
    );
    drop(lease);
    {
        let mut pool = loaded.runtime_pool.lock().unwrap();
        pool.clear_storage();
        pool.capacity = 0;
    }
    for _ in 0..2 {
        let mut lease = loaded
            .checkout_runtime_for_gas_limit(
                parameters.fuel().get(),
                parameters.memory().get(),
                &executor_test_budget(),
            )
            .unwrap();
        lease.set_register(10, Memory::OUTPUT_START);
        lease.run().unwrap();
        let verdict: Result<(), ValidationFail> = Ok(());
        assert_eq!(
            executor_output_payload(&lease, Memory::OUTPUT_START, "verdict").unwrap(),
            verdict.encode(),
            "cold execution must still publish the complete encoded verdict"
        );
        drop(lease);
        let pool = loaded.runtime_pool.lock().unwrap();
        assert!(pool.variants.is_empty());
        assert_eq!(pool.index_memory.bytes(), 0);
    }
}
#[test]
fn overlapping_executor_runtimes_return_with_their_own_baselines() {
    if executor_cache_test_in_child("overlapping_executor_runtimes_return_with_their_own_baselines")
    {
        return;
    }
    // Hold the real default retention policy while asserting cache reuse;
    // other State fixtures may deliberately configure a cold-only budget.
    let _cache_limits = ivm::ivm_cache::CacheLimitsGuard::new(ivm::ivm_cache::CacheLimits {
        capacity: iroha_config::parameters::defaults::pipeline::CACHE_SIZE,
        max_bytes: iroha_config::parameters::defaults::pipeline::IVM_CACHE_MAX_BYTES,
        max_decoded_ops: iroha_config::parameters::defaults::pipeline::IVM_CACHE_MAX_DECODED_OPS,
    });
    let verdict: Result<(), ValidationFail> = Ok(());
    let encoded = verdict.encode();
    let loaded = loaded_executor_with_result_prefix(
        EXECUTOR_LENGTH_PREFIX_BYTES_U64 + u64::try_from(encoded.len()).expect("bounded verdict"),
        &encoded,
    );
    let parameters = iroha_data_model::parameter::SmartContractParameters::default();
    // Keep the two immutable baselines and the returned VM within the real
    // aggregate retention budget while the first VM remains borrowed. The
    // maximum-stack default can legitimately force this overlap cold.
    let gas_limit = 10_000;
    let heap_limit = parameters.memory().get();
    loaded.runtime_pool.lock().unwrap().clear_storage();
    drop(
        loaded
            .checkout_runtime_for_gas_limit(gas_limit, heap_limit, &executor_test_budget())
            .expect("warm bounded-stack executor runtime"),
    );
    let (before, variants) = loaded.runtime_pool_snapshot();
    assert_eq!(variants, 1);
    let first = loaded
        .checkout_runtime_for_gas_limit(gas_limit, heap_limit, &executor_test_budget())
        .expect("first pooled executor runtime");
    let second_allocation = {
        let mut second = loaded
            .checkout_runtime_for_gas_limit(gas_limit, heap_limit, &executor_test_budget())
            .expect("overlapping cold executor runtime");
        let allocation = second
            .memory
            .load_region(0, 1)
            .expect("second code memory")
            .as_ptr();
        second
            .memory
            .preload_input(0, &[0xA5])
            .expect("dirty second input");
        allocation
    };
    let third = loaded
        .checkout_runtime_for_gas_limit(gas_limit, heap_limit, &executor_test_budget())
        .expect("second runtime must be reusable while first remains leased");
    assert_eq!(
        third
            .memory
            .load_region(0, 1)
            .expect("third code memory")
            .as_ptr(),
        second_allocation
    );
    assert_eq!(
        third
            .memory
            .load_region(Memory::INPUT_START, 1)
            .expect("third input memory"),
        [0]
    );
    let (stats, variants) = loaded.runtime_pool_snapshot();
    assert_eq!(variants, 1);
    assert_eq!(stats.hits - before.hits, 2);
    assert_eq!(stats.misses - before.misses, 1);
    assert_eq!(stats.program_loads - before.program_loads, 1);
    assert_eq!(stats.template_builds - before.template_builds, 1);
    assert_eq!(stats.dirty_resets - before.dirty_resets, 1);
    drop((third, first));
}
fn loaded_executor_returning_past_heap_result() -> LoadedExecutor {
    let metadata = ivm::ProgramMetadata {
        version_major: 1,
        version_minor: 1,
        mode: 0,
        vector_length: 0,
        max_cycles: 100_000,
        abi_version: 1,
    };
    let mut program = metadata.encode();
    let mut displacement = Memory::HEAP_MAX_SIZE - EXECUTOR_LENGTH_PREFIX_BYTES_U64;
    while displacement != 0 {
        let chunk = displacement.min(127);
        program.extend_from_slice(
            &ivm::encoding::wide::encode_ri(
                ivm::instruction::wide::arithmetic::ADDI,
                10,
                10,
                i8::try_from(chunk).expect("bounded ADDI chunk"),
            )
            .to_le_bytes(),
        );
        displacement -= chunk;
    }
    program.extend_from_slice(
        &ivm::encoding::wide::encode_ri(ivm::instruction::wide::arithmetic::ADDI, 11, 0, 0)
            .to_le_bytes(),
    );
    program.extend_from_slice(
        &ivm::encoding::wide::encode_ri(ivm::instruction::wide::arithmetic::ADDI, 11, 11, 16)
            .to_le_bytes(),
    );
    program.extend_from_slice(
        &ivm::encoding::wide::encode_store(ivm::instruction::wide::memory::STORE64, 10, 11, 0)
            .to_le_bytes(),
    );
    program.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());
    let raw = data_model_executor::Executor::new(IvmBytecode::from_compiled(program));
    LoadedExecutor::load(raw).expect("load past-heap executor test program")
}
fn assert_executor_result_length_rejected_by_validation_and_migration(
    declared_len: u64,
    expected_message: &str,
) {
    let loaded = loaded_executor_with_result_prefix(declared_len, &[]);
    let context = executor_result_test_context();
    let validation_payload = ValidatePayload {
        context: context.clone(),
        target: 0_u8,
    };
    let validation_result = run_executor_validation(
        &loaded,
        &validation_payload,
        "hostile-output-test",
        1_000_000,
        Memory::HEAP_MAX_SIZE,
        &executor_test_budget(),
    );
    let validation_error = match validation_result {
        Err(error) => error,
        Ok(_) => panic!("hostile validation result must be rejected"),
    };
    assert!(
        validation_error.to_string().contains(expected_message),
        "unexpected validation error: {validation_error}"
    );
    let migration_error = run_executor_migration(
        &loaded,
        &context,
        1_000_000,
        Memory::HEAP_MAX_SIZE,
        &executor_test_budget(),
    )
    .expect_err("hostile migration result must be rejected");
    assert!(
        migration_error.to_string().contains(expected_message),
        "unexpected migration error: {migration_error}"
    );
}
#[test]
fn executor_result_reader_rejects_short_and_gigabyte_prefixes_in_both_paths() {
    assert_executor_result_length_rejected_by_validation_and_migration(
        EXECUTOR_LENGTH_PREFIX_BYTES_U64 - 1,
        "shorter than its fixed u64 length prefix",
    );
    assert_executor_result_length_rejected_by_validation_and_migration(
        u64::from(u32::MAX),
        "length exceeds the 1048576-byte limit",
    );
    assert_executor_result_length_rejected_by_validation_and_migration(
        1_u64 << 32,
        "length exceeds the 1048576-byte limit",
    );
}
#[test]
fn executor_result_reader_rejects_ranges_past_readable_memory_in_both_paths() {
    let loaded = loaded_executor_returning_past_heap_result();
    let context = executor_result_test_context();
    let validation_result = run_executor_validation(
        &loaded,
        &ValidatePayload {
            context: context.clone(),
            target: 0_u8,
        },
        "past-heap-output-test",
        100_000,
        Memory::HEAP_MAX_SIZE,
        &executor_test_budget(),
    );
    let validation_error = match validation_result {
        Err(error) => error,
        Ok(_) => panic!("past-heap validation result must be rejected"),
    };
    assert!(
        validation_error
            .to_string()
            .contains("is not fully readable"),
        "unexpected validation error: {validation_error}"
    );
    let migration_error = run_executor_migration(
        &loaded,
        &context,
        100_000,
        Memory::HEAP_MAX_SIZE,
        &executor_test_budget(),
    )
    .expect_err("past-heap migration result must be rejected");
    assert!(
        migration_error
            .to_string()
            .contains("is not fully readable"),
        "unexpected migration error: {migration_error}"
    );
}
#[test]
fn migration_result_requires_a_canonical_complete_payload() {
    let discriminant_only = [0_u8; 4];
    let mut discriminant_only_slice = discriminant_only.as_slice();
    assert!(
        MigrationResultPayload::decode(&mut discriminant_only_slice).is_err(),
        "an Ok discriminant without an ExecutorDataModel must not decode"
    );
    let canonical_unit = MigrationUnitPayload::Ok(()).encode();
    assert_ne!(
        canonical_unit.as_slice(),
        discriminant_only.as_slice(),
        "canonical unit success includes its Norito payload framing"
    );
    let context = executor_result_test_context();
    let declared_len = EXECUTOR_LENGTH_PREFIX_BYTES_U64
        + u64::try_from(canonical_unit.len()).expect("bounded migration result");
    let unit_success = loaded_executor_with_result_prefix(declared_len, canonical_unit.as_slice());
    assert_eq!(
        run_executor_migration(
            &unit_success,
            &context,
            1_000_000,
            Memory::HEAP_MAX_SIZE,
            &executor_test_budget()
        )
        .expect("a canonical unit-success migration result must be accepted"),
        None,
        "the unit-success payload must not install an empty data model"
    );
    let model = initial_executor_data_model_fallback();
    let canonical = MigrationResultPayload::Ok(model.clone()).encode();
    let declared_len = EXECUTOR_LENGTH_PREFIX_BYTES_U64
        + u64::try_from(canonical.len()).expect("bounded migration result");
    let complete = loaded_executor_with_result_prefix(declared_len, canonical.as_slice());
    assert_eq!(
        run_executor_migration(
            &complete,
            &context,
            1_000_000,
            Memory::HEAP_MAX_SIZE,
            &executor_test_budget()
        )
        .expect("a canonical complete migration result must be accepted"),
        Some(model)
    );
    let mut non_canonical = canonical;
    non_canonical.push(0);
    let declared_len = EXECUTOR_LENGTH_PREFIX_BYTES_U64
        + u64::try_from(non_canonical.len()).expect("bounded migration result");
    let trailing = loaded_executor_with_result_prefix(declared_len, non_canonical.as_slice());
    let trailing_error = run_executor_migration(
        &trailing,
        &context,
        1_000_000,
        Memory::HEAP_MAX_SIZE,
        &executor_test_budget(),
    )
    .expect_err("a migration result with trailing bytes must be rejected");
    assert!(
        trailing_error
            .to_string()
            .contains("undecodable or non-canonical"),
        "unexpected non-canonical migration result error: {trailing_error}"
    );
}
#[test]
fn executor_result_reader_preserves_legitimate_validation_and_migration_results() {
    let context = executor_result_test_context();
    let validation_verdict: Result<(), ValidationFail> = Ok(());
    let validation = loaded_executor_with_result_prefix(
        EXECUTOR_LENGTH_PREFIX_BYTES_U64
            + u64::try_from(validation_verdict.encode().len()).expect("bounded verdict"),
        &validation_verdict.encode(),
    );
    let report = run_executor_validation(
        &validation,
        &ValidatePayload {
            context: context.clone(),
            target: 0_u8,
        },
        "legitimate-output-test",
        1_000_000,
        Memory::HEAP_MAX_SIZE,
        &executor_test_budget(),
    )
    .expect("legitimate validation result is readable");
    assert!(report.verdict.is_ok());
    let migration_verdict = MigrationUnitPayload::Ok(()).encode();
    let migration = loaded_executor_with_result_prefix(
        EXECUTOR_LENGTH_PREFIX_BYTES_U64
            + u64::try_from(migration_verdict.len()).expect("bounded migration result"),
        &migration_verdict,
    );
    assert_eq!(
        run_executor_migration(
            &migration,
            &context,
            1_000_000,
            Memory::HEAP_MAX_SIZE,
            &executor_test_budget()
        )
        .expect("legitimate migration result is readable"),
        None
    );
}
fn contract_program_with_entrypoint(entrypoint: &str, permission: Option<&str>) -> (Vec<u8>, u64) {
    use iroha_data_model::smart_contract::manifest::EntryPointKind;
    contract_program_with_entrypoint_kind(entrypoint, EntryPointKind::Kotoage, permission)
}
fn contract_program_with_entrypoint_kind(
    entrypoint: &str,
    kind: iroha_data_model::smart_contract::manifest::EntryPointKind,
    permission: Option<&str>,
) -> (Vec<u8>, u64) {
    use ivm::{EmbeddedContractInterfaceV1, EmbeddedEntrypointDescriptor, ProgramMetadata};
    let descriptor = EmbeddedEntrypointDescriptor {
        name: entrypoint.to_owned(),
        kind,
        params: Vec::new(),
        argument_schema: None,
        return_type: Some("()".to_owned()),
        return_schema: Some(
            iroha_data_model::smart_contract::entrypoint::EntrypointValueTypeV1 {
                nodes: vec![
                    iroha_data_model::smart_contract::entrypoint::EntrypointValueTypeNodeV1::Unit,
                ],
            },
        ),
        authorization: if matches!(
            kind,
            iroha_data_model::smart_contract::manifest::EntryPointKind::Hajimari
                | iroha_data_model::smart_contract::manifest::EntryPointKind::Kaizen
        ) {
            EntrypointAuthorizationV1::RuntimeLifecycle
        } else {
            permission.map_or(EntrypointAuthorizationV1::Anyone, |name| {
                EntrypointAuthorizationV1::Permission(name.parse().unwrap())
            })
        },
        read_keys: Vec::new(),
        write_keys: Vec::new(),
        access_hints_complete: Some(true),
        access_hints_skipped: Vec::new(),
        triggers: Vec::new(),
        entry_pc: 0,
    };
    let interface = EmbeddedContractInterfaceV1 {
        events: Vec::new(),
        enum_types: Vec::new(),
        permissions: permission
            .into_iter()
            .map(|name| {
                iroha_data_model::smart_contract::manifest::ContractPermissionDescriptorV1 {
                    name: name.parse().unwrap(),
                    scope: ContractPermissionScopeV1::Instance,
                }
            })
            .collect(),
        callables: vec![crate::ivm_test_support::unit_callable(0)],
        seiyaku_name: "TestContract".to_owned(),
        compiler_fingerprint: "executor-test".to_owned(),
        abi_hash: ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1),
        features_bitmap: 0,
        access_set_hints: None,
        kotoba: Vec::new(),
        entrypoints: vec![descriptor],
        error_messages: Vec::new(),
        error_types: Vec::new(),
        states: Vec::new(),
    };
    let interface_section = interface.encode_section();
    let expected_entrypoint_pc =
        u64::try_from(interface_section.len()).expect("section length fits u64");
    let metadata = ProgramMetadata {
        version_major: 1,
        version_minor: 1,
        mode: 0,
        vector_length: 0,
        max_cycles: 1_000_000,
        abi_version: 1,
    };
    let mut program = metadata.encode();
    program.extend_from_slice(&interface_section);
    program.extend_from_slice(&crate::ivm_test_support::unit_return());
    (program, expected_entrypoint_pc)
}
fn contract_program_with_private_input_entrypoint(
    entrypoint: &str,
    kind: iroha_data_model::smart_contract::manifest::EntryPointKind,
) -> Vec<u8> {
    use ivm::{EmbeddedContractInterfaceV1, EmbeddedEntrypointDescriptor, ProgramMetadata};
    let descriptor = EmbeddedEntrypointDescriptor {
        name: entrypoint.to_owned(),
        kind,
        params: Vec::new(),
        argument_schema: None,
        return_type: Some("()".to_owned()),
        return_schema: Some(
            iroha_data_model::smart_contract::entrypoint::EntrypointValueTypeV1 {
                nodes: vec![
                    iroha_data_model::smart_contract::entrypoint::EntrypointValueTypeNodeV1::Unit,
                ],
            },
        ),
        authorization: if matches!(
            kind,
            iroha_data_model::smart_contract::manifest::EntryPointKind::Hajimari
                | iroha_data_model::smart_contract::manifest::EntryPointKind::Kaizen
        ) {
            EntrypointAuthorizationV1::RuntimeLifecycle
        } else {
            EntrypointAuthorizationV1::Anyone
        },
        read_keys: Vec::new(),
        write_keys: Vec::new(),
        access_hints_complete: Some(true),
        access_hints_skipped: Vec::new(),
        triggers: Vec::new(),
        entry_pc: 0,
    };
    let interface = EmbeddedContractInterfaceV1 {
        events: Vec::new(),
        enum_types: Vec::new(),
        permissions: Vec::new(),
        callables: vec![crate::ivm_test_support::unit_callable(0)],
        seiyaku_name: "PrivateInputContract".to_owned(),
        compiler_fingerprint: "executor-private-input-test".to_owned(),
        abi_hash: ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1),
        features_bitmap: ivm::CONTRACT_FEATURE_BIT_ZK,
        access_set_hints: None,
        kotoba: Vec::new(),
        entrypoints: vec![descriptor],
        error_messages: Vec::new(),
        error_types: Vec::new(),
        states: Vec::new(),
    };
    let metadata = ProgramMetadata {
        version_major: 1,
        version_minor: 1,
        mode: ivm::ivm_mode::ZK,
        vector_length: 0,
        max_cycles: 1_000_000,
        abi_version: 1,
    };
    let mut program = metadata.encode();
    program.extend_from_slice(&interface.encode_section());
    program.extend_from_slice(
        &ivm::encoding::wide::encode_sys(
            ivm::instruction::wide::system::SCALL,
            u8::try_from(ivm::syscalls::SYSCALL_GET_PRIVATE_INPUT).expect("fixture value fits u8"),
        )
        .to_le_bytes(),
    );
    program.extend_from_slice(&crate::ivm_test_support::unit_return());
    program
}
fn prepared_parameterized_trigger_contract() -> ivm::PreparedContract {
    let source = r"
seiyaku TriggerArguments { permission Admin;
  kotoage fn run(quantity val) authorize(Admin) {
    let _val = val;
  }
}
";
    let code = kotodama_lang::compiler::Compiler::new()
        .compile_source(source)
        .expect("compile parameterized trigger callback");
    ivm::prepare_contract(Arc::<[u8]>::from(code)).expect("prepare parameterized trigger callback")
}
#[test]
fn contract_call_enforces_entrypoint_and_hold_before_argument_decode() {
    const REQUIRED_PERMISSION: &str = "CanInvokeContractEntrypoint";
    let manifest_signing = crate::manifest_signing_test_support::ManifestSigningFixture::new();
    let (program, manifest) = kotodama_lang::compiler::Compiler::new()
        .compile_source_with_manifest(
            r#"
seiyaku GuardedValue { permission CanInvokeContractEntrypoint;
  kotoage fn write(int value) authorize(CanInvokeContractEntrypoint) {
    ledger::account::set_metadata(
      account: context::authority(),
      key: Name::parse("guarded_value"),
      value: Json::parse("{\"authorized\":true}")
    );
  }
}
"#,
        )
        .expect("compile parameterized protected contract");
    let parsed = ivm::ProgramMetadata::parse(&program).expect("parse protected contract");
    let schema = parsed
        .contract_interface
        .as_ref()
        .and_then(|interface| {
            interface
                .entrypoints
                .iter()
                .find(|entry| entry.name == "write")
        })
        .and_then(|entry| entry.argument_schema.as_ref())
        .expect("write argument schema");
    let arguments = ivm_abi::arguments::encode_argument_record_from_json(
        schema,
        &Json::from(norito::json!({ "value": "7" })),
    )
    .expect("encode valid protected arguments");
    let arguments =
        iroha_data_model::transaction::executable::ContractArgumentRecord::try_new(arguments)
            .expect("bounded protected arguments");
    let authority = ALICE_ID.clone();
    let domain =
        Domain::new(DomainId::try_new("wonderland", "universal").expect("valid domain id"))
            .build(&authority);
    let account = Account::new(authority.clone()).build(&authority);
    let state = state_after_genesis(World::with([domain], [account], []));
    let code_hash = ivm::contract_code_hash(&program);
    let contract_address =
        ContractAddress::derive(&state.network_id, &authority, 0, DataSpaceId::UNIVERSAL)
            .expect("derive contract address");
    let metadata_marker: Name = "guarded_value"
        .parse()
        .expect("valid direct-call metadata marker");
    let mut world = state.world.block();
    world.contract_code.insert(
        iroha_data_model::smart_contract::ContractArtifactId::new(
            contract_address.dataspace_id().unwrap(),
            code_hash,
        ),
        program.clone(),
    );
    world.contract_manifests.insert(
        iroha_data_model::smart_contract::ContractArtifactId::new(
            contract_address.dataspace_id().unwrap(),
            code_hash,
        ),
        manifest
            .try_signed(
                manifest_signing.context(),
                manifest_signing.max_frame_bytes(),
                &ALICE_KEYPAIR,
            )
            .expect("sign bounded fixture manifest"),
    );
    // The invoked contract's subject needs its own exact grant for caller metadata.
    world.account_permissions.insert(
        contract_address.subject_id(),
        BTreeSet::from([executor_permission::account::CanModifyAccountMetadata {
            account: authority.clone(),
        }
        .into()]),
    );
    world.commit();
    bind_executor_test_contract_after_genesis(&state, &contract_address, &authority, code_hash);
    let transaction = TransactionBuilder::new(
        state.network_id,
        authority.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(
            Vec::new(),
            core::num::NonZeroU64::new(50_000_000),
        ),
    )
    .with_executable(Executable::ContractCall(ContractInvocation {
        contract_address: contract_address.clone(),
        expected_code_hash: code_hash,
        entrypoint: "write".to_owned(),
        arguments: Some(arguments),
    }))
    .sign(ALICE_KEYPAIR.private_key());
    let mut raw_metadata = Metadata::default();
    raw_metadata.insert(
        "contract_address"
            .parse()
            .expect("static contract address key"),
        Json::new(contract_address.to_string()),
    );
    raw_metadata.insert(
        "contract_entrypoint"
            .parse()
            .expect("static contract entrypoint key"),
        Json::new("write"),
    );
    raw_metadata.insert(
        "contract_payload"
            .parse()
            .expect("static contract payload key"),
        Json::from(norito::json!({ "value": "7" })),
    );
    let raw_transaction = TransactionBuilder::new(
        state.network_id,
        authority.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(
            Vec::new(),
            core::num::NonZeroU64::new(50_000_000),
        ),
    )
    .with_metadata(raw_metadata)
    .with_executable(Executable::Ivm(IvmBytecode::from_compiled(program)))
    .sign(ALICE_KEYPAIR.private_key());
    let mut block = state.block(BlockHeader::new(
        nonzero!(2_u64),
        Some(
            state
                .view()
                .latest_block_hash()
                .expect("genuine genesis parent"),
        ),
        None,
        0,
        0,
    ));
    let mut state_tx =
        block.transaction_for_fastpq_testing(Hash::from(transaction.hash_as_entrypoint()));
    state_tx.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
    state_tx.world.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
    let mut ivm_cache = IvmCache::new();
    ivm::reset_argument_record_decode_count();
    let error = super::Executor::Initial
        .execute_transaction(&mut state_tx, &authority, &transaction, &mut ivm_cache)
        .map_err(crate::execution_attempt::expect_completed_rejection)
        .expect_err("missing entrypoint permission must deny the direct call");
    assert!(
        error.to_string().contains(REQUIRED_PERMISSION),
        "unexpected direct contract authorization error: {error}"
    );
    assert_eq!(
        ivm::argument_record_decode_count(),
        0,
        "denied direct-call arguments must remain undecoded"
    );
    assert!(
        state_tx
            .world
            .account(&authority)
            .expect("authority account")
            .metadata()
            .get(&metadata_marker)
            .is_none(),
        "denied direct contract call must apply no queued effect"
    );
    drop(state_tx);
    let mut state_tx = block.transaction();
    let entrypoint_authorization: Permission =
        iroha_executor_data_model::permission::smart_contract::CanUseContractPermission {
            contract: contract_address.clone(),
            permission: "CanInvokeContractEntrypoint".parse().unwrap(),
        }
        .into();
    Grant::account_permission(entrypoint_authorization.clone(), authority.clone())
        .execute(&authority, &mut state_tx)
        .expect("grant direct-call entrypoint permission");
    state_tx.apply();
    let mut state_tx =
        block.transaction_for_fastpq_testing(Hash::from(transaction.hash_as_entrypoint()));
    state_tx.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
    state_tx.world.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
    ivm::reset_argument_record_decode_count();
    super::Executor::Initial
        .execute_transaction(&mut state_tx, &authority, &transaction, &mut ivm_cache)
        .map_err(crate::execution_attempt::expect_completed_rejection)
        .expect("granted direct contract call must execute");
    assert_eq!(
        ivm::argument_record_decode_count(),
        1,
        "granted direct-call arguments must be prepared exactly once"
    );
    let authorized_marker = state_tx
        .world
        .account(&authority)
        .expect("authority account")
        .metadata()
        .get(&metadata_marker)
        .cloned()
        .expect("authorized direct call writes its metadata marker");
    state_tx.apply();
    let mut state_tx = block.transaction();
    {
        let binding = state_tx
            .world
            .contract_subject_bindings
            .get_mut(&contract_address)
            .expect("active contract retains its lifecycle binding");
        binding.lifecycle.emergency_hold = Some(ContractEmergencyHoldV1 {
            incident_digest: [0xA1; 32],
            proposal_content_id: [0xA2; 32],
            governance_attempt_id: [0xA3; 32],
            reason: "contain direct contract execution".to_owned(),
            imposed_at_height: 2,
            expires_at_height: 3,
        });
        binding.lifecycle.revision = binding
            .lifecycle
            .revision
            .checked_add(1)
            .expect("test lifecycle revision advances");
    }
    let preproof_held = enforce_transaction_contract_permission_before_proof_verification(
        &state_tx,
        &authority,
        &raw_transaction,
        &mut ivm_cache,
        state_tx.block_height(),
        contract_address.dataspace_id().unwrap(),
    )
    .expect_err("raw-IVM pre-proof admission must observe the execution block height");
    assert!(
        matches!(&preproof_held, crate::execution_attempt::ExecutionAttemptError::Rejected(ValidationFail::NotPermitted(message))
                if message.contains("held by Parliament")),
        "unexpected raw-IVM pre-proof emergency-hold error: {preproof_held}"
    );
    enforce_transaction_contract_permission_before_proof_verification(
        &state_tx,
        &authority,
        &raw_transaction,
        &mut ivm_cache,
        3,
        contract_address.dataspace_id().unwrap(),
    )
    .expect("the half-open hold must not deny raw-IVM admission at its expiry height");
    state_tx.apply();
    let mut state_tx =
        block.transaction_for_fastpq_testing(Hash::from(transaction.hash_as_entrypoint()));
    state_tx.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
    state_tx.world.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
    ivm::reset_argument_record_decode_count();
    let held = super::Executor::Initial
        .execute_transaction(&mut state_tx, &authority, &transaction, &mut ivm_cache)
        .map_err(crate::execution_attempt::expect_completed_rejection)
        .expect_err("an active Parliament hold must suspend the direct contract call");
    assert!(
        matches!(held, ValidationFail::NotPermitted(ref message)
                if message.contains("held by Parliament")),
        "unexpected emergency-hold error: {held}"
    );
    assert_eq!(
        ivm::argument_record_decode_count(),
        0,
        "a held direct call must fail before decoding its arguments"
    );
    assert_eq!(
        state_tx
            .world
            .account(&authority)
            .expect("authority account")
            .metadata()
            .get(&metadata_marker),
        Some(&authorized_marker),
        "a held direct call must apply no queued effect"
    );
    drop(state_tx);
    let mut state_tx =
        block.transaction_for_fastpq_testing(Hash::from(raw_transaction.hash_as_entrypoint()));
    state_tx.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
    state_tx.world.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
    ivm::reset_argument_record_decode_count();
    let held_raw = super::Executor::Initial
        .execute_transaction(&mut state_tx, &authority, &raw_transaction, &mut ivm_cache)
        .map_err(crate::execution_attempt::expect_completed_rejection)
        .expect_err("an active Parliament hold must suspend raw-IVM contract dispatch");
    assert!(
        matches!(held_raw, ValidationFail::NotPermitted(ref message)
                if message.contains("held by Parliament")),
        "unexpected raw-IVM emergency-hold error: {held_raw}"
    );
    assert_eq!(
        ivm::argument_record_decode_count(),
        0,
        "held raw-IVM dispatch must fail before decoding its argument record"
    );
    assert_eq!(
        state_tx
            .world
            .account(&authority)
            .expect("authority account")
            .metadata()
            .get(&metadata_marker),
        Some(&authorized_marker),
        "held raw-IVM dispatch must apply no queued effect"
    );
    drop(state_tx);
    let mut state_tx = block.transaction();
    {
        let binding = state_tx
            .world
            .contract_subject_bindings
            .get_mut(&contract_address)
            .expect("held contract retains its lifecycle binding");
        binding.lifecycle.emergency_hold = None;
        binding.lifecycle.revision = binding
            .lifecycle
            .revision
            .checked_add(1)
            .expect("test lifecycle revision advances");
    }
    let live_code = state_tx
        .world
        .contract_code
        .remove(iroha_data_model::smart_contract::ContractArtifactId::new(
            contract_address.dataspace_id().unwrap(),
            code_hash,
        ))
        .expect("remove live bytecode for warm-cache adversarial check");
    state_tx.apply();
    let mut state_tx =
        block.transaction_for_fastpq_testing(Hash::from(transaction.hash_as_entrypoint()));
    state_tx.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
    state_tx.world.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
    ivm::reset_argument_record_decode_count();
    let missing_code = super::Executor::Initial
        .execute_transaction(&mut state_tx, &authority, &transaction, &mut ivm_cache)
        .map_err(crate::execution_attempt::expect_completed_rejection)
        .expect_err("a warm cache must not substitute for missing live bytecode");
    assert!(missing_code.to_string().contains("not found in WSV"));
    assert_eq!(ivm::argument_record_decode_count(), 0);
    assert_eq!(
        state_tx
            .world
            .account(&authority)
            .expect("authority account")
            .metadata()
            .get(&metadata_marker),
        Some(&authorized_marker),
        "missing live bytecode must apply no queued effect"
    );
    drop(state_tx);
    let mut state_tx = block.transaction();
    state_tx.world.contract_code.insert(
        iroha_data_model::smart_contract::ContractArtifactId::new(
            contract_address.dataspace_id().unwrap(),
            code_hash,
        ),
        live_code,
    );
    let live_manifest = state_tx
        .world
        .contract_manifests
        .remove(iroha_data_model::smart_contract::ContractArtifactId::new(
            contract_address.dataspace_id().unwrap(),
            code_hash,
        ))
        .expect("remove live manifest for warm-cache adversarial check");
    state_tx.apply();
    let mut state_tx =
        block.transaction_for_fastpq_testing(Hash::from(transaction.hash_as_entrypoint()));
    state_tx.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
    state_tx.world.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
    ivm::reset_argument_record_decode_count();
    let missing_manifest = super::Executor::Initial
        .execute_transaction(&mut state_tx, &authority, &transaction, &mut ivm_cache)
        .map_err(crate::execution_attempt::expect_completed_rejection)
        .expect_err("a warm cache must not substitute for a missing live manifest");
    assert!(
        matches!(&missing_manifest, ValidationFail::NotPermitted(reason)
                if reason == "contract permission requires the current signed manifest"),
        "{missing_manifest:?}"
    );
    assert_eq!(ivm::argument_record_decode_count(), 0);
    assert_eq!(
        state_tx
            .world
            .account(&authority)
            .expect("authority account")
            .metadata()
            .get(&metadata_marker),
        Some(&authorized_marker),
        "missing live manifest must apply no queued effect"
    );
    drop(state_tx);
    let mut state_tx = block.transaction();
    state_tx.world.contract_manifests.insert(
        iroha_data_model::smart_contract::ContractArtifactId::new(
            contract_address.dataspace_id().unwrap(),
            code_hash,
        ),
        live_manifest,
    );
    Revoke::account_permission(entrypoint_authorization.clone(), authority.clone())
        .execute(&authority, &mut state_tx)
        .expect("revoke direct-call entrypoint permission");
    state_tx.apply();
    let mut state_tx =
        block.transaction_for_fastpq_testing(Hash::from(transaction.hash_as_entrypoint()));
    state_tx.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
    state_tx.world.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
    ivm::reset_argument_record_decode_count();
    let revoked = super::Executor::Initial
        .execute_transaction(&mut state_tx, &authority, &transaction, &mut ivm_cache)
        .map_err(crate::execution_attempt::expect_completed_rejection)
        .expect_err("revoked direct-call permission must deny execution");
    assert!(revoked.to_string().contains(REQUIRED_PERMISSION));
    assert_eq!(
        ivm::argument_record_decode_count(),
        0,
        "revoked direct-call arguments must remain undecoded"
    );
    assert_eq!(
        state_tx
            .world
            .account(&authority)
            .expect("authority account")
            .metadata()
            .get(&metadata_marker),
        Some(&authorized_marker),
        "revoked direct contract call must preserve authorized state"
    );
    drop(state_tx);
    let mut state_tx = block.transaction();
    Grant::account_permission(entrypoint_authorization, authority.clone())
        .execute(&authority, &mut state_tx)
        .expect("restore direct-call entrypoint permission");
    let (rebound_program, rebound_manifest) = kotodama_lang::compiler::Compiler::new()
        .compile_source_with_manifest(
            r#"
seiyaku GuardedValueRebound { permission CanInvokeContractEntrypoint;
  kotoage fn write(int value) authorize(CanInvokeContractEntrypoint) {
    ledger::account::set_metadata(
      account: context::authority(),
      key: Name::parse("guarded_value"),
      value: Json::parse("{\"authorized\":\"rebound\"}")
    );
  }
}
"#,
        )
        .expect("compile a fully valid rebound contract");
    let rebound_code_hash = ivm::contract_code_hash(&rebound_program);
    state_tx.world.contract_code.insert(
        iroha_data_model::smart_contract::ContractArtifactId::new(
            contract_address.dataspace_id().unwrap(),
            rebound_code_hash,
        ),
        rebound_program,
    );
    state_tx.world.contract_manifests.insert(
        iroha_data_model::smart_contract::ContractArtifactId::new(
            contract_address.dataspace_id().unwrap(),
            rebound_code_hash,
        ),
        rebound_manifest
            .try_signed(
                manifest_signing.context(),
                manifest_signing.max_frame_bytes(),
                &ALICE_KEYPAIR,
            )
            .expect("sign bounded fixture manifest"),
    );
    state_tx
        .world
        .contract_instances
        .insert(contract_address.clone(), rebound_code_hash);
    {
        let binding = state_tx
            .world
            .contract_subject_bindings
            .get_mut(&contract_address)
            .expect("rebound contract retains its lifecycle binding");
        binding.lifecycle.active_code_hash = Some(rebound_code_hash);
        binding.lifecycle.retained_code_hash = Some(rebound_code_hash);
        binding.lifecycle.revision += 1;
    }
    state_tx.apply();
    let mut state_tx =
        block.transaction_for_fastpq_testing(Hash::from(transaction.hash_as_entrypoint()));
    state_tx.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
    state_tx.world.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
    ivm::reset_argument_record_decode_count();
    let rebound = super::Executor::Initial
        .execute_transaction(&mut state_tx, &authority, &transaction, &mut ivm_cache)
        .map_err(crate::execution_attempt::expect_completed_rejection)
        .expect_err("a signed direct call must not cross a live code rebind");
    assert!(
        matches!(rebound, ValidationFail::NotPermitted(ref message)
                if message.contains(&contract_address.to_string())
                    && message.contains(&code_hash.to_string())
                    && message.contains(&rebound_code_hash.to_string())),
        "unexpected live-rebind error: {rebound}"
    );
    assert_eq!(
        ivm::argument_record_decode_count(),
        0,
        "a live code rebind must be rejected before argument decoding"
    );
    assert_eq!(
        state_tx
            .world
            .account(&authority)
            .expect("authority account")
            .metadata()
            .get(&metadata_marker),
        Some(&authorized_marker),
        "a live code rebind must apply no queued contract effect"
    );
    drop(state_tx);
    let mut state_tx = block.transaction();
    state_tx
        .world
        .contract_instances
        .insert(contract_address.clone(), code_hash);
    {
        let binding = state_tx
            .world
            .contract_subject_bindings
            .get_mut(&contract_address)
            .expect("restored contract retains its lifecycle binding");
        binding.lifecycle.active_code_hash = Some(code_hash);
        binding.lifecycle.retained_code_hash = Some(code_hash);
        binding.lifecycle.revision += 1;
    }
    state_tx
        .world
        .contract_code
        .remove(iroha_data_model::smart_contract::ContractArtifactId::new(
            contract_address.dataspace_id().unwrap(),
            rebound_code_hash,
        ))
        .expect("remove rebound bytecode after restoring the original binding");
    state_tx
        .world
        .contract_manifests
        .remove(iroha_data_model::smart_contract::ContractArtifactId::new(
            contract_address.dataspace_id().unwrap(),
            rebound_code_hash,
        ))
        .expect("remove rebound manifest after restoring the original binding");
    state_tx
        .world
        .contract_instances
        .remove(contract_address.clone());
    {
        let binding = state_tx
            .world
            .contract_subject_bindings
            .get_mut(&contract_address)
            .expect("deactivated contract retains its lifecycle binding");
        binding.lifecycle.active_code_hash = None;
        binding.lifecycle.revision += 1;
    }
    state_tx.apply();
    let mut state_tx =
        block.transaction_for_fastpq_testing(Hash::from(transaction.hash_as_entrypoint()));
    state_tx.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
    state_tx.world.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
    ivm::reset_argument_record_decode_count();
    let deactivated = super::Executor::Initial
        .execute_transaction(&mut state_tx, &authority, &transaction, &mut ivm_cache)
        .map_err(crate::execution_attempt::expect_completed_rejection)
        .expect_err("deactivated direct-call target must deny execution");
    assert!(
        deactivated.to_string().contains("not found"),
        "unexpected deactivated direct-call error: {deactivated}"
    );
    assert_eq!(
        ivm::argument_record_decode_count(),
        0,
        "deactivated direct-call arguments must remain undecoded"
    );
    assert_eq!(
        state_tx
            .world
            .account(&authority)
            .expect("authority account")
            .metadata()
            .get(&metadata_marker),
        Some(&authorized_marker),
        "deactivated direct contract call must apply no queued effect"
    );
}
#[test]
fn mixed_batch_next_contract_reserves_accountable_nested_block_gas() {
    assert_eq!(
        super::live_batch_contract_execution_limit(Some(100), 20, 80, 70),
        10,
        "nested trigger/host work must reduce the next contract's block allowance"
    );
    assert_eq!(
        super::live_batch_contract_execution_limit(Some(30), 20, 80, 25),
        10,
        "the signature-bound direct-gas allowance remains independently enforced"
    );
    assert_eq!(
        super::live_batch_contract_execution_limit(None, 20, u64::MAX, 25),
        u64::MAX - 25,
        "an unlimited block still accounts already-retained work"
    );
}
#[test]
fn mixed_batch_observes_ordered_permission_state_and_rolls_back_on_failure() {
    let manifest_signing = crate::manifest_signing_test_support::ManifestSigningFixture::new();
    let (program, manifest) = kotodama_lang::compiler::Compiler::new()
        .compile_source_with_manifest(
            r#"
seiyaku OrderedBatchGuard { permission CanInvokeContractEntrypoint;
  kotoage fn write(int value) authorize(CanInvokeContractEntrypoint) {
    ledger::account::set_metadata(
      account: context::authority(),
      key: Name::parse("mixed_batch_marker"),
      value: Json::parse("{\"written\":true}")
    );
  }
}
"#,
        )
        .expect("compile ordered mixed-batch contract");
    let parsed = ivm::ProgramMetadata::parse(&program).expect("parse mixed-batch contract");
    let schema = parsed
        .contract_interface
        .as_ref()
        .and_then(|interface| {
            interface
                .entrypoints
                .iter()
                .find(|entry| entry.name == "write")
        })
        .and_then(|entry| entry.argument_schema.as_ref())
        .expect("write argument schema");
    let arguments = ivm_abi::arguments::encode_argument_record_from_json(
        schema,
        &Json::from(norito::json!({ "value": "9" })),
    )
    .expect("encode mixed-batch arguments");
    let arguments =
        iroha_data_model::transaction::executable::ContractArgumentRecord::try_new(arguments)
            .expect("bounded mixed-batch arguments");
    let authority = ALICE_ID.clone();
    let domain = Domain::new(DomainId::try_new("wonderland", "universal").expect("domain id"))
        .build(&authority);
    let account = Account::new(authority.clone()).build(&authority);
    let state = state_after_genesis(World::with([domain], [account], []));
    let code_hash = ivm::contract_code_hash(&program);
    let contract_address =
        ContractAddress::derive(&state.network_id, &authority, 93, DataSpaceId::UNIVERSAL)
            .expect("derive contract address");
    let mut world = state.world.block();
    world.contract_code.insert(
        iroha_data_model::smart_contract::ContractArtifactId::new(
            contract_address.dataspace_id().unwrap(),
            code_hash,
        ),
        program,
    );
    world.contract_manifests.insert(
        iroha_data_model::smart_contract::ContractArtifactId::new(
            contract_address.dataspace_id().unwrap(),
            code_hash,
        ),
        manifest
            .try_signed(
                manifest_signing.context(),
                manifest_signing.max_frame_bytes(),
                &ALICE_KEYPAIR,
            )
            .expect("sign bounded fixture manifest"),
    );
    // The invoked contract's subject needs its own exact grant for caller metadata.
    world.account_permissions.insert(
        contract_address.subject_id(),
        BTreeSet::from([executor_permission::account::CanModifyAccountMetadata {
            account: authority.clone(),
        }
        .into()]),
    );
    world.commit();
    bind_executor_test_contract_after_genesis(&state, &contract_address, &authority, code_hash);
    let entrypoint_authorization: Permission =
        iroha_executor_data_model::permission::smart_contract::CanUseContractPermission {
            contract: contract_address.clone(),
            permission: "CanInvokeContractEntrypoint".parse().unwrap(),
        }
        .into();
    let invocation = ContractInvocation {
        contract_address,
        expected_code_hash: code_hash,
        entrypoint: "write".to_owned(),
        arguments: Some(arguments),
    };
    let explicit_instructions = vec![
        InstructionBox::from(Grant::account_permission(
            entrypoint_authorization.clone(),
            authority.clone(),
        )),
        InstructionBox::from(Revoke::account_permission(
            entrypoint_authorization.clone(),
            authority.clone(),
        )),
    ];
    let transaction = TransactionBuilder::new(
        state.network_id,
        authority.clone(),
        FeePaymentIntent::authority(Vec::new(), core::num::NonZeroU64::new(50_000_000)),
    )
    .with_executable(Executable::Batch(
        vec![
            ExecutableBatchItem::Instruction(explicit_instructions[0].clone()),
            ExecutableBatchItem::ContractCall(invocation.clone()),
            ExecutableBatchItem::Instruction(explicit_instructions[1].clone()),
        ]
        .into(),
    ))
    .sign(ALICE_KEYPAIR.private_key());
    let mut block = state.block(BlockHeader::new(
        nonzero!(2_u64),
        Some(
            state
                .view()
                .latest_block_hash()
                .expect("genuine genesis parent"),
        ),
        None,
        0,
        0,
    ));
    let mut state_tx =
        block.transaction_for_fastpq_testing(Hash::from(transaction.hash_as_entrypoint()));
    state_tx.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
    state_tx.world.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
    let mut ivm_cache = IvmCache::new();
    super::Executor::Initial
        .execute_transaction(&mut state_tx, &authority, &transaction, &mut ivm_cache)
        .map_err(crate::execution_attempt::expect_completed_rejection)
        .expect("grant-call-revoke batch must execute in order");
    let marker: Name = "mixed_batch_marker".parse().expect("marker name");
    assert!(
        state_tx
            .world
            .account(&authority)
            .expect("authority account")
            .metadata()
            .get(&marker)
            .is_some(),
        "the contract call must observe the preceding permission grant"
    );
    assert!(
        !state_tx
            .world
            .account_permissions_iter(&authority)
            .expect("authority permissions")
            .any(|permission| permission == &entrypoint_authorization),
        "the trailing revoke must remain visible after the call"
    );
    assert!(
        state_tx.last_tx_gas_used
            > isi_gas::meter_instructions(&explicit_instructions, &crate::gas::DEFAULT_ZK_GAS)
                .expect("completed fixture gas quote"),
        "aggregate batch gas must include contract execution"
    );
    state_tx.apply();
    let capped_transaction = TransactionBuilder::new(
        state.network_id,
        authority.clone(),
        FeePaymentIntent::authority(Vec::new(), core::num::NonZeroU64::new(50_000_000)),
    )
    .with_executable(Executable::Batch(
        vec![
            ExecutableBatchItem::Instruction(explicit_instructions[0].clone()),
            ExecutableBatchItem::ContractCall(invocation.clone()),
        ]
        .into(),
    ))
    .sign(ALICE_KEYPAIR.private_key());
    let mut instruction_capped_state_tx =
        block.transaction_for_fastpq_testing(Hash::from(capped_transaction.hash_as_entrypoint()));
    instruction_capped_state_tx.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
    instruction_capped_state_tx.world.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
    instruction_capped_state_tx
        .pipeline
        .overlay_max_instructions = 1;
    let instruction_cap_error = super::Executor::Initial
        .execute_transaction(
            &mut instruction_capped_state_tx,
            &authority,
            &capped_transaction,
            &mut ivm_cache,
        )
        .map_err(crate::execution_attempt::expect_completed_rejection)
        .expect_err("contract-emitted ISIs must count toward the mixed-batch overlay cap");
    assert!(
        matches!(instruction_cap_error, ValidationFail::NotPermitted(ref message)
                if message == "overlay exceeds max instructions: 2 > 1"),
        "unexpected mixed-batch instruction-cap error: {instruction_cap_error}"
    );
    assert!(
        instruction_capped_state_tx.last_tx_gas_used
            > isi_gas::meter_instructions(&explicit_instructions[..1], &crate::gas::DEFAULT_ZK_GAS)
                .expect("completed fixture gas quote"),
        "overlay-cap rejection must retain the completed contract attempt"
    );
    drop(instruction_capped_state_tx);
    let explicit_overlay_bytes = super::live_batch_overlay_byte_size(&explicit_instructions[..1]);
    let mut byte_capped_state_tx =
        block.transaction_for_fastpq_testing(Hash::from(capped_transaction.hash_as_entrypoint()));
    byte_capped_state_tx.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
    byte_capped_state_tx.world.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
    byte_capped_state_tx.pipeline.overlay_max_instructions = 0;
    byte_capped_state_tx.pipeline.overlay_max_bytes = explicit_overlay_bytes;
    let byte_cap_error = super::Executor::Initial
        .execute_transaction(
            &mut byte_capped_state_tx,
            &authority,
            &capped_transaction,
            &mut ivm_cache,
        )
        .map_err(crate::execution_attempt::expect_completed_rejection)
        .expect_err("contract-emitted ISIs must count toward the mixed-batch byte cap");
    assert!(
        matches!(byte_cap_error, ValidationFail::NotPermitted(ref message)
                if message.starts_with("overlay exceeds max bytes: ")
                    && message == &format!("overlay exceeds max bytes: more than {explicit_overlay_bytes}")),
        "unexpected mixed-batch byte-cap error: {byte_cap_error}"
    );
    drop(byte_capped_state_tx);
    let cap_verification_tx = block.transaction();
    assert!(
        !cap_verification_tx
            .world
            .account_permissions_iter(&authority)
            .expect("authority permissions")
            .any(|permission| permission == &entrypoint_authorization),
        "dropping a cap-rejected mixed batch must roll back its permission grant"
    );
    drop(cap_verification_tx);
    let rollback_marker: Name = "mixed_batch_rollback_marker"
        .parse()
        .expect("rollback marker name");
    let failing_transaction = TransactionBuilder::new(
        state.network_id,
        authority.clone(),
        FeePaymentIntent::authority(Vec::new(), core::num::NonZeroU64::new(50_000_000)),
    )
    .with_executable(Executable::Batch(
        vec![
            ExecutableBatchItem::Instruction(InstructionBox::from(SetKeyValue::account(
                authority.clone(),
                rollback_marker.clone(),
                Json::new(true),
            ))),
            ExecutableBatchItem::ContractCall(invocation),
        ]
        .into(),
    ))
    .sign(ALICE_KEYPAIR.private_key());
    let mut failed_state_tx =
        block.transaction_for_fastpq_testing(Hash::from(failing_transaction.hash_as_entrypoint()));
    failed_state_tx.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
    failed_state_tx.world.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
    let error = super::Executor::Initial
        .execute_transaction(
            &mut failed_state_tx,
            &authority,
            &failing_transaction,
            &mut ivm_cache,
        )
        .map_err(crate::execution_attempt::expect_completed_rejection)
        .expect_err("the revoked permission must reject the later call");
    assert!(error.to_string().contains("CanInvokeContractEntrypoint"));
    drop(failed_state_tx);
    let verification_tx = block.transaction();
    assert!(
        verification_tx
            .world
            .account(&authority)
            .expect("authority account")
            .metadata()
            .get(&rollback_marker)
            .is_none(),
        "dropping the failed batch transaction must roll back its preceding native write"
    );
}
#[test]
fn resolved_contract_invocation_releases_cache_and_records_vm_error_gas() {
    let manifest_signing = crate::manifest_signing_test_support::ManifestSigningFixture::new();
    let (program, manifest) = kotodama_lang::compiler::Compiler::new()
        .compile_source_with_manifest(
            r#"
seiyaku MeteredFailure { permission CanInvokeContractEntrypoint;
  kotoage fn run() authorize(CanInvokeContractEntrypoint) {
    ledger::account::set_metadata(
      account: context::authority(),
      key: Name::parse("must_not_be_written"),
      value: Json::parse("true")
    );
  }
}
"#,
        )
        .expect("compile metered failure contract");
    let authority = ALICE_ID.clone();
    let domain =
        Domain::new(DomainId::try_new("wonderland", "universal").expect("valid domain id"))
            .build(&authority);
    let account = Account::new(authority.clone()).build(&authority);
    let state = state_after_genesis(World::with([domain], [account], []));
    let code_hash = ivm::contract_code_hash(&program);
    let contract_address =
        ContractAddress::derive(&state.network_id, &authority, 94, DataSpaceId::UNIVERSAL)
            .expect("derive metered failure contract address");
    let mut world = state.world.block();
    world.contract_code.insert(
        iroha_data_model::smart_contract::ContractArtifactId::new(
            contract_address.dataspace_id().unwrap(),
            code_hash,
        ),
        program,
    );
    world.contract_manifests.insert(
        iroha_data_model::smart_contract::ContractArtifactId::new(
            contract_address.dataspace_id().unwrap(),
            code_hash,
        ),
        manifest
            .try_signed(
                manifest_signing.context(),
                manifest_signing.max_frame_bytes(),
                &ALICE_KEYPAIR,
            )
            .expect("sign bounded fixture manifest"),
    );
    world.commit();
    bind_executor_test_contract_after_genesis(&state, &contract_address, &authority, code_hash);
    let mut block = state.block(BlockHeader::new(
        nonzero!(2_u64),
        Some(
            state
                .view()
                .latest_block_hash()
                .expect("genuine genesis parent"),
        ),
        None,
        0,
        0,
    ));
    let mut state_transaction = block.transaction();
    let entrypoint_authorization: Permission =
        iroha_executor_data_model::permission::smart_contract::CanUseContractPermission {
            contract: contract_address.clone(),
            permission: "CanInvokeContractEntrypoint".parse().unwrap(),
        }
        .into();
    Grant::account_permission(entrypoint_authorization, authority.clone())
        .execute(&authority, &mut state_transaction)
        .expect("grant metered failure entrypoint permission");
    let invocation = ContractInvocation {
        contract_address,
        expected_code_hash: code_hash,
        entrypoint: "run".to_owned(),
        arguments: None,
    };
    // This private resolved-call fixture still owns one actual signed root;
    // it tests cache release and failed VM work, not full source admission.
    let signed = TransactionBuilder::new(
        state.network_id,
        authority.clone(),
        FeePaymentIntent::authority(Vec::new(), core::num::NonZeroU64::new(10)),
    )
    .with_executable(Executable::ContractCall(invocation.clone()))
    .sign(ALICE_KEYPAIR.private_key());
    state_transaction.current_tx_hash = Some(signed.hash());
    state_transaction.tx_call_hash = Some(Hash::from(signed.hash_as_entrypoint()));
    state_transaction
        .begin_execution_effect_budget(&signed)
        .unwrap();
    let cache = state_transaction.ivm_cache;
    let resolved = {
        let mut guard = cache.lock();
        crate::executor::Executor::resolve_contract_invocation(
            &mut state_transaction,
            &invocation,
            &mut guard,
        )
        .expect("resolve deployed contract while the cache is locked")
    };
    assert!(
        cache.try_lock().is_some(),
        "the resolved invocation must not retain the outer cache mutex"
    );
    let error = crate::executor::Executor::execute_resolved_contract_invocation(
        &mut state_transaction,
        &authority,
        &invocation,
        resolved,
        (10, 0),
        None,
        None,
    )
    .expect_err("ten units of gas cannot complete the contract");
    state_transaction.finish_execution_effect_budget().unwrap();
    let ValidationFail::IvmFault(fault) = error else {
        panic!("expected a typed gas-exhaustion fault, got {error}");
    };
    assert_eq!(
        fault.kind,
        iroha_data_model::executor::fault::IvmFaultKindV1::OutOfGas
    );
    assert_eq!(fault.site.code_hash, code_hash);
    assert_eq!(
        fault.site.selector,
        iroha_data_model::executor::fault::IvmInvocationSelectorV1::Entrypoint(0)
    );
    assert!(
        (1..=10).contains(&state_transaction.last_tx_gas_used),
        "failed VM execution must retain its chargeable gas, observed {}",
        state_transaction.last_tx_gas_used
    );
}
#[test]
fn identityless_raw_and_proved_dispatch_reject_before_argument_decode_or_proof_work() {
    let program = kotodama_lang::compiler::Compiler::new_with_options(
        kotodama_lang::compiler::CompilerOptions {
            force_zk: true,
            max_cycles: 10_000,
            ..kotodama_lang::compiler::CompilerOptions::default()
        },
    )
    .compile_source(
        r"
seiyaku IdentityRequired {
  view fn write(int value) authorize(anyone) -> int {
    return value;
  }
}
",
    )
    .expect("compile identity-required raw contract");
    let authority = ALICE_ID.clone();
    let domain = Domain::new(DomainId::try_new("wonderland", "universal").expect("domain id"))
        .build(&authority);
    let account = Account::new(authority.clone()).build(&authority);
    let state = state_after_genesis(World::with([domain], [account], []));
    let mut metadata = Metadata::default();
    metadata.insert(
        "contract_entrypoint".parse().expect("entrypoint key"),
        Json::new("write"),
    );
    metadata.insert(
        "contract_payload".parse().expect("payload key"),
        Json::from(norito::json!({ "value": "7" })),
    );
    let bytecode = IvmBytecode::from_compiled(program);
    let raw = TransactionBuilder::new(
        state.network_id,
        authority.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(
            Vec::new(),
            core::num::NonZeroU64::new(1_000_000),
        ),
    )
    .with_metadata(metadata.clone())
    .with_executable(Executable::Ivm(bytecode.clone()))
    .sign(ALICE_KEYPAIR.private_key());
    let proved = TransactionBuilder::new(
        state.network_id,
        authority.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(
            Vec::new(),
            core::num::NonZeroU64::new(1_000_000),
        ),
    )
    .with_metadata(metadata)
    .with_executable(Executable::IvmProved(
        iroha_data_model::transaction::IvmProved {
            bytecode,
            overlay: Vec::<InstructionBox>::new().into(),
            events_commitment: Hash::new(b"identityless-events"),
            gas_policy_commitment: Hash::new(b"identityless-gas"),
        },
    ))
    .sign(ALICE_KEYPAIR.private_key());
    let initial_durable_state = {
        let view = state.view();
        view.world()
            .smart_contract_state()
            .iter()
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect::<Vec<_>>()
    };
    for (label, transaction) in [("raw", raw), ("proved", proved)] {
        let mut block = state.block(BlockHeader::new(
            nonzero!(2_u64),
            Some(
                state
                    .view()
                    .latest_block_hash()
                    .expect("genuine genesis parent"),
            ),
            None,
            0,
            0,
        ));
        let mut state_tx =
            block.transaction_for_fastpq_testing(Hash::from(transaction.hash_as_entrypoint()));
        state_tx.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
        state_tx.world.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
        let mut ivm_cache = IvmCache::new();
        ivm::reset_argument_record_decode_count();
        let error = super::Executor::Initial
            .execute_transaction(&mut state_tx, &authority, &transaction, &mut ivm_cache)
            .map_err(crate::execution_attempt::expect_completed_rejection)
            .expect_err("identity-less raw contract dispatch must fail closed");
        assert!(
            error
                .to_string()
                .contains("requires a live contract_address or contract_alias"),
            "unexpected identity-less {label} dispatch error: {error}"
        );
        assert_eq!(
            ivm::argument_record_decode_count(),
            0,
            "identity-less {label} dispatch must not decode its argument record"
        );
        let observed_durable_state = state_tx
            .world
            .smart_contract_state
            .iter()
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect::<Vec<_>>();
        assert_eq!(
            observed_durable_state, initial_durable_state,
            "identity-less {label} dispatch must not change durable state"
        );
    }
}
fn contract_permission_context(
    contract_address: ContractAddress,
    entrypoint: &str,
    permission: &str,
) -> ContractCallExecutionContext {
    ContractCallExecutionContext {
        contract_subject: Some(contract_address.subject_id()),
        contract_address: Some(contract_address),
        contract_alias: None,
        entrypoint: Some(entrypoint.to_owned()),
        entrypoint_pc: Some(0),
        entrypoint_authorization: EntrypointAuthorizationV1::Permission(
            permission.parse().expect("permission name"),
        ),
        args: Json::new(()),
        argument_record: None,
    }
}
#[test]
fn contract_invocation_rejects_a_live_code_rebind() {
    let contract_address = ContractAddress::derive(
        &"hash:0000000000000000000000000000000000000000000000000000000000000001#C50E"
            .parse()
            .expect("canonical test network id"),
        &ALICE_ID,
        77,
        DataSpaceId::UNIVERSAL,
    )
    .expect("derive contract address");
    let signed_hash = iroha_crypto::Hash::new(b"signed-contract-code");
    let live_hash = iroha_crypto::Hash::new(b"rebound-contract-code");
    let invocation = ContractInvocation {
        contract_address,
        expected_code_hash: signed_hash,
        entrypoint: "run".to_owned(),
        arguments: None,
    };
    let error = ensure_contract_invocation_code_hash(&invocation, live_hash)
        .expect_err("a signed call must not cross a live code rebind");
    assert!(
        matches!(error, ValidationFail::NotPermitted(ref message)
                if message.contains(&signed_hash.to_string())
                    && message.contains(&live_hash.to_string())),
        "unexpected binding error: {error}"
    );
}
#[test]
fn reviewable_contract_metadata_binds_exact_invocation_arguments() {
    let (program, _) = kotodama_lang::compiler::Compiler::new()
        .compile_source_with_manifest(
            r"
seiyaku ReviewedValue { permission CanInvokeContractEntrypoint;
  kotoage fn write(int value) authorize(CanInvokeContractEntrypoint) {}
}
",
        )
        .expect("compile reviewed contract");
    let prepared = ivm::prepare_contract(Arc::<[u8]>::from(program.clone()))
        .expect("prepare reviewed contract");
    let schema = prepared
        .entrypoint_descriptor("write")
        .and_then(|entrypoint| entrypoint.argument_schema.as_ref())
        .expect("reviewed argument schema");
    let reviewed_payload = Json::from(norito::json!({ "value": "7" }));
    let reviewed_arguments =
        ivm_abi::arguments::encode_argument_record_from_json(schema, &reviewed_payload)
            .expect("encode reviewed arguments");
    let swapped_arguments = ivm_abi::arguments::encode_argument_record_from_json(
        schema,
        &Json::from(norito::json!({ "value": "8" })),
    )
    .expect("encode swapped arguments");
    let contract_address = ContractAddress::derive(
        &"hash:0000000000000000000000000000000000000000000000000000000000000001#C50E"
            .parse()
            .expect("canonical test network id"),
        &ALICE_ID,
        78,
        DataSpaceId::UNIVERSAL,
    )
    .expect("derive contract address");
    let code_hash = ivm::contract_code_hash(&program);
    let mut metadata = Metadata::default();
    for (key, value) in [
        ("contract_address", Json::new(contract_address.to_string())),
        ("contract_code_hash", Json::new(code_hash.to_string())),
        ("contract_entrypoint", Json::new("write".to_owned())),
    ] {
        metadata.insert(key.parse().expect("static contract metadata key"), value);
    }
    metadata.insert(
        "contract_payload"
            .parse()
            .expect("static contract payload metadata key"),
        reviewed_payload,
    );
    let invocation = |arguments: Vec<u8>| ContractInvocation {
        contract_address: contract_address.clone(),
        expected_code_hash: code_hash,
        entrypoint: "write".to_owned(),
        arguments: Some(
            iroha_data_model::transaction::executable::ContractArgumentRecord::try_new(arguments)
                .expect("bounded reviewed arguments"),
        ),
    };
    let live_alias: iroha_data_model::smart_contract::ContractAlias =
        "reviewed_value::apps.universal"
            .parse()
            .expect("live contract alias");
    super::ensure_contract_invocation_metadata_binding(
        &invocation(reviewed_arguments.clone()),
        &metadata,
        &prepared,
        Some(&live_alias),
    )
    .expect("matching reviewed metadata and arguments");
    let error = super::ensure_contract_invocation_metadata_binding(
        &invocation(swapped_arguments),
        &metadata,
        &prepared,
        Some(&live_alias),
    )
    .expect_err("swapped argument bytes must be rejected");
    assert!(
        matches!(error, ValidationFail::NotPermitted(ref message)
                if message.contains("arguments differ from the canonical contract_payload")),
        "unexpected swapped-arguments error: {error}"
    );
    // Indexers attribute a call to its `contract_alias`, so the claim must
    // name the address's live alias.
    let alias_key: Name = "contract_alias"
        .parse()
        .expect("static contract alias metadata key");
    let mut aliased = metadata.clone();
    aliased.insert(alias_key.clone(), Json::new(live_alias.to_string()));
    super::ensure_contract_invocation_metadata_binding(
        &invocation(reviewed_arguments.clone()),
        &aliased,
        &prepared,
        Some(&live_alias),
    )
    .expect("the live alias binds");
    let other_alias: iroha_data_model::smart_contract::ContractAlias =
        "forged_router::apps.universal"
            .parse()
            .expect("other contract alias");
    for live in [None, Some(&other_alias)] {
        let error = super::ensure_contract_invocation_metadata_binding(
            &invocation(reviewed_arguments.clone()),
            &aliased,
            &prepared,
            live,
        )
        .expect_err("an alias claim must name the live alias");
        assert!(
            matches!(error, ValidationFail::NotPermitted(ref message)
                    if message.contains("is not the live alias")),
            "unexpected alias error: {error}"
        );
    }
    let mut alias_only = Metadata::default();
    alias_only.insert(alias_key, Json::new(live_alias.to_string()));
    let error = super::ensure_contract_invocation_metadata_binding(
        &invocation(reviewed_arguments),
        &alias_only,
        &prepared,
        Some(&live_alias),
    )
    .expect_err("an alias claim requires the full identity binding");
    assert!(
        matches!(error, ValidationFail::NotPermitted(ref message)
                if message.contains("requires contract_address metadata")),
        "unexpected alias-only error: {error}"
    );
}
#[test]
fn contract_dispatch_context_carries_entrypoint_authorization() {
    let (program, expected_entrypoint_pc) =
        contract_program_with_entrypoint("admin", Some("ContractAdmin"));
    let mut metadata = Metadata::default();
    metadata.insert(
        Name::from_str("contract_entrypoint").expect("static name"),
        Json::new("admin".to_owned()),
    );
    let metadata_context = parse_contract_call_execution_context(&metadata, &program)
        .expect("parse metadata dispatch")
        .expect("metadata dispatch context");
    assert_eq!(metadata_context.entrypoint.as_deref(), Some("admin"));
    assert_eq!(
        metadata_context.entrypoint_pc(),
        Some(expected_entrypoint_pc)
    );
    assert_eq!(
        metadata_context.entrypoint_authorization(),
        &EntrypointAuthorizationV1::Permission("ContractAdmin".parse().unwrap())
    );
    let contract_address = ContractAddress::derive(
        &"hash:0000000000000000000000000000000000000000000000000000000000000001#C50E"
            .parse()
            .expect("canonical test network id"),
        &ALICE_ID,
        1,
        DataSpaceId::UNIVERSAL,
    )
    .expect("derive contract address");
    let invocation = ContractInvocation {
        contract_address: contract_address.clone(),
        expected_code_hash: iroha_crypto::Hash::new(b"admin-contract-code"),
        entrypoint: "admin".to_owned(),
        arguments: None,
    };
    let invocation_context = parse_contract_invocation_execution_context(
        &invocation,
        &program,
        None,
        contract_address.subject_id(),
    )
    .expect("parse contract invocation");
    assert_eq!(
        invocation_context.entrypoint_pc(),
        Some(expected_entrypoint_pc)
    );
    assert_eq!(
        invocation_context.entrypoint_authorization(),
        &EntrypointAuthorizationV1::Permission("ContractAdmin".parse().unwrap())
    );
}
#[test]
fn nested_contract_dispatch_accepts_view_without_relaxing_top_level_calls() {
    use iroha_data_model::smart_contract::manifest::EntryPointKind;
    let (program, expected_entrypoint_pc) = contract_program_with_entrypoint_kind(
        "configuration",
        EntryPointKind::View,
        Some("CanInspectConfiguration"),
    );
    let prepared =
        ivm::prepare_contract(Arc::<[u8]>::from(program)).expect("prepare nested-view contract");
    let contract_address = ContractAddress::derive(
        &"hash:0000000000000000000000000000000000000000000000000000000000000001#C50E"
            .parse()
            .expect("canonical test network id"),
        &ALICE_ID,
        2,
        DataSpaceId::UNIVERSAL,
    )
    .expect("derive contract address");
    let invocation = ContractInvocation {
        contract_address: contract_address.clone(),
        expected_code_hash: iroha_crypto::Hash::new(b"configuration-contract-code"),
        entrypoint: "configuration".to_owned(),
        arguments: None,
    };
    let top_level_err = parse_prepared_contract_invocation_execution_context(
        &invocation,
        &prepared,
        None,
        contract_address.subject_id(),
        u64::MAX,
    )
    .map_err(crate::execution_attempt::expect_completed_rejection)
    .expect_err("top-level transaction dispatch must remain public-only");
    assert!(matches!(
        top_level_err,
        ValidationFail::NotPermitted(message) if message.contains("read-only")
    ));
    let descriptor = prepared.entrypoint_descriptor("configuration").unwrap();
    let permission = nested_contract_entrypoint_authorization(descriptor, "configuration")
        .expect("nested dispatch should accept a declared view");
    assert_eq!(
        prepared.entrypoint_pc("configuration"),
        Some(expected_entrypoint_pc)
    );
    assert_eq!(
        permission,
        EntrypointAuthorizationV1::Permission("CanInspectConfiguration".parse().unwrap())
    );
    for (selector, kind) in [
        ("hajimari", EntryPointKind::Hajimari),
        ("始まり", EntryPointKind::Hajimari),
        ("kaizen", EntryPointKind::Kaizen),
        ("改善", EntryPointKind::Kaizen),
    ] {
        let (program, _) = contract_program_with_entrypoint_kind(selector, kind, None);
        let prepared =
            ivm::prepare_contract(Arc::<[u8]>::from(program)).expect("prepare lifecycle contract");
        let descriptor = prepared.entrypoint_descriptor(selector).unwrap();
        let error = nested_contract_entrypoint_authorization(descriptor, selector)
            .expect_err("nested dispatch must reject lifecycle entrypoints");
        assert!(
            matches!(error, ValidationFail::NotPermitted(ref message) if message.contains("cannot be invoked by a nested call")),
            "unexpected {kind:?} nested-dispatch error: {error}"
        );
    }
}
#[test]
fn prepared_view_resolver_accepts_only_views_and_preserves_exact_permission() {
    use iroha_data_model::smart_contract::manifest::EntryPointKind;
    let (program, expected_pc) = contract_program_with_entrypoint_kind(
        "inspect",
        EntryPointKind::View,
        Some("CanInspectContract"),
    );
    let prepared =
        ivm::prepare_contract(Arc::<[u8]>::from(program)).expect("prepare view contract");
    let (pc, permission, arguments) =
        resolve_prepared_contract_view_entrypoint(&prepared, "inspect")
            .expect("declared view resolves");
    assert_eq!(pc, expected_pc);
    assert_eq!(
        permission,
        EntrypointAuthorizationV1::Permission("CanInspectContract".parse().unwrap())
    );
    assert!(arguments.is_none());
    assert!(
        resolve_prepared_contract_entrypoint(&prepared, "inspect").is_err(),
        "transaction resolution must continue to reject read-only views"
    );
    for (selector, kind) in [
        ("write", EntryPointKind::Kotoage),
        ("hajimari", EntryPointKind::Hajimari),
        ("始まり", EntryPointKind::Hajimari),
        ("kaizen", EntryPointKind::Kaizen),
        ("改善", EntryPointKind::Kaizen),
    ] {
        let permission = (kind == EntryPointKind::Kotoage).then_some("CanInvokeContractEntrypoint");
        let (program, _) = contract_program_with_entrypoint_kind(selector, kind, permission);
        let prepared =
            ivm::prepare_contract(Arc::<[u8]>::from(program)).expect("prepare non-view contract");
        let error = resolve_prepared_contract_view_entrypoint(&prepared, selector)
            .expect_err("the view boundary must reject every non-view entrypoint kind");
        assert!(
            matches!(error, ValidationFail::NotPermitted(ref message) if message.contains("not a read-only view")),
            "unexpected {kind:?} view-resolution error: {error}"
        );
    }
}
#[test]
fn prepared_resolvers_reject_private_witness_entrypoints_before_host_execution() {
    use iroha_data_model::smart_contract::manifest::EntryPointKind;
    let transaction_program =
        contract_program_with_private_input_entrypoint("commit", EntryPointKind::Kotoage);
    let transaction_contract = ivm::prepare_contract(Arc::<[u8]>::from(transaction_program))
        .expect("prepare ZK private-input contract");
    for error in [
        resolve_prepared_contract_entrypoint(&transaction_contract, "commit")
            .expect_err("top-level transaction resolver must reject raw private witnesses"),
        reject_unavailable_private_input_entrypoint(&transaction_contract, "commit")
            .expect_err("prepared contracts must reject raw private witnesses"),
        resolve_prepared_raw_contract_entrypoint(&transaction_contract, "commit")
            .expect_err("raw contract resolver must reject raw private witnesses"),
    ] {
        assert!(
            matches!(error, ValidationFail::NotPermitted(ref message)
                    if message.contains("complete proof-carrying invocation statement")
                        && message.contains("Secret<T>")
                        && message.contains("seiyaku declaration")),
            "unexpected private-input admission error: {error}"
        );
    }
    let view_program =
        contract_program_with_private_input_entrypoint("inspect", EntryPointKind::View);
    let view_contract = ivm::prepare_contract(Arc::<[u8]>::from(view_program))
        .expect("prepare ZK private-input view");
    let error = resolve_prepared_contract_view_entrypoint(&view_contract, "inspect")
        .expect_err("view resolver must reject raw private witnesses");
    assert!(
        matches!(error, ValidationFail::NotPermitted(ref message)
                if message.contains("complete proof-carrying invocation statement")
                    && message.contains("Secret<T>")
                    && message.contains("seiyaku declaration")),
        "unexpected private-input view error: {error}"
    );
}
#[test]
fn raw_contract_dispatch_rejects_lifecycle_entrypoints() {
    use iroha_data_model::smart_contract::manifest::EntryPointKind;
    for (selector, kind) in [
        ("hajimari", EntryPointKind::Hajimari),
        ("kaizen", EntryPointKind::Kaizen),
    ] {
        let (program, _) = contract_program_with_entrypoint_kind(selector, kind, None);
        let mut metadata = Metadata::default();
        metadata.insert(
            Name::from_str("contract_entrypoint").expect("static name"),
            Json::new(selector.to_owned()),
        );
        let error = parse_contract_call_execution_context(&metadata, &program)
            .map_err(crate::execution_attempt::expect_completed_rejection)
            .expect_err("raw transaction dispatch must not invoke lifecycle hooks");
        assert!(
            matches!(error, ValidationFail::NotPermitted(message) if message.contains("top-level deployed ContractCall") && message.contains(selector))
        );
    }
}
#[test]
fn top_level_contract_invocation_uses_branded_lifecycle_permissions() {
    use iroha_data_model::smart_contract::manifest::EntryPointKind;
    let contract_address = ContractAddress::derive(
        &"hash:0000000000000000000000000000000000000000000000000000000000000001#C50E"
            .parse()
            .expect("canonical test network id"),
        &ALICE_ID,
        44,
        DataSpaceId::UNIVERSAL,
    )
    .expect("derive contract address");
    for (selector, kind, expected_permission) in [
        (
            "hajimari",
            EntryPointKind::Hajimari,
            EntrypointAuthorizationV1::RuntimeLifecycle,
        ),
        (
            "kaizen",
            EntryPointKind::Kaizen,
            EntrypointAuthorizationV1::RuntimeLifecycle,
        ),
    ] {
        let (program, _) = contract_program_with_entrypoint_kind(selector, kind, None);
        let invocation = ContractInvocation {
            contract_address: contract_address.clone(),
            expected_code_hash: iroha_crypto::Hash::new(selector.as_bytes()),
            entrypoint: selector.to_owned(),
            arguments: None,
        };
        let context = parse_contract_invocation_execution_context(
            &invocation,
            &program,
            None,
            contract_address.subject_id(),
        )
        .expect("top-level lifecycle invocation resolves");
        assert_eq!(
            context.entrypoint_authorization(),
            &expected_permission,
            "{selector} must use its runtime-defined branded lifecycle permission"
        );
    }
}
#[test]
fn contract_transaction_dispatch_rejects_view_entrypoints() {
    use iroha_data_model::smart_contract::manifest::EntryPointKind;
    let (program, _) = contract_program_with_entrypoint_kind("inspect", EntryPointKind::View, None);
    let mut metadata = Metadata::default();
    metadata.insert(
        Name::from_str("contract_entrypoint").expect("static name"),
        Json::new("inspect".to_owned()),
    );
    let err = parse_contract_call_execution_context(&metadata, &program)
        .map_err(crate::execution_attempt::expect_completed_rejection)
        .expect_err("view transaction dispatch must reject");
    assert!(matches!(
        err,
        ValidationFail::NotPermitted(message)
            if message.contains("read-only")
    ));
}
#[test]
fn contract_dispatch_context_rejects_implicit_main_for_self_describing_artifact() {
    let (program, _) = contract_program_with_entrypoint("main", Some("ContractAdmin"));
    let metadata = Metadata::default();
    let err = parse_contract_call_execution_context(&metadata, &program)
        .map_err(crate::execution_attempt::expect_completed_rejection)
        .expect_err("implicit main dispatch must reject");
    assert!(matches!(
        err,
        ValidationFail::NotPermitted(message)
            if message.contains("require explicit contract_entrypoint")
    ));
}
#[test]
fn contract_dispatch_context_keeps_generic_raw_ivm_without_selector_unclassified() {
    let metadata = Metadata::default();
    let context = parse_contract_call_execution_context(&metadata, &generate_ok_program())
        .expect("parse generic raw ivm context");
    assert!(context.is_none());
}
#[test]
fn direct_generic_ivm_remains_reachable_and_rejects_contract_metadata() {
    let authority = ALICE_ID.clone();
    let domain = Domain::new(DomainId::try_new("wonderland", "universal").expect("domain id"))
        .build(&authority);
    let account = Account::new(authority.clone()).build(&authority);
    let state = state_after_genesis(World::with([domain], [account], []));
    let mut program = ivm::ProgramMetadata {
        max_cycles: 100,
        ..ivm::ProgramMetadata::default()
    }
    .encode();
    program.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());
    let generic_code_hash = ivm::contract_code_hash(&program);
    let executable = Executable::Ivm(IvmBytecode::from_compiled(program));
    let transaction = |metadata: Metadata| {
        TransactionBuilder::new(
            state.network_id,
            authority.clone(),
            iroha_data_model::transaction::FeePaymentIntent::authority(
                Vec::new(),
                core::num::NonZeroU64::new(1_000_000),
            ),
        )
        .with_metadata(metadata)
        .with_executable(executable.clone())
        .sign(ALICE_KEYPAIR.private_key())
    };
    let generic_metadata = Metadata::default();
    let mut block = state.block(BlockHeader::new(
        nonzero!(2_u64),
        Some(
            state
                .view()
                .latest_block_hash()
                .expect("genuine genesis parent"),
        ),
        None,
        0,
        0,
    ));
    let signed = transaction(generic_metadata.clone());
    let mut state_transaction =
        block.transaction_for_fastpq_testing(Hash::from(signed.hash_as_entrypoint()));
    state_transaction.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
    state_transaction.world.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
    let mut ivm_cache = IvmCache::new();
    super::Executor::Initial
        .execute_transaction(&mut state_transaction, &authority, &signed, &mut ivm_cache)
        .map_err(crate::execution_attempt::expect_completed_rejection)
        .expect("contract-less generic IVM must execute at pc zero");
    state_transaction.apply();
    let mut reserved_metadata = generic_metadata;
    reserved_metadata.insert(
        "contract_manifest"
            .parse()
            .expect("contract-manifest metadata key"),
        Json::new("malformed-reserved-value"),
    );
    let signed = transaction(reserved_metadata);
    let mut state_transaction =
        block.transaction_for_fastpq_testing(Hash::from(signed.hash_as_entrypoint()));
    state_transaction.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
    state_transaction.world.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
    let error = super::Executor::Initial
        .execute_transaction(&mut state_transaction, &authority, &signed, &mut ivm_cache)
        .map_err(crate::execution_attempt::expect_completed_rejection)
        .expect_err("generic IVM must not accept contract metadata");
    assert!(
        error.to_string().contains("reserved `contract_manifest`"),
        "unexpected generic-metadata rejection: {error}"
    );
    drop(state_transaction);
    let mut state_transaction = block.transaction();
    state_transaction.world.contract_manifests.insert(
        iroha_data_model::smart_contract::ContractArtifactId::new(
            DataSpaceId::UNIVERSAL,
            generic_code_hash,
        ),
        iroha_data_model::smart_contract::manifest::ContractManifest {
            events: Vec::new(),
            enum_types: Vec::new(),
            permissions: Vec::new(),
            seiyaku_name: None,
            code_hash: Some(generic_code_hash),
            abi_hash: Some(Hash::prehashed(ivm::syscalls::compute_abi_hash(
                ivm::SyscallPolicy::AbiV1,
            ))),
            compiler_fingerprint: None,
            features_bitmap: None,
            access_set_hints: None,
            entrypoints: None,
            states: None,
            kotoba: None,
            error_messages: None,
            error_types: None,
            provenance: None,
        },
    );
    state_transaction.apply();
    let signed = transaction(Metadata::default());
    let mut state_transaction =
        block.transaction_for_fastpq_testing(Hash::from(signed.hash_as_entrypoint()));
    state_transaction.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
    state_transaction.world.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
    let error = super::Executor::Initial
        .execute_transaction(&mut state_transaction, &authority, &signed, &mut ivm_cache)
        .map_err(crate::execution_attempt::expect_completed_rejection)
        .expect_err("a manifest-bound hash must not execute as generic IVM");
    assert!(error.to_string().contains("contract manifest"));
    drop(state_transaction);
    let mut state_transaction = block.transaction();
    state_transaction.world.contract_manifests.remove(
        iroha_data_model::smart_contract::ContractArtifactId::new(
            DataSpaceId::UNIVERSAL,
            generic_code_hash,
        ),
    );
    state_transaction.apply();
    let signed = transaction(Metadata::default());
    let mut state_transaction =
        block.transaction_for_fastpq_testing(Hash::from(signed.hash_as_entrypoint()));
    state_transaction.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
    state_transaction.world.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
    state_transaction.pipeline.ivm_max_cycles_upper_bound = nonzero!(50_u64);
    let error = super::Executor::Initial
        .execute_transaction(&mut state_transaction, &authority, &signed, &mut ivm_cache)
        .map_err(crate::execution_attempt::expect_completed_rejection)
        .expect_err("direct generic IVM must honor the live cycle ceiling");
    assert!(matches!(
        error,
        ValidationFail::IvmAdmission(
            iroha_data_model::executor::IvmAdmissionError::MaxCyclesExceedsUpperBound(_)
        )
    ));
}
include!("executor_contract_dispatch_tests.rs");
include!("executor_privacy_occurrence_tests.rs");
include!("executor/argument_refusal_tests.rs");

#[test]
fn execution_fee_settlement_keeps_predecessor_across_local_recipe_and_sibling_changes() {
    for apply_sibling in [false, true] {
        let state = state_after_genesis(World::with(
            [],
            [Account::new(ALICE_ID.clone()).build(&ALICE_ID)],
            [],
        ));
        let original = state.world.parameters.view().get().clone();
        let fees = original.root_fees().unwrap();
        assert!(compute_nexus_fee_amount(fees, 1, 1, 1).unwrap().is_zero());
        let mut local = fees.clone();
        local.base_fee = Quantity::from(99_u32);
        local.per_gas_unit_fee = Quantity::from(7_u32);
        let mut successor = fees.clone();
        successor.base_fee = Quantity::from(17_u32);
        let signed = TransactionBuilder::new(
            *state.network_id_ref(),
            ALICE_ID.clone(),
            FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([Log::new(Level::INFO, "predecessor fee work".to_owned())])
        .sign(ALICE_KEYPAIR.private_key());
        let wire = signed.encode_wire_v1().unwrap();
        let mut block = state.block(BlockHeader::new(
            nonzero!(2_u64),
            state.view().latest_block_hash(),
            None,
            0,
            0,
        ));
        let mut execution =
            block.transaction_for_fastpq_testing(Hash::from(signed.hash_as_entrypoint()));
        execution.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
        execution.world.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
        execution.nexus.fees = local.clone();
        super::Executor::Initial
            .execute_transaction(&mut execution, &ALICE_ID, &signed, &mut IvmCache::new())
            .expect("real signed body uses the committed zero price, not the local recipe");
        assert!(execution.last_tx_gas_used > 0);
        // Even a later write in this failed business overlay cannot replace its fee owner.
        execution
            .world
            .set_parameter_for_testing(Parameter::Custom(
                successor.clone().into_custom_parameter().unwrap(),
            ))
            .unwrap();
        assert_eq!(execution.world.parameters().root_fees(), Some(&successor));
        assert!(root_fees::execution_parameters(&execution).same_version(&original));
        let settlement = execution.take_execution_fee_settlement().unwrap().unwrap();
        assert!(execution.take_execution_fee_settlement().unwrap().is_none());
        drop(execution);
        {
            let mut sibling = block.transaction();
            sibling
                .world
                .set_parameter_for_testing(Parameter::Custom(
                    successor.clone().into_custom_parameter().unwrap(),
                ))
                .unwrap();
            if apply_sibling {
                sibling.apply();
            }
        }
        let mut fresh =
            block.transaction_for_fastpq_testing(Hash::from(signed.hash_as_entrypoint()));
        fresh.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
        fresh.world.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
        fresh.current_tx_hash = Some(signed.hash());
        fresh.tx_call_hash = Some(Hash::from(signed.hash_as_entrypoint()));
        fresh.nexus.fees = local.clone();
        assert_eq!(
            fresh.world.parameters().root_fees(),
            Some(if apply_sibling { &successor } else { fees })
        );
        assert!(root_fees::execution_parameters(&fresh).same_version(&original));
        let parameters_before = fresh.world.parameters.get().clone();
        let assets_before: Vec<_> = fresh
            .world
            .assets()
            .iter()
            .map(|(id, balance)| (id.clone(), balance.clone()))
            .collect();
        let events_before = fresh.world.external_event_buf.len();
        assert!(matches!(settlement.settle(&mut fresh, &signed), Ok(false)));
        assert_eq!(
            fresh.nexus.fees, local,
            "settlement cannot rewrite the local author recipe"
        );
        assert_eq!(fresh.last_tx_gas_used, 0);
        assert!(fresh.pending_nexus_fee_receipt.is_none());
        assert!(
            fresh
                .world
                .parameters
                .get()
                .same_version(&parameters_before)
        );
        assert_eq!(
            fresh
                .world
                .assets()
                .iter()
                .map(|(id, balance)| (id.clone(), balance.clone()))
                .collect::<Vec<_>>(),
            assets_before
        );
        assert_eq!(fresh.world.external_event_buf.len(), events_before);
        assert_eq!(signed.encode_wire_v1().unwrap(), wire);
        signed.verify_signature().unwrap();
        drop(fresh);
        drop(block);
        assert!(state.world.parameters.view().get().same_version(&original));
    }
}

#[test]
fn execution_fee_settlement_refuses_equal_value_from_a_different_predecessor() {
    let state = state_after_genesis(World::with(
        [],
        [Account::new(ALICE_ID.clone()).build(&ALICE_ID)],
        [],
    ));
    let original = state.world.parameters.view().get().clone();
    let signed = TransactionBuilder::new(
        *state.network_id_ref(),
        ALICE_ID.clone(),
        FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([Log::new(
        Level::INFO,
        "exact predecessor fee work".to_owned(),
    )])
    .sign(ALICE_KEYPAIR.private_key());
    let wire = signed.encode_wire_v1().unwrap();
    let header = BlockHeader::new(
        nonzero!(2_u64),
        state.view().latest_block_hash(),
        None,
        0,
        0,
    );
    let settlement = {
        let mut block = state.block(header.clone());
        let mut execution =
            block.transaction_for_fastpq_testing(Hash::from(signed.hash_as_entrypoint()));
        execution.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
        execution.world.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
        super::Executor::Initial
            .execute_transaction(&mut execution, &ALICE_ID, &signed, &mut IvmCache::new())
            .expect("actual metered work");
        assert!(execution.last_tx_gas_used > 0);
        execution.take_execution_fee_settlement().unwrap().unwrap()
    };
    // Structural fixture replacement: equal policy values are not the same original owner.
    let replacement = crate::pipeline_gas_policy::RetainedStateParameters::try_copy_for_testing(
        original.get(),
        state.world.sccp_committee_generations.allocation_budget(),
    )
    .unwrap();
    assert_eq!(replacement, original);
    assert!(!replacement.same_version(&original));
    let mut parameters = crate::state::parameter_cell_block_for_testing(&state.world).unwrap();
    *parameters.get_mut() = replacement;
    parameters.commit();
    let mut block = state.block(header);
    let mut fresh = block.transaction_for_fastpq_testing(Hash::from(signed.hash_as_entrypoint()));
    fresh.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
    fresh.world.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
    fresh.current_tx_hash = Some(signed.hash());
    fresh.tx_call_hash = Some(Hash::from(signed.hash_as_entrypoint()));
    assert_eq!(root_fees::execution_parameters(&fresh), &original);
    assert!(!root_fees::execution_parameters(&fresh).same_version(&original));
    let parameters_before = fresh.world.parameters.get().clone();
    let assets_before: Vec<_> = fresh
        .world
        .assets()
        .iter()
        .map(|(id, balance)| (id.clone(), balance.clone()))
        .collect();
    let events_before = fresh.world.external_event_buf.len();
    let gas_before = fresh.pipeline.gas.clone();
    let local_before = fresh.nexus.fees.clone();
    assert!(matches!(settlement.settle(&mut fresh, &signed),
        Err(ExecutionFeeSettlementError::Owner(reason))
        if reason == "rejection fee record differs from its exact fresh source overlay"));
    assert!(
        fresh
            .world
            .parameters
            .get()
            .same_version(&parameters_before)
    );
    assert_eq!(
        fresh
            .world
            .assets()
            .iter()
            .map(|(id, balance)| (id.clone(), balance.clone()))
            .collect::<Vec<_>>(),
        assets_before
    );
    assert_eq!(fresh.world.external_event_buf.len(), events_before);
    assert_eq!(fresh.pipeline.gas, gas_before);
    assert_eq!(fresh.nexus.fees, local_before);
    assert_eq!(fresh.last_tx_gas_used, 0);
    assert!(fresh.pending_nexus_fee_receipt.is_none());
    assert_eq!(signed.encode_wire_v1().unwrap(), wire);
}

#[test]
fn execution_fee_meter_keeps_genesis_exemption_and_no_work_without_policy_fallback() {
    let state = state_for_testing(World::new());
    // An explicit unconfigured predecessor exercises the initial-genesis/no-work boundary.
    let mut parameters = crate::state::parameter_cell_block_for_testing(&state.world).unwrap();
    let mut authored = parameters.get().get().clone();
    authored
        .remove_custom_parameter(&RootFeePolicy::parameter_id())
        .unwrap();
    *parameters.get_mut() =
        crate::pipeline_gas_policy::RetainedStateParameters::try_copy_for_testing(
            &authored,
            state.world.sccp_committee_generations.allocation_budget(),
        )
        .unwrap();
    parameters.commit();
    let signed = TransactionBuilder::new(
        *state.network_id_ref(),
        ALICE_ID.clone(),
        FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([Log::new(Level::INFO, "exemption owner".to_owned())])
    .sign(ALICE_KEYPAIR.private_key());
    let mut block = state.block(BlockHeader::new(nonzero!(1_u64), None, None, 0, 0));
    for exemption in [
        ExecutionFeeExemption::Charged,
        ExecutionFeeExemption::Exempt,
    ] {
        let mut tx = block.transaction();
        assert!(is_initial_genesis_context(&tx));
        assert!(root_fees::execution_parameters(&tx).root_fees().is_none());
        tx.begin_execution_fee_meter(&signed, 1, exemption).unwrap();
        assert!(tx.begin_execution_fee_meter(&signed, 1, exemption).is_err());
        if exemption == ExecutionFeeExemption::Exempt {
            tx.record_execution_fee_instructions(1, 1).unwrap();
        }
        tx.close_execution_fee_meter();
        assert!(tx.take_execution_fee_settlement().unwrap().is_none());
        assert!(tx.take_execution_fee_settlement().unwrap().is_none());
        assert_eq!(tx.last_tx_gas_used, 0);
        assert!(tx.pending_nexus_fee_receipt.is_none());
    }
    drop(block);
    // An ordinary attempted root with no committed policy must never fall back
    // to the available local author recipe.
    let mut block = state.block(BlockHeader::new(nonzero!(2_u64), None, None, 0, 0));
    let mut attempted = block.transaction();
    assert!(!is_initial_genesis_context(&attempted));
    attempted
        .begin_execution_fee_meter(&signed, 1, ExecutionFeeExemption::Charged)
        .unwrap();
    attempted.record_execution_fee_instructions(1, 1).unwrap();
    attempted.close_execution_fee_meter();
    let settlement = attempted.take_execution_fee_settlement().unwrap().unwrap();
    drop(attempted);
    let mut fresh = block.transaction();
    fresh.current_tx_hash = Some(signed.hash());
    fresh.tx_call_hash = Some(Hash::from(signed.hash_as_entrypoint()));
    let gas_before = fresh.pipeline.gas.clone();
    let local_before = fresh.nexus.fees.clone();
    assert!(matches!(settlement.settle(&mut fresh, &signed),
        Err(ExecutionFeeSettlementError::Owner(reason))
        if reason == "rejection fee record has no original committed root fee policy"));
    assert_eq!(fresh.pipeline.gas, gas_before);
    assert_eq!(fresh.nexus.fees, local_before);
    assert_eq!(fresh.last_tx_gas_used, 0);
    assert!(fresh.pending_nexus_fee_receipt.is_none());
}

#[test]
fn installed_executor_denies_original_x509_request_before_native_or_state_effects() {
    use crate::executor::attachment_custody::{
        AttachmentCustody,
        x509::tests::{planned, request},
    };
    use iroha_data_model::transaction::{
        Executable, FeePaymentIntent, TransactionBuilder, TransactionEntrypoint,
        signed_attachments::*,
    };
    let authority = ALICE_ID.clone();
    let state = state_after_genesis(World::with(
        [],
        [Account::new(authority.clone()).build(&authority)],
        [],
    ));
    let request = request(*state.network_id_ref(), authority.clone());
    let mut plan = SignedAttachmentPlanV1::new();
    plan.push_action(planned(
        &request,
        ActionInvocationBindingV1::Direct {
            instruction_index: 0,
        },
        ActionAuthorityBindingV1::TransactionAuthority,
    ));
    let source = TransactionBuilder::new(
        *state.network_id_ref(),
        authority.clone(),
        FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([request])
    .with_attachment_plan(plan)
    .unwrap()
    .sign(iroha_test_samples::ALICE_KEYPAIR.private_key());
    let raw = data_model_executor::Executor::new(IvmBytecode::from_compiled(
        generate_denied_program("installed executor rejects X509 request"),
    ));
    let executor = super::Executor::UserProvided(super::LoadedExecutor::load(raw).unwrap());
    let mut block = state.block(BlockHeader::new(
        nonzero!(2_u64),
        state.view().latest_block_hash(),
        None,
        0,
        0,
    ));
    let mut tx = block.transaction();
    crate::state::seed_committed_transaction_context(
        &mut tx,
        &TransactionEntrypoint::External(source.clone()),
        0,
    );
    let before = tx.privacy_budget_for_testing();
    let Executable::Instructions(instructions) = source.instructions() else {
        panic!("direct signed source")
    };
    let mut custody = AttachmentCustody::new(&source).unwrap();
    let error = executor
        .execute_original_direct_instruction(
            &mut tx,
            &authority,
            &instructions[0],
            0,
            Some(&mut custody),
        )
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("installed executor rejects X509 request"),
        "{error}"
    );
    assert_eq!(tx.privacy_budget_for_testing(), before);
    assert!(tx.world.privacy_nullifiers.iter().next().is_none());
    assert!(custody.finish().is_err());
}
