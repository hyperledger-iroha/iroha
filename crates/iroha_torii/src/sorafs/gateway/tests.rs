//! Cross-module gateway policy tests.

#![allow(clippy::restriction)]

use std::{
    sync::Arc,
    time::{Instant, SystemTime},
};

use super::{
    denylist::{
        DenylistEntryBuilder, GatewayDenylist, PerceptualFamilyEntry, PerceptualObservation,
    },
    policy::{GatewayPolicy, GatewayPolicyConfig, PolicyDecision, PolicyViolation, RequestContext},
    rate_limit::{ClientFingerprint, GatewayRateLimitConfig, GatewayRateLimiter},
};
use crate::sorafs::{AdmissionRegistry, gateway::PerceptualMatchBasis};

fn sample_fingerprint() -> ClientFingerprint {
    ClientFingerprint::from_identifier("gateway-test-client")
}

#[test]
fn policy_allows_when_envelope_not_required() {
    let config = GatewayPolicyConfig {
        require_manifest_envelope: false,
        enforce_admission: false,
        rate_limit: GatewayRateLimitConfig::disabled(),
        ..GatewayPolicyConfig::default()
    };

    let denylist = Arc::new(GatewayDenylist::new());
    let rate_limiter = GatewayRateLimiter::new(config.rate_limit.clone());
    let policy = GatewayPolicy::new(config, None, denylist, rate_limiter);

    let fingerprint = sample_fingerprint();
    let ctx = RequestContext::new(&fingerprint, SystemTime::now(), Instant::now());
    assert_eq!(policy.evaluate(&ctx), PolicyDecision::Allow);
}

#[test]
fn policy_denies_when_manifest_required() {
    let config = GatewayPolicyConfig::default();
    let denylist = Arc::new(GatewayDenylist::new());
    let admission = Some(Arc::new(AdmissionRegistry::empty()));
    let policy = GatewayPolicy::new(
        config,
        admission,
        denylist,
        GatewayRateLimiter::new_default(),
    );

    let provider = [0xAA; 32];
    let fingerprint = sample_fingerprint();
    let ctx = RequestContext::new(&fingerprint, SystemTime::now(), Instant::now())
        .with_provider_id(&provider);

    assert_eq!(
        policy.evaluate(&ctx),
        PolicyDecision::Deny(PolicyViolation::ManifestEnvelopeMissing)
    );
}

#[test]
fn policy_allows_without_admission_registry() {
    let config = GatewayPolicyConfig {
        require_manifest_envelope: false,
        enforce_admission: true,
        rate_limit: GatewayRateLimitConfig::disabled(),
        ..Default::default()
    };

    let denylist = Arc::new(GatewayDenylist::new());
    let rate_limiter = GatewayRateLimiter::new(config.rate_limit.clone());
    let policy = GatewayPolicy::new(config, None, denylist, rate_limiter);

    let fingerprint = sample_fingerprint();
    let ctx = RequestContext::new(&fingerprint, SystemTime::now(), Instant::now());

    assert_eq!(policy.evaluate(&ctx), PolicyDecision::Allow);
}

#[test]
fn perceptual_denylist_trips_without_provider_id() {
    let config = GatewayPolicyConfig {
        rate_limit: GatewayRateLimitConfig::disabled(),
        ..GatewayPolicyConfig::default()
    };
    let rate_limit = config.rate_limit.clone();

    let denylist = Arc::new(GatewayDenylist::new());
    let family_id = [0xEF; 16];
    let canonical_hash = [0xAA; 32];
    let observed_hash = canonical_hash;
    let metadata = DenylistEntryBuilder::default().build();
    let perceptual_entry = PerceptualFamilyEntry::new(family_id, metadata)
        .with_perceptual_hash(Some(canonical_hash), 4);
    denylist.upsert_perceptual(perceptual_entry);
    let observation = PerceptualObservation::new(Some(&observed_hash), None);

    assert!(
        denylist
            .check_perceptual(&observation, SystemTime::now())
            .is_some()
    );

    let policy = GatewayPolicy::new(
        config,
        Some(Arc::new(AdmissionRegistry::empty())),
        Arc::clone(&denylist),
        GatewayRateLimiter::new(rate_limit),
    );

    let fingerprint = sample_fingerprint();
    let ctx = RequestContext::new(&fingerprint, SystemTime::now(), Instant::now())
        .with_manifest_envelope(true)
        .with_perceptual_observation(observation);

    match policy.evaluate(&ctx) {
        PolicyDecision::Deny(PolicyViolation::Denylisted(hit)) => {
            let match_meta = hit.perceptual_match().expect("perceptual metadata");
            match match_meta.basis() {
                PerceptualMatchBasis::Hash {
                    hamming_distance, ..
                } => {
                    assert_eq!(*hamming_distance, 0);
                }
                other => panic!("unexpected perceptual basis: {other:?}"),
            }
        }
        other => panic!("expected denylisted decision, got {other:?}"),
    }
}
