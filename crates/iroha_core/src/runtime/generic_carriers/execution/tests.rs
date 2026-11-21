//! Actual C10 execution, source-bound effect groups and charged rejection regressions.
//!
//! These direct component fixtures install the dormant selection explicitly. They do not
//! establish governed activation, Network admission or whole-node qualification.

use super::*;
use crate::{
    kura::Kura,
    query::store::LiveQueryStore,
    state::{State, World},
};
use iroha_crypto::Hash;
use iroha_data_model::{
    block::{BlockHeader, consensus::SumeragiRootScope},
    runtime::definition::{DefinitionDescriptorV1, ImplementationCodeV1},
    smart_contract::ContractArtifactId,
    transaction::{FeePaymentIntent, SignedTransaction, TransactionBuilder},
};
use iroha_model_base::topology::DataSpaceId;
use iroha_test_samples::{ALICE_ID, ALICE_KEYPAIR};
use ivm_abi::{encoding::wide, instruction::wide as isa};

pub(crate) fn program(words: &[u32]) -> Vec<u8> {
    let mut bytes =
        ivm::ProgramMetadata::default().encode_for_profile(IvmProfileId::C10_GENERIC_CARRIER, 0);
    for word in words {
        bytes.extend_from_slice(&word.to_le_bytes());
    }
    bytes
}

/// An unsupported component candidate stays in a disposable block, never restored State.
pub(crate) struct Candidate {
    pub(crate) artifact: ContractArtifactId,
    selection: iroha_data_model::runtime::definition::RootSelectionV1,
    published: std::collections::BTreeMap<
        iroha_data_model::runtime::definition::DefinitionId,
        DefinitionDescriptorV1,
    >,
    program: Vec<u8>,
}

impl Candidate {
    pub(crate) fn install(&self, block: &mut crate::state::StateBlock<'_>) {
        *block.world.runtime_selection.get_mut() = self.selection.clone();
        for (id, definition) in &self.published {
            block
                .world
                .runtime_definitions
                .insert(*id, definition.clone());
        }
        let retained = crate::state::generic_carrier_storage::RetainedCarrierBytes::admit(
            &self.program,
            block.world.runtime_generic_artifacts.allocation_budget(),
        )
        .unwrap();
        block
            .world
            .runtime_generic_artifacts
            .try_insert_admitted(self.artifact, retained)
            .unwrap();
    }
}

pub(crate) fn fixture(program: Vec<u8>) -> (State, InstructionBox, Candidate) {
    let mut fixture = super::super::tests::fixture();
    let binding = fixture
        .selection
        .bindings
        .iter_mut()
        .find(|binding| binding.wire_id == "test.generic.carrier.v1")
        .unwrap();
    let old_id = binding.implementation;
    let mut implementation = fixture.published.get(&old_id).unwrap().clone();
    let DefinitionDescriptorV1::Implementation(definition) = &mut implementation else {
        panic!("implementation");
    };
    let hash = ivm::contract_code_hash(&program);
    definition.code = ImplementationCodeV1::Ivm {
        code_hash: hash.into(),
        entrypoint: "main".into(),
    };
    definition.limits.max_gas = 10_000;
    let id = implementation.id().unwrap();
    binding.implementation = id;
    fixture.selection.active.retain(|id| *id != old_id);
    fixture.selection.active.push(id);
    fixture.selection.active.sort();
    fixture.published.insert(id, implementation);
    fixture.selection.check_closure(&fixture.published).unwrap();
    let world = World::new();
    let mut block = world.block();
    block
        .set_parameter_for_testing(crate::sumeragi::lanes::routing::test_support::metadata(
            SumeragiRootScope::Global,
        ))
        .unwrap();
    block.commit();
    (
        State::new_for_testing(
            world,
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        ),
        fixture.instruction,
        Candidate {
            artifact: ContractArtifactId::new(DataSpaceId::UNIVERSAL, hash),
            selection: fixture.selection,
            published: fixture.published,
            program,
        },
    )
}

pub(crate) fn header() -> BlockHeader {
    BlockHeader::new(10.try_into().unwrap(), None, None, 1_000, 0)
}

pub(crate) fn source(
    state: &StateTransaction<'_, '_>,
    instruction: &InstructionBox,
) -> SignedTransaction {
    TransactionBuilder::new(
        *state.network_id(),
        ALICE_ID.clone(),
        FeePaymentIntent::authority(Vec::new(), 10_000.try_into().ok()),
    )
    .with_instructions([instruction.clone()])
    .sign(ALICE_KEYPAIR.private_key())
}

pub(crate) fn bind_source(state: &mut StateTransaction<'_, '_>, signed: &SignedTransaction) {
    state.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
    state.world.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
    state.current_tx_hash = Some(signed.hash());
    state.tx_call_hash = Some(Hash::from(signed.hash_as_entrypoint()));
    state.begin_execution_effect_budget(signed).unwrap();
    let iroha_data_model::transaction::Executable::Instructions(instructions) =
        signed.instructions()
    else {
        panic!("original instructions");
    };
    state
        .admit_authored_execution_effects(instructions.iter())
        .unwrap();
}

#[test]
fn successful_vm_retains_actual_gas_and_consumes_one_original_effect_group() {
    let (state, instruction, candidate) = fixture(program(&[
        wide::encode_ri(isa::arithmetic::ADDI, 10, 0, 0),
        wide::encode_halt(),
    ]));
    let mut block = state.block(header());
    candidate.install(&mut block);
    let mut transaction = block.transaction();
    let signed = source(&transaction, &instruction);
    bind_source(&mut transaction, &signed);
    let report = execute(&mut transaction, &ALICE_ID, &instruction, 10_000).unwrap();
    report.verdict.unwrap();
    assert!(report.gas_used > 0 && report.gas_used < 10_000);
    // An identical second run cannot consume the signed original's artifact group again.
    let repeated = execute(&mut transaction, &ALICE_ID, &instruction, 10_000).unwrap();
    assert!(repeated.verdict.is_err());
    assert_eq!(repeated.gas_used, report.gas_used);
    assert!(transaction.finish_execution_effect_budget().is_err());
}

#[test]
fn vm_trap_and_failed_application_preserve_work_without_publishing_effects() {
    for words in [
        vec![
            wide::encode_ri(isa::arithmetic::ADDI, 10, 0, 1),
            wide::encode_halt(),
        ],
        vec![
            wide::encode_ri(isa::arithmetic::ADDI, 10, 0, 0),
            wide::encode_halt(),
        ],
    ] {
        let (state, instruction, candidate) = fixture(program(&words));
        let mut block = state.block(header());
        candidate.install(&mut block);
        let mut transaction = block.transaction();
        transaction.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
        transaction.world.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
        let signed = source(&transaction, &instruction);
        transaction.current_tx_hash = Some(signed.hash());
        transaction.tx_call_hash = Some(Hash::from(signed.hash_as_entrypoint()));
        // The first program fails its return contract. The second runs successfully but its
        // artifacts cannot apply without the actual signed effect owner.
        let report = execute(&mut transaction, &ALICE_ID, &instruction, 10_000).unwrap();
        assert!(report.verdict.is_err());
        assert!(report.gas_used > 0);
        assert_eq!(transaction.world.runtime_generic_values.iter().count(), 0);
    }
}

#[test]
fn exact_artifact_bytes_and_captured_storage_scope_are_required_before_execution() {
    let (state, instruction, candidate) = fixture(program(&[wide::encode_halt()]));
    let mut block = state.block(header());
    candidate.install(&mut block);
    let mut transaction = block.transaction();
    let signed = source(&transaction, &instruction);
    bind_source(&mut transaction, &signed);
    let substitute = program(&[
        wide::encode_ri(isa::arithmetic::ADDI, 10, 0, 0),
        wide::encode_halt(),
    ]);
    super::super::storage::stage_artifact(&mut transaction, candidate.artifact, &substitute)
        .unwrap();
    let report = execute(&mut transaction, &ALICE_ID, &instruction, 10_000).unwrap();
    assert!(report.verdict.is_err());
    assert_eq!(report.gas_used, 0);
    transaction.current_dataspace_id = Some(DataSpaceId::new(17));
    let report = execute(&mut transaction, &ALICE_ID, &instruction, 10_000).unwrap();
    assert!(report.verdict.is_err());
    assert_eq!(report.gas_used, 0);
}

#[test]
fn actual_vm_out_of_gas_retains_the_exact_consumed_allowance() {
    let (state, instruction, candidate) = fixture(program(&[
        wide::encode_ri(isa::arithmetic::ADDI, 10, 0, 0),
        wide::encode_offset24(isa::control::JMP, 0),
    ]));
    let mut block = state.block(header());
    candidate.install(&mut block);
    let mut transaction = block.transaction();
    let signed = source(&transaction, &instruction);
    bind_source(&mut transaction, &signed);
    let report = execute(&mut transaction, &ALICE_ID, &instruction, 1_000).unwrap();
    assert!(report.verdict.is_err());
    assert_eq!(report.gas_used, 1_000);
    assert_eq!(transaction.world.runtime_generic_values.iter().count(), 0);
}

#[test]
fn actual_vm_state_set_uses_original_queue_and_disposable_world_transaction() {
    use iroha_data_model::runtime::{
        carrier::{CarrierStorageKeyV1, CarrierValueV1, encode_carrier_key_v1, encode_carrier_v1},
        definition::{CarrierNodeV1, CarrierSchemaV1},
    };
    let schema = CarrierSchemaV1 {
        nodes: vec![CarrierNodeV1::U16],
    };
    let key = encode_carrier_key_v1(&schema, &CarrierValueV1::U16(7)).unwrap();
    let literal =
        ivm_abi::pointer_abi::encode_tlv(ivm::pointer_abi::PointerType::NoritoBytes, &key).unwrap();
    for reject_after_queue in [false, true] {
        let mut image = program(&[]);
        let padding = (4 - ((24 + literal.len()) % 4)) % 4;
        image.extend_from_slice(&ivm_abi::metadata::LITERAL_SECTION_MAGIC);
        image.extend_from_slice(&1_u32.to_le_bytes());
        image.extend_from_slice(&(padding as u32).to_le_bytes());
        image.extend_from_slice(&(literal.len() as u32).to_le_bytes());
        image.extend_from_slice(
            &ivm_abi::metadata::encode_literal_descriptor(
                ivm_abi::metadata::LiteralKindV1::PointerTlv,
                24,
            )
            .unwrap()
            .to_le_bytes(),
        );
        image.extend_from_slice(&literal);
        image.extend(std::iter::repeat_n(0, padding));
        for word in [
            wide::encode_ri(isa::arithmetic::ADDI, 12, 10, 0),
            wide::encode_ri(isa::arithmetic::ADDI, 10, 0, 0),
            wide::encode_literal(isa::memory::LDLIT, 11, 0),
            wide::encode_scall(ivm::syscalls::SYSCALL_STATE_SET),
            wide::encode_ri(
                isa::arithmetic::ADDI,
                10,
                0,
                if reject_after_queue { 1 } else { 0 },
            ),
            wide::encode_halt(),
        ] {
            image.extend_from_slice(&word.to_le_bytes());
        }
        let (state, instruction, candidate) = fixture(image);
        let mut block = state.block(header());
        candidate.install(&mut block);
        let mut transaction = block.transaction();
        let storage = crate::smartcontracts::ivm::host::generic_carrier::tests::table(
            &mut transaction,
            false,
            true,
            1,
        );
        let signed = source(&transaction, &instruction);
        bind_source(&mut transaction, &signed);
        let report = execute(&mut transaction, &ALICE_ID, &instruction, 10_000).unwrap();
        assert_eq!(report.verdict.is_err(), reject_after_queue);
        assert!(report.gas_used > 0);
        let key = CarrierStorageKeyV1 {
            storage,
            key: key.clone(),
        };
        let stored = transaction.world.runtime_generic_values.get(&key);
        if reject_after_queue {
            assert!(stored.is_none());
        } else {
            assert_eq!(
                stored.unwrap().as_slice(),
                encode_carrier_v1(&schema, &CarrierValueV1::U16(256)).unwrap()
            );
        }
        transaction.finish_execution_effect_budget().unwrap();
        drop(transaction);
        drop(block);
        assert!(
            state
                .world
                .runtime_generic_values
                .view()
                .get(&key)
                .is_none()
        );
    }
}

#[test]
fn ordinary_artifact_interfaces_never_acquire_the_carrier_profile() {
    let bytes = program(&[wide::encode_halt()]);
    let (state, _, candidate) = fixture(bytes.clone());
    let mut block = state.block(header());
    candidate.install(&mut block);
    let world = &block.world;
    assert_eq!(
        crate::smartcontracts::ivm::profile::resolve_artifact(world, &bytes).id(),
        IvmProfileId::BASELINE_ABI_V1
    );
    assert_eq!(
        crate::smartcontracts::ivm::profile::resolve_raw_artifact(world, &bytes).id(),
        IvmProfileId::BASELINE_ABI_V1
    );
    assert!(
        crate::smartcontracts::ivm::profile::resolve_abi_hash(
            world,
            &IvmProfileId::C10_GENERIC_CARRIER.abi_hash()
        )
        .is_none()
    );
}

#[test]
fn retained_original_execution_refusal_is_not_taken_then_reinserted() {
    let (state, instruction, candidate) = fixture(program(&[wide::encode_halt()]));
    let mut block = state.block(header());
    candidate.install(&mut block);
    let mut transaction = block.transaction();
    let pool = iroha_allocation::AllocationBudget::new(1);
    let _occupied = pool.try_reserve_bytes(1).unwrap();
    let original = pool.try_reserve_bytes(1).unwrap_err();
    let expected = original.clone();
    transaction
        .world
        .record_execution_deferral(ExecutionDeferred::from(original));
    assert!(matches!(
        execute(&mut transaction, &ALICE_ID, &instruction, 1_000),
        Err(CarrierExecutionRefusal::ExecutionRetained)
    ));
    let exported = transaction.ensure_execution_ready().unwrap_err();
    assert_eq!(exported.allocation_refusal(), Some(&expected));
    assert!(
        transaction
            .ensure_execution_ready()
            .unwrap_err()
            .allocation_refusal()
            .is_none()
    );
}

#[test]
fn retained_original_storage_refusal_never_becomes_an_execution_or_semantic_verdict() {
    let (state, instruction, candidate) = fixture(program(&[wide::encode_halt()]));
    let mut block = state.block(header());
    candidate.install(&mut block);
    let mut transaction = block.transaction();
    let pool = iroha_allocation::AllocationBudget::new(1);
    let _occupied = pool.try_reserve_bytes(1).unwrap();
    let original = pool.try_reserve_bytes(1).unwrap_err();
    let expected = crate::state::StateStorageAdmissionError::World(
        mv::storage::AdmittedStorageError::Allocation(original),
    );
    transaction.arm_local_storage_refusal(expected.clone());
    assert!(matches!(
        execute(&mut transaction, &ALICE_ID, &instruction, 1_000),
        Err(CarrierExecutionRefusal::Storage(original)) if original == expected
    ));
    assert_eq!(
        transaction.require_storage_admission(),
        Err(expected.clone())
    );
    transaction.ensure_execution_ready().unwrap();
    assert!(
        transaction
            .world
            .runtime_generic_values
            .iter()
            .next()
            .is_none()
    );
    drop(transaction);
    assert_eq!(block.require_storage_admission(), Err(expected));
}
