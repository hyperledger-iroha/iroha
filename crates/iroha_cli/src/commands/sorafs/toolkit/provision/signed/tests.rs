//! Synthetic signer separation and native signature compatibility tests.

use super::*;
use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair};
use sorafs_manifest::gateway_compliance::GatewayComplianceTrustedSignerV1;

const NOW: u64 = 1_800_000_000;

fn key(seed: u8) -> KeyPair {
    KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519)
}

fn policy() -> (GatewayComplianceTrustPolicyV1, KeyPair, KeyPair) {
    let catalog = key(1);
    let gateway = key(2);
    (
        GatewayComplianceTrustPolicyV1 {
            policy_id: [3; 32],
            catalog_threshold: 1,
            catalog_signers: vec![GatewayComplianceTrustedSignerV1 {
                signer_id: "catalog".into(),
                public_key: ed25519_public(catalog.private_key()).expect("public key"),
            }],
            revoked_catalog_signer_ids: Vec::new(),
            gateway_ack_threshold: 1,
            gateway_signers: vec![GatewayComplianceTrustedSignerV1 {
                signer_id: "gateway".into(),
                public_key: ed25519_public(gateway.private_key()).expect("public key"),
            }],
            revoked_gateway_signer_ids: Vec::new(),
        },
        catalog,
        gateway,
    )
}

fn catalog_payload(policy: &GatewayComplianceTrustPolicyV1) -> GatewayComplianceCatalogPayloadV1 {
    GatewayComplianceCatalogPayloadV1 {
        version: 1,
        sequence: 1,
        predecessor_digest: None,
        policy_digest: policy.canonical_digest().expect("policy digest"),
        generated_at_unix: NOW - 10,
        valid_until_unix: NOW + 3600,
        source_anchors: Vec::new(),
        baseline_rules: Vec::new(),
        appeal_overrides: Vec::new(),
        legal_safety_holds: Vec::new(),
        toggles: Vec::new(),
    }
}

#[test]
fn compliance_policy_receipt_uses_native_digests_and_rejects_changed_input() {
    let (policy, _, _) = policy();
    let directory = tempfile::tempdir().expect("temporary output");
    let policy_path = directory.path().join("policy.json");
    let bytes = norito::json::to_vec(&policy).expect("public policy JSON");
    std::fs::write(&policy_path, &bytes).expect("write public policy");
    let digest = *blake3::hash(&bytes).as_bytes();
    let output = directory.path().join("digests.json");
    let args = |output| CompliancePolicyArgs {
        policy: PolicyArgs {
            policy_json: policy_path.clone(),
            policy_blake3_hex: digest,
        },
        output,
    };
    compliance_policy(args(output.clone())).expect("native digest receipt");
    let receipt: norito::json::Value =
        norito::json::from_slice(&std::fs::read(&output).expect("read receipt"))
            .expect("receipt JSON");
    assert_eq!(
        receipt["policy_digest_hex"].as_str(),
        Some(hex::encode(policy.canonical_digest().expect("native policy digest")).as_str())
    );
    assert_eq!(
        receipt["feed_transport_policy_digest_hex"].as_str(),
        Some(
            hex::encode(
                gateway_compliance_feed_transport_policy_digest(&std::collections::BTreeMap::new())
                    .expect("native empty-feed digest")
            )
            .as_str()
        )
    );
    let mut changed = bytes;
    changed.push(b'\n');
    std::fs::write(&policy_path, changed).expect("change exact reviewed bytes");
    assert!(compliance_policy(args(directory.path().join("changed.json"))).is_err());
}

#[test]
fn catalog_signatures_verify_with_native_policy_and_reject_role_substitution() {
    let (policy, catalog_key, gateway_key) = policy();
    let payload = catalog_payload(&policy);
    let catalog = sign_catalog(
        payload.clone(),
        &policy,
        &[catalog_key.private_key().clone()],
        NOW,
    )
    .expect("signed catalog");
    catalog
        .verify(&policy, NOW, 0)
        .expect("native verification");
    assert!(
        sign_catalog(
            payload.clone(),
            &policy,
            &[gateway_key.private_key().clone()],
            NOW
        )
        .is_err()
    );
    assert!(sign_catalog(payload.clone(), &policy, &[], NOW).is_err());
    assert!(
        sign_catalog(
            payload.clone(),
            &policy,
            &vec![catalog_key.private_key().clone(); 2],
            NOW
        )
        .is_err()
    );
    assert!(
        sign_catalog(
            payload,
            &policy,
            &[catalog_key.private_key().clone()],
            NOW + 3600
        )
        .is_err()
    );
}

#[test]
fn gateway_acknowledgement_is_bound_to_catalog_identity_key_and_time() {
    let (policy, catalog_key, gateway_key) = policy();
    let catalog = sign_catalog(
        catalog_payload(&policy),
        &policy,
        &[catalog_key.private_key().clone()],
        NOW,
    )
    .expect("catalog");
    let digest = catalog.verify(&policy, NOW, 0).expect("digest");
    let payload = GatewayComplianceAcknowledgementPayloadV1 {
        version: 1,
        gateway_id: "gateway".into(),
        catalog_digest: digest,
        observed_at_unix: NOW,
        accepted: true,
        rejection_code: None,
    };
    let ack = sign_acknowledgement(
        payload.clone(),
        &policy,
        digest,
        gateway_key.private_key(),
        NOW,
        0,
    )
    .expect("acknowledgement");
    ack.verify(&policy, digest, NOW, 0)
        .expect("native verification");
    assert!(
        sign_acknowledgement(
            payload.clone(),
            &policy,
            [9; 32],
            gateway_key.private_key(),
            NOW,
            0
        )
        .is_err()
    );
    assert!(
        sign_acknowledgement(
            payload.clone(),
            &policy,
            digest,
            catalog_key.private_key(),
            NOW,
            0
        )
        .is_err()
    );
    assert!(
        sign_acknowledgement(
            payload,
            &policy,
            digest,
            gateway_key.private_key(),
            NOW + 31,
            30
        )
        .is_err()
    );
}

#[test]
fn advert_signing_preserves_admitted_body_and_rejects_wrong_key_network_or_lifetime() {
    let mut envelope: ProviderAdmissionEnvelopeV1 =
        norito::decode_from_bytes(include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../fixtures/sorafs_manifest/provider_admission/envelope_v1.to"
        )))
        .expect("test envelope");
    let key = key(90);
    let network =
        NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::prehashed([11; 32])));
    envelope.network_id = *network.as_bytes();
    envelope.proposal.advert_key = ed25519_public(key.private_key()).expect("advert public key");
    envelope.issued_at = NOW - 100;
    envelope.retention_epoch = NOW + 3600;
    for endpoint in &mut envelope.proposal.endpoints {
        endpoint.attestation.attested_at = NOW - 200;
        endpoint.attestation.expires_at = NOW + 3600;
    }
    let advert =
        sign_advert(&envelope, &network, NOW, NOW + 300, NOW, key.private_key()).expect("advert");
    assert_eq!(advert.body, envelope.advert_body);
    advert.verify_signature().expect("native signature");
    assert!(sign_advert(&envelope, &network, NOW, NOW + 3601, NOW, key.private_key()).is_err());
    assert!(
        sign_advert(
            &envelope,
            &network,
            NOW,
            NOW + 300,
            NOW,
            self::key(91).private_key()
        )
        .is_err()
    );
    envelope.network_id = [22; 32];
    assert!(sign_advert(&envelope, &network, NOW, NOW + 300, NOW, key.private_key()).is_err());
}
