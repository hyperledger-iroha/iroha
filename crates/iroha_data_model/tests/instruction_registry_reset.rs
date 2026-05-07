//! Test that the global instruction registry can be overridden after it has
//! been initialized lazily.
use iroha_data_model::{instruction_registry, isi::set_instruction_registry, prelude::*};

#[test]
fn set_instruction_registry_overrides_existing() {
    // Start with registry containing only `Log`
    set_instruction_registry(instruction_registry![Log]);
    let log: InstructionBox = Log::new(Level::INFO, "hello".to_owned()).into();
    let bytes = norito::to_bytes(&log).expect("serialize");
    let decoded: InstructionBox = norito::decode_from_bytes(&bytes).expect("deserialize");
    // Type should match when decoded via the configured registry
    assert_eq!(log.id(), decoded.id());

    // Replace registry with one that doesn't include `Log`
    set_instruction_registry(instruction_registry![SetParameter]);
    let result = norito::decode_from_bytes::<InstructionBox>(&bytes);
    assert!(result.is_err(), "registry should have been replaced");

    // Restore the default registry so other tests aren't affected
    set_instruction_registry(iroha_data_model::instruction_registry::default());
}
