#[test]
fn filesystem_publisher_rejects_malformed_runtime_dag_index() {
    let temp = tempdir().expect("tempdir");
    let publisher = signed_runtime_publisher(temp.path());
    fs::write(
        temp.path().join(GOVERNANCE_RUNTIME_DAG_INDEX_FILE),
        br#"{"schema":"sorafs.governance_dag.wrong","blocks":[]}"#,
    )
    .expect("write bad runtime index");
    let (settlement, encoded) = sample_settlement();

    let err = publisher
        .publish_deal_settlement(&settlement, &encoded)
        .expect_err("malformed runtime DAG index must fail closed");
    assert!(
        err.to_string().contains("unsupported schema"),
        "unexpected error: {err}"
    );
}

#[test]
fn filesystem_publisher_writes_settlement_files() {
    let temp = tempdir().expect("tempdir");
    let publisher = signed_runtime_publisher(temp.path());

    let (settlement, encoded) = sample_settlement();

    publisher
        .publish_deal_settlement(&settlement, &encoded)
        .expect("publish");

    let deal_hex = settlement.deal_id.encode_hex::<String>();
    let dir = temp.path().join("settlements").join(deal_hex);

    let entries = fs::read_dir(&dir)
        .expect("directory exists")
        .map(|entry| entry.expect("dir entry").path())
        .collect::<Vec<_>>();
    assert_eq!(entries.len(), 4, "expected encoded + json + digests");

    let mut encoded_paths = entries
        .iter()
        .filter(|path| path.extension().map(|ext| ext == "to").unwrap_or(false));
    let encoded_path = encoded_paths.next().expect("encoded artefact present");
    assert_eq!(
        fs::read(encoded_path).expect("read encoded"),
        encoded,
        "encoded payload must match original bytes"
    );

    let json_path = entries
        .iter()
        .find(|path| path.extension().map(|ext| ext == "json").unwrap_or(false))
        .expect("json artefact present");
    let json_bytes = fs::read(json_path).expect("read json");
    let value: JsonValue = norito::json::from_slice(&json_bytes).expect("json should parse");
    let status = value
        .get("metadata")
        .and_then(|meta| meta.get("status"))
        .and_then(JsonValue::as_str)
        .expect("status");
    assert_eq!(status, "completed");

    let encoded_digest_path = entries
        .iter()
        .find(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .map(|name| name.ends_with("to.blake3"))
                .unwrap_or(false)
        })
        .expect("encoded digest present");
    let encoded_digest = fs::read_to_string(encoded_digest_path).expect("read encoded digest");
    let encoded_digest = encoded_digest.trim();
    assert_eq!(encoded_digest, blake3::hash(&encoded).to_hex().as_str());

    let json_digest_path = entries
        .iter()
        .find(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .map(|name| name.ends_with("json.blake3"))
                .unwrap_or(false)
        })
        .expect("json digest present");
    let json_digest = fs::read_to_string(json_digest_path).expect("read json digest");
    let json_digest = json_digest.trim();
    assert_eq!(json_digest, blake3::hash(&json_bytes).to_hex().as_str());

    let index_path = temp.path().join(GOVERNANCE_PUBLISH_INDEX_FILE);
    let index_bytes = fs::read(&index_path).expect("read publish index");
    let index: JsonValue = norito::json::from_slice(&index_bytes).expect("index json");
    assert_eq!(
        index.get("schema").and_then(JsonValue::as_str),
        Some(GOVERNANCE_PUBLISH_INDEX_SCHEMA)
    );
    assert_eq!(
        index.get("entry_count").and_then(JsonValue::as_u64),
        Some(1)
    );
    assert_eq!(
        index
            .get("payload_kind_counts")
            .and_then(JsonValue::as_object)
            .and_then(|counts| counts.get("deal_settlement"))
            .and_then(JsonValue::as_u64),
        Some(1)
    );
    let digest_hex = blake3::hash(&encoded).to_hex().to_string();
    let digest_positions = index
        .get("by_encoded_blake3")
        .and_then(JsonValue::as_object)
        .and_then(|map| map.get(digest_hex.as_str()))
        .and_then(JsonValue::as_array)
        .expect("digest lookup");
    assert_eq!(digest_positions.len(), 1);
    assert_eq!(digest_positions[0].as_u64(), Some(0));
    let kind_positions = index
        .get("by_payload_kind")
        .and_then(JsonValue::as_object)
        .and_then(|map| map.get("deal_settlement"))
        .and_then(JsonValue::as_array)
        .expect("kind lookup");
    assert_eq!(kind_positions[0].as_u64(), Some(0));
    let entry = index
        .get("entries")
        .and_then(JsonValue::as_array)
        .and_then(|entries| entries.first())
        .and_then(JsonValue::as_object)
        .expect("first index entry");
    assert_eq!(
        entry.get("payload_kind").and_then(JsonValue::as_str),
        Some("deal_settlement")
    );
    assert_eq!(
        entry.get("encoded_path").and_then(JsonValue::as_str),
        Some(index_path_string(temp.path(), encoded_path).as_str())
    );
    assert_eq!(
        entry
            .get("labels")
            .and_then(JsonValue::as_object)
            .and_then(|labels| labels.get("status"))
            .and_then(JsonValue::as_str),
        Some("completed")
    );
    let index_digest_path = index_path.with_extension("json.blake3");
    let index_digest = fs::read_to_string(index_digest_path).expect("read index digest");
    assert_eq!(
        index_digest.trim(),
        blake3::hash(&index_bytes).to_hex().as_str()
    );

    let queue_path = temp.path().join(GOVERNANCE_CAR_QUEUE_FILE);
    let queue_bytes = fs::read(&queue_path).expect("read CAR queue");
    let queue: JsonValue = norito::json::from_slice(&queue_bytes).expect("queue json");
    assert_eq!(
        queue.get("schema").and_then(JsonValue::as_str),
        Some(GOVERNANCE_CAR_QUEUE_SCHEMA)
    );
    assert_eq!(
        queue.get("segment_count").and_then(JsonValue::as_u64),
        Some(1)
    );
    assert_eq!(
        queue.get("assembled_count").and_then(JsonValue::as_u64),
        Some(1)
    );
    let queue_digest_path = queue_path.with_extension("json.blake3");
    let queue_digest = fs::read_to_string(queue_digest_path).expect("read queue digest");
    assert_eq!(
        queue_digest.trim(),
        blake3::hash(&queue_bytes).to_hex().as_str()
    );
    let segment = queue
        .get("segments")
        .and_then(JsonValue::as_array)
        .and_then(|segments| segments.first())
        .and_then(JsonValue::as_object)
        .expect("first CAR segment");
    assert_eq!(
        segment.get("schema").and_then(JsonValue::as_str),
        Some(GOVERNANCE_CAR_SEGMENT_SCHEMA)
    );
    assert_eq!(
        segment.get("status").and_then(JsonValue::as_str),
        Some("assembled")
    );
    assert_eq!(
        segment
            .get("source_publish_index_position")
            .and_then(JsonValue::as_u64),
        Some(0)
    );
    assert_eq!(
        segment.get("encoded_blake3").and_then(JsonValue::as_str),
        Some(digest_hex.as_str())
    );
    let car_path = resolve_index_path(
        temp.path(),
        segment
            .get("car_path")
            .and_then(JsonValue::as_str)
            .expect("car path"),
    )
    .expect("resolve car path");
    let car_bytes = fs::read(&car_path).expect("read CAR segment");
    assert_eq!(
        segment.get("car_size").and_then(JsonValue::as_u64),
        Some(car_bytes.len() as u64)
    );
    assert_eq!(
        segment
            .get("car_archive_blake3")
            .and_then(JsonValue::as_str),
        Some(blake3::hash(&car_bytes).to_hex().as_str())
    );
    let car_digest =
        fs::read_to_string(digest_sidecar_path_for(&car_path)).expect("read car sidecar");
    assert_eq!(
        car_digest.trim(),
        blake3::hash(&car_bytes).to_hex().as_str()
    );

    let plan_path = resolve_index_path(
        temp.path(),
        segment
            .get("plan_path")
            .and_then(JsonValue::as_str)
            .expect("plan path"),
    )
    .expect("resolve plan path");
    let plan_bytes = fs::read(&plan_path).expect("read CAR plan");
    let plan: JsonValue = norito::json::from_slice(&plan_bytes).expect("plan json");
    assert_eq!(
        plan.get("schema").and_then(JsonValue::as_str),
        Some(GOVERNANCE_CAR_PLAN_SCHEMA)
    );
    assert_eq!(
        plan.get("source_publish_index_position")
            .and_then(JsonValue::as_u64),
        Some(0)
    );
    assert_eq!(
        plan.get("files")
            .and_then(JsonValue::as_array)
            .map(Vec::len),
        Some(4)
    );
    assert!(
        plan.get("chunks")
            .and_then(JsonValue::as_array)
            .is_some_and(|chunks| !chunks.is_empty()),
        "CAR plan should expose deterministic chunks"
    );
    let manifest_path = resolve_index_path(
        temp.path(),
        segment
            .get("manifest_path")
            .and_then(JsonValue::as_str)
            .expect("manifest path"),
    )
    .expect("resolve segment manifest path");
    let manifest_bytes = fs::read(&manifest_path).expect("read segment manifest");
    let manifest: JsonValue =
        norito::json::from_slice(&manifest_bytes).expect("segment manifest json");
    assert_eq!(
        manifest.get("schema").and_then(JsonValue::as_str),
        Some(GOVERNANCE_CAR_SEGMENT_SCHEMA)
    );

    publisher
        .publish_deal_settlement(&settlement, &encoded)
        .expect("republish same settlement");
    let index_bytes = fs::read(&index_path).expect("read republished index");
    let index: JsonValue = norito::json::from_slice(&index_bytes).expect("index json");
    assert_eq!(
        index.get("entry_count").and_then(JsonValue::as_u64),
        Some(1),
        "republishing the same artifact must not duplicate the index entry"
    );
    let queue_bytes = fs::read(&queue_path).expect("read republished queue");
    let queue: JsonValue = norito::json::from_slice(&queue_bytes).expect("queue json");
    assert_eq!(
        queue.get("segment_count").and_then(JsonValue::as_u64),
        Some(1),
        "republishing the same artifact must not duplicate the CAR queue segment"
    );
}

#[test]
fn filesystem_publisher_settlement_json_preserves_exact_wide_quantities() {
    let temp = tempdir().expect("tempdir");
    let publisher =
        FilesystemGovernancePublisher::try_new(temp.path().to_path_buf()).expect("publisher");
    let (mut settlement, _) = sample_settlement();
    let wide = xor("340282366920938463463374607431768211456");
    let sub_micro = xor("0.0000001");
    let applied = xor("0.00000004");
    let client_debit = xor("0.00000006");
    let slash = xor("0.000000001");
    let satisfied_without_outstanding = applied
        .checked_add(&client_debit)
        .and_then(|amount| amount.checked_add(&slash))
        .expect("fixture liability components");
    let outstanding = wide
        .checked_sub(&satisfied_without_outstanding)
        .expect("wide liability exceeds fixture payments");
    settlement.status = DealSettlementStatusV1::WindowSettled;
    settlement.ledger.deal_end_epoch = settlement.ledger.window_end_epoch + 10;
    settlement.ledger.provider_accrual = "0.0000001".parse().expect("sub-micro quantity");
    settlement.ledger.client_liability = wide.clone();
    settlement.ledger.micropayment_credit_generated = applied.clone();
    settlement.ledger.micropayment_credit_applied = applied.clone();
    settlement.ledger.micropayment_credit_carry = XorQuantity::zero();
    settlement.ledger.client_debit = client_debit.clone();
    settlement.ledger.outstanding_liability = outstanding;
    settlement.ledger.bond_total = xor("1.000000002");
    settlement.ledger.bond_locked = xor("1.000000001");
    settlement.ledger.bond_slashed = slash.clone();
    settlement.ledger.bond_released = XorQuantity::zero();
    settlement.ledger.window_expected_charge = wide;
    settlement.ledger.window_micropayment_generated = applied.clone();
    settlement.ledger.window_micropayment_applied = applied;
    settlement.ledger.window_client_debit = client_debit;
    settlement.ledger.window_bond_slashed = slash;
    settlement.ledger.window_bond_released = XorQuantity::zero();
    settlement.audit_notes = Some("exact wide-quantity settlement fixture".to_owned());
    assert_eq!(settlement.ledger.provider_accrual, sub_micro);
    settlement.ledger.snapshot_id = settlement.ledger.derive_snapshot_id().expect("ledger id");
    settlement.settlement_id = settlement.derive_settlement_id().expect("settlement id");
    settlement
        .validate_transition(None)
        .expect("coherent exact settlement fixture");
    let encoded = norito::to_bytes(&settlement).expect("encode canonical settlement");

    publisher
        .publish_deal_settlement(&settlement, &encoded)
        .expect("publish exact settlement");

    let dir = temp
        .path()
        .join("settlements")
        .join(settlement.deal_id.encode_hex::<String>());
    let json_path = fs::read_dir(dir)
        .expect("settlement directory")
        .map(|entry| entry.expect("dir entry").path())
        .find(|path| path.extension().is_some_and(|ext| ext == "json"))
        .expect("settlement json");
    let body = fs::read(json_path).expect("read settlement json");
    let value: JsonValue = json::from_slice(&body).expect("parse settlement json");
    let object = value
        .get("settlement")
        .and_then(JsonValue::as_object)
        .expect("settlement object");
    for (field, expected) in [
        ("provider_accrual", "0.0000001"),
        (
            "client_liability",
            "340282366920938463463374607431768211456",
        ),
        ("bond_locked", "1.000000001"),
        ("bond_slashed", "0.000000001"),
    ] {
        assert_eq!(
            object.get(field).and_then(JsonValue::as_str),
            Some(expected),
            "exact quantity field {field}"
        );
    }
    for retired in [
        "provider_accrual_micro",
        "client_liability_micro",
        "bond_locked_micro",
        "bond_slashed_micro",
    ] {
        assert!(!object.contains_key(retired), "retired field {retired}");
    }
}

#[test]
fn filesystem_publisher_rejects_malformed_car_queue() {
    let temp = tempdir().expect("tempdir");
    let publisher =
        FilesystemGovernancePublisher::try_new(temp.path().to_path_buf()).expect("publisher");
    let (settlement, encoded) = sample_settlement();
    fs::write(
        temp.path().join(GOVERNANCE_CAR_QUEUE_FILE),
        br#"{"schema":"wrong","segments":[]}"#,
    )
    .expect("write malformed queue");

    let err = publisher
        .publish_deal_settlement(&settlement, &encoded)
        .expect_err("malformed CAR queue must fail closed");
    assert!(
        err.to_string()
            .contains("governance CAR queue uses an unsupported schema"),
        "unexpected error: {err}"
    );
}

#[test]
fn atomic_temp_path_preserves_extensions_and_hides_file() {
    let base = Path::new("/tmp/settlement/artifact.norito.to");
    let tmp = temp_path_for_atomic(base, 42, 7);
    let tmp_name = tmp
        .file_name()
        .and_then(|name| name.to_str())
        .expect("name");
    assert!(
        tmp_name.starts_with(".artifact.norito.to.tmp-42-7"),
        "tmp name should keep extensions and add suffix, got {tmp_name}"
    );
    assert!(
        tmp.as_os_str()
            .to_string_lossy()
            .ends_with(".norito.to.tmp-42-7"),
        "tmp path should append to existing extensions"
    );
}

#[cfg(unix)]
#[test]
fn write_atomic_rejects_symlink_output() {
    let dir = tempdir().expect("tempdir");
    let temp_path = canonical_temp_path(&dir);
    let root_guard =
        GovernanceFilesystemRootGuard::capture_writer(&temp_path).expect("retain test root");
    let target_path = temp_path.join("target.to");
    fs::write(&target_path, b"unchanged\n").expect("write target");
    let output_path = temp_path.join("governance.to");
    std::os::unix::fs::symlink(&target_path, &output_path).expect("create symlink");

    let err =
        write_atomic(&root_guard, &output_path, b"replace").expect_err("reject symlink output");
    let message = err.to_string();

    assert!(
        message.contains("regular file") || message.contains("reparse"),
        "unexpected error: {message}"
    );
    assert_eq!(fs::read(&target_path).expect("read target"), b"unchanged\n");
}

#[test]
fn write_atomic_surfaces_post_rename_directory_sync_failure() {
    let dir = tempdir().expect("tempdir");
    let output_path = dir.path().join("governance.to");
    let error = write_atomic_with_directory_sync(&output_path, b"committed", |_| {
        Err(io::Error::other("injected directory sync failure"))
    })
    .expect_err("directory sync failure must be reported");

    assert!(
        error
            .to_string()
            .contains("injected directory sync failure")
    );
    assert_eq!(
        fs::read(&output_path).expect("renamed output remains visible"),
        b"committed",
        "the caller must treat this as committed-unknown and retry idempotently"
    );
}

#[cfg(unix)]
#[test]
fn write_atomic_rejects_symlink_parent() {
    let dir = tempdir().expect("tempdir");
    let temp_path = canonical_temp_path(&dir);
    let root_guard =
        GovernanceFilesystemRootGuard::capture_writer(&temp_path).expect("retain test root");
    let real_dir = temp_path.join("real");
    fs::create_dir(&real_dir).expect("create real dir");
    let linked_dir = temp_path.join("linked");
    std::os::unix::fs::symlink(&real_dir, &linked_dir).expect("create symlink");
    let output_path = linked_dir.join("governance.to");

    let err =
        write_atomic(&root_guard, &output_path, b"replace").expect_err("reject symlink parent");
    let message = err.to_string();

    assert!(
        message.contains("directory")
            || message.contains("symbolic")
            || message.contains("reparse"),
        "unexpected error: {message}"
    );
    assert!(
        !real_dir.join("governance.to").exists(),
        "symlink parent should not receive output"
    );
}

#[cfg(unix)]
#[test]
fn open_atomic_temp_file_rejects_preexisting_symlink() {
    let dir = tempdir().expect("tempdir");
    let temp_path = canonical_temp_path(&dir);
    let target_path = temp_path.join("target.tmp");
    fs::write(&target_path, b"unchanged\n").expect("write target");
    let tmp_path = temp_path.join(".governance.to.tmp");
    std::os::unix::fs::symlink(&target_path, &tmp_path).expect("create symlink");

    let err = open_atomic_temp_file(&tmp_path).expect_err("reject temp symlink");
    let message = err.to_string();

    assert!(
        message.contains("failed to create atomic temp"),
        "unexpected error: {message}"
    );
    assert_eq!(fs::read(&target_path).expect("read target"), b"unchanged\n");
}

#[test]
fn filesystem_publisher_writes_gc_audit_files() {
    let temp = tempdir().expect("tempdir");
    let publisher = signed_runtime_publisher(temp.path());

    let payload = GcAuditPayloadV1 {
        version: GC_AUDIT_PAYLOAD_VERSION_V1,
        manifest_digest: [0x33; 32],
        provider_id: [0x44; 32],
        evicted_at_unix: 1_700_000_333,
        freed_bytes: 4_096,
        reason: "retention_expired".into(),
        blocked_reason: None,
    };
    let header = SorafsAuditHeaderV1 {
        sequence: 7,
        occurred_at_unix: payload.evicted_at_unix,
        signer: GC_AUDIT_SIGNER_V1.into(),
        payload_digest: gc_audit_payload_digest_v1(&payload).expect("audit digest"),
    };
    let event = GcAuditEventV1 {
        version: GC_AUDIT_EVENT_VERSION_V1,
        header,
        payload,
    };
    let encoded = norito::to_bytes(&event).expect("encode GC audit event");

    publisher
        .publish_gc_audit_event(&event, &encoded)
        .expect("publish gc audit");

    let dir = temp.path().join("gc").join("audit");
    let entries = fs::read_dir(&dir)
        .expect("directory exists")
        .map(|entry| entry.expect("dir entry").path())
        .collect::<Vec<_>>();
    assert_eq!(entries.len(), 4, "expected encoded + json + digests");

    let json_path = entries
        .iter()
        .find(|path| path.extension().map(|ext| ext == "json").unwrap_or(false))
        .expect("json artefact present");
    let json_bytes = fs::read(json_path).expect("read json");
    let value: JsonValue = norito::json::from_slice(&json_bytes).expect("json should parse");
    let reason = value
        .get("metadata")
        .and_then(|meta| meta.get("reason"))
        .and_then(JsonValue::as_str)
        .expect("reason");
    assert_eq!(reason, "retention_expired");
    assert_single_runtime_external(temp.path(), "gc_audit", &encoded);
}

#[test]
fn filesystem_publisher_writes_reconciliation_report_files() {
    let temp = tempdir().expect("tempdir");
    let publisher = signed_runtime_publisher(temp.path());

    let report = SorafsReconciliationReportV1 {
        version: SORAFS_RECONCILIATION_REPORT_VERSION_V1,
        provider_id: [0x55; 32],
        generated_at_unix: 1_700_000_444,
        repair_snapshot_hash: [0x01; 32],
        retention_snapshot_hash: [0x02; 32],
        gc_snapshot_hash: [0x03; 32],
        repair_task_count: 2,
        retention_manifest_count: 3,
        gc_evictions_total: 4,
        gc_freed_bytes_total: 5,
        divergence_count: 1,
        appeal_finance: None,
    };
    let encoded = norito::to_bytes(&report).expect("encode reconciliation report");

    publisher
        .publish_reconciliation_report(&report, &encoded)
        .expect("publish reconciliation report");

    let dir = temp.path().join("reconciliation");
    let entries = fs::read_dir(&dir)
        .expect("directory exists")
        .map(|entry| entry.expect("dir entry").path())
        .collect::<Vec<_>>();
    assert_eq!(entries.len(), 4, "expected encoded + json + digests");

    let json_path = entries
        .iter()
        .find(|path| path.extension().map(|ext| ext == "json").unwrap_or(false))
        .expect("json artefact present");
    let json_bytes = fs::read(json_path).expect("read json");
    let value: JsonValue = norito::json::from_slice(&json_bytes).expect("json should parse");
    let metadata = value
        .get("metadata")
        .and_then(JsonValue::as_object)
        .expect("metadata");
    let provider = metadata
        .get("provider")
        .and_then(JsonValue::as_str)
        .expect("provider");
    let divergence = metadata
        .get("divergence_count")
        .and_then(JsonValue::as_u64)
        .expect("divergence_count");
    assert_eq!(provider, hex::encode(report.provider_id));
    assert_eq!(divergence, 1);
    assert_single_runtime_external(temp.path(), "reconciliation", &encoded);
}

#[test]
fn filesystem_publisher_writes_reputation_snapshot_files() {
    let temp = tempdir().expect("tempdir");
    let publisher =
        FilesystemGovernancePublisher::try_new(temp.path().to_path_buf()).expect("publisher");
    let (snapshot, encoded) = sample_reputation_snapshot();

    publisher
        .publish_reputation_snapshot(&snapshot, &encoded)
        .expect("publish reputation snapshot");

    let snapshot_hex = hex::encode(snapshot.snapshot.snapshot_id);
    let dir = temp
        .path()
        .join("reputation")
        .join("snapshots")
        .join(&snapshot_hex);
    let entries = fs::read_dir(&dir)
        .expect("snapshot directory exists")
        .map(|entry| entry.expect("dir entry").path())
        .collect::<Vec<_>>();
    assert_eq!(entries.len(), 4, "expected encoded + json + digests");

    let latest_to = temp.path().join("reputation").join("latest.to");
    assert_eq!(
        fs::read(&latest_to).expect("read latest reputation snapshot"),
        encoded,
        "latest pointer must contain canonical Norito bytes"
    );

    let latest_json = temp.path().join("reputation").join("latest.json");
    let json_bytes = fs::read(latest_json).expect("read latest reputation json");
    let value: JsonValue = norito::json::from_slice(&json_bytes).expect("json should parse");
    let metadata = value
        .get("metadata")
        .and_then(JsonValue::as_object)
        .expect("metadata");
    assert_eq!(
        metadata.get("snapshot_id_hex").and_then(JsonValue::as_str),
        Some(snapshot_hex.as_str())
    );
    assert_eq!(
        metadata.get("provider_count").and_then(JsonValue::as_u64),
        Some(snapshot.snapshot.providers.len() as u64)
    );
}
