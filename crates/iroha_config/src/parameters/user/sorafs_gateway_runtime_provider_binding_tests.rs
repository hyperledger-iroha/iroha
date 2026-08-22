use super::*;

const ACME_HANDLE: &str = "runtime://sorafs/gateway-acme/primary";

#[test]
fn exact_production_binding_parses() {
    let mut emitter = Emitter::new();
    let binding = parse_sorafs_gateway_runtime_provider_binding(
        &mut emitter,
        "sorafs.gateway.acme",
        "provider",
        true,
        Some(ACME_HANDLE),
        Some(9),
        Some(&"a7".repeat(32)),
    )
    .expect("valid production binding");
    assert!(emitter.into_result().is_ok());
    assert_eq!(binding.provider_handle, ACME_HANDLE);
    assert_eq!(binding.revision, 9);
    assert_eq!(binding.policy_digest, [0xa7; 32]);
}

#[test]
fn incomplete_and_non_production_bindings_fail_without_echoing_values() {
    let valid_digest = "a7".repeat(32);
    let uppercase_digest = "A7".repeat(32);
    let zero_digest = "00".repeat(32);
    for (label, handle, revision, digest, expected) in [
        (
            "partial",
            Some(ACME_HANDLE),
            Some(9),
            None,
            "provider_policy_digest_hex is required",
        ),
        (
            "test-marked handle",
            Some("runtime://sorafs/gateway-acme/test-client-secret"),
            Some(9),
            Some(valid_digest.as_str()),
            "must be one canonical production provider handle",
        ),
        (
            "zero revision",
            Some(ACME_HANDLE),
            Some(0),
            Some(valid_digest.as_str()),
            "provider_revision must be nonzero",
        ),
        (
            "uppercase digest",
            Some(ACME_HANDLE),
            Some(9),
            Some(uppercase_digest.as_str()),
            "exactly 64 lowercase hexadecimal characters",
        ),
        (
            "zero digest",
            Some(ACME_HANDLE),
            Some(9),
            Some(zero_digest.as_str()),
            "provider_policy_digest_hex must be nonzero",
        ),
    ] {
        let mut emitter = Emitter::new();
        assert!(
            parse_sorafs_gateway_runtime_provider_binding(
                &mut emitter,
                "sorafs.gateway.acme",
                "provider",
                true,
                handle,
                revision,
                digest,
            )
            .is_none(),
            "{label}"
        );
        let diagnostic = format!(
            "{:?}",
            emitter
                .into_result()
                .expect_err("invalid binding must emit a diagnostic")
        );
        assert!(
            diagnostic.contains(expected),
            "{label} produced unexpected diagnostic: {diagnostic}"
        );
        if label == "test-marked handle" {
            assert!(
                !diagnostic.contains("test-client-secret"),
                "provider values must not be echoed"
            );
        }
    }
}

#[test]
fn disabled_provider_rejects_dormant_binding() {
    let mut emitter = Emitter::new();
    assert!(
        parse_sorafs_gateway_runtime_provider_binding(
            &mut emitter,
            "sorafs.gateway.compliance",
            "feed_transport_provider",
            false,
            Some("https://gateway-feed/primary"),
            Some(1),
            Some(&"51".repeat(32)),
        )
        .is_none()
    );
    let diagnostic = format!(
        "{:?}",
        emitter
            .into_result()
            .expect_err("disabled provider binding must fail")
    );
    assert!(diagnostic.contains("binding fields must be absent when disabled"));
    assert!(!diagnostic.contains("gateway-feed/primary"));
}
