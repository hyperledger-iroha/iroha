// Actual host boundaries for native custody keys. Included in host::tests.
// These local host fixtures do not certify deployed transaction admission or consensus.

fn custody_namespace_paths() -> [StatePath; 5] {
    use crate::query::stream_token_custody::{head_key, height_key, key_path, record_key};
    let provider = iroha_data_model::sorafs::capacity::ProviderId::new([37; 32]);
    [
        head_key(provider),
        record_key(provider, 8194),
        height_key(provider, 17, 1),
        key_path(provider, true, ALICE_KEYPAIR.public_key()).expect("signer first-use key"),
        key_path(provider, false, BOB_KEYPAIR.public_key()).expect("attester first-use key"),
    ]
}

fn custody_namespace_scoped_host() -> CoreHost {
    let authority = ALICE_ID.clone();
    let contract = ContractAddress::derive(
        &"hash:0000000000000000000000000000000000000000000000000000000000000001#C50E"
            .parse()
            .expect("test network"),
        &authority,
        947,
        DataSpaceId::UNIVERSAL,
    )
    .expect("contract address");
    let mut host = CoreHost::new(authority);
    host.set_contract_runtime_context(Some(ContractRuntimeExecutionContext {
        contract_subject: contract.subject_id(),
        contract_address: contract,
        contract_alias: None,
        entrypoint: "main".to_owned(),
    }));
    assert_eq!(host.execution_class, HostExecutionClass::Contract);
    assert!(!host.local_debug_artifacts);
    host
}

fn custody_namespace_vm(paths: &[StatePath]) -> IVM {
    let program = build_authenticated_test_contract_program_with_states(
        &ivm::encoding::wide::encode_halt().to_le_bytes(),
        0,
        false,
        paths
            .iter()
            .map(|path| ivm::EmbeddedStateDescriptor {
                name: path.to_string(),
                ty: ivm::EmbeddedStateType::Bytes,
            })
            .collect(),
    );
    let mut vm = IVM::new(u64::MAX);
    vm.load_program(&program).expect("declared-state contract");
    vm
}

// Obtain a real compiler-produced Bytes record, rather than making invalid SET
// payloads that would independently fail after a missing namespace guard.
fn custody_namespace_bytes_record() -> Vec<u8> {
    let (program, _) = ivm::KotodamaCompiler::new()
        .compile_source_with_manifest(
            r#"seiyaku CustodyNamespaceControl {
                state bytes sorafs_stream_token_custody_v1x;
                hajimari() { sorafs_stream_token_custody_v1x = b""; }
                kotoage fn main() -> int authorize("WriteState") {
                    sorafs_stream_token_custody_v1x = b"ordinary user bytes";
                    return 1;
                }
            }"#,
        )
        .expect("compile user-key positive control");
    let metadata = ivm::ProgramMetadata::parse(&program).expect("compiled metadata");
    let entry = metadata.prefix_len() as u64
        + metadata
            .contract_interface
            .as_ref()
            .expect("CNTR")
            .entrypoints
            .iter()
            .find(|entry| entry.name == "main")
            .expect("main entry")
            .entry_pc;
    let mut host = custody_namespace_scoped_host();
    let mut vm = IVM::new(u64::MAX);
    vm.load_program(&program).expect("load user-key control");
    vm.set_register(1, vm.memory.code_len());
    vm.set_program_counter(entry).expect("select main");
    vm.run_with_host(&mut host).expect("execute user-key SET");
    let path: StatePath = "sorafs_stream_token_custody_v1x".parse().expect("user key");
    let physical = host.scoped_durable_state_path(&path).unwrap().unwrap();
    assert_eq!(host.durable_state_overlay.len(), 1);
    host.durable_state_overlay
        .get(&physical)
        .unwrap()
        .clone()
        .unwrap()
}

#[test]
fn stream_token_custody_namespace_reserves_exact_root_and_all_native_key_families() {
    for path in custody_namespace_paths() {
        assert_eq!(
            CoreHost::contract_state_namespace_access(path.as_ref()),
            ContractStateNamespaceAccess::OpaqueSystem,
            "{path}"
        );
    }
    for key in [
        "sorafs_stream_token_custody_v1",
        "sorafs_stream_token_custody_v1/descendant",
        "sorafs_stream_token_custody_v1_future",
    ] {
        assert_eq!(
            CoreHost::contract_state_namespace_access(key),
            ContractStateNamespaceAccess::OpaqueSystem
        );
    }
    for key in [
        "sorafs_stream_token_custody_v1x",
        "sorafs_stream_token_custody_v10",
        "sorafs_stream_token_custody_v1x/entry",
    ] {
        assert_eq!(
            CoreHost::contract_state_namespace_access(key),
            ContractStateNamespaceAccess::User
        );
    }
}

#[test]
fn stream_token_custody_generic_classes_reject_state_before_namespace_dispatch() {
    for state_free in [false, true] {
        let mut host = CoreHost::new(ALICE_ID.clone());
        if state_free {
            host.set_state_free_generic_execution();
        } else {
            host.set_generic_execution();
        }
        let mut vm = IVM::new(u64::MAX);
        for path in custody_namespace_paths() {
            let path_ptr = store_state_path_tlv(&mut vm, &path);
            for syscall in [
                ivm_sys::SYSCALL_STATE_SET,
                ivm_sys::SYSCALL_STATE_DEL,
                ivm_sys::SYSCALL_STATE_GET,
                ivm_sys::SYSCALL_STATE_HAS,
                ivm_sys::SYSCALL_STATE_LEN,
                ivm_sys::SYSCALL_STATE_COUNT,
                ivm_sys::SYSCALL_STATE_SCAN,
            ] {
                vm.set_register(10, path_ptr);
                assert_eq!(
                    host.syscall(syscall, &mut vm),
                    Err(ivm::VMError::GenericSyscallNotAllowed { syscall })
                );
            }
        }
        assert!(host.durable_state_overlay.is_empty());
    }
}

#[test]
fn stream_token_custody_debug_syscalls_cannot_mutate_delete_or_disclose_native_state() {
    let paths = custody_namespace_paths();
    let persisted = norito::to_bytes(&91_u64).expect("sentinel native bytes");
    let mut host = CoreHost::new(ALICE_ID.clone());
    host.set_local_contract_debug_execution();
    for path in &paths {
        host.durable_state_base
            .insert(path.clone(), persisted.clone());
    }
    let before = host.durable_state_base.clone();
    let mut vm = IVM::new(u64::MAX);
    let value_ptr = store_tlv(&mut vm, PointerType::NoritoBytes, &persisted);
    for path in &paths {
        let path_ptr = store_state_path_tlv(&mut vm, path);
        for syscall in [
            ivm_sys::SYSCALL_STATE_SET,
            ivm_sys::SYSCALL_STATE_DEL,
            ivm_sys::SYSCALL_STATE_GET,
            ivm_sys::SYSCALL_STATE_HAS,
            ivm_sys::SYSCALL_STATE_LEN,
        ] {
            vm.set_register(10, path_ptr);
            vm.set_register(11, value_ptr);
            assert_eq!(
                host.syscall(syscall, &mut vm),
                Err(ivm::VMError::PermissionDenied),
                "{path}: {syscall}"
            );
        }
        vm.set_register(10, path_ptr);
        assert!(host.syscall(ivm_sys::SYSCALL_STATE_COUNT, &mut vm).is_ok());
        assert_eq!(vm.register(10), 0);
    }
    assert_eq!(host.durable_state_base, before);
    assert!(host.durable_state_overlay.is_empty());
    let user: StatePath = "sorafs_stream_token_custody_v1x".parse().unwrap();
    let user_ptr = store_state_path_tlv(&mut vm, &user);
    vm.set_register(10, user_ptr);
    vm.set_register(11, value_ptr);
    assert!(host.syscall(ivm_sys::SYSCALL_STATE_SET, &mut vm).is_ok());
    assert_eq!(
        host.durable_state_overlay.get(&user),
        Some(&Some(persisted))
    );
    vm.set_register(10, user_ptr);
    assert!(host.syscall(ivm_sys::SYSCALL_STATE_DEL, &mut vm).is_ok());
    assert_eq!(host.durable_state_overlay.get(&user), Some(&None));
}

#[test]
fn stream_token_custody_contract_syscalls_reject_logical_shadows_with_valid_typed_values() {
    let paths = custody_namespace_paths();
    let user: StatePath = "sorafs_stream_token_custody_v1x".parse().unwrap();
    let mut declarations = paths.to_vec();
    declarations.push(user.clone());
    let value = custody_namespace_bytes_record();
    let mut vm = custody_namespace_vm(&declarations);
    let mut host = custody_namespace_scoped_host();
    for path in &paths {
        ivm::host::validate_declared_state_value_payload(&vm, path, &value)
            .expect("valid typed SET candidate");
        let physical = host.scoped_durable_state_path(path).unwrap().unwrap();
        host.durable_state_base.insert(path.clone(), value.clone());
        host.durable_state_base.insert(physical, value.clone());
    }
    let before = host.durable_state_base.clone();
    let value_ptr = store_tlv(&mut vm, PointerType::NoritoBytes, &value);
    for path in &paths {
        let path_ptr = store_state_path_tlv(&mut vm, path);
        for syscall in [
            ivm_sys::SYSCALL_STATE_SET,
            ivm_sys::SYSCALL_STATE_DEL,
            ivm_sys::SYSCALL_STATE_GET,
            ivm_sys::SYSCALL_STATE_HAS,
            ivm_sys::SYSCALL_STATE_LEN,
        ] {
            vm.set_register(10, path_ptr);
            vm.set_register(11, value_ptr);
            assert_eq!(
                host.syscall(syscall, &mut vm),
                Err(ivm::VMError::PermissionDenied),
                "{path}: {syscall}"
            );
        }
        vm.set_register(10, path_ptr);
        assert!(host.syscall(ivm_sys::SYSCALL_STATE_COUNT, &mut vm).is_ok());
        assert_eq!(
            vm.register(10),
            0,
            "native-looking scoped entry remains opaque"
        );
    }
    assert!(host.durable_state_overlay.is_empty());
    assert_eq!(host.durable_state_base, before);
    let user_ptr = store_state_path_tlv(&mut vm, &user);
    vm.set_register(10, user_ptr);
    vm.set_register(11, value_ptr);
    assert!(host.syscall(ivm_sys::SYSCALL_STATE_SET, &mut vm).is_ok());
    let scoped_user = host.scoped_durable_state_path(&user).unwrap().unwrap();
    assert_eq!(
        host.durable_state_overlay.get(&scoped_user),
        Some(&Some(value))
    );
    assert!(!host.durable_state_overlay.contains_key(&user));
    vm.set_register(10, user_ptr);
    assert!(host.syscall(ivm_sys::SYSCALL_STATE_DEL, &mut vm).is_ok());
    assert_eq!(host.durable_state_overlay.get(&scoped_user), Some(&None));
    assert_eq!(host.durable_state_overlay.len(), 1);
}

#[test]
fn stream_token_custody_namespace_scan_rejects_a_declared_opaque_map() {
    let mut vm = IVM::new(u64::MAX);
    let root: StatePath = "sorafs_stream_token_custody_v1".parse().unwrap();
    let user: StatePath = "sorafs_stream_token_custody_v1x".parse().unwrap();
    let program = build_authenticated_test_contract_program_with_states(
        &ivm::encoding::wide::encode_halt().to_le_bytes(),
        0,
        false,
        [root.clone(), user.clone()]
            .into_iter()
            .map(|path| ivm::EmbeddedStateDescriptor {
                name: path.to_string(),
                ty: ivm::EmbeddedStateType::StateMap {
                    key: Box::new(ivm::EmbeddedStateType::Int),
                    value: Box::new(ivm::EmbeddedStateType::Bytes),
                },
            })
            .collect(),
    );
    vm.load_program(&program).expect("declared scan maps");
    let mut host = custody_namespace_scoped_host();
    for (path, opaque) in [(root, true), (user, false)] {
        let ptr = store_state_path_tlv(&mut vm, &path);
        vm.set_register(10, ptr);
        vm.set_register(11, 0);
        vm.set_register(12, 1);
        vm.set_register(13, 0);
        vm.set_register(14, 0);
        vm.set_register(15, 0);
        let result = host.syscall(ivm_sys::SYSCALL_STATE_SCAN, &mut vm);
        if opaque {
            assert_eq!(result, Err(ivm::VMError::PermissionDenied));
        } else {
            assert!(
                result.is_ok(),
                "ordinary declared map remains enumerable: {result:?}"
            );
        }
    }
    assert!(host.durable_state_overlay.is_empty());
}

#[test]
fn stream_token_custody_native_state_survives_debug_apply_and_export_refusal() {
    let paths = custody_namespace_paths();
    let sentinel = norito::to_bytes(&8194_u64).expect("retained native bytes");
    let mut world = World::new();
    for path in &paths {
        world
            .smart_contract_state
            .insert(path.clone(), sentinel.clone());
    }
    let state = State::new_for_testing(
        world,
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    let mut block = state.block(BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0));
    let mut tx = block.transaction();
    let mut host = CoreHost::new(ALICE_ID.clone());
    host.set_local_contract_debug_execution();
    host.set_durable_state_snapshot_from_world(&tx.world);
    let mut vm = IVM::new(u64::MAX);
    let value_ptr = store_tlv(&mut vm, PointerType::NoritoBytes, &sentinel);
    for path in &paths {
        let ptr = store_state_path_tlv(&mut vm, path);
        for syscall in [ivm_sys::SYSCALL_STATE_SET, ivm_sys::SYSCALL_STATE_DEL] {
            vm.set_register(10, ptr);
            vm.set_register(11, value_ptr);
            assert_eq!(
                host.syscall(syscall, &mut vm),
                Err(ivm::VMError::PermissionDenied)
            );
        }
    }
    let user: StatePath = "sorafs_stream_token_custody_v1x".parse().unwrap();
    let ptr = store_state_path_tlv(&mut vm, &user);
    vm.set_register(10, ptr);
    vm.set_register(11, value_ptr);
    assert!(host.syscall(ivm_sys::SYSCALL_STATE_SET, &mut vm).is_ok());
    let before_overlay = host.durable_state_overlay.clone();
    let error = host
        .apply_queued(&mut tx, &ALICE_ID)
        .expect_err("debug apply must fail");
    assert!(
        matches!(error, ValidationFail::NotPermitted(message) if message == "local debug execution artifacts cannot be committed")
    );
    assert_eq!(host.durable_state_overlay, before_overlay);
    assert!(tx.world.smart_contract_state.get(&user).is_none());
    assert!(tx.tx_call_hash.is_none());
    for path in &paths {
        assert_eq!(tx.world.smart_contract_state.get(path), Some(&sentinel));
    }
    let error = match host.into_execution_artifacts(None) {
        Err(error) => error,
        Ok(_) => panic!("debug export must fail"),
    };
    assert!(
        matches!(error, ValidationFail::NotPermitted(message) if message == "local debug execution artifacts cannot be committed")
    );
    for path in &paths {
        assert_eq!(tx.world.smart_contract_state.get(path), Some(&sentinel));
    }
}
