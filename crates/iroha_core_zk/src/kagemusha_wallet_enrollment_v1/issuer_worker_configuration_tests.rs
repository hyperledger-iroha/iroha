//! Current policy-to-private-worker DATA vectors and substitution regressions.
use super::*;

const ROOT: &[u8] = b"Unadmitted configuration vector root DATA";

fn selected(apple: bool) -> RequestV1 {
    let mut request = fixture(apple).request;
    let pin = Sha256::digest(ROOT).into();
    match &mut request.body.policy.platform {
        KagemushaWalletEnrollmentPlatformV1::Android {
            attestation_root_sha256,
            ..
        }
        | KagemushaWalletEnrollmentPlatformV1::Apple {
            attestation_root_sha256,
        } => {
            *attestation_root_sha256 = pin;
        }
    }
    request.body.challenge.enrollment_policy = request.body.policy.policy_digest().unwrap();
    request.body.marker = KagemushaWalletMarkerV1::enrollment(
        &request.body.challenge,
        request.body.marker.payment_key,
    )
    .unwrap();
    let key = KeyPair::from_seed(vec![43; 32], Algorithm::Ed25519);
    request.account_signature = Signature::try_new(
        key.private_key(),
        &request.body.account_challenge().unwrap(),
    )
    .unwrap()
    .payload()
    .try_into()
    .unwrap();
    request
}

fn google() -> Vec<u8> {
    let project = norito::json!({"id": "vector-project", "number": (123_u64)});
    let subject = norito::json!({"email": "vector-user@vector-project.iam.gserviceaccount.com", "clientId": "1234567"});
    json::to_vec(&norito::json!({
        "schema": "iroha.kagemusha.play-integrity-verification-policy.v1", "version": (1_u16),
        "cloudProject": (project), "credentialSubject": (subject),
        "packageName": "org.example.wallet", "packageVersion": (7_u64),
        "appSigningCertificateSha256Hex": (hex::encode([3; 32])),
    }))
    .unwrap()
}

fn runtime() -> VerifierRuntimeSelectionV1<'static> {
    VerifierRuntimeSelectionV1 {
        openssl_path: "/opt/iroha/unadmitted-data/openssl",
        openssl_sha256: [21; 32],
        store_directory: "/var/lib/iroha/unadmitted-data/enrollment",
    }
}

fn configuration(request: &RequestV1) -> VerifierConfigurationV1 {
    let original = google();
    let decoder = matches!(
        request.body.app.identity,
        KagemushaWalletAppIdentityV1::Android { .. }
    )
    .then_some(GoogleDecoderOriginalV1 {
        original: &original,
        sha256: Sha256::digest(&original).into(),
    });
    VerifierConfigurationV1::from_selected(
        &request.body.app,
        &request.body.policy,
        ROOT,
        decoder,
        runtime(),
    )
    .unwrap()
}

fn vectors() -> Value {
    Value::Array([false, true].into_iter().map(|apple| {
        let request = selected(apple);
        let config = configuration(&request);
        let worker = config.request(request.clone(), 1_000, 601_000).unwrap();
        norito::json!({
            "platform": (if apple { "apple" } else { "android" }),
            "configuration_base64": (STANDARD.encode(config.original())),
            "configuration_sha256": (hex::encode(config.digest())),
            "app_frame_base64": (STANDARD.encode(request.body.app.encode_canonical().unwrap())),
            "enrollment_frame_base64": (STANDARD.encode(request.body.policy.encode_canonical().unwrap())),
            "app_transcript_base64": (STANDARD.encode(request.body.app.transcript().unwrap())),
            "enrollment_transcript_base64": (STANDARD.encode(request.body.policy.transcript().unwrap())),
            "request_base64": (STANDARD.encode(worker.original())),
            "authority": "unadmitted DATA only",
        })
    }).collect())
}

#[test]
fn selected_configuration_binds_exact_originals_policy_and_request() {
    for apple in [false, true] {
        let request = selected(apple);
        let config = configuration(&request);
        assert_eq!(
            config.digest(),
            <[u8; 32]>::from(Sha256::digest(config.original()))
        );
        let value: Value = json::from_slice(config.original()).unwrap();
        assert_eq!(
            value["app_policy_hex"].as_str().unwrap(),
            hex::encode(request.body.app.policy_digest().unwrap())
        );
        assert_eq!(
            value["enrollment_policy_hex"].as_str().unwrap(),
            hex::encode(request.body.policy.policy_digest().unwrap())
        );
        let worker = config.request(request.clone(), 1_000, 601_000).unwrap();
        assert_eq!(worker.configuration, config.digest());
        assert!(config.request(request.clone(), 1_000, 601_001).is_err());
        let foreign = selected(!apple);
        assert!(config.request(foreign, 1_000, 601_000).is_err());
        let mut changed = request.clone();
        changed.body.policy.challenge_lifetime_ms += 1;
        assert!(config.request(changed, 1_000, 601_000).is_err());
        assert_eq!(config.original(), configuration(&request).original());
    }
}

#[test]
fn configuration_refuses_substituted_roots_decoder_and_runtime() {
    let android = selected(false);
    let apple = selected(true);
    let decoder = google();
    let decoder_pin = Sha256::digest(&decoder).into();
    let make = |request: &RequestV1, root: &[u8], google, runtime| {
        VerifierConfigurationV1::from_selected(
            &request.body.app,
            &request.body.policy,
            root,
            google,
            runtime,
        )
    };
    assert!(make(&android, ROOT, None, runtime()).is_err());
    assert!(
        make(
            &apple,
            ROOT,
            Some(GoogleDecoderOriginalV1 {
                original: &decoder,
                sha256: decoder_pin
            }),
            runtime()
        )
        .is_err()
    );
    for root in [b"".as_slice(), b"substituted root", &vec![1; 16 * 1024 + 1]] {
        assert!(make(&apple, root, None, runtime()).is_err());
    }
    for (original, pin) in [
        (b"".as_slice(), decoder_pin),
        (decoder.as_slice(), [0; 32]),
        (decoder.as_slice(), [9; 32]),
    ] {
        assert!(
            make(
                &android,
                ROOT,
                Some(GoogleDecoderOriginalV1 {
                    original,
                    sha256: pin
                }),
                runtime()
            )
            .is_err()
        );
    }
    let over = vec![1; 16 * 1024 + 1];
    assert!(
        make(
            &android,
            ROOT,
            Some(GoogleDecoderOriginalV1 {
                original: &over,
                sha256: Sha256::digest(&over).into()
            }),
            runtime()
        )
        .is_err()
    );
    for path in [
        "",
        "openssl",
        "/",
        "/tmp/../openssl",
        "/tmp/./openssl",
        "/tmp//openssl",
        "/tmp/openssl/",
        "/tmp/openssl\0",
    ] {
        let mut bad = runtime();
        bad.openssl_path = path;
        assert!(make(&apple, ROOT, None, bad).is_err());
        let mut bad = runtime();
        bad.store_directory = path;
        assert!(make(&apple, ROOT, None, bad).is_err());
    }
    let mut bad = runtime();
    bad.openssl_sha256 = [0; 32];
    assert!(make(&apple, ROOT, None, bad).is_err());
    let oversized = format!("/{}", "x".repeat(128 * 1024));
    let mut bad = runtime();
    bad.openssl_path = &oversized;
    assert!(make(&apple, ROOT, None, bad).is_err());
    let mut bad = apple.clone();
    bad.body.policy.app_policy[0] ^= 1;
    assert!(make(&bad, ROOT, None, runtime()).is_err());
}

#[test]
fn private_configuration_shared_data_vectors_match_native_projection() {
    let retained: Value = json::from_str(include_str!(
        "../../../../fixtures/kagemusha/wallet_enrollment_worker_configuration_v1.json"
    ))
    .unwrap();
    assert_eq!(retained, vectors());
}

#[test]
#[ignore = "explicit maintenance capture of unadmitted private worker DATA"]
fn regenerate_private_configuration_shared_data_vectors() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/kagemusha/wallet_enrollment_worker_configuration_v1.json");
    std::fs::write(path, json::to_vec(&vectors()).unwrap()).unwrap();
}
