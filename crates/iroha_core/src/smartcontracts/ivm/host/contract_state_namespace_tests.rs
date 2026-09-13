// Existing namespace and generic/debug state boundary tests, included in host::tests.
#[test]
fn contract_state_namespace_access_covers_consensus_owned_prefixes() {
    for key in [
        "sc",
        "sc/0123456789abcdef/counter",
        "da_ingest_quota_v1",
        "da_ingest_quota_v1/authority/deadbeef",
        "faucet_claim_consumed_v1",
        "faucet_claim_consumed_v1/deadbeef",
        "merge_execution_batch_applied_1_deadbeef",
        "merge_execution_lane_applied_1_2_3_deadbeef",
        "merge_lane_frontier_v1",
        "merge_lane_frontier_v1_1_2_deadbeef",
        "queue_plan_admission_v2_deadbeef_cafebabe",
        "queue_plan_pending_obligation_v1_deadbeef_cafebabe",
        "queue_plan_pending_route_member_v1_0_0_deadbeef_cafebabe",
        "queue_plan_pending_signed_alias_member_v1_deadbeef_cafebabe_deadbeef",
        "queue_plan_signed_alias_terminal_v1_deadbeef_cafebabe",
        "nexus_fee_receipt_settled_deadbeef",
        "nexus_fee_settlement_settled_1_2_3_deadbeef",
        "sealed_tx_commitment_deadbeef",
    ] {
        assert_eq!(
            CoreHost::contract_state_namespace_access(key),
            ContractStateNamespaceAccess::OpaqueSystem,
            "{key} must remain opaque to generic contract state syscalls"
        );
    }
    for key in [
        "pkdeploy_verified_lane_relay",
        "pkdeploy_verified_lane_relay_1_2_3_deadbeef",
        "pkdeploy_verified_nexus_fee_budget_deadbeef",
        "pkdeploy_verified_fee_sponsor_vault_allocation",
        "pkdeploy_verified_fee_sponsor_vault_allocation_deadbeef",
        "pkdeploy_fee_sponsor_vault_allocation_usage",
        "pkdeploy_fee_sponsor_vault_allocation_usage_deadbeef",
        "pkdeploy_fee_sponsor_vault_allocation_settled_usage",
        "pkdeploy_fee_sponsor_vault_allocation_settled_usage_deadbeef",
        "VerifiedLaneRelays",
        "VerifiedLaneRelays/deadbeef",
    ] {
        assert_eq!(
            CoreHost::contract_state_namespace_access(key),
            ContractStateNamespaceAccess::ReadOnlySystem,
            "{key} is public contract state but must remain native-authored"
        );
    }
    for key in [
        "scatter/counter",
        "da_ingest_quota_v1x",
        "faucet_claim_consumed_v1x",
        "merge_lane_frontier_v1x",
        "queue_plan_admission_v2x",
        "queue_plan_pending_obligation_v1x",
        "queue_plan_pending_route_member_v1x",
        "queue_plan_pending_signed_alias_member_v1x",
        "queue_plan_signed_alias_terminal_v1x",
        "pkdeploy_verified_lane_relayx",
        "pkdeploy_verified_fee_sponsor_vault_allocationx",
        "pkdeploy_fee_sponsor_vault_allocation_usagex",
        "pkdeploy_fee_sponsor_vault_allocation_settled_usagex",
        "counter",
    ] {
        assert_eq!(
            CoreHost::contract_state_namespace_access(key),
            ContractStateNamespaceAccess::User,
            "delimiter-aware matching must not reserve similarly named user state"
        );
    }
}
#[test]
fn state_syscalls_cannot_forge_delete_or_disclose_queue_plan_markers() {
    let markers: [StatePath; 3] = [
        format!(
            "queue_plan_admission_v2_{}_{}",
            "ab".repeat(Hash::LENGTH),
            "cd".repeat(Hash::LENGTH)
        )
        .parse()
        .expect("QueuePlan admission marker key"),
        format!(
            "queue_plan_pending_signed_alias_member_v1_{}_{}_{}",
            "ab".repeat(Hash::LENGTH),
            "cd".repeat(Hash::LENGTH),
            "ef".repeat(Hash::LENGTH)
        )
        .parse()
        .expect("QueuePlan pending signed-alias marker key"),
        format!(
            "queue_plan_signed_alias_terminal_v1_{}_{}",
            "ab".repeat(Hash::LENGTH),
            "cd".repeat(Hash::LENGTH)
        )
        .parse()
        .expect("QueuePlan signed-alias terminal marker key"),
    ];
    for marker in markers {
        let authority: AccountId = fixture_account("alice");
        let contract = ContractAddress::derive(
            &"hash:0000000000000000000000000000000000000000000000000000000000000001#C50E"
                .parse()
                .expect("canonical test network id"),
            &authority,
            178,
            DataSpaceId::UNIVERSAL,
        )
        .expect("derive adversarial contract");
        let mut host = CoreHost::new(authority);
        host.set_contract_runtime_context(Some(ContractRuntimeExecutionContext {
            contract_subject: contract.subject_id(),
            contract_address: contract,
            contract_alias: Some(
                "adversarial::queue_plan_registry"
                    .parse()
                    .expect("contract alias"),
            ),
            entrypoint: "main".to_owned(),
        }));
        let mut vm = IVM::new(10_000);
        let code = ivm::encoding::wide::encode_halt().to_le_bytes();
        vm.load_program(&build_authenticated_test_contract_program_with_states(
            &code,
            0,
            false,
            vec![ivm::EmbeddedStateDescriptor {
                name: marker.to_string(),
                ty: ivm::EmbeddedStateType::Bytes,
            }],
        ))
        .expect("load self-describing adversarial QueuePlan contract");
        let path_ptr = store_state_path_tlv(&mut vm, &marker);
        let forged = norito::to_bytes(&999_u64).expect("encode forged marker fixture");
        host.durable_state_base
            .insert(marker.clone(), forged.clone());
        let value_ptr = store_tlv(&mut vm, PointerType::NoritoBytes, &forged);
        vm.set_register(10, path_ptr);
        vm.set_register(11, value_ptr);
        assert_eq!(
            host.syscall(ivm_sys::SYSCALL_STATE_SET, &mut vm),
            Err(ivm::VMError::PermissionDenied),
            "STATE_SET must reject a QueuePlan registry marker before contract scoping"
        );
        vm.set_register(10, path_ptr);
        assert_eq!(
            host.syscall(ivm_sys::SYSCALL_STATE_DEL, &mut vm),
            Err(ivm::VMError::PermissionDenied),
            "STATE_DEL must not create a QueuePlan registry tombstone"
        );
        for syscall in [
            ivm_sys::SYSCALL_STATE_GET,
            ivm_sys::SYSCALL_STATE_HAS,
            ivm_sys::SYSCALL_STATE_LEN,
        ] {
            vm.set_register(10, path_ptr);
            assert_eq!(
                host.syscall(syscall, &mut vm),
                Err(ivm::VMError::PermissionDenied),
                "QueuePlan registry markers must remain opaque"
            );
        }
        vm.set_register(10, path_ptr);
        host.syscall(ivm_sys::SYSCALL_STATE_COUNT, &mut vm)
            .expect("opaque QueuePlan keys are omitted from counts");
        assert_eq!(vm.register(10), 0, "hidden count must exclude the marker");
        assert!(
            host.durable_state_overlay.is_empty(),
            "rejected QueuePlan registry access must not retain a raw or scoped write"
        );
    }
}
#[test]
fn state_syscalls_cannot_forge_delete_or_disclose_merge_lane_frontier() {
    let frontier: StatePath = format!("merge_lane_frontier_v1_7_11_{}", "ab".repeat(32))
        .parse()
        .expect("frontier marker key");
    let authority: AccountId = fixture_account("alice");
    let contract = ContractAddress::derive(
        &"hash:0000000000000000000000000000000000000000000000000000000000000001#C50E"
            .parse()
            .expect("canonical test network id"),
        &authority,
        177,
        DataSpaceId::UNIVERSAL,
    )
    .expect("derive contract");
    let context = ContractRuntimeExecutionContext {
        contract_subject: contract.subject_id(),
        contract_address: contract,
        contract_alias: Some("adversarial::frontier".parse().expect("contract alias")),
        entrypoint: "attack".to_owned(),
    };
    let mut scope_host = CoreHost::new(authority.clone());
    scope_host.set_local_contract_debug_execution();
    scope_host.set_contract_runtime_context(Some(context.clone()));
    let scoped_frontier = scope_host
        .scoped_durable_state_path(&frontier)
        .expect("build scoped path")
        .expect("runtime context must scope contract state");
    let physical_frontier = frontier.clone();
    let persisted = norito::to_bytes(&77_u64).expect("encode persisted marker fixture");
    let legacy_scoped = norito::to_bytes(&88_u64).expect("encode legacy scoped shadow");
    let mut world = World::new();
    world
        .smart_contract_state
        .insert(physical_frontier.clone(), persisted.clone());
    world
        .smart_contract_state
        .insert(scoped_frontier.clone(), legacy_scoped.clone());
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let state = State::new_for_testing(world, kura, query);
    super::pointer_abi_tests::establish_authenticated_axt_ledger_time(&state, 1);
    let mut host = CoreHost::from_state(authority, &state).expect("canonical state snapshots");
    host.set_local_contract_debug_execution();
    host.set_contract_runtime_context(Some(context));
    let interface = ivm::EmbeddedContractInterfaceV1 {
        seiyaku_name: "AdversarialFrontier".to_owned(),
        compiler_fingerprint: "iroha-core-host-tests".to_owned(),
        abi_hash: ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1),
        features_bitmap: 0,
        access_set_hints: None,
        kotoba: Vec::new(),
        entrypoints: vec![ivm::EmbeddedEntrypointDescriptor {
            name: "attack".to_owned(),
            kind: iroha_data_model::smart_contract::manifest::EntryPointKind::Kotoage,
            params: Vec::new(),
            argument_schema: None,
            return_type: Some("()".to_owned()),
            return_schema: Some(iroha_data_model::smart_contract::entrypoint::EntrypointValueTypeV1 {
                nodes: vec![iroha_data_model::smart_contract::entrypoint::EntrypointValueTypeNodeV1::Unit],
            }),
            permission: Some("CanAttack".to_owned()),
            read_keys: Vec::new(),
            write_keys: Vec::new(),
            access_hints_complete: Some(true),
            access_hints_skipped: Vec::new(),
            triggers: Vec::new(),
            entry_pc: 0,
        }],
        states: vec![ivm::EmbeddedStateDescriptor {
            name: frontier.to_string(),
            ty: ivm::EmbeddedStateType::Bytes,
        }],
        error_types: Vec::new(),
    };
    let mut program = ivm::ProgramMetadata::default().encode();
    program.extend_from_slice(&interface.encode_section());
    program.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());
    let mut vm = IVM::new(10_000);
    vm.load_program(&program)
        .expect("load self-describing adversarial contract");
    let path_ptr = store_state_path_tlv(&mut vm, &frontier);
    let forged = norito::to_bytes(&999_u64).expect("encode forged marker fixture");
    let value_ptr = store_tlv(&mut vm, PointerType::NoritoBytes, &forged);
    vm.set_register(10, path_ptr);
    vm.set_register(11, value_ptr);
    assert_eq!(
        host.syscall(ivm_sys::SYSCALL_STATE_SET, &mut vm),
        Err(ivm::VMError::PermissionDenied),
        "STATE_SET must reject the logical system key before creating a scoped shadow"
    );
    vm.set_register(10, path_ptr);
    assert_eq!(
        host.syscall(ivm_sys::SYSCALL_STATE_DEL, &mut vm),
        Err(ivm::VMError::PermissionDenied),
        "STATE_DEL must not create either a scoped tombstone or its legacy raw tombstone"
    );
    assert!(
        !host.durable_state_overlay.contains_key(&physical_frontier),
        "the raw consensus marker must not be staged for mutation"
    );
    assert!(
        !host.durable_state_overlay.contains_key(&scoped_frontier),
        "the scoped fallback path must not be staged for mutation"
    );
    assert_eq!(
        host.durable_state_base.get(&physical_frontier),
        Some(&persisted),
        "the host's durable snapshot must retain the exact consensus marker"
    );
    assert_eq!(
        host.durable_state_base.get(&scoped_frontier),
        Some(&legacy_scoped),
        "a pre-guard scoped shadow remains physically stored but inaccessible"
    );
    let mut generic_host =
        CoreHost::from_state(fixture_account("bob"), &state).expect("canonical state snapshots");
    vm.set_register(10, path_ptr);
    vm.set_register(11, value_ptr);
    assert_eq!(
        generic_host.syscall(ivm_sys::SYSCALL_STATE_SET, &mut vm),
        Err(ivm::VMError::GenericSyscallNotAllowed {
            syscall: ivm_sys::SYSCALL_STATE_SET,
        }),
        "generic execution must reject durable-state writes before namespace dispatch"
    );
    vm.set_register(10, path_ptr);
    assert_eq!(
        generic_host.syscall(ivm_sys::SYSCALL_STATE_DEL, &mut vm),
        Err(ivm::VMError::GenericSyscallNotAllowed {
            syscall: ivm_sys::SYSCALL_STATE_DEL,
        }),
        "generic execution must reject durable-state deletion before namespace dispatch"
    );
    assert!(generic_host.durable_state_overlay.is_empty());
    for syscall in [
        ivm_sys::SYSCALL_STATE_GET,
        ivm_sys::SYSCALL_STATE_HAS,
        ivm_sys::SYSCALL_STATE_LEN,
    ] {
        vm.set_register(10, path_ptr);
        assert_eq!(
            host.syscall(syscall, &mut vm),
            Err(ivm::VMError::PermissionDenied),
            "opaque frontier markers must not disclose presence, length, or contents"
        );
    }
    vm.set_register(10, path_ptr);
    assert_eq!(
        host.syscall(ivm_sys::SYSCALL_STATE_COUNT, &mut vm),
        Ok(test_state_path_gas(&frontier)),
        "gas must cover the caller-visible prefix without revealing hidden marker work"
    );
    assert_eq!(
        vm.register(10),
        0,
        "STATE_COUNT must not disclose the marker"
    );
}
#[test]
fn verified_relay_contract_state_is_readable_but_not_generically_mutable() {
    let path: StatePath = format!("pkdeploy_verified_lane_relay_1_2_3_{}", "cd".repeat(32))
        .parse()
        .expect("verified relay state key");
    let value = norito::to_bytes(&42_u64).expect("encode public record fixture");
    let mut world = World::new();
    world
        .smart_contract_state
        .insert(path.clone(), value.clone());
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let state = State::new_for_testing(world, kura, query);
    super::pointer_abi_tests::establish_authenticated_axt_ledger_time(&state, 1);
    let mut host =
        CoreHost::from_state(fixture_account("alice"), &state).expect("canonical state snapshots");
    host.set_local_contract_debug_execution();
    let mut vm = IVM::new(10_000);
    let path_ptr = store_state_path_tlv(&mut vm, &path);
    vm.set_register(10, path_ptr);
    assert_eq!(
        host.syscall(ivm_sys::SYSCALL_STATE_GET, &mut vm),
        Ok(ivm::host::state_value_gas(
            norito_blob(&path).len(),
            value.len(),
        )),
        "verified relay records are an intentional public contract surface"
    );
    let value_tlv = vm
        .memory
        .validate_tlv(vm.register(10))
        .expect("verified relay value TLV");
    assert_eq!(
        norito::decode_from_bytes::<u64>(value_tlv.payload).expect("decode public record"),
        42
    );
    let forged = norito::to_bytes(&99_u64).expect("encode forged record fixture");
    let value_ptr = store_tlv(&mut vm, PointerType::NoritoBytes, &forged);
    vm.set_register(10, path_ptr);
    vm.set_register(11, value_ptr);
    assert_eq!(
        host.syscall(ivm_sys::SYSCALL_STATE_SET, &mut vm),
        Err(ivm::VMError::PermissionDenied)
    );
    vm.set_register(10, path_ptr);
    assert_eq!(
        host.syscall(ivm_sys::SYSCALL_STATE_DEL, &mut vm),
        Err(ivm::VMError::PermissionDenied)
    );
    assert!(host.durable_state_overlay.is_empty());
    vm.set_register(10, path_ptr);
    host.syscall(ivm_sys::SYSCALL_STATE_COUNT, &mut vm)
        .expect("public verified relay keys remain countable");
    assert_eq!(vm.register(10), 1);
}
#[test]
fn verified_fee_sponsor_state_is_readable_but_not_generically_mutable() {
    let paths = [
        format!(
            "{VERIFIED_FEE_SPONSOR_VAULT_ALLOCATION_STATE_KEY_PREFIX}_{}",
            "11".repeat(32)
        ),
        format!(
            "{VERIFIED_FEE_SPONSOR_VAULT_ALLOCATION_USAGE_STATE_KEY_PREFIX}_{}",
            "22".repeat(32)
        ),
        format!(
            "{VERIFIED_FEE_SPONSOR_VAULT_ALLOCATION_SETTLED_USAGE_STATE_KEY_PREFIX}_{}",
            "33".repeat(32)
        ),
    ]
    .map(|key| key.parse::<StatePath>().expect("fee sponsor system key"));
    let value = norito::to_bytes(&42_u64).expect("encode public record fixture");
    let mut world = World::new();
    for path in &paths {
        world
            .smart_contract_state
            .insert(path.clone(), value.clone());
    }
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let state = State::new_for_testing(world, kura, query);
    super::pointer_abi_tests::establish_authenticated_axt_ledger_time(&state, 1);
    let mut host =
        CoreHost::from_state(fixture_account("alice"), &state).expect("canonical state snapshots");
    host.set_local_contract_debug_execution();
    let mut vm = IVM::new(100_000);
    for path in &paths {
        let path_ptr = store_state_path_tlv(&mut vm, path);
        vm.set_register(10, path_ptr);
        host.syscall(ivm_sys::SYSCALL_STATE_GET, &mut vm)
            .expect("verified fee sponsor state remains readable");
        let value_tlv = vm
            .memory
            .validate_tlv(vm.register(10))
            .expect("verified fee sponsor value TLV");
        assert_eq!(
            norito::decode_from_bytes::<u64>(value_tlv.payload)
                .expect("decode public fee sponsor record"),
            42
        );
        let forged = norito::to_bytes(&99_u64).expect("encode forged record fixture");
        let value_ptr = store_tlv(&mut vm, PointerType::NoritoBytes, &forged);
        vm.set_register(10, path_ptr);
        vm.set_register(11, value_ptr);
        assert_eq!(
            host.syscall(ivm_sys::SYSCALL_STATE_SET, &mut vm),
            Err(ivm::VMError::PermissionDenied),
            "{path} must be native-authored"
        );
        vm.set_register(10, path_ptr);
        assert_eq!(
            host.syscall(ivm_sys::SYSCALL_STATE_DEL, &mut vm),
            Err(ivm::VMError::PermissionDenied),
            "{path} must not be deleted generically"
        );
    }
    assert!(
        host.durable_state_overlay.is_empty(),
        "rejected fee sponsor state mutations must not be staged"
    );
}
#[test]
fn state_syscall_unscoped_overlay_overrides_base_value_without_context() {
    let mut world = World::new();
    let path: StatePath = "counter".parse().unwrap();
    world.smart_contract_state.insert(
        path.clone(),
        norito::to_bytes(&7_u64).expect("encode base state value"),
    );
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let state = State::new_for_testing(world, kura, query);
    super::pointer_abi_tests::establish_authenticated_axt_ledger_time(&state, 1);
    let authority: AccountId = fixture_account("alice");
    let mut host = CoreHost::from_state(authority, &state).expect("canonical state snapshots");
    host.set_local_contract_debug_execution();
    let mut vm = IVM::new(10_000);
    let path_ptr = store_state_path_tlv(&mut vm, &path);
    let value_ptr = store_tlv(
        &mut vm,
        PointerType::NoritoBytes,
        &norito::to_bytes(&22_u64).expect("encode overlay state value"),
    );
    vm.set_register(10, path_ptr);
    vm.set_register(11, value_ptr);
    assert_eq!(
        host.syscall(ivm_sys::SYSCALL_STATE_SET, &mut vm),
        Ok(test_state_value_gas(
            &path,
            norito::to_bytes(&22_u64)
                .expect("encode overlay state value")
                .len()
        )),
        "STATE_SET should stage an unscoped overlay value without runtime context"
    );
    vm.set_register(10, path_ptr);
    assert_eq!(
        host.syscall(ivm_sys::SYSCALL_STATE_GET, &mut vm),
        Ok(test_state_value_gas(
            &path,
            norito::to_bytes(&22_u64)
                .expect("encode overlay state value")
                .len()
        )),
        "unscoped overlay should override the persisted base value"
    );
    let tlv = vm
        .memory
        .validate_tlv(vm.register(10))
        .expect("overlay state tlv");
    assert_eq!(tlv.type_id, PointerType::NoritoBytes);
    let value: u64 = norito::decode_from_bytes(tlv.payload).expect("decode overlay state");
    assert_eq!(value, 22);
    let overlay = host.drain_durable_state_overlay();
    let stored = overlay
        .get(&path)
        .and_then(Option::as_ref)
        .expect("unscoped overlay entry");
    let stored_value: u64 =
        norito::decode_from_bytes(stored).expect("decode raw persisted overlay state");
    assert_eq!(stored_value, 22);
}
#[test]
fn state_syscall_unscoped_delete_shadows_base_value_without_context() {
    let mut world = World::new();
    let path: StatePath = "counter".parse().unwrap();
    world.smart_contract_state.insert(
        path.clone(),
        norito::to_bytes(&7_u64).expect("encode base state value"),
    );
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let state = State::new_for_testing(world, kura, query);
    super::pointer_abi_tests::establish_authenticated_axt_ledger_time(&state, 1);
    let authority: AccountId = fixture_account("alice");
    let mut host = CoreHost::from_state(authority, &state).expect("canonical state snapshots");
    host.set_local_contract_debug_execution();
    let mut vm = IVM::new(10_000);
    let path_ptr = store_state_path_tlv(&mut vm, &path);
    vm.set_register(10, path_ptr);
    assert_eq!(
        host.syscall(ivm_sys::SYSCALL_STATE_DEL, &mut vm),
        Ok(test_state_path_gas(&path)),
        "STATE_DEL should stage an unscoped tombstone without runtime context"
    );
    vm.set_register(10, path_ptr);
    assert_eq!(
        host.syscall(ivm_sys::SYSCALL_STATE_GET, &mut vm),
        Ok(test_state_path_gas(&path)),
        "unscoped tombstone should shadow the persisted base value"
    );
    assert_eq!(vm.register(10), 0);
    let overlay = host.drain_durable_state_overlay();
    assert_eq!(
        overlay.len(),
        1,
        "unscoped delete should only record one tombstone"
    );
    assert_eq!(overlay.get(&path), Some(&None));
}
#[test]
fn generic_ivm_host_rejects_all_durable_state_access() {
    let authority: AccountId = fixture_account("alice");
    for prefix in [
        "sc",
        crate::smartcontracts::code::CONTRACT_LIFECYCLE_STATE_PREFIX,
        "sns",
        "sealed",
        "system",
    ] {
        let mut host = CoreHost::new(authority.clone());
        host.set_generic_execution();
        let mut vm = IVM::new(10_000);
        let suffix = if prefix == "sc" { "/secret" } else { "" };
        let reserved_path: StatePath = format!("{prefix}/{}{suffix}", "00".repeat(Hash::LENGTH))
            .parse()
            .expect("valid reserved state path");
        let path_ptr = store_state_path_tlv(&mut vm, &reserved_path);
        let value_ptr = store_tlv(
            &mut vm,
            PointerType::NoritoBytes,
            &norito::to_bytes(&7_u64).expect("encode state value"),
        );
        vm.set_register(10, path_ptr);
        vm.set_register(11, value_ptr);
        assert_eq!(
            host.syscall(ivm_sys::SYSCALL_STATE_SET, &mut vm),
            Err(ivm::VMError::GenericSyscallNotAllowed {
                syscall: ivm_sys::SYSCALL_STATE_SET,
            }),
            "generic IVM must not write `{prefix}`"
        );
        assert!(host.durable_state_overlay.is_empty());
        vm.set_register(10, path_ptr);
        assert_eq!(
            host.syscall(ivm_sys::SYSCALL_STATE_GET, &mut vm),
            Err(ivm::VMError::GenericSyscallNotAllowed {
                syscall: ivm_sys::SYSCALL_STATE_GET,
            }),
            "generic IVM must not read `{prefix}`"
        );
        vm.set_register(10, path_ptr);
        assert_eq!(
            host.syscall(ivm_sys::SYSCALL_STATE_DEL, &mut vm),
            Err(ivm::VMError::GenericSyscallNotAllowed {
                syscall: ivm_sys::SYSCALL_STATE_DEL,
            }),
            "generic IVM must not tombstone `{prefix}`"
        );
        let reserved_prefix: StatePath = prefix.parse().expect("valid reserved state prefix");
        let prefix_ptr = store_state_path_tlv(&mut vm, &reserved_prefix);
        vm.set_register(10, prefix_ptr);
        vm.set_register(11, 0);
        vm.set_register(12, ivm_sys::STATE_SCAN_MAX_ITEMS_V1);
        assert_eq!(
            host.syscall(ivm_sys::SYSCALL_STATE_SCAN, &mut vm),
            Err(ivm::VMError::GenericSyscallNotAllowed {
                syscall: ivm_sys::SYSCALL_STATE_SCAN,
            }),
            "generic IVM must not enumerate `{prefix}`"
        );
    }
}
