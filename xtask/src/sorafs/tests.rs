//! Gateway operations, signatures and pin-registry fixture regressions.
use super::*;
use iroha_model_base::metadata::Metadata;
use sorafs_manifest::pin_registry::verify_alias_proof_bundle_untrusted_signers;
use std::{fs, path::Path, time::Duration};
use tempfile::tempdir;
#[test]
fn normalize_tls_host_guards_whitespace() {
    assert_eq!(
        normalize_tls_host("Docs.Sora.GW.Sora.Name").expect("normalize lowercase"),
        "docs.sora.gw.sora.name"
    );
    assert!(normalize_tls_host("invalid host").is_err());
}
#[test]
fn admission_fixtures_write_checked_public_key_artifacts() {
    let temp = tempdir().expect("tempdir");
    write_admission_fixtures(temp.path()).expect("write admission fixtures");
    let advert_path = temp.path().join("provider_alpha_advert.to");
    let envelope_path = temp.path().join("provider_alpha_envelope.to");
    assert!(envelope_path.is_file());
    assert!(temp.path().join("provider_alpha_metadata.json").is_file());
    let advert_bytes = fs::read(advert_path).expect("read provider advert");
    let advert: ProviderAdvertV1 =
        decode_from_bytes(&advert_bytes).expect("decode provider advert");
    let advert_public_key = PublicKey::from_bytes(Algorithm::Ed25519, &advert.signature.public_key)
        .expect("decode advert signing public key");
    let advert_signature_payload = advert
        .signature_payload_bytes()
        .expect("encode advert signature envelope");
    Signature::try_from_bytes(&advert.signature.signature)
        .expect("provider advert fixture signature is non-empty and nonzero")
        .verify(&advert_public_key, &advert_signature_payload)
        .expect("provider advert signature verifies");
    let envelope_bytes = fs::read(envelope_path).expect("read provider envelope");
    let envelope: ProviderAdmissionEnvelopeV1 =
        decode_from_bytes(&envelope_bytes).expect("decode provider envelope");
    let council_keypairs =
        provider_admission_fixture_council_keypairs().expect("derive fixture council keypairs");
    let council_policy = provider_admission_fixture_council_policy(&council_keypairs)
        .expect("build fixture council policy");
    AdmissionRecord::new(envelope, &council_policy).expect("council signatures verify");
}
#[test]
fn pin_registry_fixtures_use_checked_signatures() {
    let council_keys = pin_fixture_council_keypair();
    let (manifest_digest, manifest_root_cid, _) =
        pin_fixture_default_manifest().expect("build canonical fixture manifest");
    let mut record = PinManifestRecord::new(
        manifest_digest,
        manifest_root_cid,
        pin_fixture_default_chunker(),
        pin_fixture_default_chunk_digest(),
        pin_fixture_default_por_root(),
        pin_fixture_default_content_length(),
        pin_fixture_default_policy(),
        pin_fixture_alice(),
        12,
        None,
        None,
        Metadata::default(),
    );
    record.approve(12, None);
    let manifest_signatures =
        pin_fixture_build_envelope(&record, &council_keys).expect("build manifest envelope");
    let manifest_root: Value =
        json::from_slice(&manifest_signatures).expect("manifest signatures JSON");
    assert_eq!(
        verify_manifest_signatures(&manifest_root, record.digest.as_bytes(), false)
            .expect("manifest signature verifies"),
        1
    );
    let alias_binding =
        pin_fixture_alias_binding_for(&record.root_cid, "sora", "docs", 12, 36, &council_keys)
            .expect("build alias proof");
    let alias_bundle = decode_alias_proof_untrusted_signers(&alias_binding.proof)
        .expect("decode alias proof integrity");
    verify_alias_proof_bundle_untrusted_signers(&alias_bundle)
        .expect("alias proof signature integrity verifies");
}
#[test]
fn manifest_signatures_reject_malformed_ed25519_signature_r() {
    const SMALL_ORDER_R: [u8; 32] = [
        1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0,
    ];
    const NONCANONICAL_R: [u8; 32] = [
        0xee, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        0xff, 0x7f,
    ];
    let council_keys = pin_fixture_council_keypair();
    let (manifest_digest, manifest_root_cid, _) =
        pin_fixture_default_manifest().expect("build canonical fixture manifest");
    let mut record = PinManifestRecord::new(
        manifest_digest,
        manifest_root_cid,
        pin_fixture_default_chunker(),
        pin_fixture_default_chunk_digest(),
        pin_fixture_default_por_root(),
        pin_fixture_default_content_length(),
        pin_fixture_default_policy(),
        pin_fixture_alice(),
        12,
        None,
        None,
        Metadata::default(),
    );
    record.approve(12, None);
    let manifest_signatures =
        pin_fixture_build_envelope(&record, &council_keys).expect("build manifest envelope");
    let manifest_root: Value =
        json::from_slice(&manifest_signatures).expect("manifest signatures JSON");
    assert_eq!(
        verify_manifest_signatures(&manifest_root, record.digest.as_bytes(), false)
            .expect("valid manifest signature verifies"),
        1
    );
    for (label, replacement_r) in [
        ("small-order", SMALL_ORDER_R),
        ("noncanonical", NONCANONICAL_R),
    ] {
        let mut malformed_root = manifest_root.clone();
        let signature_entry = malformed_root
            .get_mut("signatures")
            .and_then(Value::as_array_mut)
            .and_then(|signatures| signatures.first_mut())
            .and_then(Value::as_object_mut)
            .expect("signature entry");
        let signature_hex = signature_entry
            .get("signature")
            .and_then(Value::as_str)
            .expect("signature hex");
        let mut signature_bytes = hex::decode(signature_hex).expect("decode signature hex");
        signature_bytes[..32].copy_from_slice(&replacement_r);
        signature_entry.insert(
            "signature".to_owned(),
            Value::String(hex::encode(signature_bytes)),
        );
        let err = verify_manifest_signatures(&malformed_root, record.digest.as_bytes(), false)
            .expect_err("malformed Ed25519 R must fail admission");
        assert!(
            err.to_string().contains("invalid signature material"),
            "unexpected {label} R error: {err}"
        );
    }
}
#[test]
fn load_tls_hosts_from_fixture_payload() {
    let temp = tempdir().expect("tempdir");
    let path = temp.path().join("hosts.json");
    let payload = serde_json::json!({
        "san_hosts": [
            "Docs.Sora.GW.Sora.Name ",
            "*.GW.SORA.ID",
            "docs.sora.gw.sora.name"
        ]
    });
    fs::write(&path, serde_json::to_vec(&payload).unwrap()).expect("write fixture");
    let hosts = load_tls_hosts_from_file(&path).expect("loaded hosts");
    assert_eq!(
        hosts,
        vec![
            "docs.sora.gw.sora.name".to_string(),
            "*.gw.sora.id".to_string()
        ]
    );
}
#[test]
fn dedup_tls_hosts_preserves_first_occurrence() {
    let mut hosts = vec![
        "docs.sora.gw.sora.name".to_string(),
        "*.gw.sora.id".to_string(),
        "docs.sora.gw.sora.name".to_string(),
    ];
    dedup_tls_hosts(&mut hosts);
    assert_eq!(
        hosts,
        vec![
            "docs.sora.gw.sora.name".to_string(),
            "*.gw.sora.id".to_string()
        ]
    );
}
#[test]
fn cache_directive_parser_extracts_expected_values() {
    let directives = parse_cache_directives("max-age=600, stale-while-revalidate=120, public");
    assert_eq!(directives.get("max-age").map(String::as_str), Some("600"));
    assert_eq!(
        directives.get("stale-while-revalidate").map(String::as_str),
        Some("120")
    );
    assert!(directives.contains_key("public"));
}
#[test]
fn proof_status_matching_handles_rotate_suffix() {
    let evaluation = AliasProofEvaluation {
        state: AliasProofState::Fresh,
        rotation_due: true,
        age: Duration::from_secs(42),
        generated_at_unix: 0,
        expires_at_unix: 0,
        expires_in: None,
    };
    assert!(proof_status_matches(&evaluation, "fresh-rotate"));
    assert!(proof_status_matches(&evaluation, "fresh"));
    assert!(!proof_status_matches(&evaluation, "refresh"));
}
#[test]
fn tls_renew_fails_without_runtime_acme_backend() {
    let temp = tempfile::tempdir().expect("create tempdir");
    let bundle_dir = temp.path().join("tls");
    let options = GatewayTlsRenewOptions {
        hostnames: vec!["gw.example.com".to_string()],
        account_email: Some("ops@example.com".to_string()),
        directory_url: "https://acme.invalid/directory".to_string(),
        dns_provider_id: None,
        output_dir: bundle_dir.clone(),
        force: false,
    };
    let err = gateway_tls_renew(options).expect_err("missing ACME backend must fail closed");
    assert!(err.to_string().contains("runtime-injected provider client"));
    assert!(
        !bundle_dir.exists(),
        "failed renewal must not write key material"
    );
}
#[test]
fn tls_revoke_archives_bundle() {
    let temp = tempfile::tempdir().expect("create tempdir");
    let bundle_dir = temp.path().join("bundle");
    fs::create_dir_all(&bundle_dir).expect("bundle dir");
    fs::write(bundle_dir.join("fullchain.pem"), "CERT").expect("seed cert");
    fs::write(bundle_dir.join("privkey.pem"), "KEY").expect("seed key");
    fs::write(bundle_dir.join("ech.json"), "null\n").expect("seed ech");
    let outcome = gateway_tls_revoke(GatewayTlsRevokeOptions {
        bundle_dir: bundle_dir.clone(),
        archive_dir: None,
        reason: Some("test".into()),
        force: false,
    })
    .expect("revoke");
    assert!(
        !bundle_dir.join("fullchain.pem").exists(),
        "certificate should be moved"
    );
    assert_eq!(outcome.archived_files.len(), 4);
    for path in outcome.archived_files {
        assert!(path.exists(), "{path:?} should exist");
    }
}
#[test]
fn key_rotate_generates_material() {
    let temp = tempfile::tempdir().expect("create tempdir");
    let private_path = temp.path().join("token_signing_sk");
    let public_path = temp.path().join("token_signing_pk.json");
    let outcome = gateway_key_rotate(GatewayKeyRotateOptions {
        kind: "token-signing".into(),
        output_path: private_path.clone(),
        public_out: Some(public_path.clone()),
        force: true,
    })
    .expect("rotate");
    let private_contents = fs::read_to_string(&private_path).expect("read private key");
    assert_eq!(private_contents.trim().len(), 64);
    assert!(public_path.exists());
    assert_eq!(outcome.public_key_hex.len(), 64);
    assert!(
        outcome.public_key_prefixed.starts_with("ed"),
        "expected ed-prefixed multihash"
    );
    let public_contents = fs::read_to_string(&public_path).expect("read public key JSON");
    let public_json: serde_json::Value =
        serde_json::from_str(&public_contents).expect("public key JSON parses");
    assert_eq!(public_json["algorithm"].as_str(), Some("ed25519"));
    assert_eq!(
        public_json["key_hex"].as_str(),
        Some(outcome.public_key_hex.as_str())
    );
    assert_eq!(
        public_json["key_multihash"].as_str(),
        Some(outcome.public_key_prefixed.as_str())
    );
}
#[test]
fn checked_ed25519_public_key_bytes_rejects_non_ed25519_key() {
    let keypair = KeyPair::random_with_algorithm(Algorithm::Secp256k1);
    let error =
        checked_ed25519_public_key_bytes(keypair.public_key(), "gateway token-signing public key")
            .expect_err("secp256k1 gateway key must be rejected");
    assert!(
        error
            .to_string()
            .contains("gateway token-signing public key must be Ed25519, got secp256k1")
    );
}
fn finding(ok: bool, name: &str, detail: &str) -> ProbeFinding {
    ProbeFinding {
        ok,
        name: name.into(),
        detail: detail.into(),
    }
}
#[test]
fn gateway_probe_report_includes_failure_summary() {
    let gar_info = GatewayProbeGarInfo {
        path: "gar/sample.jws".into(),
        name: "gateway-alpha".into(),
        record_version: 1,
        manifest_cid: "bafy-example".into(),
        valid_from_epoch: 1_700_000_000,
        valid_until_epoch: Some(1_700_086_400),
        host_patterns: vec!["gw.example.com".into(), "*.gw.example.com".into()],
    };
    let response = ProbeResponse {
        status: 503,
        headers: HeaderMap::new(),
        source: ProbeSource::File {
            path: PathBuf::from("headers.txt"),
        },
    };
    let gar_detail = "host `gw.example.com` authorised by GAR";
    let findings = vec![
        finding(false, "HTTP status", "503 HTTP probe via capture"),
        finding(true, "GAR host pattern", gar_detail),
    ];
    let report = build_probe_report_value(
        1_700_123_456,
        &response,
        "gw.example.com",
        &gar_info,
        &findings,
    );
    assert_eq!(report["ok"], norito::json!(false));
    assert_eq!(report["failure_count"], norito::json!(1u64));
    assert_eq!(report["status"], norito::json!(503u64));
    assert_eq!(report["host"], norito::json!("gw.example.com"));
    assert_eq!(report["source"]["type"], norito::json!("headers-file"));
    assert_eq!(
        report["gar"]["host_patterns"],
        norito::json!(["gw.example.com", "*.gw.example.com"])
    );
    assert_eq!(report["failures"], norito::json!(["HTTP status"]));
    let findings_array = report["findings"].as_array().expect("findings array");
    assert_eq!(findings_array.len(), 2);
    assert_eq!(
        findings_array[0]["detail"],
        norito::json!("503 HTTP probe via capture")
    );
}
#[test]
fn gateway_probe_rejects_cross_origin_head_and_get_redirects() {
    for (method, status) in [("HEAD", 302_u16), ("GET", 307_u16)] {
        let target = std::net::TcpListener::bind("127.0.0.1:0").expect("bind redirect target");
        target.set_nonblocking(true).expect("nonblocking target");
        let location = format!("http://{}/substituted", target.local_addr().unwrap());
        let origin = std::net::TcpListener::bind("127.0.0.1:0").expect("bind probe origin");
        let origin_addr = origin.local_addr().expect("probe origin address");
        let server = std::thread::spawn(move || {
            let (mut stream, _) = origin.accept().expect("accept probe request");
            let response = format!("HTTP/1.1 {status} X\r\nLocation: {location}\r\n\r\n");
            std::io::Write::write_all(&mut stream, response.as_bytes()).unwrap();
        });
        let response = probe_headers_via_http(&GatewayProbeRequest {
            url: format!("http://{origin_addr}/probe"),
            method: method.into(),
            timeout_secs: Some(1),
            extra_headers: Vec::new(),
        })
        .expect("receive the original redirect response");
        server.join().expect("redirect origin finished");
        assert_eq!(response.status, status, "{method} redirect must surface");
        let error = target.accept().expect_err("redirect target contacted");
        assert_eq!(error.kind(), std::io::ErrorKind::WouldBlock);
    }
}
#[test]
fn gateway_probe_operational_artifacts_include_summary_details() {
    let temp = tempfile::tempdir().expect("tempdir");
    let log_path = temp.path().join("drill-log.md");
    let gar_path = temp.path().join("gar.jws");
    let failure_detail = "500 HTTP probe via https://gw.example";
    let findings = vec![
        finding(false, "HTTP status", failure_detail),
        finding(true, "Cache-Control", "ok"),
    ];
    let summary = ProbeRunSummary {
        started_at: OffsetDateTime::UNIX_EPOCH,
        ended_at: OffsetDateTime::UNIX_EPOCH,
        success: false,
        source_description: "HTTP probe via https://gw.example".into(),
        target_url: Some("https://gw.example".into()),
        target_host: Some("gw.example".into()),
        gar_path,
        findings: &findings,
    };
    let config = DrillLogConfig {
        log_path: log_path.clone(),
        scenario: "tls-drill".into(),
        ic: Some("Alice".into()),
        scribe: Some("Bob".into()),
        notes: Some("note|test".into()),
        link: Some("https://example.com".into()),
    };
    append_drill_log_entry(&config, &summary).expect("append drill log");
    let contents = fs::read_to_string(&log_path).expect("read log");
    assert!(
            contents.contains(
                "| 1970-01-01 | tls-drill | fail | Alice | Bob | 00:00Z | 00:00Z | note&#124;test | https://example.com |"
            ),
            "unexpected log contents:\n{contents}"
        );
    let payload_path = temp.path().join("pd.json");
    let config = PagerDutyConfig {
        payload_path,
        routing_key: "rk".into(),
        severity: "critical".into(),
        source: "unit-test".into(),
        component: Some("gateway".into()),
        group: Some("tls".into()),
        class_name: Some("drill".into()),
        dedup_key: Some("dedup".into()),
        links: vec![PagerDutyLink {
            text: "drill-log".into(),
            href: "https://example.com/drill".into(),
        }],
        endpoint_url: None,
    };
    let value = pagerduty_payload_value(&config, &summary).expect("payload");
    assert_eq!(value["routing_key"], norito::json!("rk"));
    assert_eq!(value["payload"]["severity"], norito::json!("critical"));
    assert_eq!(
        value["payload"]["custom_details"]["failure_count"],
        norito::json!(1)
    );
    assert_eq!(
        value["links"][0]["href"],
        norito::json!("https://example.com/drill")
    );
}
