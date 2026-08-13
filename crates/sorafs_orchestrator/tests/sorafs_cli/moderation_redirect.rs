fn moderation_redirect_manifest_fixture()
-> iroha_data_model::sorafs::moderation::ModerationReproManifestV1 {
    use iroha_data_model::sorafs::moderation::{
        MODERATION_MODEL_WORKING_MEMORY_BYTES_V1, MODERATION_REPRO_MANIFEST_VERSION_V1,
        ModerationFeatureProfileV1, ModerationModelEngineV1, ModerationModelFingerprintV1,
        ModerationReproBodyV1, ModerationReproManifestV1, ModerationReproSignatureV1,
        ModerationSeedMaterialV1, ModerationThresholdsV1, moderation_model_required_operations_v1,
    };

    let max_input_bytes = 1;
    let calibration_knot_count = 2;
    let mut body = ModerationReproBodyV1 {
        schema_version: MODERATION_REPRO_MANIFEST_VERSION_V1,
        manifest_id: [0xA1; 16],
        manifest_digest: [0; 32],
        runner_hash: [0xB2; 32],
        runtime_version: "sorafs-ai-runner redirect-test".to_string(),
        issued_at_unix: 1_800_000_000,
        seed_material: ModerationSeedMaterialV1 {
            domain_tag: "sfm4a:redirect-test".to_string(),
            seed_version: 1,
            run_nonce: [0xC3; 32],
        },
        thresholds: ModerationThresholdsV1 {
            quarantine: 6_000,
            escalate: 8_500,
        },
        models: vec![ModerationModelFingerprintV1 {
            model_id: [0x11; 16],
            artifact_path: "model.norito".to_string(),
            artifact_bytes: 1,
            artifact_digest: [0x22; 32],
            weights_digest: [0x33; 32],
            engine: ModerationModelEngineV1::DeterministicLinearV1,
            feature_profile: ModerationFeatureProfileV1::ByteHistogramAndBigramV1,
            calibration_knot_count,
            max_input_bytes,
            max_operations: moderation_model_required_operations_v1(
                max_input_bytes,
                usize::from(calibration_knot_count),
            )
            .expect("fixture operation budget"),
            working_memory_bytes: MODERATION_MODEL_WORKING_MEMORY_BYTES_V1,
            weight: Some(10_000),
        }],
        notes: Some("redirect rejection fixture".to_string()),
    };
    body.refresh_manifest_digest()
        .expect("refresh fixture manifest digest");
    let keypair = KeyPair::try_from_seed(vec![0xD4; 32], Algorithm::Ed25519)
        .expect("derive moderation fixture keypair");
    let signature = iroha_crypto::SignatureOf::try_new(keypair.private_key(), &body)
        .expect("sign moderation fixture body");
    let manifest = ModerationReproManifestV1 {
        body,
        signatures: vec![ModerationReproSignatureV1 {
            role: "council".to_string(),
            public_key: keypair.public_key().clone(),
            signature,
        }],
    };
    manifest.validate().expect("fixture manifest validates");
    manifest
}

struct ModerationRedirectFixture {
    manifest_path: PathBuf,
    payload_path: PathBuf,
    result_path: PathBuf,
}

fn prepare_moderation_redirect_fixture(root: &Path) -> ModerationRedirectFixture {
    let manifest = moderation_redirect_manifest_fixture();
    let manifest_path = root.join("moderation-redirect-manifest.to");
    fs::write(
        &manifest_path,
        to_bytes(&manifest).expect("encode moderation redirect manifest"),
    )
    .expect("write moderation redirect manifest");
    let payload_path = root.join("moderation-redirect-payload.bin");
    fs::write(&payload_path, b"moderation redirect payload").expect("write redirect payload");
    let result_path = root.join("moderation-redirect-result.json");
    let result = Value::Object(Map::from_iter([
        ("subject".into(), Value::from("cid:bafy-redirect-test")),
        (
            "subject_digest_hex".into(),
            Value::from(hex_encode([0x44; 32])),
        ),
        (
            "manifest_id_hex".into(),
            Value::from(hex_encode(manifest.body.manifest_id)),
        ),
        (
            "runner_hash_hex".into(),
            Value::from(hex_encode(manifest.body.runner_hash)),
        ),
        ("combined_score_bps".into(), Value::from(5_000_u64)),
        ("verdict".into(), Value::from("pass")),
    ]));
    fs::write(
        &result_path,
        to_vec(&result).expect("encode redirect result"),
    )
    .expect("write redirect result");
    ModerationRedirectFixture {
        manifest_path,
        payload_path,
        result_path,
    }
}

fn assert_moderation_redirect_left_no_evidence(output: &std::process::Output, path: &Path) {
    assert!(!output.status.success());
    assert!(
        output.stdout.is_empty(),
        "a redirected moderation canary must emit no evidence"
    );
    assert!(
        !path.exists(),
        "a redirected moderation canary must publish no evidence file"
    );
}

#[test]
fn moderation_runner_canary_rejects_status_and_screen_redirects_before_evidence() {
    let tempdir = tempdir().expect("tempdir");
    let fixture = prepare_moderation_redirect_fixture(tempdir.path());

    for endpoint in ["status", "screen"] {
        let origin = MockServer::start();
        let substituted_origin = MockServer::start();
        let substituted_path = format!("/substituted-runner-{endpoint}");
        let substituted_response = substituted_origin.mock(|when, then| {
            when.path(substituted_path.as_str());
            then.status(200)
                .header("content-type", "application/json")
                .body("{}");
        });
        let location = substituted_origin.url(substituted_path.as_str());
        let status_response = (endpoint == "screen").then(|| {
            origin.mock(|when, then| {
                when.method(GET).path("/v1/sorafs/moderation/runner/status");
                then.status(200)
                    .header("content-type", "application/json")
                    .body("{}");
            })
        });
        let redirect = origin.mock(|when, then| {
            if endpoint == "status" {
                when.method(GET).path("/v1/sorafs/moderation/runner/status");
                then.status(302).header("location", location.as_str());
            } else {
                when.method(POST)
                    .path("/v1/sorafs/moderation/runner/screen");
                then.status(307).header("location", location.as_str());
            }
        });
        let evidence_path = tempdir
            .path()
            .join(format!("runner-{endpoint}-evidence.json"));
        let output = sorafs_cli_cmd()
            .arg("moderation")
            .arg("runner-canary")
            .arg(format!("--manifest={}", fixture.manifest_path.display()))
            .arg("--format=norito")
            .arg(format!("--runner-url={}", origin.base_url()))
            .arg(format!("--payload={}", fixture.payload_path.display()))
            .arg("--subject=cid:bafy-runner-redirect")
            .arg("--screened-at=1800004000")
            .arg("--generated-at-unix=1800004999")
            .arg("--deployment-id=ai-prescreen-production-redirect-test")
            .arg("--environment=production")
            .arg("--deployment-context-reviewed=true")
            .arg("--process-isolation-enforcement=systemd_ip_filter")
            .arg("--process-isolation-attestation-digest=000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f")
            .arg("--process-isolation-verified-at=1800004998")
            .arg("--process-isolation-reviewed=true")
            .arg("--checked-at=1800004999")
            .arg("--timeout-ms=5000")
            .arg(format!("--json-out={}", evidence_path.display()))
            .output()
            .expect("command executes");

        assert_moderation_redirect_left_no_evidence(&output, &evidence_path);
        redirect.assert_calls(1);
        if let Some(status_response) = status_response {
            status_response.assert_calls(1);
        }
        substituted_response.assert_calls(0);
    }
}

#[test]
fn moderation_committee_canary_rejects_status_and_aggregate_redirects_before_evidence() {
    let tempdir = tempdir().expect("tempdir");
    let fixture = prepare_moderation_redirect_fixture(tempdir.path());

    for endpoint in ["status", "aggregate"] {
        let origin = MockServer::start();
        let substituted_origin = MockServer::start();
        let substituted_path = format!("/substituted-committee-{endpoint}");
        let substituted_response = substituted_origin.mock(|when, then| {
            when.path(substituted_path.as_str());
            then.status(200)
                .header("content-type", "application/json")
                .body("{}");
        });
        let location = substituted_origin.url(substituted_path.as_str());
        let status_response = (endpoint == "aggregate").then(|| {
            origin.mock(|when, then| {
                when.method(GET)
                    .path("/v1/sorafs/moderation/committee/status");
                then.status(200)
                    .header("content-type", "application/json")
                    .body("{}");
            })
        });
        let redirect = origin.mock(|when, then| {
            if endpoint == "status" {
                when.method(GET)
                    .path("/v1/sorafs/moderation/committee/status");
                then.status(302).header("location", location.as_str());
            } else {
                when.method(POST)
                    .path("/v1/sorafs/moderation/committee/aggregate");
                then.status(307).header("location", location.as_str());
            }
        });
        let evidence_path = tempdir
            .path()
            .join(format!("committee-{endpoint}-evidence.json"));
        let output = sorafs_cli_cmd()
            .arg("moderation")
            .arg("committee-canary")
            .arg(format!("--manifest={}", fixture.manifest_path.display()))
            .arg("--format=norito")
            .arg(format!("--committee-url={}", origin.base_url()))
            .arg("--quorum=1")
            .arg(format!("--result={}", fixture.result_path.display()))
            .arg("--generated-at-unix=1800006999")
            .arg("--deployment-id=ai-prescreen-production-redirect-test")
            .arg("--environment=production")
            .arg("--deployment-context-reviewed=true")
            .arg("--process-isolation-enforcement=systemd_ip_filter")
            .arg("--process-isolation-attestation-digest=202122232425262728292a2b2c2d2e2f303132333435363738393a3b3c3d3e3f")
            .arg("--process-isolation-verified-at=1800006998")
            .arg("--process-isolation-reviewed=true")
            .arg("--checked-at=1800006999")
            .arg("--timeout-ms=5000")
            .arg(format!("--json-out={}", evidence_path.display()))
            .output()
            .expect("command executes");

        assert_moderation_redirect_left_no_evidence(&output, &evidence_path);
        redirect.assert_calls(1);
        if let Some(status_response) = status_response {
            status_response.assert_calls(1);
        }
        substituted_response.assert_calls(0);
    }
}
