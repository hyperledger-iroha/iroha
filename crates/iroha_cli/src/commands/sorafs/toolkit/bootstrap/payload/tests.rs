//! Production constructor tests using explicitly synthetic native key pairs.

use super::*;
use iroha_crypto::{ExposedPrivateKey, Hash, HashOf, KeyPair};

const NOW: u64 = 1_800_000_000;

fn key(seed: u8) -> KeyPair {
    KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519)
}

fn request() -> (NetworkId, PayloadRequest, KeyPair) {
    let network =
        NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::prehashed([11; 32])));
    let council_key = key(81);
    let vrf = KeyPair::from_seed(vec![82; 32], Algorithm::BlsNormal);
    let provider = ProviderInput {
        owner: AccountId::new(key(83).public_key().clone()),
        provider_id: [84; 32],
        profile_id: "sorafs.sf1@1.0.0".into(),
        stake_pool_id: [85; 32],
        stake_xor: "100".into(),
        advert_public_key: key(86).public_key().clone(),
        por_vrf_public_key: vrf.public_key().clone(),
        potr_mldsa_public_key: None,
        jurisdiction_code: "JP".into(),
        contact_uri: None,
        issued_at: NOW - 60,
        retention_epoch: NOW + 3600,
        endpoints: vec![EndpointInput {
            https_origin: "https://provider.example.com".into(),
            attested_at: NOW - 120,
            expires_at: NOW + 7200,
            // Structural test bytes only; this is not a TLS attestation/reachability test.
            leaf_certificate_der_base64: STANDARD.encode(b"synthetic certificate"),
            intermediate_certificates_der_base64: Vec::new(),
            alpn_ids: vec!["h2".into()],
        }],
        qos: QosInput {
            availability: "hot".into(),
            max_retrieval_latency_ms: 100,
            max_concurrent_streams: 4,
        },
        range: Some(RangeInput {
            max_chunk_span: 1024,
            min_granularity: 1,
            supports_sparse_offsets: false,
            requires_alignment: false,
            supports_merkle_proof: true,
        }),
        registered_account_read: Some(RegisteredAccountReadV1 {
            https_host: "provider.example.com".into(),
            https_port: 443,
            ttl_secs: 60,
            max_streams: 4,
            rate_limit_bytes: 1024 * 1024,
            requests_per_minute: 60,
        }),
        rendezvous: vec![RendezvousInput {
            topic: "sorafs.sf1.primary".into(),
            region: "global".into(),
        }],
        path_policy: PathPolicyInput {
            min_guard_weight: 1,
            max_same_asn_per_path: 2,
            max_same_pool_per_path: 2,
        },
    };
    let request = PayloadRequest {
        schema: "iroha.sorafs.bootstrap.request.v1".into(),
        council: ProviderAdmissionCouncilPolicyV1 {
            network_id: *network.as_bytes(),
            policy_id: [80; 32],
            version: 1,
            revision: 1,
            predecessor_policy_digest: None,
            trusted_signers: vec![ed25519_bytes(council_key.public_key()).expect("council key")],
            signature_threshold: 1,
            paused: false,
        },
        providers: vec![provider],
        grants: Vec::new(),
        compliance_operators: Vec::new(),
    };
    (network, request, council_key)
}

#[test]
fn public_request_constructs_native_material_and_verifiable_council_admission() {
    let (network, request, key) = request();
    let bytes = norito::json::to_vec(&request).expect("request JSON");
    let request: PayloadRequest = norito::json::from_slice(&bytes).expect("decode public request");
    let materials = validate_request(&request, &network, NOW).expect("public material");
    assert_eq!(materials.len(), 1);
    let payload = sign_payload(
        request,
        &materials,
        &[key.private_key().clone()],
        &network,
        NOW,
    )
    .expect("native signed payload");
    validate_payload(&payload, &network, NOW).expect("native council verification");
}

#[test]
fn council_signing_rejects_wrong_key_and_nonexact_threshold() {
    let (network, mut request, council) = request();
    let materials = validate_request(&request, &network, NOW).expect("material");
    assert!(
        sign_payload(
            request,
            &materials,
            &[key(99).private_key().clone()],
            &network,
            NOW
        )
        .is_err()
    );
    let (_, reset, _) = self::request();
    request = reset;
    assert!(sign_payload(request, &materials, &[], &network, NOW).is_err());
    let (_, mut request, _) = self::request();
    request
        .council
        .trusted_signers
        .push(ed25519_bytes(key(99).public_key()).expect("second key"));
    request.council.trusted_signers.sort();
    request.council.signature_threshold = 2;
    let materials = validate_request(&request, &network, NOW).expect("2-of-2 material");
    assert!(
        sign_payload(
            request,
            &materials,
            &[council.private_key().clone(), council.private_key().clone()],
            &network,
            NOW
        )
        .is_err()
    );
}

#[test]
fn endpoint_policy_rejects_ambiguous_origin_and_short_attestation() {
    let (_, request, _) = request();
    let mut endpoint = request
        .providers
        .into_iter()
        .next()
        .expect("provider")
        .endpoints
        .into_iter()
        .next()
        .expect("endpoint");
    for origin in [
        "http://provider.example.com",
        "https://127.0.0.1",
        "https://user@provider.example.com",
        "https://provider.example.com/prefix",
        "https://provider.example.com:8443",
        "https://provider.example.com/",
    ] {
        endpoint.https_origin = origin.into();
        assert!(
            endpoint_material(&endpoint, NOW - 60, NOW + 3600).is_err(),
            "{origin}"
        );
    }
    endpoint.https_origin = "https://provider.example.com".into();
    endpoint.expires_at = NOW + 3599;
    assert!(endpoint_material(&endpoint, NOW - 60, NOW + 3600).is_err());
}

#[test]
fn request_rejects_unknown_fields_foreign_network_and_expiry() {
    let (network, mut request, _) = request();
    let mut json: norito::json::Value =
        norito::json::from_slice(&norito::json::to_vec(&request).expect("JSON")).expect("value");
    json.as_object_mut()
        .expect("object")
        .insert("unknown".into(), norito::json::Value::Null);
    assert!(
        norito::json::from_slice::<PayloadRequest>(&norito::json::to_vec(&json).expect("JSON"))
            .is_err()
    );
    request.council.network_id = [12; 32];
    assert!(validate_request(&request, &network, NOW).is_err());
    request.council.network_id = *network.as_bytes();
    assert!(validate_request(&request, &network, NOW + 3600).is_err());
    request.providers[0].por_vrf_public_key = key(99).public_key().clone();
    assert!(validate_request(&request, &network, NOW).is_err());
}

#[cfg(unix)]
#[test]
fn payload_command_uses_pinned_public_request_and_runtime_only_key_file() {
    let (network, request, key) = request();
    let directory = tempfile::tempdir().expect("temporary directory");
    let request_path = directory.path().join("request.json");
    let key_path = directory.path().join("council.key");
    let output = directory.path().join("payload.json");
    let bytes = norito::json::to_vec(&request).expect("request JSON");
    write_new(&request_path, &bytes).expect("public request");
    let secret = ExposedPrivateKey(key.private_key().clone())
        .try_to_multihash_string()
        .expect("test key");
    write_new(&key_path, secret.as_bytes()).expect("private test key");
    run(
        PreparePayloadArgs {
            network_id: network,
            request_json: request_path,
            request_blake3_hex: *blake3::hash(&bytes).as_bytes(),
            council_key_file: vec![key_path],
            output: output.clone(),
        },
        NOW,
    )
    .expect("payload command");
    let result = std::fs::read(output).expect("output");
    assert!(!String::from_utf8_lossy(&result).contains(&secret));
    let payload = norito::json::from_slice(&result).expect("payload JSON");
    validate_payload(&payload, &network, NOW).expect("native verification");
}
