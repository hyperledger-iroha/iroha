//! Test that the global instruction registry can be overridden after it has
//! been initialized lazily.
use iroha_data_model::{
    instruction_registry_with_ids,
    isi::{decode_instruction_from_pair, framed_instruction_payload, set_instruction_registry},
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

    // Start with registry containing only `Log`
    set_instruction_registry(instruction_registry_with_ids![Log]);
    let log: InstructionBox = Log::new(Level::INFO, "hello".to_owned()).into();
    let bytes = norito::to_bytes(&log).expect("serialize");
    let decoded: InstructionBox = norito::decode_from_bytes(&bytes).expect("deserialize");
    // Type should match when decoded via the configured registry
    assert_eq!(log.id(), decoded.id());
    let (wire_id, frame) = framed_instruction_payload(&log).expect("registered Log frame");
    let wire_id = wire_id.to_owned();
    assert_eq!(
        decode_instruction_from_pair(&wire_id, &frame)
            .expect("registered typed decoder")
            .id(),
        log.id(),
    );
    // Replace registry with one that doesn't include `Log`
    set_instruction_registry(instruction_registry_with_ids![SetParameter]);
    assert!(
        decode_instruction_from_pair(&wire_id, &frame).is_err(),
        "the replaced registry must refuse the removed typed instruction",
    );
    // Envelope decoding keeps unknown instructions byte-for-byte; it does not
    // restore the removed typed decoder (live_runtime_upgrades.md section 5.2).
    let decoded: InstructionBox = norito::decode_from_bytes(&bytes).expect("opaque envelope");
    let opaque = decoded.as_opaque().expect("Log is no longer registered");
    assert_eq!(opaque.wire_id(), wire_id);
    assert_eq!(opaque.frame(), frame);
    assert_eq!(
        norito::to_bytes(&decoded).expect("re-encode envelope"),
        bytes
    );
    // Restore the default registry so other tests aren't affected
    set_instruction_registry(iroha_data_model::instruction_registry::default());
    let restored: InstructionBox = norito::decode_from_bytes(&bytes).expect("restored typed Log");
    assert_eq!(restored.id(), log.id());
    assert!(restored.as_opaque().is_none());
}
