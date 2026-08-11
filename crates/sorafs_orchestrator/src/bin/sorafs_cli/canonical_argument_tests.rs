// Canonical CLI arguments, signing, gateway, and telemetry regressions.

#[test]
fn report_markdown_preserves_exact_sub_micro_slashing_amount() {
    let report = PorWeeklyReportV1 {
        version: sorafs_manifest::por::POR_WEEKLY_REPORT_VERSION_V1,
        cycle: PorReportIsoWeek {
            year: 2026,
            week: 28,
        },
        generated_at: 1,
        challenges_total: 1,
        challenges_verified: 0,
        challenges_failed: 1,
        forced_challenges: 0,
        repairs_enqueued: 0,
        repairs_completed: 0,
        mean_latency_ms: None,
        p95_latency_ms: None,
        slashing_events: vec![sorafs_manifest::por::PorSlashingEventV1 {
            provider_id: [1; 32],
            manifest_digest: [2; 32],
            penalty_xor: "0.000000001"
                .parse()
                .expect("sub-micro XOR quantity is canonical"),
            verdict_cid: "bafy-verdict".to_owned(),
            decided_at: 2,
        }],
        providers_missing_vrf: Vec::new(),
        top_offenders: Vec::new(),
        notes: None,
    };

    let rendered = render_report_markdown(&report);

    assert!(rendered.contains("penalty 0.000000001 XOR"));
    assert!(!rendered.contains("penalty 0.000000 XOR"));
}

#[test]
fn numeric_arg_parsers_reject_noncanonical_unsigned_tokens() {
    for value in ["", " 1", "1 ", "+1", "01", "1_000", "-1"] {
        let err = parse_u64_arg("--timeout-ms", value, "sorafs_cli moderation runner-canary")
            .expect_err("noncanonical u64 token must fail");
        assert!(
            err.contains("canonical unsigned decimal integer"),
            "unexpected u64 error for {value:?}: {err}"
        );
    }

    assert_eq!(
        parse_u64_arg("--timeout-ms", "0", "sorafs_cli moderation runner-canary")
            .expect("zero is a canonical unsigned token"),
        0
    );
    assert_eq!(
        parse_u32_arg("--backlog", "42", "sorafs_cli appeal quote").expect("canonical u32"),
        42
    );
    assert_eq!(
        parse_u16_arg(
            "--authority-network-prefix",
            "369",
            "sorafs_cli manifest submit"
        )
        .expect("canonical u16"),
        369
    );
}

#[test]
fn signed_numeric_arg_parser_rejects_noncanonical_tokens() {
    assert_eq!(
        parse_i32_arg("--live-edge-drift-ms", "-17", "sorafs_cli taikai bundle")
            .expect("canonical negative drift"),
        -17
    );
    assert_eq!(
        parse_i32_arg("--live-edge-drift-ms", "0", "sorafs_cli taikai bundle")
            .expect("canonical zero drift"),
        0
    );

    for value in ["", " 1", "1 ", "+1", "01", "-01", "-0", "1_000"] {
        let err = parse_i32_arg("--live-edge-drift-ms", value, "sorafs_cli taikai bundle")
            .expect_err("noncanonical i32 token must fail");
        assert!(
            err.contains("canonical signed decimal integer"),
            "unexpected i32 error for {value:?}: {err}"
        );
    }
}

#[test]
fn bounded_numeric_arg_parsers_still_reject_overflow() {
    let err = parse_u16_arg(
        "--authority-network-prefix",
        "70000",
        "sorafs_cli manifest submit",
    )
    .expect_err("overflowing u16 must fail");

    assert!(
        err.contains("number too large"),
        "unexpected overflow error: {err}"
    );
}

#[test]
fn taikai_digest_fields_reject_noncanonical_hex() {
    let canonical = "33".repeat(32);
    assert_eq!(
        parse_taikai_digest_hex(&canonical, "--manifest-hash").expect("canonical taikai digest"),
        [0x33; 32]
    );

    for (value, expected) in [
        ("", "must not be empty"),
        (
            "0x3333333333333333333333333333333333333333333333333333333333333333",
            "prefix",
        ),
        (
            "333333333333333333333333333333333333333333333333333333333333333A",
            "lowercase",
        ),
        (
            "333333333333333333333333333333333333333333333333333333333333333 ",
            "whitespace",
        ),
        ("3333", "exactly 64"),
        (
            "0000000000000000000000000000000000000000000000000000000000000000",
            "all zero",
        ),
    ] {
        let err = parse_taikai_digest_hex(value, "--manifest-hash")
            .expect_err("noncanonical taikai digest must fail");
        assert!(
            err.contains(expected),
            "unexpected digest error for {value:?}: {err}"
        );
    }
}

#[test]
fn taikai_track_kind_requires_canonical_lowercase() {
    assert!(matches!(
        parse_taikai_track_kind("video").expect("video kind"),
        TaikaiCliTrackKind::Video
    ));

    for (value, expected) in [
        ("Video", "canonical lowercase"),
        (" video", "canonical lowercase"),
        ("subtitle", "invalid `--track-kind`"),
    ] {
        let err = parse_taikai_track_kind(value).expect_err("noncanonical kind must fail");
        assert!(
            err.contains(expected),
            "unexpected track kind error for {value:?}: {err}"
        );
    }
}

#[test]
fn usage_omits_retired_manifest_authentication_commands() {
    let help = usage();
    assert!(!help.contains("sorafs_cli manifest sign --"));
    assert!(!help.contains("sorafs_cli manifest verify-signature --"));
}

#[test]
fn decimal_arg_parser_rejects_noncanonical_tokens() {
    for value in [
        "", " 1.25", "1.25 ", "+1.25", "01.25", "1.250", "1.0", "1.", ".25", "-0",
    ] {
        let err = parse_decimal_arg("deposit", value, CONTEXT_APPEAL_SETTLE)
            .expect_err("noncanonical decimal token must fail");
        assert!(
            err.contains("canonical decimal"),
            "unexpected decimal error for {value:?}: {err}"
        );
    }

    assert_eq!(
        parse_decimal_arg("deposit", "1.25", CONTEXT_APPEAL_SETTLE).expect("canonical decimal"),
        Decimal::new(125, 2)
    );
    assert_eq!(
        parse_decimal_arg("deposit", "0", CONTEXT_APPEAL_SETTLE).expect("canonical zero"),
        Decimal::ZERO
    );
    assert_eq!(
        parse_decimal_arg("deposit", "0.25", CONTEXT_APPEAL_SETTLE)
            .expect("canonical fractional decimal"),
        Decimal::new(25, 2)
    );
    assert_eq!(
        parse_decimal_arg("deposit", "-1.25", CONTEXT_APPEAL_SETTLE)
            .expect("canonical negative decimal remains a parser-level value"),
        Decimal::new(-125, 2)
    );
}

#[test]
fn fixture_account_uses_checked_seed_derivation() {
    let account = fixture_account(0x5A);
    let expected = AccountId::new(fixture_keypair(0x5A).public_key().clone());

    assert_eq!(account, expected);
}

#[test]
fn load_storage_pin_payload_uses_canonical_directory_ordering() {
    let tempdir = tempdir().expect("tempdir");
    let payload_dir = tempdir.path().join("site");
    fs::create_dir_all(payload_dir.join("assets")).expect("create payload dir");
    fs::write(payload_dir.join("index.html"), "<html>hayahi</html>").expect("write index");
    fs::write(
        payload_dir.join("assets").join("app.js"),
        "console.log('hayahi');",
    )
    .expect("write script");

    let manifest = sample_manifest();
    let profile = chunk_profile_from_manifest(&manifest).expect("chunk profile");
    let (expected_plan, expected_payload) =
        CarBuildPlan::from_directory_with_profile(&payload_dir, profile)
            .expect("build canonical directory payload");

    let (payload, files, payload_kind) =
        load_storage_pin_payload(&payload_dir, &manifest).expect("load storage payload");

    assert_eq!(payload_kind, "directory");
    assert_eq!(payload, expected_payload);

    let files = files.expect("directory payload should include file entries");
    let expected_files = expected_plan
        .files
        .iter()
        .map(|file| StorageFileEntryOwned {
            path: file.path.clone(),
            size: file.size,
        })
        .collect::<Vec<_>>();
    assert_eq!(files, expected_files);
}

#[test]
fn storage_manifest_helpers_reject_noncanonical_commitments() {
    let manifest = sample_manifest();
    assert_eq!(
        manifest_root_cid_hex(&manifest).expect("canonical manifest root CID"),
        hex_encode(&manifest.root_cid)
    );
    assert_eq!(
        chunk_profile_from_manifest(&manifest).expect("registered chunk profile"),
        sorafs_manifest::chunker_registry::default_descriptor().profile
    );

    let mut invalid_cid = manifest.clone();
    invalid_cid.root_cid[0] = 0;
    assert!(manifest_root_cid_hex(&invalid_cid).is_err());

    let mut substituted_profile = manifest;
    substituted_profile.chunking.max_size -= 1;
    assert!(chunk_profile_from_manifest(&substituted_profile).is_err());
}

#[test]
fn parse_account_id_arg_with_prefix_accepts_matching_i105_discriminant() {
    let account = fixture_account(0x5A);
    let encoded = account
        .to_i105_for_discriminant(369)
        .expect("encode i105 with taira discriminant");

    let parsed = parse_account_id_arg_with_prefix(
        "--authority",
        &encoded,
        "sorafs_cli manifest submit",
        Some(369),
    )
    .expect("parse authority with explicit discriminant");

    assert_eq!(parsed, account);
}

#[test]
fn parse_account_id_arg_accepts_taira_i105_without_explicit_prefix() {
    let account = fixture_account(0x59);
    let encoded = account
        .to_i105_for_discriminant(369)
        .expect("encode i105 with taira discriminant");

    let parsed = parse_account_id_arg("--authority", &encoded, "sorafs_cli manifest submit")
        .expect("parse taira authority without explicit discriminant");

    assert_eq!(parsed, account);
}

#[test]
fn parse_account_id_arg_with_prefix_rejects_mismatched_i105_discriminant() {
    let account = fixture_account(0x6B);
    let encoded = account
        .to_i105_for_discriminant(369)
        .expect("encode i105 with taira discriminant");

    let err = parse_account_id_arg_with_prefix(
        "--authority",
        &encoded,
        "sorafs_cli manifest submit",
        Some(753),
    )
    .expect_err("mismatched discriminant should fail");

    assert!(
        err.contains("ERR_UNEXPECTED_NETWORK_PREFIX"),
        "unexpected error: {err}"
    );
}

#[test]
fn authority_payload_literal_preserves_explicit_i105_discriminant() {
    let _guard = iroha_data_model::account::address::ChainDiscriminantGuard::enter(753);
    let account = fixture_account(0x7C);
    let expected = account
        .to_i105_for_discriminant(369)
        .expect("encode taira authority");

    let literal = authority_payload_literal(&account, Some(369)).expect("render authority payload");

    assert_eq!(literal, expected);
}

#[test]
fn build_pin_register_transaction_signs_exact_native_instruction_locally() {
    let manifest = sample_manifest();
    let key_pair = fixture_keypair(0x9E);
    let authority = AccountId::new(key_pair.public_key().clone());
    let network_id = NetworkId::from_genesis_hash(iroha_crypto::HashOf::<
        iroha_data_model::block::BlockHeader,
    >::from_untyped_unchecked(
        iroha_crypto::Hash::prehashed([0x9A; iroha_crypto::Hash::LENGTH]),
    ));

    let transaction = build_pin_register_transaction(
        &network_id,
        &authority,
        key_pair.private_key(),
        &manifest,
        None,
        None,
    )
    .expect("build signed transaction");

    transaction.verify_signature().expect("signature verifies");
    assert_eq!(transaction.network_id(), Some(&network_id));
    assert_eq!(transaction.authority(), &authority);
    let iroha_data_model::transaction::Executable::Instructions(instructions) =
        transaction.instructions()
    else {
        panic!("pin registration must use native instructions");
    };
    let [instruction] = instructions.as_ref() else {
        panic!("pin registration must contain exactly one instruction");
    };
    let register = instruction
        .as_any()
        .downcast_ref::<RegisterPinManifest>()
        .expect("RegisterPinManifest");
    assert_eq!(
        register.manifest_payload,
        manifest.encode().expect("canonical manifest payload")
    );
}

#[test]
fn proposal_summary_contains_register_instruction() {
    let manifest = sample_manifest();
    let digest = manifest.digest().expect("digest");
    let summary = build_manifest_proposal_summary(ManifestProposalSummary {
        manifest_path: Path::new("/tmp/manifest.to"),
        manifest: &manifest,
        manifest_digest: &digest,
        chunk_digest_sha3: [0xCD; 32],
        chunk_plan_label: Some("plan.json"),
        alias_hint: Some("docs.sora.link"),
        successor_bytes: None,
    })
    .expect("proposal summary");

    assert_eq!(summary["proposal_version"], Value::from(1_u64));
    assert_eq!(
        summary["manifest_digest_hex"].as_str().expect("digest hex"),
        hex_encode(digest.as_bytes())
    );
    assert_eq!(
        summary["chunk_plan_source"].as_str().expect("plan label"),
        "plan.json"
    );
    assert_eq!(
        summary["alias_hint"].as_str().expect("alias hint"),
        "docs.sora.link"
    );
    assert!(
        summary
            .get("register_instruction")
            .expect("register instruction")
            .is_object(),
        "register instruction serialized as object"
    );
    assert!(summary.get("submitted_epoch").is_none());
    assert!(
        summary["register_instruction"]
            .get("submitted_epoch")
            .is_none()
    );
}

#[test]
fn governance_payload_kind_cli_labels_external_payload() {
    use sorafs_manifest::GovernanceExternalPayloadV1;

    let encoded_payload = b"external moderation evidence".to_vec();
    let payload = GovernanceLogPayloadV1::ExternalPayload(GovernanceExternalPayloadV1 {
        version: 1,
        payload_kind: "moderation_external_evidence".to_string(),
        payload_version: 1,
        encoded_blake3: *blake3_hash(&encoded_payload).as_bytes(),
        encoded_len: encoded_payload.len() as u64,
        encoded_payload,
        metadata: Vec::new(),
    });

    assert_eq!(governance_payload_kind_cli(&payload), "external_payload");
}

#[test]
fn storage_class_conversion_matches_variants() {
    assert!(matches!(
        convert_storage_class(&ManifestStorageClass::Hot),
        RegistryStorageClass::Hot
    ));
    assert!(matches!(
        convert_storage_class(&ManifestStorageClass::Warm),
        RegistryStorageClass::Warm
    ));
    assert!(matches!(
        convert_storage_class(&ManifestStorageClass::Cold),
        RegistryStorageClass::Cold
    ));
}

#[test]
fn proof_stream_evidence_helper_writes_bundle() {
    let temp = tempdir().expect("tempdir");
    let root = temp.path().canonicalize().expect("canonical tempdir");
    let manifest_path = root.join("sample_manifest.norito");
    fs::write(&manifest_path, b"norito-data").expect("write manifest");
    let evidence_dir = root.join("evidence");
    let summary_json = r#"{"proof_kind":"por"}"#;

    write_proof_stream_evidence(
        &evidence_dir,
        &manifest_path,
        b"norito-data",
        "deadbeef",
        summary_json,
        "https://torii.sora",
    )
    .expect("evidence bundle");

    let summary_path = evidence_dir.join("proof_stream_summary.json");
    assert!(summary_path.exists(), "summary file created");
    let written_summary = fs::read_to_string(&summary_path).expect("read summary");
    assert!(
        written_summary.contains("\"proof_kind\""),
        "summary data preserved"
    );

    let metadata_path = evidence_dir.join("metadata.json");
    let metadata_bytes = fs::read(&metadata_path).expect("read metadata");
    let metadata_value: Value = norito::json::from_slice(&metadata_bytes).expect("metadata json");
    assert_eq!(
        metadata_value["manifest_digest_hex"],
        Value::from("deadbeef")
    );
    assert_eq!(
        metadata_value["manifest_copy"],
        Value::from("sample_manifest.norito")
    );
    let manifest_copy_path = evidence_dir.join("sample_manifest.norito");
    assert_eq!(
        fs::read(&manifest_copy_path).expect("read copied manifest"),
        b"norito-data"
    );
}

#[test]
fn gateway_scoreboard_metadata_records_telemetry_source() {
    let metadata = build_gateway_scoreboard_metadata(&GatewayScoreboardMetadataInput {
        provider_counts: GatewayProviderCounts::new(0, 2),
        max_peers: Some(3),
        retry_budget: None,
        manifest_envelope_present: true,
        gateway_manifest_id: Some("feedface"),
        gateway_manifest_cid: None,
        transport_policy: Some(TransportPolicy::SoranetPreferred),
        transport_policy_override: None,
        anonymity_policy: Some(AnonymityPolicy::GuardPq),
        anonymity_policy_override: None,
        write_mode: WriteModeHint::ReadOnly,
        scoreboard_now: Some(1_234),
        telemetry_source: Some("otel::ci"),
    });
    let map = metadata.as_object().expect("metadata object");
    assert_eq!(
        map.get("telemetry_source").and_then(Value::as_str),
        Some("otel::ci")
    );
    assert_eq!(
        map.get("provider_mix").and_then(Value::as_str),
        Some("gateway-only")
    );
    assert_eq!(
        map.get("write_mode").and_then(Value::as_str),
        Some("read-only")
    );
    assert_eq!(
        map.get("write_mode_enforces_pq").and_then(Value::as_bool),
        Some(false)
    );
}

#[test]
fn gateway_provider_spec_requires_and_normalizes_public_key() {
    let provider_id = "AB".repeat(32);
    let gateway_key = "CD".repeat(32);
    let spec = parse_gateway_provider_spec(&format!(
        "name=alpha,provider-id={provider_id},gateway-key={gateway_key},base-url=https://alpha.example/,stream-token=dG9rZW4="
    ))
    .expect("valid gateway provider specification");

    assert_eq!(spec.provider_id_hex, provider_id.to_ascii_lowercase());
    assert_eq!(
        spec.gateway_public_key_hex,
        gateway_key.to_ascii_lowercase()
    );

    let missing_key = parse_gateway_provider_spec(&format!(
        "name=alpha,provider-id={provider_id},base-url=https://alpha.example/,stream-token=dG9rZW4="
    ))
    .expect_err("gateway public key is required");
    assert!(missing_key.contains("requires a `gateway-key=` entry"));

    let invalid_key = parse_gateway_provider_spec(&format!(
        "name=alpha,provider-id={provider_id},gateway-key=not-hex,base-url=https://alpha.example/,stream-token=dG9rZW4="
    ))
    .expect_err("gateway public key must be canonical hex");
    assert!(invalid_key.contains("gateway-key must be 32-byte hex"));
}

#[test]
fn fetch_summary_records_write_mode_hint() {
    let outcome = sorafs_car::multi_fetch::FetchOutcome {
        chunks: Vec::new(),
        chunk_receipts: Vec::new(),
        provider_reports: Vec::new(),
    };
    let policy_report = PolicyReport {
        policy: AnonymityPolicy::StrictPq,
        effective_policy: AnonymityPolicy::StrictPq,
        total_candidates: 1,
        pq_candidates: 1,
        selected_soranet_total: 1,
        selected_pq: 1,
        status: PolicyStatus::Met,
        fallback_reason: None,
    };
    let plan = CarBuildPlan {
        chunk_profile: ChunkProfile::DEFAULT,
        payload_digest: blake3_hash(&[]),
        content_length: 0,
        chunks: Vec::new(),
        files: Vec::new(),
    };
    let session = FetchSession {
        outcome,
        policy_report,
        local_proxy_manifest: None,
        car_verification: None,
        taikai_cache_stats: None,
        taikai_cache_queue: None,
    };
    let summary = build_fetch_summary(
        "deadbeef",
        "sorafs.sf1@1.0.0",
        &plan,
        &session,
        FetchSummaryOptions {
            client_id: None,
            rollout_phase: RolloutPhase::Default,
            write_mode: WriteModeHint::UploadPqOnly,
            cache_profile: None,
        },
    );
    let map = summary.as_object().expect("summary object");
    assert_eq!(
        map.get("write_mode").and_then(Value::as_str),
        Some("upload-pq-only")
    );
    assert_eq!(
        map.get("write_mode_enforces_pq").and_then(Value::as_bool),
        Some(true)
    );
}

#[test]
fn insert_telemetry_source_injects_label() {
    let mut summary = Value::Object(Map::new());
    insert_telemetry_source(&mut summary, Some("otel::staging"));
    let object = summary.as_object().expect("summary object");
    assert_eq!(
        object.get("telemetry_source").and_then(Value::as_str),
        Some("otel::staging")
    );

    insert_telemetry_source(&mut summary, None);
    let object = summary.as_object().expect("summary object");
    assert_eq!(
        object.get("telemetry_source").and_then(Value::as_str),
        Some("otel::staging"),
        "missing label should not remove existing value"
    );
}
