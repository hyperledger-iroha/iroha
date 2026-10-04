//! Fixed original ACK material and opaque local observation bindings; no native serving claim.

use super::*;
use crate::localnet::{LocalnetServiceProfile, prepare_localnet_at};

fn fixture() -> (tempfile::TempDir, PreparedLocalnet) {
    let directory = tempfile::tempdir_in(std::env::current_dir().unwrap()).unwrap();
    let ports = crate::managed::LocalnetPorts::reserve().unwrap();
    let prepared = prepare_localnet_at(
        "compliance-ack-material",
        &directory.path().join("generation"),
        &ports,
        LocalnetServiceProfile::StreamTokenAuthorities,
        None,
    )
    .unwrap();
    (directory, prepared)
}

fn observation(
    plan: &RetainedGatewayCompliancePlan,
    catalog: &GatewayComplianceCatalogV1,
    time: u64,
) -> ObservedGatewayCatalog {
    // Only this unit fixture manufactures the private observation. The production constructor
    // remains inside advance, after owned-child validation and exact staged-candidate readback.
    ObservedGatewayCatalog {
        network: plan.network_id(),
        policy: plan.trust_policy().canonical_digest().unwrap(),
        catalog: catalog.payload.catalog_digest().unwrap(),
        observed_at: time,
    }
}

#[test]
fn acknowledgement_binds_original_gateway_key_catalog_network_policy_and_live_interval() {
    let _resources = crate::managed::native_test_guard();
    let (_temporary, prepared) = fixture();
    let originals = prepared.stream_token_authorities().unwrap().unwrap();
    for original in &originals.providers {
        let provider = original.provider_id;
        let plan = prepared.gateway_compliance_plan(provider).unwrap().unwrap();
        let now = now_ms().unwrap() / 1_000;
        let catalog = prepared
            .sign_gateway_compliance_catalog(provider, None, now)
            .unwrap();
        prepared
            .validate_generated_gateway_catalog(provider, &catalog)
            .unwrap();
        let observed = observation(&plan, &catalog, now);
        let ack = prepared
            .sign_observed_gateway_catalog(provider, &observed, &catalog)
            .unwrap();
        assert_eq!(ack.payload.gateway_id, plan.gateway_label());
        assert_eq!(ack.payload.observed_at_unix, now);
        assert!(ack.payload.accepted);
        assert!(ack.payload.rejection_code.is_none());
        ack.verify(plan.trust_policy(), observed.catalog(), now, 0)
            .unwrap();
        assert_eq!(
            encode(&ack, MAX_RECORD_BYTES).unwrap(),
            encode(
                &prepared
                    .sign_observed_gateway_catalog(provider, &observed, &catalog)
                    .unwrap(),
                MAX_RECORD_BYTES,
            )
            .unwrap()
        );
        assert_eq!(observed.network(), plan.network_id());
        assert_eq!(
            observed.policy(),
            plan.trust_policy().canonical_digest().unwrap()
        );
        assert_eq!(observed.observed_at(), now);
        // The observation can expire while the genuine original catalog remains valid. This
        // pure check exercises that independent limit without replacing the signing clock.
        assert!(observed.is_fresh_at(now));
        assert!(observed.is_fresh_at(now + 30));
        assert!(now + 31 < catalog.payload.valid_until_unix);
        assert!(!observed.is_fresh_at(now + 31));
        assert!(!observed.is_fresh_at(now - 1));

        let mut changed = observation(&plan, &catalog, now);
        changed.policy[0] ^= 1;
        assert!(
            prepared
                .sign_observed_gateway_catalog(provider, &changed, &catalog)
                .is_err()
        );
        let mut changed = observation(&plan, &catalog, now);
        changed.catalog[0] ^= 1;
        assert!(
            prepared
                .sign_observed_gateway_catalog(provider, &changed, &catalog)
                .is_err()
        );
        let mut changed = observation(&plan, &catalog, now);
        changed.network = NetworkId::from_genesis_hash(iroha_crypto::HashOf::from_untyped(
            iroha_crypto::Hash::new(b"another generated genesis"),
        ));
        assert!(
            prepared
                .sign_observed_gateway_catalog(provider, &changed, &catalog)
                .is_err()
        );
        let future = observation(&plan, &catalog, now_ms().unwrap() / 1_000 + 60);
        assert!(
            prepared
                .sign_observed_gateway_catalog(provider, &future, &catalog)
                .is_err()
        );
        let stale = observation(&plan, &catalog, now.saturating_sub(31));
        assert!(
            prepared
                .sign_observed_gateway_catalog(provider, &stale, &catalog)
                .is_err()
        );

        let mut broken = catalog.clone();
        broken.approvals[0].signature[0] ^= 1;
        assert!(
            prepared
                .validate_generated_gateway_catalog(provider, &broken)
                .is_err()
        );
        assert!(
            prepared
                .sign_observed_gateway_catalog(provider, &observed, &broken)
                .is_err()
        );
        let mut oversized = catalog.clone();
        oversized.approvals = vec![catalog.approvals[0].clone(); MAX_RECORD_BYTES / 64 + 1];
        assert!(
            prepared
                .validate_generated_gateway_catalog(provider, &oversized)
                .is_err()
        );
        assert!(
            prepared
                .sign_observed_gateway_catalog(provider, &observed, &oversized)
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
        let other = originals.providers[(usize::from(original.slot) + 1) % 3].provider_id;
        assert!(
            prepared
                .validate_generated_gateway_catalog(other, &catalog)
                .is_err()
        );
        assert!(
            prepared
                .sign_observed_gateway_catalog(other, &observed, &catalog)
                .is_err()
        );
    }
}

#[test]
fn acknowledgement_refuses_substituted_original_private_material() {
    let _resources = crate::managed::native_test_guard();
    let (_temporary, prepared) = fixture();
    let provider = prepared
        .stream_token_authorities()
        .unwrap()
        .unwrap()
        .providers[2]
        .provider_id;
    let plan = prepared.gateway_compliance_plan(provider).unwrap().unwrap();
    let now = now_ms().unwrap() / 1_000;
    let catalog = prepared
        .sign_gateway_compliance_catalog(provider, None, now)
        .unwrap();
    let observed = observation(&plan, &catalog, now);
    let root =
        PrivateDirectory::open_exact(prepared.context.client_config.parent().unwrap()).unwrap();
    let keys = root
        .open_child("runtime")
        .unwrap()
        .open_child("stream-token-authorities")
        .unwrap()
        .open_child("providers")
        .unwrap()
        .open_child("2")
        .unwrap();
    let ack = keys.read("compliance-gateway.key", 4096).unwrap().to_vec();
    let governance = keys.read("compliance-catalog-0.key", 4096).unwrap();
    keys.write_atomic("compliance-gateway.key", &governance, PublishMode::Replace)
        .unwrap();
    assert!(
        prepared
            .sign_observed_gateway_catalog(provider, &observed, &catalog)
            .is_err()
    );
    keys.write_atomic("compliance-gateway.key", &ack, PublishMode::Replace)
        .unwrap();
    let observed = observation(&plan, &catalog, now_ms().unwrap() / 1_000);
    prepared
        .sign_observed_gateway_catalog(provider, &observed, &catalog)
        .unwrap();
}
