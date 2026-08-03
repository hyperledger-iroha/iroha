// Governance DAG and transparency readback endpoint regressions.

#[test]
fn governance_dag_source_payload_bytes_uses_external_inner_payload() {
    let encoded_payload = vec![0x4e, 0x52, 0x54, 0x31, 0x01, 0x02, 0x03];
    let payload =
        GovernanceLogPayloadV1::ExternalPayload(sorafs_manifest::GovernanceExternalPayloadV1 {
            version: sorafs_manifest::SORAFS_GOVERNANCE_EXTERNAL_PAYLOAD_VERSION_V1,
            payload_kind: "test_external_payload".to_owned(),
            payload_version: 1,
            encoded_blake3: *blake3::hash(&encoded_payload).as_bytes(),
            encoded_len: u64::try_from(encoded_payload.len())
                .expect("external payload length fits u64"),
            encoded_payload: encoded_payload.clone(),
            metadata: Vec::new(),
        });

    assert_eq!(
        governance_dag_source_payload_bytes(&payload)
            .expect("derive external governance source bytes"),
        encoded_payload
    );
}

#[test]
fn governance_dag_raw_ipfs_cid_commits_exact_bytes() {
    assert_eq!(
        governance_dag_raw_ipfs_cid(b"payload"),
        "bafkreibdt5m62vphg7dxcr6pkwwqygydbnwx5z2iu5bgsuxzxbjnlkjv4u"
    );
    assert_ne!(
        governance_dag_raw_ipfs_cid(b"payload-tampered"),
        governance_dag_raw_ipfs_cid(b"payload")
    );
}

#[test]
fn governance_dag_file_backed_gets_are_admitted_and_offloaded() {
    let source = include_str!("../api.rs");
    for handler_name in [
        "handle_get_sorafs_governance_dag_dashboard",
        "handle_get_sorafs_governance_dag_head",
        "handle_get_sorafs_governance_dag_block",
        "handle_get_sorafs_governance_dag_node",
        "handle_get_sorafs_governance_dag_publish_index",
        "handle_get_sorafs_governance_dag_publish_digest",
        "handle_get_sorafs_governance_dag_publish_kind",
        "handle_get_sorafs_transparency_cycles",
        "handle_get_sorafs_transparency_cycle",
        "handle_get_sorafs_transparency_cycle_entry",
        "handle_get_sorafs_transparency_explorer",
        "handle_get_sorafs_transparency_token_issuances",
        "handle_get_sorafs_appeal_finance_reports",
        "handle_get_sorafs_appeal_finance_weekly_rollups",
        "handle_get_sorafs_appeal_finance_settlement_receipts",
        "handle_get_sorafs_governance_dag_car_queue",
        "handle_get_sorafs_governance_dag_car_queue_digest",
        "handle_get_sorafs_governance_dag_car_queue_kind",
        "handle_get_sorafs_governance_dag_car_queue_archive",
        "handle_get_sorafs_governance_dag_runtime",
        "handle_get_sorafs_governance_dag_runtime_head",
        "handle_get_sorafs_governance_dag_runtime_block",
        "handle_get_sorafs_governance_dag_runtime_node",
        "handle_get_sorafs_governance_dag_runtime_digest",
        "handle_get_sorafs_governance_dag_runtime_kind",
    ] {
        let marker = format!("pub(crate) async fn {handler_name}(");
        let start = source
            .find(&marker)
            .unwrap_or_else(|| panic!("missing file-backed handler `{handler_name}`"));
        let tail = &source[start..];
        let end = tail[marker.len()..]
            .find("\npub(crate) async fn ")
            .map_or(tail.len(), |offset| marker.len() + offset);
        assert!(
            tail[..end].contains("governance_dag_blocking_response("),
            "file-backed handler `{handler_name}` must use heavy admitted blocking I/O"
        );
    }

    let helper_start = source
        .find("async fn governance_dag_blocking_response")
        .expect("governance DAG blocking helper");
    let helper_end = source[helper_start..]
        .find("\nfn governance_car_queue_response")
        .map(|offset| helper_start + offset)
        .expect("end of governance DAG blocking helper");
    let helper = &source[helper_start..helper_end];
    assert!(helper.contains("acquire_query_admission(state.as_ref(), true)"));
    assert!(helper.contains("tokio::task::spawn_blocking"));
}

#[test]
fn governance_dag_readback_cannot_reintroduce_path_based_file_reads() {
    let source = include_str!("../api.rs");
    let start = source
        .find("fn load_governance_dag_mirror_index")
        .expect("governance DAG loader region");
    let end = source[start..]
        .find("\nfn governance_dag_json_response")
        .map(|offset| start + offset)
        .expect("end of governance DAG readback region");
    let readback = &source[start..end];
    assert!(readback.contains(".read_governance_dag_file(relative, maximum)"));
    for forbidden in [
        "fs::read(",
        "fs::read_to_string(",
        "fs::canonicalize(",
        "fs::symlink_metadata(",
        "File::open(",
        "path.display().to_string()",
        "root.display().to_string()",
    ] {
        assert!(
            !readback.contains(forbidden),
            "governance DAG readback must not use path-based `{forbidden}`"
        );
    }

    let producer = include_str!("../../../../sorafs_node/src/governance.rs");
    assert!(producer.contains("const GOVERNANCE_DAG_LOGICAL_ROOT: &str = \".\";"));
    assert!(
        !producer.contains("JsonValue::from(root.display().to_string())"),
        "public Governance DAG indexes must not persist the host filesystem root"
    );
}

#[test]
fn sorafs_bounded_file_readbacks_stay_on_heavy_workers() {
    assert_eq!(MAX_LOCAL_MANIFEST_RESPONSE_BYTES, 16 * 1024 * 1024);
    assert_eq!(MAX_STORAGE_FETCH_RESPONSE_BYTES, 8 * 1024 * 1024);
    assert_eq!(MAX_CAR_RANGE_PAYLOAD_BYTES, 8 * 1024 * 1024);
    assert_eq!(MAX_CAR_RANGE_RESPONSE_BYTES, 16 * 1024 * 1024);

    let source = include_str!("../api.rs");
    for (start_marker, end_marker, io_marker) in [
        (
            "pub(crate) async fn handle_get_sorafs_storage_manifest(",
            "pub(crate) async fn handle_get_sorafs_storage_plan(",
            ".load_manifest_bytes()",
        ),
        (
            "pub(crate) async fn handle_get_sorafs_storage_plan(",
            "pub(crate) async fn handle_get_sorafs_pin_registry(",
            ".load_manifest()",
        ),
        (
            "async fn build_site_file_response(",
            "pub(crate) async fn handle_get_sorafs_site_manifest(",
            ".read_payload_range(",
        ),
        (
            "pub(crate) async fn handle_get_sorafs_site_manifest(",
            "pub(crate) async fn handle_get_sorafs_site_root(",
            ".load_manifest_bytes()",
        ),
        (
            "pub(crate) async fn handle_post_sorafs_storage_fetch(",
            "fn required_canonical_stream_header(",
            ".read_payload_range(",
        ),
        (
            "pub(crate) async fn handle_get_sorafs_storage_car_range(",
            "pub(crate) async fn handle_get_sorafs_storage_chunk(",
            ".load_manifest()",
        ),
    ] {
        let start = source
            .find(start_marker)
            .unwrap_or_else(|| panic!("missing SoraFS handler `{start_marker}`"));
        let tail = &source[start..];
        let end = tail
            .find(end_marker)
            .unwrap_or_else(|| panic!("missing end marker `{end_marker}`"));
        let handler = &tail[..end];
        let worker = handler
            .find("sorafs_heavy_blocking_task(")
            .unwrap_or_else(|| {
                panic!("file-backed SoraFS handler `{start_marker}` is not offloaded")
            });
        let io = handler.find(io_marker).unwrap_or_else(|| {
            panic!("file-backed SoraFS handler `{start_marker}` lost `{io_marker}`")
        });
        assert!(
            worker < io,
            "file-backed SoraFS handler `{start_marker}` performs I/O before entering its heavy worker"
        );
    }

    let car_start = source
        .find("pub(crate) async fn handle_get_sorafs_storage_car_range(")
        .expect("CAR range handler");
    let car_end = source[car_start..]
        .find("pub(crate) async fn handle_get_sorafs_storage_chunk(")
        .map(|offset| car_start + offset)
        .expect("end of CAR range handler");
    let car_handler = &source[car_start..car_end];
    let worker = car_handler
        .find("sorafs_heavy_blocking_task(")
        .expect("CAR range heavy worker");
    assert!(
        car_handler
            .find(".read_payload_range(")
            .is_some_and(|read| worker < read),
        "CAR range payload reads must occur inside the heavy worker"
    );
    for forbidden in [
        "fs::read(stored.manifest_path())",
        "ChunkFilesReader",
        "StreamingBodyWriter",
        "validate_chunk_files_and_payload_digest",
        "tokio::sync::mpsc",
    ] {
        assert!(
            !car_handler.contains(forbidden),
            "CAR range handler must not reintroduce `{forbidden}`"
        );
    }
}

#[tokio::test]
async fn governance_dag_indexes_reject_unknown_host_path_fields() {
    const FORBIDDEN_PATH: &str = "/private/retained/governance/state";

    let (mirror_app, _mirror_temp, _block_cid, _node_cid, _head_cid) =
        sorafs_app_state_with_governance_mirror();
    let mirror_path = mirror_app
        .sorafs_node
        .config()
        .governance_dir()
        .expect("configured mirror governance dir")
        .join(GOVERNANCE_DAG_MIRROR_INDEX_FILE);
    let mut mirror: Value =
        norito::json::from_slice(&fs::read(&mirror_path).expect("read governance mirror fixture"))
            .expect("decode governance mirror fixture");
    mirror
        .as_object_mut()
        .expect("mirror index object")
        .insert("host_path".into(), Value::from(FORBIDDEN_PATH));
    fs::write(
        &mirror_path,
        norito::json::to_vec(&mirror).expect("encode mirror with unknown field"),
    )
    .expect("write mirror with unknown field");
    let response =
        handle_get_sorafs_governance_dag_dashboard(State(mirror_app), HeaderMap::new()).await;
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let body_bytes = body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("collect rejected mirror response");
    assert!(!String::from_utf8_lossy(&body_bytes).contains(FORBIDDEN_PATH));

    let (runtime_app, _runtime_temp, _digest, _block_cid, _node_cid) =
        sorafs_app_state_with_governance_runtime_index();
    let runtime_path = runtime_app
        .sorafs_node
        .config()
        .governance_dir()
        .expect("configured runtime governance dir")
        .join(GOVERNANCE_DAG_RUNTIME_INDEX_FILE);
    let mut runtime: Value = norito::json::from_slice(
        &fs::read(&runtime_path).expect("read governance runtime fixture"),
    )
    .expect("decode governance runtime fixture");
    runtime
        .get_mut("blocks")
        .and_then(Value::as_array_mut)
        .and_then(|blocks| blocks.first_mut())
        .and_then(Value::as_object_mut)
        .expect("first runtime block")
        .insert("host_path".into(), Value::from(FORBIDDEN_PATH));
    fs::write(
        &runtime_path,
        norito::json::to_vec(&runtime).expect("encode runtime with unknown field"),
    )
    .expect("write runtime with unknown field");
    let response =
        handle_get_sorafs_governance_dag_runtime(State(runtime_app), HeaderMap::new()).await;
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let body_bytes = body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("collect rejected runtime response");
    assert!(!String::from_utf8_lossy(&body_bytes).contains(FORBIDDEN_PATH));

    let (car_app, car_temp, _digest, _archive_digest) =
        sorafs_app_state_with_governance_car_queue();
    let governance_dir = car_temp.path().join("governance");
    let mut state = read_publication_state_fixture(&governance_dir);
    publication_state_section_mut(&mut state, "car_queue")
        .get_mut("segments")
        .and_then(Value::as_array_mut)
        .and_then(|segments| segments.first_mut())
        .and_then(Value::as_object_mut)
        .expect("first CAR queue segment")
        .insert("host_path".into(), Value::from(FORBIDDEN_PATH));
    write_publication_state_value(&governance_dir, &state);
    let response =
        handle_get_sorafs_governance_dag_car_queue(State(car_app), HeaderMap::new()).await;
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let body_bytes = body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("collect rejected CAR queue response");
    assert!(!String::from_utf8_lossy(&body_bytes).contains(FORBIDDEN_PATH));
}

#[tokio::test]
async fn governance_dag_pending_car_segment_cannot_hide_an_artifact_path() {
    let (app, temp_dir, _digest, car_archive_digest) = sorafs_app_state_with_governance_car_queue();
    let governance_dir = temp_dir.path().join("governance");
    let mut state = read_publication_state_fixture(&governance_dir);
    let queue_object = publication_state_section_mut(&mut state, "car_queue")
        .as_object_mut()
        .expect("CAR queue object");
    let segment = queue_object
        .get_mut("segments")
        .and_then(Value::as_array_mut)
        .and_then(|segments| segments.first_mut())
        .and_then(Value::as_object_mut)
        .expect("first CAR queue segment");
    segment.insert("status".into(), Value::from("pending"));
    for field in [
        "car_path",
        "plan_path",
        "manifest_path",
        "car_size",
        "car_archive_blake3",
        "car_payload_blake3",
        "car_cid_hex",
        "root_cids_hex",
        "dag_codec",
        "chunk_count",
        "payload_bytes",
        "files",
        "chunk_profile",
    ] {
        segment.remove(field);
    }
    segment.insert(
        "car_path".into(),
        Value::from("/private/retained/substituted.car"),
    );
    queue_object.insert("assembled_count".into(), Value::from(1_u64));
    queue_object.insert("pending_count".into(), Value::from(1_u64));
    queue_object
        .get_mut("by_car_archive_blake3")
        .and_then(Value::as_object_mut)
        .expect("CAR archive lookup")
        .remove(&car_archive_digest);
    write_publication_state_value(&governance_dir, &state);

    let response = handle_get_sorafs_governance_dag_car_queue(State(app), HeaderMap::new()).await;
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
}

#[test]
fn public_conditional_etags_use_case_sensitive_http_entity_tags() {
    let etag = format!("\"{}\"", "ab".repeat(32));
    for matching in [etag.clone(), format!("W/{etag}"), "*".to_owned()] {
        let mut headers = HeaderMap::new();
        headers.insert(
            IF_NONE_MATCH,
            HeaderValue::from_str(&matching).expect("valid matching entity tag"),
        );
        assert!(
            if_none_match_matches(&headers, &etag),
            "valid conditional token did not match: {matching}"
        );
    }

    for non_matching in [
        etag.to_ascii_uppercase(),
        format!("\"{etag}\""),
        format!("{etag}, *"),
        format!("{etag},"),
        format!("w/{etag}"),
    ] {
        let mut headers = HeaderMap::new();
        headers.insert(
            IF_NONE_MATCH,
            HeaderValue::from_str(&non_matching).expect("visible conditional token"),
        );
        assert!(
            !if_none_match_matches(&headers, &etag),
            "malformed or nonmatching conditional token was accepted: {non_matching}"
        );
    }
}

#[tokio::test]
async fn governance_dag_dashboard_head_and_lookups_read_local_mirror() {
    let (app, _temp_dir, block_cid_hex, node_cid_hex, head_block_cid_hex) =
        sorafs_app_state_with_governance_mirror();
    let publisher_key = KeyPair::try_from_seed(
        b"torii-governance-runtime-provenance".to_vec(),
        Algorithm::Ed25519,
    )
    .expect("derive runtime provenance account");
    let publisher = AccountId::new(publisher_key.public_key().clone());
    let publisher_digest_hex =
        encode(sorafs_manifest::governance_dag_submission_account_digest_v1(&publisher.encode()));

    let response =
        handle_get_sorafs_governance_dag_dashboard(State(app.clone()), HeaderMap::new()).await;
    assert_eq!(response.status(), StatusCode::OK);
    let dashboard_etag = response
        .headers()
        .get(ETAG)
        .cloned()
        .expect("dashboard etag");
    assert_eq!(
        response
            .headers()
            .get(CACHE_CONTROL)
            .and_then(|value| value.to_str().ok()),
        Some(GOVERNANCE_DAG_CACHE_CONTROL)
    );
    let body_bytes = body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("collect dashboard body");
    let value: Value = norito::json::from_slice(&body_bytes).expect("decode dashboard JSON");
    assert_eq!(
        value.get("schema").and_then(Value::as_str),
        Some("sorafs.governance_dag.dashboard.v1")
    );
    assert_eq!(
        value.get("source_path").and_then(Value::as_str),
        Some(GOVERNANCE_DAG_MIRROR_INDEX_FILE)
    );
    assert_eq!(value.get("block_count").and_then(Value::as_u64), Some(2));
    assert_eq!(value.get("first_sequence").and_then(Value::as_u64), Some(0));
    assert_eq!(
        value.get("last_timestamp").and_then(Value::as_u64),
        Some(1_800_000_100)
    );
    assert_eq!(
        value
            .get("payload_kind_counts")
            .and_then(Value::as_object)
            .and_then(|counts| counts.get(APPEAL_FINANCE_REPORT_KIND))
            .and_then(Value::as_u64),
        Some(1)
    );

    let mut headers = HeaderMap::new();
    headers.insert(IF_NONE_MATCH, dashboard_etag.clone());
    let response = handle_get_sorafs_governance_dag_dashboard(State(app.clone()), headers).await;
    assert_eq!(response.status(), StatusCode::NOT_MODIFIED);
    assert_eq!(response.headers().get(ETAG), Some(&dashboard_etag));

    let mut headers = HeaderMap::new();
    headers.insert(IF_NONE_MATCH, dashboard_etag.clone());
    let response = handle_get_sorafs_governance_dag_head(State(app.clone()), headers).await;
    assert_eq!(response.status(), StatusCode::OK);
    let head_etag = response.headers().get(ETAG).cloned().expect("head etag");
    assert_ne!(head_etag, dashboard_etag);
    let body_bytes = body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("collect head body");
    let value: Value = norito::json::from_slice(&body_bytes).expect("decode head JSON");
    assert_eq!(
        value
            .get("head")
            .and_then(|head| head.get("head_block_cid_hex"))
            .and_then(Value::as_str),
        Some(head_block_cid_hex.as_str())
    );

    let response = handle_get_sorafs_governance_dag_block(
        State(app.clone()),
        HeaderMap::new(),
        Path(block_cid_hex.clone()),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let block_etag = response
        .headers()
        .get(ETAG)
        .cloned()
        .expect("block lookup etag");
    let body_bytes = body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("collect block lookup body");
    let value: Value = norito::json::from_slice(&body_bytes).expect("decode block JSON");
    assert_eq!(
        value.get("schema").and_then(Value::as_str),
        Some("sorafs.governance_dag.block.lookup.v1")
    );
    assert_eq!(value.get("found").and_then(Value::as_bool), Some(true));
    assert_eq!(
        value
            .get("block")
            .and_then(|block| block.get("block_cid_hex"))
            .and_then(Value::as_str),
        Some(block_cid_hex.as_str())
    );
    assert_eq!(
        value
            .get("block")
            .and_then(|block| block.get("submission_publisher_account_digest_hex"))
            .and_then(Value::as_str),
        Some(publisher_digest_hex.as_str())
    );

    let mut headers = HeaderMap::new();
    headers.insert(IF_NONE_MATCH, block_etag);
    let response =
        handle_get_sorafs_governance_dag_block(State(app.clone()), headers, Path("ff".repeat(32)))
            .await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    let response = handle_get_sorafs_governance_dag_node(
        State(app.clone()),
        HeaderMap::new(),
        Path(format!("hex:{node_cid_hex}")),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let body_bytes = body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("collect node lookup body");
    let value: Value = norito::json::from_slice(&body_bytes).expect("decode node JSON");
    assert_eq!(
        value.get("schema").and_then(Value::as_str),
        Some("sorafs.governance_dag.node.lookup.v1")
    );
    assert_eq!(
        value
            .get("block")
            .and_then(|block| block.get("node_cid_hex"))
            .and_then(Value::as_str),
        Some(node_cid_hex.as_str())
    );
    assert_eq!(
        value
            .get("block")
            .and_then(|block| block.get("submission_origin"))
            .and_then(Value::as_str),
        Some("appeal_finance_report")
    );
}

#[tokio::test]
async fn governance_dag_mirror_rejects_unsigned_provenance_substitution() {
    let (app, _temp_dir, block_cid_hex, _node_cid_hex, _head_block_cid_hex) =
        sorafs_app_state_with_governance_mirror();
    let governance_dir = app
        .sorafs_node
        .config()
        .governance_dir()
        .expect("configured governance dir");
    let mirror_path = governance_dir.join(GOVERNANCE_DAG_MIRROR_INDEX_FILE);
    let mut mirror: Value =
        norito::json::from_slice(&fs::read(&mirror_path).expect("read governance mirror index"))
            .expect("decode governance mirror index");
    mirror
        .get_mut("blocks")
        .and_then(Value::as_array_mut)
        .and_then(|blocks| blocks.first_mut())
        .and_then(Value::as_object_mut)
        .expect("first governance mirror block")
        .insert(
            "submission_publisher_account_digest_hex".into(),
            Value::from("00".repeat(32)),
        );
    fs::write(
        &mirror_path,
        norito::json::to_vec(&mirror).expect("encode tampered governance mirror index"),
    )
    .expect("write tampered governance mirror index");

    let response =
        handle_get_sorafs_governance_dag_block(State(app), HeaderMap::new(), Path(block_cid_hex))
            .await;
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
}

#[tokio::test]
async fn governance_dag_mirror_rejects_ipfs_cid_substitution() {
    let (app, _temp_dir, block_cid_hex, _node_cid_hex, _head_block_cid_hex) =
        sorafs_app_state_with_governance_mirror();
    let governance_dir = app
        .sorafs_node
        .config()
        .governance_dir()
        .expect("configured governance dir");
    let mirror_path = governance_dir.join(GOVERNANCE_DAG_MIRROR_INDEX_FILE);
    let mut mirror: Value =
        norito::json::from_slice(&fs::read(&mirror_path).expect("read governance mirror index"))
            .expect("decode governance mirror index");
    mirror
        .get_mut("blocks")
        .and_then(Value::as_array_mut)
        .and_then(|blocks| blocks.first_mut())
        .and_then(Value::as_object_mut)
        .expect("first governance mirror block")
        .insert(
            "ipfs_cid".into(),
            Value::from(governance_dag_raw_ipfs_cid(b"substituted block")),
        );
    fs::write(
        &mirror_path,
        norito::json::to_vec(&mirror).expect("encode tampered governance mirror index"),
    )
    .expect("write tampered governance mirror index");

    let response =
        handle_get_sorafs_governance_dag_block(State(app), HeaderMap::new(), Path(block_cid_hex))
            .await;
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
}

#[tokio::test]
async fn governance_dag_mirror_rejects_history_not_ending_at_signed_head() {
    let (app, _temp_dir, _block_cid_hex, _node_cid_hex, _head_block_cid_hex) =
        sorafs_app_state_with_governance_mirror();
    let governance_dir = app
        .sorafs_node
        .config()
        .governance_dir()
        .expect("configured governance dir");
    let mirror_path = governance_dir.join(GOVERNANCE_DAG_MIRROR_INDEX_FILE);
    let mut mirror: Value =
        norito::json::from_slice(&fs::read(&mirror_path).expect("read governance mirror index"))
            .expect("decode governance mirror index");
    mirror
        .get_mut("blocks")
        .and_then(Value::as_array_mut)
        .expect("governance mirror blocks")
        .truncate(1);
    mirror
        .as_object_mut()
        .expect("governance mirror root")
        .insert("indexed_block_count".into(), Value::from(1_u64));
    fs::write(
        &mirror_path,
        norito::json::to_vec(&mirror).expect("encode truncated governance mirror index"),
    )
    .expect("write truncated governance mirror index");

    let response = handle_get_sorafs_governance_dag_dashboard(State(app), HeaderMap::new()).await;
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
}

#[tokio::test]
async fn governance_dag_lookup_rejects_malformed_and_missing_cids() {
    let (app, _temp_dir, _block_cid_hex, _node_cid_hex, _head_block_cid_hex) =
        sorafs_app_state_with_governance_mirror();

    let response = handle_get_sorafs_governance_dag_block(
        State(app.clone()),
        HeaderMap::new(),
        Path("not-hex".to_string()),
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let response = handle_get_sorafs_governance_dag_block(
        State(app.clone()),
        HeaderMap::new(),
        Path("ff".to_string()),
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let response =
        handle_get_sorafs_governance_dag_block(State(app), HeaderMap::new(), Path("ff".repeat(32)))
            .await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn governance_dag_publish_index_and_lookups_read_local_index() {
    let (app, _temp_dir, digest_hex) = sorafs_app_state_with_governance_publish_index();

    let response = handle_get_sorafs_governance_dag_publish_index(
        State(app.clone()),
        HeaderMap::new(),
        axum::extract::RawQuery(None),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let index_etag = response.headers().get(ETAG).cloned().expect("index etag");
    assert_eq!(
        response
            .headers()
            .get(CACHE_CONTROL)
            .and_then(|value| value.to_str().ok()),
        Some(GOVERNANCE_DAG_CACHE_CONTROL)
    );
    let body_bytes = body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("collect publish index body");
    let value: Value = norito::json::from_slice(&body_bytes).expect("decode publish index");
    assert_eq!(
        value.get("schema").and_then(Value::as_str),
        Some("sorafs.governance_dag.publish_index.v1")
    );
    assert_eq!(
        value.get("source_path").and_then(Value::as_str),
        Some(GOVERNANCE_DAG_PUBLICATION_STATE_FILE)
    );
    assert_eq!(
        value
            .get("index")
            .and_then(|index| index.get("root"))
            .and_then(Value::as_str),
        Some(GOVERNANCE_DAG_LOGICAL_ROOT)
    );
    assert_eq!(value.get("entry_count").and_then(Value::as_u64), Some(6));
    assert_eq!(
        value.get("indexed_entry_count").and_then(Value::as_u64),
        Some(6)
    );
    assert_eq!(
        value.get("returned_entry_count").and_then(Value::as_u64),
        Some(6)
    );
    assert_eq!(
        value.get("limit").and_then(Value::as_u64),
        Some(DEFAULT_LIST_LIMIT as u64)
    );
    assert_eq!(
        value.get("truncated_entries").and_then(Value::as_bool),
        Some(false)
    );
    assert_eq!(
        value
            .get("index")
            .and_then(|index| index.get("entries"))
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(6)
    );
    assert_eq!(
        value
            .get("payload_kind_counts")
            .and_then(Value::as_object)
            .and_then(|counts| counts.get("repair_audit"))
            .and_then(Value::as_u64),
        Some(1)
    );

    let response = handle_get_sorafs_governance_dag_publish_index(
        State(app.clone()),
        HeaderMap::new(),
        axum::extract::RawQuery(Some("limit=1".to_owned())),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let capped_etag = response.headers().get(ETAG).cloned().expect("capped etag");
    assert_ne!(index_etag, capped_etag);
    let body_bytes = body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("collect capped publish index body");
    let value: Value = norito::json::from_slice(&body_bytes).expect("decode capped publish index");
    assert_eq!(value.get("entry_count").and_then(Value::as_u64), Some(6));
    assert_eq!(
        value.get("indexed_entry_count").and_then(Value::as_u64),
        Some(6)
    );
    assert_eq!(
        value.get("returned_entry_count").and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(value.get("limit").and_then(Value::as_u64), Some(1));
    assert_eq!(
        value.get("truncated_entries").and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        value
            .get("index")
            .and_then(|index| index.get("entries"))
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(1)
    );

    let mut headers = HeaderMap::new();
    headers.insert(IF_NONE_MATCH, index_etag.clone());
    let response = handle_get_sorafs_governance_dag_publish_index(
        State(app.clone()),
        headers,
        axum::extract::RawQuery(None),
    )
    .await;
    assert_eq!(response.status(), StatusCode::NOT_MODIFIED);
    assert_eq!(response.headers().get(ETAG), Some(&index_etag));

    let response = handle_get_sorafs_governance_dag_publish_digest(
        State(app.clone()),
        HeaderMap::new(),
        Path(format!("hex:{digest_hex}")),
        axum::extract::RawQuery(None),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let digest_lookup_etag = response
        .headers()
        .get(ETAG)
        .cloned()
        .expect("digest lookup etag");
    let body_bytes = body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("collect digest lookup body");
    let value: Value = norito::json::from_slice(&body_bytes).expect("decode digest lookup");
    assert_eq!(
        value.get("schema").and_then(Value::as_str),
        Some("sorafs.governance_dag.publish_index.digest.lookup.v1")
    );
    assert_eq!(value.get("count").and_then(Value::as_u64), Some(1));
    assert_eq!(
        value
            .get("entries")
            .and_then(Value::as_array)
            .and_then(|entries| entries.first())
            .and_then(|entry| entry.get("payload_kind"))
            .and_then(Value::as_str),
        Some("repair_audit")
    );

    let mut headers = HeaderMap::new();
    headers.insert(IF_NONE_MATCH, digest_lookup_etag.clone());
    let response = handle_get_sorafs_governance_dag_publish_kind(
        State(app.clone()),
        headers,
        Path("repair_audit".to_string()),
        axum::extract::RawQuery(None),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_ne!(response.headers().get(ETAG), Some(&digest_lookup_etag));
    let body_bytes = body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("collect kind lookup body");
    let value: Value = norito::json::from_slice(&body_bytes).expect("decode kind lookup");
    assert_eq!(
        value.get("schema").and_then(Value::as_str),
        Some("sorafs.governance_dag.publish_index.kind.lookup.v1")
    );
    assert_eq!(
        value
            .get("entries")
            .and_then(Value::as_array)
            .and_then(|entries| entries.first())
            .and_then(|entry| entry.get("encoded_blake3"))
            .and_then(Value::as_str),
        Some(digest_hex.as_str())
    );

    let mut headers = HeaderMap::new();
    headers.insert(IF_NONE_MATCH, digest_lookup_etag);
    let response = handle_get_sorafs_governance_dag_publish_digest(
        State(app),
        headers,
        Path(format!("hex:{}", "ff".repeat(32))),
        axum::extract::RawQuery(None),
    )
    .await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn transparency_proof_token_verify_accepts_signature_and_digest() {
    let signing = SigningKey::from_bytes(&[0x47; 32]);
    let verifying = signing.verifying_key();
    let digest_key = ProofTokenDigestKey::new([0x22; 32]);
    let evidence_digest = [0x44; 32];
    let entries = ["denylist/global"];
    let issued_at = UNIX_EPOCH + Duration::from_secs(1_700_000_000);
    let expires_at = issued_at + Duration::from_secs(600);
    let mut rng = FixedProofTokenRng { byte: 0xA7 };
    let token = ProofToken::mint(
        &mut rng,
        &digest_key,
        &signing,
        &iroha_crypto::sorafs::proof_token::ProofTokenParams {
            moderation: ProofTokenModerationAction::Block,
            entry_ids: &entries,
            evidence_digest: &evidence_digest,
            issued_at,
            expires_at: Some(expires_at),
        },
    )
    .expect("mint proof token");
    let request = TransparencyProofTokenVerifyRequestDto {
        token_b64: token.encode_base64(),
        verifying_key_hex: hex::encode(verifying.to_bytes()),
        evidence_digest_hex: Some(hex::encode(evidence_digest)),
        digest_key_hex: Some(hex::encode([0x22; 32])),
        now_unix: Some(1_700_000_120),
    };

    let app = mk_app_state_for_tests();
    let response = handle_post_sorafs_transparency_token_verify(
        State(app),
        HeaderMap::new(),
        ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 0))),
        JsonOnly(request),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let body_bytes = body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("collect proof-token verification body");
    let value: Value =
        norito::json::from_slice(&body_bytes).expect("decode proof-token verification body");
    assert_eq!(
        value.get("schema").and_then(Value::as_str),
        Some("sorafs.transparency.proof_token_verification.v1")
    );
    assert_eq!(value.get("valid").and_then(Value::as_bool), Some(true));
    assert_eq!(
        value.get("signature_valid").and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        value.get("digest_checked").and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        value.get("digest_valid").and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        value
            .get("entry_ids")
            .and_then(Value::as_array)
            .and_then(|entries| entries.first())
            .and_then(Value::as_str),
        Some("denylist/global")
    );
    assert_eq!(
        value.get("moderation_action").and_then(Value::as_str),
        Some("block")
    );
}

#[tokio::test]
async fn transparency_proof_token_verify_reports_invalid_inputs() {
    let signing = SigningKey::from_bytes(&[0x47; 32]);
    let wrong_signing = SigningKey::from_bytes(&[0x48; 32]);
    let digest_key = ProofTokenDigestKey::new([0x22; 32]);
    let evidence_digest = [0x44; 32];
    let entries = ["denylist/global"];
    let issued_at = UNIX_EPOCH + Duration::from_secs(1_700_000_000);
    let mut rng = FixedProofTokenRng { byte: 0xA8 };
    let token = ProofToken::mint(
        &mut rng,
        &digest_key,
        &signing,
        &iroha_crypto::sorafs::proof_token::ProofTokenParams {
            moderation: ProofTokenModerationAction::Quarantine,
            entry_ids: &entries,
            evidence_digest: &evidence_digest,
            issued_at,
            expires_at: None,
        },
    )
    .expect("mint proof token");
    let request = TransparencyProofTokenVerifyRequestDto {
        token_b64: token.encode_base64(),
        verifying_key_hex: hex::encode(wrong_signing.verifying_key().to_bytes()),
        evidence_digest_hex: Some(hex::encode([0x45; 32])),
        digest_key_hex: Some(hex::encode([0x22; 32])),
        now_unix: Some(1_700_000_120),
    };

    let app = mk_app_state_for_tests();
    let response = handle_post_sorafs_transparency_token_verify(
        State(app.clone()),
        HeaderMap::new(),
        ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 0))),
        JsonOnly(request),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let body_bytes = body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("collect invalid proof-token verification body");
    let value: Value =
        norito::json::from_slice(&body_bytes).expect("decode invalid proof-token body");
    assert_eq!(value.get("valid").and_then(Value::as_bool), Some(false));
    assert_eq!(
        value.get("signature_valid").and_then(Value::as_bool),
        Some(false)
    );
    assert_eq!(
        value.get("digest_valid").and_then(Value::as_bool),
        Some(false)
    );

    let request = TransparencyProofTokenVerifyRequestDto {
        token_b64: token.encode_base64(),
        verifying_key_hex: hex::encode(signing.verifying_key().to_bytes()),
        evidence_digest_hex: None,
        digest_key_hex: Some(hex::encode([0x22; 32])),
        now_unix: Some(1_700_000_120),
    };
    let response = handle_post_sorafs_transparency_token_verify(
        State(app.clone()),
        HeaderMap::new(),
        ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 0))),
        JsonOnly(request),
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let request = TransparencyProofTokenVerifyRequestDto {
        token_b64: token.encode_base64(),
        verifying_key_hex: hex::encode([0u8; 32]),
        evidence_digest_hex: None,
        digest_key_hex: None,
        now_unix: Some(1_700_000_120),
    };
    let response = handle_post_sorafs_transparency_token_verify(
        State(app),
        HeaderMap::new(),
        ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 0))),
        JsonOnly(request),
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn transparency_proof_token_verify_rejects_noncanonical_verifying_key() {
    const ED25519_NONCANONICAL_IDENTITY: [u8; ed25519_dalek::PUBLIC_KEY_LENGTH] = [
        0xee, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        0xff, 0x7f,
    ];

    let mut request = valid_transparency_proof_token_verify_request(0xA9);
    request.verifying_key_hex = hex::encode(ED25519_NONCANONICAL_IDENTITY);

    let app = mk_app_state_for_tests();
    let response = handle_post_sorafs_transparency_token_verify(
        State(app),
        HeaderMap::new(),
        ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 0))),
        JsonOnly(request),
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body_bytes = body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("collect noncanonical proof-token verification body");
    let body_text = String::from_utf8_lossy(&body_bytes);
    assert!(
        body_text.contains("non-canonical ed25519 public key encoding"),
        "unexpected proof-token verification error body: {body_text}"
    );
}

#[tokio::test]
async fn transparency_proof_token_verify_uses_proof_rate_limiter() {
    let mut app = mk_app_state_for_tests();
    {
        let state = Arc::get_mut(&mut app).expect("unique app state");
        state.proof_rate_limiter = crate::limits::RateLimiter::new(Some(1), Some(1));
        state.proof_limits.retry_after = Duration::from_secs(7);
    }

    let first = handle_post_sorafs_transparency_token_verify(
        State(app.clone()),
        HeaderMap::new(),
        ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 0))),
        JsonOnly(valid_transparency_proof_token_verify_request(0xA9)),
    )
    .await;
    assert_eq!(first.status(), StatusCode::OK);

    let second = handle_post_sorafs_transparency_token_verify(
        State(app),
        HeaderMap::new(),
        ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 0))),
        JsonOnly(valid_transparency_proof_token_verify_request(0xAA)),
    )
    .await;
    assert_eq!(second.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(
        second
            .headers()
            .get(RETRY_AFTER)
            .and_then(|h| h.to_str().ok()),
        Some("7")
    );
}

#[tokio::test]
async fn transparency_proof_token_verify_honors_api_token_enforcement() {
    let mut app = mk_app_state_for_tests();
    {
        let state = Arc::get_mut(&mut app).expect("unique app state");
        state.require_api_token = true;
        let mut tokens = HashSet::new();
        tokens.insert("secret".to_string());
        state.api_tokens_set = Arc::new(tokens);
    }

    let denied = handle_post_sorafs_transparency_token_verify(
        State(app.clone()),
        HeaderMap::new(),
        ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 0))),
        JsonOnly(valid_transparency_proof_token_verify_request(0xAB)),
    )
    .await;
    assert_eq!(denied.status(), StatusCode::FORBIDDEN);

    let mut headers = HeaderMap::new();
    headers.insert("x-api-token", HeaderValue::from_static("secret"));
    let allowed = handle_post_sorafs_transparency_token_verify(
        State(app),
        headers,
        ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 0))),
        JsonOnly(valid_transparency_proof_token_verify_request(0xAC)),
    )
    .await;
    assert_eq!(allowed.status(), StatusCode::OK);
}

#[tokio::test]
async fn transparency_proof_token_issuance_endpoint_requires_canonical_request_auth() {
    let (app, _temp_dir, _auth) = sorafs_app_state_with_appeal_finance_governance_publisher();
    let body = proof_token_issuance_body(transparency_proof_token_issuance_request(0xAD));

    let response = handle_post_sorafs_transparency_token_issuance(
        State(app),
        HeaderMap::new(),
        Method::POST,
        Uri::from_static(TRANSPARENCY_PROOF_TOKEN_ISSUANCES_ROUTE),
        body,
    )
    .await;

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn transparency_proof_token_issuance_endpoint_requires_source_publisher_role() {
    let (app, _temp_dir, auth) = sorafs_app_state_with_appeal_finance_governance_publisher();
    let body = proof_token_issuance_body(transparency_proof_token_issuance_request(0xAD));

    let response = post_transparency_proof_token_issuance(app.clone(), &auth.buyer, body).await;

    assert_forbidden_role(response, SORAFS_TRANSPARENCY_SOURCE_PUBLISHER_ROLE).await;
    assert_eq!(app.sorafs_node.pending_governance_publication_count(), 0);
}

#[tokio::test]
async fn transparency_proof_token_issuance_endpoint_publishes_signed_frame() {
    let (app, temp_dir, auth) = sorafs_app_state_with_appeal_finance_governance_publisher();
    let request = transparency_proof_token_issuance_request(0xAE);
    let signer_key_hex = request.signer_key_hex.clone();

    let response = post_transparency_proof_token_issuance(
        app.clone(),
        &auth.provider,
        proof_token_issuance_body(request),
    )
    .await;
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    let body_bytes = body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("collect proof-token issuance ingest body");
    let value: Value =
        norito::json::from_slice(&body_bytes).expect("decode proof-token issuance ingest body");
    assert_eq!(
        value.get("schema").and_then(Value::as_str),
        Some("sorafs.transparency.proof_token_issuance.ingest.v1")
    );
    assert_eq!(
        value.get("publication_status").and_then(Value::as_str),
        Some("published_to_local_governance_dag")
    );
    assert_eq!(
        value.get("signer_key_hex").and_then(Value::as_str),
        Some(signer_key_hex.as_str())
    );
    assert_eq!(value.get("entry_count").and_then(Value::as_u64), Some(2));
    assert_eq!(
        value
            .get("metadata")
            .and_then(Value::as_object)
            .and_then(|metadata| metadata.get("feed"))
            .and_then(Value::as_str),
        Some("torii")
    );
    assert_governance_publish_provenance(
        &temp_dir.path().join("governance"),
        PROOF_TOKEN_ISSUANCE_KIND,
        &auth.provider.account,
        "transparency_token_issuance",
    );

    let response = handle_get_sorafs_transparency_token_issuances(
        State(app),
        HeaderMap::new(),
        axum::extract::RawQuery(None),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let body_bytes = body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("collect proof-token issuance readback body");
    let value: Value =
        norito::json::from_slice(&body_bytes).expect("decode proof-token issuance readback");
    assert_eq!(
        value.get("published_token_count").and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        value.get("distinct_signer_count").and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        value
            .get("entries")
            .and_then(Value::as_array)
            .and_then(|entries| entries.first())
            .and_then(|entry| entry.get("labels"))
            .and_then(Value::as_object)
            .and_then(|labels| labels.get("signer_key_hex"))
            .and_then(Value::as_str),
        Some(signer_key_hex.as_str())
    );
}

#[tokio::test]
async fn transparency_proof_token_issuance_endpoint_rejects_bad_signer() {
    let (app, _temp_dir, auth) = sorafs_app_state_with_appeal_finance_governance_publisher();
    let mut request = transparency_proof_token_issuance_request(0xAF);
    let wrong_signing = SigningKey::from_bytes(&[0x48; 32]);
    request.signer_key_hex = hex::encode(wrong_signing.verifying_key().to_bytes());

    let response = post_transparency_proof_token_issuance(
        app,
        &auth.provider,
        proof_token_issuance_body(request),
    )
    .await;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn transparency_cycle_api_reads_and_verifies_local_publication() {
    let (app, _temp_dir, cycle_id_hex, entry_id_hex, digest_hex) =
        sorafs_app_state_with_transparency_publication();

    let response = handle_get_sorafs_transparency_cycles(
        State(app.clone()),
        HeaderMap::new(),
        axum::extract::RawQuery(None),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let etag = response.headers().get(ETAG).cloned().expect("cycles etag");
    let body_bytes = body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("collect transparency cycles body");
    let value: Value = norito::json::from_slice(&body_bytes).expect("decode cycles body");
    assert_eq!(
        value.get("schema").and_then(Value::as_str),
        Some("sorafs.transparency.cycles.v1")
    );
    assert_eq!(
        value.get("published_cycle_count").and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        value.get("returned_cycle_count").and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(value.get("limit").and_then(Value::as_u64), Some(50));
    assert_eq!(value.get("truncated").and_then(Value::as_bool), Some(false));
    assert_eq!(
        value
            .get("cycles")
            .and_then(Value::as_array)
            .and_then(|cycles| cycles.first())
            .and_then(|cycle| cycle.get("cycle_id_hex"))
            .and_then(Value::as_str),
        Some(cycle_id_hex.as_str())
    );

    let mut headers = HeaderMap::new();
    headers.insert(IF_NONE_MATCH, etag.clone());
    let response = handle_get_sorafs_transparency_cycles(
        State(app.clone()),
        headers,
        axum::extract::RawQuery(None),
    )
    .await;
    assert_eq!(response.status(), StatusCode::NOT_MODIFIED);
    assert_eq!(response.headers().get(ETAG), Some(&etag));

    let mut headers = HeaderMap::new();
    headers.insert(IF_NONE_MATCH, etag.clone());
    let response = handle_get_sorafs_transparency_cycle(
        State(app.clone()),
        headers,
        Path(format!("hex:{cycle_id_hex}")),
        axum::extract::RawQuery(None),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let cycle_etag = response.headers().get(ETAG).cloned().expect("cycle etag");
    assert_ne!(cycle_etag, etag);
    let body_bytes = body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("collect transparency cycle body");
    let value: Value = norito::json::from_slice(&body_bytes).expect("decode cycle body");
    assert_eq!(
        value.get("schema").and_then(Value::as_str),
        Some("sorafs.transparency.cycle_publication.v1")
    );
    assert_eq!(
        value.get("cycle_id_hex").and_then(Value::as_str),
        Some(cycle_id_hex.as_str())
    );
    assert_eq!(
        value.get("encoded_blake3").and_then(Value::as_str),
        Some(digest_hex.as_str())
    );
    assert_eq!(
        value
            .get("verification")
            .and_then(|verification| verification.get("valid"))
            .and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        value
            .get("verification")
            .and_then(|verification| verification.get("proof_count"))
            .and_then(Value::as_u64),
        Some(2)
    );
    assert_eq!(value.get("proof_count").and_then(Value::as_u64), Some(2));
    assert_eq!(
        value.get("returned_proof_count").and_then(Value::as_u64),
        Some(2)
    );
    assert_eq!(value.get("limit").and_then(Value::as_u64), Some(50));
    assert_eq!(
        value.get("truncated_proofs").and_then(Value::as_bool),
        Some(false)
    );

    let response = handle_get_sorafs_transparency_cycle(
        State(app.clone()),
        HeaderMap::new(),
        Path(cycle_id_hex.clone()),
        axum::extract::RawQuery(Some("limit=1".to_owned())),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let body_bytes = body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("collect bounded transparency cycle body");
    let value: Value = norito::json::from_slice(&body_bytes).expect("decode bounded cycle body");
    assert_eq!(value.get("proof_count").and_then(Value::as_u64), Some(2));
    assert_eq!(
        value.get("returned_proof_count").and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(value.get("limit").and_then(Value::as_u64), Some(1));
    assert_eq!(
        value.get("truncated_proofs").and_then(Value::as_bool),
        Some(true)
    );
    let proofs = value
        .get("publication")
        .and_then(|publication| publication.get("proofs"))
        .and_then(Value::as_array)
        .expect("bounded publication proofs");
    assert_eq!(proofs.len(), 1);
    assert_eq!(
        value
            .get("verification")
            .and_then(|verification| verification.get("proof_count"))
            .and_then(Value::as_u64),
        Some(2)
    );

    let mut headers = HeaderMap::new();
    headers.insert(IF_NONE_MATCH, cycle_etag);
    let response = handle_get_sorafs_transparency_cycle_entry(
        State(app.clone()),
        headers,
        Path((cycle_id_hex.clone(), entry_id_hex.clone())),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let entry_etag = response.headers().get(ETAG).cloned().expect("entry etag");
    let body_bytes = body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("collect transparency entry proof body");
    let value: Value = norito::json::from_slice(&body_bytes).expect("decode proof body");
    assert_eq!(
        value.get("schema").and_then(Value::as_str),
        Some("sorafs.transparency.entry_proof.v1")
    );
    assert_eq!(
        value.get("entry_id_hex").and_then(Value::as_str),
        Some(entry_id_hex.as_str())
    );
    assert!(value.get("proof").is_some());
    assert_eq!(
        value
            .get("verification")
            .and_then(|verification| verification.get("all_proofs_verified"))
            .and_then(Value::as_bool),
        Some(true)
    );

    let mut headers = HeaderMap::new();
    headers.insert(IF_NONE_MATCH, entry_etag);
    let response = handle_get_sorafs_transparency_cycle_entry(
        State(app),
        headers,
        Path((cycle_id_hex, "ff".repeat(16))),
    )
    .await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn transparency_cycle_api_rejects_bad_ids_missing_entries_and_path_escape() {
    let (app, temp_dir, cycle_id_hex, _entry_id_hex, _digest_hex) =
        sorafs_app_state_with_transparency_publication();

    let response = handle_get_sorafs_transparency_cycle(
        State(app.clone()),
        HeaderMap::new(),
        Path("abcd".to_string()),
        axum::extract::RawQuery(None),
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let response = handle_get_sorafs_transparency_cycle_entry(
        State(app.clone()),
        HeaderMap::new(),
        Path((cycle_id_hex.clone(), "ff".repeat(16))),
    )
    .await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    let governance_dir = temp_dir.path().join("governance");
    let mut index = read_publication_section_fixture(&governance_dir, "publish_index");
    let response = handle_get_sorafs_transparency_cycles(
        State(app.clone()),
        HeaderMap::new(),
        axum::extract::RawQuery(None),
    )
    .await;
    let etag = response
        .headers()
        .get(ETAG)
        .cloned()
        .expect("transparency cycle etag");
    let encoded_path = index
        .get("entries")
        .and_then(Value::as_array)
        .and_then(|entries| entries.first())
        .and_then(|entry| entry.get("encoded_path"))
        .and_then(Value::as_str)
        .expect("encoded path")
        .to_owned();
    fs::write(governance_dir.join(encoded_path), b"tampered transparency")
        .expect("tamper encoded publication");
    let mut headers = HeaderMap::new();
    headers.insert(IF_NONE_MATCH, etag);
    let response = handle_get_sorafs_transparency_cycle(
        State(app.clone()),
        headers,
        Path(cycle_id_hex.clone()),
        axum::extract::RawQuery(None),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CONFLICT);

    let entries = index
        .get_mut("entries")
        .and_then(Value::as_array_mut)
        .expect("entries");
    let entry = entries[0].as_object_mut().expect("entry object");
    entry.insert("encoded_path".into(), Value::from("../escape.to"));
    write_publish_index_fixture(&governance_dir, index);

    let response = handle_get_sorafs_transparency_cycle(
        State(app),
        HeaderMap::new(),
        Path(cycle_id_hex),
        axum::extract::RawQuery(None),
    )
    .await;
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
}
