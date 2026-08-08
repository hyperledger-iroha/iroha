// Fetch security and Taikai bundle CLI regressions.

#[test]
fn fetch_command_config_does_not_bypass_gateway_url_security() {
    let tempdir = tempdir().expect("tempdir");
    let payload: Vec<u8> = (0..1024).map(|idx| (idx % 151) as u8).collect();
    let plan = CarBuildPlan::single_file(&payload).expect("plan");
    let plan_json = chunk_fetch_plan_to_string(&plan).expect("plan json") + "\n";
    let plan_path = tempdir.path().join("plan.json");
    fs::write(&plan_path, plan_json.as_bytes()).expect("write plan json");

    let writer = CarWriter::new(&plan, &payload).expect("writer");
    let car_stats = writer.write_to(std::io::sink()).expect("write car stats");

    let manifest = ManifestBuilder::new()
        .root_cid(car_stats.root_cids[0].clone())
        .dag_codec(DagCodecId(car_stats.dag_codec))
        .chunking_from_profile(plan.chunk_profile, chunker_registry::DEFAULT_MULTIHASH_CODE)
        .chunk_digest_sha3_256(compute_chunk_plan_digest_sha3(&plan.chunks))
        .por_root(compute_por_root(&payload, &plan).expect("derive canonical fixture PoR root"))
        .content_length(plan.content_length)
        .car_digest(car_stats.car_archive_digest.into())
        .car_size(car_stats.car_size)
        .pin_policy(PinPolicy {
            min_replicas: 1,
            storage_class: StorageClass::Hot,
            retention_epoch: 10,
        })
        .governance(council_signed_governance_proofs())
        .build()
        .expect("manifest");
    let manifest_bytes = to_bytes(&manifest).expect("manifest bytes");
    let manifest_digest_hex = hex_encode(manifest.digest().expect("manifest digest").as_bytes());
    let manifest_id_hex = manifest_digest_hex.clone();
    let payload_digest_hex = hex_encode(plan.payload_digest.as_bytes());
    let chunk_profile_handle = "sorafs.sf1@1.0.0";

    let manifest_report_path = tempdir.path().join("direct_manifest_report.json");
    let manifest_response = format!(
        "{{\"manifest_id_hex\":\"{}\",\"manifest_b64\":\"{}\",\"manifest_digest_hex\":\"{}\",\"payload_digest_hex\":\"{}\",\"content_length\":{},\"chunk_count\":{},\"chunk_profile_handle\":\"{}\",\"stored_at_unix_secs\":1735000000}}",
        manifest_id_hex,
        BASE64_STANDARD.encode(&manifest_bytes),
        manifest_digest_hex,
        payload_digest_hex,
        plan.content_length,
        plan.chunks.len(),
        chunk_profile_handle
    );
    fs::write(
        &manifest_report_path,
        format!("{}\n", manifest_response).as_bytes(),
    )
    .expect("write manifest report");

    let server = MockServer::start();
    let manifest_path = format!("/v1/sorafs/storage/manifest/{manifest_id_hex}");
    server.mock(|when, then| {
        when.method(GET).path(manifest_path.as_str());
        then.status(200).body(manifest_response.clone());
    });
    for spec in plan.try_chunk_fetch_specs().expect("valid CAR plan") {
        let path = format!(
            "/v1/sorafs/storage/chunk/{}/{}",
            manifest_id_hex,
            hex_encode(spec.digest)
        );
        let start = spec.offset as usize;
        let end = start + spec.length as usize;
        let body = payload[start..end].to_vec();
        server.mock(move |when, then| {
            when.method(GET).path(path.as_str());
            then.status(200).body(body.clone());
        });
    }
    let provider_id_hex = "34".repeat(32);
    let (stream_token_b64, gateway_public_key_hex) =
        make_stream_token_b64(&manifest_id_hex, &provider_id_hex, "sorafs.sf1@1.0.0", 3);
    let summary_path = tempdir.path().join("policy_fetch_summary.json");
    let output_path = tempdir.path().join("policy_payload.bin");
    let scoreboard_path = tempdir
        .path()
        .join("scoreboards/direct_policy_scoreboard.json");
    let policy_path = tempdir.path().join("direct_policy.json");

    let mut scoreboard = Map::new();
    scoreboard.insert("latency_cap_ms".into(), Value::from(3500u64));
    scoreboard.insert("weight_scale".into(), Value::from(200u64));
    scoreboard.insert("telemetry_grace_secs".into(), Value::from(45u64));
    scoreboard.insert(
        "persist_path".into(),
        Value::from(scoreboard_path.display().to_string()),
    );
    scoreboard.insert("now_unix_secs".into(), Value::from(1_700_000_000u64));

    let mut fetch = Map::new();
    fetch.insert("retry_budget".into(), Value::from(4u64));
    fetch.insert("provider_failure_threshold".into(), Value::from(2u64));
    fetch.insert("global_parallel_limit".into(), Value::from(1u64));
    fetch.insert("verify_lengths".into(), Value::from(true));
    fetch.insert("verify_digests".into(), Value::from(true));

    let mut root = Map::new();
    root.insert("scoreboard".into(), Value::Object(scoreboard));
    root.insert("fetch".into(), Value::Object(fetch));
    root.insert("telemetry_region".into(), Value::from("regulated-eu"));
    root.insert("max_providers".into(), Value::from(1u64));
    root.insert("transport_policy".into(), Value::from("direct-only"));

    let rendered = norito::json::to_string_pretty(&Value::Object(root))
        .expect("render orchestrator config json");
    fs::write(&policy_path, rendered.as_bytes()).expect("write orchestrator config json");
    let base_url = server.url("/");

    let assert = sorafs_cli_cmd()
        .arg("fetch")
        .arg(format!("--plan={}", plan_path.display()))
        .arg(format!("--manifest-id={manifest_id_hex}"))
        .arg(format!("--manifest-report={}", manifest_report_path.display()))
        .arg(format!(
            "--provider=name=policy-gw,provider-id={provider_id_hex},gateway-key={gateway_public_key_hex},base-url={base_url},stream-token={stream_token_b64}",
        ))
        .arg(format!("--orchestrator-config={}", policy_path.display()))
        .arg(format!("--json-out={}", summary_path.display()))
        .arg(format!("--output={}", output_path.display()))
        .assert();
    if base_url.starts_with("http://") {
        assert_insecure_gateway_rejected(assert, &[&output_path, &summary_path, &scoreboard_path]);
        return;
    }
    let assert = assert.success();

    let summary_value: Value =
        norito::json::from_slice(assert.get_output().stdout.as_slice()).expect("stdout summary");
    assert_eq!(
        summary_value
            .get("telemetry_region")
            .and_then(Value::as_str),
        Some("regulated-eu")
    );

    let summary_bytes = fs::read(&summary_path).expect("read summary file");
    let summary_file: Value =
        norito::json::from_slice(&summary_bytes).expect("parse summary file json");
    assert_eq!(
        summary_file.get("telemetry_region").and_then(Value::as_str),
        Some("regulated-eu")
    );

    let assembled = fs::read(&output_path).expect("read assembled payload");
    assert_eq!(assembled, payload);

    let scoreboard_bytes = fs::read(&scoreboard_path).expect("persisted scoreboard json");
    let persisted_scoreboard: Value =
        norito::json::from_slice(&scoreboard_bytes).expect("parse scoreboard json");
    let providers = persisted_scoreboard
        .get("entries")
        .and_then(Value::as_array)
        .expect("persisted scoreboard entries array");
    assert_eq!(providers.len(), 1);
    assert_eq!(
        providers[0]
            .as_object()
            .and_then(|obj| obj.get("provider_id"))
            .and_then(Value::as_str),
        Some("policy-gw")
    );
}

#[test]
fn fetch_command_scoreboard_flag_does_not_bypass_gateway_url_security() {
    let tempdir = tempdir().expect("tempdir");
    let payload: Vec<u8> = (0..2048).map(|idx| (idx % 179) as u8).collect();
    let plan = CarBuildPlan::single_file(&payload).expect("plan");
    let plan_json = chunk_fetch_plan_to_string(&plan).expect("plan json") + "\n";
    let plan_path = tempdir.path().join("plan.json");
    fs::write(&plan_path, plan_json.as_bytes()).expect("write plan json");

    let writer = CarWriter::new(&plan, &payload).expect("writer");
    let car_stats = writer.write_to(std::io::sink()).expect("write car stats");

    let manifest = ManifestBuilder::new()
        .root_cid(car_stats.root_cids[0].clone())
        .dag_codec(DagCodecId(car_stats.dag_codec))
        .chunking_from_profile(plan.chunk_profile, chunker_registry::DEFAULT_MULTIHASH_CODE)
        .chunk_digest_sha3_256(compute_chunk_plan_digest_sha3(&plan.chunks))
        .por_root(compute_por_root(&payload, &plan).expect("derive canonical fixture PoR root"))
        .content_length(plan.content_length)
        .car_digest(car_stats.car_archive_digest.into())
        .car_size(car_stats.car_size)
        .pin_policy(PinPolicy {
            min_replicas: 1,
            storage_class: StorageClass::Hot,
            retention_epoch: 10,
        })
        .governance(council_signed_governance_proofs())
        .build()
        .expect("manifest");
    let manifest_bytes = to_bytes(&manifest).expect("manifest bytes");
    let manifest_digest_hex = hex_encode(manifest.digest().expect("manifest digest").as_bytes());
    let manifest_id_hex = manifest_digest_hex.clone();
    let chunk_profile_handle = "sorafs.sf1@1.0.0";

    let manifest_report_path = tempdir.path().join("flag_manifest_report.json");
    let manifest_response = format!(
        "{{\"manifest_id_hex\":\"{manifest_id_hex}\",\"manifest_b64\":\"{}\",\"manifest_digest_hex\":\"{manifest_digest_hex}\",\"payload_digest_hex\":\"{}\",\"content_length\":{},\"chunk_count\":{},\"chunk_profile_handle\":\"{}\",\"stored_at_unix_secs\":1735000000}}",
        BASE64_STANDARD.encode(&manifest_bytes),
        hex_encode(plan.payload_digest.as_bytes()),
        plan.content_length,
        plan.chunks.len(),
        chunk_profile_handle
    );
    fs::write(
        &manifest_report_path,
        format!("{manifest_response}\n").as_bytes(),
    )
    .expect("write manifest report");

    let server = MockServer::start();
    let manifest_path = format!("/v1/sorafs/storage/manifest/{manifest_id_hex}");
    server.mock(|when, then| {
        when.method(GET).path(manifest_path.as_str());
        then.status(200).body(manifest_response.clone());
    });
    for spec in plan.try_chunk_fetch_specs().expect("valid CAR plan") {
        let path = format!(
            "/v1/sorafs/storage/chunk/{}/{}",
            manifest_id_hex,
            hex_encode(spec.digest)
        );
        let start = spec.offset as usize;
        let end = start + spec.length as usize;
        let body = payload[start..end].to_vec();
        server.mock(move |when, then| {
            when.method(GET).path(path.as_str());
            then.status(200).body(body.clone());
        });
    }
    let provider_id_hex = "56".repeat(32);
    let (stream_token_b64, gateway_public_key_hex) =
        make_stream_token_b64(&manifest_id_hex, &provider_id_hex, chunk_profile_handle, 2);

    let summary_path = tempdir.path().join("flag_summary.json");
    let output_path = tempdir.path().join("flag_payload.bin");
    let scoreboard_path = tempdir
        .path()
        .join("scoreboards/flag_fetch_scoreboard.json");
    let base_url = server.url("/");

    let assert = sorafs_cli_cmd()
        .arg("fetch")
        .arg(format!("--plan={}", plan_path.display()))
        .arg(format!("--manifest-id={manifest_id_hex}"))
        .arg(format!(
            "--provider=name=flag-gw,provider-id={provider_id_hex},gateway-key={gateway_public_key_hex},base-url={base_url},stream-token={stream_token_b64}",
        ))
        .arg(format!(
            "--manifest-report={}",
            manifest_report_path.display()
        ))
        .arg(format!("--json-out={}", summary_path.display()))
        .arg(format!("--output={}", output_path.display()))
        .arg(format!("--scoreboard-out={}", scoreboard_path.display()))
        .arg("--scoreboard-now=1700000000")
        .assert();
    if base_url.starts_with("http://") {
        assert_insecure_gateway_rejected(assert, &[&output_path, &summary_path, &scoreboard_path]);
        return;
    }
    assert.success();

    let scoreboard_bytes = fs::read(&scoreboard_path).expect("read scoreboard file");
    let scoreboard_value: Value =
        norito::json::from_slice(&scoreboard_bytes).expect("parse scoreboard json");
    let entries = scoreboard_value
        .get("entries")
        .and_then(Value::as_array)
        .expect("entries array");
    assert!(
        !entries.is_empty(),
        "scoreboard entries should not be empty"
    );
    let first = entries[0]
        .get("provider_id")
        .and_then(Value::as_str)
        .expect("provider id");
    assert!(
        !first.is_empty(),
        "provider id should be recorded in scoreboard"
    );
}

#[cfg(not(feature = "local-quic-proxy"))]
#[test]
fn fetch_command_rejects_insecure_gateway_before_proxy_startup_without_runtime_feature() {
    let tempdir = tempdir().expect("tempdir");
    let payload: Vec<u8> = (0..64).map(|idx| idx as u8).collect();
    let plan = CarBuildPlan::single_file(&payload).expect("plan");
    let plan_json = chunk_fetch_plan_to_string(&plan).expect("plan json") + "\n";
    let plan_path = tempdir.path().join("plan.json");
    fs::write(&plan_path, plan_json.as_bytes()).expect("write plan json");

    let manifest_id_hex = hex_encode(blake3_hash(&payload).as_bytes());
    let provider_id_hex = "cd".repeat(32);
    let (stream_token_b64, gateway_public_key_hex) =
        make_stream_token_b64(&manifest_id_hex, &provider_id_hex, "sorafs.sf1@1.0.0", 2);
    let policy_path = tempdir.path().join("proxy_config.json");
    let manifest_out_path = tempdir.path().join("proxy_manifest.json");

    let mut local_proxy = Map::new();
    local_proxy.insert("bind_addr".into(), Value::from("127.0.0.1:0"));
    local_proxy.insert("telemetry_label".into(), Value::from("test-proxy"));
    local_proxy.insert("proxy_mode".into(), Value::from("bridge"));
    local_proxy.insert("emit_browser_manifest".into(), Value::from(true));

    let mut root = Map::new();
    root.insert("local_proxy".into(), Value::Object(local_proxy));

    let rendered =
        norito::json::to_string_pretty(&Value::Object(root)).expect("render orchestrator config");
    fs::write(&policy_path, rendered.as_bytes()).expect("write orchestrator config");

    let output = sorafs_cli_cmd()
        .arg("fetch")
        .arg(format!("--plan={}", plan_path.display()))
        .arg(format!("--manifest-id={manifest_id_hex}"))
        .arg(format!(
            "--provider=name=proxy-gw,provider-id={provider_id_hex},gateway-key={gateway_public_key_hex},base-url=http://127.0.0.1:9/,stream-token={stream_token_b64}",
        ))
        .arg(format!("--orchestrator-config={}", policy_path.display()))
        .arg(format!(
            "--local-proxy-manifest-out={}",
            manifest_out_path.display()
        ))
        .output()
        .expect("command executes");

    assert!(
        !output.status.success(),
        "command should fail before starting the proxy"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("URL must use HTTPS") || stderr.contains("globally routable"),
        "stderr should report the unsafe gateway boundary: {stderr}"
    );
    assert!(
        !manifest_out_path.exists(),
        "no proxy manifest should be written when runtime support is unavailable"
    );
}

#[cfg(feature = "local-quic-proxy")]
#[test]
fn fetch_command_proxy_does_not_bypass_gateway_url_security() {
    let tempdir = tempdir().expect("tempdir");
    let payload: Vec<u8> = (0..512).map(|idx| (idx % 97) as u8).collect();
    let plan = CarBuildPlan::single_file(&payload).expect("plan");
    let plan_json = chunk_fetch_plan_to_string(&plan).expect("plan json") + "\n";
    let plan_path = tempdir.path().join("plan.json");
    fs::write(&plan_path, plan_json.as_bytes()).expect("write plan json");

    let writer = CarWriter::new(&plan, &payload).expect("writer");
    let car_stats = writer.write_to(std::io::sink()).expect("write car stats");

    let manifest = ManifestBuilder::new()
        .root_cid(car_stats.root_cids[0].clone())
        .dag_codec(DagCodecId(car_stats.dag_codec))
        .chunking_from_profile(plan.chunk_profile, chunker_registry::DEFAULT_MULTIHASH_CODE)
        .chunk_digest_sha3_256(compute_chunk_plan_digest_sha3(&plan.chunks))
        .por_root(compute_por_root(&payload, &plan).expect("derive canonical fixture PoR root"))
        .content_length(plan.content_length)
        .car_digest(car_stats.car_archive_digest.into())
        .car_size(car_stats.car_size)
        .pin_policy(PinPolicy {
            min_replicas: 1,
            storage_class: StorageClass::Hot,
            retention_epoch: 10,
        })
        .governance(council_signed_governance_proofs())
        .build()
        .expect("manifest");
    let manifest_bytes = to_bytes(&manifest).expect("manifest bytes");
    let manifest_digest_hex = hex_encode(manifest.digest().expect("manifest digest").as_bytes());
    let manifest_id_hex = manifest_digest_hex.clone();
    let payload_digest_hex = hex_encode(plan.payload_digest.as_bytes());
    let chunk_profile_handle = "sorafs.sf1@1.0.0";

    let manifest_report_path = tempdir.path().join("proxy_manifest_report.json");
    let manifest_response = format!(
        "{{\"manifest_id_hex\":\"{}\",\"manifest_b64\":\"{}\",\"manifest_digest_hex\":\"{}\",\"payload_digest_hex\":\"{}\",\"content_length\":{},\"chunk_count\":{},\"chunk_profile_handle\":\"{}\",\"stored_at_unix_secs\":1735000000}}",
        manifest_id_hex,
        BASE64_STANDARD.encode(&manifest_bytes),
        manifest_digest_hex,
        payload_digest_hex,
        plan.content_length,
        plan.chunks.len(),
        chunk_profile_handle
    );
    fs::write(
        &manifest_report_path,
        format!("{}\n", manifest_response).as_bytes(),
    )
    .expect("write manifest report");

    let server = MockServer::start();
    for spec in plan.try_chunk_fetch_specs().expect("valid CAR plan") {
        let path = format!(
            "/v1/sorafs/storage/chunk/{}/{}",
            manifest_id_hex,
            hex_encode(spec.digest)
        );
        let start = spec.offset as usize;
        let end = start + spec.length as usize;
        let body = payload[start..end].to_vec();
        server.mock(move |when, then| {
            when.method(GET).path(path.as_str());
            then.status(200).body(body.clone());
        });
    }

    let provider_id_hex = "ab".repeat(32);
    let (stream_token_b64, gateway_public_key_hex) =
        make_stream_token_b64(&manifest_id_hex, &provider_id_hex, "sorafs.sf1@1.0.0", 2);
    let summary_path = tempdir.path().join("proxy_fetch_summary.json");
    let manifest_out_path = tempdir.path().join("proxy_manifest.json");
    let policy_path = tempdir.path().join("proxy_config.json");

    let mut scoreboard = Map::new();
    scoreboard.insert("latency_cap_ms".into(), Value::from(3000u64));
    scoreboard.insert("weight_scale".into(), Value::from(100u64));
    scoreboard.insert("telemetry_grace_secs".into(), Value::from(30u64));
    scoreboard.insert("now_unix_secs".into(), Value::from(1_701_000_000u64));

    let mut fetch = Map::new();
    fetch.insert("retry_budget".into(), Value::from(3u64));
    fetch.insert("provider_failure_threshold".into(), Value::from(2u64));
    fetch.insert("global_parallel_limit".into(), Value::from(2u64));
    fetch.insert("verify_lengths".into(), Value::from(true));
    fetch.insert("verify_digests".into(), Value::from(true));

    let mut local_proxy = Map::new();
    local_proxy.insert("bind_addr".into(), Value::from("127.0.0.1:0"));
    local_proxy.insert("telemetry_label".into(), Value::from("test-proxy"));
    local_proxy.insert("proxy_mode".into(), Value::from("bridge"));
    local_proxy.insert("emit_browser_manifest".into(), Value::from(true));

    let mut root = Map::new();
    root.insert("scoreboard".into(), Value::Object(scoreboard));
    root.insert("fetch".into(), Value::Object(fetch));
    root.insert("local_proxy".into(), Value::Object(local_proxy));

    let rendered =
        norito::json::to_string_pretty(&Value::Object(root)).expect("render orchestrator config");
    fs::write(&policy_path, rendered.as_bytes()).expect("write orchestrator config");
    let base_url = server.url("/");

    let assert = sorafs_cli_cmd()
        .arg("fetch")
        .arg(format!("--plan={}", plan_path.display()))
        .arg(format!("--manifest-id={manifest_id_hex}"))
        .arg(format!("--manifest-report={}", manifest_report_path.display()))
        .arg(format!(
            "--provider=name=proxy-gw,provider-id={provider_id_hex},gateway-key={gateway_public_key_hex},base-url={base_url},stream-token={stream_token_b64}",
        ))
        .arg(format!("--orchestrator-config={}", policy_path.display()))
        .arg(format!("--json-out={}", summary_path.display()))
        .arg(format!(
            "--local-proxy-manifest-out={}",
            manifest_out_path.display()
        ))
        .assert();
    if base_url.starts_with("http://") {
        assert_insecure_gateway_rejected(assert, &[&summary_path, &manifest_out_path]);
        return;
    }
    assert.success();

    let summary_bytes = fs::read(&summary_path).expect("read summary json");
    let summary_value: Value =
        from_slice(&summary_bytes).expect("summary json must parse into Value");
    let manifest_from_summary = summary_value
        .get("local_proxy_manifest")
        .expect("summary should include proxy manifest")
        .clone();
    let summary_mode = summary_value
        .get("local_proxy_mode")
        .and_then(Value::as_str)
        .expect("summary.local_proxy_mode");
    assert_eq!(summary_mode, "bridge");
    let summary_spool = summary_value
        .get("local_proxy_norito_spool")
        .and_then(Value::as_str)
        .expect("summary.local_proxy_norito_spool");
    assert_eq!(summary_spool, PROVISION_SPOOL_DIR);
    let summary_kaigi_spool = summary_value
        .get("local_proxy_kaigi_spool")
        .and_then(Value::as_str)
        .expect("summary.local_proxy_kaigi_spool");
    assert_eq!(summary_kaigi_spool, PROVISION_SPOOL_DIR);
    let summary_kaigi_policy = summary_value
        .get("local_proxy_kaigi_policy")
        .and_then(Value::as_str)
        .expect("summary.local_proxy_kaigi_policy");
    assert_eq!(summary_kaigi_policy, "public");

    let manifest_bytes = fs::read(&manifest_out_path).expect("read manifest json");
    let manifest_value: Value =
        from_slice(&manifest_bytes).expect("manifest json should parse into Value");
    assert_eq!(
        manifest_value, manifest_from_summary,
        "manifest exported to disk should match summary"
    );

    let authority = manifest_value
        .get("authority")
        .and_then(Value::as_str)
        .expect("manifest authority");
    assert!(
        authority.starts_with("127.0.0.1:"),
        "authority `{authority}` should bind to loopback"
    );
    let proxy_mode = manifest_value
        .get("proxy_mode")
        .and_then(Value::as_str)
        .expect("proxy_mode");
    assert_eq!(proxy_mode, "bridge");
    let cert_pem = manifest_value
        .get("certificate_pem")
        .and_then(Value::as_str)
        .expect("certificate_pem");
    assert!(
        cert_pem.contains("BEGIN CERTIFICATE"),
        "manifest should contain embedded PEM certificate"
    );
    let salt_hex = manifest_value
        .get("cache_tagging")
        .and_then(Value::as_object)
        .and_then(|obj| obj.get("salt_hex"))
        .and_then(Value::as_str)
        .expect("cache_tagging.salt_hex");
    assert_eq!(salt_hex.len(), 32, "salt_hex must be 16 bytes encoded");

    let summary_override_path = tempdir.path().join("proxy_fetch_summary_override.json");
    let manifest_override_path = tempdir.path().join("proxy_manifest_override.json");
    sorafs_cli_cmd()
        .arg("fetch")
        .arg(format!("--plan={}", plan_path.display()))
        .arg(format!("--manifest-id={manifest_id_hex}"))
        .arg(format!(
            "--provider=name=proxy-gw,provider-id={provider_id_hex},gateway-key={gateway_public_key_hex},base-url={},stream-token={stream_token_b64}",
            server.url("/")
        ))
        .arg(format!("--orchestrator-config={}", policy_path.display()))
        .arg(format!("--json-out={}", summary_override_path.display()))
        .arg(format!(
            "--local-proxy-manifest-out={}",
            manifest_override_path.display()
        ))
        .arg("--local-proxy-mode=metadata-only")
        .assert()
        .success();

    let summary_override_bytes =
        fs::read(&summary_override_path).expect("read override summary json");
    let summary_override: Value =
        from_slice(&summary_override_bytes).expect("override summary json must parse");
    assert_eq!(
        summary_override
            .get("local_proxy_mode")
            .and_then(Value::as_str),
        Some("metadata-only")
    );
    assert!(
        summary_override.get("local_proxy_norito_spool").is_none(),
        "metadata-only overrides should not advertise a spool directory"
    );
    let manifest_override_bytes =
        fs::read(&manifest_override_path).expect("read override manifest json");
    let manifest_override: Value =
        from_slice(&manifest_override_bytes).expect("override manifest json should parse");
    assert_eq!(
        manifest_override.get("proxy_mode").and_then(Value::as_str),
        Some("metadata-only")
    );
}

#[test]
fn sorafs_cli_taikai_bundle_generates_artifacts() {
    let dir = tempdir().expect("tempdir");
    let payload_path = dir.path().join("segment_bundle.bin");
    fs::write(&payload_path, b"bundle-me").expect("write payload");
    let car_path = dir.path().join("segment_bundle.car");
    let envelope_path = dir.path().join("segment_bundle.to");
    let indexes_path = dir.path().join("segment_bundle.index.json");
    let ingest_path = dir.path().join("segment_bundle.ingest.json");
    let summary_path = dir.path().join("segment_bundle.summary.json");

    let mut cmd = sorafs_cli_cmd();
    cmd.arg("taikai")
        .arg("bundle")
        .arg(format!("--payload={}", payload_path.display()))
        .arg(format!("--car-out={}", car_path.display()))
        .arg(format!("--envelope-out={}", envelope_path.display()))
        .arg(format!("--indexes-out={}", indexes_path.display()))
        .arg(format!("--ingest-metadata-out={}", ingest_path.display()))
        .arg(format!("--summary-out={}", summary_path.display()))
        .arg("--event-id=demo-event")
        .arg("--stream-id=stage-b")
        .arg("--rendition-id=720p")
        .arg("--track-kind=video")
        .arg("--codec=avc-high")
        .arg("--bitrate-kbps=4500")
        .arg("--resolution=1280x720")
        .arg("--segment-sequence=7")
        .arg("--segment-start-pts=700000")
        .arg("--segment-duration=1000000")
        .arg("--wallclock-unix-ms=1702561000000")
        .arg(format!("--manifest-hash={}", "33".repeat(32)))
        .arg(format!("--storage-ticket={}", "44".repeat(32)))
        .arg("--ingest-latency-ms=42")
        .arg("--live-edge-drift-ms=-17")
        .arg("--ingest-node-id=node-a");
    cmd.assert().success();

    assert!(car_path.exists(), "car output should exist");
    assert!(envelope_path.exists(), "envelope output should exist");
    assert!(indexes_path.exists(), "indexes output should exist");
    assert!(ingest_path.exists(), "ingest metadata output should exist");
    assert!(summary_path.exists(), "summary output should exist");

    let envelope_bytes = fs::read(&envelope_path).expect("read envelope");
    let envelope: TaikaiSegmentEnvelopeV1 =
        norito::decode_from_bytes(&envelope_bytes).expect("decode envelope");
    assert_eq!(envelope.segment_sequence, 7);
    assert_eq!(
        envelope.instrumentation.encoder_to_ingest_latency_ms,
        Some(42)
    );
    assert_eq!(envelope.instrumentation.live_edge_drift_ms, Some(-17));

    let summary_bytes = fs::read(&summary_path).expect("read summary");
    let summary_json: Value = from_slice(&summary_bytes).expect("summary json");
    assert_eq!(
        summary_json
            .get("ingest")
            .and_then(|ingest| ingest.get("event_id"))
            .and_then(Value::as_str),
        Some("demo-event")
    );
    assert_eq!(
        summary_json
            .get("car")
            .and_then(|car| car.get("cid_multibase"))
            .and_then(Value::as_str)
            .map(|s| s.starts_with('b')),
        Some(true)
    );
}

#[test]
fn sorafs_cli_taikai_bundle_rejects_noncanonical_operator_inputs() {
    let cases = vec![
        (
            "--segment-sequence",
            "07".to_string(),
            "canonical unsigned decimal integer",
        ),
        (
            "--segment-start-pts",
            "700000 ".to_string(),
            "canonical unsigned decimal integer",
        ),
        (
            "--wallclock-unix-ms",
            "+1702561000000".to_string(),
            "canonical unsigned decimal integer",
        ),
        (
            "--live-edge-drift-ms",
            "+17".to_string(),
            "canonical signed decimal integer",
        ),
        (
            "--live-edge-drift-ms",
            "-017".to_string(),
            "canonical signed decimal integer",
        ),
        ("--track-kind", "Video".to_string(), "canonical lowercase"),
        (
            "--manifest-hash",
            format!("0x{}", "33".repeat(32)),
            "hex prefix",
        ),
        ("--manifest-hash", "00".repeat(32), "all zero"),
        (
            "--storage-ticket",
            format!("{}4A", "44".repeat(31)),
            "lowercase hex",
        ),
        ("--bitrate-kbps", "0".to_string(), "greater than zero"),
        ("--segment-duration", "0".to_string(), "greater than zero"),
    ];

    for (flag, value, expected) in cases {
        let dir = tempdir().expect("tempdir");
        let payload_path = dir.path().join("segment_bundle.bin");
        fs::write(&payload_path, b"bundle-me").expect("write payload");
        let car_path = dir.path().join("segment_bundle.car");
        let envelope_path = dir.path().join("segment_bundle.to");
        let indexes_path = dir.path().join("segment_bundle.index.json");
        let ingest_path = dir.path().join("segment_bundle.ingest.json");
        let summary_path = dir.path().join("segment_bundle.summary.json");

        let mut args = vec![
            "taikai".to_string(),
            "bundle".to_string(),
            format!("--payload={}", payload_path.display()),
            format!("--car-out={}", car_path.display()),
            format!("--envelope-out={}", envelope_path.display()),
            format!("--indexes-out={}", indexes_path.display()),
            format!("--ingest-metadata-out={}", ingest_path.display()),
            format!("--summary-out={}", summary_path.display()),
            "--event-id=demo-event".to_string(),
            "--stream-id=stage-b".to_string(),
            "--rendition-id=720p".to_string(),
            "--track-kind=video".to_string(),
            "--codec=avc-high".to_string(),
            "--bitrate-kbps=4500".to_string(),
            "--resolution=1280x720".to_string(),
            "--segment-sequence=7".to_string(),
            "--segment-start-pts=700000".to_string(),
            "--segment-duration=1000000".to_string(),
            "--wallclock-unix-ms=1702561000000".to_string(),
            format!("--manifest-hash={}", "33".repeat(32)),
            format!("--storage-ticket={}", "44".repeat(32)),
            "--ingest-latency-ms=42".to_string(),
            "--live-edge-drift-ms=-17".to_string(),
            "--ingest-node-id=node-a".to_string(),
        ];
        let replacement = format!("{flag}={value}");
        let mut replaced = false;
        for arg in &mut args {
            if arg.starts_with(&format!("{flag}=")) {
                *arg = replacement.clone();
                replaced = true;
            }
        }
        assert!(replaced, "test case flag {flag} must replace a base arg");

        let output = sorafs_cli_cmd()
            .args(args)
            .output()
            .expect("run taikai bundle");
        assert!(
            !output.status.success(),
            "{flag}={value:?} unexpectedly succeeded"
        );
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains(expected),
            "{flag}={value:?} stderr should contain {expected:?}, got: {stderr}"
        );
        for path in [
            &car_path,
            &envelope_path,
            &indexes_path,
            &ingest_path,
            &summary_path,
        ] {
            assert!(
                !path.exists(),
                "{flag}={value:?} must fail before writing {}",
                path.display()
            );
        }
    }
}
