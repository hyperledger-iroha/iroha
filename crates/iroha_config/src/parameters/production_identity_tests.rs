//! Public runtime handles share exact component rejection without sharing identity syntax.
use super::*;

#[test]
fn runtime_handles_preserve_real_words_and_their_independent_byte_ceiling() {
    for word in ["attester", "attestation", "latest", "contest"] {
        validate_production_runtime_handle(&format!("signer://production/{word}")).unwrap();
    }
    for reserved in [
        "null",
        "mock",
        "test",
        "dev",
        "demo",
        "fake",
        "dummy",
        "placeholder",
    ] {
        assert_eq!(
            validate_production_runtime_handle(&format!(
                "signer://production/{}",
                reserved.to_ascii_uppercase()
            )),
            Err(ProductionRuntimeHandleError::TestMarked)
        );
    }
    let boundary = "a".repeat(PRODUCTION_RUNTIME_HANDLE_MAX_BYTES);
    validate_production_runtime_handle(&boundary).unwrap();
    assert_eq!(
        validate_production_runtime_handle(&format!("{boundary}a")),
        Err(ProductionRuntimeHandleError::InvalidSyntax)
    );
    assert_eq!(
        validate_production_runtime_handle("signer://test@attester"),
        Err(ProductionRuntimeHandleError::InvalidSyntax)
    );
}
