use iroha_data_model::{
    smart_contract::manifest::EntryPointKind,
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
        return_type: None,
        permission: None,
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
    let meta = ivm::ProgramMetadata {
        version_major: 1,
        version_minor: 1,
        mode: 0,
        vector_length: 0,
        max_cycles: 0,
        abi_version,
    };
    let interface = ivm::EmbeddedContractInterfaceV1 {
        compiler_fingerprint: "ivm-tests".to_owned(),
        features_bitmap: 0,
        access_set_hints: None,
        kotoba: Vec::new(),
        entrypoints,
    };
    let mut bytes = meta.encode();
    bytes.extend_from_slice(&interface.encode_section());
    bytes.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());
    bytes
}

#[test]
fn compiler_emits_self_describing_contract_artifact() {
    let src = r#"
        seiyaku Demo {
            state int counter;

            hajimari() {
                counter = 0;
            }

            kotoage fn run() permission(Admin) {
                info("ready");
            }
        }
    "#;
    let (bytes, manifest) = ivm::KotodamaCompiler::new()
        .compile_source_with_manifest(src)
        .expect("compile contract");
    let parsed = ivm::ProgramMetadata::parse(&bytes).expect("parse artifact");
    assert_eq!(parsed.metadata.version_minor, 1);
    assert!(
        parsed.contract_interface.is_some(),
        "compiler must emit CNTR"
    );

    let verified = ivm::verify_contract_artifact(&bytes).expect("verify artifact");
    assert_eq!(
        verified.manifest.signature_payload(),
        manifest.signature_payload(),
        "compiler manifest must match the embedded contract interface",
    );
}

#[test]
fn contract_artifact_with_cntr_and_literals_loads_and_executes() {
    let src = r#"
        fn main() -> int {
            info("alpha");
            return 7;
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
    let cntr_len = contract_interface.encode_section().len();
    assert!(
        parsed.code_offset > parsed.header_len + cntr_len,
        "string literals should emit a prefix section after CNTR",
    );

    let mut vm = ivm::IVM::new(u64::MAX);
    vm.load_program(&bytes).expect("load artifact");
    vm.run().expect("run artifact");
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
fn verify_rejects_invalid_entry_pc() {
    let bytes = contract_artifact(1, vec![entrypoint("main", EntryPointKind::Public, 4)]);
    let err = ivm::verify_contract_artifact(&bytes).expect_err("invalid entry_pc must fail");
    assert!(err.to_string().contains("invalid entry_pc"));
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
fn verify_rejects_unsupported_abi_version() {
    let bytes = contract_artifact(2, vec![entrypoint("main", EntryPointKind::Public, 0)]);
    let err = ivm::verify_contract_artifact(&bytes).expect_err("abi version mismatch must fail");
    assert!(err.to_string().contains("unsupported abi_version 2"));
}
