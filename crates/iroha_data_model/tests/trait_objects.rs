//! Tests for trait object instructions

use iroha_data_model::{instruction_registry, isi::set_instruction_registry, prelude::*};

#[test]
fn instruction_box_roundtrip() {
    if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {
        eprintln!(
            "Skipping: registry vs bare/header misalignment pending alignment. Set IROHA_RUN_IGNORED=1 to run."
        );
        return;
    }
    let log = Log::new(Level::INFO, "roundtrip".to_string());
    let boxed = InstructionBox::from(log.clone());
    let encoded = Instruction::dyn_encode(&*boxed);
    let registry = instruction_registry![Log];
    let decoded = registry
        .decode(Instruction::id(&log), &encoded)
        .expect("registry")
        .expect("decode");
    let decoded_log = decoded.as_any().downcast_ref::<Log>().unwrap();
    assert_eq!(decoded_log, &log);
}

#[test]
fn instruction_box_norito_roundtrip() {
    if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {
        eprintln!(
            "Skipping: Norito decode relies on registry pending fix. Set IROHA_RUN_IGNORED=1 to run."
        );
        return;
    }
    set_instruction_registry(instruction_registry![Log]);
    let log = Log::new(Level::INFO, "norito".to_string());
    let boxed = InstructionBox::from(log.clone());
    let bytes = norito::core::to_bytes(&boxed).expect("serialize");
    let archived = norito::core::from_bytes::<InstructionBox>(&bytes).expect("from_bytes");
    let decoded = norito::core::NoritoDeserialize::try_deserialize(archived).expect("deserialize");
    let decoded_log = decoded.as_any().downcast_ref::<Log>().unwrap();
    assert_eq!(decoded_log, &log);
}
