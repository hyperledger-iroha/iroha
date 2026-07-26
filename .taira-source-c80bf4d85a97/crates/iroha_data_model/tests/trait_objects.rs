//! Tests for trait object instructions

use iroha_data_model::{instruction_registry, isi::set_instruction_registry, prelude::*};

struct RegistryGuard;

impl Drop for RegistryGuard {
    fn drop(&mut self) {
        set_instruction_registry(iroha_data_model::instruction_registry::default());
    }
}

#[test]
fn instruction_box_roundtrip() {
    let log = Log::new(Level::INFO, "roundtrip".to_string());
    let (payload, flags) = norito::codec::encode_with_header_flags(&log);
    let framed = norito::core::frame_bare_with_header_flags::<Log>(&payload, flags)
        .expect("frame instruction payload");
    let registry = instruction_registry![Log];
    let decoded = registry
        .decode(Instruction::id(&log), &framed)
        .expect("registry")
        .expect("decode");
    let decoded_log = decoded.as_any().downcast_ref::<Log>().unwrap();
    assert_eq!(decoded_log, &log);
    assert_eq!(Instruction::dyn_encode(&*decoded), payload);
}

#[test]
fn instruction_box_norito_roundtrip() {
    let _guard = RegistryGuard;
    set_instruction_registry(instruction_registry![Log]);
    let log = Log::new(Level::INFO, "norito".to_string());
    let boxed = InstructionBox::from(log.clone());
    let bytes = norito::core::to_bytes(&boxed).expect("serialize");
    let archived = norito::core::from_bytes::<InstructionBox>(&bytes).expect("from_bytes");
    let decoded = norito::core::NoritoDeserialize::try_deserialize(archived).expect("deserialize");
    let decoded_log = decoded.as_any().downcast_ref::<Log>().unwrap();
    assert_eq!(decoded_log, &log);
}
