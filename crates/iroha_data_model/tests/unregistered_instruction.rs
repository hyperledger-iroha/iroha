//! Tests unregistered instruction deserialization.

use iroha_data_model::{
    isi::{InstructionBox, set_instruction_registry},
    prelude::Log,
};

#[test]
fn unregistered_instruction_returns_error_with_name() {
    set_instruction_registry(iroha_data_model::instruction_registry![Log]);
    let name = "dummy".to_string();
    let bytes = norito::core::to_bytes(&(name.clone(), Vec::<u8>::new())).expect("serialize");
    let archived_tuple = norito::core::from_bytes::<(String, Vec<u8>)>(&bytes).expect("from_bytes");
    let archived = archived_tuple.cast::<InstructionBox>();
    let _err = norito::core::NoritoDeserialize::try_deserialize(archived)
        .expect_err("deserializing unregistered instruction must fail");
}
