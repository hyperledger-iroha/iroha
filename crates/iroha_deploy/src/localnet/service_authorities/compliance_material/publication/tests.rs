//! Actual generated governance keys, exact catalog successors and finite original validity.

use super::*;
use iroha_fs::{PrivateDirectory, PublishMode};

fn prepared(root: &Path, label: &str) -> PreparedLocalnet {
    let ports = crate::managed::LocalnetPorts::reserve().unwrap();
    prepare_localnet_at(
        label,
        root,
        &ports,
        LocalnetServiceProfile::StreamTokenAuthorities,
        None,
    )
    .unwrap()
}

#[test]
fn generated_catalogs_sign_exact_successors_without_extending_original_material() {
    let _resources = crate::managed::native_test_guard();
    let temporary = crate::localnet::localnet_test_helpers::private_tempdir().unwrap();
    let prepared = prepared(&temporary.path().join("generation"), "catalog-signing");
    let provider = prepared
        .stream_token_authorities()
        .unwrap()
        .unwrap()
        .providers[0]
        .provider_id;
    let plan = prepared.gateway_compliance_plan(provider).unwrap().unwrap();
    let start = plan.issued_at_unix();
    let first = prepared
        .sign_gateway_compliance_catalog(provider, None, start)
        .unwrap();
    first.verify(plan.trust_policy(), start, 0).unwrap();
    assert_eq!(first.approvals.len(), 2);
    assert_eq!(first.approvals[0].signer_id, CATALOG_IDS[0]);
    assert_eq!(first.approvals[1].signer_id, CATALOG_IDS[1]);
    assert_eq!(first.payload.sequence, 1);
    assert!(first.payload.predecessor_digest.is_none());
    assert_eq!(
        first.payload.valid_until_unix,
        start + GENERATED_CATALOG_VALIDITY_SECONDS
    );
    assert_eq!(
        encode(&first).unwrap(),
        encode(
            &prepared
                .sign_gateway_compliance_catalog(provider, None, start)
                .unwrap()
        )
        .unwrap()
    );
    let next_time = first.payload.valid_until_unix;
    assert!(first.verify(plan.trust_policy(), next_time, 0).is_err());
    let second = prepared
        .sign_gateway_compliance_catalog(provider, Some(&first), next_time)
        .unwrap();
    second.verify(plan.trust_policy(), next_time, 0).unwrap();
    validate_catalog_transition(Some(&first), &second).unwrap();
    assert_eq!(second.payload.sequence, 2);
    assert_eq!(
        second.payload.predecessor_digest,
        Some(first.payload.catalog_digest().unwrap())
    );
    let last = prepared
        .sign_gateway_compliance_catalog(provider, Some(&second), plan.expires_at_unix() - 1)
        .unwrap();
    assert_eq!(last.payload.valid_until_unix, plan.expires_at_unix());
    assert!(
        prepared
            .sign_gateway_compliance_catalog(provider, None, start - 1)
            .is_err()
    );
    assert!(
        prepared
            .sign_gateway_compliance_catalog(provider, Some(&second), start)
            .is_err()
    );
    assert!(
        prepared
            .sign_gateway_compliance_catalog(provider, Some(&last), plan.expires_at_unix())
            .is_err()
    );
    assert!(
        prepared
            .sign_gateway_compliance_catalog(provider, None, u64::MAX)
            .is_err()
    );
    assert_eq!(
        prepared
            .gateway_compliance_plan(provider)
            .unwrap()
            .unwrap()
            .expires_at_unix(),
        plan.expires_at_unix()
    );

    // Generated policy never accepts injected source/rule state, even before signing.
    let mut changed = first.payload.clone();
    changed.valid_until_unix += 1;
    assert!(validate_generated_payload(&changed, &plan).is_err());
    let mut reversed = first.payload.clone();
    reversed.valid_until_unix = reversed.generated_at_unix - 1;
    assert!(validate_generated_payload(&reversed, &plan).is_err());
    let mut changed = first.payload.clone();
    changed.generated_at_unix -= 1;
    assert!(validate_generated_payload(&changed, &plan).is_err());
    let mut changed = first.payload.clone();
    changed.source_anchors.push(
        sorafs_manifest::gateway_compliance::GatewayComplianceSourceAnchorV1 {
            feed_id: "unselected-feed".into(),
            feed_digest: [1; 32],
            generated_at_unix: start,
        },
    );
    changed = changed.normalize().unwrap();
    changed.validate().unwrap();
    assert!(validate_generated_payload(&changed, &plan).is_err());
}

#[test]
fn catalog_signing_refuses_foreign_predecessors_substituted_keys_and_standard_profiles() {
    let _resources = crate::managed::native_test_guard();
    let temporary = crate::localnet::localnet_test_helpers::private_tempdir().unwrap();
    let one = prepared(&temporary.path().join("one"), "catalog-one");
    let two = prepared(&temporary.path().join("two"), "catalog-two");
    let provider = one.stream_token_authorities().unwrap().unwrap().providers[0].provider_id;
    let other_provider = two.stream_token_authorities().unwrap().unwrap().providers[0].provider_id;
    let plan = one.gateway_compliance_plan(provider).unwrap().unwrap();
    let other = two
        .gateway_compliance_plan(other_provider)
        .unwrap()
        .unwrap();
    assert_ne!(plan.network_id(), other.network_id());
    let now = plan.issued_at_unix().max(other.issued_at_unix());
    let original = one
        .sign_gateway_compliance_catalog(provider, None, now)
        .unwrap();
    assert!(
        two.sign_gateway_compliance_catalog(other_provider, Some(&original), now)
            .is_err()
    );
    let mut changed = original.clone();
    changed.approvals[0].signature[0] ^= 1;
    assert!(
        one.sign_gateway_compliance_catalog(provider, Some(&changed), now)
            .is_err()
    );
    let mut oversized = original.clone();
    oversized.approvals = vec![original.approvals[0].clone(); PLAN_MAX_BYTES / 64 + 1];
    assert!(encode(&oversized).is_err());
    assert!(
        one.sign_gateway_compliance_catalog(provider, Some(&oversized), now)
            .is_err()
    );
    let directory = PrivateDirectory::open_exact(
        one.context
            .client_config
            .parent()
            .unwrap()
            .join(LOCALNET_RUNTIME_DIRECTORY)
            .join(DIRECTORY),
    )
    .unwrap();
    let directory = open_provider_directory(&directory, 0).unwrap();
    let original_key = directory
        .read(CATALOG_KEYS[0], MAX_ROLE_CREDENTIAL_BYTES)
        .unwrap();
    let ack_key = directory
        .read(ACK_KEYS[0], MAX_ROLE_CREDENTIAL_BYTES)
        .unwrap();
    directory
        .write_atomic(CATALOG_KEYS[0], &ack_key, PublishMode::Replace)
        .unwrap();
    assert!(
        one.sign_gateway_compliance_catalog(provider, None, now)
            .is_err()
    );
    directory
        .write_atomic(CATALOG_KEYS[0], &original_key, PublishMode::Replace)
        .unwrap();
    assert_eq!(
        one.sign_gateway_compliance_catalog(provider, None, now)
            .unwrap(),
        original
    );

    let ports = crate::managed::LocalnetPorts::reserve().unwrap();
    let standard = prepare_localnet_at(
        "catalog-standard",
        &temporary.path().join("standard"),
        &ports,
        LocalnetServiceProfile::Standard,
        None,
    )
    .unwrap();
    assert!(
        standard
            .sign_gateway_compliance_catalog(provider, None, now)
            .is_err()
    );
}
