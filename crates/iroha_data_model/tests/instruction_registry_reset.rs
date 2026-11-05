//! Test that the global instruction registry can be overridden after it has
//! been initialized lazily.
use iroha_data_model::{
    instruction_registry_with_ids,
    isi::{framed_instruction_payload, set_instruction_registry, wire_id_of},
    prelude::*,
};

const REGISTRY_RESET_CHILD: &str = "IROHA_DATA_MODEL_REGISTRY_RESET_CHILD";

#[test]
fn set_instruction_registry_overrides_existing() {
    if std::env::var_os(REGISTRY_RESET_CHILD).is_none() {
        // This integration test is linked into a grouped harness. Exercise the
        // process-global setter in a child so parallel codec tests never observe
        // either deliberately incomplete registry.
        let test = "instruction_registry_reset::set_instruction_registry_overrides_existing";
        let output = std::process::Command::new(
            std::env::current_exe().expect("resolve grouped integration-test executable"),
        )
        .args(["--exact", test])
        .env(REGISTRY_RESET_CHILD, "1")
        .output()
        .expect("run isolated instruction-registry reset test");
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

    set_instruction_registry(instruction_registry_with_ids![Log]);
    let log = Log::new(Level::INFO, "hello".to_owned());
    let boxed: InstructionBox = log.clone().into();
    let (wire_id, frame) = framed_instruction_payload(&boxed).expect("registered Log envelope");
    let bytes = norito::to_bytes(&boxed).expect("serialize");
    let decoded: InstructionBox = norito::decode_from_bytes(&bytes).expect("deserialize");
    assert_eq!(decoded.as_any().downcast_ref::<Log>(), Some(&log));

    set_instruction_registry(instruction_registry_with_ids![SetParameter]);
    assert!(wire_id_of::<Log>().is_none());
    assert!(wire_id_of::<SetParameter>().is_some());
    let decoded: InstructionBox = norito::decode_from_bytes(&bytes).expect("opaque envelope");
    let opaque = decoded.as_opaque().expect("Log left the registry");
    assert_eq!(opaque.wire_id(), wire_id);
    assert_eq!(opaque.frame(), frame);
    assert_eq!(norito::to_bytes(&decoded).expect("re-encode"), bytes);
    set_instruction_registry(iroha_data_model::instruction_registry::default());
}
