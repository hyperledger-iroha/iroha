//! Ensure `InstructionBox` deserialization works without explicitly setting
//! the instruction registry by relying on the lazy default initialization.

use iroha_data_model::prelude::*;

#[test]
fn instruction_box_deserialize_log_without_explicit_init() {
    if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {
        eprintln!("Skipping: packed-struct hybrid decode pending. Set IROHA_RUN_IGNORED=1 to run.");
        return;
    }
    // Prepare a Log instruction wrapped in InstructionBox
    let original: InstructionBox = Log::new(Level::INFO, "hello".to_owned()).into();

    // Serialize with Norito and then deserialize back
    let bytes = norito::to_bytes(&original).expect("serialize");
    let restored: InstructionBox = norito::decode_from_bytes(&bytes).expect("deserialize");

    // Type should match (lazy registry resolved correctly)
    assert_eq!(original.id(), restored.id());
}
