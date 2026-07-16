//! Exact-current positive and forbidden-syscall admission vectors.

use base64::{Engine as _, engine::general_purpose::STANDARD};
use ivm_artifact_admission::{verify_contract_artifact, verify_contract_artifact_json};

const CURRENT_FIXTURE: &str =
    include_str!("../../../javascript/iroha_js/test/fixtures/current_rust_contract_artifact.json");
const CODE_OFFSET: usize = 897;

fn current_artifact() -> Vec<u8> {
    let fixture = norito::json::from_str::<norito::json::Value>(CURRENT_FIXTURE)
        .expect("parse exact-current fixture metadata");
    let encoded = fixture
        .as_object()
        .and_then(|object| object.get("artifact_base64"))
        .and_then(norito::json::Value::as_str)
        .expect("fixture carries artifact_base64");
    STANDARD.decode(encoded).expect("decode fixture artifact")
}

#[test]
fn exact_current_compiler_artifact_is_admitted() {
    let artifact = current_artifact();
    let verified = verify_contract_artifact(&artifact)
        .expect("exact-current compiler artifact must satisfy shared admission");
    assert_eq!(verified.header_len, 49);
    assert_eq!(verified.code_offset, CODE_OFFSET);
    assert_eq!(verified.contract_interface.entrypoints.len(), 2);
    assert_eq!(
        hex::encode(verified.code_hash.as_ref()),
        "52c7f6b69c678e3d6b1f3bad87e7a5b4b849284d18f406e2193e24dfd4645345"
    );
    assert_eq!(
        hex::encode(verified.abi_hash.as_ref()),
        "98679112b5a065a4dc962c5cfe128d0c545ed948f915ea8804767d369e4ef64f"
    );
    let json = verify_contract_artifact_json(&artifact);
    assert!(json.starts_with("{\"ok\":true,"), "{json}");
}

#[test]
fn host_private_system_syscall_is_rejected() {
    let mut mutated = current_artifact();
    // Canonical wide SYSTEM encoding for host-private syscall 0x00fe0000.
    mutated[CODE_OFFSET..CODE_OFFSET + 4].copy_from_slice(&[0x00, 0x00, 0xfe, 0x62]);
    let error = verify_contract_artifact(&mutated)
        .expect_err("host-private SYSTEM syscall must fail shared admission");
    assert!(error.to_string().contains("disallowed syscall 0xfe0000"));
    let json = verify_contract_artifact_json(&mutated);
    assert!(json.starts_with("{\"ok\":false,"), "{json}");
}
