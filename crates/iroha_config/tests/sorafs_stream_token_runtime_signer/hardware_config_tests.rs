//! Adversarial configuration admission for one hardware signer and independent trust.
use super::*;

#[test]
fn stream_token_runtime_binding_rejects_incomplete_disabled_and_non_production_forms() {
    let fields = hardware_fields();
    let valid = enabled_with_fields(&fields);
    parse_overlay(&valid).expect("positive control");
    rejects(
        &valid.replacen("enabled = true", "enabled = false", 1),
        "requires storage.enabled",
    );
    rejects(
        &enabled_with_fields(&[]),
        "hardware is required when issuance is enabled",
    );
    for (field, value) in &fields {
        let missing = fields
            .iter()
            .filter(|(name, _)| name != field)
            .cloned()
            .collect::<Vec<_>>();
        rejects(
            &enabled_with_fields(&missing),
            &format!("{HARDWARE_PREFIX}.{field} is required"),
        );
        let dormant = format!(
            "[sorafs.storage.stream_tokens]\nenabled = false\n{}",
            hardware_tables(&[(*field, value.clone())])
        );
        rejects(
            &dormant,
            "hardware runtime bindings are forbidden while issuance is disabled",
        );
    }
    for field in ["runtime_handle", "key_handle"] {
        for invalid in [
            "software://sorafs/stream-token/primary",
            "hsm://sorafs/stream-token/test",
            "hsm://sorafs/stream token/primary",
            "hsm://user:credential@host/key",
            "hsm://key?secret=value",
            "hsm://key#secret",
            "hsm://key%2fsecret",
            "hsm:",
            "hsm:///key",
            "hsm://key//generation",
            "HSM://key/generation",
            "hsm://prod/../key",
            "pkcs11:key;pin-value=secret",
        ] {
            rejects(
                &replaced_field(field, quoted(invalid)),
                "canonical credential-free production hardware handle",
            );
        }
    }
}

#[test]
fn stream_token_admission_binding_and_bounds_fail_closed() {
    let base = enabled_overlay();
    for (source, expected) in [
        (base.replace("sealed-cas:prod/", "sealed-cas:test/"), "admission_provider_handle must be a canonical credential-free production runtime handle"),
        (base.replace("admission_provider_revision = 7", "admission_provider_revision = 0"), "admission_provider_revision must be non-zero"),
        (base.replace(&"a5".repeat(32), &"00".repeat(32)), "admission_provider_policy_digest_hex must be non-zero"),
        (base.replace("admission_provider_revision = 7\n", ""), "admission_provider_revision is required"),
        (base.replace("admission_provider_handle = \"sealed-cas:prod/stream-token/gateway-admission/v1\"\n", ""), "admission_provider_handle is required"),
        (base.replace(&format!("admission_provider_policy_digest_hex = \"{}\"\n", "a5".repeat(32)), ""), "admission_provider_policy_digest_hex is required"),
    ] { rejects(&source, expected); }
    for (field, maximum) in [
        ("admission_max_pending", 1_000_000u64),
        ("admission_max_tracked_tokens", 1_000_000),
        ("admission_reconcile_max_items", 1024),
        ("admission_lease_ttl_ms", 300000),
    ] {
        for value in [1, maximum] {
            parse_overlay(&base.replace(
                "admission_provider_revision = 7",
                &format!("admission_provider_revision = 7\n{field} = {value}"),
            ))
            .expect("inclusive admission bound");
        }
        for value in [0, maximum + 1] {
            rejects(
                &base.replace(
                    "admission_provider_revision = 7",
                    &format!("admission_provider_revision = 7\n{field} = {value}"),
                ),
                &format!("{field} must be within 1..={maximum}"),
            );
        }
    }
}

#[test]
fn stream_token_runtime_binding_rejects_noncanonical_or_invalid_ed25519_keys() {
    for field in [
        "public_key_hex",
        "attester.public_key_hex",
        "observer.public_key_hex",
    ] {
        for (public_key, expected) in [
            (
                public_key_hex(0x44).to_ascii_uppercase(),
                "canonical lowercase non-zero 32-byte hex",
            ),
            ("00".repeat(32), "canonical lowercase non-zero 32-byte hex"),
            ("11".repeat(31), "canonical lowercase non-zero 32-byte hex"),
            ("ff".repeat(32), "is not a valid Ed25519 public key"),
            (
                format!("01{}", "00".repeat(31)),
                "is not a valid Ed25519 public key",
            ),
        ] {
            let error = parse_overlay(&replaced_field(field, quoted(&public_key)))
                .expect_err("invalid public key");
            assert!(
                error.contains(&format!("{HARDWARE_PREFIX}.{field}")) && error.contains(expected),
                "unexpected diagnostic: {error}"
            );
            assert!(
                !error.contains(&public_key),
                "diagnostic must not echo configured key material"
            );
        }
    }
}

#[test]
fn all_six_authority_identities_and_three_public_keys_are_independent() {
    let fields = hardware_fields();
    for names in [
        &[
            "service_id",
            "administrator_id",
            "attester.service_id",
            "attester.administrator_id",
            "observer.service_id",
            "observer.administrator_id",
        ][..],
        &[
            "public_key_hex",
            "attester.public_key_hex",
            "observer.public_key_hex",
        ][..],
    ] {
        for (index, destination) in names.iter().enumerate() {
            for source in &names[..index] {
                let value = fields
                    .iter()
                    .find(|(name, _)| name == source)
                    .expect("known source")
                    .1
                    .clone();
                rejects(
                    &replaced_field(destination, value),
                    if names.len() == 6 {
                        "all six signer, attester and observer service/administrator identities must be distinct"
                    } else {
                        "signer, attester and observer public keys must be distinct"
                    },
                );
            }
        }
    }
}

#[test]
fn identities_and_runtime_handles_have_exact_public_bounds() {
    for field in [
        "service_id",
        "administrator_id",
        "attester.service_id",
        "attester.administrator_id",
        "observer.service_id",
        "observer.administrator_id",
    ] {
        parse_overlay(&replaced_field(field, quoted(&"a".repeat(128)))).expect("128-byte identity");
        for invalid in [
            String::new(),
            "a".repeat(129),
            "service name".to_owned(),
            "service/primary".to_owned(),
            "service@private".to_owned(),
            "テスト".to_owned(),
            "service-TEST-primary".to_owned(),
        ] {
            rejects(
                &replaced_field(field, quoted(&invalid)),
                "canonical nonempty production identity",
            );
        }
    }
    for field in ["runtime_handle", "key_handle"] {
        for scheme in ["hsm", "kms", "pkcs11"] {
            let handle = format!("{scheme}:{}", "a".repeat(128 - scheme.len() - 1));
            parse_overlay(&replaced_field(field, quoted(&handle)))
                .expect("128-byte hardware handle");
            rejects(
                &replaced_field(field, quoted(&format!("{handle}a"))),
                "canonical credential-free production hardware handle",
            );
        }
    }
    let observer = format!(
        "finalized-source:{}",
        "a".repeat(256 - "finalized-source:".len())
    );
    parse_overlay(&replaced_field(
        "observer.runtime_handle",
        quoted(&observer),
    ))
    .expect("256-byte observer routing handle");
    for invalid in [
        format!("{observer}a"),
        "software://sorafs/observer/primary".to_owned(),
        "finalized-source:prod/software/primary".to_owned(),
        "finalized-source:test/primary".to_owned(),
        "finalized-source:prod/user:secret@host".to_owned(),
        "finalized-source:prod/key?credential=x".to_owned(),
    ] {
        let error = parse_overlay(&replaced_field("observer.runtime_handle", quoted(&invalid)))
            .expect_err("invalid observer routing");
        assert!(
            error.contains("production observer handle"),
            "unexpected diagnostic: {error}"
        );
        assert!(
            !error.contains(&invalid),
            "observer diagnostic must not echo configured handle"
        );
    }
}

#[test]
fn signer_key_revision_is_the_only_token_generation_and_never_truncates() {
    for revision in [1, u64::from(u32::MAX)] {
        let actual = parse_overlay(&replaced_field("key_revision", revision.to_string()))
            .expect("inclusive signer generation");
        assert_eq!(
            actual
                .torii
                .sorafs_storage
                .stream_tokens
                .hardware
                .expect("enabled")
                .key_revision,
            revision
        );
    }
    for revision in [0, u64::from(u32::MAX) + 1] {
        rejects(
            &replaced_field("key_revision", revision.to_string()),
            "hardware.key_revision must be within 1..=4294967295",
        );
    }
    for field in [
        "policy_revision",
        "attester.key_revision",
        "attester.policy_revision",
        "observer.key_revision",
        "observer.policy_revision",
    ] {
        rejects(
            &replaced_field(field, "0".to_owned()),
            &format!("{HARDWARE_PREFIX}.{field} must be within 1..="),
        );
        parse_overlay(&replaced_field(
            field,
            (u64::from(u32::MAX) + 1).to_string(),
        ))
        .expect("authority and policy generations remain u64");
    }
    for field in [
        "policy_digest_hex",
        "attester.policy_digest_hex",
        "observer.policy_digest_hex",
    ] {
        for (value, diagnostic) in [
            ("00".repeat(32), "must be non-zero"),
            (
                "B4".repeat(32),
                "must be exactly 64 lowercase hexadecimal characters",
            ),
            (
                "b4".repeat(31),
                "must be exactly 64 lowercase hexadecimal characters",
            ),
        ] {
            rejects(
                &replaced_field(field, quoted(&value)),
                &format!("{HARDWARE_PREFIX}.{field} {diagnostic}"),
            );
        }
    }
}

#[test]
fn custody_and_observer_time_bounds_are_finite_without_a_config_clock() {
    for (field, maximum) in [
        ("attester.max_validity_ms", 86400000u64),
        ("attester.max_anchor_age_ms", 86400000),
        ("observer.max_state_age_ms", 300000),
    ] {
        for value in [1, maximum] {
            parse_overlay(&replaced_field(field, value.to_string()))
                .expect("inclusive trust limit");
        }
        for value in [0, maximum + 1] {
            rejects(
                &replaced_field(field, value.to_string()),
                &format!("{HARDWARE_PREFIX}.{field} must be within 1..={maximum}"),
            );
        }
    }
    for prefix in ["attester", "observer"] {
        for leaf in ["active_from_unix_ms", "active_until_unix_ms"] {
            let field = format!("{prefix}.{leaf}");
            rejects(
                &replaced_field(&field, "0".to_owned()),
                &format!("{HARDWARE_PREFIX}.{field} must be within 1..="),
            );
        }
        let start = if prefix == "attester" { 800000 } else { 900000 };
        for end in [start - 1, start] {
            rejects(
                &replaced_field(&format!("{prefix}.active_until_unix_ms"), end.to_string()),
                "must be after active_from_unix_ms",
            );
        }
    }
    rejects(
        &replaced_field("observer.active_until_unix_ms", "800000".to_owned()),
        "must be after active_from_unix_ms",
    );
    let mut fields = hardware_fields();
    for (name, value) in &mut fields {
        if *name == "observer.active_from_unix_ms" {
            *value = "2000000".to_owned();
        }
        if *name == "observer.active_until_unix_ms" {
            *value = "3000000".to_owned();
        }
    }
    rejects(
        &enabled_with_fields(&fields),
        "eligibility intervals must overlap",
    );
    // Future eligibility is a valid configured policy; actual current-time admission belongs to runtime verification.
    for (name, value) in &mut fields {
        if name.ends_with("active_from_unix_ms") {
            *value = "8000000000000".to_owned();
        }
        if name.ends_with("active_until_unix_ms") {
            *value = "8000001000000".to_owned();
        }
    }
    parse_overlay(&enabled_with_fields(&fields))
        .expect("configuration does not claim current eligibility");
}

#[test]
fn provider_is_required_once_and_obsolete_flat_signer_fields_have_no_aliases() {
    let valid = enabled_overlay();
    rejects(
        &valid.replace(&format!("provider_id_hex = \"{PROVIDER_HEX}\"\n"), ""),
        "requires a canonical non-zero storage.provider_id_hex",
    );
    for invalid in [
        "00".repeat(32),
        PROVIDER_HEX.to_ascii_uppercase(),
        "ab".repeat(31),
    ] {
        rejects(&valid.replace(PROVIDER_HEX, &invalid), "provider_id_hex");
    }
    for (field, value) in [
        (
            "signer_handle",
            quoted("software://sorafs/stream-token/retired"),
        ),
        ("signer_public_key_hex", quoted(&public_key_hex(0x45))),
        ("signer_revision", "4".to_owned()),
        ("signer_policy_digest_hex", quoted(&"b4".repeat(32))),
        ("key_version", "7".to_owned()),
    ] {
        rejects(
            &format!("[sorafs.storage.stream_tokens]\n{field} = {value}\n"),
            &format!("sorafs.storage.stream_tokens.{field}"),
        );
        let error = parse_overlay(&format!(
            "[sorafs.storage.stream_tokens]\n{field} = {value}\n"
        ))
        .expect_err("no compatibility aliases");
        assert!(error.contains("unknown parameter"));
    }
    for field in [
        "provider_id_hex",
        "chain_id",
        "network_id_hex",
        "algorithm",
        "private_key",
        "signing_key_path",
    ] {
        let error = parse_overlay(&format!(
            "[{HARDWARE_PREFIX}]\n{field} = \"retired-secret-value\"\n"
        ))
        .expect_err("no duplicate context or private credentials");
        assert!(
            error.contains("unknown parameter") && error.contains(field),
            "unexpected diagnostic: {error}"
        );
        assert!(!error.contains("retired-secret-value"));
    }
}

#[test]
fn disabled_defaults_do_not_construct_hardware_or_observer_trust() {
    let actual = parse_overlay("").expect("default configuration");
    assert!(!actual.torii.sorafs_storage.stream_tokens.enabled);
    assert!(actual.torii.sorafs_storage.stream_tokens.hardware.is_none());
    assert!(
        iroha_config::parameters::actual::SorafsTokenConfig::default()
            .hardware
            .is_none()
    );
    for source in [
        "[sorafs.storage.stream_tokens.hardware]\n",
        "[sorafs.storage.stream_tokens.hardware.attester]\n",
        "[sorafs.storage.stream_tokens.hardware.observer]\n",
    ] {
        assert!(
            parse_overlay(source)
                .expect("empty disabled tables")
                .torii
                .sorafs_storage
                .stream_tokens
                .hardware
                .is_none()
        );
    }
}
