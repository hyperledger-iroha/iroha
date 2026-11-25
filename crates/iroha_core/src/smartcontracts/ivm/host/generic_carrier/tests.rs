//! Capability separation, ordered read-own-writes and all-before-first-write admission.

use super::*;
use crate::runtime::generic_carriers::execution::tests as fixture;
use iroha_data_model::runtime::{
    carrier::{CarrierValueV1, encode_carrier_key_v1, encode_carrier_v1},
    definition::{
        CarrierNodeV1, CarrierSchemaV1, DefinitionDescriptorV1, ProjectionV1, SchemaV1,
        StorageDescriptorV1, StorageShapeV1,
    },
};
use iroha_test_samples::ALICE_ID;
use ivm::host::IVMHost;

fn schema() -> CarrierSchemaV1 {
    CarrierSchemaV1 {
        nodes: vec![CarrierNodeV1::U16],
    }
}

pub(crate) fn table(
    state: &mut StateTransaction<'_, '_>,
    reads: bool,
    writes: bool,
    effects: u32,
) -> DefinitionId {
    let storage = DefinitionDescriptorV1::Storage(StorageDescriptorV1 {
        namespace: "test.carrier.host.table.v1".into(),
        shape: StorageShapeV1::Table {
            key: SchemaV1::Carrier(schema()),
        },
        value: SchemaV1::Carrier(schema()),
        initial: None,
        invariants: Hash::new(b"carrier host fixture").into(),
        projection: ProjectionV1::BareNorito,
    });
    let id = storage.id().unwrap();
    state.world.runtime_definitions.insert(id, storage);
    let selection = state.world.runtime_selection.get_mut();
    selection.installed_storage.push(id);
    selection.installed_storage.sort();
    let binding = selection
        .bindings
        .iter_mut()
        .find(|binding| binding.wire_id == "test.generic.carrier.v1")
        .unwrap();
    let old = binding.implementation;
    let mut implementation = state.world.runtime_definitions.get(&old).unwrap().clone();
    let DefinitionDescriptorV1::Implementation(inner) = &mut implementation else {
        panic!("implementation");
    };
    inner.reads = if reads { vec![id] } else { vec![] };
    inner.writes = if writes { vec![id] } else { vec![] };
    inner.limits.max_effects = effects;
    let replacement = implementation.id().unwrap();
    binding.implementation = replacement;
    selection.active.retain(|id| *id != old);
    selection.active.push(replacement);
    selection.active.sort();
    state
        .world
        .runtime_definitions
        .insert(replacement, implementation);
    id
}

fn vm() -> IVM {
    let mut vm = IVM::new(100_000);
    vm.load_program_for_profile(
        &fixture::program(&[ivm_abi::encoding::wide::encode_halt()]),
        ivm::IvmProfileId::C10_GENERIC_CARRIER,
    )
    .unwrap();
    vm
}

fn input(vm: &mut IVM, bytes: &[u8]) -> u64 {
    vm.alloc_input_tlv(&ivm_abi::pointer_abi::encode_tlv(PointerType::NoritoBytes, bytes).unwrap())
        .unwrap()
}

fn prepare(vm: &mut IVM, key: &[u8], value: Option<&[u8]>) {
    let key = input(vm, key);
    let value = value.map(|value| input(vm, value));
    vm.set_register(10, 0);
    vm.set_register(11, key);
    vm.set_register(12, value.unwrap_or(0));
}

#[test]
fn original_carrier_read_views_keep_host_send_sync_without_copying_state() {
    fn send_sync<T: Send + Sync>() {}
    send_sync::<CarrierContext<'static>>();
    send_sync::<CoreHostImpl<'static, 'static, NoQueryState>>();

    let (state, instruction, candidate) =
        fixture::fixture(fixture::program(&[ivm_abi::encoding::wide::encode_halt()]));
    let mut block = state.block(fixture::header());
    candidate.install(&mut block);
    let mut transaction = block.transaction();
    let storage = table(&mut transaction, true, true, 2);
    let signed = fixture::source(&transaction, &instruction);
    fixture::bind_source(&mut transaction, &signed);
    let invocation = Invocation::capture(&transaction, &instruction).unwrap();
    let budget = transaction.execution_budget();
    let limit = budget.limit_bytes();
    let reserved = budget.reserved_bytes();
    budget.set_limit_bytes(reserved);
    let original = CarrierSource::capture(&transaction).unwrap();
    assert_eq!(budget.reserved_bytes(), reserved);
    assert!(std::ptr::eq(
        original.selection,
        transaction.world.runtime_selection.get()
    ));
    assert!(std::ptr::eq(
        original.definitions.get(&storage).unwrap(),
        transaction.world.runtime_definitions.get(&storage).unwrap()
    ));
    assert_eq!(original.identity, invocation.source);
    drop(original);
    assert_eq!(budget.reserved_bytes(), reserved);
    budget.set_limit_bytes(limit);
}

#[test]
fn declared_storage_uses_one_ordered_queue_and_exact_read_own_writes() {
    let (state, instruction, candidate) =
        fixture::fixture(fixture::program(&[ivm_abi::encoding::wide::encode_halt()]));
    let mut block = state.block(fixture::header());
    candidate.install(&mut block);
    let mut transaction = block.transaction();
    let storage = table(&mut transaction, true, true, 4);
    let source = fixture::source(&transaction, &instruction);
    fixture::bind_source(&mut transaction, &source);
    let mut host = CoreHostImpl::<'_, '_, NoQueryState>::for_carrier(
        ALICE_ID.clone(),
        &transaction,
        &instruction,
    )
    .unwrap();
    let mut vm = vm();
    let key = encode_carrier_key_v1(&schema(), &CarrierValueV1::U16(7)).unwrap();
    for number in [17, 21] {
        let value = encode_carrier_v1(&schema(), &CarrierValueV1::U16(number)).unwrap();
        prepare(&mut vm, &key, Some(&value));
        host.syscall(ivm::syscalls::SYSCALL_STATE_SET, &mut vm)
            .unwrap();
        prepare(&mut vm, &key, None);
        host.syscall(ivm::syscalls::SYSCALL_STATE_GET, &mut vm)
            .unwrap();
        assert_eq!(
            vm.memory.validate_tlv(vm.register(10)).unwrap().payload,
            value
        );
    }
    prepare(&mut vm, &key, None);
    host.syscall(ivm::syscalls::SYSCALL_STATE_DEL, &mut vm)
        .unwrap();
    prepare(&mut vm, &key, None);
    host.syscall(ivm::syscalls::SYSCALL_STATE_GET, &mut vm)
        .unwrap();
    assert_eq!(vm.register(10), 0);
    assert_eq!(host.queued.len(), 3);
    let artifacts = host.into_execution_artifacts(None).unwrap();
    assert!(
        transaction
            .world
            .runtime_generic_values
            .get(&CarrierStorageKeyV1 {
                storage,
                key: key.clone()
            })
            .is_none()
    );
    artifacts
        .apply_to_transaction(&mut transaction, &ALICE_ID)
        .unwrap();
    assert!(
        transaction
            .world
            .runtime_generic_values
            .get(&CarrierStorageKeyV1 { storage, key })
            .is_none()
    );
    transaction.finish_execution_effect_budget().unwrap();
}

#[test]
fn write_capability_never_grants_reads_and_read_capability_never_grants_writes() {
    for (reads, writes, forbidden) in [
        (false, true, ivm::syscalls::SYSCALL_STATE_GET),
        (true, false, ivm::syscalls::SYSCALL_STATE_SET),
    ] {
        let (state, instruction, candidate) =
            fixture::fixture(fixture::program(&[ivm_abi::encoding::wide::encode_halt()]));
        let mut block = state.block(fixture::header());
        candidate.install(&mut block);
        let mut transaction = block.transaction();
        table(&mut transaction, reads, writes, 2);
        let source = fixture::source(&transaction, &instruction);
        fixture::bind_source(&mut transaction, &source);
        let mut host = CoreHostImpl::<'_, '_, NoQueryState>::for_carrier(
            ALICE_ID.clone(),
            &transaction,
            &instruction,
        )
        .unwrap();
        let mut vm = vm();
        let key = encode_carrier_key_v1(&schema(), &CarrierValueV1::U16(7)).unwrap();
        let value = encode_carrier_v1(&schema(), &CarrierValueV1::U16(17)).unwrap();
        if writes {
            prepare(&mut vm, &key, Some(&value));
            host.syscall(ivm::syscalls::SYSCALL_STATE_SET, &mut vm)
                .unwrap();
        }
        let before = host.queued.len();
        prepare(&mut vm, &key, Some(&value));
        assert!(host.syscall(forbidden, &mut vm).is_err());
        assert_eq!(host.queued.len(), before);
        for forbidden in [
            ivm::syscalls::SYSCALL_STATE_SCAN,
            ivm::syscalls::SYSCALL_CALL_CONTRACT,
        ] {
            assert!(host.syscall(forbidden, &mut vm).is_err());
        }
        drop(host);
        assert_eq!(transaction.world.runtime_generic_values.iter().count(), 0);
    }
}

#[test]
fn malformed_rows_and_excess_output_never_publish_a_partial_queue() {
    let (state, instruction, candidate) =
        fixture::fixture(fixture::program(&[ivm_abi::encoding::wide::encode_halt()]));
    let mut block = state.block(fixture::header());
    candidate.install(&mut block);
    let mut transaction = block.transaction();
    table(&mut transaction, true, true, 1);
    let source = fixture::source(&transaction, &instruction);
    fixture::bind_source(&mut transaction, &source);
    let mut host = CoreHostImpl::<'_, '_, NoQueryState>::for_carrier(
        ALICE_ID.clone(),
        &transaction,
        &instruction,
    )
    .unwrap();
    let mut vm = vm();
    let key = encode_carrier_key_v1(&schema(), &CarrierValueV1::U16(7)).unwrap();
    let value = encode_carrier_v1(&schema(), &CarrierValueV1::U16(17)).unwrap();
    for (key, value) in [
        (&[1, 2, 3][..], value.as_slice()),
        (key.as_slice(), &[0][..]),
    ] {
        prepare(&mut vm, key, Some(value));
        assert!(
            host.syscall(ivm::syscalls::SYSCALL_STATE_SET, &mut vm)
                .is_err()
        );
        assert!(host.queued.is_empty());
    }
    prepare(&mut vm, &key, Some(&value));
    host.syscall(ivm::syscalls::SYSCALL_STATE_SET, &mut vm)
        .unwrap();
    prepare(&mut vm, &key, Some(&value));
    assert!(
        host.syscall(ivm::syscalls::SYSCALL_STATE_SET, &mut vm)
            .is_err()
    );
    assert!(host.into_execution_artifacts(None).is_err());
    assert_eq!(transaction.world.runtime_generic_values.iter().count(), 0);
}

#[test]
fn ordinary_host_and_stale_even_empty_artifacts_have_no_carrier_authority() {
    let mut ordinary = CoreHost::new(ALICE_ID.clone());
    let mut vm = vm();
    assert!(
        ordinary
            .syscall(ivm::syscalls::SYSCALL_ALLOC, &mut vm)
            .is_err()
    );
    let (state, instruction, candidate) =
        fixture::fixture(fixture::program(&[ivm_abi::encoding::wide::encode_halt()]));
    let mut block = state.block(fixture::header());
    candidate.install(&mut block);
    let mut transaction = block.transaction();
    let source = fixture::source(&transaction, &instruction);
    fixture::bind_source(&mut transaction, &source);
    let host = CoreHostImpl::<'_, '_, NoQueryState>::for_carrier(
        ALICE_ID.clone(),
        &transaction,
        &instruction,
    )
    .unwrap();
    let artifacts = host.into_execution_artifacts(None).unwrap();
    transaction.world.runtime_selection.get_mut().active.pop();
    assert!(
        artifacts
            .apply_to_transaction(&mut transaction, &ALICE_ID)
            .is_err()
    );
    assert_eq!(transaction.world.runtime_generic_values.iter().count(), 0);
}

#[test]
fn retained_artifacts_cannot_move_to_another_signed_source_with_the_same_selection() {
    let (state, instruction, candidate) =
        fixture::fixture(fixture::program(&[ivm_abi::encoding::wide::encode_halt()]));
    let mut block = state.block(fixture::header());
    candidate.install(&mut block);
    let mut transaction = block.transaction();
    let source = fixture::source(&transaction, &instruction);
    fixture::bind_source(&mut transaction, &source);
    let host = CoreHostImpl::<'_, '_, NoQueryState>::for_carrier(
        ALICE_ID.clone(),
        &transaction,
        &instruction,
    )
    .unwrap();
    let artifacts = host.into_execution_artifacts(None).unwrap();
    transaction.current_tx_hash = Some(HashOf::from_untyped_unchecked(Hash::new(
        b"another signed source",
    )));
    assert!(
        artifacts
            .apply_to_transaction(&mut transaction, &ALICE_ID)
            .is_err()
    );
    assert_eq!(transaction.world.runtime_generic_values.iter().count(), 0);
}

#[test]
fn carrier_constructor_refuses_original_budget_before_native_shell_or_queue_allocation() {
    let (state, instruction, candidate) =
        fixture::fixture(fixture::program(&[ivm_abi::encoding::wide::encode_halt()]));
    let mut block = state.block(fixture::header());
    candidate.install(&mut block);
    let mut transaction = block.transaction();
    table(&mut transaction, true, true, 2);
    let signed = fixture::source(&transaction, &instruction);
    fixture::bind_source(&mut transaction, &signed);
    Invocation::capture(&transaction, &instruction).unwrap();
    let budget = transaction.execution_budget();
    let before = budget.reserved_bytes();
    let limit = budget.limit_bytes();
    let authority = ALICE_ID.clone();
    budget.set_limit_bytes(before);
    let mut result = None;
    let allocations = crate::test_allocations::allocations_during(|| {
        result = Some(CoreHostImpl::<'_, '_, NoQueryState>::for_carrier(
            authority,
            &transaction,
            &instruction,
        ));
    });
    assert_eq!(
        allocations, 0,
        "refused carrier host created native shells or a replacement budget"
    );
    assert!(matches!(
        result.unwrap(),
        Err(ivm::VMError::Deferred(VmDeferral::AllocationDeferred(_)))
    ));
    assert_eq!(budget.reserved_bytes(), before);
    budget.set_limit_bytes(limit);
}

#[test]
fn carrier_constructor_owns_only_its_prepaid_queue_and_releases_it_on_drop() {
    let (state, instruction, candidate) =
        fixture::fixture(fixture::program(&[ivm_abi::encoding::wide::encode_halt()]));
    let mut block = state.block(fixture::header());
    candidate.install(&mut block);
    let mut transaction = block.transaction();
    table(&mut transaction, true, true, 2);
    let signed = fixture::source(&transaction, &instruction);
    fixture::bind_source(&mut transaction, &signed);
    Invocation::capture(&transaction, &instruction).unwrap();
    let budget = transaction.execution_budget();
    let before = budget.reserved_bytes();
    let authority = ALICE_ID.clone();
    let mut result = None;
    let allocations = crate::test_allocations::allocations_during(|| {
        result = Some(CoreHostImpl::<'_, '_, NoQueryState>::for_carrier(
            authority,
            &transaction,
            &instruction,
        ));
    });
    let mut host = result.unwrap().unwrap();
    assert_eq!(
        allocations, 1,
        "only the exactly reserved ordered-effect Vec is constructed"
    );
    assert!(!host.default.is_present());
    assert!(!host.codec_host.is_present());
    assert!(!host.crypto.is_present());
    assert!(!host.accounts_snapshot.is_present());
    assert!(!host.prepared_contract_cache.is_present());
    assert!(!host.zk_verified_ballot.is_present());
    assert!(!host.zk_verified_tally.is_present());
    assert!(!host.zk_roots.is_present());
    assert!(!host.zk_elections.is_present());
    assert!(!host.axt_policy.is_present());
    assert!(!host.zk_last_env_hash_ballot.is_present());
    assert!(!host.zk_last_env_hash_tally.is_present());
    assert!(!host.axt_issuer_keys.is_present());
    assert!(!host.axt_proof_cache.is_present());
    assert!(budget.reserved_bytes() > before);
    host.begin_tx(&Default::default()).unwrap();
    let mut vm = vm();
    assert!(
        host.prepare_syscall(ivm::syscalls::SYSCALL_GET_AUTHORITY, vm.syscall_quote())
            .is_err()
    );
    assert!(
        host.syscall(ivm::syscalls::SYSCALL_GET_AUTHORITY, &mut vm)
            .is_err()
    );
    drop(host);
    assert_eq!(budget.reserved_bytes(), before);
}

#[test]
fn carrier_queue_allocator_refusal_preserves_original_cause_and_returns_all_credit() {
    let (state, instruction, candidate) =
        fixture::fixture(fixture::program(&[ivm_abi::encoding::wide::encode_halt()]));
    let mut block = state.block(fixture::header());
    candidate.install(&mut block);
    let mut transaction = block.transaction();
    table(&mut transaction, true, true, 2);
    let signed = fixture::source(&transaction, &instruction);
    fixture::bind_source(&mut transaction, &signed);
    let budget = transaction.execution_budget();
    let before = budget.reserved_bytes();
    let authority = ALICE_ID.clone();
    let layout = std::alloc::Layout::array::<QueuedEffect>(2).unwrap();
    let (result, refused) = crate::test_allocations::refuse_one_layout_during(layout, || {
        CoreHostImpl::<'_, '_, NoQueryState>::for_carrier(authority, &transaction, &instruction)
    });
    assert!(refused);
    assert!(matches!(
        result,
        Err(ivm::VMError::Deferred(VmDeferral::AllocatorDeferred(_)))
    ));
    assert_eq!(budget.reserved_bytes(), before);
}
