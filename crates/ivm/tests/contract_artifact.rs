use iroha_data_model::{
    smart_contract::manifest::{
        AccessSetHints, ContractErrorCodeDescriptor, DynamicAccessHint, EntryPointKind,
    },
    trigger::{TriggerId, action::Repeats},
};

fn entrypoint(
    name: &str,
    kind: EntryPointKind,
    entry_pc: u64,
) -> ivm::EmbeddedEntrypointDescriptor {
    ivm::EmbeddedEntrypointDescriptor {
        name: name.to_owned(),
        kind,
        params: Vec::new(),
        argument_schema: None,
        return_type: None,
        permission: (kind == EntryPointKind::Public).then(|| "Execute".to_owned()),
        read_keys: Vec::new(),
        write_keys: Vec::new(),
        access_hints_complete: Some(true),
        access_hints_skipped: Vec::new(),
        triggers: Vec::new(),
        entry_pc,
    }
}

fn contract_artifact(
    abi_version: u8,
    entrypoints: Vec<ivm::EmbeddedEntrypointDescriptor>,
) -> Vec<u8> {
    contract_artifact_with_access_hints(abi_version, entrypoints, None)
}

fn contract_artifact_with_access_hints(
    abi_version: u8,
    entrypoints: Vec<ivm::EmbeddedEntrypointDescriptor>,
    access_set_hints: Option<AccessSetHints>,
) -> Vec<u8> {
    contract_artifact_with_code(
        abi_version,
        entrypoints,
        access_set_hints,
        &[ivm::encoding::wide::encode_halt()],
    )
}

fn contract_artifact_with_code(
    abi_version: u8,
    entrypoints: Vec<ivm::EmbeddedEntrypointDescriptor>,
    access_set_hints: Option<AccessSetHints>,
    code: &[u32],
) -> Vec<u8> {
    let meta = ivm::ProgramMetadata {
        version_major: 1,
        version_minor: 1,
        mode: 0,
        vector_length: 0,
        max_cycles: 0,
        abi_version,
    };
    let interface = ivm::EmbeddedContractInterfaceV1 {
        contract_name: "TestContract".to_owned(),
        compiler_fingerprint: "ivm-tests".to_owned(),
        features_bitmap: 0,
        access_set_hints,
        kotoba: Vec::new(),
        entrypoints,
        error_codes: Vec::new(),
        states: Vec::new(),
    };
    let mut bytes = meta.encode();
    bytes.extend_from_slice(&interface.encode_section());
    for instruction in code {
        bytes.extend_from_slice(&instruction.to_le_bytes());
    }
    bytes
}

fn contract_artifact_with_error_codes(error_codes: Vec<ContractErrorCodeDescriptor>) -> Vec<u8> {
    let meta = ivm::ProgramMetadata {
        version_major: 1,
        version_minor: 1,
        mode: 0,
        vector_length: 0,
        max_cycles: 0,
        abi_version: 1,
    };
    let interface = ivm::EmbeddedContractInterfaceV1 {
        contract_name: "TestContract".to_owned(),
        compiler_fingerprint: "ivm-tests".to_owned(),
        features_bitmap: 0,
        access_set_hints: None,
        kotoba: Vec::new(),
        entrypoints: vec![entrypoint("main", EntryPointKind::Public, 0)],
        error_codes,
        states: Vec::new(),
    };
    let mut bytes = meta.encode();
    bytes.extend_from_slice(&interface.encode_section());
    bytes.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());
    bytes
}

#[test]
fn verify_rejects_ambiguous_or_reserved_error_codes() {
    let duplicate = contract_artifact_with_error_codes(vec![
        ContractErrorCodeDescriptor {
            namespace: "PaymentError".to_owned(),
            name: "Unauthorized".to_owned(),
            code: 1001,
        },
        ContractErrorCodeDescriptor {
            namespace: "SettlementError".to_owned(),
            name: "Expired".to_owned(),
            code: 1001,
        },
    ]);
    let error =
        ivm::verify_contract_artifact(&duplicate).expect_err("duplicate code must be rejected");
    assert!(
        error
            .to_string()
            .contains("duplicate numeric error code 1001")
    );

    let reserved = contract_artifact_with_error_codes(vec![ContractErrorCodeDescriptor {
        namespace: "PaymentError".to_owned(),
        name: "Unspecified".to_owned(),
        code: 0,
    }]);
    let error =
        ivm::verify_contract_artifact(&reserved).expect_err("reserved code must be rejected");
    assert!(error.to_string().contains("uses reserved code 0"));
}

#[test]
fn compiler_emits_self_describing_contract_artifact() {
    let src = r#"
        seiyaku Demo {
            state counter: i64;

            hajimari() {
                counter = 0;
            }

            kotoage fn run() authorize("Admin") {
                debug::info("ready");
            }
        }
    "#;
    let (bytes, manifest) = ivm::KotodamaCompiler::new()
        .compile_source_with_manifest(src)
        .expect("compile contract");
    let parsed = ivm::ProgramMetadata::parse(&bytes).expect("parse artifact");
    assert_eq!(parsed.metadata.version_minor, 1);
    let interface = parsed
        .contract_interface
        .as_ref()
        .expect("compiler must emit CNTR");
    assert_eq!(interface.contract_name, "Demo");
    assert_eq!(manifest.contract_name.as_deref(), Some("Demo"));

    let verified = ivm::verify_contract_artifact(&bytes).expect("verify artifact");
    assert_eq!(
        verified.manifest.signature_payload(),
        manifest.signature_payload(),
        "compiler manifest must match the embedded contract interface",
    );
}

#[test]
fn verified_code_hash_binds_execution_header() {
    let original = contract_artifact(1, vec![entrypoint("main", EntryPointKind::Public, 0)]);
    let original_verified =
        ivm::verify_contract_artifact(&original).expect("verify original artifact");
    let original_hash = original_verified.code_hash;

    let mut changed_cycles = original.clone();
    changed_cycles[8..16].copy_from_slice(&1_u64.to_le_bytes());
    let changed_cycles_verified = ivm::verify_contract_artifact(&changed_cycles)
        .expect("verify artifact with changed max_cycles");
    assert_ne!(changed_cycles_verified.code_hash, original_hash);
    assert_ne!(
        changed_cycles_verified.manifest.signature_payload(),
        original_verified.manifest.signature_payload(),
        "manifest signatures must bind max_cycles"
    );

    let mut changed_vector_length = original;
    changed_vector_length[7] = 1;
    let changed_vector_hash = ivm::verify_contract_artifact(&changed_vector_length)
        .expect("verify artifact with changed vector length")
        .code_hash;
    assert_ne!(changed_vector_hash, original_hash);
}

#[test]
fn signed_manifest_rejects_every_execution_header_mutation() {
    let original = contract_artifact(1, vec![entrypoint("main", EntryPointKind::Public, 0)]);
    let original_verified =
        ivm::verify_contract_artifact(&original).expect("verify original artifact");
    let signed = original_verified
        .manifest
        .clone()
        .signed(&iroha_crypto::KeyPair::try_random().expect("test signing key"));
    let provenance = signed.provenance.as_ref().expect("manifest provenance");
    provenance
        .signature
        .verify(&provenance.signer, &signed.signature_payload_bytes())
        .expect("original manifest signature");

    let mut mutations = Vec::<(&str, Vec<u8>)>::new();
    for index in 0..ivm::METADATA_MAGIC.len() {
        let mut magic = original.clone();
        magic[index] ^= 0xff;
        mutations.push(("magic", magic));
    }
    let mut version_major = original.clone();
    version_major[4] = 2;
    mutations.push(("version_major", version_major));
    let mut version_minor = original.clone();
    version_minor[5] = 0;
    mutations.push(("version_minor", version_minor));
    let mut mode = original.clone();
    mode[6] = ivm::ivm_mode::ZK;
    mutations.push(("mode", mode));
    let mut vector_length = original.clone();
    vector_length[7] = 1;
    mutations.push(("vector_length", vector_length));
    let mut max_cycles = original.clone();
    max_cycles[8..16].copy_from_slice(&1_u64.to_le_bytes());
    mutations.push(("max_cycles", max_cycles));
    let mut abi_version = original;
    abi_version[16] = 0;
    mutations.push(("abi_version", abi_version));

    for (field, mutated) in mutations {
        let Ok(verified) = ivm::verify_contract_artifact(&mutated) else {
            // Structural rejection is an admission rejection before provenance is checked.
            continue;
        };
        assert_ne!(
            verified.manifest.signature_payload(),
            signed.signature_payload(),
            "{field} mutation retained the signed manifest payload"
        );
        assert!(
            provenance
                .signature
                .verify(
                    &provenance.signer,
                    &verified.manifest.signature_payload_bytes()
                )
                .is_err(),
            "{field} mutation retained a valid signature"
        );
    }
}

#[test]
fn public_entrypoint_descriptor_targets_halting_wrapper() {
    let src = r#"
        seiyaku Demo {
            kotoage fn main()  authorize("Entry") {}

            kotoage fn run() -> i64  authorize("Entry") {
                return 42;
            }
        }
    "#;
    let bytes = ivm::KotodamaCompiler::new()
        .compile_source(src)
        .expect("compile contract");
    let parsed = ivm::ProgramMetadata::parse(&bytes).expect("parse artifact");
    let contract_interface = parsed
        .contract_interface
        .as_ref()
        .expect("contract interface");
    let run = contract_interface
        .entrypoints
        .iter()
        .find(|candidate| candidate.name == "run")
        .expect("run entrypoint");

    let mut vm = ivm::IVM::new(u64::MAX);
    vm.load_program(&bytes).expect("load artifact");
    vm.set_program_counter(parsed.prefix_len() as u64 + run.entry_pc)
        .expect("seek run wrapper");
    vm.run().expect("run entrypoint wrapper");
    assert_eq!(vm.register(10), 42);
}

#[test]
fn contract_artifact_with_cntr_requires_explicit_entrypoint_selection() {
    let src = r#"
seiyaku ContractArtifactFixture {

        kotoage fn main() -> i64 authorize("Entry") {
            debug::info("alpha");
            return 7;
        }

}
"#;
    let bytes = ivm::KotodamaCompiler::new()
        .compile_source(src)
        .expect("compile artifact");
    let parsed = ivm::ProgramMetadata::parse(&bytes).expect("parse artifact");
    let contract_interface = parsed
        .contract_interface
        .as_ref()
        .expect("CNTR must be present");
    let main = contract_interface
        .entrypoints
        .iter()
        .find(|entrypoint| entrypoint.name == "main")
        .expect("main entrypoint descriptor");
    let cntr_len = contract_interface.encode_section().len();
    assert!(
        parsed.code_offset > parsed.header_len + cntr_len,
        "string literals should emit a prefix section after CNTR",
    );

    let mut vm = ivm::IVM::new(u64::MAX);
    vm.load_program(&bytes).expect("load artifact");
    vm.run().expect("raw non-dispatching halt");
    assert_eq!(vm.register(10), 0, "raw PC 0 must not invoke main");

    let mut vm = ivm::IVM::new(u64::MAX);
    vm.load_program(&bytes).expect("load artifact");
    vm.set_program_counter(parsed.prefix_len() as u64 + main.entry_pc)
        .expect("seek CNTR main wrapper");
    vm.run().expect("run selected main entrypoint");
    assert_eq!(vm.register(10), 7);
}

#[test]
fn verify_rejects_missing_cntr() {
    let mut bytes = ivm::ProgramMetadata {
        version_major: 1,
        version_minor: 1,
        mode: 0,
        vector_length: 0,
        max_cycles: 0,
        abi_version: 1,
    }
    .encode();
    bytes.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());
    let err = ivm::verify_contract_artifact(&bytes).expect_err("missing CNTR must fail");
    assert!(err.to_string().contains("missing required CNTR"));
}

#[test]
fn verify_rejects_malformed_cntr() {
    let mut bytes = ivm::ProgramMetadata {
        version_major: 1,
        version_minor: 1,
        mode: 0,
        vector_length: 0,
        max_cycles: 0,
        abi_version: 1,
    }
    .encode();
    bytes.extend_from_slice(b"CNTR");
    bytes.extend_from_slice(&1u32.to_le_bytes());
    bytes.push(0xff);
    bytes.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());
    let err = ivm::verify_contract_artifact(&bytes).expect_err("malformed CNTR must fail");
    assert!(err.to_string().contains("metadata parse failed"));
}

#[test]
fn verify_rejects_embedded_debug_metadata() {
    let mut bytes = contract_artifact(1, vec![entrypoint("main", EntryPointKind::Public, 0)]);
    let parsed = ivm::ProgramMetadata::parse(&bytes).expect("parse base artifact");
    let code = bytes.split_off(parsed.code_offset);
    bytes.extend_from_slice(
        &ivm::EmbeddedContractDebugInfoV1 {
            source_map: Vec::new(),
            budget_report: Vec::new(),
        }
        .encode_section(),
    );
    bytes.extend_from_slice(&code);

    let err = ivm::verify_contract_artifact(&bytes)
        .expect_err("deployable artifacts must not contain debug metadata");
    assert!(err.to_string().contains("DBG1"));
}

#[test]
fn verify_rejects_direct_control_flow_outside_instruction_boundaries() {
    use ivm::instruction::wide;

    let outside = ivm::encoding::wide::encode_branch(wide::control::BEQ, 0, 0, 1);
    let bytes = contract_artifact_with_code(
        1,
        vec![entrypoint("main", EntryPointKind::Public, 0)],
        None,
        &[outside],
    );
    let err = ivm::verify_contract_artifact(&bytes)
        .expect_err("branch to the end of the stream must be rejected");
    assert!(err.to_string().contains("instruction boundary"));

    let before_start = ivm::encoding::wide::encode_offset24(wide::control::JMP, -1);
    let bytes = contract_artifact_with_code(
        1,
        vec![entrypoint("main", EntryPointKind::Public, 0)],
        None,
        &[before_start],
    );
    let err =
        ivm::verify_contract_artifact(&bytes).expect_err("jump before the stream must be rejected");
    assert!(err.to_string().contains("outside the executable stream"));
}

#[test]
fn verify_rejects_missing_direct_control_flow_fallthrough_even_when_unreachable() {
    use ivm::instruction::wide;

    let terminal_control_flow = [
        (
            "conditional branch",
            ivm::encoding::wide::encode_branch(wide::control::BEQ, 0, 0, 0),
        ),
        (
            "direct call",
            ivm::encoding::wide::encode_offset24(wide::control::JALS, 0),
        ),
        (
            "linking jump",
            ivm::encoding::wide::encode_jump(wide::control::JAL, 1, 0),
        ),
    ];

    for (encoding, terminal) in terminal_control_flow {
        let bytes = contract_artifact_with_code(
            1,
            vec![entrypoint("main", EntryPointKind::Public, 0)],
            None,
            &[ivm::encoding::wide::encode_halt(), terminal],
        );
        let error = ivm::verify_contract_artifact(&bytes)
            .expect_err("every direct control-flow fallthrough must be a decoded boundary");
        assert!(
            error.to_string().contains("control-flow fallthrough"),
            "{encoding} with a missing fallthrough was admitted: {error}"
        );
    }
}

#[test]
fn verify_rejects_unexecutable_control_flow_even_when_unreachable() {
    use ivm::instruction::wide;

    let invalid_control_flow = [
        (
            "indirect jump",
            ivm::encoding::wide::encode_rr(wide::control::JR, 2, 0, 0),
            "unverifiable indirect control flow",
        ),
        (
            "indirect call",
            ivm::encoding::wide::encode_rr(wide::control::JALR, 1, 2, 0),
            "unverifiable indirect control flow",
        ),
        (
            "unsupported link register",
            ivm::encoding::wide::encode_jump(wide::control::JAL, 2, 0),
            "unsupported link register r2",
        ),
    ];

    for (encoding, invalid, expected) in invalid_control_flow {
        let bytes = contract_artifact_with_code(
            1,
            vec![entrypoint("main", EntryPointKind::Public, 0)],
            None,
            &[ivm::encoding::wide::encode_halt(), invalid],
        );
        let error = ivm::verify_contract_artifact(&bytes)
            .expect_err("strict contract control-flow rules apply to the complete artifact");
        assert!(
            error.to_string().contains(expected),
            "unreachable {encoding} was admitted: {error}"
        );
    }
}

#[test]
fn verify_rejects_disallowed_syscalls_before_execution() {
    let disallowed = ivm::encoding::wide::encode_sys(ivm::instruction::wide::system::SCALL, 0x04);
    assert!(!ivm::syscalls::is_syscall_allowed(
        ivm::SyscallPolicy::AbiV1,
        0x04
    ));
    let bytes = contract_artifact_with_code(
        1,
        vec![entrypoint("main", EntryPointKind::Public, 0)],
        None,
        &[disallowed, ivm::encoding::wide::encode_halt()],
    );
    let err = ivm::verify_contract_artifact(&bytes)
        .expect_err("unknown bytecode syscall must fail artifact admission");
    assert!(err.to_string().contains("disallowed syscall"));
}

#[test]
fn verify_rejects_private_input_syscall_without_zk_mode() {
    let private_input = ivm::encoding::wide::encode_sys(
        ivm::instruction::wide::system::SCALL,
        ivm::syscalls::SYSCALL_GET_PRIVATE_INPUT as u8,
    );
    let bytes = contract_artifact_with_code(
        1,
        vec![entrypoint("main", EntryPointKind::Public, 0)],
        None,
        &[private_input, ivm::encoding::wide::encode_halt()],
    );

    let error = ivm::verify_contract_artifact(&bytes)
        .expect_err("non-ZK artifacts must not admit private-input syscalls");
    assert!(
        error.to_string().contains("requires ZK execution mode"),
        "unexpected admission error: {error}"
    );
}

#[test]
fn verify_derives_transitive_view_effects_from_bytecode() {
    use ivm::instruction::wide;

    let state_write = ivm::encoding::wide::encode_sys(
        wide::system::SCALL,
        ivm::syscalls::SYSCALL_STATE_SET as u8,
    );
    let return_from_helper = ivm::encoding::wide::encode_rr(wide::control::JALR, 0, 1, 0);
    let calls = [
        (
            "JAL",
            ivm::encoding::wide::encode_jump(wide::control::JAL, 1, 2),
        ),
        (
            "JALS",
            ivm::encoding::wide::encode_offset24(wide::control::JALS, 2),
        ),
    ];

    for (encoding, call_helper) in calls {
        let code = [
            call_helper,
            ivm::encoding::wide::encode_halt(),
            state_write,
            return_from_helper,
        ];

        let malicious_view = contract_artifact_with_code(
            1,
            vec![entrypoint("inspect", EntryPointKind::View, 0)],
            None,
            &code,
        );
        let error = ivm::verify_contract_artifact(&malicious_view)
            .expect_err("a view must not hide a state write in a helper");
        assert!(
            error
                .to_string()
                .contains("transitively reaches effectful syscall"),
            "{encoding} call was not included in view reachability: {error}"
        );

        let mut authorized = entrypoint("mutate", EntryPointKind::Public, 0);
        authorized.write_keys = vec!["state:*".to_owned()];
        let authorized_entry = contract_artifact_with_code(1, vec![authorized], None, &code);
        ivm::verify_contract_artifact(&authorized_entry)
            .expect("the same write is valid behind an authorized entrypoint");
    }
}

#[test]
fn strict_return_integrity_traps_view_return_address_poisoning_before_the_write() {
    use ivm::instruction::wide;

    let code = [
        // Copy an attacker-controlled target into r1, then use the syntactically
        // canonical return form to try to enter the hidden state-write block.
        ivm::encoding::wide::encode_ri(wide::arithmetic::ADDI, 1, 2, 0),
        ivm::encoding::wide::encode_rr(wide::control::JALR, 0, 1, 0),
        ivm::encoding::wide::encode_sys(
            wide::system::SCALL,
            ivm::syscalls::SYSCALL_STATE_SET as u8,
        ),
        ivm::encoding::wide::encode_halt(),
    ];
    let malicious_view = contract_artifact_with_code(
        1,
        vec![entrypoint("inspect", EntryPointKind::View, 0)],
        None,
        &code,
    );
    ivm::verify_contract_artifact(&malicious_view)
        .expect("the hidden block is unreachable under protected return semantics");

    let prepared = ivm::prepare_contract(std::sync::Arc::from(malicious_view.into_boxed_slice()))
        .expect("poisoning fixture prepares");
    let entry_pc = prepared
        .entrypoint_pc("inspect")
        .expect("view entrypoint is indexed");
    let hidden_write_pc = entry_pc + 8;
    let mut vm = ivm::IVM::new(u64::MAX);
    vm.load_prepared(&prepared).expect("prepared view loads");
    vm.set_register(2, hidden_write_pc);
    vm.set_program_counter(entry_pc)
        .expect("select malicious view");

    let error = vm
        .run()
        .expect_err("a poisoned canonical return must trap before the write");
    assert_eq!(error, ivm::VMError::AssertionFailed);
    assert_eq!(vm.pc(), entry_pc + 4, "the hidden write was not reached");
}

#[test]
fn strict_outer_return_cannot_switch_between_valid_halt_sentinels() {
    use ivm::instruction::wide;

    let code = [
        // The invocation starts with r1 pointing at the first HALT. Bytecode
        // then attempts to replace it with a different, otherwise valid HALT.
        ivm::encoding::wide::encode_ri(wide::arithmetic::ADDI, 1, 2, 0),
        ivm::encoding::wide::encode_rr(wide::control::JALR, 0, 1, 0),
        ivm::encoding::wide::encode_halt(),
        ivm::encoding::wide::encode_halt(),
    ];
    let bytes = contract_artifact_with_code(
        1,
        vec![entrypoint("inspect", EntryPointKind::View, 0)],
        None,
        &code,
    );
    let prepared = ivm::prepare_contract(std::sync::Arc::from(bytes.into_boxed_slice()))
        .expect("dual-HALT fixture prepares");
    let entry_pc = prepared
        .entrypoint_pc("inspect")
        .expect("view entrypoint is indexed");
    let mut vm = ivm::IVM::new(u64::MAX);
    vm.load_prepared(&prepared).expect("prepared view loads");
    vm.set_register(1, entry_pc + 8);
    vm.set_register(2, entry_pc + 12);
    vm.set_program_counter(entry_pc).expect("select view");

    assert_eq!(
        vm.run()
            .expect_err("outer return must remain bound to the initial HALT"),
        ivm::VMError::AssertionFailed
    );
    assert_eq!(vm.pc(), entry_pc + 4);
}

#[test]
fn verify_allows_read_only_helper_beside_a_mutating_entrypoint() {
    use ivm::instruction::wide;

    let code = [
        ivm::encoding::wide::encode_offset24(wide::control::JALS, 4),
        ivm::encoding::wide::encode_halt(),
        ivm::encoding::wide::encode_sys(
            wide::system::SCALL,
            ivm::syscalls::SYSCALL_STATE_SET as u8,
        ),
        ivm::encoding::wide::encode_halt(),
        ivm::encoding::wide::encode_sys(
            wide::system::SCALL,
            ivm::syscalls::SYSCALL_STATE_GET as u8,
        ),
        ivm::encoding::wide::encode_rr(wide::control::JALR, 0, 1, 0),
    ];
    let mut inspect = entrypoint("inspect", EntryPointKind::View, 0);
    inspect.read_keys = vec!["state:*".to_owned()];
    let mut mutate = entrypoint("mutate", EntryPointKind::Public, 8);
    mutate.write_keys = vec!["state:*".to_owned()];
    let bytes = contract_artifact_with_code(1, vec![inspect, mutate], None, &code);

    ivm::verify_contract_artifact(&bytes).expect(
        "a read-only helper must not inherit an unreachable sibling entrypoint's write effect",
    );
}

#[test]
fn strict_return_integrity_allows_nested_direct_calls_for_raw_and_prepared_loads() {
    use ivm::instruction::wide;

    let code = [
        // Outer call: return to the HALT at pc 4.
        ivm::encoding::wide::encode_jump(wide::control::JAL, 1, 2),
        ivm::encoding::wide::encode_halt(),
        // Non-leaf helper saves r1, calls the leaf, restores r1, and returns.
        ivm::encoding::wide::encode_store(wide::memory::STORE64, 31, 1, -8),
        ivm::encoding::wide::encode_offset24(wide::control::JALS, 3),
        ivm::encoding::wide::encode_ri(wide::memory::LOAD64, 1, 31, -8),
        ivm::encoding::wide::encode_rr(wide::control::JALR, 0, 1, 0),
        ivm::encoding::wide::encode_ri(wide::arithmetic::ADDI, 7, 0, 9),
        ivm::encoding::wide::encode_rr(wide::control::JALR, 0, 1, 0),
    ];
    let bytes = contract_artifact_with_code(
        1,
        vec![entrypoint("main", EntryPointKind::Public, 0)],
        None,
        &code,
    );
    let prepared = ivm::prepare_contract(std::sync::Arc::from(bytes.clone().into_boxed_slice()))
        .expect("nested call fixture prepares");

    let mut raw = ivm::IVM::new(u64::MAX);
    raw.load_program(&bytes).expect("raw contract loads");
    raw.run().expect("raw contract nested calls return");
    assert_eq!(raw.register(7), 9);

    let mut warm = ivm::IVM::new(u64::MAX);
    warm.load_prepared(&prepared)
        .expect("prepared contract loads");
    warm.run().expect("prepared contract nested calls return");
    assert_eq!(warm.register(7), 9);
}

#[test]
fn strict_return_stack_has_a_deterministic_depth_bound() {
    use ivm::instruction::wide;

    let bytes = contract_artifact_with_code(
        1,
        vec![entrypoint("main", EntryPointKind::Public, 0)],
        None,
        &[
            ivm::encoding::wide::encode_offset24(wide::control::JALS, 0),
            ivm::encoding::wide::encode_halt(),
        ],
    );
    let prepared = ivm::prepare_contract(std::sync::Arc::from(bytes.into_boxed_slice()))
        .expect("cyclic direct-call fixture prepares");
    let mut vm = ivm::IVM::new(u64::MAX);
    vm.load_prepared(&prepared)
        .expect("prepared contract loads");

    assert_eq!(
        vm.run().expect_err("unbounded call depth must trap"),
        ivm::VMError::AssertionFailed
    );
}

#[test]
fn verify_view_can_call_a_read_only_helper_when_the_artifact_has_no_writes() {
    use ivm::instruction::wide;

    let code = [
        ivm::encoding::wide::encode_jump(wide::control::JAL, 1, 2),
        ivm::encoding::wide::encode_halt(),
        ivm::encoding::wide::encode_sys(
            wide::system::SCALL,
            ivm::syscalls::SYSCALL_STATE_GET as u8,
        ),
        ivm::encoding::wide::encode_rr(wide::control::JALR, 0, 1, 0),
    ];
    let mut inspect = entrypoint("inspect", EntryPointKind::View, 0);
    inspect.read_keys = vec!["state:*".to_owned()];
    let bytes = contract_artifact_with_code(1, vec![inspect], None, &code);

    ivm::verify_contract_artifact(&bytes)
        .expect("an indirect return is safe when no artifact instruction can write state");
}

#[test]
fn verify_view_effect_analysis_follows_both_branch_arms() {
    use ivm::instruction::wide;

    let code = [
        ivm::encoding::wide::encode_branch(wide::control::BEQ, 0, 0, 2),
        ivm::encoding::wide::encode_halt(),
        ivm::encoding::wide::encode_sys(
            wide::system::SCALL,
            ivm::syscalls::SYSCALL_STATE_SET as u8,
        ),
        ivm::encoding::wide::encode_halt(),
    ];
    let bytes = contract_artifact_with_code(
        1,
        vec![entrypoint("inspect", EntryPointKind::View, 0)],
        None,
        &code,
    );
    let error = ivm::verify_contract_artifact(&bytes)
        .expect_err("a taken branch must not hide a view write");
    assert!(
        error
            .to_string()
            .contains("transitively reaches effectful syscall")
    );
}

#[test]
fn verify_view_effect_analysis_decodes_scall_and_system_writes() {
    use ivm::instruction::wide;

    let encodings = [
        (
            "high-bit SCALL",
            ivm::encoding::wide::encode_sys(
                wide::system::SCALL,
                ivm::syscalls::SYSCALL_SORACLOUD_EMIT_STATE_MUTATION as u8,
            ),
        ),
        (
            "extended SYSTEM",
            ivm::encoding::wide::encode_syscallx(ivm::syscalls::SYSCALL_STATE_SET),
        ),
    ];

    for (encoding, write) in encodings {
        let bytes = contract_artifact_with_code(
            1,
            vec![entrypoint("inspect", EntryPointKind::View, 0)],
            None,
            &[write, ivm::encoding::wide::encode_halt()],
        );
        let error = ivm::verify_contract_artifact(&bytes)
            .expect_err("every syscall encoding must participate in view-effect validation");
        assert!(
            error
                .to_string()
                .contains("transitively reaches effectful syscall"),
            "{encoding} was not decoded as an effectful syscall: {error}"
        );
    }
}

#[test]
fn verify_view_effect_analysis_ignores_unreachable_write_code() {
    use ivm::instruction::wide;

    let state_write = ivm::encoding::wide::encode_sys(
        wide::system::SCALL,
        ivm::syscalls::SYSCALL_STATE_SET as u8,
    );
    let bytes = contract_artifact_with_code(
        1,
        vec![entrypoint("inspect", EntryPointKind::View, 0)],
        None,
        &[
            ivm::encoding::wide::encode_halt(),
            state_write,
            ivm::encoding::wide::encode_halt(),
        ],
    );

    ivm::verify_contract_artifact(&bytes)
        .expect("unreachable code must not contaminate a read-only entrypoint");
}

#[test]
fn verify_rejects_unverifiable_indirect_entrypoint_control_flow() {
    use ivm::instruction::wide;

    let indirect_jumps = [
        ivm::encoding::wide::encode_rr(wide::control::JALR, 0, 2, 0),
        ivm::encoding::wide::encode_rr(wide::control::JR, 2, 0, 0),
    ];
    for indirect in indirect_jumps {
        let bytes = contract_artifact_with_code(
            1,
            vec![entrypoint("main", EntryPointKind::Public, 0)],
            None,
            &[indirect],
        );
        let error = ivm::verify_contract_artifact(&bytes)
            .expect_err("indirect entrypoint control flow cannot be admitted from CNTR claims");
        assert!(
            error
                .to_string()
                .contains("unverifiable indirect control flow")
        );
    }
}

#[test]
fn verify_rejects_helper_hidden_access_classes_under_reported_by_cntr() {
    use ivm::instruction::wide;

    for (label, syscall, expected_access) in [
        (
            "ledger write",
            ivm::syscalls::SYSCALL_TRANSFER_ASSET_SCOPED,
            "LedgerWrite",
        ),
        (
            "dynamic nested call",
            ivm::syscalls::SYSCALL_CALL_CONTRACT,
            "Dynamic",
        ),
    ] {
        let code = [
            ivm::encoding::wide::encode_offset24(wide::control::JALS, 2),
            ivm::encoding::wide::encode_halt(),
            ivm::encoding::wide::encode_syscallx(syscall),
            ivm::encoding::wide::encode_rr(wide::control::JALR, 0, 1, 0),
        ];
        let mut forged = entrypoint("mutate", EntryPointKind::Public, 0);
        forged.read_keys = vec!["state:decoy".to_owned()];
        forged.write_keys = vec!["state:decoy".to_owned()];
        let bytes = contract_artifact_with_code(1, vec![forged], None, &code);

        let error = ivm::verify_contract_artifact(&bytes)
            .expect_err("complete CNTR access claims must cover bytecode-derived access classes");
        let message = error.to_string();
        assert!(message.contains("under-reports transitively reachable"));
        assert!(
            message.contains(expected_access),
            "{label} did not report its derived access class: {message}"
        );
    }
}

#[test]
fn verify_rejects_duplicate_entrypoints() {
    let bytes = contract_artifact(
        1,
        vec![
            entrypoint("main", EntryPointKind::Public, 0),
            entrypoint("main", EntryPointKind::Public, 0),
        ],
    );
    let err = ivm::verify_contract_artifact(&bytes).expect_err("duplicate entrypoints must fail");
    assert!(err.to_string().contains("duplicate entrypoint `main`"));
}

#[test]
fn verify_rejects_entrypoint_pc_aliases_and_missing_authorization() {
    let bytes = contract_artifact(
        1,
        vec![
            entrypoint("first", EntryPointKind::Public, 0),
            entrypoint("second", EntryPointKind::View, 0),
        ],
    );
    let err = ivm::verify_contract_artifact(&bytes).expect_err("entrypoint PC alias must fail");
    assert!(err.to_string().contains("reuses entry_pc"));

    let mut public = entrypoint("main", EntryPointKind::Public, 0);
    public.permission = None;
    let bytes = contract_artifact(1, vec![public]);
    let err = ivm::verify_contract_artifact(&bytes)
        .expect_err("public entrypoint without authorization must fail");
    assert!(err.to_string().contains("missing caller authorization"));
}

#[test]
fn verify_rejects_source_controlled_lifecycle_authorization() {
    for kind in [EntryPointKind::Init, EntryPointKind::Upgrade] {
        let mut lifecycle = entrypoint("lifecycle", kind, 0);
        lifecycle.permission = Some("SourceCannotControlLifecycle".to_owned());
        let artifact = contract_artifact(1, vec![lifecycle]);
        let err = ivm::verify_contract_artifact(&artifact)
            .expect_err("lifecycle authorization must be runtime-defined");
        assert!(
            err.to_string()
                .contains("must use runtime-defined authorization"),
            "unexpected error: {err}"
        );
    }
}

#[test]
fn verify_rejects_inconsistent_access_completeness() {
    let mut complete = entrypoint("main", EntryPointKind::Public, 0);
    complete.access_hints_skipped = vec!["dynamic path".to_owned()];
    let err = ivm::verify_contract_artifact(&contract_artifact(1, vec![complete]))
        .expect_err("complete hints with skipped reasons must fail");
    assert!(err.to_string().contains("marks access hints complete"));

    let mut incomplete = entrypoint("main", EntryPointKind::Public, 0);
    incomplete.access_hints_complete = Some(false);
    let err = ivm::verify_contract_artifact(&contract_artifact(1, vec![incomplete]))
        .expect_err("incomplete hints without reason must fail");
    assert!(err.to_string().contains("without a reason"));
}

#[test]
fn verify_rejects_invalid_entry_pc() {
    for invalid_pc in [1, 2, 3, 4, u64::MAX] {
        let bytes = contract_artifact(
            1,
            vec![entrypoint("main", EntryPointKind::Public, invalid_pc)],
        );
        let err = ivm::verify_contract_artifact(&bytes)
            .expect_err("misaligned and out-of-code entry PCs must fail");
        assert!(
            err.to_string().contains("invalid entry_pc"),
            "entry_pc {invalid_pc} produced an unexpected error: {err}"
        );
    }
}

#[test]
fn verify_rejects_invalid_trigger_callback_target() {
    let mut main = entrypoint("main", EntryPointKind::Public, 0);
    main.triggers.push(
        iroha_data_model::smart_contract::manifest::TriggerDescriptor {
            id: TriggerId::new("wake".parse().expect("trigger id")),
            repeats: Repeats::Indefinitely,
            filter: iroha_data_model::events::EventFilterBox::Time(
                iroha_data_model::events::time::TimeEventFilter(
                    iroha_data_model::events::time::ExecutionTime::PreCommit,
                ),
            ),
            authority: None,
            metadata: iroha_data_model::metadata::Metadata::default(),
            callback: iroha_data_model::smart_contract::manifest::TriggerCallback {
                namespace: None,
                entrypoint: "missing".to_owned(),
            },
        },
    );
    let bytes = contract_artifact(1, vec![main]);
    let err = ivm::verify_contract_artifact(&bytes)
        .expect_err("invalid trigger callback target must fail");
    assert!(err.to_string().contains("callback target `missing`"));
}

#[test]
fn verify_accepts_namespaced_trigger_callback_target() {
    let mut main = entrypoint("main", EntryPointKind::Public, 0);
    main.triggers.push(
        iroha_data_model::smart_contract::manifest::TriggerDescriptor {
            id: TriggerId::new("wake".parse().expect("trigger id")),
            repeats: Repeats::Indefinitely,
            filter: iroha_data_model::events::EventFilterBox::Time(
                iroha_data_model::events::time::TimeEventFilter(
                    iroha_data_model::events::time::ExecutionTime::PreCommit,
                ),
            ),
            authority: None,
            metadata: iroha_data_model::metadata::Metadata::default(),
            callback: iroha_data_model::smart_contract::manifest::TriggerCallback {
                namespace: Some("callee".to_owned()),
                entrypoint: "run".to_owned(),
            },
        },
    );
    let bytes = contract_artifact(1, vec![main]);

    ivm::verify_contract_artifact(&bytes)
        .expect("namespaced trigger callback target is resolved at activation");
}

#[test]
fn verify_accepts_global_access_wildcard_hints() {
    let hints = AccessSetHints {
        read_keys: vec!["*".to_owned()],
        write_keys: vec!["*".to_owned()],
        dynamic_reads: Vec::new(),
        dynamic_writes: Vec::new(),
    };
    let bytes = contract_artifact_with_access_hints(
        1,
        vec![entrypoint("main", EntryPointKind::Public, 0)],
        Some(hints),
    );

    ivm::verify_contract_artifact(&bytes).expect("global wildcard access hints are supported");
}

#[test]
fn verify_accepts_state_access_wildcard_hints() {
    let hints = AccessSetHints {
        read_keys: vec!["state:*".to_owned()],
        write_keys: vec!["state:*".to_owned()],
        dynamic_reads: Vec::new(),
        dynamic_writes: Vec::new(),
    };
    let bytes = contract_artifact_with_access_hints(
        1,
        vec![entrypoint("main", EntryPointKind::Public, 0)],
        Some(hints),
    );

    ivm::verify_contract_artifact(&bytes).expect("state wildcard access hints are supported");
}

#[test]
fn verify_rejects_invalid_dynamic_access_hints() {
    let hints = AccessSetHints {
        read_keys: Vec::new(),
        write_keys: Vec::new(),
        dynamic_reads: vec![DynamicAccessHint {
            base_key: "state:*".to_owned(),
            key_type: "i64".to_owned(),
            bound_kind: "take".to_owned(),
            max_keys: 64,
        }],
        dynamic_writes: Vec::new(),
    };
    let bytes = contract_artifact_with_access_hints(
        1,
        vec![entrypoint("main", EntryPointKind::Public, 0)],
        Some(hints),
    );
    let err = ivm::verify_contract_artifact(&bytes).expect_err("wildcard dynamic hint must fail");
    assert!(
        err.to_string()
            .contains("unsupported dynamic access base `state:*`")
    );

    let hints = AccessSetHints {
        read_keys: Vec::new(),
        write_keys: Vec::new(),
        dynamic_reads: Vec::new(),
        dynamic_writes: vec![DynamicAccessHint {
            base_key: "state:Orders".to_owned(),
            key_type: "i64".to_owned(),
            bound_kind: "range".to_owned(),
            max_keys: 0,
        }],
    };
    let bytes = contract_artifact_with_access_hints(
        1,
        vec![entrypoint("main", EntryPointKind::Public, 0)],
        Some(hints),
    );
    let err = ivm::verify_contract_artifact(&bytes).expect_err("zero dynamic hint must fail");
    assert!(
        err.to_string()
            .contains("zero-bound dynamic access hint `state:Orders`")
    );
}

#[test]
fn verify_rejects_unsupported_abi_version() {
    let bytes = contract_artifact(2, vec![entrypoint("main", EntryPointKind::Public, 0)]);
    let err = ivm::verify_contract_artifact(&bytes).expect_err("abi version mismatch must fail");
    assert!(err.to_string().contains("unsupported abi_version 2"));
}
