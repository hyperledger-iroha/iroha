//! Executor formatting preserves redaction and ordinary Rust debug layouts.

use super::*;

#[test]
fn loaded_executor_debug_redacts_locked_runtime_and_ignores_outer_flags() {
    use core::fmt::Write as _;

    struct Refuse;
    impl core::fmt::Write for Refuse {
        fn write_str(&mut self, _: &str) -> core::fmt::Result {
            Err(core::fmt::Error)
        }
    }

    let loaded = LoadedExecutor::load(data_model_executor::Executor::new(
        iroha_data_model::transaction::IvmBytecode::from_compiled(generate_verdict_program(
            &Ok(()),
        )),
    ))
    .expect("valid executor fixture");
    let _guard = loaded.runtime_pool.lock().expect("runtime pool");
    let expected = "LoadedExecutor { runtime: <IVM> }";
    assert_eq!(format!("{loaded:?}"), expected);
    assert_eq!(format!("{loaded:#?}"), expected);
    assert_eq!(format!("{loaded:*^60.4?}"), expected);
    assert_eq!(format!("{loaded:+#060?}"), expected);
    assert!(write!(Refuse, "{loaded:?}").is_err());
    let executor = Executor::UserProvided(loaded.clone());
    assert_eq!(
        format!("{executor:?}"),
        "UserProvided(LoadedExecutor { runtime: <IVM> })"
    );
    assert_eq!(
        format!("{executor:+#080?}"),
        "UserProvided(\n    LoadedExecutor { runtime: <IVM> },\n)"
    );
    assert!(write!(Refuse, "{executor:?}").is_err());
}

#[test]
fn ordinary_executor_debug_keeps_unit_tuple_and_named_field_layouts() {
    assert_eq!(format!("{:?}", Executor::Initial), "Initial");
    assert_eq!(
        format!("{:*^20.2?}", InstructionExecutionProfile::Bench),
        "Bench"
    );
    assert_eq!(
        format!(
            "{:?}",
            NexusFeeAdmissionError::ConfigInvalid("line\n\"quoted\"".into())
        ),
        "ConfigInvalid(\"line\\n\\\"quoted\\\"\")"
    );
    let key = ExecutorRuntimeKey {
        stack_limit: 17,
        heap_limit: 4096,
    };
    assert_eq!(
        format!("{key:?}"),
        "ExecutorRuntimeKey { stack_limit: 17, heap_limit: 4096 }"
    );
    assert_eq!(
        format!("{key:#?}"),
        "ExecutorRuntimeKey {\n    stack_limit: 17,\n    heap_limit: 4096,\n}"
    );
}

#[test]
fn executor_tuple_debug_resets_nested_flags_only_for_alternate_output() {
    let error = ExecutorMigrationError::Rejected(VMError::InvalidOpcode(17));
    let pretty = "Rejected(\n    InvalidOpcode(\n        17,\n    ),\n)";
    assert_eq!(format!("{error:#?}"), pretty);
    assert_eq!(format!("{error:+#080?}"), pretty);
    assert_eq!(format!("{error:#x?}"), pretty);
    assert_eq!(format!("{error:+#080X?}"), pretty);
    assert_eq!(format!("{error:x?}"), "Rejected(InvalidOpcode(11))");
    assert_eq!(
        format!("{error:*^80.4?}"),
        format!("Rejected({:*^80.4?})", VMError::InvalidOpcode(17))
    );
    assert_eq!(
        format!("{:?}", ExecutorMigrationError::LiveCustody),
        "LiveCustody"
    );
    assert_eq!(
        format!("{:?}", ExecutorMigrationError::LocalRefusalRetained),
        "LocalRefusalRetained"
    );

    let account = iroha_test_samples::ALICE_ID.clone();
    let asset = AssetDefinitionId::derive_from_components(
        DomainId::try_new("formatting", "universal").expect("domain"),
        "coin".parse().expect("asset name"),
    );
    let incarnation = iroha_data_model::nexus::AxtAssetIncarnationV1::try_from_bytes(
        *Hash::new(b"formatting incarnation").as_ref(),
    )
    .expect("valid incarnation");
    for access in [
        NativeQueryAccess::Registered,
        NativeQueryAccess::Account(account.clone()),
        NativeQueryAccess::EnrollmentAccount(account.clone()),
        NativeQueryAccess::AssetUidOwner(asset.clone()),
        NativeQueryAccess::AssetEscrowOwner(asset.clone(), incarnation),
        NativeQueryAccess::AssetFiOwner(asset, account),
        NativeQueryAccess::AllLedger,
    ] {
        assert_eq!(format!("{access:+#080?}"), format!("{access:#?}"));
    }
    let reason = NexusFeeAdmissionError::ConfigInvalid("line\n\"quoted\"".into());
    assert_eq!(
        format!("{reason:+#080?}"),
        "ConfigInvalid(\n    \"line\\n\\\"quoted\\\"\",\n)"
    );
    let rejected = NexusFeeAdmissionError::Rejected {
        code: FeeRejectionCode::InvalidFeeIntent,
        reason: "bad intent".into(),
    };
    assert_eq!(
        format!("{rejected:?}"),
        "Rejected { code: InvalidFeeIntent, reason: \"bad intent\" }"
    );
}
