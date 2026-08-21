use iroha_data_model::{
    smart_contract::entrypoint::{
        EntrypointArgumentFieldV1, EntrypointArgumentSchemaV1, EntrypointListTypeNodeV1,
        EntrypointStructTypeNodeV1, EntrypointValueKindV1, EntrypointValueTypeNodeV1,
        EntrypointValueTypeV1,
    },
    smart_contract::manifest::{
        AccessSetHints, ContractErrorCodeDescriptor, DynamicAccessHint, EntryPointKind,
        EntrypointParamDescriptor, TriggerCallback, TriggerDescriptor,
    },
    trigger::{TriggerId, action::Repeats},
};
mod common;
fn time_trigger(id: &str, namespace: Option<&str>, entrypoint: &str) -> TriggerDescriptor {
    TriggerDescriptor {
        id: TriggerId::new(id.parse().expect("trigger id")),
        repeats: Repeats::Indefinitely,
        filter: iroha_data_model::events::EventFilterBox::Time(
            iroha_data_model::events::time::TimeEventFilter(
                iroha_data_model::events::time::ExecutionTime::PreCommit,
            ),
        ),
        authority: None,
        metadata: iroha_data_model::metadata::Metadata::default(),
        callback: TriggerCallback {
            namespace: namespace.map(str::to_owned),
            entrypoint: entrypoint.to_owned(),
        },
    }
}
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
        return_schema: None,
        permission: (kind == EntryPointKind::Kotoage).then(|| "Execute".to_owned()),
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
    contract_artifact_with_mode_and_code(abi_version, 0, 0, entrypoints, access_set_hints, code)
}
fn contract_artifact_with_mode_and_code(
    abi_version: u8,
    mode: u8,
    features_bitmap: u64,
    entrypoints: Vec<ivm::EmbeddedEntrypointDescriptor>,
    access_set_hints: Option<AccessSetHints>,
    code: &[u32],
) -> Vec<u8> {
    let meta = ivm::ProgramMetadata {
        version_major: 1,
        version_minor: 1,
        mode,
        vector_length: 0,
        max_cycles: 0,
        abi_version,
    };
    let interface = ivm::EmbeddedContractInterfaceV1 {
        seiyaku_name: "TestContract".to_owned(),
        compiler_fingerprint: "ivm-tests".to_owned(),
        abi_hash: ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1),
        features_bitmap,
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
        seiyaku_name: "TestContract".to_owned(),
        compiler_fingerprint: "ivm-tests".to_owned(),
        abi_hash: ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1),
        features_bitmap: 0,
        access_set_hints: None,
        kotoba: Vec::new(),
        entrypoints: vec![entrypoint("main", EntryPointKind::Kotoage, 0)],
        error_codes,
        states: Vec::new(),
    };
    let mut bytes = meta.encode();
    bytes.extend_from_slice(&interface.encode_section());
    bytes.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());
    bytes
}
fn contract_artifact_with_states(states: Vec<ivm::EmbeddedStateDescriptor>) -> Vec<u8> {
    contract_artifact_with_access_hints_and_states(None, states)
}
fn contract_artifact_with_access_hints_and_states(
    access_set_hints: Option<AccessSetHints>,
    states: Vec<ivm::EmbeddedStateDescriptor>,
) -> Vec<u8> {
    let meta = ivm::ProgramMetadata {
        version_major: 1,
        version_minor: 1,
        mode: 0,
        vector_length: 0,
        max_cycles: 0,
        abi_version: 1,
    };
    let interface = ivm::EmbeddedContractInterfaceV1 {
        seiyaku_name: "TestContract".to_owned(),
        compiler_fingerprint: "ivm-tests".to_owned(),
        abi_hash: ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1),
        features_bitmap: 0,
        access_set_hints,
        kotoba: Vec::new(),
        entrypoints: vec![entrypoint("main", EntryPointKind::Kotoage, 0)],
        error_codes: Vec::new(),
        states,
    };
    let mut bytes = meta.encode();
    bytes.extend_from_slice(&interface.encode_section());
    bytes.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());
    bytes
}
fn contract_artifact_with_seiyaku_name(seiyaku_name: &str) -> Vec<u8> {
    let meta = ivm::ProgramMetadata {
        version_major: 1,
        version_minor: 1,
        mode: 0,
        vector_length: 0,
        max_cycles: 0,
        abi_version: 1,
    };
    let interface = ivm::EmbeddedContractInterfaceV1 {
        seiyaku_name: seiyaku_name.to_owned(),
        compiler_fingerprint: "ivm-tests".to_owned(),
        abi_hash: ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1),
        features_bitmap: 0,
        access_set_hints: None,
        kotoba: Vec::new(),
        entrypoints: vec![entrypoint("run", EntryPointKind::Kotoage, 0)],
        error_codes: Vec::new(),
        states: Vec::new(),
    };
    let mut bytes = meta.encode();
    bytes.extend_from_slice(&interface.encode_section());
    bytes.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());
    bytes
}
fn contract_artifact_with_execution_features(mode: u8, features_bitmap: u64) -> Vec<u8> {
    let metadata = ivm::ProgramMetadata {
        version_major: 1,
        version_minor: 1,
        mode,
        vector_length: 0,
        max_cycles: 0,
        abi_version: 1,
    };
    let interface = ivm::EmbeddedContractInterfaceV1 {
        seiyaku_name: "FeatureBinding".to_owned(),
        compiler_fingerprint: "ivm-tests".to_owned(),
        abi_hash: ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1),
        features_bitmap,
        access_set_hints: None,
        kotoba: Vec::new(),
        entrypoints: vec![entrypoint("inspect", EntryPointKind::View, 0)],
        error_codes: Vec::new(),
        states: Vec::new(),
    };
    let mut bytes = metadata.encode();
    bytes.extend_from_slice(&interface.encode_section());
    bytes.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());
    bytes
}
fn value_type(kind: EntrypointValueKindV1) -> EntrypointValueTypeV1 {
    EntrypointValueTypeV1 {
        nodes: vec![EntrypointValueTypeNodeV1::Leaf(kind)],
    }
}
#[test]
fn verifier_rejects_stale_embedded_abi_hash_before_execution() {
    let artifact = contract_artifact(1, vec![entrypoint("inspect", EntryPointKind::View, 0)]);
    let parsed = ivm::ProgramMetadata::parse(&artifact).expect("parse valid contract artifact");
    let mut interface = parsed
        .contract_interface
        .expect("contract fixture carries CNTR");
    interface.abi_hash[0] ^= 0x80;
    let mut stale = parsed.metadata.encode();
    stale.extend_from_slice(&interface.encode_section());
    stale.extend_from_slice(
        artifact
            .get(parsed.code_offset..)
            .expect("parsed code offset is in bounds"),
    );
    let error = ivm::verify_contract_artifact(&stale)
        .expect_err("stale embedded ABI binding must fail closed");
    assert!(
        error
            .to_string()
            .contains("contract interface abi_hash does not match the runtime ABI descriptor"),
        "unexpected stale-ABI error: {error}"
    );
}
#[test]
fn verifier_rejects_mismatched_or_oversized_exact_boundary_schemas() {
    let mut argument_mismatch = entrypoint("inspect", EntryPointKind::View, 0);
    argument_mismatch.params = vec![EntrypointParamDescriptor {
        name: "value".to_owned(),
        type_name: "int".to_owned(),
    }];
    argument_mismatch.argument_schema = Some(EntrypointArgumentSchemaV1 {
        fields: vec![EntrypointArgumentFieldV1 {
            name: "value".to_owned(),
            ty: value_type(EntrypointValueKindV1::Bool),
        }],
    });
    let argument_code = [
        ivm::encoding::wide::encode_syscallx(ivm::syscalls::SYSCALL_DECODE_ARGUMENT_RECORD),
        ivm::encoding::wide::encode_halt(),
    ];
    let artifact = contract_artifact_with_code(1, vec![argument_mismatch], None, &argument_code);
    let error = ivm::verify_contract_artifact(&artifact)
        .expect_err("argument schema/type mismatch must fail");
    assert!(error.to_string().contains("invalid argument schema"));
    let mut return_mismatch = entrypoint("inspect", EntryPointKind::View, 0);
    return_mismatch.return_type = Some("int".to_owned());
    return_mismatch.return_schema = Some(value_type(EntrypointValueKindV1::Bool));
    let artifact = contract_artifact(1, vec![return_mismatch]);
    let error = ivm::verify_contract_artifact(&artifact)
        .expect_err("return schema/type mismatch must fail");
    assert!(error.to_string().contains("return schema"));
    let mut oversized = entrypoint("inspect", EntryPointKind::View, 0);
    oversized.return_type = Some(format!("({})", vec!["int"; 14].join(", ")));
    oversized.return_schema = Some(EntrypointValueTypeV1 {
        nodes: std::iter::once(EntrypointValueTypeNodeV1::Tuple(14))
            .chain(std::iter::repeat_n(
                EntrypointValueTypeNodeV1::Leaf(EntrypointValueKindV1::Int),
                14,
            ))
            .collect(),
    });
    let artifact = contract_artifact(1, vec![oversized]);
    let error = ivm::verify_contract_artifact(&artifact)
        .expect_err("14-word public return must fail closed");
    assert!(error.to_string().contains("public register window"));
}
#[test]
#[allow(clippy::too_many_lines)]
fn verifier_rejects_forged_reserved_query_page_schemas() {
    fn account_page_schema() -> EntrypointValueTypeV1 {
        EntrypointValueTypeV1 {
            nodes: vec![
                EntrypointValueTypeNodeV1::Struct(EntrypointStructTypeNodeV1 {
                    name: "QueryPage".to_owned(),
                    fields: vec!["items".to_owned(), "next_offset".to_owned()],
                }),
                EntrypointValueTypeNodeV1::List(EntrypointListTypeNodeV1 { capacity: 64 }),
                EntrypointValueTypeNodeV1::Struct(EntrypointStructTypeNodeV1 {
                    name: "AccountView".to_owned(),
                    fields: vec!["id".to_owned(), "metadata".to_owned()],
                }),
                EntrypointValueTypeNodeV1::Leaf(EntrypointValueKindV1::AccountId),
                EntrypointValueTypeNodeV1::Leaf(EntrypointValueKindV1::Json),
                EntrypointValueTypeNodeV1::Option,
                EntrypointValueTypeNodeV1::Leaf(EntrypointValueKindV1::Int),
            ],
        }
    }
    let valid = account_page_schema();
    let singular = EntrypointValueTypeV1 {
        nodes: std::iter::once(EntrypointValueTypeNodeV1::Option)
            .chain(
                valid
                    .subtree_nodes(2)
                    .expect("page fixture has a valid AccountView subtree")
                    .iter()
                    .cloned(),
            )
            .collect(),
    };
    let mut singular_descriptor = entrypoint("account", EntryPointKind::View, 0);
    singular_descriptor.return_type = Some("Option<AccountView>".to_owned());
    singular_descriptor.return_schema = Some(singular);
    ivm::verify_contract_artifact(&contract_artifact(1, vec![singular_descriptor]))
        .expect("exact reserved singular projection schema must be admitted");
    let mut descriptor = entrypoint("inspect", EntryPointKind::View, 0);
    descriptor.return_type = Some("QueryPage<AccountView>".to_owned());
    descriptor.return_schema = Some(valid.clone());
    ivm::verify_contract_artifact(&contract_artifact(1, vec![descriptor]))
        .expect("exact reserved QueryPage schema must be admitted");
    let assert_rejected = |label: &str, schema: EntrypointValueTypeV1, return_type: &str| {
        let mut descriptor = entrypoint("inspect", EntryPointKind::View, 0);
        descriptor.return_type = Some(return_type.to_owned());
        descriptor.return_schema = Some(schema);
        let error = match ivm::verify_contract_artifact(&contract_artifact(1, vec![descriptor])) {
            Ok(_) => panic!("{label} must fail artifact admission"),
            Err(error) => error,
        };
        assert!(
            error.to_string().starts_with("invalid contract artifact:"),
            "{label}: {error}"
        );
    };
    let mut unknown_view = valid.clone();
    let EntrypointValueTypeNodeV1::Struct(view) = &mut unknown_view.nodes[2] else {
        unreachable!("page fixture has a projected struct")
    };
    view.name = "UnknownView".to_owned();
    assert_rejected("unknown projection", unknown_view, "QueryPage<UnknownView>");
    let mut wrong_fields = valid.clone();
    let EntrypointValueTypeNodeV1::Struct(view) = &mut wrong_fields.nodes[2] else {
        unreachable!("page fixture has a projected struct")
    };
    view.fields[1] = "content".to_owned();
    assert_rejected(
        "reserved projection with wrong fields",
        wrong_fields,
        "QueryPage<AccountView>",
    );
    let mut wrong_kind = valid.clone();
    wrong_kind.nodes[3] = EntrypointValueTypeNodeV1::Leaf(EntrypointValueKindV1::DomainId);
    assert_rejected(
        "reserved projection with wrong leaf kind",
        wrong_kind,
        "QueryPage<AccountView>",
    );
    let mut wrong_capacity = valid.clone();
    let EntrypointValueTypeNodeV1::List(items) = &mut wrong_capacity.nodes[1] else {
        unreachable!("page fixture has an items list")
    };
    items.capacity = 32;
    assert_rejected(
        "wrong page capacity",
        wrong_capacity,
        "QueryPage<AccountView>",
    );
    let mut wrong_next_offset = valid;
    wrong_next_offset.nodes[6] = EntrypointValueTypeNodeV1::Leaf(EntrypointValueKindV1::String);
    assert_rejected(
        "wrong next_offset type",
        wrong_next_offset,
        "QueryPage<AccountView>",
    );
}
#[test]
fn compiler_embeds_exact_nested_return_schema_in_cntr_and_manifest() {
    let source = r#"
        seiyaku ExactReturn {
            struct Pair { int count, bool ready }

            view fn inspect() -> Result<Option<Pair>, (string, bool)> {
                return Result::ok(Option::some(Pair { count: 7, ready: true }));
            }
        }
    "#;
    let (artifact, manifest) = ivm::KotodamaCompiler::new()
        .compile_source_with_manifest(source)
        .expect("compile nested return schema");
    let parsed = ivm::ProgramMetadata::parse(&artifact).expect("parse compiled artifact");
    let embedded = parsed
        .contract_interface
        .as_ref()
        .and_then(|interface| interface.entrypoints.first())
        .expect("embedded entrypoint");
    let schema = embedded
        .return_schema
        .as_ref()
        .expect("exact return schema");
    assert_eq!(
        schema.nodes,
        vec![
            EntrypointValueTypeNodeV1::Result,
            EntrypointValueTypeNodeV1::Option,
            EntrypointValueTypeNodeV1::Struct(EntrypointStructTypeNodeV1 {
                name: "Pair".to_owned(),
                fields: vec!["count".to_owned(), "ready".to_owned()],
            }),
            EntrypointValueTypeNodeV1::Leaf(EntrypointValueKindV1::Int),
            EntrypointValueTypeNodeV1::Leaf(EntrypointValueKindV1::Bool),
            EntrypointValueTypeNodeV1::Tuple(2),
            EntrypointValueTypeNodeV1::Leaf(EntrypointValueKindV1::String),
            EntrypointValueTypeNodeV1::Leaf(EntrypointValueKindV1::Bool),
        ],
    );
    assert_eq!(
        schema.word_count(),
        Some(1),
        "nested active-only Option/Result values cross the public register boundary as one typed handle"
    );
    assert_eq!(
        manifest
            .entrypoints
            .as_ref()
            .and_then(|entrypoints| entrypoints.first())
            .and_then(|entrypoint| entrypoint.return_schema.as_ref()),
        Some(schema),
    );
    ivm::verify_contract_artifact(&artifact).expect("verify exact nested return artifact");
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
    for (namespace, name) in [
        ("1PaymentError", "Unauthorized"),
        ("PaymеntError", "Unauthorized"), // Cyrillic `е`.
        ("Option", "Unauthorized"),
        ("PaymentError", "not-valid"),
        ("PaymentError", "for"),
    ] {
        let artifact = contract_artifact_with_error_codes(vec![ContractErrorCodeDescriptor {
            namespace: namespace.to_owned(),
            name: name.to_owned(),
            code: 1001,
        }]);
        let error = ivm::verify_contract_artifact(&artifact)
            .expect_err("noncanonical or reserved error path must fail admission");
        assert!(
            error
                .to_string()
                .contains("canonical Kotodama V1 identifiers"),
            "unexpected error for `{namespace}::{name}`: {error}"
        );
    }
}
#[test]
fn compiler_emits_self_describing_contract_artifact() {
    let src = r#"
        seiyaku Demo {
            state int counter;

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
    assert_eq!(interface.seiyaku_name, "Demo");
    assert_eq!(manifest.seiyaku_name.as_deref(), Some("Demo"));
    let verified = ivm::verify_contract_artifact(&bytes).expect("verify artifact");
    assert_eq!(
        verified.manifest.signature_payload(),
        manifest.signature_payload(),
        "compiler manifest must match the embedded contract interface",
    );
}
#[test]
fn verifier_binds_feature_bitmap_to_execution_capabilities_not_host_hardware() {
    for (mode, feature, label) in [
        (ivm::ivm_mode::ZK, ivm::CONTRACT_FEATURE_BIT_ZK, "ZK"),
        (
            ivm::ivm_mode::VECTOR,
            ivm::CONTRACT_FEATURE_BIT_VECTOR,
            "VECTOR",
        ),
    ] {
        let artifact = contract_artifact_with_execution_features(mode, feature);
        let verified = ivm::verify_contract_artifact(&artifact)
            .unwrap_or_else(|error| panic!("matching {label} capability must verify: {error}"));
        assert_eq!(verified.manifest.features_bitmap, Some(feature));
        let missing = contract_artifact_with_execution_features(mode, 0);
        let error = ivm::verify_contract_artifact(&missing)
            .expect_err("execution-header capability must be mirrored in CNTR");
        assert!(
            error.to_string().contains(label),
            "unexpected missing-{label} error: {error}"
        );
        let forged = contract_artifact_with_execution_features(0, feature);
        let error = ivm::verify_contract_artifact(&forged)
            .expect_err("CNTR cannot invent an execution capability");
        assert!(
            error.to_string().contains(label),
            "unexpected forged-{label} error: {error}"
        );
    }
    let hardware_like_bit = 1_u64 << 63;
    let error = ivm::verify_contract_artifact(&contract_artifact_with_execution_features(
        0,
        hardware_like_bit,
    ))
    .expect_err("unassigned feature bits must not encode host hardware availability");
    assert!(error.to_string().contains("unsupported bits"));
}
#[test]
fn contract_code_hash_binds_every_byte_of_compiled_deployable_image() {
    let source = r#"
        seiyaku FullImageBinding {
            kotoage fn run() -> Name authorize("ReadLiteral") {
                return Name::parse("indexed_literal");
            }
        }
    "#;
    let bytes = ivm::KotodamaCompiler::new()
        .compile_source(source)
        .expect("compile contract with CNTR, indexed literal, and code");
    let parsed = ivm::ProgramMetadata::parse(&bytes).expect("parse compiled artifact");
    assert!(parsed.contract_interface.is_some(), "CNTR must be present");
    let literals = parsed
        .literal_section
        .expect("compiled typed literal must produce LTLB");
    assert!(literals.count > 0, "LTLB must contain an indexed literal");
    assert!(
        parsed.code_offset < bytes.len(),
        "executable code must be present"
    );
    let expected = ivm::contract_code_hash(&bytes);
    for offset in 0..bytes.len() {
        let mut mutated = bytes.clone();
        mutated[offset] ^= 1;
        assert_ne!(
            ivm::contract_code_hash(&mutated),
            expected,
            "deployable artifact byte {offset} was not bound by contract_code_hash"
        );
    }
}
#[test]
fn verified_code_hash_binds_execution_header() {
    let original = contract_artifact(1, vec![entrypoint("main", EntryPointKind::Kotoage, 0)]);
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
    let original = contract_artifact(1, vec![entrypoint("main", EntryPointKind::Kotoage, 0)]);
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
    let mut abi_version = original.clone();
    abi_version[16] = 0;
    mutations.push(("abi_version", abi_version));
    for index in 17..ivm::HEADER_SIZE {
        let mut abi_hash = original.clone();
        abi_hash[index] ^= 0xff;
        mutations.push(("abi_hash", abi_hash));
    }
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

            kotoage fn run() -> int  authorize("Entry") {
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
    assert_eq!(common::decode_i64_register(&vm, 10), 42);
}
#[test]
fn contract_artifact_with_cntr_requires_explicit_entrypoint_selection() {
    let src = r#"
seiyaku ContractArtifactFixture {

        kotoage fn main() -> int authorize("Entry") {
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
    assert_eq!(common::decode_i64_register(&vm, 10), 7);
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
    let mut bytes = contract_artifact(1, vec![entrypoint("main", EntryPointKind::Kotoage, 0)]);
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
fn verify_rejects_undefined_opcodes_before_execution() {
    let bytes = contract_artifact_with_code(
        1,
        vec![entrypoint("main", EntryPointKind::Kotoage, 0)],
        None,
        &[0xff00_0000, ivm::encoding::wide::encode_halt()],
    );
    let error = ivm::verify_contract_artifact(&bytes)
        .expect_err("undefined opcode must fail shared artifact admission");
    assert_eq!(
        error.to_string(),
        "invalid contract artifact: invalid opcode 0xff at pc 0"
    );
}
#[test]
fn verify_rejects_noncanonical_poseidon6_encodings_before_execution() {
    use ivm::instruction::wide;

    for malformed in [
        ivm::encoding::wide::encode_rr(wide::crypto::POSEIDON6, 9, 10, 1),
        ivm::encoding::wide::encode_rr(wide::crypto::POSEIDON6, 9, 251, 0),
    ] {
        let bytes = contract_artifact_with_code(
            1,
            vec![entrypoint("main", EntryPointKind::Kotoage, 0)],
            None,
            &[malformed, ivm::encoding::wide::encode_halt()],
        );
        let error = ivm::verify_contract_artifact(&bytes)
            .expect_err("noncanonical POSEIDON6 encoding must fail shared artifact admission");
        assert_eq!(
            error.to_string(),
            "invalid contract artifact: noncanonical POSEIDON6 encoding at pc 0"
        );
    }
}
#[test]
fn verify_rejects_direct_control_flow_outside_instruction_boundaries() {
    use ivm::instruction::wide;
    let outside = ivm::encoding::wide::encode_branch(wide::control::BEQ, 0, 0, 1);
    let bytes = contract_artifact_with_code(
        1,
        vec![entrypoint("main", EntryPointKind::Kotoage, 0)],
        None,
        &[outside],
    );
    let err = ivm::verify_contract_artifact(&bytes)
        .expect_err("branch to the end of the stream must be rejected");
    assert!(err.to_string().contains("instruction boundary"));
    let before_start = ivm::encoding::wide::encode_offset24(wide::control::JMP, -1);
    let bytes = contract_artifact_with_code(
        1,
        vec![entrypoint("main", EntryPointKind::Kotoage, 0)],
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
            vec![entrypoint("main", EntryPointKind::Kotoage, 0)],
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
            vec![entrypoint("main", EntryPointKind::Kotoage, 0)],
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
    let disallowed_number = 0x54;
    let disallowed = ivm::encoding::wide::encode_sys(
        ivm::instruction::wide::system::SCALL,
        u8::try_from(disallowed_number).expect("unassigned syscall fits SCALL immediate"),
    );
    assert!(!ivm::syscalls::is_syscall_allowed(
        ivm::SyscallPolicy::AbiV1,
        disallowed_number
    ));
    let bytes = contract_artifact_with_code(
        1,
        vec![entrypoint("main", EntryPointKind::Kotoage, 0)],
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
        vec![entrypoint("main", EntryPointKind::Kotoage, 0)],
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
fn prepared_contract_derives_transitive_private_input_requirement_from_bytecode() {
    use ivm::instruction::wide;
    let private_input = ivm::encoding::wide::encode_sys(
        wide::system::SCALL,
        ivm::syscalls::SYSCALL_GET_PRIVATE_INPUT as u8,
    );
    let bytes = contract_artifact_with_mode_and_code(
        1,
        ivm::ivm_mode::ZK,
        ivm::CONTRACT_FEATURE_BIT_ZK,
        vec![
            entrypoint("private_commitment", EntryPointKind::Kotoage, 0),
            entrypoint("plain", EntryPointKind::Kotoage, 16),
        ],
        None,
        &[
            ivm::encoding::wide::encode_offset24(wide::control::JALS, 2),
            ivm::encoding::wide::encode_halt(),
            private_input,
            ivm::encoding::wide::encode_rr(wide::control::JALR, 0, 1, 0),
            ivm::encoding::wide::encode_halt(),
        ],
    );
    let prepared = ivm::prepare_contract(std::sync::Arc::from(bytes.into_boxed_slice()))
        .expect("valid ZK contract prepares");
    assert_eq!(
        prepared.entrypoint_requires_private_inputs("private_commitment"),
        Some(true),
        "private input hidden in a helper must be derived transitively"
    );
    assert_eq!(
        prepared.entrypoint_requires_private_inputs("plain"),
        Some(false)
    );
    assert_eq!(prepared.entrypoint_requires_private_inputs("missing"), None);
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
        let mut authorized = entrypoint("mutate", EntryPointKind::Kotoage, 0);
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
    let mut mutate = entrypoint("mutate", EntryPointKind::Kotoage, 8);
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
        vec![entrypoint("main", EntryPointKind::Kotoage, 0)],
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
fn verifier_rejects_self_recursive_direct_calls_before_execution() {
    use ivm::instruction::wide;
    let bytes = contract_artifact_with_code(
        1,
        vec![entrypoint("main", EntryPointKind::Kotoage, 0)],
        None,
        &[
            ivm::encoding::wide::encode_offset24(wide::control::JALS, 0),
            ivm::encoding::wide::encode_halt(),
        ],
    );
    let error = ivm::verify_contract_artifact(&bytes)
        .expect_err("self-recursive bytecode must fail artifact admission");
    assert!(
        error.to_string().contains("recursive direct-call cycle"),
        "unexpected recursion error: {error}"
    );
}
#[test]
fn verifier_rejects_mutual_and_unreachable_direct_call_cycles() {
    use ivm::instruction::wide;
    let return_from_helper = ivm::encoding::wide::encode_rr(wide::control::JALR, 0, 1, 0);
    let mutual = contract_artifact_with_code(
        1,
        vec![entrypoint("main", EntryPointKind::Kotoage, 0)],
        None,
        &[
            ivm::encoding::wide::encode_offset24(wide::control::JALS, 2),
            ivm::encoding::wide::encode_halt(),
            ivm::encoding::wide::encode_offset24(wide::control::JALS, 2),
            return_from_helper,
            ivm::encoding::wide::encode_offset24(wide::control::JALS, -2),
            return_from_helper,
        ],
    );
    let error = ivm::verify_contract_artifact(&mutual)
        .expect_err("mutually recursive helpers must fail artifact admission");
    assert!(error.to_string().contains("recursive direct-call cycle"));
    let unreachable = contract_artifact_with_code(
        1,
        vec![entrypoint("main", EntryPointKind::Kotoage, 0)],
        None,
        &[
            ivm::encoding::wide::encode_halt(),
            ivm::encoding::wide::encode_offset24(wide::control::JALS, 0),
            ivm::encoding::wide::encode_halt(),
        ],
    );
    let error = ivm::verify_contract_artifact(&unreachable)
        .expect_err("an unreachable recursive helper must still fail artifact admission");
    assert!(error.to_string().contains("recursive direct-call cycle"));
}
#[test]
fn verifier_does_not_confuse_an_ordinary_branch_loop_with_recursion() {
    use ivm::instruction::wide;
    let bytes = contract_artifact_with_code(
        1,
        vec![entrypoint("main", EntryPointKind::Kotoage, 0)],
        None,
        &[
            ivm::encoding::wide::encode_branch(wide::control::BNE, 2, 0, 0),
            ivm::encoding::wide::encode_halt(),
        ],
    );
    ivm::verify_contract_artifact(&bytes)
        .expect("an ordinary control-flow loop is not a recursive function call");
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
            vec![entrypoint("main", EntryPointKind::Kotoage, 0)],
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
        let mut forged = entrypoint("mutate", EntryPointKind::Kotoage, 0);
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
            entrypoint("main", EntryPointKind::Kotoage, 0),
            entrypoint("main", EntryPointKind::Kotoage, 0),
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
            entrypoint("first", EntryPointKind::Kotoage, 0),
            entrypoint("second", EntryPointKind::View, 0),
        ],
    );
    let err = ivm::verify_contract_artifact(&bytes).expect_err("entrypoint PC alias must fail");
    assert!(err.to_string().contains("reuses entry_pc"));
    let mut public = entrypoint("main", EntryPointKind::Kotoage, 0);
    public.permission = None;
    let bytes = contract_artifact(1, vec![public]);
    let err = ivm::verify_contract_artifact(&bytes)
        .expect_err("public entrypoint without authorization must fail");
    assert!(err.to_string().contains("missing caller authorization"));
}
#[test]
fn verify_rejects_noncanonical_or_reserved_entrypoint_names() {
    for name in [
        "1run",
        "run-now",
        "run!",
        "mаin",  // Cyrillic `а`.
        "ｍain", // Full-width `ｍ`.
        "言挙げ",
        "始まり_",
        "fn",
        "account_id",
        "Amount",
        "__kotodama_link_private",
    ] {
        let artifact = contract_artifact(1, vec![entrypoint(name, EntryPointKind::Kotoage, 0)]);
        let error = ivm::verify_contract_artifact(&artifact)
            .expect_err("noncanonical or reserved entrypoint name must fail admission");
        assert!(
            error
                .to_string()
                .contains("canonical Kotodama V1 identifier"),
            "unexpected error for `{name}`: {error}"
        );
    }
    for name in ["entry", "init", "upgrade", "_run2"] {
        ivm::verify_contract_artifact(&contract_artifact(
            1,
            vec![entrypoint(name, EntryPointKind::Kotoage, 0)],
        ))
        .unwrap_or_else(|error| panic!("valid ordinary identifier `{name}` was rejected: {error}"));
    }
    for name in kotodama_lang::semantic::V1_RETIRED_NUMERIC_TYPE_NAMES {
        if !iroha_data_model::smart_contract::entrypoint::is_canonical_kotodama_identifier(name) {
            continue;
        }
        let artifact = contract_artifact(1, vec![entrypoint(name, EntryPointKind::Kotoage, 0)]);
        ivm::verify_contract_artifact(&artifact).unwrap_or_else(|error| {
            panic!("retired numeric entrypoint name `{name}` was rejected: {error}")
        });
    }
}
#[test]
fn verify_rejects_noncanonical_or_reserved_seiyaku_names() {
    for name in [
        "",
        "1Ledger",
        "Ledger-name",
        "Lеdger",  // Cyrillic `е`.
        "Ｌedger", // Full-width `Ｌ`.
        "seiyaku",
        "match",
        "__kotodama_link_private",
    ] {
        let artifact = contract_artifact_with_seiyaku_name(name);
        let error = ivm::verify_contract_artifact(&artifact)
            .expect_err("noncanonical or reserved seiyaku name must fail admission");
        assert!(
            error
                .to_string()
                .contains("canonical Kotodama V1 identifier"),
            "unexpected error for `{name}`: {error}"
        );
    }
    for name in kotodama_lang::semantic::V1_RETIRED_NUMERIC_TYPE_NAMES {
        let artifact = contract_artifact_with_seiyaku_name(name);
        let error = ivm::verify_contract_artifact(&artifact)
            .expect_err("every retired numeric type name must remain reserved for source units");
        assert!(
            error
                .to_string()
                .contains("canonical Kotodama V1 identifier"),
            "unexpected error for retired seiyaku name `{name}`: {error}"
        );
    }
    let artifact = contract_artifact_with_seiyaku_name("_Ledger2");
    ivm::verify_contract_artifact(&artifact)
        .expect("valid ASCII seiyaku identifier must pass admission");
}
#[test]
fn verify_rejects_noncanonical_or_reserved_state_names() {
    for name in [
        "1counter",
        "counter-name",
        "cоunter",
        "状態",
        "state",
        "Option",
        "Amount",
    ] {
        let artifact = contract_artifact_with_states(vec![ivm::EmbeddedStateDescriptor {
            name: name.to_owned(),
            ty: ivm::EmbeddedStateType::Int,
        }]);
        let error = ivm::verify_contract_artifact(&artifact)
            .expect_err("noncanonical or reserved state name must fail admission");
        assert!(
            error
                .to_string()
                .contains("canonical Kotodama V1 identifier"),
            "unexpected error for `{name}`: {error}"
        );
    }
    ivm::verify_contract_artifact(&contract_artifact_with_states(vec![
        ivm::EmbeddedStateDescriptor {
            name: "_counter2".to_owned(),
            ty: ivm::EmbeddedStateType::Int,
        },
    ]))
    .expect("valid ASCII state identifier must pass admission");
    for ty in [
        ivm::EmbeddedStateType::Struct {
            name: "Rеcord".to_owned(), // Cyrillic `е`.
            fields: vec![],
        },
        ivm::EmbeddedStateType::Struct {
            name: "Record".to_owned(),
            fields: vec![ivm::EmbeddedStateFieldDescriptor {
                name: "field-name".to_owned(),
                ty: ivm::EmbeddedStateType::Int,
            }],
        },
        ivm::EmbeddedStateType::Struct {
            name: "Amount".to_owned(),
            fields: vec![],
        },
        ivm::EmbeddedStateType::Struct {
            name: "Record".to_owned(),
            fields: vec![ivm::EmbeddedStateFieldDescriptor {
                name: "Amount".to_owned(),
                ty: ivm::EmbeddedStateType::Int,
            }],
        },
    ] {
        let artifact = contract_artifact_with_states(vec![ivm::EmbeddedStateDescriptor {
            name: "record".to_owned(),
            ty,
        }]);
        let error = ivm::verify_contract_artifact(&artifact)
            .expect_err("noncanonical embedded struct identifier must fail admission");
        assert!(
            error.to_string().contains("canonical") || error.to_string().contains("noncanonical"),
            "unexpected embedded struct error: {error}"
        );
    }
}
#[test]
fn verify_rejects_source_controlled_lifecycle_authorization() {
    for (name, kind) in [
        ("hajimari", EntryPointKind::Hajimari),
        ("始まり", EntryPointKind::Hajimari),
        ("kaizen", EntryPointKind::Kaizen),
        ("改善", EntryPointKind::Kaizen),
    ] {
        let mut lifecycle = entrypoint(name, kind, 0);
        lifecycle.permission = Some("SourceCannotControlLifecycle".to_owned());
        let artifact = contract_artifact(1, vec![lifecycle]);
        let err = ivm::verify_contract_artifact(&artifact)
            .expect_err("lifecycle authorization must be runtime-defined");
        assert!(
            err.to_string()
                .contains("must use runtime-defined authorization"),
            "unexpected error for branded selector `{name}`: {err}"
        );
    }
}
#[test]
fn verify_requires_reserved_lifecycle_names_to_match_their_kinds() {
    for (name, kind, expected) in [
        (
            "renamed_hajimari",
            EntryPointKind::Hajimari,
            "must use the reserved `hajimari` or `始まり` selector",
        ),
        (
            "renamed_kaizen",
            EntryPointKind::Kaizen,
            "must use the reserved `kaizen` or `改善` selector",
        ),
        (
            "hajimari",
            EntryPointKind::Kotoage,
            "has the wrong entrypoint kind",
        ),
        (
            "改善",
            EntryPointKind::View,
            "has the wrong entrypoint kind",
        ),
    ] {
        let artifact = contract_artifact(1, vec![entrypoint(name, kind, 0)]);
        let error = ivm::verify_contract_artifact(&artifact)
            .expect_err("reserved lifecycle selector and kind must agree");
        assert!(
            error.to_string().contains(expected),
            "unexpected error for `{name}`: {error}"
        );
    }
    for (name, kind) in [
        ("hajimari", EntryPointKind::Hajimari),
        ("始まり", EntryPointKind::Hajimari),
        ("kaizen", EntryPointKind::Kaizen),
        ("改善", EntryPointKind::Kaizen),
    ] {
        ivm::verify_contract_artifact(&contract_artifact(1, vec![entrypoint(name, kind, 0)]))
            .unwrap_or_else(|error| {
                panic!("valid branded selector `{name}` was rejected: {error}")
            });
    }
}
#[test]
fn verify_rejects_inconsistent_access_completeness() {
    let mut complete = entrypoint("main", EntryPointKind::Kotoage, 0);
    complete.access_hints_skipped = vec!["dynamic path".to_owned()];
    let err = ivm::verify_contract_artifact(&contract_artifact(1, vec![complete]))
        .expect_err("complete hints with skipped reasons must fail");
    assert!(err.to_string().contains("marks access hints complete"));
    let mut incomplete = entrypoint("main", EntryPointKind::Kotoage, 0);
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
            vec![entrypoint("main", EntryPointKind::Kotoage, invalid_pc)],
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
    let mut main = entrypoint("main", EntryPointKind::Kotoage, 0);
    main.triggers.push(time_trigger("wake", None, "missing"));
    let bytes = contract_artifact(1, vec![main]);
    let err = ivm::verify_contract_artifact(&bytes)
        .expect_err("invalid trigger callback target must fail");
    assert!(err.to_string().contains("callback target `missing`"));
}
#[test]
fn verify_rejects_forbidden_trigger_identifier_positions() {
    for (trigger, expected) in [
        (
            time_trigger("Amount", None, "main"),
            "trigger ID `Amount` must be a canonical Kotodama V1 declaration identifier",
        ),
        (
            time_trigger("wake", Some("Amount"), "main"),
            "callback namespace `Amount` must be a canonical Kotodama V1 seiyaku identifier",
        ),
    ] {
        let mut main = entrypoint("main", EntryPointKind::Kotoage, 0);
        main.triggers.push(trigger);
        let error = ivm::verify_contract_artifact(&contract_artifact(1, vec![main]))
            .expect_err("exact `Amount` must not be accepted in trigger source positions");
        assert!(
            error.to_string().contains(expected),
            "unexpected admission error: {error}"
        );
    }
}
#[test]
fn verify_rejects_raw_control_flow_into_a_distinct_entrypoint() {
    use ivm::instruction::wide;
    let transfers = [
        (
            "conditional branch",
            ivm::encoding::wide::encode_branch(wide::control::BEQ, 0, 0, 2),
            "ordinary control flow at pc 8 is shared by function roots",
        ),
        (
            "tail jump",
            ivm::encoding::wide::encode_jump(wide::control::JAL, 0, 2),
            "ordinary control flow at pc 8 is shared by function roots",
        ),
        (
            "direct call",
            ivm::encoding::wide::encode_jump(wide::control::JAL, 1, 2),
            "reaches distinct entrypoint",
        ),
        (
            "long jump",
            ivm::encoding::wide::encode_offset24(wide::control::JMP, 2),
            "ordinary control flow at pc 8 is shared by function roots",
        ),
        (
            "long direct call",
            ivm::encoding::wide::encode_offset24(wide::control::JALS, 2),
            "reaches distinct entrypoint",
        ),
    ];
    let targets = [
        ("admin", EntryPointKind::Kotoage),
        ("inspect", EntryPointKind::View),
        ("hajimari", EntryPointKind::Hajimari),
        ("kaizen", EntryPointKind::Kaizen),
    ];
    for (encoding, transfer, expected_error) in transfers {
        for (target_name, target_kind) in targets {
            let bytes = contract_artifact_with_code(
                1,
                vec![
                    entrypoint("run", EntryPointKind::Kotoage, 0),
                    entrypoint(target_name, target_kind, 8),
                ],
                None,
                &[
                    transfer,
                    ivm::encoding::wide::encode_halt(),
                    ivm::encoding::wide::encode_halt(),
                ],
            );
            let error = ivm::verify_contract_artifact(&bytes)
                .expect_err("raw cross-entrypoint control flow must fail admission");
            let error = error.to_string();
            assert!(
                error.contains(expected_error),
                "{encoding} into {target_name} returned the wrong error: {error}"
            );
            if expected_error == "reaches distinct entrypoint" {
                assert!(
                    error.contains(&format!("`{target_name}`")),
                    "{encoding} named the wrong target entrypoint: {error}"
                );
            }
        }
    }
}
#[test]
fn verify_rejects_duplicate_trigger_ids() {
    let mut main = entrypoint("main", EntryPointKind::Kotoage, 0);
    main.triggers.push(time_trigger("wake", None, "main"));
    main.triggers.push(time_trigger("wake", None, "main"));
    let err = ivm::verify_contract_artifact(&contract_artifact(1, vec![main]))
        .expect_err("duplicate trigger IDs must fail closed during artifact admission");
    assert!(err.to_string().contains("duplicate trigger `wake`"));
}
#[test]
fn verify_rejects_non_kotoage_local_trigger_callbacks() {
    for (target, kind) in [
        ("inspect", EntryPointKind::View),
        ("hajimari", EntryPointKind::Hajimari),
        ("kaizen", EntryPointKind::Kaizen),
    ] {
        let mut main = entrypoint("main", EntryPointKind::Kotoage, 0);
        main.triggers.push(time_trigger("wake", None, target));
        let target_entrypoint = entrypoint(target, kind, 4);
        let bytes = contract_artifact_with_code(
            1,
            vec![main, target_entrypoint],
            None,
            &[
                ivm::encoding::wide::encode_halt(),
                ivm::encoding::wide::encode_halt(),
            ],
        );
        let err = ivm::verify_contract_artifact(&bytes).unwrap_err();
        assert!(
            err.to_string()
                .contains("must be a `kotoage`/`言挙げ` entrypoint"),
            "unexpected admission error for {target}: {err}"
        );
    }
}
#[test]
fn verify_accepts_namespaced_trigger_callback_target() {
    let mut main = entrypoint("main", EntryPointKind::Kotoage, 0);
    main.triggers
        .push(time_trigger("amount", Some("callee"), "run"));
    let bytes = contract_artifact(1, vec![main]);
    ivm::verify_contract_artifact(&bytes)
        .expect("lowercase business identifiers remain valid trigger IDs");
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
        vec![entrypoint("main", EntryPointKind::Kotoage, 0)],
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
        vec![entrypoint("main", EntryPointKind::Kotoage, 0)],
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
            key_type: "int".to_owned(),
            bound_kind: "take".to_owned(),
            max_keys: 64,
        }],
        dynamic_writes: Vec::new(),
    };
    let bytes = contract_artifact_with_access_hints(
        1,
        vec![entrypoint("main", EntryPointKind::Kotoage, 0)],
        Some(hints),
    );
    let err = ivm::verify_contract_artifact(&bytes).expect_err("wildcard dynamic hint must fail");
    assert!(err.to_string().contains(
        "base_key must be `state:` followed by one canonical state declaration identifier"
    ));
    let hints = AccessSetHints {
        read_keys: Vec::new(),
        write_keys: Vec::new(),
        dynamic_reads: Vec::new(),
        dynamic_writes: vec![DynamicAccessHint {
            base_key: "state:Orders".to_owned(),
            key_type: "int".to_owned(),
            bound_kind: "range".to_owned(),
            max_keys: 0,
        }],
    };
    let bytes = contract_artifact_with_access_hints(
        1,
        vec![entrypoint("main", EntryPointKind::Kotoage, 0)],
        Some(hints),
    );
    let err = ivm::verify_contract_artifact(&bytes).expect_err("zero dynamic hint must fail");
    assert!(err.to_string().contains("max_keys must be in 1..=64"));
}
#[test]
fn verify_dynamic_access_hints_resolve_the_exact_declared_state_map() {
    let hint = DynamicAccessHint {
        base_key: "state:amount".to_owned(),
        key_type: "quantity".to_owned(),
        bound_kind: "range".to_owned(),
        max_keys: 64,
    };
    let hints = AccessSetHints {
        read_keys: Vec::new(),
        write_keys: Vec::new(),
        dynamic_reads: vec![hint.clone()],
        dynamic_writes: Vec::new(),
    };
    let state = ivm::EmbeddedStateDescriptor {
        name: "amount".to_owned(),
        ty: ivm::EmbeddedStateType::StateMap {
            key: Box::new(ivm::EmbeddedStateType::Quantity),
            value: Box::new(ivm::EmbeddedStateType::Bool),
        },
    };
    let artifact =
        contract_artifact_with_access_hints_and_states(Some(hints.clone()), vec![state.clone()]);
    ivm::verify_contract_artifact(&artifact)
        .expect("exact dynamic hint must resolve to its declared StateMap");
    let mut mismatched_hints = hints.clone();
    mismatched_hints.dynamic_reads[0].key_type = "int".to_owned();
    let artifact =
        contract_artifact_with_access_hints_and_states(Some(mismatched_hints), vec![state]);
    let error =
        ivm::verify_contract_artifact(&artifact).expect_err("mismatched key type must fail");
    assert!(
        error
            .to_string()
            .contains("declares key_type `int` but its StateMap key type is `quantity`")
    );
    let artifact = contract_artifact_with_access_hints_and_states(
        Some(hints),
        vec![ivm::EmbeddedStateDescriptor {
            name: "amount".to_owned(),
            ty: ivm::EmbeddedStateType::Quantity,
        }],
    );
    let error = ivm::verify_contract_artifact(&artifact)
        .expect_err("a dynamic hint must not target scalar state");
    assert!(
        error
            .to_string()
            .contains("must reference a declared top-level StateMap")
    );
    let artifact = contract_artifact_with_access_hints_and_states(
        Some(AccessSetHints {
            read_keys: Vec::new(),
            write_keys: Vec::new(),
            dynamic_reads: vec![hint],
            dynamic_writes: Vec::new(),
        }),
        Vec::new(),
    );
    let error =
        ivm::verify_contract_artifact(&artifact).expect_err("unknown dynamic base must fail");
    assert!(
        error
            .to_string()
            .contains("must reference a declared top-level StateMap")
    );
}
#[test]
fn verify_rejects_unsupported_abi_version() {
    let bytes = contract_artifact(2, vec![entrypoint("main", EntryPointKind::Kotoage, 0)]);
    let err = ivm::verify_contract_artifact(&bytes).expect_err("abi version mismatch must fail");
    assert!(
        err.to_string()
            .contains("unsupported IVM program ABI version 2")
    );
}
