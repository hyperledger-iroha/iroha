//! Tests for xtask command parsing and release helpers.

use norito::json::Value;

use super::*;

#[test]
fn norito_rpc_fixtures_accepts_only_the_canonical_output_root_option() {
    let args = [
        "xtask",
        "norito-rpc-fixtures",
        "--output-root",
        "artifacts/norito-stage",
    ];
    assert!(matches!(
        parse_command(args.into_iter().map(String::from)).expect("canonical option parses"),
        CommandKind::NoritoRpcFixtures { .. }
    ));

    for retired in [
        "--fixtures",
        "--exporter",
        "--exporter-manifest",
        "-o",
        "--out",
        "--output",
        "--out-dir",
        "--selection",
        "--selection-manifest",
        "--all",
        "--skip-encoded-check",
    ] {
        let args = ["xtask", "norito-rpc-fixtures", retired];
        let error = match parse_command(args.into_iter().map(String::from)) {
            Ok(_) => panic!("retired option {retired} must be rejected"),
            Err(error) => error,
        };
        assert!(
            error.to_string().contains("unknown flag"),
            "unexpected error for {retired}: {error}"
        );
    }
}

#[test]
fn norito_rpc_fixtures_rejects_ambiguous_output_roots() {
    for invalid in ["", ".", "..", "../stage", "stage/../escape", "/", "--all"] {
        let args = ["xtask", "norito-rpc-fixtures", "--output-root", invalid];
        let error = match parse_command(args.into_iter().map(String::from)) {
            Ok(_) => panic!("ambiguous output root {invalid:?} must be rejected"),
            Err(error) => error,
        };
        assert!(
            error.to_string().contains("output-root"),
            "unexpected error for {invalid:?}: {error}"
        );
    }
}

#[test]
fn norito_rpc_verify_accepts_only_one_explicit_report_target() {
    for arguments in [
        vec!["xtask", "norito-rpc-verify"],
        vec!["xtask", "norito-rpc-verify", "--json-out", "-"],
        vec![
            "xtask",
            "norito-rpc-verify",
            "--json-out",
            "artifacts/norito-report.json",
        ],
    ] {
        assert!(matches!(
            parse_command(arguments.into_iter().map(String::from))
                .expect("valid verifier options parse"),
            CommandKind::NoritoRpcVerify { .. }
        ));
    }

    for arguments in [
        vec!["xtask", "norito-rpc-verify", "--json-out"],
        vec!["xtask", "norito-rpc-verify", "--json-out", ""],
        vec!["xtask", "norito-rpc-verify", "--json-out", "--unknown"],
        vec![
            "xtask",
            "norito-rpc-verify",
            "--json-out",
            "first.json",
            "--json-out",
            "second.json",
        ],
    ] {
        assert!(parse_command(arguments.into_iter().map(String::from)).is_err());
    }
}

#[test]
fn norito_rpc_verify_help_describes_the_fixture_only_contract() {
    let help = NORITO_RPC_VERIFY_USAGE_DESCRIPTION.to_ascii_lowercase();
    for required in [
        "fixture", "alias", "schema", "compact", "android", "python", "swift",
    ] {
        assert!(
            help.contains(required),
            "missing `{required}` from verifier help"
        );
    }
    for false_claim in ["router", "endpoint", "transport"] {
        assert!(
            !help.contains(false_claim),
            "verifier help must not claim `{false_claim}` behavior"
        );
    }
}

#[test]
fn norito_rpc_fixture_help_discloses_every_publication_surface() {
    let help = NORITO_RPC_FIXTURES_USAGE_DESCRIPTION.to_ascii_lowercase();
    for required in [
        "canonical",
        "alias",
        "android",
        "python",
        "swift",
        "output root",
    ] {
        assert!(
            help.contains(required),
            "missing `{required}` from fixture-owner help"
        );
    }
}

#[test]
fn vote_tally_default_path_points_into_fixtures() {
    let default = default_vote_tally_path();
    assert!(default.ends_with("fixtures/zk/vote_tally"));
}

#[test]
fn soranet_fixture_default_path_points_into_tests() {
    let default = soranet::default_fixture_dir(&workspace_root());
    assert!(default.ends_with("tests/interop/soranet/capabilities"));
}

#[test]
fn parse_sorafs_adoption_check_rejects_relaxation_without_override_id() {
    let args = ["xtask", "sorafs-adoption-check", "--allow-single-source"];
    let iter = args.into_iter().map(String::from);
    let err = match parse_command(iter) {
        Ok(_) => panic!("relaxation flag must require an override id"),
        Err(err) => err,
    };
    let message = err.to_string();
    assert!(
        message.contains("require --adoption-override-id"),
        "unexpected error: {message}"
    );
}

#[test]
fn parse_sorafs_adoption_check_rejects_malformed_override_id() {
    let args = [
        "xtask",
        "sorafs-adoption-check",
        "--allow-zero-weight",
        "--adoption-override-id",
        "bad id",
    ];
    let iter = args.into_iter().map(String::from);
    let err = match parse_command(iter) {
        Ok(_) => panic!("malformed override id must fail"),
        Err(err) => err,
    };
    let message = err.to_string();
    assert!(
        message.contains("may contain only"),
        "unexpected error: {message}"
    );
}

#[test]
fn parse_sorafs_adoption_check_accepts_relaxation_with_override_id() {
    let args = [
        "xtask",
        "sorafs-adoption-check",
        "--allow-single-source",
        "--allow-zero-weight",
        "--adoption-override-id",
        "INC-2026-07-DIRECT",
    ];
    let iter = args.into_iter().map(String::from);
    let command = parse_command(iter).expect("valid override id should parse");
    match command {
        CommandKind::SorafsAdoptionCheck { options, .. } => {
            assert!(options.allow_single_source_fallback);
            assert!(!options.require_positive_weight);
        }
        _ => panic!("expected sorafs-adoption-check command"),
    }
}

#[test]
fn sm_operator_snippet_with_seed_emits_expected_files() {
    let temp = TempDir::new().expect("temp dir");
    let json_path = temp.path().join("output").join("sm2-key.json");
    let snippet_path = temp.path().join("output").join("client-sm2.toml");
    let options = crate::sm::SmOperatorSnippetOptions {
        distid: Some("CN12345678901234".to_string()),
        seed_hex: Some(
            "00112233445566778899AABBCCDDEEFF00112233445566778899AABBCCDDEEFF".to_string(),
        ),
        json_out: Some(crate::sm::OutputTarget::file(json_path.clone())),
        snippet_out: Some(crate::sm::OutputTarget::file(snippet_path.clone())),
    };
    crate::sm::generate_sm_operator_snippet(options).expect("generate snippet");

    let json_text = std::fs::read_to_string(&json_path).expect("read sm2-key.json");
    let value: Value = norito::json::from_str(&json_text).expect("parse sm2 json");
    assert_eq!(
        value["distid"],
        norito::json!("CN12345678901234"),
        "distid should match input"
    );
    assert!(
        value["public_key_config"]
            .as_str()
            .expect("public key string")
            .parse::<iroha_crypto::PublicKey>()
            .is_ok(),
        "public key config should round-trip via PublicKey::from_str"
    );

    let snippet = std::fs::read_to_string(&snippet_path).expect("read client-sm2.toml");
    assert!(
        snippet.contains("public_key = \""),
        "snippet should contain public key entry"
    );
    assert!(
        snippet.contains("[crypto]"),
        "snippet should include crypto section"
    );
    assert!(
        snippet.contains("default_hash = \"sm3-256\""),
        "snippet should set default_hash to sm3-256"
    );
    assert!(
        snippet.contains("allowed_signing = [\"ed25519\", \"sm2\"]"),
        "snippet should include sm2 in allowed_signing by default (with guidance comment)"
    );
    assert!(
        snippet.contains("sm2_distid_default = \"CN12345678901234\""),
        "snippet should embed the configured sm2_distid_default"
    );
    assert!(
        snippet.contains("# enable_sm_openssl_preview"),
        "snippet should mention optional OpenSSL preview toggle"
    );
}

#[test]
fn sm_operator_snippet_supports_stdout_targets() {
    let temp = TempDir::new().expect("temp dir");
    let snippet_path = temp.path().join("client-sm2.toml");
    let options = crate::sm::SmOperatorSnippetOptions {
        distid: Some("CN5555444433332222".to_string()),
        seed_hex: Some(
            "AA11223344556677889900AABBCCDDEEFF00112233445566778899AABBCCDD00".to_string(),
        ),
        json_out: Some(crate::sm::OutputTarget::Stdout),
        snippet_out: Some(crate::sm::OutputTarget::file(snippet_path.clone())),
    };
    crate::sm::generate_sm_operator_snippet(options).expect("generate snippet");

    assert!(
        !snippet_path.parent().unwrap().join("sm2-key.json").exists(),
        "JSON file should not be created when streamed to stdout"
    );
    let snippet = std::fs::read_to_string(&snippet_path).expect("read snippet");
    assert!(
        snippet.contains("[crypto]"),
        "stdout run should still produce snippet file when requested"
    );
}

#[test]
fn iso_bridge_lint_uses_default_reference_data() {
    lint_iso_bridge(IsoLintOptions::default()).expect("default iso lint should succeed");
}

#[cfg(feature = "vote-tally")]
#[test]
fn vote_tally_bundle_matches_expected_hashes() {
    let temp = TempDir::new().expect("temp dir");
    let summary = write_bundle(temp.path()).expect("write bundle");
    let attestation =
        vote_tally::attestation_manifest(&summary, temp.path()).expect("attestation manifest");
    assert_eq!(
        attestation["generated_unix_ms"],
        norito::json!(3513801751697071715u64)
    );
    assert_eq!(
        attestation["hash_algorithm"],
        norito::json!("blake2b-256"),
        "attestation must record the hash function"
    );
    let artifacts = attestation["artifacts"]
        .as_array()
        .expect("artifacts array");
    assert_eq!(
        artifacts.len(),
        vote_tally::bundle_file_names().len(),
        "expected attestation to include all bundle artifacts"
    );
    for expected in vote_tally::bundle_file_names() {
        let present = artifacts
            .iter()
            .any(|entry| entry["file"] == norito::json!(*expected));
        assert!(present, "missing artifact entry for {expected}");
    }
    let artifact_map = artifacts_to_map(artifacts).expect("artifact map");
    let meta_entry = artifact_map
        .get("vote_tally_meta.json")
        .expect("meta entry present");
    assert!(meta_entry.0 > 0);
    let proof_entry = artifact_map
        .get("vote_tally_proof.zk1")
        .expect("proof entry present");
    assert_eq!(proof_entry.0, summary.proof_len as u64);
    let vk_entry = artifact_map
        .get("vote_tally_vk.zk1")
        .expect("vk entry present");
    assert_eq!(vk_entry.0, summary.vk_len as u64);

    assert_eq!(
        summary.backend,
        "halo2/pasta/ipa-v1/vote-bool-commit-merkle8-v1"
    );
    assert_eq!(
        summary.circuit_id,
        "halo2/pasta/vote-bool-commit-merkle8-v1"
    );
    assert_eq!(
        summary.commit_hex,
        "20574662a58708e02e0000000000000000000000000000000000000000000000"
    );
    assert_eq!(
        summary.root_hex,
        "b63752ff429362c3a9b3cd5966c23567fdb757ce3b38af724b9303a5ea2f5817"
    );
    assert_eq!(
        summary.schema_hash_hex,
        "fae4cbe786f280b4e2184dbb06305fe46b7aee20464c0be96023ffd8eac064d3"
    );
    assert_eq!(
        summary.vk_commit_hex,
        "6f4749f5f75fee2a40880d4798123033b2b8036284225bad106b04daca5fb10e"
    );
    assert!(summary.vk_len > 0);
    assert!(summary.proof_len > 0);
}

#[cfg(feature = "vote-tally")]
#[test]
fn attestation_verification_rejects_proof_digest_drift() {
    let baseline = TempDir::new().expect("baseline dir");
    let summary = write_bundle(baseline.path()).expect("write bundle");
    let manifest_value =
        vote_tally::attestation_manifest(&summary, baseline.path()).expect("manifest");
    let manifest_path = baseline.path().join("bundle.attestation.json");
    let mut manifest_text = norito::json::to_string_pretty(&manifest_value).unwrap();
    manifest_text.push('\n');
    std::fs::write(&manifest_path, manifest_text).unwrap();

    let mut parsed: Value =
        norito::json::from_str(&std::fs::read_to_string(&manifest_path).unwrap()).unwrap();
    if let Some(object) = parsed.as_object_mut()
        && let Some(Value::Array(artifacts)) = object.get_mut("artifacts")
    {
        for entry in artifacts {
            if let Some(map) = entry.as_object_mut()
                && map.get("file") == Some(&norito::json!("vote_tally_proof.zk1"))
            {
                map.insert(
                    "blake2b_256".into(),
                    norito::json!(
                        "deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef"
                    ),
                );
            }
        }
    }
    let mut mutated = norito::json::to_string_pretty(&parsed).unwrap();
    mutated.push('\n');
    std::fs::write(&manifest_path, mutated).unwrap();
    let err = handle_attestation_manifest(
        &summary,
        baseline.path(),
        JsonTarget::File(manifest_path.clone()),
        true,
    )
    .expect_err("proof digest drift must be rejected");
    assert!(
        err.to_string().contains("vote_tally_proof.zk1"),
        "error must cite proof artefact"
    );
}

#[cfg(feature = "vote-tally")]
#[test]
fn attestation_verification_rejects_metadata_drift() {
    let baseline = TempDir::new().expect("baseline dir");
    let summary = write_bundle(baseline.path()).expect("write bundle");
    let manifest_value =
        vote_tally::attestation_manifest(&summary, baseline.path()).expect("manifest");
    let manifest_path = baseline.path().join("bundle.attestation.json");
    let mut manifest_text = norito::json::to_string_pretty(&manifest_value).unwrap();
    manifest_text.push('\n');
    std::fs::write(&manifest_path, manifest_text).unwrap();

    // Mutate the meta file digest (deterministic artefact) and ensure verification fails.
    let mut parsed: Value =
        norito::json::from_str(&std::fs::read_to_string(&manifest_path).unwrap()).unwrap();
    if let Some(object) = parsed.as_object_mut()
        && let Some(Value::Array(artifacts)) = object.get_mut("artifacts")
    {
        for entry in artifacts {
            if let Some(map) = entry.as_object_mut()
                && map.get("file") == Some(&norito::json!("vote_tally_meta.json"))
            {
                map.insert(
                    "blake2b_256".into(),
                    norito::json!(
                        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                    ),
                );
            }
        }
    }
    let mut mutated = norito::json::to_string_pretty(&parsed).unwrap();
    mutated.push('\n');
    std::fs::write(&manifest_path, mutated).unwrap();
    let err = handle_attestation_manifest(
        &summary,
        baseline.path(),
        JsonTarget::File(manifest_path.clone()),
        true,
    )
    .expect_err("metadata drift must be rejected");
    assert!(
        err.to_string().contains("vote_tally_meta.json"),
        "error should reference the divergent artefact"
    );
}

#[test]
fn verify_requires_seeded_baseline() {
    let baseline = TempDir::new().expect("baseline dir");
    let err = generate_vote_tally_bundle(baseline.path().to_path_buf(), true, false, None, None)
        .expect_err("verify without seeded fixtures must fail");
    let message = err.to_string();
    assert!(
        message.contains("run `cargo xtask zk-vote-tally-bundle"),
        "error message should suggest seeding baseline, got: {message}"
    );
}
