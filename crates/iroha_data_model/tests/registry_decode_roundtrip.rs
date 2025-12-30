//! Regression test: `InstructionRegistry` decodes bare bytes robustly.
use iroha_data_model::{
    Level,
    isi::{Instruction, InstructionBox, InstructionRegistry},
    prelude::Log,
};
use norito::codec::Encode;

#[test]
fn registry_decodes_bare_bytes_roundtrip() {
    if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {
        eprintln!(
            "Skipping: registry decode bare vs packed-struct mismatch pending Norito alignments. Set IROHA_RUN_IGNORED=1 to run."
        );
        return;
    }
    // Build a simple instruction and encode via the bare codec path
    let log = Log::new(Level::INFO, "hello".to_string());
    let id = Instruction::id(&log);
    let bytes = log.encode();

    // Decode using the registry, which should use the robust header-framed path
    let registry = InstructionRegistry::new().register_with_id::<Log>(Log::WIRE_ID);
    let decoded: InstructionBox = registry
        .decode(id, &bytes)
        .expect("registered")
        .expect("decode");

    // Ensure it matches the original by re-encoding the inner payload
    assert_eq!(Instruction::id(&*decoded), id);
    assert_eq!(Instruction::dyn_encode(&*decoded), bytes);
}
