//! Exact-current positive and forbidden-syscall admission vectors.

use base64::{Engine as _, engine::general_purpose::STANDARD};
use ivm_artifact_admission::{verify_contract_artifact, verify_contract_artifact_json};

const CURRENT_FIXTURE: &str =
    include_str!("../../../javascript/iroha_js/test/fixtures/current_rust_contract_artifact.json");

struct CurrentFixture {
    artifact: Vec<u8>,
    code_hash_hex: String,
    abi_hash_hex: String,
    header_len: usize,
    code_offset: usize,
    entrypoint_count: usize,
}

fn current_fixture() -> CurrentFixture {
    let fixture = norito::json::from_str::<norito::json::Value>(CURRENT_FIXTURE)
        .expect("parse exact-current fixture metadata");
    let fixture = fixture
        .as_object()
        .expect("exact-current fixture metadata must be an object");
    let encoded = fixture
        .get("artifact_base64")
        .and_then(norito::json::Value::as_str)
        .expect("fixture carries artifact_base64");
    let verifier = fixture
        .get("rust_verifier")
        .and_then(norito::json::Value::as_object)
        .expect("fixture carries rust_verifier");
    let string_field = |name| {
        verifier
            .get(name)
            .and_then(norito::json::Value::as_str)
            .unwrap_or_else(|| panic!("rust_verifier carries string {name}"))
            .to_owned()
    };
    let usize_field = |name| {
        verifier
            .get(name)
            .and_then(norito::json::Value::as_u64)
            .and_then(|value| usize::try_from(value).ok())
            .unwrap_or_else(|| panic!("rust_verifier carries usize {name}"))
    };
    CurrentFixture {
        artifact: STANDARD.decode(encoded).expect("decode fixture artifact"),
        code_hash_hex: string_field("code_hash_hex"),
        abi_hash_hex: string_field("abi_hash_hex"),
        header_len: usize_field("header_len"),
        code_offset: usize_field("code_offset"),
        entrypoint_count: usize_field("entrypoint_count"),
    }
}

#[test]
fn exact_current_compiler_artifact_is_admitted() {
    let fixture = current_fixture();
    let verified = verify_contract_artifact(&fixture.artifact)
        .expect("exact-current compiler artifact must satisfy shared admission");
    assert_eq!(verified.header_len, fixture.header_len);
    assert_eq!(verified.code_offset, fixture.code_offset);
    assert_eq!(
        verified.contract_interface.entrypoints.len(),
        fixture.entrypoint_count
    );
    assert_eq!(
        hex::encode(verified.code_hash.as_ref()),
        fixture.code_hash_hex
    );
    assert_eq!(
        hex::encode(verified.abi_hash.as_ref()),
        fixture.abi_hash_hex
    );
    let json = verify_contract_artifact_json(&fixture.artifact);
    assert!(json.starts_with("{\"ok\":true,"), "{json}");
}

#[test]
fn host_private_system_syscall_is_rejected() {
    let fixture = current_fixture();
    let mut mutated = fixture.artifact;
    // Canonical wide SYSTEM encoding for host-private syscall 0x00fe0000.
    mutated[fixture.code_offset..fixture.code_offset + 4]
        .copy_from_slice(&[0x00, 0x00, 0xfe, 0x62]);
    let error = verify_contract_artifact(&mutated)
        .expect_err("host-private SYSTEM syscall must fail shared admission");
    assert!(error.to_string().contains("disallowed syscall 0xfe0000"));
    let json = verify_contract_artifact_json(&mutated);
    assert!(json.starts_with("{\"ok\":false,"), "{json}");
}
