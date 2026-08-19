//! Tests unregistered instruction deserialization.
use iroha_data_model::{
    isi::{InstructionBox, set_instruction_registry},
    prelude::Log,
};

const UNREGISTERED_INSTRUCTION_CHILD: &str = "IROHA_DATA_MODEL_UNREGISTERED_INSTRUCTION_CHILD";

#[test]
fn unregistered_instruction_returns_error_with_name() {
    if std::env::var_os(UNREGISTERED_INSTRUCTION_CHILD).is_none() {
        // The registry is process-global. Exercise its deliberately incomplete
        // state in a child so parallel grouped tests retain the canonical
        // instruction inventory for both passes of every encoding.
        let status = std::process::Command::new(
            std::env::current_exe().expect("resolve grouped integration-test executable"),
        )
        .arg("unregistered_instruction_returns_error_with_name")
        .env(UNREGISTERED_INSTRUCTION_CHILD, "1")
        .status()
        .expect("run isolated unregistered-instruction test");
        assert!(status.success(), "isolated registry test failed");
        return;
    }

    set_instruction_registry(iroha_data_model::instruction_registry![Log]);
    let name = "dummy".to_string();
    let bytes = norito::core::to_bytes(&(name.clone(), Vec::<u8>::new())).expect("serialize");
    let archived_tuple = norito::core::from_bytes::<(String, Vec<u8>)>(&bytes).expect("from_bytes");
    let archived = archived_tuple.cast::<InstructionBox>();
    let _err = norito::core::NoritoDeserialize::try_deserialize(archived)
        .expect_err("deserializing unregistered instruction must fail");
}
