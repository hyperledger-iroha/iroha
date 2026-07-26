//! Regression test: `InstructionRegistry` decodes framed bytes robustly.
use iroha_data_model::{
    Level,
    isi::{Instruction, InstructionBox, InstructionRegistry},
    prelude::Log,
};

#[test]
fn registry_decodes_framed_bytes_roundtrip() {
    let log = Log::new(Level::INFO, "hello".to_string());
    let id = Instruction::id(&log);
    let (payload, flags) = norito::codec::encode_with_header_flags(&log);
    let framed = norito::core::frame_bare_with_header_flags::<Log>(&payload, flags)
        .expect("frame instruction payload");

    let registry = InstructionRegistry::new().register_with_id::<Log>(Log::WIRE_ID);
    let decoded: InstructionBox = registry
        .decode(id, &framed)
        .expect("registered")
        .expect("decode");

    assert_eq!(Instruction::id(&*decoded), id);
    assert_eq!(Instruction::dyn_encode(&*decoded), payload);
}
