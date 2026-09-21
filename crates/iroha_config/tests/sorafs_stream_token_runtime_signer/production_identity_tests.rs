//! TOML admission uses the canonical identity grammar and preserves exact configured labels.
use super::*;

const IDENTITIES: [&str; 6] = [
    "service_id",
    "administrator_id",
    "attester.service_id",
    "attester.administrator_id",
    "observer.service_id",
    "observer.administrator_id",
];

#[test]
fn all_six_configured_identities_accept_real_words_without_normalization() {
    let labels = [
        "Account-Attester",
        "Attestation-Security",
        "Latest-Authority",
        "Contest-Security",
        "Observer-Attester",
        "Observer-Attestation",
    ];
    let mut fields = signer_fields();
    for (field, label) in IDENTITIES.into_iter().zip(labels) {
        fields
            .iter_mut()
            .find(|(name, _)| *name == field)
            .unwrap()
            .1 = quoted(label);
    }
    let actual = parse_overlay(&enabled_with_fields(&fields)).unwrap();
    let h = actual
        .torii
        .sorafs_storage
        .stream_tokens
        .signer
        .as_ref()
        .unwrap();
    assert_eq!(
        [
            h.service_id.as_str(),
            h.administrator_id.as_str(),
            h.attester.authority.service_id.as_str(),
            h.attester.authority.administrator_id.as_str(),
            h.observer.authority.service_id.as_str(),
            h.observer.authority.administrator_id.as_str(),
        ],
        labels
    );
    for field in ["runtime_handle", "key_handle"] {
        parse_overlay(&replaced_field(
            field,
            quoted("hsm://attestation/attester/latest-contest"),
        ))
        .unwrap();
    }
}

#[test]
fn all_six_configured_identities_reject_every_reserved_component() {
    for field in IDENTITIES {
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
            let identity = format!("production:{}:primary", reserved.to_ascii_uppercase());
            rejects(
                &replaced_field(field, quoted(&identity)),
                "canonical nonempty production identity",
            );
        }
    }
}
