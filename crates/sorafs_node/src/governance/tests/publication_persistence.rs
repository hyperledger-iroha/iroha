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

    let (encoded_path, json_path) = only_published_source_paths(temp.path(), "deal_settlement");
    assert_eq!(
        fs::read(&encoded_path).expect("read encoded"),
        encoded,
        "encoded payload must match original bytes"
    );

    let json_bytes = fs::read(&json_path).expect("read json");
    let value: JsonValue = norito::json::from_slice(&json_bytes).expect("json should parse");
    let status = value
        .get("metadata")
        .and_then(|meta| meta.get("status"))
        .and_then(JsonValue::as_str)
        .expect("status");
    assert_eq!(status, "completed");

    let encoded_digest =
        fs::read_to_string(digest_sidecar_path_for(&encoded_path)).expect("read encoded digest");
    let encoded_digest = encoded_digest.trim();
    assert_eq!(encoded_digest, blake3::hash(&encoded).to_hex().as_str());

    let json_digest =
        fs::read_to_string(digest_sidecar_path_for(&json_path)).expect("read json digest");
    let json_digest = json_digest.trim();
    assert_eq!(json_digest, blake3::hash(&json_bytes).to_hex().as_str());

    let publication_path = temp.path().join(GOVERNANCE_PUBLICATION_STATE_FILE);
    let publication_bytes = fs::read(&publication_path).expect("read publication state");
    let publication: JsonValue =
        norito::json::from_slice(&publication_bytes).expect("publication state json");
    assert_eq!(
        publication.get("schema").and_then(JsonValue::as_str),
        Some(GOVERNANCE_PUBLICATION_STATE_SCHEMA)
    );
    assert_eq!(
        publication.get("generation").and_then(JsonValue::as_u64),
        Some(1)
    );
    let index = publication
        .get("publish_index")
        .cloned()
        .expect("nested publish index");
    assert_eq!(
        index.get("schema").and_then(JsonValue::as_str),
        Some(GOVERNANCE_PUBLISH_INDEX_SCHEMA)
    );
    assert_eq!(
        index.get("root").and_then(JsonValue::as_str),
        Some(GOVERNANCE_DAG_LOGICAL_ROOT)
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
        Some(index_path_string(temp.path(), &encoded_path).as_str())
    );
    assert_eq!(
        entry.get("json_len").and_then(JsonValue::as_u64),
        Some(json_bytes.len() as u64)
    );
    assert_eq!(
        entry.get("json_blake3").and_then(JsonValue::as_str),
        Some(blake3::hash(&json_bytes).to_hex().as_str())
    );
    assert_eq!(
        entry
            .get("labels")
            .and_then(JsonValue::as_object)
            .and_then(|labels| labels.get("status"))
            .and_then(JsonValue::as_str),
        Some("completed")
    );
    assert!(!temp.path().join(GOVERNANCE_PUBLISH_INDEX_FILE).exists());
    assert!(!temp.path().join(GOVERNANCE_CAR_QUEUE_FILE).exists());

    let queue = publication
        .get("car_queue")
        .cloned()
        .expect("nested CAR queue");
    assert_eq!(
        queue.get("schema").and_then(JsonValue::as_str),
        Some(GOVERNANCE_CAR_QUEUE_SCHEMA)
    );
    assert_eq!(
        queue.get("root").and_then(JsonValue::as_str),
        Some(GOVERNANCE_DAG_LOGICAL_ROOT)
    );
    assert_eq!(
        queue.get("segment_count").and_then(JsonValue::as_u64),
        Some(1)
    );
    assert_eq!(
        queue.get("assembled_count").and_then(JsonValue::as_u64),
        Some(1)
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
    let car_archive_digest_hex = blake3::hash(&car_bytes).to_hex().to_string();
    assert_eq!(
        segment.get("car_size").and_then(JsonValue::as_u64),
        Some(car_bytes.len() as u64)
    );
    assert_eq!(
        segment
            .get("car_archive_blake3")
            .and_then(JsonValue::as_str),
        Some(car_archive_digest_hex.as_str())
    );
    let archive_positions = queue
        .get("by_car_archive_blake3")
        .and_then(JsonValue::as_object)
        .and_then(|map| map.get(car_archive_digest_hex.as_str()))
        .and_then(JsonValue::as_array)
        .expect("CAR archive digest lookup");
    assert_eq!(archive_positions.as_slice(), [JsonValue::from(0_u64)]);
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
    assert!(manifest_bytes.len() <= GOVERNANCE_CAR_SEGMENT_MANIFEST_MAX_BYTES_V1);
    let manifest: JsonValue =
        norito::json::from_slice(&manifest_bytes).expect("segment manifest json");
    assert_eq!(
        manifest.get("schema").and_then(JsonValue::as_str),
        Some(GOVERNANCE_CAR_SEGMENT_SCHEMA)
    );

    publisher
        .publish_deal_settlement(&settlement, &encoded)
        .expect("exact duplicate publication is a no-op");
    assert_eq!(
        fs::read(&publication_path).expect("reread publication state after duplicate"),
        publication_bytes,
        "an exact duplicate must not advance or rewrite the authority envelope"
    );
    assert_eq!(
        fs::read(&car_path).expect("reread duplicate CAR"),
        car_bytes
    );

    fs::write(&car_path, b"substituted archive").expect("substitute retained CAR artifact");
    fs::write(
        digest_sidecar_path_for(&car_path),
        format!("{}\n", blake3::hash(b"substituted archive").to_hex()),
    )
    .expect("substitute retained CAR sidecar");
    let error = publisher
        .publish_deal_settlement(&settlement, &encoded)
        .expect_err("duplicate publication must reject a substituted immutable CAR");
    assert!(
        error.to_string().contains("occupied by different bytes"),
        "unexpected duplicate substitution error: {error}"
    );
    assert_eq!(
        fs::read(&publication_path).expect("reread authority after substituted duplicate"),
        publication_bytes,
        "a rejected duplicate must leave the authority envelope unchanged"
    );
    assert_eq!(
        fs::read(&car_path).expect("read rejected substituted CAR"),
        b"substituted archive",
        "the publisher must not conceal immutable-artifact substitution by overwriting it"
    );

    let publication = read_publication_state_fixture(temp.path());
    assert_eq!(
        publication.get("generation").and_then(JsonValue::as_u64),
        Some(1)
    );
    let index = publication
        .get("publish_index")
        .expect("republished nested index");
    assert_eq!(
        index.get("entry_count").and_then(JsonValue::as_u64),
        Some(1),
        "duplicate attempts must not duplicate the index entry"
    );
    let queue = publication
        .get("car_queue")
        .expect("republished nested queue");
    assert_eq!(
        queue.get("segment_count").and_then(JsonValue::as_u64),
        Some(1),
        "duplicate attempts must not duplicate the CAR queue segment"
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

    let (_, json_path) = only_published_source_paths(temp.path(), "deal_settlement");
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
fn filesystem_publisher_rejects_legacy_separate_car_queue_authority() {
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
        .expect_err("legacy CAR queue authority must fail closed");
    assert!(
        err.to_string()
            .contains("legacy governance publication authority"),
        "unexpected error: {err}"
    );
}

#[test]
fn filesystem_publisher_rejects_malformed_publication_authority_before_artifact_writes() {
    let temp = tempdir().expect("tempdir");
    fs::write(
        temp.path().join(GOVERNANCE_PUBLICATION_STATE_FILE),
        br#"{"schema":"substituted"}"#,
    )
    .expect("write malformed authoritative publication state");

    let error = FilesystemGovernancePublisher::try_new(temp.path().to_path_buf())
        .expect_err("malformed authority must reject publisher startup");
    assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    assert!(
        !temp
            .path()
            .join(GOVERNANCE_PUBLICATION_SOURCES_DIR)
            .exists(),
        "startup validation must not create immutable source artifacts"
    );
    assert!(
        !temp.path().join(GOVERNANCE_CAR_SEGMENTS_DIR).exists(),
        "startup validation must not create immutable CAR artifacts"
    );
}

#[test]
fn filesystem_publisher_reclaims_bounded_uncommitted_artifacts_at_startup() {
    let temp = tempdir().expect("tempdir");
    drop(
        FilesystemGovernancePublisher::try_new(temp.path().to_path_buf())
            .expect("initialize empty publication authority"),
    );
    {
        let root_guard = GovernanceFilesystemRootGuard::capture_writer(temp.path())
            .expect("retain publication root");
        let orphan = write_car_segment_source_fixture(temp.path(), b"orphan-publication");
        assemble_governance_car_queue(
            temp.path(),
            &root_guard,
            empty_governance_car_queue(),
            &orphan,
        )
        .expect("assemble orphan CAR artifacts");
        let source_directory = temp
            .path()
            .join(&orphan.encoded_path)
            .parent()
            .expect("source pair parent")
            .to_path_buf();
        fs::write(
            source_directory.join(".payload.to.tmp-42000-1"),
            b"interrupted source temp",
        )
        .expect("seed interrupted source temp");
        let car_base = temp
            .path()
            .join(governance_car_segment_relative_base(&orphan).expect("CAR base"));
        let car_target = car_base.with_extension("car");
        let car_target_name = car_target
            .file_name()
            .and_then(OsStr::to_str)
            .expect("canonical CAR target name");
        fs::write(
            car_target
                .parent()
                .expect("CAR parent")
                .join(format!(".{car_target_name}.tmp-42000-2")),
            b"interrupted CAR temp",
        )
        .expect("seed interrupted CAR temp");
    }
    assert!(
        temp.path()
            .join(GOVERNANCE_PUBLICATION_SOURCES_DIR)
            .exists()
    );
    assert!(temp.path().join(GOVERNANCE_CAR_SEGMENTS_DIR).exists());

    let publisher = FilesystemGovernancePublisher::try_new(temp.path().to_path_buf())
        .expect("startup reconciles one bounded interrupted publication");
    assert!(
        !temp
            .path()
            .join(GOVERNANCE_PUBLICATION_SOURCES_DIR)
            .exists(),
        "unreferenced source files and their empty directories must be reclaimed"
    );
    assert!(
        !temp.path().join(GOVERNANCE_CAR_SEGMENTS_DIR).exists(),
        "unreferenced CAR files and their empty directory must be reclaimed"
    );
    drop(publisher);
}

#[test]
fn filesystem_publisher_reclaims_interrupted_authority_temp_at_startup() {
    let temp = tempdir().expect("tempdir");
    let stale_temp = temp
        .path()
        .join(format!(".{GOVERNANCE_PUBLICATION_STATE_FILE}.tmp-42000-1"));
    let stale_marker_temp = temp.path().join(format!(
        ".{GOVERNANCE_PUBLICATION_INITIALIZED_FILE}.tmp-42000-2"
    ));
    fs::write(&stale_temp, b"interrupted authoritative state")
        .expect("seed interrupted authority temp");
    fs::write(&stale_marker_temp, b"interrupted initialization marker")
        .expect("seed interrupted marker temp");

    let publisher = FilesystemGovernancePublisher::try_new(temp.path().to_path_buf())
        .expect("startup reclaims interrupted authority temp");
    assert!(
        !stale_temp.exists(),
        "the canonical authoritative-state temp must be reclaimed before startup reads"
    );
    assert!(
        !stale_marker_temp.exists(),
        "the canonical initialization-marker temp must be reclaimed before startup reads"
    );
    drop(publisher);
}

#[test]
fn filesystem_publisher_persists_explicit_empty_authority_and_marker() {
    let temp = tempdir().expect("tempdir");
    let publisher = FilesystemGovernancePublisher::try_new(temp.path().to_path_buf())
        .expect("initialize publication authority");
    assert!(
        temp.path()
            .join(GOVERNANCE_PUBLICATION_STATE_FILE)
            .is_file(),
        "a pristine root must gain an explicit empty authority"
    );
    assert_eq!(
        fs::read(temp.path().join(GOVERNANCE_PUBLICATION_INITIALIZED_FILE))
            .expect("read initialization marker"),
        GOVERNANCE_PUBLICATION_INITIALIZED_BODY
    );
    let state = read_publication_state_fixture(temp.path());
    assert_eq!(state.get("generation").and_then(JsonValue::as_u64), Some(0));
    drop(publisher);
}

#[test]
fn filesystem_publisher_rejects_missing_authority_without_deleting_history() {
    let temp = tempdir().expect("tempdir");
    let publisher =
        FilesystemGovernancePublisher::try_new(temp.path().to_path_buf()).expect("publisher");
    let (settlement, encoded) = sample_settlement();
    publisher
        .publish_deal_settlement(&settlement, &encoded)
        .expect("publish committed settlement");
    let (encoded_path, json_path) = only_published_source_paths(temp.path(), "deal_settlement");
    let state = read_publication_state_fixture(temp.path());
    let car_paths = state
        .get("car_queue")
        .and_then(|queue| queue.get("segments"))
        .and_then(JsonValue::as_array)
        .and_then(|segments| segments.first())
        .and_then(JsonValue::as_object)
        .map(|segment| {
            ["car_path", "plan_path", "manifest_path"].map(|field| {
                temp.path().join(
                    segment
                        .get(field)
                        .and_then(JsonValue::as_str)
                        .expect("committed CAR artifact path"),
                )
            })
        })
        .expect("committed CAR segment");
    drop(publisher);
    fs::remove_file(temp.path().join(GOVERNANCE_PUBLICATION_STATE_FILE))
        .expect("remove authority fixture");

    let error = FilesystemGovernancePublisher::try_new(temp.path().to_path_buf())
        .expect_err("missing initialized authority must fail closed");
    assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    assert!(error.to_string().contains("state is missing"));
    for path in [encoded_path, json_path].into_iter().chain(car_paths) {
        assert!(
            path.is_file(),
            "missing authority must not reclaim committed artifact `{}`",
            path.display()
        );
    }
}

#[test]
fn filesystem_publisher_rejects_authority_bound_source_corruption_at_startup() {
    let temp = tempdir().expect("tempdir");
    let publisher =
        FilesystemGovernancePublisher::try_new(temp.path().to_path_buf()).expect("publisher");
    let (settlement, encoded) = sample_settlement();
    publisher
        .publish_deal_settlement(&settlement, &encoded)
        .expect("publish committed settlement");
    let (encoded_path, _) = only_published_source_paths(temp.path(), "deal_settlement");
    drop(publisher);

    let substituted = b"substituted committed source";
    fs::write(&encoded_path, substituted).expect("substitute committed source");
    fs::write(
        digest_sidecar_path_for(&encoded_path),
        format!("{}\n", blake3::hash(substituted).to_hex()),
    )
    .expect("substitute matching unauthoritative sidecar");

    let error = FilesystemGovernancePublisher::try_new(temp.path().to_path_buf())
        .expect_err("authority-bound source corruption must fail startup");
    assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    assert!(
        error.to_string().contains("does not match publish-index")
            || error.to_string().contains("canonical source projection"),
        "unexpected error: {error}"
    );
    assert_eq!(
        fs::read(&encoded_path).expect("read preserved substituted source"),
        substituted,
        "startup must fail closed without rewriting corrupted immutable history"
    );
}

#[test]
fn filesystem_publisher_rejects_authority_bound_car_corruption_at_startup() {
    let temp = tempdir().expect("tempdir");
    let publisher =
        FilesystemGovernancePublisher::try_new(temp.path().to_path_buf()).expect("publisher");
    let (settlement, encoded) = sample_settlement();
    publisher
        .publish_deal_settlement(&settlement, &encoded)
        .expect("publish committed settlement");
    let car_path = read_publication_state_fixture(temp.path())
        .get("car_queue")
        .and_then(|queue| queue.get("segments"))
        .and_then(JsonValue::as_array)
        .and_then(|segments| segments.first())
        .and_then(|segment| segment.get("car_path"))
        .and_then(JsonValue::as_str)
        .map(|path| temp.path().join(path))
        .expect("committed CAR path");
    drop(publisher);

    let substituted = b"substituted committed CAR";
    fs::write(&car_path, substituted).expect("substitute committed CAR");
    fs::write(
        digest_sidecar_path_for(&car_path),
        format!("{}\n", blake3::hash(substituted).to_hex()),
    )
    .expect("substitute matching unauthoritative CAR sidecar");

    let error = FilesystemGovernancePublisher::try_new(temp.path().to_path_buf())
        .expect_err("authority-bound CAR corruption must fail startup");
    assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    assert!(
        error.to_string().contains("authoritative canonical bytes"),
        "unexpected error: {error}"
    );
    assert_eq!(
        fs::read(&car_path).expect("read preserved substituted CAR"),
        substituted,
        "startup must fail closed without rewriting corrupted immutable history"
    );
}

#[test]
fn filesystem_publisher_rejects_missing_committed_publication_artifacts_at_startup() {
    let temp = tempdir().expect("tempdir");
    let publisher =
        FilesystemGovernancePublisher::try_new(temp.path().to_path_buf()).expect("publisher");
    let (settlement, encoded) = sample_settlement();
    publisher
        .publish_deal_settlement(&settlement, &encoded)
        .expect("publish committed settlement");
    let state = read_publication_state_fixture(temp.path());
    let car_path = state
        .get("car_queue")
        .and_then(|queue| queue.get("segments"))
        .and_then(JsonValue::as_array)
        .and_then(|segments| segments.first())
        .and_then(|segment| segment.get("car_path"))
        .and_then(JsonValue::as_str)
        .expect("committed CAR path")
        .to_owned();
    drop(publisher);
    fs::remove_file(temp.path().join(car_path)).expect("remove committed CAR artifact");

    let error = FilesystemGovernancePublisher::try_new(temp.path().to_path_buf())
        .expect_err("startup must reject a missing committed artifact");
    assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    assert!(
        error
            .to_string()
            .contains("committed governance CAR artifacts are missing")
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

    let (_, json_path) = only_published_source_paths(temp.path(), "gc_audit");
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

    let (_, json_path) = only_published_source_paths(temp.path(), "reconciliation");
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
    let (_, json_path) = only_published_source_paths(temp.path(), "reputation_snapshot");
    assert!(!temp.path().join("reputation").join("latest.to").exists());
    assert!(!temp.path().join("reputation").join("latest.json").exists());
    let json_bytes = fs::read(json_path).expect("read reputation json");
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

#[test]
fn reputation_snapshot_metadata_supports_the_full_encoded_bound_without_payload_duplication() {
    let (snapshot, _) = sample_reputation_snapshot();
    let digest_hex = "a5".repeat(32);
    let body = reputation_snapshot_json(
        &snapshot,
        GOVERNANCE_CAR_SOURCE_ENCODED_MAX_BYTES,
        &digest_hex,
    )
    .expect("project maximum-length snapshot metadata");
    assert!(
        body.len() <= GOVERNANCE_CAR_SOURCE_JSON_MAX_BYTES,
        "bounded metadata must fit the JSON source limit"
    );
    let value: JsonValue = json::from_str(&body).expect("decode snapshot metadata");
    assert_eq!(
        value.get("schema").and_then(JsonValue::as_str),
        Some("sorafs.reputation_snapshot.metadata.v1")
    );
    assert!(
        value.get("signed_snapshot").is_none(),
        "the canonical payload belongs only in payload.to"
    );
    let metadata = value
        .get("metadata")
        .and_then(JsonValue::as_object)
        .expect("snapshot metadata");
    assert_eq!(
        metadata.get("encoded_len").and_then(JsonValue::as_u64),
        Some(GOVERNANCE_CAR_SOURCE_ENCODED_MAX_BYTES as u64)
    );
    assert!(
        !metadata.contains_key("encoded_base64"),
        "JSON metadata must not duplicate the canonical encoded payload"
    );
}
