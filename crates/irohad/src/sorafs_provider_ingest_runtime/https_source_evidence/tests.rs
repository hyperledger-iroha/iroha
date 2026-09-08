//! Real council/admission/cache fixtures; excludes finality, grant issuance and TLS qualification.
use super::*;
use crate::sorafs_provider_ingest_runtime::https_source::ProviderIngestHttpsSourceLeaseV1;
use iroha_crypto::{Algorithm, Hash, HashOf, PrivateKey, PublicKey, Signature};
use iroha_data_model::block::BlockHeader;
use sorafs_car::{
    CarBuildPlan, CarStreamingWriter, compute_chunk_plan_digest_sha3, compute_por_root,
    gateway::{GatewayProviderInput, GatewaySourceLimitsV1},
};
use sorafs_manifest::{
    ChunkingProfileV1, CouncilSignature, DagCodecId, ManifestBuilder, PinPolicy,
    ProviderAdmissionCouncilPolicy, ProviderAdmissionEnvelopeV1, ProviderAdmissionRevocationV1,
    ProviderAdvertV1, compute_advert_body_digest, compute_envelope_authorization_digest,
    compute_proposal_digest,
};
use sorafs_node::{
    FinalizedProviderIngestAuthorizationV1,
    provider_ingest_runtime::ProviderIngestAuthenticatedSourceBindingV1,
};
use std::{sync::Arc, time::Duration};
const NOW: u64 = 200_000;
fn network(seed: u8) -> NetworkId {
    NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
        [seed; 32],
    )))
}
struct Fixture {
    config: ProviderIngestHttpsSourceConfigV1,
    request: ProviderIngestSourceRequestV1,
    pins: ProviderIngestHttpsTransportPinsV1,
    registry: AdmissionRegistry,
    cache: ProviderAdvertCache,
    grant: ProviderIngestHttpsGrantV1,
}
impl Fixture {
    fn evidence(&self) -> ProviderIngestHttpsEvidenceV1<'_> {
        ProviderIngestHttpsEvidenceV1 {
            source_request: &self.request,
            assignment_revision: 1,
            grant_id: [0x33; 32],
            observed_at_unix_ms: NOW - 1000,
            valid_until_unix_ms: NOW + 5000,
            admission: &self.registry,
            adverts: &self.cache,
            transport: &self.pins,
        }
    }
    fn check(&self) -> Result<(), ProviderIngestSourceFetchErrorV1> {
        validate_provider_ingest_https_evidence_v1(&self.config, &self.evidence(), &self.grant, NOW)
    }
}
fn fixture() -> Fixture {
    fixture_with_torii_authority("source.sorafs.example")
}
fn fixture_council_signature(digest: &[u8]) -> CouncilSignature {
    let key = PrivateKey::from_bytes(Algorithm::Ed25519, &[0x45; 32]).unwrap();
    CouncilSignature {
        signer: PublicKey::from(key.clone())
            .to_bytes()
            .1
            .try_into()
            .unwrap(),
        signature: Signature::try_new(&key, digest).unwrap().payload().to_vec(),
    }
}
fn fixture_with_torii_authority(authority: &str) -> Fixture {
    let mut envelope: ProviderAdmissionEnvelopeV1 = norito::decode_from_bytes(include_bytes!(
        "../../../../../fixtures/sorafs_manifest/provider_admission/envelope_v1.to"
    ))
    .unwrap();
    let mut advert: ProviderAdvertV1 = norito::decode_from_bytes(include_bytes!(
        "../../../../../fixtures/sorafs_manifest/provider_admission/advert_v1.to"
    ))
    .unwrap();
    // The shared admission fixture's host is `torii:cluster.primary.svc.local`, a
    // tagged admission label rather than an HTTPS authority. Construct an explicit
    // transport fixture and authenticate its changed topology with both native
    // signatures; production must not strip tags or rewrite governed endpoints.
    for endpoint in &mut envelope.proposal.endpoints {
        if endpoint.endpoint.kind == EndpointKind::Torii {
            endpoint.endpoint.host_pattern = authority.into();
        }
    }
    for endpoint in &mut advert.body.endpoints {
        if endpoint.kind == EndpointKind::Torii {
            endpoint.host_pattern = authority.into();
        }
    }
    envelope.advert_body = advert.body.clone();
    envelope.proposal_digest = compute_proposal_digest(&envelope.proposal).unwrap();
    envelope.advert_body_digest = compute_advert_body_digest(&envelope.advert_body).unwrap();
    envelope.council_signatures = vec![fixture_council_signature(
        &compute_envelope_authorization_digest(&envelope).unwrap(),
    )];
    let provider_key = PrivateKey::from_bytes(Algorithm::Ed25519, &[0x21; 32]).unwrap();
    advert.signature.signature =
        Signature::try_new(&provider_key, &advert.signature_payload_bytes().unwrap())
            .unwrap()
            .payload()
            .to_vec();
    // Independently specified public fixture seed, never derived from an untrusted envelope.
    let public = PublicKey::from(PrivateKey::from_bytes(Algorithm::Ed25519, &[0x45; 32]).unwrap());
    let key = public.to_bytes().1.try_into().unwrap();
    let registry = AdmissionRegistry::from_envelopes(
        ProviderAdmissionCouncilPolicy::new([key], 1).unwrap(),
        [envelope],
    )
    .unwrap();
    let mut cache = ProviderAdvertCache::new(
        [
            CapabilityType::ToriiGateway,
            CapabilityType::ChunkRangeFetch,
        ],
        Arc::new(registry.clone()),
    );
    let prepared = cache
        .validation_policy()
        .prepare(advert.clone(), NOW / 1000)
        .unwrap();
    cache.commit_prepared(prepared, NOW / 1000).unwrap();
    let provider = advert.body.provider_id;
    let config = ProviderIngestHttpsSourceConfigV1 {
        network_id: network(1),
        binding: ProviderIngestAuthenticatedSourceBindingV1 {
            provider_id: provider,
            runtime_handle: "governed-evidence-source".into(),
            revision: 1,
            policy_digest: [0x14; 32],
        },
        limits: GatewaySourceLimitsV1 {
            max_payload_bytes: 4 * 1024 * 1024,
            max_files: 16,
            max_chunks: 16,
            max_page_bytes: 64 * 1024,
            max_pages: 8,
            page_entries: 2,
        },
        connect_timeout: Duration::from_secs(1),
        request_timeout: Duration::from_secs(2),
        operation_timeout: Duration::from_secs(10),
        max_in_flight: 1,
    };
    let payload = b"SORA CARS governed evidence";
    let plan = CarBuildPlan::single_file(payload).unwrap();
    let stats = CarStreamingWriter::new(&plan)
        .write_from_reader(&mut payload.as_slice(), &mut std::io::sink())
        .unwrap();
    let manifest = ManifestBuilder::new()
        .root_cid(stats.root_cids[0].clone())
        .dag_codec(DagCodecId(stats.dag_codec))
        .chunking_profile(ChunkingProfileV1::from_descriptor(
            sorafs_manifest::chunker_registry::lookup_by_handle("sorafs.sf1@1.0.0").unwrap(),
        ))
        .chunk_digest_sha3_256(compute_chunk_plan_digest_sha3(&plan.chunks))
        .por_root(compute_por_root(payload, &plan).unwrap())
        .content_length(plan.content_length)
        .car_digest(*stats.car_archive_digest.as_bytes())
        .car_size(stats.car_size)
        .pin_policy(PinPolicy {
            retention_epoch: 1000,
            ..PinPolicy::default()
        })
        .build()
        .unwrap();
    let authorization = FinalizedProviderIngestAuthorizationV1::from_finalized_state(
        10,
        [0x22; 32],
        [0x23; 32],
        [0x24; 32],
        *manifest.digest().unwrap().as_bytes(),
        manifest.root_cid.clone(),
        "sorafs.sf1@1.0.0".into(),
        manifest.chunk_digest_sha3_256,
        manifest.por_root,
        manifest.content_length,
    )
    .unwrap();
    let request =
        ProviderIngestSourceRequestV1::new(authorization.clone(), vec![provider], None).unwrap();
    let origin = format!(
        "https://{}/",
        advert
            .body
            .endpoints
            .iter()
            .find(|endpoint| endpoint.kind == EndpointKind::Torii)
            .unwrap()
            .host_pattern
    );
    let pins = ProviderIngestHttpsTransportPinsV1 {
        network_id: config.network_id,
        provider_id: provider,
        admission_envelope_digest: *registry.entry(&provider).unwrap().envelope_digest(),
        advert_digest: *cache.record_by_provider(&provider).unwrap().fingerprint(),
        origin: origin.clone(),
        stream_token_public_key: [0x27; 32],
        tls_roots_der: vec![vec![1]],
    };
    let grant = ProviderIngestHttpsGrantV1 {
        lease: ProviderIngestHttpsSourceLeaseV1 {
            request: ProviderIngestHttpsGrantRequestV1 {
                network_id: config.network_id,
                source_provider_id: provider,
                qualification: config.binding.qualification(),
                authorization,
                musubi_archive: None,
            },
            manifest,
            grant_id: [0x33; 32],
            advert_digest: pins.advert_digest,
            assignment_revision: 1,
            expires_at_unix_ms: NOW + 4000,
        },
        provider: GatewayProviderInput {
            name: "source".into(),
            provider_id_hex: hex::encode(provider),
            gateway_public_key_hex: hex::encode(pins.stream_token_public_key),
            base_url: origin,
            stream_token_b64: "ISSUED-GRANT-PLACEHOLDER-NOT-TRANSPORT-QUALIFICATION".into(),
            privacy_events_url: None,
        },
        tls_roots_der: pins.tls_roots_der.clone(),
    };
    Fixture {
        config,
        request,
        pins,
        registry,
        cache,
        grant,
    }
}
#[test]
fn real_council_verified_admission_and_cached_signature_bind_exact_native_grant_evidence() {
    let fixture = fixture();
    fixture.check().unwrap();
    assert!(!format!("{:?}", fixture.pins).contains(&fixture.pins.origin));
    assert!(!format!("{:?}", fixture.evidence()).contains("ISSUED-GRANT"));
    let host = reqwest::Url::parse(&fixture.pins.origin)
        .unwrap()
        .host_str()
        .unwrap()
        .to_owned();
    assert!(!format!("{:?}", fixture.evidence()).contains(&host));
}
#[test]
fn signed_admission_labels_cannot_be_rewritten_into_https_authorities() {
    fixture().check().unwrap();
    let tagged = fixture_with_torii_authority("torii:cluster.primary.svc.local");
    assert!(tagged.check().is_err());
    assert!(reqwest::Url::parse(&tagged.pins.origin).is_err());
    let explicit_default_port = fixture_with_torii_authority("source.sorafs.example:443");
    assert!(explicit_default_port.check().is_err());
    let alternate_port = fixture_with_torii_authority("source.sorafs.example:8443");
    alternate_port.check().unwrap();
}
#[test]
fn wrong_network_source_assignment_grant_and_root_fail_against_valid_evidence() {
    fixture().check().unwrap();
    let mut changed = fixture();
    changed.pins.network_id = network(2);
    assert!(changed.check().is_err());
    let mut changed = fixture();
    changed.grant.lease.assignment_revision += 1;
    assert!(changed.check().is_err());
    let mut changed = fixture();
    let prior = changed.request.authorization();
    let other_order = FinalizedProviderIngestAuthorizationV1::from_finalized_state(
        prior.finalized_height(),
        prior.finalized_block_hash(),
        prior.provider_id(),
        [0x99; 32],
        prior.manifest_digest(),
        prior.manifest_cid().to_vec(),
        prior.chunker_handle().into(),
        prior.chunk_digest_sha3_256(),
        prior.por_root(),
        prior.content_length(),
    )
    .unwrap();
    changed.request = ProviderIngestSourceRequestV1::new(
        other_order,
        vec![changed.config.binding.provider_id],
        None,
    )
    .unwrap();
    assert!(changed.check().is_err());
    let mut changed = fixture();
    changed.grant.lease.grant_id[0] ^= 1;
    assert!(changed.check().is_err());
    let mut changed = fixture();
    changed.grant.lease.request.network_id = network(2);
    assert!(changed.check().is_err());
    let mut changed = fixture();
    changed.grant.lease.manifest.root_cid[12] ^= 1;
    assert!(changed.check().is_err());
    let mut changed = fixture();
    changed.request = ProviderIngestSourceRequestV1::new(
        changed.request.authorization().clone(),
        vec![[0x55; 32]],
        None,
    )
    .unwrap();
    assert!(changed.check().is_err());
}
#[test]
fn current_revocation_rejects_an_otherwise_valid_advert_still_retained_in_cache() {
    let mut changed = fixture();
    changed.check().unwrap();
    let mut revocation: ProviderAdmissionRevocationV1 = norito::decode_from_bytes(include_bytes!(
        "../../../../../fixtures/sorafs_manifest/provider_admission/revocation_v1.to"
    ))
    .unwrap();
    revocation.envelope_digest = changed.pins.admission_envelope_digest;
    revocation.council_signatures = vec![fixture_council_signature(&revocation.digest().unwrap())];
    changed.registry.revoke(&revocation).unwrap();
    assert!(
        changed
            .cache
            .record_by_provider(&changed.config.binding.provider_id)
            .is_some()
    );
    assert!(
        changed.check().is_err(),
        "a cached historical admission cannot override current revocation"
    );
}
#[test]
fn newer_advert_expiry_and_stale_snapshot_reject_retained_grants() {
    let mut changed = fixture();
    changed.check().unwrap();
    let mut newer = changed
        .cache
        .record_by_provider(&changed.config.binding.provider_id)
        .unwrap()
        .advert()
        .clone();
    newer.issued_at += 1;
    let key = PrivateKey::from_bytes(Algorithm::Ed25519, &[0x21; 32]).unwrap();
    newer.signature.signature = Signature::try_new(&key, &newer.signature_payload_bytes().unwrap())
        .unwrap()
        .payload()
        .to_vec();
    let prepared = changed
        .cache
        .validation_policy()
        .prepare(newer, NOW / 1000)
        .unwrap();
    changed.cache.commit_prepared(prepared, NOW / 1000).unwrap();
    assert!(
        changed.check().is_err(),
        "old advert pins must not survive a newer cache head"
    );
    let original = fixture();
    let mut evidence = original.evidence();
    evidence.valid_until_unix_ms = NOW;
    assert!(
        validate_provider_ingest_https_evidence_v1(
            &original.config,
            &evidence,
            &original.grant,
            NOW
        )
        .is_err()
    );
    let mut evidence = original.evidence();
    evidence.observed_at_unix_ms = NOW + 1;
    assert!(
        validate_provider_ingest_https_evidence_v1(
            &original.config,
            &evidence,
            &original.grant,
            NOW
        )
        .is_err()
    );
    let mut expired = fixture();
    expired.grant.lease.expires_at_unix_ms = 601_000;
    let mut evidence = expired.evidence();
    evidence.observed_at_unix_ms = 599_000;
    evidence.valid_until_unix_ms = 605_000;
    assert!(
        validate_provider_ingest_https_evidence_v1(
            &expired.config,
            &evidence,
            &expired.grant,
            600_000
        )
        .is_err()
    );
}
#[test]
fn independent_transport_pins_reject_changed_origin_issuer_roots_and_admission() {
    fixture().check().unwrap();
    let mut changed = fixture();
    changed.grant.provider.base_url = "https://other.example/".into();
    assert!(changed.check().is_err());
    let mut changed = fixture();
    changed.grant.provider.gateway_public_key_hex = hex::encode([0x55; 32]);
    assert!(changed.check().is_err());
    let mut changed = fixture();
    changed.grant.tls_roots_der[0][0] ^= 1;
    assert!(changed.check().is_err());
    let mut changed = fixture();
    changed.pins.admission_envelope_digest[0] ^= 1;
    assert!(changed.check().is_err());
    let mut changed = fixture();
    changed.pins.origin = "https://other.example/".into();
    changed.grant.provider.base_url = changed.pins.origin.clone();
    assert!(
        changed.check().is_err(),
        "an independently supplied origin still requires exact advertised host membership"
    );
}
