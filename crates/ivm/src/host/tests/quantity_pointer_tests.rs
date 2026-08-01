//! Canonical quantity-pointer admission tests for the default IVM host.

use super::*;

#[test]
fn expect_tlv_enforces_pointer_policy() {
    crate::set_banner_enabled(false);
    let mut vm = IVM::new(u64::MAX);
    let program = ProgramMetadata::default_for(1, 0, 1).encode();
    vm.load_program(&program).expect("load program");
    // The first release only supports ABI v1; installing any other
    // annotated ABI version must fail closed during pointer validation.
    let _guard = crate::pointer_abi::PointerPolicyGuard::install(crate::SyscallPolicy::AbiV1, 2);
    let mut tlv = Vec::new();
    tlv.extend_from_slice(&(PointerType::AccountId as u16).to_be_bytes());
    tlv.push(1);
    tlv.extend_from_slice(&0u32.to_be_bytes());
    let hash: [u8; 32] = iroha_crypto::Hash::new([]).into();
    tlv.extend_from_slice(&hash);
    let ptr = vm.alloc_input_tlv(&tlv).expect("allocate TLV");
    vm.set_register(10, ptr);
    let err = DefaultHost::expect_tlv(&vm, 10, PointerType::AccountId).unwrap_err();
    assert!(matches!(
        err,
        VMError::AbiTypeNotAllowed { abi: 2, type_id } if type_id == PointerType::AccountId as u16
    ));
}

#[test]
fn quantity_arguments_require_canonical_quantity_pointer() {
    crate::set_banner_enabled(false);
    let mut vm = IVM::new(u64::MAX);
    let canonical = "1.25".parse::<Quantity>().expect("canonical quantity");
    let canonical_payload = QuantityValueV1::new(canonical.clone())
        .encode_frame()
        .expect("encode canonical quantity frame");
    let canonical_ptr = vm
        .alloc_input_tlv(&test_tlv(PointerType::Quantity, &canonical_payload))
        .expect("allocate canonical quantity");
    vm.set_register(13, canonical_ptr);
    assert_eq!(
        DefaultHost::expect_quantity(&vm, 13),
        Ok(canonical_payload.len())
    );

    let account_ptr = vm
        .alloc_input_tlv(&test_tlv(PointerType::AccountId, &[]))
        .expect("allocate account fixture");
    let definition_ptr = vm
        .alloc_input_tlv(&test_tlv(PointerType::AssetDefinitionId, &[]))
        .expect("allocate asset definition fixture");
    let dataspace_ptr = vm
        .alloc_input_tlv(&test_tlv(PointerType::DataSpaceId, &[]))
        .expect("allocate dataspace fixture");
    vm.set_register(10, account_ptr);
    vm.set_register(11, account_ptr);
    vm.set_register(12, definition_ptr);
    vm.set_register(14, dataspace_ptr);
    let expected_gas = DefaultHost::mutation_gas(canonical_payload.len());

    let mut scoped_host = DefaultHost::new();
    assert_eq!(
        scoped_host.prepare_syscall(syscalls::SYSCALL_TRANSFER_ASSET_SCOPED, &vm),
        Ok(expected_gas)
    );
    assert_eq!(
        scoped_host.syscall(syscalls::SYSCALL_TRANSFER_ASSET_SCOPED, &mut vm),
        Ok(expected_gas)
    );

    let mut batch_host = DefaultHost::new();
    assert_eq!(
        batch_host.syscall(syscalls::SYSCALL_TRANSFER_V1_BATCH_BEGIN, &mut vm),
        Ok(gas::G_FASTPQ_BATCH)
    );
    assert_eq!(
        batch_host.syscall(syscalls::SYSCALL_TRANSFER_V1, &mut vm),
        Ok(expected_gas)
    );
    assert!(batch_host.fastpq_batch_has_entries);

    let legacy_payload =
        norito::to_bytes(&canonical.into_numeric()).expect("encode legacy Numeric");
    let legacy_ptr = vm
        .alloc_input_tlv(&test_tlv(PointerType::NoritoBytes, &legacy_payload))
        .expect("allocate legacy Numeric pointer");
    vm.set_register(13, legacy_ptr);
    assert_eq!(
        DefaultHost::expect_quantity(&vm, 13),
        Err(VMError::NoritoInvalid)
    );

    let noncanonical = Numeric::new(1_250_u32, 3);
    let noncanonical_ptr = vm
        .alloc_input_tlv(&test_tlv(
            PointerType::Quantity,
            &norito::to_bytes(&noncanonical).expect("encode noncanonical quantity"),
        ))
        .expect("allocate noncanonical quantity");
    vm.set_register(13, noncanonical_ptr);
    assert_eq!(
        DefaultHost::expect_quantity(&vm, 13),
        Err(VMError::DecodeError)
    );
}

#[test]
fn oversized_quantity_fails_from_bounded_header_before_hash_or_mutation() {
    crate::set_banner_enabled(false);
    let mut vm = IVM::new(u64::MAX);

    let account_ptr = vm
        .alloc_input_tlv(&test_tlv(PointerType::AccountId, &[]))
        .expect("allocate account fixture");
    let definition_ptr = vm
        .alloc_input_tlv(&test_tlv(PointerType::AssetDefinitionId, &[]))
        .expect("allocate asset definition fixture");
    let dataspace_ptr = vm
        .alloc_input_tlv(&test_tlv(PointerType::DataSpaceId, &[]))
        .expect("allocate dataspace fixture");
    let oversized = test_tlv(
        PointerType::Quantity,
        &[0xa5; MAX_QUANTITY_FRAME_BYTES_V1 + 1],
    );
    let valid_oversized_ptr = vm
        .alloc_input_tlv(&oversized)
        .expect("allocate oversized quantity with a valid digest");
    let mut corrupted_oversized = oversized;
    *corrupted_oversized
        .last_mut()
        .expect("quantity envelope has a digest") ^= 1;
    let quantity_ptr = vm
        .alloc_input_tlv(&corrupted_oversized)
        .expect("allocate oversized quantity with a corrupt digest");
    let mut impossible_length = test_tlv(PointerType::Quantity, &[]);
    impossible_length[3..7].copy_from_slice(&u32::MAX.to_be_bytes());
    let impossible_length_ptr = vm
        .alloc_input_tlv(&impossible_length)
        .expect("allocate quantity with an impossible declared length");
    let maximum_sized_noncanonical_ptr = vm
        .alloc_input_tlv(&test_tlv(
            PointerType::Quantity,
            &[0x5a; MAX_QUANTITY_FRAME_BYTES_V1],
        ))
        .expect("allocate maximum-sized noncanonical quantity");

    vm.set_register(13, maximum_sized_noncanonical_ptr);
    assert_eq!(
        read_bounded_tlv_payload_len_at(
            &vm,
            maximum_sized_noncanonical_ptr,
            PointerType::Quantity,
            MAX_QUANTITY_FRAME_BYTES_V1,
        ),
        Ok(MAX_QUANTITY_FRAME_BYTES_V1),
        "the exact V1 maximum must reach canonical frame validation"
    );
    assert_eq!(
        DefaultHost::expect_quantity(&vm, 13),
        Err(VMError::DecodeError),
        "size admission must not replace canonical frame validation"
    );

    for (label, pointer) in [
        ("valid digest", valid_oversized_ptr),
        ("corrupt digest", quantity_ptr),
        ("impossible declared length", impossible_length_ptr),
    ] {
        vm.set_register(13, pointer);
        vm.memory.clear_tracking();
        assert_eq!(
            DefaultHost::expect_quantity(&vm, 13),
            Err(VMError::NoritoInvalid),
            "{label}"
        );
        assert_eq!(
            vm.memory.read_set(),
            vec![crate::memory::AccessRange {
                addr: pointer,
                len: 7,
            }],
            "{label} must be rejected after the fixed header only"
        );
        assert!(vm.memory.write_log().is_empty(), "{label}");
    }

    vm.set_register(10, account_ptr);
    vm.set_register(11, account_ptr);
    vm.set_register(12, definition_ptr);
    vm.set_register(13, quantity_ptr);
    vm.set_register(14, dataspace_ptr);
    let registers_before = [
        vm.register(10),
        vm.register(11),
        vm.register(12),
        vm.register(13),
        vm.register(14),
    ];

    let mut scoped_host = DefaultHost::new();
    vm.memory.clear_tracking();
    assert_eq!(
        scoped_host.prepare_syscall(syscalls::SYSCALL_TRANSFER_ASSET_SCOPED, &vm),
        Err(VMError::NoritoInvalid)
    );
    assert!(
        vm.memory.read_set().is_empty(),
        "header-only preparation must reject the oversized frame before reading its payload"
    );
    assert!(vm.memory.write_log().is_empty());

    vm.memory.clear_tracking();
    assert_eq!(
        scoped_host.syscall(syscalls::SYSCALL_TRANSFER_ASSET_SCOPED, &mut vm),
        Err(VMError::NoritoInvalid)
    );
    assert_eq!(
        vm.memory.read_set(),
        vec![crate::memory::AccessRange {
            addr: quantity_ptr,
            len: 7,
        }],
        "scoped transfer must read only the bounded header before rejecting the payload"
    );
    assert!(vm.memory.write_log().is_empty());
    assert_eq!(
        [
            vm.register(10),
            vm.register(11),
            vm.register(12),
            vm.register(13),
            vm.register(14),
        ],
        registers_before
    );

    let mut batch_host = DefaultHost::new();
    assert_eq!(
        batch_host.syscall(syscalls::SYSCALL_TRANSFER_V1_BATCH_BEGIN, &mut vm),
        Ok(gas::G_FASTPQ_BATCH)
    );
    vm.memory.clear_tracking();
    assert_eq!(
        batch_host.syscall(syscalls::SYSCALL_TRANSFER_V1, &mut vm),
        Err(VMError::NoritoInvalid)
    );
    assert_eq!(
        vm.memory.read_set(),
        vec![crate::memory::AccessRange {
            addr: quantity_ptr,
            len: 7,
        }],
        "batch transfer must read only the bounded header before rejecting the payload"
    );
    assert!(vm.memory.write_log().is_empty());
    assert!(batch_host.fastpq_batch_active);
    assert!(!batch_host.fastpq_batch_has_entries);
}
