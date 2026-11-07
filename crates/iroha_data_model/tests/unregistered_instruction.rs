//! Unknown instruction envelopes retain their wire bytes across an archive roundtrip.
use iroha_data_model::{
    isi::{InstructionBox, decode_instruction_from_pair, set_instruction_registry},
    prelude::Log,
};

const UNREGISTERED_INSTRUCTION_CHILD: &str = "IROHA_DATA_MODEL_UNREGISTERED_INSTRUCTION_CHILD";

#[test]
fn unregistered_instruction_preserves_bytes_and_rejects_typed_decode() {
    if std::env::var_os(UNREGISTERED_INSTRUCTION_CHILD).is_none() {
        // The registry is process-global. Exercise its deliberately incomplete
        // state in a child so parallel grouped tests retain the canonical
        // instruction inventory for both passes of every encoding.
        let test = "unregistered_instruction::unregistered_instruction_preserves_bytes_and_rejects_typed_decode";
        let output = std::process::Command::new(
            std::env::current_exe().expect("resolve grouped integration-test executable"),
        )
        .args(["--exact", test])
        .env(UNREGISTERED_INSTRUCTION_CHILD, "1")
        .output()
        .expect("run isolated unregistered-instruction test");
        assert!(
            output.status.success(),
            "isolated registry test failed:\n{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            stdout
                .lines()
                .any(|line| line == format!("test {test} ... ok")),
            "isolated exact test did not pass: {stdout}",
        );
        assert!(
            stdout.contains("test result: ok. 1 passed; 0 failed; 0 ignored;"),
            "isolated test selection changed: {stdout}",
        );
        return;
    }

    set_instruction_registry(iroha_data_model::instruction_registry_with_ids![Log]);
    let name = "dummy".to_string();
    let frame = vec![0xa5, 0x00, 0xff];
    let bytes = norito::core::to_bytes(&(name.clone(), frame.clone())).expect("serialize");
    let archived_tuple = norito::core::from_bytes::<(String, Vec<u8>)>(&bytes).expect("from_bytes");
    let archived = archived_tuple.cast::<InstructionBox>();
    let decoded: InstructionBox =
        norito::core::DeserializePayload::try_deserialize(archived).expect("opaque envelope");
    let opaque = decoded
        .as_opaque()
        .expect("wire ID has no registered constructor");
    assert_eq!(opaque.wire_id(), name);
    assert_eq!(opaque.frame(), frame);
    assert_eq!(norito::core::to_bytes(&decoded).expect("re-encode"), bytes);
    assert!(matches!(
        decode_instruction_from_pair(&name, &frame),
        Err(norito::Error::Message(message)) if message == "unknown instruction wire identifier"
    ));
}
